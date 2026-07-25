//! Market: buy/sell the four tradeable resources with price trends (GDD §5.1).

use crate::simulation::ship::loadout_stats;
use crate::state::sim::TradeResource;
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(area, Some("COMMODITY EXCHANGE"));
    let content = area.inset(24.0);
    let mut y = content.y + 46.0;

    // Trade lot scales with the ship's cargo capacity (PLAN item 3).
    let lot = (loadout_stats(ctx.sim, ctx.data).cargo.max(50)) as i64;

    draw_ui_text_ex(
        &format!(
            "SHIP TREASURY: {} CREDITS   ·   HOLD {} (lot size)",
            ctx.sim.resources.credits, lot
        ),
        content.x,
        y,
        TextStyle::new(17.0, term::accent()).params(),
    );
    y += 34.0;

    // Header row — column captions aligned to the row cards below.
    let col_held = 240.0;
    let col_price = 400.0;
    let col_trend = 560.0;
    for (label, offset) in [
        ("COMMODITY", 0.0),
        ("HELD", col_held),
        ("PRICE", col_price),
        ("TREND", col_trend),
    ] {
        draw_ui_text_ex(
            label,
            content.x + 14.0 + offset,
            y,
            TextStyle::new(13.0, term::faint()).params(),
        );
    }
    y += 12.0;

    // Each commodity gets its own bordered row card, so the ledger reads as a
    // set of instruments rather than a bare table adrift in empty space.
    let row_h = 50.0;
    for entry in &ctx.sim.market.entries {
        let row = Rect::new(content.x, y, content.w, row_h);
        draw_surface(
            row,
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        let tb = row.y + row_h * 0.5 + 5.0;
        let cx = row.x + 14.0;
        let held = match entry.resource {
            TradeResource::Energy => ctx.sim.resources.energy,
            TradeResource::Minerals => ctx.sim.resources.minerals,
            TradeResource::Food => ctx.sim.resources.food,
            TradeResource::Influence => ctx.sim.resources.influence,
        };
        draw_ui_text_ex(
            entry.resource.label(),
            cx,
            tb,
            TextStyle::new(16.0, term::primary()).params(),
        );
        draw_ui_text_ex(
            &held.to_string(),
            cx + col_held,
            tb,
            TextStyle::new(16.0, term::accent()).params(),
        );
        draw_ui_text_ex(
            &format!("{:.1} cr", entry.price),
            cx + col_price,
            tb,
            TextStyle::new(16.0, term::primary()).params(),
        );
        let (arrow, color) = if entry.trend > 0.005 {
            ("▲", term::accent())
        } else if entry.trend < -0.005 {
            ("▼", term::alert())
        } else {
            ("—", term::dim())
        };
        draw_ui_text_ex(
            &format!("{arrow} {:+.2}", entry.trend),
            cx + col_trend,
            tb,
            TextStyle::new(16.0, color).params(),
        );

        let bh = 30.0;
        let by = row.y + (row_h - bh) * 0.5;
        let buy_rect = Rect::new(row.right() - 292.0, by, 132.0, bh);
        let sell_rect = Rect::new(row.right() - 146.0, by, 132.0, bh);
        if term_button(buy_rect, &format!("BUY {lot}"), true, mouse) {
            actions.push(UiAction::Buy(entry.resource, lot));
        }
        if term_button(sell_rect, &format!("SELL {lot}"), held >= lot, mouse) {
            actions.push(UiAction::Sell(entry.resource, lot));
        }
        y += row_h + 10.0;
    }

    y += 22.0;
    draw_text_block(
        "Prices drift each year the ship advances. Buy low before a long leg; sell what the next generation won't need.",
        content.x,
        y,
        content.w,
        40.0,
        13.0,
        3.0,
        term::dim(),
    );
}
