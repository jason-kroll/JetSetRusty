# PLAN: narrow-types residual — cast-lint ratchet + PixelFlags newtype

Follow-up to `plans/NARROW-TYPES-AT-BOUNDARIES.md` (kept as history; do NOT
execute it — see premise check). That plan's phases 1–4 were delivered by
later work: `buf_index` (checked `i32 -> usize`) exists at `video.rs:271`,
`system_set_pixel(pos: usize, index: u8)` at `game_main.rs:311`, the
`level_get_tile_type(tile: i32, air: i32, jump: i32)` boundary guard at
`levels.rs:5071`, the u8 `attr` array at `video.rs:393`, and `ROPE.ink:
AtomicU8`. What remains is (a) the lint ratchet (its phase 7, never run) and
(b) the un-grouped stragglers, plus (c) the `PixelFlags` newtype explicitly
deferred by `plans/PIXEL-FLAG-ACCESSOR.md`.

## Premise check (run first, at execution HEAD)

All numbers below were measured at plan time (`6269b7a`). Re-measure before
each phase; if a number moved, trust the tree, not this file:

- `as` cast inventory at HEAD: ~159 integer casts (64 `as usize`, 41 `as
  i32`, 27 `as u8`, 11 `as u32`, 8 `as u64`, 5 `as i16`, 2 `as u16`, 1 `as
  i64`), by grep over `src/`.
- Clippy fire counts with `--all-targets` (each lint alone, `-W`):
  `cast_lossless` 7 (audio.rs:493, :502, :518, replay.rs:159, :271,
  rng.rs:40, video.rs:289), `cast_possible_truncation` 36,
  `cast_possible_wrap` 22, `cast_sign_loss` 61, `cast_precision_loss` 0.
  Sites overlap across lints (e.g. `i32 -> u8` casts fire under both
  truncation and sign-loss), so fixing one lint shrinks the next lint's
  count — re-measure per phase. The `sign_loss` fires are ALL signed→unsigned at plan time
  (i32→usize ×32, i32→u8 ×18, i32→u32 ×8, i32→u16 ×2, i16→usize ×1; zero
  unsigned→signed casts exist in the crate), and lint classification is not
  uniform — masked sites can fire sign_loss only (gameover.rs:64), and
  i32↔usize sites can fire both wrap and truncation (levels.rs:5072) — so
  never predict a remaining count from the previous phase's list; re-run
  with `-W` and recount.
- `Cargo.toml` has `[lints.rust] unreachable_pub = "deny"` only; no
  `[lints.clippy]` table yet.

## Phases (one commit each; gates green after each)

### Phase 1 — deny `cast_lossless`, fix its fires

- `Cargo.toml`: add `[lints.clippy] cast_lossless = "deny"`.
- Fix all 7 fires (audio.rs:493/:502/:518, replay.rs:159/:271, rng.rs:40,
  video.rs:289) with `usize::from(..)` / `u8::from(..)` / `i32::from(..)` —
  mechanical, lossless by definition; `cargo clippy --fix` offers these
  automatically.

### Phase 2 — deny `cast_possible_truncation` + `cast_possible_wrap`

- `Cargo.toml`: add both as `"deny"` (one commit).
- Fix every fire (~58 at plan time across audio/cheat/codes/common/die/
  game_main/game/levels/miner/replay/rng/robots/title/video) by genuine
  retype, never `#[allow]`/`#[expect]` (crate policy: zero suppression
  attributes):
  - provably-in-range values at storage/IO boundaries: `u8::try_from(x)
    .expect("<what> fits u8")` / `usize::try_from(..)` — the checked
    conversion lives at the boundary, once;
  - masked indices (`(data & 0x0f) as usize`) become `usize::from(data &
    0x0f)` (lossless after the mask);
  - `video_draw_robot_i32`'s `v as u16` / `(v >> 16) as u16`
    (`video.rs:447-448`): the halves are provably < 2^16 (comment at
    :441-444) — mask-then-convert, e.g. `u16::try_from(v & 0xffff)
    .expect("low half")`, never a bare `as`;
  - FRAMEBUFFER's `WIDTH as usize * HEIGHT as usize` (`game_main.rs:174`):
    measured at plan time to fire NONE of the four lints (const-eval on
    literals); re-check at execution but expect no change there.

### Phase 3 — deny `cast_sign_loss`

- `Cargo.toml`: add `cast_sign_loss = "deny"`.
- Re-measure fires after phase 2 (the i32→u8 truncation fixes remove
  many). Measured at plan time every sign_loss fire is **signed→unsigned**
  (i32→usize ×32, i32→u8 ×18, i32→u32 ×8, i32→u16 ×2, i16→usize ×1) and the
  crate contains zero unsigned→signed casts, so no keycode recipe applies
  here. Fix each at its boundary: `usize::try_from(v).expect("<named
  invariant>")` for loaded i32 values (e.g. rope.rs:192/:208/:257/:279
  `AtomicI32` loads, gameover tick math), or mask-then-convert
  `u32::from(v & mask)` where the mask provably bounds the value. Never
  silence with an attribute.

