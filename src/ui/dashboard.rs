//! Dashboard: ship vitals, population, advance-time control, ship's log.

use crate::data::ship_components::ComponentKind;
use crate::simulation::ship::RepairKind;
use crate::state::sim::{GameSpeed, PopulationState};
use crate::state::Screen;
use crate::ui::{
    spec_line, stat_line, status_badge, term, term_button, term_meter, term_meter_toned, term_panel,
    GaugeIcon, GameplayCtx, MeterTone, UiAction,
};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    // Reserve a full-width instrument strip along the bottom (the mockup's
    // systems readout); the three panels share the space above it.
    let strip_h = 64.0;
    let panels_h = area.h - strip_h - 12.0;
    let left = Rect::new(area.x, area.y, 380.0, panels_h);
    let mid = Rect::new(area.x + 392.0, area.y, 380.0, panels_h);
    let right = Rect::new(area.x + 784.0, area.y, area.w - 784.0, panels_h);

    draw_ship_panel(ctx, left, mouse, actions);
    draw_colony_panel(ctx, mid);
    draw_log_panel(ctx, right);
    draw_systems_strip(
        ctx,
        Rect::new(area.x, area.y + panels_h + 12.0, area.w, strip_h),
    );
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
        "HULL INTEGRITY",
        &format!("{:.0}%", sim.ship.hull_integrity * 100.0),
    );
    y += 32.0;
    term_meter(
        Rect::new(content.x, y, content.w, 22.0),
        sim.ship.life_support,
        1.0,
        "LIFE SUPPORT",
        &format!("{:.0}%", sim.ship.life_support * 100.0),
    );
    y += 32.0;
    term_meter(
        Rect::new(content.x, y, content.w, 22.0),
        sim.ship.fuel,
        1.0,
        "FUEL RESERVES",
        &format!("{:.0}%", sim.ship.fuel * 100.0),
    );
    y += 34.0;

    // Spare parts ease yearly wear (PLAN M4.2); when the stores hit zero the
    // ship wears at full rate, so flag it red. Shown as a spec line below.
    let parts_dry = sim.ship.spare_parts <= 0;
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
    y += 18.0;
    draw_text_block(
        &contract_line,
        content.x,
        y - 12.0,
        content.w,
        36.0,
        14.0,
        3.0,
        term::accent(),
    );
    y += 32.0;

    // Ship-class readout: the installed hull/drive and their real cargo/berth
    // capacities (GDD §6), the count of subsystems still online, and armament.
    // A thin divider sets the spec block off from the vitals above it.
    draw_line(
        content.x,
        y - 6.0,
        content.right(),
        y - 6.0,
        1.0,
        term::faint(),
    );
    let hull = ctx
        .data
        .ship_components
        .find(ComponentKind::Hull, &sim.ship.hull);
    let engine = ctx
        .data
        .ship_components
        .find(ComponentKind::Engine, &sim.ship.engine);
    let hull_name = hull.map(|c| c.name.as_str()).unwrap_or("—");
    let drive_name = engine.map(|c| c.name.as_str()).unwrap_or("—");
    let cargo = hull.map(|c| c.stats.cargo).unwrap_or(0);
    let berths = hull.map(|c| c.stats.crew_capacity).unwrap_or(0);
    let total_systems = sim.subsystems.len();
    let online = sim
        .subsystems
        .values()
        .filter(|s| s.condition > 0.15)
        .count();
    let armament = sim
        .ship
        .weapon
        .as_deref()
        .and_then(|id| {
            ctx.data
                .ship_components
                .find(ComponentKind::Weapon, id)
                .map(|c| c.name.clone())
        })
        .unwrap_or_else(|| "UNARMED".to_owned());
    for (label, value, color) in [
        ("SHIP CLASS", hull_name.to_owned(), term::primary()),
        ("DRIVE", drive_name.to_owned(), term::primary()),
        (
            "CARGO / BERTHS",
            format!("{cargo} · {berths}"),
            term::accent(),
        ),
        (
            "SYSTEMS ONLINE",
            format!("{online} / {total_systems}"),
            if online < total_systems {
                term::alert()
            } else {
                term::accent()
            },
        ),
        (
            "SPARE PARTS",
            sim.ship.spare_parts.to_string(),
            if parts_dry {
                term::alert()
            } else {
                term::accent()
            },
        ),
        ("ARMAMENT", armament, term::accent()),
    ] {
        spec_line(content.x, y, content.w, label, &value, color);
        y += 18.0;
    }
    y += 6.0;

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
    y += 20.0;
    let field_affordable = |stat: f32| {
        stat < repair.field_ceiling
            && sim.ship.spare_parts >= repair.field_parts_cost
            && sim.resources.minerals >= repair.field_minerals_cost
    };
    if term_button(
        Rect::new(content.x, y, content.w, 24.0),
        &format!(
            "FIELD REPAIR HULL ({}p·{}min)",
            repair.field_parts_cost, repair.field_minerals_cost
        ),
        field_affordable(sim.ship.hull_integrity),
        mouse,
    ) {
        actions.push(UiAction::FieldRepair(RepairKind::Hull));
    }
    y += 27.0;
    if term_button(
        Rect::new(content.x, y, content.w, 24.0),
        &format!(
            "FIELD REPAIR LIFE SPT ({}p·{}min)",
            repair.field_parts_cost, repair.field_minerals_cost
        ),
        field_affordable(sim.ship.life_support),
        mouse,
    ) {
        actions.push(UiAction::FieldRepair(RepairKind::LifeSupport));
    }
    y += 27.0;
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
        Rect::new(content.x, y, content.w, 24.0),
        &full_label,
        full_ok,
        mouse,
    ) {
        actions.push(UiAction::FullRepair);
    }
    y += 34.0;

    // Extinction is handled by the full-screen game-over takeover
    // (`ui::game_over`), so the dashboard never renders in that state.
    //
    // Time control (real-time loop §1): under way the month clock auto-advances;
    // the row pauses it or sets the 1×/2×/3× rate. Docked, time is frozen no
    // matter the setting, so the row disables and says so. The row sits at the
    // panel foot, just below maintenance.
    let underway = sim.contract.is_some();
    draw_ui_text_ex(
        if underway {
            "TIME CONTROL"
        } else {
            "TIME CONTROL — IN DRYDOCK, TIME PAUSED"
        },
        content.x,
        y,
        TextStyle::new(13.0, if underway { term::dim() } else { term::faint() }).params(),
    );
    y += 12.0;

    let gap = 6.0;
    let bw = (content.w - gap * 3.0) / 4.0;
    for (i, step) in GameSpeed::ALL.iter().enumerate() {
        let r = Rect::new(content.x + (bw + gap) * i as f32, y, bw, 36.0);
        let active = underway && sim.speed == *step;
        let label = if active {
            format!("[{}]", step.label())
        } else {
            step.label().to_owned()
        };
        if term_button(r, &label, underway, mouse) {
            actions.push(UiAction::SetSpeed(*step));
        }
    }
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
            label,
            &format!("{:.0}%", value * 100.0),
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

    // Population breakdown: the peoples actually aboard and their head counts
    // (GDD §5.1, the same aggregate the crew screen lists), shown as a tile row
    // pinned to the panel foot so the colony's makeup reads at a glance.
    let aboard: Vec<_> = ctx.sim.factions.iter().filter(|f| f.is_aboard()).collect();
    if !aboard.is_empty() {
        let row_h = 52.0;
        let card = Rect::new(
            content.x,
            content.bottom() - row_h - 22.0,
            content.w,
            row_h + 22.0,
        );
        draw_ui_text_ex(
            "POPULATION BREAKDOWN",
            card.x,
            card.y + 12.0,
            TextStyle::new(13.0, term::primary()).params(),
        );
        let tiles = aboard.len().min(4);
        let gap = 8.0;
        let tw = (card.w - gap * (tiles as f32 - 1.0)) / tiles as f32;
        for (i, fs) in aboard.iter().take(tiles).enumerate() {
            let tile = Rect::new(card.x + i as f32 * (tw + gap), card.bottom() - row_h, tw, row_h);
            draw_surface(
                tile,
                &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
            );
            let name = ctx
                .data
                .factions
                .get(&fs.faction_id)
                .map(|d| short_faction(&d.name))
                .unwrap_or_else(|| fs.faction_id.to_uppercase());
            status_badge(
                tile.inset(6.0),
                GaugeIcon::People,
                &name,
                &fs.members.to_string(),
                term::accent(),
            );
        }
    }
    let _ = Screen::Dashboard;
}

