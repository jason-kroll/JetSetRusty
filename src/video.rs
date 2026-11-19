use crate::common::{HEIGHT, WIDTH};
use crate::game_main::system_set_pixel;

use std::sync::LazyLock;
use std::sync::Mutex;

///////////////
// From video.h

const B_LEVEL: u8 = 1;
const B_ROBOT: u8 = 2;
const B_WILLY: u8 = 4;

/// The `point` half of a pixel: the B_* flag bitfield (level ink, robot
/// ink, Willy), kept separate from the palette `ink`. Same bits as C's
/// `pixel.point`; the flag constants stay private to this module.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelFlags(u8);

impl PixelFlags {
    pub(crate) const EMPTY: Self = Self(0);
    pub(crate) const LEVEL: Self = Self(B_LEVEL);
    pub(crate) const ROBOT: Self = Self(B_ROBOT);
    pub(crate) const WILLY: Self = Self(B_WILLY);

    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub(crate) const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    pub(crate) const fn has_willy(self) -> bool {
        self.contains(Self::WILLY)
    }
}

impl std::ops::BitOr for PixelFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for PixelFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

pub(crate) const CHAR_WIDTH: usize = 8; // width of a character in pixels
pub(crate) const CHAR_HEIGHT: usize = 8;
pub(crate) const CHAR_HEIGHT_LARGE: usize = CHAR_HEIGHT * 2;

#[inline]
pub(crate) const fn tile_2_pixel(t: i32) -> i32 {
    ((t & 992) << 6) | ((t & 31) << 3)
}

#[derive(Clone, Copy)]
pub(crate) struct Pixel {
    ink: u8,
    flags: PixelFlags,
}

pub(crate) static VIDEO_PIXEL: LazyLock<Mutex<Vec<Pixel>>> = LazyLock::new(|| {
    Mutex::new(vec![
        Pixel {
            ink: 0,
            flags: PixelFlags::EMPTY,
        };
        WIDTH as usize * HEIGHT as usize
    ])
});

static TEXT_INK: Mutex<[u8; 2]> = Mutex::new([0, 0]);

