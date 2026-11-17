# PLAN: close the remaining golden-replay gaps

Goal: add golden replay scenarios for the gameplay paths that no scenario
covers yet. **Scenarios and tests only — do not change game code.** If you find
a bug while doing this, stop and report it; do not fix it in this plan.

Current coverage (all passing): `idle`, `title`, `play`, `victory`, `death`,
`gameover`, `rope`, `conveyors`, `cheat`.

Gaps to close, in order:

1. `harm.txt` — death by `TileType::Harm` (spikes). Different death path from
   the robot collision in `death.txt`.
2. `transitions.txt` — room-above transition (`miner.y < 0` →
   `game_change_level(Above)`, `src/miner.rs:506`) and room-below transition
   (`tile & 512` → Below, `src/miner.rs:427`).
3. `fuzz.txt` (optional, if 1–2 went smoothly) — seeded random input.

---

## 1. The replay harness (read first)

A scenario is a text file `tests/scenarios/<name>.txt`. Format (see the header
comment in `src/replay.rs`):

```
frames 1200              # total frames to run (required)
seed 1                   # RNG seed (optional, default 1)
100 space                # press Space at frame 100 (held 1 frame)
200 right 45             # press Right at frame 200, held for 45 frames
400 set level 28         # poke GAME_STATE at frame 400:
                         #   set item_count N | set level N | set lives N
                         #   set mode normal|maria|running|toilet
                         #   set miner X Y   (pixel coords, air=0 = standing)
```

Key names: `a-z`, `0-9`, `left`, `right`, `space`, `enter`, `lshift`, `rshift`,
`escape`, `tab`, `pause`, `lalt`, `ralt`. A press both fires the keydown
through the same `map_keycode` as live play and holds the key for
`system_is_key`.

Boot sequence used by every gameplay scenario (copy this):

```
100 space          # skip the code card
300 enter          # start the game (starts in The Bathroom, level 33)
```

Useful level constants (`src/game.rs:49-58`): THEBATHROOM=33, FIRSTLANDING=28,
THEDRIVE=4, ONTHEROOF=18, MASTERBEDROOM=35, COLDSTORE=25, QUIRKAFLEEG=16,
SWIMMINGPOOL=31, THEBEACH=57. Rope rooms: 16, 18, 25, 31, 57.

`Direction` enum (`src/game.rs:86`): `Above=0, Right=1, Below=2, Left=3`, and
each room's `LevelData.map` is `[above, right, below, left]` (see
`src/levels.rs`). E.g. First Landing (28) has `map=[34,27,22,29]`: up→34
(Top Landing), right→27, down→22, left→29.

Robot positions: `ROBOT_START[level]` in `src/robots.rs`; `pos(x, y)` packs as
`y * 256 + x * 8`, i.e. x is in 8px tiles, y in pixels. Willy dies if his
sprite overlaps a robot — when placing him with `set miner`, choose an x clear
of every patrol line in that room.

## 2. Probing toolkit (use this to author scenarios)

### 2a. Frame renderer — save this as `/tmp/render.py`:

```python
#!/usr/bin/env python3
# Render a P6 PPM from `jetsetrusty --replay --dump` as ASCII art.
# usage: python3 /tmp/render.py <file.ppm> [scale]
import sys
PALETTE = [
    (0x00,0x00,0x00),(0x00,0x00,0xFF),(0xFF,0x00,0x00),(0xFF,0x00,0xFF),
    (0x00,0xFF,0x00),(0x00,0xAA,0xFF),(0xFF,0xFF,0x00),(0xFF,0xFF,0xFF),
    (0xCC,0xCC,0xCC),(0x00,0x55,0xFF),(0xAA,0x00,0x00),(0x55,0x00,0x00),
    (0x00,0xAA,0x00),(0x00,0x55,0x00),(0xFF,0x80,0x00),(0x80,0x40,0x00),
]
CHARS = " .#*+oO@%&=X$Ww"
def main():
    path = sys.argv[1]
    scale = int(sys.argv[2]) if len(sys.argv) > 2 else 4
    data = open(path, "rb").read()
    parts = data.split(b"\n", 3)
    w, h = map(int, parts[1].split())
    pix = parts[3]
    rgb2idx = {p: i for i, p in enumerate(PALETTE)}
    for y in range(0, h, scale):
        row = []
        for x in range(0, w, scale):
            off = (y * w + x) * 3
            row.append(CHARS[rgb2idx.get((pix[off], pix[off+1], pix[off+2]), 0)])
        print("".join(row))
main()
```

