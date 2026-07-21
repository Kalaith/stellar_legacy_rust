//! Subsystems screen (W5): the six ship modules — tier, condition, and the
//! institutional knowledge that gates repair — with the Repair / Upgrade /
//! Train verbs. Pure view: it reads `&SimState` and emits `UiAction` only.

use crate::data::GameData;
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    const GAP: f32 = 12.0;
    let col_w = (area.w - GAP) / 2.0;
    let row_h = (area.h - 2.0 * GAP) / 3.0;
    for (i, id) in GameData::sorted_ids(&ctx.data.subsystems)
        .into_iter()
        .enumerate()
    {
        let col = (i % 2) as f32;
        let row = (i / 2) as f32;
        let rect = Rect::new(
            area.x + col * (col_w + GAP),
            area.y + row * (row_h + GAP),
            col_w,
            row_h,
        );
        draw_card(ctx, rect, &id, mouse, actions);
    }
}

fn draw_card(
    ctx: &GameplayCtx<'_>,
    rect: Rect,
    id: &str,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    let (Some(def), Some(state)) = (ctx.data.subsystems.get(id), ctx.sim.subsystems.get(id)) else {
        return;
    };
    let cfg = &ctx.data.config;
    let in_port = ctx.sim.contract.is_none();

    term_panel(rect, Some(&def.name.to_uppercase()));
    let content = rect.inset(14.0);
    let mut y = content.y + 32.0;

    // Tier pips + the event family this module buffers.
    let pips: String = (1..=3)
        .map(|t| if state.tier >= t { '●' } else { '○' })
        .collect();
    let family = if def.buffers_family.is_empty() {
        "habitat integrity".to_owned()
    } else {
        def.buffers_family.replace('_', " ")
    };
    draw_ui_text_ex(
        &format!("TIER {pips}   ·   buffers {family}"),
        content.x,
        y,
        TextStyle::new(12.0, term::dim()).params(),
    );
    y += 22.0;

    meter(
        Rect::new(content.x, y, content.w, 18.0),
        state.condition,
        1.0,
        term::primary(),
        Some(&format!("CONDITION {:.0}%", state.condition * 100.0)),
    );
    y += 24.0;

    // Knowledge — red when it has fallen below the repair threshold.
    let can_mend = state.knowledge >= def.repair_knowledge_required;
    meter(
        Rect::new(content.x, y, content.w, 18.0),
        state.knowledge,
        1.0,
        if can_mend {
            term::accent()
        } else {
            term::alert()
        },
        Some(&format!(
            "KNOWLEDGE {:.0}%  (mend needs {:.0}%)",
            state.knowledge * 100.0,
            def.repair_knowledge_required * 100.0
        )),
    );

    // --- Verbs: Repair / Upgrade (port) / Train ---
    let bw = (content.w - 2.0 * 8.0) / 3.0;
    let by = content.bottom() - 26.0;

    let ceiling = if in_port {
        1.0
    } else {
        cfg.repair.field_ceiling
    };
    let repair_ok = can_mend
        && state.condition < ceiling
        && ctx.sim.ship.spare_parts >= def.repair_parts_cost
        && ctx.sim.resources.minerals >= def.repair_minerals_cost;
    if term_button(
        Rect::new(content.x, by, bw, 22.0),
        &format!(
            "REPAIR ({}p·{}min)",
            def.repair_parts_cost, def.repair_minerals_cost
        ),
        repair_ok,
        mouse,
    ) {
        actions.push(UiAction::RepairSubsystem(id.to_owned()));
    }

    // Upgrade: port-only, pays the next tier's cost, caps at tier 3.
    let next = def.tiers.get(state.tier as usize);
    let upgrade_label = match next {
        Some(t) if in_port => format!("UPGRADE ({}cr)", t.cost.credits),
        Some(_) => "UPGRADE · PORT".to_owned(),
        None => "MAX TIER".to_owned(),
    };
    let upgrade_ok = in_port
        && next.is_some_and(|t| {
            ctx.sim.resources.credits >= t.cost.credits
                && ctx.sim.resources.minerals >= t.cost.minerals
        });
    if term_button(
        Rect::new(content.x + bw + 8.0, by, bw, 22.0),
        &upgrade_label,
        upgrade_ok,
        mouse,
    ) {
        actions.push(UiAction::UpgradeSubsystem(id.to_owned()));
    }

    // Train: anytime, raises this subsystem's knowledge.
    let train_ok = ctx.sim.resources.credits >= cfg.subsystems.train_cost_credits;
    if term_button(
        Rect::new(content.x + 2.0 * (bw + 8.0), by, bw, 22.0),
        &format!("TRAIN ({}cr)", cfg.subsystems.train_cost_credits),
        train_ok,
        mouse,
    ) {
        actions.push(UiAction::TrainSubsystemKnowledge(id.to_owned()));
    }
}
