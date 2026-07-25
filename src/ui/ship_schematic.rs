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

use crate::data::ship_components::{ComponentKind, ComponentStats};
use crate::data::GameData;
use crate::simulation::ship::loadout_stats;
use crate::state::sim::SimState;
use crate::ui::term;
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
        // Boxes grow with tier, so an upgrade is visible as a larger module.
        let w = 58.0 + state.tier as f32 * 9.0;
        let h = 30.0 + state.tier as f32 * 5.0;
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
        corridor: (vec2(x_at(0.05), cy), vec2(x_at(0.95), cy)),
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

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Highlight tone by condition, matching the meter convention: healthy reads
/// bright accent, worn dims, failing goes alert-red.
fn condition_tone(c: f32) -> Color {
    if c < 0.35 {
        term::alert()
    } else if c < 0.66 {
        term::dim()
    } else {
        term::accent()
    }
}

/// Draw the schematic (silhouette, ring, and every module glyph with its label)
/// inside `frame`.
pub fn draw(frame: Rect, schematic: &ShipSchematic) {
    // Hull outline as a closed blueprint stroke.
    let n = schematic.outline.len();
    for i in 0..n {
        let a = schematic.outline[i];
        let b = schematic.outline[(i + 1) % n];
        draw_line(a.x, a.y, b.x, b.y, 1.5, term::border());
    }
    if let Some((c, r)) = schematic.ring {
        draw_circle_lines(c.x, c.y, r, 1.5, term::dim());
    }

    // The corridor: a twin-line spine running bow to stern, the bus every
    // compartment taps into.
    let (a, b) = schematic.corridor;
    draw_line(a.x, a.y - 2.0, b.x, b.y - 2.0, 1.0, term::dim());
    draw_line(a.x, a.y + 2.0, b.x, b.y + 2.0, 1.0, term::dim());

    // Branch connectors: a stub tying each compartment back to the corridor, so
    // the layout reads as a wired schematic rather than floating boxes.
    for glyph in &schematic.modules {
        let r = glyph.rect;
        match glyph.kind {
            ModuleKind::Subsystem if r.center().y < a.y => {
                draw_line(
                    r.center().x,
                    r.bottom(),
                    r.center().x,
                    a.y,
                    1.0,
                    term::faint(),
                );
            }
            ModuleKind::Subsystem => {
                draw_line(r.center().x, r.y, r.center().x, a.y, 1.0, term::faint());
            }
            // The weapon taps the hull spine from its dorsal mount.
            ModuleKind::Weapon => {
                draw_line(
                    r.center().x,
                    r.bottom(),
                    r.center().x,
                    r.bottom() + 12.0,
                    1.0,
                    term::faint(),
                );
            }
            _ => {}
        }
    }

    for glyph in &schematic.modules {
        draw_glyph(frame, glyph);
    }
}

fn draw_glyph(frame: Rect, glyph: &ModuleGlyph) {
    let r = glyph.rect;
    let tone = condition_tone(glyph.condition);
    // Dark inset fill; the colored border carries the condition signal.
    draw_surface(
        r,
        &SurfaceStyle::new(term::surface_inset()).with_border(1.5, tone),
    );

    // Standardized tag inside the box, in the condition tone — the scannable
    // in-box identity, seated above the pip row.
    draw_text_centered_in_box_ex(
        &glyph.code,
        r.x,
        r.y,
        r.w,
        r.h - 10.0,
        TextStyle::new(14.0, tone),
    );

    // Tier pips (subsystems only), matching the subsystems screen convention.
    if glyph.kind == ModuleKind::Subsystem {
        for t in 0..3 {
            let cx = r.x + 8.0 + t as f32 * 9.0;
            let cy = r.bottom() - 6.0;
            if glyph.tier > t {
                draw_circle(cx, cy, 2.6, term::accent());
            } else {
                draw_circle_lines(cx, cy, 2.6, 1.0, term::faint());
            }
        }
    }

    // Manned marker: a lit corner pip when a matching post is aboard.
    if glyph.manned {
        draw_circle(r.right() - 7.0, r.y + 7.0, 3.0, term::accent());
    } else {
        draw_circle_lines(r.right() - 7.0, r.y + 7.0, 3.0, 1.0, term::faint());
    }

    // Label placement. Subsystems pin to the deck-label bands at the top and
    // bottom of the frame (upper row above, lower row below), with a leader line
    // out to the box. The component glyphs (bridge, engine, weapon) sit on the
    // spine, so they label locally — right beside the box — which also keeps the
    // top-left legend area clear.
    let ship_cy = frame.y + frame.h * 0.5;
    let label_color = if glyph.manned {
        term::primary()
    } else {
        term::faint()
    };

    // The dorsal weapon labels to its side, clear of the centre-top captions; no
    // leader, since its mount branch already ties it to the hull.
    if glyph.kind == ModuleKind::Weapon {
        draw_ui_text_ex(
            &glyph.label,
            r.right() + 8.0,
            r.center().y + 4.0,
            TextStyle::new(12.0, label_color).params(),
        );
        return;
    }

    // Everything else: name on the deck band (subsystems) or just beside the box
    // (bridge/engine), with a leader tying label to box and a smaller, dimmer
    // deck caption beneath — a clear primary/secondary typographic split.
    let (ly, leader_from, leader_to) = match glyph.kind {
        ModuleKind::Bridge | ModuleKind::Engine => {
            let ly = r.bottom() + 18.0;
            (ly, r.bottom(), ly - 12.0)
        }
        ModuleKind::Subsystem if r.center().y < ship_cy => {
            let ly = frame.y + 26.0;
            (ly, r.y, ly + 6.0)
        }
        _ => {
            let ly = frame.bottom() - 24.0;
            (ly, r.bottom(), ly - 18.0)
        }
    };
    draw_line(
        r.center().x,
        leader_from,
        r.center().x,
        leader_to,
        1.0,
        term::faint(),
    );
    draw_text_centered(
        &glyph.label,
        r.center().x,
        ly,
        TextStyle::new(12.0, label_color),
    );
    if glyph.kind == ModuleKind::Subsystem {
        draw_text_centered(
            &format!("DECKS {}-{}", glyph.deck_lo, glyph.deck_hi),
            r.center().x,
            ly + 13.0,
            TextStyle::new(10.0, term::dim()),
        );
    }
}

