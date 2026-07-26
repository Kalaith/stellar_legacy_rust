//! Ship Builder: component catalog and current loadout (GDD §9).

use crate::data::ship_components::{ComponentKind, ComponentStats, ShipComponent};
use crate::simulation::ship::{install_eligibility, InstallEligibility};
use crate::ui::{ship_schematic, stat_line, term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, is_fully_visible, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    // Under way the SHIP tab is a status readout, not a shipyard (real-time loop
    // §5): installed modules, current integrity, and the boosts/debuffs in force.
    // Buying, commissioning, and refits wait for the drydock.
    if ctx.sim.contract.is_some() {
        draw_underway(ctx, area, pointer, actions);
        return;
    }

    // Sub-tab toggle: the hull/engine/weapon LOADOUT catalog, or the subsystem
    // MODULES ladders. Both are drydock work, so both live behind this one screen.
    let modules = ctx.ship_modules_tab.get();
    const SEG_W: f32 = 160.0;
    const SEG_H: f32 = 30.0;
    for (i, label) in ["LOADOUT", "MODULES"].iter().enumerate() {
        let seg = Rect::new(area.x + i as f32 * (SEG_W + 8.0), area.y, SEG_W, SEG_H);
        let hit = touch_area(seg);
        note_neighbour(seg);
        note_target(label, seg);
        let active = (i == 1) == modules;
        let fill = if active || pointer.pressing(hit) {
            term::surface_active()
        } else if pointer.hovering_over(hit) {
            term::surface_hover()
        } else {
            term::surface()
        };
        draw_surface(
            seg,
            &SurfaceStyle::new(fill).with_border(
                1.0,
                if active {
                    term::accent()
                } else {
                    term::faint()
                },
            ),
        );
        draw_text_centered_in_box_ex(
            label,
            seg.x,
            seg.y,
            seg.w,
            seg.h,
            TextStyle::new(15.0, if active { term::accent() } else { term::dim() }),
        );
        if pointer.released_on(hit) {
            ctx.ship_modules_tab.set(i == 1);
        }
    }
    let body = Rect::new(area.x, area.y + SEG_H + 10.0, area.w, area.h - SEG_H - 10.0);
    if modules {
        draw_modules(ctx, body, pointer, actions);
    } else {
        draw_loadout(ctx, body, pointer, actions);
    }
}

/// The LOADOUT catalog: hull / engine / weapon columns, each scrolling when it
/// overflows (a full column plus a found mission-reward part is one card too tall).
fn draw_loadout(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    let columns = [
        (ComponentKind::Hull, "HULLS"),
        (ComponentKind::Engine, "ENGINES"),
        (ComponentKind::Weapon, "WEAPONS"),
    ];
    let col_w = (area.w - 24.0) / 3.0;

    const STRIDE: f32 = 100.0;
    // Gutter reserved at each column's right edge for its scrollbar.
    const GUTTER: f32 = 12.0;
    let mut scrolls = ctx.ship_scroll.get();

    for (i, (kind, title)) in columns.iter().enumerate() {
        let rect = Rect::new(area.x + i as f32 * (col_w + 12.0), area.y, col_w, area.h);
        term_panel(rect, Some(title));
        let content = rect.inset(16.0);

        // Cards a mission-reward part joins only once owned; the rest are the
        // buyable catalog. Collect the visible set first so the column can scroll
        // when it overflows (a full catalog plus a found part is one card too tall).
        let cards: Vec<&ShipComponent> = ctx
            .data
            .ship_components
            .list(*kind)
            .iter()
            .filter(|c| {
                let installed = is_installed(ctx, *kind, &c.id);
                let salvaged = ctx.sim.ship.salvage.iter().any(|s| s == &c.id);
                !c.acquisition.is_mission_only() || installed || salvaged
            })
            .collect();

        // Scroll viewport: below the panel header, the full card width less the gutter.
        let view = Rect::new(
            content.x,
            content.y + 34.0,
            content.w,
            content.bottom() - (content.y + 34.0),
        );
        let card_w = view.w - GUTTER;
        let content_h = cards.len() as f32 * STRIDE;
        let scroll = &mut scrolls[i];
        scroll.update_at(view, content_h, pointer.position);
        // A swipe down a catalog column must not also buy the part it lifts over.
        let pointer = if scroll.absorbs_press() {
            pointer.suppressed()
        } else {
            pointer
        };

        let mut y = view.y - scroll.offset();
        for component in cards {
            let card = Rect::new(view.x, y, card_w, 96.0);
            // Cull partially-scrolled cards so none spills past the panel.
            if is_fully_visible(card, view) {
                let installed = is_installed(ctx, *kind, &component.id);
                draw_component_card(ctx, card, component, installed, pointer, *kind, actions);
            }
            y += STRIDE;
        }
        scroll.draw_scrollbar_with(
            view,
            content_h,
            term::surface_inset(),
            term::dim(),
            term::primary(),
        );
    }
    ctx.ship_scroll.set(scrolls);
}

