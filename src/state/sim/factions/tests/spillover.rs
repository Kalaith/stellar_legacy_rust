//! Sentiment that travels sideways: favouring or slighting one people is
//! felt by its rivals and its allies too.

use super::*;

#[test]
fn favoring_a_people_sours_its_aboard_rivals() {
    // Content-depth factions round 14: the friction pairs made a lasting cost.
    // Lifting one people's approval spills resentment onto its aboard rivals, so
    // the meter cannot be maxed for everyone; a rival not aboard is untouched,
    // and slighting a people does not lift its rivals.
    use crate::data::events::FactionApprovalDelta;
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.rival_approval_spillover > 0.0,
        "this test needs the rivalry spillover enabled"
    );
    // Steel Covenant and Verdant Kin are authored rivals; the Hearth is neither.
    let def = data.factions.get("steel_covenant").unwrap();
    assert!(def.rivals.contains(&"verdant_kin".to_string()));

    let fs = |id: &str| FactionState {
        faction_id: id.to_string(),
        members: 400,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        9,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.factions = vec![fs("steel_covenant"), fs("verdant_kin"), fs("hearth_union")];

    // Favor the Covenant: its rival the Kin sours, the unrelated Hearth does not.
    sim.apply_rival_approval_spillover(
        &data,
        &[FactionApprovalDelta {
            id: "steel_covenant".to_string(),
            delta: 0.2,
        }],
    );
    let approval = |sim: &SimState, id: &str| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval
    };
    assert!(
        approval(&sim, "verdant_kin") < 0.5,
        "the Covenant's rival resents the favoritism"
    );
    assert_eq!(
        approval(&sim, "hearth_union"),
        0.5,
        "a people that is no rival is untouched"
    );

    // Slighting the Covenant does not lift its rival (the cost is of favoritism).
    let kin_before = approval(&sim, "verdant_kin");
    sim.apply_rival_approval_spillover(
        &data,
        &[FactionApprovalDelta {
            id: "steel_covenant".to_string(),
            delta: -0.2,
        }],
    );
    assert_eq!(
        approval(&sim, "verdant_kin"),
        kin_before,
        "a slight to a people is not a gift to its rivals"
    );
}
#[test]
fn favoring_a_people_warms_its_aboard_allies() {
    // Content-depth factions round 17: the positive twin of the rivalry spillover.
    // Lifting one people's approval shares a fraction of the goodwill with its
    // aboard allies, so the meter rewards building a coalition; an ally not aboard
    // is untouched, and slighting a people does not sour its allies (the mechanic
    // is the reward of coalition, not shared misery).
    use crate::data::events::FactionApprovalDelta;
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.ally_approval_spillover > 0.0,
        "this test needs the alliance spillover enabled"
    );
    // Hearth Union and Verdant Kin are authored allies (the green hearth); the
    // Steel Covenant is neither.
    let def = data.factions.get("hearth_union").unwrap();
    assert!(def.allies.contains(&"verdant_kin".to_string()));

    let fs = |id: &str| FactionState {
        faction_id: id.to_string(),
        members: 400,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        11,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.factions = vec![fs("hearth_union"), fs("verdant_kin"), fs("steel_covenant")];

    let approval = |sim: &SimState, id: &str| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval
    };

    // Favor the Hearth: its ally the Kin warms, the unrelated Covenant does not.
    sim.apply_ally_approval_spillover(
        &data,
        &[FactionApprovalDelta {
            id: "hearth_union".to_string(),
            delta: 0.2,
        }],
    );
    assert!(
        approval(&sim, "verdant_kin") > 0.5,
        "the Hearth's ally shares in the goodwill"
    );
    assert_eq!(
        approval(&sim, "steel_covenant"),
        0.5,
        "a people that is no ally is untouched"
    );

    // Slighting the Hearth does not sour its ally (the reward is of coalition).
    let kin_before = approval(&sim, "verdant_kin");
    sim.apply_ally_approval_spillover(
        &data,
        &[FactionApprovalDelta {
            id: "hearth_union".to_string(),
            delta: -0.2,
        }],
    );
    assert_eq!(
        approval(&sim, "verdant_kin"),
        kin_before,
        "a slight to a people is not a wound to its allies"
    );
}
#[test]
fn slighting_a_people_cheers_its_rivals_and_stings_its_allies() {
    // Content-depth factions round 32: the down-swing mirrors the it14/it17 spillovers
    // deliberately left out. Where favoring a people sours its rivals and warms its allies,
    // *slighting* it cheers those rivals (schadenfreude) and stings those allies
    // (commiseration) — the same relationships spilling over across the opposite sign. The
    // Verdant Kin carry both a rival (the Steel Covenant) and an ally (the Hearth), so one
    // wound to the Kin exercises both couplings; a *favor* leaves them to the it14/it17 path.
    use crate::data::events::FactionApprovalDelta;
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.rival_approval_schadenfreude > 0.0
            && data.config.factions.ally_approval_commiseration > 0.0,
        "this test needs the schadenfreude and commiseration spillovers enabled"
    );
    // Verdant Kin <-> Steel Covenant are rivals; Verdant Kin <-> Hearth Union are allies.
    let kin_def = data.factions.get("verdant_kin").unwrap();
    assert!(kin_def.rivals.contains(&"steel_covenant".to_string()));
    assert!(kin_def.allies.contains(&"hearth_union".to_string()));

    let fs = |id: &str| FactionState {
        faction_id: id.to_string(),
        members: 400,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        13,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.factions = vec![fs("verdant_kin"), fs("steel_covenant"), fs("hearth_union")];
    let approval = |sim: &SimState, id: &str| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval
    };

    // Slight the Kin: its rival the Covenant takes quiet satisfaction, its ally the Hearth
    // shares the sting.
    let slight = [FactionApprovalDelta {
        id: "verdant_kin".to_string(),
        delta: -0.2,
    }];
    sim.apply_rival_approval_schadenfreude(&data, &slight);
    sim.apply_ally_approval_commiseration(&data, &slight);
    assert!(
        approval(&sim, "steel_covenant") > 0.5,
        "the Kin's rival is cheered by its misfortune"
    );
    assert!(
        approval(&sim, "hearth_union") < 0.5,
        "the Kin's ally is stung by its misfortune"
    );

    // A *favor* to the Kin runs the it14/it17 path, not these — the down-swing functions
    // skip positive deltas, so a gain leaves rival and ally untouched by them.
    let cov_before = approval(&sim, "steel_covenant");
    let hearth_before = approval(&sim, "hearth_union");
    let favor = [FactionApprovalDelta {
        id: "verdant_kin".to_string(),
        delta: 0.2,
    }];
    sim.apply_rival_approval_schadenfreude(&data, &favor);
    sim.apply_ally_approval_commiseration(&data, &favor);
    assert_eq!(
        approval(&sim, "steel_covenant"),
        cov_before,
        "a favor to a people is not read by the schadenfreude coupling"
    );
    assert_eq!(
        approval(&sim, "hearth_union"),
        hearth_before,
        "a favor to a people is not read by the commiseration coupling"
    );
}
#[test]
fn a_charters_people_take_pride_in_its_success_and_are_let_down_by_its_failure() {
    // Content-depth charters round 32: the charter→faction pride/letdown coupling. The
    // Seedbearers' Writ is gated on the Verdant Kin being aboard (requires_faction_aboard), so
    // it is work the Kin are uniquely called to — seeing it through honors them (approval up),
    // botching it lets them down (approval down), while a people the writ does not name (the
    // Steel Covenant) is untouched either way.
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.charter_completion_pride > 0.0
            && data.config.factions.charter_failure_letdown > 0.0,
        "this test needs the charter pride/letdown coupling enabled"
    );
    let template = data.contracts.get("the_seedbearers_writ").unwrap();
    assert_eq!(template.requires_faction_aboard, vec!["verdant_kin"]);

    let fs = |id: &str| FactionState {
        faction_id: id.to_string(),
        members: 400,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    let approval = |sim: &SimState, id: &str| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval
    };

    // Seeing the writ through: the Kin take pride, the unnamed Covenant is untouched.
    let mut done = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    done.factions = vec![fs("verdant_kin"), fs("steel_covenant")];
    done.apply_charter_outcome_faction_sentiment(&data, template, false);
    assert!(
        approval(&done, "verdant_kin") > 0.5,
        "the Kin take pride in the writ they were called to"
    );
    assert_eq!(
        approval(&done, "steel_covenant"),
        0.5,
        "a people the writ does not name is untouched by its success"
    );

    // Botching it: the Kin are let down, and by more than the pride (a failure stings more).
    let mut failed = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    failed.factions = vec![fs("verdant_kin"), fs("steel_covenant")];
    failed.apply_charter_outcome_faction_sentiment(&data, template, true);
    assert!(
        approval(&failed, "verdant_kin") < 0.5,
        "the Kin are let down by the writ they could not keep"
    );
    assert_eq!(
        approval(&failed, "steel_covenant"),
        0.5,
        "a people the writ does not name is untouched by its failure"
    );
    assert!(
        (0.5 - approval(&failed, "verdant_kin")) > (approval(&done, "verdant_kin") - 0.5),
        "a failure stings the Kin more than a success pleases them"
    );
}
