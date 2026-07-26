//! Voyage drift: the people aboard are measurably not the ones who left,
//! and what slows that change without ever stopping it.

use super::*;

#[test]
fn voyage_drift_changes_the_people_and_stays_bounded() {
    let data = GameData::load().unwrap();
    let mut sim = SimState::new_campaign(
        &data,
        "wanderers",
        1,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let (a0, d0, l0) = (
        sim.population.adaptation,
        sim.population.cultural_drift,
        sim.population.legacy_loyalty,
    );
    // A long voyage with no events at all still reshapes the crew.
    for _ in 0..40 {
        apply_voyage_drift(&mut sim, &data);
    }
    assert!(sim.population.adaptation > a0, "adaptation rises underway");
    assert!(sim.population.cultural_drift > d0, "cultural drift rises");
    assert!(
        sim.population.legacy_loyalty < l0,
        "loyalty to the founders fades"
    );
    for v in [
        sim.population.adaptation,
        sim.population.cultural_drift,
        sim.population.legacy_loyalty,
        sim.population.morale,
        sim.population.unity,
    ] {
        assert!((0.0..=1.0).contains(&v), "drift stays a 0-1 fraction: {v}");
    }
}

#[test]
fn voyage_drift_scales_by_legacy() {
    let data = GameData::load().unwrap();
    let mut adaptors = SimState::new_campaign(
        &data,
        "adaptors",
        1,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let mut preservers = SimState::new_campaign(
        &data,
        "preservers",
        1,
        &crate::state::sim::founding_faction_ids(&data),
    );
    for _ in 0..30 {
        apply_voyage_drift(&mut adaptors, &data);
        apply_voyage_drift(&mut preservers, &data);
    }
    assert!(
        adaptors.population.cultural_drift > preservers.population.cultural_drift,
        "Adaptors change faster than Preservers"
    );
}

#[test]
fn the_dominant_faction_ideology_bends_how_fast_the_people_drift() {
    // Content-depth factions round 9: who runs the ship finally steers its
    // identity. Two otherwise-identical ships (same legacy, same starting drift)
    // led by opposite peoples — the change-embracing Ascension vs the
    // tradition-bound Keepers — must drift apart, yet both still drift.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let make = |dominant_id: &str| {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![FactionState {
            faction_id: dominant_id.to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        }];
        sim
    };
    let mut embracing = make("ascension_circle"); // ideology +0.9
    let mut holding = make("first_flame"); // ideology -0.9
    let d0 = embracing.population.cultural_drift;
    assert_eq!(
        d0, holding.population.cultural_drift,
        "the two ships launch identical"
    );

    for _ in 0..40 {
        apply_voyage_drift(&mut embracing, &data);
        apply_voyage_drift(&mut holding, &data);
    }
    assert!(
        embracing.population.cultural_drift > holding.population.cultural_drift,
        "a change-embracing majority drifts the people from the founders faster"
    );
    assert!(
        holding.population.cultural_drift > d0,
        "even under the Keepers the people still change, only slower"
    );
}

#[test]
fn a_well_kept_infirmary_slows_the_shipborn_drift_but_never_stops_it() {
    // Content-depth subsystems round 25: the bodily twin of the archive's cultural
    // resistance. A ship whose infirmary keeps its medical craft alive holds the crew
    // closer to baseline-human, adapting slower — but the bodies still adapt, only less.
    let data = GameData::load().unwrap();
    assert!(
        data.config.voyage_drift.medical_adaptation_resistance > 0.0,
        "this test needs the medical adaptation coupling enabled"
    );
    let drift_over_20y = |med_knowledge: f32| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.subsystems.get_mut("medical_bay").unwrap().knowledge = med_knowledge;
        let a0 = sim.population.adaptation;
        for _ in 0..20 {
            apply_voyage_drift(&mut sim, &data);
        }
        sim.population.adaptation - a0
    };
    let with_infirmary = drift_over_20y(1.0); // full medical craft → slowed adaptation
    let without = drift_over_20y(0.0); // no craft → the bodies adapt at full rate
    assert!(
        with_infirmary < without,
        "a well-kept infirmary slows the shipborn drift: {with_infirmary} vs {without}"
    );
    assert!(
        with_infirmary > 0.0,
        "but the bodies still adapt to the ship, only slower"
    );
}

