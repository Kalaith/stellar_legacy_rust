//! Founding factions (W7): population segments carried within one campaign.
//!
//! Factions are groups of people *aboard* — orthogonal to the campaign-level
//! legacy (preservers/adaptors/wanderers), which is unchanged. Structure plus
//! roster change (loss/merger/recruit), log/event coloring, and a one-time
//! recruitment dowry per people (content-depth round 7). No *ongoing* approval
//! meters yet — those layer on later.

use serde::{Deserialize, Serialize};

use crate::data::events::FactionApprovalDelta;
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

    /// The bright mirror of `apply_subsystem_neglect_sentiment` (content-depth factions
    /// round 22): where a people whose tended module rots *sours* (r12), a people
    /// *delighted* with its lot tends its module with pride — the makers keeping the
    /// engine bay a shade truer than duty demands, the gardeners the grow-decks a touch
    /// greener, the Keepers the archive that much better kept. Each year an aboard
    /// tending faction's approval sits at or above the proud threshold, its tended
    /// subsystem gains a little condition and knowledge (clamped to 1). This closes a
    /// feedback loop across the faction↔subsystem boundary the neglect coupling only
    /// half-drew: a kept module keeps its people content (r12 spares them the penalty)
    /// and content people keep the module kept — a virtuous circle, with a vicious twin
    /// when a module is let go and its souring people let it rot the faster.
    /// Deterministic, no RNG.
    pub fn apply_proud_tender_upkeep(&mut self, data: &GameData) {
        let cfg = data.config.factions;
        if cfg.proud_tender_condition_bonus <= 0.0 || cfg.proud_tender_approval_threshold <= 0.0 {
            return;
        }
        // Gather the tended modules of every delighted people from the immutable
        // catalog first, then apply — so the read of `data.factions` and the mutation
        // of `self.subsystems` never overlap.
        let mut lifts: Vec<String> = Vec::new();
        for fstate in &self.factions {
            if !fstate.is_aboard() || fstate.approval < cfg.proud_tender_approval_threshold {
                continue;
            }
            if let Some(def) = data.factions.get(&fstate.faction_id) {
                if !def.tended_subsystem.is_empty() {
                    lifts.push(def.tended_subsystem.clone());
                }
            }
        }
        for id in lifts {
            if let Some(state) = self.subsystems.get_mut(&id) {
                state.condition = (state.condition + cfg.proud_tender_condition_bonus).min(1.0);
                state.knowledge = (state.knowledge + cfg.proud_tender_knowledge_bonus).min(1.0);
            }
        }
    }

    /// Move the ship's `unity` by how the aboard peoples stand *to each other*
    /// (content-depth factions round 23): the relationship-side twin of the it100
    /// approval→unity coupling. Where that reads how *content* the peoples are, this
    /// reads their *standing relationships* — a pair of aboard **rivals** (it14) both
    /// holding real shares of the ship grind at cohesion year over year (a permanent
    /// friction, distinct from the event-time approval spillover), while a pair of
    /// aboard **allies** (it17) lift it. Each contribution scales by the *product* of the
    /// two peoples' shares, so a rivalry only bites when both parties are large — a tiny
    /// remnant faction troubles no one — and the balance of the whole roster, not just
    /// its mood, becomes a standing cohesion cost or dividend. Deterministic, no RNG;
    /// pairs are read symmetrically (both directions), the tuning constants absorbing it.
    pub fn apply_faction_relationship_cohesion(&mut self, data: &GameData) {
        let cfg = data.config.factions;
        if cfg.rival_unity_friction <= 0.0 && cfg.ally_unity_solidarity <= 0.0 {
            return;
        }
        let total: u32 = self
            .factions
            .iter()
            .filter(|f| f.is_aboard())
            .map(|f| f.members)
            .sum();
        if total == 0 {
            return;
        }
        let share = |id: &str| -> f32 {
            self.factions
                .iter()
                .find(|f| f.faction_id == id && f.is_aboard())
                .map_or(0.0, |f| f.members as f32 / total as f32)
        };
        let mut net = 0.0f32;
        for fstate in self.factions.iter().filter(|f| f.is_aboard()) {
            let Some(def) = data.factions.get(&fstate.faction_id) else {
                continue;
            };
            let share_f = fstate.members as f32 / total as f32;
            for rival in &def.rivals {
                net -= cfg.rival_unity_friction * share_f * share(rival);
            }
            for ally in &def.allies {
                net += cfg.ally_unity_solidarity * share_f * share(ally);
            }
        }
        if net != 0.0 {
            self.population.unity = (self.population.unity + net).clamp(0.0, 1.0);
        }
    }

    /// Sour the aboard rivals of any people an event just favored (content-depth
    /// factions round 14): each positive approval gain spills a fraction of its
    /// resentment onto the favored people's aboard rivals, so favoring one people
    /// costs you with those it quarrels with — the friction pairs made a lasting
    /// relationship. A slight (a negative delta) does not lift rivals; the mechanic
    /// is the *cost of favoritism*, not schadenfreude. Deterministic, no RNG.
    pub fn apply_rival_approval_spillover(
        &mut self,
        data: &GameData,
        deltas: &[FactionApprovalDelta],
    ) {
        let spill = data.config.factions.rival_approval_spillover;
        if spill <= 0.0 {
            return;
        }
        // Gather (rival, penalty) from the immutable catalog first, then apply — so
        // the read of `data.factions.rivals` and the mutation of `self.factions`
        // don't overlap.
        let mut penalties: Vec<(String, f32)> = Vec::new();
        for delta in deltas {
            if delta.delta <= 0.0 || !self.is_faction_aboard(&delta.id) {
                continue;
            }
            if let Some(def) = data.factions.get(&delta.id) {
                for rival in &def.rivals {
                    if self.is_faction_aboard(rival) {
                        penalties.push((rival.clone(), -spill * delta.delta));
                    }
                }
            }
        }
        for (rival_id, penalty) in penalties {
            if let Some(state) = self
                .factions
                .iter_mut()
                .find(|f| f.faction_id == rival_id && f.is_aboard())
            {
                state.adjust_approval(penalty);
            }
        }
    }

    /// Warm the aboard allies of any people an event just favored (content-depth
    /// factions round 17): the positive twin of `apply_rival_approval_spillover`.
    /// Each positive approval gain shares a fraction of its goodwill with the favored
    /// people's aboard allies, so courting one people lifts its kin — the r5 merger
    /// pairs made a standing coalition the way the friction pairs were made a standing
    /// rivalry. A slight (a negative delta) does not sour allies; the mechanic is the
    /// *reward of coalition*, not shared misery. Deterministic, no RNG.
    pub fn apply_ally_approval_spillover(
        &mut self,
        data: &GameData,
        deltas: &[FactionApprovalDelta],
    ) {
        let spill = data.config.factions.ally_approval_spillover;
        if spill <= 0.0 {
            return;
        }
        // Gather (ally, bonus) from the immutable catalog first, then apply — so the
        // read of `data.factions.allies` and the mutation of `self.factions` don't
        // overlap.
        let mut bonuses: Vec<(String, f32)> = Vec::new();
        for delta in deltas {
            if delta.delta <= 0.0 || !self.is_faction_aboard(&delta.id) {
                continue;
            }
            if let Some(def) = data.factions.get(&delta.id) {
                for ally in &def.allies {
                    if self.is_faction_aboard(ally) {
                        bonuses.push((ally.clone(), spill * delta.delta));
                    }
                }
            }
        }
        for (ally_id, bonus) in bonuses {
            if let Some(state) = self
                .factions
                .iter_mut()
                .find(|f| f.faction_id == ally_id && f.is_aboard())
            {
                state.adjust_approval(bonus);
            }
        }
    }

    /// Drift the ship's reputation by the standing character of whoever runs it
    /// (content-depth factions round 16): the dominant people's `reputation_leanings`
    /// nudge each named trait a little each year, so a ship long-run by a kind people
    /// grows known for mercy and one run by a cold people hardens — reputation built
    /// from who is in charge, not only from event choices. Deterministic, no RNG.
    pub fn apply_dominant_reputation_lean(&mut self, data: &GameData) {
        let per_year = data.config.factions.dominant_reputation_lean_per_year;
        if per_year == 0.0 {
            return;
        }
        let Some(dominant) = self.dominant_faction_id().map(str::to_owned) else {
            return;
        };
        let leanings: Vec<(String, f32)> = match data.factions.get(&dominant) {
            Some(def) => def
                .reputation_leanings
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            None => return,
        };
        for (trait_id, lean) in leanings {
            self.adjust_reputation(&trait_id, lean * per_year);
        }
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

    /// Give the ship's *political climate* a voice (content-depth voice round 15):
    /// distinct from the crew's spirits (`announce_ship_mood`) and from any one
    /// people's mood (`announce_faction_moods`), this is the member-weighted mood of
    /// the aboard peoples as a whole (it100's `aboard_approval_mean`) — how content
    /// the polity is with its treatment. When it crosses *into* broad discontent or
    /// broad ease, surface one pooled line, so a ship's peoples curdling or settling
    /// together says so. Deterministic (indexed by year), no RNG; a return to
    /// neutral is silent but remembered.
    pub fn announce_polity_mood(&mut self, data: &GameData) {
        let band = mood_band_for(self.aboard_approval_mean());
        if band == self.polity_mood_band {
            return;
        }
        let pool = match band {
            -1 => &data.config.flavor.polity_souring,
            1 => &data.config.flavor.polity_warming,
            _ => {
                self.polity_mood_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.polity_mood_band = band;
    }

    /// Give the ship's growing *reputation* a voice (content-depth voice round 16):
    /// the quiet companion to the it109 reputation beat, at a gentler threshold. When
    /// the watched trait crosses *into* a merciful or a feared band, surface one
    /// pooled line — the ship remarking that its name has begun to mean something —
    /// before that name grows defining enough to force the beat's reckoning.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_reputation_name(&mut self, data: &GameData) {
        let trait_id = &data.config.campaign_skeleton.reputation_beat_trait;
        let fl = &data.config.flavor;
        if trait_id.is_empty() || fl.reputation_voice_high <= 0.0 {
            return;
        }
        let value = self.reputation(trait_id);
        let band = if value >= fl.reputation_voice_high {
            1
        } else if value <= fl.reputation_voice_low {
            -1
        } else {
            0
        };
        if band == self.reputation_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.reputation_merciful,
            -1 => &fl.reputation_feared,
            _ => {
                self.reputation_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.reputation_voice_band = band;
    }

    /// Give the ship's *institutions* a voice (content-depth voice round 17): the
    /// governance twin of the morale (`announce_ship_mood`) and polity
    /// (`announce_polity_mood`) voices. Distinct from the crew's spirits and from how
    /// content the peoples are, this voices the *machinery of government* — when
    /// `stability` crosses *into* a fraying band (quorums missed, offices unfilled) or
    /// a firm one (the councils working, the charter honored in practice), surface one
    /// pooled line. Gated gentler than the it102 collapse *beat*, so the voice (a
    /// fraying noticed) precedes the reckoning (a government failed). Deterministic
    /// (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_stability_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.stability_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.stability,
            fl.stability_voice_high,
            fl.stability_voice_low,
        );
        if band == self.stability_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.stability_firming,
            -1 => &fl.stability_fraying,
            _ => {
                self.stability_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.stability_voice_band = band;
    }

    /// Give the crew's *devotion to the founders' mission* a voice (content-depth voice
    /// round 20): the identity-side twin of the morale (`announce_ship_mood`) and
    /// governance (`announce_stability_mood`) voices, on `legacy_loyalty`. Distinct from
    /// the crew's spirits and from how far the people have *changed* (the it-drift
    /// ambient) — this voices the founders' *purpose* itself waxing or fading: when
    /// loyalty crosses *into* a guttering band (the charter read as a story, the young
    /// unable to feel why the ship flies) or a bright one (the dream taken up afresh, the
    /// mission honored from conviction), surface one pooled line. Announced right after
    /// the year's voyage drift, which erodes loyalty, so the fading of the founders' fire
    /// is narrated as it happens. Deterministic (indexed by year), no RNG; a return to
    /// the middle re-arms.
    pub fn announce_loyalty_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.loyalty_voice_high <= 0.0 {
            return;
        }
        let value = self.population.legacy_loyalty;
        let band = if value >= fl.loyalty_voice_high {
            1
        } else if value <= fl.loyalty_voice_low {
            -1
        } else {
            0
        };
        if band == self.loyalty_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.loyalty_bright,
            -1 => &fl.loyalty_guttering,
            _ => {
                self.loyalty_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.loyalty_voice_band = band;
    }

    /// Give the crew's *physiological* identity a voice (content-depth voice round 25):
    /// the bodily companion to the loyalty voice, on `adaptation`. When the descendants'
    /// bodies cross *into* a shipborn band (longer, leaner, fitted to the ship and no
    /// longer to a world) or a baseline one (held human by a well-kept infirmary, it25),
    /// the decks remark it once — the crew becoming, or refusing to become, a new kind of
    /// people in the flesh, distinct from the it167 loyalty voice (their belief) and the
    /// drift-aware ambient (their culture). Deterministic (indexed by year), no RNG; a
    /// return to the middle re-arms.
    pub fn announce_adaptation_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.adaptation_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.adaptation,
            fl.adaptation_voice_high,
            fl.adaptation_voice_low,
        );
        if band == self.adaptation_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.crew_shipborn,
            -1 => &fl.crew_baseline,
            _ => {
                self.adaptation_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.adaptation_voice_band = band;
    }

    /// Give the crew's *cohesion* a voice (content-depth voice round 21): the fourth
    /// internal-state voice, beside the morale (`announce_ship_mood`), governance
    /// (`announce_stability_mood`), and mission-devotion (`announce_loyalty_mood`) ones,
    /// on `unity`. Distinct from all three — a crew can be high-spirited, well-governed,
    /// and sure of its purpose yet quietly *splintering* into cliques, one people
    /// becoming several. When unity crosses *into* a fraying band (the ship coming apart
    /// into wary factions) or a cohering one (the crew pulling back together as one), the
    /// decks remark it once. Distinct too from the it102 unity-*collapse* beat, which is
    /// the reckoning; this is the quieter thing noticed before and after it. Deterministic
    /// (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_unity_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.unity_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.unity,
            fl.unity_voice_high,
            fl.unity_voice_low,
        );
        if band == self.unity_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.unity_cohering,
            -1 => &fl.unity_fraying,
            _ => {
                self.unity_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.unity_voice_band = band;
    }

    /// Give the ship's *own body* a voice (content-depth voice round 22): the first that
    /// speaks for the vessel rather than the crew. Where the morale/unity/stability/
    /// loyalty voices read the *people*, this reads the aging machine that carries them —
    /// when `hull_integrity` crosses *into* a groaning band (the plates weeping at the
    /// seams, the frame complaining on every burn) or a sound one (riding tight and true
    /// again after a refit), the decks remark it once. Deterministic (indexed by year),
    /// no RNG; a return to the middle re-arms.
    pub fn announce_hull_condition(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.hull_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.ship.hull_integrity,
            fl.hull_voice_high,
            fl.hull_voice_low,
        );
        if band == self.hull_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.hull_sound,
            -1 => &fl.hull_groaning,
            _ => {
                self.hull_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.hull_voice_band = band;
    }

    /// Give the ship's *air* a voice (content-depth voice round 23): the second ship-body
    /// voice, the atmosphere twin of the it22 hull (structure) voice, on `life_support`.
    /// When the air crosses *into* a stale band (close and thick, the scrubbers labouring)
    /// or a fresh one (clean and cool again after an overhaul), the decks remark it once.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_air_condition(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.air_voice_high <= 0.0 {
            return;
        }
        let band =
            stability_voice_band_for(self.ship.life_support, fl.air_voice_high, fl.air_voice_low);
        if band == self.air_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.air_fresh,
            -1 => &fl.air_stale,
            _ => {
                self.air_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.air_voice_band = band;
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
        // Losing a whole people wounds the ship's cohesion (content-depth factions round
        // 24): beyond the bodies and the craft, a departure leaves a hole in the
        // community — a familiar quarter of the ship gone quiet, the balance upset, the
        // remaining crew shaken. Scaled by the departing people's share of the ship
        // *before* they left, so a great secession is a blow and a tiny remnant is not.
        let scar_scale = data.config.factions.departure_cohesion_scar;
        if scar_scale > 0.0 && members > 0 {
            let total_before = self.population.count + members;
            let share = members as f32 / total_before.max(1) as f32;
            let scar = scar_scale * share;
            self.population.morale = (self.population.morale - scar).max(0.0);
            self.population.unity = (self.population.unity - scar).max(0.0);
        }
        let name = log_name(&data.factions, &self.factions[idx].faction_id);
        let tail = match kind {
            FactionLossKind::Settled => "made planetfall to stay, and did not come back aboard",
            FactionLossKind::Departed => "broke away and set their own course into the dark",
        };
        self.push_log(format!("{name} {tail}."));

        // The departing people take their craft with them (content-depth factions
        // round 20): the module they tended loses a chunk of its living expertise —
        // the ones who truly understood it are gone. Feeds the knowledge-crisis
        // events and the education keystone's slow re-teaching.
        let tended = data
            .factions
            .get(&self.factions[idx].faction_id)
            .map(|f| f.tended_subsystem.clone())
            .unwrap_or_default();
        let loss = data.config.factions.departed_faction_knowledge_loss;
        if !tended.is_empty() && loss > 0.0 {
            if let Some(state) = self.subsystems.get_mut(&tended) {
                let dropped = state.knowledge.min(loss);
                if dropped > 0.0 {
                    state.knowledge -= dropped;
                    let subname = data
                        .subsystems
                        .get(&tended)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| tended.clone());
                    self.push_log(format!(
                        "The craft of the {subname} went with {name}; the hands that truly understood it are aboard no longer."
                    ));
                }
            }
        }
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
            // A people merging into the majority consolidates the polity (content-depth
            // factions round 26): the positive mirror of the it24 departure scar — no hole
            // torn, one fewer faultline, so unity lifts a little, scaled by how much of the
            // ship just folded together.
            let lift = cfg.assimilation_unity_lift * (moved as f32 / total as f32);
            if lift > 0.0 {
                self.population.unity = (self.population.unity + lift).min(1.0);
            }
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
    fn who_runs_the_ship_bends_its_reputation_over_the_generations() {
        // Content-depth factions round 16: the dominant people's standing character
        // drifts the ship's reputation. A ship run by a kind people (the Hearth)
        // grows more merciful over the years; one run by a cold people (the
        // Ascension) hardens — no dramatic choice required.
        let data = GameData::load().unwrap();
        assert!(
            data.config.factions.dominant_reputation_lean_per_year > 0.0,
            "this test needs the dominant-reputation lean enabled"
        );

        let mercy_after = |dominant: &str| -> f32 {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                31,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.factions = vec![FactionState {
                faction_id: dominant.to_string(),
                members: sim.population.count,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            }];
            for _ in 0..30 {
                sim.apply_dominant_reputation_lean(&data);
            }
            sim.reputation("mercy")
        };

        let under_hearth = mercy_after("hearth_union");
        let under_ascension = mercy_after("ascension_circle");
        assert!(
            under_hearth > 0.5,
            "a kind majority grows the ship a merciful name"
        );
        assert!(under_ascension < 0.5, "a cold majority hardens it");
        // A people with no leaning leaves the ship's name to its choices.
        let under_neutral = mercy_after("meridian_accord");
        assert_eq!(under_neutral, 0.5, "an unleaning people touches nothing");
    }

    #[test]
    fn a_content_polity_steadies_the_ship_and_a_resentful_one_frays_it() {
        // Content-depth factions round 15: the faction system's first coupling to
        // the ship's own cohesion. Two otherwise-identical ships, one carrying a
        // content people and one a resentful one, diverge in unity over the years —
        // a content polity holds the ship together where a resentful one wears at it.
        use crate::simulation::tick::advance_year;
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        // Clear the threshold beats so a fraying ship doesn't trip one mid-test.
        data.config.campaign_skeleton.crisis_beats.clear();
        data.config.campaign_skeleton.loyalty_beats.clear();
        data.config.campaign_skeleton.drift_beats.clear();
        data.config.campaign_skeleton.adaptation_beats.clear();
        assert!(
            data.config.factions.approval_unity_coupling > 0.0,
            "this test needs the faction-cohesion coupling enabled"
        );

        let unity_after = |approval: f32| -> f32 {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                79,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.resources.food = 1_000_000;
            sim.factions = vec![FactionState {
                faction_id: "steel_covenant".to_string(),
                members: sim.population.count,
                status: FactionStatus::Aboard,
                approval,
                mood_band: 0,
            }];
            sim.population.unity = 0.6;
            for _ in 0..20 {
                advance_year(&mut sim, &data);
            }
            sim.population.unity
        };
        let content = unity_after(0.95);
        let resentful = unity_after(0.05);
        assert!(
            content > resentful,
            "a content polity holds the ship together where a resentful one frays it \
             (content {content} vs resentful {resentful})"
        );
    }

    #[test]
    fn a_divided_ship_is_harder_to_govern() {
        // Content-depth factions round 18: governing a divided ship strains its
        // institutions. Two otherwise-identical ships — one carrying ideologically
        // aligned peoples, one carrying peoples at opposite ends of the tech↔tradition
        // spectrum — diverge in stability, the divided coalition eroding where the
        // aligned one holds. Distinct from the content/resentful (approval→unity) axis.
        use crate::simulation::tick::advance_year;
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        data.config.campaign_skeleton.stability_beats.clear();
        data.config.campaign_skeleton.crisis_beats.clear();
        data.config.campaign_skeleton.drift_beats.clear();
        data.config.campaign_skeleton.adaptation_beats.clear();
        data.config.campaign_skeleton.loyalty_beats.clear();
        // Isolate the coupling: no security recovery pushing stability back up.
        data.config
            .subsystems
            .security_stability_recovery_per_condition = 0.0;
        assert!(
            data.config.factions.ideology_spread_stability_penalty > 0.0,
            "this test needs the ideology-spread coupling enabled"
        );

        let stability_after = |ids: &[&str]| -> f32 {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                88,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.resources.food = 1_000_000;
            let each = sim.population.count / ids.len() as u32;
            sim.factions = ids
                .iter()
                .map(|id| FactionState {
                    faction_id: (*id).to_string(),
                    members: each,
                    status: FactionStatus::Aboard,
                    approval: 0.5,
                    mood_band: 0,
                })
                .collect();
            sim.population.stability = 0.6;
            for _ in 0..20 {
                advance_year(&mut sim, &data);
            }
            sim.population.stability
        };

        // Aligned peoples (all tradition-leaning) vs a coalition spanning the spectrum.
        let aligned = stability_after(&["verdant_kin", "hearth_union", "first_flame"]);
        let divided = stability_after(&["ascension_circle", "first_flame"]);
        assert!(
            divided < aligned,
            "a ship split across the ideological spectrum governs worse than an aligned one \
             (divided {divided} vs aligned {aligned})"
        );
        assert!(
            (aligned - 0.6).abs() < 1e-6,
            "an ideologically unified ship's institutions are untouched by the coupling"
        );
    }

    #[test]
    fn favoring_a_people_sours_its_aboard_rivals() {
        // Content-depth factions round 14: the friction pairs made a lasting cost.
        // Lifting one people's approval spills resentment onto its aboard rivals, so
        // the meter cannot be maxed for everyone; a rival not aboard is untouched,
        // and slighting a people does not lift its rivals.
        use crate::data::events::FactionApprovalDelta;
        let data = GameData::load().unwrap();
        assert!(
            data.config.factions.rival_approval_spillover > 0.0,
            "this test needs the rivalry spillover enabled"
        );
        // Steel Covenant and Verdant Kin are authored rivals; the Hearth is neither.
        let def = data.factions.get("steel_covenant").unwrap();
        assert!(def.rivals.contains(&"verdant_kin".to_string()));

        let fs = |id: &str| FactionState {
            faction_id: id.to_string(),
            members: 400,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        };
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            9,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![fs("steel_covenant"), fs("verdant_kin"), fs("hearth_union")];

        // Favor the Covenant: its rival the Kin sours, the unrelated Hearth does not.
        sim.apply_rival_approval_spillover(
            &data,
            &[FactionApprovalDelta {
                id: "steel_covenant".to_string(),
                delta: 0.2,
            }],
        );
        let approval = |sim: &SimState, id: &str| {
            sim.factions
                .iter()
                .find(|f| f.faction_id == id)
                .unwrap()
                .approval
        };
        assert!(
            approval(&sim, "verdant_kin") < 0.5,
            "the Covenant's rival resents the favoritism"
        );
        assert_eq!(
            approval(&sim, "hearth_union"),
            0.5,
            "a people that is no rival is untouched"
        );

        // Slighting the Covenant does not lift its rival (the cost is of favoritism).
        let kin_before = approval(&sim, "verdant_kin");
        sim.apply_rival_approval_spillover(
            &data,
            &[FactionApprovalDelta {
                id: "steel_covenant".to_string(),
                delta: -0.2,
            }],
        );
        assert_eq!(
            approval(&sim, "verdant_kin"),
            kin_before,
            "a slight to a people is not a gift to its rivals"
        );
    }

    #[test]
    fn favoring_a_people_warms_its_aboard_allies() {
        // Content-depth factions round 17: the positive twin of the rivalry spillover.
        // Lifting one people's approval shares a fraction of the goodwill with its
        // aboard allies, so the meter rewards building a coalition; an ally not aboard
        // is untouched, and slighting a people does not sour its allies (the mechanic
        // is the reward of coalition, not shared misery).
        use crate::data::events::FactionApprovalDelta;
        let data = GameData::load().unwrap();
        assert!(
            data.config.factions.ally_approval_spillover > 0.0,
            "this test needs the alliance spillover enabled"
        );
        // Hearth Union and Verdant Kin are authored allies (the green hearth); the
        // Steel Covenant is neither.
        let def = data.factions.get("hearth_union").unwrap();
        assert!(def.allies.contains(&"verdant_kin".to_string()));

        let fs = |id: &str| FactionState {
            faction_id: id.to_string(),
            members: 400,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        };
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            11,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![fs("hearth_union"), fs("verdant_kin"), fs("steel_covenant")];

        let approval = |sim: &SimState, id: &str| {
            sim.factions
                .iter()
                .find(|f| f.faction_id == id)
                .unwrap()
                .approval
        };

        // Favor the Hearth: its ally the Kin warms, the unrelated Covenant does not.
        sim.apply_ally_approval_spillover(
            &data,
            &[FactionApprovalDelta {
                id: "hearth_union".to_string(),
                delta: 0.2,
            }],
        );
        assert!(
            approval(&sim, "verdant_kin") > 0.5,
            "the Hearth's ally shares in the goodwill"
        );
        assert_eq!(
            approval(&sim, "steel_covenant"),
            0.5,
            "a people that is no ally is untouched"
        );

        // Slighting the Hearth does not sour its ally (the reward is of coalition).
        let kin_before = approval(&sim, "verdant_kin");
        sim.apply_ally_approval_spillover(
            &data,
            &[FactionApprovalDelta {
                id: "hearth_union".to_string(),
                delta: -0.2,
            }],
        );
        assert_eq!(
            approval(&sim, "verdant_kin"),
            kin_before,
            "a slight to a people is not a wound to its allies"
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
    fn the_ship_remarks_when_its_name_begins_to_mean_something() {
        // Content-depth voice round 16: the reputation voice, the quiet companion to
        // the it109 reputation beat at a gentler threshold. Crossing into a merciful
        // name surfaces one pooled line; a return to the middle re-arms; a feared
        // name gets its own, opposite line.
        let (data, mut sim, _picks) = armed(29);
        let trait_id = data.config.campaign_skeleton.reputation_beat_trait.clone();
        let high = data.config.flavor.reputation_voice_high;
        let low = data.config.flavor.reputation_voice_low;
        assert!(
            !trait_id.is_empty() && high > 0.0 && data.config.flavor.reputation_merciful.len() >= 3,
            "this test needs the reputation voice enabled"
        );
        let name_lines = |sim: &SimState| {
            let m = &data.config.flavor.reputation_merciful;
            let f = &data.config.flavor.reputation_feared;
            sim.log
                .iter()
                .filter(|l| m.contains(&l.text) || f.contains(&l.text))
                .count()
        };

        // A ship of neutral repute says nothing.
        sim.announce_reputation_name(&data);
        assert_eq!(name_lines(&sim), 0, "an unknown ship remarks nothing");

        // A name for mercy: one line.
        sim.reputation.insert(trait_id.clone(), high + 0.05);
        sim.announce_reputation_name(&data);
        assert_eq!(name_lines(&sim), 1, "a growing merciful name says so once");
        assert_eq!(sim.reputation_voice_band, 1);

        // Still merciful — no reprint.
        sim.announce_reputation_name(&data);
        assert_eq!(name_lines(&sim), 1, "staying merciful is not re-announced");

        // A name for fear: a second, distinct line.
        sim.reputation.insert(trait_id.clone(), low - 0.05);
        sim.announce_reputation_name(&data);
        assert_eq!(name_lines(&sim), 2, "a feared name says so afresh");
        assert_eq!(sim.reputation_voice_band, -1);
    }

    #[test]
    fn the_ship_remarks_when_its_government_slips_or_steadies() {
        // Content-depth voice round 17: the governance voice, the institutional twin of
        // the morale and polity voices. A founding ship's sound government is the silent
        // baseline; crossing into a fraying band surfaces one pooled line; a return to a
        // firm band gets its own, opposite line; staying put does not reprint.
        let (data, mut sim, _picks) = armed(31);
        let fl = &data.config.flavor;
        assert!(
            fl.stability_voice_high > 0.0 && fl.stability_fraying.len() >= 3,
            "this test needs the governance voice enabled"
        );
        let low = fl.stability_voice_low;
        let high = fl.stability_voice_high;
        let gov_lines = |sim: &SimState| {
            let fray = &data.config.flavor.stability_fraying;
            let firm = &data.config.flavor.stability_firming;
            sim.log
                .iter()
                .filter(|l| fray.contains(&l.text) || firm.contains(&l.text))
                .count()
        };

        // A founding ship's institutions are sound — the launch band is recorded, silent.
        sim.announce_stability_mood(&data);
        assert_eq!(gov_lines(&sim), 0, "a sound founding government is silent");

        // The institutions fray past the gentle line: one line.
        sim.population.stability = low - 0.05;
        sim.announce_stability_mood(&data);
        assert_eq!(gov_lines(&sim), 1, "a government slipping says so once");
        assert_eq!(sim.stability_voice_band, -1);

        // Still fraying — no reprint.
        sim.announce_stability_mood(&data);
        assert_eq!(gov_lines(&sim), 1, "staying frayed is not re-announced");

        // The institutions firm up again: a second, distinct line.
        sim.population.stability = high + 0.05;
        sim.announce_stability_mood(&data);
        assert_eq!(
            gov_lines(&sim),
            2,
            "the government steadying says so afresh"
        );
        assert_eq!(sim.stability_voice_band, 1);
    }

    #[test]
    fn a_people_merging_into_the_majority_consolidates_the_polity() {
        // Content-depth factions round 26: the positive mirror of the departure scar. A
        // tiny drifted remnant folding into the largest people lifts unity (one fewer
        // faultline), scaled by how much of the ship just merged.
        use crate::state::sim::factions::{FactionState, FactionStatus};
        let data = GameData::load().unwrap();
        let cfg = data.config.factions;
        assert!(
            cfg.assimilation_unity_lift > 0.0,
            "this test needs the assimilation-consolidation coupling enabled"
        );

        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            9,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.unity = 0.5;
        // High enough drift to assimilate, and a remnant below the share threshold.
        sim.population.cultural_drift = cfg.assimilation_drift_threshold + 0.05;
        sim.factions = vec![
            FactionState {
                faction_id: "steel_covenant".to_string(),
                members: 970,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
            FactionState {
                faction_id: "hearth_union".to_string(),
                members: 30,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
        ];

        let before = sim.population.unity;
        sim.assimilate_drifted_factions(&data);
        assert!(
            sim.factions
                .iter()
                .any(|f| f.faction_id == "hearth_union" && f.status == FactionStatus::Assimilated),
            "the tiny remnant folds into the majority"
        );
        assert!(
            sim.population.unity > before,
            "the merge consolidates the polity ({} -> {})",
            before,
            sim.population.unity
        );
    }

    #[test]
    fn losing_a_whole_people_scars_the_ships_cohesion() {
        // Content-depth factions round 24: a departure wounds cohesion beyond the bodies
        // and the craft — morale and unity both take a hit scaled by the departing
        // people's share of the ship.
        use crate::state::sim::factions::{FactionLossKind, FactionState, FactionStatus};
        let data = GameData::load().unwrap();
        let scar = data.config.factions.departure_cohesion_scar;
        assert!(
            scar > 0.0,
            "this test needs the departure-scar coupling enabled"
        );

        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            9,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.morale = 0.6;
        sim.population.unity = 0.6;
        sim.population.count = 1000;
        // Two evenly large peoples: one holds half the ship.
        sim.factions = vec![
            FactionState {
                faction_id: "steel_covenant".to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
            FactionState {
                faction_id: "hearth_union".to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
        ];

        let (m0, u0) = (sim.population.morale, sim.population.unity);
        sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, "steel_covenant");
        assert!(
            sim.population.morale < m0,
            "losing a great people wounds morale"
        );
        assert!(sim.population.unity < u0, "…and unity");
        // Half the ship departing scars by scar × 0.5.
        let expected = scar * 0.5;
        assert!(
            (m0 - sim.population.morale - expected).abs() < 1e-4,
            "the scar scales by the departing share ({} vs {expected})",
            m0 - sim.population.morale
        );
    }

    #[test]
    fn aboard_rivals_grind_at_cohesion_and_allies_lift_it() {
        // Content-depth factions round 23: the relationship-side twin of the mood→unity
        // coupling. Two large aboard rivals wear at cohesion year over year; a large
        // aboard allied bloc lifts it.
        use crate::state::sim::factions::{FactionState, FactionStatus};
        let data = GameData::load().unwrap();
        assert!(
            data.config.factions.rival_unity_friction > 0.0,
            "this test needs the relationship-cohesion coupling enabled"
        );

        // Two named peoples, evenly large, and nothing else aboard — so only their
        // mutual relationship counts. Returns the one-year unity delta.
        let run = |a: &str, b: &str| -> f32 {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                8,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.population.unity = 0.6;
            sim.factions = vec![
                FactionState {
                    faction_id: a.to_string(),
                    members: 500,
                    status: FactionStatus::Aboard,
                    approval: 0.5,
                    mood_band: 0,
                },
                FactionState {
                    faction_id: b.to_string(),
                    members: 500,
                    status: FactionStatus::Aboard,
                    approval: 0.5,
                    mood_band: 0,
                },
            ];
            let before = sim.population.unity;
            sim.apply_faction_relationship_cohesion(&data);
            sim.population.unity - before
        };

        // The Ascension and the Keepers are rivals: their sharing a hull grinds unity.
        let rival_delta = run("ascension_circle", "first_flame");
        assert!(
            rival_delta < 0.0,
            "two large aboard rivals wear at unity ({rival_delta})"
        );
        // The Hearth and the Kin are allies: their bloc lifts unity.
        let ally_delta = run("hearth_union", "verdant_kin");
        assert!(
            ally_delta > 0.0,
            "a large aboard allied bloc lifts unity ({ally_delta})"
        );
    }

    #[test]
    fn a_delighted_people_keeps_its_module_sharp() {
        // Content-depth factions round 22: the bright mirror of the neglect coupling. A
        // tending people delighted with its lot lifts its module's condition and
        // knowledge a little each year; a merely-content one (below the proud threshold)
        // lifts nothing.
        use crate::state::sim::factions::{FactionState, FactionStatus};
        let data = GameData::load().unwrap();
        let cfg = data.config.factions;
        assert!(
            cfg.proud_tender_condition_bonus > 0.0,
            "this test needs the proud-tender coupling enabled"
        );

        // Steel Covenant tends the engineering bay; hold it mid-range so no clamp hides
        // the lift, and read the delta a single year's upkeep applies.
        let run = |approval: f32| -> (f32, f32) {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                7,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.factions = vec![FactionState {
                faction_id: "steel_covenant".to_string(),
                members: sim.population.count,
                status: FactionStatus::Aboard,
                approval,
                mood_band: 0,
            }];
            {
                let bay = sim.subsystems.get_mut("engineering_bay").unwrap();
                bay.condition = 0.5;
                bay.knowledge = 0.5;
            }
            sim.apply_proud_tender_upkeep(&data);
            let bay = sim.subsystems.get("engineering_bay").unwrap();
            (bay.condition - 0.5, bay.knowledge - 0.5)
        };

        // A delighted people: its module gains exactly the year's dividend.
        let (dc, dk) = run(0.9);
        assert!(
            (dc - cfg.proud_tender_condition_bonus).abs() < 1e-6,
            "a proud people lifts its module's condition by the yearly bonus ({dc})"
        );
        assert!(
            (dk - cfg.proud_tender_knowledge_bonus).abs() < 1e-6,
            "…and its knowledge ({dk})"
        );

        // A merely-content people (below the proud threshold): no lift.
        let (dc_neutral, dk_neutral) = run(0.5);
        assert_eq!(
            (dc_neutral, dk_neutral),
            (0.0, 0.0),
            "a people below the proud threshold tends its module no better than duty"
        );
    }

    #[test]
    fn the_ship_remarks_when_its_air_goes_stale_or_clears() {
        // Content-depth voice round 23: the air (life-support) voice, the atmosphere twin
        // of the hull voice. A new ship's clean air is the silent baseline; crossing into
        // a stale band surfaces one pooled line; an overhaul back to fresh gets its own,
        // opposite line; staying put does not reprint.
        let (data, mut sim, _picks) = armed(51);
        let fl = &data.config.flavor;
        assert!(
            fl.air_voice_high > 0.0 && fl.air_stale.len() >= 3,
            "this test needs the air voice enabled"
        );
        let low = fl.air_voice_low;
        let high = fl.air_voice_high;
        let air_lines = |sim: &SimState| {
            let stale = &data.config.flavor.air_stale;
            let fresh = &data.config.flavor.air_fresh;
            sim.log
                .iter()
                .filter(|l| stale.contains(&l.text) || fresh.contains(&l.text))
                .count()
        };

        // A new ship breathes clean — the launch band is recorded, silent.
        sim.announce_air_condition(&data);
        assert_eq!(air_lines(&sim), 0, "a new ship's air is silent");

        // The air goes stale past the low line: one line.
        sim.ship.life_support = low - 0.05;
        sim.announce_air_condition(&data);
        assert_eq!(air_lines(&sim), 1, "staling air says so once");
        assert_eq!(sim.air_voice_band, -1);

        // Still stale — no reprint.
        sim.announce_air_condition(&data);
        assert_eq!(air_lines(&sim), 1, "staying stale is not re-announced");

        // An overhaul clears the air: a second, distinct line.
        sim.ship.life_support = high + 0.05;
        sim.announce_air_condition(&data);
        assert_eq!(air_lines(&sim), 2, "cleared air says so afresh");
        assert_eq!(sim.air_voice_band, 1);
    }

    #[test]
    fn the_ship_remarks_when_its_hull_groans_or_rides_sound() {
        // Content-depth voice round 22: the hull voice, the first for the ship's own
        // body. A new-built hull is the silent baseline; crossing into a groaning band
        // surfaces one pooled line; a refit back to a sound band gets its own, opposite
        // line; staying put does not reprint.
        let (data, mut sim, _picks) = armed(47);
        let fl = &data.config.flavor;
        assert!(
            fl.hull_voice_high > 0.0 && fl.hull_groaning.len() >= 3,
            "this test needs the hull voice enabled"
        );
        let low = fl.hull_voice_low;
        let high = fl.hull_voice_high;
        let hull_lines = |sim: &SimState| {
            let groan = &data.config.flavor.hull_groaning;
            let sound = &data.config.flavor.hull_sound;
            sim.log
                .iter()
                .filter(|l| groan.contains(&l.text) || sound.contains(&l.text))
                .count()
        };

        // A new-built hull is sound — the launch band is recorded, silent.
        sim.announce_hull_condition(&data);
        assert_eq!(hull_lines(&sim), 0, "a new-built hull is silent");

        // The hull wears past the low line: one line.
        sim.ship.hull_integrity = low - 0.05;
        sim.announce_hull_condition(&data);
        assert_eq!(hull_lines(&sim), 1, "an aging hull groans once");
        assert_eq!(sim.hull_voice_band, -1);

        // Still groaning — no reprint.
        sim.announce_hull_condition(&data);
        assert_eq!(hull_lines(&sim), 1, "staying worn is not re-announced");

        // A refit brings it back sound: a second, distinct line.
        sim.ship.hull_integrity = high + 0.05;
        sim.announce_hull_condition(&data);
        assert_eq!(hull_lines(&sim), 2, "a refit hull rides sound afresh");
        assert_eq!(sim.hull_voice_band, 1);
    }

    #[test]
    fn the_ship_remarks_when_the_crew_frays_or_pulls_together() {
        // Content-depth voice round 21: the unity (cohesion) voice, the fourth
        // internal-state voice. A founding crew's one-people unity is the silent
        // baseline; crossing into a fraying band surfaces one pooled line; a return to a
        // cohering band gets its own, opposite line; staying put does not reprint.
        let (data, mut sim, _picks) = armed(43);
        let fl = &data.config.flavor;
        assert!(
            fl.unity_voice_high > 0.0 && fl.unity_fraying.len() >= 3,
            "this test needs the unity voice enabled"
        );
        let low = fl.unity_voice_low;
        let high = fl.unity_voice_high;
        let unity_lines = |sim: &SimState| {
            let fray = &data.config.flavor.unity_fraying;
            let cohere = &data.config.flavor.unity_cohering;
            sim.log
                .iter()
                .filter(|l| fray.contains(&l.text) || cohere.contains(&l.text))
                .count()
        };

        // A founding crew is one people — the launch band is recorded, silent.
        sim.announce_unity_mood(&data);
        assert_eq!(unity_lines(&sim), 0, "a founding crew's unity is silent");

        // The crew frays past the low line: one line.
        sim.population.unity = low - 0.05;
        sim.announce_unity_mood(&data);
        assert_eq!(unity_lines(&sim), 1, "a crew splintering says so once");
        assert_eq!(sim.unity_voice_band, -1);

        // Still fraying — no reprint.
        sim.announce_unity_mood(&data);
        assert_eq!(unity_lines(&sim), 1, "staying frayed is not re-announced");

        // The crew pulls back together: a second, distinct line.
        sim.population.unity = high + 0.05;
        sim.announce_unity_mood(&data);
        assert_eq!(unity_lines(&sim), 2, "the crew cohering says so afresh");
        assert_eq!(sim.unity_voice_band, 1);
    }

    #[test]
    fn the_ship_remarks_when_the_crew_turns_shipborn_or_holds_baseline() {
        // Content-depth voice round 25: the adaptation voice, the physiological companion
        // to the loyalty voice. A founding crew's baseline body is the silent baseline;
        // crossing into a shipborn band surfaces one pooled line; holding to baseline gets
        // its own, opposite line; staying put does not reprint.
        let (data, mut sim, _picks) = armed(53);
        let fl = &data.config.flavor;
        assert!(
            fl.adaptation_voice_high > 0.0 && fl.crew_shipborn.len() >= 3,
            "this test needs the adaptation voice enabled"
        );
        let low = fl.adaptation_voice_low;
        let high = fl.adaptation_voice_high;
        let body_lines = |sim: &SimState| {
            let ship = &data.config.flavor.crew_shipborn;
            let base = &data.config.flavor.crew_baseline;
            sim.log
                .iter()
                .filter(|l| ship.contains(&l.text) || base.contains(&l.text))
                .count()
        };

        // A founding crew is baseline-human — the launch band is recorded, silent.
        sim.announce_adaptation_mood(&data);
        assert_eq!(body_lines(&sim), 0, "a founding crew's bodies are silent");

        // The descendants cross into shipborn: one line.
        sim.population.adaptation = high + 0.05;
        sim.announce_adaptation_mood(&data);
        assert_eq!(body_lines(&sim), 1, "a shipborn crew says so once");
        assert_eq!(sim.adaptation_voice_band, 1);

        // Still shipborn — no reprint.
        sim.announce_adaptation_mood(&data);
        assert_eq!(body_lines(&sim), 1, "staying shipborn is not re-announced");

        // Held back to baseline: a second, distinct line.
        sim.population.adaptation = low - 0.05;
        sim.announce_adaptation_mood(&data);
        assert_eq!(
            body_lines(&sim),
            2,
            "a crew held to baseline says so afresh"
        );
        assert_eq!(sim.adaptation_voice_band, -1);
    }

    #[test]
    fn the_ship_remarks_when_the_founders_fire_gutters_or_flares() {
        // Content-depth voice round 20: the loyalty voice, the identity-side twin of
        // the morale and governance voices. A founding crew's moderate loyalty is the
        // silent baseline; crossing into a guttering band (the founders' purpose fading)
        // surfaces one pooled line; a return to a bright band gets its own, opposite
        // line; staying put does not reprint.
        let (data, mut sim, _picks) = armed(37);
        let fl = &data.config.flavor;
        assert!(
            fl.loyalty_voice_high > 0.0 && fl.loyalty_guttering.len() >= 3,
            "this test needs the loyalty voice enabled"
        );
        let low = fl.loyalty_voice_low;
        let high = fl.loyalty_voice_high;
        let loyalty_lines = |sim: &SimState| {
            let gut = &data.config.flavor.loyalty_guttering;
            let bright = &data.config.flavor.loyalty_bright;
            sim.log
                .iter()
                .filter(|l| gut.contains(&l.text) || bright.contains(&l.text))
                .count()
        };

        // A founding crew's moderate devotion — the launch band is recorded, silent.
        sim.announce_loyalty_mood(&data);
        assert_eq!(
            loyalty_lines(&sim),
            0,
            "a founding crew's loyalty is silent"
        );

        // The founders' fire gutters past the low line: one line.
        sim.population.legacy_loyalty = low - 0.05;
        sim.announce_loyalty_mood(&data);
        assert_eq!(loyalty_lines(&sim), 1, "the mission fading says so once");
        assert_eq!(sim.loyalty_voice_band, -1);

        // Still guttering — no reprint.
        sim.announce_loyalty_mood(&data);
        assert_eq!(loyalty_lines(&sim), 1, "staying faded is not re-announced");

        // The dream flares bright again: a second, distinct line.
        sim.population.legacy_loyalty = high + 0.05;
        sim.announce_loyalty_mood(&data);
        assert_eq!(
            loyalty_lines(&sim),
            2,
            "the founders' fire rekindled says so afresh"
        );
        assert_eq!(sim.loyalty_voice_band, 1);
    }

    #[test]
    fn the_ships_political_climate_says_so_once_when_it_turns() {
        // Content-depth voice round 15: the polity-mood voice. Distinct from the
        // crew's spirits and from any one people's mood, this reads the aggregate
        // mood of the aboard peoples. Crossing into broad discontent surfaces one
        // pooled line; a return to broad ease gets its own, opposite line.
        let (data, mut sim, _picks) = armed(23);
        let polity_lines = |sim: &SimState| {
            let sour = &data.config.flavor.polity_souring;
            let warm = &data.config.flavor.polity_warming;
            sim.log
                .iter()
                .filter(|l| sour.contains(&l.text) || warm.contains(&l.text))
                .count()
        };

        // A fairly-treated polity (launch approvals 0.5) says nothing.
        sim.announce_polity_mood(&data);
        assert_eq!(polity_lines(&sim), 0, "a fairly-treated polity is silent");

        // Sour every aboard people: the whole political climate curdles — one line.
        for f in sim.factions.iter_mut().filter(|f| f.is_aboard()) {
            f.approval = 0.15;
        }
        sim.announce_polity_mood(&data);
        assert_eq!(polity_lines(&sim), 1, "the polity curdling says so once");
        assert_eq!(sim.polity_mood_band, -1);

        // Still sour next year — no reprint.
        sim.announce_polity_mood(&data);
        assert_eq!(polity_lines(&sim), 1, "staying sour is not re-announced");

        // Win them all back: the climate turns to broad ease — a second, distinct line.
        for f in sim.factions.iter_mut().filter(|f| f.is_aboard()) {
            f.approval = 0.9;
        }
        sim.announce_polity_mood(&data);
        assert_eq!(polity_lines(&sim), 2, "the polity settling says so afresh");
        assert_eq!(sim.polity_mood_band, 1);
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
    fn a_departing_people_takes_the_craft_of_its_tended_module() {
        // Content-depth factions round 20: shedding a people costs more than its
        // headcount — the module it tended loses a chunk of its living expertise.
        let (data, mut sim, picks) = armed(3);
        let loss = data.config.factions.departed_faction_knowledge_loss;
        assert!(loss > 0.0, "the coupling must be configured");
        let fid = picks[1].clone();
        let tended = data.factions.get(&fid).unwrap().tended_subsystem.clone();
        assert!(!tended.is_empty(), "the founding people tends a module");
        // Pin the tended module's knowledge to a known value.
        sim.subsystems.get_mut(&tended).unwrap().knowledge = 0.8;

        sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, &fid);

        assert!(!sim.is_faction_aboard(&fid), "the people are gone");
        let after = sim.subsystems.get(&tended).unwrap().knowledge;
        assert!(
            (after - (0.8 - loss)).abs() < 1e-4,
            "the tended module lost the departed's expertise (knowledge {after})"
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
