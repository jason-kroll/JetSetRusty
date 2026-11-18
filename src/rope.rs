/// rope.rs - Modernized version with proper FFI safety
use crate::common::WIDTH;
use crate::game::{COLDSTORE, Direction, ONTHEROOF, QUIRKAFLEEG, SWIMMINGPOOL, THEBEACH};
use crate::video::{
    VIDEO_PIXEL, buf_index, video_draw_rope_seg, video_draw_rope_seg_inner, video_get_pixel,
};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};

const ROPE_SEGS: i32 = 33;

// ----------------------------------------------------------------------------
// Rope animation data (immutable, no unsafe needed)
// ----------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct RopeData {
    x: i32,
    y: i32,
}

impl RopeData {
    const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

static ROPE_DATA: [RopeData; 86] = [
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(0, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(1, 3),
    RopeData::new(2, 3),
    RopeData::new(2, 3),
    RopeData::new(2, 3),
    RopeData::new(2, 3),
    RopeData::new(2, 2),
    RopeData::new(2, 3),
    RopeData::new(2, 3),
    RopeData::new(2, 2),
    RopeData::new(2, 3),
    RopeData::new(2, 2),
    RopeData::new(2, 3),
    RopeData::new(2, 2),
    RopeData::new(2, 3),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 3),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(1, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(1, 2),
    RopeData::new(1, 2),
    RopeData::new(2, 2),
    RopeData::new(1, 2),
    RopeData::new(1, 2),
    RopeData::new(2, 2),
    RopeData::new(2, 2),
    RopeData::new(3, 2),
    RopeData::new(2, 2),
    RopeData::new(3, 2),
    RopeData::new(2, 2),
    RopeData::new(3, 2),
    RopeData::new(3, 2),
    RopeData::new(3, 2),
    RopeData::new(3, 2),
    RopeData::new(3, 2),
    RopeData::new(3, 2),
];

static ROPE_MOVE: [i32; 2] = [-1, 1];

// ----------------------------------------------------------------------------
// Rope state - using Atomic types for thread-safe access
// ----------------------------------------------------------------------------

struct RopeState {
    dir: AtomicI32,
    pos: AtomicI32,
    hold: AtomicBool,
    x: AtomicI32,
    side: AtomicI32,
    ink: AtomicU8,
}

impl Default for RopeState {
    fn default() -> Self {
        Self {
            dir: AtomicI32::new(0),
            pos: AtomicI32::new(0),
            hold: AtomicBool::new(false),
            x: AtomicI32::new(0),
            side: AtomicI32::new(0),
            ink: AtomicU8::new(0),
        }
    }
}

static ROPE: RopeState = RopeState {
    dir: AtomicI32::new(0),
    pos: AtomicI32::new(0),
    hold: AtomicBool::new(false),
    x: AtomicI32::new(0),
    side: AtomicI32::new(0),
    ink: AtomicU8::new(0),
};

// Whether the current room has a rope, set by rope_init per room. The old
// ROPE_TICKER/ROPE_DRAWER fn-pointer slots collapse into this one flag:
// when set, rope_tick/rope_draw run the rope handlers; when clear they are
// no-ops (the old do_nothing slot values).
thread_local! {
    static ROPE_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

fn set_rope_active(active: bool) {
    ROPE_ACTIVE.with(|c| c.set(active));
}

/// Tick the rope if the current room has one (called from the game ticker).
pub fn rope_tick() {
    if ROPE_ACTIVE.with(Cell::get) {
        do_rope_ticker();
    }
}

/// Draw the rope if the current room has one (called from the game drawer).
pub fn rope_draw() {
    if ROPE_ACTIVE.with(Cell::get) {
        do_rope_drawer();
    }
}

// ----------------------------------------------------------------------------
// Extern declarations - the real C globals shared with levels.rs and the C code
// ----------------------------------------------------------------------------

use crate::game::GAME_STATE;

// Level constants
const B_WILLY: i32 = 4;

// ----------------------------------------------------------------------------
// Internal implementation
// ----------------------------------------------------------------------------

fn do_rope_drawer() {
    let data_idx = ROPE.pos.load(Ordering::Relaxed) as usize;
    let ink = ROPE.ink.load(Ordering::Relaxed);

    let mut x = ROPE.x.load(Ordering::Relaxed) * 8;
    let mut y: i32 = 0;

    video_draw_rope_seg(x, ink);

    if ROPE.pos.load(Ordering::Relaxed) == 0 {
        ROPE.side
            .store(ROPE.side.load(Ordering::Relaxed) ^ 1, Ordering::Relaxed);
    }

    let mut pixels = VIDEO_PIXEL.lock().unwrap();
    for (seg, data) in (1..ROPE_SEGS).zip(ROPE_DATA.iter().skip(data_idx)) {
        y += data.y;
        x -= data.x * ROPE_MOVE[ROPE.side.load(Ordering::Relaxed) as usize];

        let pos = y * WIDTH + x;

        // Check for Willy collision
        let pixel_val = video_get_pixel(&pixels[buf_index(pos)]);
        let willy_rope_zero = GAME_STATE.miner_willy_rope.load(Ordering::Relaxed) == 0;
        if willy_rope_zero && (pixel_val & B_WILLY) != 0 {
            GAME_STATE.miner_willy_rope.store(seg, Ordering::Relaxed);
            ROPE.hold.store(true, Ordering::Relaxed);
        }

        // Handle Willy position if holding rope
        let willy_on_rope = GAME_STATE.miner_willy_rope.load(Ordering::Relaxed) == seg;
        if willy_on_rope && ROPE.hold.load(Ordering::Relaxed) {
            let willy_x = x & 248;
            let willy_y = y - 8;

            let frame = if (x & 6) == 6 {
                1
            } else if (x & 4) != 0 {
                0
            } else {
                if (x & 2) != 0 { 3 } else { 2 }
            };

            let mut miner = GAME_STATE.miner.lock().unwrap();
            miner.x = if frame < 2 { willy_x } else { willy_x - 8 };
            miner.y = willy_y;
            miner.frame = frame;
            miner.tile = miner.y / 8 * 32 + miner.x / 8;
            miner.align = 4;
        }

        video_draw_rope_seg_inner(&mut pixels, pos, ink);
    }

    // Handle negative minerWillyRope
    if GAME_STATE.miner_willy_rope.load(Ordering::Relaxed) < 0 {
        GAME_STATE.miner_willy_rope.fetch_add(1, Ordering::Relaxed);
        ROPE.hold.store(false, Ordering::Relaxed);
        return;
    }

    // Handle rope movement when holding
    if ROPE.hold.load(Ordering::Relaxed) {
        let miner = GAME_STATE.miner.lock().unwrap();
        let willy_moving = miner.move_ != 0;
        drop(miner);
        if willy_moving {
            let dir = ROPE.dir.load(Ordering::Relaxed);
            let miner = GAME_STATE.miner.lock().unwrap();
            let willy_dir = miner.dir;
            drop(miner);
            let current_rope = GAME_STATE.miner_willy_rope.load(Ordering::Relaxed);
            let seg = current_rope + ROPE_MOVE[(dir ^ willy_dir) as usize];

            let level_dir = level_dir(Direction::Above);
            let adjusted_seg = if level_dir == 0 && seg < 15 { 15 } else { seg };

            if adjusted_seg < ROPE_SEGS {
                GAME_STATE
                    .miner_willy_rope
                    .store(adjusted_seg, Ordering::Relaxed);
                return;
            }

            let mut miner = GAME_STATE.miner.lock().unwrap();
            GAME_STATE.miner_willy_rope.store(-16, Ordering::Relaxed);
            miner.y &= 124;
            miner.air = 0;
        }
    }
}

fn do_rope_ticker() {
    let dir = ROPE.dir.load(Ordering::Relaxed);
    let side = ROPE.side.load(Ordering::Relaxed);
    let step = ROPE_MOVE[(dir ^ side) as usize] * 2;

    let new_pos = ROPE.pos.load(Ordering::Relaxed) + step;
    ROPE.pos.store(new_pos, Ordering::Relaxed);

    if new_pos < 16 {
        ROPE.pos.store(new_pos + step, Ordering::Relaxed);
    } else if new_pos == 54 {
        ROPE.dir.store(dir ^ 1, Ordering::Relaxed);
    }
}

pub fn rope_init() {
    let level = GAME_STATE.level.load(Ordering::Relaxed);
    let (x, ink) = match level {
        QUIRKAFLEEG => (16, 6),
        ONTHEROOF => (16, 4),
        COLDSTORE => (16, 6),
        SWIMMINGPOOL => (16, 7),
        THEBEACH => (14, 5),
        _ => {
            set_rope_active(false);
            return;
        }
    };

    ROPE.x.store(x, Ordering::Relaxed);
    ROPE.ink.store(ink, Ordering::Relaxed);
    ROPE.dir.store(0, Ordering::Relaxed);
    ROPE.pos.store(34, Ordering::Relaxed);
    ROPE.side.store(0, Ordering::Relaxed);
    ROPE.hold.store(false, Ordering::Relaxed);

    set_rope_active(true);
}

use crate::levels::level_dir;
