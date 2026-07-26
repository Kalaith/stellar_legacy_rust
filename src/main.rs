//! Stellar Legacy — generational starship strategy (see gdd.md).

use macroquad::prelude::*;
use macroquad_toolkit::capture;

mod achievements;
mod boot;
mod chronicle;
mod data;
mod game;
mod heritage;
mod save;
mod settings;
mod simulation;
mod state;
mod ui;

use game::Game;
use macroquad_toolkit::ui::{
    begin_target_audit, begin_target_frame, end_target_audit, neighbours_warm, overlapping_targets,
    smallest_touchable_width, undersized_targets,
};

/// Print what the last captured scene's controls would demand of a touchscreen.
///
/// Three separate faults, in the order they matter. Two controls whose grown hit
/// areas overlap are ambiguous, and a press landing on the wrong control is
/// worse than one landing on nothing. A control too small to *press* is one the
/// standard says a finger cannot reliably reach. A control too small to *see* is
/// one nobody presses on purpose — reachable, but not findable.
fn report_touch_targets(scene: &str) {
    if !neighbours_warm() {
        return;
    }
    let clashes = overlapping_targets();
    for (a, b, area) in &clashes {
        println!("touch[{scene}] AMBIGUOUS: \"{a}\" and \"{b}\" share {area:.0}px²");
    }
    if let Some((width, label)) = smallest_touchable_width(ui::LOGICAL_WIDTH) {
        println!(
            "touch[{scene}] every control clears 44px at {width:.0}px wide (worst: \"{label}\")"
        );
    }
    let under = undersized_targets();
    for (drawn, label) in under.iter().take(8) {
        println!("touch[{scene}] drawn {drawn:.0}px: \"{label}\"");
    }
    if under.len() > 8 {
        println!(
            "touch[{scene}] ...and {} more drawn under 44px",
            under.len() - 8
        );
    }
}

fn window_conf() -> Conf {
    capture::capture_window_conf(
        "STELLAR_LEGACY",
        "Stellar Legacy",
        ui::LOGICAL_WIDTH as i32,
        ui::LOGICAL_HEIGHT as i32,
    )
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;

    // Screenshot harness: STELLAR_LEGACY_CAPTURE_PATH renders a named scene
    // ("menu", "gameplay", "event") headlessly and exits.
    if let Some(config) = capture::CaptureConfig::from_env("STELLAR_LEGACY") {
        game.begin_capture_scene(&config.scene);
        // Measure this scene's controls while it draws. The capture harness is
        // the only place the whole game is walked screen by screen, so it is
        // where a target too small to press should be noticed — a photograph
        // shows what a control looks like, not whether a finger can hit it.
        begin_target_audit();
        // Reported from inside the last frame, because `run_capture` exits the
        // process the moment it has its PNG — anything after the call is dead.
        let mut frame_no = 0;
        capture::run_capture(&config, |dt| {
            frame_no += 1;
            begin_target_frame();
            game.update(dt);
            game.draw();
            if frame_no >= config.frames {
                end_target_audit();
                report_touch_targets(&config.scene);
            }
        })
        .await;
        return;
    }

    loop {
        let dt = get_frame_time().min(0.1);
        game.update(dt);
        game.draw();
        next_frame().await;
    }
}
