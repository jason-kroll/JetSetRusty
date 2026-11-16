#![allow(non_snake_case, non_upper_case_globals)]

use crate::common::{Event, HEIGHT, Key, WIDTH};
use crate::misc::{Timer, Timer_Set, Timer_Update, Video_Viewport, videoColour};
use sdl2::sys as sdl;
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering::Relaxed};

pub const SAMPLERATE: i32 = 22050;
pub const TICKRATE: i32 = 60;

// SDL key code table indexed by Key enum discriminant (Key::Left=0 .. Key::T=35)
const SDL_KEYS: [sdl::SDL_Keycode; 36] = [
    sdl::SDL_KeyCode::SDLK_LEFT as i32,
    sdl::SDL_KeyCode::SDLK_RIGHT as i32,
    sdl::SDL_KeyCode::SDLK_SPACE as i32,
    sdl::SDL_KeyCode::SDLK_RETURN as i32,
    sdl::SDL_KeyCode::SDLK_LSHIFT as i32,
    sdl::SDL_KeyCode::SDLK_RSHIFT as i32,
    sdl::SDL_KeyCode::SDLK_1 as i32,
    sdl::SDL_KeyCode::SDLK_2 as i32,
    sdl::SDL_KeyCode::SDLK_3 as i32,
    sdl::SDL_KeyCode::SDLK_4 as i32,
    sdl::SDL_KeyCode::SDLK_5 as i32,
    sdl::SDL_KeyCode::SDLK_6 as i32,
    sdl::SDL_KeyCode::SDLK_7 as i32,
    sdl::SDL_KeyCode::SDLK_8 as i32,
    sdl::SDL_KeyCode::SDLK_9 as i32,
    sdl::SDL_KeyCode::SDLK_0 as i32,
    sdl::SDL_KeyCode::SDLK_a as i32,
    sdl::SDL_KeyCode::SDLK_b as i32,
    sdl::SDL_KeyCode::SDLK_c as i32,
    sdl::SDL_KeyCode::SDLK_d as i32,
    sdl::SDL_KeyCode::SDLK_e as i32,
    sdl::SDL_KeyCode::SDLK_f as i32,
    sdl::SDL_KeyCode::SDLK_g as i32,
    sdl::SDL_KeyCode::SDLK_h as i32,
    sdl::SDL_KeyCode::SDLK_i as i32,
    sdl::SDL_KeyCode::SDLK_j as i32,
    sdl::SDL_KeyCode::SDLK_k as i32,
    sdl::SDL_KeyCode::SDLK_l as i32,
    sdl::SDL_KeyCode::SDLK_m as i32,
    sdl::SDL_KeyCode::SDLK_n as i32,
    sdl::SDL_KeyCode::SDLK_o as i32,
    sdl::SDL_KeyCode::SDLK_p as i32,
    sdl::SDL_KeyCode::SDLK_q as i32,
    sdl::SDL_KeyCode::SDLK_r as i32,
    sdl::SDL_KeyCode::SDLK_s as i32,
    sdl::SDL_KeyCode::SDLK_t as i32,
];

use crate::loader::Loader_Action;

