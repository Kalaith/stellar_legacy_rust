//! Contract & Systems: active-contract progress or available charters.
//! The "systems" list stays a plain panel, not a starmap (GDD §7, open q. 1).

use crate::data::contracts::{ContractObjective, ContractPhase};
use crate::data::{GameData, ResourceDelta};
use crate::state::sim::ActiveContract;
use crate::ui::{term, term_bar, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, is_fully_visible, measure_ui_text, RectExt};

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

/// The DRYDOCK tab (docked only, real-time loop §5): the PREP screen when a
/// charter is under consideration, else the available-charter board. Never shows
/// under way — the CONTRACT tab replaces it there.
pub fn draw_drydock(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    if ctx.sim.selected_charter.is_some() {
        // A charter under consideration in port → the PREP screen (W4).
        crate::ui::prep::draw(ctx, area, mouse, actions);
    } else {
        // In port, nothing selected → the available-charter list.
        draw_available(ctx, area, mouse, actions);
    }
}

/// The CONTRACT tab (under way only, real-time loop §5): the active-contract
/// progress view. Falls back to the drydock board if somehow drawn in port.
pub fn draw_active_screen(
    ctx: &GameplayCtx<'_>,
    area: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    if ctx.sim.contract.is_some() {
        draw_active(ctx, area, mouse, actions);
    } else {
        draw_drydock(ctx, area, mouse, actions);
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

    term_bar(
        Rect::new(content.x, y, content.w, 26.0),
        contract.progress(),
        term::accent(),
        "PROGRESS",
        &format!("{:.0}%", contract.progress() * 100.0),
    );
    y += 34.0;

    // Authored phase timeline (W2).
    draw_phase_timeline(contract, Rect::new(content.x, y, content.w, 20.0));
    y += 30.0;

    // Quantified objective counter (W2) — pay tracks this fraction, not the clock.
    term_bar(
        Rect::new(content.x, y, content.w, 22.0),
        contract.objective_fraction(),
        term::accent(),
        "OBJECTIVE",
        &format!(
            "{:.0} / {:.0} {}",
            contract.objective_progress, contract.objective_target, contract.objective_unit
        ),
    );
    // Clear the objective bar before the milestone list: a section header sits on
    // its baseline, so a tight gap let its ascenders overlap the bar's box.
    y += 42.0;

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
        term_bar(
            Rect::new(content.x, y, content.w, 20.0),
            (metric.current / metric.target.max(0.001)).min(1.0),
            term::accent(),
            &metric.name.to_uppercase(),
            &format!(
                "{:.2}/{:.2} (w {:.0}%)",
                metric.current,
                metric.target,
                metric.weight * 100.0
            ),
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
            "[ TURN BACK ]  ·  pay prorated to the objective banked (0 if none)",
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
    // On a brand-new campaign the line becomes the tutorial's pointer toward
    // the PREP screen; both variants are authored in game_config.
    let tutorial = &ctx.data.config.tutorial;
    let tutorial_active = !sim.tutorial_dismissed && ctx.chronicle.entries.is_empty();
    let (hint, hint_color) = if tutorial_active {
        (tutorial.drydock_hint.as_str(), term::accent())
    } else {
        (tutorial.drydock_refit_hint.as_str(), term::faint())
    };
    draw_ui_text_ex(
        hint,
        content.x,
        y,
        TextStyle::new(12.0, hint_color).params(),
    );
    let cards = Rect::new(
        content.x,
        y + 26.0,
        content.w,
        content.bottom() - (y + 26.0),
    );
    draw_charter_cards(ctx, cards, mouse, actions);
}

/// Ellipsis-truncate `text` so it renders within `max_w` at the UI font size.
fn fit_text(text: &str, size: u16, max_w: f32) -> String {
    if measure_ui_text(text, None, size, 1.0).width <= max_w {
        return text.to_owned();
    }
    let mut cut: String = text.to_owned();
    while cut.pop().is_some() {
        let candidate = format!("{}...", cut.trim_end());
        if measure_ui_text(&candidate, None, size, 1.0).width <= max_w {
            return candidate;
        }
    }
    "...".to_owned()
}

/// One charter's board entry: its id plus the gate verdict, precomputed once so
/// the list can be grouped and sorted (available-first) before layout.
struct CharterEntry {
    id: String,
    name: String,
    locked: bool,
    lock_label: String,
    min_renown: i64,
}

/// The objective sections and the order they stack on the board — economic work
/// first, expansion next, the human charters last. Every `ContractObjective`
/// must appear exactly once so no charter falls out of the grouping.
const OBJECTIVE_ORDER: [ContractObjective; 6] = [
    ContractObjective::Mining,
    ContractObjective::Salvage,
    ContractObjective::Exploration,
    ContractObjective::Colonization,
    ContractObjective::Rescue,
    ContractObjective::Diplomacy,
];

/// Resolve a charter's lock state and the label naming whatever bars it (renown,
/// loadout, or a required people aboard) — extracted so both the sort key and the
/// card draw read the same verdict.
fn charter_lock(
    ctx: &GameplayCtx<'_>,
    template: &crate::data::contracts::ContractTemplate,
) -> (bool, String) {
    let renown = crate::heritage::renown(ctx.chronicle);
    // A charter locks on either the cross-campaign renown gate or the in-world
    // gate (content-depth charters round 12: the peoples the writ needs aboard).
    // The label names whichever bars it, so the board reads honestly.
    let renown_locked = template.min_renown > renown;
    let in_world_ok = crate::simulation::contract::meets_in_world_gate(ctx.sim, template);
    // …and the drydock loadout gate (content-depth charters round 26): a writ that
    // demands guns/hold/engine the ship doesn't carry stays locked, and names the lack.
    let loadout_ok = crate::simulation::contract::meets_loadout_gate(ctx.sim, ctx.data, template);
    let locked = renown_locked || !in_world_ok || !loadout_ok;
    let lock_label = if renown_locked {
        format!("LOCKED · RENOWN {}", template.min_renown)
    } else if !loadout_ok {
        let mut needs: Vec<String> = Vec::new();
        if template.min_combat > 0 {
            needs.push(format!("CBT {}", template.min_combat));
        }
        if template.min_cargo > 0 {
            needs.push(format!("CARGO {}", template.min_cargo));
        }
        if template.min_speed > 0 {
            needs.push(format!("SPD {}", template.min_speed));
        }
        format!("LOCKED · NEEDS {}", needs.join(" · "))
    } else {
        let needed: Vec<&str> = template
            .requires_faction_aboard
            .iter()
            .map(|fid| {
                ctx.data
                    .factions
                    .get(fid)
                    .map(|f| f.name.as_str())
                    .unwrap_or(fid.as_str())
            })
            .collect();
        format!("LOCKED · NEEDS {}", needed.join(", ").to_uppercase())
    };
    (locked, lock_label)
}

/// Bucket the charters by objective in [`OBJECTIVE_ORDER`], each bucket sorted
/// available-first (then by renown gate, then name) so the missions a captain can
/// actually take sit at the top of their section. Empty sections are dropped.
fn grouped_charters(ctx: &GameplayCtx<'_>) -> Vec<(ContractObjective, Vec<CharterEntry>)> {
    let mut buckets: Vec<(ContractObjective, Vec<CharterEntry>)> =
        OBJECTIVE_ORDER.iter().map(|o| (*o, Vec::new())).collect();
    for id in GameData::sorted_ids(&ctx.data.contracts) {
        let Some(template) = ctx.data.contracts.get(&id) else {
            continue;
        };
        let (locked, lock_label) = charter_lock(ctx, template);
        let entry = CharterEntry {
            id,
            name: template.name.clone(),
            locked,
            lock_label,
            min_renown: template.min_renown,
        };
        if let Some(bucket) = buckets.iter_mut().find(|(o, _)| *o == template.objective) {
            bucket.1.push(entry);
        }
    }
    for (_, entries) in buckets.iter_mut() {
        entries.sort_by(|a, b| {
            a.locked
                .cmp(&b.locked)
                .then(a.min_renown.cmp(&b.min_renown))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    buckets.retain(|(_, entries)| !entries.is_empty());
    buckets
}

/// A section rule + label: the objective name, its charter count, and a faint
/// divider spanning the list so each type reads as its own block.
fn draw_group_header(objective: ContractObjective, count: usize, rect: Rect) {
    draw_ui_text_ex(
        &objective.label().to_uppercase(),
        rect.x,
        rect.y + 16.0,
        TextStyle::new(14.0, term::accent()).params(),
    );
    let count_text = format!("{count}");
    let cw = measure_ui_text(&count_text, None, 12, 1.0).width;
    draw_ui_text_ex(
        &count_text,
        rect.right() - cw,
        rect.y + 16.0,
        TextStyle::new(12.0, term::dim()).params(),
    );
    // Divider on the section baseline.
    draw_line(
        rect.x,
        rect.y + 24.0,
        rect.right(),
        rect.y + 24.0,
        1.0,
        term::faint(),
    );
}

/// Draw a single charter card at `card`. Wide cards carry a side SELECT button
/// and a description; a narrow (PREP swap) column gets the compact
/// whole-card-clickable layout, where the side button and prose would overlap
/// the title.
fn draw_charter_card(
    ctx: &GameplayCtx<'_>,
    entry: &CharterEntry,
    card: Rect,
    compact: bool,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    let Some(template) = ctx.data.contracts.get(&entry.id) else {
        return;
    };
    let locked = entry.locked;
    let selected = ctx.sim.selected_charter.as_deref() == Some(entry.id.as_str());
    let hovered = compact && !locked && card.contains_point(mouse);
    let fill = if selected {
        term::surface_active()
    } else if hovered {
        term::surface_hover()
    } else {
        term::surface_inset()
    };
    draw_surface(
        card,
        &SurfaceStyle::new(fill).with_border(
            1.0,
            if selected {
                term::primary()
            } else {
                term::faint()
            },
        ),
    );
    let title_color = if locked {
        term::faint()
    } else if selected {
        term::accent()
    } else {
        term::primary()
    };
    let meta = format!(
        "{} · {} YEARS · reward {} cr",
        template.objective.label().to_uppercase(),
        template.target_duration_years,
        template.reward.credits
    );

    if compact {
        // Compact card: title / meta / status stacked, the whole card is
        // the SELECT button.
        draw_ui_text_ex(
            &fit_text(&template.name, 13, card.w - 24.0),
            card.x + 12.0,
            card.y + 20.0,
            TextStyle::new(13.0, title_color).params(),
        );
        draw_ui_text_ex(
            &fit_text(&meta, 11, card.w - 24.0),
            card.x + 12.0,
            card.y + 40.0,
            TextStyle::new(11.0, term::dim()).params(),
        );
        let (status, status_color) = if locked {
            (entry.lock_label.clone(), term::faint())
        } else if selected {
            ("[ SELECTED ]".to_owned(), term::accent())
        } else {
            ("[ SELECT ]".to_owned(), term::dim())
        };
        draw_ui_text_ex(
            &status,
            card.x + 12.0,
            card.y + 62.0,
            TextStyle::new(11.0, status_color).params(),
        );
        if hovered && is_mouse_button_released(MouseButton::Left) {
            actions.push(UiAction::SelectCharter(entry.id.clone()));
        }
        return;
    }

    draw_ui_text_ex(
        &template.name,
        card.x + 14.0,
        card.y + 22.0,
        TextStyle::new(16.0, title_color).params(),
    );
    draw_ui_text_ex(
        &meta,
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
        term_button(btn, &entry.lock_label, false, mouse);
    } else {
        let label = if selected { "SELECTED" } else { "SELECT" };
        if term_button(btn, label, true, mouse) {
            actions.push(UiAction::SelectCharter(entry.id.clone()));
        }
    }
}

/// The two-column charter board (W4-shared), grouped into labelled objective
/// sections (available charters first within each) so a captain can find the kind
/// of work they want at a glance. The list smooth-scrolls when it outgrows the
/// panel — partially-clipped cards are culled so nothing spills past the frame,
/// and a scrollbar rides the right gutter. A narrow area (the PREP swap column)
/// gets the compact whole-card-clickable layout.
pub(crate) fn draw_charter_cards(
    ctx: &GameplayCtx<'_>,
    area: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    const GAP: f32 = 16.0;
    const CARD_H: f32 = 78.0;
    const ROW_STRIDE: f32 = 82.0;
    const HEADER_H: f32 = 32.0;
    const GROUP_GAP: f32 = 8.0;
    // Right-edge gutter reserved for the scrollbar so cards never sit beneath it.
    const GUTTER: f32 = 14.0;

    let compact = area.w < 900.0;
    let usable_w = area.w - GUTTER;
    let col_w = (usable_w - GAP) / 2.0;

    let groups = grouped_charters(ctx);
    if groups.is_empty() {
        return;
    }

    // Laid-out height (headers + card rows + inter-group gaps); the per-group
    // arithmetic here mirrors the draw loop below exactly.
    let content_h: f32 = groups
        .iter()
        .map(|(_, entries)| {
            let rows = entries.len().div_ceil(2) as f32;
            HEADER_H + rows * ROW_STRIDE + GROUP_GAP
        })
        .sum();

    let mut scroll = ctx.charter_scroll.get();
    scroll.update_at(area, content_h, mouse);
    let mut y = area.y - scroll.offset();

    for (objective, entries) in &groups {
        let header = Rect::new(area.x, y, usable_w, HEADER_H);
        if is_fully_visible(header, area) {
            draw_group_header(*objective, entries.len(), header);
        }
        y += HEADER_H;
        for (i, entry) in entries.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let card = Rect::new(
                area.x + col * (col_w + GAP),
                y + row * ROW_STRIDE,
                col_w,
                CARD_H,
            );
            // Cull partially-scrolled cards so panel edges stay clean (macroquad
            // has no scissor rect).
            if is_fully_visible(card, area) {
                draw_charter_card(ctx, entry, card, compact, mouse, actions);
            }
        }
        y += entries.len().div_ceil(2) as f32 * ROW_STRIDE + GROUP_GAP;
    }

    scroll.draw_scrollbar_with(
        area,
        content_h,
        term::surface_inset(),
        term::dim(),
        term::primary(),
    );
    ctx.charter_scroll.set(scroll);
}
