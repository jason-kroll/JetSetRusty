use crate::misc::{Timer, timer_set, timer_update};
use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

// from misc.h
const SAMPLERATE: i32 = 22050;
const TICKRATE: i32 = 60;

const VOLUME: i32 = 32768 / 4;
const MUSICVOLUME: i32 = VOLUME / 8;
const SFXVOLUME: i32 = VOLUME / 4;

const NCHANNELS: usize = 8;
const NMUSIC: usize = 5;
const NSFX: usize = 3;

const EV_NOTE_OFF: i16 = 0x00;
const EV_NOTE_ON: i16 = 0x10;
const EV_END: i16 = 0x40;

// The event-type nibble (high 4 bits of a score word), decoded from C's
// EV_* consts. The score tables keep the raw i16 consts; only the
// decoder uses this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKind {
    NoteOff,
    NoteOn,
    End,
}

impl EventKind {
    fn from_nibble(v: i16) -> Option<Self> {
        match v {
            EV_NOTE_OFF => Some(EventKind::NoteOff),
            EV_NOTE_ON => Some(EventKind::NoteOn),
            EV_END => Some(EventKind::End),
            _ => None,
        }
    }
}

pub(crate) const MUS_STOP: i16 = 0;
pub(crate) const MUS_PLAY: i16 = 1;

// Matches audio.h MUS_* enum order; also indexes MUSIC_SCORE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MusicScore {
    Title = 0,
    Game = 1,
    Loader = 2,
}

impl MusicScore {
    // Row of MUSIC_SCORE for this score (matches the C enum order); a match,
    // not a cast, so no cast lint fires.
    fn score_row(self) -> usize {
        match self {
            MusicScore::Title => 0,
            MusicScore::Game => 1,
            MusicScore::Loader => 2,
        }
    }
}

// Matches audio.h SFX_* enum order; also indexes SFX_PITCH. SFX_WILLY (4)
// has no variant — the willy handler never reads a pitch table, so in
// the port that row is unreachable, exactly as in C.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sfx {
    Item = 0,
    Die = 1,
    Gameover = 2,
    Arrow = 3,
    None = 5,
}

impl Sfx {
    // Row of SFX_PITCH for this effect (matches the C enum order); a match,
    // not a cast, so no cast lint fires.
    fn pitch_row(self) -> usize {
        match self {
            Sfx::Item => 0,
            Sfx::Die => 1,
            Sfx::Gameover => 2,
            Sfx::Arrow => 3,
            Sfx::None => 5,
        }
    }
}

// musicChannel[i] maps to audioChannel index
const MUSIC_CH: [usize; NMUSIC] = [3, 4, 5, 6, 7];

// ---- structs ----------------------------------------------------------------

struct Channel {
    left: [i32; 3],
    right: [i32; 3],
    phase: u32,
    frequency: u32,
    // Channel on (oscillator running) — was a fn-pointer on/off sentinel.
    // The oscillator itself is inlined in audio_output's mixing loop.
    on: bool,
}

/// What an SFX slot does when its clock fires. Was fn-pointer slots; the
/// handlers take &mut AudioState because
/// audio_output calls them while holding the single AUDIO_STATE guard
/// (std Mutex is not reentrant) — no global CUR_SFX handoff needed beyond
/// the cur_sfx slot index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SfxHandler {
    Off,
    On,
    Play,
    Willy,
}

impl SfxHandler {
    fn run(self, s: &mut AudioState) {
        match self {
            SfxHandler::Off => sfx_off(s),
            SfxHandler::On => sfx_on(s),
            SfxHandler::Play => sfx_play(s),
            SfxHandler::Willy => sfx_willy(s),
        }
    }
}

// Replaces C SFX struct; indices instead of raw pointers.
struct SfxInfo {
    pitch_table: Sfx, // which SFX_PITCH row
    pitch_idx: usize, // current position within that row
    channel: usize,   // index into AudioState.channels
    length: i32,
    clock: i32,
    handler: Option<SfxHandler>,
    play: Option<SfxHandler>,
}

// ---- static data ------------------------------------------------------------

