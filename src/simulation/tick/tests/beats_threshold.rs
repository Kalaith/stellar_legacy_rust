//! Ascending beats: the ship crosses a line - drifted enough, shipborn
//! enough, thinned enough, golden enough - and the voyage remarks on it.

use super::*;

#[test]
fn a_drift_threshold_beat_fires_when_the_people_have_changed_enough() {
    // Reactive rolls and dilemmas off, so the only thing that can fire is the
    // drift-threshold beat (content-depth round 2).
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let first = data.config.campaign_skeleton.drift_beats[0];

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    // No scheduled beats laid out (LAUNCH would add them); push the people just
    // past the first drift threshold.
    sim.contract.as_mut().unwrap().beats.clear();
    sim.population.cultural_drift = first + 0.02;

    advance_year(&mut sim, &data);

    assert_eq!(
        sim.contract.as_ref().unwrap().drift_beats_fired,
        1,
        "crossing the first drift threshold fires exactly one drift beat"
    );
}

#[test]
fn an_adaptation_threshold_beat_fires_as_the_people_grow_shipborn() {
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    let first = data.config.campaign_skeleton.adaptation_beats[0];

    let mut sim = SimState::new_campaign(
        &data,
        "adaptors",
        24,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();
    sim.population.adaptation = first + 0.02;

    advance_year(&mut sim, &data);

    assert_eq!(
        sim.contract.as_ref().unwrap().adaptation_beats_fired,
        1,
        "crossing the first adaptation threshold fires exactly one adaptation beat"
    );
}

#[test]
fn a_divergence_beat_fires_when_the_crew_grows_shipborn_and_rearms_when_held_back() {
    // Content-depth campaign-skeleton round 26: the high-side crew-body twin of the
    // hull/air/becalmed ship-body crisis beats. Once the people's adaptation rises to its
    // red line — grown so shipborn they can no longer survive a planet — the divergence
    // reckoning is forced once; a fall back below (a strong infirmary holding the baseline)
    // re-arms it.
    let data = GameData::load().unwrap();
    let line = data.config.campaign_skeleton.divergence_beat_threshold;
    assert!(line > 0.0, "this test needs the divergence beat enabled");
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        11,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // Still planet-capable (short of the line): no reckoning.
    sim.population.adaptation = line - 0.05;
    assert!(!fire_divergence_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.adaptation_divergence_band, 0);

    // Grown fully shipborn: the beat fires, once.
    sim.population.adaptation = line + 0.02;
    assert!(fire_divergence_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.adaptation_divergence_band, 1);
    assert!(
        !fire_divergence_beat(&mut sim, &data, &mut report),
        "fires once per crossing"
    );

    // The infirmary holds the line back below — re-arms it (clears the band, no fire).
    sim.population.adaptation = line - 0.05;
    assert!(!fire_divergence_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.adaptation_divergence_band, 0);
}

#[test]
fn a_cultural_divergence_beat_fires_when_the_charter_goes_unreadable_and_rearms() {
    // Content-depth campaign-skeleton round 27: the cultural twin of the divergence beat.
    // Once the crew's cultural_drift rises to its red line — the founders' charter a dead
    // language, the mission carried by rote — the reckoning is forced once; a fall back below
    // (a strong archive reviving the old ways) re-arms it. Sits above the top drift_beats
    // milestone so it is the terminal mark, not another rung.
    let data = GameData::load().unwrap();
    let line = data
        .config
        .campaign_skeleton
        .cultural_divergence_beat_threshold;
    assert!(
        line > 0.0,
        "this test needs the cultural-divergence beat enabled"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        13,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // The founding purpose still intelligible (short of the line): no reckoning.
    sim.population.cultural_drift = line - 0.05;
    assert!(!fire_cultural_divergence_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.cultural_divergence_band, 0);

    // Drifted past reading the charter: the beat fires, once.
    sim.population.cultural_drift = line + 0.02;
    assert!(fire_cultural_divergence_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.cultural_divergence_band, 1);
    assert!(
        !fire_cultural_divergence_beat(&mut sim, &data, &mut report),
        "fires once per crossing"
    );

    // A strong archive revives the old ways back below the line — re-arms it (no fire).
    sim.population.cultural_drift = line - 0.05;
    assert!(!fire_cultural_divergence_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.cultural_divergence_band, 0);
}

#[test]
fn a_depopulation_beat_fires_as_the_crew_thins() {
    // Content-depth campaign-skeleton round 12: the crew's headcount — the one
    // major state dimension no beat watched. With reactive rolls and the other
    // threshold beats off, the only thing that can fire is the depopulation beat —
    // and it must, once the crew falls past the first founding-fraction, while a
    // full ship stays silent. The beat surfaces the honest max_population content.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.reputation_beat_family.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    let first = data.config.campaign_skeleton.depopulation_beats[0];
    let founding = data.config.starting_population;

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

    // A full crew marks no thinning.
    sim.population.count = founding;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.depopulation_beats_fired, 0,
        "a full ship has no thinning to mark"
    );

    // Thin the crew past the first founding-fraction: the beat fires, once, and
    // forces a survival beat (the content pool, gated by max_population).
    sim.population.count = (first * founding as f32) as u32 - 1;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.depopulation_beats_fired, 1,
        "crossing the first fraction marks the thinning"
    );

    // Resolve whatever it surfaced, then pin the crew back to the same stage (the
    // resolution may itself cost lives); staying at one stage must not re-mark it
    // (campaign-scoped counter).
    if let Some(p) = sim.pending_event.clone() {
        let t = data.events.get(&p.template_id).cloned().unwrap();
        crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
    }
    sim.population.count = (first * founding as f32) as u32 - 1;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.depopulation_beats_fired, 1,
        "staying at one stage does not re-mark it"
    );
}