static CHAR_SET: [[u8; 10]; 128] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // paper
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // ink
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [8, 128, 128, 192, 192, 224, 224, 240, 240, 0],
    [8, 248, 248, 252, 252, 254, 254, 255, 255, 0],
    [8, 255, 255, 254, 254, 252, 252, 248, 248, 0],
    [8, 240, 240, 224, 224, 192, 192, 128, 128, 0],
    [8, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [8, 255, 255, 255, 255, 255, 255, 255, 255, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 47, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 0, 3, 0, 0, 0, 0, 0, 0],
    [7, 18, 63, 18, 18, 63, 18, 0, 0, 0],
    [6, 46, 42, 127, 42, 58, 0, 0, 0, 0],
    [7, 35, 19, 8, 4, 50, 49, 0, 0, 0],
    [7, 16, 42, 37, 42, 16, 40, 0, 0, 0],
    [3, 2, 1, 0, 0, 0, 0, 0, 0, 0],
    [3, 30, 33, 0, 0, 0, 0, 0, 0, 0],
    [3, 33, 30, 0, 0, 0, 0, 0, 0, 0],
    [6, 8, 42, 28, 42, 8, 0, 0, 0, 0],
    [6, 8, 8, 62, 8, 8, 0, 0, 0, 0],
    [3, 64, 32, 0, 0, 0, 0, 0, 0, 0],
    [6, 8, 8, 8, 8, 8, 0, 0, 0, 0],
    [2, 32, 0, 0, 0, 0, 0, 0, 0, 0],
    [6, 32, 16, 8, 4, 2, 0, 0, 0, 0],
    [6, 12, 18, 33, 18, 12, 0, 0, 0, 0],
    [4, 34, 63, 32, 0, 0, 0, 0, 0, 0],
    [6, 50, 41, 41, 41, 38, 0, 0, 0, 0],
    [6, 18, 33, 37, 37, 26, 0, 0, 0, 0],
    [5, 15, 8, 60, 8, 0, 0, 0, 0, 0],
    [6, 23, 37, 37, 37, 25, 0, 0, 0, 0],
    [6, 30, 37, 37, 37, 24, 0, 0, 0, 0],
    [6, 1, 1, 49, 13, 3, 0, 0, 0, 0],
    [6, 26, 37, 37, 37, 26, 0, 0, 0, 0],
    [6, 6, 41, 41, 41, 30, 0, 0, 0, 0],
    [2, 20, 0, 0, 0, 0, 0, 0, 0, 0],
    [3, 32, 20, 0, 0, 0, 0, 0, 0, 0],
    [4, 8, 20, 34, 0, 0, 0, 0, 0, 0],
    [6, 20, 20, 20, 20, 20, 0, 0, 0, 0],
    [4, 34, 20, 8, 0, 0, 0, 0, 0, 0],
    [6, 2, 1, 41, 5, 2, 0, 0, 0, 0],
    [7, 30, 33, 45, 43, 45, 14, 0, 0, 0],
    [6, 48, 14, 9, 14, 48, 0, 0, 0, 0],
    [6, 63, 37, 37, 37, 26, 0, 0, 0, 0],
    [6, 30, 33, 33, 33, 18, 0, 0, 0, 0],
    [6, 63, 33, 33, 18, 12, 0, 0, 0, 0],
    [6, 63, 37, 37, 37, 33, 0, 0, 0, 0],
    [6, 63, 5, 5, 5, 1, 0, 0, 0, 0],
    [6, 30, 33, 33, 41, 26, 0, 0, 0, 0],
    [6, 63, 4, 4, 4, 63, 0, 0, 0, 0],
    [4, 33, 63, 33, 0, 0, 0, 0, 0, 0],
    [6, 16, 32, 32, 32, 31, 0, 0, 0, 0],
    [6, 63, 4, 10, 17, 32, 0, 0, 0, 0],
    [6, 63, 32, 32, 32, 32, 0, 0, 0, 0],
    [8, 56, 7, 12, 16, 12, 7, 56, 0, 0],
    [7, 63, 2, 4, 8, 16, 63, 0, 0, 0],
    [6, 30, 33, 33, 33, 30, 0, 0, 0, 0],
    [6, 63, 9, 9, 9, 6, 0, 0, 0, 0],
    [7, 30, 33, 41, 49, 33, 30, 0, 0, 0],
    [6, 63, 9, 9, 25, 38, 0, 0, 0, 0],
    [6, 18, 37, 37, 37, 24, 0, 0, 0, 0],
    [6, 1, 1, 63, 1, 1, 0, 0, 0, 0],
    [6, 31, 32, 32, 32, 31, 0, 0, 0, 0],
    [6, 7, 24, 32, 24, 7, 0, 0, 0, 0],
    [8, 7, 24, 32, 24, 32, 24, 7, 0, 0],
    [7, 33, 18, 12, 12, 18, 33, 0, 0, 0],
    [6, 3, 4, 56, 4, 3, 0, 0, 0, 0],
    [7, 33, 49, 41, 37, 35, 33, 0, 0, 0],
    [3, 63, 33, 0, 0, 0, 0, 0, 0, 0],
    [6, 2, 4, 8, 16, 32, 0, 0, 0, 0],
    [3, 33, 63, 0, 0, 0, 0, 0, 0, 0],
    [6, 4, 2, 63, 2, 4, 0, 0, 0, 0],
    [7, 64, 64, 64, 64, 64, 64, 0, 0, 0],
    [6, 36, 62, 37, 33, 34, 0, 0, 0, 0],
    [5, 16, 42, 42, 60, 0, 0, 0, 0, 0],
    [5, 63, 34, 34, 28, 0, 0, 0, 0, 0],
    [5, 28, 34, 34, 34, 0, 0, 0, 0, 0],
    [5, 28, 34, 34, 63, 0, 0, 0, 0, 0],
    [5, 28, 42, 42, 36, 0, 0, 0, 0, 0],
    [4, 62, 5, 1, 0, 0, 0, 0, 0, 0],
    [5, 28, 162, 162, 126, 0, 0, 0, 0, 0],
    [5, 63, 2, 2, 60, 0, 0, 0, 0, 0],
    [2, 61, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, 32, 64, 61, 0, 0, 0, 0, 0, 0],
    [5, 63, 12, 18, 32, 0, 0, 0, 0, 0],
    [2, 63, 0, 0, 0, 0, 0, 0, 0, 0],
    [6, 62, 2, 60, 2, 60, 0, 0, 0, 0],
    [5, 62, 2, 2, 60, 0, 0, 0, 0, 0],
    [5, 28, 34, 34, 28, 0, 0, 0, 0, 0],
    [5, 254, 34, 34, 28, 0, 0, 0, 0, 0],
    [5, 28, 34, 34, 254, 128, 0, 0, 0, 0],
    [4, 60, 2, 2, 0, 0, 0, 0, 0, 0],
    [5, 36, 42, 42, 16, 0, 0, 0, 0, 0],
    [4, 2, 63, 2, 0, 0, 0, 0, 0, 0],
    [5, 30, 32, 32, 30, 0, 0, 0, 0, 0],
    [6, 6, 24, 32, 24, 6, 0, 0, 0, 0],
    [6, 30, 32, 28, 32, 30, 0, 0, 0, 0],
    [6, 34, 20, 8, 20, 34, 0, 0, 0, 0],
    [5, 30, 160, 160, 126, 0, 0, 0, 0, 0],
    [6, 34, 50, 42, 38, 34, 0, 0, 0, 0],
    [4, 4, 59, 33, 0, 0, 0, 0, 0, 0],
    [2, 63, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, 33, 59, 4, 0, 0, 0, 0, 0, 0],
    [5, 16, 8, 16, 8, 0, 0, 0, 0, 0],
    [9, 60, 66, 153, 165, 165, 129, 66, 60, 0],
];