/// The MODULES view: the six subsystems as named version ladders. Each panel
/// lists the module's versions bottom-to-top — those already passed, the one
/// fitted now, and the next one up as an INSTALL (drydock purchase). Buying emits
/// the same `UpgradeSubsystem` action the Subsystems tab uses.
fn draw_modules(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    const GAP: f32 = 12.0;
    let col_w = (area.w - GAP) / 2.0;
    let row_h = (area.h - 2.0 * GAP) / 3.0;
    for (i, id) in crate::data::GameData::sorted_ids(&ctx.data.subsystems)
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
        draw_module_ladder(ctx, rect, &id, pointer, actions);
    }
}

/// One subsystem's version ladder panel (see [`draw_modules`]).
fn draw_module_ladder(
    ctx: &GameplayCtx<'_>,
    rect: Rect,
    id: &str,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
) {
    let (Some(def), Some(state)) = (ctx.data.subsystems.get(id), ctx.sim.subsystems.get(id)) else {
        return;
    };
    term_panel(rect, Some(&def.name.to_uppercase()));
    let c = rect.inset(14.0);
    let versions = def.tiers.len() + 1; // baseline + each named upgrade
    let top = c.y + 28.0;
    let stride = ((c.bottom() - top) / versions as f32).min(34.0);

    for vi in 0..versions {
        let tier = vi as u32;
        let name = def.fitting_name(tier);
        let ry = top + vi as f32 * stride;
        let row = Rect::new(c.x, ry, c.w, stride - 4.0);

        if tier < state.tier {
            // A version already surpassed — record of the ladder climbed.
            draw_ui_text_ex(
                &format!("· {name}"),
                row.x + 6.0,
                row.y + 16.0,
                TextStyle::new(12.0, term::faint()).params(),
            );
        } else if tier == state.tier {
            // The version fitted right now.
            draw_surface(
                row,
                &SurfaceStyle::new(term::surface_active()).with_border(1.0, term::accent()),
            );
            draw_ui_text_ex(
                name,
                row.x + 8.0,
                row.y + 16.0,
                TextStyle::new(13.0, term::accent()).params(),
            );
            draw_text_right(
                "INSTALLED",
                row.right() - 8.0,
                row.y + 16.0,
                TextStyle::new(11.0, term::accent()),
            );
        } else if tier == state.tier + 1 {
            // The next rung — a drydock purchase, unless it is a mission reward
            // (never sold; unlocked only by a voyage — wired in a later pass).
            let fitting = &def.tiers[vi - 1];
            if fitting.acquisition.is_mission_only() {
                let unlocked = ctx
                    .sim
                    .ship
                    .unlocked_fittings
                    .iter()
                    .any(|f| f == &fitting.id);
                if unlocked {
                    // A voyage has recovered this version — fit it free (the mission
                    // was the price), distinct from a bought upgrade.
                    let label = format!("INSTALL {name} · RECOVERED");
                    if term_button(row, &label, true, pointer) {
                        actions.push(UiAction::InstallFitting(id.to_owned()));
                    }
                } else {
                    draw_surface(
                        row,
                        &SurfaceStyle::new(term::surface()).with_border(1.0, term::faint()),
                    );
                    draw_ui_text_ex(
                        name,
                        row.x + 8.0,
                        row.y + 16.0,
                        TextStyle::new(13.0, term::dim()).params(),
                    );
                    draw_text_right(
                        "MISSION REWARD",
                        row.right() - 8.0,
                        row.y + 16.0,
                        TextStyle::new(10.0, term::dim()),
                    );
                }
            } else {
                let cost = &fitting.cost;
                let mut bits = vec![format!("{}cr", cost.credits)];
                if cost.minerals > 0 {
                    bits.push(format!("{}min", cost.minerals));
                }
                let affordable = ctx.sim.resources.credits >= cost.credits
                    && ctx.sim.resources.minerals >= cost.minerals;
                let label = format!("INSTALL {name} · {}", bits.join(" + "));
                if term_button(row, &label, affordable, pointer) {
                    actions.push(UiAction::UpgradeSubsystem(id.to_owned()));
                }
            }
        } else {
            // A version further up the ladder, previewed but not yet reachable.
            draw_ui_text_ex(
                &format!("○ {name}"),
                row.x + 6.0,
                row.y + 16.0,
                TextStyle::new(12.0, term::faint()).params(),
            );
        }
    }
}

