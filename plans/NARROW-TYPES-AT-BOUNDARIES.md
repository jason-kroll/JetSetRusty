# PLAN: narrow types at the storage/IO boundaries (remove lossy `as` casts)

Goal: keep game logic in `i32` (transient negative positions are real — e.g.
`ny < 0` in `jump_step`, `tile & 512` probes in `miner.rs`), but make the
storage/IO layer speak its own native types (`usize` buffer indices, `u8`
palette/ink values) and convert **once, checked**, at each layer crossing.
This collapses the ~196 silent `as` casts into a handful of choke points and
makes an out-of-range value fail loudly at the boundary instead of wrapping
to ~10^18 and panicking obscurely later.

Hard constraints:

- **Behavior-preserving.** `cargo test` (all 23 golden replays) must pass
  unchanged after every phase. Never run with `UPDATE_GOLDEN=1`. If any hash
  changes, stop, report, and revert the phase — do not "fix" the golden.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` stay
  green after every phase.
- No `unsafe` (CONVENTIONS.md). No new `#[allow]` attributes — the crate
  currently has zero; eliminate casts instead of allowing them.
- One commit per phase so any phase can be reverted independently.
- Do not change `GAME_STATE` field types in this plan (separate concern).

## Cast inventory (current `main`, verified 2026-11-17)

| cast | count | hot spots |
|---|---|---|
| `as usize` | 101 | video.rs 29, miner.rs 18, audio.rs 13, game_main.rs 10, levels.rs 7, rope.rs 5 |
| `as i32` | 47 | common.rs 12 (Key pin test + `letter()`), video.rs 8, game_main.rs 6 |
| `as u8` | 30 | gameover.rs 9, levels.rs 4, codes.rs 4, video.rs 3 |
| `as u32` | 11 | game_main.rs (SDL keycodes, window sizes — IO layer) |
| `as i16` | 5 | audio.rs (music score data — encoding, not domain) |
| `as u16` | 2 | video.rs (robot i32→u16 split) |
| `as u64` | 8 | game_main.rs/replay.rs (pacer and sample-quotient math — legitimate IO math, stays) |

Not all 204 casts are the same class. The buffer-index sites are the real
targets; beyond those: the tile domain is Phase 3's, and the `as u8` in
gameover.rs/codes.rs are masked (`& 0x7`, `% 18`) hence lossless. Masking
casts (`& 0xf`, `& !7`) are fine and stay.

## The boundaries (choke points to type)

1. `system_set_pixel(pos: i32, index: i32)` — `src/game_main.rs:304`, the only
   writer of `FRAMEBUFFER: RefCell<[u8; WIDTH*HEIGHT]>` (`game_main.rs:182`).
   Sole caller: `video_set_pixel` (`video.rs:266`). Should become
   `system_set_pixel(pos: usize, index: u8)`.
2. `system_border(index: i32)` — `game_main.rs:308`, writes `BORDER_INDEX:
   AtomicUsize`. Callers pass literals (`0x0`, `0x1`), `border_value` from
   `GAME_STATE.level_border: Mutex<[i32; 60]>` (`game.rs:147,419`), and
   `video_cycle_colours()` (`title.rs:134`). Should become `index: u8`.
3. `VIDEO_PIXEL: Mutex<Vec<Pixel>>` (`video.rs:29`) — every `pixels[pos as
   usize]` in video.rs/rope.rs is a buffer-index cast.
4. `level_get_tile_type(tile: usize)` (`levels.rs:5062`) already takes
   `usize`; `miner.rs` guards (`tile < 0 || tile == 512` in `solid_check`,
   `miner.rs:214`) then cast `tile as usize` at ~19 sites.
5. `tile_2_pixel(t: i32) -> i32` — `video.rs:19`; tile domain is 0..511, the
   result is a buffer offset.
6. `present_frame` (`game_main.rs:337`) — reads FRAMEBUFFER with `pos as i32`,
   `pitch as i32`, `index as usize`; should do its math in `usize` and use
   `usize::from(index)` (infallible) for `colour_rgb` (`misc.rs:97`).

## Phases

### Phase 1 — checked buffer-index helper (mechanical, largest win)

- Add to `video.rs`:
  `fn buf_index(pos: i32) -> usize { usize::try_from(pos).expect("negative buffer index") }`
  (video.rs owns the pixel-buffer layer; rope.rs indexes `VIDEO_PIXEL`
  directly too, so export `pub(crate)` if that read warrants it).
