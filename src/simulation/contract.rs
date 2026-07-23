//! Contract success scoring and progression (GDD §5.2).

use crate::data::contracts::{ContractPhase, ContractTemplate, MetricKind};
use crate::data::ResourceDelta;
use crate::state::sim::{ActiveContract, MetricState, MilestoneState, SimState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessLevel {
    Complete,
    Partial,
    Pyrrhic,
    Failure,
}

impl SuccessLevel {
    pub fn label(self) -> &'static str {
        match self {
            SuccessLevel::Complete => "Complete",
            SuccessLevel::Partial => "Partial",
            SuccessLevel::Pyrrhic => "Pyrrhic",
            SuccessLevel::Failure => "Failure",
        }
    }
}

/// `success_score = Σ( min(1, current/target) * weight )`, banded per GDD §5.2.
pub fn score_success(metrics: &[MetricState]) -> (f32, SuccessLevel) {
    let score: f32 = metrics
        .iter()
        .map(|m| {
            let ratio = if m.target <= 0.0 {
                1.0
            } else {
                (m.current / m.target).min(1.0)
            };
            ratio * m.weight
        })
        .sum();

    let level = if score >= 0.9 {
        SuccessLevel::Complete
    } else if score >= 0.7 {
        SuccessLevel::Partial
    } else if score >= 0.4 {
        SuccessLevel::Pyrrhic
    } else {
        SuccessLevel::Failure
    };
    (score, level)
}

/// Everything one month of contract time produced that the tick must surface.
#[derive(Debug, Default)]
pub struct ContractProgress {
    pub reached_milestones: Vec<String>,
    /// Set when this month crossed into a new authored phase (W2).
    pub phase_changed: Option<ContractPhase>,
    /// Set the month the contract reaches its full duration.
    pub completed: Option<(f32, SuccessLevel)>,
}

/// Whether a charter's in-world availability gate is met right now (content-depth
/// charters round 12): every people it names is aboard. The charter-level parallel
/// to the outcome gates — checked by both the writ board (to lock/label it) and the
/// select action (so a locked writ can't be put under consideration). `min_renown`
/// stays a separate, cross-campaign gate; this reads the living roster.
pub fn meets_in_world_gate(sim: &SimState, template: &ContractTemplate) -> bool {
    template
        .requires_faction_aboard
        .iter()
        .all(|id| sim.is_faction_aboard(id))
        // Deed gates (content-depth charters round 14): a writ can require the ship
        // to have *done* something (how a charter arc unlocks its next leg) or be
        // barred by a dark deed on record.
        && template
            .requires_consequence
            .iter()
            .all(|tag| sim.consequences.contains(tag))
        && !template
            .forbidden_consequence
            .iter()
            .any(|tag| sim.consequences.contains(tag))
        // Reputation gates (content-depth charters round 16): the writ board reflects
        // the ship's cumulative character — a merciful name opens some work, a feared
        // one others.
        && template
            .min_reputation
            .iter()
            .all(|g| sim.reputation(&g.id) >= g.threshold)
        && template
            .max_reputation
            .iter()
            .all(|g| sim.reputation(&g.id) <= g.threshold)
}

/// Grant a charter's completion reward (content-depth charters round 15): the
/// lasting capability a mission leaves the ship — chiefly subsystem boons kept
/// across voyages — applied once when the charter is seen through to full term.
/// Returns the narration line (empty if the reward is empty). No-op for an ordinary
/// charter.
pub fn apply_completion_reward(sim: &mut SimState, template: &ContractTemplate) -> Option<String> {
    let reward = &template.completion_reward;
    if reward.is_none() {
        return None;
    }
    sim.resources.apply(&reward.resource);
    sim.population.apply(&reward.population);
    for delta in &reward.subsystem_deltas {
        if let Some(state) = sim.subsystems.get_mut(&delta.id) {
            state.condition = (state.condition + delta.condition).clamp(0.0, 1.0);
            state.knowledge = (state.knowledge + delta.knowledge).clamp(0.0, 1.0);
        }
    }
    // A whole voyage of one kind of work shapes the ship's character (content-depth
    // charters round 17): the mission the reputation unlocked now builds it further.
    for delta in &reward.reputation_deltas {
        sim.adjust_reputation(&delta.id, delta.delta);
    }
    // …and earns the goodwill of the peoples it served (content-depth charters round
    // 19): a completed mission can leave the named aboard factions delighted, feeding
    // the round-19 gift beats. Factions not aboard are ignored.
    for delta in &reward.faction_approval_deltas {
        if let Some(state) = sim
            .factions
            .iter_mut()
            .find(|f| f.faction_id == delta.id && f.is_aboard())
        {
            state.adjust_approval(delta.delta);
        }
    }
    Some(if reward.log.is_empty() {
        format!("The lessons of {} stay with the ship.", template.name)
    } else {
        reward.log.clone()
    })
}

