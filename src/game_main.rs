#![allow(non_snake_case, non_upper_case_globals)]

use crate::common::{Event, HEIGHT, Key, WIDTH};
use crate::misc::{Timer, Timer_Set, Timer_Update, Video_Viewport, videoColour};
use sdl2::EventPump;
use sdl2::audio::{AudioCallback, AudioSpecDesired};
use sdl2::event::Event as SdlEvent;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::Texture;
use std::cell::{Cell, RefCell};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering::Relaxed};

pub const SAMPLERATE: i32 = 22050;
pub const TICKRATE: i32 = 60;

// SDL key codes indexed by Key enum discriminant (Key::Left=0 .. Key::T=35)
static SDL_KEYS: LazyLock<[i32; 36]> = LazyLock::new(|| {
    [
        Keycode::Left.into_i32(),
        Keycode::Right.into_i32(),
        Keycode::Space.into_i32(),
        Keycode::Return.into_i32(),
        Keycode::LShift.into_i32(),
        Keycode::RShift.into_i32(),
        Keycode::NUM_1.into_i32(),
        Keycode::NUM_2.into_i32(),
        Keycode::NUM_3.into_i32(),
        Keycode::NUM_4.into_i32(),
        Keycode::NUM_5.into_i32(),
        Keycode::NUM_6.into_i32(),
        Keycode::NUM_7.into_i32(),
        Keycode::NUM_8.into_i32(),
        Keycode::NUM_9.into_i32(),
        Keycode::NUM_0.into_i32(),
        Keycode::A.into_i32(),
        Keycode::B.into_i32(),
        Keycode::C.into_i32(),
        Keycode::D.into_i32(),
        Keycode::E.into_i32(),
        Keycode::F.into_i32(),
        Keycode::G.into_i32(),
        Keycode::H.into_i32(),
        Keycode::I.into_i32(),
        Keycode::J.into_i32(),
        Keycode::K.into_i32(),
        Keycode::L.into_i32(),
        Keycode::M.into_i32(),
        Keycode::N.into_i32(),
        Keycode::O.into_i32(),
        Keycode::P.into_i32(),
        Keycode::Q.into_i32(),
        Keycode::R.into_i32(),
        Keycode::S.into_i32(),
        Keycode::T.into_i32(),
    ]
});

use crate::audio::{Audio_Init, Audio_Output};
use crate::loader::Loader_Action;

unsafe extern "C" {
    fn rand() -> i32;
    fn srand(seed: u32);
    fn time(t: *mut i64) -> i64;
}

// ---- Dispatcher state -------------------------------------------------------
// The four Event slots plus gameInput/videoFlash. All are main-thread only
// (the SDL audio callback never touches them), so thread-local Cells are
// sound and need no unsafe. This replaces the old #[no_mangle] static mut
// globals from the C port.
thread_local! {
    static ACTION: Cell<Event> = const { Cell::new(Some(Loader_Action)) };
    static RESPONDER: Cell<Event> = const { Cell::new(None) };
    static TICKER: Cell<Event> = const { Cell::new(None) };
    static DRAWER: Cell<Event> = const { Cell::new(None) };
    static GAME_INPUT: Cell<i32> = const { Cell::new(Key::None as i32) };
    static VIDEO_FLASH: Cell<i32> = const { Cell::new(0) };
}

pub(crate) fn set_action(f: Event) {
    ACTION.with(|c| c.set(f));
}
pub(crate) fn action() -> Event {
    ACTION.with(Cell::get)
}
pub(crate) fn set_responder(f: Event) {
    RESPONDER.with(|c| c.set(f));
}
pub(crate) fn responder() -> Event {
    RESPONDER.with(Cell::get)
}
pub(crate) fn set_ticker(f: Event) {
    TICKER.with(|c| c.set(f));
}
pub(crate) fn ticker() -> Event {
    TICKER.with(Cell::get)
}
pub(crate) fn set_drawer(f: Event) {
    DRAWER.with(|c| c.set(f));
}
pub(crate) fn drawer() -> Event {
    DRAWER.with(Cell::get)
}
pub(crate) fn set_game_input(key: i32) {
    GAME_INPUT.with(|c| c.set(key));
}
pub(crate) fn game_input() -> i32 {
    GAME_INPUT.with(Cell::get)
}
pub(crate) fn video_flash() -> i32 {
    VIDEO_FLASH.with(Cell::get)
}
pub(crate) fn toggle_video_flash(bit: i32) {
    VIDEO_FLASH.with(|c| c.set(c.get() ^ bit));
}

// ---- Module-private state ---------------------------------------------------

