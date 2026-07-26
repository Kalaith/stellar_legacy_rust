//! What a module's condition and knowledge are worth: the morale it
//! lifts, the decay it slows, the yield it turns out.

use super::*;

#[test]
fn a_sound_habitat_lifts_morale_and_a_failing_one_drags_it() {
    // Content-depth subsystems round 11: the habitat is where the people live,
    // so its condition moves morale — a home above the midpoint lifts spirits,
    // one below it depresses them, and a neutral one does neither.
    let (data, mut sim) = campaign(12);
    let swing = data.config.subsystems.habitat_morale_swing;
    assert!(
        swing > 0.0,
        "this test needs the habitat morale coupling enabled"
    );

    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 1.0;
    assert!(
        habitat_morale_effect(&sim, &data) > 0.0,
        "a home kept sound lifts the ship's spirits"
    );
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.1;
    assert!(
        habitat_morale_effect(&sim, &data) < 0.0,
        "a failing home drags the ship's spirits down"
    );
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.5;
    assert_eq!(
        habitat_morale_effect(&sim, &data),
        0.0,
        "a middling home neither lifts nor drags"
    );
}

#[test]
fn a_living_culture_lifts_morale_and_a_hollow_one_drags_it() {
    // Content-depth subsystems round 22: the ship's cultural life is a pillar of
    // morale beside the physical home — a vivid education/culture module lifts
    // spirits, a hollowed-out one drags them, a middling one does neither.
    let (data, mut sim) = campaign(19);
    assert!(
        data.config.subsystems.education_morale_swing > 0.0,
        "this test needs the cultural morale coupling enabled"
    );

    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 1.0;
    assert!(
        education_morale_effect(&sim, &data) > 0.0,
        "a living culture lifts the ship's spirits"
    );
    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 0.1;
    assert!(
        education_morale_effect(&sim, &data) < 0.0,
        "a hollowed-out cultural life drags the ship's spirits down"
    );
    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 0.5;
    assert_eq!(
        education_morale_effect(&sim, &data),
        0.0,
        "a middling cultural life neither lifts nor drags"
    );
}

