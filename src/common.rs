// Screen dimensions, i32 as per original C
pub const WIDTH: i32 = 256;
pub const HEIGHT: i32 = 192;

// Function pointer type — equivalent to typedef void (*EVENT)(void)
pub type Event = Option<fn()>;

// Key codes enum. The discriminants mirror the C original's KEY enum
// (reference/c-original/src/common.h) wherever the game can observe them,
// because the SDL poll tables in game_main.rs are indexed by these values:
// 0..5 the game keys, 6..15 the digits 1..9,0, 16..35 the letters a..t.
// The constants nothing in either implementation produces or polls —
// KEY_U, KEY_V, KEY_X, KEY_Z (36, 37, 39, 41) and KEY_QUIT (45) — have no
// port variant (W and Y are real inputs and keep their C values), so
// Escape is pinned at C's KEY_ESCAPE value and Else/None sit at 45/46.
// The mapping is pinned by the discriminants_match_the_c_original test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Left,
    Right,
    Jump,
    Enter,
    LShift,
    RShift,
    K1,
    K2,
    K3,
    K4,
    K5,
    K6,
    K7,
    K8,
    K9,
    K0,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    // Only the writetyper letters W and Y ever arrive as input, so the
    // other KEY_U..KEY_Z constants (36, 37, 39, 41) have no variant; W and
    // Y keep their C values, and Escape is pinned at C's KEY_ESCAPE (42).
    W = 38,
    Y = 40,
    Escape = 42,
    Pause,
    Mute,
    Else,
    None,
}

impl Key {
    /// The lowercase ASCII byte this letter key names, or `None` for every
    /// non-letter key. Cheat code-word matching goes through this.
    pub fn letter(self) -> Option<u8> {
        let offset = self as i32 - Key::A as i32;
        (0..=25).contains(&offset).then(|| b'a' + offset as u8)
    }

    /// The letter key naming this lowercase ASCII byte — the inverse of
    /// `letter()` over the letters that exist (the level-select a..t plus
    /// the code-word w and y). Panics for other bytes: map_keycode only
    /// ever produces the seven code-word letters, so this constructor
    /// exists for tests (C's cheat.c spelled inputs as KEY_A + offset).
    #[cfg(test)]
    pub fn from_letter(c: u8) -> Key {
        assert!(c.is_ascii_lowercase(), "{c} is not a letter key");
        const LEVEL_LETTERS: [Key; 20] = [
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
        match c {
            b'a'..=b't' => LEVEL_LETTERS[(c - b'a') as usize],
            b'w' => Key::W,
            b'y' => Key::Y,
            _ => panic!("no Key variant names {}", c as char),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Key;

    // The SDL poll tables in game_main.rs are indexed by Key value, so the
    // numbering must stay pinned to the C original's KEY enum (the enum's
    // doc comment explains why). Drift here would desync letters' slot in
    // the tables — the exact fragility this numbering removed.
    #[test]
    fn discriminants_match_the_c_original() {
        assert_eq!(Key::Left as i32, 0);
        assert_eq!(Key::RShift as i32, 5);
        assert_eq!(Key::K1 as i32, 6);
        assert_eq!(Key::K0 as i32, 15);
        assert_eq!(Key::A as i32, 16);
        assert_eq!(Key::T as i32, 35); // last letter with a poll-table slot
        assert_eq!(Key::W as i32, 38); // C: KEY_W (writetyper)
        assert_eq!(Key::Y as i32, 40); // C: KEY_Y (writetyper)
        // KEY_U/V/X/Z (36, 37, 39, 41) have no variants — nothing produces
        // or polls them in either implementation; Escape is pinned at C's
        // KEY_ESCAPE value instead of sitting at 36.
        assert_eq!(Key::Escape as i32, 42);
        assert_eq!(Key::None as i32, 46); // C's KEY_QUIT (45) has no variant
    }

    #[test]
    fn letter_keys_round_trip_and_others_name_no_letter() {
        // The port has Key variants for a..t plus the code-word w and y —
        // u, v, x, z never arise as input (see the Key enum comment).
        for &c in b"abcdefghijklmnopqrstwy" {
            assert_eq!(Key::from_letter(c).letter(), Some(c));
        }
        const OTHERS: [Key; 21] = [
            Key::Left,
            Key::Right,
            Key::Jump,
            Key::Enter,
            Key::LShift,
            Key::RShift,
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
            Key::Escape,
            Key::Pause,
            Key::Mute,
            Key::Else,
            Key::None,
        ];
        for k in OTHERS {
            assert_eq!(k.letter(), None);
        }
    }
}
