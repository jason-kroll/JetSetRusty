mod audio;
mod cheat;
mod codes;
mod common;
mod die;
mod dispatch;
mod game;
mod game_main;
mod gameover;
mod levels;
mod loader;
mod miner;
mod misc;
mod replay;
mod rng;
mod robots;
mod rope;
mod title;
mod video;

use std::process::ExitCode;

fn main() -> ExitCode {
    game_main::run()
}
