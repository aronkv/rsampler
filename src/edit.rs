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
    Delete,
    Silence,
    Reverse,
    Normalize,
    /// Every zone to its own pad (ZONE only).
    Slice,
}

impl Op {
    pub const ALL: [Op; 8] =
        [Op::Discard, Op::LoopStEnd, Op::NewSound, Op::Delete, Op::Silence, Op::Reverse, Op::Normalize, Op::Slice];

    pub fn label(self) -> &'static str {
        match self {
            Op::Discard => "DISCARD",
            Op::LoopStEnd => "LOOP FROM ST TO END",
            Op::NewSound => "SECTION -> NEW SOUND",
            Op::Delete => "DELETE SECTION",
            Op::Silence => "SILENCE SECTION",
            Op::Reverse => "REVERSE SECTION",
            Op::Normalize => "NORMALIZE SECTION",
            Op::Slice => "SLICE SOUND",
        }
    }

    /// Whether the operation puts sounds on other pads, chosen with `To pad`.
    pub fn makes_sounds(self) -> bool {
        matches!(self, Op::NewSound | Op::Slice)
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
        Op::NewSound | Op::Slice => return,
    }
    let len = frames.len().max(1);
    snd.end = snd.end.clamp(1, len);
    snd.start = snd.start.min(snd.end - 1);
    snd.loop_to = snd.loop_to.min(snd.end - 1);
    snd.frames = frames.into();
}

/// A section of a sound as a sound of its own.
pub fn section(snd: &Sound, section: Range<usize>, name: String) -> Sound {
    let end = section.end.min(snd.frames.len());
    let start = section.start.min(end);
    Sound::new(name, snd.frames[start..end].to_vec().into())
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
