//! Founding factions (W7): population segments carried within one campaign.
//!
//! Factions are groups of people *aboard* — orthogonal to the campaign-level
//! legacy (preservers/adaptors/wanderers), which is unchanged. Structure plus
//! roster change (loss/merger/recruit), log/event coloring, and a one-time
//! recruitment dowry per people (content-depth round 7). No *ongoing* approval
//! meters yet — those layer on later.

use serde::{Deserialize, Serialize};

use crate::data::factions::FactionDef;
use crate::data::GameData;
use crate::state::sim::SimState;
use macroquad_toolkit::data_loader::DataRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionStatus {
    Aboard,
    WipedOut,
    Settled,
    Departed,
    Assimilated,
}

impl FactionStatus {
    pub fn label(self) -> &'static str {
        match self {
            FactionStatus::Aboard => "Aboard",
            FactionStatus::WipedOut => "Wiped out",
            FactionStatus::Settled => "Settled off-ship",
            FactionStatus::Departed => "Departed",
            FactionStatus::Assimilated => "Assimilated",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionState {
    pub faction_id: String,
    pub members: u32,
    pub status: FactionStatus,
    /// How content this people is with how the ship has treated them (content-depth
    /// factions round 8): 0 (embittered) .. 1 (devoted), 0.5 at launch. Event
    /// choices shift it (`EventOutcome::faction_approval_deltas`), and a people
    /// slighted past a threshold becomes eligible for its own withdrawal — so
    /// *how you treat a faction*, not only how far the voyage has drifted,
    /// decides whether it stays. `#[serde(default)]` keeps old saves loading at
    /// the neutral midpoint.
    #[serde(default = "default_approval")]
    pub approval: f32,
    /// The sentiment band last announced to the log (content-depth voice round 8):
    /// -1 restless, 0 neutral, +1 devoted. Lets the yearly mood check surface a
    /// people crossing *into* restlessness or contentment exactly once, rather
    /// than reprinting every year it stays there. 0 (neutral) at launch.
    #[serde(default)]
    pub mood_band: i8,
}

/// Launch/neutral approval — a people that neither loves nor resents the ship yet.
pub fn default_approval() -> f32 {
    0.5
}

/// The sentiment band for an approval value (content-depth voice round 8):
/// restless at/below the withdrawal-danger line, devoted up high, neutral between.
pub fn mood_band_for(approval: f32) -> i8 {
    if approval <= 0.3 {
        -1
    } else if approval >= 0.7 {
        1
    } else {
        0
    }
}

/// The band of institutional order for a stability value (content-depth voice round
/// 17), given the governance-voice thresholds: firm (+1) at/above `high`, fraying
/// (-1) at/below `low`, steady (0) between. Shared by the launch-band record and the
/// yearly announcement so both read the same bands.
pub fn stability_voice_band_for(stability: f32, high: f32, low: f32) -> i8 {
    if stability >= high {
        1
    } else if stability <= low {
        -1
    } else {
        0
    }
}

impl FactionState {
    pub fn is_aboard(&self) -> bool {
        self.status == FactionStatus::Aboard
    }

    /// Shift approval by `delta`, clamped to [0, 1].
    pub fn adjust_approval(&mut self, delta: f32) {
        self.approval = (self.approval + delta).clamp(0.0, 1.0);
    }
}

/// A faction's pretty log name, falling back to its id if the def is missing.
pub fn log_name(registry: &DataRegistry<FactionDef>, id: &str) -> String {
    registry
        .get(id)
        .map(|f| f.log_name.clone())
        .unwrap_or_else(|| id.to_owned())
}

/// Split `total` people across the chosen factions as evenly as possible, the
/// remainder falling to the first (W7 founding).
pub fn build_founding_factions(faction_ids: &[String], total: u32) -> Vec<FactionState> {
    let n = faction_ids.len() as u32;
    if n == 0 {
        return Vec::new();
    }
    let base = total / n;
    let remainder = total % n;
    faction_ids
        .iter()
        .enumerate()
        .map(|(i, id)| FactionState {
            faction_id: id.clone(),
            members: base + if (i as u32) < remainder { 1 } else { 0 },
            status: FactionStatus::Aboard,
            approval: default_approval(),
            mood_band: 0,
        })
        .collect()
}

mod announce;
mod roster;
mod sentiment;

#[cfg(test)]
mod tests;

impl SimState {
    /// Indices of the factions still aboard.
    fn aboard_indices(&self) -> Vec<usize> {
        (0..self.factions.len())
            .filter(|&i| self.factions[i].is_aboard())
            .collect()
    }
    /// Aboard factions still on the ship.
    pub fn aboard_faction_count(&self) -> u32 {
        self.factions.iter().filter(|f| f.is_aboard()).count() as u32
    }
    /// The id of the largest aboard faction — "who runs the ship" for
    /// faction-colored event gating (content-depth iteration). Ties break on id
    /// for determinism. `None` when no faction is aboard.
    pub fn dominant_faction_id(&self) -> Option<&str> {
        self.factions
            .iter()
            .filter(|f| f.is_aboard())
            .max_by(|a, b| {
                a.members
                    .cmp(&b.members)
                    .then_with(|| b.faction_id.cmp(&a.faction_id))
            })
            .map(|f| f.faction_id.as_str())
    }
    /// Whether a specific faction is still aboard (for inter-faction friction
    /// event gating).
    pub fn is_faction_aboard(&self, id: &str) -> bool {
        self.factions
            .iter()
            .any(|f| f.faction_id == id && f.is_aboard())
    }
    /// Faction ids that could still be recruited: known factions that have never
    /// been part of this campaign (chosen or lost). Sorted for a stable menu.
    pub fn recruitable_faction_ids(&self, data: &GameData) -> Vec<String> {
        let mut ids: Vec<String> = data
            .factions
            .ids()
            .filter(|id| !self.factions.iter().any(|f| &f.faction_id == *id))
            .cloned()
            .collect();
        ids.sort();
        ids
    }
    /// The member-weighted mean approval of the aboard peoples (content-depth
    /// factions round 15): the ship's overall political mood, so a large content
    /// majority weighs more than a small soured minority. `0.5` (neutral) when no
    /// people is aboard. Drives the faction→unity cohesion coupling.
    pub fn aboard_approval_mean(&self) -> f32 {
        let mut total_members = 0u64;
        let mut weighted = 0.0f32;
        for f in &self.factions {
            if f.is_aboard() && f.members > 0 {
                total_members += f.members as u64;
                weighted += f.approval * f.members as f32;
            }
        }
        if total_members == 0 {
            0.5
        } else {
            weighted / total_members as f32
        }
    }
    /// The member-weighted ideological *spread* of the aboard peoples (content-depth
    /// factions round 18): the mean absolute deviation of their `ideology` from the
    /// member-weighted mean — how ideologically *divided* the polity is. `0` for a
    /// single-minded ship (one people, or peoples that all think alike), rising as the
    /// roster spans the tech-embracing↔tradition-bound spectrum. A wide spread is a
    /// coalition harder to govern; it drives the faction→stability coupling. Reads the
    /// catalog ideology (constant) and the living roster; deterministic, no RNG.
    pub fn aboard_ideology_spread(&self, data: &GameData) -> f32 {
        let members: Vec<(f32, f32)> = self
            .factions
            .iter()
            .filter(|f| f.is_aboard() && f.members > 0)
            .filter_map(|f| {
                data.factions
                    .get(&f.faction_id)
                    .map(|d| (d.ideology, f.members as f32))
            })
            .collect();
        let total: f32 = members.iter().map(|(_, m)| m).sum();
        if total <= 0.0 {
            return 0.0;
        }
        let mean = members.iter().map(|(i, m)| i * m).sum::<f32>() / total;
        members
            .iter()
            .map(|(i, m)| (i - mean).abs() * m)
            .sum::<f32>()
            / total
    }
    /// The approval of the aboard people that tends `subsystem_id` (content-depth
    /// factions round 12), or `None` if no aboard faction tends it. The upkeep
    /// half of the tended-subsystem coupling: `apply_subsystem_neglect_sentiment`
    /// runs neglect → sentiment, this feeds sentiment → decay (via
    /// `decay_subsystems`). Deterministic; the first aboard tender in roster order.
    pub fn tender_approval(&self, data: &GameData, subsystem_id: &str) -> Option<f32> {
        self.factions.iter().find_map(|fstate| {
            if !fstate.is_aboard() {
                return None;
            }
            let def = data.factions.get(&fstate.faction_id)?;
            (def.tended_subsystem == subsystem_id).then_some(fstate.approval)
        })
    }
}
