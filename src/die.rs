#![allow(non_snake_case, dead_code, non_upper_case_globals)]

use crate::audio::{Audio_ReduceMusicSpeed, Audio_Sfx, SFX_DIE};
use crate::common::WIDTH;
use crate::game::{GAME_STATE, Game_Action, LIVES};
use crate::video::{Video_DrawSprite, Video_PixelInkFill, Video_PixelPaperFill};
use std::sync::atomic::Ordering;

unsafe extern "C" {
    static mut audioPanX: i32;
    static mut Action: Option<unsafe extern "C" fn()>;
    static mut Ticker: Option<unsafe extern "C" fn()>;
    static mut Drawer: Option<unsafe extern "C" fn()>;
    static mut Responder: Option<unsafe extern "C" fn()>;
    fn Gameover_Action();
    fn DoNothing();
    fn Miner_Restore();
    fn System_Border(colour: i32);
}

static dieBlank: [u16; 16] = [0; 16];
static mut dieLevel: i32 = 0;

#[unsafe(no_mangle)]
extern "C" fn Die_Drawer() {
    Video_PixelInkFill(0, 128 * WIDTH, unsafe { dieLevel >> 1 } as u8);
}

#[unsafe(no_mangle)]
extern "C" fn Die_Ticker() {
    unsafe {
        dieLevel -= 1;
        if dieLevel > 0 {
            return;
        }

        if GAME_STATE.lives.load(Ordering::Relaxed) < 0 {
            Action = Some(Gameover_Action);
            return;
        }

        Video_DrawSprite(
            LIVES as i32 + GAME_STATE.lives.load(Ordering::Relaxed) * 16,
            dieBlank.as_ptr(),
            0x0,
            0x0,
        );
        Miner_Restore();
        Audio_ReduceMusicSpeed();
        Action = Some(Game_Action);
    }
}

#[unsafe(no_mangle)]
extern "C" fn Die_Init() {
    unsafe {
        GAME_STATE.lives.fetch_sub(1, Ordering::Relaxed);
        dieLevel = 15;
        System_Border(0x0);
        Video_PixelPaperFill(0, 128 * WIDTH, 0x0);
        let miner = GAME_STATE.miner.lock().unwrap();
        audioPanX = miner.x;
        drop(miner);
        Audio_Sfx(SFX_DIE as i32);
        Ticker = Some(Die_Ticker);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Die_Action() {
    unsafe {
        Responder = Some(DoNothing);
        Ticker = Some(Die_Init);
        Drawer = Some(Die_Drawer);
        Action = Some(DoNothing);
    }
}
