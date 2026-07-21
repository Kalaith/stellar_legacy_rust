//! Event rolling, outcome scoring, and resolution (GDD §5.4).

use crate::data::events::{EventCategory, EventOutcome, EventTemplate};
use crate::data::{GameConfig, GameData};
use crate::state::sim::{PendingEvent, SimState};

/// `event_chance = min(cap, base + years_since_event*0.1 + contract_progress*0.2)`.
pub fn event_chance(config: &GameConfig, years_since_event: u32, contract_progress: f32) -> f32 {
    (config.event_chance_base + years_since_event as f32 * 0.1 + contract_progress * 0.2)
        .min(config.event_chance_cap)
}

/// Category weights, scaled up by ship/population distress (GDD §5.4).
pub fn category_weights(sim: &SimState, config: &GameConfig) -> [(EventCategory, f32); 4] {
    let mut crisis = 0.3;
    if sim.resources.food < config.low_food_threshold {
        crisis += 0.2;
    }
    if sim.resources.energy < config.low_energy_threshold {
        crisis += 0.2;
    }
    if sim.ship.hull_integrity < config.hull_warning_threshold {
        crisis += 0.2;
    }
    if sim.ship.life_support < config.life_support_warning_threshold {
        crisis += 0.2;
    }
    if sim.population.morale < 0.5 {
        crisis += 0.15;
    }
    if sim.population.unity < 0.4 {
        crisis += 0.15;
    }

    let milestone = match &sim.contract {
        Some(contract) => {
            let progress = contract.progress();
            if !(0.2..=0.8).contains(&progress) {
                0.4
            } else {
                0.15
            }
        }
        None => 0.05,
    };

    let legacy = (0.1 + (sim.year() / 25) as f32 * 0.05).min(0.3);

    [
        (EventCategory::ImmediateCrisis, crisis),
        (EventCategory::GenerationalChallenge, 0.3),
        (EventCategory::MissionMilestone, milestone),
        (EventCategory::LegacyMoment, legacy),
    ]
}

/// Roll for a new event. Returns the pending event without applying anything;
/// the caller decides whether it blocks or auto-resolves.
pub fn roll_event(sim: &mut SimState, data: &GameData) -> Option<PendingEvent> {
    let progress = sim.contract.as_ref().map_or(0.0, |c| c.progress());
    // The ramp is still a per-year model; convert its whole-year gap and the
    // resulting yearly chance to a per-month roll so expected events per year
    // is preserved while events can now fire (and be dated) any month (W3).
    let years_since = sim.month_clock.saturating_sub(sim.last_event_month_clock) / 12;
    let monthly_chance = event_chance(&data.config, years_since, progress) / 12.0;
    if !sim.rng.chance(monthly_chance) {
        return None;
    }

    // Pick a category by weight, then a template within it (weighted by the
    // template's modifier for the ship's legacy). Candidate lists are sorted
    // by id so hash-map iteration order never touches the seeded RNG.
    let weights = category_weights(sim, &data.config);
    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    let mut pick = sim.rng.next_f32() * total;
    let mut category = EventCategory::ImmediateCrisis;
    for (cat, weight) in weights {
        if pick < weight {
            category = cat;
            break;
        }
        pick -= weight;
    }

    let mut candidates: Vec<(&String, &EventTemplate)> = data
        .events
        .iter()
        .filter(|(_, t)| t.category == category)
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(b.0));
    if candidates.is_empty() {
        return None;
    }

    let legacy_id = sim.legacy.legacy_id.as_str();
    let template_weights: Vec<f32> = candidates
        .iter()
        .map(|(_, t)| *t.legacy_weight_modifiers.get(legacy_id).unwrap_or(&1.0))
        .collect();
    let weight_total: f32 = template_weights.iter().sum();
    let mut roll = sim.rng.next_f32() * weight_total;
    let mut chosen = candidates[0].1;
    for (i, weight) in template_weights.iter().enumerate() {
        if roll < *weight {
            chosen = candidates[i].1;
            break;
        }
        roll -= weight;
    }

    sim.last_event_month_clock = sim.month_clock;
    Some(PendingEvent {
        template_id: chosen.id.clone(),
        rolled_month_clock: sim.month_clock,
    })
}

