//! Chronicle: completed contracts across playthroughs, plus the achievement
//! roster (GDD §7, §10).

use crate::ui::{term, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, _mouse: Vec2, _actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.62, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);
    draw_log(ctx, left);
    draw_milestones(ctx, right);
}

fn draw_log(ctx: &GameplayCtx<'_>, area: Rect) {
    term_panel(area, Some("THE CHRONICLE"));
    let content = area.inset(24.0);
    let mut y = content.y + 46.0;

    if ctx.chronicle.entries.is_empty() {
        draw_text_block(
            "No voyages recorded yet.\n\nEvery completed contract is written here, and the Chronicle outlives any single save. Past voyages grant Heritage modifiers to new dynasties.",
            content.x,
            y,
            content.w,
            120.0,
            15.0,
            5.0,
            term::dim(),
        );
        return;
    }

    for entry in ctx.chronicle.entries.iter().rev().take(9) {
        draw_ui_text_ex(
            &format!(
                "Y{:03} — {} [{}]",
                entry.completed_year, entry.contract_name, entry.outcome
            ),
            content.x,
            y,
            TextStyle::new(16.0, term::primary()).params(),
        );
        draw_ui_text_ex(
            &format!(
                "   {} charter · gen {} · under {} · score {:.2}",
                entry.objective, entry.generation, entry.leader_name, entry.score
            ),
            content.x,
            y + 18.0,
            TextStyle::new(13.0, term::dim()).params(),
        );
        y += 46.0;
    }
}

fn draw_milestones(ctx: &GameplayCtx<'_>, area: Rect) {
    let (unlocked, total) = ctx.achievements.progress();
    term_panel(area, Some("MILESTONES"));
    let content = area.inset(20.0);
    let mut y = content.y + 42.0;

    draw_ui_text_ex(
        &format!("UNLOCKED {unlocked} / {total}"),
        content.x,
        y,
        TextStyle::new(14.0, term::accent()).params(),
    );
    y += 28.0;

    for achievement in ctx.achievements.iter() {
        let (mark, name_color) = if achievement.unlocked {
            ("[x]", term::accent())
        } else {
            ("[ ]", term::dim())
        };
        draw_ui_text_ex(
            &format!("{mark} {}", achievement.name),
            content.x,
            y,
            TextStyle::new(15.0, name_color).params(),
        );
        draw_text_block(
            &achievement.description,
            content.x + 22.0,
            y + 6.0,
            content.w - 22.0,
            30.0,
            12.0,
            2.0,
            term::faint(),
        );
        y += 46.0;
    }
}
