//! State gates: a template waits until the ship is actually in the shape
//! its fiction describes - drifted, dwindled, starving, browning out.

use super::*;

#[test]
fn a_cultural_drift_gate_holds_a_template_until_the_drift_arrives() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 1, &picks);
    // The Long Schism is gated at min_cultural_drift 0.6 (W6).
    let schism = data.events.get("the_schism_deepens").unwrap();
    assert!((schism.min_cultural_drift - 0.6).abs() < 1e-6);

    sim.population.cultural_drift = 0.2;
    assert!(
        !passes_gate(&sim, schism),
        "the schism stays out of the pool below its drift gate"
    );
    sim.population.cultural_drift = 0.7;
    assert!(
        passes_gate(&sim, schism),
        "the schism enters the pool once drift is high enough"
    );
}

#[test]
fn the_dynasty_crisis_gate_waits_for_a_dwindled_line() {
    // Content-depth campaign skeleton round 20: near-end-of-the-line content
    // stays out of the pool until the founding dynasty has actually dwindled.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
    let evt = data.events.get("the_last_of_the_line").unwrap();
    assert!(
        !passes_gate(&sim, evt),
        "a healthy founding dynasty is no crisis"
    );
    sim.dynasty.members.truncate(2);
    assert!(
        passes_gate(&sim, evt),
        "a dwindled line lets the reckoning surface"
    );
}

#[test]
fn a_shortage_gate_holds_an_opportunity_until_the_ship_runs_low() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 13, &picks);
    // `the_dry_tank` only calls when the fuel fraction is at or below 0.2.
    let event = data.events.get("the_dry_tank").unwrap();
    assert_eq!(event.fuel_below, Some(0.2));
    // Put it in a phase it accepts.
    let template = data.contracts.get("deep_vein_survey").unwrap();
    let mut active = crate::simulation::contract::start_contract(template, &sim);
    active.phase = crate::data::contracts::ContractPhase::Travel;
    sim.contract = Some(active);

    sim.ship.fuel = 0.8;
    assert!(
        !passes_gate(&sim, event),
        "a full tank keeps the crisis away"
    );
    sim.ship.fuel = 0.1;
    assert!(passes_gate(&sim, event), "a near-dry tank surfaces it");
}

#[test]
fn a_double_shortage_gate_needs_both_shortages_at_once() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 29, &picks);
    // `the_long_winter` gates on low food AND low energy together.
    let event = data.events.get("the_long_winter").unwrap();
    assert!(event.food_below.is_some() && event.energy_below.is_some());
    let (food_t, energy_t) = (event.food_below.unwrap(), event.energy_below.unwrap());

    // Only one shortage → still out of the pool.
    sim.resources.food = food_t - 1;
    sim.resources.energy = energy_t + 1000;
    assert!(
        !passes_gate(&sim, event),
        "low food alone is not the long winter"
    );
    sim.resources.food = food_t + 1000;
    sim.resources.energy = energy_t - 1;
    assert!(
        !passes_gate(&sim, event),
        "low energy alone is not the long winter"
    );
    // Both short → it fires.
    sim.resources.food = food_t - 1;
    sim.resources.energy = energy_t - 1;
    assert!(
        passes_gate(&sim, event),
        "hunger and cold together bring it"
    );
}

#[test]
fn a_sustained_plenty_gate_waits_for_a_soft_generation() {
    // Content-depth provisioning round 14: the mirror of the chronic-scarcity
    // gate. `the_soft_generation` tells a lifetime of plenty from one bumper
    // year — it needs both a currently flush larder *and* a plenty that has held
    // for years, so a ship one good harvest into abundance does not yet face it.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 65, &picks);
    let event = data.events.get("the_soft_generation").unwrap();
    let flush = event.food_above.expect("gates on a full larder");
    let years = event.min_fat_food_years;
    assert!(years > 0, "the soft generation gates on sustained plenty");

    // Flush today, but only just: no soft-generation reckoning yet.
    sim.resources.food = flush + 1000;
    sim.fat_food_years = years - 1;
    assert!(
        !passes_gate(&sim, event),
        "one good harvest is not yet a generation of plenty"
    );
    // Plenty that has held for years, still flush: it surfaces.
    sim.fat_food_years = years;
    assert!(
        passes_gate(&sim, event),
        "a lifetime of plenty raises the soft generation"
    );
    // A ship whose stores have since run down does not face it.
    sim.resources.food = flush - 5000;
    assert!(
        !passes_gate(&sim, event),
        "the soft-generation reckoning needs the plenty to still be present"
    );
}

