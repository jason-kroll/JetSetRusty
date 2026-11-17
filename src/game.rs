#![warn(unused_unsafe)]
use crate::audio::{
    MUS_PLAY, MUS_STOP, SFX_ITEM, audio_music, audio_music_clock, audio_play, audio_sfx,
};
use crate::cheat::cheat_responder;
use crate::game_main::game_input;
use crate::gameover::gameover_action;
use crate::levels::{level_dir, level_drawer, level_ticker};
use crate::misc::Timer;
use crate::rope;
use crate::video::{
    video_cycle_colours, video_pixel_ink_fill, video_pixel_paper_fill, video_write_large,
};

use crate::common::{
    // Types
    Event,
    Key,
    // Constant(s)
    WIDTH,
    // C functions
    level_set_border,
};

use crate::game_main::{
    TICKRATE, do_nothing, set_action, set_drawer, set_responder, set_ticker, system_border,
};
use crate::levels::level_init;
use crate::miner::{
    miner_draw_sprite, miner_drawer, miner_inc_seq, miner_save, miner_set_seq, miner_ticker,
};
use crate::misc::{timer_set, timer_update};
use crate::robots::{robots_draw_cheat, robots_drawer, robots_flush, robots_init, robots_ticker};
use crate::rope::rope_init;
use crate::title::title_action;

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use std::sync::{LazyLock, Mutex};

// Music constants (from audio.h enum: MUS_TITLE=0, MUS_GAME=1, MUS_LOADER=2)
const MUS_GAME: i32 = 1;

// Public constants (from game.h)
pub const LIVES: usize = (18 * 8 + 4) * WIDTH as usize + 4;
pub const STATUS: usize = 21 * 8 + 4;

// Level constants
pub const THEDRIVE: i32 = 4;
pub const QUIRKAFLEEG: i32 = 16;
pub const ONTHEROOF: i32 = 18;
pub const COLDSTORE: i32 = 25;
pub const FIRSTLANDING: i32 = 28;
pub const NIGHTMAREROOM: i32 = 29;
pub const SWIMMINGPOOL: i32 = 31;
pub const THEBATHROOM: i32 = 33;
pub const MASTERBEDROOM: i32 = 35;
pub const THEBEACH: i32 = 57;

// life ink colors for drawing lives
const LIFE_INK: [u8; 7] = [0x2, 0x4, 0x6, 0x1, 0x3, 0x5, 0x7];

// Static callback variable(s)
thread_local! {
    static DO_CLOCK_UPDATE: Cell<Event> = const { Cell::new(None) };
}

fn do_clock_update() -> Event {
    DO_CLOCK_UPDATE.with(Cell::get)
}
fn set_do_clock_update(f: Event) {
    DO_CLOCK_UPDATE.with(|c| c.set(f));
}

// Probably a lot of functions go here!
// Enums (from game.h)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameMode {
    Normal = 0,
    Maria,
    Running,
    Toilet,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Above = 0,
    Right,
    Below,
    Left,
}

// miner struct (from game.h)
#[derive(Clone, Copy, Debug, Default)]
pub struct Miner {
    // Initialize to all zeros w/ Default
    pub x: i32,
    pub y: i32,
    pub tile: i32,
    pub align: i32,
    pub frame: i32,
    pub dir: i32,
    pub move_: i32, // renamed from 'move' which is a Rust keyword
    pub air: i32,
    pub jump: i32,
}

// Game state structure (private)
pub struct GameState {
    pub miner: Mutex<Miner>,

    // Frequently accessed, thread-safe, atomic fields (cheap access)
    pub cheat_enabled: AtomicBool,
    pub clock_ticks: AtomicI32,
    pub frame: AtomicI32,
    pub game_paused: AtomicI32,
    pub inactivity_timer: AtomicI32,
    pub item_count: AtomicI32,
    pub level: AtomicI32,
    pub lives: AtomicI32,
    // Sole owner (no longer synced to/from a C global); read directly by
    // miner.rs via GAME_STATE.
    pub miner_attr_split: AtomicI32,
    pub miner_willy_rope: AtomicI32,
    // Atomic caches for level_get_tile_type to avoid deadlocks
    pub miner_air: AtomicI32,
    pub miner_jump: AtomicI32,
    pub mode: AtomicU8,
    pub music: AtomicU8,

