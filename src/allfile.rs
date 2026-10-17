//! MPC2000XL `.ALL` files: every sequence and the songs, saved together from the MPC's SAVE
//! screen. Only what rsampler has a place for is read: tempo, length, time signature, loop,
//! track on/off, notes (with their NOTE VARIATION) and the first song.

use crate::program::VarParam;
use crate::seq::{Note, SEQUENCES, Sequence};

const MAGIC: &[u8] = b"MPC2KXL ALL 1.00";
const NAMES_AT: usize = 2064;
const NAME_ENTRY: usize = 18;
const SONGS_AT: usize = 3846;
const SONG_LEN: usize = 528;
const SEQS_AT: usize = 14406;
/// A sequence is this long without events (two terminators included).
const EMPTY_SEQ: usize = 10256;
const EVENTS_AT: usize = 10240;
const TRACK_STATUS_AT: usize = 384 + 1280;
const BARS_AT: usize = 5379;
const TERMINATOR: [u8; 8] = [0xFF; 8];

pub struct AllNote {
    pub track: usize,
    pub tick: u32,
    /// The MPC drum note, 35–98.
    pub note: u8,
    pub velocity: u8,
    pub duration: u32,
    pub var: (VarParam, u8),
}

pub struct AllSeq {
    pub index: usize,
    pub name: String,
    pub track_names: Vec<String>,
    pub tempo: f32,
    pub bars: u32,
    pub signature: (u32, u32),
    pub looping: bool,
    pub on: [bool; 64],
    /// Per track: 0 MIDI, 1–4 DRUM1–4.
    pub buses: [u8; 64],
    pub notes: Vec<AllNote>,
}

pub struct All {
    pub seqs: Vec<AllSeq>,
    /// The first song in use: steps of sequence index and repeats, and whether it loops.
    pub song: Option<(Vec<(usize, u32)>, bool)>,
}