/// The bottom SHIP STATUS strip: live integrity/fuel meters plus combat and the
/// active loadout modifiers, folding in the old text readout.
pub fn draw_status_strip(rect: Rect, schematic: &ShipSchematic) {
    let s = &schematic.stats;
    let gap = 8.0;
    let tiles = 6.0;
    let tw = (rect.w - gap * (tiles - 1.0)) / tiles;
    let meter_tile = |i: f32, label: &str, value: f32| {
        let tx = rect.x + i * (tw + gap);
        draw_surface(
            Rect::new(tx, rect.y, tw, rect.h),
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            label,
            tx + 10.0,
            rect.y + 20.0,
            TextStyle::new(11.0, term::dim()).params(),
        );
        meter(
            Rect::new(tx + 10.0, rect.y + 30.0, tw - 20.0, 16.0),
            value,
            1.0,
            condition_tone(value),
            Some(&format!("{:.0}%", value * 100.0)),
        );
    };
    let value_tile = |i: f32, label: &str, value: &str| {
        let tx = rect.x + i * (tw + gap);
        draw_surface(
            Rect::new(tx, rect.y, tw, rect.h),
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            label,
            tx + 10.0,
            rect.y + 20.0,
            TextStyle::new(11.0, term::dim()).params(),
        );
        draw_ui_text_ex(
            value,
            tx + 10.0,
            rect.y + 42.0,
            TextStyle::new(16.0, term::accent()).params(),
        );
    };

    meter_tile(0.0, "HULL INTEGRITY", schematic.hull_integrity);
    meter_tile(1.0, "LIFE SUPPORT", schematic.life_support);
    meter_tile(2.0, "FUEL", schematic.fuel);
    value_tile(3.0, "SPARE PARTS", &schematic.spare_parts.to_string());
    value_tile(4.0, "COMBAT", &s.combat.to_string());

    // Sixth tile: the aggregate loadout modifiers, so the picture still names
    // what the parts are doing.
    let mut mods = Vec::new();
    if s.cargo != 0 {
        mods.push(format!("CARGO {}", s.cargo));
    }
    if s.speed != 0 {
        mods.push(format!("SPD {}", s.speed));
    }
    if s.fuel_regen != 0 {
        mods.push(format!("FUEL+{}", s.fuel_regen));
    }
    let tx = rect.x + 5.0 * (tw + gap);
    draw_surface(
        Rect::new(tx, rect.y, tw, rect.h),
        &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
    );
    draw_ui_text_ex(
        "MODIFIERS",
        tx + 10.0,
        rect.y + 20.0,
        TextStyle::new(11.0, term::dim()).params(),
    );
    let mods_text = if mods.is_empty() {
        "—".to_owned()
    } else {
        mods.join("  ")
    };
    draw_ui_text_ex(
        &mods_text,
        tx + 10.0,
        rect.y + 42.0,
        TextStyle::new(13.0, term::primary()).params(),
    );
}

#[cfg(test)]
mod tests {
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
}
