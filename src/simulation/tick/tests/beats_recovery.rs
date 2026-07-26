//! The mirror of collapse: a ship pulled back from the brink says so too,
//! once, and only after the mend is real.

use super::*;

#[test]
fn a_hull_recovery_beat_marks_a_frame_rebuilt_from_failure() {
    // Content-depth campaign-skeleton round 32: the structural twin of the crew-stat recovery beats
    // (unity/stability/morale/loyalty) and the ascending mirror of the hull-collapse beat. A hull
    // that never failed marks no recovery; a frame driven past its red line fires the collapse beat
    // and arms the counter; a mere patch over the red line but below the recovery line does not
    // count; a genuine refit back over the recovery line forces the recovery beat and re-arms.
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
    let recover_line = data.config.campaign_skeleton.hull_recovery_beat_threshold;
    assert!(
        red_line > 0.0 && recover_line > red_line,
        "this test needs the hull collapse+recovery beats enabled"
    );

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
    let counter = |sim: &SimState| sim.contract.as_ref().unwrap().hull_beats_fired;
    let clear_pending = |sim: &mut SimState, data: &GameData| {
        if let Some(pending) = sim.pending_event.clone() {
            let t = data.events.get(&pending.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(sim, data, &t, 0);
        }
    };

    // A sound hull that never failed marks no recovery.
    sim.ship.hull_integrity = 0.9;
    advance_year(&mut sim, &data);
    assert_eq!(
        counter(&sim),
        0,
        "a hull that never failed arms no recovery"
    );

    // The frame fails past the red line: the collapse beat fires and arms the recovery counter.
    sim.ship.hull_integrity = red_line - 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.hull_beat_band, -1,
        "the failing frame forces the collapse"
    );
    assert_eq!(counter(&sim), 1, "the collapse arms the recovery");
    clear_pending(&mut sim, &data);

    // A patch over the red line but below the recovery line: re-arms the band, no recovery yet.
    sim.ship.hull_integrity = (red_line + recover_line) / 2.0;
    advance_year(&mut sim, &data);
    assert_eq!(sim.hull_beat_band, 0, "the patch clears the red line");
    assert_eq!(
        counter(&sim),
        1,
        "a partial patch below the recovery line is no rebuild"
    );
    clear_pending(&mut sim, &data);

    // A genuine refit back over the recovery line: the recovery beat fires and re-arms the counter.
    sim.ship.hull_integrity = 0.95;
    advance_year(&mut sim, &data);
    assert_eq!(
        counter(&sim),
        0,
        "a full refit fires the recovery and re-arms the collapse"
    );
}

