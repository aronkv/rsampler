//! The effects on the FX SEND bus, in place of the MPC2000XL's optional effects board: a reverb
//! and a delay timed to the sequence tempo. Pads reach them with their FX SEND level.

use serde::{Deserialize, Serialize};

/// Delay times as fractions of a beat (quarter note), and their labels.
pub const DELAY_TIMES: [(&str, f32); 7] = [
    ("1/4", 1.0),
    ("1/8D", 0.75),
    ("1/4T", 2.0 / 3.0),
    ("1/8", 0.5),
    ("1/16D", 0.375),
    ("1/8T", 1.0 / 3.0),
    ("1/16", 0.25),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FxSettings {
    /// Each 0..=100.
    pub reverb: u32,
    pub size: u32,
    pub damp: u32,
    pub delay: u32,
    /// Index into `DELAY_TIMES`.
    pub time: usize,
    pub feedback: u32,
}

impl Default for FxSettings {
    fn default() -> Self {
        Self { reverb: 30, size: 60, damp: 50, delay: 0, time: 1, feedback: 35 }
    }
}

/// A Freeverb comb filter: a delay line fed back through a low-pass.
struct Comb {
    buf: Vec<f32>,
    pos: usize,
    store: f32,
}

impl Comb {
    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buf[self.pos];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.pos] = x + self.store * feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn process(&mut self, x: f32) -> f32 {
        let delayed = self.buf[self.pos];
        self.buf[self.pos] = x + delayed * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        delayed - x
    }
}

pub struct Fx {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    delay: Vec<[f32; 2]>,
    delay_pos: usize,
    rate: f32,
    /// Loudest recent output, so a bounce can let the tail ring out.
    pub level: f32,
}

impl Fx {
    pub fn new(rate: u32) -> Self {
        // Freeverb's tunings at 44.1 kHz, the right channel slightly longer for width.
        let scale = rate as f32 / 44_100.0;
        let len = |n: usize, ch: usize| ((n + ch * 23) as f32 * scale) as usize + 1;
        let combs = [0, 1].map(|ch| {
            [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617]
                .map(|n| Comb { buf: vec![0.0; len(n, ch)], pos: 0, store: 0.0 })
                .into()
        });
        let allpasses =
            [0, 1].map(|ch| [556, 441, 341, 225].map(|n| Allpass { buf: vec![0.0; len(n, ch)], pos: 0 }).into());
        Self { combs, allpasses, delay: vec![[0.0; 2]; rate as usize * 2], delay_pos: 0, rate: rate as f32, level: 0.0 }
    }

    /// The effects' output for one frame of the send bus.
    pub fn process(&mut self, send: [f32; 2], s: &FxSettings, tempo: f32) -> [f32; 2] {
        let mut out = [0.0; 2];
        if s.reverb > 0 {
            let feedback = 0.7 + 0.28 * s.size as f32 / 100.0;
            let damp = 0.4 * s.damp as f32 / 100.0;
            let input = (send[0] + send[1]) * 0.015;
            // Freeverb scales its wet output by 3; 2 leaves headroom with a full send.
            let wet = 2.0 * s.reverb as f32 / 100.0;
            for (o, (combs, allpasses)) in out.iter_mut().zip(self.combs.iter_mut().zip(&mut self.allpasses)) {
                let mut x: f32 = combs.iter_mut().map(|c| c.process(input, feedback, damp)).sum();
                for a in allpasses {
                    x = a.process(x);
                }
                *o += x * wet;
            }
        }
        if s.delay > 0 || self.level > 1e-4 {
            let beats = DELAY_TIMES[s.time.min(DELAY_TIMES.len() - 1)].1;
            let frames = ((beats * 60.0 / tempo.max(1.0) * self.rate) as usize).clamp(1, self.delay.len() - 1);
            let read = (self.delay_pos + self.delay.len() - frames) % self.delay.len();
            let echo = self.delay[read];
            let feedback = s.feedback.min(95) as f32 / 100.0;
            let level = s.delay as f32 / 100.0;
            for ch in 0..2 {
                self.delay[self.delay_pos][ch] = send[ch] * level + echo[ch] * feedback;
                out[ch] += echo[ch];
            }
            self.delay_pos = (self.delay_pos + 1) % self.delay.len();
        }
        self.level = (self.level * 0.9999).max(out[0].abs().max(out[1].abs()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_repeats_on_the_beat_and_reverb_rings() {
        let rate = 8_000;
        let mut fx = Fx::new(rate);
        // Delay only: a click comes back one quarter note later at 120 BPM (0.5 s).
        let s = FxSettings { reverb: 0, delay: 100, time: 0, feedback: 50, ..FxSettings::default() };
        let out: Vec<f32> =
            (0..rate * 2).map(|i| fx.process(if i == 0 { [1.0, 1.0] } else { [0.0; 2] }, &s, 120.0)[0]).collect();
        let echoes: Vec<usize> = (0..out.len()).filter(|&i| out[i] > 0.1).collect();
        assert_eq!(echoes, [4000, 8000, 12000]);
        assert!((out[8000] - 0.5).abs() < 1e-6);

        let mut fx = Fx::new(rate);
        let s = FxSettings { reverb: 100, delay: 0, ..FxSettings::default() };
        let tail: f32 = (0..rate)
            .map(|i| fx.process(if i == 0 { [1.0, 1.0] } else { [0.0; 2] }, &s, 120.0))
            .skip(rate as usize / 2)
            .map(|f| f[0].abs())
            .sum();
        assert!(tail > 0.01, "reverb tail {tail}");
        // Off, it is silent.
        let mut fx = Fx::new(rate);
        let off = FxSettings { reverb: 0, delay: 0, ..FxSettings::default() };
        assert_eq!(fx.process([1.0; 2], &off, 120.0), [0.0; 2]);
    }
}