/// Mark the ship's name for a charter it did not see through (content-depth charters
/// round 18): the negative mirror of `apply_completion_reward`, applied once when a
/// charter concludes at Failure. A defaulted or abandoned mission costs the ship's
/// *character* — a hardened mercy for a relief run given up, a name for folding
/// (`resolve`) for any writ quit half-done. Returns the narration line (empty for a
/// charter whose failure marks nothing). No-op for an ordinary charter.
pub fn apply_abandonment(sim: &mut SimState, template: &ContractTemplate) -> Option<String> {
    let ab = &template.abandonment;
    if ab.is_none() {
        return None;
    }
    for delta in &ab.reputation_deltas {
        sim.adjust_reputation(&delta.id, delta.delta);
    }
    Some(if ab.log.is_empty() {
        format!(
            "Word travels the dark that the ship gave up the {}.",
            template.name
        )
    } else {
        ab.log.clone()
    })
}

/// Instantiate an active contract from a template at the current sim state.
pub fn start_contract(template: &ContractTemplate, sim: &SimState) -> ActiveContract {
    ActiveContract {
        template_id: template.id.clone(),
        name: template.name.clone(),
        objective: template.objective,
        target_duration_years: template.target_duration_years,
        months_elapsed: 0,
        phase: ContractPhase::Preparation,
        phases: template.phases.clone(),
        phase_index: 0,
        metrics: template
            .success_metrics
            .iter()
            .map(|m| MetricState {
                id: m.id.clone(),
                kind: m.kind,
                name: m.name.clone(),
                weight: m.weight,
                target: m.target,
                current: 0.0,
            })
            .collect(),
        milestones: template
            .milestones
            .iter()
            .map(|m| MilestoneState {
                id: m.id.clone(),
                name: m.name.clone(),
                progress_threshold: m.progress_threshold,
                reached: false,
                reward: m.reward,
            })
            .collect(),
        starting_population: sim.population.count,
        objective_target: template.objective_target,
        objective_unit: template.objective_unit.clone(),
        objective_progress: 0.0,
        // Beats are laid out at LAUNCH by the caller (W6); a bare contract has
        // none until then.
        beats: Vec::new(),
        healthy_food_months: 0,
        healthy_energy_months: 0,
        tags: template.tags.clone(),
        beat_families: template.beat_families.clone(),
        drift_beats_fired: 0,
        adaptation_beats_fired: 0,
        crisis_beats_fired: 0,
        loyalty_beats_fired: 0,
        stability_beats_fired: 0,
        anniversaries_fired: 0,
        flourish_beats_fired: 0,
        objective_beats_fired: 0,
        homecoming_beat_fired: false,
        hazard: template.hazard,
        scheduled_beats: template.scheduled_beats.clone(),
        scheduled_beats_fired: 0,
        objective_subsystem: template.objective_subsystem.clone(),
    }
}

