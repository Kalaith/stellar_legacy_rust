//! Data-layer tests.

use super::*;

#[test]
fn flavor_lines_rotate_deterministically_and_substitute_name() {
    let pool = vec!["A {name}".to_string(), "B {name}".to_string()];
    // Rotates by index, wraps, and substitutes — no RNG, so a seed replays.
    assert_eq!(
        FlavorConfig::line_with_name(&pool, 0, "Vale").unwrap(),
        "A Vale"
    );
    assert_eq!(
        FlavorConfig::line_with_name(&pool, 1, "Vale").unwrap(),
        "B Vale"
    );
    assert_eq!(
        FlavorConfig::line_with_name(&pool, 2, "Vale").unwrap(),
        "A Vale"
    );
    assert!(FlavorConfig::line_with_name(&[], 0, "Vale").is_none());
}

#[test]
fn homecoming_lines_are_authored_for_every_success_level_and_substitute() {
    // Content-depth voice round 4: every mission outcome the game can log
    // must have homecoming prose, indexed deterministically and with the
    // voyage's length/generation woven in.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for level in ["complete", "partial", "pyrrhic", "failure"] {
        let line = flavor
            .homecoming_line(level, 0, 450, 17)
            .unwrap_or_else(|| panic!("no homecoming prose for '{level}'"));
        assert!(
            line.contains("450") || line.contains("17"),
            "'{level}' homecoming should weave in the voyage's span: {line}"
        );
    }
    // Deterministic rotation by the index, and an unknown level is None.
    let a = flavor.homecoming_line("complete", 0, 300, 10);
    let b = flavor.homecoming_line("complete", 0, 300, 10);
    assert_eq!(a, b, "same index replays the same line");
    assert!(flavor.homecoming_line("triumphant", 0, 300, 10).is_none());
}

#[test]
fn generational_turnover_voice_is_authored_and_varies() {
    // Content-depth voice round 5: the crew-retirement line fires several
    // times a generation, so its pool must have real variety (not a
    // repetition tell); the extinction ending must have authored prose too.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    assert!(
        flavor.retirement.len() >= 4,
        "the retirement pool needs variety — it fires several times a generation"
    );
    assert!(
        !flavor.extinction.is_empty(),
        "the line-ends ending needs prose"
    );
    // Consecutive retirements (the same generation) draw different lines.
    let a = FlavorConfig::line_with_name(&flavor.retirement, 0, "Vale").unwrap();
    let b = FlavorConfig::line_with_name(&flavor.retirement, 1, "Vale").unwrap();
    assert_ne!(
        a, b,
        "two stand-downs in one generation must read differently"
    );
    assert!(a.contains("Vale"), "the retiring holder's name is woven in");
}

#[test]
fn crew_appointment_and_training_voice_is_authored_and_varies() {
    // Content-depth voice round 7: the appointment line (the positive twin of
    // retirement) and the training line both fire repeatedly as a roster is
    // re-crewed over the centuries, so both need pool variety and must weave
    // in the officer's name and human post — not the raw archetype id.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    assert!(
        flavor.appointment.len() >= 4,
        "the appointment pool needs variety — posts turn over all voyage"
    );
    assert!(
        flavor.training.len() >= 3,
        "the training pool needs variety — training is a repeatable verb"
    );
    // Two appointments in one drydock draw different lines, and both weave in
    // the officer's name and the human post name.
    let a = FlavorConfig::line_with_name_post(&flavor.appointment, 0, "Vale", "Chief Engineer")
        .unwrap();
    let b = FlavorConfig::line_with_name_post(&flavor.appointment, 1, "Vale", "Chief Engineer")
        .unwrap();
    assert_ne!(a, b, "two appointments must read differently");
    assert!(
        a.contains("Vale") && a.contains("Chief Engineer"),
        "the appointee's name and human post are woven in"
    );
    // The training pool carries the skill placeholder.
    assert!(
        flavor.training.iter().any(|s| s.contains("{skill}")),
        "training lines surface the new skill"
    );
}

#[test]
fn faction_mood_voice_is_authored_and_names_the_people() {
    // Content-depth voice round 8: the approval meter's voice. A people
    // crossing into restlessness or contentment gets a pooled line, so both
    // pools need variety and must weave in the people's name.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for (pool, label) in [
        (&flavor.faction_souring, "souring"),
        (&flavor.faction_warming, "warming"),
    ] {
        assert!(pool.len() >= 3, "the faction {label} pool needs variety");
        assert!(
            pool.iter().all(|s| s.contains("{name}")),
            "every faction {label} line must name the people"
        );
        let a = FlavorConfig::line_with_name(pool, 0, "the Keepers").unwrap();
        let b = FlavorConfig::line_with_name(pool, 1, "the Keepers").unwrap();
        assert_ne!(a, b, "consecutive {label} lines must differ");
        assert!(
            a.contains("the Keepers"),
            "the {label} line names the people"
        );
    }
}

#[test]
fn ship_mood_voice_is_authored_and_varies() {
    // Content-depth voice round 11: the ship's collective morale crossing into
    // a grim or a buoyant band draws a pooled ambient line — the ship-wide twin
    // of the faction-mood voice. No name to weave, but both pools need variety.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for (pool, label) in [
        (&flavor.ship_mood_darkening, "darkening"),
        (&flavor.ship_mood_lifting, "lifting"),
    ] {
        assert!(pool.len() >= 3, "the ship-mood {label} pool needs variety");
        let a = FlavorConfig::line_with_name(pool, 0, "").unwrap();
        let b = FlavorConfig::line_with_name(pool, 1, "").unwrap();
        assert_ne!(a, b, "consecutive ship-mood {label} lines must differ");
    }
}

#[test]
fn subsystem_maintenance_voice_is_authored_and_names_the_module() {
    // Content-depth voice round 9: the field-repair and knowledge-training
    // verbs fire repeatedly across a voyage, so both pools need variety and
    // must weave in the module name.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for (pool, label) in [
        (&flavor.subsystem_repair, "repair"),
        (&flavor.subsystem_training, "training"),
    ] {
        assert!(pool.len() >= 3, "the subsystem {label} pool needs variety");
        assert!(
            pool.iter().all(|s| s.contains("{name}")),
            "every subsystem {label} line must name the module"
        );
        let a = FlavorConfig::line_with_name(pool, 0, "engineering bay").unwrap();
        let b = FlavorConfig::line_with_name(pool, 1, "engineering bay").unwrap();
        assert_ne!(a, b, "consecutive {label} lines must differ");
        assert!(a.contains("engineering bay"), "the {label} line names it");
    }
}