#[rustfmt::skip]
static PAN_TABLE: [i32; 241] = [
    256, 255, 254, 253, 252, 251, 250, 249, 248, 247, 246, 245, 244, 243, 242, 240,
    239, 238, 237, 236, 235, 234, 233, 232, 231, 230, 229, 228, 227, 225, 224,
    223, 222, 221, 220, 219, 218, 217, 216, 215, 214, 213, 212, 210, 209, 208,
    207, 206, 205, 204, 203, 202, 201, 200, 199, 198, 197, 195, 194, 193, 192,
    191, 190, 189, 188, 187, 186, 185, 184, 183, 182, 180, 179, 178, 177, 176,
    175, 174, 173, 172, 171, 170, 169, 168, 167, 165, 164, 163, 162, 161, 160,
    159, 158, 157, 156, 155, 154, 153, 152, 150, 149, 148, 147, 146, 145, 144,
    143, 142, 141, 140, 139, 138, 137, 135, 134, 133, 132, 131, 130, 129, 128,
    127, 126, 125, 124, 123, 122, 121, 119, 118, 117, 116, 115, 114, 113, 112,
    111, 110, 109, 108, 107, 106, 104, 103, 102, 101, 100,  99,  98,  97,  96,
     95,  94,  93,  92,  91,  89,  88,  87,  86,  85,  84,  83,  82,  81,  80,
     79,  78,  77,  76,  74,  73,  72,  71,  70,  69,  68,  67,  66,  65,  64,
     63,  62,  61,  59,  58,  57,  56,  55,  54,  53,  52,  51,  50,  49,  48,
     47,  46,  44,  43,  42,  41,  40,  39,  38,  37,  36,  35,  34,  33,  32,
     31,  29,  28,  27,  26,  25,  24,  23,  22,  21,  20,  19,  18,  17,  16,
     14,  13,  12,  11,  10,   9,   8,   7,   6,   5,   4,   3,   2,   1,   0,
];

#[rustfmt::skip]
static FREQUENCY_TABLE: [u32; 128] = [
    0x00184cbb, 0x0019bea3, 0x001b4688, 0x001ce5bd, 0x001e9da1, 0x00206fae, 0x00225d71, 0x00246891,
    0x002692cb, 0x0028ddfb, 0x002b4c15, 0x002ddf2d, 0x00309976, 0x00337d46, 0x00368d11, 0x0039cb7a,
    0x003d3b43, 0x0040df5c, 0x0044bae3, 0x0048d122, 0x004d2597, 0x0051bbf7, 0x0056982b, 0x005bbe5b,
    0x006132ed, 0x0066fa8b, 0x006d1a25, 0x007396f4, 0x007a7686, 0x0081beba, 0x008975c6, 0x0091a244,
    0x009a4b30, 0x00a377ee, 0x00ad3056, 0x00b77cb7, 0x00c265db, 0x00cdf516, 0x00da344a, 0x00e72de9,
    0x00f4ed0c, 0x01037d74, 0x0112eb8c, 0x01234488, 0x01349660, 0x0146efdc, 0x015a60ad, 0x016ef96d,
    0x0184cbb6, 0x019bea2e, 0x01b46891, 0x01ce5bd2, 0x01e9da1b, 0x0206fae5, 0x0225d719, 0x02468913,
    0x02692cbe, 0x028ddfb9, 0x02b4c15a, 0x02ddf2dc, 0x0309976d, 0x0337d45a, 0x0368d125, 0x039cb7a5,
    0x03d3b434, 0x040df5cc, 0x044bae33, 0x048d1224, 0x04d2597f, 0x051bbf72, 0x056982b5, 0x05bbe5b7,
    0x06132edb, 0x066fa8b7, 0x06d1a249, 0x07396f4b, 0x07a76867, 0x081beb9b, 0x08975c67, 0x091a2448,
    0x09a4b300, 0x0a377ee5, 0x0ad3056f, 0x0b77cb68, 0x0c265db7, 0x0cdf5173, 0x0da3448d, 0x0e72de96,
    0x0f4ed0d9, 0x1037d72a, 0x112eb8ce, 0x1234489d, 0x134965f4, 0x146efdcb, 0x15a60ac1, 0x16ef96f1,
    0x184cbb6f, 0x19bea2c3, 0x1b468941, 0x1ce5bd2c, 0x1e9da187, 0x206fae82, 0x225d719d, 0x24689107,
    0x2692cc1e, 0x28ddfb96, 0x2b4c1582, 0x2ddf2de3, 0x309976df, 0x337d4586, 0x368d1283, 0x39cb7a58,
    0x3d3b430f, 0x40df5d05, 0x44bae33a, 0x48d1220f, 0x4d25983c, 0x51bbf72d, 0x56982bf5, 0x5bbe5ac8,
    0x6132edbe, 0x66fa8c2a, 0x6d1a23d8, 0x7396f4b1, 0x7a768772, 0x81beb8a3, 0x8975c674, 0x91a245b2,
];

