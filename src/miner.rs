// Port of miner.c — Willy (the miner): input handling, jump/fall/walk physics,
// conveyor & ramp handling, collision, item pickup, and sprite rendering.
//
// This is a faithful, behaviour-preserving port. It now operates directly on
// GAME_STATE.miner (the Rust-owned state) rather than a C-ABI global. The
// minerWillyRope field lives in GAME_STATE.miner_willy_rope (atomic).
// Other modules (robots.rs, levels.rs, die.rs, rope.rs, cheat.rs) read miner
// state through GAME_STATE as well.

use crate::audio::{audio_willy_sfx, set_audio_pan_x};
use crate::common::Key;
use crate::die::die_action;
use crate::game::{
    Direction, GAME_STATE, GameMode, Miner, NIGHTMAREROOM, game_change_level, game_got_item,
};
use crate::game_main::{
    set_action, system_is_joy_fire, system_is_joy_left, system_is_joy_right, system_is_key,
};
use crate::levels::{TileType, level_erase_item, level_get_tile_ramp, level_get_tile_type};
use crate::misc::{Timer, timer_set, timer_update};
use crate::video::{video_draw_miner, video_draw_sprite};
use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;

const D_RIGHT: i32 = 0;
const D_LEFT: i32 = 1;

// Conveyor direction (game.h C_NONE/C_LEFT/C_RIGHT).
const C_NONE: i32 = 0;
const C_LEFT: i32 = 1;
const C_RIGHT: i32 = 2;

#[derive(Clone, Copy)]
struct Jump {
    jump: i32,
    tile: i32,
    align: i32,
    // sfx
    length: i32,
    pitch: i32,
}

const fn jmp(jump: i32, tile: i32, align: i32, length: i32, pitch: i32) -> Jump {
    Jump {
        jump,
        tile,
        align,
        length,
        pitch,
    }
}

#[rustfmt::skip]
static MINER_SPRITE: [[u16; 16]; 16] = [
    [15360, 15360, 32256, 13312, 15872, 15360, 6144, 15360, 32256, 32256, 63232, 64256, 15360, 30208, 28160, 30464],
    [3840, 3840, 8064, 3328, 3968, 3840, 1536, 3840, 7040, 7040, 7040, 7552, 3840, 1536, 1536, 1792],
    [960, 960, 2016, 832, 992, 960, 384, 960, 2016, 2016, 3952, 4016, 960, 1888, 1760, 1904],
    [240, 240, 504, 208, 248, 240, 96, 240, 504, 1020, 2046, 1782, 248, 474, 782, 908],
    [3840, 3840, 8064, 2816, 7936, 3840, 1536, 3840, 8064, 16320, 32736, 28512, 7936, 23424, 28864, 12736],
    [960, 960, 2016, 704, 1984, 960, 384, 960, 2016, 2016, 3824, 3568, 960, 1760, 1888, 3808],
    [240, 240, 504, 176, 496, 240, 96, 240, 472, 472, 472, 440, 240, 96, 96, 224],
    [60, 60, 126, 44, 124, 60, 24, 60, 126, 126, 239, 223, 60, 110, 118, 238],
    [32768, 20480, 43008, 20480, 43008, 54528, 27136, 55040, 43648, 55232, 65472, 32256, 17408, 17408, 0, 0],
    [0, 0, 0, 0, 0, 11328, 7808, 16320, 10912, 22000, 11248, 24448, 4352, 8320, 0, 0],
    [0, 0, 0, 0, 0, 2832, 1952, 4080, 3432, 2748, 5500, 2784, 5440, 10816, 5120, 10240],
    [0, 0, 0, 0, 0, 712, 488, 1020, 682, 1375, 703, 1528, 272, 160, 0, 0],
    [0, 0, 0, 0, 0, 4928, 6016, 16320, 21824, 64160, 64832, 8096, 2176, 1280, 0, 0],
    [0, 0, 0, 0, 0, 2256, 1504, 4080, 5808, 15696, 16040, 1872, 680, 596, 40, 20],
    [0, 0, 0, 0, 0, 564, 376, 1020, 1364, 4010, 4052, 506, 136, 260, 0, 0],
    [3, 10, 21, 10, 21, 171, 86, 235, 341, 1003, 1023, 126, 34, 34, 0, 0],
];