#[test]
fn embedded_data_loads() {
    let data = GameData::load().unwrap();

    assert_eq!(data.config.game_name, "stellar_legacy");
    assert_eq!(data.legacies.len(), 3);
    assert!(data.events.len() >= 4);
    assert!(
        data.contracts.len() >= 10,
        "§8 target was 6-8 contracts; the pool has since grown"
    );
    // Charter tiering (PLAN M4.8): some charters gate behind renown, some
    // are available from the founding.
    assert!(
        data.contracts.iter().any(|(_, c)| c.min_renown > 0),
        "some charters should unlock with renown"
    );
    assert!(
        data.contracts.iter().any(|(_, c)| c.min_renown == 0),
        "some charters should be available from the founding"
    );
    // W1-rescale: every charter is now a generational voyage (>= 300 yr).
    // W2: authored phases sum exactly to the duration, only Travel/Operation/
    // Return kinds, at least one Operation segment, and a real objective.
    use contracts::ContractPhase;
    for (id, c) in data.contracts.iter() {
        assert!(
            c.target_duration_years >= 300,
            "charter '{id}' must be a generational voyage (>= 300 yr), is {}",
            c.target_duration_years
        );
        let phase_years: u32 = c.phases.iter().map(|p| p.years).sum();
        assert_eq!(
            phase_years, c.target_duration_years,
            "charter '{id}' phase years {phase_years} must sum to its duration {}",
            c.target_duration_years
        );
        for phase in &c.phases {
            assert!(
                matches!(
                    phase.kind,
                    ContractPhase::Travel | ContractPhase::Operation | ContractPhase::Return
                ),
                "charter '{id}' has an invalid authored phase kind {:?}",
                phase.kind
            );
        }
        assert!(
            c.phases.iter().any(|p| p.kind == ContractPhase::Operation),
            "charter '{id}' must have at least one Operation segment"
        );
        assert!(
            c.objective_target > 0.0,
            "charter '{id}' must have a positive objective target"
        );
    }
    // Salvage pool (PLAN M4.4): several event outcomes drop a found part,
    // and every granted id must resolve to a real component.
    let salvage_grants: Vec<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .filter_map(|o| o.grant_component.as_ref())
        .collect();
    assert!(
        salvage_grants.len() >= 4,
        "expected >= 4 salvage-granting outcomes, found {}",
        salvage_grants.len()
    );
    for id in salvage_grants {
        assert!(
            data.ship_components.find_any(id).is_some(),
            "event grant_component '{id}' must be a real ship component"
        );
    }
    // Mission-reward parts are never sold, so a price on one is dead data —
    // and a part nobody can buy that no mission grants is unreachable. Collect
    // every granted id (event outcomes + charter completions) and check that
    // each mission-only part is reachable, and that at least one exists.
    let granted_ids: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .filter_map(|o| o.grant_component.as_ref())
        .chain(
            data.contracts
                .iter()
                .filter_map(|(_, c)| c.completion_reward.grant_component.as_ref()),
        )
        .collect();
    let mut mission_only_parts = 0;
    for kind in [
        ship_components::ComponentKind::Hull,
        ship_components::ComponentKind::Engine,
        ship_components::ComponentKind::Weapon,
    ] {
        for component in data.ship_components.list(kind) {
            if !component.acquisition.is_mission_only() {
                continue;
            }
            mission_only_parts += 1;
            assert!(
                component.cost == crate::data::ResourceDelta::default(),
                "mission-reward part '{}' carries a price but can never be bought",
                component.id
            );
            assert!(
                granted_ids.contains(&component.id),
                "mission-reward part '{}' is granted by no mission — it is unreachable",
                component.id
            );
        }
    }
    assert!(
        mission_only_parts >= 1,
        "expected at least one mission-reward ship part, found none"
    );
    // The subsystem-version twin (2c): every mission-reward fitting must be
    // reachable — granted by some mission — and every `grant_fitting` must name
    // a real mission-reward version.
    let granted_fittings: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .filter_map(|o| o.grant_fitting.as_ref())
        .chain(
            data.contracts
                .iter()
                .filter_map(|(_, c)| c.completion_reward.grant_fitting.as_ref()),
        )
        .collect();
    let mut mission_fittings: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (sid, sub) in data.subsystems.iter() {
        for tier in &sub.tiers {
            if tier.acquisition.is_mission_only() {
                mission_fittings.insert(tier.id.as_str());
                assert!(
                    granted_fittings.contains(&tier.id),
                    "mission-reward version '{}' on subsystem '{sid}' is granted by no mission",
                    tier.id
                );
            }
        }
    }
    assert!(
        !mission_fittings.is_empty(),
        "expected at least one mission-reward subsystem version, found none"
    );
    for gf in &granted_fittings {
        assert!(
            mission_fittings.contains(gf.as_str()),
            "grant_fitting '{gf}' is not a real mission-reward subsystem version"
        );
    }
    // Content-depth charter↔event coupling: every charter-tag an event gates
    // on must exist on at least one charter, or the event can never fire.
    let charter_tags: std::collections::HashSet<&String> = data
        .contracts
        .iter()
        .flat_map(|(_, c)| c.tags.iter())
        .collect();
    for (id, e) in data.events.iter() {
        for tag in &e.requires_charter_tag {
            assert!(
                charter_tags.contains(tag),
                "event '{id}' requires charter tag '{tag}' no charter carries"
            );
        }
        // Content-depth faction↔event coupling: every faction an event gates
        // on must be a real, authored faction.
        for fid in std::iter::once(&e.requires_dominant_faction)
            .filter(|f| !f.is_empty())
            .chain(e.requires_factions_aboard.iter())
            .chain(e.outcomes.iter().filter_map(|o| o.faction_loss_id.as_ref()))
            .chain(
                e.outcomes
                    .iter()
                    .filter_map(|o| o.faction_merge_id.as_ref()),
            )
            // Content-depth round 6: complication faction gates too.
            .chain(
                e.complications
                    .iter()
                    .map(|c| &c.requires_dominant_faction)
                    .filter(|f| !f.is_empty()),
            )
            // Content-depth factions round 25: outcome-level dominant-faction gates.
            .chain(
                e.outcomes
                    .iter()
                    .map(|o| &o.requires.requires_dominant_faction)
                    .filter(|f| !f.is_empty()),
            )
            .chain(
                e.complications
                    .iter()
                    .flat_map(|c| c.requires_factions_aboard.iter()),
            )
            // Content-depth round 8/19: approval gate (both poles) + delta ids.
            .chain(e.faction_approval_below.iter().map(|g| &g.id))
            .chain(e.faction_approval_above.iter().map(|g| &g.id))
            .chain(
                e.outcomes
                    .iter()
                    .flat_map(|o| o.faction_approval_deltas.iter().map(|d| &d.id)),
            )
        {
            assert!(
                data.factions.get(fid).is_some(),
                "event '{id}' references unknown faction '{fid}'"
            );
        }
        // Content-depth subsystem↔event coupling: knowledge gates and
        // outcome subsystem deltas must name real subsystems.
        for sid in e
            .knowledge_below
            .iter()
            .map(|g| &g.id)
            .chain(e.condition_below.iter().map(|g| &g.id))
            .chain(
                e.outcomes
                    .iter()
                    .flat_map(|o| o.subsystem_deltas.iter().map(|d| &d.id)),
            )
            // Content-depth round 12: outcome availability gates name
            // subsystems in their knowledge floors.
            .chain(
                e.outcomes
                    .iter()
                    .flat_map(|o| o.requires.min_knowledge.iter().map(|f| &f.id)),
            )
            // Content-depth round 6: complication gates and deltas name
            // subsystems too.
            .chain(e.complications.iter().flat_map(|c| {
                c.condition_below
                    .iter()
                    .map(|g| &g.id)
                    .chain(c.subsystem_deltas.iter().map(|d| &d.id))
            }))
        {
            assert!(
                data.subsystems.get(sid).is_some(),
                "event '{id}' references unknown subsystem '{sid}'"
            );
        }
    }
    // Content-depth consequence chains: every tag a payoff event gates on
    // (`requires_consequence`) must be produced by some outcome's
    // `long_term_consequences`, or the payoff can never fire (typo guard).
    let produced: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .flat_map(|o| o.long_term_consequences.iter())
        .collect();
    for (id, e) in data.events.iter() {
        for tag in e
            .requires_consequence
            .iter()
            .chain(
                e.complications
                    .iter()
                    .flat_map(|c| c.requires_consequence.iter()),
            )
            // Content-depth round 12: outcome availability gates on a
            // consequence too.
            .chain(
                e.outcomes
                    .iter()
                    .flat_map(|o| o.requires.requires_consequence.iter()),
            )
            // Content-depth round 13: the negative gate names consequences too.
            .chain(e.forbidden_consequence.iter())
        {
            assert!(
                produced.contains(tag),
                "event '{id}' gates on consequence '{tag}' no outcome records"
            );
        }
    }
    // Content-depth round 16: a reputation gate must name a trait some outcome
    // actually nudges, or the ship could never build past its neutral 0.5 to
    // meet it (typo guard).
    let rep_produced: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .flat_map(|o| o.reputation_deltas.iter().map(|r| &r.id))
        // Content-depth round 17: a charter's completion also nudges reputation.
        .chain(
            data.contracts
                .iter()
                .flat_map(|(_, c)| c.completion_reward.reputation_deltas.iter().map(|r| &r.id)),
        )
        // Content-depth round 18: and its abandonment marks the ship's name too.
        .chain(
            data.contracts
                .iter()
                .flat_map(|(_, c)| c.abandonment.reputation_deltas.iter().map(|r| &r.id)),
        )
        .collect();
    for (id, e) in data.events.iter() {
        for gate in e
            .min_reputation
            .iter()
            .chain(e.max_reputation.iter())
            // Content-depth round 17: outcome availability gates on reputation too.
            .chain(e.outcomes.iter().flat_map(|o| {
                o.requires
                    .min_reputation
                    .iter()
                    .chain(o.requires.max_reputation.iter())
            }))
            // Content-depth round 22: and a complication can gate on the ship's name.
            .chain(
                e.complications
                    .iter()
                    .flat_map(|c| c.min_reputation.iter().chain(c.max_reputation.iter())),
            )
        {
            assert!(
                rep_produced.contains(&gate.id),
                "event '{id}' gates on reputation '{}' no outcome nudges",
                gate.id
            );
        }
    }
    // Content-depth charters round 16: charter reputation gates name a real trait too.
    for (id, c) in data.contracts.iter() {
        for gate in c.min_reputation.iter().chain(c.max_reputation.iter()) {
            assert!(
                rep_produced.contains(&gate.id),
                "charter '{id}' gates on reputation '{}' no outcome nudges",
                gate.id
            );
        }
    }
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
    // Content-depth round 14: a complication that targets specific choices must
    // name real outcomes of its own event (typo guard), or the toll could never
    // land.
    for (id, e) in data.events.iter() {
        let outcome_ids: std::collections::HashSet<&String> =
            e.outcomes.iter().map(|o| &o.id).collect();
        for c in &e.complications {
            for oid in &c.applies_to_outcomes {
                assert!(
                    outcome_ids.contains(oid),
                    "event '{id}' complication '{}' targets unknown outcome '{oid}'",
                    c.id
                );
            }
        }
    }
    // Content-depth round 12: the first outcome of every event must be
    // unconditional, so a ship is never left with no legal choice and the
    // auto-resolve/index-0 contract always lands on an available outcome.
    for (id, e) in data.events.iter() {
        if let Some(first) = e.outcomes.first() {
            assert!(
                first.requires.is_unconditional(),
                "event '{id}' outcome 0 must be unconditional (gated outcomes come after)"
            );
        }
    }
    // Content-depth round 9: every scheduled follow-up must name a real event
    // (typo guard), and that target should be `scheduled_only` so the timed
    // payoff never also leaks into the reactive pool.
    for (id, e) in data.events.iter() {
        for followup in e
            .outcomes
            .iter()
            .filter_map(|o| o.schedule_followup.as_ref())
        {
            let target = data.events.get(&followup.template_id);
            assert!(
                target.is_some(),
                "event '{id}' schedules unknown follow-up '{}'",
                followup.template_id
            );
            assert!(
                target.unwrap().scheduled_only,
                "scheduled follow-up '{}' must be marked scheduled_only",
                followup.template_id
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

    // W5: six subsystems load; each non-empty buffered family is one of the
    // canonical W6 family strings; tiers are well-formed (3, positive cost).
    let canonical_families: std::collections::HashSet<&str> = [
        "exploration_first_contact",
        "diplomacy",
        "engineering",
        "biology_medical",
        "science_anomaly",
        "survival",
        "mystery",
        "comedy",
        "ethics",
        "legacy_drift",
    ]
    .into_iter()
    .collect();
    assert_eq!(data.subsystems.len(), 6, "six ship subsystems");
    for (id, sub) in data.subsystems.iter() {
        if !sub.buffers_family.is_empty() {
            assert!(
                canonical_families.contains(sub.buffers_family.as_str()),
                "subsystem '{id}' buffers a non-canonical family '{}'",
                sub.buffers_family
            );
        }
        // Three bought upgrade tiers, optionally topped by mission-reward
        // versions (2c): the purchasable ladder is exactly three, and any
        // extra tiers above it must be mission rewards.
        assert!(
            sub.tiers.len() >= 3,
            "subsystem '{id}' needs three bought upgrade tiers"
        );
        // Named-version pass: the baseline and every fitting carry a name, and
        // fitting ids are unique within the subsystem (a mission's
        // `grant_fitting` addresses them by id).
        assert!(
            !sub.baseline_name.trim().is_empty(),
            "subsystem '{id}' has no baseline_name"
        );
        let mut fitting_ids = std::collections::HashSet::new();
        for (ti, tier) in sub.tiers.iter().enumerate() {
            let mission_reward = tier.acquisition.is_mission_only();
            // The first three rungs are bought (positive cost); mission-reward
            // versions sit above them and are free (the mission is the price).
            if mission_reward {
                assert!(
                    ti >= 3,
                    "subsystem '{id}' mission-reward version '{}' must sit above the three bought tiers",
                    tier.id
                );
                assert!(
                    tier.cost == crate::data::ResourceDelta::default(),
                    "subsystem '{id}' mission-reward version '{}' carries a price but is never bought",
                    tier.id
                );
            } else {
                assert!(
                    ti < 3,
                    "subsystem '{id}' has a bought tier above the three-rung ladder",
                );
                assert!(
                    tier.cost.credits > 0,
                    "subsystem '{id}' tier cost must be positive"
                );
            }
            // Content-depth subsystems round 5: every tier carries its own
            // upgrade prose, so a rebuild never falls back to the generic
            // shared line.
            assert!(
                !tier.flavor.trim().is_empty(),
                "subsystem '{id}' has a tier with no upgrade flavor"
            );
            assert!(
                !tier.name.trim().is_empty(),
                "subsystem '{id}' has a fitting with no name"
            );
            assert!(
                fitting_ids.insert(tier.id.as_str()),
                "subsystem '{id}' has a duplicate fitting id '{}'",
                tier.id
            );
        }
        // Content-depth subsystem coverage: every subsystem has at least one
        // knowledge-crisis event, so a module's know-how decaying always has
        // a beat to surface.
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.knowledge_below.iter().any(|g| &g.id == id)),
            "subsystem '{id}' has no knowledge_below crisis event"
        );
        // Content-depth subsystem coverage (round 4): and at least one
        // condition-breakdown event, so a module physically rotting always
        // has a beat to surface — the parallel to the knowledge crisis above.
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.condition_below.iter().any(|g| &g.id == id)),
            "subsystem '{id}' has no condition_below breakdown event"
        );
    }
    // Content-depth subsystems round 21: the security crisis-mitigation is a gentle
    // positive dampener (a corps quiets danger, never conjures it), and its floor
    // keeps the crisis category from being dampened out of existence.
    let subs_cfg = &data.config.subsystems;
    assert!(
        (0.0..=0.3).contains(&subs_cfg.security_crisis_mitigation),
        "security_crisis_mitigation {} out of the gentle range [0, 0.3]",
        subs_cfg.security_crisis_mitigation
    );
    // Content-depth subsystems round 22: the cultural morale swing is gentle, like
    // the habitat one it mirrors — a pillar of spirits, not a lever that swamps them.
    assert!(
        (0.0..=0.05).contains(&subs_cfg.education_morale_swing),
        "education_morale_swing {} out of the gentle range [0, 0.05]",
        subs_cfg.education_morale_swing
    );
    // Content-depth subsystems round 23: the medical renewal penalty is a fraction
    // (a failing bay loses *some* of the young, never all — a collapsed infirmary
    // must not zero out births).
    assert!(
        (0.0..=0.8).contains(&subs_cfg.medical_renewal_penalty),
        "medical_renewal_penalty {} out of range [0, 0.8]",
        subs_cfg.medical_renewal_penalty
    );
    // Content-depth subsystems round 24: the engineering→hull decay penalty is a
    // bounded acceleration (a wrecked bay wears the hull faster, but not so much it
    // shatters a sound ship in a season).
    assert!(
        (0.0..=1.5).contains(&subs_cfg.engineering_hull_decay_penalty),
        "engineering_hull_decay_penalty {} out of range [0, 1.5]",
        subs_cfg.engineering_hull_decay_penalty
    );
    // Content-depth subsystems round 26: the engineering→fabrication penalty is a
    // fraction in [0, 1] (a wrecked bay fabricates less, but never a negative yield;
    // 1 means a fully wrecked bay fabricates nothing beyond the one-part floor).
    assert!(
        (0.0..=1.0).contains(&subs_cfg.engineering_fabrication_penalty),
        "engineering_fabrication_penalty {} out of range [0, 1]",
        subs_cfg.engineering_fabrication_penalty
    );
    // Content-depth subsystems round 34: the field-repair penalty is a fraction in [0, 1] — a
    // fully wrecked bay makes the weakest field mend (at 1, only the residual gain), but even it
    // patches something, and a sound bay always makes a full one.
    assert!(
        (0.0..=1.0).contains(&subs_cfg.engineering_field_repair_penalty),
        "engineering_field_repair_penalty {} out of range [0, 1]",
        subs_cfg.engineering_field_repair_penalty
    );
    // Content-depth subsystems round 30: the engineering→fuel-regen penalty is a fraction in
    // [0, 1] (a wrecked bay scoops less, but never a negative — fuel regen floors at zero).
    assert!(
        (0.0..=1.0).contains(&subs_cfg.engineering_fuel_regen_penalty),
        "engineering_fuel_regen_penalty {} out of range [0, 1]",
        subs_cfg.engineering_fuel_regen_penalty
    );
    // Content-depth subsystems round 27: the education→training penalty must sit strictly
    // below 1, so even a wrecked academy still teaches something and a crew can bootstrap
    // its schools back — a training deadlock (0 gain forever) would be unrecoverable.
    assert!(
        (0.0..1.0).contains(&subs_cfg.education_training_penalty),
        "education_training_penalty {} must be in [0, 1) so a wrecked academy still teaches",
        subs_cfg.education_training_penalty
    );
    // Content-depth subsystems round 28: the security→ideology-spread relief must sit
    // strictly below 1, so even a perfect corps only softens the strain of a divided polity
    // and never wholly cancels the governance drain a genuine split inflicts.
    assert!(
        (0.0..1.0).contains(&subs_cfg.ideology_spread_security_relief),
        "ideology_spread_security_relief {} must be in [0, 1) so a corps never fully cancels the drain",
        subs_cfg.ideology_spread_security_relief
    );
    // Content-depth subsystems round 32: the rival-friction relief is a fraction in [0, 1) —
    // even a perfect corps only softens a standing rivalry, never abolishes it.
    assert!(
        (0.0..1.0).contains(&subs_cfg.security_rival_friction_relief),
        "security_rival_friction_relief {} must be in [0, 1) so a corps never fully cancels the grind",
        subs_cfg.security_rival_friction_relief
    );
    // Content-depth subsystems round 25: the medical adaptation resistance is a
    // fraction below 1 (even a perfect infirmary only *slows* the shipborn drift, it
    // never wholly stops the bodies adapting to the ship).
    assert!(
        (0.0..1.0).contains(&data.config.voyage_drift.medical_adaptation_resistance),
        "medical_adaptation_resistance {} must be a fraction in [0, 1)",
        data.config.voyage_drift.medical_adaptation_resistance
    );
    // Content-depth subsystems round 29: the agriculture adaptation resistance, the
    // biosphere twin, is likewise a fraction below 1 (a living farm only *slows* the drift).
    assert!(
        (0.0..1.0).contains(&data.config.voyage_drift.agriculture_adaptation_resistance),
        "agriculture_adaptation_resistance {} must be a fraction in [0, 1)",
        data.config.voyage_drift.agriculture_adaptation_resistance
    );
    // Content-depth subsystems round 31: the medical life-support relief is a fraction below 1
    // (even a perfect infirmary only *saves some* of the asphyxiating; it cannot make air).
    assert!(
        (0.0..1.0).contains(&subs_cfg.medical_life_support_relief),
        "medical_life_support_relief {} must be a fraction in [0, 1)",
        subs_cfg.medical_life_support_relief
    );
    // Content-depth charters round 33: mission-training is a *small* per-month knowledge gain in
    // [0, 0.1) — a mission runs many months, so even a modest rate masters a craft over a long
    // voyage without a single month vaulting the subsystem to expert.
    assert!(
        (0.0..0.1).contains(&subs_cfg.objective_subsystem_training_per_month),
        "objective_subsystem_training_per_month {} must be a small monthly gain [0, 0.1)",
        subs_cfg.objective_subsystem_training_per_month
    );
    // Content-depth subsystems round 33: the knowledge-upkeep reduction is a fraction in [0, 1)
    // — a mastered module decays slower, but even perfect craft only slows the rot, never stops
    // it (a full 1.0 would make a well-known module immortal).
    assert!(
        (0.0..1.0).contains(&subs_cfg.knowledge_decay_reduction),
        "knowledge_decay_reduction {} must be a fraction in [0, 1)",
        subs_cfg.knowledge_decay_reduction
    );
    if subs_cfg.security_crisis_mitigation > 0.0 {
        assert!(
            subs_cfg.crisis_weight_floor > 0.0,
            "a crisis-weight floor must be set so security can never silence danger"
        );
    }

    assert_eq!(data.ship_components.hulls.len(), 6);
    assert_eq!(data.ship_components.engines.len(), 6);
    assert_eq!(data.ship_components.weapons.len(), 6);
    assert_eq!(data.crew_archetypes.len(), 7);
    // Doubled name pools (§8): 50 given names, 20 surnames + 10 traits
    // per legacy.
    assert!(data.dynasty_names.given_names.len() >= 50);
    for legacy_id in ["preservers", "adaptors", "wanderers"] {
        assert!(data.legacies.contains(legacy_id));
        let surnames = &data.dynasty_names.surnames_by_legacy[legacy_id];
        let traits = &data.dynasty_names.traits_by_legacy[legacy_id];
        assert!(
            surnames.len() >= 20,
            "{legacy_id} surnames: {}",
            surnames.len()
        );
        assert!(traits.len() >= 10, "{legacy_id} traits: {}", traits.len());
        // Each legacy carries its defining dilemmas (§8 target 6; the
        // pool has since been deepened past it).
        let legacy = data.legacies.get(legacy_id).unwrap();
        assert!(
            legacy.dilemmas.len() >= 8,
            "{legacy_id} should have >= 8 dilemmas, has {}",
            legacy.dilemmas.len()
        );
        // Content-depth factions round 10: a dilemma option's faction-odds
        // modifier must name a real faction.
        for dil in &legacy.dilemmas {
            for opt in &dil.options {
                assert!(
                    opt.dominant_faction.is_empty()
                        || data.factions.get(&opt.dominant_faction).is_some(),
                    "dilemma '{}' option '{}' names unknown faction '{}'",
                    dil.id,
                    opt.id,
                    opt.dominant_faction
                );
            }
        }
    }
}

