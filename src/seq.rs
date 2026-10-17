//! Sequences, tracks and note events, timed in ticks like the MPC2000XL (96 per quarter note).

pub const PPQ: u32 = 96;
/// MIDI notes, the range of a MIDI track's notes.
pub const KEYS: usize = 128;

/// A MIDI note's name, middle C (60) being C3 as on the MPC.
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[note as usize % 12], note as i32 / 12 - 2)
}
pub const SEQUENCES: usize = 99;
pub const TRACKS: usize = 64;

/// Timing correct choices: label and grid size in ticks (1 = off).
pub const TIMING: [(&str, u32); 7] =
    [("OFF", 1), ("1/8", 48), ("1/8(3)", 32), ("1/16", 24), ("1/16(3)", 16), ("1/32", 12), ("1/32(3)", 8)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Note {
    pub tick: u32,
    /// On a DRUM track the pad, 0..64 (A01–D16); on a MIDI track the MIDI note, 0..128.
    pub pad: u8,
    pub velocity: u8,
    pub duration: u32,
    /// NOTE VARIATION recorded with the note.
    pub var: crate::program::Variation,
}

#[derive(Clone, Debug)]
pub struct Track {
    /// Empty for the default "Track-NN".
    pub name: String,
    pub on: bool,
    /// 0 for MIDI (the notes only go out over MIDI), 1–4 for DRUM1–4.
    pub bus: u8,
    /// MIDI channel (0..16) the track's notes are sent on, if any.
    pub out: Option<u8>,
    /// Sorted by tick.
    pub notes: Vec<Note>,
}

impl Track {
    pub fn new() -> Self {
        Self { name: String::new(), on: true, bus: 1, out: None, notes: Vec::new() }
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
    /// Empty for the default "SequenceNN".
    pub name: String,
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
            name: String::new(),
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

/// A sequence's name as the LCD shows it: its own, or "SequenceNN" (counted from 1).
pub fn seq_name(index: usize, seq: &Sequence) -> String {
    if seq.name.is_empty() { format!("Sequence{:02}", index + 1) } else { seq.name.clone() }
}

pub fn track_name(index: usize, track: &Track) -> String {
    if track.name.is_empty() { format!("Track-{:02}", index + 1) } else { track.name.clone() }
}

/// EDIT BARS: inserts `copies` copies of bars `first..=last` (counted from 1) of `from` into `to`
/// after its bar `after` (0 = before the first), moving the later bars back. Both sequences need
/// the same time signature, since bars here have no signature of their own.
pub fn copy_bars(
    from: &Sequence,
    to: &mut Sequence,
    first: u32,
    last: u32,
    after: u32,
    copies: u32,
) -> Result<(), String> {
    if (from.beats, from.beat_unit) != (to.beats, to.beat_unit) {
        return Err("The time signatures differ".to_owned());
    }
    let (first, last) = (first.clamp(1, from.bars), last.clamp(1, from.bars));
    let (first, last) = (first.min(last), first.max(last));
    let added = (last - first + 1) * copies;
    if to.bars + added > 999 {
        return Err("Longer than 999 bars".to_owned());
    }
    let bar = to.ticks_per_bar();
    let (at, src, shift) = (after.min(to.bars) * bar, (first - 1) * bar..last * bar, added * bar);
    to.bars += added;
    for (t, f) in to.tracks.iter_mut().zip(&from.tracks) {
        for n in t.notes.iter_mut().filter(|n| n.tick >= at) {
            n.tick += shift;
        }
        for k in 0..copies {
            for n in f.range(src.start, src.end) {
                t.insert(Note { tick: at + k * src.len() as u32 + n.tick - src.start, ..*n });
            }
        }
    }
    Ok(())
}

/// What EDIT EVENTS does to the notes in its range.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventOp {
    Copy,
    Duration,
    Velocity,
}

/// How DURATION and VELOCITY change a value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueMode {
    Add,
    Sub,
    /// Percent.
    Multiply,
    Set,
}

impl ValueMode {
    pub const ALL: [ValueMode; 4] = [ValueMode::Add, ValueMode::Sub, ValueMode::Multiply, ValueMode::Set];

    pub fn label(self) -> &'static str {
        match self {
            ValueMode::Add => "ADD VALUE",
            ValueMode::Sub => "SUB VALUE",
            ValueMode::Multiply => "MULTI VAL%",
            ValueMode::Set => "SET TO VAL",
        }
    }

    pub fn apply(self, v: u32, by: u32) -> u32 {
        match self {
            ValueMode::Add => v + by,
            ValueMode::Sub => v.saturating_sub(by),
            ValueMode::Multiply => v * by / 100,
            ValueMode::Set => by,
        }
    }
}

