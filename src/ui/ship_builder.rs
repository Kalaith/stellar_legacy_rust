//! Ship Builder: component catalog and current loadout (GDD §9).

use crate::data::ship_components::{ComponentKind, ShipComponent};
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
            let card = Rect::new(content.x, y, content.w, 118.0);
            draw_component_card(ctx, card, component, installed, mouse, *kind, actions);
            y += 128.0;
        }
    }
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
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.07, 0.055, 0.012, 1.0)).with_border(
            1.0,
            if installed {
                term::GREEN
            } else {
                term::AMBER_FAINT
            },
        ),
    );
    draw_ui_text_ex(
        &component.name,
        rect.x + 12.0,
        rect.y + 22.0,
        TextStyle::new(16.0, if installed { term::GREEN } else { term::AMBER }).params(),
    );
    draw_text_block(
        &component.description,
        rect.x + 12.0,
        rect.y + 30.0,
        rect.w - 24.0,
        30.0,
        12.0,
        2.0,
        term::AMBER_DIM,
    );

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
    let cost_text = if cost_parts.is_empty() {
        "installed at launch".to_owned()
    } else {
        cost_parts.join(" + ")
    };
    draw_ui_text_ex(
        &cost_text,
        rect.x + 12.0,
        rect.y + 76.0,
        TextStyle::new(13.0, term::AMBER_DIM).params(),
    );

    let btn = Rect::new(rect.x + 12.0, rect.y + 84.0, rect.w - 24.0, 26.0);
    if installed {
        draw_text_centered_in_box_ex(
            "INSTALLED",
            btn.x,
            btn.y,
            btn.w,
            btn.h,
            TextStyle::new(14.0, term::GREEN),
        );
    } else {
        let negated = crate::data::ResourceDelta {
            credits: -cost.credits,
            energy: -cost.energy,
            minerals: -cost.minerals,
            food: -cost.food,
            influence: -cost.influence,
        };
        let affordable = ctx.sim.resources.can_afford(&negated);
        if term_button(btn, "PURCHASE & INSTALL", affordable, mouse) {
            actions.push(UiAction::PurchaseComponent(kind, component.id.clone()));
        }
    }
}
