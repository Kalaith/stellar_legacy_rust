//! What a finished charter leaves behind: the ship's character, its
//! peoples' goodwill, the hold's contents, and the mark of a botched one.

use super::*;

#[test]
fn completing_a_charter_shapes_the_ships_character() {
    // Content-depth charters round 17: the missions a reputation unlocks build it
    // further. Seeing the sanctuary run through deepens the ship's mercy; the
    // hard contract hardens it — a self-reinforcing spiral through the missions.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
    let hard = data.contracts.get("the_hard_contract").unwrap();
    assert!(
        !sanctuary.completion_reward.reputation_deltas.is_empty()
            && !hard.completion_reward.reputation_deltas.is_empty(),
        "both reputation-gated charters shape character on completion"
    );

    let mut kind = SimState::new_campaign(&data, "preservers", 87, &picks);
    let m0 = kind.reputation("mercy");
    apply_completion_reward(&mut kind, sanctuary, 1.0);
    assert!(
        kind.reputation("mercy") > m0,
        "a voyage of carrying refugees deepens the ship's mercy"
    );

    let mut cold = SimState::new_campaign(&data, "preservers", 88, &picks);
    let c0 = cold.reputation("mercy");
    apply_completion_reward(&mut cold, hard, 1.0);
    assert!(
        cold.reputation("mercy") < c0,
        "a voyage of cold enforcement hardens it"
    );
}

#[test]
fn a_completed_mission_earns_its_peoples_goodwill() {
    // Content-depth charters round 19: a mission the ship flew for a people can
    // leave that people delighted — the completion goodwill that feeds the
    // round-19 gift beats. Only lands on a faction actually aboard.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
    // The sanctuary run rewards the Hearth, who ride in the founding set.
    let run = data.contracts.get("the_sanctuary_run").unwrap();
    assert!(
        run.completion_reward
            .faction_approval_deltas
            .iter()
            .any(|d| d.id == "hearth_union"),
        "the sanctuary run earns the Hearth's goodwill"
    );
    assert!(sim.is_faction_aboard("hearth_union"));
    let before = sim
        .factions
        .iter()
        .find(|f| f.faction_id == "hearth_union")
        .unwrap()
        .approval;
    apply_completion_reward(&mut sim, run, 1.0);
    let after = sim
        .factions
        .iter()
        .find(|f| f.faction_id == "hearth_union")
        .unwrap()
        .approval;
    assert!(
        after > before,
        "carrying the frightened home leaves the Hearth glad it came"
    );
}

#[test]
fn a_completed_salvage_charter_leaves_a_component_in_the_hold() {
    // Content-depth charters round 20: a mission can recover a lasting piece of
    // kit — the Long Tow pulls a warp coil from the dead titan it hauls home.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 1, &picks);
    let tow = data.contracts.get("the_long_tow").unwrap();
    let comp = tow
        .completion_reward
        .grant_component
        .clone()
        .expect("the long tow recovers a component");
    assert!(
        data.ship_components.find_any(&comp).is_some(),
        "the recovered component is real"
    );
    assert!(
        !sim.ship.salvage.contains(&comp),
        "the hold starts without it"
    );

    apply_completion_reward(&mut sim, tow, 1.0);
    assert!(
        sim.ship.salvage.contains(&comp),
        "the recovered component lands in the salvage hold to install"
    );
}

#[test]
fn a_completed_charter_leaves_a_lasting_capability() {
    // Content-depth charters round 15: a mission seen through leaves the ship a
    // skill it keeps, beyond the pay. The Karst Works masters extraction (an
    // engineering boon); an ordinary charter leaves nothing.
    let data = GameData::load().unwrap();
    let works = data.contracts.get("the_karst_works").unwrap();
    assert!(
        !works.completion_reward.is_none(),
        "the works leave a legacy"
    );

    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 77, &picks);
    // Room to grow (a fresh bay is already near full; start it low to see the lift).
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.5;
    let before = sim.subsystems["engineering_bay"].knowledge;
    let line = apply_completion_reward(&mut sim, works, 1.0);
    assert!(line.is_some(), "the boon narrates itself");
    assert!(
        sim.subsystems["engineering_bay"].knowledge > before,
        "building the great works masters extraction for good"
    );

    // A charter with no completion reward changes nothing and says nothing.
    let ordinary = data
        .contracts
        .iter()
        .map(|(_, c)| c)
        .find(|c| c.completion_reward.is_none())
        .expect("some charter leaves no legacy");
    let k0 = sim.subsystems["engineering_bay"].knowledge;
    assert!(apply_completion_reward(&mut sim, ordinary, 1.0).is_none());
    assert_eq!(sim.subsystems["engineering_bay"].knowledge, k0);
}