    // How to use enums in Atomics:
    // MY_ENUM.store(MyEnum::B as i32, Ordering::Release);
    // let val = MyEnum::from_repr(MY_ENUM.load(Ordering::Acquire)).unwrap();

    // Less frequent, needs interior mutability, mutex-protected fields (more expensive)
    pub level_border: Mutex<[i32; 60]>,
    pub score_clock: Mutex<[u8; 3]>,
    pub score_items: Mutex<u8>,
    pub timer: Mutex<Timer>,
}

// Per-room border colour, indexed by level. Was the `levelBorder` C global (in
// game.c, then cglobals.rs); now owned here as the initial value of
// GAME_STATE.level_border. Constant data — never mutated at runtime.
const LEVEL_BORDER: [i32; 60] = [
    5, 4, 6, 2, 3, 1, 2, 1, 4, 2, //
    2, 4, 6, 5, 1, 3, 2, 1, 2, 1, //
    2, 1, 4, 4, 1, 1, 5, 2, 3, 2, //
    2, 2, 2, 2, 1, 1, 5, 6, 2, 2, //
    1, 1, 2, 5, 3, 4, 1, 2, 4, 5, //
    5, 2, 1, 2, 5, 1, 2, 2, 5, 5, //
];

pub static GAME_STATE: LazyLock<GameState> = LazyLock::new(|| GameState {
    cheat_enabled: AtomicBool::new(false),
    clock_ticks: AtomicI32::new(0),
    frame: AtomicI32::new(0),
    game_paused: AtomicI32::new(0),
    inactivity_timer: AtomicI32::new(0),
    item_count: AtomicI32::new(0),
    level: AtomicI32::new(0),
    level_border: Mutex::new(LEVEL_BORDER),
    lives: AtomicI32::new(7),
    miner: Mutex::new(Miner::default()),
    miner_attr_split: AtomicI32::new(6),
    miner_willy_rope: AtomicI32::new(0),
    miner_air: AtomicI32::new(0),
    miner_jump: AtomicI32::new(0),
    mode: AtomicU8::new(0),
    // C: `int gameMusic = MUS_PLAY;` — music on by default.
    music: AtomicU8::new(MUS_PLAY as u8),
    score_clock: Mutex::new([0, 7, 0]), // 7:00 AM
    score_items: Mutex::new(0),
    timer: Mutex::new(Timer::default()),
});

// Game functions
//
// Installed as the `Ticker` by game_action and runs exactly once: it sets up the
// room, then swaps `Ticker` to do_game_ticker and `Action` to do_nothing — that
// do_nothing handoff is what stops the title's `game_start` from re-firing (and
// restarting the music) every frame. Must be `extern "C"` to fit `Event`.
pub fn game_init_room() {
    level_init();
    robots_init();
    rope_init();

    let game = &*GAME_STATE;
    let current_level = game.level.load(Ordering::Relaxed);

    // system_border(levelBorder[gameLevel])
    let border_value = game.level_border.lock().unwrap()[current_level as usize];
    system_border(border_value);

    miner_save();

    // Write to GAME_STATE
    game.miner_attr_split.store(6, Ordering::Relaxed);
    if game.level.load(Ordering::Relaxed) == SWIMMINGPOOL {
        game.miner_attr_split.store(5, Ordering::Relaxed); // willy goes blue when underwater
    }

    // Timer
    timer_set(&mut game.timer.lock().unwrap(), 12, TICKRATE);
    game.frame.store(1, Ordering::Relaxed);
    game.inactivity_timer.store(0, Ordering::Relaxed);
    game.miner_willy_rope.store(0, Ordering::Relaxed);

    // Ticker, Drawer, Action: the Event function-pointer globals
    if game.game_paused.load(Ordering::Relaxed) != 0 {
        set_ticker(Some(do_nothing));
        set_drawer(Some(do_draw_once));
    } else {
        set_ticker(Some(do_game_ticker));
    }
    set_action(Some(do_nothing));
}