// Scores are &[i16] slices; the EV_END + MUS_STOP/MUS_PLAY terminates event parsing.
#[rustfmt::skip]
static MUSIC_SCORE_0: &[i16] = &[
    16, 58, 0, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 0, 0, 3, 1,
    16, 56, 0, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 0, 0, 3, 1,
    16, 54, 0, 19, 66, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 66, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 0, 0, 3, 1, 16, 51, 0, 19, 66, 13, 3, 1, 19, 71, 13, 3, 1, 19, 75, 13, 3, 1, 19, 66, 13, 3, 1, 19, 71, 13, 3, 1, 19, 75, 13, 0, 0, 3, 1,
    16, 53, 0, 19, 65, 13, 3, 1, 19, 69, 13, 3, 1, 19, 75, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 0, 0, 3, 1, 16, 53, 0, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 72, 13, 3, 1, 19, 63, 13, 3, 1, 19, 69, 13, 3, 1, 19, 72, 13, 0, 0, 3, 1,
    16, 46, 0, 17, 53, 0, 19, 61, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 0, 20, 77, 13, 3, 1, 19, 70, 13, 4, 0, 3, 1, 19, 73, 13, 4, 1, 20, 77, 6, 0, 0, 1, 0, 3, 0, 4, 1,
    16, 45, 0, 17, 53, 0, 19, 65, 0, 20, 77, 13, 3, 1, 19, 72, 13, 3, 1, 19, 75, 13, 3, 1, 19, 65, 13, 3, 1, 19, 72, 13, 3, 1, 19, 75, 13, 3, 1, 19, 65, 13, 3, 1, 19, 72, 13, 3, 1, 19, 75, 13, 3, 0, 4, 1, 19, 65, 0, 20, 77, 13, 3, 1, 19, 72, 13, 4, 0, 3, 1, 19, 75, 13, 4, 1, 20, 77, 6, 0, 0, 1, 0, 3, 0, 4, 1,
    16, 46, 0, 19, 65, 0, 20, 77, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 0, 0, 3, 0, 4, 1, 16, 51, 0, 19, 66, 0, 20, 78, 13, 3, 1, 19, 70, 13, 3, 1, 19, 75, 13, 3, 1, 19, 66, 13, 3, 1, 19, 70, 13, 3, 1, 19, 75, 13, 0, 0, 3, 0, 4, 1,
    16, 56, 0, 19, 65, 0, 20, 77, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 0, 0, 3, 0, 4, 1, 16, 56, 0, 19, 66, 0, 20, 75, 13, 3, 1, 19, 68, 13, 3, 0, 4, 1,
    19, 72, 13, 3, 1, 19, 66, 0, 20, 80, 13, 3, 1, 19, 68, 13, 3, 0, 4, 1, 19, 72, 13, 0, 0, 3, 1, 16, 61, 0, 19, 65, 0, 20, 73, 13, 3, 1, 19, 68, 13, 3, 0, 4, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 3, 1, 19, 65, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 0, 0, 3, 1,
    16, 49, 0, 19, 64, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 3, 1, 19, 64, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 3, 1, 19, 64, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 3, 1, 19, 64, 0, 20, 76, 13, 3, 1, 19, 68, 13, 4, 0, 3, 1, 19, 73, 13, 4, 1, 20, 76, 6, 0, 0, 3, 0, 4, 1,
    16, 47, 0, 19, 64, 0, 20, 76, 13, 3, 1, 19, 68, 13, 3, 1, 19, 74, 13, 3, 1, 19, 64, 13, 3, 1, 19, 68, 13, 3, 1, 19, 74, 13, 3, 1, 19, 64, 13, 3, 1, 19, 68, 13, 3, 1, 19, 74, 13, 3, 0, 4, 1, 19, 64, 0, 20, 76, 13, 3, 1, 19, 68, 13, 4, 0, 3, 1, 19, 74, 13, 4, 1, 20, 76, 6, 0, 0, 3, 0, 4, 1,
    16, 45, 0, 19, 64, 0, 20, 76, 13, 3, 1, 19, 69, 13, 0, 0, 3, 1, 19, 73, 13, 3, 1, 19, 64, 0, 16, 44, 13, 3, 1, 19, 68, 13, 3, 1, 19, 73, 13, 0, 0, 3, 1, 19, 64, 0, 16, 43, 13, 3, 1, 19, 70, 13, 3, 1, 19, 73, 13, 3, 0, 4, 1, 19, 63, 0, 20, 75, 13, 3, 1, 19, 70, 13, 3, 0, 4, 1, 19, 73, 13, 3, 0, 0, 1,
    16, 44, 0, 19, 63, 0, 20, 75, 13, 3, 1, 19, 68, 13, 3, 1, 19, 71, 13, 3, 1, 19, 63, 13, 3, 1, 19, 68, 13, 3, 1, 19, 71, 13, 0, 0, 3, 0, 4, 1, 16, 49, 0, 19, 64, 0, 20, 76, 13, 3, 1, 19, 68, 13, 0, 0, 3, 0, 4, 1, 19, 70, 13, 3, 1, 19, 61, 0, 20, 73, 0, 16, 52, 13, 3, 1, 19, 68, 13, 3, 0, 4, 1, 19, 70, 13, 0, 0, 3, 1, 19, 63, 0, 20, 75, 0,
    16, 51, 13, 3, 1, 19, 68, 13, 3, 1, 19, 71, 13, 3, 1, 19, 63, 13, 3, 1, 19, 68, 13, 3, 1, 19, 71, 13, 3, 0, 4, 0, 0, 1, 16, 51, 0, 19, 63, 0, 20, 75, 13, 3, 1, 19, 67, 13, 3, 1, 19, 70, 13, 3, 1, 19, 63, 13, 3, 1, 19, 67, 13, 3, 1, 19, 70, 13, 0, 0, 3, 0, 4, 1,
    16, 56, 0, 19, 68, 13, 3, 1, 19, 71, 13, 3, 1, 19, 75, 13, 3, 1, 19, 68, 13, 3, 1, 19, 71, 13, 3, 1, 19, 75, 13, 3, 1, 19, 68, 83, 0, 0, 3, 1,
    EV_END, MUS_STOP,
];

