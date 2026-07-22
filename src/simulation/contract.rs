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
        anniversaries_fired: 0,
        flourish_beats_fired: 0,
        objective_beats_fired: 0,
        scheduled_beats: template.scheduled_beats.clone(),
        scheduled_beats_fired: 0,
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
            contract.objective_progress += base_rate * speed_factor;
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
