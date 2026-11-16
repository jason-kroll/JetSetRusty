use crate::audio::{MUS_PLAY, audio_music, audio_music_playing};
use crate::common::{HEIGHT, Key, WIDTH};
use crate::game::{GAME_STATE, Game_Action, Game_DrawStatus, Game_GameReset};
use crate::game_main::{
    game_input, set_action, set_drawer, set_responder, set_ticker, video_flash,
};
use crate::levels::{Level_ItemCount, Level_RestoreItems};
use crate::miner::Miner_Init;
use crate::robots::Robots_DrawCheat;
use crate::video::{tile_2_pixel, video_pixel_fill};
use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;

use crate::game::GameMode;

use crate::video::{Video_CycleColours, Video_Write, Video_WriteLarge};

use crate::game_main::{DoNothing, DoQuit, System_Border};

const MUS_TITLE: i32 = 0;
const THEBATHROOM: i32 = 33;

static TITLE_JSW: [i32; 100] = [
    100, 101, 102, 104, 105, 106, 108, 109, 110, 113, 114, 115, 117, 118, 119, 121, 122, 123, 133,
    136, 141, 145, 149, 154, 165, 168, 169, 170, 173, 177, 178, 179, 181, 182, 183, 186, 197, 200,
    205, 211, 213, 218, 228, 229, 232, 233, 234, 237, 241, 242, 243, 245, 246, 247, 250, 326, 330,
    332, 334, 338, 341, 345, 358, 362, 364, 366, 370, 373, 377, 390, 392, 394, 396, 398, 402, 405,
    406, 407, 408, 409, 422, 424, 426, 428, 430, 434, 439, 454, 455, 456, 457, 458, 460, 462, 463,
    464, 466, 467, 468, 471,
];

thread_local! {
    static TEXT_JSW: RefCell<[u8; 6]> =
        const { RefCell::new([b'\x01', b'\x02', b'\x02', b'\x0b', b'\x14', 0]) };
    static TEXT_POS: Cell<i32> = const { Cell::new(0) };
    static TEXT_FRAME: Cell<i32> = const { Cell::new(0) };
    static COLOUR_CYCLE: Cell<u8> = const { Cell::new(0) };
}

fn text_pos() -> i32 {
    TEXT_POS.with(Cell::get)
}
fn set_text_pos(v: i32) {
    TEXT_POS.with(|c| c.set(v));
}
fn text_frame() -> i32 {
    TEXT_FRAME.with(Cell::get)
}
fn set_text_frame(v: i32) {
    TEXT_FRAME.with(|c| c.set(v));
}
fn colour_cycle() -> u8 {
    COLOUR_CYCLE.with(Cell::get)
}
fn set_colour_cycle(v: u8) {
    COLOUR_CYCLE.with(|c| c.set(v));
}
fn set_text_jsw_byte(i: usize, v: u8) {
    TEXT_JSW.with(|t| t.borrow_mut()[i] = v);
}

static TEXT_TICKER: &[u8] = b"      Press ENTER to Start                                JET-SET WILLY by Matthew Smith   1984 SOFTWARE PROJECTS Ltd                                Guide Willy to collect all the items around the house before Midnight so Maria will let you get to your bed                                Press ENTER to Start      \0";

static TEXT_END: i32 = (TEXT_TICKER.len() - 33) as i32;

static COLOUR_CYCLE_ADJ: [u8; 6] = [1, 2, 3, 4, 5, 1];

fn game_start() {
    video_pixel_fill(128 * WIDTH, 64 * WIDTH);

    Game_GameReset();
    Game_DrawStatus();

    GAME_STATE.level.store(THEBATHROOM, Ordering::Relaxed);
    let debug_items = crate::game_main::DEBUG_ITEM_COUNT.load(Ordering::Relaxed);
    if debug_items >= 0 {
        GAME_STATE.item_count.store(debug_items, Ordering::Relaxed);
    } else {
        GAME_STATE
            .item_count
            .store(Level_ItemCount(), Ordering::Relaxed);
    }
    Level_RestoreItems();

    Miner_Init();

    if GAME_STATE.cheat_enabled.load(Ordering::Relaxed) {
        Robots_DrawCheat();
    }

    GAME_STATE
        .mode
        .store(GameMode::Normal as u8, Ordering::Relaxed);
    GAME_STATE.game_paused.store(0, Ordering::Relaxed);

    Game_Action();
}

fn do_title_ticker() {
    if audio_music_playing() != 0 {
        if video_flash() != 0 {
            set_text_jsw_byte(1, b'\x0b');
            set_text_jsw_byte(3, b'\x02');
        } else {
            set_text_jsw_byte(1, b'\x02');
            set_text_jsw_byte(3, b'\x0b');
        }
        return;
    }

    set_colour_cycle(COLOUR_CYCLE_ADJ[colour_cycle() as usize]);

    if text_pos() < TEXT_END {
        if text_frame() < 6 {
            set_text_frame(text_frame() + 2);
            return;
        }
        set_text_pos(text_pos() + 1);
        set_text_frame(0);
        return;
    }

    set_action(Some(Title_Action));
}

