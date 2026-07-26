//! What moves a people's approval: who rules, what name the ship earns,
//! how divided the polity is, and how well its own module is kept.

use super::*;

#[test]
fn who_runs_the_ship_bends_its_reputation_over_the_generations() {
    // Content-depth factions round 16: the dominant people's standing character
    // drifts the ship's reputation. A ship run by a kind people (the Hearth)
    // grows more merciful over the years; one run by a cold people (the
    // Ascension) hardens — no dramatic choice required.
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.dominant_reputation_lean_per_year > 0.0,
        "this test needs the dominant-reputation lean enabled"
    );

    let mercy_after = |dominant: &str| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            31,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![FactionState {
            faction_id: dominant.to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        }];
        for _ in 0..30 {
            sim.apply_dominant_reputation_lean(&data);
        }
        sim.reputation("mercy")
    };

    let under_hearth = mercy_after("hearth_union");
    let under_ascension = mercy_after("ascension_circle");
    assert!(
        under_hearth > 0.5,
        "a kind majority grows the ship a merciful name"
    );
    assert!(under_ascension < 0.5, "a cold majority hardens it");
    // A people with no leaning leaves the ship's name to its choices.
    let under_neutral = mercy_after("meridian_accord");
    assert_eq!(under_neutral, 0.5, "an unleaning people touches nothing");
}
#[test]
fn the_name_the_ship_earns_warms_or_cools_each_people_toward_it() {
    // Content-depth factions round 27: the reverse of the round-16 lean above, closing
    // the reputation_leanings loop. A merciful ship contents the people that prizes mercy
    // (the Hearth, mercy +0.5) and sours the one that scorns it (the Ascension, mercy
    // −0.4); a ruthless ship does the reverse. A neutral character moves neither.
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.reputation_alignment_approval_scale > 0.0,
        "this test needs the reputation-alignment sentiment enabled"
    );

    // Approval change over one year for a given people at a given ship mercy.
    let delta_for = |faction: &str, mercy: f32| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            33,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![FactionState {
            faction_id: faction.to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        }];
        sim.reputation.insert("mercy".to_string(), mercy);
        sim.apply_reputation_alignment_sentiment(&data);
        sim.factions[0].approval - 0.5
    };

    // A merciful ship (mercy 1.0): the mercy-prizing Hearth warms, the mercy-scorning
    // Ascension cools.
    assert!(
        delta_for("hearth_union", 1.0) > 0.0,
        "a merciful ship contents the people that prizes mercy"
    );
    assert!(
        delta_for("ascension_circle", 1.0) < 0.0,
        "a merciful ship sours the people that scorns it"
    );
    // A ruthless ship (mercy 0.0): the signs flip.
    assert!(
        delta_for("hearth_union", 0.0) < 0.0,
        "a ruthless ship sours the mercy-prizing people"
    );
    assert!(
        delta_for("ascension_circle", 0.0) > 0.0,
        "a ruthless ship contents the mercy-scorning people"
    );
    // A neutral character (the launch state) moves no one.
    assert_eq!(
        delta_for("hearth_union", 0.5),
        0.0,
        "a ship of neutral character warms no one either way"
    );
}
#[test]
fn a_content_polity_steadies_the_ship_and_a_resentful_one_frays_it() {
    // Content-depth factions round 15: the faction system's first coupling to
    // the ship's own cohesion. Two otherwise-identical ships, one carrying a
    // content people and one a resentful one, diverge in unity over the years —
    // a content polity holds the ship together where a resentful one wears at it.
    use crate::simulation::tick::advance_year;
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    // Clear the threshold beats so a fraying ship doesn't trip one mid-test.
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    assert!(
        data.config.factions.approval_unity_coupling > 0.0,
        "this test needs the faction-cohesion coupling enabled"
    );

    let unity_after = |approval: f32| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            79,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;
        sim.factions = vec![FactionState {
            faction_id: "steel_covenant".to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval,
            mood_band: 0,
        }];
        sim.population.unity = 0.6;
        for _ in 0..20 {
            advance_year(&mut sim, &data);
        }
        sim.population.unity
    };
    let content = unity_after(0.95);
    let resentful = unity_after(0.05);
    assert!(
        content > resentful,
        "a content polity holds the ship together where a resentful one frays it \
             (content {content} vs resentful {resentful})"
    );
}
#[test]
fn a_divided_ship_is_harder_to_govern() {
    // Content-depth factions round 18: governing a divided ship strains its
    // institutions. Two otherwise-identical ships — one carrying ideologically
    // aligned peoples, one carrying peoples at opposite ends of the tech↔tradition
    // spectrum — diverge in stability, the divided coalition eroding where the
    // aligned one holds. Distinct from the content/resentful (approval→unity) axis.
    use crate::simulation::tick::advance_year;
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.stability_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.loyalty_beats.clear();
    // Isolate the coupling: no security recovery pushing stability back up.
    data.config
        .subsystems
        .security_stability_recovery_per_condition = 0.0;
    assert!(
        data.config.factions.ideology_spread_stability_penalty > 0.0,
        "this test needs the ideology-spread coupling enabled"
    );

    let stability_after = |ids: &[&str]| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            88,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;
        let each = sim.population.count / ids.len() as u32;
        sim.factions = ids
            .iter()
            .map(|id| FactionState {
                faction_id: (*id).to_string(),
                members: each,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            })
            .collect();
        sim.population.stability = 0.6;
        for _ in 0..20 {
            advance_year(&mut sim, &data);
        }
        sim.population.stability
    };

    // Aligned peoples (all tradition-leaning) vs a coalition spanning the spectrum.
    let aligned = stability_after(&["verdant_kin", "hearth_union", "first_flame"]);
    let divided = stability_after(&["ascension_circle", "first_flame"]);
    assert!(
        divided < aligned,
        "a ship split across the ideological spectrum governs worse than an aligned one \
             (divided {divided} vs aligned {aligned})"
    );
    assert!(
        (aligned - 0.6).abs() < 1e-6,
        "an ideologically unified ship's institutions are untouched by the coupling"
    );
}
#[test]
fn aboard_rivals_grind_at_cohesion_and_allies_lift_it() {
    // Content-depth factions round 23: the relationship-side twin of the mood→unity
    // coupling. Two large aboard rivals wear at cohesion year over year; a large
    // aboard allied bloc lifts it.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.rival_unity_friction > 0.0,
        "this test needs the relationship-cohesion coupling enabled"
    );

    // Two named peoples, evenly large, and nothing else aboard — so only their
    // mutual relationship counts. Returns the one-year unity delta.
    let run = |a: &str, b: &str| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            8,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.unity = 0.6;
        sim.factions = vec![
            FactionState {
                faction_id: a.to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
            FactionState {
                faction_id: b.to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
        ];
        let before = sim.population.unity;
        sim.apply_faction_relationship_cohesion(&data);
        sim.population.unity - before
    };

    // The Ascension and the Keepers are rivals: their sharing a hull grinds unity.
    let rival_delta = run("ascension_circle", "first_flame");
    assert!(
        rival_delta < 0.0,
        "two large aboard rivals wear at unity ({rival_delta})"
    );
    // The Hearth and the Kin are allies: their bloc lifts unity.
    let ally_delta = run("hearth_union", "verdant_kin");
    assert!(
        ally_delta > 0.0,
        "a large aboard allied bloc lifts unity ({ally_delta})"
    );
}
#[test]
fn a_kept_peacekeeping_corps_cools_the_standing_rivalry() {
    // Content-depth subsystems round 32: the security corps damps the round-23 rival-cohesion
    // grind at its source. Two large aboard rivals wear at unity; a corps in good repair softens
    // that grind (the councils mediating the quarrel), a wrecked one lets it bite full — but
    // neither can abolish a real rivalry, since the relief is a fraction below 1. The ally
    // solidarity is untouched (peacekeepers quiet quarrels, not friendships).
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.rival_unity_friction > 0.0
            && data.config.subsystems.security_rival_friction_relief > 0.0,
        "this test needs the rival grind and its security relief enabled"
    );

    // Two large aboard rivals and nothing else; the one-year rival grind under a given corps.
    let grind_under = |security: f32| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            8,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.unity = 0.6;
        sim.factions = vec![
            FactionState {
                faction_id: "ascension_circle".to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
            FactionState {
                faction_id: "first_flame".to_string(),
                members: 500,
                status: FactionStatus::Aboard,
                approval: 0.5,
                mood_band: 0,
            },
        ];
        sim.subsystems.get_mut("security").unwrap().condition = security;
        let before = sim.population.unity;
        sim.apply_faction_relationship_cohesion(&data);
        sim.population.unity - before
    };

    let wrecked = grind_under(0.0); // no corps: the grind bites full
    let kept = grind_under(1.0); // corps at full repair: the grind is softened
    assert!(
        wrecked < 0.0,
        "with no corps the rivalry grinds unity full ({wrecked})"
    );
    assert!(
        kept < 0.0,
        "even a perfect corps cannot abolish a real rivalry ({kept})"
    );
    assert!(
        kept > wrecked,
        "a kept corps cools the grind — less unity is lost ({kept} vs {wrecked})"
    );
}
#[test]
fn a_delighted_people_keeps_its_module_sharp() {
    // Content-depth factions round 22: the bright mirror of the neglect coupling. A
    // tending people delighted with its lot lifts its module's condition and
    // knowledge a little each year; a merely-content one (below the proud threshold)
    // lifts nothing.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let cfg = data.config.factions;
    assert!(
        cfg.proud_tender_condition_bonus > 0.0,
        "this test needs the proud-tender coupling enabled"
    );

    // Steel Covenant tends the engineering bay; hold it mid-range so no clamp hides
    // the lift, and read the delta a single year's upkeep applies.
    let run = |approval: f32| -> (f32, f32) {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            7,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![FactionState {
            faction_id: "steel_covenant".to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval,
            mood_band: 0,
        }];
        {
            let bay = sim.subsystems.get_mut("engineering_bay").unwrap();
            bay.condition = 0.5;
            bay.knowledge = 0.5;
        }
        sim.apply_proud_tender_upkeep(&data);
        let bay = sim.subsystems.get("engineering_bay").unwrap();
        (bay.condition - 0.5, bay.knowledge - 0.5)
    };

    // A delighted people: its module gains exactly the year's dividend.
    let (dc, dk) = run(0.9);
    assert!(
        (dc - cfg.proud_tender_condition_bonus).abs() < 1e-6,
        "a proud people lifts its module's condition by the yearly bonus ({dc})"
    );
    assert!(
        (dk - cfg.proud_tender_knowledge_bonus).abs() < 1e-6,
        "…and its knowledge ({dk})"
    );

    // A merely-content people (below the proud threshold): no lift.
    let (dc_neutral, dk_neutral) = run(0.5);
    assert_eq!(
        (dc_neutral, dk_neutral),
        (0.0, 0.0),
        "a people below the proud threshold tends its module no better than duty"
    );
}
#[test]
fn a_neglected_module_sours_the_people_who_tend_it() {
    // Content-depth subsystems round 8: the people whose craft is a subsystem
    // lose approval each year it sits below the neglect threshold, while a
    // sound module leaves them content — the coupling that lets subsystem
    // neglect feed the round-8 faction withdrawal.
    let (data, mut sim, _picks) = armed(11);
    // The Steel Covenant tend the engineering bay; ensure they are aboard.
    if sim
        .factions
        .iter()
        .all(|f| f.faction_id != "steel_covenant")
    {
        sim.factions.push(fs("steel_covenant", 300));
    }
    let cov_approval = |sim: &SimState| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == "steel_covenant")
            .unwrap()
            .approval
    };

    // A sound engineering bay: the makers stay content year over year.
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.9;
    let before = cov_approval(&sim);
    sim.apply_subsystem_neglect_sentiment(&data);
    assert_eq!(
        cov_approval(&sim),
        before,
        "a well-kept module breeds no grievance"
    );

    // Let the bay rot below the threshold: their approval erodes each year,
    // and only theirs — a faction whose module is fine is untouched.
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.2;
    let gardener_before = sim
        .factions
        .iter()
        .find(|f| f.faction_id == "verdant_kin")
        .map(|f| f.approval);
    sim.apply_subsystem_neglect_sentiment(&data);
    assert!(
        cov_approval(&sim) < before,
        "the makers sour watching their bay rot"
    );
    if let Some(g0) = gardener_before {
        let g1 = sim
            .factions
            .iter()
            .find(|f| f.faction_id == "verdant_kin")
            .unwrap()
            .approval;
        // The gardeners' farm was untouched, so their mood is (unless their
        // own module also happens to be low) unchanged by the bay's rot.
        if sim.subsystems["agriculture"].condition
            >= data.config.factions.neglect_condition_threshold
        {
            assert_eq!(g1, g0, "a people whose module is sound is not soured");
        }
    }
}
#[test]
fn a_module_kept_excellent_pleases_the_people_who_tend_it() {
    // Content-depth factions round 29: the bright mirror of the neglect penalty. The people
    // whose craft is a subsystem gain approval each year it is kept excellent (condition at
    // or above the honor line), while a merely-adequate module leaves them unmoved — the
    // condition→approval *up* direction the neglect coupling (which ran only down) never drew.
    let (data, mut sim, _picks) = armed(12);
    let cfg = &data.config.factions;
    assert!(
        cfg.honored_tender_approval_bonus > 0.0 && cfg.honored_tender_condition_threshold > 0.0,
        "this test needs the honored-tender coupling enabled"
    );
    // The Steel Covenant tend the engineering bay; ensure they are aboard.
    if sim
        .factions
        .iter()
        .all(|f| f.faction_id != "steel_covenant")
    {
        sim.factions.push(fs("steel_covenant", 300));
    }
    let cov_approval = |sim: &SimState| {
        sim.factions
            .iter()
            .find(|f| f.faction_id == "steel_covenant")
            .unwrap()
            .approval
    };

    // A merely-adequate bay (kept, but below the honor line): no pride, no grievance.
    sim.subsystems.get_mut("engineering_bay").unwrap().condition =
        cfg.honored_tender_condition_threshold - 0.1;
    let before = cov_approval(&sim);
    sim.apply_honored_tender_sentiment(&data);
    assert_eq!(
        cov_approval(&sim),
        before,
        "a merely-adequate module wins no pride"
    );

    // An excellent bay: the makers gain approval, by exactly the honor bonus.
    sim.subsystems.get_mut("engineering_bay").unwrap().condition = 1.0;
    let before = cov_approval(&sim);
    sim.apply_honored_tender_sentiment(&data);
    assert!(
        cov_approval(&sim) > before,
        "the makers warm to see their bay kept excellent"
    );
    assert!(
        (cov_approval(&sim) - before - cfg.honored_tender_approval_bonus).abs() < 1e-6,
        "the lift is exactly the honor bonus"
    );
}