/// The under-way SHIP tab (real-time loop §5): a procedural blueprint of the
/// vessel. The central schematic ([`ship_schematic`]) draws the hull with every
/// module highlighted by condition, tier, and crew manning, and reacts on its own
/// to anything that changes mid-mission. A left rail carries the overview and the
/// one interactive under-way job — field-fitting salvaged parts. No
/// purchase/commission; those are drydock work.
fn draw_underway(ctx: &GameplayCtx<'_>, area: Rect, pointer: Pointer, actions: &mut Vec<UiAction>) {
    const GAP: f32 = 12.0;
    let left_w = 296.0;
    let left = Rect::new(area.x, area.y, left_w, area.h);
    let main = Rect::new(left.right() + GAP, area.y, area.w - left_w - GAP, area.h);

    // --- Main: procedural schematic over a live status strip ---
    let status_h = 120.0;
    let layout = Rect::new(main.x, main.y, main.w, main.h - status_h - GAP);
    term_panel(layout, Some("SHIP LAYOUT"));
    // Extra top margin clears the 34px panel header: the schematic's top deck-label
    // band and the legend both sit near frame.y, so a bare 16px inset let them
    // collide with the "SHIP LAYOUT" title. Sides/bottom stay at 16.
    let frame = Rect::new(
        layout.x + 16.0,
        layout.y + 40.0,
        layout.w - 32.0,
        layout.h - 56.0,
    );
    let schematic = ship_schematic::build(ctx.sim, ctx.data, frame);
    ship_schematic::draw(frame, &schematic);
    draw_legend(frame);

    let status = Rect::new(main.x, layout.bottom() + GAP, main.w, status_h);
    term_panel(status, Some("SHIP STATUS"));
    // Clear the 34px panel header before laying the tiles.
    let strip = Rect::new(
        status.x + 14.0,
        status.y + 40.0,
        status.w - 28.0,
        status.h - 52.0,
    );
    ship_schematic::draw_status_strip(strip, &schematic);

    // --- Left rail: overview + field ops ---
    let rail_h = (area.h - GAP) * 0.5;
    draw_overview(ctx, Rect::new(left.x, left.y, left.w, rail_h), &schematic);
    draw_field_ops(
        ctx,
        Rect::new(left.x, left.y + rail_h + GAP, left.w, rail_h),
        pointer,
        actions,
    );
}

/// A compact key for the schematic's highlight language, tucked in the top-left
/// where the hull leaves whitespace.
fn draw_legend(frame: Rect) {
    draw_ui_text_ex(
        "◉ MANNED   ○ VACANT",
        frame.x,
        frame.y + 14.0,
        TextStyle::new(11.0, term::dim()).params(),
    );
    let mut x = frame.x;
    draw_ui_text_ex(
        "COND",
        x,
        frame.y + 30.0,
        TextStyle::new(11.0, term::dim()).params(),
    );
    x += 42.0;
    for (label, color) in [
        ("GOOD", term::accent()),
        ("WORN", term::dim()),
        ("CRIT", term::alert()),
    ] {
        draw_ui_text_ex(
            label,
            x,
            frame.y + 30.0,
            TextStyle::new(11.0, color).params(),
        );
        x += 42.0;
    }
}

/// The left-rail overview: which ship this is, who it carries, and the loadout
/// at a glance.
fn draw_overview(ctx: &GameplayCtx<'_>, rect: Rect, schematic: &ship_schematic::ShipSchematic) {
    term_panel(rect, Some("SHIP OVERVIEW"));
    let c = rect.inset(16.0);
    // Drop the ship name clear of the 34px panel header — at the old offset its
    // caps sat on the header divider and read as overlapping.
    draw_ui_text_ex(
        &schematic.hull_name,
        c.x,
        c.y + 40.0,
        TextStyle::new(18.0, term::accent()).params(),
    );
    draw_ui_text_ex(
        "GENERATION SHIP · UNDER WAY",
        c.x,
        c.y + 58.0,
        TextStyle::new(11.0, term::dim()).params(),
    );
    let s = &schematic.stats;
    let mut y = c.y + 90.0;
    let rows = [
        ("SOULS ABOARD", ctx.sim.population.count.to_string()),
        ("CREW POSTS", ctx.sim.crew.len().to_string()),
        ("CARGO", s.cargo.to_string()),
        ("SPEED", s.speed.to_string()),
        ("COMBAT", s.combat.to_string()),
    ];
    for (label, value) in rows {
        stat_line(c.x, y, label, &value, term::accent());
        y += 24.0;
    }
}

