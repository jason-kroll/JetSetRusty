// Headless, deterministic replay for regression testing.
//
//     jetsetrusty --replay <script> [--dump <frame> <out.ppm>]...
//
// Runs the game with no SDL at all: no window, no audio device, no wall-clock
// pacing. Input comes from the script, the RNG gets a fixed seed, and audio is
// generated in lockstep (exactly one frame's worth of samples per frame), which
// matters because the loader and title screens advance on `audio_music_playing`.
// The same inputs therefore always produce the same frames.
//
// Each frame prints one line to stdout:
//
//     <frame> <video hash> <audio hash>
//
// The video hash covers the palette-index framebuffer plus the border colour;
// the audio hash covers that frame's stereo samples. Both are FNV-1a 64, which
// is stable across Rust versions (unlike std's DefaultHasher). tests/replay.rs
// compares these lines against golden files to prove a refactor changed nothing.
//
// Script format: one directive per line; `#` starts a comment.
//
//     frames 3000        # how many frames to run (required)
//     seed 1             # RNG seed (optional, default 1)
//     120 enter          # press Enter at frame 120 (held for 1 frame)
//     200 right 45       # press Right at frame 200, held for 45 frames
//     400 set mode toilet   # poke GAME_STATE at frame 400 (debug/testing):
//                           #   set item_count N | set level N | set lives N
//                           #   set mode normal|maria|running|toilet
//                           #   set miner X Y   (place Willy at pixel x,y, on the floor)
//
// A press both sends the keydown event (mapped exactly as in live play, so e.g.
// `right` arrives as "any key") and holds the key for system_is_key. Key names:
// a-z, 0-9, left, right, space, enter, lshift, rshift, escape, tab, pause,
// lalt, ralt.

use crate::audio::{audio_init, audio_output};
use crate::common::{HEIGHT, WIDTH};
use crate::dispatch::{GamePhase, Screen, set_screen};
use crate::game::GameMode;
use crate::game_main::{
    SAMPLERATE, TICKRATE, border_index, game_frame, game_running, map_keycode, seed_rng,
    set_replay_held, set_replay_mode, with_framebuffer,
};
use crate::misc::{Timer, VIDEO_COLOUR, timer_set};
// SDL3's SDLK_* constants (SDL_Keycode, a u32 newtype); letters and digits
// are still their ASCII codes, so script key names map exactly as before.
use sdl3::sys::keycode as K;
use std::io::Write;
use std::process::ExitCode;

struct Press {
    frame: u32,
    sym: i32,
    duration: u32,
}

enum SetOp {
    ItemCount(i32),
    Level(i32),
    Mode(GameMode),
    Miner(i32, i32),
    Lives(i32),
}

struct Script {
    frames: u32,
    seed: u32,
    presses: Vec<Press>,
    sets: Vec<(u32, SetOp)>,
}

fn keycode(name: &str) -> Option<i32> {
    let named = match name {
        "left" => K::SDLK_LEFT,
        "right" => K::SDLK_RIGHT,
        "space" => K::SDLK_SPACE,
        "enter" => K::SDLK_RETURN,
        "lshift" => K::SDLK_LSHIFT,
        "rshift" => K::SDLK_RSHIFT,
        "escape" => K::SDLK_ESCAPE,
        "tab" => K::SDLK_TAB,
        "pause" => K::SDLK_PAUSE,
        "lalt" => K::SDLK_LALT,
        "ralt" => K::SDLK_RALT,
        _ => {
            // SDL keycodes for letters and digits are their ASCII codes.
            let mut chars = name.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_lowercase() || c.is_ascii_digit() => Some(c as i32),
                _ => None,
            };
        }
    };
    Some(i32::try_from(named.0).expect("SDL keycode < 2^31"))
}

fn parse_script(text: &str) -> Result<Script, String> {
    let mut frames = None;
    let mut seed = 1;
    let mut presses = Vec::new();
    let mut sets = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let err = |msg: &str| format!("line {}: {msg}: {line}", n + 1);
        let words: Vec<&str> = line.split_whitespace().collect();
        let num = |w: &str| w.parse::<u32>().map_err(|_| err("expected a number"));
        match words.as_slice() {
            ["frames", f] => frames = Some(num(f)?),
            ["seed", s] => seed = num(s)?,
            [f, "set", field, val] => {
                let op = match *field {
                    "item_count" => SetOp::ItemCount(
                        val.parse::<i32>().map_err(|_| err("expected an integer"))?,
                    ),
                    "level" => {
                        SetOp::Level(val.parse::<i32>().map_err(|_| err("expected an integer"))?)
                    }
                    "mode" => SetOp::Mode(match *val {
                        "normal" => GameMode::Normal,
                        "maria" => GameMode::Maria,
                        "running" => GameMode::Running,
                        "toilet" => GameMode::Toilet,
                        _ => return Err(err("mode must be normal|maria|running|toilet")),
                    }),
                    "lives" => {
                        SetOp::Lives(val.parse::<i32>().map_err(|_| err("expected an integer"))?)
                    }
                    _ => return Err(err("unknown field (item_count|level|mode|miner|lives)")),
                };
                sets.push((num(f)?, op));
            }
            [f, "set", "miner", x, y] => {
                let px = x.parse::<i32>().map_err(|_| err("expected an integer"))?;
                let py = y.parse::<i32>().map_err(|_| err("expected an integer"))?;
                sets.push((num(f)?, SetOp::Miner(px, py)));
            }
            [f, key] | [f, key, _] => presses.push(Press {
                frame: num(f)?,
                sym: keycode(key).ok_or_else(|| err("unknown key"))?,
                duration: if words.len() == 3 { num(words[2])? } else { 1 },
            }),
            _ => return Err(err("unrecognised directive")),
        }
    }
    Ok(Script {
        frames: frames.ok_or("script has no `frames` directive")?,
        seed,
        presses,
        sets,
    })
}

