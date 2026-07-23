//! Ship Builder: component catalog and current loadout (GDD §9).

use crate::data::ship_components::{ComponentKind, ComponentStats, ShipComponent};
use crate::simulation::ship::{install_eligibility, loadout_stats, InstallEligibility};
use crate::ui::{term, term_button, term_meter, term_panel, GameplayCtx, UiAction};
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

/// The under-way SHIP readout (real-time loop §5): installed hull/engine/weapon
/// with their stats, the ship's live integrity meters, and the aggregate loadout
/// boosts currently in force. No purchase/commission — those are drydock jobs.
fn draw_underway(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let sim = ctx.sim;
    let left = Rect::new(area.x, area.y, area.w * 0.5 - 6.0, area.h);
    let right = Rect::new(left.right() + 12.0, area.y, area.w - left.w - 12.0, area.h);

    // --- Installed modules ---
    term_panel(left, Some("INSTALLED MODULES"));
    let content = left.inset(18.0);
    let mut y = content.y + 40.0;
    let modules = [
        (ComponentKind::Hull, "HULL", Some(sim.ship.hull.as_str())),
        (
            ComponentKind::Engine,
            "ENGINE",
            Some(sim.ship.engine.as_str()),
        ),
        (ComponentKind::Weapon, "WEAPON", sim.ship.weapon.as_deref()),
    ];
    for (kind, slot, id) in modules {
        let component = id.and_then(|i| ctx.data.ship_components.find(kind, i));
        draw_ui_text_ex(
            slot,
            content.x,
            y,
            TextStyle::new(12.0, term::dim()).params(),
        );
        match component {
            Some(c) => {
                draw_ui_text_ex(
                    &c.name,
                    content.x + 80.0,
                    y,
                    TextStyle::new(15.0, term::accent()).params(),
                );
                let stats = stats_line(&c.stats);
                if !stats.is_empty() {
                    draw_ui_text_ex(
                        &stats,
                        content.x + 80.0,
                        y + 18.0,
                        TextStyle::new(12.0, term::primary()).params(),
                    );
                }
            }
            None => {
                draw_ui_text_ex(
                    "— none —",
                    content.x + 80.0,
                    y,
                    TextStyle::new(15.0, term::faint()).params(),
                );
            }
        }
        y += 48.0;
    }

    // Aggregate loadout modifiers currently in force — the ship's live boosts.
    y += 8.0;
    draw_ui_text_ex(
        "ACTIVE MODIFIERS",
        content.x,
        y,
        TextStyle::new(14.0, term::primary()).params(),
    );
    y += 22.0;
    let agg = loadout_stats(sim, ctx.data);
    let agg_line = stats_line(&agg);
    draw_ui_text_ex(
        if agg_line.is_empty() {
            "no loadout bonuses"
        } else {
            &agg_line
        },
        content.x,
        y,
        TextStyle::new(13.0, term::accent()).params(),
    );

    // --- Integrity + salvage hold ---
    term_panel(right, Some("INTEGRITY"));
    let rc = right.inset(18.0);
    let mut ry = rc.y + 42.0;
    let meters = [
        ("HULL", sim.ship.hull_integrity),
        ("LIFE SUPPORT", sim.ship.life_support),
        ("FUEL", sim.ship.fuel),
    ];
    for (label, value) in meters {
        term_meter(
            Rect::new(rc.x, ry, rc.w, 22.0),
            value,
            1.0,
            &format!("{label} {:.0}%", value * 100.0),
        );
        ry += 34.0;
    }
    ry += 6.0;
    draw_ui_text_ex(
        &format!("SPARE PARTS {}", sim.ship.spare_parts),
        rc.x,
        ry,
        TextStyle::new(14.0, term::accent()).params(),
    );
    ry += 30.0;

    // A part in the salvage hold can still be field-fitted under way if crew and
    // stores allow (PLAN M4.4) — the one loadout change the black permits.
    if !sim.ship.salvage.is_empty() {
        draw_ui_text_ex(
            "SALVAGE HOLD",
            rc.x,
            ry,
            TextStyle::new(14.0, term::primary()).params(),
        );
        ry += 24.0;
        for id in sim.ship.salvage.clone() {
            let name = ctx
                .data
                .ship_components
                .find_any(&id)
                .map(|(_, c)| c.name.clone())
                .unwrap_or_else(|| id.clone());
            let (enabled, label) = match install_eligibility(sim, ctx.data, &id) {
                InstallEligibility::Ready => (true, format!("FIELD INSTALL — {name}")),
                InstallEligibility::NeedsEngineer => (false, format!("{name} · NEEDS ENGINEER")),
                InstallEligibility::NeedsConsumables => (false, format!("{name} · NEEDS PARTS")),
                _ => (false, format!("{name} · UNAVAILABLE")),
            };
            if term_button(Rect::new(rc.x, ry, rc.w, 26.0), &label, enabled, mouse) {
                actions.push(UiAction::InstallSalvage(id.clone()));
            }
            ry += 30.0;
        }
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