/// Shorten a faction name for a narrow tile: drop a leading article and keep the
/// most distinctive word, uppercased ("The Ascension Circle" → "ASCENSION").
fn short_faction(name: &str) -> String {
    name.split_whitespace()
        .find(|w| !matches!(w.to_ascii_lowercase().as_str(), "the" | "of" | "first"))
        .unwrap_or(name)
        .to_uppercase()
}

/// The bottom instrument strip: a row of system gauges reading straight from
/// ship + colony state — hull, life support, fuel, the years of food the stores
/// cover, the maintenance posture, and a count of systems currently in the red.
fn draw_systems_strip(ctx: &GameplayCtx<'_>, rect: Rect) {
    term_panel(rect, None);
    let sim = ctx.sim;
    let inner = rect.inset(10.0);

    let food_per_year = ctx.data.config.food_per_person_per_year * sim.population.count.max(1) as f32;
    let food_years = if food_per_year > 0.0 {
        sim.resources.food as f32 / food_per_year
    } else {
        0.0
    };
    let parts_dry = sim.ship.spare_parts <= 0;
    let alerts = [
        sim.ship.hull_integrity < 0.35,
        sim.ship.life_support < 0.35,
        sim.ship.fuel < 0.2,
        parts_dry,
        food_years < 3.0,
    ]
    .iter()
    .filter(|f| **f)
    .count();

    let tone = |ok: bool| if ok { term::accent() } else { term::alert() };
    let cells: [(GaugeIcon, &str, String, Color); 6] = [
        (
            GaugeIcon::Hull,
            "HULL",
            format!("{:.0}%", sim.ship.hull_integrity * 100.0),
            tone(sim.ship.hull_integrity >= 0.35),
        ),
        (
            GaugeIcon::Life,
            "LIFE SUPPORT",
            format!("{:.0}%", sim.ship.life_support * 100.0),
            tone(sim.ship.life_support >= 0.35),
        ),
        (
            GaugeIcon::Fuel,
            "FUEL",
            format!("{:.0}%", sim.ship.fuel * 100.0),
            tone(sim.ship.fuel >= 0.2),
        ),
        (
            GaugeIcon::Food,
            "FOOD SUPPLY",
            format!("{food_years:.0} YRS"),
            tone(food_years >= 3.0),
        ),
        (
            GaugeIcon::Maint,
            "MAINTENANCE",
            if parts_dry { "STORES DRY" } else { "ON SCHEDULE" }.to_owned(),
            tone(!parts_dry),
        ),
        (
            GaugeIcon::Alert,
            "SHIP ALERTS",
            alerts.to_string(),
            tone(alerts == 0),
        ),
    ];
    let n = cells.len();
    let cw = inner.w / n as f32;
    for (i, (icon, label, value, color)) in cells.into_iter().enumerate() {
        let cell = Rect::new(inner.x + i as f32 * cw, inner.y, cw, inner.h);
        status_badge(cell, icon, label, &value, color);
        if i + 1 < n {
            draw_line(
                cell.right(),
                cell.y + 6.0,
                cell.right(),
                cell.bottom() - 6.0,
                1.0,
                term::faint(),
            );
        }
    }
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
            &format!("Y{}·M{:02}", entry.year, entry.month),
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
