//! Granted parts and fittings: every id resolves, and every part that can
//! only arrive as a mission reward is actually reachable from some mission.

use super::*;

/// Salvage grants, mission-reward components and subsystem fittings.
#[test]
fn every_granted_part_is_real_and_every_mission_only_part_is_reachable() {
    let data = GameData::load().unwrap();
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
}
