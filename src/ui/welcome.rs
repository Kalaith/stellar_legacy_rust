//! First-run WELCOME overlay: a dismissible full-screen briefing shown once per
//! install (gated on a saved flag) over the new-game picker, greeting a new
//! commander after they choose NEW GAME, before they pick a legacy and crew.
//! Pure view — returns true on the frame the player
//! dismisses it (button click); the game also treats any keypress as dismiss, to
//! honor the "press any key" label. All text is data (`config.welcome`).

use crate::data::WelcomeConfig;
use crate::ui::{term, term_button, term_panel, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_text_block, draw_ui_text_ex, RectExt};

/// Draw the overlay. Returns true when the player clicks the dismiss button.
pub fn draw(welcome: &WelcomeConfig, pointer: Pointer) -> bool {
    // Dim the menu behind the briefing.
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    let panel = Rect::new(
        LOGICAL_WIDTH / 2.0 - 380.0,
        LOGICAL_HEIGHT / 2.0 - 300.0,
        760.0,
        600.0,
    );
    term_panel(panel, Some("STELLAR LEGACY // ORIENTATION"));
    let content = panel.inset(30.0);

    let mut y = content.y + 34.0;
    draw_ui_text_ex(
        &welcome.title,
        content.x,
        y,
        TextStyle::new(24.0, term::primary()).params(),
    );
    y += 30.0;
    let intro = draw_text_block(
        &welcome.intro,
        content.x,
        y,
        content.w,
        56.0,
        15.0,
        4.0,
        term::dim(),
    );
    y += intro.lines.len() as f32 * 21.0 + 16.0;

    for section in &welcome.sections {
        draw_ui_text_ex(
            &section.heading,
            content.x,
            y,
            TextStyle::new(15.0, term::accent()).params(),
        );
        y += 22.0;
        let body = draw_text_block(
            &section.body,
            content.x,
            y,
            content.w,
            84.0,
            14.0,
            4.0,
            term::primary(),
        );
        y += body.lines.len() as f32 * 20.0 + 18.0;
    }

    term_button(
        Rect::new(content.x, content.bottom() - 44.0, content.w, 40.0),
        &welcome.dismiss_label,
        true,
        pointer,
    )
}
