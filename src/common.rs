// Screen dimensions, i32 as per original C
pub const WIDTH: i32 = 256;
pub const HEIGHT: i32 = 192;

// Function pointer type — equivalent to typedef void (*EVENT)(void)
pub type Event = Option<fn()>;

// Key codes enum
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
    A,
    // Letters B..Z are never constructed as variants — map_keycode produces
    // letter inputs arithmetically as `Key::A as i32 + offset` (values 10..35,
    // which still index SDL_KEYS in game_main.rs). Escape is pinned at 36 so
    // the discriminant layout below stays stable.
    Escape = 36,
    Pause,
    Mute,
    Else,
    None,
}

impl Key {
    /// Map a raw key code back to a Key. Codes 10..=35 are the letter keys;
    /// only `A` has a variant (letters are produced arithmetically as
    /// `Key::A as i32 + offset`), so they all map to `Key::A`. The 36..=40
    /// range relies on Escape being pinned at 36 and the variants after it
    /// following declaration order.
    pub fn from_key_code(code: i32) -> Key {
        const LOW: [Key; 10] = [
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
        ];
        const HIGH: [Key; 5] = [Key::Escape, Key::Pause, Key::Mute, Key::Else, Key::None];
        match code {
            0..=9 => LOW[code as usize],
            10..=35 => Key::A,
            36..=40 => HIGH[code as usize - 36],
            _ => Key::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Key;

    #[test]
    fn key_code_round_trips_named_variants() {
        const ALL: [Key; 16] = [
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
            Key::A,
            Key::Escape,
            Key::Pause,
            Key::Mute,
            Key::Else,
            Key::None,
        ];
        for k in ALL {
            assert_eq!(Key::from_key_code(k as i32), k);
        }
        // Letter codes (A + offset) collapse to Key::A; out-of-range to None
        assert_eq!(Key::from_key_code(Key::A as i32 + 25), Key::A);
        assert_eq!(Key::from_key_code(41), Key::None);
        assert_eq!(Key::from_key_code(-1), Key::None);
    }
}
