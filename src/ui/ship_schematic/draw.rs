//! The rendering half of the schematic: turns a built [`ShipSchematic`] into
//! glyphs, icons and the status strip beneath them. Every macroquad call the
//! schematic makes lives here, so the layout half stays testable.
//!
//! [`ShipSchematic`]: super::ShipSchematic

use super::*;

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

/// A standardized line pictogram for each compartment type, drawn at `c` with
/// half-extent `s` in `color`. Keyed by subsystem id (or component kind), so a
/// room shows the same glyph on every hull — the schematic's icon vocabulary.
fn draw_icon(glyph: &ModuleGlyph, c: Vec2, s: f32, color: Color) {
    let stroke = 1.2;
    let line = |a: Vec2, b: Vec2| draw_line(a.x, a.y, b.x, b.y, stroke, color);
    let poly = |pts: &[Vec2]| {
        for i in 0..pts.len() {
            line(pts[i], pts[(i + 1) % pts.len()]);
        }
    };
    let key = match glyph.kind {
        ModuleKind::Bridge => "bridge",
        ModuleKind::Engine => "engine",
        ModuleKind::Weapon => "weapon",
        ModuleKind::Subsystem => glyph.id.as_str(),
    };
    match key {
        // A sprout: stem and two leaves.
        "agriculture" => {
            line(vec2(c.x, c.y + s), vec2(c.x, c.y - s * 0.3));
            line(vec2(c.x, c.y - s * 0.1), vec2(c.x - s, c.y - s * 0.8));
            line(vec2(c.x, c.y - s * 0.1), vec2(c.x + s, c.y - s * 0.8));
        }
        // The medical cross.
        "medical_bay" => {
            draw_line(c.x, c.y - s, c.x, c.y + s, 2.2, color);
            draw_line(c.x - s, c.y, c.x + s, c.y, 2.2, color);
        }
        // A gear: hub ring with radial teeth.
        "engineering_bay" => {
            draw_circle_lines(c.x, c.y, s * 0.55, stroke, color);
            for k in 0..6 {
                let ang = k as f32 * std::f32::consts::TAU / 6.0;
                let (sn, cs) = ang.sin_cos();
                line(
                    vec2(c.x + cs * s * 0.55, c.y + sn * s * 0.55),
                    vec2(c.x + cs * s, c.y + sn * s),
                );
            }
        }
        // A droplet: peaked top over a rounded base.
        "life_support_habitat" => {
            draw_circle_lines(c.x, c.y + s * 0.3, s * 0.55, stroke, color);
            line(vec2(c.x, c.y - s), vec2(c.x - s * 0.55, c.y + s * 0.15));
            line(vec2(c.x, c.y - s), vec2(c.x + s * 0.55, c.y + s * 0.15));
        }
        // An open book.
        "education_culture" => {
            line(vec2(c.x, c.y - s * 0.7), vec2(c.x, c.y + s * 0.7));
            poly(&[
                vec2(c.x, c.y - s * 0.7),
                vec2(c.x - s, c.y - s * 0.35),
                vec2(c.x - s, c.y + s * 0.7),
                vec2(c.x, c.y + s * 0.7),
            ]);
            poly(&[
                vec2(c.x, c.y - s * 0.7),
                vec2(c.x + s, c.y - s * 0.35),
                vec2(c.x + s, c.y + s * 0.7),
                vec2(c.x, c.y + s * 0.7),
            ]);
        }
        // A shield.
        "security" => {
            poly(&[
                vec2(c.x - s, c.y - s * 0.8),
                vec2(c.x + s, c.y - s * 0.8),
                vec2(c.x + s, c.y + s * 0.1),
                vec2(c.x, c.y + s),
                vec2(c.x - s, c.y + s * 0.1),
            ]);
        }
        // A forward chevron for the command bridge.
        "bridge" => {
            line(vec2(c.x + s * 0.5, c.y - s), vec2(c.x - s * 0.6, c.y));
            line(vec2(c.x - s * 0.6, c.y), vec2(c.x + s * 0.5, c.y + s));
        }
        // An engine nozzle with an exhaust tick.
        "engine" => {
            poly(&[
                vec2(c.x - s * 0.5, c.y - s),
                vec2(c.x + s * 0.5, c.y - s),
                vec2(c.x + s, c.y + s * 0.4),
                vec2(c.x - s, c.y + s * 0.4),
            ]);
            line(vec2(c.x, c.y + s * 0.5), vec2(c.x, c.y + s));
        }
        // A weapon crosshair.
        "weapon" => {
            draw_circle_lines(c.x, c.y, s * 0.7, stroke, color);
            draw_line(c.x - s, c.y, c.x + s, c.y, 1.0, color);
            draw_line(c.x, c.y - s, c.x, c.y + s, 1.0, color);
        }
        _ => {
            draw_circle_lines(c.x, c.y, s * 0.6, stroke, color);
        }
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

    // Branch connectors: a stub tying each compartment back to the corridor, with
    // a small junction node where it taps the bus — so the layout reads as a wired
    // schematic rather than floating boxes.
    for glyph in &schematic.modules {
        let r = glyph.rect;
        match glyph.kind {
            ModuleKind::Subsystem => {
                let from = if r.center().y < a.y { r.bottom() } else { r.y };
                draw_line(r.center().x, from, r.center().x, a.y, 1.0, term::faint());
                draw_circle(r.center().x, a.y, 2.0, term::dim());
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

    // In-box identity, all in the condition tone: a standardized pictogram on the
    // left, its 3-letter tag to the right, seated above the pip row.
    let icon_c = vec2(r.x + 13.0, r.y + (r.h - 8.0) * 0.5);
    draw_icon(glyph, icon_c, 6.5, tone);
    draw_text_centered_in_box_ex(
        &glyph.code,
        r.x + 20.0,
        r.y,
        r.w - 28.0,
        r.h - 10.0,
        TextStyle::new(13.0, tone),
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
        term_bar(
            Rect::new(tx + 10.0, rect.y + 30.0, tw - 20.0, 16.0),
            value,
            condition_tone(value),
            "",
            &format!("{:.0}%", value * 100.0),
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
        mods.join(" ")
    };
    // Fit the modifiers into the tile: a full loadout ("CARGO 400 SPD 2 FUEL+1")
    // overran a fixed-size line and spilled past the tile edges, so let the block
    // shrink and wrap to two lines within the cell instead.
    draw_text_block(
        &mods_text,
        tx + 10.0,
        rect.y + 28.0,
        tw - 20.0,
        rect.h - 34.0,
        13.0,
        3.0,
        term::primary(),
    );
}