#[test]
fn a_chronic_scarcity_gate_waits_for_a_lean_generation() {
    // Content-depth provisioning round 13: the persistence gate. `the_long_hunger`
    // tells a chronic hunger from one bad winter — it needs both a currently lean
    // larder *and* a shortage that has ground on for years, so a ship one season
    // into a famine does not yet face the long-hunger reckoning.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 63, &picks);
    let event = data.events.get("the_long_hunger").unwrap();
    let famine = event.food_below.expect("gates on a lean larder");
    let years = event.min_lean_food_years;
    assert!(years > 0, "the long hunger gates on a sustained shortage");

    // Lean today, but only just: no long-hunger reckoning yet.
    sim.resources.food = famine - 1;
    sim.lean_food_years = years - 1;
    assert!(
        !passes_gate(&sim, event),
        "one season of hunger is not yet a lean generation"
    );
    // A shortage that has ground on for years, still lean: it surfaces.
    sim.lean_food_years = years;
    assert!(
        passes_gate(&sim, event),
        "years of grinding scarcity bring the long hunger"
    );
    // A ship that has recovered its stores does not face it, however long the
    // past lean lasted (the streak resets on recovery in the tick).
    sim.resources.food = famine + 5000;
    assert!(
        !passes_gate(&sim, event),
        "a recovered larder ends the long hunger"
    );
}

#[test]
fn a_paradox_gate_needs_abundance_and_scarcity_at_once() {
    // Content-depth provisioning round 12: the abundance gates (it75) gain their
    // first interaction with the shortage set. `the_gilded_hunger` surfaces only
    // when the ship is *both* rich in credits and starving — a fortune it cannot
    // eat — so neither condition alone brings it.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 53, &picks);
    let event = data.events.get("the_gilded_hunger").unwrap();
    let rich = event.credits_above.expect("gates on a fat treasury");
    let starving = event.food_below.expect("gates on an empty larder");

    // Rich but fed: no paradox.
    sim.resources.credits = rich + 1;
    sim.resources.food = starving + 1000;
    assert!(
        !passes_gate(&sim, event),
        "a rich, fed ship has no gilded hunger"
    );
    // Starving but poor: the ordinary famine, not this one.
    sim.resources.credits = rich - 1000;
    sim.resources.food = starving - 1;
    assert!(
        !passes_gate(&sim, event),
        "a poor, starving ship faces plain famine, not gilded hunger"
    );
    // Rich *and* starving: the fortune it cannot eat.
    sim.resources.credits = rich + 1;
    sim.resources.food = starving - 1;
    assert!(
        passes_gate(&sim, event),
        "wealth it cannot eat and a larder run dry, at once"
    );
}

#[test]
fn a_governance_gate_waits_for_a_failing_government() {
    // Content-depth campaign-skeleton round 15: the honest gate for
    // institutional-collapse content. `the_ungoverned_ship` stays out of the
    // pool on a well-ordered ship and surfaces only once stability has fallen.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 62, &picks);
    let event = data.events.get("the_ungoverned_ship").unwrap();
    let ceiling = event.max_stability;
    assert!(
        ceiling > 0.0,
        "the ungoverned ship gates on fallen stability"
    );

    sim.population.stability = ceiling + 0.1;
    assert!(
        !passes_gate(&sim, event),
        "a well-ordered ship's government still functions"
    );
    sim.population.stability = ceiling;
    assert!(
        passes_gate(&sim, event),
        "a failing government surfaces the reckoning"
    );
}

