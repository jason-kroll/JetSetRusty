use crate::common::{Event, Key};
use crate::game::{FIRSTLANDING, GAME_STATE, game_cheat_enabled, game_init_room, game_pause};
use crate::game_main::{game_input, set_action, system_is_key};
use std::sync::atomic::{AtomicUsize, Ordering};

static CHEAT_CODE: &[u8] = b"writetyper";

// miner.y on First Landing's floor — the only spot where the code is read
const FIRSTLANDING_FLOOR_Y: i32 = 104;

pub fn cheat_responder() -> Event {
    if GAME_STATE.cheat_enabled.load(Ordering::Relaxed) {
        Some(cheat_enabled)
    } else {
        Some(cheat_disabled)
    }
}

pub fn cheat_enabled() {
    let mut level: i32 = 0;

    for i in 0..30 {
        if system_is_key(Key::K1 as i32 + i) != 0 {
            level = i + 1;
            break;
        }
    }

    if system_is_key(Key::Enter as i32) == 0 {
        game_pause(false);
        return;
    }

    if level == 0 {
        return;
    }

    if system_is_key(Key::LShift as i32) != 0 || system_is_key(Key::RShift as i32) != 0 {
        level += 30;
    }

    level -= 1;
    if level == GAME_STATE.level.load(Ordering::Relaxed) {
        return;
    }

    GAME_STATE.level.store(level, Ordering::Relaxed);

    set_action(Some(game_init_room));
}

pub fn cheat_disabled() {
    static CHEAT_POS: AtomicUsize = AtomicUsize::new(0);

    let miner = GAME_STATE.miner.lock().unwrap();
    let pos = CHEAT_POS.load(Ordering::Relaxed);
    // Letter keys arrive as Key::A + offset; map the input back to ASCII to
    // compare against the code word. The bounds check keeps the index in
    // range even if the code has already been fully entered.
    if GAME_STATE.level.load(Ordering::Relaxed) != FIRSTLANDING
        || miner.y != FIRSTLANDING_FLOOR_Y
        || pos >= CHEAT_CODE.len()
        || CHEAT_CODE[pos] != (game_input() - Key::A as i32 + b'a' as i32) as u8
    {
        game_pause(false);
        return;
    }

    CHEAT_POS.fetch_add(1, Ordering::Relaxed);

    if CHEAT_POS.load(Ordering::Relaxed) < CHEAT_CODE.len() {
        return;
    }

    game_cheat_enabled();
}