#[test]
fn a_mission_outcome_moves_the_crews_spirits() {
    // Content-depth charters round 31: the crew feels a mission's outcome. A clean run (a
    // high success score) lifts morale, a botched one (a low score) dents it, and a
    // break-even middling result barely moves the needle; a 0 scale leaves spirits untouched.
    let scale = 0.1;
    assert!(
        mission_outcome_morale_shift(1.0, scale) > 0.0,
        "a mission seen through lifts the crew's spirits"
    );
    assert!(
        mission_outcome_morale_shift(0.1, scale) < 0.0,
        "a mission botched or abandoned dents them"
    );
    assert_eq!(
        mission_outcome_morale_shift(0.5, scale),
        0.0,
        "a break-even result is neutral"
    );
    assert_eq!(
        mission_outcome_morale_shift(1.0, 0.0),
        0.0,
        "a zero scale leaves the crew's spirits untouched"
    );
}

#[test]
fn abandoning_a_charter_marks_the_ships_name() {
    // Content-depth charters round 18: the negative mirror of the completion
    // reward — the first charter effect keyed to failure. Giving up the sanctuary
    // run hardens the mercy the crew couldn't keep and earns a name for folding;
    // giving up the hard contract is a *merciful* fold — the ship that would not,
    // in the end, strip a home comes home kinder but still a hull that folds.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
    let hard = data.contracts.get("the_hard_contract").unwrap();
    assert!(
        !sanctuary.abandonment.is_none() && !hard.abandonment.is_none(),
        "both relief charters mark the ship's name when defaulted"
    );

    // Abandon the sanctuary run: mercy hardens and resolve falls.
    let mut a = SimState::new_campaign(&data, "preservers", 91, &picks);
    let (m0, r0) = (a.reputation("mercy"), a.reputation("resolve"));
    apply_abandonment(&mut a, sanctuary);
    assert!(
        a.reputation("mercy") < m0,
        "leaving refugees behind hardens the ship"
    );
    assert!(
        a.reputation("resolve") < r0,
        "a relief run given up earns a name for folding"
    );

    // Abandon the hard contract: a merciful fold — mercy rises, resolve still falls.
    let mut b = SimState::new_campaign(&data, "preservers", 92, &picks);
    let (m1, r1) = (b.reputation("mercy"), b.reputation("resolve"));
    apply_abandonment(&mut b, hard);
    assert!(
        b.reputation("mercy") > m1,
        "refusing to finish the cruel job comes home kinder"
    );
    assert!(
        b.reputation("resolve") < r1,
        "but it is still, to the dark, a hull that folded"
    );

    // An ordinary charter marks nothing on a failed conclusion.
    let ordinary = data.contracts.get("deep_vein_survey").unwrap();
    let mut c = SimState::new_campaign(&data, "preservers", 93, &picks);
    assert!(
        apply_abandonment(&mut c, ordinary).is_none(),
        "an ordinary charter's failure costs only its pay"
    );
}

#[test]
fn a_botched_charter_leaves_a_mark_that_bars_the_like() {
    // Content-depth charters round 30: a failed charter seeds a dark deed the board reads.
    // The Sanctuary Run's failure marks the ship as having broken a sanctuary trust; the Ark
    // Charter then bars that ship — a hull that abandoned refugees is not handed another cargo
    // of vulnerable lives.
    let data = GameData::load().unwrap();
    let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
    let ark = data.contracts.get("the_ark_run").unwrap();
    let tag = sanctuary.failure_consequence.clone();
    assert!(
        !tag.is_empty(),
        "the sanctuary run marks its failure on record"
    );
    assert!(
        ark.forbidden_consequence.contains(&tag),
        "the ark charter bars a ship carrying that mark"
    );

    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 20, &picks);
    // Before the failure: the ark charter is open to an untarnished ship.
    assert!(
        meets_in_world_gate(&sim, ark),
        "an untarnished ship may take the ark charter"
    );
    // Record the failure deed exactly as the conclude path does on a Failure.
    sim.consequences.push(tag);
    assert!(
        !meets_in_world_gate(&sim, ark),
        "a ship that broke a sanctuary trust is barred from the ark charter"
    );
}
