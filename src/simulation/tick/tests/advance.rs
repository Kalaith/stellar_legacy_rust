//! The advance loop itself: determinism, where a fast-forward must stop,
//! and a charter running to its appointed end.

use super::*;

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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();

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