pub fn game_pause(state: bool) {
    let game = &*GAME_STATE;

    if (game.game_paused.load(Ordering::Relaxed) != 0) == state
        || game.mode.load(Ordering::Relaxed) >= GameMode::Running as u8
    {
        return;
    }

    game.game_paused
        .store(if state { 1 } else { 0 }, Ordering::Relaxed);

    if game.game_paused.load(Ordering::Relaxed) != 0 {
        if game.cheat_enabled.load(Ordering::Relaxed) {
            set_ticker(Some(do_nothing));
            set_drawer(Some(do_nothing));
        } else {
            set_ticker(Some(do_pause_ticker));
            set_drawer(Some(do_pause_drawer));
        }
        audio_play(MUS_STOP);
    } else {
        set_ticker(Some(do_game_ticker));
        set_drawer(Some(do_game_drawer));
        audio_play(game.music.load(Ordering::Relaxed) as i32);

        game.inactivity_timer.store(0, Ordering::Relaxed);
        // Original: `if (cheatEnabled == 0)`. Redraw the status band on unpause
        // when NOT cheating — do_pause_drawer's video_cycle_colours has been
        // cycling the ink of every pixel in the buffer, and the level playfield
        // recovers because it's repainted every frame, but the status/lives band
        // is only painted here. Without this redraw it keeps the last cycled
        // colour. (The condition was inverted, so it never fired in normal play.)
        if !game.cheat_enabled.load(Ordering::Relaxed) {
            game_draw_status();
            let level = game.level.load(Ordering::Relaxed);
            let border_value = game.level_border.lock().unwrap()[level as usize];
            system_border(border_value);
        }
    }
}

pub fn do_game_responder() {
    let game = &*GAME_STATE;

    game.inactivity_timer.store(0, Ordering::Relaxed);

    let input = game_input();

    match input {
        x if x == Key::Pause as i32 => game_pause(game.game_paused.load(Ordering::Relaxed) == 0),
        x if x == Key::Mute as i32 => {
            let new_music = if game.music.load(Ordering::Relaxed) == 0 {
                1
            } else {
                0
            };
            game.music.store(new_music, Ordering::Relaxed);
            audio_play(new_music as i32);
        }
        x if x == Key::Escape as i32 => set_action(Some(title_action)),
        _ => {
            if let Some(handler) = cheat_responder() {
                handler();
            }
        }
    }
}

pub fn do_game_drawer() {
    let game = &*GAME_STATE;

    if game.frame.load(Ordering::Relaxed) == 0 {
        return;
    }

    // Matches the C original's DoGameDrawer order: level and robots (including
    // the flushing toilet) are drawn in GM_TOILET mode; only miner/rope/clock
    // are skipped, so Willy visibly disappears into the toilet during the flush.
    level_drawer();
    robots_drawer();

    if game.music.load(Ordering::Relaxed) == MUS_PLAY as u8 {
        game_draw_lives();
    }

    if game.mode.load(Ordering::Relaxed) == GameMode::Toilet as u8 {
        return;
    }

    miner_drawer();

    if let Some(func) = rope::rope_drawer() {
        func();
    }
    if let Some(func) = do_clock_update() {
        func();
    }
}

