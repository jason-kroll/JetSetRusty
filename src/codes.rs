// jetset willy

use crate::common::Key::{Enter, Escape, K1, K2, K3, K4};
use crate::common::{HEIGHT, WIDTH};
use crate::dispatch::{Pending, Screen, TitlePhase, queue_pending};
use crate::game_main::{do_quit, game_input, system_border, system_rnd, video_flash};

use crate::video::{video_draw_robot_i32, video_pixel_fill, video_write, video_write_large};
use std::cell::RefCell;

thread_local! {
    static CODES_STATE: RefCell<CodesState> = const { RefCell::new(CodesState::new()) };
}

pub(crate) fn do_codes_responder() {
    CODES_STATE.with(|c| c.borrow_mut().do_codes_responder());
}

pub(crate) fn do_codes_ticker() {
    CODES_STATE.with(|c| c.borrow_mut().do_codes_ticker());
}

pub(crate) fn do_codes_drawer() {
    CODES_STATE.with(|c| c.borrow_mut().do_codes_drawer());
}

/// The codes screen's enter work, run when Pending::Enter(Screen::Codes) is
/// consumed; the screen itself is installed by the dispatcher.
pub(crate) fn codes_enter() {
    CODES_STATE.with(|c| c.borrow_mut().codes_enter());
}

pub(crate) struct CodesState {
    pub(crate) attempt: i32,
    pub(crate) needed: i32,
    pub(crate) pos: i32,
    pub(crate) pos_last: i32,
    pub(crate) code: [i32; 4],
    pub(crate) key: i32,
    pub(crate) cell: [u8; 7],
}

impl CodesState {
    pub(crate) const fn new() -> Self {
        Self {
            attempt: 1,
            needed: 0,
            pos: 0,
            pos_last: 0,
            code: [0; 4],
            key: 0,
            cell: [0x01, 0x00, 0x02, 0x07, 0x14, 0x14, 0x00],
        }
    }

    fn code_val(&self, p: usize, s: i32) -> i32 {
        (self.code[p] - 1) << s
    }

    fn get_code(&mut self) {
        let mut location: [u8; 5] = [0x02, 0x07, b' ', b' ', 0x00];

        self.needed = system_rnd() % 180;

        location[2] = u8::try_from(self.needed % 18).expect("needed < 180 so %18 < 18") + b'A';
        location[3] = u8::try_from(self.needed / 18).expect("needed < 180 so /18 < 10") + b'0';

        video_write_large(29 * 8, 8 * 8, &location[..]);

        self.needed = CODES_DIGIT[usize::try_from(self.needed).expect("needed 0..179")];

        self.code = [0; 4];
        self.key = -1;
        self.pos = 0;
        self.pos_last = 0;

        self.cell[0] = 0x01;
        self.cell[2] = 0x02;

        draw_cursor(&self.code, &mut self.cell, 1);
        draw_cursor(&self.code, &mut self.cell, 2);
        draw_cursor(&self.code, &mut self.cell, 3);
    }

    fn do_codes_drawer(&mut self) {
        self.cell[3] = 0x07;
        draw_cursor(&self.code, &mut self.cell, self.pos);

        if self.key == 0 {
            return;
        }

        self.cell[3] =
            u8::try_from(self.code[usize::try_from(self.pos_last).expect("cursor pos 0..=3")])
                .expect("code char fits u8");
        draw_cursor(&self.code, &mut self.cell, self.pos_last);

        self.key = 0;
    }

    fn do_codes_ticker(&mut self) {
        if video_flash() {
            self.cell[0] = 0x02;
            self.cell[2] = 0x01;
        } else {
            self.cell[0] = 0x01;
            self.cell[2] = 0x02;
        }

        if self.key < 1 {
            return;
        }

        self.pos_last = self.pos;
        self.code[usize::try_from(self.pos).expect("cursor pos 0..=3")] = self.key;
        self.pos = (self.pos + 1) & 3;
    }

    // b"" byte-string literals carry the interior nul bytes these control strings need
    fn do_codes_responder(&mut self) {
        match game_input() {
            K1 => self.key = 1, // blue
            K2 => self.key = 2, // red
            K3 => self.key = 3, // magenta
            K4 => self.key = 4, // green

            Enter => {
                if self.code[3] == 0 {
                    return;
                }

                let entered = self.code_val(0, 6)
                    | self.code_val(1, 4)
                    | self.code_val(2, 2)
                    | self.code_val(3, 0);

                if entered == self.needed {
                    queue_pending(Pending::Enter(Screen::Title(TitlePhase::Init)));
                    return;
                }

                if self.attempt == 2 {
                    do_quit();
                    return;
                }

                video_write_large(
                    0,
                    8 * 8,
                    b"\x01\x00\x02\x05Sorry, try code at location     \x00",
                );
                self.attempt = 2;
                self.get_code();
            }

            Escape => do_quit(),

            _ => {}
        }
    }