`%` = Willy (palette 7 is white; Willy's sprite uses it), `#`/`O` etc. are
level graphics/robots. Char (c, r) at scale 4 = pixel (c*4, r*4).

### 2b. Dump frames while probing:

```sh
cd /home/jasonkroll/Code/JetSetRusty
cargo run -q -- --replay /tmp/draft.txt --dump 460 /tmp/f460.ppm --dump 560 /tmp/f560.ppm
python3 /tmp/render.py /tmp/f460.ppm 4
```

### 2c. Level-map parser (find tiles: Harm, conveyors, ramps, floor rows):

```python
import re
src = open('src/levels.rs').read()
m = re.search(r'static LEVEL_DATA: \[LevelData; 60\] = \[(.*?)\n\];', src, re.S)
chunks = re.split(r'\n    LevelData \{', m.group(1))
ch = chunks[1 + LEVEL]                      # LEVEL = room number
title = re.search(r'title: "([^"]+)"', ch).group(1)
mp = [int(x) for x in re.findall(r'-?\d+', re.search(r'map: \[(.*?)\]', ch, re.S).group(1))]
data = [int(t) for t in re.findall(r'-?\d+', re.search(r'data: \[(.*?)\],', ch, re.S).group(1))]
info = re.findall(r'tile_type: TileType::(\w+)', ch)
# data is row-major, 32 cols x 16 rows, 8px tiles: pixel = (col*8, row*8)
for r in range(16):
    print(r, "".join({'Space':'.','Solid':'#','Floor':'=','ConveyL':'<','ConveyR':'>',
        'RampL':'/','RampR':'\\','Harm':'^','Item':'$'}.get(info[data[r*32+c]], '?') if 0 <= data[r*32+c] < len(info) else '?' for c in range(32)))
```

## 3. Scenario 1: `harm.txt` (spike death)

Target code: `src/miner.rs:435` (Harm tile under Willy → `die_action`) and
`src/miner.rs:541` (sprite overlapping Harm → `die_action`).

Concrete starting point: First Landing (level 28) has a spike patch
(`Harm`) at cols 28–30, rows 3–7 → pixels x=224–247, y=24–63, with a platform
below it at y≈112, x=184–247. There is a vertical robot patrolling x=208
(sprite 208–223), so stand at x≈236 to avoid it.

Draft script (probe it, adjust timings after looking at renders):

```
frames 900
100 space          # skip the code card
300 enter          # start the game (The Bathroom)
340 set level 28   # First Landing
360 set miner 236 96   # stand on the right platform (feet at y=112)
420 space 1        # jump up into the spikes above -> Harm death
```

Confirm by rendering frames ~420–520: you should see the ink-fill death
animation (whole screen recolours progressively), then Willy restored at the
room entry. If the jump doesn't reach the spikes, raise the platform y or jump
later; if the robot hits him first, move x. When the death is visually
confirmed, finalize the scenario (section 5).

## 4. Scenario 2: `transitions.txt` (room above + room below)

Targets: `src/miner.rs:506` (`miner.y < 0` → Above) and `src/miner.rs:427`
(`tile & 512` → Below). Also pins the `941841d` room-above special case in
`game_change_level` (`src/game.rs:419`).

- **Above**: First Landing (28) has up→34 and row 0 is open (`Space`) at
  cols 2–9 → pixels x=16–79. Draft:

  ```
  340 set level 28
  360 set miner 40 8     # near the top, under the open ceiling
  380 space 1          # jump; when y < 0 the room changes to 34
  ```

  If Willy falls before the jump registers, press `space` in the same frame as
  or one frame after `set miner`, or stand on a high platform and jump from
  there. Confirm by rendering: after the transition you should see Top
  Landing (34) — note the border colour also changes (it is part of the video
  hash), which the golden records.

- **Below**: First Landing has down→22. The Below trigger needs Willy in the
  bottom rows of the room (`tile + 64 & 512 != 0`). Probe: place him on the
  lowest floor of First Landing and walk. Draft:

  ```
  500 set level 28
  520 set miner 100 112  # bottom rows of the room
  540 right 60           # walk; expect change to room 22
  ```

  Adjust y/x from the level-28 map printout (section 2c) until the render
  shows the room change.

One file `transitions.txt` containing both transitions (above first, then
re-enter 28 and go below) is fine.

## 5. Finalizing a scenario (checklist — do exactly this)

1. Copy the verified draft to `tests/scenarios/<name>.txt` with a header
   comment explaining what it covers.
2. Add a test in `tests/replay.rs`, in the "Golden scenarios" section:
   `#[test] fn harm() { check("harm"); }`
3. Generate the golden: `UPDATE_GOLDEN=1 cargo test -q --test replay harm`
4. Verify it passes on re-run: `cargo test -q --test replay harm`
5. Run the full gates: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --check` (if it fails, run `cargo fmt` and re-check).
6. Commit — one commit per scenario:
   `git add tests/scenarios/<name>.txt tests/golden/<name>.hashes tests/replay.rs`
   Message style: imperative mood, no `type:` prefix, keep the trailer:

   ```
   git commit -m "Add harm golden scenario

   <one-paragraph body: what it covers and how>

   Generated by Mistral Vibe.
   Co-Authored-By: Mistral Vibe <vibe@mistral.ai>
   ```

7. Update the scenario list in `AGENTS.md` (the line starting "Scenarios:")
   with the new names — same commit or a small follow-up.

## 6. Scenario 3 (optional): `fuzz.txt`

Generate a deterministic random-input script with this snippet and commit the
generated file as `tests/scenarios/fuzz.txt`:

```python
import random
random.seed(7)
keys = ["left", "right", "space"]
lines = ["frames 3000", "seed 7", "100 space", "300 enter"]
for f in range(400, 2900, 7):
    lines.append(f"{f} {random.choice(keys)} {random.randint(1, 20)}")
open("tests/scenarios/fuzz.txt", "w").write("\n".join(lines) + "\n")
```

Then finalize as in section 5. Purpose: pins that arbitrary input never
crashes and the hashes stay deterministic. Deaths/room changes during the run
are fine — determinism is what matters.

## 7. Hard rules

- Never regenerate an *existing* golden. `UPDATE_GOLDEN=1` only for the new
  scenario's test. If an existing scenario fails, a behaviour changed — stop
  and report.
- Replay is deterministic: same script + seed → same hashes. If a new test is
  flaky, you changed game code, not the test.
- Do not modify game source files in this plan. Do not push. Commit locally.
- `set miner X Y` sets `air=0` (standing); y is the sprite top, so to stand on
  a floor whose top is at pixel row `r*8`, use `y = r*8 - 16`.
- `set level N` mimics entering from the left (Willy at x=0, room re-inits
  next tick); follow it with `set miner X Y` a few frames later to place him.

## 8. Definition of done

- `tests/scenarios/harm.txt` + golden + test, committed.
- `tests/scenarios/transitions.txt` + golden + test, committed (fuzz optional).
- `cargo test` fully green (unit + all replay scenarios), clippy
  `-D warnings` clean, `cargo fmt --check` clean.
- AGENTS.md scenario list updated.
- Report: which code paths each scenario covers, and anything surprising found.
