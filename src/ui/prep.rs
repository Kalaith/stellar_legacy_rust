//! PREP screen (W4): the pre-launch beat. Shows the selected charter's phase
//! plan and a provisioning readout (food / parts / fuel need vs stores), and
//! commits the voyage with the explicit [ LAUNCH ] button. Pure view — it emits
//! `SelectCharter` / `Launch` / `Refuel` only.

use crate::data::contracts::ContractPhase;
use crate::simulation::crew::production_multipliers;
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.55, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);

    draw_prep(ctx, left, mouse, actions);

    // Swap column: the charter list, so a different charter can be selected.
    term_panel(right, Some("CHOOSE / SWAP CHARTER"));
    crate::ui::contract_systems::draw_charter_cards(ctx, right.inset(18.0), mouse, actions);
}

/// One `LABEL — have / need` provisioning line, reddened when short.
fn provision_line(x: f32, y: f32, label: &str, have: i64, need: i64, note: &str) {
    let color = if have < need {
        term::alert()
    } else {
        term::accent()
    };
    let tail = if note.is_empty() {
        String::new()
    } else {
        format!("   ·   {note}")
    };
    draw_ui_text_ex(
        &format!("{label} — have {have} / need {need}{tail}"),
        x,
        y,
        TextStyle::new(13.0, color).params(),
    );
}

fn draw_prep(ctx: &GameplayCtx<'_>, rect: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let sim = ctx.sim;
    let Some(id) = sim.selected_charter.as_deref() else {
        return;
    };
    let Some(template) = ctx.data.contracts.get(id) else {
        return;
    };
    let config = &ctx.data.config;

    term_panel(rect, Some("PREP // DEPARTURE"));
    let content = rect.inset(18.0);
    let mut y = content.y + 38.0;

    draw_ui_text_ex(
        &template.name,
        content.x,
        y,
        TextStyle::new(19.0, term::accent()).params(),
    );
    y += 24.0;
    draw_ui_text_ex(
        &format!(
            "{} · {} YEARS · reward {} cr",
            template.objective.label().to_uppercase(),
            template.target_duration_years,
            template.reward.credits
        ),
        content.x,
        y,
        TextStyle::new(13.0, term::dim()).params(),
    );
    y += 28.0;

    // --- Phase plan (authored segments, proportional) ---
    draw_ui_text_ex(
        "PHASE PLAN",
        content.x,
        y,
        TextStyle::new(14.0, term::primary()).params(),
    );
    y += 12.0;
    let total_years = template.target_duration_years.max(1) as f32;
    let bar = Rect::new(content.x, y, content.w, 22.0);
    let mut bx = bar.x;
    for seg in &template.phases {
        let w = bar.w * (seg.years as f32 / total_years);
        let seg_rect = Rect::new(bx, bar.y, (w - 3.0).max(1.0), bar.h);
        draw_surface(
            seg_rect,
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            &format!("{} {}y", seg.kind.label().to_uppercase(), seg.years),
            seg_rect.x + 5.0,
            seg_rect.y + 15.0,
            TextStyle::new(10.0, term::dim()).params(),
        );
        bx += w;
    }
    y += 36.0;

    // --- Provisioning readout ---
    draw_ui_text_ex(
        "PROVISIONING vs VOYAGE",
        content.x,
        y,
        TextStyle::new(14.0, term::primary()).params(),
    );
    y += 22.0;
    let dur = template.target_duration_years as f32;

    // Food: need over the whole voyage vs stores, plus the net yearly balance
    // production offsets it by (crew-multiplied).
    let food_need = (sim.population.count as f32 * config.food_per_person_per_year * dur) as i64;
    let food_mult = production_multipliers(sim, ctx.data).food;
    let net_food = sim.production.food * food_mult
        - sim.population.count as f32 * config.food_per_person_per_year;
    provision_line(
        content.x,
        y,
        "FOOD ",
        sim.resources.food,
        food_need,
        &format!("net {net_food:+.0}/yr from production"),
    );
    y += 22.0;

    // Spare parts: yearly upkeep across the voyage vs stores.
    let parts_need = config.parts_upkeep_per_year * template.target_duration_years as i64;
    provision_line(
        content.x,
        y,
        "PARTS",
        sim.ship.spare_parts,
        parts_need,
        "restock via a full refit",
    );
    y += 22.0;

    // Fuel: burned only across Travel months; the tank caps at 1.0 and the
    // engine regen tops it up underway, so need can exceed a single tank.
    let travel_months: u32 = template
        .phases
        .iter()
        .filter(|p| p.kind == ContractPhase::Travel)
        .map(|p| p.years * 12)
        .sum();
    let fuel_need = config.provisioning.fuel_burn_per_travel_month * travel_months as f32;
    let fuel_color = if sim.ship.fuel < 1.0 {
        term::alert()
    } else {
        term::accent()
    };
    draw_ui_text_ex(
        &format!(
            "FUEL  — tank {:.0}%  ·  burn {:.2} over {} travel yrs (engine regen offsets)",
            sim.ship.fuel * 100.0,
            fuel_need,
            travel_months / 12
        ),
        content.x,
        y,
        TextStyle::new(13.0, fuel_color).params(),
    );
    y += 8.0;

    let _ = y;

    // --- Commit / refuel ---
    let refuel_missing = 1.0 - sim.ship.fuel;
    let refuel_cost =
        (config.provisioning.fuel_cost_credits_per_point as f32 * refuel_missing * 100.0).ceil()
            as i64;
    let by = content.bottom() - 44.0;
    let bw = (content.w - 12.0) / 2.0;
    if term_button(
        Rect::new(content.x, by, bw, 40.0),
        "[ LAUNCH ]",
        true,
        mouse,
    ) {
        actions.push(UiAction::Launch);
    }
    let refuel_label = if refuel_missing > 0.0 {
        format!("REFUEL ({refuel_cost} CR)")
    } else {
        "TANKS FULL".to_owned()
    };
    if term_button(
        Rect::new(content.x + bw + 12.0, by, bw, 40.0),
        &refuel_label,
        refuel_missing > 0.0 && sim.resources.credits >= refuel_cost,
        mouse,
    ) {
        actions.push(UiAction::Refuel);
    }
}