#[test]
fn event_categories_all_represented() {
    use events::EventCategory::*;
    let data = GameData::load().unwrap();
    for category in [
        ImmediateCrisis,
        GenerationalChallenge,
        MissionMilestone,
        LegacyMoment,
    ] {
        // Every category is well represented (§8 M3 target is 30+ total).
        let count = data
            .events
            .iter()
            .filter(|(_, e)| e.category == category)
            .count();
        assert!(
            count >= 11,
            "expected >= 11 event templates for {category:?}, found {count}"
        );
    }
    // §8 M3 target is 30+; the pool has since grown well past it.
    assert!(
        data.events.len() >= 46,
        "expected >= 46 event templates, found {}",
        data.events.len()
    );
    // Content-depth campaign-skeleton coupling: every family a beat pool can
    // draw must have authored events, or a beat could land on an empty pool.
    let families: std::collections::HashSet<&String> =
        data.events.iter().map(|(_, e)| &e.family).collect();
    let sk = &data.config.campaign_skeleton;
    for fam in sk
        .travel_pool
        .iter()
        .chain(&sk.operation_pool)
        .chain(&sk.return_pool)
        .chain(&sk.any_pool)
        .chain(&sk.early_pool)
        .chain(&sk.mid_pool)
        .chain(&sk.late_pool)
        .chain(&sk.dead_air_pool)
    {
        assert!(
            families.contains(fam),
            "campaign_skeleton pool family '{fam}' has no events"
        );
    }
    // Content-depth charters round 7: a charter's beat-pool bias must name
    // real families, or a biased beat could land on an empty pool. At least
    // one charter must carry a bias, so the mechanic stays exercised.
    assert!(
        data.contracts
            .iter()
            .any(|(_, c)| !c.beat_families.is_empty()),
        "some charter should bias its seeded skeleton via beat_families"
    );
    for (id, c) in data.contracts.iter() {
        for fam in &c.beat_families {
            assert!(
                families.contains(fam),
                "charter '{id}' beat_families '{fam}' has no events"
            );
        }
        // Content-depth charters round 9: a scripted timed beat must name a
        // real, scheduled_only event, and the beats must ascend by year so
        // they fire in order.
        for beat in &c.scheduled_beats {
            let target = data.events.get(&beat.template_id);
            assert!(
                target.is_some_and(|e| e.scheduled_only),
                "charter '{id}' scheduled beat '{}' must be a scheduled_only event",
                beat.template_id
            );
        }
        assert!(
            c.scheduled_beats
                .windows(2)
                .all(|w| w[0].at_year <= w[1].at_year),
            "charter '{id}' scheduled_beats must ascend by at_year"
        );
        // Content-depth charters round 11: route hazard is a sane weight bump.
        assert!(
            (0.0..=1.0).contains(&c.hazard),
            "charter '{id}' hazard {} out of range [0, 1]",
            c.hazard
        );
        // Content-depth charters round 12: an in-world availability gate must
        // name real founding peoples, or the writ could never be offered.
        for fid in &c.requires_faction_aboard {
            assert!(
                data.factions.get(fid).is_some(),
                "charter '{id}' requires unknown faction '{fid}' aboard"
            );
        }
        // Content-depth charters round 19: a completion goodwill reward must name
        // a real people, or the goodwill would land nowhere.
        for d in &c.completion_reward.faction_approval_deltas {
            assert!(
                data.factions.get(&d.id).is_some(),
                "charter '{id}' completion_reward names unknown faction '{}'",
                d.id
            );
        }
        // Content-depth charters round 20: a completion component reward must name
        // a real ship component, or the salvage hold gains a phantom.
        if let Some(comp) = &c.completion_reward.grant_component {
            assert!(
                data.ship_components.find_any(comp).is_some(),
                "charter '{id}' completion_reward grant_component '{comp}' is not a real component"
            );
        }
        // Content-depth charters round 13: a route toll must be a gentle,
        // survivable headwind — a per-year crew drain that could empty a
        // generational voyage is a bug, not a hazard.
        assert!(
            c.annual_toll.population.count.abs() <= 3,
            "charter '{id}' annual_toll drains {} crew/yr — too steep for a voyage",
            c.annual_toll.population.count
        );
        // Content-depth subsystems round 14: the module a mission leans on must
        // be a real subsystem, or its condition could never scale the work.
        assert!(
            c.objective_subsystem.is_empty()
                || data.subsystems.get(&c.objective_subsystem).is_some(),
            "charter '{id}' objective_subsystem names unknown module '{}'",
            c.objective_subsystem
        );
        // Content-depth charters round 21: a mission's combat scaling is a
        // positive accelerator (firepower quickens contested work, never slows
        // it) and gentle — an over-steep value would make the drydock's guns the
        // only thing that matters. Bounded like the speed lever's reach.
        assert!(
            (0.0..=0.2).contains(&c.objective_combat_scaling),
            "charter '{id}' objective_combat_scaling {} out of range [0, 0.2]",
            c.objective_combat_scaling
        );
        // Content-depth charters round 24: cargo scaling is a small per-unit rate
        // (cargo counts in the hundreds, not the single digits combat does), so its
        // ceiling is far lower — a big hold helps a haul, it does not dominate it.
        assert!(
            (0.0..=0.01).contains(&c.objective_cargo_scaling),
            "charter '{id}' objective_cargo_scaling {} out of range [0, 0.01]",
            c.objective_cargo_scaling
        );
        // Content-depth charters round 26: loadout gates are non-negative minimums.
        assert!(
            c.min_combat >= 0 && c.min_cargo >= 0 && c.min_speed >= 0,
            "charter '{id}' has a negative loadout requirement"
        );
        // Content-depth charters round 29: a reputation-scaled reward must name a positive
        // scale (a trait with a zero scale is a dead field), and the scale is gentle so a
        // name is worth a premium but never multiplies or erases the pay outright.
        if !c.reward_reputation_trait.is_empty() {
            assert!(
                (0.0..=1.0).contains(&c.reward_reputation_scale) && c.reward_reputation_scale > 0.0,
                "charter '{id}' names a reward reputation trait but its scale {} is not in (0, 1]",
                c.reward_reputation_scale
            );
        }
        // Content-depth charters round 23: a preserve charter must actually erode
        // (a positive, gentle yearly attrition), or "keep the cargo" is a free win.
        if c.preserve_objective {
            assert!(
                c.preserve_attrition_per_year > 0.0 && c.preserve_attrition_per_year <= 0.01,
                "charter '{id}' preserve_attrition_per_year {} must be a gentle positive rate",
                c.preserve_attrition_per_year
            );
        }
        // Content-depth charters round 15: a completion reward's subsystem boons
        // must name real modules, or the legacy could never land.
        for delta in &c.completion_reward.subsystem_deltas {
            assert!(
                data.subsystems.get(&delta.id).is_some(),
                "charter '{id}' completion_reward names unknown module '{}'",
                delta.id
            );
        }
    }
    // Content-depth charters round 13: at least one charter should carry a
    // standing route toll, so the mechanic is exercised.
    assert!(
        data.contracts.iter().any(|(_, c)| !c.annual_toll.is_none()),
        "some charter should exact a per-year route toll"
    );
    // Content-depth charters round 21: at least one charter should reward
    // firepower (a contested writ worked faster by an armed ship), so the
    // charter↔combat coupling is actually exercised.
    assert!(
        data.contracts
            .iter()
            .any(|(_, c)| c.objective_combat_scaling > 0.0),
        "some charter should let combat quicken its objective"
    );
    // Content-depth charters round 14: a charter's deed gates must name a
    // consequence *something* produces — an event outcome or another charter's
    // completion — or the writ (or its bar) could never resolve (typo guard).
    let charter_produced: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .flat_map(|o| o.long_term_consequences.iter())
        .chain(
            data.contracts
                .iter()
                .filter(|(_, c)| !c.completion_consequence.is_empty())
                .map(|(_, c)| &c.completion_consequence),
        )
        // Content-depth charters round 30: a *failed* charter's deed-mark is a producer too.
        .chain(
            data.contracts
                .iter()
                .filter(|(_, c)| !c.failure_consequence.is_empty())
                .map(|(_, c)| &c.failure_consequence),
        )
        .collect();
    for (id, c) in data.contracts.iter() {
        for tag in c
            .requires_consequence
            .iter()
            .chain(c.forbidden_consequence.iter())
        {
            assert!(
                charter_produced.contains(tag),
                "charter '{id}' gates on consequence '{tag}' nothing records"
            );
        }
    }
    // Content-depth charters round 12: at least one charter should key on an
    // in-world gate, so the mechanic is exercised.
    assert!(
        data.contracts
            .iter()
            .any(|(_, c)| !c.requires_faction_aboard.is_empty()),
        "some charter should gate on a people being aboard"
    );
    // Content-depth round 5: the dead-air backstop needs a pool to draw from
    // when it is switched on, or a forced beat has nothing to force.
    if sk.dead_air_years > 0 {
        assert!(
            !sk.dead_air_pool.is_empty(),
            "dead_air_years is set but dead_air_pool is empty"
        );
    }
    // Content-depth threshold beats: each family they draw from must have
    // events, and thresholds must be ascending in (0, 1] so each fires once
    // in order. Same rules for drift (round 2) and adaptation (round 3).
    for (beats, family, label) in [
        (&sk.drift_beats, &sk.drift_beat_family, "drift"),
        (
            &sk.adaptation_beats,
            &sk.adaptation_beat_family,
            "adaptation",
        ),
    ] {
        if beats.is_empty() {
            continue;
        }
        assert!(
            families.contains(family),
            "campaign_skeleton {label}_beat_family '{family}' has no events"
        );
        assert!(
            beats.windows(2).all(|w| w[0] < w[1]),
            "campaign_skeleton {label}_beats must be strictly ascending"
        );
        assert!(
            beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton {label}_beats must be within (0, 1]"
        );
    }
    // Content-depth round 6: crisis beats are the DESCENDING mirror — the
    // ship's cohesion falling past each level in turn — so the same rules
    // hold but the thresholds must be strictly descending.
    if !sk.crisis_beats.is_empty() {
        assert!(
            families.contains(&sk.crisis_beat_family),
            "campaign_skeleton crisis_beat_family '{}' has no events",
            sk.crisis_beat_family
        );
        assert!(
            sk.crisis_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton crisis_beats must be strictly descending"
        );
        assert!(
            sk.crisis_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton crisis_beats must be within (0, 1]"
        );
        // Content-depth round 13: the recovery threshold must sit clear above
        // the highest crisis threshold, so a fractured ship must genuinely climb
        // out (a hysteresis band where neither beat fires) before it mends.
        if !sk.recovery_beat_family.is_empty() {
            let worst_crisis = sk.crisis_beats.iter().cloned().fold(0.0_f32, f32::max);
            assert!(
                sk.recovery_beat_threshold > worst_crisis && sk.recovery_beat_threshold <= 1.0,
                "recovery_beat_threshold {} must sit above the crisis band {worst_crisis}",
                sk.recovery_beat_threshold
            );
        }
    }
    // Content-depth round 13: the recovery beat's family must have events.
    if !sk.recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.recovery_beat_family),
            "campaign_skeleton recovery_beat_family '{}' has no events",
            sk.recovery_beat_family
        );
    }
    // Content-depth campaign-skeleton round 29: despair beats are the descending
    // morale-collapse pole of the flourish beats — a family with events, and strictly
    // descending thresholds in (0, 1], each fired once as spirits sink past it.
    if !sk.despair_beats.is_empty() {
        assert!(
            families.contains(&sk.despair_beat_family),
            "campaign_skeleton despair_beat_family '{}' has no events",
            sk.despair_beat_family
        );
        assert!(
            sk.despair_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton despair_beats must be strictly descending"
        );
        assert!(
            sk.despair_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton despair_beats must be within (0, 1]"
        );
    }
    // Content-depth campaign-skeleton round 30: the heartening-recovery beat, the morale twin
    // of the it13/it28 recovery beats. Its family must have events, and its threshold must sit
    // clear above the worst despair-collapse line (hysteresis) and at most 1.0.
    if !sk.heartening_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.heartening_recovery_beat_family),
            "campaign_skeleton heartening_recovery_beat_family '{}' has no events",
            sk.heartening_recovery_beat_family
        );
        let worst_despair = sk.despair_beats.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            sk.heartening_recovery_beat_threshold > worst_despair
                && sk.heartening_recovery_beat_threshold <= 1.0,
            "heartening_recovery_beat_threshold {} must sit above the despair band {worst_despair}",
            sk.heartening_recovery_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 31: the covenant-recovery beat, the loyalty twin
    // of the it13/it28/it30 recovery beats. Its family must have events, and its threshold must
    // sit clear above the worst loyalty-collapse line (hysteresis) and at most 1.0.
    if !sk.loyalty_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.loyalty_recovery_beat_family),
            "campaign_skeleton loyalty_recovery_beat_family '{}' has no events",
            sk.loyalty_recovery_beat_family
        );
        let worst_loyalty = sk.loyalty_beats.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            sk.loyalty_recovery_beat_threshold > worst_loyalty
                && sk.loyalty_recovery_beat_threshold <= 1.0,
            "loyalty_recovery_beat_threshold {} must sit above the collapse band {worst_loyalty}",
            sk.loyalty_recovery_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 28: the governance-recovery beat, the stability
    // twin of the it13 unity recovery beat. Its family must have events; its threshold must
    // sit clear above the worst stability-collapse line (hysteresis) and at most 1.0; and a
    // `stability_above`-gated event (gated at or below the threshold, so it is eligible the
    // moment the recovery fires) must exist to surface the reckoning on theme.
    if !sk.stability_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.stability_recovery_beat_family),
            "campaign_skeleton stability_recovery_beat_family '{}' has no events",
            sk.stability_recovery_beat_family
        );
        let worst_stability = sk.stability_beats.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            sk.stability_recovery_beat_threshold > worst_stability
                && sk.stability_recovery_beat_threshold <= 1.0,
            "stability_recovery_beat_threshold {} must sit above the collapse band {worst_stability}",
            sk.stability_recovery_beat_threshold
        );
        assert!(
            data.events.iter().any(|(_, e)| e
                .stability_above
                .is_some_and(|t| t <= sk.stability_recovery_beat_threshold)),
            "the governance-recovery beat needs a stability_above event eligible at its threshold"
        );
    }
    // Content-depth round 14: loyalty-collapse beats are the DESCENDING mirror
    // on legacy_loyalty — strictly descending, in range, family with events.
    if !sk.loyalty_beats.is_empty() {
        assert!(
            families.contains(&sk.loyalty_beat_family),
            "campaign_skeleton loyalty_beat_family '{}' has no events",
            sk.loyalty_beat_family
        );
        assert!(
            sk.loyalty_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton loyalty_beats must be strictly descending"
        );
        assert!(
            sk.loyalty_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton loyalty_beats must be within (0, 1]"
        );
    }
    // Content-depth round 16: the reputation beat's family must have events, its
    // trait must be one some outcome nudges, and its band thresholds must order.
    if !sk.reputation_beat_family.is_empty() {
        assert!(
            families.contains(&sk.reputation_beat_family),
            "campaign_skeleton reputation_beat_family '{}' has no events",
            sk.reputation_beat_family
        );
        assert!(
            data.events.iter().any(|(_, e)| e.outcomes.iter().any(|o| o
                .reputation_deltas
                .iter()
                .any(|r| r.id == sk.reputation_beat_trait))),
            "campaign_skeleton reputation_beat_trait '{}' no outcome nudges",
            sk.reputation_beat_trait
        );
        assert!(
            sk.reputation_beat_low < sk.reputation_beat_high
                && (0.0..=1.0).contains(&sk.reputation_beat_low)
                && (0.0..=1.0).contains(&sk.reputation_beat_high),
            "campaign_skeleton reputation beat bands must order within [0, 1]"
        );
    }
    // Content-depth round 15: stability beats are the DESCENDING governance
    // mirror — strictly descending, in range, family with events.
    if !sk.stability_beats.is_empty() {
        assert!(
            families.contains(&sk.stability_beat_family),
            "campaign_skeleton stability_beat_family '{}' has no events",
            sk.stability_beat_family
        );
        assert!(
            sk.stability_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton stability_beats must be strictly descending"
        );
        assert!(
            sk.stability_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton stability_beats must be within (0, 1]"
        );
    }
    // Content-depth round 8: flourish beats are the ASCENDING positive pole —
    // morale climbing past each level in turn — so the thresholds must be
    // strictly ascending and in range, and the family must have events.
    if !sk.flourish_beats.is_empty() {
        assert!(
            families.contains(&sk.flourish_beat_family),
            "campaign_skeleton flourish_beat_family '{}' has no events",
            sk.flourish_beat_family
        );
        assert!(
            sk.flourish_beats.windows(2).all(|w| w[0] < w[1]),
            "campaign_skeleton flourish_beats must be strictly ascending"
        );
        assert!(
            sk.flourish_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton flourish_beats must be within [0, 1]"
        );
    }
    // Content-depth round 12: depopulation beats — founding-fraction thresholds
    // the crew falls past in turn, so strictly descending and in range, family
    // with events.
    if !sk.depopulation_beats.is_empty() {
        assert!(
            families.contains(&sk.depopulation_beat_family),
            "campaign_skeleton depopulation_beat_family '{}' has no events",
            sk.depopulation_beat_family
        );
        assert!(
            sk.depopulation_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton depopulation_beats must be strictly descending"
        );
        assert!(
            sk.depopulation_beats
                .iter()
                .all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton depopulation_beats must be within (0, 1]"
        );
    }
    // Content-depth round 17: subsystem-collapse beats — each names a real
    // module, a red line in (0, 1], and a family with events.
    for beat in &sk.subsystem_beats {
        assert!(
            data.subsystems.get(&beat.subsystem).is_some(),
            "campaign_skeleton subsystem_beat names unknown module '{}'",
            beat.subsystem
        );
        assert!(
            beat.threshold > 0.0 && beat.threshold <= 1.0,
            "campaign_skeleton subsystem_beat '{}' threshold {} must be within (0, 1]",
            beat.subsystem,
            beat.threshold
        );
        assert!(
            families.contains(&beat.family),
            "campaign_skeleton subsystem_beat '{}' family '{}' has no events",
            beat.subsystem,
            beat.family
        );
    }
    // Content-depth round 9: objective-progress beats — mission-fraction
    // milestones, ascending and in range, family with events.
    if !sk.objective_beats.is_empty() {
        assert!(
            families.contains(&sk.objective_beat_family),
            "campaign_skeleton objective_beat_family '{}' has no events",
            sk.objective_beat_family
        );
        assert!(
            sk.objective_beats.windows(2).all(|w| w[0] < w[1]),
            "campaign_skeleton objective_beats must be strictly ascending"
        );
        assert!(
            sk.objective_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton objective_beats must be within [0, 1]"
        );
    }
    // Content-depth round 7: the periodic anniversary beat needs a family
    // with events when it is switched on.
    if sk.anniversary_years > 0 {
        assert!(
            families.contains(&sk.anniversary_beat_family),
            "campaign_skeleton anniversary_beat_family '{}' has no events",
            sk.anniversary_beat_family
        );
    }
    // Content-depth round 18: the succession beat (a sitting leader dying in
    // office) needs a family with events when set.
    if !sk.succession_beat_family.is_empty() {
        assert!(
            families.contains(&sk.succession_beat_family),
            "campaign_skeleton succession_beat_family '{}' has no events",
            sk.succession_beat_family
        );
    }
    // Content-depth round 19: the long-reign beat needs a family with events
    // when switched on.
    if sk.long_reign_years > 0 && !sk.long_reign_beat_family.is_empty() {
        assert!(
            families.contains(&sk.long_reign_beat_family),
            "campaign_skeleton long_reign_beat_family '{}' has no events",
            sk.long_reign_beat_family
        );
    }
    // Content-depth round 20: the dynasty-crisis beat needs a family with events
    // when switched on.
    if sk.dynasty_crisis_size > 0 && !sk.dynasty_crisis_beat_family.is_empty() {
        assert!(
            families.contains(&sk.dynasty_crisis_beat_family),
            "campaign_skeleton dynasty_crisis_beat_family '{}' has no events",
            sk.dynasty_crisis_beat_family
        );
    }
    // Content-depth campaign-skeleton round 21: the mid-voyage (deep-middle) beat
    // needs a family with events when switched on.
    if !sk.midvoyage_beat_family.is_empty() {
        assert!(
            families.contains(&sk.midvoyage_beat_family),
            "campaign_skeleton midvoyage_beat_family '{}' has no events",
            sk.midvoyage_beat_family
        );
    }
    // Content-depth campaign-skeleton round 22: the founding-era beat needs a family
    // with events when switched on, and an early year to fire at.
    if !sk.founding_beat_family.is_empty() {
        assert!(
            families.contains(&sk.founding_beat_family),
            "campaign_skeleton founding_beat_family '{}' has no events",
            sk.founding_beat_family
        );
        assert!(
            sk.founding_beat_year > 0,
            "the founding beat needs an early year to fire at"
        );
    }
    // Content-depth campaign-skeleton round 23: the hull-collapse beat needs a family
    // with events and a red line in (0,1) when switched on, and — like the subsystem
    // collapse beat — a `hull_below`-gated event to surface so the reckoning lands on
    // theme.
    if !sk.hull_beat_family.is_empty() {
        assert!(
            families.contains(&sk.hull_beat_family),
            "campaign_skeleton hull_beat_family '{}' has no events",
            sk.hull_beat_family
        );
        assert!(
            sk.hull_beat_threshold > 0.0 && sk.hull_beat_threshold < 1.0,
            "hull_beat_threshold {} must be a red line inside (0, 1)",
            sk.hull_beat_threshold
        );
        assert!(
            data.events.iter().any(|(_, e)| e.hull_below.is_some()),
            "the hull-collapse beat needs a hull_below-gated event to surface"
        );
    }
    // Content-depth campaign-skeleton round 32: the hull-recovery beat needs a family with
    // events and a threshold that sits *above* the collapse red line (hysteresis) and no higher
    // than a whole hull — a rebuild, not a wobble over the line.
    if !sk.hull_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.hull_recovery_beat_family),
            "campaign_skeleton hull_recovery_beat_family '{}' has no events",
            sk.hull_recovery_beat_family
        );
        assert!(
            sk.hull_recovery_beat_threshold > sk.hull_beat_threshold
                && sk.hull_recovery_beat_threshold <= 1.0,
            "hull_recovery_beat_threshold {} must sit above the collapse red line {}",
            sk.hull_recovery_beat_threshold,
            sk.hull_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 24: the air-collapse beat, the same shape,
    // needs a family with events, a red line in (0,1), and a life_support_below event.
    if !sk.air_beat_family.is_empty() {
        assert!(
            families.contains(&sk.air_beat_family),
            "campaign_skeleton air_beat_family '{}' has no events",
            sk.air_beat_family
        );
        assert!(
            sk.air_beat_threshold > 0.0 && sk.air_beat_threshold < 1.0,
            "air_beat_threshold {} must be a red line inside (0, 1)",
            sk.air_beat_threshold
        );
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.life_support_below.is_some()),
            "the air-collapse beat needs a life_support_below-gated event to surface"
        );
    }
    // Content-depth campaign-skeleton round 33: the air-recovery beat, the atmosphere twin of
    // the hull-recovery guard — a family with events and a threshold that sits *above* the
    // collapse red line (hysteresis) and no higher than a whole plant (an overhaul, not a wobble).
    if !sk.air_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.air_recovery_beat_family),
            "campaign_skeleton air_recovery_beat_family '{}' has no events",
            sk.air_recovery_beat_family
        );
        assert!(
            sk.air_recovery_beat_threshold > sk.air_beat_threshold
                && sk.air_recovery_beat_threshold <= 1.0,
            "air_recovery_beat_threshold {} must sit above the collapse red line {}",
            sk.air_recovery_beat_threshold,
            sk.air_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 25: the becalmed (mobility) beat needs a
    // family with events and a sustained-stall year count when switched on.
    if !sk.becalmed_beat_family.is_empty() {
        assert!(
            families.contains(&sk.becalmed_beat_family),
            "campaign_skeleton becalmed_beat_family '{}' has no events",
            sk.becalmed_beat_family
        );
        assert!(
            sk.becalmed_beat_years > 0,
            "the becalmed beat needs a sustained-stall threshold"
        );
    }
    // Content-depth campaign-skeleton round 34: the becalmed-recovery beat needs a family with
    // events (it has no threshold — the recovery fires when the stall counter returns to 0).
    if !sk.becalmed_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.becalmed_recovery_beat_family),
            "campaign_skeleton becalmed_recovery_beat_family '{}' has no events",
            sk.becalmed_recovery_beat_family
        );
    }
    // Content-depth campaign-skeleton round 26: the adaptation-divergence beat, the high-side
    // crew-body twin, needs a family with events, a red line in (0,1), and an
    // adaptation_above-gated event to surface the reckoning on theme.
    if !sk.divergence_beat_family.is_empty() {
        assert!(
            families.contains(&sk.divergence_beat_family),
            "campaign_skeleton divergence_beat_family '{}' has no events",
            sk.divergence_beat_family
        );
        assert!(
            sk.divergence_beat_threshold > 0.0 && sk.divergence_beat_threshold < 1.0,
            "divergence_beat_threshold {} must be a red line inside (0, 1)",
            sk.divergence_beat_threshold
        );
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.adaptation_above.is_some()),
            "the adaptation-divergence beat needs an adaptation_above-gated event to surface"
        );
    }
    // Content-depth campaign-skeleton round 27: the cultural-divergence beat, the cultural
    // twin, needs a family with events, a red line in (0,1) set above the top drift_beats
    // milestone (so it is terminal, not another rung), and a high-`min_cultural_drift` event
    // to surface the reckoning on theme.
    if !sk.cultural_divergence_beat_family.is_empty() {
        assert!(
            families.contains(&sk.cultural_divergence_beat_family),
            "campaign_skeleton cultural_divergence_beat_family '{}' has no events",
            sk.cultural_divergence_beat_family
        );
        assert!(
            sk.cultural_divergence_beat_threshold > 0.0
                && sk.cultural_divergence_beat_threshold < 1.0,
            "cultural_divergence_beat_threshold {} must be a red line inside (0, 1)",
            sk.cultural_divergence_beat_threshold
        );
        assert!(
            sk.drift_beats
                .iter()
                .all(|t| *t <= sk.cultural_divergence_beat_threshold),
            "the cultural-divergence red line must sit at or above every drift_beats milestone"
        );
        assert!(
            data.events.iter().any(|(_, e)| e.min_cultural_drift >= 0.8),
            "the cultural-divergence beat needs a high-min_cultural_drift event to surface"
        );
    }
    // Content-depth voice: every generational-flavor pool must be non-empty
    // (or a generation turns over in silence) and carry its placeholder.
    let fl = &data.config.flavor;
    assert!(
        fl.obituary.iter().any(|s| s.contains("{name}")),
        "obituary flavor needs a {{name}} line"
    );
    assert!(
        fl.succession.iter().any(|s| s.contains("{name}")),
        "succession flavor needs a {{name}} line"
    );
    assert!(
        !fl.coming_of_age.is_empty(),
        "coming_of_age flavor must not be empty"
    );
    // Content-depth voice round 19: the two high-frequency pooled lines must
    // carry their substitution slot, or the mark/summons reads with a literal.
    assert!(
        fl.milestone.is_empty() || fl.milestone.iter().all(|s| s.contains("{milestone}")),
        "every milestone flavor line needs its {{milestone}} slot"
    );
    assert!(
        fl.council_summons.is_empty() || fl.council_summons.iter().all(|s| s.contains("{title}")),
        "every council_summons flavor line needs its {{title}} slot"
    );
    // Content-depth voice round 24: the pooled disaster-death and failing-air lines
    // carry their substitution slots, or a loss reads with a literal placeholder.
    assert!(
        fl.event_loss_officer
            .iter()
            .all(|s| s.contains("{name}") && s.contains("{post}")),
        "every event_loss_officer line needs its {{name}} and {{post}} slots"
    );
    assert!(
        fl.event_loss_member.iter().all(|s| s.contains("{name}")),
        "every event_loss_member line needs its {{name}} slot"
    );
    assert!(
        fl.life_support_loss.iter().all(|s| s.contains("{losses}")),
        "every life_support_loss line needs its {{losses}} slot"
    );
    // Content-depth provisioning round 21: the fabrication narration carries its
    // {parts} slot, and if the mechanic is on its costs/yield are sane (a positive
    // yield, and a mineral gate so it never runs a poor ship's ore dry).
    assert!(
        fl.fabrication.is_empty() || fl.fabrication.iter().all(|s| s.contains("{parts}")),
        "every fabrication flavor line needs its {{parts}} slot"
    );
    if data.config.surplus_energy_threshold > 0 {
        assert!(
            data.config.fabrication_parts_yield > 0
                && data.config.fabrication_minerals_cost > 0
                && data.config.fabrication_energy_cost > 0,
            "the fabrication mechanic is on but its costs/yield are not all positive"
        );
    }
    // Content-depth provisioning round 22: the market impact is a gentle per-unit
    // nudge — a bulk trade moves a thin market, but a single unit barely stirs it,
    // and the clamp plus the yearly drift keep even a whale ship from breaking it.
    assert!(
        (0.0..=0.01).contains(&data.config.market_impact_per_unit),
        "market_impact_per_unit {} out of the gentle range [0, 0.01]",
        data.config.market_impact_per_unit
    );
    // Content-depth provisioning round 30: the reputation trade scale is a gentle bend on
    // prices — a strong name shades the terms a captain's way but never makes trade free or
    // ruinous (kept below 1 so even a spotless or infamous name only tilts, never inverts).
    assert!(
        (0.0..1.0).contains(&data.config.trade_reputation_scale),
        "trade_reputation_scale {} must be a gentle fraction in [0, 1)",
        data.config.trade_reputation_scale
    );
    // Content-depth provisioning round 32: the desperation premium is a gentle markup in
    // [0, 1) — a crisis-buyer pays more for the good it cannot do without, but a waystation
    // never doubles the price on need alone.
    assert!(
        (0.0..1.0).contains(&data.config.market_desperation_premium),
        "market_desperation_premium {} must be a gentle markup in [0, 1)",
        data.config.market_desperation_premium
    );
    // Content-depth provisioning round 33: the distress discount is a fraction in [0, 1) — a
    // fire sale pays less, but a broke ship's stores are never taken for nothing.
    assert!(
        (0.0..1.0).contains(&data.config.market_distress_discount),
        "market_distress_discount {} must be a fraction in [0, 1)",
        data.config.market_distress_discount
    );
    // Content-depth provisioning round 25: the becalmed morale drain is a gentle
    // yearly attrition, like the chronic-hunger one it mirrors — the slow despair of a
    // voyage that will not move, not a single hard blow.
    assert!(
        (0.0..=0.05).contains(&data.config.becalmed_morale_drain),
        "becalmed_morale_drain {} must be a gentle yearly attrition [0, 0.05]",
        data.config.becalmed_morale_drain
    );
    // Content-depth provisioning round 27: the disrepair morale drain is a gentle yearly
    // attrition too, the third of the sustained-privation costs — the slow demoralization
    // of a home coming apart, not a single hard blow.
    assert!(
        (0.0..=0.05).contains(&data.config.disrepair_morale_drain),
        "disrepair_morale_drain {} must be a gentle yearly attrition [0, 0.05]",
        data.config.disrepair_morale_drain
    );
    // Content-depth provisioning round 34: the chronic-low-energy morale drain is a gentle
    // yearly attrition too, the fourth of the sustained-privation costs — the slow wearing of a
    // crew living in the dark, not a single hard blow.
    assert!(
        (0.0..=0.05).contains(&data.config.chronic_low_energy_morale_drain),
        "chronic_low_energy_morale_drain {} must be a gentle yearly attrition [0, 0.05]",
        data.config.chronic_low_energy_morale_drain
    );
    // Content-depth provisioning round 28: the chronic-hunger faction penalty is a gentle
    // yearly souring — the slow political erosion of a people that keeps going hungry, not a
    // single rupture (the acute famine events carry the sharp breaks).
    assert!(
        (0.0..=0.05).contains(&data.config.chronic_hunger_faction_penalty),
        "chronic_hunger_faction_penalty {} must be a gentle yearly souring [0, 0.05]",
        data.config.chronic_hunger_faction_penalty
    );
    // Content-depth provisioning round 31: the sustained-plenty faction bonus is the positive
    // mirror of that souring — a gentle yearly warming as a well-fed people learns to trust its
    // council — so it lives in the same [0, 0.05] band, never a single rupture of goodwill.
    assert!(
        (0.0..=0.05).contains(&data.config.sustained_plenty_faction_bonus),
        "sustained_plenty_faction_bonus {} must be a gentle yearly warming [0, 0.05]",
        data.config.sustained_plenty_faction_bonus
    );
    // Content-depth provisioning round 29: the low-energy production shed is a fraction in
    // [0, 1) — a power crisis dents industrial output but, kept below 1, never wholly stops
    // the factories, so a starved reactor slows the ship's earnings without freezing them.
    assert!(
        (0.0..1.0).contains(&data.config.low_energy_production_shed),
        "low_energy_production_shed {} must be in [0, 1) so power scarcity never zeroes production",
        data.config.low_energy_production_shed
    );
    // Content-depth provisioning round 24: the food carrying capacity, if set, must
    // sit above the fat line (a prudent reserve should still read as plenty, not spoil
    // the ship out of its own abundance), its spoilage a gentle fraction, and its
    // narration carry the {spoiled} slot.
    if data.config.food_carrying_capacity > 0 {
        assert!(
            data.config.food_carrying_capacity > data.config.fat_food_threshold,
            "food_carrying_capacity {} must sit above the fat line {} so plenty still reads",
            data.config.food_carrying_capacity,
            data.config.fat_food_threshold
        );
        assert!(
            data.config.food_spoilage_fraction > 0.0 && data.config.food_spoilage_fraction <= 0.5,
            "food_spoilage_fraction {} must be a gentle positive fraction",
            data.config.food_spoilage_fraction
        );
        assert!(
            data.config.flavor.food_spoilage.is_empty()
                || data
                    .config
                    .flavor
                    .food_spoilage
                    .iter()
                    .all(|s| s.contains("{spoiled}")),
            "every food_spoilage line needs its {{spoiled}} slot"
        );
    }
    // Content-depth provisioning round 26: the influence→governance income coupling. The
    // line is a fraction in (0,1) when enabled, and the floor a fraction in [0,1) strictly
    // below it (even a collapsed government mints *some* influence, but a healthy one must
    // out-earn it) — so the factor is continuous and never inverts.
    if data.config.influence_governance_threshold > 0.0 {
        assert!(
            data.config.influence_governance_threshold < 1.0,
            "influence_governance_threshold {} must be a fraction inside (0, 1)",
            data.config.influence_governance_threshold
        );
        assert!(
            (0.0..1.0).contains(&data.config.influence_governance_floor)
                && data.config.influence_governance_floor
                    < data.config.influence_governance_threshold,
            "influence_governance_floor {} must be in [0, 1) and below the threshold {}",
            data.config.influence_governance_floor,
            data.config.influence_governance_threshold
        );
    }
    // Content-depth charters round 22: the crew-morale accrual swing is gentle —
    // a devoted crew works meaningfully but not miraculously faster, and even a
    // broken one is floored above a stall at runtime.
    assert!(
        (0.0..=1.0).contains(&data.config.ship.morale_objective_swing),
        "morale_objective_swing {} out of the gentle range [0, 1]",
        data.config.ship.morale_objective_swing
    );
    // Content-depth charters round 34: the crew-unity accrual swing, the same gentle shape as
    // the morale one — a cohesive crew works meaningfully but not miraculously faster, floored
    // above a stall at runtime.
    assert!(
        (0.0..=1.0).contains(&data.config.ship.unity_objective_swing),
        "unity_objective_swing {} out of the gentle range [0, 1]",
        data.config.ship.unity_objective_swing
    );
    // Content-depth charters round 27: each point of combat deters route hazard by a
    // gentle fraction — a moderately-armed ship should meaningfully quiet a lawless route,
    // not make a single gun cancel the worst hazard outright.
    assert!(
        (0.0..=0.2).contains(&data.config.ship.hazard_combat_mitigation),
        "hazard_combat_mitigation {} out of the gentle range [0, 0.2]",
        data.config.ship.hazard_combat_mitigation
    );
    // Content-depth charters round 28: each berth eases preserve attrition by a gentle
    // fraction — a roomy hull should meaningfully outperform a cramped one, but not make a
    // single point of crew_capacity nearly cancel the whole attrition (the in-code floor of
    // 0.2 also caps the total relief regardless).
    assert!(
        (0.0..=0.05).contains(&data.config.ship.preserve_berth_relief),
        "preserve_berth_relief {} out of the gentle range [0, 0.05]",
        data.config.ship.preserve_berth_relief
    );
    // Content-depth charters round 31: the mission-outcome morale scale is a gentle one-time
    // shift — a clean run lifts spirits and a botched one dents them, but a single mission's
    // outcome should not, by itself, swing the whole crew's morale.
    assert!(
        (0.0..=0.5).contains(&data.config.ship.mission_outcome_morale_scale),
        "mission_outcome_morale_scale {} out of the gentle range [0, 0.5]",
        data.config.ship.mission_outcome_morale_scale
    );
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
    // Content-depth voice round 2: if ambient flavor is switched on, it needs
    // lines to draw from.
    if fl.ambient_gap_years > 0 {
        assert!(
            !fl.ambient.is_empty(),
            "ambient_gap_years is set but the ambient pool is empty"
        );
    }
    // Content-depth voice round 20: if the loyalty voice is switched on, both
    // bands need lines to draw from, and the thresholds must order (low < high,
    // both inside 0..1) or the crossing logic is nonsense.
    if fl.loyalty_voice_high > 0.0 {
        assert!(
            !fl.loyalty_guttering.is_empty() && !fl.loyalty_bright.is_empty(),
            "loyalty voice is enabled but a band pool is empty"
        );
        assert!(
            fl.loyalty_voice_low > 0.0 && fl.loyalty_voice_low < fl.loyalty_voice_high,
            "loyalty voice thresholds must order: 0 < low ({}) < high ({})",
            fl.loyalty_voice_low,
            fl.loyalty_voice_high
        );
        assert!(
            fl.loyalty_voice_high < 1.0,
            "loyalty_voice_high {} must be below 1.0 to be reachable",
            fl.loyalty_voice_high
        );
    }
    // Content-depth voice round 25: the adaptation (shipborn) voice, the same shape.
    if fl.adaptation_voice_high > 0.0 {
        assert!(
            !fl.crew_shipborn.is_empty() && !fl.crew_baseline.is_empty(),
            "adaptation voice is enabled but a band pool is empty"
        );
        assert!(
            fl.adaptation_voice_low > 0.0 && fl.adaptation_voice_low < fl.adaptation_voice_high,
            "adaptation voice thresholds must order: 0 < low ({}) < high ({})",
            fl.adaptation_voice_low,
            fl.adaptation_voice_high
        );
    }
    // Content-depth voice round 26: the cultural-drift (new-people) voice, the same shape
    // as the adaptation voice — its cultural twin.
    if fl.drift_voice_high > 0.0 {
        assert!(
            !fl.culture_newfound.is_empty() && !fl.culture_founders_kept.is_empty(),
            "drift voice is enabled but a band pool is empty"
        );
        assert!(
            fl.drift_voice_low > 0.0 && fl.drift_voice_low < fl.drift_voice_high,
            "drift voice thresholds must order: 0 < low ({}) < high ({})",
            fl.drift_voice_low,
            fl.drift_voice_high
        );
    }
    // Content-depth voice round 21: likewise the unity (cohesion) voice needs both
    // band pools stocked and its thresholds ordered when it is switched on.
    if fl.unity_voice_high > 0.0 {
        assert!(
            !fl.unity_fraying.is_empty() && !fl.unity_cohering.is_empty(),
            "unity voice is enabled but a band pool is empty"
        );
        assert!(
            fl.unity_voice_low > 0.0 && fl.unity_voice_low < fl.unity_voice_high,
            "unity voice thresholds must order: 0 < low ({}) < high ({})",
            fl.unity_voice_low,
            fl.unity_voice_high
        );
    }
    // Content-depth voice round 22: and the hull (ship's body) voice, the same shape.
    if fl.hull_voice_high > 0.0 {
        assert!(
            !fl.hull_groaning.is_empty() && !fl.hull_sound.is_empty(),
            "hull voice is enabled but a band pool is empty"
        );
        assert!(
            fl.hull_voice_low > 0.0 && fl.hull_voice_low < fl.hull_voice_high,
            "hull voice thresholds must order: 0 < low ({}) < high ({})",
            fl.hull_voice_low,
            fl.hull_voice_high
        );
    }
    // Content-depth voice round 23: and the air (life-support) voice, the same shape.
    if fl.air_voice_high > 0.0 {
        assert!(
            !fl.air_stale.is_empty() && !fl.air_fresh.is_empty(),
            "air voice is enabled but a band pool is empty"
        );
        assert!(
            fl.air_voice_low > 0.0 && fl.air_voice_low < fl.air_voice_high,
            "air voice thresholds must order: 0 < low ({}) < high ({})",
            fl.air_voice_low,
            fl.air_voice_high
        );
    }
    // Content-depth voice round 27: the drive (fuel) voice, the same shape as the hull and
    // air voices — the third ship-body voice.
    if fl.fuel_voice_high > 0.0 {
        assert!(
            !fl.drive_thin.is_empty() && !fl.drive_strong.is_empty(),
            "drive voice is enabled but a band pool is empty"
        );
        assert!(
            fl.fuel_voice_low > 0.0 && fl.fuel_voice_low < fl.fuel_voice_high,
            "drive voice thresholds must order: 0 < low ({}) < high ({})",
            fl.fuel_voice_low,
            fl.fuel_voice_high
        );
    }
    // Content-depth voice round 30: the crew-size voice needs both band pools stocked, and its
    // ratios must order — a swelling line above the founding complement (> 1) and a thinning
    // one below it (0 < low < 1 < high) — so the two bands never overlap.
    if fl.crew_size_voice_high_ratio > 0.0 {
        assert!(
            !fl.crew_swelling.is_empty() && !fl.crew_thinning.is_empty(),
            "crew-size voice is enabled but a band pool is empty"
        );
        assert!(
            fl.crew_size_voice_low_ratio > 0.0
                && fl.crew_size_voice_low_ratio < 1.0
                && fl.crew_size_voice_high_ratio > 1.0,
            "crew-size voice ratios must order: 0 < low ({}) < 1 < high ({})",
            fl.crew_size_voice_low_ratio,
            fl.crew_size_voice_high_ratio
        );
    }
    // Content-depth voice round 32: the treasury voice, the same shape as the crew-size voice —
    // both band pools stocked and its ratios ordered (a flush band above the founding stake, a
    // bare one below it) when enabled, so the two bands never overlap.
    if fl.treasury_voice_high_ratio > 0.0 {
        assert!(
            !fl.treasury_flush.is_empty() && !fl.treasury_bare.is_empty(),
            "treasury voice is enabled but a band pool is empty"
        );
        assert!(
            fl.treasury_voice_low_ratio > 0.0
                && fl.treasury_voice_low_ratio < 1.0
                && fl.treasury_voice_high_ratio > 1.0,
            "treasury voice ratios must order: 0 < low ({}) < 1 < high ({})",
            fl.treasury_voice_low_ratio,
            fl.treasury_voice_high_ratio
        );
    }
    // Content-depth voice round 33: the power voice, the treasury's energy sibling — both band
    // pools stocked and its absolute energy lines ordered (0 < dark < flush) when enabled, and
    // the founding stock bracketed between them so a launched ship reads neutral, not flush.
    if fl.power_voice_high > 0 {
        assert!(
            !fl.power_flush.is_empty() && !fl.power_starved.is_empty(),
            "power voice is enabled but a band pool is empty"
        );
        assert!(
            fl.power_voice_low > 0 && fl.power_voice_low < fl.power_voice_high,
            "power voice lines must order: 0 < dark ({}) < flush ({})",
            fl.power_voice_low,
            fl.power_voice_high
        );
        assert!(
            data.config.starting_resources.energy > fl.power_voice_low
                && data.config.starting_resources.energy < fl.power_voice_high,
            "the founding energy stock {} must sit between the power lines ({}, {}) so launch reads neutral",
            data.config.starting_resources.energy,
            fl.power_voice_low,
            fl.power_voice_high
        );
    }
    // Content-depth voice round 31: the ruling-people voice, when stocked, must name the new
    // majority — every line carries the `{name}` placeholder, or the changing of the guard
    // would announce a ship passing into the hands of nobody.
    assert!(
        fl.ruling_people_change
            .iter()
            .all(|line| line.contains("{name}")),
        "every ruling_people_change line must carry the {{name}} placeholder"
    );
    // Content-depth voice round 28: the wonder reputation voice, the same shape — both band
    // pools stocked and the thresholds ordered when enabled.
    if fl.wonder_voice_high > 0.0 {
        assert!(
            !fl.wonder_famed.is_empty() && !fl.wonder_incurious.is_empty(),
            "wonder voice is enabled but a band pool is empty"
        );
        assert!(
            fl.wonder_voice_low > 0.0 && fl.wonder_voice_low < fl.wonder_voice_high,
            "wonder voice thresholds must order: 0 < low ({}) < high ({})",
            fl.wonder_voice_low,
            fl.wonder_voice_high
        );
    }
    // Content-depth voice round 29: the resolve reputation voice, the same shape — the third
    // built-trait voice, both pools stocked and the thresholds ordered when enabled.
    if fl.resolve_voice_high > 0.0 {
        assert!(
            !fl.resolve_steadfast.is_empty() && !fl.resolve_yielding.is_empty(),
            "resolve voice is enabled but a band pool is empty"
        );
        assert!(
            fl.resolve_voice_low > 0.0 && fl.resolve_voice_low < fl.resolve_voice_high,
            "resolve voice thresholds must order: 0 < low ({}) < high ({})",
            fl.resolve_voice_low,
            fl.resolve_voice_high
        );
    }
    // Content-depth voice round 6: the recurring-crisis pools need variety
    // (they fire per year the crisis lasts), and famine weaves in its toll.
    assert!(
        fl.famine.len() >= 3 && fl.famine.iter().any(|s| s.contains("{losses}")),
        "the famine pool needs variety and a {{losses}} line"
    );
    assert!(
        fl.fuel_stall.len() >= 3,
        "the fuel-stall pool needs variety"
    );
    // Content-depth voice round 3: phase-line pool keys must be real phases.
    for key in fl.phase_lines.keys() {
        assert!(
            matches!(
                key.as_str(),
                "preparation" | "travel" | "operation" | "return" | "completion"
            ),
            "flavor.phase_lines has an unknown phase key '{key}'"
        );
    }
}

