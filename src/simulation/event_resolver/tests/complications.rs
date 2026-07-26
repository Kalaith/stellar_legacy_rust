//! Complications: the rider an event picks up when the ship is already in
//! trouble, and the extra toll it lands on the choice it names.

use super::*;

#[test]
fn a_worn_ship_complication_rides_only_when_its_state_holds() {
    // Content-depth event families round 20: a crisis reads and bites worse on a
    // ship the mortality/famine systems have worn down — here a fever turns
    // killer only when the infirmary that should break it is itself failing.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
    let fever = data.events.get("quiet_fever").unwrap();

    // A sound ward: no complication rides.
    if let Some(bay) = sim.subsystems.get_mut("medical_bay") {
        bay.condition = 0.8;
    }
    assert!(
        active_complication(&sim, fever).is_none(),
        "a working ward keeps the fever a nuisance"
    );
    // A failing ward: the killer twist rides.
    if let Some(bay) = sim.subsystems.get_mut("medical_bay") {
        bay.condition = 0.2;
    }
    assert_eq!(
        active_complication(&sim, fever).map(|c| c.id.as_str()),
        Some("no_ward_to_hold_it"),
        "a broken ward lets the fever turn deadly"
    );
}

#[test]
fn a_faction_colored_complication_rides_only_under_its_faction() {
    // Content-depth factions round 6: the same crisis reads differently
    // depending on who runs the ship. micrometeoroid_storm gains a First
    // Flame reaction (a trial of faith) only while the Keepers are dominant.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 88, &picks);
    let template = data.events.get("micrometeoroid_storm").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.requires_dominant_faction == "first_flame")
        .expect("the storm carries a First Flame reaction");
    assert!(
        sim.is_faction_aboard("first_flame"),
        "seed campaign holds the Flame"
    );

    // Someone else dominant: the faction reaction stays out.
    for f in &mut sim.factions {
        f.members = if f.faction_id == "first_flame" {
            50
        } else {
            900
        };
    }
    assert_ne!(sim.dominant_faction_id(), Some("first_flame"));
    assert!(active_complication(&sim, template).is_none());

    // The Keepers running the ship: the reaction rides and shows.
    for f in &mut sim.factions {
        f.members = if f.faction_id == "first_flame" {
            900
        } else {
            50
        };
    }
    assert_eq!(sim.dominant_faction_id(), Some("first_flame"));
    assert_eq!(
        active_complication(&sim, template).map(|c| &c.id),
        Some(&comp.id)
    );
    assert!(shown_description(&sim, template).contains("Keepers"));
}

#[test]
fn a_soft_ship_complication_rides_only_after_a_long_plenty() {
    // Content-depth event families round 23: the abundance twin of the lean-years
    // complication. Micrometeoroid Storm gains a twist that rides only on a crew
    // grown soft over many fat years — unpractised at real danger.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 41, &picks);
    let template = data.events.get("micrometeoroid_storm").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.min_fat_food_years > 0)
        .expect("the storm carries a soft-generation reaction");

    // Hold the Steel Covenant dominant so the First Flame reaction (the other
    // complication) never rides — this isolates the fat-years gate.
    sim.factions = vec![FactionState {
        faction_id: "steel_covenant".to_string(),
        members: sim.population.count,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    }];

    // Short of a long plenty: the soft-generation twist stays out.
    sim.fat_food_years = comp.min_fat_food_years - 1;
    assert!(active_complication(&sim, template).is_none());

    // Once the plenty has held long enough: it rides and shows.
    sim.fat_food_years = comp.min_fat_food_years;
    assert_eq!(
        active_complication(&sim, template).map(|c| &c.id),
        Some(&comp.id)
    );
    assert!(shown_description(&sim, template).contains("easy years"));
}

#[test]
fn a_drift_gated_comedy_complication_rides_only_on_a_drifted_ship() {
    // Content-depth event families round 29: comedy's first complications. The Festival
    // War gains a twist that rides only once the crew's culture has drifted far enough that
    // the two decks' festivals are no longer recognizably the same rite — the comedy of a
    // ship quietly becoming more than one people.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 29, &picks);
    let template = data.events.get("the_festival_war").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.min_cultural_drift > 0.0)
        .expect("the festival war carries a drift reaction");

    // A crew still close to the founders: the twist stays out.
    sim.population.cultural_drift = comp.min_cultural_drift - 0.1;
    assert!(active_complication(&sim, template).is_none());

    // A drifted crew: it rides, and the two-peoples weight shows in the modal.
    sim.population.cultural_drift = comp.min_cultural_drift + 0.05;
    assert_eq!(
        active_complication(&sim, template).map(|c| &c.id),
        Some(&comp.id)
    );
    assert!(shown_description(&sim, template).contains("two peoples"));
}

