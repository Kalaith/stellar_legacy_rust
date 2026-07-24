//! Tests for the advance loop and the economic year — split out of `tick.rs`
//! to keep it under the size limit.

use super::economy::{apply_voyage_drift, influence_governance_factor, quiet_ambient_pool};
use super::*;
use crate::data::GameData;
use crate::simulation::contract::start_contract;

/// Content-depth factions round 21: strip every people's quiet-voice lines so the
/// *ordinary* ambient falls back to the generic pool. The condition-precedence
/// tests below predate the factions↔voice coupling and assert on the generic
/// ordinary lines — this keeps them testing precedence, not whoever runs the ship
/// (the coupling itself is covered by `the_ordinary_quiet_reads_in_the_dominant_peoples_voice`).
fn without_faction_voices(data: &mut GameData) {
    let ids: Vec<String> = data.factions.ids().cloned().collect();
    for id in ids {
        if let Some(mut f) = data.factions.remove(&id) {
            f.ambient.clear();
            data.factions.insert(id, f);
        }
    }
}

fn fresh(seed: u64) -> (GameData, SimState) {
    let data = GameData::load().unwrap();
    let sim = SimState::new_campaign(
        &data,
        "preservers",
        seed,
        &crate::state::sim::founding_faction_ids(&data),
    );
    (data, sim)
}

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
fn the_ordinary_quiet_reads_in_the_dominant_peoples_voice() {
    // Content-depth factions round 21: the ambient dead-air line, in ordinary
    // times, draws from the largest aboard people's own quiet-voice lines — a
    // Hearth ship's calm and an Ascension ship's are nothing alike — but a real
    // *condition* (a long hunger) still speaks over any people's ordinary voice.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let make = |dominant_id: &str| {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
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

    // Ordinary conditions: each people's quiet reads in its own voice.
    let hearth = make("hearth_union");
    let ascension = make("ascension_circle");
    let hearth_pool = quiet_ambient_pool(&hearth, &data);
    let ascension_pool = quiet_ambient_pool(&ascension, &data);
    assert_eq!(
        hearth_pool,
        &data.factions.get("hearth_union").unwrap().ambient,
        "an ordinary Hearth quiet draws from the Hearth's own voice"
    );
    assert_ne!(
        hearth_pool, ascension_pool,
        "two different peoples' ordinary quiets read differently"
    );

    // A real condition speaks over the people's ordinary voice: a long hunger.
    let mut lean = make("hearth_union");
    lean.lean_food_years = data.config.flavor.ambient_lean_years_threshold + 5;
    assert_eq!(
        quiet_ambient_pool(&lean, &data),
        &data.config.flavor.ambient_lean,
        "a long hunger reads as hunger, whoever runs the ship"
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
fn a_far_drifted_ships_quiet_reads_alien() {
    // Content-depth voice round 10: the ambient dead-air lines reflect the ship's
    // identity. Past the drift threshold, a quiet stretch draws from the drifted
    // pool — the same lived-in texture gone strange — where an early ship's quiet
    // still reads familiar.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    let gap = data.config.flavor.ambient_gap_years;
    let threshold = data.config.flavor.ambient_drift_threshold;
    assert!(
        gap > 0 && threshold > 0.0 && data.config.flavor.ambient_drifted.len() >= 4,
        "this test needs the drift-aware ambient pool enabled"
    );
    without_faction_voices(&mut data);

    let run = |drift: f32| -> Vec<String> {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            6,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.cultural_drift = drift;
        for _ in 0..gap {
            advance_year(&mut sim, &data);
        }
        sim.log.iter().map(|l| l.text.clone()).collect()
    };
    let drifted = run(threshold + 0.1);
    let early = run(0.0);
    let ambient = &data.config.flavor.ambient;
    let ambient_drifted = &data.config.flavor.ambient_drifted;

    assert!(
        drifted.iter().any(|t| ambient_drifted.contains(t)),
        "a far-drifted ship's quiet reads alien"
    );
    assert!(
        early.iter().any(|t| ambient.contains(t)),
        "an early ship's quiet reads familiar"
    );
    assert!(
        !early.iter().any(|t| ambient_drifted.contains(t)),
        "an early ship's quiet is not yet alien"
    );
}

#[test]
fn a_hollowed_out_ships_quiet_reads_empty() {
    // Content-depth voice round 12: the ambient dead-air lines reflect the ship's
    // headcount. Once the crew has thinned past the threshold, a quiet stretch
    // draws from the hollow pool — the same lived-in texture gone sparse and
    // echoing — and it takes precedence over the drifted pool, since emptiness is
    // the louder note in a silence.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.depopulation_beats.clear();
    let gap = data.config.flavor.ambient_gap_years;
    let ceiling = data.config.flavor.ambient_population_threshold;
    assert!(
        gap > 0 && ceiling > 0 && data.config.flavor.ambient_hollow.len() >= 4,
        "this test needs the population-aware ambient pool enabled"
    );
    without_faction_voices(&mut data);

    let run = |count: u32, drift: f32| -> Vec<String> {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            6,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.count = count;
        sim.population.cultural_drift = drift;
        for _ in 0..gap {
            advance_year(&mut sim, &data);
        }
        sim.log.iter().map(|l| l.text.clone()).collect()
    };
    let hollow = &data.config.flavor.ambient_hollow;
    let ambient = &data.config.flavor.ambient;
    let drifted = &data.config.flavor.ambient_drifted;

    // A thinned crew reads hollow…
    let thinned = run(ceiling - 1, 0.0);
    assert!(
        thinned.iter().any(|t| hollow.contains(t)),
        "a hollowed-out ship's quiet reads empty"
    );
    // …a full crew reads its ordinary quiet…
    let full = run(ceiling + 400, 0.0);
    assert!(
        full.iter().any(|t| ambient.contains(t)),
        "a full ship's quiet reads ordinary"
    );
    assert!(
        !full.iter().any(|t| hollow.contains(t)),
        "a full ship's quiet is not yet hollow"
    );
    // …and on a ship both thinned *and* far-drifted, emptiness wins.
    let thinned_and_drifted = run(
        ceiling - 1,
        data.config.flavor.ambient_drift_threshold + 0.1,
    );
    assert!(
        thinned_and_drifted.iter().any(|t| hollow.contains(t))
            && !thinned_and_drifted.iter().any(|t| drifted.contains(t)),
        "emptiness is the louder note: hollow precedes drifted"
    );
}

#[test]
fn a_long_hungry_ships_quiet_reads_lean() {
    // Content-depth voice round 13: the ambient dead-air lines reflect a sustained
    // hunger. Once the ship has been lean for years, a quiet stretch draws from the
    // lean pool — the rationed, harvest-preoccupied texture — and it takes
    // precedence over the hollow pool, since a long hunger is the most immediate
    // lived condition.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.depopulation_beats.clear();
    let gap = data.config.flavor.ambient_gap_years;
    let lean_years = data.config.flavor.ambient_lean_years_threshold;
    assert!(
        gap > 0 && lean_years > 0 && data.config.flavor.ambient_lean.len() >= 4,
        "this test needs the scarcity-aware ambient pool enabled"
    );
    without_faction_voices(&mut data);

    let run = |lean: u32, count: u32| -> Vec<String> {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            6,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.count = count;
        // A lean run holds the larder below the lean line so the tick *keeps* the
        // injected streak (incrementing, not resetting); a fed run stocks it high so
        // the tick zeroes the streak. Either way the store stays above upkeep, so no
        // famine muddies the ambient read.
        let food = if lean > 0 { 2_000 } else { 1_000_000 };
        for _ in 0..gap {
            sim.resources.food = food;
            sim.lean_food_years = lean;
            advance_year(&mut sim, &data);
        }
        sim.log.iter().map(|l| l.text.clone()).collect()
    };
    let lean_pool = &data.config.flavor.ambient_lean;
    let ambient = &data.config.flavor.ambient;
    let hollow = &data.config.flavor.ambient_hollow;

    // A long-hungry ship reads lean…
    let hungry = run(lean_years, 1000);
    assert!(
        hungry.iter().any(|t| lean_pool.contains(t)),
        "a long-hungry ship's quiet reads lean"
    );
    // …a well-fed ship reads its ordinary quiet…
    let fed = run(0, 1000);
    assert!(
        fed.iter().any(|t| ambient.contains(t)) && !fed.iter().any(|t| lean_pool.contains(t)),
        "a well-fed ship's quiet is not lean"
    );
    // …and on a ship both hungry and hollowed, hunger is the louder note.
    let hungry_and_hollow = run(
        lean_years,
        data.config.flavor.ambient_population_threshold - 1,
    );
    assert!(
        hungry_and_hollow.iter().any(|t| lean_pool.contains(t))
            && !hungry_and_hollow.iter().any(|t| hollow.contains(t)),
        "a sustained hunger speaks louder in the quiet than an empty deck"
    );
}

#[test]
fn a_long_prosperous_ships_quiet_reads_fat() {
    // Content-depth voice round 14: the first positive-condition ambient. Once the
    // larder has stood full for years and no grimmer note holds, a quiet stretch
    // reads fat and easy — but any grim condition (here, a hollowed crew) still
    // takes precedence, since the good years only sound good on a ship not otherwise
    // in decline.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    let gap = data.config.flavor.ambient_gap_years;
    let fat_years = data.config.flavor.ambient_fat_years_threshold;
    assert!(
        gap > 0 && fat_years > 0 && data.config.flavor.ambient_fat.len() >= 4,
        "this test needs the prosperity-aware ambient pool enabled"
    );
    without_faction_voices(&mut data);

    let run = |fat: u32, count: u32| -> Vec<String> {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            6,
            &crate::state::sim::founding_faction_ids(&data),
        );
        // Hold the larder full so the tick keeps the injected plenty streak.
        for _ in 0..gap {
            sim.resources.food = 1_000_000;
            sim.fat_food_years = fat;
            sim.population.count = count;
            advance_year(&mut sim, &data);
        }
        sim.log.iter().map(|l| l.text.clone()).collect()
    };
    let fat_pool = &data.config.flavor.ambient_fat;
    let ambient = &data.config.flavor.ambient;
    let hollow = &data.config.flavor.ambient_hollow;

    // A long-prosperous ship reads fat…
    let prosperous = run(fat_years, 1000);
    assert!(
        prosperous.iter().any(|t| fat_pool.contains(t)),
        "a long-prosperous ship's quiet reads fat and easy"
    );
    // …a ship not notably flush reads its ordinary quiet…
    let ordinary = run(0, 1000);
    assert!(
        ordinary.iter().any(|t| ambient.contains(t))
            && !ordinary.iter().any(|t| fat_pool.contains(t)),
        "a merely getting-by ship's quiet is not fat"
    );
    // …and a prosperous but hollowed ship reads hollow — a grim note wins.
    let flush_but_empty = run(
        fat_years,
        data.config.flavor.ambient_population_threshold - 1,
    );
    assert!(
        flush_but_empty.iter().any(|t| hollow.contains(t))
            && !flush_but_empty.iter().any(|t| fat_pool.contains(t)),
        "the good years only sound good on a ship not otherwise in decline"
    );
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
fn a_year_produces_resources_and_consumes_food() {
    // Events off so the year runs to its boundary without a decision stop.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        21,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let food_before = sim.resources.food;
    let credits_before = sim.resources.credits;

    let crew_mult = crate::simulation::crew::production_multipliers(&sim, &data);
    advance_year(&mut sim, &data);

    assert_eq!(sim.year(), 1);
    let upkeep = (sim.population.count as f32 * data.config.food_per_person_per_year).ceil() as i64;
    assert_eq!(
        sim.resources.food,
        food_before + (data.config.base_production.food * crew_mult.food).floor() as i64 - upkeep
    );
    assert!(crew_mult.food > 1.0, "founding agronomist boosts food");
    assert!(sim.resources.credits >= credits_before);
    assert!(sim.ship.hull_integrity < 1.0);
}

#[test]
fn a_long_lean_wears_the_crews_spirits_down() {
    // Content-depth provisioning round 17: the axis's first *systemic* coupling. A
    // chronic hunger — years of a store below the lean line — drains morale each year
    // the lean holds, where a comfortably fed ship's spirits are untouched by the
    // larder. Isolated by matching two ships in all but their stores (production off,
    // a small crew so neither famines), so the only morale difference is the toll.
    let mut data = GameData::load().unwrap();
    assert!(
        data.config.chronic_hunger_morale_drain > 0.0 && data.config.chronic_hunger_years > 0,
        "this test needs the chronic-hunger coupling enabled"
    );
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.base_production.food = 0.0; // isolate the larder from fresh yield

    let make = |food: i64, lean_years: u32| -> SimState {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            17,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.count = 200; // a crew the stores easily feed (no famine)
        sim.resources.food = food;
        sim.lean_food_years = lean_years;
        sim
    };

    // A ship long lean (store below the lean line, years of it) vs one comfortably fed.
    let mut hungry = make(
        data.config.lean_food_threshold - 500,
        data.config.chronic_hunger_years,
    );
    let mut fed = make(data.config.fat_food_threshold + 5000, 0);
    assert_eq!(
        hungry.population.morale, fed.population.morale,
        "the two ships launch in the same spirits"
    );

    advance_year(&mut hungry, &data);
    advance_year(&mut fed, &data);

    assert!(
        hungry.population.morale < fed.population.morale,
        "a chronic hunger wears the crew's spirits down where a full larder does not \
         (hungry {} vs fed {})",
        hungry.population.morale,
        fed.population.morale
    );
    // All else matched, the gap is exactly the year's chronic-hunger toll.
    let gap = fed.population.morale - hungry.population.morale;
    assert!(
        (gap - data.config.chronic_hunger_morale_drain).abs() < 1e-4,
        "the morale gap is the chronic-hunger drain ({gap})"
    );
}

#[test]
fn identical_seeds_produce_identical_decades() {
    let (data, mut a) = fresh(77);
    let (_, mut b) = fresh(77);
    for _ in 0..10 {
        a.pending_event = None;
        a.pending_dilemma = None;
        b.pending_event = None;
        b.pending_dilemma = None;
        advance_year(&mut a, &data);
        advance_year(&mut b, &data);
    }
    assert_eq!(a.resources.credits, b.resources.credits);
    assert_eq!(a.population.count, b.population.count);
    assert_eq!(
        serde_json::to_string(&a.market.entries).unwrap(),
        serde_json::to_string(&b.market.entries).unwrap()
    );
    assert_eq!(a.log.len(), b.log.len());
}

#[test]
fn contract_completes_at_target_duration() {
    // Events off isolates the timeline; advance_year now hard-stops on phase
    // boundaries too (W2), so loop to completion rather than a fixed count.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    // Threshold beats fire independent of event chance (content-depth rounds
    // 2-3); clear them too so the timeline stays uninterrupted. The dead-air
    // backstop (round 5) is another event source that ignores event chance —
    // switch it off so the silent run stays silent.
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.reputation_beat_family.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.homecoming_beat_family.clear();
    // The mid-voyage beat (round 21) fires once at the deep middle of any full
    // voyage, and the founding beat (round 22) once early on — silence both for
    // these isolated-timeline runs too.
    data.config.campaign_skeleton.midvoyage_beat_family.clear();
    data.config.campaign_skeleton.founding_beat_family.clear();
    data.config
        .campaign_skeleton
        .power_transition_beat_family
        .clear();
    // The succession beat (round 18) forces an event when a sitting leader dies —
    // continuous mortality can kill one mid-run — so silence it for these
    // isolated-timeline tests too, along with the round-19 long-reign beat (an
    // enduring leader can trip it on a full voyage).
    data.config.campaign_skeleton.succession_beat_family.clear();
    data.config.campaign_skeleton.long_reign_beat_family.clear();
    data.config
        .campaign_skeleton
        .dynasty_crisis_beat_family
        .clear();
    // The subsystem-collapse beat (round 17) also ignores event chance; a full
    // unrepaired voyage rots engineering past its red line, so clear it too — and
    // likewise the round-23 hull-collapse beat, which a neglected hull trips, and the
    // round-24 air-collapse beat, which a neglected life-support trips.
    data.config.campaign_skeleton.subsystem_beats.clear();
    data.config.campaign_skeleton.hull_beat_family.clear();
    data.config.campaign_skeleton.air_beat_family.clear();
    // …and the round-25 becalmed beat, which a fuel-starved voyage trips.
    data.config.campaign_skeleton.becalmed_beat_family.clear();
    // …and the round-26 divergence beat, which a long voyage's rising adaptation trips.
    data.config.campaign_skeleton.divergence_beat_family.clear();
    // …and the round-27 cultural-divergence beat, which a long voyage's rising drift trips.
    data.config
        .campaign_skeleton
        .cultural_divergence_beat_family
        .clear();
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    // Plenty of food so the population survives the run deterministically.
    sim.resources.food = 1_000_000;

    let mut completed = None;
    // Each advance_year covers up to a year; the cap comfortably exceeds the
    // calls needed to reach target_duration_years * 12 months.
    for _ in 0..(template.target_duration_years * 12) {
        let report = advance_year(&mut sim, &data);
        if report.contract_completed.is_some() {
            completed = report.contract_completed;
            break;
        }
    }
    let (score, _) = completed.expect("contract must complete at its target duration");
    assert!(score > 0.0);
    let active = sim.contract.as_ref().unwrap();
    assert_eq!(
        active.months_elapsed,
        template.target_duration_years * 12,
        "completes exactly at the authored duration"
    );
    assert!(active.milestones.iter().all(|m| m.reached));
}

#[test]
fn a_phase_boundary_hard_stops_the_fast_forward() {
    // Events off + a fresh charter: a 10-yr advance departs and hard-stops
    // on the very first phase crossing (Preparation → Travel) after 1 month.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        9,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.resources.food = 1_000_000;

    let report = advance_months(&mut sim, &data, 120);
    assert_eq!(report.months_advanced, 1, "departure is a hard-stop");
    assert_eq!(
        report.phase_changed,
        Some(crate::data::contracts::ContractPhase::Travel)
    );
    assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Travel);
}

#[test]
fn a_certain_dilemma_fires_on_the_generation_boundary() {
    // Events off isolates the generation dilemma as the only decision.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 1.0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        11,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;

    for _ in 0..data.config.generation_interval_years {
        advance_year(&mut sim, &data);
    }
    let pending = sim
        .pending_dilemma
        .as_ref()
        .expect("a dilemma must confront the new generation at 100% chance");
    assert_eq!(pending.rolled_month_clock, sim.month_clock);
    // The dilemma blocks the month's event roll — one decision at a time.
    assert!(sim.pending_event.is_none());
}

#[test]
fn a_ten_year_advance_matches_ten_one_year_advances() {
    // Events off isolates the deterministic economic path so the two
    // cadences must land byte-for-byte on the same state.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;

    let mut fast = SimState::new_campaign(
        &data,
        "preservers",
        123,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let mut slow = SimState::new_campaign(
        &data,
        "preservers",
        123,
        &crate::state::sim::founding_faction_ids(&data),
    );
    fast.resources.food = 1_000_000;
    slow.resources.food = 1_000_000;

    let report = advance_months(&mut fast, &data, 120);
    assert_eq!(
        report.months_advanced, 120,
        "a clear 10-yr advance crosses exactly 120 months"
    );
    assert_eq!(fast.month_clock, 120);
    assert_eq!(fast.year(), 10);

    for _ in 0..10 {
        advance_year(&mut slow, &data);
    }
    assert_eq!(fast.month_clock, slow.month_clock);
    assert_eq!(fast.resources.credits, slow.resources.credits);
    assert_eq!(fast.population.count, slow.population.count);
    assert_eq!(
        fast.ship.hull_integrity.to_bits(),
        slow.ship.hull_integrity.to_bits(),
        "10 boundary ticks either way leave identical hull wear"
    );
}

#[test]
fn a_fast_advance_stops_at_the_generation_dilemma() {
    // Short generations + a certain dilemma + events off: a 10-yr press must
    // stop dead on the first generation boundary, not run the full 120.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 1.0;
    data.config.generation_interval_years = 5;

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        11,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;

    let report = advance_months(&mut sim, &data, 120);
    assert!(
        sim.pending_dilemma.is_some(),
        "the generation dilemma must block the fast-forward"
    );
    assert_eq!(
        report.months_advanced, 60,
        "stopped on the year-5 boundary, not the full 120 months"
    );
    assert!(report.months_advanced < 120);
    assert_eq!(sim.year(), 5);
}

#[test]
fn a_fired_event_is_dated_in_the_log() {
    // Force an event every month (no dilemmas) so a blocking one lands fast;
    // its pending date must match a stamped log line (W3).
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 1.0;
    data.config.event_chance_cap = 1.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        3,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;

    // Advance until a council-blocking event is pending.
    for _ in 0..40 {
        if sim.pending_event.is_some() {
            break;
        }
        advance_year(&mut sim, &data);
    }
    let pending = sim
        .pending_event
        .clone()
        .expect("a blocking event should fire under a certain event chance");
    let year = pending.rolled_month_clock / 12;
    let month = pending.rolled_month_clock % 12 + 1;
    assert!(
        sim.log.iter().any(|e| e.year == year && e.month == month),
        "the fired event must leave a log line dated Y{year}·M{month:02}"
    );
}

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
fn a_multi_year_famine_reads_with_variety() {
    // Content-depth voice round 6: a famine that lasts several years used to
    // reprint one line per year. It now draws from a pool indexed by year, so a
    // long famine reads as a lengthening ordeal, not a stuck message.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        13,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();
    // Starve the ship and keep it starving (no food, no food production).
    sim.resources.food = 0;
    sim.production.food = 0.0;

    for _ in 0..6 {
        advance_year(&mut sim, &data);
    }

    // Normalize a log line by collapsing its digit-run (the {losses} count) so it
    // can be matched against the authored famine templates.
    let normalize = |s: &str| -> String {
        let mut out = String::new();
        let mut in_digits = false;
        for c in s.chars() {
            if c.is_ascii_digit() {
                if !in_digits {
                    out.push_str("{losses}");
                    in_digits = true;
                }
            } else {
                in_digits = false;
                out.push(c);
            }
        }
        out
    };
    let templates: std::collections::HashSet<&str> = data
        .config
        .flavor
        .famine
        .iter()
        .map(|s| s.as_str())
        .collect();
    let seen: std::collections::HashSet<String> = sim
        .log
        .iter()
        .map(|e| normalize(&e.text))
        .filter(|n| templates.contains(n.as_str()))
        .collect();
    assert!(
        seen.len() >= 2,
        "a multi-year famine should surface more than one distinct line (saw {})",
        seen.len()
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
fn a_founding_beat_fires_once_as_the_launch_generation_passes() {
    // Content-depth campaign-skeleton round 22: the early member of the era trio. With
    // reactive rolls and the other beats off, the campaign must force a founding-era
    // reckoning the year it passes founding_beat_year — and only once, ever.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
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
    // A short founding year so the test flies only a few years, not fifty.
    data.config.campaign_skeleton.founding_beat_year = 4;

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

    // Before the founding year: no beat.
    for _ in 0..3 {
        advance_year(&mut sim, &data);
    }
    assert!(
        !sim.founding_beat_fired,
        "no founding beat before the launch generation has passed"
    );

    // Cross the founding year: the beat fires once, and does not re-fire after.
    for _ in 0..3 {
        if let Some(pending) = sim.pending_event.clone() {
            let t = data.events.get(&pending.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
        }
        advance_year(&mut sim, &data);
    }
    assert!(
        sim.founding_beat_fired,
        "the founding era's close forces a beat once the launch generation passes"
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
fn a_reputation_beat_fires_when_the_ships_name_becomes_defining() {
    // Content-depth campaign-skeleton round 16: the first beat on the ship's
    // cumulative character. With reactive rolls and the other beats off, only the
    // reputation beat can fire — and it must, once the mercy trait crosses into a
    // strong band, once per crossing, re-arming when the name returns to the middle.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.stability_beats.clear();
    // Isolate the crossings we set from the dominant-faction reputation drift.
    data.config.factions.dominant_reputation_lean_per_year = 0.0;
    let high = data.config.campaign_skeleton.reputation_beat_high;
    let low = data.config.campaign_skeleton.reputation_beat_low;

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

    let resolve_pending = |sim: &mut SimState| {
        if let Some(p) = sim.pending_event.clone() {
            let t = data.events.get(&p.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(sim, &data, &t, 0);
        }
    };

    // A neutral name marks nothing.
    sim.reputation.insert("mercy".to_string(), 0.5);
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.reputation_beat_band, 0,
        "a middling name is no reckoning"
    );

    // A famously merciful name: the beat fires.
    sim.reputation.insert("mercy".to_string(), high + 0.05);
    sim.contract.as_mut().unwrap().beats.clear();
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.reputation_beat_band, 1,
        "crossing into a merciful name forces the reckoning"
    );
    resolve_pending(&mut sim);

    // Back to the middle re-arms silently.
    sim.reputation.insert("mercy".to_string(), 0.5);
    sim.contract.as_mut().unwrap().beats.clear();
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.reputation_beat_band, 0,
        "a return to the middle re-arms"
    );

    // A feared name: the beat fires afresh, in the other band.
    sim.reputation.insert("mercy".to_string(), low - 0.05);
    sim.contract.as_mut().unwrap().beats.clear();
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.reputation_beat_band, -1,
        "crossing into a feared name reckons anew"
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
fn the_sunset_relief_plays_its_two_act_scripted_arc_in_order() {
    // Content-depth charters round 10: the first scripted-narrative charter — a
    // mission architected around a *sequence* of timed beats, an authored arc
    // rather than an emergent one. The sunset relief fires its rising tide, then
    // its last evacuation, in order, on their appointed years.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.reputation_beat_family.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.homecoming_beat_family.clear();
    // The mid-voyage beat (round 21) fires once at the deep middle of any full
    // voyage, and the founding beat (round 22) once early on — silence both for
    // these isolated-timeline runs too.
    data.config.campaign_skeleton.midvoyage_beat_family.clear();
    data.config.campaign_skeleton.founding_beat_family.clear();
    data.config
        .campaign_skeleton
        .power_transition_beat_family
        .clear();
    // The succession beat (round 18) forces an event when a sitting leader dies —
    // continuous mortality can kill one mid-run — so silence it for these
    // isolated-timeline tests too, along with the round-19 long-reign beat (an
    // enduring leader can trip it on a full voyage).
    data.config.campaign_skeleton.succession_beat_family.clear();
    data.config.campaign_skeleton.long_reign_beat_family.clear();
    data.config
        .campaign_skeleton
        .dynasty_crisis_beat_family
        .clear();
    // The subsystem-collapse beat (round 17) also ignores event chance; a full
    // unrepaired voyage rots engineering past its red line, so clear it too — and
    // likewise the round-23 hull-collapse beat, which a neglected hull trips, and the
    // round-24 air-collapse beat, which a neglected life-support trips.
    data.config.campaign_skeleton.subsystem_beats.clear();
    data.config.campaign_skeleton.hull_beat_family.clear();
    data.config.campaign_skeleton.air_beat_family.clear();
    // …and the round-25 becalmed beat, which a fuel-starved voyage trips.
    data.config.campaign_skeleton.becalmed_beat_family.clear();
    // …and the round-26 divergence beat, which a long voyage's rising adaptation trips.
    data.config.campaign_skeleton.divergence_beat_family.clear();
    // …and the round-27 cultural-divergence beat, which a long voyage's rising drift trips.
    data.config
        .campaign_skeleton
        .cultural_divergence_beat_family
        .clear();
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;

    let charter = data.contracts.get("the_sunset_relief").unwrap().clone();
    assert_eq!(charter.scheduled_beats.len(), 2, "a two-act scripted arc");
    let acts: Vec<u32> = charter.scheduled_beats.iter().map(|b| b.at_year).collect();
    assert!(acts[0] < acts[1], "the acts are ordered");
    for b in &charter.scheduled_beats {
        assert!(
            data.events.get(&b.template_id).unwrap().scheduled_only,
            "each act is a scheduled-only beat"
        );
    }

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    sim.contract = Some(start_contract(&charter, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    let resolve_pending = |sim: &mut SimState, data: &GameData| {
        if let Some(p) = sim.pending_event.clone() {
            let t = data.events.get(&p.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(sim, data, &t, 0);
        }
    };
    // Fly past the second act's year; both beats must have fired, in order.
    let mut fired_at: Vec<u32> = Vec::new();
    let mut last = 0u32;
    while sim
        .contract
        .as_ref()
        .is_some_and(|c| (c.months_elapsed / 12) <= acts[1] + 2)
    {
        let before = sim.contract.as_ref().map_or(0, |c| c.scheduled_beats_fired);
        advance_year(&mut sim, &data);
        if let Some(c) = sim.contract.as_ref() {
            if c.scheduled_beats_fired > before {
                fired_at.push(c.months_elapsed / 12);
                last = c.scheduled_beats_fired;
            }
        }
        resolve_pending(&mut sim, &data);
        if sim.contract.is_none() {
            break;
        }
    }
    assert_eq!(last, 2, "both acts of the scripted arc fired");
    assert!(
        fired_at.len() == 2 && fired_at[0] < fired_at[1],
        "the tide rose before the last evacuation: {fired_at:?}"
    );
}

#[test]
fn a_charter_fires_its_scripted_beat_on_its_appointed_year() {
    // Content-depth charters round 9: a mission built around a reckoning on a
    // known clock. The sunward dive schedules a stellar beat at a fixed voyage
    // year; it fires when the voyage reaches it, and the payoff is scheduled_only
    // so it never rolls on its own.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.reputation_beat_family.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.homecoming_beat_family.clear();
    // The mid-voyage beat (round 21) fires once at the deep middle of any full
    // voyage, and the founding beat (round 22) once early on — silence both for
    // these isolated-timeline runs too.
    data.config.campaign_skeleton.midvoyage_beat_family.clear();
    data.config.campaign_skeleton.founding_beat_family.clear();
    data.config
        .campaign_skeleton
        .power_transition_beat_family
        .clear();
    // The succession beat (round 18) forces an event when a sitting leader dies —
    // continuous mortality can kill one mid-run — so silence it for these
    // isolated-timeline tests too, along with the round-19 long-reign beat (an
    // enduring leader can trip it on a full voyage).
    data.config.campaign_skeleton.succession_beat_family.clear();
    data.config.campaign_skeleton.long_reign_beat_family.clear();
    data.config
        .campaign_skeleton
        .dynasty_crisis_beat_family
        .clear();
    // The subsystem-collapse beat (round 17) also ignores event chance; a full
    // unrepaired voyage rots engineering past its red line, so clear it too — and
    // likewise the round-23 hull-collapse beat, which a neglected hull trips, and the
    // round-24 air-collapse beat, which a neglected life-support trips.
    data.config.campaign_skeleton.subsystem_beats.clear();
    data.config.campaign_skeleton.hull_beat_family.clear();
    data.config.campaign_skeleton.air_beat_family.clear();
    // …and the round-25 becalmed beat, which a fuel-starved voyage trips.
    data.config.campaign_skeleton.becalmed_beat_family.clear();
    // …and the round-26 divergence beat, which a long voyage's rising adaptation trips.
    data.config.campaign_skeleton.divergence_beat_family.clear();
    // …and the round-27 cultural-divergence beat, which a long voyage's rising drift trips.
    data.config
        .campaign_skeleton
        .cultural_divergence_beat_family
        .clear();
    data.config.campaign_skeleton.dead_air_years = 0;
    data.config.campaign_skeleton.anniversary_years = 0;

    let dive = data.contracts.get("the_sunward_dive").unwrap().clone();
    let beat = dive
        .scheduled_beats
        .first()
        .expect("the dive carries a scripted beat")
        .clone();
    assert!(
        data.events.get(&beat.template_id).unwrap().scheduled_only,
        "a scripted charter beat must be scheduled_only"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    sim.contract = Some(start_contract(&dive, &sim));
    // Clear the seeded skeleton so only the scripted beat can fire.
    sim.contract.as_mut().unwrap().beats.clear();
    assert_eq!(
        sim.contract.as_ref().unwrap().scheduled_beats.len(),
        1,
        "the scripted beat is copied onto the active contract"
    );

    let resolve_pending = |sim: &mut SimState, data: &GameData| {
        if let Some(p) = sim.pending_event.clone() {
            let t = data.events.get(&p.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(sim, data, &t, 0);
        }
    };
    // Before the appointed year the star is silent.
    while (sim.contract.as_ref().unwrap().months_elapsed / 12) < beat.at_year {
        assert_eq!(
            sim.contract.as_ref().unwrap().scheduled_beats_fired,
            0,
            "the appointed hour has not come (year {})",
            sim.contract.as_ref().unwrap().months_elapsed / 12
        );
        advance_year(&mut sim, &data);
        resolve_pending(&mut sim, &data);
        if sim.contract.is_none() {
            break; // completed early (should not, mid-voyage)
        }
    }
    assert!(
        sim.contract
            .as_ref()
            .is_some_and(|c| c.scheduled_beats_fired == 1),
        "the star's appointed hour fires on its year"
    );
}

#[test]
fn a_scheduled_followup_fires_on_its_determined_year_not_before() {
    // Content-depth event families round 9: the deterministic-timing chain. Sealing
    // the capsule queues its payoff for a fixed year; the payoff fires then and not
    // before, and — being scheduled_only — never rolls into the pool on its own.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();

    let setup = data.events.get("the_sealed_capsule").unwrap();
    let payoff = data.events.get("the_capsule_opens").unwrap();
    assert!(
        payoff.scheduled_only,
        "the payoff must be scheduled-only so it never rolls on its own"
    );
    let delay = setup
        .outcomes
        .iter()
        .find_map(|o| o.schedule_followup.as_ref())
        .expect("sealing schedules a follow-up")
        .delay_years;

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("the_long_dark").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // Seal the capsule: a follow-up is queued for exactly `delay` years on.
    let seal = setup
        .outcomes
        .iter()
        .position(|o| o.id == "seal_the_capsule")
        .unwrap();
    let year0 = sim.year();
    crate::simulation::event_resolver::apply_outcome(&mut sim, &data, setup, seal);
    assert_eq!(
        sim.scheduled_events.len(),
        1,
        "sealing queues one follow-up"
    );
    assert_eq!(sim.scheduled_events[0].fire_year, year0 + delay);

    // Advance year by year, always resolving any block so time can keep moving.
    // The capsule stays sealed every year before its due year.
    let resolve_pending = |sim: &mut SimState, data: &GameData| {
        if let Some(p) = sim.pending_event.clone() {
            let t = data.events.get(&p.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(sim, data, &t, 0);
        }
    };
    while sim.year() < year0 + delay {
        assert_eq!(
            sim.scheduled_events.len(),
            1,
            "the capsule has not opened before its year (year {})",
            sim.year()
        );
        advance_year(&mut sim, &data);
        resolve_pending(&mut sim, &data);
    }

    // On/after the due year the payoff has fired and the queue has emptied.
    assert!(
        sim.scheduled_events.is_empty(),
        "the capsule opens on its determined year"
    );
    assert!(
        sim.log.iter().any(|l| l.text.contains("capsule")),
        "the opening is narrated"
    );
}

#[test]
fn a_power_transition_beat_fires_when_the_ship_changes_hands() {
    // Content-depth campaign-skeleton round 11: a beat keyed to a political change.
    // With reactive rolls and the threshold beats off, nothing fires while the
    // majority holds — but the first tick a different people runs the ship, the
    // power-transition beat is forced (and the launch majority is only recorded,
    // never marked with a beat).
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    assert!(
        !data
            .config
            .campaign_skeleton
            .power_transition_beat_family
            .is_empty(),
        "this test needs the power-transition beat enabled"
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
    // A clear majority so demographic noise cannot flip it on its own.
    let fs = |id: &str, m: u32| FactionState {
        faction_id: id.to_string(),
        members: m,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    sim.factions = vec![fs("steel_covenant", 700), fs("hearth_union", 300)];
    sim.population.count = 1000;

    // First year: the launch majority is only recorded, no beat.
    advance_year(&mut sim, &data);
    assert_eq!(sim.last_dominant_faction, "steel_covenant");

    // Flip to a decisive new majority: the transition beat fires (marking the new
    // majority is the firer's own act, so the updated record proves it fired).
    sim.factions[0].members = 100;
    sim.factions[1].members = 900;
    assert_eq!(sim.dominant_faction_id(), Some("hearth_union"));
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.last_dominant_faction, "hearth_union",
        "the skeleton fires on the change and marks the new majority"
    );

    // Resolve whatever beat it surfaced, then advance again: the majority holds,
    // so no further transition beat and the record stays put.
    if let Some(p) = sim.pending_event.clone() {
        let t = data.events.get(&p.template_id).cloned().unwrap();
        crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
    }
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.last_dominant_faction, "hearth_union",
        "a steady majority is not re-marked"
    );
}

#[test]
fn the_homecoming_beat_fires_when_the_voyage_turns_for_home() {
    // Content-depth campaign-skeleton round 10: the first beat keyed to a phase.
    // With reactive rolls and the threshold beats off, nothing fires while the
    // ship is still outbound or on station — but the moment it enters its Return
    // leg, the homecoming beat is forced, exactly once.
    use crate::data::contracts::ContractPhase;
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    // This test jumps the clock past the voyage midpoint, so silence the round-21
    // mid-voyage beat to isolate the homecoming one.
    data.config.campaign_skeleton.midvoyage_beat_family.clear();
    assert!(
        !data
            .config
            .campaign_skeleton
            .homecoming_beat_family
            .is_empty(),
        "this test needs the homecoming beat enabled"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    // The phase is derived from months_elapsed, so drive the test by the clock:
    // months of travel + operation before the return leg begins.
    let mut months_before_return = 0u32;
    for p in &template.phases {
        if p.kind == ContractPhase::Return {
            break;
        }
        months_before_return += p.years * 12;
    }
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // Still on the outbound/operation legs: no homecoming beat.
    sim.contract.as_mut().unwrap().months_elapsed = months_before_return - 24;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().phase,
        ContractPhase::Operation,
        "still on station a year before the turn"
    );
    assert!(
        !sim.contract.as_ref().unwrap().homecoming_beat_fired,
        "the ship has not yet turned for home"
    );

    // Cross into the return leg: the beat fires this year, and only once.
    sim.contract.as_mut().unwrap().months_elapsed = months_before_return - 6;
    advance_year(&mut sim, &data);
    assert_eq!(
        sim.contract.as_ref().unwrap().phase,
        ContractPhase::Return,
        "the voyage has turned for home"
    );
    assert!(
        sim.contract.as_ref().unwrap().homecoming_beat_fired,
        "turning for home forces the homecoming beat"
    );
    // Resolve any block and advance again — it does not re-fire.
    if let Some(p) = sim.pending_event.clone() {
        let t = data.events.get(&p.template_id).cloned().unwrap();
        crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
    }
    advance_year(&mut sim, &data);
    assert!(
        sim.contract.as_ref().unwrap().homecoming_beat_fired,
        "the homecoming beat fires at most once a voyage"
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
fn dead_air_forces_a_beat_after_too_long_a_silence() {
    // Everything that could fire an event is off: no reactive rolls, no drift or
    // adaptation beats, no scheduled beats. The only thing left that can break
    // the silence is the dead-air backstop (content-depth round 5) — and it must,
    // once the eventless gap exceeds `dead_air_years`.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    // The succession beat (round 18) forces an event when a sitting leader dies —
    // continuous mortality can take one within the gap — so silence it too. The
    // plenty morale lift (round 20) would climb morale into a flourish beat over the
    // gap; clearing flourish covers it, but zero the lift too so the timeline is inert.
    data.config.campaign_skeleton.succession_beat_family.clear();
    data.config.sustained_plenty_morale_lift = 0.0;
    let dead = data.config.campaign_skeleton.dead_air_years;
    assert!(
        dead > 0 && !data.config.campaign_skeleton.dead_air_pool.is_empty(),
        "this test needs the dead-air backstop enabled"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        12,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    // Well short of the backstop: the silence stands, the event clock untouched.
    for _ in 0..(dead - 1) {
        advance_year(&mut sim, &data);
    }
    assert_eq!(
        sim.last_event_month_clock, 0,
        "nothing should force an event before the dead-air gap is reached"
    );

    // Cross the backstop: a beat is forced, which resets the event clock.
    for _ in 0..3 {
        if let Some(pending) = sim.pending_event.clone() {
            let t = data.events.get(&pending.template_id).cloned().unwrap();
            crate::simulation::event_resolver::apply_outcome(&mut sim, &data, &t, 0);
        }
        advance_year(&mut sim, &data);
    }
    assert!(
        sim.last_event_month_clock > 0,
        "a silence longer than the dead-air gap must force a beat"
    );
}

#[test]
fn ambient_flavor_surfaces_during_a_long_quiet_stretch() {
    // No events, no dilemmas, no drift beats: a pure quiet run. An ambient line
    // must appear once the event-less gap reaches ambient_gap_years.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    let gap = data.config.flavor.ambient_gap_years;
    assert!(gap > 0, "this test needs ambient flavor enabled");
    without_faction_voices(&mut data);
    let ambient: std::collections::HashSet<String> =
        data.config.flavor.ambient.iter().cloned().collect();

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        21,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000;
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.contract.as_mut().unwrap().beats.clear();

    for _ in 0..(gap + 1) {
        advance_year(&mut sim, &data);
    }
    assert!(
        sim.log.iter().any(|e| ambient.contains(&e.text)),
        "a quiet stretch of {gap}+ years should surface an ambient flavor line"
    );
}

fn provisioned(seed: u64, fuel: f32) -> (GameData, SimState) {
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", seed, &picks);
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.resources.food = 10_000_000;
    sim.ship.fuel = fuel;
    (data, sim)
}

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
fn a_failing_life_supports_toll_is_narrated_from_a_varied_pool() {
    // Content-depth voice round 24: the life-support mortality line, which once reprinted
    // one flat string every year the air failed, is now a pool. A crashed plant thins the
    // crew, and the loss is narrated by a substituted pool line, not a literal.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    assert!(
        data.config.flavor.life_support_loss.len() >= 3,
        "this test needs the pooled life-support loss lines"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.resources.food = 1_000_000; // keep famine out of the way
                                    // Crash the plant *and* the green decks that would otherwise supplement it
                                    // (subsystems r17), so the effective air falls well past the failure line.
    sim.subsystems
        .get_mut("life_support_habitat")
        .unwrap()
        .condition = 0.05;
    sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
    let before = sim.population.count;
    advance_year(&mut sim, &data);
    assert!(
        sim.population.count < before,
        "a failed life-support plant thins the crew"
    );

    // No line reads with the literal placeholder, and the loss matches a pool template.
    assert!(
        !sim.log.iter().any(|l| l.text.contains("{losses}")),
        "the loss line substitutes its count"
    );
    let narrated = sim.log.iter().any(|l| {
        data.config.flavor.life_support_loss.iter().any(|tmpl| {
            let (pre, post) = tmpl.split_once("{losses}").unwrap();
            l.text.starts_with(pre)
                && l.text.ends_with(post)
                && l.text.len() > pre.len() + post.len()
        })
    });
    assert!(narrated, "the life-support toll is narrated from the pool");
}

#[test]
fn a_chronic_becalming_wears_the_crews_spirits() {
    // Content-depth provisioning round 25: a ship stalled dry for years loses heart, the
    // fuel/mobility twin of the chronic-hunger morale drain. A ship that stays becalmed
    // this year ends it a shade grimmer than one that burns again, the gap the year's
    // becalmed drain exactly.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let drain = data.config.becalmed_morale_drain;
    let years = data.config.chronic_hunger_years;
    assert!(
        drain > 0.0 && years > 0,
        "this test needs the becalming drain enabled"
    );

    // No contract, so no travel burn touches the stall flag — we set it directly.
    let run = |stalled_this_year: bool| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;
        sim.population.morale = 0.6;
        sim.fuel_stall_years = years; // already chronically becalmed at the year's start
        sim.fuel_stalled_this_year = stalled_this_year;
        advance_year(&mut sim, &data);
        sim.population.morale
    };
    let stays_becalmed = run(true); // still stalled → the drain bites
    let burns_again = run(false); // burns again → counter resets, no drain
    assert!(
        stays_becalmed < burns_again,
        "a ship still going nowhere loses heart where one that burns again does not"
    );
    assert!(
        (burns_again - stays_becalmed - drain).abs() < 1e-4,
        "the gap is exactly the year's becalmed morale drain ({} vs {burns_again})",
        stays_becalmed
    );
}

#[test]
fn a_chronic_disrepair_wears_the_crews_spirits() {
    // Content-depth provisioning round 27: a ship left unmended for years loses heart, the
    // toolroom twin of the chronic-hunger and becalming morale drains. A ship that stays short
    // of its maintenance stock this year ends it a shade grimmer than one that can cover it,
    // the gap the year's disrepair drain exactly.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.fabrication_parts_yield = 0; // no fabrication topping up the parts mid-year
    let drain = data.config.disrepair_morale_drain;
    let years = data.config.chronic_hunger_years;
    let upkeep = data.config.parts_upkeep_per_year;
    assert!(
        drain > 0.0 && years > 0,
        "this test needs the disrepair drain enabled"
    );

    let run = |unmended: bool| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000; // keep famine out of the way
        sim.population.morale = 0.6;
        sim.lean_parts_years = years; // already chronically unmended at the year's start
                                      // Short of upkeep → stays unmended; stocked → covers upkeep and the count resets.
        sim.ship.spare_parts = if unmended { 0 } else { upkeep + 100 };
        advance_year(&mut sim, &data);
        sim.population.morale
    };
    let stays_broken = run(true); // still unmended → the drain bites
    let mended = run(false); // stores cover upkeep → counter resets, no drain
    assert!(
        stays_broken < mended,
        "a ship still falling apart loses heart where one it can maintain does not"
    );
    assert!(
        (mended - stays_broken - drain).abs() < 1e-4,
        "the gap is exactly the year's disrepair morale drain ({stays_broken} vs {mended})"
    );
}

#[test]
fn over_deep_food_stores_spoil_toward_the_carrying_capacity() {
    // Content-depth provisioning round 24: food beyond what the ship can keep fresh
    // rots. A hoard above the carrying capacity loses a fraction of the excess each
    // year; a ship at sensible stores loses nothing.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let cap = data.config.food_carrying_capacity;
    assert!(
        cap > 0 && data.config.food_spoilage_fraction > 0.0,
        "this test needs the spoilage coupling enabled"
    );

    // A deep hoard: the excess above the cap erodes this year.
    let mut hoard = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let start = cap + 40_000;
    hoard.resources.food = start;
    advance_year(&mut hoard, &data);
    assert!(
        hoard.resources.food < start,
        "an over-deep hoard loses stores to spoilage ({} -> {})",
        start,
        hoard.resources.food
    );
    assert!(
        hoard.resources.food > cap,
        "spoilage only erodes toward the cap, not below it in one year"
    );

    // A ship at sensible stores (below the cap): spoilage takes nothing (production and
    // upkeep move it a little, but no spoilage line fires and it is never clipped down).
    let mut modest = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );
    modest.resources.food = cap / 2;
    advance_year(&mut modest, &data);
    let spoil_lines = modest
        .log
        .iter()
        .filter(|l| data.config.flavor.food_spoilage.contains(&l.text))
        .count();
    assert_eq!(
        spoil_lines, 0,
        "a ship below its carrying capacity loses nothing to spoilage"
    );
}

#[test]
fn a_power_rich_ship_fabricates_its_own_spare_parts() {
    // Content-depth provisioning round 21: idle reactor surplus — otherwise wasted —
    // is worked with raw ore into spare parts. A ship above the surplus line converts
    // each year (energy and minerals down, parts up); one below it does not.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    assert!(
        data.config.surplus_energy_threshold > 0 && data.config.fabrication_parts_yield > 0,
        "this test needs the fabrication mechanic enabled"
    );

    let run = |energy: i64| -> (i64, i64, i64) {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            8,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000; // keep famine out of the way
        sim.resources.energy = energy;
        sim.resources.minerals = 5_000;
        let (parts0, min0) = (sim.ship.spare_parts, sim.resources.minerals);
        advance_year(&mut sim, &data);
        (
            sim.ship.spare_parts - parts0,
            min0 - sim.resources.minerals,
            sim.resources.energy,
        )
    };

    // Above the surplus line vs a power-starved ship (energy 0). Both runs share the
    // seed, so their yearly production is identical — the difference isolates the
    // fabrication: net minerals spent differs by exactly the ore feedstock, and the
    // surplus run banks parts the starved one never gets.
    let (parts_rich, min_spent_rich, _e) = run(data.config.surplus_energy_threshold + 4_000);
    let (parts_poor, min_spent_poor, _e) = run(0);
    assert_eq!(
        min_spent_rich - min_spent_poor,
        data.config.fabrication_minerals_cost,
        "the surplus run spends exactly its ore feedstock more than the starved run"
    );
    assert!(
        parts_rich > parts_poor,
        "the surplus buys parts the starved ship never gets ({parts_rich} vs {parts_poor})"
    );
}

#[test]
fn a_governed_ship_mints_full_influence_and_a_collapsing_one_earns_less() {
    // Content-depth provisioning round 26: influence is political capital, only as real as
    // the institutions that mint it. A ship at or above the governance line earns full
    // income (factor 1.0); below it the factor falls proportionally toward the floor at
    // zero stability — but never to zero, and never above 1.0.
    let data = GameData::load().unwrap();
    let threshold = data.config.influence_governance_threshold;
    let floor = data.config.influence_governance_floor;
    assert!(threshold > 0.0, "this test needs the coupling enabled");
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );

    // At and above the line: full income.
    sim.population.stability = threshold;
    assert_eq!(influence_governance_factor(&sim, &data.config), 1.0);
    sim.population.stability = 1.0;
    assert_eq!(influence_governance_factor(&sim, &data.config), 1.0);

    // Below the line: less than full, and monotonically lower as governance slips.
    sim.population.stability = threshold * 0.5;
    let mid = influence_governance_factor(&sim, &data.config);
    assert!(
        mid < 1.0 && mid > floor,
        "a slipping government earns less ({mid})"
    );

    // Total collapse: the floor exactly, never zero.
    sim.population.stability = 0.0;
    let collapsed = influence_governance_factor(&sim, &data.config);
    assert!(
        (collapsed - floor).abs() < 1e-6,
        "an ungoverned ship mints only the floor ({collapsed} vs {floor})"
    );
    assert!(
        collapsed > 0.0,
        "even a collapsed government mints something"
    );
}

#[test]
fn characters_age_die_and_the_line_renews_over_a_long_voyage() {
    // Real-time loop follow-up: aging is yearly, death is a monthly age-scaled
    // roll, and yearly births keep the line viable. Over a long crossing the
    // founders pass on, leadership changes hands, new members come of age, and
    // the dynasty survives — always led while anyone lives.
    let (data, mut sim) = provisioned(3, 1.0);
    let founder_id = sim.dynasty.leader().unwrap().id;
    let founding_next_id = sim.dynasty.next_member_id;
    let founding_ages: Vec<u32> = sim.dynasty.members.iter().map(|m| m.age).collect();

    for _ in 0..(120 * 12) {
        sim.pending_event = None;
        sim.pending_dilemma = None;
        advance_months(&mut sim, &data, 1);
        if sim.dynasty.extinct {
            break;
        }
    }

    assert!(
        !sim.dynasty.extinct,
        "a renewing line survives the crossing"
    );
    assert!(
        sim.dynasty.leader().is_some(),
        "a living dynasty is always led"
    );
    assert_ne!(
        sim.dynasty.leader().unwrap().id,
        founder_id,
        "the founding leader did not reign for 120 years"
    );
    assert!(
        sim.dynasty.next_member_id > founding_next_id,
        "new members came of age to renew the line"
    );
    // The surviving members are not the founding cohort frozen in time.
    let current_ages: Vec<u32> = sim.dynasty.members.iter().map(|m| m.age).collect();
    assert_ne!(
        current_ages, founding_ages,
        "the roster aged and turned over"
    );
}

#[test]
fn an_enduring_reign_earns_a_long_reign_beat_once() {
    // Content-depth campaign skeleton round 19: a leader who beats the odds of
    // continuous mortality and holds the chair for `long_reign_years` earns a beat,
    // once — the hopeful mirror of the succession beat.
    let (data, mut sim) = provisioned(3, 1.0);
    // Keep the leader young so no death/retirement resets the reign mid-test.
    for member in &mut sim.dynasty.members {
        if member.is_leader {
            member.age = 40;
        }
    }
    let threshold = data.config.campaign_skeleton.long_reign_years;
    assert!(threshold > 0, "the long-reign beat must be configured");
    sim.dynasty.leader_reign_years = threshold;
    assert!(
        !sim.dynasty.long_reign_marked,
        "the reign is not yet marked"
    );

    sim.pending_event = None;
    sim.pending_dilemma = None;
    advance_months(&mut sim, &data, 1);
    assert!(
        sim.dynasty.long_reign_marked,
        "an enduring reign is marked with a beat"
    );

    // A fresh succession re-arms it for the next reign.
    crate::simulation::succession::install_successor(&mut sim.dynasty, &data.config);
    assert!(
        !sim.dynasty.long_reign_marked && sim.dynasty.leader_reign_years == 0,
        "a handoff starts a new, unmarked reign"
    );
}

#[test]
fn a_long_plenty_lifts_the_crews_spirits() {
    // Content-depth provisioning round 20: a fat spell held past the sustained
    // threshold eases morale each year — the mirror of the chronic-hunger drain.
    let (data, base) = provisioned(5, 1.0);
    // Next month crosses a year boundary; everything else identical between the two.
    let setup = |food: i64, fat_years: u32| {
        let mut s = base.clone();
        s.month_clock = 11;
        s.resources.food = food;
        s.fat_food_years = fat_years;
        s.lean_food_years = 0;
        s.population.morale = 0.5;
        s.pending_event = None;
        s.pending_dilemma = None;
        s
    };
    let mut fat = setup(100_000, data.config.chronic_hunger_years.max(1));
    let mut plain = setup(8_000, 0);

    advance_months(&mut fat, &data, 1);
    advance_months(&mut plain, &data, 1);
    assert!(
        fat.population.morale > plain.population.morale,
        "a well-fed generation is a happier one (fat {} vs plain {})",
        fat.population.morale,
        plain.population.morale
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
fn a_dwindled_line_forces_a_dynasty_crisis_beat_once() {
    // Content-depth campaign skeleton round 20: when the founding line dwindles to
    // the crisis size, a beat marks the ship's brush with the end of its dynasty.
    let (data, mut sim) = provisioned(3, 1.0);
    // Thin the line into crisis (the leader stays, so no succession churn).
    sim.dynasty.members.truncate(2);
    assert!(
        (sim.dynasty.members.len() as u32) <= data.config.campaign_skeleton.dynasty_crisis_size
    );
    assert!(!sim.dynasty.dynasty_crisis_marked, "not yet marked");

    sim.pending_event = None;
    sim.pending_dilemma = None;
    advance_months(&mut sim, &data, 1);
    assert!(
        sim.dynasty.dynasty_crisis_marked,
        "the near-end of the founding line is marked with a beat"
    );
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