#[test]
fn every_event_is_tagged_and_families_are_filled() {
    use crate::data::contracts::ContractPhase;
    use std::collections::HashMap;
    let data = GameData::load().unwrap();
    let canonical: std::collections::HashSet<&str> = [
        "exploration_first_contact",
        "diplomacy",
        "engineering",
        "biology_medical",
        "science_anomaly",
        "survival",
        "mystery",
        "comedy",
        "ethics",
        "legacy_drift",
    ]
    .into_iter()
    .collect();

    let mut counts: HashMap<String, usize> = HashMap::new();
    for (id, e) in data.events.iter() {
        assert!(!e.family.is_empty(), "event '{id}' has no family (W6)");
        assert!(
            canonical.contains(e.family.as_str()),
            "event '{id}' family '{}' is not one of the canonical ten",
            e.family
        );
        for phase in &e.phases {
            assert!(
                matches!(
                    phase,
                    ContractPhase::Travel | ContractPhase::Operation | ContractPhase::Return
                ),
                "event '{id}' has a non-voyage phase gate {phase:?}"
            );
        }
        *counts.entry(e.family.clone()).or_default() += 1;
    }

    assert!(
        data.events.len() >= 60,
        "W6 wants >= 60 templates, found {}",
        data.events.len()
    );
    for family in &canonical {
        let n = counts.get(*family).copied().unwrap_or(0);
        assert!(
            n >= 6,
            "family '{family}' has only {n} templates (W6 wants >= 6)"
        );
    }
}

