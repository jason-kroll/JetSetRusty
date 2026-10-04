# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A Rust port of the 1984 ZX Spectrum game "Jet Set Willy" (originally by Matthew Smith), incrementally porting an existing C/SDL2 implementation. Gameplay is 100% identical to the original. The project adds per-pixel coloring (no color clash), polyphonic music, and stereo SFX.

## Build & Run

```bash
cargo build           # debug build
cargo build --release # optimized
cargo run             # run the game
```

**Required system libraries:** `libsdl2-dev libsdl2-mixer-dev build-essential`

`build.rs` links SDL2 and SDL2_mixer via pkg-config and sets `env!("BUILD")` to a datestamped version string used in the loader screen. **No C is compiled** — the game is 100% Rust.

## Architecture

The C→Rust port is **complete: the game is 100% Rust** (no C is compiled), and as of the latest commits the shared mutable state has been consolidated into a single Rust-owned `GAME_STATE` (a `LazyLock<GameState>`). The transitional C-ABI globals (`cglobals.rs` plus the `miner.rs` globals) have been **dissolved** and the two-world sync layer that mirrored them is down to one last field (`cheatEnabled`); what physically remains of them is vestigial scaffolding awaiting a teardown commit (see *Migration status* below). Both the entry point (`main.rs`) and the main loop (`game_main.rs::run`) live in Rust; `game_main.rs` owns SDL2 window/audio setup and drives the loop via four global function pointers:

```
Action    — current game state handler (runs once per frame, then sets itself to DoNothing)
Responder — input processor
Ticker    — logic/physics update
Drawer    — rendering
```

These are `#[unsafe(no_mangle)] pub static mut` globals defined in `game_main.rs` (and re-exported by `common.rs`); sibling modules reference them via `extern` (a holdover from the C ABI — the symbols are Rust-defined now). Each state transition works by re-assigning the pointer (e.g., `Action = Some(Title_Action)`). The `Event` type is `Option<unsafe extern "C" fn()>`, so an unset slot is `None` and behaves as a no-op.

### Ported to Rust (`src/*.rs`)

| Module | Role |
|--------|------|
| `main.rs` | Entry point — calls `game_main::run()` |
| `game_main.rs` | SDL2 init, main loop, audio callback, keyboard polling, the four function-pointer globals |
| `common.rs` | `WIDTH=256`, `HEIGHT=192`, `Key` enum (`#[repr(i32)]`), `Event` type alias, re-exports of `game_main` globals, extern decls for any unported C `*_Action` |
| `video.rs` | Full rendering engine — `videoPixel` buffer, sprite drawing, two character sets (8px/128-char and 16px/96-char) |
| `misc.rs` | 16-color `videoColour` palette (`#[no_mangle]`), `Timer` struct, `Video_Viewport` |
| `cheat.rs` | "writetyper" cheat detection, level selection (1–60) |
| `die.rs` | Death animation and life-loss sequence |
| `audio.rs` | Square-wave synth — 8-channel mixer (3 SFX + 5 music), polyphonic sequencer, stereo panning |
| `title.rs` | Title screen — JSW logo, scrolling ticker, starts music |
| `gameover.rs` | Game over animation — boot kicks Willy, then returns to title |
| `loader.rs` | Loading screen — flashing text, loading bar, copy protection flow |
| `levels.rs` | Level data and room layout definitions |
| `rope.rs` | Rope swing physics and rendering |
| `codes.rs` | Copy-protection code entry and validation |
| `game.rs` | Port of `game.c`/`game.h` — complete (no `unimplemented!()` stubs). Owns `Game_Action`, `DoGameTicker`, `do_game_drawer`, `game_init_room`, `clock_ticker`, `Game_ChangeLevel`, `Game_GameReset`, `game_pause`, and the `GAME_STATE` model (see below) — now the single source of truth for shared game state. The `sync_c_to_rust`/`sync_rust_to_c` bridges still exist, now mirroring only `cheatEnabled` ↔ `GAME_STATE.cheat_enabled` — the last two-copy field (see *Migration status*). Calls robot physics (`Robots_*`) — the Rust impls in `robots.rs`, via C-ABI extern decls. |
| `miner.rs` | Port of `miner.c` — Willy physics (`Miner_*`): input, jump/fall/walk, ramps/conveyors, collision, item pickup, sprite rendering. Willy's state (`minerWilly` / `minerWillyRope` / `minerAttrSplit`) has been **dissolved into `GAME_STATE`** (the `miner` mutex plus the `miner_air` / `miner_jump` / `miner_willy_rope` / `miner_attr_split` atomic caches); no shared globals remain here. The `Miner_*` entry points keep their C ABI only because `game.rs`/`die.rs` call them by name. |
| `robots.rs` | Port of `robots.c` — the guardians: per-room hostile sprites, the toilet, Maria, the two screen-crossing arrows. Robot state (`robotThis[8]`, the `curRobot` cursor) was file-static and is now Rust-private (no `#[repr(C)]`/`#[no_mangle]`); only the five `Robots_*` entry points keep their C ABI, since `game.rs`/`title.rs` still call them by name. Now reads shared state (`GAME_STATE.miner`, `level`, `mode`, `clock_ticks`) **directly from `GAME_STATE`** — the old `c_*` alias reads are gone. (Its file-header comment still describes the old alias path and is **stale**.) |
| `cglobals.rs` | **Orphaned, pending `git rm`.** All 13 ex-`game.c` globals have been dissolved into `GAME_STATE`; `mod cglobals;` was dropped from `main.rs` (commit `0ef7c71`), so the file is no longer compiled but is still tracked on disk (comments only). |

