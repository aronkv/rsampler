//! MPC2000XL `.PGM` programs, and the programs in `.APS` files (all programs, saved together):
//! which sound each pad plays and its note parameters. The sounds themselves are `.SND` files
//! with the same names, next to the program.

use std::path::Path;

use crate::program::{FilterEnv, Generation, Overlap, PADS, Pad, Velo};
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
    pub fenv: FilterEnv,
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
    let pads = pads(
        &b[pads_at..pads_at + 64],
        |n| b[notes_at + n * NOTE_BYTES..notes_at + (n + 1) * NOTE_BYTES].to_vec(),
        |n| &b[mixers_at + n * MIXER_BYTES..mixers_at + (n + 1) * MIXER_BYTES],
        &sounds,
    );
    Ok(Pgm { name: text(&b[name_at..name_at + 17]), pads })
}

fn note_index(note: u8) -> Option<usize> {
    (note as usize).checked_sub(FIRST_NOTE).filter(|n| *n < 64)
}

/// Per pad, from the pad→note table: its note and that note's settings, from its 25-byte note
/// block `q` and 6-byte mixer entry `m`.
fn pads<'a>(
    table: &[u8],
    q: impl Fn(usize) -> Vec<u8>,
    m: impl Fn(usize) -> &'a [u8],
    sounds: &[String],
) -> Vec<Option<(usize, NoteParams)>> {
    table
        .iter()
        .map(|&note| {
            let n = note_index(note)?;
            let (q, m) = (q(n), m(n));
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
                fenv: FilterEnv {
                    attack: q[16].min(100) as u32,
                    decay: q[17].min(100) as u32,
                    amount: q[18].min(100) as u32,
                },
            };
            Some((n, params))
        })
        .collect()
}

/// An `.APS` file's programs with their slot numbers, and the slot each DRUM bus plays.
pub type Aps = (Vec<(usize, Pgm)>, [usize; 4]);

/// The programs of an `.APS` file, each with its slot number, and the program slot each DRUM
/// bus plays. Its note blocks have one byte more than a `.PGM`'s, after the sound index.
pub fn parse_aps(b: &[u8]) -> Result<Aps, String> {
    const PROGRAM: usize = 2350;
    let invalid = || "Not an MPC2000XL APS file".to_owned();
    if b.len() < 4 {
        return Err(invalid());
    }
    let count = u16::from_le_bytes([b[2], b[3]]) as usize;
    let names_end = 4 + 17 * count;
    // Name, global settings and the assign table, then four drum mixers with their settings.
    let drums_at = names_end + 2 + 17 + 8 + 64 + 7;
    let first = drums_at + (384 + 9) * 4 + 15 - 6;
    let programs = b.len().saturating_sub(1689 + 17 * count) / PROGRAM;
    if programs == 0 || b.len() < first + PROGRAM {
        return Err(invalid());
    }
    let sounds: Vec<String> = (0..count).map(|i| text(&b[4 + 17 * i..4 + 17 * (i + 1)])).collect();
    let drums = [0, 1, 2, 3].map(|i| b[drums_at + i * (384 + 9 + 3) + 384 + 5] as usize);
    let programs = (0..programs)
        .map_while(|i| b.get(first + i * (PROGRAM + 4)..first + i * (PROGRAM + 4) + PROGRAM))
        .map(|p| {
            let pgm = Pgm {
                name: text(&p[6..23]),
                pads: pads(
                    &p[2090..2154],
                    |n| {
                        let q = &p[38 + n * 26..38 + (n + 1) * 26];
                        [&q[..1], &q[2..]].concat()
                    },
                    |n| &p[1703 + n * MIXER_BYTES..1703 + (n + 1) * MIXER_BYTES],
                    &sounds,
                ),
            };
            (p[0] as usize, pgm)
        })
        .collect();
    Ok((programs, drums))
}

pub struct LoadedProgram {
    pub name: String,
    pub pads: Vec<Pad>,
    /// Which pad plays each MPC drum note (35–98), for importing the MPC's sequences.
    pub note_pads: [Option<u8>; 64],
}

pub struct Loaded {
    pub programs: Vec<LoadedProgram>,
    /// From an `.APS` file: the program (index into `programs`) each DRUM bus plays.
    pub drums: Option<[usize; 4]>,
    /// Sounds that were not found next to the program.
    pub missing: Vec<String>,
}

