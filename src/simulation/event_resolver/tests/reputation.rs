//! The name the ship earns opens and closes doors: reputation gates on
//! outcomes, and the two faces a long campaign builds.

use super::*;

#[test]
fn reputation_unlocks_the_choice_a_name_earns() {
    // Content-depth event families round 17: reputation-gated outcomes. In a
    // wary encounter, a merciful ship can trade on its good name and a feared
    // ship can let its name intimidate — options a no-name ship simply lacks,
    // while the base choices (withdraw / deal hard) are always there.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let event = data.events.get("the_wary_encounter").unwrap();
    let good = event
        .outcomes
        .iter()
        .position(|o| o.id == "trade_on_our_good_name")
        .unwrap();
    let feared = event
        .outcomes
        .iter()
        .position(|o| o.id == "let_our_name_intimidate")
        .unwrap();
    assert!(good > 0 && feared > 0, "the base choices come first");

    let mut sim = SimState::new_campaign(&data, "preservers", 85, &picks);

    // A no-name ship: only the base options (withdraw / deal), neither leverage.
    let neutral = available_outcome_indices(&sim, event);
    assert!(
        neutral.contains(&0) && !neutral.contains(&good) && !neutral.contains(&feared),
        "an unknown ship has no name to trade on"
    );

    // A merciful name unlocks the good-name option, not the intimidation.
    sim.reputation.insert("mercy".to_string(), 0.7);
    let kind = available_outcome_indices(&sim, event);
    assert!(
        kind.contains(&good) && !kind.contains(&feared),
        "a merciful ship trades on its good name, it does not intimidate"
    );

    // A feared name unlocks the intimidation, not the good-name option.
    sim.reputation.insert("mercy".to_string(), 0.2);
    let cold = available_outcome_indices(&sim, event);
    assert!(
        cold.contains(&feared) && !cold.contains(&good),
        "a feared ship lets its name do the talking"
    );
}

#[test]
fn a_gated_outcome_is_offered_only_to_a_ship_that_earned_it() {
    // Content-depth event families round 12: state-gated outcomes. A crisis
    // offers a better exit only to a prepared ship — a fix a kept-expert bay
    // can attempt, a repair a banked reserve can buy — while the base choices
    // always show and the auto-resolve index-0 contract is untouched.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 43, &picks);

    // A knowledge floor: the coolant breach's master cooldown appears only
    // while the engineering bay's expertise is kept high.
    let breach = data.events.get("coolant_breach").unwrap();
    let master = breach
        .outcomes
        .iter()
        .position(|o| o.id == "master_controlled_cooldown")
        .unwrap();
    assert!(
        master > 0,
        "the gated outcome is authored after the base ones"
    );
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.4;
    assert!(
        !available_outcome_indices(&sim, breach).contains(&master),
        "a bay that has lost its masters cannot offer the master fix"
    );
    // Base outcomes are always on the table.
    assert!(available_outcome_indices(&sim, breach).contains(&0));
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.8;
    assert!(
        available_outcome_indices(&sim, breach).contains(&master),
        "expertise kept sharp unlocks the clean fix"
    );
    // …and it resolves by its real index like any outcome.
    apply_outcome(&mut sim, &data, breach, master);

    // A consequence gate: the hull fracture's shipyard repair appears only for
    // a ship that banked the war chest (ties back to the-full-coffers, it75).
    let fracture = data.events.get("hull_fracture").unwrap();
    let repair = fracture
        .outcomes
        .iter()
        .position(|o| o.id == "draw_on_the_war_chest")
        .unwrap();
    assert!(
        !available_outcome_indices(&sim, fracture).contains(&repair),
        "a ship with no reserve cannot draw on one"
    );
    sim.consequences.push("war_chest".to_string());
    assert!(
        available_outcome_indices(&sim, fracture).contains(&repair),
        "the banked reserve unlocks the proper repair years later"
    );
}

#[test]
fn a_reputation_builds_across_choices_and_opens_or_closes_doors() {
    // Content-depth event families round 16: graded reputation. Merciful choices
    // lift the ship's `mercy` trait, and a scenario that a merciful name opens
    // (the reputation precedes us) stays out of reach until enough of them add
    // up — while a feared one's scenario opens the opposite door.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 81, &picks);

    // A fresh ship reads neutral, and neither reputation door is open.
    assert_eq!(
        sim.reputation("mercy"),
        0.5,
        "an untouched trait is neutral"
    );
    let kind = data.events.get("the_reputation_precedes_us").unwrap();
    let feared = data.events.get("the_feared_name").unwrap();
    assert!(!passes_gate(&sim, kind), "no name yet, no merciful door");
    assert!(!passes_gate(&sim, feared), "and no feared door either");

    // Take castaways aboard, share the thin table: mercy builds.
    let castaways = data.events.get("the_castaways").unwrap();
    let aboard = castaways
        .outcomes
        .iter()
        .position(|o| o.id == "take_them_aboard")
        .unwrap();
    for _ in 0..3 {
        apply_outcome(&mut sim, &data, castaways, aboard);
    }
    assert!(
        sim.reputation("mercy") > 0.5,
        "merciful choices build a merciful name"
    );
    assert!(
        passes_gate(&sim, kind),
        "a name for mercy opens the door only trust extends"
    );

    // A ship that instead built ruthlessness opens the other door.
    let mut cold = SimState::new_campaign(&data, "preservers", 82, &picks);
    let stores = castaways
        .outcomes
        .iter()
        .position(|o| o.id == "take_the_stores_only")
        .unwrap();
    for _ in 0..5 {
        apply_outcome(&mut cold, &data, castaways, stores);
    }
    assert!(
        cold.reputation("mercy") < 0.3,
        "cold choices earn a cold name"
    );
    assert!(
        passes_gate(&cold, feared) && !passes_gate(&cold, kind),
        "a feared name opens the wary door and closes the merciful one"
    );
}