#[test]
fn tutorial_steps_cover_the_launch_flow() {
    let data = GameData::load().unwrap();
    let tutorial = &data.config.tutorial;
    assert!(!tutorial.drydock_hint.trim().is_empty());
    assert!(!tutorial.drydock_refit_hint.trim().is_empty());
    // The PREP checklist binds these ids to completion checks — the
    // authored steps must match them exactly, in launch order.
    let ids: Vec<&str> = tutorial.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "choose_charter",
            "stock_food",
            "stock_parts",
            "fuel_tanks",
            "launch"
        ],
        "tutorial steps must match the PREP checklist's known ids"
    );
    for step in &tutorial.steps {
        assert!(!step.label.trim().is_empty(), "step '{}' label", step.id);
        assert!(!step.tip.trim().is_empty(), "step '{}' tip", step.id);
    }
}

#[test]
fn a_new_ship_sails_provisioned_for_a_starter_charter() {
    // A new player should be able to fly a renown-0 charter without
    // shopping first: the founding stores cover the shortest one whole.
    let data = GameData::load().unwrap();
    let config = &data.config;
    let starter_years = data
        .contracts
        .iter()
        .filter(|(_, c)| c.min_renown == 0)
        .map(|(_, c)| c.target_duration_years)
        .min()
        .expect("at least one renown-0 charter");
    let food_need = (config.starting_population as f32
        * config.food_per_person_per_year
        * starter_years as f32)
        .ceil() as i64;
    assert!(
        config.starting_resources.food >= food_need,
        "founding food {} must cover a {starter_years}-yr starter charter ({food_need})",
        config.starting_resources.food
    );
    assert!(
        config.starting_spare_parts >= config.parts_upkeep_per_year * starter_years as i64,
        "founding parts {} must cover {starter_years} years of upkeep",
        config.starting_spare_parts
    );

    // Economy rebalance (economy_balance_plan.md phase 3, target T3): after
    // the phase-2 price hike the founding stake must still put a first
    // tier-1 upgrade within reach of a new captain — the early game keeps
    // its tension, but the first improvement is a choice on turn one, not a
    // wall to save toward. The stake covers the cheapest tier-1 subsystem
    // plus whatever launching the shortest starter charter costs in credits
    // (parts beyond the founding stock; the tank starts full).
    let cheapest_tier1 = data
        .subsystems
        .iter()
        .filter_map(|(_, s)| s.tiers.first())
        .map(|t| t.cost.credits)
        .min()
        .expect("subsystems have at least one purchasable tier");
    let parts_shortfall =
        (config.parts_upkeep_per_year * starter_years as i64 - config.starting_spare_parts).max(0);
    let launch_bill = parts_shortfall * config.provisioning.part_cost_credits;
    assert!(
        config.starting_resources.credits >= cheapest_tier1 + launch_bill,
        "founding stake {} must cover the cheapest tier-1 upgrade ({cheapest_tier1}) plus \
         the shortest starter charter's launch bill ({launch_bill})",
        config.starting_resources.credits
    );
}

