use crate::common::{Event, HEIGHT, Key, WIDTH};
use crate::misc::{Timer, colour_rgb, timer_set, timer_update, video_viewport, windowed_size};
use sdl3::EventPump;
use sdl3::JoystickSubsystem;
use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};
use sdl3::event::Event as SdlEvent;
use sdl3::joystick::{Joystick, JoystickId};
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::rect::Rect;
use sdl3::render::{BlendMode, ScaleMode, Texture};
use std::cell::{Cell, RefCell};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering::Relaxed};

pub const SAMPLERATE: i32 = 22050;
pub const TICKRATE: i32 = 60;

// SDL3 keycodes are u32 (SDL_Keycode); the game's input mapping works in i32.
fn keycode_i32(k: Keycode) -> i32 {
    sdl3::sys::keycode::SDL_Keycode::from(k).0 as i32
}

// SDL keycodes indexed by Key value 0..=35 (Key::Left=0 .. Key::T=35, the
// last letter with a poll slot — C's sdlKey[36] covers exactly these).
// system_is_key compares these against replay-scripted presses.
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
    static GAME_INPUT: Cell<Key> = const { Cell::new(Key::None) };
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
pub(crate) fn set_game_input(key: Key) {
    GAME_INPUT.with(|c| c.set(key));
}
pub(crate) fn game_input() -> Key {
    GAME_INPUT.with(Cell::get)
}
pub(crate) fn video_flash() -> bool {
    VIDEO_FLASH.with(|c| c.get() != 0)
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
    static FRAMEBUFFER: RefCell<[u8; WIDTH as usize * HEIGHT as usize]> = const { RefCell::new([0; WIDTH as usize * HEIGHT as usize]) };
    // In replay mode, the SDL keycodes the script is holding down this frame;
    // system_is_key reads this instead of SDL's keyboard state. None = live play.
    static REPLAY_HELD: RefCell<Option<Vec<i32>>> = const { RefCell::new(None) };
    // Upstream joyState[3]: [left, right, fire] (fire = any joystick button).
    // Set from joystick events in live play; stays false in replay mode, where
    // the SDL event loop never runs, so replays are unaffected.
    static JOY_STATE: Cell<[bool; 3]> = const { Cell::new([false; 3]) };
    static JOYSTICK: RefCell<Option<Joystick>> = const { RefCell::new(None) };
    static JOY_SUBSYSTEM: RefCell<Option<JoystickSubsystem>> = const { RefCell::new(None) };
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

/// Is this key held? Maps the Key to its SDL keycode/scancode through the
/// poll tables, which are indexed by Key value (C's sdlKey layout — only
/// Left..=T, values 0..=35, have slots).
pub fn system_is_key(key: Key) -> bool {
    let sym = SDL_KEYS[key as usize];
    if let Some(held) = REPLAY_HELD.with(|h| h.borrow().as_ref().map(|keys| keys.contains(&sym))) {
        return held;
    }
    // Live play: read SDL's keyboard state through the event pump.
    EVENT_PUMP.with(|p| {
        let pump = p.borrow();
        let pump = pump
            .as_ref()
            .expect("system_is_key called before run() initialised SDL");
        pump.keyboard_state()
            .is_scancode_pressed(SDL_SCANCODES[key as usize])
    })
}

// Joystick state polled by the game (upstream System_IsJoyLeft/Right/Fire).
pub(crate) fn system_is_joy_left() -> bool {
    JOY_STATE.with(|s| s.get()[0])
}
pub(crate) fn system_is_joy_right() -> bool {
    JOY_STATE.with(|s| s.get()[1])
}
pub(crate) fn system_is_joy_fire() -> bool {
    JOY_STATE.with(|s| s.get()[2])
}

fn set_joy_state(left: bool, right: bool, fire: bool) {
    JOY_STATE.with(|s| s.set([left, right, fire]));
}

// Upstream 69070b9 joystick events. The mapping from event to state lives in
// the pure map_joy_event below so it is unit-testable — replay mode never runs
// the SDL event loop (JOY_STATE stays zero there), so the golden hashes cannot
// cover it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoyEvent {
    ButtonDown,
    ButtonUp,
    Axis0(i16),
}