static CHAR_SET_LARGE: [[u16; 8]; 96] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 60, 7166, 7166, 60, 0, 0],
    [0, 7, 15, 0, 0, 15, 7, 0],
    [528, 8190, 8190, 528, 8190, 8190, 528, 0],
    [1592, 3196, 2116, 6214, 6214, 4044, 1928, 0],
    [6158, 7694, 1920, 480, 120, 7198, 7174, 0],
    [3968, 8156, 4222, 4578, 4030, 8156, 4160, 0],
    [0, 0, 8, 15, 7, 0, 0, 0],
    [0, 0, 2040, 4092, 6150, 4098, 0, 0],
    [0, 0, 4098, 6150, 4092, 2040, 0, 0],
    [128, 672, 992, 448, 992, 672, 128, 0],
    [0, 128, 128, 992, 992, 128, 128, 0],
    [0, 0, 8192, 14336, 6144, 0, 0, 0],
    [128, 128, 128, 128, 128, 128, 128, 0],
    [0, 0, 0, 6144, 6144, 0, 0, 0],
    [6144, 7680, 1920, 480, 120, 30, 6, 0],
    [2040, 4092, 6150, 4098, 6150, 4092, 2040, 0],
    [0, 4104, 4108, 8190, 8190, 4096, 4096, 0],
    [7684, 7942, 4482, 4290, 4194, 6206, 6172, 0],
    [2052, 6150, 4162, 4162, 4162, 8190, 4028, 0],
    [510, 510, 4352, 8160, 8160, 4352, 256, 0],
    [2174, 6270, 4162, 4162, 4162, 8130, 3970, 0],
    [4088, 8188, 4166, 4162, 4162, 8128, 3968, 0],
    [6, 6, 7682, 8066, 450, 126, 62, 0],
    [4028, 8190, 4162, 4162, 4162, 8190, 4028, 0],
    [60, 4222, 4162, 4162, 6210, 4094, 2044, 0],
    [0, 0, 0, 3096, 3096, 0, 0, 0],
    [0, 0, 4096, 7192, 3096, 0, 0, 0],
    [0, 192, 480, 816, 1560, 3084, 2052, 0],
    [576, 576, 576, 576, 576, 576, 576, 0],
    [0, 2052, 3084, 1560, 816, 480, 192, 0],
    [12, 14, 2, 7042, 7106, 126, 60, 0],
    [4088, 8188, 4100, 5060, 5060, 5116, 504, 0],
    [8176, 8184, 140, 134, 140, 8184, 8176, 0],
    [4098, 8190, 8190, 4162, 4162, 8190, 4028, 0],
    [2040, 4092, 6150, 4098, 4098, 6150, 3084, 0],
    [4098, 8190, 8190, 4098, 6150, 4092, 2040, 0],
    [4098, 8190, 8190, 4162, 4322, 6150, 7182, 0],
    [4098, 8190, 8190, 4162, 226, 6, 14, 0],
    [2040, 4092, 6150, 4226, 4226, 3974, 8076, 0],
    [8190, 8190, 64, 64, 64, 8190, 8190, 0],
    [0, 0, 4098, 8190, 8190, 4098, 0, 0],
    [3072, 7168, 4096, 4098, 8190, 4094, 2, 0],
    [4098, 8190, 8190, 192, 1008, 8126, 7182, 0],
    [4098, 8190, 8190, 4098, 4096, 6144, 7168, 0],
    [8190, 8190, 28, 120, 28, 8190, 8190, 0],
    [8190, 8190, 120, 480, 1920, 8190, 8190, 0],
    [4092, 8190, 4098, 4098, 4098, 8190, 4092, 0],
    [4098, 8190, 8190, 4162, 66, 126, 60, 0],
    [4092, 8190, 4098, 7170, 30722, 32766, 20476, 0],
    [4098, 8190, 8190, 66, 450, 8190, 7740, 0],
    [3100, 7230, 4194, 4162, 4290, 8078, 3852, 0],
    [14, 6, 4098, 8190, 8190, 4098, 6, 14],
    [4094, 8190, 4096, 4096, 4096, 8190, 4094, 0],
    [1022, 2046, 3072, 6144, 3072, 2046, 1022, 0],
    [2046, 8190, 7168, 2016, 7168, 8190, 2046, 0],
    [7182, 7998, 1008, 192, 1008, 7998, 7182, 0],
    [30, 62, 4192, 8128, 8128, 4192, 62, 30],
    [7694, 7942, 4482, 4290, 4194, 6206, 7198, 0],
    [0, 0, 8190, 8190, 4098, 4098, 0, 0],
    [6, 30, 120, 480, 1920, 7680, 6144, 0],
    [0, 0, 4098, 4098, 8190, 8190, 0, 0],
    [8, 12, 6, 3, 6, 12, 8, 0],
    [16384, 16384, 16384, 16384, 16384, 16384, 16384, 16384],
    [6176, 8190, 8191, 4129, 4099, 6150, 2048, 0],
    [3584, 7968, 4384, 4384, 4064, 8128, 4096, 0],
    [4098, 8190, 4094, 4128, 4192, 8128, 3968, 0],
    [4032, 8160, 4128, 4128, 4128, 6240, 2112, 0],
    [3968, 8128, 4192, 4130, 4094, 8190, 4096, 0],
    [4032, 8160, 4384, 4384, 4384, 6624, 2496, 0],
    [4128, 8188, 8190, 4130, 6, 12, 0, 0],
    [20416, 57312, 36896, 36896, 65472, 32736, 32, 0],
    [4098, 8190, 8190, 64, 32, 8160, 8128, 0],
    [0, 0, 4128, 8166, 8166, 4096, 0, 0],
    [0, 24576, 57344, 32768, 32800, 65510, 32742, 0],
    [4098, 8190, 8190, 768, 1920, 7392, 6240, 0],
    [0, 0, 4098, 8190, 8190, 4096, 0, 0],
    [8160, 8160, 96, 8128, 96, 8160, 8128, 0],
    [32, 8160, 8128, 32, 32, 8160, 8128, 0],
    [4032, 8160, 4128, 4128, 4128, 8160, 4032, 0],
    [32800, 65504, 65472, 36896, 4128, 8160, 4032, 0],
    [4032, 8160, 4128, 36896, 65472, 65504, 32800, 0],
    [4128, 8160, 8128, 4192, 32, 224, 192, 0],
    [2112, 6368, 4512, 4384, 4896, 7776, 3136, 0],
    [32, 32, 4092, 8190, 4128, 6176, 2048, 0],
    [4064, 8160, 4096, 4096, 4064, 8160, 4096, 0],
    [0, 2016, 4064, 6144, 6144, 4064, 2016, 0],
    [4064, 8160, 6144, 3840, 6144, 8160, 4064, 0],
    [6240, 7392, 1920, 768, 1920, 7392, 6240, 0],
    [4064, 40928, 36864, 36864, 53248, 32736, 16352, 0],
    [6240, 7264, 5664, 4896, 4512, 6368, 6240, 0],
    [0, 192, 192, 4092, 7998, 4098, 4098, 0],
    [0, 0, 0, 8190, 8190, 0, 0, 0],
    [0, 4098, 4098, 7998, 4092, 192, 192, 0],
    [4, 6, 2, 6, 4, 6, 2, 0],
    [2032, 3096, 6604, 4644, 4644, 6476, 3096, 2032],
];