#[test]
fn a_founder_authority_gate_waits_for_a_lapsed_covenant() {
    // Content-depth campaign-skeleton round 14: the honest gate for covenant-lapse
    // content. `the_lapsed_covenant` stays out of the pool on a still-devoted
    // ship and surfaces only once loyalty to the founders has fallen far enough.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 60, &picks);
    let event = data.events.get("the_lapsed_covenant").unwrap();
    let ceiling = event.max_legacy_loyalty;
    assert!(ceiling > 0.0, "the covenant lapse gates on fallen loyalty");

    sim.population.legacy_loyalty = ceiling + 0.1;
    assert!(
        !passes_gate(&sim, event),
        "a still-devoted ship holds the founders' charter binding"
    );
    sim.population.legacy_loyalty = ceiling;
    assert!(
        passes_gate(&sim, event),
        "a lapsed covenant surfaces the reckoning"
    );
}

#[test]
fn a_cohesion_gate_waits_for_a_reunited_ship() {
    // Content-depth campaign-skeleton round 13: the honest gate for recovery
    // content, the cohesion twin of min_morale. `the_mending` stays out of the
    // pool on a fracturing ship and surfaces only once unity has climbed back.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 59, &picks);
    let event = data.events.get("the_mending").unwrap();
    let floor = event.min_unity;
    assert!(floor > 0.0, "the mending gates on recovered cohesion");

    sim.population.unity = floor - 0.1;
    assert!(
        !passes_gate(&sim, event),
        "a fracturing ship has no mending to reflect on"
    );
    sim.population.unity = floor;
    assert!(
        passes_gate(&sim, event),
        "a reunited ship surfaces the mending"
    );
}

#[test]
fn a_depopulation_gate_waits_for_a_thinned_crew() {
    // Content-depth campaign-skeleton round 12: the honest gate for crew-thinning
    // content, the descending mirror of min_morale. `the_thinning_decks` stays
    // out of the pool on a full ship and surfaces only once the crew has fallen
    // to or below its headcount ceiling.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 51, &picks);
    let event = data.events.get("the_thinning_decks").unwrap();
    let ceiling = event.max_population;
    assert!(ceiling > 0, "the thinning content gates on a headcount");

    sim.population.count = ceiling + 1;
    assert!(
        !passes_gate(&sim, event),
        "a full ship does not reckon with empty decks"
    );
    sim.population.count = ceiling;
    assert!(
        passes_gate(&sim, event),
        "a crew fallen to the ceiling surfaces the thinning"
    );
}

#[test]
fn an_abundance_gate_waits_for_real_plenty_and_softness_worsens_the_winter() {
    // Content-depth provisioning round 11: the first gate keyed to *plenty*
    // rather than want. `the_fat_years` stays out of the pool at ordinary
    // stores and only surfaces when the granaries are genuinely swollen — and
    // feasting through it (grown_soft) makes the later long winter bite harder.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 41, &picks);
    let fat = data.events.get("the_fat_years").unwrap();
    let threshold = fat.food_above.expect("the fat years gate on abundance");

    // Ordinary and even lean stores: no fat-years choice.
    sim.resources.food = threshold - 1;
    assert!(
        !passes_gate(&sim, fat),
        "a merely comfortable ship has no surplus to reckon with"
    );
    // Granaries swollen past the threshold: the choice of plenty arrives.
    sim.resources.food = threshold + 1;
    assert!(
        passes_gate(&sim, fat),
        "genuine abundance surfaces the fat-years choice"
    );

    // The loop closes on the long winter: a ship that grew soft in the fat
    // years carries the soft-generation complication where a thrifty one does
    // not — the abundance choice reaches forward into the later famine.
    let winter = data.events.get("the_long_winter").unwrap();
    let soft = winter
        .complications
        .iter()
        .find(|c| c.requires_consequence.iter().any(|s| s == "grown_soft"))
        .expect("the long winter carries the soft-generation complication");
    assert!(
        active_complication(&sim, winter).is_none(),
        "a ship that never feasted meets the winter with its thrift intact"
    );
    // Feast through the fat years, then face the winter.
    let live_well = fat
        .outcomes
        .iter()
        .position(|o| o.long_term_consequences.iter().any(|s| s == "grown_soft"))
        .unwrap();
    apply_outcome(&mut sim, &data, fat, live_well);
    assert!(
        sim.consequences.iter().any(|c| c == "grown_soft"),
        "living well in the fat years softens the ship"
    );
    assert!(
        active_complication(&sim, winter).is_some_and(|c| c.id == soft.id),
        "the softened generation bears the long winter worse"
    );
}