#[test]
fn a_living_biosphere_slows_the_shipborn_drift_but_never_stops_it() {
    // Content-depth subsystems round 29: the environmental twin of the infirmary's craft. A ship
    // whose agriculture keeps a living biosphere holds the crew closer to planet-like, adapting
    // slower — but the bodies still adapt, only less. Isolated from the medical resistance by
    // zeroing the infirmary's knowledge, so only the biosphere's condition moves the drift.
    let data = GameData::load().unwrap();
    assert!(
        data.config.voyage_drift.agriculture_adaptation_resistance > 0.0,
        "this test needs the agriculture adaptation coupling enabled"
    );
    let drift_over_20y = |agri_condition: f32| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.subsystems.get_mut("agriculture").unwrap().condition = agri_condition;
        sim.subsystems.get_mut("medical_bay").unwrap().knowledge = 0.0; // isolate the biosphere
        let a0 = sim.population.adaptation;
        for _ in 0..20 {
            apply_voyage_drift(&mut sim, &data);
        }
        sim.population.adaptation - a0
    };
    let lush = drift_over_20y(1.0); // a living biosphere → slowed adaptation
    let sterile = drift_over_20y(0.0); // dead grow-decks → the bodies adapt at full rate
    assert!(
        lush < sterile,
        "a living biosphere slows the shipborn drift: {lush} vs {sterile}"
    );
    assert!(
        lush > 0.0,
        "but the bodies still adapt to the ship, only slower"
    );
}

#[test]
fn a_well_kept_culture_archive_slows_the_cultural_drift_but_not_adaptation() {
    // Content-depth subsystems round 10: the education/culture archive is the
    // ship's memory of the founders. A vivid archive (high knowledge) resists the
    // cultural drift and the loyalty fade — but the bodies still adapt to the ship
    // whether the archive holds or not.
    let data = GameData::load().unwrap();
    let make = |archive: f32| {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            2,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.subsystems
            .get_mut("education_culture")
            .unwrap()
            .knowledge = archive;
        sim
    };
    let mut remembered = make(1.0); // the founding kept vivid
    let mut forgotten = make(0.0); // the archive lost
    let d0 = remembered.population.cultural_drift;
    let a0 = remembered.population.adaptation;
    assert_eq!(d0, forgotten.population.cultural_drift, "identical start");

    for _ in 0..50 {
        apply_voyage_drift(&mut remembered, &data);
        apply_voyage_drift(&mut forgotten, &data);
    }
    assert!(
        remembered.population.cultural_drift < forgotten.population.cultural_drift,
        "a vivid archive drifts culturally slower than a lost one"
    );
    assert!(
        remembered.population.cultural_drift > d0,
        "even a kept archive only slows the drift, never stops it"
    );
    // Adaptation is physiological and untouched by the archive: both adapt alike.
    assert!(
        (remembered.population.adaptation - forgotten.population.adaptation).abs() < 1e-6,
        "the archive does not slow the body's adaptation to the ship"
    );
    assert!(
        remembered.population.adaptation > a0,
        "adaptation still rises"
    );
}

#[test]
fn a_neglected_generational_voyage_wears_the_ship_to_the_edge() {
    // Events off + well-fed isolates the wear curve (PLAN M4.2) for a
    // charter-length voyage flown with *no* field repairs — the neglect
    // baseline the autoplay repair policy is measured against (W1-rescale).
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    // Disable in-flight fabrication (round 21): this test measures the *pure* neglect
    // wear curve, and a power-rich ship would otherwise refill its own parts from idle
    // reactor surplus and never run the stores dry.
    data.config.surplus_energy_threshold = 0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    // Pin the stores to a 60-part baseline (the founding stock is far larger
    // now) so the test keeps measuring the unmaintained wear curve: ~60
    // maintained years, then the ship wears at full rate.
    sim.ship.spare_parts = 60;

    for _ in 0..300 {
        advance_year(&mut sim, &data);
    }

    assert_eq!(
        sim.ship.spare_parts, 0,
        "a generational voyage long outlasts the spare-parts stores"
    );
    // Still nominally flying (hull > 0), but only just — held together on
    // hope and prayers, a hair from total loss. This is why the voyage
    // needs the field-repair sink the autoplay policy exercises.
    assert!(
        (0.0..=0.10).contains(&sim.ship.hull_integrity),
        "a neglected 300-year voyage should limp in near total loss: hull {}",
        sim.ship.hull_integrity
    );
}
