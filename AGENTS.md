# AGENTS.md

This file provides guidance to coding agents when working with code in this repository. It follows the cross-tool `AGENTS.md` convention (read by Vibe, Claude Code, Codex, Gemini CLI, and others) rather than a tool-specific filename; `CONVENTIONS.md` holds the short hard rules.

## What This Is

A Rust port of the 1984 ZX Spectrum game "Jet Set Willy" (originally by Matthew Smith), incrementally porting an existing C/SDL2 implementation. Gameplay is 100% identical to the original. The project adds per-pixel coloring (no color clash), polyphonic music, and stereo SFX.

## Build & Run

```bash
cargo build           # debug build
cargo build --release # optimized
cargo run             # run the game (fullscreen)
cargo run -- --windowed  # run in a resizable window instead
cargo test            # golden-frame regression tests (see *Regression tests*)
```

Dev flags (parsed in `game_main.rs::parse_flags`): `--windowed` opens a resizable window at an integer scale of the full 272x208 screen (game area + border, e.g. 1088x832 on 1080p — see `misc.rs::windowed_size`) instead of fullscreen; the main loop re-runs `video_viewport` every frame against the window's current drawable size, so the game stays letterboxed at 4:3 with the border colour filling the bars whatever shape a tiling WM gives it. `--items N` overrides the remaining-item count at game start. Both compose; under `--replay` they are ignored (replay never initialises SDL, so the golden hashes are unaffected).

**Required tools:** `build-essential cmake` — no system SDL package is needed.

SDL3 comes from the `sdl3` crate's `build-from-source-static` feature: `sdl3-sys` compiles SDL 3.x from source and links it statically, so a fresh clone builds on any Linux (including distros without `libsdl3-dev`, e.g. Ubuntu/Pop!_OS 24.04) and the binary has no runtime SDL dependency. The cost is a slower first build. `build.rs` no longer probes pkg-config; it only sets `env!("BUILD")` to a datestamped version string used in the loader screen. **No C is compiled by this crate** — the game is 100% Rust (all sound comes from the `audio.rs` synth; SDL2_mixer was never used).

## Architecture

The C→Rust port is **complete: the game is 100% Rust** (no C is compiled), and as of the latest commits the shared mutable state has been consolidated into a single Rust-owned `GAME_STATE` (a `LazyLock<GameState>`). The transitional C-ABI globals (`cglobals.rs` plus the `miner.rs` globals) — and finally `cheatEnabled` (commit `0546a8e`) — have **all been dissolved**, and the two-world sync layer that mirrored them has been deleted (see *Migration status* below). Both the entry point (`main.rs`) and the main loop (`game_main.rs::run`) live in Rust; `game_main.rs` owns SDL3 window/audio setup and drives the loop through the screen state machine in `dispatch.rs`:

```
game_frame — one tick: run the queued Pending one-shot, then each input through the screen's responder, then the screen's ticker and drawer
Screen     — the current screen + phase (Loader/Codes/Title/Game/Die/GameOver), matched in dispatch.rs's run_pending/run_responder/run_ticker/run_drawer
Pending    — a one-shot transition queued for the start of the next frame
```

This is one `Copy` `Dispatch { screen, pending }` in a `thread_local! Cell` in `dispatch.rs`, behind safe accessors (`set_screen`, `queue_pending`, `with_dispatch`, plus `set_game_input`/`game_input` and `video_flash`/`toggle_video_flash` in `game_main.rs`); it is main-thread only, so no synchronization is needed. A transition works by assigning the state: `queue_pending(Pending::Enter(Screen::Title(..)))` fires at the next frame's step 1, while `set_screen(..)` is visible to later handler reads in the same frame. (This replaced the C-era `EVENT` fn-pointer slots — `Option<fn()>` globals mutated at runtime — in the `FN-POINTER-DISPATCH` plan.)

### Ported to Rust (`src/*.rs`)

