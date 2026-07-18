//! Blocking council-decision modal (GDD §9 step 4).

use crate::ui::{term, term_button, GameplayCtx, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let Some(pending) = &ctx.sim.pending_event else {
        return;
    };
    let Some(template) = ctx.data.events.get(&pending.template_id) else {
        return;
    };

    // Dim the world behind the decision.
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.75),
    );

    let height = 240.0 + template.outcomes.len() as f32 * 96.0;
    let rect = Rect::new(
        LOGICAL_WIDTH / 2.0 - 330.0,
        (LOGICAL_HEIGHT - height) / 2.0,
        660.0,
        height,
    );
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.06, 0.05, 0.012, 1.0))
            .with_border(2.0, term::RED)
            .with_header(40.0, term::PANEL_HEADER)
            .with_header_divider(1.0, term::RED),
    );

    let content = rect.inset(26.0);
    let mut y = content.y + 22.0;
    draw_ui_text_ex(
        &format!(
            "COUNCIL DECISION — {}",
            template.category.label().to_uppercase()
        ),
        content.x,
        y,
        TextStyle::new(14.0, term::RED).params(),
    );
    y += 30.0;
    draw_ui_text_ex(
        &template.title,
        content.x,
        y,
        TextStyle::new(22.0, term::AMBER).params(),
    );
    y += 20.0;
    draw_text_block(
        &template.description,
        content.x,
        y,
        content.w,
        70.0,
        14.0,
        4.0,
        term::AMBER_DIM,
    );
    y += 84.0;

    for (i, outcome) in template.outcomes.iter().enumerate() {
        let card = Rect::new(content.x, y, content.w, 84.0);
        draw_surface(
            card,
            &SurfaceStyle::new(Color::new(0.08, 0.065, 0.015, 1.0))
                .with_border(1.0, term::AMBER_FAINT),
        );
        draw_text_block(
            &outcome.description,
            card.x + 14.0,
            card.y + 8.0,
            card.w - 200.0,
            60.0,
            13.0,
            3.0,
            term::AMBER_DIM,
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
