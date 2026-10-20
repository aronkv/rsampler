# Sounds

## Loading samples

1. Hit the pad you want to load into.
2. Press **LOAD** (the blue **3** key, <kbd>Ctrl</kbd>+<kbd>3</kbd>). The browser opens in
   `~/Music`.
3. Move with the arrow keys, the **DATA** wheel or by scrolling over the LCD; <kbd>Page Up</kbd> /
   <kbd>Page Down</kbd> move ten files, <kbd>Home</kbd> / <kbd>End</kbd> to the first and last.
4. <kbd>Enter</kbd>, **→**, **ENTER** or **F6** opens a folder, or loads the sound into the pad and
   plays it. The pad keeps its settings (tuning, envelope, mixer); only the sound changes.
5. <kbd>Backspace</kbd>, **←** or **F1** goes up a folder.

**PLAY** (F5) listens to the selected sound file without loading it; **STOP** or leaving the screen
stops it.

The browser lists folders and the files rsampler can load (`.wav`, `.mp3`, `.flac`, `.m4a`,
`.mp4`, `.aac`, `.ogg`, `.oga`, `.opus`, `.aif`, `.aiff`, `.aifc`, and the MPC's `.snd`, `.pgm`, `.aps`,
`.all`, plus `.mid`); hidden files and folders (names starting with a dot, or marked hidden on
Windows) are left out. Names are sorted ignoring accents, with numbers in order (`KICK2` before
`KICK10`). These formats load, up to 20 minutes long:

- WAV and RF64: 8, 16, 24 or 32-bit, 32 or 64-bit float, A-law, µ-law and ADPCM, any sample rate
- MP3, FLAC, M4A or MP4 (AAC or Apple Lossless), AAC, Ogg Vorbis, Opus, AIFF and AIFC
- The MPC2000XL's own `.SND` sounds, with their trim points, loop and tuning, so old MPC sound
  libraries can be used directly

Mono files play on both channels; files with more than two channels keep the first two. A file
that is cut off part-way loads up to where it ends. A loop saved in a WAV file (its `smpl` chunk,
written by other samplers and sound editors) carries over to the [LOOP](#trim-loop-and-zone)
screen. Emoji in names show as `?` on the LCD.

MPC programs and sequences load from the same screen; see [Files](files.md).

## SAMPLE: recording from the input

The blue **4 / SAMPLE** key (<kbd>Ctrl</kbd>+<kbd>4</kbd>) opens the default audio input; it is
closed again when you leave the screen. Hit the pad the new sound should go to, then:

| Field       | What it does |
| ----------- | ------------ |
| `Mode`      | STEREO, or MONO (both channels mixed) |
| `Threshold` | The recording starts when the input gets this loud; OFF starts at once |
| `Time`      | Longest take, 1–60 seconds |

**REC GAIN** (drag or scroll) sets the input level from −12 to +12 dB; the meters show the input
from −60 to 0 dBFS with the threshold marked.

**REC** (F6) waits for the threshold and records, **STOP** ends the take early, then **KEEP** (F6)
puts it on the pad as `SAMPLExx` and plays it, or **RETRY** (F5) records again. Use TRIM afterwards
to cut off silence.

## TRIM, LOOP and ZONE

The blue **5 / TRIM** key (<kbd>Ctrl</kbd>+<kbd>5</kbd>) shows the selected pad's waveform.
**TRIM** (F1), **LOOP** (F2) and **ZONE** (F3) switch between the three views, as on the MPC. On
all three, **PLAY X** (F6) plays the highlighted part once, as it is, and **EDIT** (F5) opens the
[EDIT window](#edit) on it.

- **TRIM**: `St` and `End` set the part of the sound that plays. **RESET** (F4) restores the whole
  sound.
- **LOOP**: with `Loop` ON the sound jumps back from `End` to `To` for as long as the pad is held
  (or, in a sequence, for the length of the note), then fades out over its decay. `Lngth` moves
  `To` so the loop gets longer or shorter. A looping sound always plays as NOTE OFF, as on the
  MPC. **RESET** (F4) turns the loop off.
- **ZONE** chops the sound: `Zones` splits the whole sound into 1–16 equal zones, back to back.
  `Zone` picks one, and its `St` and `End` move the boundaries it shares with its neighbours, so no
  part is lost or played twice. A zone whose `St` and `End` meet is empty, and EDIT refuses to work
  on it. **RESET** (F4) splits it evenly again.

Points are shown in frames (samples at the output rate), as on the MPC; `Len` and `Play` are in
seconds. Every point moves by 1/500 of the sound per DATA wheel step, or by single frames with
<kbd>Shift</kbd>. `St` always stays at least one frame before `End`.

## EDIT

EDIT works on St–End from TRIM, on the loop from LOOP, and on the selected zone from ZONE:

| Edit | What it does |
| ---- | ------------ |
| DISCARD | Keeps only the section and throws away the rest |
| LOOP FROM ST TO END | Sets `To` to `St` and turns `Loop` on |
| SECTION -> NEW SOUND | Copies the section to `To pad` |
| INSERT SOUND -> SECTION START | Puts the whole sound of another pad (`Sound of pad`) in at the start of the section, e.g. to join two chops |
| DELETE SECTION | Cuts the section out. Deleting St–End itself leaves St–End on the rest of the sound |
| SILENCE SECTION | Makes the section silent |
| REVERSE SECTION | Plays the section backwards |
| TIME STRETCH | Puts the section on `To pad` made longer or shorter at the same pitch: `Ratio` is the new length, 50–200 % (1 % a step, 0.1 % with Shift). Sections shorter than 80 ms are copied unchanged |
| RESAMPLE | Puts the section on `To pad` as if sampled again at `New fs` (4000–48000 Hz, 1000 Hz a step, 50 Hz with Shift) and 8, 12 or 16 `Bits`, without smoothing: the gritty sound of old samplers |
| NORMALIZE SECTION | Raises the section until its loudest point is at full level |
| SLICE SOUND (ZONE) | Puts every zone on its own pad, from `To pad` on |

**DO IT** (F5) applies it, **CANCEL** (F4) goes back. New sounds are named after the original
(`KICK-1`, `KICK-2`, … for sections and slices, `KICK-T` stretched, `KICK-R` resampled, with a
number added when the name is taken: `KICK-T2`) and their
pads start from the default settings, replacing what was there. **UNDO SEQ**
(<kbd>Ctrl</kbd>+<kbd>Z</kbd>) on these screens takes back the last edit, and pressed again redoes
it; there is one step of undo. Edited sounds are saved with the [project](files.md#saving-and-loading-projects).

!!! quote "On the MPC"

    The MPC itself has no undo for sounds.

### Chopping a break onto 16 pads

1. Load the break on a pad and open TRIM.
2. Go to **ZONE** (F3) and set `Zones` to 16.
3. Fix the boundaries by ear with **PLAY X**.
4. **EDIT**, choose SLICE SOUND, set `To pad` to B01, **DO IT**.

### Fitting a loop to the tempo

Stretch it by the old tempo over the new with TIME STRETCH: a 90 BPM break in a 95 BPM sequence
needs 90 / 95 = 94.7 %.
