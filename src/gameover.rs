use crate::audio::{Audio_Play, Audio_Sfx, MUS_STOP, SFX_GAMEOVER};
use crate::common::WIDTH;
use crate::game_main::{
    DoNothing, System_Border, set_action, set_drawer, set_responder, set_ticker,
};
use crate::video::video_pixel_fill;
use std::cell::{Cell, RefCell};

// External functions
use crate::title::Title_Action;
use crate::video::{Video_DrawSprite, Video_PixelPaperFill, Video_WriteLarge};

static PLINTH_SPRITE: [u16; 16] = [
    14316, 30702, 0, 28662, 61431, 61431, 54619, 56251, 54619, 57339, 60791, 61175, 28022, 0,
    30702, 14316,
];

static BOOT_SPRITE: [u16; 16] = [
    4224, 4224, 4224, 4224, 4224, 4224, 4224, 8320, 8320, 18498, 34869, 33801, 32769, 32770, 17293,
    15478,
];

static MINER_SPRITE: [u16; 16] = [
    960, 960, 2016, 832, 992, 960, 384, 960, 2016, 2016, 3952, 4016, 960, 1888, 1760, 1904,
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

fn gameover_drawer() {
    let ticks = boot_ticks();
    if ticks <= 96 {
        Video_DrawSprite((ticks & 126) * WIDTH + 15 * 8, &BOOT_SPRITE, 0x0, 0x7);
        Video_PixelPaperFill(0, 128 * WIDTH, ((ticks & 12) >> 2) as u8);
    }

    if ticks < 96 {
        return;
    }

    TEXT_GAME.with(|t| Video_WriteLarge(7 * 8, 6 * 8, &t.borrow()[..]));
    TEXT_OVER.with(|t| Video_WriteLarge(18 * 8, 6 * 8, &t.borrow()[..]));
}

fn gameover_ticker() {
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

    set_action(Some(Title_Action));
}

fn gameover_init() {
    System_Border(0x0);
    video_pixel_fill(0, 128 * WIDTH);
    Video_DrawSprite(96 * WIDTH + 15 * 8, &MINER_SPRITE, 0x0, 0x7);
    Video_DrawSprite(112 * WIDTH + 15 * 8, &PLINTH_SPRITE, 0x0, 0x2);
    set_boot_ticks(0);

    Audio_Play(MUS_STOP);
    Audio_Sfx(SFX_GAMEOVER as i32);

    set_ticker(Some(gameover_ticker));
}

pub fn Gameover_Action() {
    set_responder(Some(DoNothing));
    set_ticker(Some(gameover_init));
    set_drawer(Some(gameover_drawer));
    set_action(Some(DoNothing));
}
