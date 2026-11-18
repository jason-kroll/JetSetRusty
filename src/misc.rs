use crate::common::{HEIGHT, WIDTH};

// Colour table ----------------------------------------------------------------
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub static VIDEO_COLOUR: [Colour; 16] = [
    Colour {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    }, // black
    Colour {
        r: 0x00,
        g: 0x00,
        b: 0xff,
    }, // blue
    Colour {
        r: 0xff,
        g: 0x00,
        b: 0x00,
    }, // red
    Colour {
        r: 0xff,
        g: 0x00,
        b: 0xff,
    }, // magenta
    Colour {
        r: 0x00,
        g: 0xff,
        b: 0x00,
    }, // green
    Colour {
        r: 0x00,
        g: 0xaa,
        b: 0xff,
    }, // light blue
    Colour {
        r: 0xff,
        g: 0xff,
        b: 0x00,
    }, // yellow
    Colour {
        r: 0xff,
        g: 0xff,
        b: 0xff,
    }, // white
    Colour {
        r: 0xcc,
        g: 0xcc,
        b: 0xcc,
    }, // mid grey (only for Willy or BT_SOLID)
    Colour {
        r: 0x00,
        g: 0x55,
        b: 0xff,
    }, // mid blue
    Colour {
        r: 0xaa,
        g: 0x00,
        b: 0x00,
    }, // mid red
    Colour {
        r: 0x55,
        g: 0x00,
        b: 0x00,
    }, // dark red
    Colour {
        r: 0x00,
        g: 0xaa,
        b: 0x00,
    }, // mid green
    Colour {
        r: 0x00,
        g: 0x55,
        b: 0x00,
    }, // dark green
    Colour {
        r: 0xff,
        g: 0x80,
        b: 0x00,
    }, // orange
    Colour {
        r: 0x80,
        g: 0x40,
        b: 0x00,
    }, // brown
];

// RGB triple for a palette index, in r,g,b order. This is what the SDL
// present path (texture upload and border) ultimately shows, so it is
// pinned entry-by-entry by the unit test below — an SDL3 migration must
// reproduce these exact bytes.
pub fn colour_rgb(index: usize) -> [u8; 3] {
    let c = &VIDEO_COLOUR[index];
    [c.r, c.g, c.b]
}

// Timer -----------------------------------------------------------------------
#[derive(Clone, Copy, Default)]
pub struct Timer {
    // Field order matches the original C `TIMER` ({rate, acc, remainder,
    // divisor}) for provenance. This used to be a live ABI contract — the
    // struct was aliased over the C `gameTimer` global — but with no FFI left
    // the `#[repr(C)]` is gone and only the field order remains as a nod to C.
    pub rate: i32,
    pub acc: i32,
    pub remainder: i32,
    pub divisor: i32,
}

impl std::fmt::Debug for Timer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timer")
            .field("rate", &self.rate)
            .field("acc", &self.acc)
            .field("remainder", &self.remainder)
            .field("divisor", &self.divisor)
            .finish()
    }
}

pub fn timer_set(timer: &mut Timer, numerator: i32, divisor: i32) {
    timer.acc = 0;
    timer.rate = numerator / divisor;
    timer.remainder = numerator - timer.rate * divisor;
    timer.divisor = divisor;
}

pub fn timer_update(timer: &mut Timer) -> i32 {
    timer.acc += timer.remainder;
    if timer.acc < timer.divisor {
        return timer.rate;
    }
    timer.acc -= timer.divisor;
    timer.rate + 1
}

// External --------------------------------------------------------------------

/// The per-frame letterbox: whatever the window size (fullscreen desktop or a
/// reshaped `--windowed` window), the game area is centered and keeps the 4:3
/// ratio of the full 272x208 screen (game area + 8px border), with the integer
/// upscale clamped to >= 1. The `Viewport` x/y/w/h are the letterbox's pixel
/// geometry; the returned multiply is the game texture's integer scale for the
/// target texture. (Pure — the C original wrote five out-params through
/// pointers; the shape lives in the tests as the pinned tuple.)
pub struct Viewport {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub fn video_viewport(width: i32, height: i32) -> (Viewport, i32) {
    let (w, h) = if height * 4 / 3 <= width {
        // landscape
        let h = (HEIGHT * height) / (HEIGHT + 16);
        (h * 4 / 3, h)
    } else {
        // portrait
        let w = (WIDTH * width) / (WIDTH + 16);
        (w, w * 3 / 4)
    };

