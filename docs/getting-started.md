---
title: Getting started
nav_order: 2
---

# Getting started
{: .no_toc }

1. TOC
{:toc}

---

## Install

### Linux

```sh
curl -fsSL https://raw.githubusercontent.com/aronkv/rsampler/main/install.sh | sh
```

On x86_64 this downloads the latest prebuilt release (checksum-verified) into `~/.local/bin` and
adds rsampler with its icon to the app launcher. Set `RSAMPLER_INSTALL_DIR` to put it elsewhere.
On other systems it builds from source, which needs the [Rust toolchain](https://rustup.rs) and
the ALSA development files (the script tells you the package name if they are missing).

Then start it with `rsampler`.

### Windows

Download `rsampler-x86_64-pc-windows-msvc.zip` from the
[releases page](https://github.com/aronkv/rsampler/releases/latest), unzip it and run
`rsampler.exe`.

### From source

From a checkout of the [repository](https://github.com/aronkv/rsampler), `cargo install --path .`
installs it and `cargo run` runs it without installing.

## Finding your way around

rsampler works like the MPC2000XL: the LCD shows one screen at a time, the **CURSOR** keys move
between its fields, and the **DATA** wheel changes the highlighted field. The six keys under the
LCD (**F1–F6**) do what the labels above them say.

- **Cursor**: arrow keys or the **CURSOR** buttons. You can also click a field on the LCD.
- **DATA wheel**: drag or scroll over the wheel, scroll over the LCD, or press <kbd>+</kbd> /
  <kbd>-</kbd>. Hold <kbd>Shift</kbd> for fine steps (0.1 BPM tempo, 0.1 semitone tuning, single
  frames in a sound).
- **MAIN SCREEN** (<kbd>Esc</kbd>) always takes you back to the sequencer.
- **OPEN WINDOW** (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) opens the extra settings of the field under the
  cursor, where it has any.

The blue keys open the other modes, also reachable with <kbd>Ctrl</kbd>+number:

| Key | Mode |
| --- | --- |
| 1 | [SONG](sequencing#song) |
| 2 | [MISC](sequencing#misc-punch-transpose-and-a-second-sequence) |
| 3 | [LOAD](sounds#loading-samples) |
| 4 | [SAMPLE](sounds#sample-recording-from-the-input) |
| 5 | [TRIM](sounds#trim-loop-and-zone) |
| 6 | [PROGRAM](programs) |
| 7 | [SETUP](midi#setup-and-other) |
| 8 | [MIXER](programs#mixer-and-effects) |
| 9 | [OTHER](midi#setup-and-other) |

## Your first beat

1. Bank A starts with a kick, snare, hi-hat and clap on pads 1–4: hit <kbd>Z</kbd> <kbd>X</kbd>
   <kbd>C</kbd> <kbd>V</kbd> (<kbd>Y</kbd> instead of <kbd>Z</kbd> on a QWERTZ keyboard) to hear
   them.
2. Press **REC** (<kbd>Ctrl</kbd>+<kbd>R</kbd>), then **PLAY** (<kbd>Space</kbd>). After one bar of
   count-in, play the pads. The sequence loops; notes snap to the 1/16 grid.
3. Press **STOP** (<kbd>Space</kbd>). Press **PLAY** again to hear it.
4. Add more with **OVERDUB** (<kbd>Ctrl</kbd>+<kbd>O</kbd>), or take a pass back with **UNDO SEQ**
   (<kbd>Ctrl</kbd>+<kbd>Z</kbd>).
5. Load your own sounds onto other pads from [LOAD](sounds#loading-samples) and save the whole
   thing with <kbd>Ctrl</kbd>+<kbd>S</kbd> ([projects](files#saving-and-loading-projects)).