// The event pump is owned by run() but stored here so System_IsKey and
// system_get_event can reach it from deep in the game code. Main thread only.
//
// FRAMEBUFFER is everything the game has drawn: one palette index (0-15) per
// pixel, written by System_SetPixel. It persists across frames, because the
// game only redraws pixels that change, and present_frame() copies all of it
// to the SDL texture once per frame. Keeping it in our own buffer, not a
// locked SDL surface, makes the game's output independent of SDL.
thread_local! {
    static EVENT_PUMP:  RefCell<Option<EventPump>>               = const { RefCell::new(None) };
    static FRAMEBUFFER: RefCell<[u8; (WIDTH * HEIGHT) as usize]> = const { RefCell::new([0; (WIDTH * HEIGHT) as usize]) };
    // In replay mode, the SDL keycodes the script is holding down this frame;
    // System_IsKey reads this instead of SDL's keyboard state. None = live play.
    static REPLAY_HELD: RefCell<Option<Vec<i32>>> = const { RefCell::new(None) };
}

static BORDER_INDEX: AtomicUsize = AtomicUsize::new(0);
static GAME_RUNNING: AtomicBool = AtomicBool::new(true);
static REPLAY_MODE: AtomicBool = AtomicBool::new(false);
// `--items N`: debug override for the remaining-item count at game start
// (negative = unset). Lets you reach the Maria/toilet victory sequence
// without collecting every item.
pub(crate) static DEBUG_ITEM_COUNT: AtomicI32 = AtomicI32::new(-1);

// ---- Exported C-facing functions --------------------------------------------

pub fn DoNothing() {}

pub fn DoQuit() {
    GAME_RUNNING.store(false, Relaxed);
    // Setting these to None is equivalent to DoNothing — fire() is a no-op on None.
    set_drawer(None);
    set_ticker(None);
}

pub fn System_Rnd() -> i32 {
    unsafe { rand() }
}

pub fn System_IsKey(key: i32) -> i32 {
    let sym = SDL_KEYS[key as usize];
    if let Some(held) = REPLAY_HELD.with(|h| h.borrow().as_ref().map(|keys| keys.contains(&sym))) {
        return held as i32;
    }
    // Live play: read SDL's keyboard state through the event pump.
    EVENT_PUMP.with(|p| {
        let pump = p.borrow();
        let pump = pump
            .as_ref()
            .expect("System_IsKey called before run() initialised SDL");
        let keycode = Keycode::from_i32(sym).expect("unknown SDL keycode");
        let scancode = Scancode::from_keycode(keycode).expect("keycode without scancode");
        pump.keyboard_state().is_scancode_pressed(scancode) as i32
    })
}

pub fn System_SetPixel(pos: i32, index: i32) {
    FRAMEBUFFER.with(|fb| fb.borrow_mut()[pos as usize] = index as u8);
}

pub fn System_Border(index: i32) {
    BORDER_INDEX.store(index as usize, Relaxed);
}

// ---- Private SDL helpers ----------------------------------------------------

// SDL calls this on its audio thread with the whole output buffer; we fill it
// one stereo frame at a time. The sdl2 crate generates the required
// `unsafe extern "C"` trampoline internally, so this callback itself is safe.
struct SdlAudioCallback;

impl AudioCallback for SdlAudioCallback {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        for frame in out.as_chunks_mut::<2>().0 {
            Audio_Output(frame);
        }
    }
}

// Copy the palette-index framebuffer into the streaming texture as RGB. Only
// the B, G, R bytes are written; the alpha byte is left untouched, as the
// original per-pixel writes did. ARGB8888 is 4 bytes/pixel, laid out B,G,R,A
// in memory on little-endian.
fn present_frame(texture: &mut Texture) {
    texture
        .with_lock(None, |buf: &mut [u8], pitch: usize| {
            FRAMEBUFFER.with(|fb| {
                for (pos, &index) in fb.borrow().iter().enumerate() {
                    let pos = pos as i32;
                    let offset = ((pos / WIDTH) * pitch as i32 + (pos & 255) * 4) as usize;
                    let c = &videoColour[index as usize];
                    buf[offset] = c.b;
                    buf[offset + 1] = c.g;
                    buf[offset + 2] = c.r;
                }
            });
        })
        .expect("lock streaming texture");
}

// Call an Event's function pointer if it is set.
fn fire(event: Event) {
    if let Some(f) = event {
        f();
    }
}

// Run one game tick: the Action, then each pending input through the
// Responder, then the Ticker and Drawer. `next_input` yields each input event
// queued since the last tick (Key::None for events that aren't game input) and
// None once the queue is empty. Shared by live play and replay so both run the
// exact same frame logic.
pub(crate) fn game_frame(timer_flash: &mut Timer, mut next_input: impl FnMut() -> Option<i32>) {
    fire(action());
    while let Some(input) = next_input() {
        set_game_input(input);
        if game_input() != Key::None as i32 {
            fire(responder());
        }
    }
    // As in live play, where the final (empty) poll leaves gameInput at None.
    set_game_input(Key::None as i32);
    fire(ticker());
    fire(drawer());
    toggle_video_flash(Timer_Update(timer_flash));
}

