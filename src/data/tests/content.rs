//! The embedded registries load at the sizes the design calls for.

use super::*;

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

/// Every registry parses and carries at least the authored minimum.
#[test]
fn the_embedded_registries_load_at_their_authored_sizes() {
    let data = GameData::load().unwrap();
    assert_eq!(data.config.game_name, "stellar_legacy");
    assert_eq!(data.legacies.len(), 3);
    assert!(data.events.len() >= 4);
    assert!(
        data.contracts.len() >= 10,
        "§8 target was 6-8 contracts; the pool has since grown"
    );
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

/// No category may thin out to the point a roll has nothing to draw.
#[test]
fn every_event_category_is_well_represented() {
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
}

/// The on-station leg is where the charter's *work* happens, so every objective
/// family the contract pool defines must have Operation content that touches the
/// tally rather than only flavouring it. A family with no such event leaves its
/// missions playing out as a Travel voyage with a waiting period bolted on.
#[test]
fn every_objective_family_has_operation_content_that_moves_the_tally() {
    use crate::data::contracts::ContractPhase;
    let data = GameData::load().unwrap();
    // The tag each objective family is recognised by on the charter board.
    for tag in [
        "mining",
        "colony",
        "survey",
        "salvage",
        "relief",
        "patrol",
        "inhabited",
    ] {
        let tag = tag.to_string();
        assert!(
            data.contracts.iter().any(|(_, c)| c.tags.contains(&tag)),
            "no charter carries the objective tag '{tag}'"
        );
        let biting = data.events.iter().filter(|(_, e)| {
            e.phases.contains(&ContractPhase::Operation)
                && e.requires_charter_tag.contains(&tag)
                && e.outcomes.iter().any(|o| o.objective_progress_delta != 0.0)
        });
        assert!(
            biting.count() > 0,
            "objective family '{tag}' has no Operation event whose choices move the objective"
        );
    }
}
