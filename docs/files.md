# Files

## Saving and loading projects

A project is a folder holding `rsampler.json` (sequences, programs, mixer, effects) and the pad
sounds as WAV files in `sounds/`, so the sounds can be used anywhere else too.

To save, press <kbd>Ctrl</kbd>+<kbd>S</kbd>, **SHIFT + ENTER** or **SAVE** (F2) on the LOAD screen:

1. Pick the folder to save in, like on the LOAD screen.
2. Press **NAME** (F5), type the name and press <kbd>Enter</kbd>. Choosing an existing project in
   the list takes its name.
3. Press **DO IT** (F6). Replacing an existing project asks for a second **DO IT**.

Once a project has been saved or loaded, <kbd>Ctrl</kbd>+<kbd>S</kbd> saves over it straight away.

To load, choose the project (marked `PROJECT`) on the LOAD screen and press <kbd>Enter</kbd> twice;
loading replaces everything that is open.

!!! note

    Closing the window with unsaved changes shows a warning on the LCD instead; close it again to quit
    without saving.

## Exporting audio

On the SAVE screen, **WAV** (F3) writes the current sequence, played once from the start, as
`<name>.wav`, and **SONG** (F2) the whole song as `<name>-song.wav`: 24-bit stereo at the output
sample rate, with muted tracks left out, SOLO respected, and the last sounds left to ring out. It
renders faster than real time, so nothing has to play.

## MPC2000XL files

The MPC's own files load from the LOAD screen, so an MPC disk can be opened as it was saved.

| File | What loads |
| ---- | ---------- |
| `.SND` | A sound, with its trim points, loop and tuning, onto the selected pad |
| `.PGM` | A program into the current program, all 64 pads at once (press <kbd>Enter</kbd> twice) |
| `.APS` | All programs and their DRUM buses, replacing the ones open |
| `.ALL` | Every sequence and the first song, replacing the ones open |

**Programs**: each pad gets its sound from the `.SND` files next to the program, with the
program's tuning, envelope, filter, overlap, mute assign, SIMULT / VEL SW mode, velocity
modulation, filter envelope, level, pan and FX send. Sounds that are not there are listed on the
LCD.

**Sequences**: an `.ALL` file brings back the tempo, length, time signature, loop, names, muted
tracks, track buses, notes and NOTE VARIATION of every sequence, and its first song. Its notes are
played by the program they were made with: if no MPC program is loaded yet, the `.APS` file of the
same name next to it is loaded first.

!!! tip

    Copy the whole MPC disk folder (`NAME.ALL`, `NAME.APS` and the `.SND` files) and load the `.ALL`:
    the sequences, programs and sounds come back together.

## MIDI files

A MIDI file (`.mid`) chosen on the LOAD screen replaces the current sequence (press
<kbd>Enter</kbd> twice; **UNDO SEQ** brings the old one back), with its tempo, time signature and
length, one track per MIDI track. Notes 36–99 play pads A01–D16.

Sequences saved by an MPC2000XL use its own drum note numbers, so load the matching `.PGM` first:
its pad assignments are then used for the notes.

**MIDI** (F4) on the SAVE screen writes the current sequence as `<name>.mid`, to open in a DAW.