#[test]
fn a_flourish_beat_fires_as_the_ship_reaches_its_golden_age() {
    // Content-depth campaign-skeleton round 8: the ascending positive pole of the
    // crisis beat. With reactive rolls and the other threshold beats off, the only
    // thing that can fire is the flourish beat — and it must, once morale climbs
    // past the first threshold, while a low-morale ship stays silent.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    let first = data.config.campaign_skeleton.flourish_beats[0];

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

    // A middling-morale ship generates no golden age.
    sim.population.morale = first - 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().flourish_beats_fired,
        0,
        "a ship short of the threshold has no golden age to mark"
    );

    // Lift the people past the first flourish threshold — the beat must fire.
    sim.population.morale = first + 0.02;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().flourish_beats_fired,
        1,
        "morale climbing past the first threshold forces exactly one flourish beat"
    );
}

#[test]
fn an_objective_beat_fires_as_the_mission_crosses_its_milestone() {
    // Content-depth campaign-skeleton round 9: the first pacing keyed to the
    // mission itself. With reactive rolls and the other threshold beats off, the
    // only thing that can fire is the objective beat — and it must, once the
    // charter's objective crosses the first authored fraction.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    let first = data.config.campaign_skeleton.objective_beats[0];

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

    // Objective untouched: no milestone beat.
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().objective_beats_fired,
        0,
        "a mission with no progress has no milestone to mark"
    );

    // Bank the objective past the first fraction — the beat must fire.
    {
        let c = sim.contract.as_mut().unwrap();
        c.objective_progress = c.objective_target * (first + 0.01);
    }
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().objective_beats_fired,
        1,
        "crossing the first objective fraction forces exactly one milestone beat"
    );
}

#[test]
fn an_anniversary_beat_fires_on_its_periodic_cadence() {
    // Content-depth campaign-skeleton round 7: the periodic archetype. With every
    // other event source off and a short anniversary cadence, the voyage must
    // observe its anniversary once the clock passes the interval.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.despair_beats.clear();
    // A short cadence so the test does not fly a full century.
    data.config.campaign_skeleton.anniversary_years = 5;

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // Before the first interval: no anniversary yet.
    for _ in 0..4 {
        advance_year(&mut sim, &data);
    }
    assert_eq!(
        sim.contract.as_ref().unwrap().anniversaries_fired,
        0,
        "no anniversary before the first interval elapses"
    );

    // Cross the interval, resolving the forced beat so the loop can proceed.
    for _ in 0..3 {
        if let Some(pending) = sim.pending_event.clone() {
            let t = data.events.get(&pending.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
        }
        advance_year(&mut sim, &data);
    }
    assert!(
        sim.contract.as_ref().unwrap().anniversaries_fired >= 1,
        "the voyage observes its anniversary once the cadence elapses"
    );
}

#[test]
fn a_midvoyage_beat_fires_at_the_deep_middle_of_the_voyage() {
    // Content-depth campaign-skeleton round 21: the era beat the "early / mid /
    // homecoming" texture lacked in the middle. With reactive rolls and the other
    // threshold beats off, the voyage must force a deep-middle reckoning the first tick
    // it passes its temporal midpoint with home still ahead — and only once.
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
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    // deep_vein_survey: 340 years, midpoint (170y) safely inside its Operation leg.
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    {
        let c = sim.contract.as_mut().unwrap();
        c.beats.clear();
        // Jump to one year shy of the midpoint — no deep-middle beat yet. Settle the
        // phase to match the clock so the first advance doesn't register a spurious
        // Preparation→Operation change and hard-stop before the midpoint.
        c.months_elapsed = 169 * 12;
        let (idx, phase) = c.phase_at(c.months_elapsed);
        c.phase_index = idx;
        c.phase = phase;
    }
    assert_eq!(
        sim.contract.as_ref().unwrap().phase,
        crate::data::contracts::ContractPhase::Operation,
        "a year shy of the midpoint the ship is on station"
    );
    assert!(
        !sim.contract.as_ref().unwrap().midvoyage_beat_fired,
        "no deep-middle beat before the midpoint"
    );

    // Cross the midpoint: the beat fires, once, while home is still ahead.
    advance_year(&mut sim, &data);
    assert!(
        sim.contract.as_ref().unwrap().midvoyage_beat_fired,
        "the voyage marks its deep middle once it passes the halfway point"
    );
    assert_ne!(
        sim.contract.as_ref().unwrap().phase,
        crate::data::contracts::ContractPhase::Return,
        "the deep-middle beat fires before the ship turns for home"
    );
}