#[test]
fn a_sound_engineering_bay_holds_the_hull_and_a_failing_one_lets_it_rot_faster() {
    // Content-depth subsystems round 24: the engineering bay maintains the hull, not
    // just the modules. A sound bay holds hull wear at the baseline (factor 1.0); a
    // failing one accelerates it; a good bay never drops below the baseline (no
    // immortal hulls).
    let (data, mut sim) = campaign(23);
    let penalty = data.config.subsystems.engineering_hull_decay_penalty;
    assert!(
        penalty > 0.0,
        "this test needs the engineering→hull coupling enabled"
    );

    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 1.0;
    assert_eq!(
        engineering_hull_decay_factor(&sim, &data),
        1.0,
        "a sound bay holds the hull at its baseline wear"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.5;
    let half = engineering_hull_decay_factor(&sim, &data);
    assert!(
        half > 1.0,
        "a failing bay lets the hull rot faster ({half})"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.0;
    let wrecked = engineering_hull_decay_factor(&sim, &data);
    assert!(
        wrecked > half,
        "a wrecked bay wears the hull fastest of all ({wrecked})"
    );
}

#[test]
fn a_failing_engineering_bay_rots_the_whole_ship_faster() {
    // Content-depth subsystems round 7: the engineering bay is the keystone —
    // its condition scales every *other* module's decay. A year with a sound
    // bay wears the medical bay less than a year with a failing one.
    assert!(
        data_swing() > 0.0,
        "this test needs the keystone coupling enabled"
    );

    let wear_med = |eng: f32| -> f32 {
        let (data, mut sim) = campaign(5);
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = eng;
        sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.8;
        decay_subsystems(&mut sim, &data, 1.0);
        0.8 - sim.subsystems["medical_bay"].condition
    };

    let sound = wear_med(1.0); // top-repair bay slows the rot
    let failing = wear_med(0.0); // a failing bay speeds it
    assert!(
        failing > sound,
        "a failing engineering bay should rot the ship faster than a sound one \
             (failing {failing} vs sound {sound})"
    );
}

#[test]
fn a_sharp_fabrication_hall_turns_out_the_full_run_and_a_wrecked_one_only_a_trickle() {
    // Content-depth subsystems round 26: the engineering bay is the fabrication hall,
    // so its condition scales the fabrication yield. A sharp bay fabricates the full
    // run (factor 1.0); a failing one turns out less; a wrecked one only a fraction —
    // but never a negative, and (with the caller's floor) never quite nothing.
    let (data, mut sim) = campaign(26);
    let penalty = data.config.subsystems.engineering_fabrication_penalty;
    assert!(
        penalty > 0.0,
        "this test needs the engineering→fabrication coupling enabled"
    );

    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 1.0;
    assert_eq!(
        engineering_fabrication_factor(&sim, &data),
        1.0,
        "a sharp bay fabricates the full run"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.5;
    let half = engineering_fabrication_factor(&sim, &data);
    assert!(
        half < 1.0 && half > 0.0,
        "a failing bay turns out less ({half})"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.0;
    let wrecked = engineering_fabrication_factor(&sim, &data);
    assert!(
        wrecked < half && wrecked >= 0.0,
        "a wrecked bay fabricates the least, but never a negative yield ({wrecked})"
    );
}

#[test]
fn a_rotting_bay_fouls_the_ships_own_scoops() {
    // Content-depth subsystems round 30: the engineering bay maintains the drive's intakes,
    // so its condition scales fuel scooping too — the production side of the burn coupling. A
    // sound bay scoops at the baseline (factor 1.0); a failing one less; a wrecked one least,
    // but never a negative.
    let (data, mut sim) = campaign(30);
    let penalty = data.config.subsystems.engineering_fuel_regen_penalty;
    assert!(
        penalty > 0.0,
        "this test needs the engineering→fuel-regen coupling enabled"
    );

    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 1.0;
    assert_eq!(
        engineering_fuel_regen_factor(&sim, &data),
        1.0,
        "a sound bay scoops at the full rate"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.5;
    let half = engineering_fuel_regen_factor(&sim, &data);
    assert!(
        half < 1.0 && half > 0.0,
        "a failing bay scoops less ({half})"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.0;
    let wrecked = engineering_fuel_regen_factor(&sim, &data);
    assert!(
        wrecked < half && wrecked >= 0.0,
        "a wrecked bay scoops the least, but never a negative ({wrecked})"
    );
}

#[test]
fn a_true_academy_trains_a_cohort_whole_and_a_crumbling_one_only_partly() {
    // Content-depth subsystems round 27: the education/culture module is the ship's
    // schools, so its condition scales how much a training cohort learns. A true academy
    // imparts the full gain (factor 1.0); a crumbling one less; a wrecked one least — but
    // never nothing (the penalty is below 1), so a crew can always bootstrap its schools
    // back. The actual training run must scale with the factor.
    let (data, mut sim) = campaign(27);
    let penalty = data.config.subsystems.education_training_penalty;
    assert!(
        penalty > 0.0,
        "this test needs the education→training coupling enabled"
    );

    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 1.0;
    assert_eq!(
        education_training_factor(&sim, &data),
        1.0,
        "a true academy trains the full cohort"
    );
    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 0.0;
    let wrecked = education_training_factor(&sim, &data);
    assert!(
        wrecked > 0.0 && wrecked < 1.0,
        "a wrecked academy still teaches something, but less ({wrecked})"
    );

    // The training run itself scales: a wrecked academy imparts less knowledge per cohort
    // than a sound one, both for the same credits.
    sim.resources.credits = 1_000_000;
    let gain = |sim: &mut SimState| {
        let before = sim.subsystems["security"].knowledge;
        train_subsystem_knowledge(sim, &data, "security").unwrap();
        sim.subsystems["security"].knowledge - before
    };
    sim.subsystems.get_mut("security").unwrap().knowledge = 0.2;
    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 1.0;
    let sound_gain = gain(&mut sim);
    sim.subsystems.get_mut("security").unwrap().knowledge = 0.2;
    sim.subsystems
        .get_mut("education_culture")
        .unwrap()
        .condition = 0.0;
    let wrecked_gain = gain(&mut sim);
    assert!(
        wrecked_gain > 0.0 && wrecked_gain < sound_gain,
        "a wrecked academy trains a fainter cohort ({wrecked_gain} vs {sound_gain})"
    );
}

#[test]
fn a_strong_corps_softens_the_strain_of_a_divided_polity() {
    // Content-depth subsystems round 28: the peacekeeping corps directly dampens the
    // ideology-spread governance drain. A full corps lets less of the drain through than a
    // wrecked one, but never wholly cancels it (the relief is below 1).
    let (data, mut sim) = campaign(28);
    let relief = data.config.subsystems.ideology_spread_security_relief;
    assert!(
        relief > 0.0,
        "this test needs the security→spread-relief coupling enabled"
    );

    sim.subsystems.get_mut("security").unwrap().condition = 1.0;
    let full = security_spread_relief_factor(&sim, &data);
    assert!(
        full < 1.0 && full > 0.0,
        "a full corps lets less of the drain through, but not none ({full})"
    );
    assert!(
        (full - (1.0 - relief)).abs() < 1e-6,
        "a full corps lets exactly (1 - relief) of the drain through"
    );

    sim.subsystems.get_mut("security").unwrap().condition = 0.5;
    let half = security_spread_relief_factor(&sim, &data);
    assert!(
        half > full && half < 1.0,
        "a half-kept corps softens the strain less ({half})"
    );

    sim.subsystems.get_mut("security").unwrap().condition = 0.0;
    assert_eq!(
        security_spread_relief_factor(&sim, &data),
        1.0,
        "no corps softens nothing — the full drain lands"
    );
}

#[test]
fn a_functioning_security_corps_keeps_the_institutions_in_order() {
    // Content-depth subsystems round 16: the security corps' governance domain.
    // A sound corps recovers stability toward the ceiling, a wrecked one far
    // less, and a ship already well-ordered gets no boost.
    let data = GameData::load().unwrap();
    let cfg = &data.config.subsystems;
    assert!(
        cfg.security_stability_recovery_per_condition > 0.0,
        "this test needs the security→stability coupling enabled"
    );
    let (_, mut sim) = campaign(17);

    // A fracturing government: a sound corps steadies it more than a decayed one.
    sim.population.stability = 0.3;
    sim.subsystems.get_mut("security").unwrap().condition = 1.0;
    let recover_sound = security_stability_recovery(&sim, &data);
    sim.subsystems.get_mut("security").unwrap().condition = 0.1;
    let recover_wrecked = security_stability_recovery(&sim, &data);
    assert!(
        recover_sound > recover_wrecked && recover_wrecked >= 0.0,
        "a functioning corps keeps the institutions in better order than a decayed one"
    );

    // A well-ordered ship at the ceiling gets no manufactured order.
    sim.population.stability = cfg.security_stability_recovery_ceiling;
    sim.subsystems.get_mut("security").unwrap().condition = 1.0;
    assert_eq!(
        security_stability_recovery(&sim, &data),
        0.0,
        "a well-governed ship needs no shoring up"
    );
}

#[test]
fn a_sound_infirmary_brings_the_cohort_up_whole_and_a_failing_one_loses_the_young() {
    // Content-depth subsystems round 23: the medical bay is what keeps the young
    // alive to grow up. A sound bay leaves the renewal at baseline; a failing one
    // scales it down (fewer of the cohort reach their majority); a wrecked one loses
    // the most, but never all (the penalty is a fraction).
    let (data, mut sim) = campaign(21);
    let penalty = data.config.subsystems.medical_renewal_penalty;
    assert!(
        penalty > 0.0,
        "this test needs the medical renewal coupling enabled"
    );

    sim.subsystems.get_mut("medical_bay").unwrap().condition = 1.0;
    assert_eq!(
        medical_renewal_factor(&sim, &data),
        1.0,
        "a sound infirmary keeps the baseline renewal"
    );
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.5;
    let half = medical_renewal_factor(&sim, &data);
    assert!(
        half < 1.0,
        "a failing infirmary raises fewer of the young ({half})"
    );
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.0;
    let wrecked = medical_renewal_factor(&sim, &data);
    assert!(
        wrecked < half && wrecked > 0.0,
        "a wrecked infirmary loses the most, but never all ({wrecked})"
    );
}

#[test]
fn a_well_kept_medical_bay_and_corps_earn_their_keep_by_condition() {
    // Content-depth subsystems round 9: the two modules that only ever cost
    // the ship now pay it back, and by how well they are *kept*. A sound
    // medical bay softens famine relief above a wrecked one; a sound security
    // corps recovers more unity than a wrecked one.
    let (data, mut sim) = campaign(9);

    // Medical: relief scales with condition, so a rotted bay saves fewer.
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 1.0;
    let relief_sound = medical_famine_relief(&sim, &data);
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.1;
    let relief_wrecked = medical_famine_relief(&sim, &data);
    assert!(
        relief_sound > relief_wrecked && relief_wrecked >= 0.0,
        "a bay in good repair keeps more of the starving alive"
    );

    // Security: recovery scales with condition, but only below the ceiling.
    sim.population.unity = 0.3;
    sim.subsystems.get_mut("security").unwrap().condition = 1.0;
    let recover_sound = security_unity_recovery(&sim, &data);
    sim.subsystems.get_mut("security").unwrap().condition = 0.1;
    let recover_wrecked = security_unity_recovery(&sim, &data);
    assert!(
        recover_sound > recover_wrecked,
        "a functioning corps steadies the ship more than a decayed one"
    );
    // Above the ceiling neither the chief nor the corps manufactures harmony.
    sim.population.unity = data.config.crew.unity_recovery_ceiling;
    sim.subsystems.get_mut("security").unwrap().condition = 1.0;
    assert_eq!(
        security_unity_recovery(&sim, &data),
        0.0,
        "a steady ship needs no steadying"
    );
}

#[test]
fn a_well_kept_medical_bay_thins_the_reapers_odds() {
    // Content-depth subsystems round 18: the infirmary's condition eases the
    // monthly age-based death roll — a sound bay saves more of the aging, and
    // it can never grant immortality.
    let (data, mut sim) = campaign(18);
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 1.0;
    let sound = medical_mortality_relief(&sim, &data);
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.1;
    let wrecked = medical_mortality_relief(&sim, &data);
    assert!(
        sound > wrecked && wrecked >= 0.0,
        "a bay in good repair lowers the reaper's odds more than a rotted one"
    );
    assert!(sound < 1.0, "the bay can never make anyone immortal");

    // The relief genuinely eases an elder's effective monthly chance.
    let cfg = &data.config.mortality;
    let max = data.config.member_max_age;
    let old = cfg.onset_age + 20;
    let base = crate::simulation::mortality::monthly_death_chance(old, cfg, max);
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 1.0;
    let eased = base * (1.0 - medical_mortality_relief(&sim, &data));
    assert!(
        eased < base,
        "a sound infirmary eases an aging character's odds"
    );
}

#[test]
fn a_failing_habitat_slows_the_dynastys_renewal() {
    // Content-depth subsystems round 19: the home the young are raised in scales
    // the yearly birth chance — a sound one keeps the baseline, a failing one
    // raises fewer children (but never none).
    let (data, mut sim) = campaign(19);
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 1.0;
    assert_eq!(
        habitat_renewal_factor(&sim, &data),
        1.0,
        "a sound home keeps the baseline renewal"
    );
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.0;
    let failed = habitat_renewal_factor(&sim, &data);
    assert!(
        (0.0..1.0).contains(&failed) && failed > 0.0,
        "a failed home raises fewer children, but not none"
    );
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.5;
    let mid = habitat_renewal_factor(&sim, &data);
    assert!(mid > failed && mid < 1.0, "a middling home falls between");
}

#[test]
fn a_mastered_module_decays_slower_than_one_the_crew_barely_knows() {
    // Content-depth subsystems round 33: a module's own knowledge slows its rot. Two identical
    // medical bays, one the crew has mastered and one they barely understand, wear at different
    // rates over a year — but even perfect mastery only slows the decay, never stops it. The
    // engineering bay is held neutral (condition 0.5 → keystone mult 1.0) so only knowledge
    // differs.
    assert!(
        GameData::load()
            .unwrap()
            .config
            .subsystems
            .knowledge_decay_reduction
            > 0.0,
        "this test needs the knowledge-upkeep coupling enabled"
    );

    let wear_med = |knowledge: f32| -> f32 {
        let (data, mut sim) = campaign(6);
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.5;
        sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.8;
        sim.subsystems.get_mut("medical_bay").unwrap().knowledge = knowledge;
        decay_subsystems(&mut sim, &data, 1.0);
        0.8 - sim.subsystems["medical_bay"].condition
    };

    let ignorant = wear_med(0.0); // a module the crew barely knows rots full
    let mastered = wear_med(1.0); // a mastered one rots slower
    assert!(
        mastered < ignorant,
        "a mastered module decays slower than one the crew barely knows \
             (mastered {mastered} vs ignorant {ignorant})"
    );
    assert!(
        mastered > 0.0,
        "but even perfect mastery does not stop the rot ({mastered})"
    );
}