/// Checked conversion from the game-logic i32 pixel domain to a buffer
/// index: a negative position is a bug, not data, so fail loudly here
/// instead of wrapping to ~2^64 and panicking obscurely at the index.
pub(crate) fn buf_index(pos: i32) -> usize {
    usize::try_from(pos).expect("negative buffer index")
}

fn video_set_pixel(pixel: &mut Pixel, pos: i32, ink: u8) {
    pixel.ink = ink;
    system_set_pixel(buf_index(pos), ink);
}

pub(crate) fn video_pixel_has_willy(pixel: &Pixel) -> bool {
    pixel.flags.has_willy()
}

pub(crate) fn video_text_width(text: &[u8]) -> i32 {
    // Same semantics as the old CStr::from_ptr: stop at the first NUL byte.
    let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
    text[..end]
        .iter()
        .map(|&ch| i32::from(CHAR_SET[usize::from(ch)][0]))
        .sum()
}

// game.c and title.c
pub(crate) fn video_cycle_colours() -> u8 {
    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    for (pos, pixel) in pixels.iter_mut().enumerate() {
        let ink = (pixel.ink + 3) & 0xf;
        let pos = i32::try_from(pos).expect("framebuffer pos fits i32");
        video_set_pixel(pixel, pos, ink);
    }
    pixels[0].ink
}