#[rustfmt::skip]
static MUSIC_SCORE_1: &[i16] = &[
    16, 48, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 1, 0, 3, 1, 16, 48, 0, 19, 64, 23, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 60, 23, 0, 0, 1, 0, 3, 1,
    16, 48, 23, 0, 1, 16, 52, 0, 17, 55, 0, 19, 64, 11, 3, 1, 19, 65, 11, 0, 0, 1, 0, 3, 1, 16, 48, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 1, 0, 3, 1,
    16, 48, 0, 19, 64, 11, 3, 1, 19, 65, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 67, 11, 3, 1, 19, 69, 11, 0, 0, 1, 0, 3, 1, 16, 48, 0, 19, 70, 11, 3, 1, 19, 69, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 70, 11, 3, 1, 19, 69, 11, 0, 0, 1, 0, 3, 1,
    16, 48, 0, 19, 67, 23, 0, 1, 16, 52, 0, 17, 55, 23, 0, 0, 1, 0, 3, 1, 16, 48, 23, 0, 1, 16, 52, 0, 17, 55, 23, 0, 0, 1, 1,
    16, 43, 0, 19, 68, 23, 0, 0, 3, 1, 16, 47, 0, 17, 53, 0, 19, 67, 23, 0, 0, 1, 0, 3, 1, 16, 43, 0, 19, 66, 23, 0, 0, 3, 1, 16, 47, 0, 17, 53, 0, 19, 65, 23, 0, 0, 1, 0, 3, 1,
    16, 48, 0, 17, 51, 0, 19, 63, 11, 3, 1, 19, 62, 11, 0, 0, 1, 0, 3, 1, 19, 60, 11, 3, 1, 19, 62, 11, 3, 0, 0, 1, 16, 48, 0, 19, 60, 0, 20, 63, 23, 0, 1, 16, 46, 23, 0, 0, 3, 0, 4, 1,
    16, 45, 0, 17, 54, 0, 19, 63, 11, 3, 1, 19, 62, 11, 0, 0, 1, 0, 3, 1, 19, 60, 11, 3, 1, 19, 62, 11, 3, 0, 0, 1, 16, 45, 0, 17, 53, 0, 19, 63, 23, 0, 0, 1, 0, 3, 1, 16, 50, 0, 17, 53, 0, 19, 60, 23, 0, 0, 1, 0, 3, 1,
    16, 43, 0, 17, 53, 0, 19, 59, 0, 20, 67, 47, 0, 0, 1, 0, 3, 0, 4, 1, 16, 43, 23, 0, 25, 0, 1,
    16, 48, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 1, 0, 3, 1, 16, 48, 0, 19, 64, 23, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 60, 23, 0, 0, 1, 0, 3, 1,
    16, 48, 23, 0, 1, 16, 52, 0, 17, 55, 0, 19, 64, 11, 3, 1, 19, 65, 11, 0, 0, 1, 0, 3, 1, 16, 48, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 67, 11, 3, 1, 19, 65, 11, 0, 0, 1, 0, 3, 1,
    16, 48, 0, 19, 64, 11, 3, 1, 19, 65, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 67, 11, 3, 1, 19, 69, 11, 0, 0, 1, 0, 3, 1, 16, 48, 0, 19, 70, 11, 3, 1, 19, 69, 11, 0, 0, 3, 1, 16, 52, 0, 17, 55, 0, 19, 70, 11, 3, 1, 19, 69, 11, 0, 0, 1, 0, 3, 1,
    16, 48, 0, 19, 67, 23, 0, 1, 16, 52, 0, 17, 55, 23, 0, 0, 1, 0, 3, 1, 16, 48, 23, 0, 1, 16, 52, 0, 17, 55, 23, 0, 0, 1, 1,
    16, 43, 0, 19, 68, 23, 0, 0, 3, 1, 16, 47, 0, 17, 53, 0, 19, 67, 23, 0, 0, 1, 0, 3, 1, 16, 43, 0, 19, 66, 23, 0, 0, 3, 1, 16, 47, 0, 17, 53, 0, 19, 65, 23, 0, 0, 1, 0, 3, 1,
    16, 48, 0, 17, 51, 0, 19, 63, 11, 3, 1, 19, 62, 11, 0, 0, 1, 0, 3, 0, 19, 60, 11, 3, 1, 19, 62, 11, 0, 0, 3, 1, 16, 48, 0, 19, 60, 0, 20, 64, 23, 0, 1, 16, 46, 23, 0, 0, 3, 0, 4, 1,
    16, 45, 0, 17, 54, 0, 19, 63, 11, 3, 1, 19, 62, 11, 0, 0, 1, 0, 3, 0, 19, 60, 11, 3, 1, 19, 64, 11, 0, 0, 3, 1, 16, 43, 0, 17, 53, 0, 19, 62, 11, 3, 1, 19, 60, 11, 0, 0, 1, 0, 3, 0, 19, 59, 11, 3, 1, 19, 62, 11, 0, 0, 3, 1,
    16, 48, 0, 17, 52, 0, 19, 60, 23, 0, 0, 1, 0, 3, 49,
    EV_END, MUS_PLAY,
];

