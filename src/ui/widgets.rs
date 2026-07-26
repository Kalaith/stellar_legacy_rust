//! The shared terminal-styled widgets every screen draws with: panels,
//! buttons, bars, meters and the little gauge icons beside them.

use super::*;

pub fn term_panel(rect: Rect, title: Option<&str>) {
    let style = SurfaceStyle::new(term::panel())
        .with_border(1.0, term::border())
        .with_header(34.0, term::panel_header())
        .with_header_divider(1.0, term::border());
    if let Some(title) = title {
        draw_surface_with_title(
            rect,
            Some(title),
            &style,
            TextStyle::new(17.0, term::primary()),
        );
    } else {
        draw_surface(
            rect,
            &SurfaceStyle::new(term::panel()).with_border(1.0, term::border()),
        );
    }
}

pub fn term_button(rect: Rect, label: &str, enabled: bool, mouse: Vec2) -> bool {
    let hovered = enabled && rect.contains_point(mouse);
    let fill = if !enabled {
        term::surface_disabled()
    } else if hovered {
        term::surface_hover()
    } else {
        term::surface()
    };
    let border = if enabled { term::dim() } else { term::faint() };
    draw_surface(rect, &SurfaceStyle::new(fill).with_border(1.0, border));
    draw_text_centered_in_box_ex(
        label,
        rect.x + 6.0,
        rect.y,
        rect.w - 12.0,
        rect.h,
        TextStyle::new(
            16.0,
            if enabled {
                term::primary()
            } else {
                term::faint()
            },
        ),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}

/// Which end of a meter is the dangerous one, for the critical-red highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterTone {
    /// A low reading is bad (hull, morale, fuel…): red under 35%.
    LowCritical,
    /// A high reading is bad (cultural drift…): red over 65%.
    HighCritical,
    /// Neither end is inherently bad (adaptation): never red.
    Neutral,
}

/// The shared readout gauge behind every meter in the UI. A recessed track
/// holds a segment-notched fill; the `label` sits flush left and the `value`
/// flush right so neither straddles the fill edge — the old centered label was
/// unreadable exactly where it crossed the fill boundary. The label reads dark
/// over a covering fill and switches to the tube's light dim where the fill
/// falls short, so it stays legible at any level.
pub fn term_bar(rect: Rect, frac: f32, fill: Color, label: &str, value: &str) {
    let frac = frac.clamp(0.0, 1.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
    );
    let fill_w = (rect.w - 2.0) * frac;
    if fill_w > 0.5 {
        draw_rectangle(rect.x + 1.0, rect.y + 1.0, fill_w, rect.h - 2.0, fill);
        // Notch the filled span into even segments for the terminal-gauge read.
        // The notch is only faintly darker than the fill so it reads as a ruled
        // gauge without chopping the dark label that sits over it.
        let seg = 12.0;
        let notch = Color::new(0.0, 0.0, 0.0, 0.18);
        let mut sx = rect.x + 1.0 + seg;
        let fill_right = rect.x + 1.0 + fill_w;
        while sx < fill_right {
            draw_rectangle(sx, rect.y + 1.0, 2.0, rect.h - 2.0, notch);
            sx += seg;
        }
    }
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, term::border());

    let font = 14.0;
    let baseline = rect.y + rect.h * 0.5 + font * 0.36;
    // The label reads as near-black over a fill that covers it, and switches to
    // the bright tube color where the fill falls short so it stays legible on
    // the dark track. The char-width estimate slightly overshoots, keeping the
    // switch conservative.
    let covered = fill_w >= label.chars().count() as f32 * font * 0.62 + 12.0;
    let label_color = if covered { term::bg() } else { term::primary() };
    // A soft dark shadow keeps a bright label legible where it overhangs the
    // green fill onto the dark track (short bars like cultural drift).
    if !covered {
        draw_ui_text_ex(
            label,
            rect.x + 9.0,
            baseline + 1.0,
            TextStyle::new(font, Color::new(0.0, 0.0, 0.0, 0.55)).params(),
        );
    }
    draw_ui_text_ex(
        label,
        rect.x + 8.0,
        baseline,
        TextStyle::new(font, label_color).params(),
    );
    let value_color = if frac > 0.9 {
        term::bg()
    } else {
        term::accent()
    };
    draw_text_right(
        value,
        rect.right() - 8.0,
        baseline,
        TextStyle::new(font, value_color),
    );
}

pub fn term_meter(rect: Rect, value: f32, max: f32, label: &str, value_text: &str) {
    term_meter_toned(rect, value, max, label, value_text, MeterTone::LowCritical);
}

