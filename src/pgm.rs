//! MPC2000XL `.PGM` programs: which sound each pad plays and its note parameters. The sounds
//! themselves are `.SND` files with the same names, next to the program.

use std::path::Path;

use crate::program::{Generation, Overlap, PADS, Pad, Velo};
use crate::snd;

/// The MPC numbers its 64 drum notes 35–98; 34 (or 0) means "none".
const FIRST_NOTE: usize = 35;
const NOTE_BYTES: usize = 25;
const MIXER_BYTES: usize = 6;
const NO_SOUND: u8 = 0xFF;

/// One drum note's settings, before its sound is loaded.
#[derive(Debug, PartialEq)]
pub struct NoteParams {
    pub sound: Option<String>,
    pub overlap: Overlap,
    /// Notes this one cuts off.
    pub mutes: [Option<usize>; 2],
    pub tune: i32,
    pub attack: u32,
    pub decay: u32,
    pub decay_from_start: bool,
    pub cutoff: u32,
    pub resonance: u32,
    pub level: u32,
    pub pan: i32,
    pub generation: Generation,
    /// Notes played along (SIMULT) or instead (VEL SW).
    pub also: [Option<usize>; 2],
    pub switch: [u8; 2],
    pub velo: Velo,
}

pub struct Pgm {
    pub name: String,
    /// Per pad: the note it plays and that note's settings.
    pub pads: Vec<Option<(usize, NoteParams)>>,
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b.split(|c| *c == 0).next().unwrap_or_default()).trim().to_owned()
}

pub fn parse(b: &[u8]) -> Result<Pgm, String> {
    let damaged = || "Damaged program file".to_owned();
    if b.len() < 4 || b[..2] != [0x07, 0x04] {
        return Err("Not an MPC2000XL program".to_owned());
    }
    let count = u16::from_le_bytes([b[2], b[3]]) as usize;
    let names_end = 4 + 17 * count;
    let name_at = names_end + 2;
    let notes_at = name_at + 17 + 9 + 1 + 5;
    let mixers_at = notes_at + 64 * NOTE_BYTES + 1;
    let pads_at = mixers_at + 64 * MIXER_BYTES + 3;
    if b.len() < pads_at + 64 {
        return Err(damaged());
    }
    let sounds: Vec<String> = (0..count).map(|i| text(&b[4 + 17 * i..4 + 17 * (i + 1)])).collect();
    let note_index = |note: u8| (note as usize).checked_sub(FIRST_NOTE).filter(|n| *n < 64);

    let pads = b[pads_at..pads_at + 64]
        .iter()
        .map(|&note| {
            let n = note_index(note)?;
            let q = &b[notes_at + n * NOTE_BYTES..notes_at + (n + 1) * NOTE_BYTES];
            let m = &b[mixers_at + n * MIXER_BYTES..mixers_at + (n + 1) * MIXER_BYTES];
            let params = NoteParams {
                sound: (q[0] != NO_SOUND).then(|| sounds.get(q[0] as usize).cloned()).flatten(),
                overlap: match q[6] {
                    1 => Overlap::Mono,
                    2 => Overlap::NoteOff,
                    _ => Overlap::Poly,
                },
                mutes: [note_index(q[7]), note_index(q[8])],
                tune: i16::from_le_bytes([q[9], q[10]]) as i32,
                attack: q[11].min(100) as u32,
                decay: q[12].min(100) as u32,
                decay_from_start: q[13] == 1,
                cutoff: q[14].min(100) as u32,
                // The MPC's resonance runs 0–15.
                resonance: (q[15].min(15) as u32 * 100).div_ceil(15),
                level: m[1].min(100) as u32,
                pan: m[2].min(100) as i32 - 50,
                generation: match q[1] {
                    1 => Generation::Simult,
                    2 => Generation::VelSw,
                    _ => Generation::Normal,
                },
                also: [note_index(q[3]), note_index(q[5])],
                switch: [q[2].min(127), q[4].min(127)],
                velo: Velo {
                    level: q[19].min(100) as u32,
                    attack: q[20].min(100) as u32,
                    start: q[21].min(100) as u32,
                    filter: q[22].min(100) as u32,
                },
            };
            Some((n, params))
        })
        .collect();
    Ok(Pgm { name: text(&b[name_at..name_at + 17]), pads })
}

pub struct Loaded {
    pub name: String,
    pub pads: Vec<Pad>,
    /// Sounds that were not found next to the program.
    pub missing: Vec<String>,
    /// Which pad plays each MPC drum note (35–98), for importing the MPC's sequences.
    pub note_pads: [Option<u8>; 64],
}

