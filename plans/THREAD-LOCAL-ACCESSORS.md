# PLAN: delete the thread-local accessor-pair boilerplate

Task: remove the paired free fns hand-written over `thread_local!`
statics and use the statics directly, matching the pattern the
hygiene plan already established for CODES_STATE/TEXT_INK (direct
`.with(...)` at call sites). Inventory (measured at `7bee0c7`):

| module | statics | pair fns (callers) |
|---|---|---|
| title.rs | TEXT_POS, TEXT_FRAME, COLOUR_CYCLE (Cell), TEXT_JSW (RefCell) | text_pos/set_text_pos (3/2), text_frame/set_text_frame (3/3), colour_cycle/set_colour_cycle (2/2), set_text_jsw_byte (4) |
| gameover.rs | BOOT_TICKS, TEXT_GAME, TEXT_OVER | boot_ticks/set_boot_ticks (4/2), set_text_game_byte (4), set_text_over_byte (4) |
| die.rs | DIE_LEVEL | die_level/set_die_level (3/2) |
| game.rs | CLOCK_REDRAW | clock_redraw/set_clock_redraw (1/3) |
| game_main.rs | GAME_INPUT, VIDEO_FLASH | game_input/set_game_input (7/5), video_flash/toggle_video_flash (5/1) - pub(crate) fns |
| audio.rs | AUDIO_PAN_X | audio_pan_x/set_audio_pan_x (2/6) - pub(crate) fns |
| miner.rs | MINER_FRAME, MINER_SEQ_INDEX | miner_frame/set_miner_frame (1/1), miner_seq_index/set_miner_seq_index (1/2) |
| rope.rs | ROPE_ACTIVE | set_rope_active (2; getter already inline at rope_tick/rope_draw) |

Call-site replacements: `name()` -> `STATIC.with(Cell::get)`,
`set_name(v)` -> `STATIC.with(|c| c.set(v))`, byte setters ->
`STATIC.with(|t| t.borrow_mut()[i] = v)`, `toggle_video_flash(f)` ->
`VIDEO_FLASH.with(|c| c.set(c.get() ^ f))`. Statics that are
cross-module (AUDIO_PAN_X, GAME_INPUT, VIDEO_FLASH) become
`pub(crate)`; the rest stay module-private.

Considered and kept: dispatch.rs's `dispatch()`/`with_dispatch()`/
`set_screen()`/`queue_pending()` are the screen-state API, not bare
get/set pairs - they stay. RefCell compound statics (MINER_STORE,
MINER_TIMER, ROBOT_THIS, LOADER_TEXT, ...) already use `.with`
directly. loader.rs's LOADER_TICKS already uses `.with` directly.

Hard constraints: pure mechanical refactor - all 24 golden replays
byte-identical, never `UPDATE_GOLDEN=1`. Gates after every phase:
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` (56 unit + 24 replay), `cargo build`. Do not push; one
commit per phase, imperative mood, no type prefix, 72-col body,
trailer. Watch: title.rs:133 `colour_cycle() as usize` is a recorded
lint-invisible stay - the cast itself does not change, only the getter
call around it.

## Status: REVIEWED - accept-with-defects folded in (reviewer turn 32: D1 title.rs:103->:133 both spots, D2 rope_tick/rope_draw, D3 determinate phase 4)

## Phase 1 - title.rs, gameover.rs, die.rs (same-module pairs + byte setters)

## Phase 2 - game.rs, audio.rs, game_main.rs (incl. pub(crate) statics + toggle inline)

## Phase 3 - miner.rs, rope.rs

## Phase 4 - docs + plan COMPLETE: AGENTS.md:35 names `set_game_input`/
`game_input` and `video_flash`/`toggle_video_flash` in its dispatch
paragraph - update it to describe direct `.with` access on the
pub(crate) GAME_INPUT/VIDEO_FLASH statics; then mark this plan COMPLETE
with the commit list.

## Risks

- Call-site renames are mechanical but numerous (~40 sites); a missed
  site is a compile error (fn deleted), so nothing can silently
  survive - build green implies complete.
- title.rs:133's `as usize` stay must not be "fixed" while renaming
  the getter call.
- toggle_video_flash has one caller; inlining it is safe.

## Size

P1: 3 files, ~15 fns/call-site groups. P2: 3 files. P3: 2 files.
P4: docs. Each phase an independent commit with all gates green.
