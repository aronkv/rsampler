//! Saving and loading a whole project: a folder with `rsampler.json` and one WAV per pad sound.

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio::{Shared, Song, load_wav};
use crate::program::{Frames, Overlap, PADS, Pad, Sound, VarParam, pad_name};
use crate::seq::{Note, SEQUENCES, Sequence, TIMING, TRACKS};

pub const FILE: &str = "rsampler.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    master: f32,
    timing: usize,
    swing: u32,
    count_in: bool,
    click: bool,
    /// Only pads that differ from the default.
    pads: Vec<PadFile>,
    /// Only sequences that differ from the default.
    sequences: Vec<SeqFile>,
    #[serde(default)]
    song: SongFile,
    #[serde(default)]
    setup: SetupFile,
}

#[derive(Serialize, Deserialize)]
struct SetupFile {
    clock_out: bool,
    key_velocity: u8,
    #[serde(default)]
    sync: bool,
}

impl Default for SetupFile {
    fn default() -> Self {
        Self { clock_out: false, key_velocity: 101, sync: false }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct SongFile {
    /// Sequence index and repeats.
    steps: Vec<[u32; 2]>,
    looping: bool,
}

#[derive(Serialize, Deserialize)]
struct PadFile {
    pad: usize,
    sound: Option<SoundFile>,
    tune: i32,
    attack: u32,
    decay: u32,
    decay_from_start: bool,
    cutoff: u32,
    resonance: u32,
    overlap: Overlap,
    mutes: [Option<u8>; 2],
    level: u32,
    pan: i32,
}

#[derive(Serialize, Deserialize)]
struct SoundFile {
    name: String,
    /// Relative to the project folder.
    file: String,
    /// Length when saved, to rescale the trim points if the sample rate changed since.
    frames: usize,
    start: usize,
    end: usize,
}

#[derive(Serialize, Deserialize)]
struct SeqFile {
    index: usize,
    tempo: f32,
    bars: u32,
    beats: u32,
    beat_unit: u32,
    looping: bool,
    tracks: Vec<TrackFile>,
}

#[derive(Serialize, Deserialize)]
struct TrackFile {
    index: usize,
    on: bool,
    #[serde(default)]
    out: Option<u8>,
    /// tick, pad, velocity, duration, and for notes with a NOTE VARIATION its parameter
    /// (0 TUNE, 1 DECAY, 2 ATTACK, 3 FILTER) and value.
    notes: Vec<Vec<u32>>,
}

pub fn is_project(dir: &Path) -> bool {
    dir.join(FILE).is_file()
}

/// The project as it is written to `rsampler.json`.
fn describe(s: &Shared) -> ProjectFile {
    let default = Pad::default();
    let mut pads = Vec::new();
    for (i, p) in s.pads.iter().enumerate() {
        let sound = p.sound.as_ref().map(|snd| SoundFile {
            name: snd.name.clone(),
            file: sound_file(i),
            frames: snd.frames.len(),
            start: snd.start,
            end: snd.end,
        });
        let changed = sound.is_some()
            || (
                p.tune,
                p.attack,
                p.decay,
                p.decay_from_start,
                p.cutoff,
                p.resonance,
                p.overlap,
                p.mutes,
                p.level,
                p.pan,
            ) != (
                default.tune,
                default.attack,
                default.decay,
                default.decay_from_start,
                default.cutoff,
                default.resonance,
                default.overlap,
                default.mutes,
                default.level,
                default.pan,
            );
        if changed {
            pads.push(PadFile {
                pad: i,
                sound,
                tune: p.tune,
                attack: p.attack,
                decay: p.decay,
                decay_from_start: p.decay_from_start,
                cutoff: p.cutoff,
                resonance: p.resonance,
                overlap: p.overlap,
                mutes: p.mutes,
                level: p.level,
                pan: p.pan,
            });
        }
    }

    let fresh = Sequence::default();
    let sequences = s
        .seqs
        .iter()
        .enumerate()
        .filter_map(|(index, q)| {
            let tracks: Vec<TrackFile> = q
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| !t.on || t.out.is_some() || !t.notes.is_empty())
                .map(|(index, t)| TrackFile {
                    index,
                    on: t.on,
                    out: t.out,
                    notes: t
                        .notes
                        .iter()
                        .map(|n| {
                            let mut v = vec![n.tick, n.pad as u32, n.velocity as u32, n.duration];
                            if let Some((param, value)) = n.var {
                                let index = VarParam::ALL.iter().position(|p| *p == param).unwrap_or(0);
                                v.extend([index as u32, value as u32]);
                            }
                            v
                        })
                        .collect(),
                })
                .collect();
            let changed = !tracks.is_empty()
                || (q.tempo, q.bars, q.beats, q.beat_unit, q.looping)
                    != (fresh.tempo, fresh.bars, fresh.beats, fresh.beat_unit, fresh.looping);
            changed.then_some(SeqFile {
                index,
                tempo: q.tempo,
                bars: q.bars,
                beats: q.beats,
                beat_unit: q.beat_unit,
                looping: q.looping,
                tracks,
            })
        })
        .collect();

    ProjectFile {
        version: VERSION,
        master: s.master,
        timing: s.t.timing,
        swing: s.t.swing,
        count_in: s.t.count_in,
        click: s.t.click,
        pads,
        sequences,
        song: SongFile {
            steps: s.song.steps.iter().map(|&(seq, reps)| [seq as u32, reps]).collect(),
            looping: s.song.looping,
        },
        setup: SetupFile { clock_out: s.t.clock_out, key_velocity: s.key_velocity, sync: s.t.sync },
    }
}

fn sound_file(pad: usize) -> String {
    format!("sounds/{}.wav", pad_name(pad))
}

/// Everything a save writes, taken while holding the engine lock; the slow file writing then
/// happens without it, so the audio keeps playing.
pub struct Snapshot {
    file: ProjectFile,
    sounds: Vec<(String, Frames)>,
    rate: u32,
}

pub fn snapshot(s: &Shared) -> Snapshot {
    let sounds = s
        .pads
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.sound.as_ref().map(|snd| (sound_file(i), snd.frames.clone())))
        .collect();
    Snapshot { file: describe(s), sounds, rate: s.rate }
}

