[ed: LLM-generated doc file]

# scripts/

One-off generators/transcribers used to produce committed artifacts in `src/`
and `tests/`. None of these are invoked by CI, `build.rs`, or the test suite —
the committed output is what the build and tests use. Where a script is
deterministic, re-running it must reproduce the committed file byte-for-byte.

| Script | Used for | Commit |
|---|---|---|
| `convert_levels.py` | First hop of the level-data provenance chain: transcribed `levelData[60]` from `reference/c-original/src/levels.c` into the flattened `LevelData` literals in `src/levels.rs`. Usage: `python3 convert_levels.py levels.c > level_data.rs`. | initial port |
| `convert_level_dsl.py` | Second hop: converts one flattened room literal to a `level!` DSL block. `python3 scripts/convert_level_dsl.py --scan` reports all 60 rooms' readiness, `<level>` prints the block, `--write <level>` splices it in. Verifies in layers: shape-asserts the literal, re-parses its own emitted block and diffs it field-by-field, cross-checks every field against the C original (asserting the Rust padding is exactly C's zero-fill), and after `--write` re-reads the file to confirm. Used for The Bathroom. | `7df1205`, `8eada91` |
| `convert_top_landing.py` | The one-off that actually converted room 34 (Top Landing) to the DSL. Kept for the record: its output for room 34 is byte-identical to `convert_level_dsl.py 34`, so the general tool is the one to use for the remaining rooms. | `0de7e6b` |
| `gen_robotgfx.py` | Transcribed `robotGfx[45][8][16]` from `robots.c` into the `ROBOT_GFX` static in `src/robots.rs`, padding C's implicit trailing zero frames; re-parses its own output and asserts every C-provided frame survived byte-for-byte. | `048afb7` |
| `gen_robotstart.py` | Transcribed `robotStart[][8]` from `robots.c` into `ROBOT_START` in `src/robots.rs` (depth-aware split, since fields contain commas inside `POS(...)`/`robotGfx[...]`), with the same re-parse-and-verify step. | `048afb7` |
| `gen_sweep.py` | Generated `tests/scenarios/sweep-NN.txt` (one seeded fuzz replay per room, 0–59) that the golden test suite runs. Fixed seed, so re-running is byte-identical. | `e1865a4` |

## Level-data provenance chain

`levels.c` (C original) → `convert_levels.py` → flattened literals in
`src/levels.rs` → `convert_level_dsl.py` → `level!` DSL blocks. Each hop
verifies against its input, and the golden replay tests (`cargo test`) pin
the final behavior: converting a room must leave every golden hash
unchanged (e.g. `sweep-34` covers Top Landing).