| Module | Role |
|--------|------|
| `main.rs` | Entry point — calls `game_main::run()` |
| `game_main.rs` | SDL3 init/teardown via the `sdl3` crate's safe API (`Sdl`/`Canvas`/textures/`EventPump`/`open_playback_stream`; timer via the `sdl3::timer` free fns), main loop (wall-clock 60 Hz pacer — see *Frame pacing*), audio callback (`AudioCallback` impl → `audio_output`), keyboard polling. Owns `FRAMEBUFFER` (see *Graphics Model*) and `game_frame()`, the per-tick body shared by live play and replay, which calls the `dispatch.rs` handlers. |
| `dispatch.rs` | The screen state machine: `Dispatch { screen, pending }` in one thread-local `Cell`, the `Screen` enum with per-screen phase enums, `Pending` one-shots, and the `run_pending`/`run_responder`/`run_ticker`/`run_drawer` matches that `game_frame` calls |
| `replay.rs` | Headless deterministic replay (`jetsetrusty --replay <script>`) for the regression tests: scripted input, fixed RNG seed, lockstep audio, per-frame FNV-1a hashes of video and audio. No SDL. |
| `common.rs` | `WIDTH=256`, `HEIGHT=192`, `Key` enum |
| `video.rs` | Full rendering engine — `videoPixel` buffer, sprite drawing, two character sets (8px/128-char and 16px/96-char) |
| `misc.rs` | 16-color `VIDEO_COLOUR` palette, `Timer` struct, `video_viewport` |
| `cheat.rs` | "writetyper" cheat detection, level selection (1–60). The cheat flag lives in `GAME_STATE.cheat_enabled` (the old `static mut cheatEnabled` is gone). |
| `die.rs` | Death animation and life-loss sequence |
| `audio.rs` | Square-wave synth — 8-channel mixer (3 SFX + 5 music), polyphonic sequencer, stereo panning |
| `title.rs` | Title screen — JSW logo, scrolling ticker, starts music |
| `gameover.rs` | Game over animation — boot kicks Willy, then returns to title |
| `loader.rs` | Loading screen — flashing text, loading bar, copy protection flow |
| `levels.rs` | Level data and room layout definitions |
| `rope.rs` | Rope swing physics and rendering |
| `codes.rs` | Copy-protection code entry and validation |
| `game.rs` | Port of `game.c`/`game.h` — complete (no `unimplemented!()` stubs). Owns `game_action`, `do_game_ticker`, `do_game_drawer`, `game_init_room`, `clock_ticker`, `game_change_level`, `game_game_reset`, `game_pause`, and the `GAME_STATE` model (see below) — now the single source of truth for shared game state. Calls robot physics (`robots_*`) — the Rust impls in `robots.rs`, imported directly. |
| `miner.rs` | Port of `miner.c` — Willy physics (`miner_*`): input, jump/fall/walk, ramps/conveyors, collision, item pickup, sprite rendering. Willy's state (`minerWilly` / `minerWillyRope` / `minerAttrSplit`) has been **dissolved into `GAME_STATE`** (the `miner` mutex plus the `miner_willy_rope` / `miner_attr_split` atomics); no shared globals remain here. The `miner_*` entry points are plain `pub fn`s called from `game.rs`/`die.rs`, taking `&mut Miner` where the caller already holds the lock. |
| `robots.rs` | Port of `robots.c` — the guardians: per-room hostile sprites, the toilet, Maria, the two screen-crossing arrows. Robot state (`robotThis[8]`, the `curRobot` cursor) is Rust-private in a thread-local `RefCell`; each robot's movement/draw behaviour is a `Move`/`Draw` enum matched in `Robot::step`/`draw`; the five `robots_*` entry points are plain `pub fn`s. Reads shared state (`GAME_STATE.miner`, `level`, `mode`, `clock_ticks`) **directly from `GAME_STATE`**. |

### Migration status — Milestone 2 complete

`build.rs` compiles **no** C files, and **every** shared mutable global has now been dissolved into `GAME_STATE`:

- `cglobals.rs` — deleted (`0ef7c71` dropped the module, `2405a54` the file); the 13 ex-`game.c` globals (`gameLevel`, `gameLives`, `levelBorder`, `gameScoreClock`, `gameTimer`, …) all live in `GAME_STATE`.
- `common.rs` — the `c_*` `#[link_name]` alias block is gone; **no `#[link_name]` aliases remain anywhere in `src/`.**
- `miner.rs` — `minerWilly` / `minerWillyRope` / `minerAttrSplit` dissolved into `GAME_STATE`.
- `cheat.rs` — `cheatEnabled` dissolved into `GAME_STATE.cheat_enabled` (`0546a8e`); `title.rs` reads the atomic directly.