#[test]
fn a_drifted_crew_reads_a_mystery_as_a_taboo() {
    // Content-depth event families round 32: mystery's first real depth. The Sealed Deck gains
    // a twist that rides only once the crew's culture has drifted far enough that the
    // unexplained door has calcified from a curiosity into a genuine taboo — the eerie read as
    // the sacred.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 32, &picks);
    sim.dynasty.generation = 4; // clear the event's own min_generation
    let template = data.events.get("the_sealed_deck").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.min_cultural_drift > 0.0)
        .expect("the sealed deck carries a drift reaction");

    // A crew still close to the founders' matter-of-factness: the taboo stays out.
    sim.population.cultural_drift = comp.min_cultural_drift - 0.1;
    assert!(active_complication(&sim, template).is_none());

    // A drifted crew: the not-knowing has become faith, and the twist rides.
    sim.population.cultural_drift = comp.min_cultural_drift + 0.05;
    assert_eq!(
        active_complication(&sim, template).map(|c| &c.id),
        Some(&comp.id)
    );
    assert!(shown_description(&sim, template).contains("sacred"));
}

#[test]
fn a_shipborn_crew_grieves_a_world_it_can_no_longer_walk() {
    // Content-depth event families round 30: exploration_first_contact's first complications,
    // reactive to the ship's *identity*. The Young World gains a twist that rides only once
    // the crew has grown so shipborn (adaptation past the divergence line) that the very
    // world the voyage was launched to find is one they could no longer live on — a grief
    // only a diverged crew can feel. This also exercises the new adaptation_above complication
    // gate.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 30, &picks);
    sim.dynasty.generation = 2; // clear the event's own min_generation
    let template = data.events.get("the_young_world").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.adaptation_above.is_some())
        .expect("the young world carries a shipborn-grief reaction");
    let line = comp.adaptation_above.unwrap();

    // A still-planet-capable crew: the grief stays out.
    sim.population.adaptation = line - 0.1;
    assert!(active_complication(&sim, template).is_none());

    // A fully shipborn crew: it rides, and the "no longer walk" grief shows.
    sim.population.adaptation = line + 0.05;
    assert_eq!(
        active_complication(&sim, template).map(|c| &c.id),
        Some(&comp.id)
    );
    assert!(shown_description(&sim, template).contains("no longer able to live"));
}

#[test]
fn a_reputation_gated_complication_rides_only_on_a_ship_of_that_name() {
    // Content-depth event families round 22: the same crisis reads differently by
    // the *name* the ship has earned. The Petitioners gains a twist that rides only
    // on a famously merciful hull — the desperate steered for its mercy by name.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 91, &picks);
    let template = data.events.get("asylum_request").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.min_reputation.iter().any(|g| g.id == "mercy"))
        .expect("the petitioners carry a merciful-name reaction");

    // A neutral (0.5) name: the twist stays out.
    assert!(sim.reputation("mercy") < 0.62);
    assert!(active_complication(&sim, template).is_none());

    // A famously merciful name: the twist rides and shows.
    sim.adjust_reputation("mercy", 0.2);
    assert!(sim.reputation("mercy") >= 0.62);
    assert_eq!(
        active_complication(&sim, template).map(|c| &c.id),
        Some(&comp.id)
    );
    assert!(shown_description(&sim, template).contains("kind"));
}

#[test]
fn an_event_with_two_complications_rides_the_first_that_matches() {
    // Content-depth event families round 7: the doc's "2-3 complications is
    // worth three flat events." system_failure now carries two — a failing
    // engineering bay (first) and a Steel Covenant reaction (second). The
    // first whose gates hold rides, so a worn bay wins even when the Covenant
    // is in charge, and the Covenant's is what shows on a sound ship.
    let data = GameData::load().unwrap();
    let picks = vec![
        "steel_covenant".to_string(),
        "hearth_union".to_string(),
        "meridian_accord".to_string(),
    ];
    let template = data.events.get("system_failure").unwrap();
    assert_eq!(template.complications.len(), 2);

    // Steel Covenant running a sound ship: their reaction rides.
    let mut covenant = SimState::new_campaign(&data, "adaptors", 71, &picks);
    for f in &mut covenant.factions {
        f.members = if f.faction_id == "steel_covenant" {
            900
        } else {
            50
        };
    }
    covenant
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .condition = 0.9;
    assert_eq!(
        active_complication(&covenant, template).map(|c| c.id.as_str()),
        Some("covenant_takes_it_in_hand")
    );

    // Same ship, but the bay is failing: the earlier complication wins.
    let mut failing = covenant.clone();
    failing
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .condition = 0.2;
    assert_eq!(
        active_complication(&failing, template).map(|c| c.id.as_str()),
        Some("bay_already_failing"),
        "the first matching complication takes precedence"
    );
}

