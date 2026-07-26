//! What the drydock offers: the writs a ship's name, its peoples and its
//! guns open or close, and the arc that unlocks its next leg.

use super::*;

#[test]
fn the_writ_board_reflects_the_ships_reputation() {
    // Content-depth charters round 16: the board reads the ship's cumulative
    // character. The sanctuary run opens only to a hull famous for mercy; the
    // enforcement writ only to one known not to flinch — and neither is offered
    // to a ship whose name is still neutral.
    let data = GameData::load().unwrap();
    let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
    let hard = data.contracts.get("the_hard_contract").unwrap();
    let mercy_floor = sanctuary.min_reputation[0].threshold;
    let mercy_ceiling = hard.max_reputation[0].threshold;

    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 83, &picks);

    // A neutral name (0.5) opens neither door.
    assert!(
        !meets_in_world_gate(&sim, sanctuary) && !meets_in_world_gate(&sim, hard),
        "a ship with no reputation yet is offered neither"
    );

    // A merciful name opens the sanctuary run and keeps the hard writ shut.
    sim.reputation.insert("mercy".to_string(), mercy_floor);
    assert!(
        meets_in_world_gate(&sim, sanctuary),
        "a famous mercy is trusted with the vulnerable"
    );
    assert!(
        !meets_in_world_gate(&sim, hard),
        "a merciful ship is not offered the cold work"
    );

    // A feared name opens the enforcement writ and shuts the sanctuary run.
    sim.reputation.insert("mercy".to_string(), mercy_ceiling);
    assert!(
        meets_in_world_gate(&sim, hard),
        "a ship known not to flinch is hired for the hard thing"
    );
    assert!(
        !meets_in_world_gate(&sim, sanctuary),
        "and is not trusted with a people's children"
    );
}

#[test]
fn a_charter_arc_unlocks_its_next_leg_only_once_the_first_is_done() {
    // Content-depth charters round 14: a charter arc. The Karst Belt works are
    // offered only to a ship that has proven the veins (the survey's completion
    // mark) — and, being delicate high-trust work, only to a ship that has not
    // broken its word.
    let data = GameData::load().unwrap();
    let survey = data.contracts.get("deep_vein_survey").unwrap();
    let works = data.contracts.get("the_karst_works").unwrap();
    let seed = &survey.completion_consequence;
    assert!(!seed.is_empty(), "the survey seeds an arc on completion");
    assert_eq!(&works.requires_consequence, &vec![seed.clone()]);

    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 71, &picks);

    // A ship that has never surveyed the belt is not offered the works…
    assert!(
        !meets_in_world_gate(&sim, works),
        "the permanent works need the veins proven first"
    );
    // …but a ship that completed the survey is.
    sim.consequences.push(seed.clone());
    assert!(
        meets_in_world_gate(&sim, works),
        "proving the veins unlocks the works"
    );
    // …unless it has broken a bargain — the consortium won't trust it.
    sim.consequences.push("broke_a_bargain".to_string());
    assert!(
        !meets_in_world_gate(&sim, works),
        "a known oathbreaker is barred from the delicate works"
    );
}

#[test]
fn an_in_world_charter_is_offered_only_while_its_people_are_aboard() {
    // Content-depth charters round 12: the in-world availability gate. The
    // Seedbearers' Writ is offered only to a ship carrying the Verdant Kin —
    // it appears when they are aboard and vanishes if they leave, distinct
    // from the cross-campaign renown gate.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let template = data.contracts.get("the_seedbearers_writ").unwrap();
    assert_eq!(template.requires_faction_aboard, vec!["verdant_kin"]);

    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 47, &picks);

    // A ship without the Verdant Kin is not offered the writ…
    let fs = |id: &str| FactionState {
        faction_id: id.to_string(),
        members: 500,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    sim.factions = vec![fs("steel_covenant"), fs("hearth_union")];
    assert!(
        !meets_in_world_gate(&sim, template),
        "a ship without the gardeners is not trusted with the greening"
    );
    // …but a ship that carries them is.
    sim.factions.push(fs("verdant_kin"));
    assert!(
        meets_in_world_gate(&sim, template),
        "carrying the Verdant Kin unlocks the seedworld writ"
    );
    // A charter with no in-world gate is always offered.
    let ungated = data.contracts.get("founding_colony").unwrap();
    assert!(meets_in_world_gate(&sim, ungated));
}

#[test]
fn a_writ_that_needs_guns_is_barred_to_an_unarmed_hull() {
    // Content-depth charters round 26: the loadout availability gate. The Vane Default
    // enforcement writ demands real firepower; an unarmed hull cannot take it, though
    // an ungated founding writ always clears, and fitting a weapon opens it.
    let data = crate::data::GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 12, &picks);
    let hard = data.contracts.get("the_hard_contract").unwrap();
    let ordinary = data.contracts.get("deep_vein_survey").unwrap();
    assert!(hard.min_combat > 0, "the enforcement writ demands combat");

    // An unarmed hull (no weapon fitted): the writ is barred, the founding one open.
    assert!(sim.ship.weapon.is_none());
    assert!(
        !meets_loadout_gate(&sim, &data, hard),
        "an unarmed hull can't take the enforcement writ"
    );
    assert!(
        meets_loadout_gate(&sim, &data, ordinary),
        "a founding writ asks nothing of the loadout"
    );

    // Fit a real weapon: the writ opens.
    sim.ship.weapon = Some("pulse_cannon".to_string());
    assert!(
        meets_loadout_gate(&sim, &data, hard),
        "with the guns fitted, the enforcement writ opens"
    );
}

#[test]
fn a_double_hop_reads_a_different_line_on_its_second_departure() {
    let data = GameData::load().unwrap();
    let fl = &data.config.flavor;
    // The twin_survey re-enters Travel and Operation; the second entry must
    // not reprint the first entry's line (content-depth voice round 3).
    let first_travel = phase_transition_line(fl, ContractPhase::Travel, 1);
    let second_travel = phase_transition_line(fl, ContractPhase::Travel, 2);
    assert_ne!(
        first_travel, second_travel,
        "a double-hop's second departure should read differently"
    );
    // Out-of-range occurrences wrap rather than panic.
    let _ = phase_transition_line(fl, ContractPhase::Operation, 99);
}
