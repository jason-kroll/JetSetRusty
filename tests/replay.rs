// Golden-frame regression tests.
//
// Each tests/scenarios/<name>.txt is replayed headlessly (`jetsetrusty
// --replay`, see src/replay.rs), and its per-frame video and audio hashes must
// match tests/golden/<name>.hashes exactly. Each replay runs in its own
// process, because the game's state is global.
//
// After an intentional change to the game's output, regenerate the goldens:
//
//     UPDATE_GOLDEN=1 cargo test --test replay
//
// and say in the commit why the frames changed.
//
// The tests at the bottom test the harness itself: the comparison logic, and
// that the hashes really respond to input.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn replay(scenario: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_jetsetrusty"))
        .arg("--replay")
        .arg(scenario)
        .output()
        .expect("failed to run jetsetrusty --replay");
    assert!(
        out.status.success(),
        "replay of {} failed: {}",
        scenario.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("replay output is not UTF-8")
}

// Replay a script given as text (written to Cargo's per-test temp dir).
fn replay_text(name: &str, script: &str) -> String {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.txt"));
    std::fs::write(&path, script).unwrap();
    replay(&path)
}

fn scenario_text(name: &str) -> String {
    std::fs::read_to_string(root().join(format!("tests/scenarios/{name}.txt"))).unwrap()
}

#[derive(Debug, PartialEq)]
enum Change {
    Video,
    Audio,
    Both,
    Numbering,
}

#[derive(Debug, PartialEq)]
enum Divergence {
    // The first frame (by line) whose hashes differ.
    Frame { frame: u32, change: Change },
    // `actual` stopped before `expected` did.
    EndedEarly { frame: u32 },
    // `actual` kept going after `expected` ended.
    TooLong { frame: u32 },
}

// Compare two replay outputs (`<frame> <video> <audio>` per line) and find the
// first place they differ, or None if they are identical.
fn first_divergence(expected: &str, actual: &str) -> Option<Divergence> {
    let (mut exp, mut act) = (expected.lines(), actual.lines());
    for n in 0u32.. {
        match (exp.next(), act.next()) {
            (None, None) => return None,
            (Some(_), None) => return Some(Divergence::EndedEarly { frame: n }),
            (None, Some(_)) => return Some(Divergence::TooLong { frame: n }),
            (Some(e), Some(a)) if e == a => {}
            (Some(e), Some(a)) => {
                let (e, a): (Vec<&str>, Vec<&str>) =
                    (e.split(' ').collect(), a.split(' ').collect());
                let change = match (e.get(1) != a.get(1), e.get(2) != a.get(2)) {
                    (true, true) => Change::Both,
                    (true, false) => Change::Video,
                    (false, true) => Change::Audio,
                    (false, false) => Change::Numbering,
                };
                return Some(Divergence::Frame { frame: n, change });
            }
        }
    }
    unreachable!()
}

fn check(name: &str) {
    let scenario = root().join(format!("tests/scenarios/{name}.txt"));
    let golden = root().join(format!("tests/golden/{name}.hashes"));
    let actual = replay(&scenario);

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &actual).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&golden).unwrap_or_else(|e| {
        panic!(
            "{}: {e}\n(generate it with UPDATE_GOLDEN=1 cargo test --test replay)",
            golden.display()
        )
    });
    match first_divergence(&expected, &actual) {
        None => {}
        Some(Divergence::Frame { frame, change }) => {
            let line = |s: &str| s.lines().nth(frame as usize).unwrap_or("").to_string();
            panic!(
                "{name}: first divergence at frame {frame}: {change:?} changed\n  \
                 expected {}\n  actual   {}\n\
                 to see the frame: cargo run -- --replay {} --dump {frame} /tmp/{name}-{frame}.ppm",
                line(&expected),
                line(&actual),
                scenario.display()
            );
        }
        Some(d) => panic!(
            "{name}: {d:?} (golden has {} frames)",
            expected.lines().count()
        ),
    }
}

// ---- Golden scenarios --------------------------------------------------------