### Migration status — Milestone 2 substantively complete

`build.rs` compiles **no** C files, and every shared mutable global except `cheatEnabled` has now been dissolved into `GAME_STATE`:

- `cglobals.rs` — empty and no longer a module; the 13 ex-`game.c` globals (`gameLevel`, `gameLives`, `levelBorder`, `gameScoreClock`, `gameTimer`, …) all live in `GAME_STATE`.
- `common.rs` — the `c_*` `#[link_name]` alias block is empty; **no `#[link_name]` aliases remain anywhere in `src/`.**
- `miner.rs` — `minerWilly` / `minerWillyRope` / `minerAttrSplit` dissolved into `GAME_STATE`.

What physically remains is vestigial scaffolding awaiting one final teardown commit:

1. The orphaned `src/cglobals.rs` file (its `mod` line is already gone — just `git rm` it).
2. The empty `c_*` placeholder comment block in `common.rs`.
3. `sync_c_to_rust` / `sync_rust_to_c` in `game.rs` and their ~34 call sites. **Not quite a no-op:** they still mirror `cheat.rs`'s `static mut cheatEnabled` ↔ `GAME_STATE.cheat_enabled`. `game.rs` writes and reads the `GAME_STATE` copy (writetyper sets it), while `title.rs` reads the raw `cheatEnabled` — so `sync_rust_to_c` is what carries the cheat flag back to the title screen. Dissolve `cheatEnabled` first (point `title.rs` at `GAME_STATE.cheat_enabled`, delete the static), *then* delete the sync functions and their call sites.
4. The explicit `{ }` guard-dropping scopes in `game_init_room` (and friends) that existed only to dodge the old sync deadlock — now unnecessary but harmless.
5. Stale comments still describing the two-world model (`robots.rs` header, `common.rs`, parts of `game.rs`).

Beyond that, the whole C-ABI apparatus — `#[unsafe(no_mangle)]` on functions, the `unsafe extern "C"` call decls, the `Event = Option<unsafe extern "C" fn()>` typedef — is now Rust-to-Rust across a C ABI boundary that no longer needs to exist. Shedding it is optional future cleanup, not required for correctness. For this reason `Level_Ticker`/`level_init` (`levels.rs`), `Rope_Ticker`/`Rope_Init` (`rope.rs`), the `Miner_*` functions (`miner.rs`), and the `Robots_*` functions (`robots.rs`) are still called through `unsafe extern "C"` decls from `game.rs`/`title.rs`/`die.rs`.

Note: `src/` is now **pure Rust** — no `.c`/`.h` files remain there. The complete original C/SDL2 implementation lives, buildable, in **`reference/c-original/`**: a pristine snapshot of upstream `github.com/fawtytoo/JetSetWilly` at this repo's `db7fc50` import, with its own `Makefile` (`make` there builds it with gcc + libSDL2 — no mixer). It is **not compiled by Cargo** and is reference only; see `reference/c-original/PROVENANCE.md`. The exact *as-transcribed* version of any ported module stays in git history (e.g. `git show <pre-port-commit>:src/robots.c`).

