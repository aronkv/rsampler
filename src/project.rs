//! Saving and loading a whole project: a folder with `rsampler.json` and one WAV per pad sound.

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio::{Shared, Song, load_wav};
use crate::program::{
    DRUMS, FilterEnv, Frames, Generation, Overlap, PADS, PROGRAMS, Pad, Program, Sound, VarParam, Velo, pad_name,
};
use crate::seq::{Note, SEQUENCES, Sequence, TIMING, TRACKS};

pub const FILE: &str = "rsampler.json";
/// 2: `decay` became a time, as on the MPC; version 1 stored where the END fade began, in percent
/// of the sound. 3: several programs, in `programs`; earlier versions had one, in `pads`.
const VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    master: f32,
    timing: usize,
    swing: u32,
    count_in: bool,
    click: bool,
    /// The one program of versions 1 and 2.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pads: Vec<PadFile>,
    #[serde(default)]
    programs: Vec<ProgramFile>,
    /// The program each DRUM bus plays.
    #[serde(default)]
    drums: [usize; DRUMS],
    #[serde(default)]
    fx: crate::fx::FxSettings,
    /// Only sequences that differ from the default.
    sequences: Vec<SeqFile>,
    #[serde(default)]
    song: SongFile,
    #[serde(default)]
    setup: SetupFile,
}

#[derive(Serialize, Deserialize)]
struct ProgramFile {
    name: String,
    /// Only pads that differ from the default.
    pads: Vec<PadFile>,
}

#[derive(Serialize, Deserialize)]
struct SetupFile {
    clock_out: bool,
    key_velocity: u8,
    #[serde(default)]
    sync: bool,
    /// The MIDI out port by name; older projects leave the current one.
    #[serde(default)]
    midi_out: Option<String>,
}

