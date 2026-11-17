# JetSetRusty

*Some rando on the internet is porting fawtytoo's C version of JetSetWilly into
Rust. This is that port.*

A Rust port of **Jet Set Willy**, the 1984 ZX Spectrum game by Matthew Smith
(Software Projects). It is ported from
[fawtytoo/JetSetWilly](https://github.com/fawtytoo/JetSetWilly), Steve Clark's
C/SDL2 reimplementation. The game is now 100% Rust, with no C compiled, and
gameplay is meant to be identical to the C version.

The reference implementation in C lives surprisingly in the
[reference/c-original/](reference/c-original/) — directory. More commits have
been made to the original since I started this and I haven't ported them over
quite yet.

**Note** the zlib [`LICENCE`](LICENCE) (clause 2) requires altered versions to
be *plainly marked as such*. So, this is a **modified port**, not the original
software (see *Licence* below), although the C original in included for
reference (in a directory by that very name).

You can see the original README.md [here](reference/c-original/README.md)

![JetSet Willy loading screen](images/JetSetWilly.gif)

## What carries over from the C version

These are all features of fawtytoo's version, preserved in the port:

- **Gameplay identical to the original**, minus a few original bugs that were
  fixed, such as landing at the end of a jump into a solid wall.
- **Per-pixel colouring**, so there is no colour clash, with a 16-colour palette.
- Two replacement character-set fonts, one small and one large.
- **Polyphonic** title and in-game music, plus approximate original sound
  effects with **stereo panning**. All sound comes from a square-wave synth
  for that retro "beepy" sound.

![JetSet Willy title screen](images/JetSetWilly-Title.gif)

## Controls

| Key | Action |
|-----|--------|
| ← / → | Walk |
| Space | Jump |
| Tab or Pause | Pause |
| Alt | Music on/off |
| Esc | Quit to title |

### Cheat mode

Type the cheat code just like in the original. Once it's active, hold a level
key and press Enter: `1`–`0` select levels 1–10, `A`–`T` select levels 11–30,
and holding Shift moves those to 31–60. See [`ROOMS.md`](ROOMS.md) for the
full room list.

### Copy protection

The original shipped with a "code card" you had to consult after loading. To
skip entering codes, press any key during the "JetSet Willy Loading" sequence.

![JetSet Willy codes](images/JetSetWilly-Codes.gif)

## Building

You need a Rust toolchain (edition 2024), a C compiler, and cmake. SDL3 is
built from source by the `sdl3` crate and linked statically, so no system SDL
package is required:

```sh
sudo apt install build-essential cmake   # Debian/Ubuntu
cargo run --release    # the first build compiles SDL3 itself; takes a few minutes
```

Only Linux is tested at the moment.

Fresh clones need `git config core.hooksPath .githooks` once to enable the
repo's hooks — `pre-commit` runs `cargo build` and blocks the commit if the
build fails.

![JetSet Willy level 1](images/JetSetWilly-Level1.png)

## The original C

The C implementation this was ported from is kept, buildable, in
[`reference/c-original/`](reference/c-original/). It includes upstream's
original [README](reference/c-original/README.md), and
[`PROVENANCE.md`](reference/c-original/PROVENANCE.md) explains where it came
from. Cargo does not compile it.

## Licence

zlib licence; see [`LICENCE`](LICENCE). Copyright © 2021–2026 Steve Clark and
© 1984 Matthew Smith (Software Projects). In keeping with the licence, this is
an **altered version**: a Rust port, not the original software.