#[rustfmt::skip]
static MUSIC_SCORE_2: &[i16] = &[
    0, 30, 16, 60, 6, 16, 62, 6, 16, 64, 6, 16, 65, 6, 16, 67, 6, 16, 69, 6, 16, 71, 6, 16, 72, 6, 0, 0,
    EV_END, MUS_STOP,
];

static MUSIC_SCORE: [&[i16]; 3] = [MUSIC_SCORE_0, MUSIC_SCORE_1, MUSIC_SCORE_2];

// sfxPitch rows, terminated by 0. Index matches SFX_* constants.
#[rustfmt::skip]
static SFX_PITCH: [&[i32]; 6] = [
    &[96, 90, 84, 78, 72, 66, 60, 54, 0],                                                        // SFX_ITEM
    &[84, 81, 78, 75, 72, 69, 66, 63, 60, 57, 54, 51, 48, 45, 42, 39, 0],                       // SFX_DIE
    &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55,
      56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75,
      76, 77, 78, 79, 80, 81, 82, 83, 84, 0],                                                    // SFX_GAMEOVER
    &[48, 54, 60, 66, 72, 78, 84, 90, 0],                                                        // SFX_ARROW
    &[0, 0],                                                                                     // SFX_WILLY
    &[0],                                                                                        // SFX_NONE
];

// ---- mutable state ----------------------------------------------------------
// audio_output runs on SDL's audio callback thread, while audio_sfx /
// audio_music / audio_play / audio_init run on the main thread, so the mixer
// state lives behind one mutex. Every critical section is a handful of field
// writes, so the audio thread never blocks for long. In replay mode both run
// lockstep on the main thread, so locking is uncontended there and the output
// stays deterministic. audio_music_playing is also polled by the loader/title
// screens on the main thread, so it stays a plain atomic; audio_pan_x is only
// ever touched on the main thread, so it is a thread-local Cell.

static AUDIO_MUSIC_PLAYING: AtomicBool = AtomicBool::new(false); // MUS_STOP

thread_local! {
    static AUDIO_PAN_X: Cell<i32> = const { Cell::new(0) };
}

pub(crate) fn audio_music_playing() -> bool {
    AUDIO_MUSIC_PLAYING.load(Ordering::Relaxed)
}

