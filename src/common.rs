// Screen dimensions, i32 as per original C
pub const WIDTH: i32 = 256;
pub const HEIGHT: i32 = 192;

// Function pointer type — equivalent to typedef void (*EVENT)(void)
pub type Event = Option<fn()>;

// Key codes enum. The discriminants mirror the C original's KEY enum
// (reference/c-original/src/common.h) exactly, because the SDL poll tables
// in game_main.rs are indexed by these values — slots 0..5 are the game
// keys, 6..15 the digits 1..9,0, 16..35 the letters a..t. C's KEY_QUIT (45)
// has no variant — the port has no quit key — so Else/None sit at 45/46.
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
    U,
    V,
    W,
    X,
    Y,
    Z,
    Escape,
    Pause,
    Mute,
    Else,
    None,
}

impl Key {
    /// Map a raw key code (a Key discriminant, as carried by gameInput back
    /// to a Key. Every variant round-trips; codes outside the enum map to
    /// `Key::None`, the old C-style "no input" sentinel.
    pub fn from_key_code(code: i32) -> Key {
        const LOW: [Key; 16] = [
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
        ];
        const LETTERS: [Key; 26] = [
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
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        const HIGH: [Key; 5] = [Key::Escape, Key::Pause, Key::Mute, Key::Else, Key::None];
        match code {
            0..=15 => LOW[code as usize],
            16..=41 => LETTERS[(code - 16) as usize],
            42..=46 => HIGH[(code - 42) as usize],
            _ => Key::None,
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
        assert_eq!(Key::Z as i32, 41);
        assert_eq!(Key::Escape as i32, 42); // C's KEY_ESCAPE
        assert_eq!(Key::None as i32, 46); // C's KEY_QUIT (45) has no variant
    }

    #[test]
    fn key_code_round_trips_every_variant() {
        const ALL: [Key; 47] = [
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
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
            Key::Escape,
            Key::Pause,
            Key::Mute,
            Key::Else,
            Key::None,
        ];
        for k in ALL {
            assert_eq!(Key::from_key_code(k as i32), k);
        }
        // Out-of-range codes map to None, like C's no-input sentinel.
        assert_eq!(Key::from_key_code(47), Key::None);
        assert_eq!(Key::from_key_code(-1), Key::None);
    }
}