/// Advance the active contract by one month (W2): step the timeline, recompute
/// the authored phase, accrue objective work while on-station, refresh metrics,
/// pay any newly reached milestone, and detect completion. `speed` is the ship
/// loadout's aggregate speed, which quickens objective work.
pub fn advance_contract(
    sim: &mut SimState,
    config: &crate::data::GameConfig,
    speed: i32,
) -> ContractProgress {
    let population_count = sim.population.count;
    let unity = sim.population.unity;
    let food_ok = sim.resources.food >= config.low_food_threshold;
    let energy_ok = sim.resources.energy >= config.low_energy_threshold;
    let progress_per_speed = config.ship.contract_progress_per_speed;

    // The module this mission leans on scales how fast its work accrues (content-depth
    // subsystems round 14): a pristine bay works at the base rate, a degraded one
    // slower. Read before the mutable contract borrow. Penalty-below-full keeps the
    // baseline, so a well-kept ship's objective is unchanged.
    let objective_condition = sim
        .contract
        .as_ref()
        .filter(|c| !c.objective_subsystem.is_empty())
        .map(|c| {
            let cond = sim
                .subsystems
                .get(&c.objective_subsystem)
                .map_or(1.0, |s| s.condition);
            (1.0 - config.subsystems.objective_condition_penalty * (1.0 - cond)).max(0.0)
        })
        .unwrap_or(1.0);

    let mut out = ContractProgress::default();

    // Mutate the contract in a scope so its borrow ends before we grant any
    // milestone rewards to the shared resource pool.
    let rewards = {
        let Some(contract) = sim.contract.as_mut() else {
            return out;
        };

        let prev_phase = contract.phase;
        contract.months_elapsed += 1;
        // Provisioning discipline accrues month by month: each upkeep store
        // above its crisis threshold banks credit toward ResourceEfficiency.
        contract.healthy_food_months += food_ok as u32;
        contract.healthy_energy_months += energy_ok as u32;
        let (index, phase) = contract.phase_at(contract.months_elapsed);
        contract.phase_index = index;
        contract.phase = phase;
        if phase != prev_phase {
            out.phase_changed = Some(phase);
        }

        // Objective work happens only on-station (Operation): base_rate spreads
        // the target across the operation window, and ship speed quickens it.
        if phase == ContractPhase::Operation {
            let operation_months = contract.operation_months().max(1);
            let base_rate = contract.objective_target / operation_months as f32;
            let speed_factor = 1.0 + speed.max(0) as f32 * progress_per_speed;
            contract.objective_progress += base_rate * speed_factor * objective_condition;
        }

        let progress = contract.progress();
        let mut reached_rewards = Vec::new();
        for milestone in &mut contract.milestones {
            if !milestone.reached && progress >= milestone.progress_threshold {
                milestone.reached = true;
                out.reached_milestones.push(milestone.name.clone());
                reached_rewards.push(milestone.reward);
            }
        }

        let objective_fraction = contract.objective_fraction();
        let upkeep_health = contract.upkeep_health();
        for metric in &mut contract.metrics {
            metric.current = match metric.kind {
                MetricKind::PopulationSurvival => {
                    if contract.starting_population == 0 {
                        1.0
                    } else {
                        population_count as f32 / contract.starting_population as f32
                    }
                }
                // Mission completion now reads the quantified objective (W2).
                MetricKind::MissionCompletion => objective_fraction,
                // Provisioning discipline across the whole voyage: the fraction
                // of elapsed months each upkeep store held above its crisis
                // threshold. A ship that never ran low scores 1.0; every lean
                // month drags the score down for the rest of the contract.
                MetricKind::ResourceEfficiency => upkeep_health,
                MetricKind::SocialCohesion => unity,
            };
        }

        if contract.months_elapsed >= contract.total_months() {
            out.completed = Some(score_success(&contract.metrics));
        }

        reached_rewards
    };

    // A milestone's reward lands the month it is first reached.
    for reward in rewards {
        sim.resources.apply(&reward);
    }
    out
}

/// Turn the ship for home early (W2): jump the contract to the start of its
/// first Return segment, freezing objective progress where it stands. No-op
/// without a contract, without a Return segment, or already in/past Return.
/// Returns whether the ship turned back.
pub fn jump_to_return(sim: &mut SimState) -> bool {
    let Some(contract) = sim.contract.as_mut() else {
        return false;
    };
    let Some(return_index) = contract.first_return_index() else {
        return false;
    };
    let return_start = contract.segment_start(return_index);
    if contract.months_elapsed >= return_start {
        return false;
    }
    contract.months_elapsed = return_start;
    contract.phase_index = return_index;
    contract.phase = contract.phases[return_index].kind;
    true
}

/// Prorate a charter reward by objective completion (W2): pay = reward ×
/// fraction, rounded toward zero per resource. Every completion pays exactly
/// this — full-term or truncated; zero objective progress ⇒ zero pay.
pub fn prorated_reward(reward: &ResourceDelta, fraction: f32) -> ResourceDelta {
    ResourceDelta {
        credits: (reward.credits as f32 * fraction) as i64,
        energy: (reward.energy as f32 * fraction) as i64,
        minerals: (reward.minerals as f32 * fraction) as i64,
        food: (reward.food as f32 * fraction) as i64,
        influence: (reward.influence as f32 * fraction) as i64,
    }
}

