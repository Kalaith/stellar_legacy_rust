//! Crew & Dynasty: dynasty roster with heir designation, ship posts with
//! recruit/train, succession outlook, delegation toggles, failure risk.

use crate::data::events::EventCategory;
use crate::simulation::crew::post_holder;
use crate::simulation::legacy::failure_risk;
use crate::ui::{stat_line, term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, is_fully_visible, RectExt};

/// Height of one SHIP POSTS row, and so of its TRAIN/RECRUIT target.
///
/// The rows used to sit at 24 with a 24-tall button, which is zero gap: the
/// touch expansion had nowhere to grow into and the target stayed 24 logical
/// pixels — around 22 CSS pixels on a tablet, against a standard of 44.
///
/// 30 is what this column can afford. The left side carries the roster, seven
/// post rows and the peoples list in 576 pixels, and reaching a full 44 here
/// would cost the roster every row it shows. That is the honest ceiling for
/// this screen without redrawing it, and it is recorded here so the next
/// person does not rediscover it by trying.
const POST_STRIDE: f32 = 30.0;
/// The button drawn inside a [`POST_STRIDE`] row. The 4px it leaves is what the
/// hit area grows into, 2px each way, reaching the full stride.
const POST_BUTTON_H: f32 = 26.0;

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    let left = Rect::new(area.x, area.y, area.w * 0.55, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);
    // Posts is sized to exactly one row per archetype — as a fixed ratio its
    // last TRAIN/RECRUIT button bled into the PEOPLES panel below.
    let posts_h = 78.0 + (ctx.data.crew_archetypes.len().saturating_sub(1)) as f32 * POST_STRIDE;
    let roster = Rect::new(left.x, left.y, left.w, left.h * 0.325);
    let posts = Rect::new(left.x, roster.bottom() + 8.0, left.w, posts_h);
    let factions = Rect::new(
        left.x,
        posts.bottom() + 8.0,
        left.w,
        left.h - roster.h - posts.h - 16.0,
    );

    draw_roster(ctx, roster, pointer, actions);
    draw_posts(ctx, posts, pointer, actions);
    draw_factions(ctx, factions, pointer, actions);
    draw_council(ctx, right, pointer, actions);
}

/// Factions aboard (W7): name, members, share, status. Lost factions dim out.
/// In drydock, when short of the founding count, offers to recruit a new people.
fn draw_factions(ctx: &GameplayCtx<'_>, rect: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("PEOPLES ABOARD"));
    let content = rect.inset(16.0);
    let mut y = content.y + 34.0;
    let sim = ctx.sim;

    let total_aboard: u32 = sim
        .factions
        .iter()
        .filter(|f| f.is_aboard())
        .map(|f| f.members)
        .sum();

    let faction_name = |id: &str| {
        ctx.data
            .factions
            .get(id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| id.to_owned())
    };

    // The peoples still aboard come first — they are the living ship.
    for fs in sim.factions.iter().filter(|f| f.is_aboard()) {
        let share = if total_aboard > 0 {
            fs.members as f32 / total_aboard as f32 * 100.0
        } else {
            0.0
        };
        draw_ui_text_ex(
            &format!(
                "{} — {} ({share:.0}%)",
                faction_name(&fs.faction_id),
                fs.members
            ),
            content.x,
            y,
            TextStyle::new(13.0, term::primary()).params(),
        );
        y += 22.0;
    }

    // Recruit a fresh people in drydock when the ship is short (W7) — the verb
    // outranks the memorial rows below when space runs out.
    let cfg = &ctx.data.config.factions;
    if sim.contract.is_none() && sim.aboard_faction_count() < cfg.starting_count {
        for id in sim.recruitable_faction_ids(ctx.data) {
            if y > content.bottom() - 26.0 {
                break;
            }
            if term_button(
                Rect::new(content.x, y + 2.0, content.w, 24.0),
                &format!(
                    "RECRUIT {} ({} CR)",
                    faction_name(&id),
                    cfg.recruit_group_cost_credits
                ),
                sim.resources.credits >= cfg.recruit_group_cost_credits,
                pointer,
            ) {
                actions.push(UiAction::RecruitFactionGroup(id.clone()));
            }
            y += 28.0;
        }
    }

    // Lost peoples dim out below, clamped to the panel — they must never spill
    // into whatever sits underneath.
    for fs in sim.factions.iter().filter(|f| !f.is_aboard()) {
        if y > content.bottom() - 4.0 {
            break;
        }
        draw_ui_text_ex(
            &format!("{} — {}", faction_name(&fs.faction_id), fs.status.label()),
            content.x,
            y,
            TextStyle::new(12.0, term::faint()).params(),
        );
        y += 20.0;
    }
}