#[test]
fn a_charter_fee_is_worth_the_voyage() {
    // Economy rebalance (economy_balance_plan.md phase 1): the fee is the
    // story. Every charter's credit fee sits in an authored band per
    // voyage-year — above the passive drip's shadow, below a blank check —
    // and the ladder climbs with the renown gate: founding writs pay modestly,
    // the storied ones pay like the legends they are.
    let data = GameData::load().unwrap();
    for (id, c) in data.contracts.iter() {
        let per_year = c.reward.credits as f32 / c.target_duration_years as f32;
        assert!(
            (35.0..=100.0).contains(&per_year),
            "charter '{id}' pays {per_year:.1} cr/voyage-year; the authored band is 35-100"
        );
        if c.min_renown == 0 {
            assert!(
                per_year <= 50.0,
                "founding charter '{id}' pays {per_year:.1} cr/yr; renown-0 writs stay at or under 50"
            );
        }
        if c.min_renown >= 400 {
            assert!(
                per_year >= 80.0,
                "storied charter '{id}' pays {per_year:.1} cr/yr; renown-400 writs pay 80+"
            );
        }
    }
}

#[test]
fn a_charter_fee_clears_its_provisioning_bill() {
    // Economy rebalance (economy_balance_plan.md phase 1): a writ must pay
    // for the sailing several times over. The bill estimated here is what
    // the voyage itself costs the treasury — the spare parts consumed beyond
    // the founding stock, and a full tank — so a mission is never a wash.
    let data = GameData::load().unwrap();
    let config = &data.config;
    for (id, c) in data.contracts.iter() {
        let parts_needed = config.parts_upkeep_per_year * c.target_duration_years as i64;
        let parts_shortfall = (parts_needed - config.starting_spare_parts).max(0);
        let bill = parts_shortfall * config.provisioning.part_cost_credits
            + 100 * config.provisioning.fuel_cost_credits_per_point;
        assert!(
            c.reward.credits >= 3 * bill,
            "charter '{id}' fee {} must be at least 3x its provisioning bill {bill}",
            c.reward.credits
        );
    }
}