/// The log line narrating a crossing into `phase` (W2).
/// The log line for entering `phase` on its `occurrence`-th time this voyage
/// (1-based). Draws from the data-driven `flavor.phase_lines` pool so a
/// double-hop's second departure/arrival reads differently from the first
/// (content-depth voice round 3); an empty or missing pool falls back to the
/// built-in line so the log is never blank.
pub fn phase_transition_line(
    flavor: &crate::data::FlavorConfig,
    phase: ContractPhase,
    occurrence: usize,
) -> String {
    let key = match phase {
        ContractPhase::Preparation => "preparation",
        ContractPhase::Travel => "travel",
        ContractPhase::Operation => "operation",
        ContractPhase::Return => "return",
        ContractPhase::Completion => "completion",
    };
    if let Some(pool) = flavor.phase_lines.get(key) {
        if !pool.is_empty() {
            return pool[occurrence.saturating_sub(1) % pool.len()].clone();
        }
    }
    match phase {
        ContractPhase::Preparation => "Standing by for departure.",
        ContractPhase::Travel => "Departure burn complete — the ship is underway.",
        ContractPhase::Operation => "The ship makes station. On-site operations begin.",
        ContractPhase::Return => "Objective work concluded — course set for home.",
        ContractPhase::Completion => "The ship returns to its home berth.",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::contracts::MetricKind;
    use crate::data::GameData;

    #[test]
    fn a_degraded_key_module_works_the_mission_slower() {
        // Content-depth subsystems round 14: the subsystem axis's first coupling to
        // the mission. The deep vein survey's work leans on the engineering bay; a
        // rotting bay mines slower than a pristine one, while a charter with no key
        // module is indifferent to any module's state.
        let data = GameData::load().unwrap();
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        assert_eq!(template.objective_subsystem, "engineering_bay");

        // Objective banked over one operation year at a given bay condition.
        let mined = |bay: f32| -> f32 {
            let picks = crate::state::sim::founding_faction_ids(&data);
            let mut sim = SimState::new_campaign(&data, "preservers", 73, &picks);
            sim.contract = Some(start_contract(&template, &sim));
            sim.subsystems.get_mut("engineering_bay").unwrap().condition = bay;
            // Fast-forward the clock into the Operation window, then bank a year.
            let ops_start = template
                .phases
                .iter()
                .take_while(|p| p.kind != ContractPhase::Operation)
                .map(|p| p.years * 12)
                .sum::<u32>();
            sim.contract.as_mut().unwrap().months_elapsed = ops_start;
            let before = sim.contract.as_ref().unwrap().objective_progress;
            for _ in 0..12 {
                advance_contract(&mut sim, &data.config, 0);
            }
            sim.contract.as_ref().unwrap().objective_progress - before
        };

        let pristine = mined(1.0);
        let rotting = mined(0.2);
        assert!(pristine > 0.0, "a working bay banks the mission's work");
        assert!(
            rotting < pristine,
            "a rotting bay mines slower than a pristine one ({rotting} vs {pristine})"
        );
    }

    #[test]
    fn a_route_toll_wears_the_ship_every_year_of_its_voyage() {
        // Content-depth charters round 13: a charter whose nature wears at a ship
        // exacts a steady per-year drain — hazard's deterministic companion. The
        // coronal tap's radiation-and-heat toll drops morale and hull each year;
        // an ordinary survey exacts nothing.
        use crate::simulation::tick::advance_year;
        let mut data = GameData::load().unwrap();
        // Isolate the toll: no reactive rolls, no threshold beats. Voyage drift
        // still wears both ships, but it wears them identically, so the *difference*
        // is the route's own standing toll.
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        data.config.campaign_skeleton.drift_beats.clear();
        data.config.campaign_skeleton.adaptation_beats.clear();
        data.config.campaign_skeleton.crisis_beats.clear();
        data.config.campaign_skeleton.flourish_beats.clear();
        data.config.campaign_skeleton.objective_beats.clear();
        data.config.campaign_skeleton.depopulation_beats.clear();
        let toll = &data.contracts.get("coronal_tap").unwrap().annual_toll;
        assert!(!toll.is_none(), "the coronal tap is a punishing route");
        assert!(
            data.contracts
                .get("deep_vein_survey")
                .unwrap()
                .annual_toll
                .is_none(),
            "an ordinary survey exacts no standing toll"
        );

        let fly = |charter: &str| -> (f32, f32) {
            let picks = crate::state::sim::founding_faction_ids(&data);
            let mut sim = SimState::new_campaign(&data, "preservers", 61, &picks);
            sim.resources.food = 1_000_000; // isolate the toll from famine
            let template = data.contracts.get(charter).unwrap().clone();
            sim.contract = Some(start_contract(&template, &sim));
            sim.contract.as_mut().unwrap().beats.clear();
            let (m0, h0) = (sim.population.morale, sim.ship.hull_integrity);
            for _ in 0..10 {
                advance_year(&mut sim, &data);
            }
            (m0 - sim.population.morale, h0 - sim.ship.hull_integrity)
        };
        let (tapped_morale, tapped_hull) = fly("coronal_tap");
        let (survey_morale, survey_hull) = fly("deep_vein_survey");
        assert!(
            tapped_morale > survey_morale && tapped_hull > survey_hull,
            "the star's reach wears morale and hull faster than a quiet survey \
             (tap {tapped_morale}/{tapped_hull} vs survey {survey_morale}/{survey_hull})"
        );
    }

    #[test]
    fn completing_a_charter_shapes_the_ships_character() {
        // Content-depth charters round 17: the missions a reputation unlocks build it
        // further. Seeing the sanctuary run through deepens the ship's mercy; the
        // hard contract hardens it — a self-reinforcing spiral through the missions.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
        let hard = data.contracts.get("the_hard_contract").unwrap();
        assert!(
            !sanctuary.completion_reward.reputation_deltas.is_empty()
                && !hard.completion_reward.reputation_deltas.is_empty(),
            "both reputation-gated charters shape character on completion"
        );

        let mut kind = SimState::new_campaign(&data, "preservers", 87, &picks);
        let m0 = kind.reputation("mercy");
        apply_completion_reward(&mut kind, sanctuary);
        assert!(
            kind.reputation("mercy") > m0,
            "a voyage of carrying refugees deepens the ship's mercy"
        );

        let mut cold = SimState::new_campaign(&data, "preservers", 88, &picks);
        let c0 = cold.reputation("mercy");
        apply_completion_reward(&mut cold, hard);
        assert!(
            cold.reputation("mercy") < c0,
            "a voyage of cold enforcement hardens it"
        );
    }

    #[test]
    fn a_completed_mission_earns_its_peoples_goodwill() {
        // Content-depth charters round 19: a mission the ship flew for a people can
        // leave that people delighted — the completion goodwill that feeds the
        // round-19 gift beats. Only lands on a faction actually aboard.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
        // The sanctuary run rewards the Hearth, who ride in the founding set.
        let run = data.contracts.get("the_sanctuary_run").unwrap();
        assert!(
            run.completion_reward
                .faction_approval_deltas
                .iter()
                .any(|d| d.id == "hearth_union"),
            "the sanctuary run earns the Hearth's goodwill"
        );
        assert!(sim.is_faction_aboard("hearth_union"));
        let before = sim
            .factions
            .iter()
            .find(|f| f.faction_id == "hearth_union")
            .unwrap()
            .approval;
        apply_completion_reward(&mut sim, run);
        let after = sim
            .factions
            .iter()
            .find(|f| f.faction_id == "hearth_union")
            .unwrap()
            .approval;
        assert!(
            after > before,
            "carrying the frightened home leaves the Hearth glad it came"
        );
    }

    #[test]
    fn abandoning_a_charter_marks_the_ships_name() {
        // Content-depth charters round 18: the negative mirror of the completion
        // reward — the first charter effect keyed to failure. Giving up the sanctuary
        // run hardens the mercy the crew couldn't keep and earns a name for folding;
        // giving up the hard contract is a *merciful* fold — the ship that would not,
        // in the end, strip a home comes home kinder but still a hull that folds.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
        let hard = data.contracts.get("the_hard_contract").unwrap();
        assert!(
            !sanctuary.abandonment.is_none() && !hard.abandonment.is_none(),
            "both relief charters mark the ship's name when defaulted"
        );

        // Abandon the sanctuary run: mercy hardens and resolve falls.
        let mut a = SimState::new_campaign(&data, "preservers", 91, &picks);
        let (m0, r0) = (a.reputation("mercy"), a.reputation("resolve"));
        apply_abandonment(&mut a, sanctuary);
        assert!(
            a.reputation("mercy") < m0,
            "leaving refugees behind hardens the ship"
        );
        assert!(
            a.reputation("resolve") < r0,
            "a relief run given up earns a name for folding"
        );

        // Abandon the hard contract: a merciful fold — mercy rises, resolve still falls.
        let mut b = SimState::new_campaign(&data, "preservers", 92, &picks);
        let (m1, r1) = (b.reputation("mercy"), b.reputation("resolve"));
        apply_abandonment(&mut b, hard);
        assert!(
            b.reputation("mercy") > m1,
            "refusing to finish the cruel job comes home kinder"
        );
        assert!(
            b.reputation("resolve") < r1,
            "but it is still, to the dark, a hull that folded"
        );

        // An ordinary charter marks nothing on a failed conclusion.
        let ordinary = data.contracts.get("deep_vein_survey").unwrap();
        let mut c = SimState::new_campaign(&data, "preservers", 93, &picks);
        assert!(
            apply_abandonment(&mut c, ordinary).is_none(),
            "an ordinary charter's failure costs only its pay"
        );
    }

    #[test]
    fn a_completed_charter_leaves_a_lasting_capability() {
        // Content-depth charters round 15: a mission seen through leaves the ship a
        // skill it keeps, beyond the pay. The Karst Works masters extraction (an
        // engineering boon); an ordinary charter leaves nothing.
        let data = GameData::load().unwrap();
        let works = data.contracts.get("the_karst_works").unwrap();
        assert!(
            !works.completion_reward.is_none(),
            "the works leave a legacy"
        );

        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 77, &picks);
        // Room to grow (a fresh bay is already near full; start it low to see the lift).
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.5;
        let before = sim.subsystems["engineering_bay"].knowledge;
        let line = apply_completion_reward(&mut sim, works);
        assert!(line.is_some(), "the boon narrates itself");
        assert!(
            sim.subsystems["engineering_bay"].knowledge > before,
            "building the great works masters extraction for good"
        );

        // A charter with no completion reward changes nothing and says nothing.
        let ordinary = data
            .contracts
            .iter()
            .map(|(_, c)| c)
            .find(|c| c.completion_reward.is_none())
            .expect("some charter leaves no legacy");
        let k0 = sim.subsystems["engineering_bay"].knowledge;
        assert!(apply_completion_reward(&mut sim, ordinary).is_none());
        assert_eq!(sim.subsystems["engineering_bay"].knowledge, k0);
    }

    #[test]
    fn the_writ_board_reflects_the_ships_reputation() {
        // Content-depth charters round 16: the board reads the ship's cumulative
        // character. The sanctuary run opens only to a hull famous for mercy; the
        // enforcement writ only to one known not to flinch — and neither is offered
        // to a ship whose name is still neutral.
        let data = GameData::load().unwrap();
        let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
        let hard = data.contracts.get("the_hard_contract").unwrap();
        let mercy_floor = sanctuary.min_reputation[0].threshold;
        let mercy_ceiling = hard.max_reputation[0].threshold;

        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 83, &picks);

        // A neutral name (0.5) opens neither door.
        assert!(
            !meets_in_world_gate(&sim, sanctuary) && !meets_in_world_gate(&sim, hard),
            "a ship with no reputation yet is offered neither"
        );

        // A merciful name opens the sanctuary run and keeps the hard writ shut.
        sim.reputation.insert("mercy".to_string(), mercy_floor);
        assert!(
            meets_in_world_gate(&sim, sanctuary),
            "a famous mercy is trusted with the vulnerable"
        );
        assert!(
            !meets_in_world_gate(&sim, hard),
            "a merciful ship is not offered the cold work"
        );

        // A feared name opens the enforcement writ and shuts the sanctuary run.
        sim.reputation.insert("mercy".to_string(), mercy_ceiling);
        assert!(
            meets_in_world_gate(&sim, hard),
            "a ship known not to flinch is hired for the hard thing"
        );
        assert!(
            !meets_in_world_gate(&sim, sanctuary),
            "and is not trusted with a people's children"
        );
    }

    #[test]
    fn a_charter_arc_unlocks_its_next_leg_only_once_the_first_is_done() {
        // Content-depth charters round 14: a charter arc. The Karst Belt works are
        // offered only to a ship that has proven the veins (the survey's completion
        // mark) — and, being delicate high-trust work, only to a ship that has not
        // broken its word.
        let data = GameData::load().unwrap();
        let survey = data.contracts.get("deep_vein_survey").unwrap();
        let works = data.contracts.get("the_karst_works").unwrap();
        let seed = &survey.completion_consequence;
        assert!(!seed.is_empty(), "the survey seeds an arc on completion");
        assert_eq!(&works.requires_consequence, &vec![seed.clone()]);

        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 71, &picks);

        // A ship that has never surveyed the belt is not offered the works…
        assert!(
            !meets_in_world_gate(&sim, works),
            "the permanent works need the veins proven first"
        );
        // …but a ship that completed the survey is.
        sim.consequences.push(seed.clone());
        assert!(
            meets_in_world_gate(&sim, works),
            "proving the veins unlocks the works"
        );
        // …unless it has broken a bargain — the consortium won't trust it.
        sim.consequences.push("broke_a_bargain".to_string());
        assert!(
            !meets_in_world_gate(&sim, works),
            "a known oathbreaker is barred from the delicate works"
        );
    }

    #[test]
    fn an_in_world_charter_is_offered_only_while_its_people_are_aboard() {
        // Content-depth charters round 12: the in-world availability gate. The
        // Seedbearers' Writ is offered only to a ship carrying the Verdant Kin —
        // it appears when they are aboard and vanishes if they leave, distinct
        // from the cross-campaign renown gate.
        use crate::state::sim::factions::{FactionState, FactionStatus};
        let data = GameData::load().unwrap();
        let template = data.contracts.get("the_seedbearers_writ").unwrap();
        assert_eq!(template.requires_faction_aboard, vec!["verdant_kin"]);

        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 47, &picks);

        // A ship without the Verdant Kin is not offered the writ…
        let fs = |id: &str| FactionState {
            faction_id: id.to_string(),
            members: 500,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        };
        sim.factions = vec![fs("steel_covenant"), fs("hearth_union")];
        assert!(
            !meets_in_world_gate(&sim, template),
            "a ship without the gardeners is not trusted with the greening"
        );
        // …but a ship that carries them is.
        sim.factions.push(fs("verdant_kin"));
        assert!(
            meets_in_world_gate(&sim, template),
            "carrying the Verdant Kin unlocks the seedworld writ"
        );
        // A charter with no in-world gate is always offered.
        let ungated = data.contracts.get("founding_colony").unwrap();
        assert!(meets_in_world_gate(&sim, ungated));
    }

    #[test]
    fn a_double_hop_reads_a_different_line_on_its_second_departure() {
        let data = GameData::load().unwrap();
        let fl = &data.config.flavor;
        // The twin_survey re-enters Travel and Operation; the second entry must
        // not reprint the first entry's line (content-depth voice round 3).
        let first_travel = phase_transition_line(fl, ContractPhase::Travel, 1);
        let second_travel = phase_transition_line(fl, ContractPhase::Travel, 2);
        assert_ne!(
            first_travel, second_travel,
            "a double-hop's second departure should read differently"
        );
        // Out-of-range occurrences wrap rather than panic.
        let _ = phase_transition_line(fl, ContractPhase::Operation, 99);
    }

    fn metric(weight: f32, target: f32, current: f32) -> MetricState {
        MetricState {
            id: "m".into(),
            kind: MetricKind::MissionCompletion,
            name: "m".into(),
            weight,
            target,
            current,
        }
    }

    #[test]
    fn score_bands_match_gdd_thresholds() {
        let full = vec![metric(1.0, 1.0, 1.0)];
        assert_eq!(score_success(&full).1, SuccessLevel::Complete);

        let partial = vec![metric(1.0, 1.0, 0.75)];
        assert_eq!(score_success(&partial).1, SuccessLevel::Partial);

        let pyrrhic = vec![metric(1.0, 1.0, 0.5)];
        assert_eq!(score_success(&pyrrhic).1, SuccessLevel::Pyrrhic);

        let failure = vec![metric(1.0, 1.0, 0.1)];
        assert_eq!(score_success(&failure).1, SuccessLevel::Failure);
    }

    #[test]
    fn milestone_reward_lands_once_on_reach() {
        use crate::data::{GameData, ResourceDelta};
        use crate::state::sim::SimState;

        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            31,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let mut contract = start_contract(data.contracts.get("deep_vein_survey").unwrap(), &sim);
        // Force the first milestone to fire immediately with a known reward.
        contract.milestones[0].progress_threshold = 0.0;
        contract.milestones[0].reached = false;
        contract.milestones[0].reward = ResourceDelta {
            minerals: 500,
            ..Default::default()
        };
        sim.contract = Some(contract);

        let before = sim.resources.minerals;
        advance_contract(&mut sim, &data.config, 0);
        assert_eq!(
            sim.resources.minerals,
            before + 500,
            "the reward lands the year the milestone is reached"
        );

        let after = sim.resources.minerals;
        advance_contract(&mut sim, &data.config, 0);
        assert_eq!(
            sim.resources.minerals, after,
            "an already-reached milestone does not pay out again"
        );
    }

    #[test]
    fn overshooting_a_target_does_not_overscore() {
        let metrics = vec![metric(0.5, 1.0, 3.0), metric(0.5, 1.0, 0.0)];
        let (score, _) = score_success(&metrics);
        assert!((score - 0.5).abs() < f32::EPSILON);
    }

    fn armed(seed: u64, contract_id: &str) -> (crate::data::GameData, crate::state::sim::SimState) {
        let data = crate::data::GameData::load().unwrap();
        let mut sim = crate::state::sim::SimState::new_campaign(
            &data,
            "preservers",
            seed,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.contracts.get(contract_id).unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        (data, sim)
    }

    #[test]
    fn phases_are_set_from_the_authored_segments() {
        let (data, mut sim) = armed(1, "deep_vein_survey");

        // Month 1 crosses the pre-launch Preparation into Travel.
        let first = advance_contract(&mut sim, &data.config, 0);
        assert_eq!(first.phase_changed, Some(ContractPhase::Travel));

        // Travel holds until the authored travel years elapse, then Operation.
        let op_start = loop {
            let p = advance_contract(&mut sim, &data.config, 0);
            if let Some(phase) = p.phase_changed {
                assert_eq!(
                    phase,
                    ContractPhase::Operation,
                    "travel yields to operation"
                );
                break sim.contract.as_ref().unwrap().months_elapsed;
            }
            assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Travel);
        };
        // deep_vein_survey travels 110 years before making station.
        assert_eq!(op_start, 110 * 12 + 1);
    }

    #[test]
    fn objective_accrues_only_during_operation() {
        let (data, mut sim) = armed(2, "deep_vein_survey");

        // Nothing accrues in Preparation or Travel.
        loop {
            let p = advance_contract(&mut sim, &data.config, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
            assert_eq!(
                sim.contract.as_ref().unwrap().objective_progress,
                0.0,
                "no objective work before the ship is on-station"
            );
        }
        // The first on-station month accrues one base_rate share (speed 0).
        let c = sim.contract.as_ref().unwrap();
        let expected = c.objective_target / c.operation_months() as f32;
        assert!(
            (c.objective_progress - expected).abs() < 1e-3,
            "one operation month accrues base_rate: {} vs {expected}",
            c.objective_progress
        );
    }

    #[test]
    fn objective_fraction_clamps_and_zero_target_is_complete() {
        let (_data, mut sim) = armed(3, "deep_vein_survey");
        let c = sim.contract.as_mut().unwrap();
        c.objective_progress = c.objective_target * 3.0;
        assert_eq!(c.objective_fraction(), 1.0, "overshoot clamps to full");
        c.objective_progress = 0.0;
        assert_eq!(c.objective_fraction(), 0.0);
        c.objective_target = 0.0;
        assert_eq!(c.objective_fraction(), 1.0, "a zero target counts as met");
    }

    #[test]
    fn a_truncated_mission_pays_proportional_to_the_objective() {
        let (data, mut sim) = armed(7, "deep_vein_survey");
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();

        // Make station, then bank a clean quarter of the objective.
        loop {
            let p = advance_contract(&mut sim, &data.config, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }
        {
            let c = sim.contract.as_mut().unwrap();
            c.objective_progress = c.objective_target * 0.25;
        }

        // Turn back mid-Operation and fly the return leg home.
        assert!(
            jump_to_return(&mut sim),
            "turning back mid-Operation is allowed"
        );
        assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Return);
        let total = sim.contract.as_ref().unwrap().total_months();
        while sim.contract.as_ref().unwrap().months_elapsed < total {
            advance_contract(&mut sim, &data.config, 0);
        }

        let contract = sim.contract.as_ref().unwrap();
        assert_eq!(
            contract.objective_fraction(),
            0.25,
            "objective is frozen through Return"
        );
        let pay = prorated_reward(&template.reward, contract.objective_fraction());
        assert_eq!(pay.credits, template.reward.credits / 4);
        assert_eq!(pay.minerals, template.reward.minerals / 4);
        assert!(
            pay.credits > 0 && pay.credits < template.reward.credits,
            "prorated pay is neither full nor zero"
        );
    }

    #[test]
    fn resource_efficiency_tracks_lean_months_across_the_voyage() {
        let (data, mut sim) = armed(6, "deep_vein_survey");
        let efficiency = |sim: &crate::state::sim::SimState| {
            sim.contract
                .as_ref()
                .unwrap()
                .metrics
                .iter()
                .find(|m| m.kind == MetricKind::ResourceEfficiency)
                .unwrap()
                .current
        };

        // Ten well-provisioned months: full marks.
        sim.resources.food = data.config.low_food_threshold + 1_000;
        sim.resources.energy = data.config.low_energy_threshold + 1_000;
        for _ in 0..10 {
            advance_contract(&mut sim, &data.config, 0);
        }
        assert_eq!(
            efficiency(&sim),
            1.0,
            "a voyage that never runs low scores full efficiency"
        );

        // Ten months with the larder empty: only the energy half banks credit,
        // so the running fraction settles at (10*2 + 10*1) / (20*2) = 0.75.
        sim.resources.food = 0;
        for _ in 0..10 {
            advance_contract(&mut sim, &data.config, 0);
        }
        assert!(
            (efficiency(&sim) - 0.75).abs() < 1e-6,
            "lean months drag the voyage-long score: {}",
            efficiency(&sim)
        );

        // The lean stretch stays on the record after stores recover.
        sim.resources.food = data.config.low_food_threshold + 1_000;
        advance_contract(&mut sim, &data.config, 0);
        assert!(
            efficiency(&sim) < 1.0,
            "a famine is not forgotten once the stores refill"
        );
    }

    #[test]
    fn an_abort_in_travel_pays_nothing() {
        let (data, mut sim) = armed(4, "deep_vein_survey");
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();

        // A few months into Travel — no objective work has happened.
        for _ in 0..50 {
            advance_contract(&mut sim, &data.config, 0);
        }
        assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Travel);

        assert!(jump_to_return(&mut sim));
        assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Return);
        let total = sim.contract.as_ref().unwrap().total_months();
        while sim.contract.as_ref().unwrap().months_elapsed < total {
            advance_contract(&mut sim, &data.config, 0);
        }

        let contract = sim.contract.as_ref().unwrap();
        assert_eq!(
            contract.objective_fraction(),
            0.0,
            "no objective work → no pay"
        );
        let pay = prorated_reward(&template.reward, contract.objective_fraction());
        assert_eq!(pay.credits, 0);
        assert_eq!(pay.minerals, 0);
    }
}