// Upstream a53b948 animates the lives icons with musicClock >> 4.
pub(crate) fn audio_music_clock() -> i32 {
    AUDIO_STATE
        .lock()
        .expect("AUDIO_STATE lock poisoned")
        .music_clock
}

pub(crate) fn audio_pan_x() -> i32 {
    AUDIO_PAN_X.with(Cell::get)
}

pub(crate) fn set_audio_pan_x(pan: i32) {
    AUDIO_PAN_X.with(|c| c.set(pan));
}

struct AudioState {
    channels: [Channel; NCHANNELS],
    sfx: [SfxInfo; NSFX],
    // Index of the sfx slot being processed by the SFX handler functions below.
    cur_sfx: usize,
    music_index: MusicScore,
    music_tempo: i32,
    music_pitch: i32,
    music_clock: i32,
    music_delta: i32,
    cur_music_table: MusicScore,
    cur_music_idx: usize,
    samples_music: i32,
    music_channels: usize,
    sfx_clock: i32,
    samples_sfx: i32,
    timer_sfx: Timer,
    timer_music: Timer,
}

static AUDIO_STATE: Mutex<AudioState> = Mutex::new(AudioState::new());

impl AudioState {
    const fn new() -> Self {
        const fn channel(volume: i32) -> Channel {
            Channel {
                left: [volume, -volume, 0],
                right: [-volume, volume, 0],
                phase: 0,
                frequency: 0,
                on: false,
            }
        }
        const fn sfx_slot(channel: usize) -> SfxInfo {
            SfxInfo {
                pitch_table: Sfx::None,
                pitch_idx: 0,
                channel,
                length: 0,
                clock: 0,
                handler: None,
                play: None,
            }
        }
        AudioState {
            channels: [
                channel(0),
                channel(0),
                channel(0),
                channel(MUSICVOLUME),
                channel(MUSICVOLUME),
                channel(MUSICVOLUME),
                channel(MUSICVOLUME),
                channel(MUSICVOLUME),
            ],
            sfx: [sfx_slot(0), sfx_slot(1), sfx_slot(2)],
            cur_sfx: 0,
            music_index: MusicScore::Title,
            music_tempo: TICKRATE,
            music_pitch: 0,
            music_clock: 0,
            music_delta: 0,
            cur_music_table: MusicScore::Title,
            cur_music_idx: 0,
            samples_music: 0,
            music_channels: 0,
            sfx_clock: 0,
            samples_sfx: 0,
            timer_sfx: Timer {
                rate: 0,
                acc: 0,
                remainder: 0,
                divisor: 0,
            },
            timer_music: Timer {
                rate: 0,
                acc: 0,
                remainder: 0,
                divisor: 0,
            },
        }
    }
}

// ---- SFX handlers -------------------------------------------------------------
// Stored in SfxInfo.handler/play as SfxHandler variants; audio_output calls
// them via SfxHandler::run while holding the AUDIO_STATE lock.
// AudioState.cur_sfx selects the slot they operate on.

fn sfx_off(s: &mut AudioState) {
    let i = s.cur_sfx;
    s.sfx[i].handler = None;
    let ch = s.sfx[i].channel;
    s.channels[ch].on = false;
    s.channels[ch].left[2] = 0;
    s.channels[ch].right[2] = 0;
}

fn sfx_play(s: &mut AudioState) {
    let i = s.cur_sfx;
    let pitch = usize::try_from(SFX_PITCH[s.sfx[i].pitch_table.pitch_row()][s.sfx[i].pitch_idx])
        .expect("pitch >= 0");
    let ch = s.sfx[i].channel;
    s.channels[ch].frequency = FREQUENCY_TABLE[pitch];
    let len = s.sfx[i].length;
    s.sfx[i].clock += len;
    s.sfx[i].pitch_idx += 1;
    if SFX_PITCH[s.sfx[i].pitch_table.pitch_row()][s.sfx[i].pitch_idx] > 0 {
        return;
    }
    s.sfx[i].handler = Some(SfxHandler::Off);
}

fn sfx_willy(s: &mut AudioState) {
    let i = s.cur_sfx;
    let ch = s.sfx[i].channel;
    s.channels[ch].on = true;
    let len = s.sfx[i].length;
    s.sfx[i].clock += len;
    s.sfx[i].handler = Some(SfxHandler::Off);
}

fn sfx_on(s: &mut AudioState) {
    let i = s.cur_sfx;
    let ch = s.sfx[i].channel;
    s.channels[ch].on = true;
    s.sfx[i].handler = s.sfx[i].play;
    let handler = s.sfx[i].handler;
    if let Some(h) = handler {
        h.run(s);
    }
}

