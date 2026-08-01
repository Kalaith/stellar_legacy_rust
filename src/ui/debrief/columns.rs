//! The homecoming's three columns: the report (prose, tallies, pay, scorecard
//! and marks), the chain of command, and the voyage log. Split from
//! `debrief.rs` so neither file carries both the page layout and the contents
//! of every panel on it.

use super::{column_panel, outcome_tone};
use crate::state::sim::debrief::VoyageDebrief;
use crate::ui::{spec_line, term, term_bar, GameplayCtx};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, is_fully_visible};

/// Reserved at a scrolling panel's right edge for the scrollbar.
const GUTTER: f32 = 12.0;

/// The report column: what the voyage cost and earned, and the arithmetic
/// behind the band on the banner.
pub(super) fn draw_report(report: &VoyageDebrief, area: Rect) {
    let content = column_panel(area, "VOYAGE REPORT");
    let mut y = content.y + 8.0;

    // Lead with the authored prose — the one line that says how it felt, not
    // how it scored.
    if let Some(line) = &report.homecoming_line {
        let block = draw_text_block(
            line,
            content.x,
            y,
            content.w,
            60.0,
            14.0,
            4.0,
            term::primary(),
        );
        y += block.lines.len() as f32 * 19.0 + 14.0;
    }

    // The tallies.
    let rows: [(&str, String); 5] = [
        (
            "YEARS UNDER WAY",
            format!(
                "{} (Y{:03}-Y{:03})",
                report.duration_years, report.began_year, report.ended_year
            ),
        ),
        ("GENERATIONS PASSED", report.generations.to_string()),
        (
            "SOULS ABOARD",
            format!(
                "{} -> {} ({:+})",
                report.population_start,
                report.population_end,
                report.population_change()
            ),
        ),
        ("MARKS MADE", {
            let (hit, total) = report.milestones_reached();
            format!("{hit} / {total}")
        }),
        ("FINAL SCORE", format!("{:.2}", report.score)),
    ];
    for (label, value) in rows {
        let tone = if label == "FINAL SCORE" {
            outcome_tone(&report.outcome)
        } else {
            term::accent()
        };
        spec_line(content.x, y, content.w, label, &value, tone);
        y += 22.0;
    }
    y += 10.0;

    y = draw_payout(report, content, y);
    draw_scorecard(report, content, y);
}

/// What the charter actually paid, after the objective proration and the
/// ship's reputation multiplier — not the writ's headline figure.
fn draw_payout(report: &VoyageDebrief, content: Rect, mut y: f32) -> f32 {
    draw_ui_text_ex(
        "-- PAY --",
        content.x,
        y,
        TextStyle::new(13.0, term::faint()).params(),
    );
    y += 20.0;

    if report.unpaid() {
        draw_ui_text_ex(
            "The writ paid nothing.",
            content.x,
            y,
            TextStyle::new(14.0, term::alert()).params(),
        );
        return y + 26.0;
    }

    let p = &report.payout;
    let paid: Vec<(&str, i64)> = [
        ("CREDITS", p.credits),
        ("ENERGY", p.energy),
        ("MINERALS", p.minerals),
        ("FOOD", p.food),
        ("INFLUENCE", p.influence),
    ]
    .into_iter()
    .filter(|(_, amount)| *amount != 0)
    .collect();

    // Two per row: the pay is rarely more than two or three lines, and a full
    // column of mostly-zero rows reads as noise.
    for chunk in paid.chunks(2) {
        for (col, (label, amount)) in chunk.iter().enumerate() {
            let x = content.x + col as f32 * (content.w / 2.0);
            draw_ui_text_ex(
                &format!("{label} {amount:+}"),
                x,
                y,
                TextStyle::new(14.0, term::accent()).params(),
            );
        }
        y += 20.0;
    }
    y + 8.0
}