/// Score an outcome for auto-resolution (GDD §5.4). Higher is better.
pub fn score_outcome(outcome: &EventOutcome, sim: &SimState, config: &GameConfig) -> f32 {
    let food_weight = if sim.resources.food < config.low_food_threshold {
        2.0
    } else {
        1.0
    };
    let ship_distressed = sim.ship.hull_integrity < config.hull_warning_threshold
        || sim.ship.life_support < config.life_support_warning_threshold;
    let ship_weight = if ship_distressed { 1000.0 } else { 100.0 };

    outcome.resource_delta.food as f32 * food_weight
        + (outcome.ship_delta.hull_integrity + outcome.ship_delta.life_support) * ship_weight
        + outcome.resource_delta.credits as f32 * 0.1
        + outcome.resource_delta.energy as f32 * 0.2
        + outcome.resource_delta.minerals as f32 * 0.3
        + outcome.population_delta.morale * 500.0
        + outcome.population_delta.unity * 600.0
        - 100.0 * outcome.long_term_consequences.len() as f32
    // TODO(next agent): + legacy_specific_modifier * 200 once outcomes carry
    // per-legacy modifiers (GDD §5.4).
}

/// Apply one outcome of a pending event to the sim and log it.
pub fn apply_outcome(sim: &mut SimState, template: &EventTemplate, outcome_index: usize) {
    let Some(outcome) = template.outcomes.get(outcome_index) else {
        return;
    };
    sim.resources.apply(&outcome.resource_delta);
    sim.ship.apply(&outcome.ship_delta);
    sim.population.apply(&outcome.population_delta);
    sim.consequences
        .extend(outcome.long_term_consequences.iter().cloned());
    // A salvaged component drops into the hold, to be installed later
    // (PLAN M4.4). The outcome's own log narrates the find.
    if let Some(component_id) = &outcome.grant_component {
        sim.ship.salvage.push(component_id.clone());
    }

    let text = if outcome.log.is_empty() {
        format!("{}: {}", template.title, outcome.label)
    } else {
        outcome.log.clone()
    };
    sim.push_log(text);
    // An outcome may turn the mission for home early (W2) — the outcome's own
    // deltas carry the flavor; this just bends the voyage onto its return leg.
    if outcome.force_return {
        crate::simulation::contract::jump_to_return(sim);
    }
    sim.pending_event = None;
}

/// Pick the best-scoring outcome and apply it (delegated/no-decision path).
/// Returns the applied outcome's label.
pub fn auto_resolve(sim: &mut SimState, data: &GameData, template: &EventTemplate) -> String {
    let best = template
        .outcomes
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            score_outcome(a, sim, &data.config).total_cmp(&score_outcome(b, sim, &data.config))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);
    let label = template
        .outcomes
        .get(best)
        .map(|o| o.label.clone())
        .unwrap_or_default();
    apply_outcome(sim, template, best);
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn event_chance_is_capped() {
        let data = GameData::load().unwrap();
        assert!((event_chance(&data.config, 100, 1.0) - data.config.event_chance_cap).abs() < 1e-6);
        assert!((event_chance(&data.config, 0, 0.0) - data.config.event_chance_base).abs() < 1e-6);
    }

    #[test]
    fn starving_ship_doubles_food_weight_in_scoring() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 3);
        let template = data.events.get("population_growth").unwrap();
        let feed = &template.outcomes[0]; // food -300
        let hold = &template.outcomes[1]; // no food cost

        sim.resources.food = 100; // below low_food_threshold
        let feed_starving = score_outcome(feed, &sim, &data.config);
        sim.resources.food = 5000;
        let feed_fed = score_outcome(feed, &sim, &data.config);
        assert!(
            feed_starving < feed_fed,
            "spending food must score worse while starving"
        );

        sim.resources.food = 100;
        assert!(score_outcome(hold, &sim, &data.config) > score_outcome(feed, &sim, &data.config));
    }

    #[test]
    fn apply_outcome_clears_pending_and_records_consequences() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "adaptors", 5);
        let template = data.events.get("system_failure").unwrap().clone();
        sim.pending_event = Some(crate::state::sim::PendingEvent {
            template_id: template.id.clone(),
            rolled_month_clock: sim.month_clock,
        });

        apply_outcome(&mut sim, &template, 1); // reroute_power
        assert!(sim.pending_event.is_none());
        assert!(sim
            .consequences
            .contains(&"deferred_maintenance".to_owned()));
        assert!(sim.ship.life_support < 1.0);
    }

    #[test]
    fn a_force_return_outcome_turns_the_ship_home() {
        use crate::data::contracts::ContractPhase;
        use crate::simulation::contract::{advance_contract, start_contract};

        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 5);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));

        // Put the ship on-station so there is a Return leg to jump to.
        loop {
            let p = advance_contract(&mut sim, &data.config, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }

        // The catastrophic reactor-scram outcome forces the mission home early.
        let scram = data.events.get("reactor_scram").unwrap().clone();
        let idx = scram
            .outcomes
            .iter()
            .position(|o| o.force_return)
            .expect("reactor_scram carries a force_return outcome");
        apply_outcome(&mut sim, &scram, idx);

        assert_eq!(
            sim.contract.as_ref().unwrap().phase,
            ContractPhase::Return,
            "a force_return outcome jumps the contract onto its return leg"
        );
    }
}
