//! Event gating: every tag, consequence, trait and outcome an event gates
//! on must be something the rest of the content can actually produce.

use super::*;

/// A typo in a gate is invisible at runtime - the event simply never
/// fires - so every reference is checked against the content it names.
#[test]
fn every_event_gate_names_something_real() {
    let data = GameData::load().unwrap();
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
}

/// The keeper-of-memory arc: the ship's archive engine is the one custodian that
/// outlives every generation, so its arc must stay a *fork with two reckonings*
/// rather than a chain of isolated beats. The petition's two mandates each promise
/// a payoff decades out; each payoff is gated to its own branch; and the terminal
/// beat is reachable only from the branch that let the record stand.
#[test]
fn the_stewards_arc_forks_and_both_branches_pay_off() {
    let data = GameData::load().unwrap();
    let petition = data
        .events
        .get("the_stewards_petition")
        .expect("the steward arc's entry event");
    // Every branch that hands the record's fate to the engine or to the letter
    // must schedule its own reckoning; the supervised branch buys its way out.
    let scheduled: std::collections::HashMap<&str, &str> = petition
        .outcomes
        .iter()
        .filter_map(|o| {
            o.schedule_followup
                .as_ref()
                .map(|f| (o.id.as_str(), f.template_id.as_str()))
        })
        .collect();
    assert_eq!(
        scheduled.get("grant_the_mandate"),
        Some(&"the_stewards_edit"),
        "the mandate branch must promise the edit reckoning"
    );
    assert_eq!(
        scheduled.get("bind_to_the_letter"),
        Some(&"the_unread_years"),
        "the binding branch must promise the unread-archive reckoning"
    );
    // Each payoff is walled off to the branch that earned it, so a ship never
    // meets the reckoning for a choice it did not make.
    for (payoff, gate) in [
        ("the_stewards_edit", "steward_mandate"),
        ("the_unread_years", "steward_bound"),
    ] {
        let e = data.events.get(payoff).expect("steward arc payoff");
        assert!(
            e.requires_consequence.iter().any(|c| c == gate),
            "'{payoff}' must be gated on '{gate}'"
        );
    }
    // The terminal beat only exists for a ship that let the smoothing stand, and
    // only once the people have drifted far enough for it to cost them something.
    let terminal = data
        .events
        .get("what_the_steward_remembers")
        .expect("the steward arc's terminal beat");
    assert!(
        terminal
            .requires_consequence
            .iter()
            .any(|c| c == "record_smoothed"),
        "the terminal beat must follow the smoothed record"
    );
    assert!(
        terminal.min_cultural_drift > 0.0 && terminal.min_generation > 0,
        "the terminal beat is century-scale content and must be gated as such"
    );
}