pub fn parse(b: &[u8]) -> Result<All, String> {
    let damaged = || "Damaged ALL file".to_owned();
    if b.len() < SEQS_AT || &b[..16] != MAGIC {
        return Err("Not an MPC2000XL ALL file".to_owned());
    }
    let u16_at = |s: &[u8], i: usize| u16::from_le_bytes([s[i], s[i + 1]]);

    let song = (0..20).find_map(|i| {
        let s = &b[SONGS_AT + i * SONG_LEN..SONGS_AT + (i + 1) * SONG_LEN];
        if s[..8] == *b"(Unused)" {
            return None;
        }
        let steps: Vec<(usize, u32)> = s[16..516]
            .chunks(2)
            .filter(|c| c[0] != 0xFF && c[1] != 0xFF && (c[0] as usize) < SEQUENCES)
            .map(|c| (c[0] as usize, c[1].max(1) as u32))
            .collect();
        (!steps.is_empty()).then_some((steps, s[521] == 1))
    });

    let mut seqs = Vec::new();
    let mut pos = SEQS_AT;
    for index in 0..SEQUENCES {
        let at = NAMES_AT + index * NAME_ENTRY + 16;
        if b[at] == 0 && b[at + 1] == 0 {
            continue;
        }
        let s = b.get(pos..).filter(|s| s.len() >= EVENTS_AT).ok_or_else(damaged)?;
        let (mut off, mut segments, mut notes) = (EVENTS_AT, 0, Vec::new());
        loop {
            let e = s.get(off..off + 8).ok_or_else(damaged)?;
            if e == TERMINATOR {
                break;
            }
            // System exclusive events run over several segments, up to one marked 0xF8.
            let count = if e[4] == 0xF0 {
                (0..256).find(|k| s.get(off + k * 8 + 4) == Some(&0xF8)).ok_or_else(damaged)? + 1
            } else {
                1
            };
            if e[4] < 0x80 {
                let var = match (e[6] >> 7, e[7] >> 7) {
                    (1, 1) => VarParam::Filter,
                    (1, 0) => VarParam::Attack,
                    (0, 1) => VarParam::Decay,
                    _ => VarParam::Tune,
                };
                notes.push(AllNote {
                    track: (e[3] & 0x3F) as usize,
                    tick: u32::from_le_bytes([e[0], e[1], e[2] & 0x0F, 0]),
                    note: e[4],
                    velocity: e[6] & 0x7F,
                    duration: ((e[2] as u32 & 0xF0) << 6) | ((e[3] as u32 & 0xC0) << 2) | e[5] as u32,
                    var: (var, e[7] & 0x7F),
                });
            }
            off += count * 8;
            segments += count;
        }
        // Bar 1's beat length and end give the time signature.
        let tpb = s[BARS_AT] as u32;
        let first_bar = u32::from_le_bytes([s[BARS_AT + 1], s[BARS_AT + 2], s[BARS_AT + 3], 0]);
        let signature = match tpb {
            192 | 96 | 48 | 24 if first_bar > 0 => ((first_bar / tpb).clamp(1, 32), 384 / tpb),
            _ => (4, 4),
        };
        let buses: [u8; 64] = std::array::from_fn(|t| s[384 + 1088 + t].min(4));
        let mut on = [true; 64];
        for (t, on) in on.iter_mut().enumerate() {
            *on = s[TRACK_STATUS_AT + t] >> 1 & 1 == 1;
        }
        let text =
            |b: &[u8]| String::from_utf8_lossy(b.split(|c| *c == 0).next().unwrap_or_default()).trim().to_owned();
        seqs.push(AllSeq {
            index,
            name: text(&b[at - 16..at]),
            track_names: (0..64).map(|t| text(&s[384 + t * 16..384 + (t + 1) * 16])).collect(),
            tempo: u16_at(s, 22) as f32 / 10.0,
            bars: (u16_at(s, 26) as u32).clamp(1, 999),
            signature,
            looping: s[52] > 0,
            on,
            buses,
            notes,
        });
        // An even number of segments is followed by two terminators, an odd one by one.
        pos += EMPTY_SEQ + segments * 8 - if segments % 2 == 1 { 8 } else { 0 };
    }
    Ok(All { seqs, song })
}