pub fn do_draw_clock() {
    let game = &*GAME_STATE;

    let mut text: Vec<u8> = vec![
        0x1, 0x0, 0x2, 0x7, b' ', 0x2, 0x6, b' ', 0x2, 0x5, b':', 0x2, 0x4, b' ', 0x2, 0x3, b' ',
        0x2, 0x2, b' ', 0x2, 0x1, b'm', 0,
    ];

    let score_clock = *game.score_clock.lock().unwrap();

    text[19] = if score_clock[2] != 0 { b'p' } else { b'a' };
    text[16] = (score_clock[0] % 10) + b'0';
    text[13] = (score_clock[0] / 10) + b'0';
    text[7] = (score_clock[1] % 10) + b'0';
    if score_clock[1] > 9 {
        text[4] = (score_clock[1] / 10) + b'0';
    }

    video_write_large(WIDTH - 60, STATUS as i32, &text[..]);
    set_do_clock_update(Some(do_nothing));
}

pub fn do_pause_ticker() {
    let game = &*GAME_STATE;

    let old_paused = game.game_paused.fetch_add(1, Ordering::Relaxed);
    if old_paused == 16 * 5 {
        game.game_paused.store(1, Ordering::Relaxed);
    }
}

pub fn do_pause_drawer() {
    let game = &*GAME_STATE;

    if game.game_paused.load(Ordering::Relaxed) == 16 * 5 {
        level_set_border();
        video_cycle_colours();
    }
}

pub fn game_game_reset() {
    let game = &*GAME_STATE;

    // gameScoreItems = 0
    *game.score_items.lock().unwrap() = 0;

    // gameScoreClock[0] = 0; gameScoreClock[1] = 7; gameScoreClock[2] = 0
    *game.score_clock.lock().unwrap() = [0, 7, 0];

    // DoClockUpdate = do_draw_clock
    set_do_clock_update(Some(do_draw_clock));

    // gameClockTicks = 0
    game.clock_ticks.store(0, Ordering::Relaxed);

    // gamePaused = 0
    game.game_paused.store(0, Ordering::Relaxed);

    // miner_set_seq(0, 20)
    miner_set_seq(0, 20);

    // gameLives = 7
    game.lives.store(7, Ordering::Relaxed);

    // audio_music(MUS_GAME, gameMusic)
    let music = game.music.load(Ordering::Relaxed);
    audio_music(MUS_GAME, music as i32);
}

pub fn game_cheat_enabled() {
    let game = &*GAME_STATE;

    if game.game_paused.load(Ordering::Relaxed) != 0 {
        // gameFrame = 1
        game.frame.store(1, Ordering::Relaxed);

        // Ticker = do_nothing; Drawer = do_nothing;
        set_ticker(Some(do_nothing));
        set_drawer(Some(do_nothing));

        // game_draw_status(); system_border(levelBorder[gameLevel])
        game_draw_status();
        let level = game.level.load(Ordering::Relaxed);
        system_border(game.level_border.lock().unwrap()[level as usize]);
    }

    // cheatEnabled = 1
    game.cheat_enabled.store(true, Ordering::Relaxed);

    // robots_draw_cheat()
    robots_draw_cheat();
}

pub fn game_change_level(dir: i32) {
    let game = &*GAME_STATE;

    // int level = level_dir(dir)
    let level = level_dir(dir as usize);

    // Special case for R_ABOVE from THEDRIVE or FIRSTLANDING
    if dir == Direction::Above as i32 {
        let mut miner = game.miner.lock().unwrap();
        if (level == THEDRIVE && miner.x > 22 && miner.x < 32)
            || (level == FIRSTLANDING && miner.x > 182)
        {
            // minerWilly.air = 2; return
            miner.air = 2;
            return;
        }
    }

    // gameLevel = level
    game.level.store(level, Ordering::Relaxed);

    // Update minerWilly based on direction
    // Scoped so the miner guard drops before game_init_room() (miner_save
    // locks GAME_STATE.miner itself).
    {
        let mut miner = game.miner.lock().unwrap();
        match dir {
            x if x == Direction::Above as i32 => {
                // minerWilly.y = 13 * 8 = 104
                miner.y = 13 * 8;
                // minerWilly.x = (minerWilly.tile & 31) * 8
                miner.x = (miner.tile & 31) * 8;
                // minerWilly.tile = 13 * 32 + (minerWilly.tile & 31)
                miner.tile = 13 * 32 + (miner.tile & 31);
                // minerWilly.align = 4
                miner.align = 4;
                // minerWilly.air = 0
                miner.air = 0;
            }
            x if x == Direction::Right as i32 => {
                // minerWilly.x = 0
                miner.x = 0;
                // minerWilly.tile &= ~31
                miner.tile &= !31;
            }
            x if x == Direction::Below as i32 => {
                // if (minerWilly.air < 11) { minerWilly.air = 2; }
                if miner.air < 11 {
                    miner.air = 2;
                }
                // minerWilly.y = 0
                miner.y = 0;
                // minerWilly.tile &= 31
                miner.tile &= 31;
            }
            x if x == Direction::Left as i32 => {
                // minerWilly.x = 30 * 8 = 240
                miner.x = 30 * 8;
                // minerWilly.tile |= 30
                miner.tile |= 30;
            }
            _ => {}
        }
    }

    game_init_room();
}