- Replace the **pixel-buffer index casts** — `pixels[.. as usize]` in
  video.rs (fills, drawers, text writers) and rope.rs, plus the
  `FRAMEBUFFER` write — with `buf_index(..)`. That is ~40 sites, not 100:
  the rest of the `as usize` inventory is a different class and must NOT
  be swept here:
  - tile-domain casts (miner.rs `tile as usize` lookups) — Phase 3;
  - masked table indices (`(data & 0x0f) as usize`, `f_index & f_mask`,
    `CHAR_SET[ch as usize]` with ASCII proven text) — stay;
  - proven-range table indices (`level as usize` into 60-entry tables,
    `level_border[..]`) — stay for now (replay `set level` already
    panics past the table today, unchanged);
  - lossless `u8 -> usize` (replay's `VIDEO_COLOUR[index as usize]`) —
    `usize::from(..)`, cosmetic;
  - SDL IO (`pitch`, pacer) — stays.
- Verify: build, clippy `-D warnings`, fmt, `cargo test` (23/23 unchanged).

### Phase 2 — storage-layer signatures take native types

- `system_set_pixel(pos: usize, index: u8)`; its caller `video_set_pixel`
  (`video.rs:264`) becomes `system_set_pixel(buf_index(pos), ink)` and keeps
  its `pos: i32` (game-logic domain) signature.
- `system_border(index: u8)`; fix the non-literal callers: `game.rs`
  (`level_border` is a GAME_STATE field typed `[i32; 60]` — this plan
  must NOT retype it (see hard constraints); convert the value at the
  call with `u8::try_from(border_value).expect("border palette index")`,
  leaving the field and its `LEVEL_BORDER` initializer as `[i32; 60]`),
  `title.rs`
  (make `video_cycle_colours` return `u8` — `pixels[0].ink` directly
  instead of `as i32`), `levels.rs` (already `.into()` — becomes a plain
  u8 pass).
- `FRAMEBUFFER` declaration: `RefCell<[u8; (WIDTH * HEIGHT) as usize]>` — the
  `as` here is const-eval on provable constants; either leave with a comment
  or compute as `WIDTH as usize * HEIGHT as usize`.
- Rewrite `present_frame` index math in `usize` (`pos / WIDTH as usize *
  pitch + (pos & 255) * 4`), `usize::from(index)` for `colour_rgb`.
- Verify as above.

### Phase 3 — tile-domain boundary takes `i32` (NOT a panicking converter)

**Why not `fn tile_index(tile: i32) -> usize` with an `expect`: transiently
negative tiles are ordinary gameplay, not corruption.** Jumping from the
top row computes `tile + JUMP_INFO[..].tile` with `j.tile = -32` on a
`tile < 32` (Willy standing in the ceiling row), and `jump_step`
(`miner.rs`) only guards `ny < 0` — so `nt < 0` flows into the lookup on
every ceiling jump, wraps to ~10^18 under `as usize`, and lands on
`level_get_tile_type`'s `>= LEVEL_TILES -> Space` guard, yielding Space:
Willy rises through and exits the room above. The engine's own i32-side
guards exist for exactly this domain (`solid_check` early-returns on
`tile < 0 || tile == 512`; `ground_check` on `tile & 512`). The C original
(`reference/c-original/src/levels.c:2535`) reads `levelTile[tile]` with no
guard at all — a plain out-of-bounds read — so the port's negative->Space
behavior is the *safe* variant already pinned by the goldens. A
panicking converter would crash the live game on the first ceiling jump,
and the `transitions` golden exits by ramp-walk, so the goldens could
stay green while play crashes. Do not introduce that helper.

Instead, type the boundary itself and make the out-of-domain rule
explicit and singular:

- `level_get_tile_type(tile: i32) -> TileType` and
  `level_get_tile_ramp(tile: i32) -> TileType` in `levels.rs`, with the
  guard written in the i32 domain *and made intentional*:
  `if tile < 0 || tile >= LEVEL_TILES { return TileType::Space; }`
  (today the negative case reaches the same Space by an accidental
  `usize` wrap — same behavior, now by stated design). After the guard,
  index via `usize::try_from(tile).expect("guarded to 0..511")` — the
  one checked conversion, at the boundary.
- `level_erase_item(tile: i32)`: reachable values are provably `>= 0`
  (the walker only adds positives to `miner.tile`, and erase only runs
  after an Item match that the Space guard makes impossible for
  negatives) — so it can take `i32` and `expect`, or stay `usize`
  with callers converting; taking `i32` kills the last miner.rs casts.
- In `miner.rs`, the ~18 call sites (`level_get_tile_ramp((miner.tile +
  64) as usize)`, the Harm/Item walkers, the `solid_check` /
  `jump_step` / `ground_check` lookups) become plain `tile` /
  `miner.tile + 64` arguments — no casts, no helpers. The lookup type
  in the pure cores becomes `&dyn Fn(i32) -> TileType`, and the
  physics-test stubs (`lookup_of`) mirror the production guard:
  `if i < 0 || i >= 512 { TileType::Space }`.
- `tile_2_pixel(t: i32) -> i32` stays as it is: its callers pass
  enumerated tiles `0..=511` (level_drawer, TITLE_JSW), the math is
  constant-safe, and the result stays in the i32 pixel-domain.
- Verify as above.

### Phase 4 — palette/ink `u8` end-to-end (only where the cast is real)

The masking rule from the inventory governs here: most `as u8` sites are
masked and lossless (`(c & 0x7) as u8`, `(attr & 15) as u8`, `(die_level()
>> 1) as u8` — all provably in range) and are NOT targets. The genuine
palette-domain mistyping to fix:

- `video_draw_miner`'s `let attr = [0x8i32, 0x8, 0x8, 0x1]` (video.rs:381)
  becomes `[u8; 4]`, deleting the `ink as u8` at the `video_set_pixel`
  call and letting the attr-index line stay `usize::from`-free arithmetic.
- `system_set_pixel(.., index: u8)` (Phase 2) already removes
  `index as u8` in game_main.rs:305.
- rope.rs:189 `ROPE.ink.load(..) as u8` feeds a `u8` ink through an i32
  atomic — retype `ROPE.ink` to `AtomicU8` (rope-local state, NOT a
  GAME_STATE field, so it does not collide with the constraints).
- Leave on purpose: codes.rs' digit-state writes (masked, and the
  `self.key: i32` digit state is a declared non-goal), gameover.rs'
  nine masked text-cell writes unless cheaply retyped.
