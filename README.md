<!--
TODO(Jason): This README is a placeholder that was LLM-generated (Claude Code,
Claude Opus 5.5). It will be rewritten by hand. Don't polish it; replace it.
-->

> **⚠️ LLM-generated placeholder.** An AI model (Claude) wrote this README as a
> stand-in. **TODO:** Jason is going to rewrite it. Until then, trust the code
> over this file.

# JetSetRusty

A Rust port of **Jet Set Willy**, the 1984 ZX Spectrum game by Matthew Smith
(Software Projects). It is ported from
[fawtytoo/JetSetWilly](https://github.com/fawtytoo/JetSetWilly), Steve Clark's
C/SDL2 reimplementation. The game is now 100% Rust, with no C compiled, and
gameplay is meant to be identical to the C version.

It's a for-fun project: some rando on the internet porting it to Rust.

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

You need a Rust toolchain (edition 2024) and the SDL2 development library:

```sh
sudo apt install libsdl2-dev build-essential   # Debian/Ubuntu
cargo run --release
```

Only Linux is tested at the moment.

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
