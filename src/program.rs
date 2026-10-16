//! The program: what each of the 64 pads (A01–D16) plays, and how.

use std::sync::Arc;

pub const PADS: usize = 64;

/// Stereo frames at the output sample rate.
pub type Frames = Arc<[[f32; 2]]>;

#[derive(Clone)]
pub struct Sound {
    pub name: String,
    pub frames: Frames,
    /// Playback range in frames; TRIM edits these.
    pub start: usize,
    pub end: usize,
}

impl Sound {
    pub fn new(name: impl Into<String>, frames: Frames) -> Self {
        let end = frames.len();
        Self { name: name.into(), frames, start: 0, end }
    }
}

/// What happens when a pad is hit while its previous hit still sounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlap {
    /// Hits layer.
    Poly,
    /// A new hit cuts the previous one.
    Mono,
    /// The sound stops when the pad is released.
    NoteOff,
}

impl Overlap {
    pub fn label(self) -> &'static str {
        match self {
            Overlap::Poly => "POLY",
            Overlap::Mono => "MONO",
            Overlap::NoteOff => "NOTE OFF",
        }
    }
}

#[derive(Clone)]
pub struct Pad {
    pub sound: Option<Sound>,
    /// Tenths of a semitone, −120..=120.
    pub tune: i32,
    /// 0..=100; 0 starts instantly.
    pub attack: u32,
    /// 0..=100; 100 lets the sound play out.
    pub decay: u32,
    /// Decay measured from the attack (START) instead of towards the sound's end (END).
    pub decay_from_start: bool,
    /// Low-pass cutoff 0..=100; 100 is fully open.
    pub cutoff: u32,
    /// Filter resonance 0..=100.
    pub resonance: u32,
    pub overlap: Overlap,
    /// Pads this pad cuts off when hit (the MPC's mute assign).
    pub mutes: [Option<u8>; 2],
    /// Mixer level 0..=100.
    pub level: u32,
    /// Mixer pan −50 (left) ..= 50 (right).
    pub pan: i32,
}

impl Default for Pad {
    fn default() -> Self {
        Self {
            sound: None,
            tune: 0,
            attack: 0,
            decay: 100,
            decay_from_start: false,
            cutoff: 100,
            resonance: 0,
            overlap: Overlap::Poly,
            mutes: [None, None],
            level: 100,
            pan: 0,
        }
    }
}

/// Pad name as printed on the MPC, e.g. 17 → "B02".
pub fn pad_name(pad: usize) -> String {
    format!("{}{:02}", ["A", "B", "C", "D"][pad / 16 % 4], pad % 16 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_names() {
        assert_eq!(pad_name(0), "A01");
        assert_eq!(pad_name(17), "B02");
        assert_eq!(pad_name(63), "D16");
    }
}
