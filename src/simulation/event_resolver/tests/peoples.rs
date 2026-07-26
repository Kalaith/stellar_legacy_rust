//! Who runs the ship colors what it meets: faction gates on templates,
//! on complications, and on the choices only one people will take.

use super::*;

#[test]
fn a_faction_approval_floor_gates_a_gift_only_to_a_delighted_people() {
    // Content-depth factions round 19: the positive mirror of the grievance
    // gate — a gift/volunteered-effort beat surfaces only while the named
    // people is aboard and genuinely warm to the ship.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
    let feast = data.events.get("the_hearths_feast").unwrap();
    assert!(
        sim.is_faction_aboard("hearth_union"),
        "the founding set carries the Hearth"
    );

    // Merely content (launch approval 0.5): no feast is offered.
    assert!(
        !passes_gate(&sim, feast),
        "a merely-content people opens no tables"
    );
    // Delighted: the gift beat enters the pool.
    for faction in &mut sim.factions {
        if faction.faction_id == "hearth_union" {
            faction.approval = 0.9;
        }
    }
    assert!(
        passes_gate(&sim, feast),
        "a delighted people offers its feast"
    );
}

#[test]
fn faction_approval_gates_a_slighted_peoples_withdrawal() {
    // Content-depth factions round 8: the reserved approval mechanic. Event
    // choices earn or spend a people's goodwill, and a faction slighted past
    // a threshold generates its own withdrawal — so *how you treat a people*,
    // not only how far the voyage has drifted, decides whether it stays.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 19, &picks);
    sim.dynasty.generation = 4; // clear the withdrawal's min_generation

    // Ensure the First Flame is aboard at the launch midpoint.
    if sim.factions.iter().all(|f| f.faction_id != "first_flame") {
        sim.factions.push(FactionState {
            faction_id: "first_flame".to_string(),
            members: 300,
            status: FactionStatus::Aboard,
            approval: crate::state::sim::factions::default_approval(),
            mood_band: 0,
        });
        sim.population.count += 300;
    }
    let flame_approval = |sim: &SimState| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == "first_flame")
            .map(|f| f.approval)
    };
    assert_eq!(flame_approval(&sim), Some(0.5), "a people launches neutral");

    let petition = data.events.get("the_flame_petition").unwrap();
    let withdrawal = data.events.get("the_flame_withdrawal").unwrap();

    // The grievance fires whenever the Flame is aboard; the withdrawal waits
    // until they have actually soured.
    assert!(
        passes_gate(&sim, petition),
        "the Keepers can always petition"
    );
    assert!(
        !passes_gate(&sim, withdrawal),
        "a content people does not withdraw"
    );

    // Slight them once — approval drops but not yet past the threshold.
    let slight = petition
        .outcomes
        .iter()
        .position(|o| o.id == "hold_the_line")
        .unwrap();
    apply_outcome(&mut sim, &data, petition, slight);
    assert!(
        flame_approval(&sim).unwrap() < 0.5,
        "the slight is remembered"
    );
    assert!(
        !passes_gate(&sim, withdrawal),
        "one slight is not yet a departure"
    );

    // Slight them again — now they have soured past the threshold and the
    // withdrawal enters the pool.
    apply_outcome(&mut sim, &data, petition, slight);
    assert!(
        passes_gate(&sim, withdrawal),
        "a people slighted past the threshold moves to leave"
    );

    // Paying to keep them lifts approval back above the line and closes the
    // withdrawal (the loop can recover).
    let mut kept = sim.clone();
    let beg = withdrawal
        .outcomes
        .iter()
        .position(|o| o.id == "beg_them_stay")
        .unwrap();
    apply_outcome(&mut kept, &data, withdrawal, beg);
    assert!(kept.is_faction_aboard("first_flame"), "bought back aboard");
    assert!(
        !passes_gate(&kept, withdrawal),
        "goodwill restored closes the withdrawal"
    );

    // Or letting them go actually sheds the people.
    let go = withdrawal
        .outcomes
        .iter()
        .position(|o| o.id == "let_them_go")
        .unwrap();
    apply_outcome(&mut sim, &data, withdrawal, go);
    assert!(
        !sim.is_faction_aboard("first_flame"),
        "the slighted people departs"
    );
}

#[test]
fn an_assimilation_beat_folds_a_people_in_without_losing_them() {
    // Content-depth factions round 5: the union counterpart to a schism. The
    // merger dissolves the named faction's separate identity but keeps its
    // people aboard — head count untouched, its members folded into the host
    // — where a fracture would have dropped them off the ship entirely.
    let data = GameData::load().unwrap();
    let picks = vec![
        "hearth_union".to_string(),
        "verdant_kin".to_string(),
        "meridian_accord".to_string(),
    ];
    let mut sim = SimState::new_campaign(&data, "preservers", 55, &picks);
    sim.dynasty.generation = 6;
    sim.population.cultural_drift = 0.5;

    let event = data.events.get("the_green_hearth").unwrap();
    assert!(passes_gate(&sim, event), "the union fires with both aboard");
    let bless = event
        .outcomes
        .iter()
        .position(|o| o.faction_merge_id.as_deref() == Some("verdant_kin"))
        .expect("the green hearth can bless the union");

    let heads_before = sim.population.count;
    let kin_members = sim
        .factions
        .iter()
        .find(|f| f.faction_id == "verdant_kin")
        .map(|f| f.members)
        .unwrap();
    assert!(kin_members > 0);
    apply_outcome(&mut sim, &data, event, bless);

    assert!(
        !sim.is_faction_aboard("verdant_kin"),
        "the merged people lose their separate name"
    );
    assert!(
        sim.is_faction_aboard("hearth_union"),
        "the host people remain"
    );
    assert_eq!(
        sim.population.count, heads_before,
        "a union keeps every soul aboard — unlike a schism, which sheds them"
    );
}

