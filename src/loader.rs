use crate::audio::{MUS_PLAY, audio_music, audio_music_playing};
use crate::common::{HEIGHT, WIDTH};
use crate::game_main::{
    System_Border, set_action, set_drawer, set_responder, set_ticker, video_flash,
};
use crate::video::{Video_PixelPaperFill, Video_Write, Video_WriteLarge, video_text_width};

use crate::codes::Codes_Action;
use crate::game_main::DoNothing;
use crate::title::Title_Action;
use std::cell::{Cell, RefCell};

// MUS_TITLE=0, MUS_GAME=1, MUS_LOADER=2 (from audio.h enum order)
const MUS_LOADER: i32 = 2;

thread_local! {
    static LOADER_TICKS: Cell<u32> = const { Cell::new(0) };
    // \x01\x07 = set paper 7, \x02\x02 = set ink 2 (colors flip with videoFlash)
    static LOADER_TEXT: RefCell<[u8; 25]> =
        const { RefCell::new(*b"\x01\x07\x02\x02JetSet Willy Loading\0") };
}

fn do_loader_responder() {
    set_action(Some(Title_Action))
}

fn do_loader_ticker() {
    LOADER_TEXT.with(|t| {
        let mut text = t.borrow_mut();
        if video_flash() != 0 {
            text[1] = 7;
            text[3] = 2;
        } else {
            text[1] = 2;
            text[3] = 7;
        }
    });
    // Matches C's post-increment: transition when value was 256
    let ticks = LOADER_TICKS.with(Cell::get);
    if ticks == 256 {
        set_action(Some(Codes_Action));
    }
    LOADER_TICKS.with(|c| c.set(ticks + 1));
}

fn do_loader_drawer3() {
    LOADER_TEXT.with(|t| Video_WriteLarge(6 * 8, 11 * 8, &t.borrow()[..]));
}

fn do_loader_drawer2() {
    if audio_music_playing() != 0 {
        return;
    }

    // First row of loading bar: set paper color then draw 22 block chars
    Video_Write(
            80 * WIDTH + 5 * 8,
            c"\x01\x06\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14".to_bytes_with_nul(),
        );
    // Three more identical rows
    for row in [88i32, 96, 104] {
        Video_Write(
                row * WIDTH + 5 * 8,
                c"\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14".to_bytes_with_nul(),
            );
    }

    do_loader_drawer3();

    set_responder(Some(do_loader_responder));
    set_ticker(Some(do_loader_ticker));
    set_drawer(Some(do_loader_drawer3));
}

fn do_loader_drawer1() {
    System_Border(0x1);
    Video_PixelPaperFill(0, WIDTH * HEIGHT, 0x1);

    // "fawtytoo" (paper=1, ink=7)
    Video_Write(
        23 * 8 * WIDTH,
        c"\x01\x01\x02\x07fawtytoo".to_bytes_with_nul(),
    );

    // BUILD string right-aligned on the same row
    // \x02\x00 = set ink to 0 (black); the embedded null is the ink arg, not a terminator
    let version = crate::game_main::build_version();
    let build = format!("{version}\0").into_bytes();
    let build_with_ink = format!("\x02\x00{version}\0").into_bytes();
    Video_Write(
        23 * 8 * WIDTH + WIDTH - video_text_width(&build[..]),
        &build_with_ink[..],
    );

    audio_music(MUS_LOADER, MUS_PLAY);
    set_drawer(Some(do_loader_drawer2));
}

pub fn Loader_Action() {
    set_drawer(Some(do_loader_drawer1));
    set_action(Some(DoNothing));
}