pub fn do_draw_once() {
    do_game_drawer();

    set_drawer(Some(do_nothing));
}

pub fn do_game_ticker() {
    let game = &*GAME_STATE;

    // Inactivity check: pause after 5 minutes when music stopped
    if game.music.load(Ordering::Relaxed) == MUS_STOP as u8 {
        let old = game.inactivity_timer.fetch_add(1, Ordering::Relaxed);
        if old == 256 * 5 && game.mode.load(Ordering::Relaxed) < GameMode::Running as u8 {
            game_pause(true);
            return;
        }
    }

    // Miner animation
    if game.music.load(Ordering::Relaxed) == MUS_PLAY as u8 {
        miner_inc_seq();
    }

    // Update game frame
    let frame = {
        let mut timer = game.timer.lock().unwrap();
        timer_update(&mut timer)
    };
    game.frame.store(frame, Ordering::Relaxed);
    if frame == 0 {
        return;
    }

    // Tick level and robots
    level_ticker();
    robots_ticker();

    // GM_TOILET mode: return to title after timeout
    if game.mode.load(Ordering::Relaxed) == GameMode::Toilet as u8 {
        let old_ticks = game.clock_ticks.fetch_add(1, Ordering::Relaxed);
        if old_ticks == 256 {
            set_action(Some(title_action));
        }
        return;
    }

    // Tick miner
    miner_ticker();

    // GM_RUNNING mode
    if game.mode.load(Ordering::Relaxed) == GameMode::Running as u8 {
        let mut miner = game.miner.lock().unwrap();
        miner.frame |= 1;

        if miner.x == 224 && game.level.load(Ordering::Relaxed) == THEBATHROOM {
            drop(miner);
            game.mode.store(GameMode::Toilet as u8, Ordering::Relaxed);
            robots_flush();
            game.clock_ticks.store(0, Ordering::Relaxed);
        }
        return;
    }

    // GM_MARIA mode: check for victory
    if game.mode.load(Ordering::Relaxed) == GameMode::Maria as u8
        && game.level.load(Ordering::Relaxed) == MASTERBEDROOM
    {
        // Guard drops at the end of this if-block, before Rope_Ticker.
        let miner = game.miner.lock().unwrap();
        if miner.air == 0 && miner.x == 40 {
            game.mode.store(GameMode::Running as u8, Ordering::Relaxed);
        }
    }

    // Tick rope
    if let Some(f) = rope::rope_ticker() {
        f();
    }

    // Tick clock
    clock_ticker();
}

pub fn game_got_item() {
    let game = &*GAME_STATE;

    // gameScoreItems++
    *game.score_items.lock().unwrap() += 1;

    // game_draw_status()
    game_draw_status();

    // if (--itemCount == 0) { gameMode = GM_MARIA; }
    if game.item_count.fetch_sub(1, Ordering::Relaxed) == 1 {
        game.mode.store(GameMode::Maria as u8, Ordering::Relaxed);
    }

    // audio_pan_x = minerWilly.x
    // Use try_lock to avoid deadlock if we already hold the miner lock
    if let Ok(miner_guard) = game.miner.try_lock() {
        crate::audio::set_audio_pan_x(miner_guard.x);
    }

    audio_sfx(SFX_ITEM as i32);
}

