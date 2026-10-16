# rsampler

A desktop sampler inspired by the Akai MPC2000XL, written in pure Rust.

> **Status:** early development. Pads, sample playback and WAV loading work; the sequencer does not exist yet.

## Goals

- MPC2000XL-style screens in a native, lightweight GUI
- 16 velocity pads, four banks (A–D)
- Programs: sample assignment per pad, tuning, envelopes, mixer
- Sequencer: tracks, tempo, quantize, loop, step editing
- Loading and saving the original file formats (`.SND`, `.PGM`, `.APS`, `.SEQ`/`.MID`, `.ALL`)

## Platforms

Linux first, Windows later.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/aronkv/rsampler/main/install.sh | sh
```

On Linux x86_64 this downloads the latest prebuilt release (checksum-verified) into `~/.local/bin`;
set `RSAMPLER_INSTALL_DIR` to put it elsewhere. On other systems it builds from source, which needs the
[Rust toolchain](https://rustup.rs) and, on Linux, the ALSA development files (the script tells you
the package name if they are missing). Then start it with `rsampler`.

Prebuilt binaries are also on the [releases page](https://github.com/aronkv/rsampler/releases).

From a local checkout, `cargo install --path .` does the same, and `cargo run` runs it without
installing.

## Usage

### Pads

Click a pad or hit its key. On the mouse, hitting higher on a pad plays louder; keys always play at
velocity 101, and **FULL LEVEL** makes everything 127.

The keys follow their physical position, so the layout works the same on QWERTY, QWERTZ (Hungarian,
German) and AZERTY keyboards:

| Row    | Pads  | QWERTY        | Hungarian QWERTZ |
| ------ | ----- | ------------- | ---------------- |
| top    | 13–16 | `1` `2` `3` `4` | `1` `2` `3` `4`    |
|        | 9–12  | `Q` `W` `E` `R` | `Q` `W` `E` `R`    |
|        | 5–8   | `A` `S` `D` `F` | `A` `S` `D` `F`    |
| bottom | 1–4   | `Z` `X` `C` `V` | `Y` `X` `C` `V`    |

**PAD BANK A–D** switches between four banks of 16 pads. Bank A starts with a built-in kick, snare,
hi-hat and clap on pads 1–4.

### Loading samples

1. Hit the pad you want to load into.
2. Press **LOAD** (the blue `3` key). The browser opens in `~/Music`.
3. Move with the arrow keys, the **DATA** wheel (scroll or drag) or the **CURSOR** buttons.
4. **Enter**, **→**, **ENTER** or **F6** opens a folder, or loads the WAV file into the pad and plays it.
5. **Backspace**, **←** or **F1** goes up a folder.
6. **Esc** or **MAIN SCREEN** goes back to the main screen.

WAV files of any bit depth and sample rate work; stereo files are mixed down to mono.

### Other controls

- **MAIN VOLUME**: drag up/down or scroll over the knob.
- **REC**, **OVERDUB**, **PLAY**, **STOP**: only toggle their lights for now, the sequencer is not
  implemented yet.

Loaded samples are not saved yet; they are gone when the app closes.

## Reference

[izzyreal/mpc](https://github.com/izzyreal/mpc) (the engine behind VMPC2000XL) is used as a behavioural and
file-format reference only. rsampler contains no code from it.

## Disclaimer

MPC® and Akai Professional® are registered trademarks of inMusic Brands, Inc. This project is not affiliated with,
endorsed by or sponsored by inMusic.

## License

Apache License 2.0, see [LICENSE](LICENSE).
