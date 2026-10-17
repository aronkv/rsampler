//! The MPC's sound EDIT operations, each on a section of a sound: St–End on TRIM, the loop on
//! LOOP, the selected zone on ZONE.

use std::ops::Range;

use crate::program::Sound;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// Keeps only the section.
    Discard,
    /// Loops from St to End.
    LoopStEnd,
    /// Copies the section to another pad as a new sound.
    NewSound,
    /// Puts another pad's sound in at the start of the section.
    Insert,
    Delete,
    Silence,
    Reverse,
    /// A new sound of the section played longer or shorter at the same pitch, on `To pad`.
    TimeStretch,
    Normalize,
    /// Every zone to its own pad (ZONE only).
    Slice,
}

impl Op {
    pub const ALL: [Op; 10] = [
        Op::Discard,
        Op::LoopStEnd,
        Op::NewSound,
        Op::Insert,
        Op::Delete,
        Op::Silence,
        Op::Reverse,
        Op::TimeStretch,
        Op::Normalize,
        Op::Slice,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Op::Discard => "DISCARD",
            Op::LoopStEnd => "LOOP FROM ST TO END",
            Op::NewSound => "SECTION -> NEW SOUND",
            Op::Insert => "INSERT SOUND -> SECTION START",
            Op::Delete => "DELETE SECTION",
            Op::Silence => "SILENCE SECTION",
            Op::Reverse => "REVERSE SECTION",
            Op::TimeStretch => "TIME STRETCH",
            Op::Normalize => "NORMALIZE SECTION",
            Op::Slice => "SLICE SOUND",
        }
    }

    /// Whether the operation puts sounds on other pads, chosen with `To pad`.
    pub fn makes_sounds(self) -> bool {
        matches!(self, Op::NewSound | Op::Slice | Op::TimeStretch)
    }
}

/// A point after `cut` is removed shifts back with the frames after it.
fn after_cut(v: usize, cut: &Range<usize>) -> usize {
    if v >= cut.end { v - cut.len() } else { v.min(cut.start) }
}

/// Applies an operation that changes the sound itself (all but NewSound and Slice).
pub fn apply(snd: &mut Sound, op: Op, section: Range<usize>) {
    let len = snd.frames.len();
    let section = section.start.min(len)..section.end.min(len);
    let mut frames = snd.frames.to_vec();
    match op {
        Op::Discard => {
            frames = frames[section.clone()].to_vec();
            snd.loop_to = snd.loop_to.saturating_sub(section.start);
            (snd.start, snd.end) = (0, frames.len());
        }
        Op::LoopStEnd => {
            (snd.loop_to, snd.looping) = (snd.start, true);
            return;
        }
        Op::Delete => {
            // The MPC keeps at least one frame.
            if section.len() >= len {
                return;
            }
            frames.drain(section.clone());
            (snd.start, snd.end) = (after_cut(snd.start, &section), after_cut(snd.end, &section));
            snd.loop_to = after_cut(snd.loop_to, &section);
        }
        Op::Silence => frames[section].fill([0.0; 2]),
        Op::Reverse => frames[section].reverse(),
        Op::Normalize => {
            let peak = frames[section.clone()].iter().flatten().fold(0.0f32, |m, x| m.max(x.abs()));
            if peak > 0.0 {
                frames[section].iter_mut().flatten().for_each(|x| *x /= peak);
            }
        }
        Op::NewSound | Op::Slice | Op::TimeStretch | Op::Insert => return,
    }
    let len = frames.len().max(1);
    snd.end = snd.end.clamp(1, len);
    snd.start = snd.start.min(snd.end - 1);
    snd.loop_to = snd.loop_to.min(snd.end - 1);
    snd.frames = frames.into();
}

/// INSERT SOUND: `other` goes in at `at`; the points after it move back with the audio.
pub fn insert(snd: &mut Sound, at: usize, other: &[[f32; 2]]) {
    let at = at.min(snd.frames.len());
    let mut frames = snd.frames.to_vec();
    frames.splice(at..at, other.iter().copied());
    let moved = |v: usize| if v > at { v + other.len() } else { v };
    (snd.end, snd.loop_to) = (moved(snd.end), moved(snd.loop_to));
    if snd.start > at {
        snd.start += other.len();
    }
    snd.frames = frames.into();
}

/// A section of a sound as a sound of its own.
pub fn section(snd: &Sound, section: Range<usize>, name: String) -> Sound {
    let end = section.end.min(snd.frames.len());
    let start = section.start.min(end);
    Sound::new(name, snd.frames[start..end].to_vec().into())
}

/// Time stretching by WSOLA: overlapping windows of the input are laid out at a new spacing, each
/// shifted by up to a few milliseconds to where it lines up best with the audio before it, so the
/// pitch stays and there are no phase jumps. `ratio` is the new length over the old, 0.5..=2.
pub fn stretch(x: &[[f32; 2]], ratio: f64, rate: u32) -> Vec<[f32; 2]> {
    let win = (rate as usize * 40 / 1000).max(16) & !1;
    let hop = win / 2;
    let tolerance = rate as usize * 8 / 1000;
    let out_len = (x.len() as f64 * ratio).round() as usize;
    if x.len() < win * 2 || out_len < win {
        // Too short to stretch: plain resampling would change the pitch, so keep it as it is.
        return x.to_vec();
    }
    let mono = |i: usize| x.get(i).map_or(0.0, |f| f[0] + f[1]);
    let hann: Vec<f32> = (0..win).map(|i| (std::f32::consts::PI * i as f32 / win as f32).sin().powi(2)).collect();
    let mut out = vec![[0.0f32; 2]; out_len + win];
    // Where the previous window's continuation is in the input.
    let mut natural = 0usize;
    for k in 0..=out_len / hop {
        let nominal = (k as f64 * hop as f64 / ratio) as usize;
        let pos = if k == 0 {
            0
        } else {
            // ponytail: correlates every 4th sample to stay fast; full resolution if it ever smears.
            let lo = nominal.saturating_sub(tolerance);
            let hi = (nominal + tolerance).min(x.len().saturating_sub(win));
            let score = |p: usize| (0..win).step_by(4).map(|i| mono(p + i) * mono(natural + i)).sum::<f32>();
            (lo..=hi.max(lo)).map(|p| (p, score(p))).max_by(|a, b| a.1.total_cmp(&b.1)).map_or(nominal, |(p, _)| p)
        };
        for i in 0..win {
            let Some(f) = x.get(pos + i) else { break };
            let o = &mut out[k * hop + i];
            o[0] += f[0] * hann[i];
            o[1] += f[1] * hann[i];
        }
        natural = pos + hop;
    }
    out.truncate(out_len);
    out
}

