//! Crew & Dynasty: dynasty roster with heir designation, ship posts with
//! recruit/train, succession outlook, delegation toggles, failure risk.

use crate::data::events::EventCategory;
use crate::simulation::crew::post_holder;
use crate::simulation::legacy::failure_risk;
use crate::ui::{stat_line, term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.55, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);
    let roster = Rect::new(left.x, left.y, left.w, left.h * 0.52);
    let posts = Rect::new(
        left.x,
        roster.bottom() + 10.0,
        left.w,
        left.h - roster.h - 10.0,
    );

    draw_roster(ctx, roster, mouse, actions);
    draw_posts(ctx, posts, mouse, actions);
    draw_council(ctx, right, mouse, actions);
}

fn draw_roster(ctx: &GameplayCtx<'_>, rect: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("DYNASTY ROSTER"));
    let content = rect.inset(18.0);
    let mut y = content.y + 42.0;

    let config = &ctx.data.config;
    let mut members: Vec<_> = ctx.sim.dynasty.members.iter().collect();
    members.sort_by(|a, b| b.is_leader.cmp(&a.is_leader).then(b.age.cmp(&a.age)));

    let visible = ((content.h - 42.0) / 38.0).floor() as usize;
    for member in members.iter().take(visible) {
        let heir_eligible = member.age >= config.heir_min_age && member.age <= config.heir_max_age;
        let designated = ctx.sim.dynasty.designated_heir == Some(member.id);
        let color = if member.is_leader {
            term::accent()
        } else if heir_eligible {
            term::primary()
        } else {
            term::dim()
        };
        let role = if member.is_leader {
            " [LEADER]"
        } else if designated {
            " [HEIR DESIGNATE]"
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
            TextStyle::new(12.0, term::faint()).params(),
        );
        if heir_eligible
            && !member.is_leader
            && !designated
            && term_button(
                Rect::new(content.right() - 96.0, y - 14.0, 90.0, 26.0),
                "NAME HEIR",
                true,
                mouse,
            )
        {
            actions.push(UiAction::SelectHeir(member.id));
        }
        y += 38.0;
    }

    if ctx.sim.dynasty.members.len() > visible {
        draw_ui_text_ex(
            &format!("... and {} more", ctx.sim.dynasty.members.len() - visible),
            content.x,
            y,
            TextStyle::new(13.0, term::faint()).params(),
        );
    }
}

fn draw_posts(ctx: &GameplayCtx<'_>, rect: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("SHIP POSTS"));
    let content = rect.inset(18.0);
    let mut y = content.y + 40.0;

    let crew_cfg = &ctx.data.config.crew;
    for archetype in &ctx.data.crew_archetypes {
        match post_holder(ctx.sim, &archetype.id) {
            Some(holder) => {
                draw_ui_text_ex(
                    &format!(
                        "{} — {} ({}) · SK {}",
                        archetype.name, holder.name, holder.age, holder.skill
                    ),
                    content.x,
                    y,
                    TextStyle::new(13.0, term::accent()).params(),
                );
                let maxed = holder.skill >= archetype.skill_max;
                if term_button(
                    Rect::new(content.right() - 150.0, y - 14.0, 144.0, 24.0),
                    &if maxed {
                        "MASTERED".to_owned()
                    } else {
                        format!("TRAIN ({} CR)", crew_cfg.train_cost_credits)
                    },
                    !maxed,
                    mouse,
                ) {
                    actions.push(UiAction::TrainCrew(archetype.id.clone()));
                }
            }
            None => {
                draw_ui_text_ex(
                    &format!("{} — POST VACANT", archetype.name),
                    content.x,
                    y,
                    TextStyle::new(13.0, term::dim()).params(),
                );
                if term_button(
                    Rect::new(content.right() - 150.0, y - 14.0, 144.0, 24.0),
                    &format!("RECRUIT ({} CR)", crew_cfg.recruit_cost_credits),
                    ctx.sim.resources.credits >= crew_cfg.recruit_cost_credits,
                    mouse,
                ) {
                    actions.push(UiAction::RecruitCrew(archetype.id.clone()));
                }
            }
        }
        y += 27.0;
    }
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
        term::accent(),
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
        term::primary(),
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
        term::dim(),
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
    let counters: [(&str, String); 4] = [
        ("TRADITION POINTS", legacy.tradition_points.to_string()),
        ("BODY-HORROR EVENTS", legacy.body_horror_events.to_string()),
        (
            "EXISTENTIAL DREAD",
            format!("{:.2}", legacy.existential_dread),
        ),
        (
            "PIRACY REPUTATION",
            format!("{:.2}", legacy.piracy_reputation),
        ),
    ];
    for (label, value) in &counters {
        stat_line(content.x, y, label, value, term::primary());
        y += 22.0;
    }

    // Failure risk (GDD §5.5), with its contributing factors spelled out.
    y += 12.0;
    let risk = failure_risk(ctx.sim, &ctx.data.config);
    let risk_name = ctx
        .data
        .legacies
        .get(&legacy.legacy_id)
        .map(|l| l.failure_risk.replace('_', " ").to_uppercase())
        .unwrap_or_default();
    let (status, color) = if risk.at_risk {
        ("AT RISK", term::alert())
    } else {
        ("STABLE", term::accent())
    };
    stat_line(
        content.x,
        y,
        &format!("RISK: {risk_name}"),
        &format!("{} ({status})", risk.total),
        color,
    );
    y += 22.0;
    for factor in &risk.factors {
        draw_ui_text_ex(
            &format!("  + {} ({})", factor.label, factor.points),
            content.x,
            y,
            TextStyle::new(13.0, term::alert()).params(),
        );
        y += 18.0;
    }
}
