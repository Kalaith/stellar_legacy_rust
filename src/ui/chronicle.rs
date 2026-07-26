//! Chronicle: completed contracts across playthroughs, plus the achievement
//! roster (GDD §7, §10).

use crate::ui::{term, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, is_fully_visible, RectExt};

/// Vertical stride of one Chronicle entry, and the height of the entry itself.
const ENTRY_STRIDE: f32 = 46.0;
const ENTRY_H: f32 = 40.0;
/// Reserved at the panel's right edge for the scrollbar.
const GUTTER: f32 = 12.0;

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer, _actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.62, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);
    draw_log(ctx, left, pointer);
    draw_milestones(ctx, right);
}

fn draw_log(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer) {
    term_panel(area, Some("THE CHRONICLE"));
    let content = area.inset(24.0);
    let y = content.y + 46.0;

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

    // The Chronicle outlives any single save, so it only ever grows — and it
    // used to show its newest nine and drop the rest without saying so, which
    // for a record whose whole purpose is to be the long memory is the one
    // thing it must not do. It scrolls now, oldest still reachable.
    let view = Rect::new(
        content.x,
        y - 14.0,
        content.w,
        content.bottom() - (y - 14.0),
    );
    let content_h = ctx.chronicle.entries.len() as f32 * ENTRY_STRIDE;
    let mut scroll = ctx.chronicle_scroll.get();
    scroll.update_at(view, content_h, pointer.position);

    let mut row_top = view.y - scroll.offset();
    for entry in ctx.chronicle.entries.iter().rev() {
        let row = Rect::new(view.x, row_top, view.w - GUTTER, ENTRY_H);
        row_top += ENTRY_STRIDE;
        // macroquad has no scissor rect, so cull the partly-scrolled entries
        // rather than letting them spill past the panel.
        if !is_fully_visible(row, view) {
            continue;
        }
        draw_ui_text_ex(
            &format!(
                "Y{:03} — {} [{}]",
                entry.completed_year, entry.contract_name, entry.outcome
            ),
            row.x,
            row.y + 14.0,
            TextStyle::new(16.0, term::primary()).params(),
        );
        draw_ui_text_ex(
            &format!(
                "   {} charter · {} yr · gen {} · under {} · score {:.2}",
                entry.objective,
                entry.duration_years,
                entry.generation,
                entry.leader_name,
                entry.score
            ),
            row.x,
            row.y + 32.0,
            TextStyle::new(13.0, term::dim()).params(),
        );
    }

    scroll.draw_scrollbar_with(
        view,
        content_h,
        term::surface_inset(),
        term::dim(),
        term::primary(),
    );
    ctx.chronicle_scroll.set(scroll);
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