// ---- helpers ----------------------------------------------------------------

fn channel_stereo(ch: &mut Channel, left: i32, right: i32) {
    let l = (SFXVOLUME * left) >> 8;
    let r = (SFXVOLUME * right) >> 8;
    ch.left[0] = l;
    ch.right[0] = -r;
    ch.left[1] = -l;
    ch.right[1] = r;
}

fn channel_pan(ch: &mut Channel, pan: usize) {
    let p = PAN_TABLE[pan];
    channel_stereo(ch, p, 256 - p);
}

fn music_reset(s: &mut AudioState) {
    for &ch_idx in MUSIC_CH.iter() {
        let ch = &mut s.channels[ch_idx];
        ch.on = false;
        ch.left[2] = 0;
        ch.right[2] = 0;
    }
    s.cur_music_table = s.music_index;
    s.cur_music_idx = 0;
    s.music_delta = 0;
    s.music_clock = 0;
}

// EV_NOTE_OFF: silence one score channel
fn music_note_off(s: &mut AudioState, ch: usize) {
    let channel = &mut s.channels[MUSIC_CH[ch]];
    channel.on = false;
    channel.left[2] = 0;
    channel.right[2] = 0;
}

// ---- public API -------------------------------------------------------------

pub(crate) fn audio_reduce_music_speed() {
    let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");
    s.music_pitch -= 1;
    s.music_tempo -= 6;
    let tempo = s.music_tempo;
    timer_set(&mut s.timer_music, SAMPLERATE, tempo);
}

pub(crate) fn audio_willy_sfx(note: i32, length: i32) {
    let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");
    let ch = s.sfx[0].channel;
    s.channels[ch].frequency = FREQUENCY_TABLE[usize::try_from(note).expect("note >= 0")];
    s.sfx[0].clock = s.sfx_clock;
    channel_pan(
        &mut s.channels[ch],
        usize::try_from(audio_pan_x()).expect("pan >= 0"),
    );
    s.sfx[0].length = length;
    s.sfx[0].handler = Some(SfxHandler::Willy);
}

// Starts an SFX in the given slot: resets the pitch-row cursor, applies
// the channel setup, then marks the slot playing.
fn begin_sfx(
    s: &mut AudioState,
    slot: usize,
    table: Sfx,
    length: i32,
    setup: impl FnOnce(&mut Channel),
) {
    s.sfx[slot].length = length;
    s.sfx[slot].pitch_table = table;
    s.sfx[slot].pitch_idx = 0;
    let ch = s.sfx[slot].channel;
    setup(&mut s.channels[ch]);
    s.sfx[slot].play = Some(SfxHandler::Play);
}

pub(crate) fn audio_sfx(sfx: Sfx) {
    let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");
    let pan = audio_pan_x();

    let target_slot = if sfx == Sfx::Gameover || sfx == Sfx::Die {
        // Schedule channels 0 and 1 to stop; don't call immediately.
        s.sfx[0].handler = Some(SfxHandler::Off);
        s.sfx[1].handler = Some(SfxHandler::Off);
        2
    } else if sfx == Sfx::Arrow {
        2
    } else {
        1
    };

    match sfx {
        Sfx::Item => begin_sfx(&mut s, 1, Sfx::Item, 1, |ch| {
            channel_pan(ch, usize::try_from(pan).expect("pan >= 0"));
        }),
        Sfx::Arrow => begin_sfx(&mut s, 2, Sfx::Arrow, 1, |ch| {
            channel_stereo(ch, 256 - pan, pan);
        }),
        Sfx::Die => begin_sfx(&mut s, 2, Sfx::Die, 1, |ch| {
            channel_pan(ch, usize::try_from(pan).expect("pan >= 0"));
        }),
        Sfx::Gameover => begin_sfx(&mut s, 2, Sfx::Gameover, 2, |ch| {
            channel_stereo(ch, 256, -256);
        }),
        _ => {}
    }

    s.sfx[target_slot].handler = Some(SfxHandler::On);
    s.sfx[target_slot].clock = s.sfx_clock;
}