#[test]
fn the_best_ship_is_earned_across_many_voyages() {
    // Economy rebalance (economy_balance_plan.md phase 2): the best ship and
    // its full kit should cost several successful missions, not one lucky
    // payday. This pins the whole-catalog credit cost against what a voyage
    // actually banks — fee, milestones, and the passive drip the crossing
    // mints, less what the sailing costs — so fees (phase 1) and prices
    // (phase 2) can never drift apart into trivial wealth or endless grind.
    let data = GameData::load().unwrap();
    let config = &data.config;
    use ship_components::ComponentKind;

    // The full best-buyable kit: the dearest hull (plus the commission
    // premium a new hull costs), the dearest engine, the dearest weapon,
    // and every subsystem tier bought up the ladder. Mission-reward relics
    // carry no price and never enter the reckoning.
    let dearest = |kind: ComponentKind| {
        data.ship_components
            .list(kind)
            .iter()
            .map(|c| c.cost.credits)
            .max()
            .unwrap_or(0)
    };
    let subsystem_ladders: i64 = data
        .subsystems
        .iter()
        .flat_map(|(_, s)| s.tiers.iter())
        .map(|t| t.cost.credits)
        .sum();
    let kit_cost = dearest(ComponentKind::Hull)
        + config.commission.premium_credits
        + dearest(ComponentKind::Engine)
        + dearest(ComponentKind::Weapon)
        + subsystem_ladders;

    // What a successful charter banks: its fee, its milestone credits, and
    // the base passive production over the whole crossing, less the voyage's
    // own provisioning bill (parts beyond the founding stock, plus a tank).
    let net_incomes: Vec<i64> = data
        .contracts
        .iter()
        .map(|(_, c)| {
            let milestones: i64 = c.milestones.iter().map(|m| m.reward.credits).sum();
            let drip = (config.base_production.credits * c.target_duration_years as f32) as i64;
            let parts_needed = config.parts_upkeep_per_year * c.target_duration_years as i64;
            let parts_shortfall = (parts_needed - config.starting_spare_parts).max(0);
            let bill = parts_shortfall * config.provisioning.part_cost_credits
                + 100 * config.provisioning.fuel_cost_credits_per_point;
            c.reward.credits + milestones + drip - bill
        })
        .collect();
    let mean_income = net_incomes.iter().sum::<i64>() / net_incomes.len() as i64;

    let missions = kit_cost as f32 / mean_income as f32;
    assert!(
        (4.0..=7.0).contains(&missions),
        "the full kit costs {kit_cost} cr = {missions:.1} mean-mission incomes ({mean_income} each); \
         the authored pacing is 4-7 successful voyages"
    );
}