/// Field ops (PLAN M4.4): the salvage hold and its under-way field-install
/// buttons — the one loadout change the black permits, gated by crew and stores.
fn draw_field_ops(
    ctx: &GameplayCtx<'_>,
    rect: Rect,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
) {
    term_panel(rect, Some("FIELD OPS · SALVAGE HOLD"));
    let c = rect.inset(16.0);
    if ctx.sim.ship.salvage.is_empty() {
        draw_ui_text_ex(
            "The salvage hold is empty.",
            c.x,
            c.y + 30.0,
            TextStyle::new(13.0, term::faint()).params(),
        );
        draw_ui_text_ex(
            "Parts found on the voyage are fitted here.",
            c.x,
            c.y + 50.0,
            TextStyle::new(11.0, term::dim()).params(),
        );
        return;
    }
    let mut y = c.y + 30.0;
    for id in ctx.sim.ship.salvage.clone() {
        let name = ctx
            .data
            .ship_components
            .find_any(&id)
            .map(|(_, comp)| comp.name.clone())
            .unwrap_or_else(|| id.clone());
        let (enabled, label) = match install_eligibility(ctx.sim, ctx.data, &id) {
            InstallEligibility::Ready => (true, format!("FIELD INSTALL — {name}")),
            InstallEligibility::NeedsEngineer => (false, format!("{name} · NEEDS ENGINEER")),
            InstallEligibility::NeedsConsumables => (false, format!("{name} · NEEDS PARTS")),
            _ => (false, format!("{name} · UNAVAILABLE")),
        };
        if term_button(Rect::new(c.x, y, c.w, 26.0), &label, enabled, pointer) {
            actions.push(UiAction::InstallSalvage(id.clone()));
        }
        y += 32.0;
    }
}

/// Compact terminal readout of a component's non-zero stats, e.g.
/// `CARGO 200 · SPD 2 · CBT 3`.
fn stats_line(stats: &ComponentStats) -> String {
    let mut parts = Vec::new();
    if stats.cargo != 0 {
        parts.push(format!("CARGO {}", stats.cargo));
    }
    if stats.crew_capacity != 0 {
        parts.push(format!("CREW {}", stats.crew_capacity));
    }
    if stats.speed != 0 {
        parts.push(format!("SPD {}", stats.speed));
    }
    if stats.combat != 0 {
        parts.push(format!("CBT {}", stats.combat));
    }
    if stats.fuel_regen != 0 {
        parts.push(format!("FUEL+{}", stats.fuel_regen));
    }
    parts.join(" · ")
}

fn is_installed(ctx: &GameplayCtx<'_>, kind: ComponentKind, id: &str) -> bool {
    let ship = &ctx.sim.ship;
    match kind {
        ComponentKind::Hull => ship.hull == id,
        ComponentKind::Engine => ship.engine == id,
        ComponentKind::Weapon => ship.weapon.as_deref() == Some(id),
    }
}