The teardown that followed deleted the vestigial scaffolding: the orphaned `cglobals.rs`, the empty `c_*` placeholder block in `common.rs`, the `sync_c_to_rust`/`sync_rust_to_c` bridges and their 34 call sites, the `{ }` scopes that existed only to drop mutex guards before a sync call, and the stale two-world comments (`robots.rs` header, `game.rs`). Scopes that still matter were kept — e.g. `game_change_level` drops the `miner` guard before `game_init_room()`, because `miner_save` locks `GAME_STATE.miner` itself.

The C-ABI shed is **complete**: there are no `#[unsafe(no_mangle)]` items, no `static mut`, and no `unsafe extern "C"` call decls left in `src/`. All former extern-decl entry points (`level_ticker`/`level_init`, `rope_ticker`/`rope_init`, `miner_*`, `robots_*`, `video_*`, `system_*`, …) are plain `pub fn`s imported with `use`, and no fn-pointer dispatch slots remain anywhere in `src/` (the C `EVENT` type became `dispatch.rs`'s `Screen` state machine; the rope/clock/audio sub-slots became a `bool`, a `bool`, and an `SfxHandler` enum). SDL is used entirely through the `sdl3` crate's safe API, and libc `rand`/`srand` is vendored in safe Rust in `src/rng.rs` (glibc's TYPE_3 generator, pinned to libc's exact sequence by unit tests plus the golden replays), so the crate contains zero `unsafe`. The `#[repr(i32)]`/`#[repr(C)]` attributes on `Key`, `Colour`, `Pixel`, `Miner`, `Timer`, and the game enums are gone too — nothing C-facing needs them. The C-style names are gone as well: every identifier is idiomatic snake_case and the crate has **zero `allow` attributes** (comments still cite the C original's names where that helps parity).

Note: `src/` is now **pure Rust** — no `.c`/`.h` files remain there. The complete original C/SDL2 implementation lives, buildable, in **`reference/c-original/`**: a snapshot of upstream `github.com/fawtytoo/JetSetWilly` at `783d7c1` (its HEAD when last synced; the older `db7fc50` snapshot is in git history), with its own `Makefile` (`make` there builds it with gcc + libSDL2 — no mixer). It is **not compiled by Cargo** and is reference only; see `reference/c-original/PROVENANCE.md`. The exact *as-transcribed* version of any ported module stays in git history (e.g. `git show <pre-port-commit>:src/robots.c`).

### GAME_STATE — the single source of truth (read before touching `game.rs`)

`GAME_STATE` is a `LazyLock<GameState>` holding all shared game state; it is now the **authoritative store, not a shadow.** `GameState` mixes cheap `Atomic*` fields (`level`, `lives`, `frame`, `clock_ticks`, `item_count`, `mode`, `music`, `miner_willy_rope`, `miner_attr_split`, …) with `Mutex<>` fields for the compound ones (`miner`, `timer`, `level_border`, `score_clock`, `score_items`). Every reader and writer is Rust and references `GAME_STATE` directly.

**Historical sync model (deleted):** the port used to keep shared state in C-ABI globals and mirror it into `GAME_STATE` via two bridge functions — `sync_c_to_rust()` (globals → shadow) and `sync_rust_to_c()` (shadow → globals) — bracketing each Rust function that touched shared state. With every global dissolved they became no-ops and were removed. Old commits and memories that mention them describe that era.

**Locking rule:** `GAME_STATE`'s `Mutex` fields are not reentrant — holding a guard across a call that locks the same field self-deadlocks and silently freezes the game. Since the `ATOMIC-MIRRORS-AND-LOCK-CHOREOGRAPHY` plan this is structural, not conventional: the ticker/drawer passes take the one miner lock at the top and pass `&mut Miner`/`&Miner` down (`game_init_room`, `game_change_level`, `miner_save`/`miner_restore`, `miner_ticker`/`miner_drawer`, `robots_ticker`, `rope_draw`), so the borrow checker rejects a re-entrant use at compile time. The `Mutex`es stay (not `RefCell`) because `cargo test` runs unit tests on parallel threads against the shared `static GAME_STATE`, so the fields must be `Sync`; the audio thread never touches `GAME_STATE` (it uses `AUDIO_STATE`). The old `miner_air`/`miner_jump` atomic mirrors of `Miner.air`/`Miner.jump` are gone — `level_get_tile_type` takes `air`/`jump` as parameters. See the `game-state-guard-across-sync-deadlock` memory (historical).

### FFI Conventions

There is no FFI left in our code — the crate contains zero `unsafe`. SDL is used entirely through the `sdl3` crate's safe high-level API (`sdl3::init()`, `Sdl::video()/audio()/event_pump()`, `render::create_renderer` → `Canvas` + `Texture::with_lock`/`set_blend_mode`/`set_scale_mode`, `EventPump::poll_event` + `keyboard_state`, `AudioSubsystem::open_playback_stream` with an `AudioCallback` impl that pushes samples via `put_data_i16`, and `sdl3::timer::ticks()`/`delay()` — the crate generates the required `extern "C"` audio trampoline internally, so the only `unsafe` anywhere is inside the `sdl3` crate itself). See *SDL3 port notes* below for the behavior changes vs SDL2 that we pin explicitly. libc `rand`/`srand` is vendored in safe Rust in `src/rng.rs` (glibc's TYPE_3 generator; replay determinism is preserved because unit tests there pin reference vectors captured from the real libc `rand`, and the golden replays confirm it in-game).

Everything else is Rust-to-Rust and idiomatically named: there are no `allow` attributes anywhere in `src/`, and nothing is exported or called through a C ABI. When adding code, prefer safe Rust types at module boundaries.

### Graphics Model

- Internal 256×192 `videoPixel` array of `Pixel { ink: u8, point: u8 }` structs
- Per-pixel bit flags in `ink`: `B_LEVEL=1`, `B_ROBOT=2`, `B_WILLY=4`
- Text uses embedded control bytes: `\x01` = set paper (next byte = index), `\x02` = set ink
- `TILE2PIXEL(t)` macro converts tile coordinates to pixel buffer offsets
- Output path: `video.rs` → `system_set_pixel(pos, palette_index)` → `FRAMEBUFFER` (a 256×192 thread-local `[u8]` of palette indices in `game_main.rs`). Once per frame `present_frame()` converts the whole buffer to RGB on the SDL texture. The buffer persists across frames because the game only redraws pixels that change. The palette (`VIDEO_COLOUR`) is an immutable `static`, and the border is a separate palette index (`system_border`).

### Audio Model

- `audio_output` is called from `game_main.rs`'s `AudioCallback` impl on SDL's audio thread (22050 Hz, stereo i16; SDL3 streams have no fixed buffer size — the callback fills the requested sample count and pushes it with `put_data_i16`). The audio thread paces itself by sample consumption, independent of the frame loop
- 8 channels: indices 0–2 are SFX slots, 3–7 are music channels
- Square wave oscillator: MSB of `phase` (u32) selects between two amplitude values
- Music scores are `&[i16]` slices with events encoded as `(channel | type, note, duration)` triplets; `EV_END` terminates and either loops (`MUS_PLAY`) or stops (`MUS_STOP`)
- `audio_pan_x` (a thread-local `Cell` in `audio.rs`, main-thread only) controls stereo position for SFX; `audio_music_playing` is an `AtomicI32` polled by the loader/title screens. The rest of the mixer state lives behind one `Mutex<AudioState>` shared with the main thread

### Frame pacing

`game_main.rs::run` throttles the main loop with a **wall-clock pacer** adapted from C's `System_StartFrame`: each iteration sets `frame_time = now + 1000` and spins on `sdl3::timer::delay(1)` until the millisecond clock (`sdl3::timer::ticks() * TICKRATE`, already u64 in SDL3) catches up — 60 iterations/second, one game tick each (`timer_set(&timer_frame, TICKRATE, TICKRATE)`). The arithmetic lives in pure fns (`pacer_should_wait`/`next_frame_time`) pinned by unit tests. **Deliberate deviation from C:** the original does `time += 1000` and accumulates a backlog after any loop stall, then bursts at full speed to catch up (e.g. after alt-tabbing out and back, when vsync blocks `present()` while unfocused — the music keeps playing because the audio thread is independent). We drop the backlog instead, so the game resumes at 60 Hz rather than fast-forwarding. The renderer still defaults to vsync for tear-free output (SDL3 has no `PRESENTVSYNC` flag; `create_renderer` enables it where supported), but **do not rely on vsync for timing**: many drivers/compositors (XWayland, Wayland, driver overrides) silently ignore it, and an earlier vsync-paced port ran the game many times too fast while the music stayed correct (fixed in `5b91ebb`).

### SDL3 port notes

The game moved from SDL2 (`sdl2` crate 0.38, system libsdl2 via pkg-config) to SDL3 (`sdl3` crate 0.22, `build-from-source-static`, SDL 3.4.x). Behavior changes vs SDL2 that we handle explicitly in `game_main.rs`:

- Alpha-format textures default to `BLENDMODE_BLEND` in SDL3 (SDL2: NONE). `present_frame` leaves the alpha byte untouched, so we set `BlendMode::None` on both textures — otherwise the game image vanishes and only the border color shows.
- New textures inherit the renderer's scale mode, which defaults to LINEAR in SDL3; the `SDL_RENDER_SCALE_QUALITY` hint is deprecated and ignored. We set `ScaleMode::Nearest` on both textures to keep the upscale pixel-crisp.
- `SDL_WINDOW_FULLSCREEN_DESKTOP` and the renderer flags (`accelerated`/`present_vsync`/`target_texture`) are gone; plain `fullscreen()` is borderless at desktop resolution and `create_renderer` picks accelerated+vsync by default.
- `SDL_GetTicks` is u64 in SDL3 (the old u32-wrap `Clock` accumulator is gone); audio specs have no fixed sample-buffer size — the `AudioCallback` fills whatever the stream requests.
- Held-key polling uses a parallel `SDL_SCANCODES` table because SDL3's keyboard state is scancode-indexed and the crate's keycode→scancode conversion is `unsafe` (the crate stays zero-`unsafe`).
- Joystick support (ported from upstream `69070b9`) uses the `JoystickSubsystem`: events enabled, first device opened at startup, later devices via `JoyDeviceAdded`. `JoyAxisMotion` on axis 0 beyond ±255 is left/right, any `JoyButtonDown` is fire (jump / start / a one-shot `Key::Else` input). The event→state mapping is the pure `map_joy_event` in `game_main.rs`, pinned by unit tests; the state lives in a thread-local `JOY_STATE` cell that stays zero in replay mode, so goldens are unaffected.
- Keycode *values* are unchanged from SDL2, so `map_keycode`'s arithmetic and the replay scripts are untouched — all golden hashes survived the migration byte-identical.

### Upstream parity — fawtytoo/JetSetWilly @ 783d7c1

`reference/c-original/` is synced to upstream `783d7c1` (HEAD at last sync). We audited everything between the old `db7fc50` snapshot and `783d7c1` against this port: most gameplay fixes were already in the port (rope cluster, miner movement/sprite fixes, pause rework, Shift-based level cheat, copy-protection default, palette, pan table, large font). Ported since the audit: `7c1aed3` (die level underflow), `941841d` (room-above transition), `a53b948`/`783d7c1` (lives animate with the music clock; gameover sprite), `69070b9` (joystick). Deliberately **not** ported:

- **All timing/vsync commits** — `90b5421` ("Rely on VSYNC alone"), `333eba0`, `a0b2283`, `34dfe4f`, `ae2451c`, `30ea641`, `f1881ec`, `119c332`, `82cea95`. Upstream HEAD paces the game solely via vsync, which runs unthrottled (fast Willy) wherever vsync is ignored; our wall-clock pacer (see *Frame pacing*) already does this correctly and is a deliberate deviation.
- `9433dcc` — self-admitted regression ("New level uses immediate drawing (regression)").
- `5729eb8` — SDL2 scale-quality hint; SDL3 side already pinned to `ScaleMode::Nearest`.
- Pure refactors/housekeeping: `static` keywords, renames, system.c→main.c merge, licence headers, data-table reformats (`92c0cc1`, `1d3fb33`, `6494cea`, `6c534d2`), VS/Debian/images/.gitkeep.

Deliberate deviations *beyond* upstream (not ports of any commit): the `--windowed`/`--items` dev flags (see *Build & Run*), and the window-manager key plus any chord containing it is swallowed in `system_get_event` via `swallow_keydown` (`game_main.rs`). Upstream lets every key reach the responder, where `Cheat_Disabled` calls `Game_Pause(0)` on any unrecognised input — so a stray Super press in fullscreen would unpause a paused game (and Super+Tab would toggle pause). The game has no binding for the GUI keys, so swallowing them loses nothing. Replay is unaffected: scripts can't name GUI keys.

### Lints

`CONVENTIONS.md` asks for code to pass `cargo clippy --all-targets -- -D warnings`; the crate has been clean under that since `f807297`. There are no `static mut`s left, but if one ever reappears, beware `cargo clippy --fix`: its `deref_addrof` suggestion rewrites the `&mut *(&raw mut STATIC)` workaround into `&mut STATIC`, which the deny-by-default `static_mut_refs` lint rejects and breaks the build (`4e36bf1`).

The crate is `cargo fmt --check` clean. The hand-aligned data tables (guardian graphics in `robots.rs`, music/SFX tables in `audio.rs`, and friends) are protected with `#[rustfmt::skip]`, so running `cargo fmt` is safe and will not rewrite them.

## Regression tests

`cargo test` runs `tests/replay.rs`: each `tests/scenarios/<name>.txt` is replayed headlessly with `jetsetrusty --replay` (`src/replay.rs`), and its per-frame video+audio hashes must match `tests/golden/<name>.hashes` exactly. The test reports the **first divergent frame** and whether the video, the audio, or both changed. Use this to prove a refactor is behaviour-preserving.

- **Inspect a frame:** `cargo run -- --replay tests/scenarios/play.txt --dump 301 /tmp/f.ppm` (any number of `--dump <frame> <file>`).
- **Intentional output change:** `UPDATE_GOLDEN=1 cargo test --test replay`, and say in the commit why the frames changed. The goldens record current behaviour, bugs included: they detect *change*, not correctness.
- **Script format** (documented at the top of `replay.rs`): `frames N`, optional `seed N`, then `<frame> <key> [<held frames>]` lines, plus `set` directives (`set item_count N`, `set level N`, `set mode normal|maria|running|toilet`, `set miner X Y`) to script arbitrary game state. A press sends the keydown through the same `map_keycode()` as live play *and* holds the key for `system_is_key`.
- **Why audio is in lockstep:** the loader and title screens advance on `audio_music_playing`, which the audio sequencer updates. Replay therefore generates exactly `SAMPLERATE / TICKRATE` samples per frame instead of using the audio thread. Live play still depends on audio-thread timing there.
- **Replay-mode differences from live play:** no SDL at all, a fixed RNG seed, and the loader's version string pinned to `1.0.0 replay` (`build_version()`), since the real one carries the build date. Scenarios: `idle` (loader + code card), `title` (title screen + music), `play` (start a game, walk/jump/pick up an item, mute, pause, Top Landing, escape), `victory` (last item → Maria mode → running Willy → toilet flush → back to title), `death` (guardian collision → die animation → lives decrement → respawn), `gameover` (`set lives 0` + death → boot-kick GAME OVER → title), `rope` (rope grab/swing/climb in On the Roof), `conveyors` (conveyor belt carries Willy in The Bathroom), `cheat` (writetyper code word in First Landing, teleport via held level key + Enter, persistence across games), `harm` (dropped onto First Landing's spike tiles → tile-type death → respawn), `pause-level` (pause + `set level` while paused → the PausedDrawOnce once-draw path), `transitions` (ramp-climb room-above, right-edge room change, stairwell hole room-below), `fuzz` (seed-7 random left/right/space presses, generated from the snippet in `plans/REMAINING-GOLDEN-GAPS.md`; pins determinism under arbitrary input), `sweep-00..59` (the room sweep: one seeded fuzz replay for each of the 60 rooms — ROOMS.md's 0-based numbering — dropping Willy at the left edge of every room, generated by `scripts/gen_sweep.py`). The `set` directives (`item_count`/`level`/`lives`/`mode`/`miner`) are how scenarios reach otherwise-hard states.

## Debugging

No C is compiled by our crate and it contains zero `unsafe`; valgrind still guards the raw SDL calls inside the `sdl3`/`sdl3-sys` crates:

```sh
valgrind --error-exitcode=1 ./target/debug/jetsetrusty 2>&1 | head -40
```

## Room reference

`ROOMS.md` is the authoritative numbered list of all 60 rooms with their teleport-code key bindings (used by the "writetyper" cheat). Refer to it when working on level transitions, cheat code handling, or anything that names a specific room.

## Independent review

After completing each phase or multi-step change, spawn the `reviewer` subagent (a GLM Flash fresh-eyes child; its definition: `~/.vibe/agents/reviewer.toml`) with the artifact under test and the claim it makes — a diff, a plan section, or a golden enumeration — in one message. It reads only what you pass it and reports a verdict with file-and-line evidence; apply only what its verdict proves.