#[test]
fn a_friction_fracture_sheds_the_named_faction_and_its_craft() {
    // Content-depth factions round 4: an inter-faction quarrel that gates on
    // BOTH factions being aboard and whose "let it break" outcome sheds the
    // named one via faction_loss_id AND carries its subsystem coupling — the
    // machinists take their engineering know-how with them when they go.
    let data = GameData::load().unwrap();
    // Found a campaign that actually holds the quarrelling pair aboard.
    let picks = vec![
        "steel_covenant".to_string(),
        "verdant_kin".to_string(),
        "hearth_union".to_string(),
    ];
    let mut sim = SimState::new_campaign(&data, "adaptors", 41, &picks);
    sim.dynasty.generation = 5;
    sim.population.cultural_drift = 0.6;

    let event = data.events.get("the_forge_and_the_garden").unwrap();
    assert!(
        passes_gate(&sim, event),
        "the quarrel fires with both aboard"
    );
    // Make the Covenant the LARGEST, so a shed-the-smallest rule would spare
    // it — proving the fracture targets the named faction, not the smallest.
    for f in &mut sim.factions {
        f.members = if f.faction_id == "steel_covenant" {
            900
        } else {
            100
        };
    }
    let before = sim.subsystems["engineering_bay"].knowledge;

    let fracture = event
        .outcomes
        .iter()
        .position(|o| o.faction_loss_id.as_deref() == Some("steel_covenant"))
        .expect("the forge quarrel can end in the Covenant leaving");
    apply_outcome(&mut sim, &data, event, fracture);

    assert!(
        !sim.is_faction_aboard("steel_covenant"),
        "the named faction departs even as the largest aboard"
    );
    assert!(
        sim.is_faction_aboard("verdant_kin"),
        "the other quarreller stays"
    );
    assert!(
        sim.subsystems["engineering_bay"].knowledge < before,
        "the machinists' craft leaves with them"
    );
}

#[test]
fn a_dominant_faction_gate_colors_events_by_who_runs_the_ship() {
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "adaptors", 9, &picks);
    // `the_rewriting` is Ascension-Circle-flavored augmentation zealotry.
    let event = data.events.get("the_rewriting").unwrap();
    assert_eq!(event.requires_dominant_faction, "ascension_circle");
    sim.dynasty.generation = 3; // clear its min_generation gate

    // Make the Ascension Circle the clear majority aboard.
    for f in &mut sim.factions {
        f.members = if f.faction_id == "ascension_circle" {
            900
        } else {
            50
        };
    }
    assert_eq!(sim.dominant_faction_id(), Some("ascension_circle"));
    assert!(passes_gate(&sim, event));

    // Shift dominance elsewhere: the event drops out of the pool.
    for f in &mut sim.factions {
        f.members = if f.faction_id == "ascension_circle" {
            50
        } else {
            900
        };
    }
    assert_ne!(sim.dominant_faction_id(), Some("ascension_circle"));
    assert!(!passes_gate(&sim, event));
}

#[test]
fn a_dominant_faction_unlocks_a_choice_others_cannot_take() {
    // Content-depth factions round 25: who runs the ship puts a distinct option on
    // the table. The Wasting's germ-line cure appears only while the Ascension Circle
    // is dominant, and never for a ship the Steel Covenant runs.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 71, &picks);
    let tmpl = data.events.get("the_wasting").unwrap();
    let cure = tmpl
        .outcomes
        .iter()
        .position(|o| o.id == "rewrite_the_affliction")
        .unwrap();

    let set_dominant = |sim: &mut SimState, id: &str| {
        sim.factions = vec![FactionState {
            faction_id: id.to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        }];
    };

    // Under the Steel Covenant: the Ascension cure is not on the table…
    set_dominant(&mut sim, "steel_covenant");
    assert!(
        !available_outcome_indices(&sim, tmpl).contains(&cure),
        "the germ-line cure needs the Ascension in charge"
    );
    // …but the base choices always are.
    assert!(
        available_outcome_indices(&sim, tmpl).contains(&0),
        "the base choices are always offered"
    );

    // Under the Ascension Circle: the cure appears.
    set_dominant(&mut sim, "ascension_circle");
    assert!(
        available_outcome_indices(&sim, tmpl).contains(&cure),
        "the Ascension running the ship unlocks the germ-line cure"
    );
}

#[test]
fn the_offered_road_reads_who_runs_the_ship() {
    // Content-depth event families round 26: the deepened first-contact family carries
    // the faction-outcome coupling — the Ascension can bargain with an advanced
    // civilization as near-kin, a door the base choices don't open, and one no other
    // polity gets.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 73, &picks);
    let tmpl = data.events.get("the_offered_road").unwrap();
    let bargain = tmpl
        .outcomes
        .iter()
        .position(|o| o.id == "seek_a_deeper_bargain")
        .unwrap();

    // The two base choices — keep the road, trade the archive — always show.
    assert!(available_outcome_indices(&sim, tmpl).contains(&0));
    assert!(available_outcome_indices(&sim, tmpl).len() >= 2);

    // Not under the Hearth Union: no deeper bargain.
    sim.factions = vec![FactionState {
        faction_id: "hearth_union".to_string(),
        members: sim.population.count,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    }];
    assert!(!available_outcome_indices(&sim, tmpl).contains(&bargain));

    // Under the Ascension Circle: the kindred bargain opens.
    sim.factions[0].faction_id = "ascension_circle".to_string();
    assert!(available_outcome_indices(&sim, tmpl).contains(&bargain));
}