// Map a joystick event to (new [left, right, fire] state, one-shot game input).
// Any button is "fire" (plus a one-shot Key::Else input); axis 0 beyond
// +/-255 is left/right, inside the deadzone the reading is ignored.
fn map_joy_event(state: [bool; 3], ev: JoyEvent) -> ([bool; 3], Key) {
    match ev {
        JoyEvent::ButtonDown => ([state[0], state[1], true], Key::Else),
        JoyEvent::ButtonUp => ([state[0], state[1], false], Key::None),
        JoyEvent::Axis0(value) => {
            let (left, right) = if value < -255 {
                (true, false)
            } else if value > 255 {
                (false, true)
            } else {
                (false, false)
            };
            ([left, right, state[2]], Key::None)
        }
    }
}

fn apply_joy_event(ev: JoyEvent) {
    let (state, input) = map_joy_event(JOY_STATE.with(|s| s.get()), ev);
    set_joy_state(state[0], state[1], state[2]);
    set_game_input(input);
}

// Open a joystick if none is open yet (startup + hotplug), as upstream's
// SDL_JoystickOpen(0) plus event-driven open.
fn open_joystick(id: JoystickId) {
    let already_open = JOYSTICK.with(|j| j.borrow().is_some());
    if already_open {
        return;
    }
    JOY_SUBSYSTEM.with(|sub| {
        if let Some(sub) = sub.borrow().as_ref()
            && let Ok(j) = sub.open(id)
        {
            JOYSTICK.with(|jh| *jh.borrow_mut() = Some(j));
        }
    });
}

pub fn system_set_pixel(pos: usize, index: u8) {
    FRAMEBUFFER.with(|fb| fb.borrow_mut()[pos] = index);
}

pub fn system_border(index: u8) {
    BORDER_INDEX.store(usize::from(index), Relaxed);
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
                    let offset = (pos / WIDTH as usize) * pitch + (pos & 255) * 4;
                    let [r, g, b] = colour_rgb(usize::from(index));
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
pub(crate) fn game_frame(timer_flash: &mut Timer, mut next_input: impl FnMut() -> Option<Key>) {
    fire(action());
    while let Some(input) = next_input() {
        set_game_input(input);
        if game_input() != Key::None {
            fire(responder());
        }
    }
    // As in live play, where the final (empty) poll leaves gameInput at None.
    set_game_input(Key::None);
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
    set_game_input(Key::None);

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
                let sym = keycode_i32(keycode);
                // Swallow the window key and GUI chords before they become
                // game input (see swallow_keydown).
                let state = pump.keyboard_state();
                let gui_held = state.is_scancode_pressed(Scancode::LGui)
                    || state.is_scancode_pressed(Scancode::RGui);
                set_game_input(if swallow_keydown(sym, gui_held) {
                    Key::None
                } else {
                    map_keycode(sym)
                });
                true
            }
            // Upstream 69070b9: any joystick button is "fire" (and a one-shot
            // Key::Else input); axis 0 beyond +/-255 is left/right. The
            // mapping itself is the pure, unit-tested map_joy_event.
            Some(SdlEvent::JoyButtonDown { .. }) => {
                apply_joy_event(JoyEvent::ButtonDown);
                true
            }
            Some(SdlEvent::JoyButtonUp { .. }) => {
                apply_joy_event(JoyEvent::ButtonUp);
                true
            }
            Some(SdlEvent::JoyAxisMotion {
                axis_idx: 0, value, ..
            }) => {
                apply_joy_event(JoyEvent::Axis0(value));
                true
            }
            Some(SdlEvent::JoyDeviceAdded { which, .. }) => {
                open_joystick(which);
                true
            }
            Some(_) => true,
        }
    })
}

