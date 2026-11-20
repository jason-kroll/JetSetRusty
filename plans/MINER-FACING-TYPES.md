# PLAN: type Willy's movement state (Facing / moving / Conveyor)

Task: eliminate the last raw-int flags in gameplay logic - the deferred
item from plans/ENUMS-AS-INTEGERS.md ("miner.dir's D_RIGHT/D_LEFT int
consts and update_dir's dir += 1 / dir += 2 flag accumulation"):

1. **`Miner.dir: i32` -> `Facing` enum.** New private enum in miner.rs:
   `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] enum Facing
   { #[default] Right, Left }` - the #[default] lands on Right because
   C's D_RIGHT=0 and Miner's derived Default must stay zero-diff.
   C's D_RIGHT=0/D_LEFT=1 become variants; the int consts at
   miner.rs:25-26 are deleted. The struct fields themselves are retyped:
   `dir: i32` -> `dir: Facing` (game.rs:109), `move_: i32` ->
   `moving: bool` (game.rs:110). Sites:
   - miner.rs MINER_ZERO :103-104 (`Facing::Right`, `false`);
     save/restore miner.rs:169,184 (plain copies, retyped).
   - miner.rs:375 move_left_right `== D_RIGHT` -> `== Facing::Right`;
     update_dir :461-472 flips -> `Facing::Left`/`Facing::Right`.
   - Sprite packing miner.rs:615 `(miner.dir << 2) | miner.frame` is
     replaced by an explicit method - NOT the discriminant (Direction's
     Right/Left are 1/3, and the packing needs 0/4):
     `impl Facing { fn sprite_row(&self, frame: usize) -> usize {
       match self { Facing::Right => frame, Facing::Left => 4 + frame } } }`
     and `let row = miner_frame() + miner.dir.sprite_row(usize::try_from(
     miner.frame).expect("frame 0..=3"));` - MINER_SPRITE is [[u16;16];16]
     with base MINER_FRAME 0 or 8, so rows base+0..7, facing offset 0/4.
     Pin test: sprite_row(0..3) = 0..3 right, 4..7 left.
   - rope.rs:253-256 `dir ^ willy_dir` (both 0/1 ints) ->
     `dir ^ i32::from(miner.dir == Facing::Left)` (bool->i32 From, no cast).
2. **`Miner.move_: i32` -> `moving: bool`** (renamed from the keyword-
   dodging `move_`). The port only ever assigns 0/1 and tests ==0/!=0
   (verified by grep; C original miner.c likewise only uses 0/1). Sites:
   game.rs:110 field; MINER_ZERO:104; save/restore :170,185; miner.rs:367
   (`if miner.move_ == 0` -> `if !miner.moving`), :459-483 (update_dir),
   :514, :576, :657 (`= 0` -> `= false`, `= 1` -> `= true`); rope.rs:250
   (`miner.move_ != 0` -> `miner.moving`).
3. **Conveyor ints -> `Conveyor` enum.** miner.rs:29-31 C_NONE/C_LEFT/
   C_RIGHT (0/1/2) -> `#[derive(Debug, Clone, Copy, PartialEq, Eq)] enum
   Conveyor { None, Left, Right }`. Sites: GroundOutcome::Land field
   (:301), ground_check construction (:324-331), update_dir param and
   compares (:440,444,452), do_miner_ticker :489, the Land arm :561-563,
   test constructors :868,879,888.
4. **update_dir's flag accumulator -> pure core.** The `let mut dir = 0;
   dir += 1 / dir += 2` int is replaced by two bools and a pure function:
   ```rust
   fn decide_movement(left: bool, right: bool, dir: Facing, moving: bool)
       -> (Facing, bool) {
       match (left, right) {
           (false, false) => (dir, false),
           (true, false) if dir == Facing::Right => (Facing::Left, false),
           (true, false) => (dir, true),
           (false, true) if dir == Facing::Left => (Facing::Right, false),
           (false, true) => (dir, true),
           // both pressed: C's if-chain has no arm for dir==3, so both
           // fields stay unchanged
           (true, true) => (dir, moving),
       }
   }
   ```
   update_dir computes `left`/`right` from keys/joy/conveyor/mode exactly
   as today, calls decide_movement, assigns the pair, then the jump block
   is unchanged. Unit tests for decide_movement covering all four
   (left,right) combos x both facings.

Hard constraints: pure refactor - all 24 golden replays byte-identical,
never `UPDATE_GOLDEN=1` (every scenario exercises this physics). Gates
after every phase: `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings` (watch: no new `as` casts - the four cast lints plus
unwrap_used are denied), `cargo test` (54 unit + new pins, 24 replay),
`cargo build`. Do not push; one commit per phase, imperative mood, no
type prefix, 72-col body, trailer. `Direction` (game.rs:92-98) and
`level_dir`'s `dir as usize` map index stay untouched (separate concern).

## Status: COMPLETE

Five commits, all gates green after each (fmt, clippy `-D warnings`,
cargo test, build); goldens byte-identical throughout (never
`UPDATE_GOLDEN=1`); unit count 54 -> 56 (+2 pin tests:
facing_sprite_row, decide_movement):

- `7a3c100` this plan (reviewer accept, wording nits folded in)
- `1c8f977` Phase 1 — `Facing` enum for `Miner.dir`, `sprite_row`
  method replacing the `(dir<<2)|frame` packing, rope xor bit via
  `i32::from(dir == Facing::Left)`, D_* consts deleted, pin test
- `4ca7721` Phase 2 — `move_` -> `moving: bool`, 13 sites
- `a801dca` Phase 3 — `Conveyor` enum replacing C_NONE/C_LEFT/C_RIGHT
- `4ed21db` Phase 4 — `decide_movement` pure core replacing update_dir's
  flag accumulator, (true,true) fall-through preserved, pin test
- Phase 5 (this commit) — ENUMS-AS-INTEGERS.md non-goal cross-ref;
  plan marked COMPLETE

## Phase 1 - Facing enum for Miner.dir (+ sprite_row, rope bit); Miner field dir: i32 -> dir: Facing (game.rs:109), #[default] on Facing::Right

## Phase 2 - move_ -> moving: bool

## Phase 3 - Conveyor enum for convey_dir

## Phase 4 - update_dir pure core decide_movement + pin tests

## Phase 5 - docs: ENUMS-AS-INTEGERS.md non-goal line points here; plan
COMPLETE with commit list

## Risks

- sprite_row mapping must be exactly Right->0..3 / Left->4..7 (the old
  D_RIGHT=0/D_LEFT=1 << 2 packing); a swap changes every Willy frame -
  the goldens' video column would catch it immediately.
- rope.rs `dir ^ willy_dir`: the willy bit must be 1 iff facing Left
  (old D_LEFT=1).
- decide_movement (true,true) arm must leave BOTH fields unchanged (old
  code had no arm for the combined flag).
- Miner keeps derive(Default): Facing gets #[default] Right, bool false.
- save/restore and MINER_STORE are plain field copies - retype only.

## Size

P1: game.rs+miner.rs+rope.rs, ~25 lines + pin test. P2: ~12 sites, bool
rename. P3: ~10 sites. P4: update_dir rewrite + tests. P5: docs.
Each phase an independent commit with all gates green.
