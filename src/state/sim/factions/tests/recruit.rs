//! Taking a people aboard and losing one over the side: the gate, the
//! dowry, and the wake each leaves in the ship's name and cohesion.

use super::*;

#[test]
fn recruiting_a_people_is_gated_and_charges_credits() {
    let (data, mut sim, _picks) = armed(1);
    sim.resources.credits = 100_000;
    let newcomer = sim.recruitable_faction_ids(&data)[0].clone();

    // Full complement → refused (not short of the founding count).
    assert!(sim.recruit_faction_group(&data, &newcomer).is_err());

    // Lose the smallest faction → short by one.
    sim.apply_faction_loss(&data, FactionLossKind::Departed);
    let lost_id = sim
        .factions
        .iter()
        .find(|f| !f.is_aboard())
        .unwrap()
        .faction_id
        .clone();

    // Underway → refused even while short.
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(crate::simulation::contract::start_contract(&template, &sim));
    assert!(sim.recruit_faction_group(&data, &newcomer).is_err());
    sim.contract = None;

    // A lost people never returns.
    assert!(sim.recruit_faction_group(&data, &lost_id).is_err());

    // In port, short, from the untouched pool → allowed; credits + head count.
    let credits_before = sim.resources.credits;
    let pop_before = sim.population.count;
    sim.recruit_faction_group(&data, &newcomer).unwrap();
    assert_eq!(
        credits_before - sim.resources.credits,
        data.config.factions.recruit_group_cost_credits
    );
    assert_eq!(
        sim.population.count - pop_before,
        data.config.factions.recruit_group_size
    );
    assert!(sim
        .factions
        .iter()
        .any(|f| f.faction_id == newcomer && f.is_aboard()));
}
#[test]
fn a_recruited_people_brings_its_signature_dowry() {
    // Content-depth factions round 7: recruiting a people is no longer a bare
    // head count — the Steel Covenant walk into the engineering bay and leave
    // it sharper. Which people you take on matters.
    let (data, mut sim, _picks) = armed(9);
    sim.resources.credits = 100_000;
    // Free a slot, then recruit the makers specifically.
    sim.apply_faction_loss(&data, FactionLossKind::Departed);
    assert!(
        !sim.is_faction_aboard("steel_covenant"),
        "the makers are recruitable in this campaign"
    );
    let boon = &data.factions.get("steel_covenant").unwrap().recruit_boon;
    assert!(boon
        .subsystem_deltas
        .iter()
        .any(|d| d.id == "engineering_bay"));

    let before = sim.subsystems["engineering_bay"].knowledge;
    sim.recruit_faction_group(&data, "steel_covenant").unwrap();
    assert!(
        sim.subsystems["engineering_bay"].knowledge > before,
        "the Covenant's craft lifts the engineering bay on arrival"
    );
    // The dowry's own line was logged (not the generic recruit line).
    assert!(
        sim.log.iter().any(|e| e.text.contains("engineering bay")),
        "the recruit logs the people's signature arrival"
    );
}
#[test]
fn taking_a_people_aboard_warms_the_ships_name() {
    // Content-depth factions round 34: the reputation mirror of the round-31 departure penalty.
    // Where a people fleeing the ship lowers its mercy, welcoming one aboard raises it — a hull
    // that takes people in earns a merciful name, by exactly the recruit bonus.
    let (data, mut sim, _picks) = armed(9);
    let cfg = &data.config.factions;
    assert!(
        cfg.recruit_reputation_bonus > 0.0,
        "this test needs the recruit reputation bonus enabled"
    );
    sim.resources.credits = 100_000;
    // Free a slot the same way the dowry test does; the departure's own mercy penalty lands here,
    // so we read mercy *after* it and measure only the recruit's lift.
    sim.apply_faction_loss(&data, FactionLossKind::Departed);
    assert!(!sim.is_faction_aboard("steel_covenant"));

    let before = sim.reputation("mercy");
    sim.recruit_faction_group(&data, "steel_covenant").unwrap();
    let after = sim.reputation("mercy");
    assert!(
        after > before,
        "welcoming a new people warms the ship's mercy ({after} vs {before})"
    );
    assert!(
        (after - before - cfg.recruit_reputation_bonus).abs() < 1e-6,
        "the warming is exactly the recruit reputation bonus ({})",
        after - before
    );
}
#[test]
fn taking_a_people_aboard_dents_the_ships_cohesion() {
    // Content-depth factions round 35: the cohesion mirror of the round-26 assimilation unity
    // lift. Where folding a people into the majority removes a faultline and lifts unity, taking
    // a new people aboard adds one, so unity dents by exactly the recruit unity cost.
    let (data, mut sim, _picks) = armed(9);
    let cfg = &data.config.factions;
    assert!(
        cfg.recruit_unity_cost > 0.0,
        "this test needs the recruit unity cost enabled"
    );
    sim.resources.credits = 100_000;
    // Free a slot; the departure's own cohesion scar lands here, so we set a clean baseline
    // after it and measure only the recruit's dent.
    sim.apply_faction_loss(&data, FactionLossKind::Departed);
    assert!(!sim.is_faction_aboard("steel_covenant"));
    sim.population.unity = 0.6;
    let before = sim.population.unity;
    sim.recruit_faction_group(&data, "steel_covenant").unwrap();
    assert!(
        sim.population.unity < before,
        "a new people dents the ship's cohesion ({} vs {before})",
        sim.population.unity
    );
    assert!(
        (before - sim.population.unity - cfg.recruit_unity_cost).abs() < 1e-6,
        "the dent is exactly the recruit unity cost ({})",
        before - sim.population.unity
    );
}
#[test]
fn recruiting_a_people_stirs_the_ships_old_rivalries_and_friendships() {
    // Content-depth factions round 28: taking on a new people is a political act. The
    // Steel Covenant rivals the Verdant Kin and allies the Meridian Accord, so bringing the
    // makers aboard bristles the gardeners and gladdens the arbiters — the incumbent peoples
    // react to who you bring home, by the same catalog relationships the cohesion coupling
    // uses.
    let (data, mut sim, _picks) = armed(15);
    let cfg = &data.config.factions;
    assert!(
        cfg.recruit_rival_approval_penalty > 0.0 && cfg.recruit_ally_approval_bonus > 0.0,
        "this test needs the recruit-reaction coupling enabled"
    );
    sim.resources.credits = 100_000;
    // Seat the newcomer's rival and ally, leaving a slot open for the newcomer.
    sim.factions = vec![
        FactionState {
            faction_id: "verdant_kin".to_string(),
            members: 300,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        },
        FactionState {
            faction_id: "meridian_accord".to_string(),
            members: 300,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        },
    ];
    assert!(!sim.is_faction_aboard("steel_covenant"));

    sim.recruit_faction_group(&data, "steel_covenant").unwrap();

    let approval = |id: &str| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval
    };
    // The rival bristles, by exactly the penalty; the ally warms, by exactly the bonus.
    assert!(
        approval("verdant_kin") < 0.5,
        "a rival bristles at the newcomer"
    );
    assert!(
        (0.5 - approval("verdant_kin") - cfg.recruit_rival_approval_penalty).abs() < 1e-6,
        "the rival's souring is exactly the penalty"
    );
    assert!(
        approval("meridian_accord") > 0.5,
        "an ally welcomes the newcomer"
    );
    assert!(
        (approval("meridian_accord") - 0.5 - cfg.recruit_ally_approval_bonus).abs() < 1e-6,
        "the ally's warming is exactly the bonus"
    );
    // The newcomer no longer arrives neutral (content-depth factions round 33): it boards wary
    // of its aboard rival the Kin and gladdened by its aboard ally the Accord, so its start is
    // the default shifted by (one ally's comfort − one rival's wariness).
    let expected = default_approval() + cfg.recruit_newcomer_ally_comfort
        - cfg.recruit_newcomer_rival_wariness;
    assert!(
        (approval("steel_covenant") - expected).abs() < 1e-6,
        "the newcomer boards shifted by who it is joining"
    );
}
#[test]
fn a_newcomer_boards_wary_of_a_rival_and_glad_of_a_friend() {
    // Content-depth factions round 33: the newcomer's-eye mirror of the round-28 incumbent
    // reactions. The Steel Covenant rivals the Verdant Kin and allies the Meridian Accord, so
    // it boards a ship carrying its rival warier than the neutral default, and a ship carrying
    // its ally gladder — where a ship carrying neither leaves it at the default.
    let data = GameData::load().unwrap();
    let cfg = &data.config.factions;
    assert!(
        cfg.recruit_newcomer_rival_wariness > 0.0 && cfg.recruit_newcomer_ally_comfort > 0.0,
        "this test needs the newcomer-wariness coupling enabled"
    );

    // Recruit the Covenant into a roster holding a single named people, and read the approval
    // it boards with. The Hearth is neither rival nor ally of the Covenant (a neutral witness).
    let boarded_with = |incumbent: &str| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            4,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.credits = 100_000;
        sim.factions = vec![FactionState {
            faction_id: incumbent.to_string(),
            members: 300,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        }];
        sim.recruit_faction_group(&data, "steel_covenant").unwrap();
        sim.factions
            .iter()
            .find(|f| f.faction_id == "steel_covenant")
            .unwrap()
            .approval
    };

    let with_rival = boarded_with("verdant_kin"); // its rival is aboard
    let with_ally = boarded_with("meridian_accord"); // its ally is aboard
    let with_neutral = boarded_with("hearth_union"); // neither

    assert!(
        (with_neutral - default_approval()).abs() < 1e-6,
        "with no rival or ally aboard the newcomer boards at the neutral default ({with_neutral})"
    );
    assert!(
            with_rival < with_neutral,
            "a newcomer joining a ship that carries its rival boards warier ({with_rival} vs {with_neutral})"
        );
    assert!(
            with_ally > with_neutral,
            "a newcomer joining a ship that carries its friend boards gladder ({with_ally} vs {with_neutral})"
        );
}
#[test]
fn a_departing_people_takes_the_craft_of_its_tended_module() {
    // Content-depth factions round 20: shedding a people costs more than its
    // headcount — the module it tended loses a chunk of its living expertise.
    let (data, mut sim, picks) = armed(3);
    let loss = data.config.factions.departed_faction_knowledge_loss;
    assert!(loss > 0.0, "the coupling must be configured");
    let fid = picks[1].clone();
    let tended = data.factions.get(&fid).unwrap().tended_subsystem.clone();
    assert!(!tended.is_empty(), "the founding people tends a module");
    // Pin the tended module's knowledge to a known value.
    sim.subsystems.get_mut(&tended).unwrap().knowledge = 0.8;

    sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, &fid);

    assert!(!sim.is_faction_aboard(&fid), "the people are gone");
    let after = sim.subsystems.get(&tended).unwrap().knowledge;
    assert!(
        (after - (0.8 - loss)).abs() < 1e-4,
        "the tended module lost the departed's expertise (knowledge {after})"
    );
}
#[test]
fn a_departure_stirs_the_ships_rivalries_and_friendships() {
    // Content-depth factions round 30: the mirror of the recruitment reactions. When the
    // Steel Covenant depart, their aboard rival the Verdant Kin are quietly relieved (approval
    // up by the relief) and their aboard ally the Meridian Accord are saddened (down by the
    // penalty); the departed people, no longer aboard, reacts to nothing.
    let (data, mut sim, _picks) = armed(16);
    let cfg = &data.config.factions;
    assert!(
        cfg.departure_rival_approval_relief > 0.0 && cfg.departure_ally_approval_penalty > 0.0,
        "this test needs the departure-reaction coupling enabled"
    );
    // Seat the departing people, their rival, and their ally (all at neutral approval).
    sim.factions = ["steel_covenant", "verdant_kin", "meridian_accord"]
        .iter()
        .map(|id| FactionState {
            faction_id: (*id).to_string(),
            members: 300,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        })
        .collect();
    let approval = |sim: &SimState, id: &str| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .unwrap()
            .approval
    };

    sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, "steel_covenant");

    // The rival is relieved, by exactly the relief; the ally saddened, by exactly the penalty.
    assert!(
        approval(&sim, "verdant_kin") > 0.5,
        "a rival is relieved to see them go"
    );
    assert!(
        (approval(&sim, "verdant_kin") - 0.5 - cfg.departure_rival_approval_relief).abs() < 1e-6,
        "the rival's relief is exactly the configured amount"
    );
    assert!(
        approval(&sim, "meridian_accord") < 0.5,
        "an ally is saddened to see them go"
    );
    assert!(
        (0.5 - approval(&sim, "meridian_accord") - cfg.departure_ally_approval_penalty).abs()
            < 1e-6,
        "the ally's grief is exactly the configured amount"
    );
}
#[test]
fn a_break_away_marks_the_ships_name_but_a_planetfall_does_not() {
    // Content-depth factions round 31: a people that *breaks away* (Departed) spreads word
    // that this is a hull peoples flee, and the ship's mercy reputation suffers; a people that
    // *settles* a world (a colony founded, not a flight) marks nothing.
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.departure_reputation_penalty > 0.0,
        "this test needs the departure-reputation coupling enabled"
    );
    let mercy_after = |kind: FactionLossKind| -> f32 {
        let (_d, mut sim, _p) = armed(17);
        // Two aboard peoples so a loss is allowed (never the ship's last).
        sim.factions = vec![fs("steel_covenant", 300), fs("verdant_kin", 300)];
        let before = sim.reputation("mercy");
        sim.apply_faction_loss_by_id(&data, kind, "steel_covenant");
        before - sim.reputation("mercy")
    };

    let broke_away = mercy_after(FactionLossKind::Departed);
    let settled = mercy_after(FactionLossKind::Settled);
    assert!(
        broke_away > 0.0,
        "a break-away costs the ship its merciful name ({broke_away})"
    );
    assert_eq!(
        settled, 0.0,
        "a people that makes planetfall to settle marks nothing"
    );
}