// Read-only view of the framebuffer (palette indices) and the border colour,
// for replay hashing and frame dumps.
pub(crate) fn with_framebuffer<R>(f: impl FnOnce(&[u8]) -> R) -> R {
    FRAMEBUFFER.with(|fb| f(&fb.borrow()[..]))
}

pub(crate) fn border_index() -> usize {
    BORDER_INDEX.load(Relaxed)
}

pub(crate) fn set_replay_mode() {
    REPLAY_MODE.store(true, Relaxed);
}

// The version string shown on the loader screen. Replay mode pins it, because
// the real one carries the build date and would change the frame hashes daily.
pub(crate) fn build_version() -> &'static str {
    if REPLAY_MODE.load(Relaxed) {
        "1.0.0 replay"
    } else {
        env!("BUILD")
    }
}

pub(crate) fn set_replay_held(keys: Option<Vec<i32>>) {
    REPLAY_HELD.with(|h| *h.borrow_mut() = keys);
}

pub(crate) fn seed_rng(seed: u32) {
    unsafe { srand(seed) };
}

pub(crate) fn game_running() -> bool {
    GAME_RUNNING.load(Relaxed)
}

pub(crate) fn audio_init() {
    Audio_Init();
}

pub(crate) fn audio_output(stereo_frame: &mut [i16; 2]) {
    Audio_Output(stereo_frame);
}

// Poll one SDL event, update gameInput, return false when the queue is empty.
fn system_get_event() -> bool {
    set_game_input(Key::None as i32);

    EVENT_PUMP.with(|p| {
        let mut pump = p.borrow_mut();
        let Some(pump) = pump.as_mut() else {
            return false;
        };
        match pump.poll_event() {
            None => false,
            Some(SdlEvent::Quit { .. }) => {
                DoQuit();
                true
            }
            Some(SdlEvent::KeyDown {
                keycode: Some(keycode),
                repeat: false,
                ..
            }) => {
                set_game_input(map_keycode(keycode.into_i32()));
                true
            }
            Some(_) => true,
        }
    })
}

// Map an SDL keycode to the game's one-shot input (gameInput) for a keydown.
// Shared by live play and replay, so scripted presses map exactly like real ones.
pub(crate) fn map_keycode(sym: i32) -> i32 {
    let k1 = Keycode::NUM_1.into_i32();
    let ka = Keycode::A.into_i32();

    if sym == Keycode::Return.into_i32() {
        Key::Enter as i32
    } else if sym == Keycode::Escape.into_i32() {
        Key::Escape as i32
    } else if sym == Keycode::Pause.into_i32() || sym == Keycode::Tab.into_i32() {
        Key::Pause as i32
    } else if sym == Keycode::LAlt.into_i32() || sym == Keycode::RAlt.into_i32() {
        Key::Mute as i32
    } else if sym >= k1 && sym <= Keycode::NUM_4.into_i32() {
        Key::K1 as i32 + (sym - k1)
    } else if matches!(sym - ka, 4 | 8 | 15 | 17 | 19 | 22 | 24) {
        // e i p r t w y
        Key::A as i32 + (sym - ka)
    } else {
        Key::Else as i32
    }
}

// ---- Entry point ------------------------------------------------------------

// Monotonic millisecond clock. TimerSubsystem::ticks() returns u32 ms since
// SDL init and wraps every ~49.7 days, so accumulate wrapping deltas into a
// u64 (the pre-safe-API port called SDL_GetTicks64, which is already u64).
struct Clock {
    last: u32,
    total: u64,
}

impl Clock {
    fn new() -> Self {
        Clock { last: 0, total: 0 }
    }

    fn now(&mut self, ticks: u32) -> u64 {
        self.total += ticks.wrapping_sub(self.last) as u64;
        self.last = ticks;
        self.total
    }
}

