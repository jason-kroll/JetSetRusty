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

pub fn Timer_Set(timer: &mut Timer, numerator: i32, divisor: i32) {
    timer.acc = 0;
    timer.rate = numerator / divisor;
    timer.remainder = numerator - timer.rate * divisor;
    timer.divisor = divisor;
}

pub fn Timer_Update(timer: &mut Timer) -> i32 {
    timer.acc += timer.remainder;
    if timer.acc < timer.divisor {
        return timer.rate;
    }
    timer.acc -= timer.divisor;
    timer.rate + 1
}

// External --------------------------------------------------------------------

pub fn Video_Viewport(
    width: i32,
    height: i32,
    x: &mut i32,
    y: &mut i32,
    w: &mut i32,
    h: &mut i32,
) -> i32 {
    if height * 4 / 3 <= width {
        // landscape
        *h = (HEIGHT * height) / (HEIGHT + 16) as f32 as i32;
        *w = *h * 4 / 3;
    } else {
        // portrait
        *w = (WIDTH * width) / (WIDTH + 16) as f32 as i32;
        *h = *w * 3 / 4;
    }

    *x = (width - *w) / 2;
    *y = (height - *h) / 2;

    let mut multiply = *h / HEIGHT;
    if multiply < 1 {
        multiply = 1;
    }
    multiply
}