/// Loads a `.PGM` program, or all programs of an `.APS` file, with their sounds.
pub fn load(path: &Path, rate: u32) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (pgms, drums) = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("aps")) {
        let (pgms, slots) = parse_aps(&bytes)?;
        // The drums name program slots; programs are kept in file order.
        let drums = slots.map(|slot| pgms.iter().position(|(s, _)| *s == slot).unwrap_or(0));
        (pgms.into_iter().map(|(_, p)| p).collect(), Some(drums))
    } else {
        (vec![parse(&bytes)?], None)
    };
    let dir = path.parent().unwrap_or(Path::new("."));
    let files: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    let mut missing = Vec::new();
    // A sound used by several programs is read once.
    let mut cache: Vec<(String, Option<(crate::program::Sound, i32)>)> = Vec::new();
    let mut sound = |name: &String| {
        if let Some((_, s)) = cache.iter().find(|(n, _)| n == name) {
            return s.clone();
        }
        // Sound names are matched without regard to case, as MPC disks are case-insensitive.
        let file = files.iter().find(|p| {
            p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(name))
                && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("snd"))
        });
        let loaded = file.and_then(|p| snd::load(p, rate).ok());
        if loaded.is_none() && !missing.contains(name) {
            missing.push(name.clone());
        }
        cache.push((name.clone(), loaded.clone()));
        loaded
    };
    let programs = pgms
        .into_iter()
        .map(|pgm| {
            // Pads mute other pads by their note.
            let pad_of_note = |note: usize| pgm.pads.iter().position(|p| p.as_ref().is_some_and(|(n, _)| *n == note));
            let mut pads = Vec::with_capacity(PADS);
            for entry in &pgm.pads {
                let Some((_, q)) = entry else {
                    pads.push(Pad::default());
                    continue;
                };
                let (snd, snd_tune) = match q.sound.as_ref().and_then(&mut sound) {
                    Some((s, tune)) => (Some(s), tune),
                    None => (None, 0),
                };
                pads.push(Pad {
                    sound: snd,
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
                    fenv: q.fenv,
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
            LoadedProgram { name: pgm.name, pads, note_pads }
        })
        .collect();
    Ok(Loaded { programs, drums, missing })
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
                (q[16], q[17], q[18]) = (5, 50, 40);
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
                fenv: FilterEnv { attack: 5, decay: 50, amount: 40 },
            }
        );
        assert_eq!(pgm.pads[1].as_ref().unwrap().1.sound.as_deref(), Some("KICK"));
        assert!(pgm.pads[2].is_none());
        assert!(parse(&program()[..500]).is_err());
    }

    #[test]
    fn aps_programs() {
        // One sound and one program; the program starts after the name, settings, assign table
        // and drum mixers.
        let mut b = vec![0u8; 4060];
        b[..4].copy_from_slice(&[0x0A, 0x05, 1, 0]);
        b[4..9].copy_from_slice(b"OBOE ");
        let p = 4 + 17 + 2 + 17 + 8 + 64 + 7 + (384 + 9) * 4 + 15 - 6;
        b[p + 6..p + 10].copy_from_slice(b"PROG");
        b[p + 2090..p + 2154].fill(34);
        // Pad 2 plays note 36, whose block (26 bytes, with the extra second byte) has sound 0,
        // tune −20 and POLY; its mixer level is 77.
        b[p + 2091] = 36;
        let q = p + 38 + 26;
        (b[q], b[q + 1], b[q + 7]) = (0, 0xEE, 0);
        b[q + 10..q + 12].copy_from_slice(&(-20i16).to_le_bytes());
        b[p + 1703 + 6 + 1] = 77;
        let (pgms, drums) = parse_aps(&b).unwrap();
        assert_eq!((pgms.len(), drums), (1, [0; 4]));
        let pgm = &pgms[0].1;
        assert_eq!(pgm.name, "PROG");
        assert!(pgm.pads[0].is_none());
        let (note, q) = pgm.pads[1].as_ref().unwrap();
        assert_eq!((*note, q.sound.as_deref(), q.tune, q.level), (1, Some("OBOE"), -20, 77));
        assert!(parse_aps(&b[..3000]).is_err());
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
        let Loaded { mut programs, missing, .. } = load(&dir.join("TEST.PGM"), 44_100).unwrap();
        let LoadedProgram { name, pads, note_pads } = programs.remove(0);
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