// gameover.c, game.c
pub(crate) fn video_pixel_paper_fill(pos: i32, size: i32, ink: u8) {
    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    for p in pos..pos + size {
        let pixel = &mut pixels[buf_index(p)];
        if !pixel.flags.contains(PixelFlags::LEVEL) {
            video_set_pixel(pixel, p, ink);
        }
    }
}

// game.c is still using this
pub(crate) fn video_pixel_ink_fill(pos: i32, size: i32, ink: u8) {
    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!
    for p in pos..pos + size {
        let pixel = &mut pixels[buf_index(p)];
        if pixel.flags.contains(PixelFlags::LEVEL) {
            video_set_pixel(pixel, p, ink);
        }
    }
}

pub(crate) fn video_pixel_fill(pos: i32, size: i32) {
    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!
    for p in pos..pos + size {
        let pixel = &mut pixels[buf_index(p)];
        pixel.flags = PixelFlags::EMPTY;
        video_set_pixel(pixel, p, 0x0);
    }
}

pub(crate) fn video_draw_rope_seg(pos: i32, ink: u8) {
    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!
    video_draw_rope_seg_inner(&mut pixels, pos, ink);
}

pub(crate) fn video_draw_rope_seg_inner(pixels: &mut [Pixel], pos: i32, ink: u8) {
    let pixel = &mut pixels[buf_index(pos)];
    pixel.flags = PixelFlags::LEVEL;
    video_set_pixel(pixel, pos, ink);
}