// Port of C's ClockTicker; called once per game frame from do_game_ticker.
pub fn clock_ticker() {
    // [ed: porting comments over directly]
    // 256 frames = 1 game minute
    // 19 game hours = 6.75... actual hours (19 * 60 * 256 / 12 / 60 / 60)
    // there's a guy on YouTube that can do it in less than 20m
    //  (2m15s using cheat mode)
    let game = &*GAME_STATE;
    // fetch_add returns the pre-increment value, so `< 256` matches C's
    // `gameClockTicks++ < 256` exactly (a tick fires on the 257th call).
    if game.clock_ticks.fetch_add(1, Ordering::Relaxed) < 256 {
        return;
    }

    game.clock_ticks.store(0, Ordering::Relaxed);

    let mut clock = game.score_clock.lock().unwrap();
    clock[0] += 1;
    if clock[0] == 60 {
        clock[0] = 0;
        clock[1] += 1;
        if clock[1] == 12 {
            clock[2] = 1 - clock[2];
            if (clock[2] == 0) && (game.mode.load(Ordering::Relaxed) < GameMode::Maria as u8) {
                set_action(Some(gameover_action));
            }
        } else if clock[1] == 13 {
            clock[1] = 1;
        }
    }

    set_do_clock_update(Some(do_draw_clock));
}

pub fn game_draw_lives() {
    let game = &*GAME_STATE;
    let lives = game.lives.load(Ordering::Relaxed) as usize;

    for (l, &ink) in LIFE_INK.iter().enumerate().take(lives) {
        let pos = LIVES + l * 16;
        // Upstream a53b948: lives animate with musicClock >> 4, as in the original.
        miner_draw_sprite(pos as i32, 0x0, ink, audio_music_clock() >> 4);
    }
}

pub fn game_draw_status() {
    let game = &*GAME_STATE;

    // video_pixel_paper_fill(128 * WIDTH, 64 * WIDTH, 0x0);
    video_pixel_paper_fill(128 * WIDTH, 64 * WIDTH, 0x0);

    // video_pixel_ink_fill(129 * WIDTH, 8 * WIDTH, 0x6);
    video_pixel_ink_fill(129 * WIDTH, 8 * WIDTH, 0x6);

    // video_write_large(4, STATUS, "\x1\x0\x2\x1" "I" "\x2\x2" "t" "\x2\x3" "e" "\x2\x4" "m" "\x2\x5" "s");
    // Using vec! for the byte string as requested
    let items_label: Vec<u8> = vec![
        0x1, 0x0, 0x2, 0x1, b'I', 0x2, 0x2, b't', 0x2, 0x3, b'e', 0x2, 0x4, b'm', 0x2, 0x5, b's', 0,
    ];
    video_write_large(4, STATUS as i32, &items_label[..]);

    // DrawItems() equivalent
    let score_items = *game.score_items.lock().unwrap();
    let mut items_text: Vec<u8> = vec![0x1, 0x0, 0x2, 0x6, b' ', 0x2, 0x7, b' ', 0];

    items_text[7] = (score_items % 10) + b'0';
    if score_items > 9 {
        items_text[4] = (score_items / 10) + b'0';
    }

    video_write_large(6 * 8 + 4, STATUS as i32, &items_text[..]);

    // do_draw_clock();
    do_draw_clock();

    // C: GameDrawLives();
    game_draw_lives();
}

pub fn game_action() {
    set_responder(Some(do_game_responder));
    // Runs once next frame, inits the room, then installs do_game_ticker and
    // sets Action = do_nothing. Matches C's `Ticker = Game_InitRoom`.
    set_ticker(Some(game_init_room));
    set_drawer(Some(do_game_drawer));
}
