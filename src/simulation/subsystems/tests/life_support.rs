//! The plant that keeps the crew breathing, and the modules that stand
//! between a failing one and the people it is meant to keep alive.

use super::*;

#[test]
fn a_failing_life_support_plant_thins_the_crew() {
    // Content-depth subsystems round 15: the life-support plant's most
    // fundamental effect. A plant above the failure threshold sustains everyone;
    // one that has collapsed thins the crew each year, worse the further it has
    // failed.
    let data = GameData::load().unwrap();
    let cfg = &data.config.subsystems;
    assert!(
        cfg.life_support_failure_threshold > 0.0 && cfg.life_support_failure_mortality > 0.0,
        "this test needs the life-support mortality coupling enabled"
    );

    let loss_at = |condition: f32| -> u32 {
        let (_, mut sim) = campaign(11);
        sim.population.count = 1000;
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = condition;
        // Isolate the plant: a dead garden contributes no bio life-support
        // (round 17), so this measures the mechanical plant alone.
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
        life_support_mortality_loss(&sim, &data)
    };

    // A plant holding above the threshold costs nothing.
    assert_eq!(
        loss_at(cfg.life_support_failure_threshold + 0.1),
        0,
        "a sustaining plant loses no one"
    );
    // A collapsing plant thins the crew, and a worse collapse thins it more.
    let half_failed = loss_at(cfg.life_support_failure_threshold / 2.0);
    let fully_failed = loss_at(0.0);
    assert!(half_failed > 0, "a failing plant costs lives");
    assert!(
        fully_failed > half_failed,
        "a fully collapsed plant thins the crew faster than a half-failed one \
             ({fully_failed} vs {half_failed})"
    );
}

#[test]
fn a_green_garden_helps_the_air_plant_sustain_the_crew() {
    // Content-depth subsystems round 17: the green decks are the ship's lungs. A
    // living agriculture biosphere supplements the failing plant's effective
    // condition, so the same collapsed plant kills far fewer with a thriving
    // garden than with a dead one — real redundancy, but (capped below the
    // threshold) never a wholesale replacement for the plant.
    let data = GameData::load().unwrap();
    let cfg = &data.config.subsystems;
    assert!(
        cfg.agriculture_life_support_contribution > 0.0,
        "this test needs the bio life-support coupling enabled"
    );
    // Capped below the threshold: even a pristine garden cannot alone sustain air.
    assert!(
        cfg.agriculture_life_support_contribution < cfg.life_support_failure_threshold,
        "the garden softens a dead plant, it does not replace it"
    );

    let loss_with_garden = |garden: f32| -> u32 {
        let (_, mut sim) = campaign(17);
        sim.population.count = 1000;
        // A badly collapsed plant, on full power — only the garden differs.
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 0.0;
        sim.subsystems.get_mut("agriculture").unwrap().condition = garden;
        // Isolate the garden coupling from the round-31 medical life-support relief.
        sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.0;
        life_support_mortality_loss(&sim, &data)
    };

    let dead_garden = loss_with_garden(0.0);
    let green_garden = loss_with_garden(1.0);
    assert!(
        dead_garden > 0,
        "a dead plant with no garden thins the crew"
    );
    assert!(
        green_garden < dead_garden,
        "a thriving garden helps the plant sustain more of the crew \
             (green {green_garden} vs dead-garden {dead_garden})"
    );
    assert!(
        green_garden > 0,
        "but a garden alone cannot wholly replace a dead plant"
    );
}

#[test]
fn a_serving_infirmary_keeps_some_of_the_asphyxiating_alive() {
    // Content-depth subsystems round 31: when the air fails, the medics fight to keep the
    // asphyxiating alive, so the medical bay's condition mitigates the life-support-failure
    // deaths — but even a perfect infirmary only saves some; it cannot make air.
    let data = GameData::load().unwrap();
    let relief = data.config.subsystems.medical_life_support_relief;
    assert!(
        relief > 0.0,
        "this test needs the medical life-support coupling enabled"
    );
    let deaths_with_medical = |medical: f32| -> u32 {
        let (_, mut sim) = campaign(31);
        sim.population.count = 5000;
        // A fully collapsed plant, no garden, so only the infirmary differs.
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 0.0;
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
        sim.subsystems.get_mut("medical_bay").unwrap().condition = medical;
        life_support_mortality_loss(&sim, &data)
    };
    let no_infirmary = deaths_with_medical(0.0);
    let full_infirmary = deaths_with_medical(1.0);
    assert!(
        no_infirmary > 0,
        "a dead plant with no infirmary thins the crew"
    );
    assert!(
        full_infirmary < no_infirmary,
        "a serving infirmary saves some of the asphyxiating ({full_infirmary} vs {no_infirmary})"
    );
    assert!(
        full_infirmary > 0,
        "but even a perfect infirmary cannot make air — some are still lost"
    );
}

#[test]
fn a_power_starved_plant_kills_even_when_well_repaired() {
    // Content-depth provisioning round 15: a life-support plant needs power as
    // well as repair. A sound plant on a full grid sustains everyone; the same
    // sound plant on a near-empty grid thins the crew — power starvation is as
    // deadly as physical collapse.
    let data = GameData::load().unwrap();
    let critical = data.config.subsystems.life_support_energy_critical;
    assert!(
        critical > 0,
        "this test needs the power-starvation coupling"
    );

    let loss_at_energy = |energy: i64| -> u32 {
        let (_, mut sim) = campaign(13);
        sim.population.count = 1000;
        // A pristine plant — only the grid differs.
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 1.0;
        // Isolate power: a dead garden contributes no bio life-support (round 17),
        // so only the grid moves the effective condition here.
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
        sim.resources.energy = energy;
        life_support_mortality_loss(&sim, &data)
    };

    // A well-powered, sound plant loses no one.
    assert_eq!(
        loss_at_energy(critical * 2),
        0,
        "a plant with power and repair sustains the ship"
    );
    // The same sound plant on a near-dead grid cannot run, and the ship thins.
    assert!(
        loss_at_energy(0) > 0,
        "a sound plant with no current to run it still kills"
    );
}

#[test]
fn a_rotting_farm_feeds_fewer_than_a_pristine_one() {
    // Content-depth subsystems round 12: the food module's condition→output
    // coupling. A pristine farm yields the untouched baseline (factor 1.0),
    // and a degraded one yields proportionally less, so upkeep on the
    // hydroponics pays back every year — not only at the breakdown cliff.
    let (data, mut sim) = campaign(9);
    assert!(
        data.config.subsystems.agriculture_condition_food_penalty > 0.0,
        "this test needs the agriculture condition coupling enabled"
    );

    sim.subsystems.get_mut("agriculture").unwrap().condition = 1.0;
    let pristine = agriculture_condition_food_factor(&sim, &data);
    assert_eq!(pristine, 1.0, "a farm in full repair yields the baseline");

    sim.subsystems.get_mut("agriculture").unwrap().condition = 0.4;
    let neglected = agriculture_condition_food_factor(&sim, &data);
    assert!(
        neglected < pristine,
        "a rotting farm feeds fewer than a pristine one \
             (neglected {neglected} vs pristine {pristine})"
    );
    // The factor never turns food production negative, even at total collapse.
    sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
    assert!((0.0..=1.0).contains(&agriculture_condition_food_factor(&sim, &data)));
}