/// Loads a program and its sounds as the 64 pads.
pub fn load(path: &Path, rate: u32) -> Result<Loaded, String> {
    let pgm = parse(&std::fs::read(path).map_err(|e| e.to_string())?)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let files: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    // Sound names are matched without regard to case, as MPC disks are case-insensitive.
    let find = |name: &str| {
        files.iter().find(|p| {
            p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(name))
                && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("snd"))
        })
    };
    // Pads mute other pads by their note.
    let pad_of_note = |note: usize| pgm.pads.iter().position(|p| p.as_ref().is_some_and(|(n, _)| *n == note));

    let mut missing = Vec::new();
    let mut pads = Vec::with_capacity(PADS);
    for entry in &pgm.pads {
        let Some((_, q)) = entry else {
            pads.push(Pad::default());
            continue;
        };
        let (sound, snd_tune) = match &q.sound {
            Some(name) => match find(name).map(|p| snd::load(p, rate)) {
                Some(Ok((sound, tune))) => (Some(sound), tune),
                _ => {
                    if !missing.contains(name) {
                        missing.push(name.clone());
                    }
                    (None, 0)
                }
            },
            None => (None, 0),
        };
        pads.push(Pad {
            sound,
            // The sound's own tuning adds to the note's, as on the MPC.
            tune: (q.tune + snd_tune).clamp(-120, 120),
            attack: q.attack,
            decay: q.decay,
            decay_from_start: q.decay_from_start,
            cutoff: q.cutoff,
            resonance: q.resonance,
            overlap: q.overlap,
            mutes: q.mutes.map(|m| m.and_then(pad_of_note).map(|p| p as u8)),
            level: q.level,
            pan: q.pan,
            generation: q.generation,
            also: q.also.map(|m| m.and_then(pad_of_note).map(|p| p as u8)),
            switch: q.switch,
            velo: q.velo,
        });
    }
    let mut note_pads = [None; 64];
    for (pad, entry) in pgm.pads.iter().enumerate() {
        if let Some((note, _)) = entry
            && note_pads[*note].is_none()
        {
            note_pads[*note] = Some(pad as u8);
        }
    }
    Ok(Loaded { name: pgm.name, pads, missing, note_pads })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program with two sounds: pad 1 plays note 37 with sound 1, pad 2 note 36 with sound 0.
    fn program() -> Vec<u8> {
        let mut b = vec![0x07, 0x04, 2, 0];
        for name in [b"KICK\0\0\0\0\0\0\0\0\0\0\0\0\0", b"SNARE\0\0\0\0\0\0\0\0\0\0\0\0"] {
            b.extend(name);
        }
        b.extend([0, 0]);
        b.extend(b"MyPgm\0\0\0\0\0\0\0\0\0\0\0\0");
        b.extend([0u8; 9 + 1 + 5]);
        for n in 0..64usize {
            let mut q = [0u8; NOTE_BYTES];
            q[0] = match n {
                1 => 0,
                2 => 1,
                _ => NO_SOUND,
            };
            if n == 2 {
                (q[1], q[2], q[3]) = (2, 44, 36);
                q[6] = 2;
                q[7] = 36;
                q[8] = 34;
                q[9..11].copy_from_slice(&(-30i16).to_le_bytes());
                (q[11], q[12], q[13], q[14], q[15]) = (4, 20, 1, 60, 15);
                (q[19], q[20], q[21], q[22]) = (70, 10, 20, 30);
            }
            b.extend(q);
        }
        b.push(0);
        for n in 0..64 {
            b.extend([0, if n == 2 { 80 } else { 100 }, if n == 2 { 30 } else { 50 }, 0, 0, 0]);
        }
        b.extend([0, 0, 0]);
        let mut pads = [34i8; 64];
        (pads[0], pads[1]) = (37, 36);
        b.extend(pads.map(|p| p as u8));
        b.extend([0u8; 200]);
        b
    }

    #[test]
    fn notes_mixer_and_pad_mapping() {
        let pgm = parse(&program()).unwrap();
        assert_eq!(pgm.name, "MyPgm");
        let (note, q) = pgm.pads[0].as_ref().unwrap();
        assert_eq!(*note, 2);
        assert_eq!(
            *q,
            NoteParams {
                sound: Some("SNARE".to_owned()),
                overlap: Overlap::NoteOff,
                mutes: [Some(1), None],
                tune: -30,
                attack: 4,
                decay: 20,
                decay_from_start: true,
                cutoff: 60,
                resonance: 100,
                level: 80,
                pan: -20,
                generation: Generation::VelSw,
                also: [Some(1), None],
                switch: [44, 0],
                velo: Velo { level: 70, attack: 10, start: 20, filter: 30 },
            }
        );
        assert_eq!(pgm.pads[1].as_ref().unwrap().1.sound.as_deref(), Some("KICK"));
        assert!(pgm.pads[2].is_none());
        assert!(parse(&program()[..500]).is_err());
    }

    #[test]
    fn load_maps_mutes_to_pads_and_reports_missing_sounds() {
        let dir = std::env::temp_dir().join(format!("rsampler-pgm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("TEST.PGM"), program()).unwrap();
        // Only the snare is there, named in lower case.
        let mut snd = vec![0x01, 0x04];
        snd.extend(b"snare\0\0\0\0\0\0\0\0\0\0\0\0");
        snd.extend([100, 5, 0]);
        for v in [0u32, 2, 2, 0] {
            snd.extend(v.to_le_bytes());
        }
        snd.extend([0, 4]);
        snd.extend(44_100u16.to_le_bytes());
        snd.extend([0, 1, 0, 2]);
        std::fs::write(dir.join("snare.snd"), snd).unwrap();
        let Loaded { name, pads, missing, note_pads } = load(&dir.join("TEST.PGM"), 44_100).unwrap();
        // Note 37 is on pad 1, note 36 on pad 2.
        assert_eq!((note_pads[2], note_pads[1], note_pads[0]), (Some(0), Some(1), None));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(name, "MyPgm");
        assert_eq!(missing, ["KICK"]);
        assert_eq!(pads[0].sound.as_ref().unwrap().name, "snare");
        // Note tune −30 plus the sound's own +5.
        assert_eq!(pads[0].tune, -25);
        // Note 36 is played by pad 2.
        assert_eq!(pads[0].mutes, [Some(1), None]);
        assert!(pads[1].sound.is_none());
    }
}