#[test]
fn a_full_refit_is_a_visible_slice_of_a_fee_but_never_a_wall() {
    // Economy rebalance (economy_balance_plan.md phase 3): a battered return
    // should cost real coin — a full refit is the sink that makes thrashing
    // the ship matter — but never so much that even the leanest fee cannot
    // cover the way home. Pinned as a band against the cheapest charter fee.
    let data = GameData::load().unwrap();
    let refit = data.config.repair.full_credits_cost;
    let cheapest_fee = data
        .contracts
        .iter()
        .map(|(_, c)| c.reward.credits)
        .min()
        .expect("at least one charter");
    let slice = refit as f32 / cheapest_fee as f32;
    assert!(
        (0.10..=0.50).contains(&slice),
        "a full refit ({refit} cr) is {:.0}% of the leanest fee ({cheapest_fee}); \
         the sink should be a felt 10-50%, visible but never a wall",
        slice * 100.0
    );
}

#[test]
fn a_heritage_head_start_is_a_boost_not_a_replacement() {
    // Economy rebalance (economy_balance_plan.md phase 3 heritage review): the
    // rebalance left the founding stake untouched, so a storied dynasty's
    // credit head start stays anchored to it — a real leg up (the top tier is
    // a large fraction of the stake) that never eclipses a fresh captain's own
    // footing. This holds the "boost, not replacement" line against a future
    // heritage bump or a stake cut, and is why the grants were kept as-is
    // rather than scaled with the catalog: they ride the (unchanged) stake,
    // not the (raised) prices.
    let data = GameData::load().unwrap();
    let stake = data.config.starting_resources.credits;
    let top_grant = data
        .config
        .heritage
        .iter()
        .map(|h| h.credits)
        .max()
        .expect("heritage tiers exist");
    assert!(
        top_grant > 0 && top_grant < stake,
        "the richest heritage grant ({top_grant} cr) must be a boost — nonzero, but under the \
         founding stake ({stake}) so meta-progression never dominates the founding position"
    );
}
