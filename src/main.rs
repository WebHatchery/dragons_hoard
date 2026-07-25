//! Dragon's Hoard — a play-money dragon-themed slot built on macroquad-toolkit.

use macroquad::prelude::*;
use macroquad_toolkit::capture;

mod actions;
mod audio;
mod data;
mod engine;
mod game;
mod music;
mod state;
mod ui;

use game::Game;

fn window_conf() -> Conf {
    capture::capture_window_conf(
        "DRAGONS_HOARD",
        "Dragon's Hoard",
        ui::LOGICAL_WIDTH as i32,
        ui::LOGICAL_HEIGHT as i32,
    )
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;

    // Screenshot harness: when DRAGONS_HOARD_CAPTURE_PATH is set, seed the
    // requested scene, render deterministic frames, write a PNG, and exit.
    if let Some(config) = capture::CaptureConfig::from_env("DRAGONS_HOARD") {
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
