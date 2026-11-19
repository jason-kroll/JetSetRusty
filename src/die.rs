use crate::audio::{Sfx, audio_reduce_music_speed, audio_sfx, set_audio_pan_x};
use crate::common::WIDTH;
use crate::dispatch::{
    DiePhase, GameOverPhase, GamePhase, Pending, Screen, queue_pending, set_screen,
};
use crate::game::{GAME_STATE, LIVES};
use crate::game_main::system_border;
use crate::video::{video_draw_sprite, video_pixel_ink_fill, video_pixel_paper_fill};
use std::cell::Cell;
use std::sync::atomic::Ordering;

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

pub(crate) fn die_drawer() {
    video_pixel_ink_fill(
        0,
        128 * WIDTH,
        u8::try_from(die_level() >> 1).expect("die level 0..=15"),
    );
}

pub(crate) fn die_ticker() {
    // Upstream 7c1aed3: check before decrementing so dieLevel never falls
    // below zero (the old post-decrement left it at -1 on the final frame).
    if die_level() > 0 {
        set_die_level(die_level() - 1);
        return;
    }

    if GAME_STATE.lives.load(Ordering::Relaxed) < 0 {
        queue_pending(Pending::Enter(Screen::GameOver(GameOverPhase::Init)));
        return;
    }

    video_draw_sprite(
        i32::try_from(LIVES).expect("LIVES fits i32")
            + GAME_STATE.lives.load(Ordering::Relaxed) * 16,
        &DIE_BLANK,
        0x0,
        0x0,
    );
    miner_restore(&mut GAME_STATE.miner.lock().unwrap());
    audio_reduce_music_speed();
    queue_pending(Pending::Enter(Screen::Game(GamePhase::Init)));
}

pub(crate) fn die_init() {
    GAME_STATE.lives.fetch_sub(1, Ordering::Relaxed);
    set_die_level(15);
    system_border(0x0);
    video_pixel_paper_fill(0, 128 * WIDTH, 0x0);
    let miner = GAME_STATE.miner.lock().unwrap();
    set_audio_pan_x(miner.x);
    drop(miner);
    audio_sfx(Sfx::Die);
    set_screen(Screen::Die(DiePhase::Anim));
}
