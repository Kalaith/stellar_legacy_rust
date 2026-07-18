//! Contract success scoring and progression (GDD §5.2).

use crate::data::contracts::{ContractPhase, ContractTemplate, MetricKind};
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

/// Instantiate an active contract from a template at the current sim state.
pub fn start_contract(template: &ContractTemplate, sim: &SimState) -> ActiveContract {
    ActiveContract {
        template_id: template.id.clone(),
        name: template.name.clone(),
        objective: template.objective,
        target_duration_years: template.target_duration_years,
        years_elapsed: 0,
        phase: ContractPhase::Preparation,
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
        bonus_progress: 0.0,
    }
}

/// Advance the active contract by one year: progress, phase, milestones, and
/// refreshed metric readings. `speed` is the ship loadout's aggregate speed,
/// which adds bonus progress (PLAN item 3). Returns newly reached milestone
/// names.
pub fn advance_contract(
    sim: &mut SimState,
    config: &crate::data::GameConfig,
    speed: i32,
) -> Vec<String> {
    let population_count = sim.population.count;
    let unity = sim.population.unity;
    let food_ok = sim.resources.food >= config.low_food_threshold;
    let energy_ok = sim.resources.energy >= config.low_energy_threshold;
    let progress_per_speed = config.ship.contract_progress_per_speed;

    // Mutate the contract in a scope so its borrow ends before we grant any
    // milestone rewards to the shared resource pool.
    let (reached, rewards) = {
        let Some(contract) = sim.contract.as_mut() else {
            return Vec::new();
        };

        contract.years_elapsed += 1;
        if speed > 0 {
            contract.bonus_progress += speed as f32 * progress_per_speed;
        }
        let progress = contract.progress();
        contract.phase = ContractPhase::for_progress(progress);

        let mut reached = Vec::new();
        let mut rewards = Vec::new();
        for milestone in &mut contract.milestones {
            if !milestone.reached && progress >= milestone.progress_threshold {
                milestone.reached = true;
                reached.push(milestone.name.clone());
                rewards.push(milestone.reward);
            }
        }

        for metric in &mut contract.metrics {
            metric.current = match metric.kind {
                MetricKind::PopulationSurvival => {
                    if contract.starting_population == 0 {
                        1.0
                    } else {
                        population_count as f32 / contract.starting_population as f32
                    }
                }
                MetricKind::MissionCompletion => progress,
                // Skeleton reading: fraction of the two upkeep resources above
                // their crisis thresholds. TODO(next agent): replace with a real
                // spent-vs-produced efficiency ratio tracked across the contract.
                MetricKind::ResourceEfficiency => {
                    (food_ok as u32 as f32 + energy_ok as u32 as f32) / 2.0
                }
                MetricKind::SocialCohesion => unity,
            };
        }
        (reached, rewards)
    };

    // A milestone's reward lands the year it is first reached.
    for reward in rewards {
        sim.resources.apply(&reward);
    }
    reached
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::contracts::MetricKind;

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
        let mut sim = SimState::new_campaign(&data, "preservers", 31);
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
}
