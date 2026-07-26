//! Descending beats: a system, an institution or a crew's heart fails,
//! the voyage says so once, and the beat rearms only on a real mend.

use super::*;

#[test]
fn an_air_collapse_beat_fires_when_the_life_support_fails_and_rearms_on_overhaul() {
    // Content-depth campaign-skeleton round 24: the atmosphere twin of the hull-collapse
    // beat. With rolls and the other beats off, life-support crossing its red line must
    // force a reckoning once; an overhaul back above the line re-arms it.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.stability_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.subsystem_beats.clear();
    data.config.campaign_skeleton.hull_beat_family.clear();
    data.config.campaign_skeleton.reputation_beat_family.clear();
    data.config.campaign_skeleton.succession_beat_family.clear();
    data.config.campaign_skeleton.long_reign_beat_family.clear();
    data.config
        .campaign_skeleton
        .dynasty_crisis_beat_family
        .clear();
    data.config
        .campaign_skeleton
        .power_transition_beat_family
        .clear();
    data.config.campaign_skeleton.founding_beat_family.clear();
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;
    let red_line = data.config.campaign_skeleton.air_beat_threshold;
    assert!(red_line > 0.0, "this test needs the air beat enabled");

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        7,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // Clean air: no reckoning.
    sim.ship.life_support = 0.9;
    advance_year(&mut sim, &data);
    assert_eq!(sim.air_beat_band, 0, "clean air forces no beat");

    // The air fails past the red line: the beat fires once.
    sim.ship.life_support = red_line - 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.air_beat_band, -1,
        "a ship suffocating past its red line forces the collapse reckoning"
    );

    // An overhaul clears the air: the beat re-arms.
    if let Some(pending) = sim.pending_event.clone() {
        let t = data.events.get(&pending.template_id).cloned().unwrap();
        crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
    }
    sim.ship.life_support = 0.9;
    advance_year(&mut sim, &data);
    assert_eq!(sim.air_beat_band, 0, "an overhaul re-arms the air beat");
}

#[test]
fn a_hull_collapse_beat_fires_when_the_frame_fails_and_rearms_on_refit() {
    // Content-depth campaign-skeleton round 23: the structural twin of the subsystem
    // collapse beat. With rolls and the other beats off, a hull crossing its red line
    // must force a reckoning once; a refit back above the line re-arms it.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.stability_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.subsystem_beats.clear();
    data.config.campaign_skeleton.reputation_beat_family.clear();
    data.config.campaign_skeleton.succession_beat_family.clear();
    data.config.campaign_skeleton.long_reign_beat_family.clear();
    data.config
        .campaign_skeleton
        .dynasty_crisis_beat_family
        .clear();
    data.config
        .campaign_skeleton
        .power_transition_beat_family
        .clear();
    data.config.campaign_skeleton.founding_beat_family.clear();
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;
    let red_line = data.config.campaign_skeleton.hull_beat_threshold;
    assert!(red_line > 0.0, "this test needs the hull beat enabled");

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        7,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // A sound hull: no reckoning.
    sim.ship.hull_integrity = 0.9;
    advance_year(&mut sim, &data);
    assert_eq!(sim.hull_beat_band, 0, "a sound hull forces no beat");

    // The frame fails past the red line: the beat fires once.
    sim.ship.hull_integrity = red_line - 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.hull_beat_band, -1,
        "a hull past its red line forces the collapse reckoning"
    );

    // A refit brings the hull back sound: the beat re-arms (band clears).
    if let Some(pending) = sim.pending_event.clone() {
        let t = data.events.get(&pending.template_id).cloned().unwrap();
        crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
    }
    sim.ship.hull_integrity = 0.9;
    advance_year(&mut sim, &data);
    assert_eq!(sim.hull_beat_band, 0, "a refit re-arms the hull beat");
}

#[test]
fn a_becalmed_beat_fires_when_the_ship_is_long_stranded_and_rearms_when_it_burns() {
    // Content-depth campaign-skeleton round 25: the mobility twin of the hull/air collapse
    // beats. Once the ship has been fuel-stalled for the threshold years running, the
    // becalmed reckoning is forced once; a year that burns again re-arms it. Tested
    // against the fire hook directly, since the stall counter is driven by real stalls.
    let data = GameData::load().unwrap();
    let years = data.config.campaign_skeleton.becalmed_beat_years;
    assert!(years > 0, "this test needs the becalmed beat enabled");
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        7,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // Still moving (short of the threshold): no reckoning.
    sim.fuel_stall_years = years - 1;
    assert!(!fire_becalmed_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.becalmed_beat_band, 0);

    // Long stranded: the beat fires, once.
    sim.fuel_stall_years = years;
    assert!(fire_becalmed_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.becalmed_beat_band, -1);
    assert!(
        !fire_becalmed_beat(&mut sim, &data, &mut report),
        "fires once per stranding"
    );

    // Burning again re-arms it (clears the band, no fire).
    sim.fuel_stall_years = 0;
    assert!(!fire_becalmed_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.becalmed_beat_band, 0);
}

