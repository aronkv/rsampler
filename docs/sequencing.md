# Sequencer

## The MAIN screen

The MAIN screen (**MAIN SCREEN** or <kbd>Esc</kbd>) shows the sequencer:

| Field      | What it does |
| ---------- | ------------ |
| `Sq`       | Sequence 1–99. Chosen while playing, it starts when the current one ends |
| `Now`      | Position as bar.beat.tick (96 ticks per quarter note) |
| `Tempo`    | 30–300 BPM. **TAP TEMPO** (or <kbd>T</kbd>) sets it from your taps |
| `Timing`   | Quantize grid for recording: OFF, 1/8, 1/16, 1/32 and their triplets |
| `Swing`    | 50–75 %, for 1/8 and 1/16 |
| `Tsig`     | Time signature |
| `Count`    | One bar of count-in before recording |
| `Click`    | Metronome while playing and recording |
| `Loop`     | Loop the sequence |
| `Bars`     | Sequence length |
| `Tr`, `On` | Track 1–64 and whether it plays |
| `Bus`      | DRUM1–4: the [program](programs.md) the track plays; MIDI: no sound of its own, only [MIDI out](midi.md) |
| `Out`      | MIDI channel the track's notes are sent on, OFF by default |

## Recording

1. Press **REC** (<kbd>Ctrl</kbd>+<kbd>R</kbd>, replace) or **OVERDUB** (<kbd>Ctrl</kbd>+<kbd>O</kbd>,
   add). Its light shows it is armed.
2. Press **PLAY** (<kbd>Space</kbd>) or **PLAY START** (<kbd>Shift</kbd>+<kbd>Space</kbd>). After the
   count-in, play the pads.
3. Press **STOP**.

**REC** erases what is on the track as the playhead passes, and turns into **OVERDUB** after the
first loop so the new take is kept. Pressing **REC** or **OVERDUB** while playing punches in or
out. Notes are quantized to `Timing` as they are recorded; **OPEN WINDOW** on the `Timing` field
(timing correct) quantizes a whole track afterwards.

**UNDO SEQ** (<kbd>Ctrl</kbd>+<kbd>Z</kbd>) takes back the last recording or edit; its light shows
that it can. Hold **ERASE** (or <kbd>Delete</kbd>) while playing and hold pads to delete their
notes as the playhead passes.

**TR.ON** (F3) mutes the track, **SOLO** (F4) plays only this track, and **TR -** / **TR +** (F5/F6)
change track.

### Sequence and track windows

**OPEN WINDOW** (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) on the `Sq` field opens the SEQUENCE window, and
on the `Tr` field the TRACK window:

- **NAME** (F5) names the sequence or track (16 characters, <kbd>Enter</kbd> to keep it,
  <kbd>Esc</kbd> to cancel).
- **COPY** (F3) copies it over the one chosen in `Copy to`.
- **DELETE** (F4) clears it.
- **CLOSE** (F6) goes back.

UNDO SEQ takes back a copy or delete.

## Playing live

**TRACK MUTE** (<kbd>F10</kbd>) turns the pads into track switches, for muting parts live: pad 1
mutes or unmutes track 1, and so on, with PAD BANK B–D for tracks 17–64. The LCD shows the tracks
laid out like the pads, `*` marking those with notes. **SOLO** (F6) and then a pad plays only that
track; SOLO again brings all tracks back.

**NEXT SEQ** (<kbd>F9</kbd>) does the same for sequences: while playing, a pad queues its sequence
(pad 1 = sequence 1, bank B = 17–32, …) to start when the current one ends; while stopped it
selects it. Press either key again to go back to MAIN.

**NOTE REPEAT**: while playing, hold **TAP TEMPO / NOTE REPEAT** (or <kbd>T</kbd>) and hold pads;
they play again on every `Timing` grid line (1/16 when `Timing` is OFF) and are recorded when
recording. While stopped the same key taps the tempo.

**LOCATE**: **STEP <** / **>** (<kbd>,</kbd> / <kbd>.</kbd>) move by the `Timing` grid,
**BAR <<** / **>>** (<kbd>Page Up</kbd> / <kbd>Page Down</kbd>) by a bar, and **GO TO**
(<kbd>Home</kbd>) returns to the start. **STOP** pressed while stopped also returns to the start.

## STEP EDIT

**STEP** (F1 on the MAIN screen) lists the notes of the current track. **↑** / **↓** choose a note
and move `Now` to it; **←** / **→** move between its position, pad, velocity and duration, and the
DATA wheel changes them (position and duration move by the `Timing` grid, or by single ticks with
<kbd>Shift</kbd>).

Hitting a pad while stopped adds a note at `Now`, so you can step-record with **STEP <** / **>**.
**DELETE** (F1) removes the selected note, **TR -** / **TR +** change track, and **UNDO SEQ** takes
back everything done since opening the screen.

## EDIT: copying bars and notes

**EDIT** (F2 on the MAIN screen) works on whole stretches of a sequence, like the MPC's EDIT mode.
It opens on the whole current sequence; **EVENTS** (F1) and **BARS** (F2) switch between its two
pages, and **DO IT** (F6) applies the edit and returns to MAIN. **UNDO SEQ** takes it back. Times
move by bars, or by beats with <kbd>Shift</kbd>.

### BARS

The quickest way to turn a 2-bar loop into 8 bars: bars `First bar` to `Last bar` of `From sq` are
inserted `Copies` times into `To sq` after `After bar` (0 = before the first bar), and the bars
after them move back. Both sequences need the same time signature.

### EVENTS

Works on the notes of the current track (`Sq`, `Tr`) within `Time`, of one pad or ALL (`Notes`):

| Edit     | What it does |
| -------- | ------------ |
| COPY     | Copies the notes to `To sq` / `tr` from `Start`, `Copies` times one after another. REPLACE clears that stretch first, MERGE adds to it |
| DURATION | Changes their length: `Mode` ADD VALUE, SUB VALUE, MULTI VAL% or SET TO VAL, by `Value` ticks (or percent) |
| VELOCITY | Changes their velocity the same way, between 1 and 127 |

## MISC: punch, transpose and a second sequence

The blue **2 / MISC** key (<kbd>Ctrl</kbd>+<kbd>2</kbd>) has three pages, **PUNCH** (F1), **TRANS**
(F2) and **2ND SEQ** (F3); **ON/OFF** (F6) switches the one shown on or off.

- **PUNCH** limits recording to a stretch of the sequence, so a take can be redone without
  touching the bars around it: `Auto punch` PUNCH IN ONLY records from `Time` on, PUNCH OUT ONLY
  until it, AUTO IN/OUT only between the two times. Outside them the sequence just plays.
- **TRANS** moves the MIDI out notes of one track (`Tr`) or ALL by up to an octave either way
  while playing, for playing a bass line on an external synth in another key. Pads are not
  affected.
- **2ND SEQ** plays another sequence (`Sq`) along with the current one, looping on its own length,
  for example drums in one sequence and a bassline in another. WAV export includes it.

## SONG

The blue **1 / SONG** key (<kbd>Ctrl</kbd>+<kbd>1</kbd>) chains sequences into a song. **INSERT**
(F1) adds a step after the selected one, playing the sequence chosen on the MAIN screen; **DELETE**
(F2) removes it. **↑** / **↓** choose a step, and its `Sq` and `Reps` (how many times it plays) are
set with the DATA wheel. `Loop` starts the song over at the end.

On this screen **PLAY START** plays the song from the first step and **PLAY** from the selected
one; `>` marks the step that is playing. Song mode does not record.