// The window-manager ("Super"/GUI) keys. The game has no binding for them
// (map_keycode maps them to Key::Else).
fn is_gui_keycode(sym: i32) -> bool {
    sym == keycode_i32(Keycode::LGui) || sym == keycode_i32(Keycode::RGui)
}

// A keydown is swallowed when it is itself a GUI key, or when any GUI key is
// currently held (the chord case — e.g. Super+Tab reaching us because the WM
// let it through). Deliberate deviation from upstream, which lets every key
// reach the responder: there, any unrecognised input while paused calls
// Game_Pause(0) via Cheat_Disabled, so a stray Super press would unpause the
// game (and Super+Tab would toggle pause).
fn swallow_keydown(sym: i32, gui_held: bool) -> bool {
    gui_held || is_gui_keycode(sym)
}

// Map an SDL keycode to the game's one-shot input for a keydown. Shared by
// live play and replay, so scripted presses map exactly like real ones.
// C MapKeycode: only digits 1..4 and the seven code-word letters e i p r
// t w y are inputs; every other key — movement included — maps to Key::Else
// (held keys are polled via system_is_key instead).
pub(crate) fn map_keycode(sym: i32) -> Key {
    const DIGITS: [Key; 4] = [Key::K1, Key::K2, Key::K3, Key::K4];
    const LETTER_INPUTS: [(Keycode, Key); 7] = [
        (Keycode::E, Key::E),
        (Keycode::I, Key::I),
        (Keycode::P, Key::P),
        (Keycode::R, Key::R),
        (Keycode::T, Key::T),
        (Keycode::W, Key::W),
        (Keycode::Y, Key::Y),
    ];

    if sym == keycode_i32(Keycode::Return) {
        Key::Enter
    } else if sym == keycode_i32(Keycode::Escape) {
        Key::Escape
    } else if sym == keycode_i32(Keycode::Pause) || sym == keycode_i32(Keycode::Tab) {
        Key::Pause
    } else if sym == keycode_i32(Keycode::LAlt) || sym == keycode_i32(Keycode::RAlt) {
        Key::Mute
    } else if (keycode_i32(Keycode::_1)..=keycode_i32(Keycode::_4)).contains(&sym) {
        DIGITS[(sym - keycode_i32(Keycode::_1)) as usize]
    } else {
        match LETTER_INPUTS
            .iter()
            .find(|&&(kc, _)| keycode_i32(kc) == sym)
        {
            Some(&(_, key)) => key,
            None => Key::Else,
        }
    }
}

// ---- Command-line dev flags -------------------------------------------------

// `jetsetrusty [--windowed] [--items N]` (`--replay` is handled separately in
// run() before SDL init). Parsed as a pure function so the flag combinations
// are unit-testable.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DevFlags {
    // Open a resizable window instead of fullscreen; the game is letterboxed
    // to 4:3 inside whatever size the window manager gives the window.
    pub windowed: bool,
    // `--items N`: debug override for the remaining-item count at game start
    // (None = unset).
    pub items: Option<i32>,
}

