//! Who is aboard: rescaling the shares, losing and merging peoples,
//! assimilating the drifted, and taking a new people aboard.

use crate::data::factions::{FactionDef, FactionLossKind};
use crate::data::{GameData, ResourceDelta};
use crate::state::sim::SimState;

use super::{default_approval, log_name};
use super::{FactionState, FactionStatus};

impl SimState {
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
        // A people that *breaks away* marks the ship's name (content-depth factions round 31):
        // word spreads that this is a hull peoples flee, and its mercy reputation suffers — the
        // reputation cost of a rift, distinct from the it24 cohesion scar (the crew) and the it20
        // knowledge loss (the craft). A `Settled` departure — a people making planetfall to found
        // a colony — is a parting, not a flight, and marks nothing. Scaled by the departing
        // people's share, so a great secession is a worse name than a small remnant. Composes with
        // the it30 reputation-trade coupling (a ship known to drive its peoples off is dealt with
        // worse) and the it16 mercy voice/beat.
        if matches!(kind, FactionLossKind::Departed) {
            let rep_penalty = data.config.factions.departure_reputation_penalty;
            if rep_penalty > 0.0 && members > 0 {
                let total_before = self.population.count + members;
                let share = members as f32 / total_before.max(1) as f32;
                self.adjust_reputation("mercy", -rep_penalty * share);
            }
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

        // The peoples left aboard feel the departure by their standing to the one gone
        // (content-depth factions round 30): the mirror of the it28 recruitment reactions —
        // where taking a people aboard stirs the roster's rivalries and friendships, so does one
        // leaving. A rival of the departed is quietly relieved (approval up); an ally is saddened
        // (approval down). Read against the same catalog relationships (either direction) the it23
        // cohesion and it28 recruitment couplings use; the departed people (now not aboard) does
        // not react to its own going.
        let fac_cfg = data.config.factions;
        let relief = fac_cfg.departure_rival_approval_relief;
        let penalty = fac_cfg.departure_ally_approval_penalty;
        if relief > 0.0 || penalty > 0.0 {
            let departed_id = self.factions[idx].faction_id.clone();
            let related = |a: &str, b: &str, pick: fn(&FactionDef) -> &Vec<String>| -> bool {
                data.factions
                    .get(a)
                    .is_some_and(|d| pick(d).iter().any(|x| x == b))
                    || data
                        .factions
                        .get(b)
                        .is_some_and(|d| pick(d).iter().any(|x| x == a))
            };
            for fstate in &mut self.factions {
                if !fstate.is_aboard() {
                    continue;
                }
                if relief > 0.0 && related(&fstate.faction_id, &departed_id, |d| &d.rivals) {
                    fstate.adjust_approval(relief);
                } else if penalty > 0.0 && related(&fstate.faction_id, &departed_id, |d| &d.allies)
                {
                    fstate.adjust_approval(-penalty);
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
        // The peoples already aboard notice who you bring home (content-depth factions round
        // 28): recruiting is a political act, so the newcomer's aboard rivals bristle and its
        // aboard allies are glad — read against the same catalog relationships (either
        // direction) the it23 cohesion coupling uses. Applied after the newcomer is aboard;
        // the newcomer itself, arriving at neutral approval, does not react to its own coming.
        let rival_penalty = cfg.recruit_rival_approval_penalty;
        let ally_bonus = cfg.recruit_ally_approval_bonus;
        if rival_penalty > 0.0 || ally_bonus > 0.0 {
            let related = |a: &str, b: &str, pick: fn(&FactionDef) -> &Vec<String>| -> bool {
                data.factions
                    .get(a)
                    .is_some_and(|d| pick(d).iter().any(|x| x == b))
                    || data
                        .factions
                        .get(b)
                        .is_some_and(|d| pick(d).iter().any(|x| x == a))
            };
            for fstate in &mut self.factions {
                if fstate.faction_id == faction_id || !fstate.is_aboard() {
                    continue;
                }
                if rival_penalty > 0.0 && related(&fstate.faction_id, faction_id, |d| &d.rivals) {
                    fstate.adjust_approval(-rival_penalty);
                } else if ally_bonus > 0.0 && related(&fstate.faction_id, faction_id, |d| &d.allies)
                {
                    fstate.adjust_approval(ally_bonus);
                }
            }
        }
        // …and the newcomer reacts to who they are joining (content-depth factions round 33): the
        // newcomer's-eye mirror of the round-28 incumbent reactions. A people taken onto a ship that
        // already carries its rival boards wary (its starting approval reduced per aboard rival),
        // one joining its friends boards glad (raised per aboard ally) — so recruiting a rival's foe
        // costs on both sides. Rivalries/alliances are authored symmetric, so the newcomer's own
        // lists suffice; applied to its neutral starting approval after it is aboard.
        let wariness = cfg.recruit_newcomer_rival_wariness;
        let comfort = cfg.recruit_newcomer_ally_comfort;
        if wariness > 0.0 || comfort > 0.0 {
            if let Some(def) = data.factions.get(faction_id) {
                let aboard_and_other = |id: &str| {
                    self.factions
                        .iter()
                        .any(|f| f.faction_id == id && f.is_aboard() && f.faction_id != faction_id)
                };
                let rivals_aboard = def.rivals.iter().filter(|r| aboard_and_other(r)).count();
                let allies_aboard = def.allies.iter().filter(|a| aboard_and_other(a)).count();
                let shift = allies_aboard as f32 * comfort - rivals_aboard as f32 * wariness;
                if shift != 0.0 {
                    if let Some(newcomer) = self
                        .factions
                        .iter_mut()
                        .find(|f| f.faction_id == faction_id)
                    {
                        newcomer.adjust_approval(shift);
                    }
                }
            }
        }
        // …and taking a people in marks the ship's name (content-depth factions round 34): the
        // reputation mirror of the it31 departure penalty. Where a people *fleeing* the ship lowers
        // its mercy (a hull peoples flee), *welcoming* one aboard — giving them a berth and a future
        // in the dark — raises it: word spreads that this is a hull that takes people in. A flat
        // bonus (the mercy is in the *act* of inclusion, not the newcomer's eventual size), it
        // composes with the it30 reputation-trade coupling (a merciful ship is dealt with squarely)
        // and the it16 mercy voice/beat, exactly as the departure penalty does from the other side.
        let recruit_rep = cfg.recruit_reputation_bonus;
        if recruit_rep > 0.0 {
            self.adjust_reputation("mercy", recruit_rep);
        }
        // …and a new people is a new seam in the community (content-depth factions round 35): the
        // cohesion mirror of the it26 assimilation unity lift. Where folding a people into the
        // majority removes a faultline, taking one aboard adds one, so unity dents a little — the
        // one-time integration shock, distinct from the it23 standing grind that then reckons with
        // the new pairings year over year.
        if cfg.recruit_unity_cost > 0.0 {
            self.population.unity = (self.population.unity - cfg.recruit_unity_cost).max(0.0);
        }
        Ok(())
    }
}
