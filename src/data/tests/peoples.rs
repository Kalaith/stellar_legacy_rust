//! The founding peoples and the couplings that move their approval.

use super::*;

/// Six peoples, ideology in [-1, 1], and the relationships the
/// spillover mechanics need in order to be exercised at all.
#[test]
fn the_founding_peoples_are_authored_within_bounds() {
    let data = GameData::load().unwrap();
    let rep_produced = reputation_traits_produced(&data);
    // Content-depth factions round 16: a dominant-faction reputation leaning must
    // name a real trait and be a gentle lean, not a lever.
    for (id, f) in data.factions.iter() {
        for (trait_id, lean) in &f.reputation_leanings {
            assert!(
                rep_produced.contains(trait_id),
                "faction '{id}' leans reputation '{trait_id}' no outcome nudges"
            );
            assert!(
                (-1.0..=1.0).contains(lean),
                "faction '{id}' reputation lean {lean} out of range [-1, 1]"
            );
        }
    }
    // W7: six authored founding factions, ideology within [-1, 1]. The
    // registry keys on id, so a count of six also proves the ids are unique.
    assert_eq!(data.factions.len(), 6, "six founding factions");
    for (id, faction) in data.factions.iter() {
        assert!(
            (-1.0..=1.0).contains(&faction.ideology),
            "faction '{id}' ideology out of range: {}",
            faction.ideology
        );
        // Content-depth faction coverage: every faction has at least one
        // signature event that fires while it runs the ship, so no group is
        // mechanically silent when dominant.
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.requires_dominant_faction == *id),
            "faction '{id}' has no signature (requires_dominant_faction) event"
        );
        // Content-depth round 7: every people brings a distinct recruitment
        // dowry — a personality, not a bare head count — and any subsystem it
        // lifts must be real.
        let boon = &faction.recruit_boon;
        assert!(
            !boon.flavor.trim().is_empty(),
            "faction '{id}' has no recruit_boon flavor"
        );
        for delta in &boon.subsystem_deltas {
            assert!(
                data.subsystems.get(&delta.id).is_some(),
                "faction '{id}' recruit_boon names unknown subsystem '{}'",
                delta.id
            );
        }
        // Content-depth subsystems round 8: the module a people answers for
        // (its neglect erodes their approval) must be a real subsystem.
        assert!(
            faction.tended_subsystem.is_empty()
                || data.subsystems.get(&faction.tended_subsystem).is_some(),
            "faction '{id}' tends unknown subsystem '{}'",
            faction.tended_subsystem
        );
        // Content-depth factions round 21 (the first factions↔voice coupling):
        // every people colors the ordinary quiet in its own voice, so a long calm
        // stretch sounds like whoever runs the ship. No group falls back to the
        // generic ambient — each authors its own.
        assert!(
            faction.ambient.len() >= 2,
            "faction '{id}' has fewer than 2 ambient (quiet-voice) lines"
        );
        for line in &faction.ambient {
            assert!(
                !line.trim().is_empty(),
                "faction '{id}' has a blank ambient line"
            );
        }
        // Content-depth factions round 11: demographic drift is a gentle
        // per-generation share shift, not a population weapon.
        assert!(
            (-0.2..=0.2).contains(&faction.growth_bias),
            "faction '{id}' growth_bias {} out of the gentle range [-0.2, 0.2]",
            faction.growth_bias
        );
        // Content-depth factions round 14: a rival must be a real, other people,
        // and rivalries must be authored *symmetric* (if A names B, B names A) —
        // a one-sided grudge is an authoring slip.
        for rival in &faction.rivals {
            assert_ne!(rival, id, "faction '{id}' lists itself as a rival");
            let other = data
                .factions
                .get(rival)
                .unwrap_or_else(|| panic!("faction '{id}' names unknown rival '{rival}'"));
            assert!(
                other.rivals.contains(id),
                "rivalry '{id}' <-> '{rival}' is not symmetric"
            );
        }
        // Content-depth factions round 17: an ally must likewise be a real, other
        // people; alliances symmetric; and a pair is never both kin and rival —
        // the positive and negative spillover would fight over the same relation.
        for ally in &faction.allies {
            assert_ne!(ally, id, "faction '{id}' lists itself as an ally");
            let other = data
                .factions
                .get(ally)
                .unwrap_or_else(|| panic!("faction '{id}' names unknown ally '{ally}'"));
            assert!(
                other.allies.contains(id),
                "alliance '{id}' <-> '{ally}' is not symmetric"
            );
            assert!(
                !faction.rivals.contains(ally),
                "'{id}' <-> '{ally}' is listed as both ally and rival"
            );
        }
    }
    // Content-depth factions round 14: at least one people should carry a rival,
    // so the spillover mechanic is exercised.
    assert!(
        data.factions.iter().any(|(_, f)| !f.rivals.is_empty()),
        "some faction should have a standing rival"
    );
    // Content-depth factions round 17: and at least one a standing ally, so the
    // positive spillover is exercised too.
    assert!(
        data.factions.iter().any(|(_, f)| !f.allies.is_empty()),
        "some faction should have a standing ally"
    );
    // Content-depth factions round 32: the schadenfreude and commiseration spillovers are
    // fractions of a slight in [0, 1) — a wounded people's rivals share only *part* of the
    // relief and its allies only *part* of the sting, never more than the wound itself.
    assert!(
        (0.0..1.0).contains(&data.config.factions.rival_approval_schadenfreude),
        "rival_approval_schadenfreude {} must be a fraction of the slight [0, 1)",
        data.config.factions.rival_approval_schadenfreude
    );
    assert!(
        (0.0..1.0).contains(&data.config.factions.ally_approval_commiseration),
        "ally_approval_commiseration {} must be a fraction of the slight [0, 1)",
        data.config.factions.ally_approval_commiseration
    );
    // Content-depth factions round 33: the newcomer's wariness/comfort are gentle per-relation
    // shifts to a *starting* approval, in [0, 0.5) — a people boards uneasier or gladder for who
    // is already aboard, but no single incumbent makes or breaks the newcomer's whole standing.
    assert!(
        (0.0..0.5).contains(&data.config.factions.recruit_newcomer_rival_wariness),
        "recruit_newcomer_rival_wariness {} must be a gentle per-rival shift [0, 0.5)",
        data.config.factions.recruit_newcomer_rival_wariness
    );
    assert!(
        (0.0..0.5).contains(&data.config.factions.recruit_newcomer_ally_comfort),
        "recruit_newcomer_ally_comfort {} must be a gentle per-ally shift [0, 0.5)",
        data.config.factions.recruit_newcomer_ally_comfort
    );
    // Content-depth factions round 34: the recruit mercy bonus is a gentle one-shot reputation
    // nudge in [0, 0.5) — welcoming a people warms the ship's name, but no single recruitment
    // makes it a saint.
    assert!(
        (0.0..0.5).contains(&data.config.factions.recruit_reputation_bonus),
        "recruit_reputation_bonus {} must be a gentle reputation nudge [0, 0.5)",
        data.config.factions.recruit_reputation_bonus
    );
    // Content-depth charters round 32: the charter pride/letdown are gentle one-shot approval
    // shifts in [0, 0.5) — a mission's outcome moves the crew's politics, but no single writ
    // makes or breaks a people's whole standing with the ship.
    assert!(
        (0.0..0.5).contains(&data.config.factions.charter_completion_pride),
        "charter_completion_pride {} must be a gentle approval shift [0, 0.5)",
        data.config.factions.charter_completion_pride
    );
    assert!(
        (0.0..0.5).contains(&data.config.factions.charter_failure_letdown),
        "charter_failure_letdown {} must be a gentle approval shift [0, 0.5)",
        data.config.factions.charter_failure_letdown
    );
}

