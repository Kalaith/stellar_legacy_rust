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
        draw_rectangle(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT, term::BG);

        let mut budget = (self.elapsed * CPS) as usize;
        let blink = (self.elapsed * 3.0).fract() < 0.5;
        let x = LOGICAL_WIDTH / 2.0 - 320.0;
        let mut y = 150.0;
        let mut cursor_drawn = false;

        for (i, line) in LINES.iter().enumerate() {
            let n = line.chars().count();
            let show = budget.min(n);
            let text: String = line.chars().take(show).collect();
            let color = if i == 0 || i == LINES.len() - 1 {
                term::AMBER
            } else {
                term::GREEN
            };
            let dims = draw_ui_text_ex(&text, x, y, TextStyle::new(16.0, color).params());
            if !cursor_drawn && show < n {
                if blink {
                    draw_ui_text_ex(
                        "_",
                        x + dims.width,
                        y,
                        TextStyle::new(16.0, term::AMBER).params(),
                    );
                }
                cursor_drawn = true;
            }
            budget -= show;
            y += LINE_H;
        }

        // Fully streamed: park a blinking cursor on the prompt line.
        if !cursor_drawn && blink {
            let last = LINES[LINES.len() - 1];
            let w = measure_text_size(last, TextStyle::new(16.0, term::AMBER)).width;
            let prompt_y = 150.0 + (LINES.len() - 1) as f32 * LINE_H;
            draw_ui_text_ex(
                "_",
                x + w + 4.0,
                prompt_y,
                TextStyle::new(16.0, term::AMBER).params(),
            );
        }
    }
}
