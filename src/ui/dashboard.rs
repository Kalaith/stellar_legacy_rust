//! Dashboard: ship vitals, population, advance-time control, ship's log.

use crate::state::Screen;
use crate::ui::{
    stat_line, term, term_button, term_meter, term_meter_toned, term_panel, GameplayCtx, MeterTone,
    UiAction,
};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, 380.0, area.h);
    let mid = Rect::new(area.x + 392.0, area.y, 380.0, area.h);
    let right = Rect::new(area.x + 784.0, area.y, area.w - 784.0, area.h);

    draw_ship_panel(ctx, left, mouse, actions);
    draw_colony_panel(ctx, mid);
    draw_log_panel(ctx, right);
}

fn draw_ship_panel(ctx: &GameplayCtx<'_>, rect: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("SHIP STATUS"));
    let content = rect.inset(20.0);
    let mut y = content.y + 40.0;
    let sim = ctx.sim;

    term_meter(
        Rect::new(content.x, y, content.w, 22.0),
        sim.ship.hull_integrity,
        1.0,
        &format!("HULL {:.0}%", sim.ship.hull_integrity * 100.0),
    );
    y += 32.0;
    term_meter(
        Rect::new(content.x, y, content.w, 22.0),
        sim.ship.life_support,
        1.0,
        &format!("LIFE SUPPORT {:.0}%", sim.ship.life_support * 100.0),
    );
    y += 32.0;
    term_meter(
        Rect::new(content.x, y, content.w, 22.0),
        sim.ship.fuel,
        1.0,
        &format!("FUEL {:.0}%", sim.ship.fuel * 100.0),
    );
    y += 40.0;

    stat_line(
        content.x,
        y,
        "SPARE PARTS",
        &sim.ship.spare_parts.to_string(),
        term::GREEN,
    );
    y += 26.0;
    let contract_line = sim
        .contract
        .as_ref()
        .map(|c| format!("{} ({:.0}%)", c.name, c.progress() * 100.0))
        .unwrap_or_else(|| "NONE — accept one on CONTRACT".to_owned());
    draw_ui_text_ex(
        "ACTIVE CONTRACT",
        content.x,
        y,
        TextStyle::new(15.0, term::AMBER_DIM).params(),
    );
    y += 20.0;
    draw_text_block(
        &contract_line,
        content.x,
        y - 12.0,
        content.w,
        40.0,
        14.0,
        3.0,
        term::GREEN,
    );
    y += 44.0;

    if sim.dynasty.extinct {
        draw_text_block(
            "THE DYNASTY IS EXTINCT. The ship sails on, unmanned by any bloodline. Retire this voyage from the CHRONICLE screen.",
            content.x,
            y,
            content.w,
            60.0,
            14.0,
            3.0,
            term::RED,
        );
        y += 70.0;
    }

    let advance = Rect::new(content.x, content.bottom() - 56.0, content.w, 48.0);
    let can_advance = sim.pending_event.is_none() && !sim.dynasty.extinct;
    if term_button(advance, "ADVANCE ONE YEAR >>", can_advance, mouse) {
        actions.push(UiAction::AdvanceYear);
    }
    let _ = y;
}

fn draw_colony_panel(ctx: &GameplayCtx<'_>, rect: Rect) {
    term_panel(rect, Some("SHIP-CITY POPULATION"));
    let content = rect.inset(20.0);
    let mut y = content.y + 40.0;
    let pop = &ctx.sim.population;

    stat_line(
        content.x,
        y,
        "POPULATION",
        &pop.count.to_string(),
        term::GREEN,
    );
    y += 30.0;

    // Most meters read low-is-bad; adaptation is neutral and cultural drift is
    // high-is-bad, so their critical-red highlight is toned accordingly.
    let bars: [(&str, f32, MeterTone); 6] = [
        ("MORALE", pop.morale, MeterTone::LowCritical),
        ("UNITY", pop.unity, MeterTone::LowCritical),
        ("STABILITY", pop.stability, MeterTone::LowCritical),
        ("LEGACY LOYALTY", pop.legacy_loyalty, MeterTone::LowCritical),
        ("ADAPTATION", pop.adaptation, MeterTone::Neutral),
        (
            "CULTURAL DRIFT",
            pop.cultural_drift,
            MeterTone::HighCritical,
        ),
    ];
    for (label, value, tone) in bars {
        term_meter_toned(
            Rect::new(content.x, y, content.w, 20.0),
            value,
            1.0,
            &format!("{label} {:.0}%", value * 100.0),
            tone,
        );
        y += 30.0;
    }

    y += 12.0;
    let legacy = &ctx.sim.legacy;
    stat_line(
        content.x,
        y,
        "TRADITION",
        &legacy.tradition_points.to_string(),
        term::AMBER,
    );
    y += 24.0;
    stat_line(
        content.x,
        y,
        "CONSEQUENCES CARRIED",
        &ctx.sim.consequences.len().to_string(),
        if ctx.sim.consequences.is_empty() {
            term::GREEN
        } else {
            term::RED
        },
    );
    y += 24.0;
    stat_line(
        content.x,
        y,
        "DELEGATED DOMAINS",
        &format!(
            "{}",
            [
                ctx.sim.delegation.immediate_crisis,
                ctx.sim.delegation.generational_challenge,
                ctx.sim.delegation.mission_milestone,
                ctx.sim.delegation.legacy_moment,
            ]
            .iter()
            .filter(|d| **d)
            .count()
        ),
        term::AMBER,
    );
    let _ = Screen::Dashboard;
}

fn draw_log_panel(ctx: &GameplayCtx<'_>, rect: Rect) {
    term_panel(rect, Some("SHIP'S LOG"));
    let content = rect.inset(18.0);
    let line_h = 34.0;
    let visible = ((content.h - 44.0) / line_h).floor() as usize;
    let start = ctx.sim.log.len().saturating_sub(visible);

    let mut y = content.y + 44.0;
    for entry in ctx.sim.log.iter().skip(start) {
        draw_ui_text_ex(
            &format!("Y{:03}", entry.year),
            content.x,
            y,
            TextStyle::new(13.0, term::AMBER_FAINT).params(),
        );
        draw_text_block(
            &entry.text,
            content.x + 46.0,
            y - 12.0,
            content.w - 46.0,
            30.0,
            13.0,
            2.0,
            term::AMBER_DIM,
        );
        y += line_h;
    }
}
