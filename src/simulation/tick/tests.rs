//! Tests for the advance loop and the economic year — split out of `tick.rs`
//! to keep it under the size limit.

use super::economy::apply_voyage_drift;
use super::*;
use crate::data::GameData;
use crate::simulation::contract::start_contract;

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
    data.config.campaign_skeleton.objective_beats.clear();
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
    sim.speed = SpeedStep::TenYears;

    let report = advance(&mut sim, &data);
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

    fast.speed = SpeedStep::TenYears;
    let report = advance(&mut fast, &data);
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
    sim.speed = SpeedStep::TenYears;

    let report = advance(&mut sim, &data);
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
    data.config.campaign_skeleton.objective_beats.clear();
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
    data.config.campaign_skeleton.objective_beats.clear();
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
    sim.speed = SpeedStep::OneMonth;
    advance(&mut sim, &data);
    assert!(sim.ship.fuel < 1.0, "the first travel month burns fuel");

    // An operation month burns none.
    let (data, mut sim) = provisioned(5, 1.0);
    sim.contract.as_mut().unwrap().months_elapsed = 110 * 12; // end of Travel
    sim.speed = SpeedStep::OneMonth;
    advance(&mut sim, &data);
    assert_eq!(sim.ship.fuel, 1.0, "on-station months burn no fuel");
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
