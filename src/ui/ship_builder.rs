//! Ship Builder: component catalog and current loadout (GDD §9).

use crate::data::ship_components::{ComponentKind, ComponentStats, ShipComponent};
use crate::simulation::ship::{install_eligibility, InstallEligibility};
use crate::ui::{term, term_button, term_panel, GameplayCtx, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, area: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
        let label = if cost_parts.is_empty() {
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
        let affordable = ctx.sim.resources.can_afford(&negated);
        if term_button(btn, &label, affordable, mouse) {
            actions.push(UiAction::PurchaseComponent(kind, component.id.clone()));
        }
    }
}
