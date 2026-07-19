//! Dashboard: ship vitals, population, advance-time control, ship's log.

use crate::simulation::ship::RepairKind;
use crate::state::sim::PopulationState;
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

    // Spare parts ease yearly wear (PLAN M4.2); when the stores hit zero the
    // ship wears at full rate, so flag it red.
    let parts_dry = sim.ship.spare_parts <= 0;
    stat_line(
        content.x,
        y,
        "SPARE PARTS",
        &sim.ship.spare_parts.to_string(),
        if parts_dry {
            term::alert()
        } else {
            term::accent()
        },
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
        TextStyle::new(15.0, term::dim()).params(),
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
        term::accent(),
    );
    y += 44.0;

    // Maintenance (PLAN M4.3). Field repairs patch the ship underway from
    // spare parts + minerals but can't reach pristine; a full refit is
    // port-only. Buttons enable only when the action is currently possible.
    let repair = ctx.data.config.repair;
    let in_port = sim.contract.is_none();
    draw_ui_text_ex(
        "MAINTENANCE",
        content.x,
        y,
        TextStyle::new(15.0, term::dim()).params(),
    );
    y += 22.0;
    let field_affordable = |stat: f32| {
        stat < repair.field_ceiling
            && sim.ship.spare_parts >= repair.field_parts_cost
            && sim.resources.minerals >= repair.field_minerals_cost
    };
    if term_button(
        Rect::new(content.x, y, content.w, 26.0),
        &format!(
            "FIELD REPAIR HULL ({}p·{}min)",
            repair.field_parts_cost, repair.field_minerals_cost
        ),
        field_affordable(sim.ship.hull_integrity),
        mouse,
    ) {
        actions.push(UiAction::FieldRepair(RepairKind::Hull));
    }
    y += 30.0;
    if term_button(
        Rect::new(content.x, y, content.w, 26.0),
        &format!(
            "FIELD REPAIR LIFE SPT ({}p·{}min)",
            repair.field_parts_cost, repair.field_minerals_cost
        ),
        field_affordable(sim.ship.life_support),
        mouse,
    ) {
        actions.push(UiAction::FieldRepair(RepairKind::LifeSupport));
    }
    y += 30.0;
    let full_label = if in_port {
        format!(
            "FULL REFIT ({}cr·{}min)",
            repair.full_credits_cost, repair.full_minerals_cost
        )
    } else {
        "FULL REFIT — PORT ONLY".to_owned()
    };
    let full_ok = in_port
        && sim.resources.credits >= repair.full_credits_cost
        && sim.resources.minerals >= repair.full_minerals_cost;
    if term_button(
        Rect::new(content.x, y, content.w, 26.0),
        &full_label,
        full_ok,
        mouse,
    ) {
        actions.push(UiAction::FullRepair);
    }

    // Extinction is handled by the full-screen game-over takeover
    // (`ui::game_over`), so the dashboard never renders in that state.
    let advance = Rect::new(content.x, content.bottom() - 56.0, content.w, 48.0);
    let can_advance = sim.pending_event.is_none() && !sim.dynasty.extinct;
    if term_button(advance, "ADVANCE ONE YEAR  [SPACE]", can_advance, mouse) {
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
        term::accent(),
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
        term::primary(),
    );
    y += 24.0;
    stat_line(
        content.x,
        y,
        "CONSEQUENCES CARRIED",
        &ctx.sim.consequences.len().to_string(),
        if ctx.sim.consequences.is_empty() {
            term::accent()
        } else {
            term::alert()
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
        term::primary(),
    );
    y += 24.0;

    // How far this crew has drifted from the hopeful founders who cast off
    // (PLAN M4.1). Voyage drift makes this climb over a long run. The
    // percentage sits in the stat column; the evocative descriptor gets its
    // own full-width line so neither collides with the label.
    let dist = founder_distance(pop);
    let dist_color = if dist < 0.5 {
        term::primary()
    } else {
        term::alert()
    };
    stat_line(
        content.x,
        y,
        "FROM THE FOUNDING",
        &format!("{:.0}%", dist * 100.0),
        dist_color,
    );
    y += 18.0;
    draw_ui_text_ex(
        &format!("> {}", founder_distance_label(dist)),
        content.x,
        y,
        TextStyle::new(13.0, term::dim()).params(),
    );
    let _ = Screen::Dashboard;
}

/// How far the population has diverged from the founding crew (0 = as the
/// founders were, 1 = unrecognizable), a composite of risen adaptation, risen
/// cultural drift, and faded legacy loyalty. Baselines mirror the founding
/// values set in `SimState::new_campaign`.
fn founder_distance(pop: &PopulationState) -> f32 {
    const F_ADAPT: f32 = 0.3;
    const F_DRIFT: f32 = 0.1;
    const F_LOYALTY: f32 = 0.6;
    let a = ((pop.adaptation - F_ADAPT) / (1.0 - F_ADAPT)).clamp(0.0, 1.0);
    let d = ((pop.cultural_drift - F_DRIFT) / (1.0 - F_DRIFT)).clamp(0.0, 1.0);
    let l = ((F_LOYALTY - pop.legacy_loyalty) / F_LOYALTY).clamp(0.0, 1.0);
    (a + d + l) / 3.0
}

fn founder_distance_label(distance: f32) -> &'static str {
    match distance {
        x if x < 0.15 => "true to the founding",
        x if x < 0.40 => "quietly diverging",
        x if x < 0.65 => "a changed people",
        x if x < 0.85 => "distant from the founders",
        _ => "unrecognizable",
    }
}

/// Characters-per-second for the newest log line streaming in.
const LOG_CPS: f32 = 45.0;

fn draw_log_panel(ctx: &GameplayCtx<'_>, rect: Rect) {
    term_panel(rect, Some("SHIP'S LOG"));
    let content = rect.inset(18.0);
    let line_h = 34.0;
    let visible = ((content.h - 44.0) / line_h).floor() as usize;
    let total = ctx.sim.log.len();
    let start = total.saturating_sub(visible);

    let mut y = content.y + 44.0;
    for (i, entry) in ctx.sim.log.iter().enumerate().skip(start) {
        draw_ui_text_ex(
            &format!("Y{:03}", entry.year),
            content.x,
            y,
            TextStyle::new(13.0, term::faint()).params(),
        );
        // The newest line streams in like live console output, with a blinking
        // cursor while it types; older lines are shown in full.
        let newest = i + 1 == total;
        let shown = if newest {
            let mut text = typed_prefix(&entry.text, ctx.log_reveal, LOG_CPS).to_owned();
            if !is_fully_typed(&entry.text, ctx.log_reveal, LOG_CPS) && blink(ctx.log_reveal, 2.5) {
                text.push('_');
            }
            text
        } else {
            entry.text.clone()
        };
        draw_text_block(
            &shown,
            content.x + 46.0,
            y - 12.0,
            content.w - 46.0,
            30.0,
            13.0,
            2.0,
            term::dim(),
        );
        y += line_h;
    }
}
