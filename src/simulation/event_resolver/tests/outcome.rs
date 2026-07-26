//! Landing a choice: the population banding, the scoring the autoplayer
//! uses, and what an outcome writes back to the sim.

use super::*;

#[test]
fn impact_range_bands_large_deltas_and_leaves_small_ones_exact() {
    let cfg = impact_cfg();
    // A specific, small toll stays exact — no band (real-time loop §3).
    assert_eq!(impact_range(-8, cfg), None);
    assert_eq!(impact_range(19, cfg), None);
    // A big toll becomes a ±variance band, ordered low ≤ high.
    assert_eq!(impact_range(-300, cfg), Some((-420, -180)));
    assert_eq!(impact_range(500, cfg), Some((300, 700)));
}

#[test]
fn rolled_pop_count_stays_within_its_band() {
    let cfg = impact_cfg();
    let mut rng = macroquad_toolkit::rng::SeededRng::new(42);
    // Below the floor: applied exactly.
    assert_eq!(rolled_pop_count(-8, cfg, &mut rng), -8);
    // Above the floor: every draw lands inside the shown band.
    for _ in 0..200 {
        let rolled = rolled_pop_count(-300, cfg, &mut rng);
        assert!(
            (-420..=-180).contains(&(rolled as i64)),
            "rolled {rolled} outside [-420, -180]"
        );
    }
}

#[test]
fn event_chance_is_capped() {
    let data = GameData::load().unwrap();
    assert!((event_chance(&data.config, 100, 1.0) - data.config.event_chance_cap).abs() < 1e-6);
    assert!((event_chance(&data.config, 0, 0.0) - data.config.event_chance_base).abs() < 1e-6);
}

#[test]
fn starving_ship_doubles_food_weight_in_scoring() {
    let data = GameData::load().unwrap();
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        3,
        &crate::state::sim::founding_faction_ids(&data),
    );
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
    let mut sim = SimState::new_campaign(
        &data,
        "adaptors",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.events.get("system_failure").unwrap().clone();
    sim.pending_event = Some(crate::state::sim::PendingEvent {
        template_id: template.id.clone(),
        rolled_month_clock: sim.month_clock,
    });

    apply_outcome(&mut sim, &data, &template, 1); // reroute_power
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
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));

    // Put the ship on-station so there is a Return leg to jump to.
    loop {
        let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
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
    apply_outcome(&mut sim, &data, &scram, idx);

    assert_eq!(
        sim.contract.as_ref().unwrap().phase,
        ContractPhase::Return,
        "a force_return outcome jumps the contract onto its return leg"
    );
}

#[test]
fn trilemma_events_offer_a_genuinely_distinct_third_path() {
    // Content-depth event-families round 8: the set was overwhelmingly binary
    // (175/189 events had exactly two outcomes). Five iconic dilemmas gained a
    // real third path — each a different strategic axis, not a milquetoast
    // middle. This locks that they resolve as three legal, distinct outcomes.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    for id in [
        "tithe_demand",
        "micrometeoroid_storm",
        "cultural_schism",
        "skills_drought",
        "the_wary_frontier",
    ] {
        let event = data.events.get(id).unwrap();
        assert_eq!(event.outcomes.len(), 3, "{id} should be a trilemma now");
    }

    // The tithe's third path (offer service) is materially distinct from the
    // other two: unlike paying it spends no hard credits, unlike running it
    // takes no hull damage, and it earns influence the ship would not get by
    // either. Apply it from a clean state and check those effects land.
    let event = data.events.get("tithe_demand").unwrap();
    let idx = event
        .outcomes
        .iter()
        .position(|o| o.id == "offer_service")
        .unwrap();
    let mut sim = SimState::new_campaign(&data, "preservers", 12, &picks);
    let credits_before = sim.resources.credits;
    let influence_before = sim.resources.influence;
    let hull_before = sim.ship.hull_integrity;
    apply_outcome(&mut sim, &data, event, idx);
    assert_eq!(
        sim.resources.credits, credits_before,
        "offering service costs no treasury"
    );
    assert!(
        sim.resources.influence > influence_before,
        "competence-for-passage earns standing"
    );
    assert_eq!(
        sim.ship.hull_integrity, hull_before,
        "no shots fired, no hull lost"
    );
}
