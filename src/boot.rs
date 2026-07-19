//! Power-on self-test sequence: a terminal boot log streamed once at launch,
//! before the main menu, to sell the old-CRT-monitor feel (GDD §9). Purely
//! cosmetic — it owns a wall-clock timer and never touches the sim.

use crate::ui::{term, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

/// Characters streamed per second — fast, like a real terminal scroll.
const CPS: f32 = 170.0;
/// Seconds to hold the finished screen before auto-advancing to the menu.
const HOLD_AFTER: f32 = 0.8;
const LINE_H: f32 = 26.0;

/// The boot log. The first line is the banner (amber); the last is the
/// blinking prompt; the rest are POST status lines (green phosphor).
const LINES: &[&str] = &[
    "STELLAR LEGACY TERMINAL  //  GEN-VII SHIPBOARD BIOS",
    "COLONY AUTHORITY   (C) CYCLE 2387   ALL RIGHTS RESERVED",
    "",
    "POWER-ON SELF TEST .................................",
    "  CORE MEMORY ............. 640K ......... OK",
    "  REACTOR CORE ............ NOMINAL ...... OK",
    "  LIFE SUPPORT ............ ONLINE ....... OK",
    "  NAV COMPUTER ............ READY ........ OK",
    "  CRYO ARCHIVE ............ 12,000 SOULS",
    "  DYNASTY REGISTRY ........ SEALED",
    "",
    "ALL SYSTEMS NOMINAL.   MOUNTING FOUNDING CHARTER...",
    "",
    ">> PRESS ANY KEY TO BEGIN <<",
];

/// One-shot boot animation. Held on `Game`; shown while it is not [`is_done`].
pub struct BootScreen {
    elapsed: f32,
    done: bool,
    total_chars: usize,
}

impl BootScreen {
    pub fn new() -> Self {
        let total_chars = LINES.iter().map(|l| l.chars().count()).sum();
        Self {
            elapsed: 0.0,
            done: false,
            total_chars,
        }
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Skip straight to the menu (any key, or screenshot capture).
    pub fn finish(&mut self) {
        self.done = true;
    }

    /// Jump to a fixed point in the stream (screenshot capture only).
    pub fn seek(&mut self, secs: f32) {
        self.elapsed = secs;
        self.done = false;
    }

    /// Advance the timer; self-completes once the log has streamed and held.
    pub fn update(&mut self, dt: f32) {
        if self.done {
            return;
        }
        self.elapsed += dt;
        let type_time = self.total_chars as f32 / CPS;
        if self.elapsed >= type_time + HOLD_AFTER {
            self.done = true;
        }
    }

    /// Draw the streaming log centred on a black screen. The CRT overlay is
    /// applied on top by `Game::draw`, so this already reads as a monitor.
    pub fn draw(&self) {
        draw_rectangle(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT, term::bg());

        // The whole log streams as one shared character budget (toolkit
        // `reveal_block`), so lines fill in order like real console output.
        let reveal = reveal_block(LINES, self.elapsed, CPS);
        let x = LOGICAL_WIDTH / 2.0 - 320.0;
        let mut y = 150.0;

        for (i, line) in LINES.iter().enumerate() {
            let text = prefix_chars(line, reveal.shown[i]);
            let color = if i == 0 || i == LINES.len() - 1 {
                term::primary()
            } else {
                term::accent()
            };
            let style = TextStyle::new(16.0, color);
            let dims = if i == 0 {
                // The banner line glows like a warm phosphor header.
                draw_text_glow(text, x, y, style, 0.14, 2.0);
                measure_text_size(text, style)
            } else {
                draw_ui_text_ex(text, x, y, style.params())
            };
            // The write-cursor sits on one line: mid-stream it trails the last
            // typed glyph; once complete it parks at the end of the prompt.
            if i == reveal.cursor_line && blink(self.elapsed, 3.0) {
                let gap = if reveal.complete { 4.0 } else { 0.0 };
                draw_ui_text_ex(
                    "_",
                    x + dims.width + gap,
                    y,
                    TextStyle::new(16.0, term::primary()).params(),
                );
            }
            y += LINE_H;
        }
    }
}