pub fn term_meter_toned(
    rect: Rect,
    value: f32,
    max: f32,
    label: &str,
    value_text: &str,
    tone: MeterTone,
) {
    let frac = if max > 0.0 { value / max } else { 0.0 };
    let critical = match tone {
        MeterTone::LowCritical => frac < 0.35,
        MeterTone::HighCritical => frac > 0.65,
        MeterTone::Neutral => false,
    };
    let fill = if critical {
        term::alert()
    } else {
        term::accent()
    };
    term_bar(rect, frac, fill, label, value_text);
}

pub fn stat_line(x: f32, y: f32, label: &str, value: &str, value_color: Color) {
    draw_ui_text_ex(label, x, y, TextStyle::new(15.0, term::dim()).params());
    draw_text_right(value, x + 250.0, y, TextStyle::new(15.0, value_color));
}

/// A dense spec row: dimmed label flush left, value flush right at `x + w`.
/// Used for the ship-class readout and other two-column stat blocks that want
/// the value pinned to a panel's right edge rather than a fixed column.
pub fn spec_line(x: f32, y: f32, w: f32, label: &str, value: &str, value_color: Color) {
    draw_ui_text_ex(label, x, y, TextStyle::new(14.0, term::dim()).params());
    draw_text_right(value, x + w, y, TextStyle::new(14.0, value_color));
}

/// Minimal line-art glyphs for the status gauges. Drawn from primitives (no
/// texture assets) so they tint with the active phosphor and stay crisp at any
/// scale. The labels name each gauge, so these read as distinguishing marks
/// rather than literal illustrations.
#[derive(Debug, Clone, Copy)]
pub enum GaugeIcon {
    Hull,
    Life,
    Fuel,
    Food,
    Maint,
    Alert,
    People,
}

pub fn draw_gauge_icon(icon: GaugeIcon, cx: f32, cy: f32, r: f32, color: Color) {
    let t = 1.6;
    match icon {
        GaugeIcon::Hull => draw_poly_lines(cx, cy, 6, r, 90.0, t, color),
        GaugeIcon::Life => {
            draw_circle_lines(cx, cy, r, t, color);
            draw_circle_lines(cx, cy, r * 0.42, t, color);
        }
        GaugeIcon::Fuel => draw_poly_lines(cx, cy, 4, r, 45.0, t, color),
        GaugeIcon::Food => {
            draw_line(cx, cy - r, cx, cy + r, t, color);
            for i in 0..3 {
                let yy = cy - r * 0.7 + i as f32 * r * 0.65;
                draw_line(cx, yy, cx - r * 0.6, yy + r * 0.4, t, color);
                draw_line(cx, yy, cx + r * 0.6, yy + r * 0.4, t, color);
            }
        }
        GaugeIcon::Maint => {
            draw_poly_lines(cx, cy, 8, r, 22.5, t, color);
            draw_circle_lines(cx, cy, r * 0.4, t, color);
        }
        GaugeIcon::Alert => {
            draw_triangle_lines(
                vec2(cx, cy - r),
                vec2(cx - r, cy + r * 0.85),
                vec2(cx + r, cy + r * 0.85),
                t,
                color,
            );
            draw_line(cx, cy - r * 0.25, cx, cy + r * 0.35, t, color);
            draw_circle(cx, cy + r * 0.62, t * 0.9, color);
        }
        GaugeIcon::People => {
            draw_circle_lines(cx, cy - r * 0.4, r * 0.42, t, color);
            draw_line(
                cx - r * 0.8,
                cy + r * 0.85,
                cx + r * 0.8,
                cy + r * 0.85,
                t,
                color,
            );
            draw_line(
                cx - r * 0.8,
                cy + r * 0.85,
                cx - r * 0.5,
                cy + r * 0.15,
                t,
                color,
            );
            draw_line(
                cx + r * 0.8,
                cy + r * 0.85,
                cx + r * 0.5,
                cy + r * 0.15,
                t,
                color,
            );
        }
    }
}

/// A system-status cell: a ringed line-art icon at the left with a dimmed label
/// over a bright value to its right — the mockup's bottom instrument strip and
/// the dashboard demographic tiles are rows of these.
pub fn status_badge(rect: Rect, icon: GaugeIcon, label: &str, value: &str, tone: Color) {
    let cy = rect.y + rect.h * 0.5;
    let r = (rect.h * 0.5 - 7.0).clamp(10.0, 17.0);
    let cx = rect.x + r + 6.0;
    draw_circle_lines(cx, cy, r + 5.0, 1.0, term::faint());
    draw_gauge_icon(icon, cx, cy, r * 0.72, tone);
    let tx = cx + r + 16.0;
    draw_ui_text_ex(
        label,
        tx,
        cy - 3.0,
        TextStyle::new(12.0, term::dim()).params(),
    );
    draw_ui_text_ex(value, tx, cy + 15.0, TextStyle::new(15.0, tone).params());
}
