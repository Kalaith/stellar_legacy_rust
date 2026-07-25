//! Ship Builder: component catalog and current loadout (GDD §9).

use crate::data::ship_components::{ComponentKind, ComponentStats, ShipComponent};
use crate::simulation::ship::{install_eligibility, InstallEligibility};
use crate::ui::{ship_schematic, stat_line, term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    // Under way the SHIP tab is a status readout, not a shipyard (real-time loop
    // §5): installed modules, current integrity, and the boosts/debuffs in force.
    // Buying, commissioning, and refits wait for the drydock.
    if ctx.sim.contract.is_some() {
        draw_underway(ctx, area, mouse, actions);
        return;
    }

    let columns = [
        (ComponentKind::Hull, "HULLS"),
        (ComponentKind::Engine, "ENGINES"),
        (ComponentKind::Weapon, "WEAPONS"),
    ];
    let col_w = (area.w - 24.0) / 3.0;

    for (i, (kind, title)) in columns.iter().enumerate() {
        let rect = Rect::new(area.x + i as f32 * (col_w + 12.0), area.y, col_w, area.h);
        term_panel(rect, Some(title));
        let content = rect.inset(16.0);
        let mut y = content.y + 40.0;

        for component in ctx.data.ship_components.list(*kind) {
            let installed = is_installed(ctx, *kind, &component.id);
            let card = Rect::new(content.x, y, content.w, 96.0);
            draw_component_card(ctx, card, component, installed, mouse, *kind, actions);
            y += 100.0;
        }
    }
}

/// The under-way SHIP tab (real-time loop §5): a procedural blueprint of the
/// vessel. The central schematic ([`ship_schematic`]) draws the hull with every
/// module highlighted by condition, tier, and crew manning, and reacts on its own
/// to anything that changes mid-mission. A left rail carries the overview and the
/// one interactive under-way job — field-fitting salvaged parts. No
/// purchase/commission; those are drydock work.
fn draw_underway(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
        mouse,
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
fn draw_field_ops(ctx: &GameplayCtx<'_>, rect: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
        if term_button(Rect::new(c.x, y, c.w, 26.0), &label, enabled, mouse) {
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
    mouse: Vec2,
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
        if term_button(btn, label, enabled, mouse) {
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
        if term_button(btn, &label, affordable, mouse) {
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
        if term_button(btn, &label, affordable, mouse) {
            actions.push(UiAction::PurchaseComponent(kind, component.id.clone()));
        }
    }
}