#[test]
fn an_air_recovery_beat_marks_a_plant_overhauled_from_failure() {
    // Content-depth campaign-skeleton round 33: the atmosphere twin of the hull-recovery beat. Air
    // that never failed marks no recovery; life-support driven past its red line fires the collapse
    // beat and arms the counter; a patch over the red line but below the recovery line is no
    // overhaul; a real overhaul back over the recovery line forces the recovery beat and re-arms.
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
    // Silence the hull beats so only the air collapse/recovery can fire.
    data.config.campaign_skeleton.hull_beat_family.clear();
    data.config
        .campaign_skeleton
        .hull_recovery_beat_family
        .clear();
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;
    let red_line = data.config.campaign_skeleton.air_beat_threshold;
    let recover_line = data.config.campaign_skeleton.air_recovery_beat_threshold;
    assert!(
        red_line > 0.0 && recover_line > red_line,
        "this test needs the air collapse+recovery beats enabled"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        7,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    sim.ship.hull_integrity = 0.95; // keep the frame sound so only the air speaks
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();
    let counter = |sim: &SimState| sim.contract.as_ref().unwrap().air_beats_fired;
    let clear_pending = |sim: &mut SimState, data: &GameData| {
        if let Some(pending) = sim.pending_event.clone() {
            let t = data.events.get(&pending.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(sim, data, &t, 0);
        }
    };

    // Sound air that never failed marks no recovery.
    sim.ship.life_support = 0.9;
    advance_year(&mut sim, &data);
    assert_eq!(counter(&sim), 0, "air that never failed arms no recovery");

    // The air fails past the red line: the collapse beat fires and arms the recovery counter.
    sim.ship.life_support = red_line - 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(sim.air_beat_band, -1, "the failing air forces the collapse");
    assert_eq!(counter(&sim), 1, "the collapse arms the recovery");
    clear_pending(&mut sim, &data);

    // A patch over the red line but below the recovery line: re-arms the band, no recovery yet.
    sim.ship.life_support = (red_line + recover_line) / 2.0;
    advance_year(&mut sim, &data);
    assert_eq!(sim.air_beat_band, 0, "the patch clears the red line");
    assert_eq!(
        counter(&sim),
        1,
        "a partial patch below the recovery line is no overhaul"
    );
    clear_pending(&mut sim, &data);

    // A real overhaul back over the recovery line: the recovery beat fires and re-arms the counter.
    sim.ship.life_support = 0.95;
    advance_year(&mut sim, &data);
    assert_eq!(
        counter(&sim),
        0,
        "a full overhaul fires the recovery and re-arms the collapse"
    );
}

#[test]
fn a_becalmed_recovery_beat_marks_a_ship_freed_from_the_doldrums() {
    // Content-depth campaign-skeleton round 34: the mobility twin of the hull/air recovery beats,
    // the last collapse beat to gain its recovery. A ship that never stranded marks no recovery; a
    // long stranding fires the collapse and arms the counter; while still stranded no recovery
    // fires; the drive lit again (the stall counter back to 0) fires the recovery and re-arms. The
    // recovery reads the collapse *counter* and the stall count, not the collapse band, so it is
    // tested against the fire hooks directly like the collapse test.
    let data = GameData::load().unwrap();
    let years = data.config.campaign_skeleton.becalmed_beat_years;
    assert!(
        years > 0
            && !data
                .config
                .campaign_skeleton
                .becalmed_recovery_beat_family
                .is_empty(),
        "this test needs the becalmed collapse+recovery beats enabled"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        7,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();
    let counter = |sim: &SimState| sim.contract.as_ref().unwrap().becalmed_beats_fired;

    // A ship moving all along marks no recovery.
    sim.fuel_stall_years = 0;
    assert!(!fire_becalmed_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(
        counter(&sim),
        0,
        "a ship that never stranded arms no recovery"
    );

    // Long stranded: the collapse beat fires and arms the recovery counter.
    sim.fuel_stall_years = years;
    assert!(fire_becalmed_beat(&mut sim, &data, &mut report));
    assert_eq!(counter(&sim), 1, "the stranding arms the recovery");

    // Still stranded (the stall persists): the recovery does not fire.
    assert!(
        !fire_becalmed_recovery_beat(&mut sim, &data, &mut report),
        "a still-stranded ship marks no recovery"
    );
    assert_eq!(counter(&sim), 1);

    // The drive lit again (the stall counter back to 0): the recovery fires and re-arms.
    sim.fuel_stall_years = 0;
    assert!(
        fire_becalmed_recovery_beat(&mut sim, &data, &mut report),
        "a ship freed from the doldrums reckons with the recovery"
    );
    assert_eq!(
        counter(&sim),
        0,
        "the recovery re-arms the collapse counter"
    );
    assert!(
        !fire_becalmed_recovery_beat(&mut sim, &data, &mut report),
        "fires once per stranding"
    );
}

#[test]
fn a_recovery_beat_marks_a_ship_pulling_back_from_the_brink() {
    // Content-depth campaign-skeleton round 13: the crisis beat's hopeful mirror.
    // A ship that never fractured has nothing to recover; one that fell into a
    // unity crisis and then climbs back out forces a recovery beat, which resets
    // the crisis counter so a relapse re-arms the collapse beats.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.depopulation_beats.clear();
    let crisis0 = data.config.campaign_skeleton.crisis_beats[0];
    let recovery = data.config.campaign_skeleton.recovery_beat_threshold;
    assert!(
        recovery > 0.0
            && !data
                .config
                .campaign_skeleton
                .recovery_beat_family
                .is_empty(),
        "this test needs the recovery beat enabled"
    );

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

    // A united ship that never fractured: recovery has nothing to mark.
    sim.population.unity = recovery + 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().crisis_beats_fired,
        0,
        "a ship that never came apart has no mending to mark"
    );

    // Fracture it: the crisis beat fires.
    sim.contract.as_mut().unwrap().beats.clear();
    sim.population.unity = crisis0 - 0.02;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().crisis_beats_fired,
        1,
        "unity falling past the threshold forces a crisis beat"
    );
    if let Some(p) = sim.pending_event.clone() {
        let t = data.events.get(&p.template_id).cloned().unwrap();
        crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
    }

    // Climb back out: the recovery beat fires and re-arms the collapse beats
    // (only the recovery firer resets the crisis counter to zero).
    sim.contract.as_mut().unwrap().beats.clear();
    sim.population.unity = recovery + 0.05;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().crisis_beats_fired,
        0,
        "climbing back from the brink marks the mending and re-arms the collapse beats"
    );
}

#[test]
fn a_governance_recovery_beat_marks_a_ship_rebuilding_its_institutions() {
    // Content-depth campaign-skeleton round 28: the stability twin of the unity recovery beat.
    // A ship whose government never collapsed has nothing to recover; one that fell into a
    // stability collapse and then climbs back forces a governance-recovery beat, resetting the
    // stability-collapse counter so a relapse re-arms the collapse beats.
    let data = GameData::load().unwrap();
    let threshold = data
        .config
        .campaign_skeleton
        .stability_recovery_beat_threshold;
    let collapse0 = data.config.campaign_skeleton.stability_beats[0];
    assert!(
        threshold > 0.0,
        "this test needs the governance-recovery beat enabled"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        8,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // A well-governed ship that never collapsed: recovery has nothing to mark.
    sim.population.stability = threshold + 0.05;
    assert!(!fire_stability_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().stability_beats_fired, 0);

    // The institutions collapse: the stability-collapse beat fires.
    sim.population.stability = collapse0 - 0.02;
    assert!(fire_stability_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().stability_beats_fired, 1);
    // …but no recovery while the government is still in anarchy.
    assert!(!fire_stability_recovery_beat(&mut sim, &data, &mut report));

    // Rebuild it: the recovery beat fires and resets the collapse counter.
    sim.population.stability = threshold + 0.05;
    assert!(fire_stability_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(
        sim.contract.as_ref().unwrap().stability_beats_fired,
        0,
        "rebuilding the government marks the recovery and re-arms the collapse beats"
    );
    // Fires once per collapse episode.
    assert!(!fire_stability_recovery_beat(&mut sim, &data, &mut report));
}

#[test]
fn a_heartening_recovery_beat_marks_a_crew_that_finds_its_heart_again() {
    // Content-depth campaign-skeleton round 30: the morale twin of the unity/stability recovery
    // beats, the hopeful mirror of the despair beat. A crew that never despaired has nothing to
    // recover; one that sank into despair and then climbs back forces a heartening reckoning,
    // resetting the despair counter so a relapse re-arms the collapse beats.
    let data = GameData::load().unwrap();
    let threshold = data
        .config
        .campaign_skeleton
        .heartening_recovery_beat_threshold;
    let despair0 = data.config.campaign_skeleton.despair_beats[0];
    assert!(
        threshold > 0.0,
        "this test needs the heartening-recovery beat enabled"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        10,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // A high-hearted crew that never despaired: recovery has nothing to mark.
    sim.population.morale = threshold + 0.05;
    assert!(!fire_heartening_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().despair_beats_fired, 0);

    // The crew sinks into despair: the despair beat fires.
    sim.population.morale = despair0 - 0.02;
    assert!(fire_despair_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().despair_beats_fired, 1);
    // …but no recovery while spirits are still in the depths.
    assert!(!fire_heartening_recovery_beat(&mut sim, &data, &mut report));

    // Spirits climb back: the recovery beat fires and resets the despair counter.
    sim.population.morale = threshold + 0.05;
    assert!(fire_heartening_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(
        sim.contract.as_ref().unwrap().despair_beats_fired,
        0,
        "finding its heart again marks the recovery and re-arms the despair beats"
    );
    // Fires once per despair episode.
    assert!(!fire_heartening_recovery_beat(&mut sim, &data, &mut report));
}

#[test]
fn a_covenant_recovery_beat_marks_a_crew_that_renews_the_founding_cause() {
    // Content-depth campaign-skeleton round 31: the loyalty twin of the unity/stability/morale
    // recovery beats, the last of the four decline stats to get one. A crew that never lapsed has
    // nothing to renew; one whose covenant lapsed and then climbs back forces a recovery beat,
    // resetting the loyalty-collapse counter so a relapse re-arms the collapse beats.
    let data = GameData::load().unwrap();
    let threshold = data
        .config
        .campaign_skeleton
        .loyalty_recovery_beat_threshold;
    let collapse0 = data.config.campaign_skeleton.loyalty_beats[0];
    assert!(
        threshold > 0.0,
        "this test needs the covenant-recovery beat enabled"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        11,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    let mut report = TickReport::default();

    // A faithful crew that never lapsed: recovery has nothing to mark.
    sim.population.legacy_loyalty = threshold + 0.05;
    assert!(!fire_loyalty_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().loyalty_beats_fired, 0);

    // The covenant lapses: the loyalty-collapse beat fires.
    sim.population.legacy_loyalty = collapse0 - 0.02;
    assert!(fire_loyalty_beat(&mut sim, &data, &mut report));
    assert_eq!(sim.contract.as_ref().unwrap().loyalty_beats_fired, 1);
    // …but no renewal while the covenant is still lapsed.
    assert!(!fire_loyalty_recovery_beat(&mut sim, &data, &mut report));

    // The crew re-embraces the founding cause: the recovery beat fires and resets the counter.
    sim.population.legacy_loyalty = threshold + 0.05;
    assert!(fire_loyalty_recovery_beat(&mut sim, &data, &mut report));
    assert_eq!(
        sim.contract.as_ref().unwrap().loyalty_beats_fired,
        0,
        "renewing the covenant marks the recovery and re-arms the collapse beats"
    );
    // Fires once per lapse episode.
    assert!(!fire_loyalty_recovery_beat(&mut sim, &data, &mut report));
}
