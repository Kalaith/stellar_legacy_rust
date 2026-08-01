//! HOMECOMING: the full-screen report a charter comes home with (GDD §5.2).
//!
//! The counterpart to `game_over` — extinction already got a takeover screen
//! and a final readout, while success, the run's actual climax, was two lines
//! pushed into a scrolling log that scrolls away. This screen is where the
//! voyage is paid, scored, and remembered: what the writ paid, how the score
//! was arrived at, which marks were made, what the council decided along the
//! way, and every captain who held the chair between launch and homecoming.
//!
//! Pure view, like every screen here: it reads `sim.debrief` and returns a
//! single [`UiAction::FileReport`] when the player is done reading.

mod columns;

use crate::state::sim::debrief::VoyageDebrief;
use crate::ui::{
    term, term_button, term_panel, GameplayCtx, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH,
};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

/// Column geometry. Three panels under the banner: the report itself, the
/// chain of command, and the voyage log.
const MARGIN: f32 = 18.0;
const GAP: f32 = 12.0;
const BANNER_H: f32 = 92.0;
const FOOTER_H: f32 = 54.0;

pub fn draw(ctx: &GameplayCtx<'_>, pointer: Pointer, actions: &mut Vec<UiAction>) {
    let Some(report) = ctx.sim.debrief.as_ref() else {
        return;
    };
    draw_rectangle(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT, term::bg());
    draw_banner(report);

    let top = BANNER_H + 8.0;
    let body_h = LOGICAL_HEIGHT - top - FOOTER_H - MARGIN;
    let usable = LOGICAL_WIDTH - MARGIN * 2.0 - GAP * 2.0;
    // The report column carries the numbers and needs the most room; the two
    // list columns are narrower and scroll.
    let report_w = (usable * 0.40).floor();
    let command_w = (usable * 0.26).floor();
    let log_w = usable - report_w - command_w;

    let report_col = Rect::new(MARGIN, top, report_w, body_h);
    let command_col = Rect::new(report_col.right() + GAP, top, command_w, body_h);
    let log_col = Rect::new(command_col.right() + GAP, top, log_w, body_h);

    columns::draw_report(report, report_col);
    columns::draw_commanders(ctx, report, command_col, pointer);
    columns::draw_voyage_log(ctx, report, log_col, pointer);

    // One way out. Filing the report clears it and returns the ship to the
    // drydock board, where the next charter is chosen.
    let btn = Rect::new(
        LOGICAL_WIDTH / 2.0 - 190.0,
        LOGICAL_HEIGHT - FOOTER_H - 2.0,
        380.0,
        42.0,
    );
    let caret = if blink(get_time() as f32, 2.5) {
        ">"
    } else {
        " "
    };
    if term_button(btn, &format!("{caret} FILE THE REPORT"), true, pointer) {
        actions.push(UiAction::FileReport);
    }
}

/// The banner: what came home, and how it went.
fn draw_banner(report: &VoyageDebrief) {
    let band = outcome_tone(&report.outcome);
    draw_text_glow(
        "HOMECOMING",
        LOGICAL_WIDTH / 2.0 - 148.0,
        52.0,
        TextStyle::new(42.0, band),
        0.14,
        3.0,
    );
    let subtitle = format!(
        "// {} · {} //",
        report.contract_name.to_uppercase(),
        report.outcome.to_uppercase()
    );
    // Centred by measurement rather than a magic offset: charter names run from
    // "The Long Tow" to "Founding Charter: Meridian Reach", and a fixed nudge
    // that suits one leaves the other hanging off the edge.
    let width = measure_text(&subtitle, None, 16, 1.0).width;
    draw_ui_text_ex(
        &subtitle,
        (LOGICAL_WIDTH - width) / 2.0,
        78.0,
        TextStyle::new(16.0, term::dim()).params(),
    );
}

/// Warm-red for a defaulted charter, the tube's own primary for anything the
/// ship can be proud of — the same signal language `game_over` uses.
pub(super) fn outcome_tone(outcome: &str) -> Color {
    match outcome.to_lowercase().as_str() {
        "failure" => term::alert(),
        "pyrrhic" => term::dim(),
        _ => term::primary(),
    }
}

/// A panel with its header, returning the inset content rect — every column
/// here opens the same way.
pub(super) fn column_panel(area: Rect, title: &str) -> Rect {
    term_panel(area, Some(title));
    let content = area.inset(18.0);
    Rect::new(content.x, content.y + 26.0, content.w, content.h - 26.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_defaulted_charter_is_toned_apart_from_one_the_ship_can_be_proud_of() {
        // The band is the first thing read off the banner, so a failure must
        // not come home wearing the same color as a completed run.
        assert_eq!(outcome_tone("Failure"), term::alert());
        assert_ne!(outcome_tone("Complete"), term::alert());
        assert_ne!(outcome_tone("Partial"), term::alert());
        // The lookup is case-insensitive: the band arrives as a `SuccessLevel`
        // label ("Failure"), but the report stores whatever it was given.
        assert_eq!(outcome_tone("failure"), outcome_tone("FAILURE"));
    }
}
