use crate::common::{Event, Key};
use crate::game::{FIRSTLANDING, GAME_STATE, game_cheat_enabled, game_init_room, game_pause};
use crate::game_main::{game_input, set_action, system_is_key};
use std::sync::atomic::{AtomicUsize, Ordering};

const CHEAT_CODE: &[u8] = b"writetyper";

// miner.y on First Landing's floor — the only spot where the code is read
const FIRSTLANDING_FLOOR_Y: i32 = 104;

pub fn cheat_responder() -> Event {
    if GAME_STATE.cheat_enabled.load(Ordering::Relaxed) {
        Some(cheat_level_select)
    } else {
        Some(cheat_code_entry)
    }
}

fn cheat_level_select() {
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

/// The lowercase ASCII byte a letter input names. Letter keys arrive as
/// `Key::A` + offset (see `common::Key`); every other input maps to None.
fn letter_byte(input: i32) -> Option<u8> {
    let offset = input - Key::A as i32;
    (0..=25).contains(&offset).then(|| b'a' + offset as u8)
}

/// Progress through CHEAT_CODE after one input: one past the matched letter
/// on a match, zero on anything else — a wrong letter, a non-letter input,
/// or an already-complete code word. Matches C: cheat.c reads the '\0'
/// sentinel one past the last letter and resets cheatPos on any mismatch.
fn code_progress(pos: usize, input: i32) -> usize {
    // Not an Option equality: no expected letter (pos == len) paired with a
    // non-letter input must NOT count as a match
    match (CHEAT_CODE.get(pos), letter_byte(input)) {
        (Some(want), Some(ch)) if *want == ch => pos + 1,
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

    // map_keycode produces letter inputs arithmetically as Key::A + offset
    fn key(c: u8) -> i32 {
        Key::A as i32 + (c - b'a') as i32
    }

    #[test]
    fn letter_byte_maps_letters_and_rejects_everything_else() {
        for (offset, c) in (b'a'..=b'z').enumerate() {
            assert_eq!(letter_byte(Key::A as i32 + offset as i32), Some(c));
        }
        for k in [
            Key::Left as i32,
            Key::Right as i32,
            Key::Jump as i32,
            Key::Enter as i32,
            Key::LShift as i32,
            Key::RShift as i32,
            Key::K1 as i32,
            Key::K4 as i32,
            Key::Escape as i32,
            Key::Pause as i32,
            Key::Mute as i32,
            Key::Else as i32,
            Key::None as i32,
        ] {
            assert_eq!(letter_byte(k), None);
        }
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
        // "writ" typed, then a stray 'x' — C parity: the position resets,
        // and 'e' restarts from 'w' rather than resuming at position 4
        assert_eq!(code_progress(4, key(b'x')), 0);
        assert_eq!(code_progress(0, key(b'e')), 0);
        // movement and digit keys are not code-word candidates
        assert_eq!(code_progress(4, Key::Left as i32), 0);
        assert_eq!(code_progress(4, Key::K1 as i32), 0);
    }

    #[test]
    fn code_progress_resets_a_completed_code_word() {
        // C reads the '\0' sentinel one past the last letter, so any input
        // after a completed word restarts it
        let end = CHEAT_CODE.len();
        assert_eq!(code_progress(end, key(b'w')), 0);
        assert_eq!(code_progress(end, Key::Left as i32), 0);
    }
}
