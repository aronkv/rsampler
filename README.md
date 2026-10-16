# rsampler

A desktop sampler inspired by the Akai MPC2000XL, written in pure Rust.

> **Status:** project skeleton only, nothing is implemented yet.

## Goals

- MPC2000XL-style screens in a native, lightweight GUI
- 16 velocity pads, four banks (A–D)
- Programs: sample assignment per pad, tuning, envelopes, mixer
- Sequencer: tracks, tempo, quantize, loop, step editing
- Loading and saving the original file formats (`.SND`, `.PGM`, `.APS`, `.SEQ`/`.MID`, `.ALL`)

## Platforms

Linux first, Windows later.

## Build

```sh
cargo run
```

## Reference

[izzyreal/mpc](https://github.com/izzyreal/mpc) (the engine behind VMPC2000XL) is used as a behavioural and
file-format reference only. rsampler contains no code from it.

## Disclaimer

MPC® and Akai Professional® are registered trademarks of inMusic Brands, Inc. This project is not affiliated with,
endorsed by or sponsored by inMusic.

## License

Apache License 2.0, see [LICENSE](LICENSE).
