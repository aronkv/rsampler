# Programs and mixer

## Programs and DRUM buses

As on the MPC, there can be up to 24 programs, each a full set of 64 pads. Every track has a `Bus`
(on the [MAIN screen](sequencing.md#the-main-screen)): DRUM1–4, or MIDI for a track that only sends
its notes out over MIDI. Each DRUM bus plays one program, chosen with `Pgm` on the PROGRAM screen
for the selected track's bus.

The pads, and the PROGRAM, MIXER and TRIM screens, always work on the program of the selected
track, so a drum kit and a set of chopped samples can play side by side from different tracks.

**OPEN WINDOW** (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) on `Pgm` opens the PROGRAM window:

- **NEW** (F2) makes an empty program and **COPY** (F3) a copy of this one, both put on the
  track's bus.
- **DELETE** (F4) removes this one, **NAME** (F5) names it, **CLOSE** (F6) goes back.

## The PROGRAM screen

Open it with the blue **6 / PROGRAM** key (<kbd>Ctrl</kbd>+<kbd>6</kbd>). Hit a pad (or change
`Pad`) to choose it.

| Field     | What it does |
| --------- | ------------ |
| `Tune`    | ±12 semitones, in 0.1 steps |
| `Attack`  | Fade-in time, 0 = instant |
| `Decay`   | Fade time, up to two seconds (default 5, a short fade) |
| `Dcy md`  | END fades out over the decay time before the sound ends; START dies away from the end of the attack, slower for larger values |
| `Filter`  | Low-pass cutoff, 100 = open |
| `Reso`    | Filter resonance |
| `Overlap` | POLY layers repeated hits, MONO cuts the previous hit, NOTE OFF stops when the pad is released |
| `Mutes`   | Up to two pads this pad cuts off, e.g. a closed hi-hat stopping an open one |
| `Mode`    | NORMAL; SIMULT also plays the two `Also` pads, layered; VEL SW plays another pad when hit harder than its threshold |
| `Level`, `Pan` | Same as in the mixer |

!!! tip

    With VEL SW, `Over 64:A02 100:A03` means: above velocity 64 play A02, above 100 play A03.

**CLEAR** (F6) removes the pad's sound.

## VELO/ENV

**VELO** (F4) opens the VELO/ENV window. Its first part is velocity modulation: how much the
strength of a hit shapes the sound, each 0–100, for expressive playing from MIDI pads:

| Field         | What it does |
| ------------- | ------------ |
| `Velo>Level`  | Softer hits are quieter; 100 (the default) follows velocity fully, 0 plays every hit at full level |
| `Velo>Attack` | Softer hits fade in more slowly |
| `Velo>Start`  | Softer hits start later into the sound, skipping the sharp beginning |
| `Velo>Filter` | Harder hits open the filter further (set `Filter` below 100 first) |

Below it is the **FILTER ENVELOPE**: on each hit the filter opens by `Amount` over `Attack`, then
closes back to `Filter` over `Decay` (each 0–100, up to five seconds), the classic sweep of a
filtered bass or a "wah" on a drum. It needs `Filter` below 100. **PROGRAM** (F1) goes back.

## Mixer and effects

The blue **8 / MIXER** key (<kbd>Ctrl</kbd>+<kbd>8</kbd>) shows the 16 pads of the current bank as
level faders with pan markers. **←** / **→** choose the pad, **↑** selects `Level` and **↓**
selects `Pan`, and the DATA wheel sets the value. **RESET** (F6) returns the pad to full level,
centre.

**FX SEND** (F2) shows how much of each pad goes to the effects instead (0–100; **STEREO**, F1,
goes back), and **FX** (F5) opens the effects themselves:

| Field | What it does |
| ----- | ------------ |
| Reverb `Level`, `Size`, `Damp` | How loud the reverb is, how long it rings, how dark it gets |
| Delay `Level` | How loud the echoes are |
| `Time` | Echo spacing in note lengths at the sequence tempo: 1/4, 1/8D, 1/4T, 1/8, 1/16D, 1/8T, 1/16 |
| `Feedback` | How many echoes follow, up to 95 |

Both effects work on everything sent to them, so a snare with FX SEND 40 gets room while the kick
stays dry. The settings are saved with the project, the WAV export includes the effects and their
tails, and the FX send of an MPC `.PGM` carries over.

!!! quote "On the MPC"

    The effects stand in for the MPC2000XL's optional effects board.

