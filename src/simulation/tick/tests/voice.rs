//! What the quiet years sound like: the ambient line a ship speaks when
//! nothing is happening reads its state, and its state takes precedence.

use super::*;

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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();

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
    data.config.campaign_skeleton.despair_beats.clear();
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
