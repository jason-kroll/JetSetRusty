use crate::audio::{Sfx, audio_play, audio_sfx};
use crate::common::WIDTH;
use crate::game_main::{
    Action, Drawer, Responder, Ticker, set_action, set_drawer, set_responder, set_ticker,
    system_border,
};
use crate::miner::miner_draw_sprite;
use crate::video::video_pixel_fill;
use std::cell::{Cell, RefCell};

use crate::video::{video_draw_sprite, video_pixel_paper_fill, video_write_large};

static PLINTH_SPRITE: [u16; 16] = [
    14316, 30702, 0, 28662, 61431, 61431, 54619, 56251, 54619, 57339, 60791, 61175, 28022, 0,
    30702, 14316,
];

static BOOT_SPRITE: [u16; 16] = [
    4224, 4224, 4224, 4224, 4224, 4224, 4224, 8320, 8320, 18498, 34869, 33801, 32769, 32770, 17293,
    15478,
];

thread_local! {
    static BOOT_TICKS: Cell<i32> = const { Cell::new(0) };
    static TEXT_GAME: RefCell<[u8; 18]> = const { RefCell::new([
        0x1, 0x0, 0x2, 0x0, b'G', b' ', 0x2, 0x0, b'a', b' ', 0x2, 0x0,
        b'm', b' ', 0x2, 0x0, b'e', 0,
    ]) };
    static TEXT_OVER: RefCell<[u8; 18]> = const { RefCell::new([
        0x1, 0x0, 0x2, 0x0, b'O', b' ', 0x2, 0x0, b'v', b' ', 0x2, 0x0,
        b'e', b' ', 0x2, 0x0, b'r', 0,
    ]) };
}

fn boot_ticks() -> i32 {
    BOOT_TICKS.with(Cell::get)
}
fn set_boot_ticks(v: i32) {
    BOOT_TICKS.with(|c| c.set(v));
}
fn set_text_game_byte(i: usize, v: u8) {
    TEXT_GAME.with(|t| t.borrow_mut()[i] = v);
}
fn set_text_over_byte(i: usize, v: u8) {
    TEXT_OVER.with(|t| t.borrow_mut()[i] = v);
}

pub(crate) fn gameover_drawer() {
    let ticks = boot_ticks();
    if ticks <= 96 {
        video_draw_sprite((ticks & 126) * WIDTH + 15 * 8, &BOOT_SPRITE, 0x0, 0x7);
        video_pixel_paper_fill(0, 128 * WIDTH, ((ticks & 12) >> 2) as u8);
    }

    if ticks < 96 {
        return;
    }

    TEXT_GAME.with(|t| video_write_large(7 * 8, 6 * 8, &t.borrow()[..]));
    TEXT_OVER.with(|t| video_write_large(18 * 8, 6 * 8, &t.borrow()[..]));
}

pub(crate) fn gameover_ticker() {
    let mut c = boot_ticks() >> 2;

    set_text_game_byte(3, (c & 0x7) as u8);
    c += 1;
    set_text_game_byte(7, (c & 0x7) as u8);
    c += 1;
    set_text_game_byte(11, (c & 0x7) as u8);
    c += 1;
    set_text_game_byte(15, (c & 0x7) as u8);
    c += 1;
    set_text_over_byte(3, (c & 0x7) as u8);
    c += 1;
    set_text_over_byte(7, (c & 0x7) as u8);
    c += 1;
    set_text_over_byte(11, (c & 0x7) as u8);
    c += 1;
    set_text_over_byte(15, (c & 0x7) as u8);

    set_boot_ticks(boot_ticks() + 1);

    if boot_ticks() < 256 {
        return;
    }

    set_action(Action::Title);
}

pub(crate) fn gameover_init() {
    system_border(0x0);
    video_pixel_fill(0, 128 * WIDTH);
    // Upstream 783d7c1: draw Willy via Miner_DrawSprite with index 2.
    miner_draw_sprite(96 * WIDTH + 15 * 8, 0x0, 0x7, 2);
    video_draw_sprite(112 * WIDTH + 15 * 8, &PLINTH_SPRITE, 0x0, 0x2);
    set_boot_ticks(0);

    audio_play(false);
    audio_sfx(Sfx::Gameover);

    set_ticker(Ticker::GameOver);
}

pub fn gameover_action() {
    set_responder(Responder::Noop);
    set_ticker(Ticker::GameOverInit);
    set_drawer(Drawer::GameOver);
    set_action(Action::None);
}
