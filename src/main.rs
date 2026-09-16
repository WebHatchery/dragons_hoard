//! Dragon's Hoard — a play-money dragon-themed slot built on macroquad-toolkit.

use macroquad::prelude::*;
use macroquad_toolkit::capture;

use dragons_hoard::game::Game;
use dragons_hoard::ui;

fn window_conf() -> Conf {
    capture::capture_window_conf(
        "DRAGONS_HOARD",
        "Dragon's Hoard",
        ui::frame::DESIGN_WIDTH as i32,
        ui::LOGICAL_HEIGHT as i32,
    )
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;

    // Random-play harness (§5.76): ten thousand arbitrary presses, checked
    // after every one. Before the capture branch because it is not a capture —
    // it draws nothing and exits with a code.
    if let Some(config) = dragons_hoard::game::drift::DriftConfig::from_env() {
        std::process::exit(game.drift(&config));
    }

    // Screenshot harness: when DRAGONS_HOARD_CAPTURE_PATH is set, seed the
    // requested scene, render deterministic frames, write a PNG, and exit.
    if let Some(configs) = capture::CaptureConfig::all_from_env("DRAGONS_HOARD") {
        for config in configs {
            game.begin_capture_scene(&config.scene);
            // A strip rather than a photograph when asked for one: a settled frame
            // cannot show a fault that only exists while something is moving
            // (§5.52), and the motion audit wants every frame rather than every
            // fourth one, so it rides along with the tiling.
            if let Some(strip) = capture::filmstrip::StripConfig::from_env("DRAGONS_HOARD") {
                capture::filmstrip::run_filmstrip(&config, &strip, |dt| {
                    game.update(dt);
                    game.draw();
                    game.observe_motion();
                })
                .await;
                game.finish_motion_audit();
                continue;
            }
            capture::run_capture_once(&config, |dt| {
                game.update(dt);
                game.draw();
            })
            .await;
        }
        return;
    }

    loop {
        let dt = get_frame_time().min(0.1);
        game.update(dt);
        game.draw();
        next_frame().await;
    }
}
