//! Consequence chains: a choice writes a tag, and a later event waits on
//! that tag - sometimes on two of them, sometimes on one never written.

use super::*;

#[test]
fn a_chain_payoff_waits_for_its_seeded_consequence() {
    // Content-depth event families round 21: closing the loops — a payoff event
    // stays out of the pool until the choice that seeds it is on record.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
    let payoff = data.events.get("the_unready_hour").unwrap();
    assert!(
        !passes_gate(&sim, payoff),
        "the unready hour stays out until a reign has run unprepared"
    );
    sim.consequences.push("unprepared_succession".to_owned());
    assert!(
        passes_gate(&sim, payoff),
        "once the consequence is on record, the reckoning can fire"
    );
}

#[test]
fn a_consequence_gate_holds_the_payoff_until_the_setup_choice_fires() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 5, &picks);
    // `the_ward_reopens` is the payoff half of the `sealed_ward` chain
    // (content-depth iteration): it may not fire until sealing the ward
    // recorded that consequence.
    let payoff = data.events.get("the_ward_reopens").unwrap();
    assert_eq!(payoff.requires_consequence, vec!["sealed_ward".to_string()]);
    sim.dynasty.generation = 5; // clear its min_generation gate

    assert!(
        !passes_gate(&sim, payoff),
        "the reopening stays out of the pool before the ward was ever sealed"
    );
    sim.consequences.push("sealed_ward".to_string());
    assert!(
        passes_gate(&sim, payoff),
        "sealing the ward unlocks the reopening decades later"
    );
}

#[test]
fn a_seeded_payoff_waits_for_its_consequence_on_record() {
    // Content-depth event families round 27: the payoffs that land this session's
    // seeded chains. The Drift People reckoning surfaces only for a ship that actually
    // settled into the becalming — its seed on record — and not before.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 62, &picks);
    sim.dynasty.generation = 4; // clear the min_generation gate
    let tmpl = data.events.get("the_drift_people").unwrap();

    assert!(
        !passes_gate(&sim, tmpl),
        "the drift-people reckoning waits until the drift was chosen"
    );
    sim.consequences.push("settled_into_the_drift".to_string());
    assert!(
        passes_gate(&sim, tmpl),
        "settling into the drift on record opens its payoff"
    );
}

#[test]
fn the_slow_pulse_chain_lands_only_once_it_was_chased() {
    // Content-depth event families round 28: the science_anomaly family's self-contained
    // chain. What-the-pulse-was (the beacon over a grave) surfaces only for a ship that
    // spent the season chasing the slow pulse — its seed on record — and not before.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 44, &picks);
    sim.dynasty.generation = 3; // clear the min_generation gate
    let payoff = data.events.get("what_the_pulse_was").unwrap();

    assert!(
        !passes_gate(&sim, payoff),
        "the pulse's answer waits until the ship chose to chase it"
    );
    sim.consequences.push("chased_the_slow_pulse".to_string());
    assert!(
        passes_gate(&sim, payoff),
        "having chased the slow pulse on record opens its answer"
    );

    // The seed's own event carries no gate — a fresh ship can meet the pulse at once.
    let seed = data.events.get("the_slow_pulse").unwrap();
    let fresh = SimState::new_campaign(&data, "preservers", 45, &picks);
    assert!(
        passes_gate(&fresh, seed),
        "the pulse itself is offered to any ship"
    );
}

#[test]
fn the_young_world_we_woke_comes_back_generations_later() {
    // Content-depth event families round 31: a chains round closing the exploration family's
    // two long-dangling first-contact seeds. The World We Woke surfaces only for a ship that
    // once revealed itself to a young world — its seed on record — and a few generations on.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 46, &picks);
    sim.dynasty.generation = 4; // clear the min_generation gate
    let payoff = data.events.get("the_world_we_woke").unwrap();

    assert!(
        !passes_gate(&sim, payoff),
        "the woken world's return waits until the ship actually touched one"
    );
    sim.consequences.push("touched_a_young_world".to_string());
    assert!(
        passes_gate(&sim, payoff),
        "having touched a young world on record opens its generational reckoning"
    );

    // …and the frontier-suspicion payoff likewise waits for its own guarded-answers seed.
    let frontier = data.events.get("the_word_passed_ahead").unwrap();
    sim.dynasty.generation = 3;
    assert!(
        !passes_gate(&sim, frontier),
        "the word-passed-ahead reckoning waits for the guarded answers that seeded it"
    );
    sim.consequences.push("frontier_suspicion".to_string());
    assert!(
        passes_gate(&sim, frontier),
        "a frontier soured by guarded answers brings its suspicion back around"
    );
}

#[test]
fn a_convergent_chain_needs_both_its_seeds_on_record() {
    // Content-depth event families round 24: a payoff gated on TWO seed consequences.
    // The Untethered reckons only for a ship that both let its founders go AND turned
    // its purpose inward — closing two chains at once, and proving the AND semantics
    // (one releasing is not enough).
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 55, &picks);
    let tmpl = data.events.get("the_untethered").unwrap();
    sim.dynasty.generation = 6; // clear the min_generation gate

    // Neither releasing on record: barred.
    assert!(
        !passes_gate(&sim, tmpl),
        "no releasing on record, no reckoning"
    );
    // Only one: still barred — this is an AND, not an OR.
    sim.consequences.push("founding_let_go".to_string());
    assert!(
        !passes_gate(&sim, tmpl),
        "one releasing alone is not enough"
    );
    // Both: the capstone opens.
    sim.consequences.push("purpose_turned_inward".to_string());
    assert!(
        passes_gate(&sim, tmpl),
        "both releasings on record open the untethered reckoning"
    );
}