// FNV-1a 64.
fn fnv1a(hash: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *hash ^= u64::from(b);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}
const FNV_OFFSET: u64 = 0xcbf29ce484222325;

fn write_ppm(path: &str) -> std::io::Result<()> {
    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    with_framebuffer(|fb| {
        for &index in fb {
            let c = &VIDEO_COLOUR[usize::from(index)];
            out.extend_from_slice(&[c.r, c.g, c.b]);
        }
    });
    std::fs::write(path, out)
}

pub(crate) fn main(args: &[String]) -> ExitCode {
    let usage = "usage: jetsetrusty --replay <script> [--dump <frame> <out.ppm>]...";
    let Some(path) = args.first() else {
        eprintln!("{usage}");
        return ExitCode::from(2);
    };
    let mut dumps: Vec<(u32, String)> = Vec::new();
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match (arg.as_str(), rest.next(), rest.next()) {
            ("--dump", Some(f), Some(out)) if f.parse::<u32>().is_ok() => {
                dumps.push((f.parse().unwrap(), out.clone()))
            }
            _ => {
                eprintln!("{usage}");
                return ExitCode::from(2);
            }
        }
    }
    let script = match std::fs::read_to_string(path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|text| parse_script(&text))
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("replay: {e}");
            return ExitCode::from(2);
        }
    };

    // The same setup live play does, minus SDL.
    set_replay_mode();
    seed_rng(script.seed);
    audio_init();
    let mut timer_flash = Timer::default();
    // rate == 0 here (25/480): timer_update yields only 0/1, the exact bool
    // that toggle_video_flash expects (same invariant as game_main.rs).
    timer_set(&mut timer_flash, 25, TICKRATE * 8);

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let rate = SAMPLERATE as u64;
    let tick = TICKRATE as u64;

    for frame in 0..script.frames {
        if !game_running() {
            break;
        }
        for (f, op) in &script.sets {
            if *f == frame {
                use crate::game::GAME_STATE;
                use std::sync::atomic::Ordering::Relaxed;
                match op {
                    SetOp::ItemCount(v) => GAME_STATE.item_count.store(*v, Relaxed),
                    SetOp::Level(v) => {
                        // Mimic entering the room from the left (game_change_level
                        // Right): Willy at the left edge, room re-inits next tick.
                        {
                            let mut miner = GAME_STATE.miner.lock().unwrap();
                            miner.x = 0;
                            miner.tile &= !31;
                        }
                        GAME_STATE.level.store(*v, Relaxed);
                        // Direct phase write (not Pending): game_init_room must
                        // run at this frame's step 4, like the old TICKER write.
                        set_screen(Screen::Game(GamePhase::Init));
                    }
                    SetOp::Mode(m) => GAME_STATE.set_mode(*m),
                    SetOp::Lives(v) => GAME_STATE.lives.store(*v, Relaxed),
                    SetOp::Miner(x, y) => {
                        let mut miner = GAME_STATE.miner.lock().unwrap();
                        miner.x = *x;
                        miner.y = *y;
                        miner.tile = (y / 8) * 32 + (x / 8);
                        miner.air = 0;
                        miner.align = 4;
                    }
                }
            }
        }
        let held: Vec<i32> = script
            .presses
            .iter()
            .filter(|p| frame >= p.frame && frame < p.frame + p.duration)
            .map(|p| p.sym)
            .collect();
        set_replay_held(Some(held));
        let mut events = script
            .presses
            .iter()
            .filter(|p| p.frame == frame)
            .map(|p| map_keycode(p.sym));
        game_frame(&mut timer_flash, || events.next());

        // Exactly SAMPLERATE samples per TICKRATE frames (367 or 368 each).
        let f = u64::from(frame);
        let samples = (f + 1) * rate / tick - f * rate / tick;
        let mut audio_hash = FNV_OFFSET;
        let mut stereo = [0i16; 2];
        for _ in 0..samples {
            audio_output(&mut stereo);
            fnv1a(&mut audio_hash, &stereo[0].to_le_bytes());
            fnv1a(&mut audio_hash, &stereo[1].to_le_bytes());
        }

        let mut video_hash = FNV_OFFSET;
        with_framebuffer(|fb| fnv1a(&mut video_hash, fb));
        fnv1a(
            &mut video_hash,
            &[u8::try_from(border_index()).expect("palette index fits u8")],
        );

        if writeln!(out, "{frame:06} {video_hash:016x} {audio_hash:016x}").is_err() {
            return ExitCode::from(1);
        }
        for (_, dump_path) in dumps.iter().filter(|(f, _)| *f == frame) {
            if let Err(e) = write_ppm(dump_path) {
                eprintln!("replay: {dump_path}: {e}");
                return ExitCode::from(1);
            }
        }
    }
    set_replay_held(None);
    ExitCode::SUCCESS
}
