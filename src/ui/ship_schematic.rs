//! Procedural ship schematic (mission SHIP tab): a blueprint of the vessel
//! built from its actual loadout and subsystems, with each module highlighted by
//! condition, tier, and crew manning.
//!
//! Two halves, split so the layout is testable without a live frame:
//! [`build`] is a pure, deterministic function that reads `&SimState` and returns
//! a [`ShipSchematic`] of positioned glyphs (no macroquad calls, no RNG); [`draw`]
//! and [`draw_status_strip`] render that spec. Because `build` reads the sim fresh
//! each frame, the picture reacts on its own to anything that changes mid-mission —
//! a subsystem repaired or upgraded, a salvaged part field-installed, or a crew
//! member added by an event — with no wiring beyond the per-frame call.

mod draw;

pub use draw::*;

use crate::data::ship_components::{ComponentKind, ComponentStats};
use crate::data::GameData;
use crate::simulation::ship::loadout_stats;
use crate::state::sim::SimState;
use crate::ui::{term, term_bar};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

/// Which class of ship part a glyph stands for. Bridge/Engine/Weapon are the
/// installed components; Subsystem is one of the six module families (W5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Bridge,
    Subsystem,
    Engine,
    Weapon,
}

/// One highlighted module on the schematic: where it sits, what it is, and the
/// three signals the highlight encodes — condition, tier, and manning.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleGlyph {
    /// Subsystem id, or `"bridge"` / component id for the loadout glyphs.
    pub id: String,
    pub label: String,
    /// Standardized 3-letter tag drawn inside the box — the scannable identity,
    /// independent of the long external caption.
    pub code: String,
    pub rect: Rect,
    pub kind: ModuleKind,
    /// 0-1 physical condition (subsystem condition; component integrity/fuel).
    pub condition: f32,
    /// 0..=3 upgrade tier (subsystems only; components stay 0).
    pub tier: u32,
    /// A matching crew post is aboard.
    pub manned: bool,
    /// Cosmetic deck range for the caption, e.g. `DECKS 3-6`. Deterministic,
    /// never read by the sim.
    pub deck_lo: u8,
    pub deck_hi: u8,
}

/// A fully-placed schematic: the hull silhouette, an optional habitat ring, the
/// module glyphs, and the live ship-status numbers for the bottom strip.
#[derive(Debug, Clone, PartialEq)]
pub struct ShipSchematic {
    pub hull_id: String,
    pub hull_name: String,
    /// Closed hull outline in absolute logical coordinates.
    pub outline: Vec<Vec2>,
    /// The primary corridor (bow end → stern end) that every compartment branches
    /// off — the schematic's structural spine.
    pub corridor: (Vec2, Vec2),
    /// Central spun-gravity ring (centre, radius) for hulls that carry one.
    pub ring: Option<(Vec2, f32)>,
    pub modules: Vec<ModuleGlyph>,
    pub hull_integrity: f32,
    pub life_support: f32,
    pub fuel: f32,
    pub spare_parts: i64,
    /// Aggregate installed-loadout stats — drives the combat/modifiers tiles.
    pub stats: ComponentStats,
}

/// The crew posts that "man" a subsystem. A module with one of these aboard reads
/// as crewed; the rest stand dim. Kept here (not in data) because it is a UI
/// affordance, not balance.
fn subsystem_crew_posts(id: &str) -> &'static [&'static str] {
    match id {
        "agriculture" => &["agronomist"],
        "medical_bay" => &["medic"],
        "engineering_bay" => &["engineer"],
        // Life support is kept by the same hands that keep the reactor.
        "life_support_habitat" => &["engineer"],
        "security" => &["security_chief"],
        "education_culture" => &["scientist"],
        _ => &[],
    }
}

/// The standardized 3-letter tag stamped inside a subsystem compartment. Kept
/// stable so a room reads the same across every hull it appears on.
fn subsystem_code(id: &str) -> &'static str {
    match id {
        "agriculture" => "AGR",
        "education_culture" => "EDU",
        "engineering_bay" => "ENG",
        "life_support_habitat" => "LSH",
        "medical_bay" => "MED",
        "security" => "SEC",
        _ => "SYS",
    }
}

/// A short single-phrase caption for the diagram — the full formal name lives on
/// the Subsystems tab. Short so adjacent captions never collide, even when a lean
/// hull packs the rooms close together.
fn subsystem_short(id: &str) -> &'static str {
    match id {
        "agriculture" => "AGRICULTURE",
        "education_culture" => "ARCHIVES",
        "engineering_bay" => "ENGINEERING",
        "life_support_habitat" => "LIFE SUPPORT",
        "medical_bay" => "MEDICAL",
        "security" => "SECURITY",
        _ => "SYSTEM",
    }
}

/// The bridge is stood by the ship's command staff.
const BRIDGE_POSTS: &[&str] = &["commander", "navigator"];

