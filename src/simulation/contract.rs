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
            })
            .collect(),
        starting_population: sim.population.count,
    }
}

/// Advance the active contract by one year: progress, phase, milestones, and
/// refreshed metric readings. Returns newly reached milestone names.
pub fn advance_contract(sim: &mut SimState, config: &crate::data::GameConfig) -> Vec<String> {
    let population_count = sim.population.count;
    let unity = sim.population.unity;
    let food_ok = sim.resources.food >= config.low_food_threshold;
    let energy_ok = sim.resources.energy >= config.low_energy_threshold;

    let Some(contract) = sim.contract.as_mut() else {
        return Vec::new();
    };

    contract.years_elapsed += 1;
    let progress = contract.progress();
    contract.phase = ContractPhase::for_progress(progress);

    let mut reached = Vec::new();
    for milestone in &mut contract.milestones {
        if !milestone.reached && progress >= milestone.progress_threshold {
            milestone.reached = true;
            reached.push(milestone.name.clone());
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
    fn overshooting_a_target_does_not_overscore() {
        let metrics = vec![metric(0.5, 1.0, 3.0), metric(0.5, 1.0, 0.0)];
        let (score, _) = score_success(&metrics);
        assert!((score - 0.5).abs() < f32::EPSILON);
    }
}
