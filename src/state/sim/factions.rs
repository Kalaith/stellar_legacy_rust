//! Founding factions (W7): population segments carried within one campaign.
//!
//! Factions are groups of people *aboard* — orthogonal to the campaign-level
//! legacy (preservers/adaptors/wanderers), which is unchanged. Structure plus
//! roster change (loss/merger/recruit), log/event coloring, and a one-time
//! recruitment dowry per people (content-depth round 7). No *ongoing* approval
//! meters yet — those layer on later.

use serde::{Deserialize, Serialize};

use crate::data::factions::{FactionDef, FactionLossKind};
use crate::data::{FlavorConfig, GameData, ResourceDelta};
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

    /// Proportionally rescale Aboard members to the current `population.count`
    /// with largest-remainder rounding (W7), keeping the share invariant
    /// `sum(Aboard members) == population.count`. A faction rescaled to zero
    /// while others survive is marked WipedOut; its id is returned so the caller
    /// can log it with the faction's pretty name.
    pub fn rebalance_factions(&mut self) -> Vec<String> {
        let aboard = self.aboard_indices();
        if aboard.is_empty() {
            return Vec::new();
        }
        let old_total: u32 = aboard.iter().map(|&i| self.factions[i].members).sum();
        let target = self.population.count;

        if old_total == 0 {
            // Degenerate (guarded against elsewhere): seat everyone in the first.
            for (k, &i) in aboard.iter().enumerate() {
                self.factions[i].members = if k == 0 { target } else { 0 };
            }
        } else {
            let mut assigned = 0u32;
            let mut remainders: Vec<(usize, f64)> = Vec::with_capacity(aboard.len());
            for &i in &aboard {
                let exact = self.factions[i].members as f64 / old_total as f64 * target as f64;
                let floor = exact.floor() as u32;
                self.factions[i].members = floor;
                assigned += floor;
                remainders.push((i, exact - floor as f64));
            }
            // Distribute the leftover to the largest remainders, breaking ties on
            // faction id so the outcome is deterministic.
            remainders.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        self.factions[a.0]
                            .faction_id
                            .cmp(&self.factions[b.0].faction_id)
                    })
            });
            let mut leftover = target.saturating_sub(assigned);
            for &(i, _) in &remainders {
                if leftover == 0 {
                    break;
                }
                self.factions[i].members += 1;
                leftover -= 1;
            }
        }

        // Any faction rescaled to nothing while others survive is gone for good.
        let survivors = aboard
            .iter()
            .filter(|&&i| self.factions[i].members > 0)
            .count();
        let mut wiped = Vec::new();
        if survivors > 0 {
            for &i in &aboard {
                if self.factions[i].members == 0 {
                    self.factions[i].status = FactionStatus::WipedOut;
                    wiped.push(self.factions[i].faction_id.clone());
                }
            }
        }
        wiped
    }

    /// Remove the smallest Aboard faction from the ship (W7 event-driven loss:
    /// they settled off-ship or departed on their own course). Ties break on the
    /// lexicographically-first id. If only one faction is Aboard this is a
    /// near-miss — the ship never loses its last people this way (extinction is
    /// the succession system's job).
    pub fn apply_faction_loss(&mut self, data: &GameData, kind: FactionLossKind) {
        let aboard = self.aboard_indices();
        if aboard.len() <= 1 {
            self.push_log(
                "A faction talked of breaking away, but with the ship's last people aboard, \
                 they stayed.",
            );
            return;
        }
        let idx = *aboard
            .iter()
            .min_by(|&&a, &&b| {
                self.factions[a]
                    .members
                    .cmp(&self.factions[b].members)
                    .then_with(|| {
                        self.factions[a]
                            .faction_id
                            .cmp(&self.factions[b].faction_id)
                    })
            })
            .expect("aboard is non-empty");

        self.remove_faction(idx, kind, data);
    }

    /// Remove a *named* faction from the ship (content-depth round 3: faction-
    /// specific schism beats). Unlike `apply_faction_loss`, which sheds whoever
    /// is smallest, this loses the faction the event is actually about — but
    /// still never the ship's last aboard people, and never a no-op silent when
    /// the named faction has already gone.
    pub fn apply_faction_loss_by_id(&mut self, data: &GameData, kind: FactionLossKind, id: &str) {
        if self.aboard_faction_count() <= 1 {
            self.push_log(
                "A faction talked of breaking away, but with the ship's last people aboard, \
                 they stayed.",
            );
            return;
        }
        match self
            .factions
            .iter()
            .position(|f| f.faction_id == id && f.is_aboard())
        {
            Some(idx) => self.remove_faction(idx, kind, data),
            None => self.push_log(
                "The talk of a schism came to nothing — those who might have led it were \
                 already gone.",
            ),
        }
    }

    /// Merge a *named* faction into the largest other aboard (content-depth
    /// round 5: event-driven assimilation, the union counterpart to
    /// `apply_faction_loss_by_id`). Unlike a schism, the people stay — the head
    /// count is untouched, only the separate identity dissolves as its members
    /// fold into the host. No-op if the named faction is not aboard, or is the
    /// ship's last aboard people (nothing to fold it into).
    pub fn apply_faction_merge(&mut self, data: &GameData, id: &str) {
        if self.aboard_faction_count() <= 1 {
            self.push_log(
                "There was talk of two peoples becoming one, but only one still keeps its name \
                 aboard.",
            );
            return;
        }
        let Some(idx) = self
            .factions
            .iter()
            .position(|f| f.faction_id == id && f.is_aboard())
        else {
            self.push_log("The talk of union came to nothing — that people had already gone.");
            return;
        };
        let host = self
            .aboard_indices()
            .into_iter()
            .filter(|&i| i != idx)
            .max_by(|&a, &b| {
                self.factions[a]
                    .members
                    .cmp(&self.factions[b].members)
                    .then_with(|| {
                        self.factions[b]
                            .faction_id
                            .cmp(&self.factions[a].faction_id)
                    })
            });
        let Some(host) = host else { return };
        let moved = self.factions[idx].members;
        self.factions[host].members += moved;
        self.factions[idx].members = 0;
        self.factions[idx].status = FactionStatus::Assimilated;
        let merged = log_name(&data.factions, &self.factions[idx].faction_id);
        let into = log_name(&data.factions, &self.factions[host].faction_id);
        self.push_log(format!(
            "{merged} and {into} became one people; the children of {merged} keep the shared \
             name now."
        ));
    }

    /// Yearly (content-depth subsystems round 8): a people whose craft is bound
    /// to a subsystem sours a little each year it is left below the neglect
    /// threshold — the makers cannot abide a rotting engine bay, the gardeners a
    /// dying farm, the Keepers a crumbling archive. Deterministic, no RNG. This
    /// feeds the round-8 approval withdrawal, so neglecting a people's module is
    /// one more way — the most self-inflicted — to lose them.
    pub fn apply_subsystem_neglect_sentiment(&mut self, data: &GameData) {
        let cfg = data.config.factions;
        if cfg.neglect_approval_penalty <= 0.0 {
            return;
        }
        for fstate in &mut self.factions {
            if !fstate.is_aboard() {
                continue;
            }
            let Some(def) = data.factions.get(&fstate.faction_id) else {
                continue;
            };
            if def.tended_subsystem.is_empty() {
                continue;
            }
            let neglected = self
                .subsystems
                .get(&def.tended_subsystem)
                .is_some_and(|s| s.condition < cfg.neglect_condition_threshold);
            if neglected {
                fstate.adjust_approval(-cfg.neglect_approval_penalty);
            }
        }
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

    /// Per-generation (content-depth factions round 11): each aboard people's
    /// numbers wax or wane by its `growth_bias`, so the balance of power shifts
    /// over the centuries — a fecund people grows toward the majority, a people
    /// that does not reproduce naturally dwindles, and the dominant faction (the
    /// lever behind drift, dilemmas, and gates) can change mid-voyage. The
    /// following `rebalance_factions` renormalizes the shifted members back to the
    /// head count. Never drifts a people below one soul — that is the schism's and
    /// the assimilation's job, not attrition's. Deterministic, no RNG.
    pub fn apply_faction_demographic_drift(&mut self, data: &GameData) {
        // How you treat a people bends how it grows (content-depth factions round
        // 13): approval adds to the base bias, so a beloved people waxes and a
        // resented one wanes even beyond its nature. Neutral approval (0.5) is inert.
        let approval_factor = data.config.factions.approval_growth_factor;
        for fstate in &mut self.factions {
            if !fstate.is_aboard() {
                continue;
            }
            let base = data
                .factions
                .get(&fstate.faction_id)
                .map_or(0.0, |d| d.growth_bias);
            let bias = base + approval_factor * (fstate.approval - 0.5);
            if bias != 0.0 {
                let grown = (fstate.members as f32 * (1.0 + bias)).round();
                fstate.members = grown.max(1.0) as u32;
            }
        }
    }

    /// Yearly (content-depth voice round 8): give the otherwise-silent approval
    /// meter a voice. When an aboard people crosses *into* restlessness or
    /// contentment — not every year it stays there — surface one pooled line, so
    /// the player feels a faction souring long before its withdrawal beat fires.
    /// Deterministic (indexed by year), no RNG; neutral crossings are silent.
    pub fn announce_faction_moods(&mut self, data: &GameData) {
        let year = self.year();
        let mut lines: Vec<String> = Vec::new();
        for fstate in &mut self.factions {
            if !fstate.is_aboard() {
                continue;
            }
            let band = mood_band_for(fstate.approval);
            if band == fstate.mood_band {
                continue;
            }
            let pool = match band {
                -1 => &data.config.flavor.faction_souring,
                1 => &data.config.flavor.faction_warming,
                // Settling back to neutral is silent, but still remembered so a
                // later re-souring announces afresh.
                _ => {
                    fstate.mood_band = band;
                    continue;
                }
            };
            let name = log_name(&data.factions, &fstate.faction_id);
            let idx = year as usize + fstate.faction_id.len();
            if let Some(line) = FlavorConfig::line_with_name(pool, idx, &name) {
                lines.push(line);
            }
            fstate.mood_band = band;
        }
        for line in lines {
            self.push_log(line);
        }
    }

    /// Give the *ship's* overall morale a voice (content-depth voice round 11):
    /// the collective parallel to `announce_faction_moods`. When the whole crew's
    /// morale crosses *into* a heavy or a light band — not every year it sits
    /// there — surface one pooled ambient line, so the decks going grim or lifting
    /// together says so. Deterministic (indexed by year), no RNG; settling back to
    /// steady is silent but remembered so a later crossing announces afresh.
    pub fn announce_ship_mood(&mut self, data: &GameData) {
        let band = mood_band_for(self.population.morale);
        if band == self.morale_band {
            return;
        }
        let pool = match band {
            -1 => &data.config.flavor.ship_mood_darkening,
            1 => &data.config.flavor.ship_mood_lifting,
            _ => {
                self.morale_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.morale_band = band;
    }

    /// Shift the smallest aboard faction's approval by `delta`, clamped
    /// (content-depth provisioning round 8): the "who bears the cut" mechanic for
    /// a shortage triage, resolved dynamically so a general rationing beat need
    /// not name a people. Ties break on the lexicographically-first id, matching
    /// `apply_faction_loss`. No-op if no faction is aboard.
    pub fn adjust_smallest_faction_approval(&mut self, delta: f32) {
        let aboard = self.aboard_indices();
        let Some(&idx) = aboard.iter().min_by(|&&a, &&b| {
            self.factions[a]
                .members
                .cmp(&self.factions[b].members)
                .then_with(|| {
                    self.factions[a]
                        .faction_id
                        .cmp(&self.factions[b].faction_id)
                })
        }) else {
            return;
        };
        self.factions[idx].adjust_approval(delta);
    }

    /// Shared removal: mark the faction lost, drop its members from the head
    /// count, and log the parting in the flavor of `kind`.
    fn remove_faction(&mut self, idx: usize, kind: FactionLossKind, data: &GameData) {
        let members = self.factions[idx].members;
        self.factions[idx].members = 0;
        self.factions[idx].status = match kind {
            FactionLossKind::Settled => FactionStatus::Settled,
            FactionLossKind::Departed => FactionStatus::Departed,
        };
        self.population.count = self.population.count.saturating_sub(members);
        let name = log_name(&data.factions, &self.factions[idx].faction_id);
        let tail = match kind {
            FactionLossKind::Settled => "made planetfall to stay, and did not come back aboard",
            FactionLossKind::Departed => "broke away and set their own course into the dark",
        };
        self.push_log(format!("{name} {tail}."));
    }

    /// On a generation boundary, fold any tiny, drifted faction into the largest
    /// (W7 soft assimilation): once cultural drift is high enough, a faction
    /// whose share has fallen below the threshold loses its name to a larger
    /// one. Repeats until no candidate remains.
    pub fn assimilate_drifted_factions(&mut self, data: &GameData) {
        let cfg = &data.config.factions;
        if self.population.cultural_drift <= cfg.assimilation_drift_threshold {
            return;
        }
        loop {
            let aboard = self.aboard_indices();
            if aboard.len() <= 1 {
                break;
            }
            let total: u32 = aboard.iter().map(|&i| self.factions[i].members).sum();
            if total == 0 {
                break;
            }
            let candidate = aboard
                .iter()
                .copied()
                .filter(|&i| {
                    (self.factions[i].members as f32 / total as f32)
                        < cfg.assimilation_share_threshold
                })
                .min_by(|&a, &b| {
                    self.factions[a]
                        .members
                        .cmp(&self.factions[b].members)
                        .then_with(|| {
                            self.factions[a]
                                .faction_id
                                .cmp(&self.factions[b].faction_id)
                        })
                });
            let Some(idx) = candidate else { break };
            let host = aboard
                .iter()
                .copied()
                .filter(|&i| i != idx)
                .max_by(|&a, &b| {
                    self.factions[a]
                        .members
                        .cmp(&self.factions[b].members)
                        .then_with(|| {
                            self.factions[b]
                                .faction_id
                                .cmp(&self.factions[a].faction_id)
                        })
                });
            let Some(host) = host else { break };
            let moved = self.factions[idx].members;
            self.factions[host].members += moved;
            self.factions[idx].members = 0;
            self.factions[idx].status = FactionStatus::Assimilated;
            let name = log_name(&data.factions, &self.factions[idx].faction_id);
            self.push_log(format!(
                "The children of {name} now answer to another name."
            ));
        }
    }

    /// Recruit a fresh people in drydock (W7): only in port, only when short of
    /// the founding count, only from the untouched pool. Charges credits and
    /// grows the colony. Lost factions never return.
    pub fn recruit_faction_group(
        &mut self,
        data: &GameData,
        faction_id: &str,
    ) -> Result<(), String> {
        if self.contract.is_some() {
            return Err("A new people can only be taken aboard in drydock.".to_owned());
        }
        let cfg = &data.config.factions;
        if self.aboard_faction_count() >= cfg.starting_count {
            return Err("The ship already carries its full complement of peoples.".to_owned());
        }
        if self.factions.iter().any(|f| f.faction_id == faction_id) {
            return Err("That people has already sailed with this ship.".to_owned());
        }
        if data.factions.get(faction_id).is_none() {
            return Err("Unknown people.".to_owned());
        }
        let cost = ResourceDelta {
            credits: -cfg.recruit_group_cost_credits,
            ..Default::default()
        };
        if !self.resources.can_afford(&cost) {
            return Err("The treasury cannot cover recruiting a new people.".to_owned());
        }
        self.resources.apply(&cost);
        self.factions.push(FactionState {
            faction_id: faction_id.to_owned(),
            members: cfg.recruit_group_size,
            status: FactionStatus::Aboard,
            approval: default_approval(),
            mood_band: 0,
        });
        self.population.count += cfg.recruit_group_size;
        let name = log_name(&data.factions, faction_id);
        // A recruited people brings its signature dowry (content-depth round 7):
        // the makers a sharper engineering bay, the gardeners a greener one, and
        // so on — so which people you take on matters beyond the head count.
        if let Some(def) = data.factions.get(faction_id) {
            let boon = &def.recruit_boon;
            self.population.apply(&boon.population_delta);
            for delta in &boon.subsystem_deltas {
                if let Some(state) = self.subsystems.get_mut(&delta.id) {
                    state.condition = (state.condition + delta.condition).clamp(0.0, 1.0);
                    state.knowledge = (state.knowledge + delta.knowledge).clamp(0.0, 1.0);
                }
            }
            if boon.flavor.is_empty() {
                self.push_log(format!(
                    "{name} came aboard in drydock — new blood for the long voyage."
                ));
            } else {
                self.push_log(boon.flavor.clone());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::factions::FactionLossKind;
    use crate::data::GameData;
    use crate::state::sim::founding_faction_ids;

    fn fs(id: &str, members: u32) -> FactionState {
        FactionState {
            faction_id: id.to_owned(),
            members,
            status: FactionStatus::Aboard,
            approval: default_approval(),
            mood_band: 0,
        }
    }

    #[test]
    fn demographic_drift_shifts_the_balance_of_power_over_generations() {
        // Content-depth factions round 11: which people runs the ship is not fixed
        // at launch. A fecund people (the Hearth) grows its share over the
        // generations while a people that does not reproduce naturally (the
        // augmented Ascension) dwindles — so a launch minority can become the
        // majority and the dominant faction can flip mid-voyage.
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        // Start the two peoples level, Ascension a shade ahead.
        sim.factions = vec![fs("ascension_circle", 520), fs("hearth_union", 480)];
        sim.population.count = 1000;
        let share = |sim: &SimState, id: &str| {
            let total: u32 = sim
                .factions
                .iter()
                .filter(|f| f.is_aboard())
                .map(|f| f.members)
                .sum();
            sim.factions
                .iter()
                .find(|f| f.faction_id == id)
                .map_or(0.0, |f| f.members as f32 / total as f32)
        };
        assert_eq!(
            sim.dominant_faction_id(),
            Some("ascension_circle"),
            "the augmented lead at launch"
        );
        let asc0 = share(&sim, "ascension_circle");

        // Twelve generations of demographic drift (rebalancing to the head count).
        for _ in 0..12 {
            sim.apply_faction_demographic_drift(&data);
            sim.rebalance_factions();
        }
        assert!(
            share(&sim, "ascension_circle") < asc0,
            "the augmented dwindle over the centuries"
        );
        assert_eq!(
            sim.dominant_faction_id(),
            Some("hearth_union"),
            "a fecund launch-minority has become the majority"
        );
    }

    #[test]
    fn how_a_people_is_treated_bends_how_it_grows() {
        // Content-depth factions round 13: approval bends demographic growth — the
        // link between the approval meter (r8) and demographic drift (r11). Two
        // peoples of identical nature (same base bias), one cherished and one
        // resented, diverge over the generations: the beloved waxes, the resented
        // wanes, even though nothing about their kind differs.
        let data = GameData::load().unwrap();
        assert!(
            data.config.factions.approval_growth_factor > 0.0,
            "this test needs the approval→growth coupling enabled"
        );
        // Both tend the same base bias; only their standing with the ship differs.
        // (Use one faction id so the base growth_bias is identical for both runs.)
        let grow = |approval: f32| -> u32 {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                7,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.factions = vec![FactionState {
                faction_id: "meridian_accord".to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval,
                mood_band: 0,
            }];
            for _ in 0..12 {
                sim.apply_faction_demographic_drift(&data);
            }
            sim.factions[0].members
        };
        let cherished = grow(0.95);
        let resented = grow(0.05);
        assert!(
            cherished > resented,
            "a cherished people waxes where a resented one wanes \
             (cherished {cherished} vs resented {resented})"
        );
        // Neutral standing leaves growth to nature alone (the base bias only).
        let neutral = grow(0.5);
        assert!(
            cherished > neutral && neutral > resented,
            "approval pushes growth both ways around a neutral baseline"
        );
    }

    fn armed(seed: u64) -> (GameData, SimState, Vec<String>) {
        let data = GameData::load().unwrap();
        let picks = founding_faction_ids(&data);
        let sim = SimState::new_campaign(&data, "preservers", seed, &picks);
        (data, sim, picks)
    }

    #[test]
    fn a_souring_people_says_so_once_not_every_year() {
        // Content-depth voice round 8: the approval meter's voice. A people
        // crossing into restlessness surfaces one pooled line, then stays quiet
        // while it remains there — no yearly reprint — and a recovery to
        // contentment gets its own, opposite line.
        let (data, mut sim, _picks) = armed(14);
        let target = sim.factions.iter().find(|f| f.is_aboard()).unwrap();
        let id = target.faction_id.clone();
        let name = log_name(&data.factions, &id);
        let restless = |sim: &SimState| {
            sim.log
                .iter()
                .filter(|l| l.text.contains(&name) && l.text.contains("restless"))
                .count()
        };

        // A neutral people says nothing.
        sim.announce_faction_moods(&data);
        assert_eq!(restless(&sim), 0, "a content people is silent");

        // Sour them past the restless line — one announcement.
        sim.factions
            .iter_mut()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval = 0.2;
        sim.announce_faction_moods(&data);
        let after_first = restless(&sim);
        assert_eq!(after_first, 1, "crossing into restlessness says so once");

        // Still restless the next year — no reprint.
        sim.announce_faction_moods(&data);
        assert_eq!(restless(&sim), 1, "staying restless is not re-announced");

        // Win them all the way back — a warming line, distinct from the souring.
        sim.factions
            .iter_mut()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval = 0.85;
        let log_before = sim.log.len();
        sim.announce_faction_moods(&data);
        assert!(
            sim.log.len() > log_before,
            "a people won back to contentment says so"
        );
    }

    #[test]
    fn the_ships_collective_mood_says_so_once_when_it_turns() {
        // Content-depth voice round 11: the ship-wide morale voice. Crossing into a
        // grim band surfaces one pooled line, then stays quiet while it sits there,
        // and a recovery into a buoyant band gets its own, opposite line.
        let (data, mut sim, _picks) = armed(19);
        let mood_lines = |sim: &SimState| {
            let dark = &data.config.flavor.ship_mood_darkening;
            let light = &data.config.flavor.ship_mood_lifting;
            sim.log
                .iter()
                .filter(|l| {
                    dark.iter().chain(light.iter()).any(|p| {
                        // Match on a distinctive opening clause so we count only
                        // these pooled lines, not other log text.
                        l.text.contains("heaviness has settled")
                            || l.text.contains("lightness has come")
                            || l.text.contains("mood aboard has turned")
                            || l.text.contains("greyness in the crew")
                            || l.text.contains("gone out of the ship's spirit")
                            || l.text.contains("low season")
                            || l.text.contains("something has lifted")
                            || l.text.contains("warmth has spread")
                            || l.text.contains("happy this season")
                            || p == &l.text
                    })
                })
                .count()
        };

        // At its launch baseline the ship says nothing (the starting band is
        // recorded, not announced).
        sim.announce_ship_mood(&data);
        assert_eq!(mood_lines(&sim), 0, "the launch baseline is silent");

        // Sink the crew into a grim band — one announcement.
        sim.population.morale = 0.2;
        sim.announce_ship_mood(&data);
        assert_eq!(mood_lines(&sim), 1, "the decks going grim says so once");
        assert_eq!(sim.morale_band, -1);

        // Still grim next year — no reprint.
        sim.announce_ship_mood(&data);
        assert_eq!(mood_lines(&sim), 1, "staying grim is not re-announced");

        // Lift them into a buoyant band — a second, distinct line.
        sim.population.morale = 0.85;
        sim.announce_ship_mood(&data);
        assert_eq!(mood_lines(&sim), 2, "the ship lifting says so afresh");
        assert_eq!(sim.morale_band, 1);
    }

    #[test]
    fn a_neglected_module_sours_the_people_who_tend_it() {
        // Content-depth subsystems round 8: the people whose craft is a subsystem
        // lose approval each year it sits below the neglect threshold, while a
        // sound module leaves them content — the coupling that lets subsystem
        // neglect feed the round-8 faction withdrawal.
        let (data, mut sim, _picks) = armed(11);
        // The Steel Covenant tend the engineering bay; ensure they are aboard.
        if sim
            .factions
            .iter()
            .all(|f| f.faction_id != "steel_covenant")
        {
            sim.factions.push(fs("steel_covenant", 300));
        }
        let cov_approval = |sim: &SimState| {
            sim.factions
                .iter()
                .find(|f| f.faction_id == "steel_covenant")
                .unwrap()
                .approval
        };

        // A sound engineering bay: the makers stay content year over year.
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.9;
        let before = cov_approval(&sim);
        sim.apply_subsystem_neglect_sentiment(&data);
        assert_eq!(
            cov_approval(&sim),
            before,
            "a well-kept module breeds no grievance"
        );

        // Let the bay rot below the threshold: their approval erodes each year,
        // and only theirs — a faction whose module is fine is untouched.
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.2;
        let gardener_before = sim
            .factions
            .iter()
            .find(|f| f.faction_id == "verdant_kin")
            .map(|f| f.approval);
        sim.apply_subsystem_neglect_sentiment(&data);
        assert!(
            cov_approval(&sim) < before,
            "the makers sour watching their bay rot"
        );
        if let Some(g0) = gardener_before {
            let g1 = sim
                .factions
                .iter()
                .find(|f| f.faction_id == "verdant_kin")
                .unwrap()
                .approval;
            // The gardeners' farm was untouched, so their mood is (unless their
            // own module also happens to be low) unchanged by the bay's rot.
            if sim.subsystems["agriculture"].condition
                >= data.config.factions.neglect_condition_threshold
            {
                assert_eq!(g1, g0, "a people whose module is sound is not soured");
            }
        }
    }

    #[test]
    fn founding_splits_population_and_is_deterministic() {
        let (data, sim, picks) = armed(7);
        let sum: u32 = sim.factions.iter().map(|f| f.members).sum();
        assert_eq!(sum, sim.population.count, "members sum to the head count");
        assert_eq!(sim.factions.len(), picks.len());
        assert!(sim.factions.iter().all(|f| f.is_aboard()));

        let again = SimState::new_campaign(&data, "preservers", 7, &picks);
        let a: Vec<_> = sim.factions.iter().map(|f| f.members).collect();
        let b: Vec<_> = again.factions.iter().map(|f| f.members).collect();
        assert_eq!(a, b, "deterministic per (seed, factions)");
    }

    #[test]
    fn rebalance_preserves_shares_and_the_sum_invariant() {
        let (_data, mut sim, _picks) = armed(1);
        let total: u32 = sim.factions.iter().map(|f| f.members).sum();
        let before: Vec<f32> = sim
            .factions
            .iter()
            .map(|f| f.members as f32 / total as f32)
            .collect();

        sim.population.count /= 2;
        sim.rebalance_factions();

        let aboard_sum: u32 = sim
            .factions
            .iter()
            .filter(|f| f.is_aboard())
            .map(|f| f.members)
            .sum();
        assert_eq!(aboard_sum, sim.population.count, "sum invariant holds");
        let now: u32 = sim.factions.iter().map(|f| f.members).sum();
        for (i, f) in sim.factions.iter().enumerate() {
            let share = f.members as f32 / now as f32;
            assert!(
                (share - before[i]).abs() < 0.02,
                "share preserved for {}",
                f.faction_id
            );
        }
    }

    #[test]
    fn a_near_total_collapse_wipes_the_smallest_faction() {
        let (_data, mut sim, picks) = armed(1);
        sim.factions = vec![fs(&picks[0], 1), fs(&picks[1], 500), fs(&picks[2], 500)];
        sim.population.count = 2;

        let wiped = sim.rebalance_factions();
        assert_eq!(wiped, vec![picks[0].clone()]);
        assert_eq!(sim.factions[0].status, FactionStatus::WipedOut);
        let aboard_sum: u32 = sim
            .factions
            .iter()
            .filter(|f| f.is_aboard())
            .map(|f| f.members)
            .sum();
        assert_eq!(aboard_sum, 2);
    }

    #[test]
    fn a_tiny_drifted_faction_is_assimilated_only_when_drift_is_high() {
        let (data, mut sim, picks) = armed(1);
        let seed_factions = || vec![fs(&picks[0], 40), fs(&picks[1], 480), fs(&picks[2], 480)];

        // Low drift: the small faction holds on.
        sim.factions = seed_factions();
        sim.population.count = 1000;
        sim.population.cultural_drift = 0.3;
        sim.assimilate_drifted_factions(&data);
        assert!(
            sim.factions.iter().all(|f| f.is_aboard()),
            "drift 0.3 spares it"
        );

        // High drift: the 4% faction (< 5% threshold) folds into a larger one.
        sim.factions = seed_factions();
        sim.population.cultural_drift = 0.8;
        sim.assimilate_drifted_factions(&data);
        assert_eq!(sim.factions[0].status, FactionStatus::Assimilated);
        assert_eq!(sim.factions[0].members, 0);
        let aboard_sum: u32 = sim
            .factions
            .iter()
            .filter(|f| f.is_aboard())
            .map(|f| f.members)
            .sum();
        assert_eq!(
            aboard_sum, 1000,
            "assimilation transfers, never loses, members"
        );
    }

    #[test]
    fn faction_loss_removes_the_smallest_but_spares_the_last() {
        let (data, mut sim, picks) = armed(1);
        sim.factions = vec![fs(&picks[0], 100), fs(&picks[1], 500), fs(&picks[2], 400)];
        sim.population.count = 1000;

        sim.apply_faction_loss(&data, FactionLossKind::Settled);
        assert_eq!(sim.factions[0].status, FactionStatus::Settled);
        assert_eq!(sim.factions[0].members, 0);
        assert_eq!(
            sim.population.count, 900,
            "the settlers leave the head count"
        );

        // Reduce to a single Aboard faction; it can never be lost this way.
        let mut solo = SimState::new_campaign(&data, "preservers", 2, &picks);
        solo.factions = vec![fs(&picks[0], 1000)];
        solo.population.count = 1000;
        solo.apply_faction_loss(&data, FactionLossKind::Departed);
        assert!(
            solo.factions[0].is_aboard(),
            "the last people are never lost"
        );
        assert_eq!(solo.population.count, 1000);
    }

    #[test]
    fn targeted_faction_loss_sheds_the_named_group_not_the_smallest() {
        let (data, mut sim, picks) = armed(1);
        // Named faction (picks[1]) is the LARGEST, so a smallest-loss would spare
        // it — targeting must remove it anyway (content-depth round 3 schism).
        sim.factions = vec![fs(&picks[0], 100), fs(&picks[1], 500), fs(&picks[2], 400)];
        sim.population.count = 1000;

        sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, &picks[1]);
        assert_eq!(sim.factions[1].status, FactionStatus::Departed);
        assert_eq!(sim.factions[1].members, 0);
        assert_eq!(
            sim.population.count, 500,
            "the departed faction leaves the head count"
        );
        assert!(sim.factions[0].is_aboard() && sim.factions[2].is_aboard());

        // Never the last aboard people, even when named.
        let mut solo = SimState::new_campaign(&data, "preservers", 2, &picks);
        solo.factions = vec![fs(&picks[0], 1000)];
        solo.population.count = 1000;
        solo.apply_faction_loss_by_id(&data, FactionLossKind::Departed, &picks[0]);
        assert!(
            solo.factions[0].is_aboard(),
            "the last people are never lost"
        );
    }

    #[test]
    fn recruiting_a_people_is_gated_and_charges_credits() {
        let (data, mut sim, _picks) = armed(1);
        sim.resources.credits = 100_000;
        let newcomer = sim.recruitable_faction_ids(&data)[0].clone();

        // Full complement → refused (not short of the founding count).
        assert!(sim.recruit_faction_group(&data, &newcomer).is_err());

        // Lose the smallest faction → short by one.
        sim.apply_faction_loss(&data, FactionLossKind::Departed);
        let lost_id = sim
            .factions
            .iter()
            .find(|f| !f.is_aboard())
            .unwrap()
            .faction_id
            .clone();

        // Underway → refused even while short.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(crate::simulation::contract::start_contract(&template, &sim));
        assert!(sim.recruit_faction_group(&data, &newcomer).is_err());
        sim.contract = None;

        // A lost people never returns.
        assert!(sim.recruit_faction_group(&data, &lost_id).is_err());

        // In port, short, from the untouched pool → allowed; credits + head count.
        let credits_before = sim.resources.credits;
        let pop_before = sim.population.count;
        sim.recruit_faction_group(&data, &newcomer).unwrap();
        assert_eq!(
            credits_before - sim.resources.credits,
            data.config.factions.recruit_group_cost_credits
        );
        assert_eq!(
            sim.population.count - pop_before,
            data.config.factions.recruit_group_size
        );
        assert!(sim
            .factions
            .iter()
            .any(|f| f.faction_id == newcomer && f.is_aboard()));
    }

    #[test]
    fn a_recruited_people_brings_its_signature_dowry() {
        // Content-depth factions round 7: recruiting a people is no longer a bare
        // head count — the Steel Covenant walk into the engineering bay and leave
        // it sharper. Which people you take on matters.
        let (data, mut sim, _picks) = armed(9);
        sim.resources.credits = 100_000;
        // Free a slot, then recruit the makers specifically.
        sim.apply_faction_loss(&data, FactionLossKind::Departed);
        assert!(
            !sim.is_faction_aboard("steel_covenant"),
            "the makers are recruitable in this campaign"
        );
        let boon = &data.factions.get("steel_covenant").unwrap().recruit_boon;
        assert!(boon
            .subsystem_deltas
            .iter()
            .any(|d| d.id == "engineering_bay"));

        let before = sim.subsystems["engineering_bay"].knowledge;
        sim.recruit_faction_group(&data, "steel_covenant").unwrap();
        assert!(
            sim.subsystems["engineering_bay"].knowledge > before,
            "the Covenant's craft lifts the engineering bay on arrival"
        );
        // The dowry's own line was logged (not the generic recruit line).
        assert!(
            sim.log.iter().any(|e| e.text.contains("engineering bay")),
            "the recruit logs the people's signature arrival"
        );
    }
}
