use crate::common::{Event, HEIGHT, Key, WIDTH};
use crate::misc::{Timer, colour_rgb, timer_set, timer_update, video_viewport};
use sdl3::EventPump;
use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};
use sdl3::event::Event as SdlEvent;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::rect::Rect;
use sdl3::render::Texture;
use std::cell::{Cell, RefCell};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering::Relaxed};

pub const SAMPLERATE: i32 = 22050;
pub const TICKRATE: i32 = 60;

// SDL3 keycodes are u32 (SDL_Keycode); the game's input mapping works in i32.
fn keycode_i32(k: Keycode) -> i32 {
    sdl3::sys::keycode::SDL_Keycode::from(k).0 as i32
}

// SDL keycodes indexed by key value 0..35 (Key::Left=0 .. Key::A=10 plus the
// letter range 10..35 produced arithmetically by map_keycode). system_is_key
// compares these against replay-scripted presses.
static SDL_KEYS: LazyLock<[i32; 36]> = LazyLock::new(|| {
    [
        keycode_i32(Keycode::Left),
        keycode_i32(Keycode::Right),
        keycode_i32(Keycode::Space),
        keycode_i32(Keycode::Return),
        keycode_i32(Keycode::LShift),
        keycode_i32(Keycode::RShift),
        keycode_i32(Keycode::_1),
        keycode_i32(Keycode::_2),
        keycode_i32(Keycode::_3),
        keycode_i32(Keycode::_4),
        keycode_i32(Keycode::_5),
        keycode_i32(Keycode::_6),
        keycode_i32(Keycode::_7),
        keycode_i32(Keycode::_8),
        keycode_i32(Keycode::_9),
        keycode_i32(Keycode::_0),
        keycode_i32(Keycode::A),
        keycode_i32(Keycode::B),
        keycode_i32(Keycode::C),
        keycode_i32(Keycode::D),
        keycode_i32(Keycode::E),
        keycode_i32(Keycode::F),
        keycode_i32(Keycode::G),
        keycode_i32(Keycode::H),
        keycode_i32(Keycode::I),
        keycode_i32(Keycode::J),
        keycode_i32(Keycode::K),
        keycode_i32(Keycode::L),
        keycode_i32(Keycode::M),
        keycode_i32(Keycode::N),
        keycode_i32(Keycode::O),
        keycode_i32(Keycode::P),
        keycode_i32(Keycode::Q),
        keycode_i32(Keycode::R),
        keycode_i32(Keycode::S),
        keycode_i32(Keycode::T),
    ]
});

// Parallel scancode table for the same 36 keys: SDL3's keyboard state is
// scancode-indexed and the crate's keycode->scancode conversion is unsafe, so
// held-key polling uses this table directly instead.
static SDL_SCANCODES: LazyLock<[Scancode; 36]> = LazyLock::new(|| {
    [
        Scancode::Left,
        Scancode::Right,
        Scancode::Space,
        Scancode::Return,
        Scancode::LShift,
        Scancode::RShift,
        Scancode::_1,
        Scancode::_2,
        Scancode::_3,
        Scancode::_4,
        Scancode::_5,
        Scancode::_6,
        Scancode::_7,
        Scancode::_8,
        Scancode::_9,
        Scancode::_0,
        Scancode::A,
        Scancode::B,
        Scancode::C,
        Scancode::D,
        Scancode::E,
        Scancode::F,
        Scancode::G,
        Scancode::H,
        Scancode::I,
        Scancode::J,
        Scancode::K,
        Scancode::L,
        Scancode::M,
        Scancode::N,
        Scancode::O,
        Scancode::P,
        Scancode::Q,
        Scancode::R,
        Scancode::S,
        Scancode::T,
    ]
});

use crate::audio::{audio_init, audio_output};
use crate::loader::loader_action;

// libc rand/srand are vendored in safe Rust in src/rng.rs (glibc's TYPE_3
// generator, pinned to libc's exact sequence by tests there), so no libc FFI
// remains in the crate.

