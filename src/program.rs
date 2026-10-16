//! The program: what each of the 64 pads (A01–D16) plays, and how.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
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

/// What the NOTE VARIATION slider changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum VarParam {
    Tune,
    Decay,
    Attack,
    Filter,
}

impl VarParam {
    pub const ALL: [VarParam; 4] = [VarParam::Tune, VarParam::Decay, VarParam::Attack, VarParam::Filter];

    pub fn label(self) -> &'static str {
        match self {
            VarParam::Tune => "TUNE",
            VarParam::Decay => "DECAY",
            VarParam::Attack => "ATTACK",
            VarParam::Filter => "FILTER",
        }
    }
}

/// A NOTE VARIATION slider position (0..=127) applied to one hit.
pub type Variation = Option<(VarParam, u8)>;

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

impl Pad {
    /// The pad's parameters, without the sound, with a NOTE VARIATION applied. TUNE moves up to
    /// an octave either way from the slider's centre (64); the others replace the setting.
    pub fn varied(&self, var: Variation) -> Pad {
        let mut p = Pad {
            sound: None,
            tune: self.tune,
            attack: self.attack,
            decay: self.decay,
            decay_from_start: self.decay_from_start,
            cutoff: self.cutoff,
            resonance: self.resonance,
            overlap: self.overlap,
            mutes: self.mutes,
            level: self.level,
            pan: self.pan,
        };
        if let Some((param, v)) = var {
            let scaled = v.min(127) as u32 * 100 / 127;
            match param {
                VarParam::Tune => {
                    // 0..64 and 64..127 each span a full octave.
                    let offset = (v as i32 - 64) * 120 / if v >= 64 { 63 } else { 64 };
                    p.tune = (p.tune + offset).clamp(-120, 120);
                }
                VarParam::Decay => p.decay = scaled,
                VarParam::Attack => p.attack = scaled,
                VarParam::Filter => p.cutoff = scaled,
            }
        }
        p
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

    #[test]
    fn variation() {
        let p = Pad { tune: 10, ..Pad::default() };
        assert_eq!(p.varied(None).tune, 10);
        assert_eq!(p.varied(Some((VarParam::Tune, 64))).tune, 10);
        assert_eq!(p.varied(Some((VarParam::Tune, 0))).tune, -110);
        assert_eq!(p.varied(Some((VarParam::Tune, 127))).tune, 120);
        assert_eq!(Pad::default().varied(Some((VarParam::Tune, 127))).tune, 120);
        assert_eq!(Pad::default().varied(Some((VarParam::Tune, 0))).tune, -120);
        assert_eq!(p.varied(Some((VarParam::Filter, 127))).cutoff, 100);
        assert_eq!(p.varied(Some((VarParam::Decay, 0))).decay, 0);
    }
}