fn do_title_drawer() {
    if audio_music_playing() != 0 {
        for &tile in TITLE_JSW.iter().take(100) {
            TEXT_JSW.with(|t| {
                Video_Write(tile_2_pixel(tile), &t.borrow()[..]);
            });
        }
        return;
    }

    if colour_cycle() == 1 {
        System_Border(Video_CycleColours());
    }

    Video_WriteLarge(0, 0, c"\x01\x01\x02\x07".to_bytes_with_nul());
    Video_WriteLarge(
        -(text_frame() & 6),
        19 * 8,
        &TEXT_TICKER[text_pos() as usize..],
    );
}

fn do_title_responder() {
    if game_input() == Key::Enter as i32 {
        set_action(Some(game_start));
    } else if game_input() == Key::Escape as i32 {
        DoQuit();
    }
}

// b"" byte-string literals carry the interior nul bytes these control strings need
fn do_title_init() {
    System_Border(0x0);
    video_pixel_fill(0, WIDTH * HEIGHT);

    Video_Write(16 * WIDTH + 144, b"\x01\x00\x02\x05\x10\x11\x12\x13\0");
    Video_Write(
        24 * WIDTH + 128,
        c"\x10\x14\x01\x05\x14\x14\x02\x09\x10\x14".to_bytes_with_nul(),
    );
    Video_Write(
        32 * WIDTH + 112,
        b"\x01\x00\x02\x05\x10\x11\x01\x05\x14\x14\x02\x09\x10\x11\x01\x09\x14\x14\0",
    );
    Video_Write(
        40 * WIDTH + 96,
        b"\x01\x00\x02\x05\x10\x14\x01\x05\x14\x14\x02\x09\x10\x14\x01\x09\x14\x14\x14\x14\0",
    );
    Video_Write(
            48 * WIDTH + 80,
            b"\x01\x00\x02\x05\x10\x11\x01\x05\x14\x14\x02\x09\x10\x11\x01\x09\x14\x14\x02\x01\x10\x14\x14\x14\0",
        );
    Video_Write(
            56 * WIDTH + 64,
            b"\x01\x00\x02\x05\x14\x14\x01\x05\x14\x14\x02\x09\x10\x14\x01\x09\x14\x14\x02\x00\x10\x14\x01\x01\x14\x14\x01\x09\x14\x14\0",
        );
    Video_Write(
            64 * WIDTH + 64,
            b"\x01\x05\x02\x01\x12\x13\x14\x14\x01\x09\x02\x05\x12\x13\x02\x00\x10\x11\x01\x00\x14\x14\x01\x01\x14\x14\x01\x09\x14\x14\0",
        );
    Video_Write(
            72 * WIDTH + 64,
            b"\x01\x01\x14\x14\x01\x05\x02\x01\x12\x13\x14\x14\x01\x00\x02\x05\x12\x13\x14\x14\x01\x01\x14\x14\x01\x09\x14\x14\0",
        );
    Video_Write(
            80 * WIDTH + 64,
            b"\x01\x01\x02\x00\x12\x13\x14\x14\x01\x05\x02\x01\x14\x13\x14\x14\x01\x00\x02\x05\x12\x13\x01\x01\x14\x14\x01\x09\x14\x14\0",
        );
    Video_Write(
            88 * WIDTH + 80,
            b"\x01\x01\x02\x00\x14\x13\x14\x14\x01\x05\x02\x01\x14\x13\x14\x14\x01\x01\x14\x14\x01\x09\x14\x14\0",
        );
    Video_Write(
            96 * WIDTH + 96,
            b"\x01\x01\x02\x00\x14\x13\x14\x14\x01\x05\x02\x01\x12\x13\x01\x01\x14\x14\x01\x09\x14\x14\0",
        );
    Video_Write(
        104 * WIDTH + 112,
        b"\x01\x01\x02\x00\x14\x13\x14\x14\x14\x14\x01\x09\x14\x14\0",
    );
    Video_Write(
        112 * WIDTH + 128,
        c"\x01\x01\x14\x13\x14\x14\x01\x09\x14\x14".to_bytes_with_nul(),
    );
    Video_Write(
        120 * WIDTH + 144,
        c"\x01\x01\x12\x13\x01\x09\x10\x11".to_bytes_with_nul(),
    );

    Video_WriteLarge(0, 0, b"\x01\x00\x02\x04\0");
    Video_WriteLarge(0, 19 * 8, TEXT_TICKER);

    set_text_pos(0);
    set_text_frame(-1);
    set_colour_cycle(1);

    audio_music(MUS_TITLE, MUS_PLAY);

    set_ticker(Some(do_title_ticker));
}

pub fn Title_Action() {
    set_responder(Some(do_title_responder));
    set_ticker(Some(do_title_init));
    set_drawer(Some(do_title_drawer));
    set_action(Some(DoNothing));
}