impl AllSeq {
    /// The sequence with each MPC drum note on the pad `pad_of(track, note)` gives; notes
    /// without a pad are dropped and counted.
    pub fn to_sequence(&self, pad_of: impl Fn(usize, u8) -> Option<u8>) -> (Sequence, usize) {
        let mut seq = Sequence { tempo: self.tempo.clamp(30.0, 300.0), looping: self.looping, ..Sequence::default() };
        // Names the MPC gives by default stay empty, so they follow the sequence's number.
        if self.name != format!("Sequence{:02}", self.index + 1) {
            seq.name = self.name.chars().take(16).collect();
        }
        for (i, (t, name)) in seq.tracks.iter_mut().zip(&self.track_names).enumerate() {
            if *name != format!("Track-{:02}", i + 1) {
                t.name = name.chars().take(16).collect();
            }
        }
        seq.set_signature(self.signature.0, self.signature.1);
        seq.set_bars(self.bars);
        for ((t, on), bus) in seq.tracks.iter_mut().zip(self.on).zip(self.buses) {
            (t.on, t.bus) = (on, bus);
        }
        let (len, mut dropped) = (seq.len(), 0);
        for n in &self.notes {
            let Some(pad) = pad_of(n.track, n.note).filter(|_| n.tick < len) else {
                dropped += 1;
                continue;
            };
            // The MPC keeps DECAY, ATTACK and FILTER as 0–100; rsampler's slider runs 0–127.
            let var = match n.var {
                (VarParam::Tune, 64) => None,
                (VarParam::Tune, v) => Some((VarParam::Tune, v)),
                (p, v) => Some((p, (v.min(100) as u32 * 127 / 100) as u8)),
            };
            let note = Note { tick: n.tick, pad, velocity: n.velocity.max(1), duration: n.duration.max(1), var };
            seq.tracks[n.track].insert(note);
        }
        (seq, dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An ALL file with sequence 3 holding two notes on track 2 and a song of sequence 3 twice.
    fn file() -> Vec<u8> {
        let mut b = vec![0u8; SEQS_AT];
        b[..16].copy_from_slice(MAGIC);
        for i in 0..20 {
            b[SONGS_AT + i * SONG_LEN..][..8].copy_from_slice(b"(Unused)");
        }
        let song = SONGS_AT + 4 * SONG_LEN;
        b[song..song + 8].copy_from_slice(b"Song05  ");
        b[song + 16..song + 516].fill(0xFF);
        (b[song + 16], b[song + 17], b[song + 521]) = (2, 2, 1);
        b[NAMES_AT + 2 * NAME_ENTRY + 16] = 1;

        let mut s = vec![0u8; EVENTS_AT];
        s[22..24].copy_from_slice(&935u16.to_le_bytes());
        s[26..28].copy_from_slice(&3u16.to_le_bytes());
        s[52] = 1;
        // 3/4: 96 ticks a beat, the first bar ending at 288.
        s[BARS_AT] = 96;
        s[BARS_AT + 1..BARS_AT + 3].copy_from_slice(&288u16.to_le_bytes());
        for t in 0..64 {
            s[TRACK_STATUS_AT + t] = if t == 5 { 1 } else { 3 };
            s[384 + 1088 + t] = 1;
        }
        s[384 + 1088 + 7] = 0;
        // Note 37 at tick 70000 (needs the third tick byte), held 1000 ticks, velocity 90,
        // ATTACK variation 50.
        let (tick, dur) = (70_000u32, 1000u32);
        let t = tick.to_le_bytes();
        s.extend([
            t[0],
            t[1],
            t[2] | ((dur >> 6) as u8 & 0xF0),
            2 | ((dur >> 2) as u8 & 0xC0),
            37,
            dur as u8,
            0x80 | 90,
            50,
        ]);
        // A two-segment system exclusive event, then note 36 at tick 0.
        s.extend([0, 0, 0, 2, 0xF0, 1, 2, 3, 0, 0, 0, 2, 0xF8, 0, 0, 0]);
        s.extend([0, 0, 0, 2, 36, 10, 100, 64]);
        // Four segments: an even count, so two terminators.
        s.extend(TERMINATOR);
        s.extend(TERMINATOR);
        b.extend(&s);
        b
    }

    #[test]
    fn reads_sequences_notes_and_the_song() {
        let all = parse(&file()).unwrap();
        assert_eq!(all.song, Some((vec![(2, 2)], true)));
        let [seq] = &all.seqs[..] else { panic!("one sequence") };
        assert_eq!((seq.index, seq.tempo, seq.bars, seq.signature, seq.looping), (2, 93.5, 3, (3, 4), true));
        assert!(!seq.on[5] && seq.on[0]);
        assert_eq!((seq.buses[0], seq.buses[7]), (1, 0));
        let n = &seq.notes[0];
        assert_eq!((n.track, n.tick, n.note, n.velocity, n.duration), (2, 70_000, 37, 90, 1000));
        assert_eq!(n.var, (VarParam::Attack, 50));
        assert_eq!(seq.notes.len(), 2);

        // 3 bars of 3/4 end at 864, so the note at 70000 is dropped; note 36 goes to pad 1.
        let (s, dropped) = seq.to_sequence(|_, n| n.checked_sub(35));
        assert_eq!(dropped, 1);
        assert_eq!(s.tracks[2].notes[0].pad, 1);
        assert_eq!((s.tempo, s.beats, s.beat_unit, s.bars), (93.5, 3, 4, 3));
        assert!(parse(&file()[..SEQS_AT + 100]).is_err());
        assert!(parse(b"MPC2KXL ALL 1.0").is_err());
    }
}