/// Writes the project into `dir/name`, replacing an earlier save there.
pub fn save(snap: &Snapshot, dir: &Path, name: &str) -> Result<PathBuf, String> {
    let root = dir.join(name);
    let sounds = root.join("sounds");
    if root.exists() && !is_project(&root) {
        return Err(format!("{name} exists and is not a project"));
    }
    // Sound files from an earlier save may belong to pads that are empty now.
    if sounds.is_dir() {
        fs::remove_dir_all(&sounds).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&sounds).map_err(|e| e.to_string())?;
    for (file, frames) in &snap.sounds {
        write_wav(&root.join(file), frames, snap.rate).map_err(|e| format!("{file}: {e}"))?;
    }
    let json = serde_json::to_string_pretty(&snap.file).map_err(|e| e.to_string())?;
    // Write next to the target and rename, so a failed write never leaves a half-written JSON file.
    let tmp = root.join(format!("{FILE}.tmp"));
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, root.join(FILE)).map_err(|e| e.to_string())?;
    Ok(root)
}

/// Changes whenever anything that would be saved changes, to tell whether there is unsaved work.
pub fn fingerprint(s: &Shared) -> u64 {
    let mut h = DefaultHasher::new();
    serde_json::to_string(&describe(s)).unwrap_or_default().hash(&mut h);
    // Sound contents only change by replacing the frames, so their identity is enough.
    for p in &s.pads {
        p.sound.as_ref().map(|snd| std::sync::Arc::as_ptr(&snd.frames) as *const u8 as usize).hash(&mut h);
    }
    h.finish()
}

/// A project read from disk, ready to replace the open one.
pub struct Loaded {
    pads: Vec<Pad>,
    seqs: Vec<Sequence>,
    song: Song,
    file: ProjectFile,
}