#[test]
fn a_complication_reads_the_ships_thinned_and_hungry_state() {
    // Content-depth event families round 15: complications now read the new
    // lived-state dimensions. The failing air's twist rides only on a thinned
    // crew; the ration triage's rides only on a ship worn by years of hunger.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);

    // A thinned-crew twist on the failing air.
    let air = data.events.get("the_failing_air").unwrap();
    let thin = air
        .complications
        .iter()
        .find(|c| c.max_population > 0)
        .expect("the failing air carries a thinned-crew complication");
    let mut sim = SimState::new_campaign(&data, "preservers", 75, &picks);
    sim.population.count = thin.max_population + 100;
    assert!(
        active_complication(&sim, air).is_none(),
        "a full crew answers the failing air on every deck"
    );
    sim.population.count = thin.max_population;
    assert!(
        active_complication(&sim, air).is_some_and(|c| c.id == thin.id),
        "a skeleton crew cannot, and the twist rides"
    );

    // A chronic-hunger twist on the ration triage.
    let table = data.events.get("the_thin_table").unwrap();
    let worn = table
        .complications
        .iter()
        .find(|c| c.min_lean_food_years > 0)
        .expect("the thin table carries a chronic-hunger complication");
    let mut sim = SimState::new_campaign(&data, "preservers", 76, &picks);
    // Meet the event's own food gate so the complication is what we isolate.
    sim.resources.food = table.food_below.unwrap() - 1;
    sim.lean_food_years = worn.min_lean_food_years - 1;
    assert!(
        active_complication(&sim, table).is_none(),
        "a ship only lately hungry still has fat to cut"
    );
    sim.lean_food_years = worn.min_lean_food_years;
    assert!(
        active_complication(&sim, table).is_some_and(|c| c.id == worn.id),
        "a ship worn by years of want has nothing left, and the twist rides"
    );
}

#[test]
fn a_choice_targeting_complication_punishes_only_the_choice_it_names() {
    // Content-depth event families round 14: outcome-conditional complications.
    // The hull fracture's deferral twist rides on a ship that already puts work
    // off, but its extra toll lands only on the choice to defer *again* — fixing
    // the crack (or paying for a proper repair) escapes it.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let event = data.events.get("hull_fracture").unwrap();
    let comp = event
        .complications
        .iter()
        .find(|c| c.applies_to_outcomes.iter().any(|o| o == "monitor_it"))
        .expect("hull_fracture carries a choice-targeting complication");
    let defer = event
        .outcomes
        .iter()
        .position(|o| o.id == "monitor_it")
        .unwrap();
    let fix = event
        .outcomes
        .iter()
        .position(|o| o.id == "reinforce_now")
        .unwrap();

    // The twist rides only on a ship that already carries deferred work.
    let mut deferring = SimState::new_campaign(&data, "preservers", 67, &picks);
    deferring
        .consequences
        .push("deferred_maintenance".to_string());
    assert!(
        active_complication(&deferring, event).is_some_and(|c| c.id == comp.id),
        "the deferral twist rides on a ship that already defers"
    );

    // Hull change from applying an outcome, with or without the deferral history.
    let hull_delta = |outcome: usize, deferred: bool| -> f32 {
        let mut sim = SimState::new_campaign(&data, "preservers", 67, &picks);
        sim.resources.minerals = 100_000; // afford the reinforce
        if deferred {
            sim.consequences.push("deferred_maintenance".to_string());
        }
        let h0 = sim.ship.hull_integrity;
        apply_outcome(&mut sim, &data, event, outcome);
        sim.ship.hull_integrity - h0
    };

    // Deferring *again* on a deferring ship costs extra hull; fixing it does not.
    assert!(
        hull_delta(defer, true) < hull_delta(defer, false),
        "deferring again eats the complication's extra toll"
    );
    assert!(
        (hull_delta(fix, true) - hull_delta(fix, false)).abs() < 1e-6,
        "fixing the crack is untouched — the twist targets only the defer choice"
    );
}

#[test]
fn a_complication_rides_only_when_its_state_gate_holds_and_lands_extra_toll() {
    // Content-depth event families round 6: system_failure carries a
    // complication that rides only while the engineering bay is itself
    // failing. When it rides it (a) shows in the description and (b) lands an
    // extra toll on top of whichever outcome was taken.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let template = data.events.get("system_failure").unwrap();
    assert!(!template.complications.is_empty());

    // Sound bay: no complication rides; the description is the plain one.
    let mut sound = SimState::new_campaign(&data, "adaptors", 51, &picks);
    sound
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .condition = 0.9;
    assert!(active_complication(&sound, template).is_none());
    assert_eq!(shown_description(&sound, template), template.description);

    // Failing bay: the complication rides, and its twist joins the shown text.
    let mut failing = SimState::new_campaign(&data, "adaptors", 51, &picks);
    failing
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .condition = 0.2;
    assert!(active_complication(&failing, template).is_some());
    assert!(shown_description(&failing, template).len() > template.description.len());

    // Same outcome, two states: the complicated run takes the heavier hull hit.
    let hull_of = |mut sim: SimState| {
        apply_outcome(&mut sim, &data, template, 0); // emergency_repair
        sim.ship.hull_integrity
    };
    let (a, b) = (sound.clone(), failing.clone());
    assert!(
        hull_of(b) < hull_of(a),
        "the complication lands an extra toll the flat event does not"
    );
}