    // b"" byte-string literals carry the interior nul bytes these control strings need
    fn codes_enter(&mut self) {
        system_border(0);
        video_pixel_fill(0, WIDTH * HEIGHT);
        video_write_large(
            0,
            8 * 8,
            b"\x01\x00\x02\x05Enter Code at grid location     \x00",
        );
        video_write(88 * WIDTH + 3 * 8 - 1, c"\x02\x07\x15".to_bytes_with_nul());
        video_draw_robot_i32(88 * WIDTH + 2 * 8, &CODES_SPRITE[0], 1);
        video_write(88 * WIDTH + 6 * 8 - 1, c"\x15".to_bytes_with_nul());
        video_draw_robot_i32(88 * WIDTH + 5 * 8, &CODES_SPRITE[1], 2);
        video_write(88 * WIDTH + 9 * 8 - 1, c"\x15".to_bytes_with_nul());
        video_draw_robot_i32(88 * WIDTH + 8 * 8, &CODES_SPRITE[2], 3);
        video_write(88 * WIDTH + 12 * 8 - 1, c"\x15".to_bytes_with_nul());
        video_draw_robot_i32(88 * WIDTH + 11 * 8, &CODES_SPRITE[3], 4);

        self.get_code();
    }
}

// These are chars but original code is ints
// static not const for large arrays in case of multiple inlining
#[rustfmt::skip]
static CODES_DIGIT: [i32; 180] = [
    43, 76, 15, 123, 206, 101, 35, 212, 99, 39, 8, 55, 204, 37, 1, 32, 2, 81,
    44, 202, 222, 181, 47, 83, 74, 179, 90, 45, 154, 27, 165, 71, 44, 238, 124, 65,
    228, 159, 217, 233, 237, 71, 102, 67, 46, 4, 238, 89, 30, 113, 29, 93, 30, 117,
    88, 217, 120, 36, 250, 185, 243, 93, 66, 98, 99, 100, 101, 102, 190, 232, 130, 9,
    77, 104, 132, 40, 140, 138, 24, 13, 109, 20, 87, 114, 33, 113, 129, 23, 15, 35,
    164, 121, 153, 228, 189, 93, 141, 153, 100, 129, 138, 40, 8, 128, 121, 115, 106, 64,
    148, 132, 59, 190, 142, 92, 85, 155, 133, 96, 69, 163, 99, 163, 94, 187, 103, 165,
    132, 76, 218, 159, 68, 26, 157, 112, 2, 60, 82, 211, 168, 173, 112, 205, 192, 112,
    208, 114, 180, 117, 212, 86, 89, 202, 178, 102, 209, 190, 26, 155, 202, 107, 24, 190,
    160, 109, 112, 29, 195, 210, 141, 118, 62, 180, 141, 213, 181, 134, 138, 115, 208, 118
];

#[rustfmt::skip]
const CODES_SPRITE: [[i32; 16]; 4] =
    [
        [ 32766, 49923, 49149, 49149, 65533, 65533, 65533, 63997, 61951, 63999, 63999, 63999, 63997, 63997, 65531, 32766 ],
    [ 32766, 50947, 49149, 49149, 65533, 65533, 65533, 61951, 60671, 64767, 63999, 62463, 59391, 57597, 65523, 32766 ],
    [ 32766, 50691, 49149, 49149, 49149, 65533, 65535, 61951, 60671, 64767, 61951, 64767, 60671, 61949, 65511, 32766 ],
    [ 32766, 50691, 49149, 49149, 49149, 49151, 65535, 65023, 63999, 61951, 59903, 57599, 63999, 63997, 65435, 32766 ]
    ];

fn draw_cursor(code: &[i32; 4], cell: &mut [u8; 7], pos: i32) {
    let pixel = 88 * WIDTH + 16 * 8;
    // In the original code, cell[1] gets mutated. Don't immediately know why or what for.
    cell[1] = u8::try_from(code[usize::try_from(pos).expect("cursor pos 0..=3")])
        .expect("code char fits u8");
    video_write(pixel + pos * 24, &cell[..]);
    video_write(pixel + 8 * WIDTH + pos * 24, &cell[..]);
}

// jetset willy
