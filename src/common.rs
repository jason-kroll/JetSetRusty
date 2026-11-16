use crate::levels;

// Screen dimensions, i32 as per original C
pub const WIDTH: i32 = 256;
pub const HEIGHT: i32 = 192;

// Function pointer type — equivalent to typedef void (*EVENT)(void)
pub type Event = Option<fn()>;

// Key codes enum
#[derive(Clone, Copy, PartialEq, Eq)]
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

// Globals defined in game_main.rs, re-exported here for convenience
pub use crate::game_main::{DoNothing, DoQuit, System_Rnd, System_SetPixel};

// The screen-state Action entry points, defined in their own modules
pub use crate::title::Title_Action;

pub fn Level_SetBorder() {
    levels::level_set_border();
}

pub fn system_set_pixel(pos: i32, ink: i32) {
    System_SetPixel(pos, ink)
}
