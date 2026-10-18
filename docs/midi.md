---
title: MIDI and setup
nav_order: 8
---

# MIDI and setup
{: .no_toc }

1. TOC
{:toc}

---

## MIDI in

rsampler listens to every MIDI input it finds, including controllers plugged in while it runs, and
on Linux it also offers its own `rsampler` input port for other programs. Notes 36–99 (C1–D#6) play
pads A01–D16 with their velocity, whatever bank is selected, and are recorded like pad hits.

{: .tip }
Most 16-pad controllers send notes 36–51 by default, which lands on bank A.

## MIDI out

`MIDI out` in [SETUP](#setup-and-other) chooses where MIDI goes:

- on Linux, by default, rsampler's own `rsampler` port, for a synth or DAW to connect to;
- any MIDI port the system has: a USB interface, a synth, or on Windows the built-in Microsoft GS
  Wavetable Synth;
- OFF sends nothing.

The choice is saved with the project. A track whose `Out` field is set to a channel sends its notes
there as it plays (pad A01 = note 36 again), and pads you hit pass through on the selected track's
channel, so the pads can play an external synth.

{: .note }
Windows has no virtual MIDI ports, so there is no own `rsampler` port there; choose an output in
SETUP instead.

## MIDI tracks

Tracks with `Bus` set to MIDI on the MAIN screen make rsampler a sequencer for synths: they take
every note 0–127 from a MIDI keyboard as it is (the pads play 36–99), record and play them back
over MIDI on the track's `Out` channel, and make no sound of their own. STEP EDIT shows their notes
by name (C3 is middle C, as on the MPC) and changes them over the whole range.

## Clock

**Clock out:** with `MIDI clock out` on in SETUP, rsampler sends MIDI clock while playing, and
start, stop, or song position and continue, so a DAW or drum machine can follow its tempo.

**Clock in:** with `Sync` set to MIDI CLOCK in SETUP, rsampler follows another device instead: it
starts, stops and locates when the master does, plays at the master's tempo (shown as `EXT` on the
MAIN screen and in SETUP) and stays locked to its clock pulses. PLAY on rsampler itself is
disabled while syncing; STOP still works.

## SETUP and OTHER

The blue **7 / SETUP** key (<kbd>Ctrl</kbd>+<kbd>7</kbd>) holds `MIDI clock out`, `Sync` (INTERNAL
or MIDI CLOCK), the velocity of pads hit from the computer keyboard (101 by default) and the
`MIDI out` port; all are saved with the project.

**9 / OTHER** (<kbd>Ctrl</kbd>+<kbd>9</kbd>) shows the version and the output sample rate.