/// Vertical stride of one roster row.
const ROSTER_STRIDE: f32 = 38.0;
/// How much of that stride the row actually occupies: the NAME HEIR button, the
/// name line and the trait line beneath it. Used to cull a row that would hang
/// over the panel edge, so it has to cover the descenders of the lower line —
/// the 2px it leaves the stride is only there to keep neighbouring rows from
/// touching.
const ROSTER_ROW_H: f32 = 36.0;
/// Reserved at the panel's right edge for the scrollbar, so no row sits under it.
const ROSTER_GUTTER: f32 = 12.0;

fn draw_roster(ctx: &GameplayCtx<'_>, rect: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("DYNASTY ROSTER"));
    let content = rect.inset(18.0);

    let config = &ctx.data.config;
    let mut members: Vec<_> = ctx.sim.dynasty.members.iter().collect();
    members.sort_by(|a, b| b.is_leader.cmp(&a.is_leader).then(b.age.cmp(&a.age)));

    // The roster used to draw whatever fitted and admit to the rest with
    // "... and 3 more" — which named the people it was hiding without letting
    // anyone read them, and hid the very rows a heir is chosen from. It scrolls
    // now, so the panel's height stops deciding who is in the dynasty.
    let view = Rect::new(
        content.x,
        content.y + 26.0,
        content.w,
        content.bottom() - (content.y + 26.0),
    );
    let row_w = view.w - ROSTER_GUTTER;
    let content_h = members.len() as f32 * ROSTER_STRIDE;
    let mut scroll = ctx.roster_scroll.get();
    scroll.update_at(view, content_h, pointer.position);
    // A drag down the roster must not also name an heir on the way past.
    let pointer = if scroll.absorbs_press() {
        pointer.suppressed()
    } else {
        pointer
    };

    let mut row_top = view.y - scroll.offset();
    for member in members.iter() {
        let row = Rect::new(view.x, row_top, row_w, ROSTER_ROW_H);
        row_top += ROSTER_STRIDE;
        // macroquad has no scissor rect, so cull the partly-scrolled rows
        // rather than letting them spill past the panel.
        if !is_fully_visible(row, view) {
            continue;
        }
        let y = row.y + 16.0;
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
                Rect::new(row.right() - 90.0, row.y, 90.0, 30.0),
                "NAME HEIR",
                true,
                pointer,
            )
        {
            actions.push(UiAction::SelectHeir(member.id));
        }
    }

    scroll.draw_scrollbar_with(
        view,
        content_h,
        term::surface_inset(),
        term::dim(),
        term::primary(),
    );
    ctx.roster_scroll.set(scroll);
}

fn draw_posts(ctx: &GameplayCtx<'_>, rect: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    term_panel(rect, Some("SHIP POSTS"));
    let content = rect.inset(18.0);
    let mut y = content.y + 30.0;

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
                    Rect::new(content.right() - 150.0, y - 14.0, 144.0, POST_BUTTON_H),
                    &if maxed {
                        "MASTERED".to_owned()
                    } else {
                        format!("TRAIN ({} CR)", crew_cfg.train_cost_credits)
                    },
                    !maxed,
                    pointer,
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
                    Rect::new(content.right() - 150.0, y - 14.0, 144.0, POST_BUTTON_H),
                    &format!("RECRUIT ({} CR)", crew_cfg.recruit_cost_credits),
                    ctx.sim.resources.credits >= crew_cfg.recruit_cost_credits,
                    pointer,
                ) {
                    actions.push(UiAction::RecruitCrew(archetype.id.clone()));
                }
            }
        }
        y += POST_STRIDE;
    }
}

fn draw_council(ctx: &GameplayCtx<'_>, rect: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
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
            pointer,
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
