//! Standard MIDI files: importing a sequence from one (the MPC2000XL saves its sequences as
//! `.MID`) and exporting one for a DAW.

use crate::seq::{Note, PPQ, Sequence};

pub struct MidiNote {
    /// In the file's own ticks.
    pub tick: u32,
    pub note: u8,
    pub velocity: u8,
    pub duration: u32,
}

pub struct Smf {
    pub ppq: u32,
    pub tempo: Option<f32>,
    pub signature: Option<(u32, u32)>,
    /// End of the longest track, in the file's ticks.
    pub length: u32,
    /// Notes per track, tracks without notes left out.
    pub tracks: Vec<Vec<MidiNote>>,
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let c = *self.b.get(self.i).ok_or("MIDI file ends early")?;
        self.i += 1;
        Ok(c)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or("MIDI file ends early")?;
        self.i += n;
        Ok(s)
    }

    /// Variable-length quantity, at most four bytes.
    fn vlq(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let c = self.byte()?;
            v = (v << 7) | (c & 0x7F) as u32;
            if c < 0x80 {
                return Ok(v);
            }
        }
        Err("Damaged MIDI file".to_owned())
    }
}

pub fn parse(b: &[u8]) -> Result<Smf, String> {
    if b.len() < 14 || &b[..4] != b"MThd" {
        return Err("Not a MIDI file".to_owned());
    }
    let tracks = u16::from_be_bytes([b[10], b[11]]);
    let division = u16::from_be_bytes([b[12], b[13]]);
    if division == 0 || division & 0x8000 != 0 {
        return Err("SMPTE-timed MIDI files are not supported".to_owned());
    }
    let header_len = u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let mut r = Reader { b, i: 8 + header_len };
    let mut smf = Smf { ppq: division as u32, tempo: None, signature: None, length: 0, tracks: Vec::new() };

    for _ in 0..tracks {
        if r.take(4)? != b"MTrk" {
            return Err("Damaged MIDI file".to_owned());
        }
        let len = u32::from_be_bytes(r.take(4)?.try_into().unwrap_or_default()) as usize;
        let end = r.i + len;
        let (mut tick, mut status) = (0u32, 0u8);
        let mut notes: Vec<MidiNote> = Vec::new();
        // Notes waiting for their note off: index into `notes`, by key and channel.
        let mut open: Vec<(u8, u8, usize)> = Vec::new();
        while r.i < end {
            tick = tick.saturating_add(r.vlq()?);
            let mut c = r.byte()?;
            match c {
                0xFF => {
                    let kind = r.byte()?;
                    let len = r.vlq()? as usize;
                    let data = r.take(len)?;
                    match (kind, data) {
                        (0x51, [a, b, c]) if smf.tempo.is_none() => {
                            let us = u32::from_be_bytes([0, *a, *b, *c]).max(1);
                            smf.tempo = Some(60_000_000.0 / us as f32);
                        }
                        (0x58, [n, d, ..]) if smf.signature.is_none() && *d < 6 => {
                            smf.signature = Some((*n as u32, 1 << d));
                        }
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let len = r.vlq()? as usize;
                    r.take(len)?;
                }
                _ => {
                    // Running status: a data byte reuses the last status.
                    if c < 0x80 {
                        r.i -= 1;
                        c = status;
                    } else {
                        status = c;
                    }
                    let channel = c & 0x0F;
                    match c & 0xF0 {
                        0xC0 | 0xD0 => {
                            r.byte()?;
                        }
                        0x80 | 0x90 => {
                            let (key, velocity) = (r.byte()?, r.byte()?);
                            if let Some(at) = open.iter().position(|(k, ch, _)| *k == key && *ch == channel) {
                                let (_, _, i) = open.remove(at);
                                notes[i].duration = (tick - notes[i].tick).max(1);
                            }
                            if c & 0xF0 == 0x90 && velocity > 0 {
                                open.push((key, channel, notes.len()));
                                notes.push(MidiNote { tick, note: key, velocity, duration: 1 });
                            }
                        }
                        0xA0 | 0xB0 | 0xE0 => {
                            r.take(2)?;
                        }
                        _ => return Err("Damaged MIDI file".to_owned()),
                    }
                }
            }
        }
        r.i = end;
        smf.length = smf.length.max(tick);
        if !notes.is_empty() {
            smf.tracks.push(notes);
        }
    }
    Ok(smf)
}

/// Builds a sequence from a MIDI file. `pad_of` maps a MIDI note to a pad; notes without a pad are
/// dropped and counted.
pub fn to_sequence(smf: &Smf, pad_of: impl Fn(u8) -> Option<u8>) -> (Sequence, usize) {
    let mut seq = Sequence::default();
    if let Some(tempo) = smf.tempo {
        seq.tempo = (tempo.clamp(30.0, 300.0) * 10.0).round() / 10.0;
    }
    if let Some((beats, unit)) = smf.signature.filter(|(_, u)| [2, 4, 8, 16].contains(u)) {
        seq.set_signature(beats, unit);
    }
    let scale = |t: u32| (t as u64 * PPQ as u64 / smf.ppq as u64) as u32;
    let last = smf.tracks.iter().flatten().map(|n| scale(n.tick) + 1).max().unwrap_or(0);
    let length = scale(smf.length).max(last);
    seq.set_bars(length.div_ceil(seq.ticks_per_bar()).max(1));
    let mut dropped = 0;
    for (track, notes) in seq.tracks.iter_mut().zip(&smf.tracks) {
        for n in notes {
            match pad_of(n.note) {
                Some(pad) => track.insert(Note {
                    tick: scale(n.tick),
                    pad,
                    velocity: n.velocity.min(127),
                    duration: scale(n.duration).max(1),
                    var: None,
                }),
                None => dropped += 1,
            }
        }
    }
    (seq, dropped)
}

/// Writes a sequence as a format 1 MIDI file at 96 ticks per quarter note: a tempo track, then
/// one track per used track. Pads play notes 36–99, as on rsampler's MIDI output.
pub fn from_sequence(seq: &Sequence) -> Vec<u8> {
    fn vlq(out: &mut Vec<u8>, mut v: u32) {
        let mut bytes = vec![(v & 0x7F) as u8];
        v >>= 7;
        while v > 0 {
            bytes.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
        out.extend(bytes.iter().rev());
    }
    fn chunk(out: &mut Vec<u8>, events: Vec<(u32, Vec<u8>)>, end: u32) {
        let mut data = Vec::new();
        let mut now = 0;
        for (tick, bytes) in events {
            vlq(&mut data, tick - now);
            data.extend(bytes);
            now = tick;
        }
        vlq(&mut data, end.saturating_sub(now));
        data.extend([0xFF, 0x2F, 0x00]);
        out.extend(b"MTrk");
        out.extend((data.len() as u32).to_be_bytes());
        out.extend(data);
    }

    let used: Vec<_> = seq.tracks.iter().filter(|t| !t.notes.is_empty()).collect();
    let mut out = Vec::new();
    out.extend(b"MThd");
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend((used.len() as u16 + 1).to_be_bytes());
    out.extend((PPQ as u16).to_be_bytes());

    let us = (60_000_000.0 / seq.tempo).round() as u32;
    let denominator = seq.beat_unit.trailing_zeros() as u8;
    let tempo = vec![
        (0, [&[0xFF, 0x51, 0x03][..], &us.to_be_bytes()[1..]].concat()),
        (0, vec![0xFF, 0x58, 0x04, seq.beats as u8, denominator, 24, 8]),
    ];
    let len = seq.len();
    chunk(&mut out, tempo, len);
    for t in used {
        let ch = t.out.unwrap_or(0);
        let mut events: Vec<(u32, Vec<u8>)> = Vec::new();
        for n in &t.notes {
            let key = n.pad + crate::midi::FIRST_NOTE;
            events.push((n.tick, vec![0x90 | ch, key, n.velocity.max(1)]));
            events.push(((n.tick + n.duration).min(len), vec![0x80 | ch, key, 0]));
        }
        // Note offs sort before note ons at the same tick, so repeated notes do not overlap.
        events.sort_by_key(|(tick, bytes)| (*tick, bytes[0] & 0xF0 == 0x90));
        chunk(&mut out, events, len);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_then_import_round_trip() {
        let mut seq = Sequence { tempo: 93.0, ..Sequence::default() };
        seq.set_signature(3, 4);
        seq.set_bars(3);
        seq.tracks[0].insert(Note { tick: 0, pad: 0, velocity: 100, duration: 48, var: None });
        seq.tracks[0].insert(Note { tick: 300, pad: 2, velocity: 64, duration: 10, var: None });
        seq.tracks[4].insert(Note { tick: 860, pad: 63, velocity: 127, duration: 4, var: None });
        let bytes = from_sequence(&seq);
        let smf = parse(&bytes).unwrap();
        assert_eq!((smf.ppq, smf.signature, smf.tracks.len()), (96, Some((3, 4)), 2));
        let (back, dropped) = to_sequence(&smf, |n| n.checked_sub(36));
        assert_eq!(dropped, 0);
        assert_eq!((back.tempo, back.beats, back.bars), (93.0, 3, 3));
        assert_eq!(back.tracks[0].notes, seq.tracks[0].notes);
        // Used tracks are packed together on import.
        assert_eq!(back.tracks[1].notes, seq.tracks[4].notes);
    }

    #[test]
    fn running_status_other_resolution_and_damage() {
        // 480 PPQ, running status for the second note on and both note offs (velocity 0).
        let track: Vec<u8> =
            vec![0x00, 0x90, 36, 100, 0x83, 0x60, 38, 90, 0x81, 0x70, 36, 0, 0x00, 38, 0, 0x00, 0xFF, 0x2F, 0x00];
        let mut b = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0".to_vec();
        b.extend(b"MTrk");
        b.extend((track.len() as u32).to_be_bytes());
        b.extend(&track);
        let smf = parse(&b).unwrap();
        let (seq, dropped) = to_sequence(&smf, |n| (n == 36).then_some(0));
        assert_eq!(dropped, 1);
        // 480 ticks in → 96; held 720 ticks → 144.
        assert_eq!(seq.tracks[0].notes[0].tick, 0);
        assert_eq!(seq.tracks[0].notes[0].duration, 144);
        assert!(parse(&b[..b.len() - 5]).is_err());
        assert!(parse(b"RIFF").is_err());
    }
}
