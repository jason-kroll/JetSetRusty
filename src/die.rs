use crate::audio::{Sfx, audio_reduce_music_speed, audio_sfx, set_audio_pan_x};
use crate::common::WIDTH;
use crate::game::{GAME_STATE, LIVES, game_action};
use crate::game_main::{set_action, set_drawer, set_responder, set_ticker};
use crate::video::{video_draw_sprite, video_pixel_ink_fill, video_pixel_paper_fill};
use std::cell::Cell;
use std::sync::atomic::Ordering;

use crate::game_main::{do_nothing, system_border};
use crate::gameover::gameover_action;
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

fn die_drawer() {
    video_pixel_ink_fill(0, 128 * WIDTH, (die_level() >> 1) as u8);
}

fn die_ticker() {
    // Upstream 7c1aed3: check before decrementing so dieLevel never falls
    // below zero (the old post-decrement left it at -1 on the final frame).
    if die_level() > 0 {
        set_die_level(die_level() - 1);
        return;
    }

    if GAME_STATE.lives.load(Ordering::Relaxed) < 0 {
        set_action(Some(gameover_action));
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
    set_action(Some(game_action));
}

fn die_init() {
    GAME_STATE.lives.fetch_sub(1, Ordering::Relaxed);
    set_die_level(15);
    system_border(0x0);
    video_pixel_paper_fill(0, 128 * WIDTH, 0x0);
    let miner = GAME_STATE.miner.lock().unwrap();
    set_audio_pan_x(miner.x);
    drop(miner);
    audio_sfx(Sfx::Die);
    set_ticker(Some(die_ticker));
}

pub fn die_action() {
    set_responder(Some(do_nothing));
    set_ticker(Some(die_init));
    set_drawer(Some(die_drawer));
    set_action(Some(do_nothing));
}
