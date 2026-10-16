//! Sequences, tracks and note events, timed in ticks like the MPC2000XL (96 per quarter note).

pub const PPQ: u32 = 96;
pub const SEQUENCES: usize = 99;
pub const TRACKS: usize = 64;

/// Timing correct choices: label and grid size in ticks (1 = off).
pub const TIMING: [(&str, u32); 7] =
    [("OFF", 1), ("1/8", 48), ("1/8(3)", 32), ("1/16", 24), ("1/16(3)", 16), ("1/32", 12), ("1/32(3)", 8)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Note {
    pub tick: u32,
    /// Program note, 0..64 (pads A01–D16).
    pub pad: u8,
    pub velocity: u8,
    pub duration: u32,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub on: bool,
    /// Sorted by tick.
    pub notes: Vec<Note>,
}

impl Track {
    fn new() -> Self {
        Self { on: true, notes: Vec::new() }
    }

    pub fn insert(&mut self, note: Note) {
        let at = self.notes.partition_point(|n| n.tick <= note.tick);
        self.notes.insert(at, note);
    }

    /// Removes notes with `from <= tick < to`, only those of `pad` if given.
    pub fn erase(&mut self, from: u32, to: u32, pad: Option<u8>) {
        self.notes.retain(|n| n.tick < from || n.tick >= to || pad.is_some_and(|p| p != n.pad));
    }

    /// Notes with `from <= tick < to`.
    pub fn range(&self, from: u32, to: u32) -> &[Note] {
        let start = self.notes.partition_point(|n| n.tick < from);
        let end = self.notes.partition_point(|n| n.tick < to);
        &self.notes[start..end.max(start)]
    }
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub tempo: f32,
    pub bars: u32,
    pub beats: u32,
    /// Time signature denominator: 4 means quarter-note beats.
    pub beat_unit: u32,
    pub looping: bool,
    pub tracks: Vec<Track>,
}

impl Default for Sequence {
    fn default() -> Self {
        Self {
            tempo: 120.0,
            bars: 2,
            beats: 4,
            beat_unit: 4,
            looping: true,
            tracks: (0..TRACKS).map(|_| Track::new()).collect(),
        }
    }
}

impl Sequence {
    pub fn ticks_per_beat(&self) -> u32 {
        PPQ * 4 / self.beat_unit
    }

    pub fn ticks_per_bar(&self) -> u32 {
        self.ticks_per_beat() * self.beats
    }

    pub fn len(&self) -> u32 {
        self.ticks_per_bar() * self.bars
    }

    /// Changes the bar count, dropping notes past the new end.
    pub fn set_bars(&mut self, bars: u32) {
        self.bars = bars.clamp(1, 999);
        let len = self.len();
        for t in &mut self.tracks {
            t.notes.retain(|n| n.tick < len);
        }
    }

    /// Changes the time signature, keeping the bar count and dropping notes past the new end.
    pub fn set_signature(&mut self, beats: u32, beat_unit: u32) {
        self.beats = beats.clamp(1, 32);
        self.beat_unit = beat_unit;
        self.set_bars(self.bars);
    }

    /// Position as the MPC shows it: bar.beat.tick, counted from 1.
    pub fn bbt(&self, tick: u32) -> String {
        let (bar, rest) = (tick / self.ticks_per_bar(), tick % self.ticks_per_bar());
        format!("{:03}.{:02}.{:02}", bar + 1, rest / self.ticks_per_beat() + 1, rest % self.ticks_per_beat())
    }
}

/// Snaps `tick` to the nearest grid line; `swing` (50–75 %) delays every second grid line, which
/// the MPC applies to 1/8 and 1/16 only. The result wraps at `len`.
pub fn quantize(tick: u32, grid: u32, swing: u32, len: u32) -> u32 {
    let step = ((tick + grid / 2) / grid) * grid;
    let swung = if (grid == 48 || grid == 24) && (step / grid) % 2 == 1 {
        step + (swing.saturating_sub(50) * 2 * grid + 50) / 100
    } else {
        step
    };
    swung % len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbt_counts_from_one() {
        let s = Sequence::default();
        assert_eq!(s.bbt(0), "001.01.00");
        assert_eq!(s.bbt(PPQ + 5), "001.02.05");
        assert_eq!(s.bbt(4 * PPQ), "002.01.00");
        assert_eq!(s.len(), 8 * PPQ);
    }

    #[test]
    fn quantize_snaps_swings_and_wraps() {
        assert_eq!(quantize(30, 24, 50, 768), 24);
        assert_eq!(quantize(37, 24, 50, 768), 48);
        assert_eq!(quantize(13, 1, 50, 768), 13);
        // 66 % swing on 1/16: the off-beat sixteenth moves from 24 to 32 ticks (a triplet feel).
        assert_eq!(quantize(25, 24, 66, 768), 32);
        assert_eq!(quantize(47, 24, 66, 768), 48);
        // Swing only applies to 1/8 and 1/16.
        assert_eq!(quantize(17, 16, 66, 768), 16);
        // A note just before the loop end lands on the first beat.
        assert_eq!(quantize(765, 24, 50, 768), 0);
    }

    #[test]
    fn track_insert_erase_range() {
        let mut t = Track::new();
        for (tick, pad) in [(48, 1), (0, 0), (24, 2), (48, 0)] {
            t.insert(Note { tick, pad, velocity: 100, duration: 10 });
        }
        assert_eq!(t.notes.iter().map(|n| n.tick).collect::<Vec<_>>(), [0, 24, 48, 48]);
        assert_eq!(t.range(24, 48).len(), 1);
        assert_eq!(t.range(48, 49).len(), 2);
        t.erase(0, 49, Some(0));
        assert_eq!(t.notes.iter().map(|n| n.pad).collect::<Vec<_>>(), [2, 1]);
        t.erase(0, 768, None);
        assert!(t.notes.is_empty());
    }

    #[test]
    fn shrinking_drops_notes_past_the_end() {
        let mut s = Sequence::default();
        s.tracks[0].insert(Note { tick: 500, pad: 0, velocity: 100, duration: 10 });
        s.set_bars(1);
        assert!(s.tracks[0].notes.is_empty());
    }
}
