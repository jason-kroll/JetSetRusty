// Golden-frame regression tests.
//
// Each tests/scenarios/<name>.txt is replayed headlessly (`jetsetrusty
// --replay`, see src/replay.rs), and its per-frame video and audio hashes must
// match tests/golden/<name>.hashes exactly. Each scenario runs in its own
// process, because the game's state is global.
//
// After an intentional change to the game's output, regenerate the goldens:
//
//     UPDATE_GOLDEN=1 cargo test --test replay
//
// and say in the commit why the frames changed.

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
    if actual == expected {
        return;
    }

    // Report the first frame that differs, and whether it's the picture, the
    // sound, or both.
    let mut actual_lines = actual.lines();
    for exp in expected.lines() {
        let Some(act) = actual_lines.next() else {
            panic!("{name}: replay ended early; golden continues with frame {exp}");
        };
        if act == exp {
            continue;
        }
        let (e, a): (Vec<&str>, Vec<&str>) = (exp.split(' ').collect(), act.split(' ').collect());
        let what = match (e.get(1) != a.get(1), e.get(2) != a.get(2)) {
            (true, true) => "video and audio differ",
            (true, false) => "video differs",
            (false, true) => "audio differs",
            (false, false) => "frame numbering differs",
        };
        let frame = e[0].trim_start_matches('0').parse::<u32>().unwrap_or(0);
        panic!(
            "{name}: first divergence at frame {frame}: {what}\n  expected {exp}\n  actual   {act}\n\
             to see the frame: cargo run -- --replay {} --dump {frame} /tmp/{name}-{frame}.ppm",
            scenario.display()
        );
    }
    panic!("{name}: replay produced more frames than the golden file");
}

#[test]
fn idle() {
    check("idle");
}

#[test]
fn title() {
    check("title");
}

#[test]
fn play() {
    check("play");
}
