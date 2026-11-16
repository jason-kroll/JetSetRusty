use crate::audio::{Audio_Music, MUS_PLAY, audioMusicPlaying};
use crate::common::{HEIGHT, WIDTH};
use crate::game_main::{set_action, set_drawer, set_responder, set_ticker, video_flash};
use crate::video::{Video_PixelPaperFill, Video_Write, Video_WriteLarge, video_text_width};

unsafe extern "C" {
    fn DoNothing();
    fn System_Border(x: i32);
    fn Codes_Action();
    fn Title_Action();
}

// MUS_TITLE=0, MUS_GAME=1, MUS_LOADER=2 (from audio.h enum order)
const MUS_LOADER: i32 = 2;

static mut LOADER_TICKS: u32 = 0;
// \x01\x07 = set paper 7, \x02\x02 = set ink 2 (colors flip with videoFlash)
static mut LOADER_TEXT: [u8; 25] = *b"\x01\x07\x02\x02JetSet Willy Loading\0";

extern "C" fn do_loader_responder() {
    set_action(Some(Title_Action))
}

extern "C" fn do_loader_ticker() {
    unsafe {
        // Use raw pointers to avoid static_mut_refs lint (Rust 2024)
        let text = (&raw mut LOADER_TEXT).cast::<u8>();
        if video_flash() != 0 {
            text.add(1).write(7);
            text.add(3).write(2);
        } else {
            text.add(1).write(2);
            text.add(3).write(7);
        }
        // Matches C's post-increment: transition when value was 256
        if LOADER_TICKS == 256 {
            set_action(Some(Codes_Action));
        }
        LOADER_TICKS += 1;
    }
}

extern "C" fn do_loader_drawer3() {
    Video_WriteLarge(6 * 8, 11 * 8, (&raw const LOADER_TEXT).cast::<i8>());
}

extern "C" fn do_loader_drawer2() {
    unsafe {
        if audioMusicPlaying != 0 {
            return;
        }

        // First row of loading bar: set paper color then draw 22 block chars
        Video_Write(
            80 * WIDTH + 5 * 8,
            c"\x01\x06\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14".as_ptr(),
        );
        // Three more identical rows
        for row in [88i32, 96, 104] {
            Video_Write(
                row * WIDTH + 5 * 8,
                c"\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14\x14".as_ptr(),
            );
        }

        do_loader_drawer3();

        set_responder(Some(do_loader_responder));
        set_ticker(Some(do_loader_ticker));
        set_drawer(Some(do_loader_drawer3));
    }
}

extern "C" fn do_loader_drawer1() {
    unsafe {
        System_Border(0x1);
        Video_PixelPaperFill(0, WIDTH * HEIGHT, 0x1);

        // "fawtytoo" (paper=1, ink=7)
        Video_Write(23 * 8 * WIDTH, c"\x01\x01\x02\x07fawtytoo".as_ptr());

        // BUILD string right-aligned on the same row
        // \x02\x00 = set ink to 0 (black); the embedded null is the ink arg, not a terminator
        let version = crate::game_main::build_version();
        let build = format!("{version}\0").into_bytes();
        let build_with_ink = format!("\x02\x00{version}\0").into_bytes();
        Video_Write(
            23 * 8 * WIDTH + WIDTH - video_text_width(build.as_ptr() as *const i8),
            build_with_ink.as_ptr() as *const i8,
        );

        Audio_Music(MUS_LOADER, MUS_PLAY);
        set_drawer(Some(do_loader_drawer2));
    }
}

// game_main.c initializes `Action = Loader_Action` by symbol name, so no_mangle is required.
#[unsafe(no_mangle)]
pub extern "C" fn Loader_Action() {
    set_drawer(Some(do_loader_drawer1));
    set_action(Some(DoNothing));
}