#[rustfmt::skip]
static JUMP_INFO: [Jump; 18] = [
    jmp(-4, -32, 6, 5, 72),
    jmp(-4, 0, 4, 5, 74),
    jmp(-3, -32, 6, 4, 76),
    jmp(-3, 0, 6, 4, 78),
    jmp(-2, 0, 4, 3, 80),
    jmp(-2, -32, 6, 3, 82),
    jmp(-1, 0, 6, 2, 84),
    jmp(-1, 0, 6, 2, 86),
    jmp(0, 0, 6, 1, 88),
    jmp(0, 0, 6, 1, 88),
    jmp(1, 0, 6, 2, 86),
    jmp(1, 0, 6, 2, 84),
    jmp(2, 32, 4, 3, 82),
    jmp(2, 0, 6, 3, 80),
    jmp(3, 0, 6, 4, 78),
    jmp(3, 32, 4, 4, 76),
    jmp(4, 0, 6, 5, 74),
    jmp(4, 32, 4, 5, 72),
];

static MINER_SEQUENCE: [usize; 8] = [0, 1, 2, 3, 7, 6, 5, 4];

const MINER_ZERO: Miner = Miner {
    x: 0,
    y: 0,
    tile: 0,
    align: 0,
    frame: 0,
    dir: D_RIGHT,
    move_: 0,
    air: 0,
    jump: 0,
};

// File-local miner state (private; thread-local, no static mut needed).
thread_local! {
    static MINER_FRAME: Cell<usize> = const { Cell::new(0) }; // base row into MINER_SPRITE (0 or 8)
    static MINER_SEQ_INDEX: Cell<u8> = const { Cell::new(0) };
    static MINER_TIMER: RefCell<Timer> = const { RefCell::new(Timer {
        rate: 0,
        acc: 0,
        remainder: 0,
        divisor: 0,
    }) };
}

fn miner_frame() -> usize {
    MINER_FRAME.with(Cell::get)
}
fn set_miner_frame(v: usize) {
    MINER_FRAME.with(|c| c.set(v));
}
fn miner_seq_index() -> u8 {
    MINER_SEQ_INDEX.with(Cell::get)
}
fn set_miner_seq_index(v: u8) {
    MINER_SEQ_INDEX.with(|c| c.set(v));
}

// YALIGN macro from video.h.
const fn yalign(y: i32) -> i32 {
    4 | ((y & 4) >> 1) | (y & 2) | ((y & 1) << 1)
}

pub fn miner_set_seq(index: i32, speed: i32) {
    MINER_TIMER.with(|t| timer_set(&mut t.borrow_mut(), 1, speed));
    set_miner_seq_index(index as u8);
}

pub fn miner_inc_seq() {
    let next = MINER_TIMER.with(|t| timer_update(&mut t.borrow_mut())) as u8;
    set_miner_seq_index(miner_seq_index().wrapping_add(next) & 7);
}

// Upstream Miner_DrawSprite: minerSequence[index & 7].
pub fn miner_draw_sprite(pos: i32, paper: u8, ink: u8, index: i32) {
    let row = MINER_SEQUENCE[(index & 7) as usize];
    video_draw_sprite(pos, &MINER_SPRITE[row], paper, ink);
}

// MINER_STORE is used for save/restore of miner state across room transitions.
thread_local! {
    static MINER_STORE: RefCell<Miner> = const { RefCell::new(MINER_ZERO) };
}

