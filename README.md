# rsampler

A desktop sampler inspired by the Akai MPC2000XL, written in pure Rust.

> **Status:** early development, but usable: pads, programs, mixer, trimming, a 99-sequence,
> 64-track sequencer, song mode, step editing, sampling from the input, MIDI in, out and clock,
> and project saving all work. See [Not there yet](#not-there-yet) for what is missing.

## Goals

- MPC2000XL-style screens in a native, lightweight GUI
- 16 velocity pads, four banks (A–D)
- Programs: sample assignment per pad, tuning, envelopes, mixer
- Sequencer: tracks, tempo, quantize, loop, step editing
- Sampling, song mode, MIDI in and out
- Loading and saving the original file formats (`.SND`, `.PGM`, `.APS`, `.SEQ`/`.MID`, `.ALL`)

## Platforms

Linux first. Windows builds are published too but get less testing; MIDI output is Linux-only
for now.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/aronkv/rsampler/main/install.sh | sh
```

On Linux x86_64 this downloads the latest prebuilt release (checksum-verified) into `~/.local/bin`;
set `RSAMPLER_INSTALL_DIR` to put it elsewhere. On other systems it builds from source, which needs the
[Rust toolchain](https://rustup.rs) and, on Linux, the ALSA development files (the script tells you
the package name if they are missing). Then start it with `rsampler`.

On Windows, download `rsampler-x86_64-pc-windows-msvc.zip` from the
[releases page](https://github.com/aronkv/rsampler/releases), unzip it and run `rsampler.exe`.

From a local checkout, `cargo install --path .` does the same, and `cargo run` runs it without
installing.

## Usage

[Pads](#pads) · [NOTE VARIATION](#note-variation) · [Recording](#recording-a-sequence) ·
[SONG](#song) · [STEP EDIT](#step-edit) · [PROGRAM](#program-how-each-pad-plays) · [MIXER](#mixer) ·
[TRIM](#trim) · [Loading samples](#loading-samples) · [SAMPLE](#sample-recording-from-the-input) ·
[Projects](#saving-and-loading-projects) · [MIDI](#midi) · [Shortcuts](#keyboard-shortcuts)

rsampler works like the MPC2000XL: the LCD shows one screen at a time, the **CURSOR** keys move
between its fields, and the **DATA** wheel changes the highlighted field. The six keys under the LCD
(**F1–F6**) do what the labels above them say.

- **Cursor**: arrow keys or the **CURSOR** buttons. You can also click a field on the LCD.
- **DATA wheel**: drag or scroll over the wheel, scroll over the LCD, or press `+` / `-`. Hold
  **Shift** for fine steps (0.1 BPM tempo, 0.1 semitone tuning, single-frame trimming).

### Pads

Click a pad or hit its key. On the mouse, hitting higher on a pad plays louder; keys always play at
velocity 101 (changeable in SETUP), and **FULL LEVEL** makes everything 127.

The keys follow their physical position, so the layout works the same on QWERTY, QWERTZ (Hungarian,
German) and AZERTY keyboards:

| Row    | Pads  | QWERTY        | Hungarian QWERTZ |
| ------ | ----- | ------------- | ---------------- |
| top    | 13–16 | `1` `2` `3` `4` | `1` `2` `3` `4`    |
|        | 9–12  | `Q` `W` `E` `R` | `Q` `W` `E` `R`    |
|        | 5–8   | `A` `S` `D` `F` | `A` `S` `D` `F`    |
| bottom | 1–4   | `Z` `X` `C` `V` | `Y` `X` `C` `V`    |

Keys pressed together with Shift, Ctrl or Alt never play pads, so shortcuts and symbols such as
`+` (Shift+3 on a Hungarian keyboard) work.

**PAD BANK A–D** switches between four banks of 16 pads (A01–D16). Bank A starts with a built-in
kick, snare, hi-hat and clap on pads 1–4. The last pad you hit is the selected pad that the
PROGRAM, MIXER, TRIM and LOAD screens work on.

**16 LEVELS** spreads the selected pad's sound over all 16 pads, from velocity 7 on pad 1 to 127 on
pad 16. Press it again to go back.

### NOTE VARIATION

The slider changes one pad's sound each time it is hit: drag it, scroll over it, or double-click
to centre it. **AFTER** opens its ASSIGN screen, where `Pad` chooses the pad and `Param` what the
slider changes:

| Param  | Slider bottom … top                                  |
| ------ | ---------------------------------------------------- |
| TUNE   | one octave down … one octave up (centre = no change) |
| DECAY  | 0 … 100                                              |
| ATTACK | 0 … 100                                              |
| FILTER | closed … open                                        |

The value is recorded with each note and played back with it. On the MPC, AFTER makes pad
pressure drive the variation; keyboards and mice have no pressure, so here it only opens ASSIGN.

### Recording a sequence

The MAIN screen (**MAIN SCREEN** or **Esc**) shows the sequencer:

| Field         | What it does                                                         |
| ------------- | -------------------------------------------------------------------- |
| `Sq`          | Sequence 1–99. Chosen while playing, it starts when the current one ends |
| `Now`         | Position as bar.beat.tick (96 ticks per quarter note)                |
| `Tempo`       | 30–300 BPM. **TAP TEMPO** (or `T`) sets it from your taps            |
| `Timing`      | Quantize grid for recording: OFF, 1/8, 1/16, 1/32 and their triplets |
| `Swing`       | 50–75 %, for 1/8 and 1/16                                            |
| `Tsig`        | Time signature                                                       |
| `Count`       | One bar of count-in before recording                                 |
| `Click`       | Metronome while playing and recording                                |
| `Loop`        | Loop the sequence                                                    |
| `Bars`        | Sequence length                                                      |
| `Tr`, `On`    | Track 1–64 and whether it plays                                      |
| `Out`         | MIDI channel the track's notes are sent on (see [MIDI](#midi)), OFF by default |

To record:

1. Press **REC** (replace) or **OVERDUB** (add). Its light shows it is armed.
2. Press **PLAY** or **PLAY START**. After the count-in, play the pads.
3. Press **STOP**.

**REC** erases what is on the track as the playhead passes, and turns into **OVERDUB** after the
first loop so the new take is kept. Pressing **REC** or **OVERDUB** while playing punches in or out.
Notes are quantized to `Timing` as they are recorded; **OPEN WINDOW** on the `Timing` field (timing
correct) quantizes a whole track afterwards.

**UNDO SEQ** (F2, `Ctrl+Z`) takes back the last recording or edit; its light shows that it can.
Hold **ERASE** (or `Delete`) while playing and hold pads to delete their notes as the playhead
passes. **TR.ON** (F3) and **TRACK MUTE** mute the track, **SOLO** (F4) plays only this track,
**TR -** / **TR +** (F5/F6) change track and **NEXT SEQ** queues the next sequence.

**NOTE REPEAT**: while playing, hold **TAP TEMPO / NOTE REPEAT** (or `T`) and hold pads; they play
again on every `Timing` grid line (1/16 when `Timing` is OFF) and are recorded when recording.
While stopped the same key taps the tempo.

**LOCATE**: **STEP <** / **>** move by the `Timing` grid, **BAR <<** / **>>** by a bar, and
**GO TO** returns to the start. **STOP** pressed while stopped also returns to the start.

### SONG

The blue **1 / SONG** key (`Ctrl+1`) chains sequences into a song. **INSERT** (F1) adds a step after
the selected one, playing the sequence chosen on the MAIN screen; **DELETE** (F2) removes it.
**↑** / **↓** choose a step, and its `Sq` and `Reps` (how many times it plays) are set with the
DATA wheel. `Loop` starts the song over at the end.

On this screen **PLAY START** plays the song from the first step and **PLAY** from the selected
one; `>` marks the step that is playing. Song mode does not record.

### STEP EDIT

**STEP** (F1 on the MAIN screen) lists the notes of the current track. **↑** / **↓** choose a note
and move `Now` to it; **←** / **→** move between its position, pad, velocity and duration, and the
DATA wheel changes them (position and duration move by the `Timing` grid, or by single ticks with
Shift). Hitting a pad while stopped adds a note at `Now`, so you can step-record with **STEP <** /
**>**. **DELETE** (F1) removes the selected note, **TR -** / **TR +** change track, and **UNDO SEQ**
takes back everything done since opening the screen.

### PROGRAM: how each pad plays

Open it with the blue **6 / PROGRAM** key. Hit a pad (or change `Pad`) to choose it.

| Field     | What it does                                                               |
| --------- | -------------------------------------------------------------------------- |
| `Tune`    | ±12 semitones, in 0.1 steps                                                 |
| `Attack`  | Fade-in time, 0 = instant                                                  |
| `Decay`   | 100 lets the sound play out; lower values shorten it                        |
| `Dcy md`  | END fades out towards the end of the sound, START decays right after the attack |
| `Filter`  | Low-pass cutoff, 100 = open                                                |
| `Reso`    | Filter resonance                                                           |
| `Overlap` | POLY layers repeated hits, MONO cuts the previous hit, NOTE OFF stops when the pad is released |
| `Mutes`   | Up to two pads this pad cuts off, e.g. a closed hi-hat stopping an open one |
| `Level`, `Pan` | Same as in the mixer                                                  |

**CLEAR** (F6) removes the pad's sound.

### MIXER

The blue **8 / MIXER** key shows the 16 pads of the current bank as level faders with pan markers.
**←** / **→** choose the pad, **↑** selects `Level` and **↓** selects `Pan`, and the DATA wheel sets
the value. **RESET** (F6) returns the pad to full level, centre.

### TRIM

The blue **5 / TRIM** key shows the selected pad's waveform. `St` and `End` set the part that plays.
**PLAY** (F1) auditions it, **RESET** (F6) restores the whole sound.

### Loading samples

1. Hit the pad you want to load into.
2. Press **LOAD** (the blue `3` key). The browser opens in `~/Music`.
3. Move with the arrow keys or the **DATA** wheel.
4. **Enter**, **→**, **ENTER** or **F6** opens a folder, or loads the WAV file into the pad and plays it.
5. **Backspace**, **←** or **F1** goes up a folder.

WAV files of any bit depth and sample rate work, mono or stereo.

### SAMPLE: recording from the input

The blue **4 / SAMPLE** key (`Ctrl+4`) opens the default audio input; it is closed again when you
leave the screen. Hit the pad the new sound should go to, then:

| Field       | What it does                                                            |
| ----------- | ----------------------------------------------------------------------- |
| `Mode`      | STEREO, or MONO (both channels mixed)                                   |
| `Threshold` | The recording starts when the input gets this loud; OFF starts at once   |
| `Time`      | Longest take, 1–60 seconds                                              |

**REC GAIN** (drag or scroll) sets the input level from −12 to +12 dB; the meters show the input
from −60 to 0 dBFS with the threshold marked. **REC** (F6) waits for the threshold and records,
**STOP** ends the take early, then **KEEP** (F6) puts it on the pad as `SAMPLExx` and plays it, or
**RETRY** (F5) records again. Use TRIM afterwards to cut off silence.

### Saving and loading projects

A project is a folder holding `rsampler.json` (sequences, pad settings, mixer) and the pad sounds as
WAV files in `sounds/`, so the sounds can be used anywhere else too.

To save, press `Ctrl+S`, **SHIFT + ENTER** or **SAVE** (F2) on the LOAD screen:

1. Pick the folder to save in, like on the LOAD screen.
2. Press **NAME** (F5), type the name and press `Enter`. Choosing an existing project in the list
   takes its name.
3. Press **DO IT** (F6). Replacing an existing project asks for a second **DO IT**.

Once a project has been saved or loaded, `Ctrl+S` saves over it straight away.

To load, choose the project (marked `PROJECT`) on the LOAD screen and press `Enter` twice; loading
replaces everything that is open.

Closing the window with unsaved changes shows a warning on the LCD instead; close it again to quit
without saving.

### MIDI

**In:** rsampler listens to every MIDI input it finds, including controllers plugged in while it
runs, and on Linux it also offers its own `rsampler` input port for other programs. Notes 36–99
(C1–D#6) play pads A01–D16 with their velocity, whatever bank is selected, and are recorded like
pad hits. Most 16-pad controllers send 36–51 by default, which lands on bank A.

**Out (Linux):** connect a synth or DAW to the `rsampler` output port. A track whose `Out` field is
set to a channel sends its notes there as it plays (pad A01 = note 36 again), and pads you hit pass
through on the selected track's channel, so the pads can play an external synth.

**Clock:** with `MIDI clock out` on in SETUP, rsampler sends MIDI clock while playing, and start,
stop, or song position and continue, so a DAW or drum machine can follow its tempo.

### SETUP and OTHER

The blue **7 / SETUP** key (`Ctrl+7`) holds `MIDI clock out` and the velocity of pads hit from the
computer keyboard (101 by default); both are saved with the project. **9 / OTHER** (`Ctrl+9`)
shows the version and the output sample rate.

### Keyboard shortcuts

| Key                         | Does                         |
| --------------------------- | ---------------------------- |
| `Space`                     | PLAY / STOP                  |
| `Shift+Space`               | PLAY START                   |
| `Ctrl+R` / `Ctrl+O`         | REC / OVERDUB                |
| `Ctrl+Z`                    | UNDO SEQ                     |
| `Ctrl+S`                    | Save                         |
| `+` / `-`                   | DATA wheel                   |
| `Delete` (hold)             | ERASE                        |
| `T`                         | TAP TEMPO (stopped), NOTE REPEAT (held while playing) |
| `,` / `.`                   | STEP < / >                   |
| `Page Up` / `Page Down`     | BAR << / >>                  |
| `Home`                      | GO TO start                  |
| `F1`–`F6`                   | Soft keys                    |
| `Ctrl+1` / `3`–`9`          | SONG / LOAD, SAMPLE, TRIM, PROGRAM, SETUP, MIXER, OTHER |
| `Esc`                       | MAIN SCREEN                  |

### Not there yet

- Following an external MIDI clock, and MIDI output on Windows
- Windows-specific testing
- The MISC mode
- Loading the MPC's own file formats
- Windows builds

## Reference

[izzyreal/mpc](https://github.com/izzyreal/mpc) (the engine behind VMPC2000XL) is used as a behavioural and
file-format reference only. rsampler contains no code from it.

## Disclaimer

MPC® and Akai Professional® are registered trademarks of inMusic Brands, Inc. This project is not affiliated with,
endorsed by or sponsored by inMusic.

## License

Apache License 2.0, see [LICENSE](LICENSE).