pub(crate) fn audio_output(output: &mut [i16; 2]) {
    let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");

    if s.samples_music == 0 {
        s.samples_music = timer_update(&mut s.timer_music);

        if audio_music_playing() {
            if s.music_delta == s.music_clock {
                // Process all events due this tick (time==0 events chain immediately).
                loop {
                    let data = MUSIC_SCORE[s.cur_music_table.score_row()][s.cur_music_idx];
                    s.cur_music_idx += 1;
                    let ch = usize::try_from(data & 0x0f).expect("4-bit channel mask");

                    match EventKind::from_nibble(data & 0xf0) {
                        Some(EventKind::NoteOn) => {
                            let note = i32::from(
                                MUSIC_SCORE[s.cur_music_table.score_row()][s.cur_music_idx],
                            ) + s.music_pitch;
                            s.cur_music_idx += 1;
                            s.channels[MUSIC_CH[ch]].frequency =
                                FREQUENCY_TABLE[usize::try_from(note).expect("note >= 0")];
                            s.channels[MUSIC_CH[ch]].on = true;
                        }
                        Some(EventKind::End) => {
                            let playing = i32::from(
                                MUSIC_SCORE[s.cur_music_table.score_row()][s.cur_music_idx],
                            );
                            AUDIO_MUSIC_PLAYING.store(playing != 0, Ordering::Relaxed);
                            music_reset(&mut s);
                            // MUS_PLAY → time=0 → continue loop from top of reset score
                            // MUS_STOP → time=1 → break
                            if playing == i32::from(MUS_STOP) {
                                break;
                            }
                            continue;
                        }
                        // EV_NOTE_OFF, and unknown nibbles: C's switch has no
                        // default case, but the port has always silenced here too
                        Some(EventKind::NoteOff) | None => music_note_off(&mut s, ch),
                    }

                    let time =
                        i32::from(MUSIC_SCORE[s.cur_music_table.score_row()][s.cur_music_idx]);
                    s.cur_music_idx += 1;
                    s.music_delta += time;
                    if time != 0 {
                        break;
                    }
                }
            }
            s.music_clock += 1;
        }
    }

    if s.samples_sfx == 0 {
        s.samples_sfx = timer_update(&mut s.timer_sfx);

        for i in 0..NSFX {
            if s.sfx[i].clock == s.sfx_clock {
                s.cur_sfx = i;
                let handler = s.sfx[i].handler;
                if let Some(h) = handler {
                    h.run(&mut s);
                }
            }
        }

        s.sfx_clock += 1;
    }

    // Mix all active channels into output[0] (L) and output[1] (R).
    let mut out_l: i16 = 0;
    let mut out_r: i16 = 0;

    let music_channels = s.music_channels;
    for ch in s.channels.iter_mut().take(music_channels) {
        if ch.on {
            // Square wave oscillator: phase MSB selects left[0]/left[1]
            let phase = usize::try_from(ch.phase >> 31).expect("0/1");
            ch.left[2] = ch.left[phase];
            ch.right[2] = ch.right[phase];
            ch.phase = ch.phase.wrapping_add(ch.frequency);
        } else {
            ch.left[2] = 0;
            ch.right[2] = 0;
        }
        // Sample amplitudes are bounded by VOLUME (8192), so they fit i16.
        out_l = out_l.wrapping_add(i16::try_from(ch.left[2]).expect("amplitude <= VOLUME"));
        out_r = out_r.wrapping_add(i16::try_from(ch.right[2]).expect("amplitude <= VOLUME"));
    }

    output[0] = out_l;
    output[1] = out_r;

    s.samples_music -= 1;
    s.samples_sfx -= 1;
}

pub(crate) fn audio_play(playing: bool) {
    AUDIO_MUSIC_PLAYING.store(playing, Ordering::Relaxed);
    let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");
    s.music_channels = if playing { NCHANNELS } else { NSFX };
}

pub(crate) fn audio_music(music: MusicScore, playing: bool) {
    {
        let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");
        s.music_index = music;
        music_reset(&mut s);
        s.music_pitch = 0;
        s.music_tempo = TICKRATE;

        s.sfx[0].handler = None;
        s.sfx[1].handler = None;
        s.sfx[2].handler = None;

        s.samples_music = 0;
        timer_set(&mut s.timer_music, SAMPLERATE, TICKRATE);
    }
    audio_play(playing);
}

pub(crate) fn audio_init() {
    let mut s = AUDIO_STATE.lock().expect("AUDIO_STATE lock poisoned");
    timer_set(&mut s.timer_sfx, SAMPLERATE, TICKRATE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_decodes_the_c_nibbles() {
        assert_eq!(EventKind::from_nibble(0x00), Some(EventKind::NoteOff));
        assert_eq!(EventKind::from_nibble(0x10), Some(EventKind::NoteOn));
        assert_eq!(EventKind::from_nibble(0x40), Some(EventKind::End));
        assert_eq!(EventKind::from_nibble(0x20), None);
        assert_eq!(EventKind::from_nibble(0xf0), None);
    }
}