#[test]
fn a_forbidden_consequence_closes_a_door_a_choice_slammed() {
    // Content-depth event families round 13: the negative gate. A generally
    // available opportunity is barred once a disqualifying history is on record
    // — trust never extended to a ship known to have broken its word — and a
    // multi-tag bar closes on *either* tag.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 57, &picks);

    // The stranger's trust is offered to a ship with a clean name…
    let trust = data.events.get("the_strangers_trust").unwrap();
    assert_eq!(
        trust.forbidden_consequence,
        vec!["broke_a_bargain".to_string()]
    );
    assert!(
        passes_gate(&sim, trust),
        "an unspoiled name is extended the stranger's trust"
    );
    // …and never again once the ship has broken a bargain.
    sim.consequences.push("broke_a_bargain".to_string());
    assert!(
        !passes_gate(&sim, trust),
        "a known oathbreaker is offered no trust"
    );

    // A multi-tag bar: the founders' vindication is closed by either a buried
    // record or a lost archive — you cannot revere a founding truth you hid or
    // forgot.
    let vindication = data.events.get("the_founders_vindicated").unwrap();
    assert!(vindication.forbidden_consequence.len() >= 2);
    let mut clean = SimState::new_campaign(&data, "preservers", 58, &picks);
    clean.dynasty.generation = 6;
    assert!(
        passes_gate(&clean, vindication),
        "an intact founding record can be vindicated"
    );
    clean.consequences.push("the_lost_archive".to_string());
    assert!(
        !passes_gate(&clean, vindication),
        "a ship that let its archive die cannot vindicate a founding it forgot"
    );
}

#[test]
fn the_triage_rule_pays_off_generations_after_it_is_written() {
    // Content-depth event-families round 5: a chain payoff completing a
    // formerly-dangling consequence. The cold triage rule (set by
    // `triage_rule`) re-fires as `the_rule_comes_due` only once that choice
    // was made — and its two ways out genuinely diverge (honor the cold law
    // vs break it, opposite morale/stability swings).
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 83, &picks);
    let payoff = data.events.get("the_rule_comes_due").unwrap();
    assert_eq!(
        payoff.requires_consequence,
        vec!["cold_triage_rule".to_string()]
    );
    sim.dynasty.generation = 4; // clear min_generation

    // Without the setup choice on record, the payoff stays out of the pool.
    assert!(
        !passes_gate(&sim, payoff),
        "the reckoning cannot fire before the cold rule was ever written"
    );
    // Writing the cold rule (the setup's consequence) unlocks it.
    sim.consequences.push("cold_triage_rule".to_string());
    assert!(passes_gate(&sim, payoff), "the written rule comes due");

    // The two resolutions move morale in opposite directions.
    let mut honor = sim.clone();
    let apply = payoff
        .outcomes
        .iter()
        .position(|o| o.id == "apply_the_rule")
        .unwrap();
    apply_outcome(&mut honor, &data, payoff, apply);
    let mut refuse = sim.clone();
    let brk = payoff
        .outcomes
        .iter()
        .position(|o| o.id == "break_the_rule")
        .unwrap();
    apply_outcome(&mut refuse, &data, payoff, brk);
    assert!(
        refuse.population.morale > honor.population.morale,
        "breaking the cold law lifts morale where honoring it costs it"
    );
}

#[test]
fn the_provisioners_debt_becomes_a_branching_generational_chain() {
    // Content-depth provisioning round 7: complete the dangling `owed_a_favor`
    // debt the fuel-bargain seeded. Generations on, the strangers collect
    // (`the_debt_called_in`); reneging seeds `broke_a_bargain`, which itself
    // re-fires as `the_marked_hull` a further stretch on — a real branching
    // arc, not a single flat payoff.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 51, &picks);

    let called_in = data.events.get("the_debt_called_in").unwrap();
    assert_eq!(
        called_in.requires_consequence,
        vec!["owed_a_favor".to_string()]
    );
    sim.dynasty.generation = 5; // clear min_generation

    // No debt on record → the collectors never come.
    assert!(
        !passes_gate(&sim, called_in),
        "no collector comes for a debt that was never taken"
    );
    sim.consequences.push("owed_a_favor".to_string());
    assert!(passes_gate(&sim, called_in), "the taken favor comes due");

    // Honoring the debt closes it clean and never marks the hull.
    let mut honor = sim.clone();
    let hon = called_in
        .outcomes
        .iter()
        .position(|o| o.id == "honor_the_debt")
        .unwrap();
    apply_outcome(&mut honor, &data, called_in, hon);
    assert!(
        !honor.consequences.contains(&"broke_a_bargain".to_string()),
        "keeping the founders' word does not brand the ship an oathbreaker"
    );

    // Reneging keeps resources but seeds the reputation consequence.
    let mut renege = sim.clone();
    let ren = called_in
        .outcomes
        .iter()
        .position(|o| o.id == "renege_the_debt")
        .unwrap();
    apply_outcome(&mut renege, &data, called_in, ren);
    assert!(
        renege.consequences.contains(&"broke_a_bargain".to_string()),
        "disowning the debt marks the hull"
    );

    // The mark re-fires generations later, and only for a ship that reneged.
    let marked = data.events.get("the_marked_hull").unwrap();
    assert_eq!(
        marked.requires_consequence,
        vec!["broke_a_bargain".to_string()]
    );
    renege.dynasty.generation = 7; // clear the marked hull's later gate
    assert!(
        passes_gate(&renege, marked),
        "the closed ports find the ship that broke its word"
    );
    honor.dynasty.generation = 7;
    assert!(
        !passes_gate(&honor, marked),
        "a ship that kept its word is never turned away"
    );
}