/// The scorecard: every success metric with the target it was weighed against,
/// so the band on the banner is explained rather than pronounced.
fn draw_scorecard(report: &VoyageDebrief, content: Rect, mut y: f32) {
    draw_ui_text_ex(
        "-- SCORED ON --",
        content.x,
        y,
        TextStyle::new(13.0, term::faint()).params(),
    );
    y += 20.0;

    if report.metrics.is_empty() {
        draw_ui_text_ex(
            "No metrics recorded.",
            content.x,
            y,
            TextStyle::new(13.0, term::dim()).params(),
        );
        return;
    }

    for metric in &report.metrics {
        // Stop drawing rather than spill past the panel: a charter with an
        // unusual number of metrics must not overrun the frame.
        if y + 30.0 > content.bottom() {
            break;
        }
        let frac = if metric.target > 0.0 {
            (metric.current / metric.target).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let fill = if frac < 0.5 {
            term::alert()
        } else {
            term::accent()
        };
        term_bar(
            Rect::new(content.x, y, content.w, 18.0),
            frac,
            fill,
            &metric.name,
            &format!("{:.2}/{:.2}", metric.current, metric.target),
        );
        // The weight is what turns a met metric into score; showing it is the
        // difference between "you did well at this" and "this is why the number
        // is what it is".
        draw_ui_text_ex(
            &format!(
                "  weight {:.2}  ->  +{:.3}",
                metric.weight,
                metric.contribution()
            ),
            content.x,
            y + 32.0,
            TextStyle::new(12.0, term::faint()).params(),
        );
        y += 46.0;
    }
}

/// Every captain who held the chair between launch and homecoming. On a long
/// charter this is the column that makes the generational premise land: the
/// commander who signed the writ is rarely the one who files the report.
pub(super) fn draw_commanders(
    ctx: &GameplayCtx<'_>,
    report: &VoyageDebrief,
    area: Rect,
    pointer: Pointer,
) {
    let content = column_panel(area, "CHAIN OF COMMAND");

    if report.commanders.is_empty() {
        draw_text_block(
            "No command record for this voyage.",
            content.x,
            content.y + 8.0,
            content.w,
            40.0,
            13.0,
            4.0,
            term::dim(),
        );
        return;
    }

    draw_ui_text_ex(
        &format!(
            "{} held the chair",
            plural(report.commanders.len(), "captain", "captains")
        ),
        content.x,
        content.y + 6.0,
        TextStyle::new(13.0, term::faint()).params(),
    );

    let view = Rect::new(
        content.x,
        content.y + 18.0,
        content.w,
        content.bottom() - (content.y + 18.0),
    );
    const STRIDE: f32 = 52.0;
    let content_h = report.commanders.len() as f32 * STRIDE;
    let mut scroll = ctx.debrief_commanders_scroll.get();
    scroll.update_at(view, content_h, pointer.position);

    let mut row_top = view.y - scroll.offset();
    for (i, reign) in report.commanders.iter().enumerate() {
        let row = Rect::new(view.x, row_top, view.w - GUTTER, STRIDE - 6.0);
        row_top += STRIDE;
        // macroquad has no scissor rect, so cull partly-scrolled rows rather
        // than letting them spill past the panel.
        if !is_fully_visible(row, view) {
            continue;
        }
        // The captain who was sitting when the writ was signed is marked, so a
        // long chain still reads as "this began under her".
        let marker = if i == 0 { ">" } else { " " };
        draw_ui_text_ex(
            &format!("{marker} {}", reign.name),
            row.x,
            row.y + 14.0,
            TextStyle::new(15.0, term::primary()).params(),
        );
        let held = reign.years_held(report.ended_year);
        draw_ui_text_ex(
            &format!("  gen {} - {held} yr in the chair", reign.generation),
            row.x,
            row.y + 31.0,
            TextStyle::new(12.0, term::dim()).params(),
        );
        if !reign.trait_name.is_empty() {
            draw_ui_text_ex(
                &format!("  {}", reign.trait_name),
                row.x,
                row.y + 45.0,
                TextStyle::new(11.0, term::faint()).params(),
            );
        }
    }

    scroll.draw_scrollbar_with(
        view,
        content_h,
        term::surface_inset(),
        term::dim(),
        term::primary(),
    );
    ctx.debrief_commanders_scroll.set(scroll);
}

/// The voyage log: the beats remembered as they happened — marks made, legs
/// turned, chairs passed on, and above all the council's own decisions.
pub(super) fn draw_voyage_log(
    ctx: &GameplayCtx<'_>,
    report: &VoyageDebrief,
    area: Rect,
    pointer: Pointer,
) {
    let content = column_panel(area, "WHAT HAPPENED");

    if report.highlights.is_empty() {
        draw_text_block(
            "An uneventful crossing. Nothing was put to the council, and no marks were made.",
            content.x,
            content.y + 8.0,
            content.w,
            60.0,
            13.0,
            4.0,
            term::dim(),
        );
        return;
    }

    let view = Rect::new(content.x, content.y, content.w, content.h);
    const STRIDE: f32 = 40.0;
    let content_h = report.highlights.len() as f32 * STRIDE;
    let mut scroll = ctx.debrief_log_scroll.get();
    scroll.update_at(view, content_h, pointer.position);

    let mut row_top = view.y - scroll.offset();
    for beat in &report.highlights {
        let row = Rect::new(view.x, row_top, view.w - GUTTER, STRIDE - 4.0);
        row_top += STRIDE;
        if !is_fully_visible(row, view) {
            continue;
        }
        draw_ui_text_ex(
            &format!("Y{:03}M{:02}", beat.year, beat.month),
            row.x,
            row.y + 12.0,
            TextStyle::new(12.0, term::faint()).params(),
        );
        draw_ui_text_ex(
            beat.kind.tag(),
            row.x + 64.0,
            row.y + 12.0,
            TextStyle::new(12.0, term::accent()).params(),
        );
        // The beat itself, clipped to the row so a long council title cannot
        // walk over the entry beneath it.
        draw_text_block(
            &beat.text,
            row.x,
            row.y + 18.0,
            row.w,
            STRIDE - 20.0,
            13.0,
            2.0,
            term::primary(),
        );
    }

    scroll.draw_scrollbar_with(
        view,
        content_h,
        term::surface_inset(),
        term::dim(),
        term::primary(),
    );
    ctx.debrief_log_scroll.set(scroll);
}

/// "1 captain" / "3 captains" — the count reads as a sentence in the header.
fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_header_counts_in_words_that_agree() {
        assert_eq!(plural(1, "captain", "captains"), "1 captain");
        assert_eq!(plural(0, "captain", "captains"), "0 captains");
        assert_eq!(plural(4, "captain", "captains"), "4 captains");
    }
}