pub fn miner_restore() {
    let mut miner = GAME_STATE.miner.lock().unwrap();
    MINER_STORE.with(|s| {
        let s = s.borrow();
        miner.x = s.x;
        miner.y = s.y;
        miner.tile = s.tile;
        miner.align = s.align;
        miner.frame = s.frame;
        miner.dir = s.dir;
        miner.move_ = s.move_;
        miner.air = s.air;
        miner.jump = s.jump;
    });
    GAME_STATE.miner_air.store(miner.air, Ordering::Relaxed);
    GAME_STATE.miner_jump.store(miner.jump, Ordering::Relaxed);
}

pub fn miner_save() {
    let miner = GAME_STATE.miner.lock().unwrap();
    MINER_STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.x = miner.x;
        s.y = miner.y;
        s.tile = miner.tile;
        s.align = miner.align;
        s.frame = miner.frame;
        s.dir = miner.dir;
        s.move_ = miner.move_;
        s.air = miner.air;
        s.jump = miner.jump;
    });

    set_miner_frame(
        if GAME_STATE.level.load(Ordering::Relaxed) == NIGHTMAREROOM {
            8
        } else {
            0
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolidOutcome {
    Solid,
    Clear,
    // Solid two rows down mid-descent (air == 1, jump > 9): the jump
    // arc ends here; the wrapper applies the air = 0 reset.
    Land,
}

fn solid_check(tile: i32, lookup: &dyn Fn(usize) -> TileType, miner: &Miner) -> SolidOutcome {
    if tile < 0 || tile == 512 {
        return SolidOutcome::Clear;
    }

    if lookup(tile as usize) == TileType::Solid {
        return SolidOutcome::Solid;
    }

    if lookup((tile + 32) as usize) == TileType::Solid {
        return SolidOutcome::Solid;
    }

    if tile + 64 > 511 {
        return SolidOutcome::Clear;
    }

    if lookup((tile + 64) as usize) != TileType::Solid {
        return SolidOutcome::Clear;
    }

    if miner.align == 6 {
        return SolidOutcome::Solid;
    }

    if miner.air == 1 && miner.jump > 9 {
        return SolidOutcome::Land;
    }

    SolidOutcome::Clear
}

fn is_solid(tile: i32, miner: &mut Miner) -> bool {
    match solid_check(tile, &level_get_tile_type, miner) {
        SolidOutcome::Solid => true,
        SolidOutcome::Clear => false,
        SolidOutcome::Land => {
            // the original C mutates air inside the probe; keeping the
            // mutation beside its atomic store makes the core pure
            miner.air = 0;
            GAME_STATE.miner_air.store(0, Ordering::Relaxed);
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JumpOutcome {
    Above,
    BumpHead {
        y: i32,
        tile: i32,
    },
    Rise {
        y: i32,
        tile: i32,
        align: i32,
        pitch: i32,
        length: i32,
        apex: bool,
    },
}

// Pure core of the air == 1 (jumping) branch of do_miner_ticker.
fn jump_step(
    jump_idx: usize,
    y: i32,
    tile: i32,
    lookup: &dyn Fn(usize) -> TileType,
) -> JumpOutcome {
    let j = JUMP_INFO[jump_idx];
    let ny = y + j.jump;
    if ny < 0 {
        return JumpOutcome::Above;
    }
    let nt = tile + j.tile;
    if lookup(nt as usize) == TileType::Solid || lookup((nt + 1) as usize) == TileType::Solid {
        return JumpOutcome::BumpHead {
            y: (ny + 8) & 120,
            tile: nt + 32,
        };
    }
    JumpOutcome::Rise {
        y: ny,
        tile: nt,
        align: j.align,
        pitch: j.pitch,
        length: j.length,
        apex: jump_idx + 1 == 18,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroundOutcome {
    Below,
    HarmPass,
    Die,
    Land { convey_dir: i32 },
    Fall,
}

// Pure core of the align == 4 (feet on tile boundary) branch of
// do_miner_ticker: what is below Willy, and what happens this tick.
fn ground_check(tile_base: i32, air: i32, lookup: &dyn Fn(usize) -> TileType) -> GroundOutcome {
    let tile = tile_base + 64;
    if tile & 512 != 0 {
        return GroundOutcome::Below;
    }
    let t0 = lookup(tile as usize);
    let t1 = lookup((tile + 1) as usize);
    if t0 == TileType::Harm || t1 == TileType::Harm {
        if air == 1 && (t0 as i32 <= TileType::Space as i32 || t1 as i32 <= TileType::Space as i32)
        {
            return GroundOutcome::HarmPass;
        }
        return GroundOutcome::Die;
    }
    if t0 as i32 > TileType::Space as i32 || t1 as i32 > TileType::Space as i32 {
        if air >= 12 {
            return GroundOutcome::Die;
        }
        let convey_dir = if t0 == TileType::ConveyL || t1 == TileType::ConveyL {
            C_LEFT
        } else if t0 == TileType::ConveyR || t1 == TileType::ConveyR {
            C_RIGHT
        } else {
            C_NONE
        };
        return GroundOutcome::Land { convey_dir };
    }
    GroundOutcome::Fall
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FallStep {
    air: i32,
    y: i32,
    align: i32,
    tile_add: i32,
    pitch: i32,
}

// Pure core of the falling tail of do_miner_ticker.
fn fall_step(air: i32, y: i32) -> FallStep {
    let mut air = air + 1;
    if air == 16 {
        // this affects the falling sound effect
        air = 12;
    }
    let y = y + 4;
    let (align, tile_add) = if y & 7 != 0 { (6, 0) } else { (4, 32) };
    FallStep {
        air,
        y,
        align,
        tile_add,
        pitch: 78 - air,
    }
}

fn move_left_right() {
    let mut miner = GAME_STATE.miner.lock().unwrap();
    let mut y = 0;
    let mut offset = 0;

    if miner.move_ == 0 {
        return;
    }

    if GAME_STATE.miner_willy_rope.load(Ordering::Relaxed) > 0 {
        return;
    }

    if miner.dir == D_RIGHT {
        if miner.frame < 3 {
            miner.frame += 1;
            return;
        }

        if miner.air == 0 {
            if level_get_tile_ramp((miner.tile + 64) as usize) == TileType::RampL {
                y = 8;
                offset = 32;
            } else if level_get_tile_ramp((miner.tile + 34) as usize) == TileType::RampR {
                y = -8;
                offset = -32;
            }
        }

        if miner.x == 30 * 8 {
            drop(miner);
            game_change_level(Direction::Right as i32);
            return;
        }

        if is_solid(miner.tile + offset + 2, &mut miner) {
            return;
        }

        miner.x += 8;
        miner.tile += 1;
        miner.frame = 0;
    } else if GAME_STATE.mode.load(Ordering::Relaxed) != GameMode::Running as u8 {
        if miner.frame > 0 {
            miner.frame -= 1;
            return;
        }

        if miner.air == 0 {
            if level_get_tile_ramp((miner.tile + 31) as usize) == TileType::RampL {
                y = -8;
                offset = -32;
            } else if level_get_tile_ramp((miner.tile + 65) as usize) == TileType::RampR {
                y = 8;
                offset = 32;
            }
        }

        if miner.x == 0 {
            drop(miner);
            game_change_level(Direction::Left as i32);
            return;
        }

        if is_solid(miner.tile + offset - 1, &mut miner) {
            return;
        }

        miner.x -= 8;
        miner.tile -= 1;
        miner.frame = 3;
    }

    // Apply vertical movement from ramps. Upstream 941841d: no early
    // change-level here when y + y would go negative — Miner_Ticker's
    // `y < 0` check catches it on the next pass, as in the original.
    miner.y += y;
    miner.tile += offset;
}

fn update_dir(convey_dir: i32) {
    let mut miner = GAME_STATE.miner.lock().unwrap();
    let mut dir = 0;

    // Upstream 69070b9: joystick left/right/fire feed the same inputs as the keys.
    if (system_is_key(Key::Left as i32) || system_is_joy_left() || convey_dir == C_LEFT)
        && GAME_STATE.mode.load(Ordering::Relaxed) < GameMode::Running as u8
    {
        dir += 1;
    }

    if system_is_key(Key::Right as i32)
        || system_is_joy_right()
        || convey_dir == C_RIGHT
        || GAME_STATE.mode.load(Ordering::Relaxed) == GameMode::Running as u8
    {
        dir += 2;
    }

    if dir == 0 {
        miner.move_ = 0;
    } else if dir == 1 {
        if miner.dir == D_RIGHT {
            miner.dir = D_LEFT;
            miner.move_ = 0;
        } else {
            miner.move_ = 1;
        }
    } else if dir == 2 {
        if miner.dir == D_LEFT {
            miner.dir = D_RIGHT;
            miner.move_ = 0;
        } else {
            miner.move_ = 1;
        }
    }

    if (system_is_key(Key::Jump as i32) || system_is_joy_fire())
        && GAME_STATE.mode.load(Ordering::Relaxed) < GameMode::Running as u8
    {
        miner.air = 1;
        miner.jump = 0;
        GAME_STATE.miner_air.store(1, Ordering::Relaxed);
        GAME_STATE.miner_jump.store(0, Ordering::Relaxed);
        if GAME_STATE.miner_willy_rope.load(Ordering::Relaxed) > 0 {
            GAME_STATE.miner_willy_rope.store(-16, Ordering::Relaxed);
            miner.y &= 120;
            miner.align = 4;
            miner.move_ = 1;
        }
    }
}

fn do_miner_ticker() {
    let mut miner = GAME_STATE.miner.lock().unwrap();
    let convey_dir = C_NONE;

    if GAME_STATE.miner_willy_rope.load(Ordering::Relaxed) > 0 {
        drop(miner);
        update_dir(convey_dir);
        return;
    }

    if miner.air == 1 {
        match jump_step(
            miner.jump as usize,
            miner.y,
            miner.tile,
            &level_get_tile_type,
        ) {
            JumpOutcome::Above => {
                drop(miner);
                game_change_level(Direction::Above as i32);
                return;
            }
            JumpOutcome::BumpHead { y, tile } => {
                // we need to re-align Willy
                miner.y = y;
                miner.tile = tile;
                miner.align = 4;
                miner.air = 2;
                miner.move_ = 0;
                GAME_STATE.miner_air.store(2, Ordering::Relaxed);
                return;
            }
            JumpOutcome::Rise {
                y,
                tile,
                align,
                pitch,
                length,
                apex,
            } => {
                set_audio_pan_x(miner.x);
                audio_willy_sfx(pitch, length);

                miner.y = y;
                miner.tile = tile;
                miner.align = align;
                miner.jump += 1;
                GAME_STATE.miner_jump.store(miner.jump, Ordering::Relaxed);

                if apex {
                    miner.air = 6;
                    GAME_STATE.miner_air.store(6, Ordering::Relaxed);
                    return;
                }

                if miner.jump != 13 && miner.jump != 16 {
                    drop(miner);
                    move_left_right();
                    return;
                }
            }
        }
    }

    if miner.align == 4 {
        match ground_check(miner.tile, miner.air, &level_get_tile_type) {
            GroundOutcome::Below => {
                drop(miner);
                game_change_level(Direction::Below as i32);
                return;
            }
            GroundOutcome::HarmPass => {
                drop(miner);
                move_left_right();
                return;
            }
            GroundOutcome::Die => {
                set_action(Some(die_action));
                return;
            }
            GroundOutcome::Land { convey_dir } => {
                miner.air = 0;
                GAME_STATE.miner_air.store(0, Ordering::Relaxed);
                drop(miner);
                update_dir(convey_dir);
                move_left_right();
                return;
            }
            GroundOutcome::Fall => {}
        }
    }

    if miner.air == 1 {
        drop(miner);
        move_left_right();
        return;
    }

    miner.move_ = 0;
    if miner.air == 0 {
        miner.air = 2;
        GAME_STATE.miner_air.store(2, Ordering::Relaxed);
        return;
    }

    let step = fall_step(miner.air, miner.y);
    miner.air = step.air;
    GAME_STATE.miner_air.store(step.air, Ordering::Relaxed);
    set_audio_pan_x(miner.x);
    audio_willy_sfx(step.pitch, 4);
    miner.y = step.y;
    miner.align = step.align;
    miner.tile += step.tile_add;
}

pub fn miner_ticker() {
    do_miner_ticker();

    let miner = GAME_STATE.miner.lock().unwrap();
    if miner.y < 0 {
        drop(miner);
        game_change_level(Direction::Above as i32);
    }
}

pub fn miner_drawer() {
    let miner = GAME_STATE.miner.lock().unwrap();
    let mut offset = 0;
    let mut align = miner.align;

    if miner.air == 0 {
        if level_get_tile_ramp((miner.tile + 64) as usize) == TileType::RampL {
            offset = miner.frame << 1;
            align = yalign(offset);
        } else if level_get_tile_ramp((miner.tile + 65) as usize) == TileType::RampR {
            offset = 6 - (miner.frame << 1);
            align = yalign(offset);
        }
    }

    let row = miner_frame() + ((miner.dir << 2) | miner.frame) as usize;
    if video_draw_miner(
        ((miner.y + offset) << 8) | miner.x,
        &MINER_SPRITE[row],
        GAME_STATE.miner_attr_split.load(Ordering::Relaxed),
    ) != 0
    {
        set_action(Some(die_action));
        return;
    }

    let mut tile = miner.tile;
    let mut adj = 1;
    for _ in 0..align {
        if level_get_tile_type(tile as usize) == TileType::Harm {
            set_action(Some(die_action));
            return;
        }
        tile += adj;
        adj ^= 30;
    }

    let mut tile = miner.tile;
    let mut adj = 1;
    for _ in 0..align {
        if level_get_tile_type(tile as usize) == TileType::Item {
            level_erase_item(tile as usize);
            game_got_item();
        }
        tile += adj;
        adj ^= 30;
    }
}

pub fn miner_init() {
    let mut miner = GAME_STATE.miner.lock().unwrap();
    miner.x = 20 * 8;
    miner.y = 13 * 8;
    miner.tile = 13 * 32 + 20;
    miner.align = 4;
    miner.move_ = 0;
    miner.air = 0;
    GAME_STATE.miner_air.store(0, Ordering::Relaxed);
    drop(miner);

    miner_save();
}

#[cfg(test)]
mod tests {
    use super::*;

    // Stub for the real level_get_tile_type: all Space, with the same
    // out-of-bounds -> Space behaviour.
    fn lookup_of(tiles: &[TileType; 512]) -> impl Fn(usize) -> TileType + '_ {
        move |i| {
            if i >= 512 { TileType::Space } else { tiles[i] }
        }
    }

    fn miner_with(align: i32, air: i32, jump: i32) -> Miner {
        Miner {
            align,
            air,
            jump,
            ..Default::default()
        }
    }

    #[test]
    fn out_of_range_tiles_are_not_solid() {
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        let m = Miner::default();
        assert_eq!(solid_check(-1, &lookup, &m), SolidOutcome::Clear);
        assert_eq!(solid_check(512, &lookup, &m), SolidOutcome::Clear);
    }

    #[test]
    fn solid_at_tile_or_one_row_down_blocks() {
        let mut tiles = [TileType::Space; 512];
        tiles[100] = TileType::Solid;
        let lookup = lookup_of(&tiles);
        let m = miner_with(4, 0, 0);
        assert_eq!(solid_check(100, &lookup, &m), SolidOutcome::Solid);

        let mut tiles = [TileType::Space; 512];
        tiles[132] = TileType::Solid; // tile + 32
        let lookup = lookup_of(&tiles);
        let m = miner_with(4, 0, 0);
        assert_eq!(solid_check(100, &lookup, &m), SolidOutcome::Solid);
    }

    #[test]
    fn past_bottom_edge_is_never_solid() {
        // tile + 64 > 511 returns Clear before the align check
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        let m = miner_with(6, 0, 0);
        assert_eq!(solid_check(448, &lookup, &m), SolidOutcome::Clear);

        let mut tiles = [TileType::Space; 512];
        tiles[448] = TileType::Solid;
        let lookup = lookup_of(&tiles);
        let m = miner_with(4, 0, 0);
        assert_eq!(solid_check(448, &lookup, &m), SolidOutcome::Solid);

        let mut tiles = [TileType::Space; 512];
        tiles[480] = TileType::Solid; // tile + 32
        let lookup = lookup_of(&tiles);
        let m = miner_with(4, 0, 0);
        assert_eq!(solid_check(448, &lookup, &m), SolidOutcome::Solid);
    }

    #[test]
    fn solid_two_rows_down_only_blocks_when_aligned_or_landing() {
        let mut tiles = [TileType::Space; 512];
        tiles[164] = TileType::Solid; // tile + 64 for tile = 100
        let lookup = lookup_of(&tiles);

        let m = miner_with(4, 0, 0);
        assert_eq!(solid_check(100, &lookup, &m), SolidOutcome::Clear);

        let m = miner_with(6, 0, 0);
        assert_eq!(solid_check(100, &lookup, &m), SolidOutcome::Solid);

        // mid-jump past jump 9: the jump arc ends here
        let m = miner_with(4, 1, 10);
        assert_eq!(solid_check(100, &lookup, &m), SolidOutcome::Land);

        // jump <= 9 does not trigger it
        let m = miner_with(4, 1, 9);
        assert_eq!(solid_check(100, &lookup, &m), SolidOutcome::Clear);
    }

    #[test]
    fn jump_step_rises_using_jump_table() {
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        // JUMP_INFO[0] = jmp(-4, -32, 6, 5, 72)
        assert_eq!(
            jump_step(0, 10, 100, &lookup),
            JumpOutcome::Rise {
                y: 6,
                tile: 68,
                align: 6,
                pitch: 72,
                length: 5,
                apex: false,
            }
        );
    }

    #[test]
    fn jump_step_above_when_y_goes_negative() {
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        assert_eq!(jump_step(0, 2, 100, &lookup), JumpOutcome::Above);
    }

    #[test]
    fn jump_step_bumps_head_on_solid() {
        let mut tiles = [TileType::Space; 512];
        tiles[100] = TileType::Solid;
        let lookup = lookup_of(&tiles);
        // JUMP_INFO[1] = jmp(-4, 0, 4, 5, 74): ny = 96, nt = 100
        assert_eq!(
            jump_step(1, 100, 100, &lookup),
            JumpOutcome::BumpHead { y: 104, tile: 132 }
        );

        // solid only at nt + 1 also bumps
        let mut tiles = [TileType::Space; 512];
        tiles[101] = TileType::Solid;
        let lookup = lookup_of(&tiles);
        assert_eq!(
            jump_step(1, 100, 100, &lookup),
            JumpOutcome::BumpHead { y: 104, tile: 132 }
        );
    }

    #[test]
    fn jump_step_apex_on_last_table_entry() {
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        // JUMP_INFO[17] = jmp(4, 32, 4, 5, 72)
        assert_eq!(
            jump_step(17, 100, 100, &lookup),
            JumpOutcome::Rise {
                y: 104,
                tile: 132,
                align: 4,
                pitch: 72,
                length: 5,
                apex: true,
            }
        );
    }

    // ground_check with tile_base = 100 inspects tiles 164 and 165.
    #[test]
    fn ground_below_when_tile_row_wraps() {
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        assert_eq!(ground_check(448, 0, &lookup), GroundOutcome::Below);
    }

    #[test]
    fn ground_nothing_below_falls() {
        let tiles = [TileType::Space; 512];
        let lookup = lookup_of(&tiles);
        assert_eq!(ground_check(100, 0, &lookup), GroundOutcome::Fall);

        // an item does not stop a fall (Item < Space in the enum order)
        let mut tiles = [TileType::Space; 512];
        tiles[164] = TileType::Item;
        let lookup = lookup_of(&tiles);
        assert_eq!(ground_check(100, 0, &lookup), GroundOutcome::Fall);
    }

    #[test]
    fn ground_harm_kills_except_mid_jump_pass() {
        let mut tiles = [TileType::Space; 512];
        tiles[164] = TileType::Harm;
        let lookup = lookup_of(&tiles);
        assert_eq!(ground_check(100, 0, &lookup), GroundOutcome::Die);
        // air == 1 and the other tile is Space: Willy passes through mid-jump
        assert_eq!(ground_check(100, 1, &lookup), GroundOutcome::HarmPass);

        // the other tile solid-ish: still dies
        let mut tiles = [TileType::Space; 512];
        tiles[164] = TileType::Harm;
        tiles[165] = TileType::Solid;
        let lookup = lookup_of(&tiles);
        assert_eq!(ground_check(100, 1, &lookup), GroundOutcome::Die);

        // harm on the second tile only
        let mut tiles = [TileType::Space; 512];
        tiles[165] = TileType::Harm;
        let lookup = lookup_of(&tiles);
        assert_eq!(ground_check(100, 0, &lookup), GroundOutcome::Die);
    }

    #[test]
    fn ground_landing_detects_fall_death_and_conveyors() {
        let mut tiles = [TileType::Space; 512];
        tiles[164] = TileType::Solid;
        let lookup = lookup_of(&tiles);
        assert_eq!(
            ground_check(100, 11, &lookup),
            GroundOutcome::Land { convey_dir: C_NONE }
        );
        // hard fall (air >= 12) onto something solid-ish kills
        assert_eq!(ground_check(100, 12, &lookup), GroundOutcome::Die);

        let mut tiles = [TileType::Space; 512];
        tiles[164] = TileType::ConveyR;
        let lookup = lookup_of(&tiles);
        assert_eq!(
            ground_check(100, 5, &lookup),
            GroundOutcome::Land {
                convey_dir: C_RIGHT
            }
        );

        let mut tiles = [TileType::Space; 512];
        tiles[165] = TileType::ConveyL;
        let lookup = lookup_of(&tiles);
        assert_eq!(
            ground_check(100, 5, &lookup),
            GroundOutcome::Land { convey_dir: C_LEFT }
        );
    }

    #[test]
    fn fall_step_moves_down_and_tracks_alignment() {
        // y lands on an 8px boundary: full tile step, align 4
        assert_eq!(
            fall_step(2, 100),
            FallStep {
                air: 3,
                y: 104,
                align: 4,
                tile_add: 32,
                pitch: 75,
            }
        );
        // mid-tile: align 6, no tile advance
        assert_eq!(
            fall_step(11, 96),
            FallStep {
                air: 12,
                y: 100,
                align: 6,
                tile_add: 0,
                pitch: 66,
            }
        );
    }

    #[test]
    fn fall_step_caps_air_at_16_to_12() {
        // air 15 -> 16 would keep rising; the engine clamps to 12 (falling sfx)
        assert_eq!(
            fall_step(15, 103),
            FallStep {
                air: 12,
                y: 107,
                align: 6,
                tile_add: 0,
                pitch: 66,
            }
        );
    }
}