fn draw_component_card(
    ctx: &GameplayCtx<'_>,
    rect: Rect,
    component: &ShipComponent,
    installed: bool,
    pointer: Pointer,
    kind: ComponentKind,
    actions: &mut Vec<UiAction>,
) {
    let salvaged = ctx.sim.ship.salvage.iter().any(|s| s == &component.id);
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.07, 0.055, 0.012, 1.0)).with_border(
            1.0,
            if installed {
                term::accent()
            } else if salvaged {
                // A part in the salvage hold stands out brighter than the
                // buy-it-new catalog entries (PLAN M4.4).
                term::primary()
            } else {
                term::faint()
            },
        ),
    );
    draw_ui_text_ex(
        &component.name,
        rect.x + 12.0,
        rect.y + 20.0,
        TextStyle::new(
            15.0,
            if installed {
                term::accent()
            } else {
                term::primary()
            },
        )
        .params(),
    );
    // A part with no catalog price wears its provenance: a mission recovered it,
    // it cannot be bought, so the tag reads where a price would otherwise sit.
    if component.acquisition.is_mission_only() {
        draw_text_right(
            "MISSION REWARD",
            rect.right() - 12.0,
            rect.y + 20.0,
            TextStyle::new(10.0, term::dim()),
        );
    }
    draw_text_block(
        &component.description,
        rect.x + 12.0,
        rect.y + 26.0,
        rect.w - 24.0,
        24.0,
        11.0,
        2.0,
        term::dim(),
    );

    let stats = stats_line(&component.stats);
    if !stats.is_empty() {
        draw_ui_text_ex(
            &stats,
            rect.x + 12.0,
            rect.y + 56.0,
            TextStyle::new(12.0, term::accent()).params(),
        );
    }

    // Cost is folded into the button so the card stays compact enough for a
    // five-deep catalog column.
    let cost = &component.cost;
    let mut cost_parts = Vec::new();
    if cost.credits != 0 {
        cost_parts.push(format!("{} cr", cost.credits));
    }
    if cost.minerals != 0 {
        cost_parts.push(format!("{} min", cost.minerals));
    }
    if cost.energy != 0 {
        cost_parts.push(format!("{} en", cost.energy));
    }

    let btn = Rect::new(rect.x + 12.0, rect.y + 68.0, rect.w - 24.0, 22.0);
    if installed {
        draw_text_centered_in_box_ex(
            "INSTALLED",
            btn.x,
            btn.y,
            btn.w,
            btn.h,
            TextStyle::new(14.0, term::accent()),
        );
    } else if salvaged {
        // A found part installs from the hold rather than being bought — free
        // in port, gated by crew + parts underway (PLAN M4.4).
        let (enabled, label) = match install_eligibility(ctx.sim, ctx.data, &component.id) {
            InstallEligibility::Ready if ctx.sim.contract.is_none() => (true, "INSTALL (SALVAGED)"),
            InstallEligibility::Ready => (true, "FIELD INSTALL (SALVAGED)"),
            InstallEligibility::NeedsDrydock => (false, "SALVAGED · NEEDS DRYDOCK"),
            InstallEligibility::NeedsEngineer => (false, "SALVAGED · NEEDS ENGINEER"),
            InstallEligibility::NeedsConsumables => (false, "SALVAGED · NEEDS PARTS"),
            InstallEligibility::NotSalvaged => (false, "SALVAGED"),
        };
        if term_button(btn, label, enabled, pointer) {
            actions.push(UiAction::InstallSalvage(component.id.clone()));
        }
    } else if kind == ComponentKind::Hull {
        // A new hull is a whole new ship — commissioning it fully refits the
        // vessel and lifts hope, port-only, at the hull price + a premium
        // (PLAN M4.5).
        let cm = ctx.data.config.commission;
        let in_port = ctx.sim.contract.is_none();
        let total_credits = cost.credits + cm.premium_credits;
        let total_minerals = cost.minerals + cm.premium_minerals;
        let label = if in_port {
            let mut bits = vec![format!("{total_credits} cr")];
            if total_minerals > 0 {
                bits.push(format!("{total_minerals} min"));
            }
            format!("COMMISSION · {}", bits.join(" + "))
        } else {
            "COMMISSION · PORT ONLY".to_owned()
        };
        let affordable = in_port
            && ctx.sim.resources.credits >= total_credits
            && ctx.sim.resources.minerals >= total_minerals;
        if term_button(btn, &label, affordable, pointer) {
            actions.push(UiAction::CommissionShip(component.id.clone()));
        }
    } else {
        // Buying a component is a drydock job — port-only (PLAN M4.6).
        let in_port = ctx.sim.contract.is_none();
        let label = if !in_port {
            "PURCHASE · PORT ONLY".to_owned()
        } else if cost_parts.is_empty() {
            "INSTALL (free)".to_owned()
        } else {
            format!("PURCHASE · {}", cost_parts.join(" + "))
        };
        let negated = crate::data::ResourceDelta {
            credits: -cost.credits,
            energy: -cost.energy,
            minerals: -cost.minerals,
            food: -cost.food,
            influence: -cost.influence,
        };
        let affordable = in_port && ctx.sim.resources.can_afford(&negated);
        if term_button(btn, &label, affordable, pointer) {
            actions.push(UiAction::PurchaseComponent(kind, component.id.clone()));
        }
    }
}