pub(crate) fn parse_flags(args: &[String]) -> DevFlags {
    let mut flags = DevFlags::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--windowed" {
            flags.windowed = true;
        } else if arg == "--items" {
            // A missing or unparseable value still consumes the slot.
            if let Some(v) = args.next().and_then(|s| s.parse::<i32>().ok()) {
                flags.items = Some(v);
            }
        }
    }
    flags
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
    let flags = parse_flags(&args[1..]);
    if let Some(v) = flags.items {
        DEBUG_ITEM_COUNT.store(v, Relaxed);
        eprintln!("jetsetrusty: --items {v} (remaining-item count overridden at game start)");
    }

    // SDL init ----------------------------------------------------------------
    let sdl = sdl3::init().expect("SDL init");
    sdl3::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");

    let video = sdl.video().expect("SDL video subsystem");
    let mode = video.displays().expect("list displays")[0]
        .get_mode()
        .expect("desktop display mode");

    // The letterboxed viewport is recomputed every frame in the main loop from
    // the window's current drawable size; `multiply` is the integer upscale of
    // the game texture into the target texture below. Initialised from the
    // desktop mode — in `--windowed` mode the first loop iteration recomputes
    // it from the actual window size before anything is drawn.
    let (mut vx, mut vy, mut vw, mut vh) = (0i32, 0i32, 0i32, 0i32);
    let mut multiply = video_viewport(mode.w, mode.h, &mut vx, &mut vy, &mut vw, &mut vh);

    // Window and renderer -----------------------------------------------------
    // SDL3 dropped SDL_WINDOW_FULLSCREEN_DESKTOP: plain fullscreen is
    // borderless at the desktop resolution, which is what we want.
    // `--windowed` instead opens a resizable window at an integer scale of the
    // full 272x208 screen (game area + border); the per-frame letterbox in the
    // main loop keeps the game at 4:3 inside whatever shape the window manager
    // gives the window, with the border colour filling the bars.
    let window = if flags.windowed {
        let (ww, wh) = windowed_size(mode.w, mode.h);
        video
            .window("Jet-Set Willy", ww as u32, wh as u32)
            .position_centered()
            .resizable()
            .build()
            .expect("create window")
    } else {
        video
            .window("Jet-Set Willy", 0, 0)
            .position_centered()
            .fullscreen()
            .build()
            .expect("create window")
    };
    // SDL3 dropped the renderer flags: create_renderer picks an accelerated
    // vsync renderer by default where supported. Pacing is wall-clock (see
    // the main loop), so vsync is only for tear-free output, never timing.
    let mut canvas = sdl3::render::create_renderer(window, None).expect("create renderer");

    let texture_creator = canvas.texture_creator();
    let mut target = texture_creator
        .create_texture_target(
            PixelFormat::ARGB8888,
            (WIDTH * multiply) as u32,
            (HEIGHT * multiply) as u32,
        )
        .expect("create target texture");
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormat::ARGB8888, WIDTH as u32, HEIGHT as u32)
        .expect("create streaming texture");
    // Two SDL3 behavior changes vs SDL2, restored explicitly here:
    // - alpha-format textures default to BLENDMODE_BLEND (SDL2: NONE), but
    //   present_frame leaves the alpha byte untouched (zero), so blending
    //   would erase the game image.
    // - new textures inherit the renderer's scale mode, which defaults to
    //   LINEAR in SDL3; the SDL_RENDER_SCALE_QUALITY hint the C original used
    //   is deprecated and ignored. NEAREST keeps the upscale pixel-crisp.
    target.set_blend_mode(BlendMode::None);
    texture.set_blend_mode(BlendMode::None);
    target.set_scale_mode(ScaleMode::Nearest);
    texture.set_scale_mode(ScaleMode::Nearest);

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

    // Events, keyboard, joystick, RNG -----------------------------------------
    let event_pump = sdl.event_pump().expect("SDL event pump");
    EVENT_PUMP.with(|p| *p.borrow_mut() = Some(event_pump));

    // Joystick (upstream 69070b9): SDL_INIT_JOYSTICK + event state enabled,
    // then open the first joystick if one is already connected. Devices
    // plugged in later arrive as JoyDeviceAdded and are opened there.
    if let Ok(joy) = sdl.joystick() {
        joy.set_joystick_events_enabled(true);
        if let Some(first) = joy.joysticks().ok().and_then(|ids| ids.first().copied())
            && let Ok(j) = joy.open(first)
        {
            JOYSTICK.with(|jh| *jh.borrow_mut() = Some(j));
        }
        JOY_SUBSYSTEM.with(|s| *s.borrow_mut() = Some(joy));
    }

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

        // Re-letterbox for the window's current size. output_size is the
        // renderer's drawable size in pixels, so HiDPI scaling is handled;
        // fall back to the window's screen-coordinate size on error. In
        // fullscreen the size never changes, so this yields the same viewport
        // as at startup; in `--windowed` mode a reshaped window is re-fit
        // every frame.
        let (win_w, win_h) = canvas
            .output_size()
            .unwrap_or_else(|_| canvas.window().size());
        let new_multiply = video_viewport(
            win_w as i32,
            win_h as i32,
            &mut vx,
            &mut vy,
            &mut vw,
            &mut vh,
        );
        if new_multiply != multiply {
            // The window crossed an integer-scale threshold: recreate the
            // upscale target at the new multiply (same SDL3 modes as below).
            multiply = new_multiply;
            target = texture_creator
                .create_texture_target(
                    PixelFormat::ARGB8888,
                    (WIDTH * multiply) as u32,
                    (HEIGHT * multiply) as u32,
                )
                .expect("create target texture");
            target.set_blend_mode(BlendMode::None);
            target.set_scale_mode(ScaleMode::Nearest);
        }

        let [r, g, b] = colour_rgb(BORDER_INDEX.load(Relaxed));
        canvas.set_draw_color(Color::RGB(r, g, b));
        canvas.clear();
        canvas
            .with_texture_canvas(&mut target, |c| {
                c.copy(&texture, None, None)
                    .expect("copy game texture to target");
            })
            .expect("render target support");
        if vw > 0 && vh > 0 {
            let viewport = Rect::new(vx, vy, vw as u32, vh as u32);
            canvas
                .copy(&target, None, viewport)
                .expect("copy target to window");
        }
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
        assert_eq!(map_keycode(keycode_i32(Kc::Return)), Key::Enter);
        assert_eq!(map_keycode(keycode_i32(Kc::Escape)), Key::Escape);
        assert_eq!(map_keycode(keycode_i32(Kc::Pause)), Key::Pause);
        assert_eq!(map_keycode(keycode_i32(Kc::Tab)), Key::Pause);
        assert_eq!(map_keycode(keycode_i32(Kc::LAlt)), Key::Mute);
        assert_eq!(map_keycode(keycode_i32(Kc::RAlt)), Key::Mute);
    }

    #[test]
    fn map_keycode_pins_digits() {
        let base = keycode_i32(Kc::_1);
        for (i, expected) in [Key::K1, Key::K2, Key::K3, Key::K4].iter().enumerate() {
            assert_eq!(map_keycode(base + i as i32), *expected, "digit {i}");
        }
        for kc in [Kc::_0, Kc::_5, Kc::_6, Kc::_7, Kc::_8, Kc::_9] {
            assert_eq!(map_keycode(keycode_i32(kc)), Key::Else);
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
                Key::from_letter(b'a' + offset as u8)
            } else {
                Key::Else
            };
            assert_eq!(map_keycode(sym), expected, "letter offset {offset}");
        }
    }

    #[test]
    fn map_keycode_pins_movement_keys_to_else() {
        // Held movement keys are polled via system_is_key, not one-shot input.
        for kc in [Kc::Left, Kc::Right, Kc::Space, Kc::LShift, Kc::RShift] {
            assert_eq!(map_keycode(keycode_i32(kc)), Key::Else);
        }
    }

    // SDL_KEYS must stay coherent with map_keycode for the inputs that
    // system_is_key polls: Return maps to Enter and digits 1-4 to K1-K4.
    #[test]
    fn sdl_keys_table_matches_map_keycode() {
        assert_eq!(SDL_KEYS.len(), 36);
        for k in [Key::Enter, Key::K1, Key::K2, Key::K3, Key::K4] {
            assert_eq!(map_keycode(SDL_KEYS[k as usize]), k);
        }
    }

    // The window-manager key and any chord containing it must never reach the
    // game: upstream's Cheat_Disabled unpauses on any unrecognised input, so a
    // leaked Super press (or Super+Tab, which maps to Key::Pause) would
    // otherwise pause/unpause the game from under the WM.
    // The joystick event mapping (upstream 69070b9), pinned as pure logic:
    // axis 0 beyond +/-255 is left/right with a deadzone, any button is fire
    // plus a one-shot Key::Else input, and everything else emits no input.
    #[test]
    fn joy_axis_threshold_is_pinned() {
        let s = [false, false, false];
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(-256)),
            ([true, false, false], Key::None)
        );
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(-255)),
            ([false, false, false], Key::None)
        );
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(0)),
            ([false, false, false], Key::None)
        );
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(255)),
            ([false, false, false], Key::None)
        );
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(256)),
            ([false, true, false], Key::None)
        );
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(i16::MIN)),
            ([true, false, false], Key::None)
        );
        assert_eq!(
            map_joy_event(s, JoyEvent::Axis0(i16::MAX)),
            ([false, true, false], Key::None)
        );
    }

    #[test]
    fn joy_buttons_and_axis_preserve_the_other_channels() {
        // button down: fire=true and a one-shot Key::Else input, left/right kept
        assert_eq!(
            map_joy_event([true, false, false], JoyEvent::ButtonDown),
            ([true, false, true], Key::Else)
        );
        // button up: fire=false, no one-shot input, left/right kept
        assert_eq!(
            map_joy_event([false, true, true], JoyEvent::ButtonUp),
            ([false, true, false], Key::None)
        );
        // axis motion: fire preserved, no one-shot input
        assert_eq!(
            map_joy_event([false, false, true], JoyEvent::Axis0(300)),
            ([false, true, true], Key::None)
        );
    }

    #[test]
    fn gui_keycodes_and_chords_are_swallowed() {
        let lgui = keycode_i32(Kc::LGui);
        let rgui = keycode_i32(Kc::RGui);
        assert!(is_gui_keycode(lgui));
        assert!(is_gui_keycode(rgui));
        assert!(!is_gui_keycode(keycode_i32(Kc::A)));
        assert!(!is_gui_keycode(keycode_i32(Kc::Tab)));
        // A bare window key is swallowed even with no other GUI key held...
        assert!(swallow_keydown(lgui, false));
        assert!(swallow_keydown(rgui, false));
        // ...and any key is swallowed while a GUI key is held (chords).
        assert!(swallow_keydown(keycode_i32(Kc::Tab), true));
        assert!(swallow_keydown(keycode_i32(Kc::Escape), true));
        assert!(swallow_keydown(keycode_i32(Kc::A), true));
        // Normal keys pass through when no GUI key is held.
        assert!(!swallow_keydown(keycode_i32(Kc::Tab), false));
        assert!(!swallow_keydown(keycode_i32(Kc::A), false));
        assert!(!swallow_keydown(keycode_i32(Kc::Return), false));
    }

    #[test]
    fn parse_flags_handles_windowed_and_items() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(parse_flags(&a(&[])), DevFlags::default());
        assert_eq!(
            parse_flags(&a(&["--windowed"])),
            DevFlags {
                windowed: true,
                items: None
            }
        );
        assert_eq!(
            parse_flags(&a(&["--items", "7", "--windowed"])),
            DevFlags {
                windowed: true,
                items: Some(7)
            }
        );
        assert_eq!(
            parse_flags(&a(&["--windowed", "--items", "-3"])),
            DevFlags {
                windowed: true,
                items: Some(-3)
            }
        );
        // Unknown flags are ignored; a missing or invalid --items value is
        // consumed without setting items (same as the old inline loop).
        assert_eq!(
            parse_flags(&a(&["--bogus", "--items"])),
            DevFlags {
                windowed: false,
                items: None
            }
        );
        assert_eq!(
            parse_flags(&a(&["--items", "x", "--windowed"])),
            DevFlags {
                windowed: true,
                items: None
            }
        );
        // Repeated --items: the last one wins (the old loop overwrote too).
        assert_eq!(
            parse_flags(&a(&["--items", "1", "--items", "2"])),
            DevFlags {
                windowed: false,
                items: Some(2)
            }
        );
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