- Verify as above.

### Phase 5 — Key/Keycode domain: ALREADY DONE (stale in the original draft)

This shipped as Phase 3b of `plans/ENUMS-AS-INTEGERS.md` (commit `2ce7d61`,
2026-11-17): `map_keycode` returns `Key`, the `GAME_INPUT` slot holds a
`Key`, `system_is_key` takes a `Key`, and the SDL keycode layer
(`keycode_i32`'s `u32 -> i32`, `SDL_KEYS: [i32; 36]`, the replay-held
symcodes) is the IO boundary this plan already designates as staying.
The remaining `Key .. as i32` casts are the C-parity pin test in
`common.rs` and `Key::letter()`'s internal offset — both by design.

### Phase 6 (optional) — audio note domain

- `MUSIC_SCORE` events are `i16` by design (score encoding; keep the
  `as i16` in the score tables). The 13 audio `as usize` are masked
  channel/row indices (`(data & 0x0f) as usize`, `pitch_table as usize`)
  and stay. The only possibly-worth-it item: `FREQUENCY_TABLE[note as
  usize]` where `note = score + music_pitch` — if Phase 7's lints flag
  it, guard at the `freq(note)` boundary rather than sweeping audio.
  Low value; skip unless Phase 7 forces it.

### Phase 7 — lint enforcement

- Once the above lands, try adding crate-root
  `#![warn(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]`
  and fix whatever remains (aim: zero, no `#[allow]`s). If a handful of
  provably-safe const-eval casts remain, prefer restructuring
  (`usize::from`, masking) over allowing.
- Expected survivors to settle then, not before: the SDL keycode
  `u32 -> i32` in `keycode_i32` (convert via `i32::try_from(..)` or accept
  at the IO layer), the window-size `as u32` builds, and the pacer's
  `as u64`.

## Verification (run after every phase, in this order)

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test        # must be 23/23 with zero golden changes
```

If `cargo test` shows a divergent frame: stop, do not update goldens, report
which scenario/frame changed and revert the phase commit.

## Rollback

Each phase is one commit on top of the previous. To back out phase N:
`git revert <phase-N-commit>` (or reset to the phase N−1 commit if unpushed
and nothing else depends on it). Never rewrite pushed history.

## Corrections applied in review (2026-11-17)

The draft was reviewed against the tree (`d2e6f15`) and the C original;
six corrections were folded into this text:

1. **Phase 3 redesigned (critical).** The draft's panicking `tile_index`
   would crash live play: transiently-negative tiles are ordinary
   gameplay (ceiling jumps; see the phase's rationale), and the current
   wrap-to-Space behavior — an accident of `as usize` landing on the
   `>= 512` guard — is the behavior the engine and the goldens rely on
   (C reads out of bounds there instead). The boundary now takes `i32`
   with an explicit guard, and callers cast nothing.
2. **Phase 5 was stale.** `map_keycode -> Key` / `game_input() -> Key`
   already shipped as `2ce7d61` (plans/ENUMS-AS-INTEGERS.md Phase 3b);
   the phase is now a pointer to that.
3. **Phase 2 contradicted the hard constraints.** Retyping `LEVEL_BORDER`
   to `[u8; 60]` would change a GAME_STATE field; the value now converts
   at the call site via `u8::try_from` instead.
4. **Phase 4 contradicted the intro's masking rule.** It proposed fixing
   masked, lossless casts (`(c & 0x7) as u8`); its real targets are the
   genuinely-mistyped palette bits — `attr: [i32; 4]` in video_draw_miner
   and rope's ink atomic.
5. **Phase 1 scope corrected** to exactly the pixel-buffer index casts
   (~40 sites), with the other `as usize` classes explicitly excluded
   (tile domain, masked, proven-range, lossless, IO).
6. **Inventory refreshed** against `d2e6f15` (per-file drift after the
   enum work; the unlisted `as u64: 8` pacer cast class added).