// robots.c
pub(crate) fn video_draw_arrow(pos: i32, dir: i32) {
    let width = WIDTH;
    let mut pos = pos + dir;

    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    pixels[buf_index(pos)].flags |= PixelFlags::ROBOT | PixelFlags::LEVEL;
    video_set_pixel(&mut pixels[buf_index(pos)], pos, 7);
    pixels[buf_index(pos + 6)].flags |= PixelFlags::ROBOT | PixelFlags::LEVEL;
    video_set_pixel(&mut pixels[buf_index(pos + 6)], pos + 6, 7);
    pos += width;
    pixels[buf_index(pos + width)].flags |= PixelFlags::ROBOT | PixelFlags::LEVEL;
    video_set_pixel(&mut pixels[buf_index(pos + width)], pos + width, 7);
    pixels[buf_index(pos + width + 6)].flags |= PixelFlags::ROBOT | PixelFlags::LEVEL;
    video_set_pixel(&mut pixels[buf_index(pos + width + 6)], pos + width + 6, 7);
    pos -= dir;
    for i in 0..8 {
        let p = pos + i;
        let pixel = &mut pixels[buf_index(p)];
        pixel.flags |= PixelFlags::ROBOT | PixelFlags::LEVEL;
        video_set_pixel(pixel, p, 7);
    }
}

pub(crate) fn video_draw_tile(tile: i32, what: [u8; 8], paper: u8, ink: u8) {
    let mut pos = tile_2_pixel(tile) + 7;
    let colour = [paper, ink];

    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    for byte in what {
        for bit in 0..8 {
            let d = (byte >> bit) & B_LEVEL;
            let p = pos - bit;
            let pixel = &mut pixels[buf_index(p)];
            pixel.flags = PixelFlags::from_bits(d);
            video_set_pixel(pixel, p, colour[usize::from(d)]);
        }
        pos += WIDTH;
    }
}

// miner.c still needs this
pub(crate) fn video_draw_miner(pos: i32, line: &[u16; 16], level: i32) -> bool {
    let attr = [0x8u8, 0x8, 0x8, 0x1];
    let mut die = false;

    let mut pos = pos & !7;
    let y0 = pos / WIDTH;
    pos += 15;

    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    for (row, word) in line.iter().copied().enumerate() {
        let row_i = i32::try_from(row).expect("sprite row < 16");
        let ink = attr[usize::try_from((y0 + row_i) >> level).expect("attr index 0..=3")];
        for bit in 0..16 {
            if ((word >> bit) & 1) != 0 {
                let p = pos - bit;
                let pixel = &mut pixels[buf_index(p)];
                if pixel.flags.contains(PixelFlags::ROBOT) {
                    die = true;
                }
                pixel.flags |= PixelFlags::WILLY | PixelFlags::LEVEL;
                video_set_pixel(pixel, p, ink);
            }
        }
        pos += WIDTH;
    }

    die
}

// robots.c needs this
pub(crate) fn video_draw_robot(pos: i32, line: &[u16; 16], ink: u8) {
    let mut pos = pos + 15;

    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    for &word in line {
        for bit in 0..16 {
            if ((word >> bit) & 1) != 0 {
                let p = pos - bit;
                let pixel = &mut pixels[buf_index(p)];
                pixel.flags |= PixelFlags::ROBOT | PixelFlags::LEVEL;
                video_set_pixel(pixel, p, ink);
            }
        }
        pos += WIDTH;
    }
}

pub(crate) fn video_draw_robot_i32(pos: i32, line: &[i32; 16], ink: u8) {
    // The C original passed an int[16] where a u16* was expected, so the
    // raster lines are the interleaved low/high halves of the first 8 i32s
    // (all values are < 2^16, so the high halves are zero). Reproduce that
    // byte layout exactly.
    let mut words = [0u16; 16];
    for (i, &v) in line[..8].iter().enumerate() {
        words[2 * i] = u16::try_from(v & 0xffff).expect("low 16 bits");
        words[2 * i + 1] = u16::try_from((v >> 16) & 0xffff).expect("high 16 bits");
    }
    video_draw_robot(pos, &words, ink);
}