/// Every faction lever is a nudge, never a lever that swamps the rest.
#[test]
fn the_faction_couplings_are_gentle_bounded_nudges() {
    let data = GameData::load().unwrap();
    // Content-depth factions round 22: the proud-tender upkeep is a gentle yearly
    // dividend of a delighted people, at a plausible "delighted" approval band — not
    // a repair crew that rebuilds a module from pride alone.
    let fac_cfg = data.config.factions;
    if fac_cfg.proud_tender_condition_bonus > 0.0 {
        assert!(
            (0.5..1.0).contains(&fac_cfg.proud_tender_approval_threshold),
            "proud_tender_approval_threshold {} should sit in a 'delighted' band [0.5, 1.0)",
            fac_cfg.proud_tender_approval_threshold
        );
        assert!(
            fac_cfg.proud_tender_condition_bonus <= 0.05
                && fac_cfg.proud_tender_knowledge_bonus <= 0.05,
            "proud-tender upkeep must be a gentle yearly dividend, not a rebuild"
        );
    }
    // Content-depth factions round 23: the standing rival/ally cohesion pressures are
    // gentle yearly drifts (scaled further down by the product of two shares), not
    // levers that swing unity in a season.
    assert!(
        fac_cfg.rival_unity_friction <= 0.1 && fac_cfg.ally_unity_solidarity <= 0.1,
        "rival/ally unity pressures must be gentle yearly drifts"
    );
    // Content-depth factions round 24: the departure cohesion scar is a bounded blow
    // (a full-ship secession, share 1.0, must not empty morale/unity in one stroke).
    assert!(
        (0.0..=0.5).contains(&fac_cfg.departure_cohesion_scar),
        "departure_cohesion_scar {} out of range [0, 0.5]",
        fac_cfg.departure_cohesion_scar
    );
    // Content-depth factions round 26: the assimilation unity lift is a gentle
    // consolidation (share-scaled further down), the positive mirror of the scar.
    assert!(
        (0.0..=0.5).contains(&fac_cfg.assimilation_unity_lift),
        "assimilation_unity_lift {} out of range [0, 0.5]",
        fac_cfg.assimilation_unity_lift
    );
    // Content-depth factions round 35: the recruit unity cost is the one-time integration shock,
    // in the same gentle [0, 0.5] band as the assimilation lift it mirrors — a new people dents
    // cohesion, but no single recruitment shatters the crew.
    assert!(
        (0.0..=0.5).contains(&fac_cfg.recruit_unity_cost),
        "recruit_unity_cost {} out of range [0, 0.5]",
        fac_cfg.recruit_unity_cost
    );
}
