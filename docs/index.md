---
title: Home
layout: home
nav_order: 1
---

<div class="hero" markdown="1">

# rsampler
{: .fs-9 }

A desktop sampler inspired by the Akai MPC2000XL, written in pure Rust.
{: .fs-6 .fw-300 }

[Get started](getting-started){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[Download](https://github.com/aronkv/rsampler/releases/latest){: .btn .fs-5 .mb-4 .mb-md-0 }

</div>

<img class="screenshot" src="{{ '/assets/screenshot.png' | relative_url }}" alt="rsampler playing a sequence: the LCD, DATA wheel, transport and 16 pads">

rsampler works like the MPC2000XL: one LCD screen at a time, CURSOR keys to move between fields,
the DATA wheel to change them, and 16 velocity pads to play. If you have used an MPC, you already
know your way around; if not, this guide takes you through it.

## What it does

| | |
| --- | --- |
| **[Pads](pads)** | 16 velocity pads in four banks, played with the mouse, the computer keyboard or a MIDI controller. 16 LEVELS and NOTE VARIATION. |
| **[Sequencer](sequencing)** | 99 sequences of 64 tracks, quantize and swing, overdub, note repeat, step and bar editing, punch in and out, song mode. |
| **[Programs](programs)** | 24 programs on four DRUM buses: tuning, envelopes, filter with its own envelope, velocity modulation, mute groups, layers and velocity switching. |
| **[Mixer and effects](programs#mixer-and-effects)** | Level and pan per pad, a reverb and a tempo-synced delay on the FX send. |
| **[Sounds](sounds)** | Load WAV, MP3, FLAC, M4A, Ogg and AIFF, sample from the input, trim, loop, chop into zones, stretch, resample, slice. |
| **[Files](files)** | The MPC2000XL's own `.SND`, `.PGM`, `.APS` and `.ALL` files, MIDI files, projects and WAV export. |
| **[MIDI](midi)** | MIDI in from any controller, MIDI out per track, MIDI tracks for synths, clock in and out. |

## Platforms

Linux and Windows, from the same code. See [Getting started](getting-started) for installing.

{: .note }
[izzyreal/mpc](https://github.com/izzyreal/mpc) (the engine behind VMPC2000XL) is used as a
behavioural and file-format reference only; rsampler contains no code from it.
