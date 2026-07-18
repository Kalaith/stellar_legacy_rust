//! Chronicle: completed contracts across playthroughs (GDD §7).
//!
//! TODO(next agent, M2/M3): end-of-playthrough summary flow and Heritage
//! modifier selection when starting a new save.

use crate::ui::{term, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, _mouse: Vec2, _actions: &mut Vec<UiAction>) {
    term_panel(area, Some("THE CHRONICLE"));
    let content = area.inset(24.0);
    let mut y = content.y + 46.0;

    if ctx.chronicle.entries.is_empty() {
        draw_text_block(
            "No voyages recorded yet.\n\nEvery completed contract is written here, and the Chronicle outlives any single save. In time, past voyages will grant Heritage modifiers to new dynasties.",
            content.x,
            y,
            content.w * 0.7,
            120.0,
            15.0,
            5.0,
            term::AMBER_DIM,
        );
        return;
    }

    for entry in ctx.chronicle.entries.iter().rev().take(10) {
        draw_ui_text_ex(
            &format!(
                "Y{:03} — {} [{}]",
                entry.completed_year, entry.contract_name, entry.outcome
            ),
            content.x,
            y,
            TextStyle::new(16.0, term::AMBER).params(),
        );
        draw_ui_text_ex(
            &format!(
                "   {} charter · gen {} · under {} · score {:.2}",
                entry.objective, entry.generation, entry.leader_name, entry.score
            ),
            content.x,
            y + 18.0,
            TextStyle::new(13.0, term::AMBER_DIM).params(),
        );
        y += 46.0;
    }
}