### GAME_STATE — the single source of truth (read before touching `game.rs`)

`GAME_STATE` is a `LazyLock<GameState>` holding all shared game state; it is now the **authoritative store, not a shadow.** `GameState` mixes cheap `Atomic*` fields (`level`, `lives`, `frame`, `clock_ticks`, `item_count`, `mode`, `music`, `miner_air`, `miner_jump`, `miner_willy_rope`, `miner_attr_split`, …) with `Mutex<>` fields for the compound ones (`miner`, `timer`, `level_border`, `score_clock`, `score_items`). Every reader and writer is Rust and references `GAME_STATE` directly.

**Historical sync model (being torn down):** the port used to keep shared state in C-ABI globals and mirror it into `GAME_STATE` via two bridge functions — `sync_c_to_rust()` (globals → shadow) and `sync_rust_to_c()` (shadow → globals) — bracketing each Rust function that touched shared state. With the other globals dissolved, these bridges now copy only `cheatEnabled` ↔ `GAME_STATE.cheat_enabled` — still load-bearing for `title.rs` until that last global is dissolved (see *Migration status* item 3); after that they and their call sites can be deleted.

**Deadlock rule (now historical — but know it before refactoring):** the old `sync_*` functions locked *every* `Mutex` field, so holding a `GAME_STATE` mutex guard across a sync call self-deadlocked and froze the game. That is why `game_init_room` and friends still wrap guards (`timer`, `level_border`, …) in explicit `{ }` scopes that drop before the sync call. Now that sync touches no mutex the hazard is gone and those scopes are redundant — but if you ever reintroduce a cross-cutting lock-everything helper, the rule applies again. See the `game-state-guard-across-sync-deadlock` memory (likewise now historical).

### FFI Conventions

- Rust functions exported to C: `#[unsafe(no_mangle)]` with C-style names (e.g., `Video_DrawSprite`, `Audio_Init`)
- C functions called from Rust: declared as `unsafe extern "C"` blocks in each Rust file (or `common.rs`)
- Shared globals: `static mut` with `#[unsafe(no_mangle)]`; Rust 2024 edition requires `&raw mut` / `&raw const` to take pointers to them (avoids `static_mut_refs` lint)
- `Key` enum and `Colour` struct are `#[repr(C)]`/`#[repr(i32)]` for ABI compatibility; `Colour` has a `_padding: u8` field to match the C layout

**Module hierarchy for FFI declarations** (from `notes.md`): if an unported C function is used by multiple modules, declare it in `common.rs`. If only one module uses it, declare it locally. As a C file is ported, replace its extern declarations with the real `pub` Rust impl in the natural module — do **not** put FFI decls in a sibling module and import them, which inverts the dependency graph.

### Graphics Model

- Internal 256×192 `videoPixel` array of `Pixel { ink: u8, point: u8 }` structs
- Per-pixel bit flags in `ink`: `B_LEVEL=1`, `B_ROBOT=2`, `B_WILLY=4`
- Text uses embedded control bytes: `\x01` = set paper (next byte = index), `\x02` = set ink
- `TILE2PIXEL(t)` macro converts tile coordinates to pixel buffer offsets

### Audio Model

- `Audio_Output` is called from C's SDL audio callback (22050 Hz, stereo i16)
- 8 channels: indices 0–2 are SFX slots, 3–7 are music channels
- Square wave oscillator: MSB of `phase` (u32) selects between two amplitude values
- Music scores are `&[i16]` slices with events encoded as `(channel | type, note, duration)` triplets; `EV_END` terminates and either loops (`MUS_PLAY`) or stops (`MUS_STOP`)
- `audioPanX` (exported to C) controls stereo position for SFX

## Debugging

Valgrind catches FFI memory mistakes at the remaining C-ABI boundary (the `#[no_mangle]` entry points and `extern "C"` call decls):

```sh
valgrind --error-exitcode=1 ./target/debug/jetsetrusty 2>&1 | head -40
```

## Room reference

`ROOMS.md` is the authoritative numbered list of all 60 rooms with their teleport-code key bindings (used by the "writetyper" cheat). Refer to it when working on level transitions, cheat code handling, or anything that names a specific room.
