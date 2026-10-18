---
title: Pads
nav_order: 3
---

# Pads
{: .no_toc }

1. TOC
{:toc}

---

## Playing the pads

Click a pad or hit its key. With the mouse, hitting higher on a pad plays louder; keys always play
at velocity 101 (changeable in [SETUP](midi#setup-and-other)), and **FULL LEVEL** (<kbd>F7</kbd>)
makes everything 127. A [MIDI controller](midi) plays them with its own velocity.

The keys follow their physical position, so the layout works the same on QWERTY, QWERTZ
(Hungarian, German) and AZERTY keyboards:

| Row    | Pads  | QWERTY | Hungarian QWERTZ |
| ------ | ----- | ------ | ---------------- |
| top    | 13–16 | <kbd>1</kbd> <kbd>2</kbd> <kbd>3</kbd> <kbd>4</kbd> | <kbd>1</kbd> <kbd>2</kbd> <kbd>3</kbd> <kbd>4</kbd> |
|        | 9–12  | <kbd>Q</kbd> <kbd>W</kbd> <kbd>E</kbd> <kbd>R</kbd> | <kbd>Q</kbd> <kbd>W</kbd> <kbd>E</kbd> <kbd>R</kbd> |
|        | 5–8   | <kbd>A</kbd> <kbd>S</kbd> <kbd>D</kbd> <kbd>F</kbd> | <kbd>A</kbd> <kbd>S</kbd> <kbd>D</kbd> <kbd>F</kbd> |
| bottom | 1–4   | <kbd>Z</kbd> <kbd>X</kbd> <kbd>C</kbd> <kbd>V</kbd> | <kbd>Y</kbd> <kbd>X</kbd> <kbd>C</kbd> <kbd>V</kbd> |

Keys pressed together with <kbd>Shift</kbd>, <kbd>Ctrl</kbd> or <kbd>Alt</kbd> never play pads, so
shortcuts and symbols such as <kbd>+</kbd> (<kbd>Shift</kbd>+<kbd>3</kbd> on a Hungarian keyboard)
work.

## Pad banks

**PAD BANK A–D** (<kbd>Tab</kbd> / <kbd>Shift</kbd>+<kbd>Tab</kbd>) switches between four banks of
16 pads (A01–D16). Bank A starts with a built-in kick, snare, hi-hat and clap on pads 1–4.

The last pad you hit is the **selected pad** that the PROGRAM, MIXER, TRIM and LOAD screens work on.

## Stopping a sound

A pad plays its whole sound, even after you let go. To make it stop:

- Set its `Overlap` to **NOTE OFF** on the [PROGRAM](programs) screen: it stops when the pad is
  released (or, in a sequence, when the note ends).
- **MONO** cuts the previous hit when the pad is hit again; `Mutes` lets one pad cut off another,
  like a closed hi-hat stopping an open one.
- A looping sound always stops when the pad is released.
- Long sounds can be shortened with `End` on the [TRIM](sounds#trim-loop-and-zone) screen.

## 16 LEVELS

**16 LEVELS** (<kbd>F8</kbd>) spreads one sound over all 16 pads and opens its ASSIGN screen:
`Note` is the pad whose sound is spread (the selected one to begin with), and `Param` what changes
from pad to pad:

- **VELOCITY**: velocity 7 on pad 1 up to 127 on pad 16.
- **NOTE VAR**: the `Type` TUNE, DECAY, ATTACK or FILTER changes across the pads. TUNE plays the
  sound at its own pitch on the `Original key pad` (4 by default) and a semitone higher on each
  pad after it, so a sound can be played like a keyboard; the others run from 6 on pad 1 to 96 on
  pad 16.

Notes recorded this way keep their variation. Press 16 LEVELS again to go back.

## NOTE VARIATION

The slider changes one pad's sound each time it is hit: drag it, scroll over it, or double-click
to centre it. **AFTER** opens its ASSIGN screen, where `Pad` chooses the pad and `Param` what the
slider changes:

| Param  | Slider bottom … top                                  |
| ------ | ---------------------------------------------------- |
| TUNE   | one octave down … one octave up (centre = no change) |
| DECAY  | 0 … 100                                              |
| ATTACK | 0 … 100                                              |
| FILTER | closed … open                                        |

The value is recorded with each note and played back with it.

{: .mpc }
On the MPC, AFTER makes pad pressure drive the variation. Keyboards and mice have no pressure, so
here it only opens ASSIGN.