/// Fixed distance from the corridor to each deck's room centres. Rooms are placed
/// relative to this, not to the hull size, so the layout survives any hull class.
const ROW_OFFSET: f32 = 48.0;
/// The vertical slot each room occupies (tallest tier-3 box plus headroom).
const ROOM_SLOT_H: f32 = 46.0;

fn any_post_aboard(sim: &SimState, posts: &[&str]) -> bool {
    sim.crew
        .iter()
        .any(|c| posts.contains(&c.archetype_id.as_str()))
}

/// FNV-1a over the id bytes: a stable per-module number for cosmetic deck labels
/// and greeble linework. Deterministic — same id, same value, every run.
fn hash_id(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn deck_range(id: &str) -> (u8, u8) {
    let h = hash_id(id);
    let lo = (h % 8) as u8 + 1;
    let span = ((h >> 8) % 5) as u8 + 1;
    (lo, lo + span)
}

/// Hull silhouette parameters, chosen by hull id so each ship reads distinct.
/// Only the *outline* is shaped here; the compartment grid is placed independently
/// (rooms hug the corridor at a fixed offset), so no hull can make a room overflow.
struct Profile {
    /// 0 blunt … 1 needle prow.
    nose: f32,
    /// Mid-body fullness — how full the shoulders are (broad freighter vs. lean
    /// corvette).
    bulge: f32,
    /// Stern width fraction.
    tail: f32,
    /// Fraction of the available width the hull spans — a corvette is short, an
    /// ark long.
    length: f32,
    /// Multiplier on the hull's half-height above the room-enclosing minimum, so
    /// bulky classes stand taller without ever pinching the compartments.
    height: f32,
    /// Carries a central spun-gravity ring.
    ring: bool,
}

fn hull_profile(hull_id: &str) -> Profile {
    match hull_id {
        "colony_barge" => Profile {
            nose: 0.2,
            bulge: 1.0,
            tail: 0.75,
            length: 0.94,
            height: 1.12,
            ring: false,
        },
        "light_corvette" => Profile {
            nose: 0.95,
            bulge: 0.4,
            tail: 0.36,
            length: 0.72,
            height: 0.98,
            ring: false,
        },
        "generation_ark" => Profile {
            nose: 0.3,
            bulge: 1.0,
            tail: 0.85,
            length: 1.0,
            height: 1.2,
            ring: true,
        },
        "habitat_ring" => Profile {
            nose: 0.45,
            bulge: 0.72,
            tail: 0.6,
            length: 0.9,
            height: 1.16,
            ring: true,
        },
        "armored_prow" => Profile {
            nose: 1.0,
            bulge: 0.6,
            tail: 0.5,
            length: 0.84,
            height: 1.0,
            ring: false,
        },
        _ => Profile {
            nose: 0.5,
            bulge: 0.75,
            tail: 0.6,
            length: 0.88,
            height: 1.05,
            ring: false,
        },
    }
}

/// Build the schematic for the current ship inside `frame` (the content area the
/// silhouette should fill). Pure and deterministic: given the same sim it returns
/// byte-identical geometry.
pub fn build(sim: &SimState, data: &GameData, frame: Rect) -> ShipSchematic {
    let profile = hull_profile(&sim.ship.hull);
    let hull_name = data
        .ship_components
        .find(ComponentKind::Hull, &sim.ship.hull)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| sim.ship.hull.clone());

    let cy = frame.y + frame.h * 0.5;
    let cx = frame.x + frame.w * 0.5;

    // Rooms hug the corridor at a FIXED offset, independent of the hull — so a
    // room can never overflow a lean hull nor float in a broad one. The hull is
    // then sized to enclose them; class shape only ever varies the outline.
    let row_offset = ROW_OFFSET;
    // Hull half-height at mid-body: enough to wrap the tallest room, then scaled
    // up for bulky classes.
    let base_max_h = row_offset + ROOM_SLOT_H * 0.5 + 20.0;
    let max_h = base_max_h * profile.height;

    // Length-scaled, centred span: a corvette is visibly shorter, an ark longer.
    let hull_span = frame.w * 0.86 * profile.length;
    let sx0 = cx - hull_span * 0.5;
    let sx1 = cx + hull_span * 0.5;
    let x_at = |t: f32| sx0 + t * (sx1 - sx0);

    // Control stations (t, height-fraction) mirrored top/bottom into a closed
    // outline. Mid is full (encloses rooms); the shoulders stay high enough to
    // wrap the room columns; only the bow and stern taper by nose/tail.
    let shoulder = 0.82 + profile.bulge * 0.13;
    let bow = 0.12 + (1.0 - profile.nose) * 0.26;
    let tail = 0.22 + profile.tail * 0.30;
    let stations = [
        (0.00, bow),
        (0.12, (bow + shoulder) * 0.5),
        (0.30, shoulder),
        (0.50, 1.0),
        (0.70, shoulder),
        (0.88, (tail + shoulder) * 0.5),
        (1.00, tail),
    ];
    let mut outline: Vec<Vec2> = stations
        .iter()
        .map(|&(t, f)| vec2(x_at(t), cy - f * max_h))
        .collect();
    for &(t, f) in stations.iter().rev() {
        outline.push(vec2(x_at(t), cy + f * max_h));
    }

    // The spun-gravity ring frames the central habitat, seated just outside the
    // centre rooms rather than slicing through them.
    let ring = profile
        .ring
        .then(|| (vec2(cx, cy), row_offset + ROOM_SLOT_H * 0.5 + 10.0));

    let mut modules = Vec::new();

    // --- Bridge (bow) and Engine (stern): the two anchor components ---
    let bridge_manned = any_post_aboard(sim, BRIDGE_POSTS);
    modules.push(component_glyph(
        "bridge",
        "COMMAND BRIDGE",
        "CMD",
        Rect::new(x_at(0.05) - 6.0, cy - 18.0, 66.0, 36.0),
        ModuleKind::Bridge,
        sim.ship.hull_integrity,
        bridge_manned,
    ));

    let engine_label = data
        .ship_components
        .find(ComponentKind::Engine, &sim.ship.engine)
        .map(|c| c.name.to_uppercase())
        .unwrap_or_else(|| sim.ship.engine.to_uppercase());
    modules.push(component_glyph(
        &sim.ship.engine,
        &engine_label,
        "DRV",
        Rect::new(x_at(0.95) - 64.0, cy - 22.0, 72.0, 44.0),
        ModuleKind::Engine,
        // The engine reads by how much reaction mass it has to work with.
        sim.ship.fuel,
        any_post_aboard(sim, &["engineer"]),
    ));

    // --- Weapon mount (dorsal), only when one is fitted ---
    if let Some(weapon_id) = sim.ship.weapon.as_deref() {
        let label = data
            .ship_components
            .find(ComponentKind::Weapon, weapon_id)
            .map(|c| c.name.to_uppercase())
            .unwrap_or_else(|| weapon_id.to_uppercase());
        modules.push(component_glyph(
            weapon_id,
            &label,
            "WPN",
            Rect::new(x_at(0.5) - 34.0, cy - max_h - 30.0, 68.0, 24.0),
            ModuleKind::Weapon,
            sim.ship.hull_integrity,
            false,
        ));
    }

    // --- Subsystem compartments in a modular grid that reflows for any count ---
    // Two decks flank the corridor; columns are sized to the compartment count so
    // rooms can be added, removed, or resized without the layout breaking. The
    // first row fills the upper deck left-to-right, the remainder the lower deck,
    // each lower box seated directly under its upper-deck column.
    let ids = GameData::sorted_ids(&data.subsystems);
    let cols = ids.len().div_ceil(2).max(1);
    let (t_lo, t_hi) = (0.20_f32, 0.80_f32);
    for (i, id) in ids.iter().enumerate() {
        let Some(state) = sim.subsystems.get(id) else {
            continue;
        };
        let col = i % cols;
        let upper = i < cols;
        let t = t_lo + (col as f32 + 0.5) / cols as f32 * (t_hi - t_lo);
        // Boxes grow with tier, so an upgrade is visible as a larger module. A
        // mission-reward 4th version reads at the tier-3 size (the slot's cap) so
        // it never overflows into its neighbours.
        let visual_tier = state.tier.min(3) as f32;
        let w = 58.0 + visual_tier * 9.0;
        let h = 30.0 + visual_tier * 5.0;
        let band_y = if upper {
            cy - row_offset
        } else {
            cy + row_offset
        };
        let rect = Rect::new(x_at(t) - w * 0.5, band_y - h * 0.5, w, h);
        let (deck_lo, deck_hi) = deck_range(id);
        modules.push(ModuleGlyph {
            id: id.clone(),
            label: subsystem_short(id).to_owned(),
            code: subsystem_code(id).to_owned(),
            rect,
            kind: ModuleKind::Subsystem,
            condition: state.condition,
            tier: state.tier,
            manned: any_post_aboard(sim, subsystem_crew_posts(id)),
            deck_lo,
            deck_hi,
        });
    }

    ShipSchematic {
        hull_id: sim.ship.hull.clone(),
        hull_name,
        outline,
        // Corridor runs between the bridge and engine boxes as its terminals,
        // rather than passing through them.
        corridor: (vec2(x_at(0.05) + 60.0, cy), vec2(x_at(0.95) - 64.0, cy)),
        ring,
        modules,
        hull_integrity: sim.ship.hull_integrity,
        life_support: sim.ship.life_support,
        fuel: sim.ship.fuel,
        spare_parts: sim.ship.spare_parts,
        stats: loadout_stats(sim, data),
    }
}

fn component_glyph(
    id: &str,
    label: &str,
    code: &str,
    rect: Rect,
    kind: ModuleKind,
    condition: f32,
    manned: bool,
) -> ModuleGlyph {
    let (deck_lo, deck_hi) = deck_range(id);
    ModuleGlyph {
        id: id.to_owned(),
        label: label.to_owned(),
        code: code.to_owned(),
        rect,
        kind,
        condition,
        tier: 0,
        manned,
        deck_lo,
        deck_hi,
    }
}

#[cfg(test)]
mod tests;
