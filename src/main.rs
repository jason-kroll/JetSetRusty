#![allow(non_snake_case, non_upper_case_globals)] // C-style names are kept deliberately to match the C original

mod audio;
mod cheat;
mod codes;
mod common;
mod die;
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

fn main() {
    game_main::run();
}
