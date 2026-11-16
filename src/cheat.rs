use crate::common::{Event, Key};
use crate::game::{GAME_STATE, game_cheat_enabled, game_init_room, game_pause};
use crate::game_main::{game_input, set_action, system_is_key};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

// These will move to game.rs when we port that
const FIRSTLANDING: i32 = 28;

static CHEAT_CODE: &[u8] = b"writetyper";

thread_local! {
    static CHEAT_RESPONDER: Cell<Event> = const { Cell::new(Some(cheat_disabled)) };
}

pub fn cheat_responder() -> Event {
    CHEAT_RESPONDER.with(Cell::get)
}
fn set_cheat_responder(f: Event) {
    CHEAT_RESPONDER.with(|c| c.set(f));
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
    if GAME_STATE.level.load(Ordering::Relaxed) != FIRSTLANDING
        || miner.y != 104
        || CHEAT_CODE[CHEAT_POS.load(Ordering::Relaxed)]
            != (game_input() - Key::A as i32 + b'a' as i32) as u8
    {
        game_pause(false);
        return;
    }

    CHEAT_POS.fetch_add(1, Ordering::Relaxed);

    if CHEAT_POS.load(Ordering::Relaxed) < CHEAT_CODE.len() {
        return;
    }

    game_cheat_enabled();

    set_cheat_responder(Some(cheat_enabled));
}