/// Changes the velocity or duration of the notes of `track` in `range`, only `pad`'s if given.
pub fn edit_notes(
    track: &mut Track,
    range: std::ops::Range<u32>,
    pad: Option<u8>,
    op: EventOp,
    mode: ValueMode,
    by: u32,
) {
    for n in track.notes.iter_mut().filter(|n| range.contains(&n.tick) && pad.is_none_or(|p| p == n.pad)) {
        match op {
            EventOp::Velocity => n.velocity = mode.apply(n.velocity as u32, by).clamp(1, 127) as u8,
            EventOp::Duration => n.duration = mode.apply(n.duration, by).clamp(1, 9999),
            EventOp::Copy => {}
        }
    }
}

/// EDIT EVENTS COPY: puts `copies` copies of `notes` (taken from `range`) on `to` one after the
/// other from `start`. REPLACE first clears where they go (only `pad`'s notes, if given); MERGE
/// adds to what is there. Copies past the end of the sequence are dropped.
#[allow(clippy::too_many_arguments)] // The fields of the MPC's COPY screen, one to one.
pub fn copy_events(
    notes: &[Note],
    range: std::ops::Range<u32>,
    pad: Option<u8>,
    to: &mut Track,
    len: u32,
    start: u32,
    copies: u32,
    merge: bool,
) {
    let span = range.len() as u32;
    if !merge {
        to.erase(start, start + span * copies, pad);
    }
    for k in 0..copies {
        for n in notes.iter().filter(|n| range.contains(&n.tick) && pad.is_none_or(|p| p == n.pad)) {
            let tick = start + k * span + n.tick - range.start;
            if tick < len {
                to.insert(Note { tick, ..*n });
            }
        }
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
            t.insert(Note { tick, pad, velocity: 100, duration: 10, var: None });
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
        s.tracks[0].insert(Note { tick: 500, pad: 0, velocity: 100, duration: 10, var: None });
        s.set_bars(1);
        assert!(s.tracks[0].notes.is_empty());
    }

    fn note(tick: u32, pad: u8) -> Note {
        Note { tick, pad, velocity: 100, duration: 10, var: None }
    }

    #[test]
    fn copy_bars_inserts_and_moves_later_bars_back() {
        let mut seq = Sequence::default();
        let bar = seq.ticks_per_bar();
        seq.tracks[0].insert(note(0, 0));
        seq.tracks[0].insert(note(bar + 5, 1));
        let from = seq.clone();
        // Bar 1 twice after bar 1: bar 2 becomes bar 4.
        copy_bars(&from, &mut seq, 1, 1, 1, 2).unwrap();
        assert_eq!(seq.bars, 4);
        let ticks: Vec<_> = seq.tracks[0].notes.iter().map(|n| (n.tick, n.pad)).collect();
        assert_eq!(ticks, [(0, 0), (bar, 0), (2 * bar, 0), (3 * bar + 5, 1)]);
        let mut odd = Sequence::default();
        odd.set_signature(3, 4);
        assert!(copy_bars(&from, &mut odd, 1, 1, 0, 1).is_err());
        // 4 bars plus 2 × 498 is past 999.
        assert!(copy_bars(&from, &mut seq, 1, 2, 4, 498).is_err());
    }

    #[test]
    fn copy_events_replace_merge_and_edit_values() {
        let mut t = Track::new();
        t.insert(note(0, 0));
        t.insert(note(24, 1));
        t.insert(note(200, 2));
        let notes = t.notes.clone();
        // Two copies of 0..48, from 96, replacing what was there (the note at 200 is pad 2).
        copy_events(&notes, 0..48, None, &mut t, 384, 96, 2, false);
        let ticks: Vec<_> = t.notes.iter().map(|n| n.tick).collect();
        assert_eq!(ticks, [0, 24, 96, 120, 144, 168, 200]);
        // Pad 1 only, merged at 300: lands on 324 and 372; a third copy would be past the end.
        copy_events(&notes, 0..48, Some(1), &mut t, 384, 300, 3, true);
        assert_eq!(t.notes.len(), 9);
        assert_eq!(t.notes.last().unwrap().tick, 372);

        edit_notes(&mut t, 0..100, Some(0), EventOp::Velocity, ValueMode::Multiply, 50);
        // Pad 0 at 0 and 96 is halved; pad 1 at 24 is not.
        assert_eq!((t.notes[0].velocity, t.notes[1].velocity, t.notes[2].velocity), (50, 100, 50));
        edit_notes(&mut t, 0..400, None, EventOp::Velocity, ValueMode::Add, 100);
        assert_eq!(t.notes[1].velocity, 127);
        edit_notes(&mut t, 0..1, None, EventOp::Duration, ValueMode::Set, 48);
        assert_eq!((t.notes[0].duration, t.notes[1].duration), (48, 10));
    }
}
