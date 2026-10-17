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
    /// LOOP: with `looping` on, playback jumps back to `loop_to` from the end for as long as the
    /// note is held.
    pub loop_to: usize,
    pub looping: bool,
}

impl Sound {
    pub fn new(name: impl Into<String>, frames: Frames) -> Self {
        let end = frames.len();
        Self { name: name.into(), frames, start: 0, end, loop_to: 0, looping: false }
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

/// How a hit chooses its sounds: the MPC's sound generation mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Generation {
    #[default]
    Normal,
    /// Also plays up to two other pads, layered.
    Simult,
    /// Hits louder than a threshold play another pad instead.
    VelSw,
}

impl Generation {
    pub const ALL: [Generation; 3] = [Generation::Normal, Generation::Simult, Generation::VelSw];

    pub fn label(self) -> &'static str {
        match self {
            Generation::Normal => "NORMAL",
            Generation::Simult => "SIMULT",
            Generation::VelSw => "VEL SW",
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

/// The MPC's VELOCITY MODULATION: how much a softer hit changes the sound, each 0..=100.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Velo {
    /// Softer is quieter: 100 follows the velocity fully, 0 plays every hit at full level.
    pub level: u32,
    /// Softer hits fade in more slowly.
    pub attack: u32,
    /// Softer hits start later into the sound.
    pub start: u32,
    /// Harder hits open the filter further.
    pub filter: u32,
}

impl Default for Velo {
    fn default() -> Self {
        Self { level: 100, attack: 0, start: 0, filter: 0 }
    }
}

impl Velo {
    /// The share of `amount` a hit of `velocity` gets, growing as the hit gets softer: the MPC's
    /// `(128 − velocity) × amount / 127`.
    pub fn softness(velocity: u8, amount: u32) -> u32 {
        (128 - velocity.min(127) as u32) * amount / 127
    }
}

#[derive(Clone)]
pub struct Pad {
    pub sound: Option<Sound>,
    /// Tenths of a semitone, −120..=120.
    pub tune: i32,
    /// 0..=100; 0 starts instantly.
    pub attack: u32,
    /// Decay time 0..=100 (up to two seconds): how long the fade-out before the end of the sound
    /// lasts (END), or how slowly it dies away after the attack (START).
    pub decay: u32,
    /// Decay right after the attack (START) instead of before the sound's end (END).
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
    pub generation: Generation,
    /// The other pads for SIMULT and VEL SW.
    pub also: [Option<u8>; 2],
    /// VEL SW: a velocity above `switch[i]` plays `also[i]` (the higher one wins).
    pub switch: [u8; 2],
    pub velo: Velo,
}

impl Default for Pad {
    fn default() -> Self {
        Self {
            sound: None,
            tune: 0,
            attack: 0,
            // The MPC's default: a short fade at the very end.
            decay: 5,
            decay_from_start: false,
            cutoff: 100,
            resonance: 0,
            overlap: Overlap::Poly,
            mutes: [None, None],
            level: 100,
            pan: 0,
            generation: Generation::Normal,
            also: [None, None],
            switch: [64, 100],
            velo: Velo::default(),
        }
    }
}

impl Pad {
    /// A looping sound plays until its note ends, whatever `overlap` says, as on the MPC.
    pub fn note_off(&self) -> bool {
        self.overlap == Overlap::NoteOff || self.sound.as_ref().is_some_and(|s| s.looping)
    }

    /// The pads a hit plays: itself, plus its SIMULT layers, or the pad VEL SW picks.
    pub fn targets(&self, pad: u8, velocity: u8) -> impl Iterator<Item = u8> {
        let (first, layers) = match self.generation {
            Generation::Normal => (pad, [None, None]),
            Generation::Simult => (pad, self.also),
            Generation::VelSw => {
                let pick = (0..2).rev().find_map(|i| self.also[i].filter(|_| velocity > self.switch[i]));
                (pick.unwrap_or(pad), [None, None])
            }
        };
        std::iter::once(first).chain(layers.into_iter().flatten())
    }

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
            generation: self.generation,
            also: self.also,
            switch: self.switch,
            velo: self.velo,
        };
        if let Some((param, v)) = var {
            let scaled = v.min(127) as u32 * 100 / 127;
            match param {
                VarParam::Tune => {
                    // As on the MPC: 0.2 semitone per step from 64, up to an octave either way,
                    // so 5 steps are exactly a semitone.
                    let offset = ((v as i32 - 64) * 2).clamp(-120, 120);
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
    fn generation_targets() {
        let mut p = Pad { also: [Some(5), Some(9)], switch: [44, 88], ..Pad::default() };
        assert_eq!(p.targets(1, 100).collect::<Vec<_>>(), [1]);
        p.generation = Generation::Simult;
        assert_eq!(p.targets(1, 100).collect::<Vec<_>>(), [1, 5, 9]);
        p.generation = Generation::VelSw;
        assert_eq!(p.targets(1, 30).collect::<Vec<_>>(), [1]);
        assert_eq!(p.targets(1, 60).collect::<Vec<_>>(), [5]);
        assert_eq!(p.targets(1, 120).collect::<Vec<_>>(), [9]);
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
        assert_eq!(p.varied(Some((VarParam::Tune, 69))).tune, 20);
        assert_eq!(p.varied(Some((VarParam::Filter, 127))).cutoff, 100);
        assert_eq!(p.varied(Some((VarParam::Decay, 0))).decay, 0);
    }
}