impl Default for SetupFile {
    fn default() -> Self {
        Self { clock_out: false, key_velocity: 101, sync: false, midi_out: None }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct SongFile {
    /// Sequence index and repeats.
    steps: Vec<[u32; 2]>,
    looping: bool,
}

#[derive(Serialize, Deserialize, PartialEq)]
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
    #[serde(default)]
    generation: Generation,
    #[serde(default)]
    also: [Option<u8>; 2],
    #[serde(default = "default_switch")]
    switch: [u8; 2],
    #[serde(default)]
    velo: Velo,
    #[serde(default)]
    fenv: FilterEnv,
    #[serde(default)]
    fx_send: u32,
}

fn default_switch() -> [u8; 2] {
    Pad::default().switch
}

#[derive(Serialize, Deserialize, PartialEq)]
struct SoundFile {
    name: String,
    /// Relative to the project folder.
    file: String,
    /// Length when saved, to rescale the trim points if the sample rate changed since.
    frames: usize,
    start: usize,
    end: usize,
    #[serde(default)]
    loop_to: usize,
    #[serde(default)]
    looping: bool,
}

#[derive(Serialize, Deserialize)]
struct SeqFile {
    index: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    on: bool,
    /// 0 MIDI, 1–4 DRUM1–4.
    #[serde(default = "drum1")]
    bus: u8,
    #[serde(default)]
    out: Option<u8>,
    /// tick, pad, velocity, duration, and for notes with a NOTE VARIATION its parameter
    /// (0 TUNE, 1 DECAY, 2 ATTACK, 3 FILTER) and value.
    notes: Vec<Vec<u32>>,
}

fn drum1() -> u8 {
    1
}

pub fn is_project(dir: &Path) -> bool {
    dir.join(FILE).is_file()
}

/// The project as it is written to `rsampler.json`.
fn describe(s: &Shared) -> ProjectFile {
    let programs = s
        .programs
        .iter()
        .enumerate()
        .map(|(n, p)| ProgramFile { name: p.name.clone(), pads: describe_pads(n, &p.pads) })
        .collect();
    let fresh = Sequence::default();
    let sequences = describe_seqs(s, &fresh);
    ProjectFile {
        version: VERSION,
        master: s.master,
        timing: s.t.timing,
        swing: s.t.swing,
        count_in: s.t.count_in,
        click: s.t.click,
        pads: Vec::new(),
        programs,
        drums: s.drums,
        fx: s.fx,
        sequences,
        song: SongFile { steps: s.song.steps.iter().map(|&(q, r)| [q as u32, r]).collect(), looping: s.song.looping },
        setup: SetupFile {
            clock_out: s.t.clock_out,
            key_velocity: s.key_velocity,
            sync: s.t.sync,
            midi_out: Some(s.out_port.clone()),
        },
    }
}

/// A program's pads that differ from the default.
fn describe_pads(program: usize, all: &[Pad]) -> Vec<PadFile> {
    let default = Pad::default();
    let mut pads = Vec::new();
    for (i, p) in all.iter().enumerate() {
        let sound = p.sound.as_ref().map(|snd| SoundFile {
            name: snd.name.clone(),
            file: sound_file(program, i),
            frames: snd.frames.len(),
            start: snd.start,
            end: snd.end,
            loop_to: snd.loop_to,
            looping: snd.looping,
        });
        let file = pad_file(i, p, sound);
        // Pads left at their defaults are not written.
        if file.sound.is_some() || file != pad_file(i, &default, None) {
            pads.push(file);
        }
    }
    pads
}

/// The sequences that differ from `fresh`.
fn describe_seqs(s: &Shared, fresh: &Sequence) -> Vec<SeqFile> {
    s.seqs
        .iter()
        .enumerate()
        .filter_map(|(index, q)| {
            let tracks: Vec<TrackFile> = q
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| !t.on || t.bus != 1 || t.out.is_some() || !t.notes.is_empty() || !t.name.is_empty())
                .map(|(index, t)| TrackFile {
                    index,
                    name: t.name.clone(),
                    on: t.on,
                    bus: t.bus,
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
                || !q.name.is_empty()
                || (q.tempo, q.bars, q.beats, q.beat_unit, q.looping)
                    != (fresh.tempo, fresh.bars, fresh.beats, fresh.beat_unit, fresh.looping);
            changed.then_some(SeqFile {
                index,
                name: q.name.clone(),
                tempo: q.tempo,
                bars: q.bars,
                beats: q.beats,
                beat_unit: q.beat_unit,
                looping: q.looping,
                tracks,
            })
        })
        .collect()
}

fn pad_file(pad: usize, p: &Pad, sound: Option<SoundFile>) -> PadFile {
    PadFile {
        pad,
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
        generation: p.generation,
        also: p.also,
        switch: p.switch,
        velo: p.velo,
        fenv: p.fenv,
        fx_send: p.fx_send,
    }
}

fn sound_file(program: usize, pad: usize) -> String {
    format!("sounds/P{:02}-{}.wav", program + 1, pad_name(pad))
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
        .programs
        .iter()
        .enumerate()
        .flat_map(|(n, p)| {
            p.pads
                .iter()
                .enumerate()
                .filter_map(move |(i, p)| p.sound.as_ref().map(|snd| (sound_file(n, i), snd.frames.clone())))
        })
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
    for p in s.programs.iter().flat_map(|p| &p.pads) {
        p.sound.as_ref().map(|snd| std::sync::Arc::as_ptr(&snd.frames) as *const u8 as usize).hash(&mut h);
    }
    h.finish()
}

/// A program's pads from the project file, with their sounds read from `root`.
fn load_pads(list: Vec<PadFile>, root: &Path, rate: u32, version: u32) -> Result<Vec<Pad>, String> {
    let mut pads: Vec<Pad> = (0..PADS).map(|_| Pad::default()).collect();
    for p in list {
        let Some(slot) = pads.get_mut(p.pad) else { continue };
        let sound = match p.sound {
            Some(snd) => {
                let frames = load_wav(&root.join(&snd.file), rate).map_err(|e| format!("{}: {e}", snd.file))?;
                // The output may run at another sample rate than when saved; keep the trim in proportion.
                let len = frames.len().max(1);
                let ratio = len as f64 / snd.frames.max(1) as f64;
                let start = ((snd.start as f64 * ratio) as usize).min(len - 1);
                let end = ((snd.end as f64 * ratio).round() as usize).clamp(start + 1, len);
                let loop_to = ((snd.loop_to as f64 * ratio) as usize).min(end - 1);
                Some(Sound { name: snd.name, frames, start, end, loop_to, looping: snd.looping })
            }
            None => None,
        };
        let mut decay = p.decay.min(100);
        if version < 2 && !p.decay_from_start {
            // Version 1 faded from `decay` percent of the sound to its end; turn that into the
            // same fade length.
            let secs = sound.as_ref().map_or(0.0, |s| (s.end - s.start) as f32 / rate as f32);
            let fade = secs * (100 - decay) as f32 / 100.0;
            decay = ((fade / 2.0).sqrt() * 100.0).round().min(100.0) as u32;
        }
        *slot = Pad {
            sound,
            tune: p.tune.clamp(-120, 120),
            attack: p.attack.min(100),
            decay,
            decay_from_start: p.decay_from_start,
            cutoff: p.cutoff.min(100),
            resonance: p.resonance.min(100),
            overlap: p.overlap,
            mutes: p.mutes.map(|m| m.filter(|&m| (m as usize) < PADS)),
            level: p.level.min(100),
            pan: p.pan.clamp(-50, 50),
            generation: p.generation,
            also: p.also.map(|m| m.filter(|&m| (m as usize) < PADS)),
            switch: p.switch.map(|v| v.min(127)),
            velo: Velo {
                level: p.velo.level.min(100),
                attack: p.velo.attack.min(100),
                start: p.velo.start.min(100),
                filter: p.velo.filter.min(100),
            },
            fenv: FilterEnv {
                attack: p.fenv.attack.min(100),
                decay: p.fenv.decay.min(100),
                amount: p.fenv.amount.min(100),
            },
            fx_send: p.fx_send.min(100),
        };
    }

    Ok(pads)
}

/// A project read from disk, ready to replace the open one.
pub struct Loaded {
    programs: Vec<Program>,
    drums: [usize; DRUMS],
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

    // Before version 3 the one program was in `pads`.
    let old = std::mem::take(&mut file.pads);
    let mut programs = Vec::new();
    for p in std::mem::take(&mut file.programs).into_iter().take(PROGRAMS) {
        programs.push(Program {
            name: p.name.chars().take(16).collect(),
            pads: load_pads(p.pads, root, rate, file.version)?,
        });
    }
    if programs.is_empty() {
        programs.push(Program { name: "NewPgm-A".to_owned(), pads: load_pads(old, root, rate, file.version)? });
    }
    let drums = file.drums.map(|d| d.min(programs.len() - 1));

    let mut seqs: Vec<Sequence> = (0..SEQUENCES).map(|_| Sequence::default()).collect();
    for q in std::mem::take(&mut file.sequences) {
        let Some(seq) = seqs.get_mut(q.index) else { continue };
        seq.name = q.name.chars().take(16).collect();
        seq.tempo = q.tempo.clamp(30.0, 300.0);
        seq.looping = q.looping;
        seq.set_signature(q.beats, if [2, 4, 8, 16].contains(&q.beat_unit) { q.beat_unit } else { 4 });
        seq.set_bars(q.bars);
        let len = seq.len();
        for t in q.tracks.into_iter().filter(|t| t.index < TRACKS) {
            let track = &mut seq.tracks[t.index];
            track.name = t.name.chars().take(16).collect();
            track.on = t.on;
            track.bus = t.bus.min(DRUMS as u8);
            track.out = t.out.filter(|ch| *ch < 16);
            for n in t.notes {
                let [tick, pad, velocity, duration, ..] = n[..] else { continue };
                let var = match n[4..] {
                    [param, value] => VarParam::ALL.get(param as usize).map(|p| (*p, value.min(127) as u8)),
                    _ => None,
                };
                let top = if track.bus == 0 { crate::seq::KEYS } else { PADS };
                if tick < len && (pad as usize) < top {
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

    Ok(Loaded { programs, drums, seqs, song, file })
}

impl Loaded {
    /// Replaces the open project; quick enough to do while holding the engine lock.
    pub fn apply(self, s: &mut Shared) {
        let Loaded { programs, drums, seqs, song, file } = self;
        s.stop();
        (s.programs, s.drums) = (programs, drums);
        s.seqs = seqs;
        s.song = song;
        s.master = file.master.clamp(0.0, 1.0);
        s.fx = crate::fx::FxSettings {
            reverb: file.fx.reverb.min(100),
            size: file.fx.size.min(100),
            damp: file.fx.damp.min(100),
            delay: file.fx.delay.min(100),
            time: file.fx.time.min(crate::fx::DELAY_TIMES.len() - 1),
            feedback: file.fx.feedback.min(95),
        };
        s.t.timing = file.timing.min(TIMING.len() - 1);
        s.t.swing = file.swing.clamp(50, 75);
        s.t.count_in = file.count_in;
        s.t.click = file.click;
        s.t.clock_out = file.setup.clock_out;
        s.t.sync = file.setup.sync;
        s.key_velocity = file.setup.key_velocity.clamp(1, 127);
        if let Some(port) = file.setup.midi_out {
            s.out_port = port;
        }
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
            s.programs[0].pads[pad].sound = Some(sound);
        }
        s.programs[0].pads[0].tune = -35;
        let kick = s.programs[0].pads[0].sound.as_mut().unwrap();
        (kick.loop_to, kick.looping) = (500, true);
        s.programs[0].pads[1].overlap = Overlap::NoteOff;
        s.programs[0].pads[2].mutes = [Some(3), None];
        s.programs[0].pads[2].generation = Generation::VelSw;
        s.programs[0].pads[2].also = [Some(0), Some(1)];
        s.programs[0].pads[2].switch = [40, 90];
        s.programs[0].pads[3].sound.as_mut().unwrap().start = 100;
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
        s.seqs[4].tracks[6].bus = 2;
        // A second program, with the clap on its pad B03, played by DRUM2.
        let mut two = crate::program::Program::new("Second");
        two.pads[18].sound = s.programs[0].pads[3].sound.clone();
        s.programs.push(two);
        s.drums = [0, 1, 0, 1];
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

        assert_eq!(t.programs[0].pads[0].tune, -35);
        assert_eq!((t.programs.len(), t.programs[1].name.as_str(), t.drums), (2, "Second", [0, 1, 0, 1]));
        assert_eq!(t.programs[1].pads[18].sound.as_ref().unwrap().name, "CLAP");
        assert_eq!(t.seqs[4].tracks[6].bus, 2);
        let kick = t.programs[0].pads[0].sound.as_ref().unwrap();
        assert_eq!((kick.loop_to, kick.looping), (500, true));
        assert_eq!(t.programs[0].pads[1].overlap, Overlap::NoteOff);
        assert_eq!(t.programs[0].pads[2].mutes, [Some(3), None]);
        assert_eq!(
            (t.programs[0].pads[2].generation, t.programs[0].pads[2].also, t.programs[0].pads[2].switch),
            (Generation::VelSw, [Some(0), Some(1)], [40, 90])
        );
        let clap = t.programs[0].pads[3].sound.as_ref().unwrap();
        assert_eq!((clap.name.as_str(), clap.start), ("CLAP", 100));
        assert_eq!(&*clap.frames, &*s.programs[0].pads[3].sound.as_ref().unwrap().frames);
        assert!(t.programs[0].pads[4].sound.is_none());
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
    fn version_1_decay_becomes_a_fade_time() {
        let rate = 48_000;
        let mut s = audio::offline_shared(rate);
        // Two seconds of sound; version 1 started the fade halfway through.
        s.programs[0].pads[0].sound = Some(Sound::new("TONE", vec![[0.1f32; 2]; 2 * rate as usize].into()));
        s.programs[0].pads[0].decay = 50;
        s.programs[0].pads[1].decay = 30;
        s.programs[0].pads[1].decay_from_start = true;
        let dir = std::env::temp_dir().join(format!("rsampler-v1-{}", std::process::id()));
        let root = save(&snapshot(&s), &dir, "Old").unwrap();
        // Rewritten as version 1 had it: one program, in `pads`.
        let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(root.join(FILE)).unwrap()).unwrap();
        json["version"] = 1.into();
        json["pads"] = json["programs"][0]["pads"].take();
        json.as_object_mut().unwrap().remove("programs");
        fs::write(root.join(FILE), json.to_string()).unwrap();
        let mut t = audio::offline_shared(rate);
        load(&root, rate).unwrap().apply(&mut t);
        fs::remove_dir_all(&dir).unwrap();
        // A one-second fade is decay 71 (0.71² × 2 s); START mode keeps its value.
        assert_eq!(t.programs[0].pads[0].decay, 71);
        assert_eq!(t.programs[0].pads[1].decay, 30);
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
