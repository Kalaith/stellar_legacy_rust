//! CRT display-settings overlay — the monitor's on-screen "display" menu.
//! Reachable from any screen (F1). Returns [`DisplayAction`] intents the game
//! applies to its `DisplaySettings`; it never mutates state here.

use crate::settings::{DisplaySettings, Phosphor};
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
    Close,
}

pub fn draw(display: &DisplaySettings, mouse: Vec2) -> Vec<DisplayAction> {
    let mut actions = Vec::new();

    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.8),
    );

    let panel = Rect::new(
        LOGICAL_WIDTH / 2.0 - 240.0,
        LOGICAL_HEIGHT / 2.0 - 190.0,
        480.0,
        380.0,
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
        TextStyle::new(16.0, term::AMBER_DIM).params(),
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
    y += 58.0;

    draw_ui_text_ex(
        "F1 / F10 toggle this panel and the CRT effect.",
        content.x,
        y + 14.0,
        TextStyle::new(13.0, term::AMBER_FAINT).params(),
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
        TextStyle::new(16.0, term::AMBER_DIM).params(),
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
        Color::new(0.2, 0.15, 0.02, 1.0)
    } else if hovered {
        Color::new(0.12, 0.09, 0.015, 1.0)
    } else {
        Color::new(0.07, 0.055, 0.012, 1.0)
    };
    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(
            1.0,
            if active {
                term::AMBER
            } else {
                term::AMBER_FAINT
            },
        ),
    );
    draw_text_centered_in_box_ex(
        label,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(15.0, if active { term::GREEN } else { term::AMBER_DIM }),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}
