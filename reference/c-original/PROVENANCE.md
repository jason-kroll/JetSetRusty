# Provenance — original C source

This directory is a frozen, buildable snapshot of the **original C/SDL2
implementation of Jet Set Willy** that the Rust port in `../../src` was
transcribed from. It is **reference only** — Cargo does not compile it, and
nothing in the Rust build depends on it.

## Where it came from

- **Upstream ("master") repo:** <https://github.com/fawtytoo/JetSetWilly> — the
  C/SDL2 reimplementation by *fawtytoo* of Matthew Smith's 1984 ZX Spectrum
  game.
- **This snapshot:** extracted verbatim from *this* repo's own history at
  commit **`db7fc503e7d7bf2634e57cbb7a9fb38cf159c40f`** ("Initial commit",
  2024-01-17) — the pristine C as first imported here, before any Rust porting.
  The exact upstream commit it was imported from was not recorded; treat
  `db7fc50` as the pinned baseline (see "Diffing against upstream" below to
  locate the nearest upstream match).

## Caveat: pristine import vs. what was actually transcribed

The pristine import (2024-01-17) predates the Rust transcription by ~2 years —
the first module was ported out in **`cff8e93`** (2026-02-27, "Remove cheat.c
since it's been ported"). In between, the C evolved in-repo (bug fixes and the
per-pixel-colour / audio improvements described in the top-level README). So the
code here is the *canonical starting point*, not necessarily the exact bytes a
given module was ported from.

To see the exact C a specific module was transcribed from, use git history
directly, e.g.:

```bash
git show cff8e93^:src/miner.c     # miner.c as it stood right before it was ported
git log --oneline -- src/robots.c # every revision of a since-deleted file
```

## Building it

Needs `gcc` and `libSDL2-dev` only — **no SDL2_mixer** (the original used raw
SDL2 audio; the Rust port's build briefly linked the mixer but never called it,
and `3fe7f1a` dropped it).

```bash
cd reference/c-original
make            # -> ./jetsetwilly   (objects land in linux/)
./jetsetwilly
make clean      # remove linux/ and the binary
```

The Makefile defines `-DNOCODES` (skips the copy-protection code entry on
startup) and stamps `-DBUILD="v1.0.<year>"`. Build output is git-ignored.

## `Win32/` — upstream's Visual Studio project (stale, does not build)

`Win32/` is upstream's Windows build: a Visual Studio solution/project
(`JetSetWilly.sln`, `.vcxproj`, toolsets v142 / v140_xp), the icon resource
(`JetSetWilly.rc`, `resource.h`, `jetsetwilly.ico`), and `SDL/`, where you were
meant to drop the SDL2 headers and libs by hand. It sat at the repo root until
the Rust port no longer needed it, then moved here, where its `..\src\`
paths resolve to this snapshot's C sources again.

It was **already stale upstream**: the `.vcxproj` omits `system.c` (so it won't
link) and lists ten headers that don't exist (`cheat.h`, `die.h`, `robots.h`,
…). Keep it as a historical artifact, not as a working build. The `.ico` is the
one asset worth reusing (e.g. embedding it in a future Windows build of the
Rust port).

## Diffing against current upstream

The port is a point-in-time transcription; upstream has moved on since. To
review what changed upstream without disturbing this tree:

```bash
git remote add upstream https://github.com/fawtytoo/JetSetWilly.git
git fetch upstream
# What upstream changed to the C since our baseline (paths may differ upstream):
git log --oneline db7fc50..upstream/master -- '*.c' '*.h'
```

Jet Set Willy's gameplay is frozen (1984), so upstream changes to a faithful
reimplementation are almost always refactor / portability / build, not new game
logic. The Rust port's source of truth is this repo, not upstream — sync from
upstream only deliberately, by hand-carrying any real gameplay fix into the Rust.