    let viewport = Viewport {
        x: (width - w) / 2,
        y: (height - h) / 2,
        w,
        h,
    };

    let multiply = (viewport.h / HEIGHT).max(1);

    (viewport, multiply)
}

// Initial size for a `--windowed` window: the largest integer scale (up to 4)
// of the full screen including its 8-pixel border — (WIDTH+16) x (HEIGHT+16)
// = 272x208, exactly 4:3 — that fits inside the desktop mode, so the window
// opens as a crisp integer upscale (1088x832 on a 1080p display). A tiling
// window manager can then reshape it freely; the per-frame letterbox in
// game_main keeps the game at 4:3 whatever size it ends up.
pub fn windowed_size(display_w: i32, display_h: i32) -> (i32, i32) {
    const MAX_MULTIPLY: i32 = 4;
    let screen_w = WIDTH + 16;
    let screen_h = HEIGHT + 16;
    for m in (1..=MAX_MULTIPLY).rev() {
        if screen_w * m <= display_w && screen_h * m <= display_h {
            return (screen_w * m, screen_h * m);
        }
    }
    // Degenerate tiny display: fall back to the 1x screen size.
    (screen_w, screen_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    // video_viewport as a pure tuple — the same shape the fn returns; these
    // pin the exact arithmetic.
    fn vp(width: i32, height: i32) -> (i32, i32, i32, i32, i32) {
        let (viewport, m) = video_viewport(width, height);
        (viewport.x, viewport.y, viewport.w, viewport.h, m)
    }

    // video_viewport is the per-frame letterbox: whatever the window size
    // (fullscreen desktop or a reshaped --windowed window), the game area is
    // centered and keeps the 4:3 ratio of the full 272x208 screen, with an
    // integer multiply clamped to >= 1. These pin the exact arithmetic.
    #[test]
    fn video_viewport_pins_fullscreen_1080p() {
        assert_eq!(vp(1920, 1080), (296, 42, 1328, 996, 5));
    }

    #[test]
    fn video_viewport_pins_exact_4x_window() {
        // 1088x832 = 4x the 272x208 screen: exact 4x game area, 32px border.
        assert_eq!(vp(1088, 832), (32, 32, 1024, 768, 4));
    }

    #[test]
    fn video_viewport_pins_landscape_window() {
        assert_eq!(vp(1000, 500), (193, 19, 614, 461, 2));
    }

    #[test]
    fn video_viewport_pins_portrait_window() {
        assert_eq!(vp(800, 1200), (24, 318, 752, 564, 2));
    }

    #[test]
    fn video_viewport_clamps_multiply_on_tiny_windows() {
        let (x, y, w, h, m) = vp(200, 200);
        assert_eq!((x, y, w, h), (6, 29, 188, 141));
        assert_eq!(m, 1); // 141 / 192 = 0, clamped to 1
    }

    #[test]
    fn windowed_size_picks_largest_fitting_scale() {
        assert_eq!(windowed_size(1920, 1080), (1088, 832)); // 4x fits
        assert_eq!(windowed_size(1366, 768), (816, 624)); // 4x too tall, 3x fits
        assert_eq!(windowed_size(1024, 768), (816, 624));
        assert_eq!(windowed_size(500, 400), (272, 208)); // only 1x fits
        assert_eq!(windowed_size(100, 100), (272, 208)); // degenerate fallback
    }

    // Pin every palette entry as an exact RGB triple.
    #[test]
    fn palette_rgb_values_are_pinned() {
        let expected: [[u8; 3]; 16] = [
            [0x00, 0x00, 0x00], // black
            [0x00, 0x00, 0xff], // blue
            [0xff, 0x00, 0x00], // red
            [0xff, 0x00, 0xff], // magenta
            [0x00, 0xff, 0x00], // green
            [0x00, 0xaa, 0xff], // light blue
            [0xff, 0xff, 0x00], // yellow
            [0xff, 0xff, 0xff], // white
            [0xcc, 0xcc, 0xcc], // mid grey
            [0x00, 0x55, 0xff], // mid blue
            [0xaa, 0x00, 0x00], // mid red
            [0x55, 0x00, 0x00], // dark red
            [0x00, 0xaa, 0x00], // mid green
            [0x00, 0x55, 0x00], // dark green
            [0xff, 0x80, 0x00], // orange
            [0x80, 0x40, 0x00], // brown
        ];
        assert_eq!(VIDEO_COLOUR.len(), expected.len());
        for (i, e) in expected.iter().enumerate() {
            assert_eq!(colour_rgb(i), *e, "palette entry {i}");
        }
    }
}
