# Provenance — original C source

This directory is a frozen, buildable snapshot of the **original C/SDL2
implementation of Jet Set Willy** that the Rust port in `../../src` is
transcribed from. It is **reference only** — Cargo does not compile it, and
nothing in the Rust build depends on it.

## Where it came from

- **Upstream repo:** <https://github.com/fawtytoo/JetSetWilly> — the C/SDL2
  reimplementation by *fawtytoo* of Matthew Smith's 1984 ZX Spectrum game.
- **This snapshot:** upstream **HEAD `783d7c109c50623354bd5bc033f4ca8ad2570fed`**
  ("Use Miner_DrawSprite to draw Willy", 2026-11-14), synced verbatim with
  `git archive 783d7c1`. The previous snapshot (this repo's `db7fc50` import,
  2024-01-17) remains in this repo's git history.
- **Local changes on top of the verbatim sync** (keep these when re-syncing):
  the four screenshot links in `README.md` are rewritten from `images/…` to
  `../../images/…` so they resolve to the repo root's images; plus this
  `PROVENANCE.md` and the `.gitignore` for build output. Upstream's top-level
  `debian/` and `images/` are deliberately **not** mirrored here — the repo
  root has its own `images/` and `debian/` for the Rust port.

## What changed between the old db7fc50 snapshot and 783d7c1

Upstream's post-import history (~70 commits) is mostly refactors, build and
docs, plus gameplay fixes toward the 1984 original ("as in OG"): rope rework,
miner movement/sprite fixes, pause rework, Shift-based level cheat,
copy-protection on by default, joystick support, and a vsync-only pacing
change (`90b5421`) that runs the game unthrottled wherever vsync is ignored —
the Rust port deliberately keeps its wall-clock pacer instead. The Rust port
was audited against `db7fc50..783d7c1` and now tracks this snapshot; see the
"Upstream parity" section in `../../AGENTS.md` for the full ported/skipped
list. Notable file-level changes: `src/include.h`, `src/system.c` and
`src/system.h` were deleted upstream (system code merged into `main.c`);
`LICENCE` was added upstream; `Win32/` now exists upstream.

## Caveat: what was actually transcribed

The pristine import (2024-01-17) predates the Rust transcription by ~2 years —
the first module was ported out in **`cff8e93`** (2026-02-27, "Remove cheat.c
since it's been ported"). In between, the C evolved in-repo (bug fixes and the
per-pixel-colour / audio improvements described in the top-level README). So
the *as-transcribed* bytes of a given module are in this repo's git history,
not necessarily in this snapshot:

```bash
git show cff8e93^:src/miner.c     # miner.c as it stood right before it was ported
git log --oneline -- src/robots.c # every revision of a since-deleted file
```

## Building it

Needs `gcc` and `libSDL2-dev` only (`sdl2-config` must be on PATH) — **no
SDL2_mixer** (the original used raw SDL2 audio; the Rust port's build briefly
linked the mixer but never called it, and `3fe7f1a` dropped it).

```bash
cd reference/c-original
make            # -> ./jetsetwilly   (objects land in linux/)
./jetsetwilly
make clean      # remove linux/ and the binary
```

The Makefile compiles with `` `sdl2-config --cflags` `` / ``--libs``. At this
snapshot upstream no longer defines `-DNOCODES`: the copy-protection code
entry is on by default (press any key during the "JetSet Willy Loading"
sequence to skip it), and there is no `-DBUILD` version stamp. Build output
is git-ignored.

## `README.md` — upstream's README

`README.md` is upstream's README as of `e4d4ecd` ("Add joystick support",
2026-05-17). The **only change** from upstream is the four screenshot links,
rewritten from `images/…` to `../../images/…` so they resolve from this
directory. Its build instructions (`make install`, Visual Studio) describe
upstream's C build, not the Rust port.

## `Win32/` — upstream's Visual Studio project (stale, does not build)

`Win32/` is upstream's Windows build: a Visual Studio solution/project
(`JetSetWilly.sln`, `.vcxproj`), the icon resource (`JetSetWilly.rc`,
`resource.h`, `jetsetwilly.ico`), and `SDL/`, where you were meant to drop
the SDL2 headers and libs by hand.

It was **already stale upstream**: the `.vcxproj` omits `system.c` (so it
won't link) and lists headers that no longer exist. Keep it as a historical
artifact, not as a working build. The `.ico` is the one asset worth reusing
(e.g. embedding it in a future Windows build of the Rust port).

## Diffing against current upstream

This snapshot *is* upstream HEAD (`783d7c1`), so to see what upstream changes
next, without disturbing this tree:

```bash
git remote add upstream https://github.com/fawtytoo/JetSetWilly.git
git fetch upstream
git log --oneline 783d7c1..upstream/master -- '*.c' '*.h'
```

Jet Set Willy's gameplay is frozen (1984), so upstream changes to a faithful
reimplementation are almost always refactor / portability / build, not new
game logic. The Rust port's source of truth is this repo, not upstream — sync
from upstream only deliberately: re-run `git archive` here, re-apply the
local changes listed above, and hand-carry any real gameplay fix into the
Rust (see "Upstream parity" in `../../AGENTS.md`).
