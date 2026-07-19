//! Contract & Systems: active-contract progress or available charters.
//! The "systems" list stays a plain panel, not a starmap (GDD §7, open q. 1).

use crate::data::{GameData, ResourceDelta};
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

/// A compact ` → +N res` suffix for a milestone's one-time reward (empty when
/// there is none).
fn reward_hint(reward: &ResourceDelta) -> String {
    let mut parts = Vec::new();
    for (amount, unit) in [
        (reward.credits, "cr"),
        (reward.minerals, "min"),
        (reward.energy, "en"),
        (reward.food, "food"),
        (reward.influence, "inf"),
    ] {
        if amount != 0 {
            parts.push(format!("+{amount} {unit}"));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("   ({})", parts.join(" "))
    }
}

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    match &ctx.sim.contract {
        Some(_) => draw_active(ctx, area),
        None => draw_available(ctx, area, mouse, actions),
    }
}

fn draw_active(ctx: &GameplayCtx<'_>, area: Rect) {
    let contract = ctx.sim.contract.as_ref().unwrap();
    let left = Rect::new(area.x, area.y, area.w * 0.6, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);

    term_panel(left, Some("ACTIVE CONTRACT"));
    let content = left.inset(20.0);
    let mut y = content.y + 42.0;

    draw_ui_text_ex(
        &contract.name,
        content.x,
        y,
        TextStyle::new(19.0, term::accent()).params(),
    );
    y += 26.0;
    draw_ui_text_ex(
        &format!(
            "{} · PHASE: {} · YEAR {}/{}",
            contract.objective.label().to_uppercase(),
            contract.phase.label().to_uppercase(),
            contract.years_elapsed,
            contract.target_duration_years
        ),
        content.x,
        y,
        TextStyle::new(14.0, term::dim()).params(),
    );
    y += 24.0;

    meter(
        Rect::new(content.x, y, content.w, 26.0),
        contract.progress(),
        1.0,
        term::primary(),
        Some(&format!("PROGRESS {:.0}%", contract.progress() * 100.0)),
    );
    y += 34.0;
    if contract.bonus_progress > 0.0 {
        draw_ui_text_ex(
            &format!(
                "DRIVE ASSIST: +{:.1} yr from ship speed",
                contract.bonus_progress
            ),
            content.x,
            y,
            TextStyle::new(12.0, term::accent()).params(),
        );
    }
    y += 22.0;

    draw_ui_text_ex(
        "MILESTONES",
        content.x,
        y,
        TextStyle::new(15.0, term::primary()).params(),
    );
    y += 22.0;
    for milestone in &contract.milestones {
        let (mark, color) = if milestone.reached {
            ("[x]", term::accent())
        } else {
            ("[ ]", term::dim())
        };
        let bounty = reward_hint(&milestone.reward);
        draw_ui_text_ex(
            &format!("{mark} {}{bounty}", milestone.name),
            content.x,
            y,
            TextStyle::new(14.0, color).params(),
        );
        y += 22.0;
    }
    y += 14.0;

    draw_ui_text_ex(
        "SUCCESS METRICS",
        content.x,
        y,
        TextStyle::new(15.0, term::primary()).params(),
    );
    y += 22.0;
    for metric in &contract.metrics {
        meter(
            Rect::new(content.x, y, content.w, 20.0),
            (metric.current / metric.target.max(0.001)).min(1.0),
            1.0,
            term::accent(),
            Some(&format!(
                "{} {:.2}/{:.2} (w {:.0}%)",
                metric.name,
                metric.current,
                metric.target,
                metric.weight * 100.0
            )),
        );
        y += 28.0;
    }

    term_panel(right, Some("RELEVANT SYSTEMS"));
    let rcontent = right.inset(20.0);
    // TODO(next agent, M2): populate origin/waypoint/destination entries per
    // contract template (GDD §7) instead of this static journey summary.
    draw_text_block(
        "ORIGIN: Home Berth (departed)\nWAYPOINT: deep transit\nDESTINATION: per charter\n\nSystems relevant to the active charter appear here. Not a starmap by design — see gdd.md §7.",
        rcontent.x,
        rcontent.y + 40.0,
        rcontent.w,
        rcontent.h - 60.0,
        14.0,
        4.0,
        term::dim(),
    );
}

fn draw_available(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(area, Some("AVAILABLE CHARTERS"));
    let content = area.inset(20.0);
    let top = content.y + 42.0;

    // Two-column charter grid so the list scales past six charters without a
    // scrollbar (each column is half-width; four rows fit comfortably).
    const GAP: f32 = 16.0;
    let col_w = (content.w - GAP) / 2.0;

    for (i, id) in GameData::sorted_ids(&ctx.data.contracts)
        .into_iter()
        .enumerate()
    {
        let Some(template) = ctx.data.contracts.get(&id) else {
            continue;
        };
        let col = (i % 2) as f32;
        let row = (i / 2) as f32;
        let card = Rect::new(
            content.x + col * (col_w + GAP),
            top + row * 82.0,
            col_w,
            78.0,
        );
        draw_surface(
            card,
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            &template.name,
            card.x + 14.0,
            card.y + 22.0,
            TextStyle::new(16.0, term::primary()).params(),
        );
        draw_ui_text_ex(
            &format!(
                "{} · {} YEARS · reward {} cr",
                template.objective.label().to_uppercase(),
                template.target_duration_years,
                template.reward.credits
            ),
            card.x + 14.0,
            card.y + 40.0,
            TextStyle::new(12.0, term::dim()).params(),
        );
        draw_text_block(
            &template.description,
            card.x + 14.0,
            card.y + 46.0,
            card.w - 190.0,
            26.0,
            11.0,
            2.0,
            term::dim(),
        );
        if term_button(
            Rect::new(card.right() - 170.0, card.y + 24.0, 156.0, 30.0),
            "ACCEPT CHARTER",
            true,
            mouse,
        ) {
            actions.push(UiAction::AcceptContract(id.clone()));
        }
    }
}
