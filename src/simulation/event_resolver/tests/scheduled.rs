//! Debts with a date on them: an event that schedules its own return and
//! arrives on the appointed year, not before.

use super::*;

#[test]
fn a_mortgaged_bond_comes_due_on_the_named_clock() {
    // Content-depth event families round 25: a *timed* chain (distinct from the
    // state-based requires_consequence ones). Taking the waystation's bond schedules
    // the collectors' return to the year; declining schedules nothing, and the payoff
    // is scheduled_only so it never rolls on its own.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 44, &picks);

    let seed = data.events.get("the_mortgaged_passage").unwrap();
    let payoff = data.events.get("the_collectors_return").unwrap();
    assert!(
        payoff.scheduled_only,
        "the collectors' return must never roll on its own"
    );
    let take = seed
        .outcomes
        .iter()
        .position(|o| o.id == "take_the_bond")
        .unwrap();
    let delay = seed.outcomes[take]
        .schedule_followup
        .as_ref()
        .expect("taking the bond schedules the collectors")
        .delay_years;

    // Taking the bond queues the debt for the year it named.
    let year0 = sim.year();
    apply_outcome(&mut sim, &data, seed, take);
    assert_eq!(
        sim.scheduled_events.len(),
        1,
        "the bond queues its reckoning"
    );
    assert_eq!(sim.scheduled_events[0].template_id, "the_collectors_return");
    assert_eq!(
        sim.scheduled_events[0].fire_year,
        year0 + delay,
        "the debt comes due on the clock the waystation named"
    );

    // Declining the bond on a fresh ship schedules nothing.
    let mut clean = SimState::new_campaign(&data, "preservers", 45, &picks);
    let decline = seed
        .outcomes
        .iter()
        .position(|o| o.id == "decline_the_bond")
        .unwrap();
    apply_outcome(&mut clean, &data, seed, decline);
    assert!(
        clean.scheduled_events.is_empty(),
        "declining the bond leaves no debt on the clock"
    );
}

#[test]
fn the_ghost_signal_schedules_its_own_appointed_hour() {
    // Content-depth event families round 10: the predestination loop, closed
    // with the round-9 scheduling. Answering the ghost signal — the ship's own
    // call sign timestamped for a future year — schedules that year's reckoning,
    // and the payoff is scheduled_only so it fires only when its date arrives.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "wanderers", 41, &picks);
    sim.dynasty.generation = 2; // an age past the ghost's own drift complication is irrelevant here

    let ghost = data.events.get("ghost_signal").unwrap();
    let payoff = data.events.get("the_appointed_signal").unwrap();
    assert!(
        payoff.scheduled_only,
        "the appointed signal must never roll on its own"
    );
    let answer = ghost
        .outcomes
        .iter()
        .position(|o| o.id == "answer_the_ghost")
        .unwrap();
    let delay = ghost.outcomes[answer]
        .schedule_followup
        .as_ref()
        .expect("answering the ghost schedules its return")
        .delay_years;

    let year0 = sim.year();
    apply_outcome(&mut sim, &data, ghost, answer);
    assert_eq!(sim.scheduled_events.len(), 1, "answering queues the payoff");
    assert_eq!(sim.scheduled_events[0].template_id, "the_appointed_signal");
    assert_eq!(
        sim.scheduled_events[0].fire_year,
        year0 + delay,
        "the loop is set for the year the signal named"
    );
}

#[test]
fn deferred_maintenance_comes_due_a_generation_on() {
    // Content-depth event families round 10: completing a dangling thread. The
    // "defer the fix" outcomes of three engineering crises recorded a debt no
    // event ever collected; now it comes due a generation later.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 8, &picks);
    let bill = data.events.get("the_bill_comes_due").unwrap();
    assert_eq!(
        bill.requires_consequence,
        vec!["deferred_maintenance".to_string()]
    );
    sim.dynasty.generation = 5; // clear its min_generation

    assert!(
        !passes_gate(&sim, bill),
        "no reckoning for a ship that never deferred"
    );
    sim.consequences.push("deferred_maintenance".to_string());
    assert!(
        passes_gate(&sim, bill),
        "the deferred ledger comes due once it is on record"
    );
}

#[test]
fn a_charted_dearth_arrives_on_its_date_softened_only_if_provisioned() {
    // Content-depth provisioning round 10: foresight on a determined clock.
    // Charting the dearth schedules its guaranteed arrival; laying in stores
    // seeds the consequence the payoff's complication reads to soften it; the
    // payoff itself is scheduled-only and never rolls on its own.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 33, &picks);

    let setup = data.events.get("the_charted_dearth").unwrap();
    let payoff = data.events.get("the_dearth_arrives").unwrap();
    assert!(
        payoff.scheduled_only,
        "the dearth fires only when its charted year comes"
    );
    // Its relief complication rides on the laid-in-stores consequence.
    let comp = payoff
        .complications
        .iter()
        .find(|c| {
            c.requires_consequence
                .contains(&"laid_in_for_dearth".to_string())
        })
        .expect("a relief complication for the provisioned ship");

    // Laying in stores queues the dearth *and* records the foresight.
    let lay_in = setup
        .outcomes
        .iter()
        .position(|o| o.id == "lay_in_stores")
        .unwrap();
    let delay = setup.outcomes[lay_in]
        .schedule_followup
        .as_ref()
        .unwrap()
        .delay_years;
    let year0 = sim.year();
    apply_outcome(&mut sim, &data, setup, lay_in);
    assert_eq!(sim.scheduled_events[0].template_id, "the_dearth_arrives");
    assert_eq!(sim.scheduled_events[0].fire_year, year0 + delay);
    assert!(
        sim.consequences.contains(&"laid_in_for_dearth".to_string()),
        "laying in is on record for the complication to find"
    );

    // With the foresight on record, the relief complication rides the payoff.
    assert!(
        active_complication(&sim, payoff).is_some_and(|c| c.id == comp.id),
        "the laid-in stores answer the dearth"
    );

    // A ship that trusted to slack has no such relief.
    let mut unready = SimState::new_campaign(&data, "preservers", 33, &picks);
    assert!(
        active_complication(&unready, payoff).is_none(),
        "an unprovisioned ship meets the dearth bare"
    );
    // And trusting the slack still schedules the (unsoftened) dearth.
    let trust = setup
        .outcomes
        .iter()
        .position(|o| o.id == "trust_the_slack")
        .unwrap();
    apply_outcome(&mut unready, &data, setup, trust);
    assert_eq!(sim.scheduled_events.len(), 1);
    assert!(
        !unready
            .consequences
            .contains(&"laid_in_for_dearth".to_string()),
        "trusting the slack lays in nothing"
    );
}
