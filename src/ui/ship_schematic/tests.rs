//! Schematic layout tests. `build` is pure, so these need no live frame.

use super::*;
use crate::state::sim::founding_faction_ids;

fn frame() -> Rect {
    Rect::new(0.0, 0.0, 900.0, 430.0)
}

fn sim(data: &GameData) -> SimState {
    SimState::new_campaign(data, "preservers", 0xC0FFEE, &founding_faction_ids(data))
}

#[test]
fn build_is_deterministic() {
    let data = GameData::load().unwrap();
    let s = sim(&data);
    assert_eq!(build(&s, &data, frame()), build(&s, &data, frame()));
}

#[test]
fn every_subsystem_plus_bridge_and_engine_get_a_glyph() {
    let data = GameData::load().unwrap();
    let s = sim(&data);
    let sch = build(&s, &data, frame());
    for id in GameData::sorted_ids(&data.subsystems) {
        assert!(
            sch.modules.iter().any(|m| m.id == id),
            "missing subsystem glyph {id}"
        );
    }
    assert!(sch.modules.iter().any(|m| m.kind == ModuleKind::Bridge));
    assert!(sch.modules.iter().any(|m| m.kind == ModuleKind::Engine));
}

#[test]
fn weapon_glyph_appears_only_when_a_weapon_is_fitted() {
    let data = GameData::load().unwrap();
    let mut s = sim(&data);
    s.ship.weapon = None;
    assert!(!build(&s, &data, frame())
        .modules
        .iter()
        .any(|m| m.kind == ModuleKind::Weapon));
    s.ship.weapon = Some("mass_driver".to_owned());
    assert!(build(&s, &data, frame())
        .modules
        .iter()
        .any(|m| m.kind == ModuleKind::Weapon));
}

#[test]
fn manning_follows_the_crew_aboard() {
    let data = GameData::load().unwrap();
    let mut s = sim(&data);
    // Founding crew includes an agronomist → agriculture is manned.
    let manned = |sch: &ShipSchematic| {
        sch.modules
            .iter()
            .find(|m| m.id == "agriculture")
            .map(|m| m.manned)
            .unwrap()
    };
    assert!(manned(&build(&s, &data, frame())));
    // A crew member added mid-mission updates the picture; removing one clears it.
    s.crew.retain(|c| c.archetype_id != "agronomist");
    assert!(!manned(&build(&s, &data, frame())));
}

#[test]
fn upgrading_a_subsystem_grows_its_glyph() {
    let data = GameData::load().unwrap();
    let mut s = sim(&data);
    let before = build(&s, &data, frame());
    let a0 = before
        .modules
        .iter()
        .find(|m| m.id == "agriculture")
        .unwrap();
    let w0 = a0.rect.w;
    s.subsystems.get_mut("agriculture").unwrap().tier = 3;
    let after = build(&s, &data, frame());
    let a1 = after
        .modules
        .iter()
        .find(|m| m.id == "agriculture")
        .unwrap();
    assert_eq!(a1.tier, 3);
    assert!(a1.rect.w > w0, "a higher tier draws a larger module");
}

#[test]
fn swapping_the_engine_changes_the_engine_glyph() {
    let data = GameData::load().unwrap();
    let mut s = sim(&data);
    let before = build(&s, &data, frame());
    let e0 = before
        .modules
        .iter()
        .find(|m| m.kind == ModuleKind::Engine)
        .unwrap();
    s.ship.engine = "warp_coil".to_owned();
    let after = build(&s, &data, frame());
    let e1 = after
        .modules
        .iter()
        .find(|m| m.kind == ModuleKind::Engine)
        .unwrap();
    assert_ne!(e0.id, e1.id);
    assert_eq!(e1.id, "warp_coil");
}

#[test]
fn every_subsystem_carries_a_three_letter_code() {
    let data = GameData::load().unwrap();
    let sch = build(&sim(&data), &data, frame());
    for m in sch
        .modules
        .iter()
        .filter(|m| m.kind == ModuleKind::Subsystem)
    {
        assert_eq!(m.code.len(), 3, "{} has a non-triliteral code", m.id);
        assert_ne!(
            m.code, "SYS",
            "{} fell through to the placeholder code",
            m.id
        );
    }
}

#[test]
fn deck_range_is_stable_and_ordered() {
    let (lo, hi) = deck_range("agriculture");
    assert_eq!((lo, hi), deck_range("agriculture"));
    assert!(hi > lo);
}