#[test]
fn the_ship_has_a_second_face_its_resolve() {
    // Content-depth event families round 18: the graded-character system gets a
    // second trait. `resolve` — the ship's name for steadfastness, seeing things
    // through and holding its nerve — is built and read entirely through event
    // choices, and is orthogonal to `mercy`: holding a line builds resolve without
    // touching mercy, and a resolute name opens a door a yielding one cannot.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 83, &picks);

    // A fresh ship reads neutral on the new trait, and the steadfast door is shut.
    assert_eq!(
        sim.reputation("resolve"),
        0.5,
        "an untouched trait is neutral"
    );
    let unblinking = data.events.get("the_unblinking_ship").unwrap();
    let folds = data.events.get("the_ship_that_folds").unwrap();
    assert!(
        !passes_gate(&sim, unblinking),
        "no name for nerve yet, no door"
    );
    assert!(
        folds.max_reputation.iter().any(|g| g.id == "resolve"),
        "the yielding-name door reads the same second trait"
    );

    // Hold the line, again and again: resolve builds — and mercy does not move.
    let standoff = data.events.get("the_line_in_the_dark").unwrap();
    let hold = standoff
        .outcomes
        .iter()
        .position(|o| o.id == "hold_the_line")
        .unwrap();
    let mercy_before = sim.reputation("mercy");
    for _ in 0..3 {
        apply_outcome(&mut sim, &data, standoff, hold);
    }
    assert!(
        sim.reputation("resolve") > 0.62,
        "holding the line builds a name for nerve"
    );
    assert_eq!(
        sim.reputation("mercy"),
        mercy_before,
        "resolve is its own axis — building it leaves mercy untouched"
    );
    assert!(
        passes_gate(&sim, unblinking),
        "a name for nerve opens a door a softer ship can't reach"
    );

    // A ship that instead yields builds the opposite name.
    let mut soft = SimState::new_campaign(&data, "preservers", 84, &picks);
    let give = standoff
        .outcomes
        .iter()
        .position(|o| o.id == "yield_the_ground")
        .unwrap();
    for _ in 0..3 {
        apply_outcome(&mut soft, &data, standoff, give);
    }
    assert!(
        soft.reputation("resolve") < 0.38,
        "yielding the ground earns a name for folding"
    );
}

#[test]
fn a_famines_options_turn_on_the_ships_reputation() {
    // Content-depth provisioning round 16: reputation as a survival factor. In a
    // famine, a merciful ship's name brings aid unbidden; a feared ship's finds
    // every door closed; a neutral ship faces the ordinary famine, neither.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let aid = data.events.get("the_kindness_returned").unwrap();
    let alone = data.events.get("the_famine_faced_alone").unwrap();
    let famine = aid.food_below.expect("the aid gates on a famine");

    let mut sim = SimState::new_campaign(&data, "preservers", 91, &picks);
    sim.resources.food = famine - 1; // a real famine
    sim.dynasty.generation = 5; // past the feared-alone gate's min_generation

    // A neutral name: neither reputation-conditioned famine surfaces.
    assert!(
        !passes_gate(&sim, aid) && !passes_gate(&sim, alone),
        "an unknown ship faces its famine on ordinary terms"
    );
    // A merciful name in a famine: aid comes, and the feared version stays shut.
    sim.reputation.insert("mercy".to_string(), 0.7);
    assert!(
        passes_gate(&sim, aid) && !passes_gate(&sim, alone),
        "a merciful name is helped, not shunned"
    );
    // A feared name in a famine: the doors close, and no aid comes.
    sim.reputation.insert("mercy".to_string(), 0.2);
    assert!(
        passes_gate(&sim, alone) && !passes_gate(&sim, aid),
        "a feared name faces its famine alone"
    );
    // But only *in* a famine — a fed feared ship faces neither.
    sim.resources.food = famine + 5000;
    assert!(
        !passes_gate(&sim, alone),
        "reputation only bites where the larder is already thin"
    );
}
