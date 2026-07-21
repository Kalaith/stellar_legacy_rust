//! Contract & Systems: active-contract progress or available charters.
//! The "systems" list stays a plain panel, not a starmap (GDD §7, open q. 1).

use crate::data::contracts::ContractPhase;
use crate::data::{GameData, ResourceDelta};
use crate::state::sim::ActiveContract;
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
        Some(_) => draw_active(ctx, area, mouse, actions),
        None => draw_available(ctx, area, mouse, actions),
    }
}

/// Draw the authored phase timeline (W2): one bar per Travel/Operation/Return
/// segment, widths proportional to their years, the current segment lit.
fn draw_phase_timeline(contract: &ActiveContract, rect: Rect) {
    let total = contract.target_duration_years.max(1) as f32;
    let mut x = rect.x;
    for (i, segment) in contract.phases.iter().enumerate() {
        let w = rect.w * (segment.years as f32 / total);
        let seg_rect = Rect::new(x, rect.y, (w - 3.0).max(1.0), rect.h);
        let current = i == contract.phase_index
            && !matches!(
                contract.phase,
                ContractPhase::Preparation | ContractPhase::Completion
            );
        let fill = if current {
            term::accent()
        } else {
            term::surface_inset()
        };
        draw_surface(
            seg_rect,
            &SurfaceStyle::new(fill).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            &format!("{} {}y", segment.kind.label().to_uppercase(), segment.years),
            seg_rect.x + 5.0,
            seg_rect.y + seg_rect.h * 0.5 + 4.0,
            TextStyle::new(10.0, if current { term::bg() } else { term::dim() }).params(),
        );
        x += w;
    }
}

fn draw_active(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
            contract.months_elapsed / 12,
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

    // Authored phase timeline (W2).
    draw_phase_timeline(contract, Rect::new(content.x, y, content.w, 20.0));
    y += 30.0;

    // Quantified objective counter (W2) — pay tracks this fraction, not the clock.
    meter(
        Rect::new(content.x, y, content.w, 22.0),
        contract.objective_fraction(),
        1.0,
        term::accent(),
        Some(&format!(
            "OBJECTIVE {:.0} / {:.0} {}",
            contract.objective_progress, contract.objective_target, contract.objective_unit
        )),
    );
    y += 30.0;

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

    // [ TURN BACK ] (W2): available only underway (Travel/Operation), anchored
    // to the panel bottom so it never collides with the growing metric list.
    let underway = matches!(
        contract.phase,
        ContractPhase::Travel | ContractPhase::Operation
    );
    let abort = Rect::new(content.x, content.bottom() - 40.0, content.w, 32.0);
    if underway {
        if term_button(
            abort,
            "[ TURN BACK ]  ·  pay prorated to progress",
            true,
            mouse,
        ) {
            actions.push(UiAction::AbortMission);
        }
    } else {
        term_button(abort, "— HOMEBOUND —", false, mouse);
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
    // Between missions the ship is in port — frame the arrival-and-refit beat
    // (PLAN M4.6) above the charter list.
    term_panel(area, Some("IN DRYDOCK // AVAILABLE CHARTERS"));
    let content = area.inset(20.0);
    let sim = ctx.sim;
    // Charter tiering (PLAN M4.8): richer charters unlock as Chronicle renown
    // accrues, so a storied dynasty earns the century-long prestige missions.
    // Shown in the condition line so the LOCKED · RENOWN N gates are legible.
    let renown = crate::heritage::renown(ctx.chronicle);
    let mut y = content.y + 40.0;

    // Homecoming: the mission just concluded (latest Chronicle entry).
    let homecoming = match ctx.chronicle.entries.last() {
        Some(last) => {
            let mut s = format!(
                "HOMECOMING · {} — {} (score {:.2}), Y{} after {} yr",
                last.contract_name,
                last.outcome.to_uppercase(),
                last.score,
                last.completed_year,
                last.duration_years
            );
            // Real time the run took (PLAN M4.7), when it was flown this session.
            if let Some(secs) = ctx.run_clock {
                s.push_str(&format!(" · played {}m", (secs / 60.0).round() as u32));
            }
            s
        }
        None => "IN DRYDOCK · the ship rides at anchor, fresh and untried.".to_owned(),
    };
    draw_ui_text_ex(
        &homecoming,
        content.x,
        y,
        TextStyle::new(14.0, term::accent()).params(),
    );
    y += 22.0;
    // Current condition — a reminder to refit before casting off again.
    draw_ui_text_ex(
        &format!(
            "CONDITION · hull {:.0}% · life {:.0}% · parts {} · crew {} · RENOWN {}",
            sim.ship.hull_integrity * 100.0,
            sim.ship.life_support * 100.0,
            sim.ship.spare_parts,
            sim.crew.len(),
            renown
        ),
        content.x,
        y,
        TextStyle::new(13.0, term::dim()).params(),
    );
    y += 20.0;
    draw_ui_text_ex(
        "Repair or refit on the SHIP screen, then accept the next charter:",
        content.x,
        y,
        TextStyle::new(12.0, term::faint()).params(),
    );
    let top = y + 26.0;

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
        let locked = template.min_renown > renown;
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
            TextStyle::new(
                16.0,
                if locked {
                    term::faint()
                } else {
                    term::primary()
                },
            )
            .params(),
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
        let btn = Rect::new(card.right() - 170.0, card.y + 24.0, 156.0, 30.0);
        if locked {
            // Reads like a terminal access gate — the escalation path in view.
            term_button(
                btn,
                &format!("LOCKED · RENOWN {}", template.min_renown),
                false,
                mouse,
            );
        } else if term_button(btn, "ACCEPT CHARTER", true, mouse) {
            actions.push(UiAction::AcceptContract(id.clone()));
        }
    }
}
