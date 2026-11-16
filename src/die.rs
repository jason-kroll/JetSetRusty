#![allow(non_snake_case)]

use crate::audio::{SFX_DIE, audio_reduce_music_speed, audio_sfx, set_audio_pan_x};
use crate::common::WIDTH;
use crate::game::{GAME_STATE, Game_Action, LIVES};
use crate::game_main::{set_action, set_drawer, set_responder, set_ticker};
use crate::video::{video_draw_sprite, video_pixel_ink_fill, video_pixel_paper_fill};
use std::cell::Cell;
use std::sync::atomic::Ordering;

use crate::game_main::{DoNothing, System_Border};
use crate::gameover::Gameover_Action;
use crate::miner::miner_restore;

static DIE_BLANK: [u16; 16] = [0; 16];
thread_local! {
    static DIE_LEVEL: Cell<i32> = const { Cell::new(0) };
}

fn die_level() -> i32 {
    DIE_LEVEL.with(Cell::get)
}
fn set_die_level(v: i32) {
    DIE_LEVEL.with(|c| c.set(v));
}

fn Die_Drawer() {
    video_pixel_ink_fill(0, 128 * WIDTH, (die_level() >> 1) as u8);
}

fn Die_Ticker() {
    set_die_level(die_level() - 1);
    if die_level() > 0 {
        return;
    }

    if GAME_STATE.lives.load(Ordering::Relaxed) < 0 {
        set_action(Some(Gameover_Action));
        return;
    }

    video_draw_sprite(
        LIVES as i32 + GAME_STATE.lives.load(Ordering::Relaxed) * 16,
        &DIE_BLANK,
        0x0,
        0x0,
    );
    miner_restore();
    audio_reduce_music_speed();
    set_action(Some(Game_Action));
}

fn Die_Init() {
    GAME_STATE.lives.fetch_sub(1, Ordering::Relaxed);
    set_die_level(15);
    System_Border(0x0);
    video_pixel_paper_fill(0, 128 * WIDTH, 0x0);
    let miner = GAME_STATE.miner.lock().unwrap();
    set_audio_pan_x(miner.x);
    drop(miner);
    audio_sfx(SFX_DIE as i32);
    set_ticker(Some(Die_Ticker));
}

pub fn Die_Action() {
    set_responder(Some(DoNothing));
    set_ticker(Some(Die_Init));
    set_drawer(Some(Die_Drawer));
    set_action(Some(DoNothing));
}
