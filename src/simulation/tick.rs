//! The yearly simulation tick (GDD §3 step 3, §5.1).
//!
//! Time advances only on explicit player action (Pillar 4). One call =
//! one in-game year: production, upkeep, wear, aging, contract progress,
//! market drift, then an event roll.

use crate::data::{GameData, ResourceDelta};
use crate::simulation::contract::{score_success, SuccessLevel};
use crate::simulation::{contract, event_resolver, market, succession};
use crate::state::sim::SimState;

/// Everything a single year produced that the caller (game.rs) must react
/// to: log lines are already recorded on the sim; a completed contract and a
/// blocking event are surfaced explicitly.
#[derive(Debug, Default)]
pub struct TickReport {
    /// Set when the active contract reached its target duration this year.
    pub contract_completed: Option<(f32, SuccessLevel)>,
    /// Set when an event fired that needs a council decision (not delegated).
    pub decision_required: bool,
    pub dynasty_extinct: bool,
}

pub fn advance_year(sim: &mut SimState, data: &GameData) -> TickReport {
    debug_assert!(
        sim.pending_event.is_none(),
        "caller must resolve the pending event before advancing time"
    );
    let config = &data.config;
    let mut report = TickReport::default();

    sim.year += 1;

    // Production (GDD §5.1: floor(rate * years), one year per tick).
    let produced = ResourceDelta {
        credits: sim.production.credits.floor() as i64,
        energy: sim.production.energy.floor() as i64,
        minerals: sim.production.minerals.floor() as i64,
        food: sim.production.food.floor() as i64,
        influence: sim.production.influence.floor() as i64,
    };
    sim.resources.apply(&produced);

    // Food upkeep; famine bleeds morale and people.
    let upkeep = (sim.population.count as f32 * config.food_per_person_per_year).ceil() as i64;
    if sim.resources.food >= upkeep {
        sim.resources.food -= upkeep;
    } else {
        sim.resources.food = 0;
        let losses = (sim.population.count as f32 * 0.02).ceil() as u32;
        sim.population.count = sim.population.count.saturating_sub(losses);
        sim.population.morale = (sim.population.morale - 0.05).max(0.0);
        sim.push_log(format!(
            "Rations ran out. The population diminished by {losses}."
        ));
    }

    // Ship wear.
    sim.ship.hull_integrity = (sim.ship.hull_integrity - config.hull_decay_per_year).max(0.0);
    sim.ship.life_support = (sim.ship.life_support - config.life_support_decay_per_year).max(0.0);

    // Generational tick (GDD §5.3).
    sim.dynasty.years_since_generation += 1;
    if sim.dynasty.years_since_generation >= config.generation_interval_years {
        let generation = succession::process_generation(sim, data);
        for name in &generation.deaths {
            sim.push_log(format!("{name} was laid to rest among the stars."));
        }
        if let Some(name) = &generation.new_leader {
            sim.push_log(format!("{name} assumed leadership of the dynasty."));
        }
        if generation.births > 0 {
            sim.push_log(format!(
                "Generation {} came of age: {} new dynasty member(s).",
                sim.dynasty.generation, generation.births
            ));
        }
        if generation.extinct {
            sim.push_log("The dynasty has no heirs. The line ends here.");
            report.dynasty_extinct = true;
        }
    }

    // Contract progress and completion (GDD §5.2).
    for milestone in contract::advance_contract(sim, config) {
        sim.push_log(format!("Milestone reached: {milestone}"));
    }
    if let Some(active) = &sim.contract {
        if active.years_elapsed >= active.target_duration_years {
            report.contract_completed = Some(score_success(&active.metrics));
        }
    }

    // Market drift.
    market::drift_prices(sim);

    // Event roll (GDD §5.4). Delegated or no-decision events resolve
    // immediately but still log their outcome (GDD §3 step 4).
    if let Some(pending) = event_resolver::roll_event(sim, data) {
        if let Some(template) = data.events.get(&pending.template_id).cloned() {
            let delegated = sim.delegation.is_delegated(template.category);
            if template.requires_decision && !delegated {
                sim.push_log(format!("Council decision required: {}", template.title));
                sim.pending_event = Some(pending);
                report.decision_required = true;
            } else {
                let label = event_resolver::auto_resolve(sim, data, &template);
                if delegated {
                    sim.push_log(format!(
                        "Delegated advisor resolved '{}' with: {label}",
                        template.title
                    ));
                }
            }
        }
    }

    sim.trim_log(config.log_limit);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::simulation::contract::start_contract;

    fn fresh(seed: u64) -> (GameData, SimState) {
        let data = GameData::load().unwrap();
        let sim = SimState::new_campaign(&data, "preservers", seed);
        (data, sim)
    }

    #[test]
    fn a_year_produces_resources_and_consumes_food() {
        let (data, mut sim) = fresh(21);
        let food_before = sim.resources.food;
        let credits_before = sim.resources.credits;
        sim.pending_event = None;

        advance_year(&mut sim, &data);

        assert_eq!(sim.year, 1);
        let upkeep =
            (sim.population.count as f32 * data.config.food_per_person_per_year).ceil() as i64;
        assert_eq!(
            sim.resources.food,
            food_before + data.config.base_production.food.floor() as i64 - upkeep
        );
        assert!(sim.resources.credits >= credits_before);
        assert!(sim.ship.hull_integrity < 1.0);
    }

    #[test]
    fn identical_seeds_produce_identical_decades() {
        let (data, mut a) = fresh(77);
        let (_, mut b) = fresh(77);
        for _ in 0..10 {
            a.pending_event = None;
            b.pending_event = None;
            advance_year(&mut a, &data);
            advance_year(&mut b, &data);
        }
        assert_eq!(a.resources.credits, b.resources.credits);
        assert_eq!(a.population.count, b.population.count);
        assert_eq!(
            serde_json::to_string(&a.market.entries).unwrap(),
            serde_json::to_string(&b.market.entries).unwrap()
        );
        assert_eq!(a.log.len(), b.log.len());
    }

    #[test]
    fn contract_completes_at_target_duration() {
        let (data, mut sim) = fresh(5);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        // Plenty of food so the population survives the run deterministically.
        sim.resources.food = 1_000_000;

        let mut completed = None;
        for _ in 0..template.target_duration_years {
            sim.pending_event = None;
            let report = advance_year(&mut sim, &data);
            if report.contract_completed.is_some() {
                completed = report.contract_completed;
                break;
            }
        }
        let (score, _) = completed.expect("contract must complete at its target duration");
        assert!(score > 0.0);
        let active = sim.contract.as_ref().unwrap();
        assert!(active.milestones.iter().all(|m| m.reached));
    }
}