// ---- Dispatcher state -------------------------------------------------------
// The four Event slots plus gameInput/videoFlash. All are main-thread only
// (the SDL audio callback never touches them), so thread-local Cells are
// sound and need no unsafe. This replaces the old #[no_mangle] static mut
// globals from the C port.
thread_local! {
    static ACTION: Cell<Event> = const { Cell::new(Some(loader_action)) };
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

// The event pump is owned by run() but stored here so system_is_key and
// system_get_event can reach it from deep in the game code. Main thread only.
//
// FRAMEBUFFER is everything the game has drawn: one palette index (0-15) per
// pixel, written by system_set_pixel. It persists across frames, because the
// game only redraws pixels that change, and present_frame() copies all of it
// to the SDL texture once per frame. Keeping it in our own buffer, not a
// locked SDL surface, makes the game's output independent of SDL.
thread_local! {
    static EVENT_PUMP:  RefCell<Option<EventPump>>               = const { RefCell::new(None) };
    static FRAMEBUFFER: RefCell<[u8; (WIDTH * HEIGHT) as usize]> = const { RefCell::new([0; (WIDTH * HEIGHT) as usize]) };
    // In replay mode, the SDL keycodes the script is holding down this frame;
    // system_is_key reads this instead of SDL's keyboard state. None = live play.
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

pub fn do_nothing() {}

pub fn do_quit() {
    GAME_RUNNING.store(false, Relaxed);
    // Setting these to None is equivalent to do_nothing — fire() is a no-op on None.
    set_drawer(None);
    set_ticker(None);
}

pub fn system_rnd() -> i32 {
    crate::rng::rand()
}

pub fn system_is_key(key: i32) -> i32 {
    let sym = SDL_KEYS[key as usize];
    if let Some(held) = REPLAY_HELD.with(|h| h.borrow().as_ref().map(|keys| keys.contains(&sym))) {
        return held as i32;
    }
    // Live play: read SDL's keyboard state through the event pump.
    EVENT_PUMP.with(|p| {
        let pump = p.borrow();
        let pump = pump
            .as_ref()
            .expect("system_is_key called before run() initialised SDL");
        pump.keyboard_state()
            .is_scancode_pressed(SDL_SCANCODES[key as usize]) as i32
    })
}

pub fn system_set_pixel(pos: i32, index: i32) {
    FRAMEBUFFER.with(|fb| fb.borrow_mut()[pos as usize] = index as u8);
}

pub fn system_border(index: i32) {
    BORDER_INDEX.store(index as usize, Relaxed);
}

// ---- Private SDL helpers ----------------------------------------------------

// SDL calls this on its audio thread whenever the output stream needs more
// data; we fill a buffer one stereo frame at a time and hand it to the stream.
// The sdl3 crate generates the required `unsafe extern "C"` trampoline
// internally, so this callback itself is safe.
struct SdlAudioCallback;

impl AudioCallback<i16> for SdlAudioCallback {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        // `requested` is the number of interleaved i16 samples SDL wants
        // (SDL3's additional_amount is in bytes; the crate divides by the
        // sample size).
        let mut buf = vec![0i16; requested.max(0) as usize];
        for frame in buf.as_chunks_mut::<2>().0 {
            audio_output(frame);
        }
        let _ = stream.put_data_i16(&buf);
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
                    let [r, g, b] = colour_rgb(index as usize);
                    buf[offset] = b;
                    buf[offset + 1] = g;
                    buf[offset + 2] = r;
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
    toggle_video_flash(timer_update(timer_flash));
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
    crate::rng::srand(seed);
}

pub(crate) fn game_running() -> bool {
    GAME_RUNNING.load(Relaxed)
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
                do_quit();
                true
            }
            Some(SdlEvent::KeyDown {
                keycode: Some(keycode),
                repeat: false,
                ..
            }) => {
                set_game_input(map_keycode(keycode_i32(keycode)));
                true
            }
            Some(_) => true,
        }
    })
}

// Map an SDL keycode to the game's one-shot input (gameInput) for a keydown.
// Shared by live play and replay, so scripted presses map exactly like real ones.
pub(crate) fn map_keycode(sym: i32) -> i32 {
    let k1 = keycode_i32(Keycode::_1);
    let ka = keycode_i32(Keycode::A);

    if sym == keycode_i32(Keycode::Return) {
        Key::Enter as i32
    } else if sym == keycode_i32(Keycode::Escape) {
        Key::Escape as i32
    } else if sym == keycode_i32(Keycode::Pause) || sym == keycode_i32(Keycode::Tab) {
        Key::Pause as i32
    } else if sym == keycode_i32(Keycode::LAlt) || sym == keycode_i32(Keycode::RAlt) {
        Key::Mute as i32
    } else if sym >= k1 && sym <= keycode_i32(Keycode::_4) {
        Key::K1 as i32 + (sym - k1)
    } else if matches!(sym - ka, 4 | 8 | 15 | 17 | 19 | 22 | 24) {
        // e i p r t w y
        Key::A as i32 + (sym - ka)
    } else {
        Key::Else as i32
    }
}

// ---- Entry point ------------------------------------------------------------