#[test]
fn idle() {
    check("idle");
}

#[test]
fn title() {
    check("title");
}

#[test]
fn victory() {
    check("victory");
}

#[test]
fn play() {
    check("play");
}

#[test]
fn death() {
    check("death");
}

#[test]
fn gameover() {
    check("gameover");
}

#[test]
fn rope() {
    check("rope");
}

#[test]
fn conveyors() {
    check("conveyors");
}


// ---- Tests of the harness: comparison logic ----------------------------------

const A: &str = "000000 aaaa 1111\n000001 bbbb 2222\n000002 cccc 3333\n";

#[test]
fn compare_identical() {
    assert_eq!(first_divergence(A, A), None);
}

#[test]
fn compare_video_only() {
    let b = A.replace("bbbb", "BBBB");
    assert_eq!(
        first_divergence(A, &b),
        Some(Divergence::Frame {
            frame: 1,
            change: Change::Video
        })
    );
}

#[test]
fn compare_audio_only() {
    let b = A.replace("3333", "0000");
    assert_eq!(
        first_divergence(A, &b),
        Some(Divergence::Frame {
            frame: 2,
            change: Change::Audio
        })
    );
}

#[test]
fn compare_both() {
    let b = A.replace("aaaa 1111", "AAAA 0000");
    assert_eq!(
        first_divergence(A, &b),
        Some(Divergence::Frame {
            frame: 0,
            change: Change::Both
        })
    );
}

#[test]
fn compare_reports_first_of_several() {
    let b = A.replace("bbbb", "BBBB").replace("cccc", "CCCC");
    assert_eq!(
        first_divergence(A, &b),
        Some(Divergence::Frame {
            frame: 1,
            change: Change::Video
        })
    );
}

#[test]
fn compare_ended_early() {
    let b: String = A.lines().take(2).map(|l| format!("{l}\n")).collect();
    assert_eq!(
        first_divergence(A, &b),
        Some(Divergence::EndedEarly { frame: 2 })
    );
}

#[test]
fn compare_too_long() {
    let b = format!("{A}000003 dddd 4444\n");
    assert_eq!(
        first_divergence(A, &b),
        Some(Divergence::TooLong { frame: 3 })
    );
}

// ---- Tests of the harness: the hashes respond to input -----------------------

// Delaying one keypress must leave every earlier frame identical and change the
// output soon after the original press. (Willy only moves on game ticks, which
// run slower than frames, so allow a few frames' slack.)
#[test]
fn hashes_see_a_delayed_keypress() {
    let original = scenario_text("play");
    assert!(
        original.contains("420 space 1"),
        "play.txt changed; update this test"
    );
    let delayed = original.replace("420 space 1", "421 space 1");
    let (a, b) = (
        replay_text("play-original", &original),
        replay_text("play-delayed", &delayed),
    );
    match first_divergence(&a, &b) {
        Some(Divergence::Frame { frame, .. }) => assert!(
            (420..430).contains(&frame),
            "delayed jump first changed the output at frame {frame}, expected 420..430"
        ),
        other => panic!("delayed jump: expected a frame divergence, got {other:?}"),
    }
}

// The RNG seed picks the code-card challenge, which appears after the loader.
// A different seed must leave the loader identical and change the code card.
#[test]
fn hashes_see_the_rng_seed() {
    let a = replay_text("seed-1", "frames 600\nseed 1\n");
    let b = replay_text("seed-2", "frames 600\nseed 2\n");
    match first_divergence(&a, &b) {
        Some(Divergence::Frame {
            frame,
            change: Change::Video,
        }) => assert!(
            frame > 300,
            "seed changed the output at frame {frame}, before the code card"
        ),
        other => panic!("different seeds: expected a video divergence, got {other:?}"),
    }
}

// The same script must always produce the same output.
#[test]
fn replay_is_deterministic() {
    let script = "frames 400\n100 space\n300 enter\n";
    assert_eq!(replay_text("det-a", script), replay_text("det-b", script));
}