/// Zones split a sound into `count` equal, back-to-back parts: zone `i` runs from `bounds[i]` to
/// `bounds[i + 1]`.
pub fn zones(len: usize, count: usize) -> Vec<usize> {
    (0..=count).map(|i| i * len / count).collect()
}

/// Moves one zone boundary, never past its neighbours, so the zones stay back to back.
pub fn move_bound(bounds: &mut [usize], i: usize, to: i64, len: usize) {
    let lo = if i == 0 { 0 } else { bounds[i - 1] };
    let hi = bounds.get(i + 1).copied().unwrap_or(len);
    bounds[i] = (to.max(0) as usize).clamp(lo, hi.max(lo));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sound(n: usize) -> Sound {
        Sound::new("S", (0..n).map(|i| [i as f32 / 10.0, -(i as f32) / 10.0]).collect::<Vec<_>>().into())
    }

    fn left(s: &Sound) -> Vec<f32> {
        s.frames.iter().map(|f| f[0] * 10.0).map(f32::round).collect()
    }

    #[test]
    fn discard_and_delete_keep_points_on_the_same_audio() {
        let mut s = sound(10);
        (s.start, s.end, s.loop_to) = (2, 8, 5);
        apply(&mut s, Op::Discard, 2..8);
        assert_eq!(left(&s), [2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!((s.start, s.end, s.loop_to), (0, 6, 3));

        let mut s = sound(10);
        (s.start, s.end, s.loop_to) = (1, 9, 7);
        apply(&mut s, Op::Delete, 3..6);
        assert_eq!(left(&s), [0.0, 1.0, 2.0, 6.0, 7.0, 8.0, 9.0]);
        assert_eq!((s.start, s.end, s.loop_to), (1, 6, 4));
        // Deleting everything is refused.
        apply(&mut s, Op::Delete, 0..7);
        assert_eq!(s.frames.len(), 7);
    }

    #[test]
    fn reverse_silence_normalize_and_loop() {
        let mut s = sound(6);
        apply(&mut s, Op::Reverse, 1..4);
        assert_eq!(left(&s), [0.0, 3.0, 2.0, 1.0, 4.0, 5.0]);
        apply(&mut s, Op::Silence, 4..6);
        assert_eq!(left(&s), [0.0, 3.0, 2.0, 1.0, 0.0, 0.0]);
        apply(&mut s, Op::Normalize, 0..6);
        assert_eq!(s.frames[1], [1.0, -1.0]);
        s.start = 2;
        apply(&mut s, Op::LoopStEnd, 2..6);
        assert_eq!((s.loop_to, s.looping), (2, true));
    }

    #[test]
    fn stretch_keeps_pitch_and_changes_length() {
        let rate = 8_000;
        // One second of 200 Hz.
        let tone: Vec<[f32; 2]> = (0..rate)
            .map(|i| (i as f32 * 200.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .map(|v| [v, v])
            .collect();
        for ratio in [0.5, 1.5] {
            let out = stretch(&tone, ratio, rate as u32);
            assert_eq!(out.len(), (rate as f64 * ratio) as usize);
            // Count rising zero crossings in the middle: still 200 per second.
            let mid = &out[out.len() / 4..out.len() * 3 / 4];
            let rises = mid.windows(2).filter(|w| w[0][0] < 0.0 && w[1][0] >= 0.0).count() as f64;
            let hz = rises / (mid.len() as f64 / rate as f64);
            assert!((hz - 200.0).abs() < 6.0, "ratio {ratio}: {hz} Hz");
            // No dropouts or overshoot from the overlapping windows.
            let peak = mid.iter().fold(0.0f32, |m, f| m.max(f[0].abs()));
            assert!(peak > 0.45 && peak < 0.55, "ratio {ratio}: peak {peak}");
        }
    }

    #[test]
    fn insert_moves_later_points_back() {
        let mut s = sound(4);
        (s.start, s.end, s.loop_to) = (0, 4, 3);
        insert(&mut s, 2, &sound(2).frames);
        assert_eq!(left(&s), [0.0, 1.0, 0.0, 1.0, 2.0, 3.0]);
        assert_eq!((s.start, s.end, s.loop_to), (0, 6, 5));
    }

    #[test]
    fn zones_stay_back_to_back() {
        let mut b = zones(100, 3);
        assert_eq!(b, [0, 33, 66, 100]);
        move_bound(&mut b, 1, 80, 100);
        assert_eq!(b, [0, 66, 66, 100]);
        move_bound(&mut b, 3, 200, 100);
        assert_eq!(b[3], 100);
        let s = section(&sound(10), 4..7, "S-2".to_owned());
        assert_eq!((left(&s), s.name.as_str()), (vec![4.0, 5.0, 6.0], "S-2"));
    }
}
