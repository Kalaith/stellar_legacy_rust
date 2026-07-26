//! How the peoples feel: the yearly movers on faction approval and on the
//! cohesion their rivalries and friendships grind out between them.

use crate::data::events::FactionApprovalDelta;
use crate::data::GameData;
use crate::state::sim::SimState;

impl SimState {
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
    /// The bright mirror of `apply_subsystem_neglect_sentiment` (content-depth factions round
    /// 29): where a people whose tended module *rots* sours (condition→approval down), a people
    /// whose module the ship keeps *excellent* is pleased (condition→approval up) — its craft
    /// honored, its domain valued. Each year an aboard tending faction's module sits at or above
    /// the honor threshold, it gains a little approval. It closes the two-sided condition↔approval
    /// loop the neglect penalty only half-drew, and is the *other* direction from `apply_proud_
    /// tender_upkeep` (which runs approval→condition): together they make investing in a people's
    /// module a virtuous circle — a kept module pleases its people (this) and pleased people keep
    /// it kept (r22). The honor threshold sits well above the neglect one, so a middle band moves
    /// no one. Deterministic, no RNG.
    pub fn apply_honored_tender_sentiment(&mut self, data: &GameData) {
        let cfg = data.config.factions;
        if cfg.honored_tender_approval_bonus <= 0.0 || cfg.honored_tender_condition_threshold <= 0.0
        {
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
            let honored = self
                .subsystems
                .get(&def.tended_subsystem)
                .is_some_and(|s| s.condition >= cfg.honored_tender_condition_threshold);
            if honored {
                fstate.adjust_approval(cfg.honored_tender_approval_bonus);
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
        // A well-kept peacekeeping corps cools the standing quarrel (content-depth subsystems round
        // 32): the corps mediates the rival friction at its source, so its condition damps the grind
        // by `1 - condition·relief`. Only the rivalry is cooled — peacekeepers quiet quarrels, not
        // friendships — so the ally solidarity below is untouched. A missing corps (condition 0) or
        // a 0 config leaves the friction to bite full.
        let security = self.subsystems.get("security").map_or(0.0, |s| s.condition);
        let friction_scale =
            (1.0 - data.config.subsystems.security_rival_friction_relief * security).max(0.0);
        let mut net = 0.0f32;
        for fstate in self.factions.iter().filter(|f| f.is_aboard()) {
            let Some(def) = data.factions.get(&fstate.faction_id) else {
                continue;
            };
            let share_f = fstate.members as f32 / total as f32;
            for rival in &def.rivals {
                net -= cfg.rival_unity_friction * share_f * share(rival) * friction_scale;
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
    /// Warm the aboard rivals of any people an event just *slighted* (content-depth
    /// factions round 32): the schadenfreude mirror the it14 favoritism spillover left
    /// out. Each *negative* approval delta lifts the wounded people's aboard rivals by a
    /// fraction of the wound — a rival humbled is a small victory to those it quarrels
    /// with — completing the rivalry spillover across both signs (favoring one people
    /// sours its rivals; slighting it cheers them). A favor (a positive delta) is handled
    /// by `apply_rival_approval_spillover`; this fires only on the down-swing. No RNG.
    pub fn apply_rival_approval_schadenfreude(
        &mut self,
        data: &GameData,
        deltas: &[FactionApprovalDelta],
    ) {
        let glee = data.config.factions.rival_approval_schadenfreude;
        if glee <= 0.0 {
            return;
        }
        // Gather (rival, bonus) from the immutable catalog first, then apply — so the
        // read of `data.factions.rivals` and the mutation of `self.factions` don't overlap.
        let mut bonuses: Vec<(String, f32)> = Vec::new();
        for delta in deltas {
            if delta.delta >= 0.0 || !self.is_faction_aboard(&delta.id) {
                continue;
            }
            if let Some(def) = data.factions.get(&delta.id) {
                for rival in &def.rivals {
                    if self.is_faction_aboard(rival) {
                        // delta < 0, so `-glee * delta` is a positive lift.
                        bonuses.push((rival.clone(), -glee * delta.delta));
                    }
                }
            }
        }
        for (rival_id, bonus) in bonuses {
            if let Some(state) = self
                .factions
                .iter_mut()
                .find(|f| f.faction_id == rival_id && f.is_aboard())
            {
                state.adjust_approval(bonus);
            }
        }
    }
    /// Sour the aboard allies of any people an event just *slighted* (content-depth
    /// factions round 32): the commiseration mirror the it17 coalition spillover left
    /// out. Each *negative* approval delta drags the wounded people's aboard allies down
    /// by a fraction of the wound — a friend wronged is a hurt the coalition shares —
    /// completing the alliance spillover across both signs (favoring one people warms its
    /// allies; slighting it stings them). A favor (a positive delta) is handled by
    /// `apply_ally_approval_spillover`; this fires only on the down-swing. No RNG.
    pub fn apply_ally_approval_commiseration(
        &mut self,
        data: &GameData,
        deltas: &[FactionApprovalDelta],
    ) {
        let commis = data.config.factions.ally_approval_commiseration;
        if commis <= 0.0 {
            return;
        }
        // Gather (ally, penalty) from the immutable catalog first, then apply — so the
        // read of `data.factions.allies` and the mutation of `self.factions` don't overlap.
        let mut penalties: Vec<(String, f32)> = Vec::new();
        for delta in deltas {
            if delta.delta >= 0.0 || !self.is_faction_aboard(&delta.id) {
                continue;
            }
            if let Some(def) = data.factions.get(&delta.id) {
                for ally in &def.allies {
                    if self.is_faction_aboard(ally) {
                        // delta < 0, so `commis * delta` is a negative drag.
                        penalties.push((ally.clone(), commis * delta.delta));
                    }
                }
            }
        }
        for (ally_id, penalty) in penalties {
            if let Some(state) = self
                .factions
                .iter_mut()
                .find(|f| f.faction_id == ally_id && f.is_aboard())
            {
                state.adjust_approval(penalty);
            }
        }
    }
    /// Let the people a charter was *uniquely called to* feel its conclusion (content-depth
    /// charters round 32): each founding people the writ was gated on (`requires_faction_aboard`)
    /// takes pride when the work is seen through and is let down when it is botched. On a
    /// completed charter every named aboard faction gains `charter_completion_pride`; on a failed
    /// one every named aboard faction loses `charter_failure_letdown` — so a mission's outcome
    /// now moves the crew's *politics* beside its pay, its name, its morale (it31), and the deed
    /// it leaves on record (it14/it30). The charter→faction pride/letdown coupling. No RNG.
    pub fn apply_charter_outcome_faction_sentiment(
        &mut self,
        data: &GameData,
        template: &crate::data::contracts::ContractTemplate,
        failed: bool,
    ) {
        let delta = if failed {
            -data.config.factions.charter_failure_letdown
        } else {
            data.config.factions.charter_completion_pride
        };
        if delta == 0.0 {
            return;
        }
        for id in &template.requires_faction_aboard {
            if let Some(state) = self
                .factions
                .iter_mut()
                .find(|f| f.faction_id == *id && f.is_aboard())
            {
                state.adjust_approval(delta);
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
    /// A people grows content or discontent as the ship's *character* honors or affronts its
    /// values (content-depth factions round 27): the reverse of `apply_dominant_reputation_lean`,
    /// and the half of the `reputation_leanings` loop that was missing. Where that lets whoever
    /// runs the ship bend its reputation toward their leanings, this lets the reputation the ship
    /// has actually earned bend every aboard people's *approval* — a merciful ship contents the
    /// factions that prize mercy and sours the ones that scorn it, a ruthless one the reverse.
    /// For each aboard people, the yearly shift is `scale · Σ leaning · (reputation − 0.5)·2` over
    /// its leanings — the ship's traits recentred to [−1, 1] so a neutral character (every trait
    /// at 0.5, the launch state) pulls no one either way. Reads reputation + the catalog leanings;
    /// deterministic, no RNG. Inert when the scale is 0.
    pub fn apply_reputation_alignment_sentiment(&mut self, data: &GameData) {
        let scale = data.config.factions.reputation_alignment_approval_scale;
        if scale == 0.0 {
            return;
        }
        // Read phase (immutable): each aboard people's alignment with the ship's earned character.
        let adjustments: Vec<(usize, f32)> = self
            .factions
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_aboard())
            .filter_map(|(i, f)| {
                let def = data.factions.get(&f.faction_id)?;
                if def.reputation_leanings.is_empty() {
                    return None;
                }
                let alignment: f32 = def
                    .reputation_leanings
                    .iter()
                    .map(|(trait_id, lean)| lean * (self.reputation(trait_id) - 0.5) * 2.0)
                    .sum();
                Some((i, scale * alignment))
            })
            .collect();
        // Write phase (mutable): the ship's name warms or cools each people toward it.
        for (i, delta) in adjustments {
            self.factions[i].adjust_approval(delta);
        }
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
}