/// Reads the project in `root`, resampling its sounds to `rate`. Nothing is replaced yet, so a
/// broken project leaves the open one untouched.
pub fn load(root: &Path, rate: u32) -> Result<Loaded, String> {
    let json = fs::read_to_string(root.join(FILE)).map_err(|e| e.to_string())?;
    let mut file: ProjectFile = serde_json::from_str(&json).map_err(|e| format!("{FILE}: {e}"))?;
    if file.version > VERSION {
        return Err("Made by a newer rsampler".to_owned());
    }

    let mut pads: Vec<Pad> = (0..PADS).map(|_| Pad::default()).collect();
    for p in std::mem::take(&mut file.pads) {
        let Some(slot) = pads.get_mut(p.pad) else { continue };
        let sound = match p.sound {
            Some(snd) => {
                let frames = load_wav(&root.join(&snd.file), rate).map_err(|e| format!("{}: {e}", snd.file))?;
                // The output may run at another sample rate than when saved; keep the trim in proportion.
                let len = frames.len().max(1);
                let ratio = len as f64 / snd.frames.max(1) as f64;
                let start = ((snd.start as f64 * ratio) as usize).min(len - 1);
                let end = ((snd.end as f64 * ratio).round() as usize).clamp(start + 1, len);
                Some(Sound { name: snd.name, frames, start, end })
            }
            None => None,
        };
        *slot = Pad {
            sound,
            tune: p.tune.clamp(-120, 120),
            attack: p.attack.min(100),
            decay: p.decay.min(100),
            decay_from_start: p.decay_from_start,
            cutoff: p.cutoff.min(100),
            resonance: p.resonance.min(100),
            overlap: p.overlap,
            mutes: p.mutes.map(|m| m.filter(|&m| (m as usize) < PADS)),
            level: p.level.min(100),
            pan: p.pan.clamp(-50, 50),
        };
    }

    let mut seqs: Vec<Sequence> = (0..SEQUENCES).map(|_| Sequence::default()).collect();
    for q in std::mem::take(&mut file.sequences) {
        let Some(seq) = seqs.get_mut(q.index) else { continue };
        seq.tempo = q.tempo.clamp(30.0, 300.0);
        seq.looping = q.looping;
        seq.set_signature(q.beats, if [2, 4, 8, 16].contains(&q.beat_unit) { q.beat_unit } else { 4 });
        seq.set_bars(q.bars);
        let len = seq.len();
        for t in q.tracks.into_iter().filter(|t| t.index < TRACKS) {
            let track = &mut seq.tracks[t.index];
            track.on = t.on;
            track.out = t.out.filter(|ch| *ch < 16);
            for n in t.notes {
                let [tick, pad, velocity, duration, ..] = n[..] else { continue };
                let var = match n[4..] {
                    [param, value] => VarParam::ALL.get(param as usize).map(|p| (*p, value.min(127) as u8)),
                    _ => None,
                };
                if tick < len && (pad as usize) < PADS {
                    track.insert(Note {
                        tick,
                        pad: pad as u8,
                        velocity: velocity.min(127) as u8,
                        duration: duration.max(1),
                        var,
                    });
                }
            }
        }
    }

    let song = Song {
        steps: file
            .song
            .steps
            .iter()
            .filter(|[seq, _]| (*seq as usize) < SEQUENCES)
            .map(|&[seq, reps]| (seq as usize, reps.clamp(1, 99)))
            .collect(),
        looping: file.song.looping,
    };

    Ok(Loaded { pads, seqs, song, file })
}

