//! Market: buy/sell the four tradeable resources with price trends (GDD §5.1).

use crate::state::sim::TradeResource;
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

const LOT: i64 = 100;

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(area, Some("COMMODITY EXCHANGE"));
    let content = area.inset(24.0);
    let mut y = content.y + 46.0;

    draw_ui_text_ex(
        &format!("SHIP TREASURY: {} CREDITS", ctx.sim.resources.credits),
        content.x,
        y,
        TextStyle::new(17.0, term::accent()).params(),
    );
    y += 34.0;

    // Header row.
    for (label, offset) in [
        ("COMMODITY", 0.0),
        ("HELD", 220.0),
        ("PRICE", 360.0),
        ("TREND", 500.0),
    ] {
        draw_ui_text_ex(
            label,
            content.x + offset,
            y,
            TextStyle::new(13.0, term::faint()).params(),
        );
    }
    y += 10.0;

    for entry in &ctx.sim.market.entries {
        y += 44.0;
        let held = match entry.resource {
            TradeResource::Energy => ctx.sim.resources.energy,
            TradeResource::Minerals => ctx.sim.resources.minerals,
            TradeResource::Food => ctx.sim.resources.food,
            TradeResource::Influence => ctx.sim.resources.influence,
        };
        draw_ui_text_ex(
            entry.resource.label(),
            content.x,
            y,
            TextStyle::new(16.0, term::primary()).params(),
        );
        draw_ui_text_ex(
            &held.to_string(),
            content.x + 220.0,
            y,
            TextStyle::new(16.0, term::accent()).params(),
        );
        draw_ui_text_ex(
            &format!("{:.1} cr", entry.price),
            content.x + 360.0,
            y,
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
            content.x + 500.0,
            y,
            TextStyle::new(16.0, color).params(),
        );

        let buy_rect = Rect::new(content.x + 660.0, y - 22.0, 130.0, 30.0);
        let sell_rect = Rect::new(content.x + 800.0, y - 22.0, 130.0, 30.0);
        if term_button(buy_rect, &format!("BUY {LOT}"), true, mouse) {
            actions.push(UiAction::Buy(entry.resource, LOT));
        }
        if term_button(sell_rect, &format!("SELL {LOT}"), held >= LOT, mouse) {
            actions.push(UiAction::Sell(entry.resource, LOT));
        }
    }

    y += 60.0;
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
