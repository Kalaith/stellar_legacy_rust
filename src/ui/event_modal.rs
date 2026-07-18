//! Blocking council-decision modals: events (GDD §9 step 4) and legacy
//! dilemmas (GDD §5.5).

use crate::simulation::legacy::pending_dilemma_def;
use crate::ui::{term, term_button, GameplayCtx, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

/// Characters-per-second for the terminal reveal of modal body text.
const REVEAL_CPS: f32 = 55.0;

pub fn draw(ctx: &GameplayCtx<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let Some(pending) = &ctx.sim.pending_event else {
        return;
    };
    let Some(template) = ctx.data.events.get(&pending.template_id) else {
        return;
    };

    let header = format!(
        "COUNCIL DECISION — {}",
        template.category.label().to_uppercase()
    );
    let content = modal_frame(&header, template.outcomes.len(), term::alert());
    let mut y = content.y + 22.0;
    draw_ui_text_ex(
        &template.title,
        content.x,
        y,
        TextStyle::new(22.0, term::primary()).params(),
    );
    y += 20.0;
    draw_typed_block(
        &template.description,
        content.x,
        y,
        content.w,
        ctx.modal_reveal,
    );
    y += 84.0;

    for (i, outcome) in template.outcomes.iter().enumerate() {
        let card = Rect::new(content.x, y, content.w, 84.0);
        draw_surface(
            card,
            &SurfaceStyle::new(Color::new(0.08, 0.065, 0.015, 1.0)).with_border(1.0, term::faint()),
        );
        draw_text_block(
            &outcome.description,
            card.x + 14.0,
            card.y + 8.0,
            card.w - 200.0,
            60.0,
            13.0,
            3.0,
            term::dim(),
        );
        if term_button(
            Rect::new(card.right() - 178.0, card.y + 24.0, 164.0, 36.0),
            &outcome.label.to_uppercase(),
            true,
            mouse,
        ) {
            actions.push(UiAction::ResolveEvent(i));
        }
        y += 96.0;
    }
}

/// Blocking legacy-dilemma modal. Options show their success odds up front —
/// the roll is honest, so the interface is too (Pillar 3).
pub fn draw_dilemma(ctx: &GameplayCtx<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let Some(dilemma) = pending_dilemma_def(ctx.sim, ctx.data) else {
        return;
    };
    let legacy_name = ctx
        .data
        .legacies
        .get(&ctx.sim.legacy.legacy_id)
        .map(|l| l.name.clone())
        .unwrap_or_default();

    let header = format!("LEGACY DILEMMA — {}", legacy_name.to_uppercase());
    let content = modal_frame(&header, dilemma.options.len(), term::primary());
    let mut y = content.y + 22.0;
    draw_ui_text_ex(
        &dilemma.title,
        content.x,
        y,
        TextStyle::new(22.0, term::primary()).params(),
    );
    y += 20.0;
    draw_typed_block(
        &dilemma.description,
        content.x,
        y,
        content.w,
        ctx.modal_reveal,
    );
    y += 84.0;

    for (i, option) in dilemma.options.iter().enumerate() {
        let card = Rect::new(content.x, y, content.w, 84.0);
        draw_surface(
            card,
            &SurfaceStyle::new(Color::new(0.08, 0.065, 0.015, 1.0)).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            &format!("Success odds: {:.0}%", option.success_chance * 100.0),
            card.x + 14.0,
            card.y + 24.0,
            TextStyle::new(13.0, term::accent()).params(),
        );
        draw_text_block(
            &option.success.log,
            card.x + 14.0,
            card.y + 34.0,
            card.w - 200.0,
            40.0,
            12.0,
            3.0,
            term::faint(),
        );
        if term_button(
            Rect::new(card.right() - 178.0, card.y + 24.0, 164.0, 36.0),
            &option.label.to_uppercase(),
            true,
            mouse,
        ) {
            actions.push(UiAction::ResolveDilemma(i));
        }
        y += 96.0;
    }
}

/// Dim the world and draw the modal surface with `header` in the title band;
/// returns the content rect.
fn modal_frame(header: &str, option_count: usize, accent: Color) -> Rect {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.75),
    );

    let height = 240.0 + option_count as f32 * 96.0;
    let rect = Rect::new(
        LOGICAL_WIDTH / 2.0 - 330.0,
        (LOGICAL_HEIGHT - height) / 2.0,
        660.0,
        height,
    );
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.06, 0.05, 0.012, 1.0))
            .with_border(2.0, accent)
            .with_header(40.0, term::panel_header())
            .with_header_divider(1.0, accent),
    );
    draw_text_centered_in_box_ex(
        header,
        rect.x,
        rect.y,
        rect.w,
        40.0,
        TextStyle::new(15.0, accent),
    );
    rect.inset(26.0)
}

/// Word-wrapped body text revealed left-to-right terminal style, with a
/// blinking underscore cursor while it is still typing.
fn draw_typed_block(text: &str, x: f32, y: f32, w: f32, reveal: f32) {
    let shown = typed_prefix(text, reveal, REVEAL_CPS);
    let cursor = if !is_fully_typed(text, reveal, REVEAL_CPS) && (reveal * 2.5).fract() < 0.5 {
        "_"
    } else {
        ""
    };
    draw_text_block(
        &format!("{shown}{cursor}"),
        x,
        y,
        w,
        70.0,
        14.0,
        4.0,
        term::dim(),
    );
}