#[test]
fn a_subsystem_collapse_beat_fires_when_a_keystone_truly_fails() {
    // Content-depth campaign-skeleton round 17: the first forced beat keyed to a
    // *subsystem's condition*. With reactive rolls and the other threshold beats off,
    // the only thing that can fire is the keystone-collapse beat — and it must, once
    // the engineering bay rots past its red line, while a sound bay stays silent.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    let beat = data
        .config
        .campaign_skeleton
        .subsystem_beats
        .iter()
        .find(|b| b.subsystem == "engineering_bay")
        .expect("the engineering keystone should carry a collapse beat")
        .clone();

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // A sound engineering bay: no keystone failure to mark.
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = beat.threshold + 0.3;
    advance_year(&mut sim, &data);
    assert!(
        sim.subsystem_beats_fired.is_empty(),
        "a sound keystone forces no collapse beat"
    );

    // The bay rots past its red line: the beat fires, and marks the module once.
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = beat.threshold - 0.02;
    advance_year(&mut sim, &data);
    assert!(
        sim.subsystem_beats_fired
            .contains(&"engineering_bay".to_string()),
        "the keystone failing past its red line forces a beat"
    );
}

#[test]
fn a_crisis_beat_fires_as_the_ship_comes_apart() {
    // Content-depth campaign-skeleton round 6: the descending mirror of the
    // drift/adaptation beats. With reactive rolls and the other threshold beats
    // off, the only thing that can fire is the cohesion-collapse crisis beat —
    // and it must, once unity falls past the first threshold.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    let first = data.config.campaign_skeleton.crisis_beats[0];

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();
    // Push the people just past the first collapse threshold (unity falling).
    sim.population.unity = first - 0.02;

    advance_year(&mut sim, &data);

    assert_eq!(
        sim.contract.as_ref().unwrap().crisis_beats_fired,
        1,
        "unity falling past the first threshold forces exactly one crisis beat"
    );
}

#[test]
fn a_stability_beat_fires_as_the_ships_institutions_fail() {
    // Content-depth campaign-skeleton round 15: the last population stat to get a
    // beat. With reactive rolls and the other threshold beats off, the only thing
    // that can fire is the governance-collapse beat — and it must, once stability
    // falls past the first threshold, while a well-ordered ship stays silent.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    let first = data.config.campaign_skeleton.stability_beats[0];

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // A well-governed ship: no institutional collapse to mark.
    sim.population.stability = first + 0.1;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().stability_beats_fired,
        0,
        "a functioning government has no collapse to mark"
    );

    // Stability falls past the first threshold: the beat fires.
    sim.population.stability = first - 0.02;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().stability_beats_fired,
        1,
        "the institutions failing past the threshold forces one beat"
    );
}

#[test]
fn a_loyalty_beat_fires_as_the_founders_covenant_lapses() {
    // Content-depth campaign-skeleton round 14: the last identity stat to get a
    // beat. With reactive rolls and the other threshold beats off, the only thing
    // that can fire is the loyalty-collapse beat — and it must, once legacy_loyalty
    // falls past the first threshold, while a still-devoted ship stays silent.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    let first = data.config.campaign_skeleton.loyalty_beats[0];

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // Still devoted to the founders: no covenant to mark as lapsed.
    sim.population.legacy_loyalty = first + 0.1;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().loyalty_beats_fired,
        0,
        "a devoted ship has no lapse to mark"
    );

    // Loyalty collapses past the first threshold: the beat fires.
    sim.population.legacy_loyalty = first - 0.02;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().loyalty_beats_fired,
        1,
        "the founders' covenant lapsing past the threshold forces one beat"
    );
}

#[test]
fn a_despair_beat_marks_a_crew_that_has_lost_its_heart() {
    // Content-depth campaign-skeleton round 29: the descending morale-collapse pole of the
    // flourish beat. A ship of decent spirits fires nothing; as morale crashes past each despair
    // threshold in turn, a beat is forced once per level — the reckoning a crew that simply loses
    // heart never had, distinct from the crisis beat that watches unity fracture.
    let data = GameData::load().unwrap();
    let beats = data.config.campaign_skeleton.despair_beats.clone();
    assert!(
        beats.len() >= 2,
        "this test needs at least two despair thresholds"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        9,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // Decent spirits (above the first line): no reckoning.
    sim.population.morale = beats[0] + 0.05;
    assert!(!fire_despair_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().despair_beats_fired, 0);

    // Spirits crash past the first line: one beat, and staying there does not reprint.
    sim.population.morale = beats[0] - 0.01;
    assert!(fire_despair_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().despair_beats_fired, 1);
    assert!(
        !fire_despair_beat(&mut sim, &data, &mut report),
        "the next (lower) despair threshold has not been crossed"
    );

    // Crash past the second, lower line: the next beat fires.
    sim.population.morale = beats[1] - 0.01;
    assert!(fire_despair_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().despair_beats_fired, 2);
}