pub(crate) fn video_draw_sprite(pos: i32, line: &[u16; 16], paper: u8, ink: u8) {
    let colour = [paper, ink];
    let mut pos = pos + 15;

    let mut pixels = VIDEO_PIXEL.lock().unwrap(); // WARN: Lock!

    for &word in line {
        for bit in 0..16 {
            let d = u8::try_from((word >> bit) & 1).expect("bit is 0/1");
            let p = pos - bit;
            let pixel = &mut pixels[buf_index(p)];
            pixel.flags = PixelFlags::from_bits(d);
            video_set_pixel(pixel, p, colour[usize::from(d)]);
        }
        pos += WIDTH;
    }
}

fn text_code(text: &[u8], ink: &mut [u8; 2]) -> bool {
    match text[0] {
        0x1 => {
            ink[0] = text[1];
            true
        }
        0x2 => {
            ink[1] = text[1];
            true
        }
        _ => false,
    }
}

// used by game.c
pub(crate) fn video_write_large(x: i32, y: i32, text: &[u8]) {
    let mut x = x;
    let pos = y * WIDTH;

    let text = &text[..find_string_end(text)];

    // WARN: Locks!
    let mut pixels = VIDEO_PIXEL.lock().unwrap();
    let mut ink = TEXT_INK.lock().unwrap();

    let mut i = 0;
    while i < text.len() {
        if text_code(&text[i..], &mut ink) {
            i += 2; // two-byte jump
            continue;
        }

        // Space is the offset for start of alphabet
        let ch = usize::from(text[i] - b' ');
        let byte = &CHAR_SET_LARGE[ch];
        for line in byte.iter().take(CHAR_WIDTH).copied() {
            if (0..WIDTH).contains(&x) {
                let start = pos + x;
                for row in 0..i32::try_from(CHAR_HEIGHT_LARGE).expect("char height fits i32") {
                    let p = start + row * WIDTH;
                    let pixel = &mut pixels[buf_index(p)];
                    let bit = usize::from((line >> row) & 1);
                    video_set_pixel(pixel, p, ink[bit]);
                }
            }
            x += 1;
        }
        i += 1;
    }
}

// Length of the NUL-terminated string at the start of `text`. A NUL right
// after a 0x01/0x02 control byte is that control's argument, not the
// terminator, so control+argument pairs are skipped (as in the C original).
fn find_string_end(text: &[u8]) -> usize {
    let mut i = 0;
    while i < text.len() {
        match text[i] {
            0 => break,
            0x1 | 0x2 if i + 1 >= text.len() => break, // dangling control byte
            0x1 | 0x2 => i += 2,                       // skip control + data byte
            _ => i += 1,
        }
    }
    i
}

// game.c
pub(crate) fn video_write(pos: i32, text: &[u8]) {
    let text = &text[..find_string_end(text)];

    // WARN: Locks
    let mut pixels = VIDEO_PIXEL.lock().unwrap();
    let mut ink = TEXT_INK.lock().unwrap();

    let mut pos = pos;
    let mut i = 0;

    while i < text.len() {
        if text_code(&text[i..], &mut ink) {
            i += 2; // In this case we have to jump 2 bytes
            continue;
        }

        let ch = usize::from(text[i]);
        let row = &CHAR_SET[ch];
        let width = usize::from(row[0]);
        for line in row.iter().take(width + 1).skip(1).copied() {
            for idx in 0..i32::try_from(CHAR_HEIGHT).expect("char height fits i32") {
                let p = pos + idx * WIDTH;
                let pixel = &mut pixels[buf_index(p)];
                let bit = usize::from((line >> idx) & 1);
                pixel.flags = PixelFlags::from_bits(u8::try_from(bit).expect("bit is 0/1"));
                video_set_pixel(pixel, p, ink[bit]);
            }
            pos += 1;
        }
        i += 1;
    }
}
