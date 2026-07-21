//! PREP screen (W4): the pre-launch beat. Shows the selected charter's phase
//! plan and a provisioning readout (food / parts / fuel need vs stores) with
//! stock-up buttons per store, and commits the voyage with the explicit
//! [ LAUNCH ] button. Pure view — it emits `SelectCharter` / `Launch` /
//! `Refuel` / `Buy` / `BuyParts` only.

use crate::data::contracts::ContractPhase;
use crate::simulation::crew::production_multipliers;
use crate::state::sim::TradeResource;
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.55, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);

    draw_prep(ctx, left, mouse, actions);

    // Swap column: the charter list, so a different charter can be selected.
    // Cards start below the panel's header band so they never overlap its title.
    term_panel(right, Some("CHOOSE / SWAP CHARTER"));
    let inner = right.inset(18.0);
    let cards = Rect::new(inner.x, inner.y + 28.0, inner.w, inner.h - 28.0);
    crate::ui::contract_systems::draw_charter_cards(ctx, cards, mouse, actions);
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

    // Each provisioning row carries its own stock-up button so filling the
    // stores never means leaving the PREP screen.
    let stock_btn = |y: f32| Rect::new(content.right() - 200.0, y - 17.0, 194.0, 26.0);

    // Food: need over the whole voyage vs stores, plus the net yearly balance
    // production offsets it by (crew-multiplied). The button buys the shortfall
    // at market price, capped at what the treasury can afford.
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
    let food_short = (food_need - sim.resources.food).max(0);
    let food_price = crate::simulation::market::price_of(sim, TradeResource::Food);
    let food_afford = if food_price > 0.0 {
        (sim.resources.credits as f32 / food_price).floor() as i64
    } else {
        0
    };
    let food_buy = food_short.min(food_afford);
    let food_cost = (food_price * food_buy as f32).ceil() as i64;
    let food_label = if food_short == 0 {
        "FOOD STOCKED".to_owned()
    } else if food_buy <= 0 {
        "NO CREDITS FOR FOOD".to_owned()
    } else {
        format!("+{food_buy} FOOD · {food_cost} CR")
    };
    if term_button(stock_btn(y), &food_label, food_buy > 0, mouse) {
        actions.push(UiAction::Buy(TradeResource::Food, food_buy));
    }
    y += 30.0;

    // Spare parts: yearly upkeep across the voyage vs stores. The button stocks
    // the shortfall at the drydock part price, capped by the treasury.
    let parts_need = config.parts_upkeep_per_year * template.target_duration_years as i64;
    provision_line(
        content.x,
        y,
        "PARTS",
        sim.ship.spare_parts,
        parts_need,
        "or restock via a full refit",
    );
    let parts_short = (parts_need - sim.ship.spare_parts).max(0);
    let part_price = config.provisioning.part_cost_credits;
    let parts_afford = if part_price > 0 {
        sim.resources.credits / part_price
    } else {
        0
    };
    let parts_buy = parts_short.min(parts_afford);
    let parts_label = if parts_short == 0 {
        "PARTS STOCKED".to_owned()
    } else if parts_buy <= 0 {
        "NO CREDITS FOR PARTS".to_owned()
    } else {
        format!("+{parts_buy} PARTS · {} CR", parts_buy * part_price)
    };
    if term_button(stock_btn(y), &parts_label, parts_buy > 0, mouse) {
        actions.push(UiAction::BuyParts(parts_buy));
    }
    y += 30.0;

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

    // --- First-voyage checklist (tutorial) ---
    // Shown until the Chronicle records a mission or the player dismisses it;
    // every label and tip is authored in game_config, per the data rule.
    if !sim.tutorial_dismissed && ctx.chronicle.entries.is_empty() {
        let step_done = |id: &str| match id {
            "choose_charter" => true, // being on PREP means one is selected
            "stock_food" => food_short == 0,
            "stock_parts" => parts_short == 0,
            "fuel_tanks" => sim.ship.fuel >= 0.999,
            // "launch" (and anything unknown) completes only by doing it.
            _ => false,
        };
        let steps = &ctx.data.config.tutorial.steps;
        let boxed = Rect::new(
            content.x,
            y + 20.0,
            content.w,
            92.0 + steps.len() as f32 * 22.0,
        );
        draw_surface(
            boxed,
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            "FIRST VOYAGE // PRE-LAUNCH CHECKLIST",
            boxed.x + 12.0,
            boxed.y + 22.0,
            TextStyle::new(14.0, term::primary()).params(),
        );
        if term_button(
            Rect::new(boxed.right() - 92.0, boxed.y + 8.0, 84.0, 22.0),
            "DISMISS",
            true,
            mouse,
        ) {
            actions.push(UiAction::DismissTutorial);
        }

        let active = steps.iter().position(|s| !step_done(&s.id));
        let mut sy = boxed.y + 46.0;
        for (i, step) in steps.iter().enumerate() {
            let done = step_done(&step.id);
            let (mark, color) = if done {
                ("[x]", term::accent())
            } else if active == Some(i) {
                ("[>]", term::primary())
            } else {
                ("[ ]", term::dim())
            };
            draw_ui_text_ex(
                &format!("{mark} {}", step.label),
                boxed.x + 12.0,
                sy,
                TextStyle::new(13.0, color).params(),
            );
            sy += 22.0;
        }
        // The tip for whatever the voyage needs next.
        if let Some(step) = active.and_then(|i| steps.get(i)) {
            draw_text_block(
                &step.tip,
                boxed.x + 12.0,
                sy + 4.0,
                boxed.w - 24.0,
                40.0,
                12.0,
                3.0,
                term::dim(),
            );
        }
    }

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
