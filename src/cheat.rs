use crate::common::{Event, Key};
use crate::game::{FIRSTLANDING, GAME_STATE, game_cheat_enabled, game_pause};
use crate::game_main::{Action, game_input, set_action, system_is_key};
use std::sync::atomic::{AtomicUsize, Ordering};

const CHEAT_CODE: &[u8] = b"writetyper";

// miner.y on First Landing's floor — the only spot where the code is read
const FIRSTLANDING_FLOOR_Y: i32 = 104;

/// The level-select keys in scan order — digits 1..0 then letters a..t, the
/// same 30 keys C's CheatLevelSelect loop polls as
/// `System_IsKey(KEY_1 + i)`; the matched position names room 1..30.
const LEVEL_KEYS: [Key; 30] = [
    Key::K1,
    Key::K2,
    Key::K3,
    Key::K4,
    Key::K5,
    Key::K6,
    Key::K7,
    Key::K8,
    Key::K9,
    Key::K0,
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
];

pub fn cheat_responder() -> Event {
    if GAME_STATE.cheat_enabled.load(Ordering::Relaxed) {
        Some(cheat_level_select)
    } else {
        Some(cheat_code_entry)
    }
}

fn cheat_level_select() {
    let mut level: i32 = 0;

    for (i, &key) in LEVEL_KEYS.iter().enumerate() {
        if system_is_key(key) {
            level = (i + 1) as i32;
            break;
        }
    }

    if !system_is_key(Key::Enter) {
        game_pause(false);
        return;
    }

    if level == 0 {
        return;
    }

    if system_is_key(Key::LShift) || system_is_key(Key::RShift) {
        level += 30;
    }

    level -= 1;
    if level == GAME_STATE.level.load(Ordering::Relaxed) {
        return;
    }

    GAME_STATE.level.store(level, Ordering::Relaxed);

    set_action(Action::GameInitRoom);
}

/// Progress through CHEAT_CODE after one input: one past the matched letter
/// on a match, zero on anything else — a wrong letter, a non-letter input,
/// or an already-complete code word. Matches C: cheat.c reads the '\0'
/// sentinel one past the last letter and resets cheatPos on any mismatch.
fn code_progress(pos: usize, key: Key) -> usize {
    // Not an Option equality: no expected letter (pos == len) paired with a
    // non-letter input must NOT count as a match
    match (CHEAT_CODE.get(pos), key.letter()) {
        (Some(&want), Some(ch)) if want == ch => pos + 1,
        _ => 0,
    }
}

fn cheat_code_entry() {
    static CHEAT_POS: AtomicUsize = AtomicUsize::new(0);

    let y = GAME_STATE.miner.lock().unwrap().y;
    // C gates on the room and Willy's height first, then the code word
    let on_entry_floor =
        GAME_STATE.level.load(Ordering::Relaxed) == FIRSTLANDING && y == FIRSTLANDING_FLOOR_Y;
    let pos = if on_entry_floor {
        code_progress(CHEAT_POS.load(Ordering::Relaxed), game_input())
    } else {
        0
    };
    CHEAT_POS.store(pos, Ordering::Relaxed);

    if pos == 0 {
        // Matches C: Game_Pause(0) on every non-advancing input — the
        // responder consumes any keystroke, and a stray key closes the
        // pause screen.
        game_pause(false);
        return;
    }

    if pos < CHEAT_CODE.len() {
        return;
    }

    game_cheat_enabled();
}

#[cfg(test)]
mod tests {
    use super::*;

    // map_keycode produces letter inputs as their Key variant
    fn key(c: u8) -> Key {
        Key::from_letter(c)
    }

    #[test]
    fn code_progress_walks_the_code_word() {
        let mut pos = 0;
        for c in CHEAT_CODE {
            assert_eq!(code_progress(pos, key(*c)), pos + 1, "after {}", *c as char);
            pos += 1;
        }
        assert_eq!(pos, CHEAT_CODE.len());
    }

    #[test]
    fn code_progress_resets_on_any_mismatch() {
        // "writ" typed, then a stray 'a' — C parity: the position resets,
        // and 'e' restarts from 'w' rather than resuming at position 4
        assert_eq!(code_progress(4, key(b'a')), 0);
        assert_eq!(code_progress(0, key(b'e')), 0);
        // movement and digit keys are not code-word candidates
        assert_eq!(code_progress(4, Key::Left), 0);
        assert_eq!(code_progress(4, Key::K1), 0);
    }

    #[test]
    fn code_progress_resets_a_completed_code_word() {
        // C reads the '\0' sentinel one past the last letter, so any input
        // after a completed word restarts it
        let end = CHEAT_CODE.len();
        assert_eq!(code_progress(end, key(b'w')), 0);
        assert_eq!(code_progress(end, Key::Left), 0);
    }
}