// Wall-clock pacer arithmetic as pure functions, so unit tests can pin the
// cadence and the no-catch-up behavior (see the main loop in run()). Both
// work on the "milliseconds * TICKRATE" scale: frame_time > now * TICKRATE
// means a frame is still owed.
fn pacer_should_wait(frame_time: u64, now_ms: u64) -> bool {
    frame_time > now_ms * TICKRATE as u64
}

// Schedule the next frame 1000 (ms*TICKRATE) after now. Restarting from the
// current wall clock — rather than accumulating like the C original's
// System_StartFrame — drops any backlog after a stall.
fn next_frame_time(now_ms: u64) -> u64 {
    now_ms * TICKRATE as u64 + 1000
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
    let sdl = sdl3::init().expect("SDL init");
    sdl3::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");

    let video = sdl.video().expect("SDL video subsystem");
    let mode = video.displays().expect("list displays")[0]
        .get_mode()
        .expect("desktop display mode");

    let (mut vx, mut vy, mut vw, mut vh) = (0i32, 0i32, 0i32, 0i32);
    let multiply = video_viewport(mode.w, mode.h, &mut vx, &mut vy, &mut vw, &mut vh);
    let viewport = Rect::new(vx, vy, vw as u32, vh as u32);

    // Window and renderer -----------------------------------------------------
    // SDL3 dropped SDL_WINDOW_FULLSCREEN_DESKTOP: plain fullscreen is
    // borderless at the desktop resolution, which is what we want.
    let window = video
        .window("Jet-Set Willy", 0, 0)
        .position_centered()
        .fullscreen()
        .build()
        .expect("create window");
    // SDL3 dropped the renderer flags: create_renderer picks an accelerated
    // vsync renderer by default where supported. Pacing is wall-clock (see
    // the main loop), so vsync is only for tear-free output, never timing.
    let mut canvas = sdl3::render::create_renderer(window, None).expect("create renderer");

    sdl3::hint::set("SDL_RENDER_SCALE_QUALITY", "2");
    let texture_creator = canvas.texture_creator();
    let mut target = texture_creator
        .create_texture_target(
            PixelFormat::ARGB8888,
            (WIDTH * multiply) as u32,
            (HEIGHT * multiply) as u32,
        )
        .expect("create target texture");
    sdl3::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormat::ARGB8888, WIDTH as u32, HEIGHT as u32)
        .expect("create streaming texture");

    sdl.mouse().show_cursor(false);

    // Audio -------------------------------------------------------------------
    // 22050 Hz stereo S16 (i16 Channel = native-endian S16SYS), pull-based via
    // the AudioCallback above. SDL3 dropped the fixed sample-buffer size from
    // the spec; the callback is asked for whatever the stream needs.
    let audio = sdl.audio().expect("SDL audio subsystem");
    let spec = AudioSpec::new(Some(SAMPLERATE), Some(2), Some(AudioFormat::s16_sys()));
    let audio_stream = audio
        .open_playback_stream::<SdlAudioCallback, i16>(&spec, SdlAudioCallback)
        .expect("open audio stream");
    audio_stream.resume().expect("resume audio stream");

    // Events, keyboard, RNG ---------------------------------------------------
    let event_pump = sdl.event_pump().expect("SDL event pump");
    EVENT_PUMP.with(|p| *p.borrow_mut() = Some(event_pump));

    // Seed from the wall clock, like the C original's srand(time(NULL)).
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::rng::srand(now as u32);

    audio_init();

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
    timer_set(&mut timer_frame, TICKRATE, TICKRATE);
    timer_set(&mut timer_flash, 25, TICKRATE * 8);

    // Main loop ---------------------------------------------------------------
    // Wall-clock pacer: frame_time is set to now+1000 per rendered frame, while
    // now * TICKRATE puts the millisecond clock on the same scale, so the loop
    // runs one iteration per 1000/TICKRATE ms (60 Hz). Unlike the C original
    // (System_StartFrame, which does `time += 1000` and therefore fast-forwards
    // to catch up after any stall), we restart frame_time from the current wall
    // clock, dropping any backlog: if the loop stalled (e.g. the window was
    // unfocused and vsync blocked present()), the game resumes at 60 Hz instead
    // of bursting at full speed. Deliberate deviation from the C original.
    let mut frame_time: u64 = 0;
    while GAME_RUNNING.load(Relaxed) {
        while pacer_should_wait(frame_time, sdl3::timer::ticks()) {
            sdl3::timer::delay(1);
        }
        frame_time = next_frame_time(sdl3::timer::ticks());

        let frame = timer_update(&mut timer_frame);

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

        let [r, g, b] = colour_rgb(BORDER_INDEX.load(Relaxed));
        canvas.set_draw_color(Color::RGB(r, g, b));
        canvas.clear();
        canvas
            .with_texture_canvas(&mut target, |c| {
                c.copy(&texture, None, None)
                    .expect("copy game texture to target");
            })
            .expect("render target support");
        canvas
            .copy(&target, None, viewport)
            .expect("copy target to window");
        canvas.present();
    }

    // Teardown: SDL_Quit and friends run via Drop when sdl, canvas, the
    // textures, and the audio stream go out of scope.
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl3::keyboard::Keycode as Kc;

    // Pin the SDL-keycode -> game-input contract that live play and replay
    // share through map_keycode. Written against named Keycode constants, so
    // the assertions keep the same meaning across an SDL2 -> SDL3 migration:
    // the same physical key must produce the same game input.
    #[test]
    fn map_keycode_pins_special_keys() {
        assert_eq!(map_keycode(keycode_i32(Kc::Return)), Key::Enter as i32);
        assert_eq!(map_keycode(keycode_i32(Kc::Escape)), Key::Escape as i32);
        assert_eq!(map_keycode(keycode_i32(Kc::Pause)), Key::Pause as i32);
        assert_eq!(map_keycode(keycode_i32(Kc::Tab)), Key::Pause as i32);
        assert_eq!(map_keycode(keycode_i32(Kc::LAlt)), Key::Mute as i32);
        assert_eq!(map_keycode(keycode_i32(Kc::RAlt)), Key::Mute as i32);
    }

    #[test]
    fn map_keycode_pins_digits() {
        let base = keycode_i32(Kc::_1);
        for i in 0..4 {
            assert_eq!(map_keycode(base + i), Key::K1 as i32 + i, "digit {i}");
        }
        for kc in [Kc::_0, Kc::_5, Kc::_6, Kc::_7, Kc::_8, Kc::_9] {
            assert_eq!(map_keycode(keycode_i32(kc)), Key::Else as i32);
        }
    }

    #[test]
    fn map_keycode_pins_letters() {
        // Only e i p r t w y produce letter inputs; every other letter is
        // Key::Else.
        let mapped = [4, 8, 15, 17, 19, 22, 24]; // e i p r t w y
        for offset in 0..26 {
            let sym = keycode_i32(Kc::A) + offset;
            let expected = if mapped.contains(&offset) {
                Key::A as i32 + offset
            } else {
                Key::Else as i32
            };
            assert_eq!(map_keycode(sym), expected, "letter offset {offset}");
        }
    }

    #[test]
    fn map_keycode_pins_movement_keys_to_else() {
        // Held movement keys are polled via system_is_key, not one-shot input.
        for kc in [Kc::Left, Kc::Right, Kc::Space, Kc::LShift, Kc::RShift] {
            assert_eq!(map_keycode(keycode_i32(kc)), Key::Else as i32);
        }
    }

    // SDL_KEYS must stay coherent with map_keycode for the inputs that
    // system_is_key polls: Return maps back to Enter and digits 1-4 to K1-K4.
    #[test]
    fn sdl_keys_table_matches_map_keycode() {
        assert_eq!(SDL_KEYS.len(), 36);
        for k in [Key::Enter, Key::K1, Key::K2, Key::K3, Key::K4] {
            assert_eq!(map_keycode(SDL_KEYS[k as usize]), k as i32);
        }
    }

    #[test]
    fn pacer_runs_at_60hz() {
        let t0 = 1000u64;
        let ft = next_frame_time(t0); // frame scheduled at t0
        assert!(pacer_should_wait(ft, t0)); // just ran: owed until 1000/60 ms pass
        assert!(pacer_should_wait(ft, t0 + 16)); // 16 ms < 16.67 ms
        assert!(!pacer_should_wait(ft, t0 + 17)); // 17 ms > 16.67 ms: next frame fires
        assert_eq!(next_frame_time(t0 + 17), (t0 + 17) * TICKRATE as u64 + 1000);
    }

    #[test]
    fn pacer_drops_backlog_after_a_stall() {
        // The C original's System_StartFrame accumulates `time += 1000` and
        // fast-forwards to catch up after a stall. We deliberately restart
        // from the wall clock instead: after a long stall exactly one frame
        // runs, and the next frame is scheduled from the new now, never from
        // the backlog.
        let t0 = 1000u64;
        let ft = next_frame_time(t0);
        let stalled = t0 + 5000; // e.g. window unfocused for 5 s
        assert!(!pacer_should_wait(ft, stalled)); // no burst of catch-up frames
        assert_eq!(next_frame_time(stalled), stalled * TICKRATE as u64 + 1000);
    }
}