### Phase 4 — residual-cast audit (docs-in-plan, code only if a site is real)

- After phases 1–3, every cast that any denied lint fired is gone; the
  survivors are by definition invisible to the four lints. Measured
  candidates at plan time: enum→i32 casts (common.rs:68 and the Key pin
  test — confirmed zero fires), tests/replay.rs:114 `frame as usize`
  (u32→usize, no `From` impl, lint-invisible), and same-type no-ops.
  Classify exactly these with a one-line reason each; if a survivor fires
  a denied lint when re-checked, phase 3 is not done — go back. Append the
  final stays list to this plan's Status section.

### Phase 5 — `PixelFlags(u8)` newtype over `Pixel.point`

- video.rs: add
  `#[derive(Clone, Copy, PartialEq, Eq)] pub(crate) struct PixelFlags(u8)`
  with the `B_LEVEL`/`B_ROBOT`/`B_WILLY` consts kept private and exposed
  only as associated consts/methods (`EMPTY`, `LEVEL`, `ROBOT`, `WILLY`,
  `from_bits(u8)`, `insert`, `contains`, `has_willy()`), plus
  `impl std::ops::BitOr for PixelFlags` (and `BitOrAssign`) so the
  associated consts can be OR'd into `insert(..)`. `Pixel` becomes
  `{ ink: u8, flags: PixelFlags }` — **`ink` stays a raw `u8` on purpose**:
  it is the palette index domain, not a bitfield; only `point` is flags.
  State this in the commit message.
- Convert all 16 `.point` sites in video.rs (:281 predicate, :311/:322
  raw `& 1` → `contains(LEVEL)`, :332 `= 0` → `EMPTY`, :344 `= 1` →
  `LEVEL`, :355-368/:432 `|= B_ROBOT | 1` → `insert(ROBOT | LEVEL)`, :384
  `= d` → `from_bits(d)`, :408 `& B_ROBOT` → `contains(ROBOT)`, :411
  `|= B_WILLY | 1` → `insert(WILLY | LEVEL)`; :464/:562 — their `as u8`
  was fixed in phase 2, wrap the resulting u8 in `from_bits(..)`).
  `video_pixel_has_willy(&Pixel)` stays the pub(crate)
  accessor (rope.rs unchanged) and is implemented via `flags.has_willy()`.
- Behavior identical (same bits, same byte); goldens pin it: the loader
  flash and every fill run in all 24 scenarios, the rope probe in
  `rope.txt`.

### Phase 6 — docs

- AGENTS.md Lints section: list the four denied clippy cast lints next to
  `unreachable_pub`, restating the zero-`#[allow]` policy.
- AGENTS.md Graphics Model bullet: `Pixel { ink: u8, point: u8 }` →
  `Pixel { ink: u8, flags: PixelFlags }`.
- Mark this plan COMPLETE with the commit list and the phase-4 stays list.

## Hard constraints

- Behavior-preserving throughout: **never `UPDATE_GOLDEN=1`**. If any
  golden or unit test diverges at any phase, stop, do not update, report
  the scenario/frame, and revert that phase commit. Gates after every
  phase: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` (53 unit + 24 replay), `cargo build`.
- Zero new `#[allow]`/`#[expect]` attributes and zero `unsafe` — the crate
  currently has none of either; keep it that way. Fix by retype/restructure.
- Do not change `GAME_STATE` field types (separate concern, issue #3).
- One commit per phase; imperative-mood subject ≤50 chars, no type prefix,
  body ≤72 cols, `Generated by Mistral Vibe.` / `Co-Authored-By: Mistral
  Vibe <vibe@mistral.ai>` trailer. Do not push.

## Risks

- `cast_sign_loss`'s 61 fires at plan time are the volume risk; most
  overlap phase-2 fixes, so re-measure before starting phase 3. If a site
  is genuinely unfixable without suppression, STOP and report rather than
  adding an attribute — that would be a policy change for the user. Before
  declaring a site unfixable, try `from_bits` (bit-preserving, e.g. glibc's
  u32→i32 reinterpret in rng.rs), mask-then-`try_from`, or retyping the
  neighbouring field.
- Boundary conversions must not panic live: the values converted with
  `try_from().expect(..)` are provably in range by a named invariant (SDL
  keycodes < 2^31; rng's Schrage result in [0, 2^31) per rng.rs:33-36;
  rope's AtomicI32 loads are small positions), and the expect message
  names that invariant.
- PixelFlags leaves `ink` raw — a deliberately half-typed struct; the
  alternative (newtyping the palette index too) is out of scope.
- Phase ordering matters: enabling a lint as deny with `-D warnings` gates
  means each phase must leave ZERO fires for the lints enabled so far.

## Size

Phase 1: ~10 lines. Phase 2: ~58 fires across 14 files. Phase 3: remainder
of sign_loss after phase 2. Phase 4: audit only. Phase 5: ~16 sites, one
file + the newtype (~25 lines). Phase 6: docs. Goldens byte-identical
throughout.

## Status: DRAFT — pending reviewer pass
