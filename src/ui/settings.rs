//! CRT display-settings overlay — the monitor's on-screen "display" menu.
//! Reachable from any screen (F1). Returns [`DisplayAction`] intents the game
//! applies to its `DisplaySettings`; it never mutates state here.

use crate::data::events::EventCategory;
use crate::settings::{DisplaySettings, Phosphor};
use crate::state::sim::DelegationSettings;
use crate::ui::{term, term_button, term_panel, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

/// A change the display overlay is requesting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayAction {
    ToggleCrt,
    ToggleScanlines,
    ToggleFlicker,
    SetPhosphor(Phosphor),
    /// Flip whether this category is delegated by default in new voyages.
    ToggleDelegationDefault(EventCategory),
    Close,
}

pub fn draw(
    display: &DisplaySettings,
    delegation: &DelegationSettings,
    mouse: Vec2,
) -> Vec<DisplayAction> {
    let mut actions = Vec::new();

    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.8),
    );

    let panel = Rect::new(
        LOGICAL_WIDTH / 2.0 - 250.0,
        LOGICAL_HEIGHT / 2.0 - 280.0,
        500.0,
        560.0,
    );
    term_panel(panel, Some("DISPLAY // CRT MONITOR"));
    let content = panel.inset(28.0);
    let mut y = content.y + 30.0;

    // On/off rows: label left, a single toggle button right.
    toggle_row(
        content.x,
        y,
        content.w,
        "CRT EFFECT",
        display.crt_enabled,
        mouse,
        DisplayAction::ToggleCrt,
        &mut actions,
    );
    y += 52.0;
    toggle_row(
        content.x,
        y,
        content.w,
        "SCANLINES",
        display.scanlines,
        mouse,
        DisplayAction::ToggleScanlines,
        &mut actions,
    );
    y += 52.0;
    toggle_row(
        content.x,
        y,
        content.w,
        "FLICKER",
        display.flicker,
        mouse,
        DisplayAction::ToggleFlicker,
        &mut actions,
    );
    y += 52.0;

    // Phosphor: two mutually-exclusive choices.
    draw_ui_text_ex(
        "PHOSPHOR",
        content.x,
        y + 22.0,
        TextStyle::new(16.0, term::dim()).params(),
    );
    let bw = 92.0;
    if choice_button(
        Rect::new(content.right() - bw * 2.0 - 8.0, y, bw, 34.0),
        "AMBER",
        display.phosphor == Phosphor::Amber,
        mouse,
    ) {
        actions.push(DisplayAction::SetPhosphor(Phosphor::Amber));
    }
    if choice_button(
        Rect::new(content.right() - bw, y, bw, 34.0),
        "GREEN",
        display.phosphor == Phosphor::Green,
        mouse,
    ) {
        actions.push(DisplayAction::SetPhosphor(Phosphor::Green));
    }
    y += 62.0;

    // Delegation defaults: which council categories auto-resolve in new voyages.
    draw_ui_text_ex(
        "DELEGATION DEFAULTS // NEW VOYAGES",
        content.x,
        y,
        TextStyle::new(14.0, term::primary()).params(),
    );
    y += 24.0;
    for category in EventCategory::ALL {
        let delegated = delegation.is_delegated(category);
        draw_ui_text_ex(
            &category.label().to_uppercase(),
            content.x,
            y + 21.0,
            TextStyle::new(15.0, term::dim()).params(),
        );
        let bw = 120.0;
        if choice_button(
            Rect::new(content.right() - bw, y, bw, 32.0),
            if delegated { "DELEGATED" } else { "COUNCIL" },
            delegated,
            mouse,
        ) {
            actions.push(DisplayAction::ToggleDelegationDefault(category));
        }
        y += 40.0;
    }
    y += 6.0;

    draw_ui_text_ex(
        "F1 / F10 toggle this panel and the CRT effect.",
        content.x,
        y + 12.0,
        TextStyle::new(13.0, term::faint()).params(),
    );

    if term_button(
        Rect::new(content.x, content.bottom() - 44.0, content.w, 40.0),
        "CLOSE",
        true,
        mouse,
    ) {
        actions.push(DisplayAction::Close);
    }

    actions
}

#[allow(clippy::too_many_arguments)]
fn toggle_row(
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    on: bool,
    mouse: Vec2,
    action: DisplayAction,
    actions: &mut Vec<DisplayAction>,
) {
    draw_ui_text_ex(
        label,
        x,
        y + 22.0,
        TextStyle::new(16.0, term::dim()).params(),
    );
    let rect = Rect::new(x + w - 92.0, y, 92.0, 34.0);
    if choice_button(rect, if on { "ON" } else { "OFF" }, on, mouse) {
        actions.push(action);
    }
}

/// A button whose fill/border brightens when it represents the active choice.
fn choice_button(rect: Rect, label: &str, active: bool, mouse: Vec2) -> bool {
    let hovered = rect.contains_point(mouse);
    let fill = if active {
        term::surface_active()
    } else if hovered {
        term::surface_hover()
    } else {
        term::surface_inset()
    };
    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(
            1.0,
            if active {
                term::primary()
            } else {
                term::faint()
            },
        ),
    );
    draw_text_centered_in_box_ex(
        label,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(15.0, if active { term::accent() } else { term::dim() }),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}