pub fn run() {
    // `jetsetrusty --replay <script>`: headless, deterministic run for the
    // regression tests (see replay.rs). No SDL is initialised.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--replay") {
        std::process::exit(crate::replay::main(&args[2..]));
    }
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--items" {
            if let Some(v) = args.get(i + 1).and_then(|s| s.parse::<i32>().ok()) {
                DEBUG_ITEM_COUNT.store(v, Relaxed);
                eprintln!(
                    "jetsetrusty: --items {v} (remaining-item count overridden at game start)"
                );
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    // SDL init ----------------------------------------------------------------
    let sdl = sdl2::init().expect("SDL init");
    sdl2::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");

    let video = sdl.video().expect("SDL video subsystem");
    let mode = video.desktop_display_mode(0).expect("desktop display mode");

    let (mut vx, mut vy, mut vw, mut vh) = (0i32, 0i32, 0i32, 0i32);
    let multiply = Video_Viewport(mode.w, mode.h, &mut vx, &mut vy, &mut vw, &mut vh);
    let viewport = Rect::new(vx, vy, vw as u32, vh as u32);

    // Window and renderer -----------------------------------------------------
    let window = video
        .window("Jet-Set Willy", 0, 0)
        .position_centered()
        .fullscreen_desktop()
        .build()
        .expect("create window");
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .target_texture()
        .build()
        .expect("create canvas");

    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "2");
    let texture_creator = canvas.texture_creator();
    let mut target = texture_creator
        .create_texture_target(
            PixelFormatEnum::ARGB8888,
            (WIDTH * multiply) as u32,
            (HEIGHT * multiply) as u32,
        )
        .expect("create target texture");
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::ARGB8888, WIDTH as u32, HEIGHT as u32)
        .expect("create streaming texture");

    sdl.mouse().show_cursor(false);

    // Audio -------------------------------------------------------------------
    // 22050 Hz stereo S16 (i16 Channel = native-endian S16SYS), 256-sample
    // buffers, pull-based via the AudioCallback above.
    let audio = sdl.audio().expect("SDL audio subsystem");
    let audio_device = audio
        .open_playback(
            None,
            &AudioSpecDesired {
                freq: Some(SAMPLERATE),
                channels: Some(2),
                samples: Some(256),
            },
            |_spec| SdlAudioCallback,
        )
        .expect("open audio device");
    audio_device.resume();

    // Events, keyboard, timer, RNG ----------------------------------------------
    let event_pump = sdl.event_pump().expect("SDL event pump");
    EVENT_PUMP.with(|p| *p.borrow_mut() = Some(event_pump));
    let timer = sdl.timer().expect("SDL timer subsystem");

    unsafe { srand(time(std::ptr::null_mut::<i64>()) as u32) };

    Audio_Init();

    // Timers ------------------------------------------------------------------
    let mut timer_frame = Timer {
        acc: 0,
        rate: 0,
        remainder: 0,
        divisor: 0,
    };
    let mut timer_flash = Timer {
        acc: 0,
        rate: 0,
        remainder: 0,
        divisor: 0,
    };
    // One game tick per loop iteration; the loop itself is throttled to
    // TICKRATE iterations/second by the wall-clock pacer below (a port of the
    // original C System_StartFrame). The renderer still requests vsync for
    // tear-free output, but vsync must not be relied on for pacing: several
    // drivers and compositors silently ignore PRESENTVSYNC, which lets the
    // loop spin unthrottled and the game run many times too fast.
    Timer_Set(&mut timer_frame, TICKRATE, TICKRATE);
    Timer_Set(&mut timer_flash, 25, TICKRATE * 8);

    // Main loop ---------------------------------------------------------------
    // Wall-clock pacer: frame_time is set to now+1000 per rendered frame, while
    // now * TICKRATE puts the millisecond clock on the same scale, so the loop
    // runs one iteration per 1000/TICKRATE ms (60 Hz). Unlike the C original
    // (System_StartFrame, which does `time += 1000` and therefore fast-forwards
    // to catch up after any stall), we restart frame_time from the current wall
    // clock, dropping any backlog: if the loop stalled (e.g. the window was
    // unfocused and vsync blocked present()), the game resumes at 60 Hz instead
    // of bursting at full speed. Deliberate deviation from the C original.
    let mut clock = Clock::new();
    let mut frame_time: u64 = 0;
    while GAME_RUNNING.load(Relaxed) {
        while frame_time > clock.now(timer.ticks()) * TICKRATE as u64 {
            timer.delay(1);
        }
        frame_time = clock.now(timer.ticks()) * TICKRATE as u64 + 1000;

        let frame = Timer_Update(&mut timer_frame);

        for _ in 0..frame {
            game_frame(&mut timer_flash, || {
                if system_get_event() {
                    Some(game_input())
                } else {
                    None
                }
            });
        }

        present_frame(&mut texture);

        let border = &videoColour[BORDER_INDEX.load(Relaxed)];
        canvas.set_draw_color(Color::RGB(border.r, border.g, border.b));
        canvas.clear();
        canvas
            .with_texture_canvas(&mut target, |c| {
                c.copy(&texture, None, None)
                    .expect("copy game texture to target");
            })
            .expect("render target support");
        canvas
            .copy(&target, None, Some(viewport))
            .expect("copy target to window");
        canvas.present();
    }

    // Teardown: SDL_Quit and friends run via Drop when sdl, canvas, the
    // textures, and the audio device go out of scope.
}