impl Loaded {
    /// Replaces the open project; quick enough to do while holding the engine lock.
    pub fn apply(self, s: &mut Shared) {
        let Loaded { pads, seqs, song, file } = self;
        s.stop();
        s.pads = pads;
        s.seqs = seqs;
        s.song = song;
        s.master = file.master.clamp(0.0, 1.0);
        s.t.timing = file.timing.min(TIMING.len() - 1);
        s.t.swing = file.swing.clamp(50, 75);
        s.t.count_in = file.count_in;
        s.t.click = file.click;
        s.t.clock_out = file.setup.clock_out;
        s.t.sync = file.setup.sync;
        s.key_velocity = file.setup.key_velocity.clamp(1, 127);
        s.t.seq = 0;
        s.t.track = 0;
        s.t.solo = None;
        s.t.undo = None;
        s.locate(0);
    }
}

fn write_wav(path: &Path, frames: &Frames, rate: u32) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for [l, r] in frames.iter() {
        w.write_sample(*l)?;
        w.write_sample(*r)?;
    }
    w.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio;

    #[test]
    fn save_and_load_round_trip() {
        let rate = 48_000;
        let mut s = audio::offline_shared(rate);
        for (pad, sound) in audio::builtin_kit(rate).into_iter().enumerate() {
            s.pads[pad].sound = Some(sound);
        }
        s.pads[0].tune = -35;
        s.pads[1].overlap = Overlap::NoteOff;
        s.pads[2].mutes = [Some(3), None];
        s.pads[3].sound.as_mut().unwrap().start = 100;
        s.seqs[4].tempo = 93.5;
        s.seqs[4].set_bars(4);
        s.seqs[4].tracks[2].insert(Note {
            tick: 200,
            pad: 1,
            velocity: 77,
            duration: 30,
            var: Some((VarParam::Tune, 90)),
        });
        s.seqs[4].tracks[5].on = false;
        s.seqs[4].tracks[6].out = Some(3);
        s.t.clock_out = true;
        s.key_velocity = 90;
        s.t.swing = 62;
        s.song.steps = vec![(4, 2), (0, 1)];
        s.song.looping = true;

        let dir = std::env::temp_dir().join(format!("rsampler-project-{}", std::process::id()));
        let root = save(&snapshot(&s), &dir, "Beat").unwrap();
        // Saving again over the same project works.
        save(&snapshot(&s), &dir, "Beat").unwrap();
        assert!(is_project(&root));

        let mut t = audio::offline_shared(rate);
        load(&root, rate).unwrap().apply(&mut t);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(t.pads[0].tune, -35);
        assert_eq!(t.pads[1].overlap, Overlap::NoteOff);
        assert_eq!(t.pads[2].mutes, [Some(3), None]);
        let clap = t.pads[3].sound.as_ref().unwrap();
        assert_eq!((clap.name.as_str(), clap.start), ("CLAP", 100));
        assert_eq!(&*clap.frames, &*s.pads[3].sound.as_ref().unwrap().frames);
        assert!(t.pads[4].sound.is_none());
        assert_eq!((t.seqs[4].tempo, t.seqs[4].bars), (93.5, 4));
        assert_eq!(t.seqs[4].tracks[2].notes, s.seqs[4].tracks[2].notes);
        assert!(!t.seqs[4].tracks[5].on);
        assert_eq!(t.seqs[4].tracks[6].out, Some(3));
        assert_eq!((t.t.clock_out, t.key_velocity), (true, 90));
        assert_eq!(t.t.swing, 62);
        assert_eq!(t.song, s.song);
    }

    #[test]
    fn fingerprint_tracks_changes() {
        let mut s = audio::offline_shared(48_000);
        let before = fingerprint(&s);
        assert_eq!(before, fingerprint(&s));
        s.seqs[0].tracks[0].insert(Note { tick: 0, pad: 0, velocity: 1, duration: 1, var: None });
        assert_ne!(before, fingerprint(&s));
    }

    #[test]
    fn refuses_to_overwrite_a_folder_that_is_not_a_project() {
        let dir = std::env::temp_dir().join(format!("rsampler-notproject-{}", std::process::id()));
        fs::create_dir_all(dir.join("Music")).unwrap();
        let s = audio::offline_shared(48_000);
        assert!(save(&snapshot(&s), &dir, "Music").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
