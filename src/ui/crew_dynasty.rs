//! Crew & Dynasty: roster, succession outlook, delegation toggles.
//!
//! TODO(next agent, M2): recruit/train crew actions, heir designation
//! (UiAction::SelectHeir), and the ~5-6 real dynasty actions replacing the
//! original's cosmetic no-ops (GDD §0).

use crate::data::events::EventCategory;
use crate::ui::{stat_line, term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.55, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);

    draw_roster(ctx, left);
    draw_council(ctx, right, mouse, actions);
}

fn draw_roster(ctx: &GameplayCtx<'_>, rect: Rect) {
    term_panel(rect, Some("DYNASTY ROSTER"));
    let content = rect.inset(18.0);
    let mut y = content.y + 42.0;

    let config = &ctx.data.config;
    let mut members: Vec<_> = ctx.sim.dynasty.members.iter().collect();
    members.sort_by(|a, b| b.is_leader.cmp(&a.is_leader).then(b.age.cmp(&a.age)));

    for member in members.iter().take(12) {
        let heir_eligible = member.age >= config.heir_min_age && member.age <= config.heir_max_age;
        let color = if member.is_leader {
            term::GREEN
        } else if heir_eligible {
            term::AMBER
        } else {
            term::AMBER_DIM
        };
        let role = if member.is_leader {
            " [LEADER]"
        } else if heir_eligible {
            " [heir-eligible]"
        } else {
            ""
        };
        draw_ui_text_ex(
            &format!(
                "{} — {} · {} · LD {}{}",
                member.name, member.age, member.specialization, member.leadership, role
            ),
            content.x,
            y,
            TextStyle::new(14.0, color).params(),
        );
        draw_ui_text_ex(
            &format!("   trait: {}", member.trait_name),
            content.x,
            y + 16.0,
            TextStyle::new(12.0, term::AMBER_FAINT).params(),
        );
        y += 38.0;
    }

    if ctx.sim.dynasty.members.len() > 12 {
        draw_ui_text_ex(
            &format!("... and {} more", ctx.sim.dynasty.members.len() - 12),
            content.x,
            y,
            TextStyle::new(13.0, term::AMBER_FAINT).params(),
        );
        y += 24.0;
    }

    // Crew posts the roster will eventually fill (recruit/train lands in M2).
    y = y.max(content.bottom() - 54.0);
    let posts: Vec<&str> = ctx
        .data
        .crew_archetypes
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    draw_text_block(
        &format!("SHIP POSTS AWAITING ASSIGNMENT: {}", posts.join(", ")),
        content.x,
        y,
        content.w,
        44.0,
        12.0,
        3.0,
        term::AMBER_FAINT,
    );
}

fn draw_council(ctx: &GameplayCtx<'_>, rect: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("COUNCIL & DELEGATION"));
    let content = rect.inset(18.0);
    let mut y = content.y + 42.0;

    stat_line(
        content.x,
        y,
        "GENERATION",
        &ctx.sim.dynasty.generation.to_string(),
        term::GREEN,
    );
    y += 24.0;
    let next_gen = ctx
        .data
        .config
        .generation_interval_years
        .saturating_sub(ctx.sim.dynasty.years_since_generation);
    stat_line(
        content.x,
        y,
        "NEXT GENERATION IN",
        &format!("{next_gen} yr"),
        term::AMBER,
    );
    y += 34.0;

    draw_text_block(
        "Delegated event domains auto-resolve via the council's advisors; outcomes are still logged (GDD §5.4).",
        content.x,
        y,
        content.w,
        44.0,
        13.0,
        3.0,
        term::AMBER_DIM,
    );
    y += 54.0;

    for category in EventCategory::ALL {
        let delegated = ctx.sim.delegation.is_delegated(category);
        let label = format!(
            "{} — {}",
            category.label().to_uppercase(),
            if delegated { "DELEGATED" } else { "COUNCIL" }
        );
        if term_button(
            Rect::new(content.x, y, content.w, 34.0),
            &label,
            true,
            mouse,
        ) {
            actions.push(UiAction::ToggleDelegation(category));
        }
        y += 42.0;
    }

    y += 10.0;
    let legacy = &ctx.sim.legacy;
    stat_line(
        content.x,
        y,
        "TRADITION POINTS",
        &legacy.tradition_points.to_string(),
        term::AMBER,
    );
    y += 22.0;
    stat_line(
        content.x,
        y,
        "BODY-HORROR EVENTS",
        &legacy.body_horror_events.to_string(),
        term::AMBER,
    );
    y += 22.0;
    stat_line(
        content.x,
        y,
        "PIRACY REPUTATION",
        &format!("{:.2}", legacy.piracy_reputation),
        term::AMBER,
    );
}