unsafe extern "C" {
    fn Audio_Output(output: *mut i16);
    fn Audio_Init();
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

// The key state is a raw pointer needed by exported functions, but only ever
// accessed on the main thread — thread_local Cell avoids static mut.
//
// FRAMEBUFFER is everything the game has drawn: one palette index (0-15) per
// pixel, written by System_SetPixel. It persists across frames, because the
// game only redraws pixels that change, and present_frame() copies all of it
// to the SDL texture once per frame. Keeping it in our own buffer, not a
// locked SDL surface, makes the game's output independent of SDL.
thread_local! {
    static KEY_STATE:   Cell<*const u8>                          = const { Cell::new(std::ptr::null()) };
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

#[unsafe(no_mangle)]
pub extern "C" fn System_Rnd() -> i32 {
    unsafe { rand() }
}

#[unsafe(no_mangle)]
pub extern "C" fn System_IsKey(key: i32) -> i32 {
    let sym = SDL_KEYS[key as usize];
    if let Some(held) = REPLAY_HELD.with(|h| h.borrow().as_ref().map(|keys| keys.contains(&sym))) {
        return held as i32;
    }
    let key_state = KEY_STATE.with(Cell::get);
    unsafe {
        let scancode = sdl::SDL_GetScancodeFromKey(sym);
        *key_state.add(scancode as usize) as i32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn System_SetPixel(pos: i32, index: i32) {
    FRAMEBUFFER.with(|fb| fb.borrow_mut()[pos as usize] = index as u8);
}

#[unsafe(no_mangle)]
pub extern "C" fn System_Border(index: i32) {
    BORDER_INDEX.store(index as usize, Relaxed);
}

// ---- Private SDL helpers ----------------------------------------------------

// Must be `unsafe extern "C"` — that is the required signature for SDL_AudioCallback.
unsafe extern "C" fn sdl_audio_callback(
    _userdata: *mut core::ffi::c_void,
    stream: *mut u8,
    mut length: i32,
) {
    let mut output = stream as *mut i16;
    while length > 0 {
        unsafe { Audio_Output(output) };
        unsafe { output = output.add(2) };
        length -= 4;
    }
}

// Copy the palette-index framebuffer to the (locked) SDL surface as RGB. Only
// the B, G, R bytes are written; the alpha byte is left untouched, as the
// original per-pixel writes did.
fn present_frame(surface: &sdl::SDL_Surface) {
    let pixels = surface.pixels as *mut u8;
    let bpp = unsafe { (*surface.format).BytesPerPixel } as i32;
    FRAMEBUFFER.with(|fb| {
        for (pos, &index) in fb.borrow().iter().enumerate() {
            let pos = pos as i32;
            let offset = (pos / WIDTH) * surface.pitch + (pos & 255) * bpp;
            let c = &videoColour[index as usize];
            unsafe {
                let pixel = pixels.add(offset as usize);
                *pixel = c.b;
                *pixel.add(1) = c.g;
                *pixel.add(2) = c.r;
            }
        }
    });
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
    toggle_video_flash(unsafe { Timer_Update(timer_flash) });
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
    unsafe { Audio_Init() };
}

pub(crate) fn audio_output(stereo_frame: &mut [i16; 2]) {
    unsafe { Audio_Output(stereo_frame.as_mut_ptr()) };
}

// Poll one SDL event, update gameInput, return false when the queue is empty.
fn system_get_event() -> bool {
    unsafe {
        set_game_input(Key::None as i32);

        let mut event: sdl::SDL_Event = std::mem::zeroed();
        if sdl::SDL_PollEvent(&raw mut event) == 0 {
            return false;
        }

        let event_type = event.type_;

        if event_type == sdl::SDL_EventType::SDL_QUIT as u32 {
            DoQuit();
            return true;
        }
        if event_type != sdl::SDL_EventType::SDL_KEYDOWN as u32 {
            return true;
        }
        if event.key.repeat != 0 {
            return true;
        }

        set_game_input(map_keycode(event.key.keysym.sym));

        true
    }
}

// Map an SDL keycode to the game's one-shot input (gameInput) for a keydown.
// Shared by live play and replay, so scripted presses map exactly like real ones.
pub(crate) fn map_keycode(sym: i32) -> i32 {
    let k1 = sdl::SDL_KeyCode::SDLK_1 as i32;
    let ka = sdl::SDL_KeyCode::SDLK_a as i32;

    if sym == sdl::SDL_KeyCode::SDLK_RETURN as i32 {
        Key::Enter as i32
    } else if sym == sdl::SDL_KeyCode::SDLK_ESCAPE as i32 {
        Key::Escape as i32
    } else if sym == sdl::SDL_KeyCode::SDLK_PAUSE as i32 || sym == sdl::SDL_KeyCode::SDLK_TAB as i32
    {
        Key::Pause as i32
    } else if sym == sdl::SDL_KeyCode::SDLK_LALT as i32 || sym == sdl::SDL_KeyCode::SDLK_RALT as i32
    {
        Key::Mute as i32
    } else if sym >= k1 && sym <= sdl::SDL_KeyCode::SDLK_4 as i32 {
        Key::K1 as i32 + (sym - k1)
    } else if matches!(sym - ka, 4 | 8 | 15 | 17 | 19 | 22 | 24) {
        // e i p r t w y
        Key::A as i32 + (sym - ka)
    } else {
        Key::Else as i32
    }
}

// ---- Entry point ------------------------------------------------------------

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
    unsafe { sdl::SDL_Init(sdl::SDL_INIT_VIDEO | sdl::SDL_INIT_AUDIO) };
    unsafe { sdl::SDL_SetHint(c"SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS".as_ptr(), c"0".as_ptr()) };

    let mut mode = sdl::SDL_DisplayMode {
        format: 0,
        w: 0,
        h: 0,
        refresh_rate: 0,
        driverdata: std::ptr::null_mut(),
    };
    unsafe { sdl::SDL_GetDesktopDisplayMode(0, &raw mut mode) };

    let (mut vx, mut vy, mut vw, mut vh) = (0i32, 0i32, 0i32, 0i32);
    let multiply = unsafe {
        Video_Viewport(
            mode.w,
            mode.h,
            &raw mut vx,
            &raw mut vy,
            &raw mut vw,
            &raw mut vh,
        )
    };
    let viewport = sdl::SDL_Rect {
        x: vx,
        y: vy,
        w: vw,
        h: vh,
    };

    // Window and renderer -----------------------------------------------------
    let window = unsafe {
        sdl::SDL_CreateWindow(
            c"Jet-Set Willy".as_ptr(),
            sdl::SDL_WINDOWPOS_CENTERED_MASK as i32,
            sdl::SDL_WINDOWPOS_CENTERED_MASK as i32,
            0,
            0,
            sdl::SDL_WindowFlags::SDL_WINDOW_FULLSCREEN_DESKTOP as u32,
        )
    };

    let renderer = unsafe {
        sdl::SDL_CreateRenderer(
            window,
            -1,
            (sdl::SDL_RendererFlags::SDL_RENDERER_TARGETTEXTURE as u32)
                | (sdl::SDL_RendererFlags::SDL_RENDERER_ACCELERATED as u32)
                | (sdl::SDL_RendererFlags::SDL_RENDERER_PRESENTVSYNC as u32),
        )
    };

    unsafe { sdl::SDL_SetHint(c"SDL_RENDER_SCALE_QUALITY".as_ptr(), c"2".as_ptr()) };
    let target = unsafe {
        sdl::SDL_CreateTexture(
            renderer,
            sdl::SDL_PixelFormatEnum::SDL_PIXELFORMAT_ARGB8888 as u32,
            sdl::SDL_TextureAccess::SDL_TEXTUREACCESS_TARGET as i32,
            WIDTH * multiply,
            HEIGHT * multiply,
        )
    };

    unsafe { sdl::SDL_SetHint(c"SDL_RENDER_SCALE_QUALITY".as_ptr(), c"0".as_ptr()) };
    let texture = unsafe {
        sdl::SDL_CreateTexture(
            renderer,
            sdl::SDL_PixelFormatEnum::SDL_PIXELFORMAT_ARGB8888 as u32,
            sdl::SDL_TextureAccess::SDL_TEXTUREACCESS_STREAMING as i32,
            WIDTH,
            HEIGHT,
        )
    };

    unsafe { sdl::SDL_ShowCursor(sdl::SDL_DISABLE as i32) };

    // Audio -------------------------------------------------------------------
    let want = sdl::SDL_AudioSpec {
        freq: SAMPLERATE,
        format: sdl::AUDIO_S16SYS as sdl::SDL_AudioFormat,
        channels: 2,
        silence: 0,
        samples: 256,
        padding: 0,
        size: 0,
        callback: Some(sdl_audio_callback),
        userdata: std::ptr::null_mut(),
    };
    let audio = unsafe {
        sdl::SDL_OpenAudioDevice(
            std::ptr::null(),
            0,
            &raw const want,
            std::ptr::null_mut(),
            0,
        )
    };
    unsafe { sdl::SDL_PauseAudioDevice(audio, 0) };

    // Keyboard and RNG --------------------------------------------------------
    KEY_STATE.with(|c| {
        c.set(unsafe { sdl::SDL_GetKeyboardState(std::ptr::null_mut()) });
    });
    unsafe { srand(time(std::ptr::null_mut::<i64>()) as u32) };

    unsafe { Audio_Init() };

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
    unsafe {
        Timer_Set(&raw mut timer_frame, TICKRATE, TICKRATE);
        Timer_Set(&raw mut timer_flash, 25, TICKRATE * 8);
    }

    // Main loop ---------------------------------------------------------------
    // Wall-clock pacer: frame_time advances 1000 units per rendered frame,
    // while SDL_GetTicks64() * TICKRATE puts the millisecond clock on the same
    // scale, so the loop runs one iteration per 1000/TICKRATE ms (60 Hz).
    let mut frame_time: u64 = 0;
    while GAME_RUNNING.load(Relaxed) {
        while frame_time > unsafe { sdl::SDL_GetTicks64() } * TICKRATE as u64 {
            unsafe { sdl::SDL_Delay(1) }
        }
        frame_time += 1000;

        let frame = unsafe { Timer_Update(&raw mut timer_frame) };

        for _ in 0..frame {
            game_frame(&mut timer_flash, || {
                if system_get_event() {
                    Some(game_input())
                } else {
                    None
                }
            });
        }

        let mut sdl_surface: *mut sdl::SDL_Surface = std::ptr::null_mut();
        unsafe { sdl::SDL_LockTextureToSurface(texture, std::ptr::null(), &raw mut sdl_surface) };
        if let Some(surface) = unsafe { sdl_surface.as_ref() } {
            present_frame(surface);
        }
        unsafe { sdl::SDL_UnlockTexture(texture) };

        let border = &videoColour[BORDER_INDEX.load(Relaxed)];
        unsafe {
            sdl::SDL_SetRenderDrawColor(renderer, border.r, border.g, border.b, 0xff);
            sdl::SDL_RenderClear(renderer);
            sdl::SDL_SetRenderTarget(renderer, target);
            sdl::SDL_RenderCopy(renderer, texture, std::ptr::null(), std::ptr::null());
            sdl::SDL_SetRenderTarget(renderer, std::ptr::null_mut());
            sdl::SDL_RenderCopy(renderer, target, std::ptr::null(), &raw const viewport);
            sdl::SDL_RenderPresent(renderer);
        }
    }

    // Teardown ----------------------------------------------------------------
    unsafe {
        sdl::SDL_CloseAudioDevice(audio);
        sdl::SDL_DestroyTexture(texture);
        sdl::SDL_DestroyTexture(target);
        sdl::SDL_DestroyRenderer(renderer);
        sdl::SDL_DestroyWindow(window);
        sdl::SDL_Quit();
    }
}
