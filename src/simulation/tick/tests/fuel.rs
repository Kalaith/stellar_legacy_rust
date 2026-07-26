//! Fuel: spent crossing the dark, scooped on the way, and what a dry tank
//! does to a ship that cannot move.

use super::*;

#[test]
fn fuel_is_spent_in_travel_but_not_on_station() {
    // A travel month burns fuel.
    let (data, mut sim) = provisioned(5, 1.0);
    advance_months(&mut sim, &data, 1);
    assert!(sim.ship.fuel < 1.0, "the first travel month burns fuel");

    // An operation month burns none.
    let (data, mut sim) = provisioned(5, 1.0);
    sim.contract.as_mut().unwrap().months_elapsed = 110 * 12; // end of Travel
    advance_months(&mut sim, &data, 1);
    assert_eq!(sim.ship.fuel, 1.0, "on-station months burn no fuel");
}

#[test]
fn the_drive_reports_the_fuel_it_scoops_on_a_crossing_and_is_silent_on_station() {
    // A crossing sags the tank monthly and the scoop tops it up yearly; the
    // periodic provisioning line makes that rise legible (real-time loop
    // follow-up). On a full tank on-station, nothing is scooped, so it is silent.
    let gap = GameData::load()
        .unwrap()
        .config
        .flavor
        .fuel_report_gap_years;
    assert!(gap > 0, "fuel report cadence must be configured");
    // Step month-by-month past the phase-change hard-stops, clearing any decision
    // so the crossing runs uninterrupted (the autoplay soak pattern).
    let run = |sim: &mut SimState, data: &GameData, months: u32| {
        for _ in 0..months {
            sim.pending_event = None;
            sim.pending_dilemma = None;
            advance_months(sim, data, 1);
        }
    };

    // Under way on a full tank: the burn/scoop churn accrues a real haul.
    let (data, mut sim) = provisioned(5, 1.0);
    run(&mut sim, &data, gap * 12 + 12);
    assert!(
        sim.log.iter().any(|e| e.text.contains("fuel)")),
        "the drive's fuel haul is reported after a long crossing"
    );

    // On-station on a full tank: no burn, the scoop is capped away, so no report.
    let (data, mut sim) = provisioned(5, 1.0);
    sim.contract.as_mut().unwrap().months_elapsed = 110 * 12; // into Operation
    run(&mut sim, &data, gap * 12 + 12);
    assert!(
        !sim.log.iter().any(|e| e.text.contains("fuel)")),
        "a full tank sitting on-station reports no fuel haul"
    );
}

#[test]
fn a_failing_engineering_bay_burns_fuel_faster() {
    // Content-depth subsystems round 20: a degraded drive burns rich, so the same
    // travel month drinks more of the tank than a sound bay's would.
    let (data, mut sound) = provisioned(5, 1.0);
    let mut wrecked = sound.clone();
    sound
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .condition = 1.0;
    wrecked
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .condition = 0.0;

    advance_months(&mut sound, &data, 1);
    advance_months(&mut wrecked, &data, 1);
    assert!(
        wrecked.ship.fuel < sound.ship.fuel,
        "a rotting drive wastes reaction mass a sound one would keep"
    );
}

#[test]
fn a_dry_tank_stalls_travel_and_doubles_systems_decay() {
    // Launch dry: every travel month coasts until the year-boundary regen
    // frees one, so the voyage barely moves and the year's decay doubles.
    let (data, mut sim) = provisioned(5, 0.0);
    advance_year(&mut sim, &data);

    assert_eq!(sim.stalled_months, 11, "eleven months coasted before regen");
    assert_eq!(sim.month_clock, 12, "a full calendar year passed");
    assert_eq!(
        sim.contract.as_ref().unwrap().months_elapsed,
        1,
        "but the contract barely advanced"
    );
    let expected_hull = 1.0
        - data.config.hull_decay_per_year
            * (1.0 - data.config.maintenance_decay_relief)
            * data.config.provisioning.no_fuel_decay_multiplier;
    assert!(
        (sim.ship.hull_integrity - expected_hull).abs() < 1e-5,
        "a dry year wears the ship at the no-fuel rate: {} vs {expected_hull}",
        sim.ship.hull_integrity
    );
}