#[test]
fn a_condition_gate_waits_for_a_module_to_break_down() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 23, &picks);
    // `the_failing_air` only fires as the habitat plant physically fails.
    let event = data.events.get("the_failing_air").unwrap();
    assert_eq!(event.condition_below[0].id, "life_support_habitat");

    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.9;
    assert!(
        !passes_gate(&sim, event),
        "a sound plant keeps the crisis away"
    );
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.2;
    assert!(passes_gate(&sim, event), "a failing plant surfaces it");
}

#[test]
fn an_era_ceiling_retires_deep_middle_content_before_homecoming() {
    // Content-depth campaign-skeleton round 4: the max_generation ceiling is
    // the mirror of min_generation — a deep-middle beat unlocks after the
    // founding generations and retires before the homecoming ones, so "the
    // ship is the only world" cannot fire once the ship is nearly home.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 61, &picks);
    let event = data.events.get("the_only_world").unwrap();
    assert!(event.min_generation > 0 && event.max_generation >= event.min_generation);

    // Before its era: still gated out by min_generation.
    sim.dynasty.generation = event.min_generation - 1;
    assert!(
        !passes_gate(&sim, event),
        "too early: the founders still live"
    );
    // Inside its era: it fires.
    sim.dynasty.generation = event.min_generation;
    assert!(passes_gate(&sim, event), "the deep middle surfaces it");
    // Past its era: the ceiling retires it.
    sim.dynasty.generation = event.max_generation + 1;
    assert!(
        !passes_gate(&sim, event),
        "too late: near home it is no longer the only world"
    );
}

#[test]
fn an_energy_shortage_gate_waits_for_a_browning_reactor() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 17, &picks);
    // `the_dimming` only enters the pool when energy is at or below 1200.
    let event = data.events.get("the_dimming").unwrap();
    assert_eq!(event.energy_below, Some(1200));

    sim.resources.energy = 5000;
    assert!(
        !passes_gate(&sim, event),
        "a full grid keeps the crisis away"
    );
    sim.resources.energy = 800;
    assert!(passes_gate(&sim, event), "a browning grid surfaces it");
}

#[test]
fn a_knowledge_crisis_gates_on_low_know_how_and_its_outcome_reteaches_it() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 11, &picks);
    let event = data.events.get("the_last_engineer").unwrap();
    assert_eq!(event.knowledge_below[0].id, "engineering_bay");

    // Healthy know-how: the crisis stays out of the pool.
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.8;
    assert!(!passes_gate(&sim, event));

    // Once knowledge has decayed under the threshold, it can fire.
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.2;
    assert!(passes_gate(&sim, event));

    // Applying the apprentice outcome re-teaches the bay (knowledge +0.35).
    let before = sim.subsystems["engineering_bay"].knowledge;
    apply_outcome(&mut sim, &data, event, 0);
    let after = sim.subsystems["engineering_bay"].knowledge;
    assert!(
        after > before,
        "the teaching succession restores lost know-how ({before} -> {after})"
    );
}

#[test]
fn the_wandering_mind_gates_on_lost_know_how_and_its_choices_diverge() {
    // Content-depth event-families round 4: a mystery gated on the same
    // engineering knowledge decay, whose two outcomes push that knowledge in
    // opposite directions — trusting the old system erodes understanding,
    // rebuilding it by hand restores it. The choice must genuinely matter.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 3, &picks);
    let event = data.events.get("the_wandering_mind").unwrap();
    assert_eq!(event.knowledge_below[0].id, "engineering_bay");
    sim.dynasty.generation = 3; // clear its min_generation gate

    // Healthy know-how: the mystery stays out of the pool.
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.8;
    assert!(!passes_gate(&sim, event));
    // Decayed: it can fire.
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.2;
    assert!(passes_gate(&sim, event));

    // Outcome 0 (trust it) erodes knowledge; outcome 1 (rebuild) restores it.
    let mut trusting = sim.clone();
    apply_outcome(&mut trusting, &data, event, 0);
    let mut rebuilding = sim.clone();
    apply_outcome(&mut rebuilding, &data, event, 1);
    assert!(
        trusting.subsystems["engineering_bay"].knowledge
            < rebuilding.subsystems["engineering_bay"].knowledge,
        "obeying the old mind should cost understanding that rebuilding restores"
    );
}
