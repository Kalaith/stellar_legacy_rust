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
    // Only the outcomes this ship has earned are offered (content-depth event
    // families round 12): a gated outcome — a fix only a kept-expert crew can try,
    // a path only a past choice unlocks — is hidden until its condition holds. The
    // real outcome index is preserved for `ResolveEvent`.
    let available = crate::simulation::event_resolver::available_outcome_indices(ctx.sim, template);
    let content = modal_frame(
        &header,
        countdown_secs(ctx.decision_remaining),
        available.len(),
        term::alert(),
    );
    let mut y = content.y + 30.0;
    draw_ui_text_ex(
        &template.title,
        content.x,
        y,
        TextStyle::new(22.0, term::primary()).params(),
    );
    y += 20.0;
    // Show the twist a riding complication adds (content-depth round 6): the
    // sim is paused, so this reads the same state the outcome will resolve in.
    let description = crate::simulation::event_resolver::shown_description(ctx.sim, template);
    draw_typed_block(&description, content.x, y, content.w, ctx.modal_reveal);
    y += 84.0;

    for (shown, &i) in available.iter().enumerate() {
        let outcome = &template.outcomes[i];
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
            48.0,
            13.0,
            3.0,
            term::dim(),
        );
        // The uncertain human cost of this choice (real-time loop §3): a band, not
        // a number — the council rarely knows exactly who a crisis will take.
        if let Some((lo, hi)) = crate::simulation::event_resolver::outcome_pop_impact_range(
            ctx.sim, ctx.data, template, i,
        ) {
            let (text, color) = impact_label(lo, hi);
            draw_ui_text_ex(
                &text,
                card.x + 14.0,
                card.y + 72.0,
                TextStyle::new(12.0, color).params(),
            );
        }
        if term_button(
            Rect::new(card.right() - 178.0, card.y + 24.0, 164.0, 36.0),
            &format!("[{}] {}", shown + 1, outcome.label.to_uppercase()),
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
    let content = modal_frame(
        &header,
        countdown_secs(ctx.decision_remaining),
        dilemma.options.len(),
        term::primary(),
    );
    let mut y = content.y + 30.0;
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
        let odds = crate::simulation::legacy::dilemma_odds(ctx.sim, ctx.data, option);
        // The shown odds are honest (Pillar 3): a combat-backed Wanderer gamble or
        // a faction-backed one reads higher, a faction-hindered one lower.
        let modifier = odds - option.success_chance;
        let odds_text = if modifier > 0.001 {
            format!(
                "Success odds: {:.0}%  (+{:.0}%)",
                odds * 100.0,
                modifier * 100.0
            )
        } else if modifier < -0.001 {
            format!(
                "Success odds: {:.0}%  ({:.0}%)",
                odds * 100.0,
                modifier * 100.0
            )
        } else {
            format!("Success odds: {:.0}%", odds * 100.0)
        };
        draw_ui_text_ex(
            &odds_text,
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
            &format!("[{}] {}", i + 1, option.label.to_uppercase()),
            true,
            mouse,
        ) {
            actions.push(UiAction::ResolveDilemma(i));
        }
        y += 96.0;
    }
}

/// Whole seconds left on the auto-resolve countdown, floored at 0 (real-time
/// loop §2).
fn countdown_secs(remaining: f32) -> i32 {
    remaining.ceil().max(0.0) as i32
}

/// Human phrasing of a population-impact band (real-time loop §3), with a tone:
/// a loss band reads warm-red, a gain accent, a straddle neutral.
fn impact_label(lo: i64, hi: i64) -> (String, Color) {
    if hi <= 0 {
        (
            format!("~ {}–{} souls may be lost", hi.abs(), lo.abs()),
            term::alert(),
        )
    } else if lo >= 0 {
        (format!("~ {lo}–{hi} souls may join"), term::accent())
    } else {
        (format!("~ {lo} to +{hi} souls"), term::dim())
    }
}

/// Dim the world and draw the modal surface with `header` centered in the title
/// band and the auto-resolve `countdown` right-aligned within it (real-time loop
/// §2); returns the content rect.
fn modal_frame(header: &str, countdown: i32, option_count: usize, accent: Color) -> Rect {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.75),
    );

    let height = 248.0 + option_count as f32 * 96.0;
    let rect = Rect::new(
        LOGICAL_WIDTH / 2.0 - 330.0,
        (LOGICAL_HEIGHT - height) / 2.0,
        660.0,
        height,
    );
    let header_h = 40.0;
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.06, 0.05, 0.012, 1.0))
            .with_border(2.0, accent)
            .with_header(header_h, term::panel_header())
            .with_header_divider(1.0, accent),
    );
    draw_text_centered_in_box_ex(
        header,
        rect.x,
        rect.y,
        rect.w,
        header_h,
        TextStyle::new(15.0, accent),
    );
    // The countdown tucks into the right of the header band so it never crowds
    // the centered title.
    draw_text_right(
        &format!("AUTO-RESOLVE {countdown}s"),
        rect.right() - 16.0,
        rect.y + header_h * 0.5 + 4.0,
        TextStyle::new(13.0, accent),
    );
    rect.inset(26.0)
}

/// Word-wrapped body text revealed left-to-right terminal style, with a
/// blinking underscore cursor while it is still typing.
fn draw_typed_block(text: &str, x: f32, y: f32, w: f32, reveal: f32) {
    let shown = typed_prefix(text, reveal, REVEAL_CPS);
    let cursor = if !is_fully_typed(text, reveal, REVEAL_CPS) && blink(reveal, 2.5) {
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
