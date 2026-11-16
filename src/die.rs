#![allow(non_snake_case, dead_code, non_upper_case_globals)]

use crate::audio::{Audio_ReduceMusicSpeed, Audio_Sfx, SFX_DIE};
use crate::common::WIDTH;
use crate::game::{GAME_STATE, Game_Action, LIVES};
use crate::game_main::{set_action, set_drawer, set_responder, set_ticker};
use crate::video::{Video_DrawSprite, Video_PixelInkFill, Video_PixelPaperFill};
use std::sync::atomic::Ordering;

use crate::game_main::{DoNothing, System_Border};
use crate::gameover::Gameover_Action;
use crate::miner::Miner_Restore;

unsafe extern "C" {
    static mut audioPanX: i32;
}

static dieBlank: [u16; 16] = [0; 16];
static mut dieLevel: i32 = 0;

fn Die_Drawer() {
    Video_PixelInkFill(0, 128 * WIDTH, unsafe { dieLevel >> 1 } as u8);
}

fn Die_Ticker() {
    unsafe {
        dieLevel -= 1;
        if dieLevel > 0 {
            return;
        }

        if GAME_STATE.lives.load(Ordering::Relaxed) < 0 {
            set_action(Some(Gameover_Action));
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
        set_action(Some(Game_Action));
    }
}

fn Die_Init() {
    unsafe {
        GAME_STATE.lives.fetch_sub(1, Ordering::Relaxed);
        dieLevel = 15;
        System_Border(0x0);
        Video_PixelPaperFill(0, 128 * WIDTH, 0x0);
        let miner = GAME_STATE.miner.lock().unwrap();
        audioPanX = miner.x;
        drop(miner);
        Audio_Sfx(SFX_DIE as i32);
        set_ticker(Some(Die_Ticker));
    }
}

pub fn Die_Action() {
    set_responder(Some(DoNothing));
    set_ticker(Some(Die_Init));
    set_drawer(Some(Die_Drawer));
    set_action(Some(DoNothing));
}
