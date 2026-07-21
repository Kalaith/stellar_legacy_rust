//! Founding factions (W7): population segments carried within one campaign.
//!
//! Factions are groups of people *aboard* — orthogonal to the campaign-level
//! legacy (preservers/adaptors/wanderers), which is unchanged. v1 is structure
//! only: segments, loss/recruit, and log/event coloring. No approval meters or
//! stat modifiers yet — those layer on later.

use serde::{Deserialize, Serialize};

use crate::data::factions::{FactionDef, FactionLossKind};
use crate::data::{GameData, ResourceDelta};
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
}

impl FactionState {
    pub fn is_aboard(&self) -> bool {
        self.status == FactionStatus::Aboard
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
        });
        self.population.count += cfg.recruit_group_size;
        let name = log_name(&data.factions, faction_id);
        self.push_log(format!(
            "{name} came aboard in drydock — new blood for the long voyage."
        ));
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
        }
    }

    fn armed(seed: u64) -> (GameData, SimState, Vec<String>) {
        let data = GameData::load().unwrap();
        let picks = founding_faction_ids(&data);
        let sim = SimState::new_campaign(&data, "preservers", seed, &picks);
        (data, sim, picks)
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
}
