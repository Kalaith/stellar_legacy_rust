//! Beats with an author behind them: an arc that plays in order, a charter
//! firing on its appointed year, and the backstop against dead air.

use super::*;

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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();
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
    data.config.campaign_skeleton.despair_beats.clear();
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
