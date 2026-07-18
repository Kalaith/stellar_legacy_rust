//! Stellar Legacy — generational starship strategy (see gdd.md).

use macroquad::prelude::*;
use macroquad_toolkit::capture;

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
        capture::run_capture(&config, |dt| {
            game.update(dt);
            game.draw();
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
