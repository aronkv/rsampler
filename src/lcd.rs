//! What the LCD shows on each screen, as rows of text segments; some segments are editable fields.

use std::fs;
use std::path::{Path, PathBuf};

use crate::audio::{RecMode, Shared};
use crate::program::pad_name;
use crate::seq::TIMING;

/// LOAD and SAVE screens: a directory listing of subfolders, and WAV files when loading.
pub struct Browser {
    pub dir: PathBuf,
    pub entries: Vec<PathBuf>,
    pub cursor: usize,
}

impl Browser {
    pub fn open(dir: PathBuf, wavs: bool) -> Self {
        let visible = |p: &Path| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| visible(p) && (p.is_dir() || (wavs && is_wav(p))))
            .collect();
        entries.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|n| n.to_string_lossy().to_lowercase())));
        Self { dir, entries, cursor: 0 }
    }
}

fn is_wav(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

pub enum Screen {
    Main,
    Load(Browser),
    Save(Browser),
    Program,
    Mixer,
    Trim,
    Step,
    Sample,
    Song,
    /// NOTE VARIATION assign.
    Assign,
    Setup,
    Other,
}

impl Screen {
    pub fn soft_keys(&self) -> [&'static str; 6] {
        match self {
            Screen::Main => ["STEP", "UNDO", "TR.ON", "SOLO", "TR -", "TR +"],
            Screen::Step => ["DELETE", "", "", "", "TR -", "TR +"],
            Screen::Load(_) => ["UP DIR", "SAVE", "", "", "", "DO IT"],
            Screen::Save(_) => ["UP DIR", "", "", "", "NAME", "DO IT"],
            Screen::Program => ["LOAD", "TRIM", "MIXER", "", "", "CLEAR"],
            Screen::Mixer => ["", "", "", "", "", "RESET"],
            Screen::Trim => ["PLAY", "", "", "", "", "RESET"],
            Screen::Sample => ["", "", "", "", "", "REC"],
            Screen::Song => ["INSERT", "DELETE", "", "", "", ""],
            Screen::Assign | Screen::Setup | Screen::Other => ["", "", "", "", "", ""],
        }
    }

    /// Mixer and TRIM draw graphics below their first rows.
    pub fn text_rows(&self) -> usize {
        match self {
            Screen::Mixer | Screen::Trim => 2,
            _ => 6,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Seq,
    Tempo,
    Timing,
    Swing,
    Count,
    Click,
    Loop,
    Bars,
    Beats,
    BeatUnit,
    Track,
    TrackOn,
    Now,
    Pad,
    Tune,
    Attack,
    Decay,
    DecayMode,
    Cutoff,
    Reso,
    Overlap,
    Mute1,
    Mute2,
    Level,
    Pan,
    Start,
    End,
    NotePos,
    NotePad,
    NoteVel,
    NoteDur,
    SampleMode,
    Threshold,
    SampleTime,
    SongLoop,
    SongSeq,
    SongReps,
    VarPad,
    VarParam,
    TrackOut,
    ClockOut,
    KeyVelocity,
}

pub struct Seg {
    pub text: String,
    pub field: Option<Field>,
}

fn t(text: impl Into<String>) -> Seg {
    Seg { text: text.into(), field: None }
}

fn f(field: Field, text: impl Into<String>) -> Seg {
    Seg { text: text.into(), field: Some(field) }
}

/// A line of the LCD: segments flush left and segments flush right, and whether it is
/// highlighted (the LOAD cursor).
#[derive(Default)]
pub struct Row {
    pub left: Vec<Seg>,
    pub right: Vec<Seg>,
    pub inverted: bool,
}

fn row(left: Vec<Seg>, right: Vec<Seg>) -> Row {
    Row { left, right, inverted: false }
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

pub fn pan_text(pan: i32) -> String {
    match pan {
        0 => "MID".to_owned(),
        p if p < 0 => format!("L{}", -p),
        p => format!("R{p}"),
    }
}

fn mute_text(m: Option<u8>) -> String {
    m.map_or("---".to_owned(), |p| pad_name(p as usize))
}

pub struct View<'a> {
    pub s: &'a Shared,
    pub screen: &'a Screen,
    /// Selected program pad, 0..64.
    pub pad: usize,
    pub velocity: u8,
    pub message: &'a str,
    /// Project name on the SAVE screen, and whether it is being typed.
    pub name: &'a str,
    pub editing: bool,
    /// Selected note on the STEP EDIT screen.
    pub step: usize,
    pub sample: Option<SampleView>,
    /// Selected step on the SONG screen.
    pub song_step: usize,
    /// NOTE VARIATION: assigned pad, parameter and slider position.
    pub var: (usize, crate::program::VarParam, u8),
}

/// What the SAMPLE screen shows.
pub struct SampleView {
    pub input: String,
    pub state: crate::sample::State,
    pub seconds: f32,
    pub mono: bool,
    /// dBFS; below −60 means off.
    pub threshold: i32,
    pub time: u32,
    pub gain_db: f32,
}

fn status(s: &Shared) -> &'static str {
    match (s.t.playing, s.t.rec) {
        (true, _) if s.t.tick < 0.0 => "COUNT IN",
        (true, _) if s.t.song.is_some() => "SONG PLAY",
        (true, RecMode::Rec) => "REC",
        (true, RecMode::Overdub) => "OVERDUB",
        (true, RecMode::Off) => "PLAY",
        (false, RecMode::Rec) => "REC READY",
        (false, RecMode::Overdub) => "ODUB READY",
        (false, RecMode::Off) => "STOP",
    }
}

fn sound_name(v: &View) -> String {
    v.s.pads[v.pad].sound.as_ref().map_or("(no sound)".to_owned(), |s| s.name.chars().take(20).collect())
}

pub fn rows(v: &View) -> Vec<Row> {
    let s = v.s;
    let msg = |rows: &mut Vec<Row>| {
        rows.resize_with(5, Row::default);
        rows.push(row(vec![t(v.message)], vec![t(status(s))]));
    };
    match v.screen {
        Screen::Main => {
            let seq = s.seq();
            let tr = s.t.track;
            let track = &seq.tracks[tr];
            let swing = TIMING[s.t.timing].1;
            let mut rows = vec![
                row(
                    vec![f(Field::Seq, format!("Sq:{:02}", s.t.seq + 1)), t(format!("-Sequence{:02}", s.t.seq + 1))],
                    vec![t("Now:"), f(Field::Now, seq.bbt(s.now()))],
                ),
                row(
                    vec![
                        t("Tempo:"),
                        f(Field::Tempo, format!("{:5.1}", seq.tempo)),
                        t("  Timing:"),
                        f(Field::Timing, TIMING[s.t.timing].0),
                        t("  Swing:"),
                        f(
                            Field::Swing,
                            if swing == 48 || swing == 24 { format!("{}%", s.t.swing) } else { "--".into() },
                        ),
                    ],
                    vec![
                        t("Tsig:"),
                        f(Field::Beats, seq.beats.to_string()),
                        t("/"),
                        f(Field::BeatUnit, seq.beat_unit.to_string()),
                    ],
                ),
                row(
                    vec![
                        t("Count:"),
                        f(Field::Count, on_off(s.t.count_in)),
                        t("  Click:"),
                        f(Field::Click, on_off(s.t.click)),
                        t("  Loop:"),
                        f(Field::Loop, on_off(seq.looping)),
                    ],
                    vec![t("Bars:"), f(Field::Bars, seq.bars.to_string())],
                ),
                row(
                    vec![
                        t("Tr:"),
                        f(Field::Track, format!("{:02}", tr + 1)),
                        t(format!("-Track-{:02}", tr + 1)),
                        t("  On:"),
                        f(Field::TrackOn, if track.on { "YES" } else { "NO" }),
                        t("  Out:"),
                        f(Field::TrackOut, track.out.map_or("OFF".to_owned(), |ch| (ch + 1).to_string())),
                        t(if s.t.solo == Some(tr) { "  SOLO" } else { "" }),
                    ],
                    vec![t(if track.notes.is_empty() { "(Unused)" } else { "(Used)" })],
                ),
                row(
                    vec![t(format!("Pad:{} {}", pad_name(v.pad), sound_name(v)))],
                    vec![t(format!("Vel:{:3}", v.velocity))],
                ),
            ];
            if let Some(next) = s.t.next_seq {
                rows.push(row(vec![t(v.message)], vec![t(format!("Next:{:02}  {}", next + 1, status(s)))]));
            } else {
                msg(&mut rows);
            }
            rows
        }
        Screen::Sample => {
            use crate::sample::State;
            let Some(sv) = &v.sample else {
                let mut rows = vec![row(vec![t("SAMPLE")], vec![])];
                msg(&mut rows);
                return rows;
            };
            let state = match sv.state {
                State::Idle => "READY".to_owned(),
                State::Armed => "WAITING FOR INPUT".to_owned(),
                State::Recording => format!("REC {:.1}s", sv.seconds),
                State::Done => format!("{:.2}s  KEEP OR RETRY?", sv.seconds),
            };
            let input: String = sv.input.chars().take(36).collect();
            let mut rows = vec![
                row(vec![t(format!("SAMPLE  In:{input}"))], vec![t(state)]),
                row(
                    vec![
                        t("Mode:"),
                        f(Field::SampleMode, if sv.mono { "MONO" } else { "STEREO" }),
                        t("  Threshold:"),
                        f(
                            Field::Threshold,
                            if sv.threshold < -60 { "OFF".to_owned() } else { format!("{}dB", sv.threshold) },
                        ),
                        t("  Time:"),
                        f(Field::SampleTime, format!("{}s", sv.time)),
                    ],
                    vec![t(format!("Gain:{:+.1}dB", sv.gain_db))],
                ),
                row(vec![t(format!("To pad:{} {}", pad_name(v.pad), sound_name(v)))], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Setup => {
            let mut rows = vec![
                row(vec![t("SETUP")], vec![]),
                row(vec![t("MIDI clock out:"), f(Field::ClockOut, on_off(s.t.clock_out))], vec![]),
                row(vec![t("Keyboard pad velocity:"), f(Field::KeyVelocity, s.key_velocity.to_string())], vec![]),
                row(
                    vec![t(if s.midi_out.is_some() { "MIDI out port: rsampler" } else { "MIDI out: not available" })],
                    vec![],
                ),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Other => {
            let mut rows = vec![
                row(vec![t(format!("rsampler {}", env!("CARGO_PKG_VERSION")))], vec![]),
                row(vec![t("Sampler and sequencer after the MPC2000XL")], vec![]),
                row(vec![t(format!("Output: {} Hz", s.rate))], vec![]),
                row(vec![t("Apache License 2.0")], vec![]),
                row(vec![t("github.com/aronkv/rsampler")], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Assign => {
            use crate::program::VarParam;
            let (pad, param, value) = v.var;
            let base = &s.pads[pad];
            let varied = base.varied(Some((param, value)));
            let effect = match param {
                VarParam::Tune => format!("Tune:{:+.1}", varied.tune as f32 / 10.0),
                VarParam::Decay => format!("Decay:{}", varied.decay),
                VarParam::Attack => format!("Attack:{}", varied.attack),
                VarParam::Filter => format!("Filter:{}", varied.cutoff),
            };
            let name = base.sound.as_ref().map_or("(no sound)".to_owned(), |snd| snd.name.chars().take(20).collect());
            let mut rows = vec![
                row(vec![t("NOTE VARIATION ASSIGN")], vec![]),
                row(vec![t("Pad:"), f(Field::VarPad, pad_name(pad)), t(format!("  {name}"))], vec![]),
                row(vec![t("Param:"), f(Field::VarParam, param.label())], vec![]),
                row(vec![t(format!("Slider:{value:3}  gives {effect}"))], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Song => {
            let song = &s.song;
            let playing = match s.t.song {
                Some((step, rep)) => format!(
                    "Step:{}/{} Rep:{}/{}",
                    step + 1,
                    song.steps.len(),
                    rep + 1,
                    song.steps[step.min(song.steps.len() - 1)].1
                ),
                None => format!("{} steps", song.steps.len()),
            };
            let mut rows =
                vec![row(vec![t("SONG  Loop:"), f(Field::SongLoop, on_off(song.looping))], vec![t(playing)])];
            let start = v.song_step.saturating_sub(1).min(song.steps.len().saturating_sub(4));
            for (i, &(seq, reps)) in song.steps.iter().enumerate().skip(start).take(4) {
                let used = if s.seqs[seq].tracks.iter().any(|t| !t.notes.is_empty()) { "" } else { " (Unused)" };
                let current = if s.t.song.is_some_and(|(step, _)| step == i) { ">" } else { " " };
                let left = if i == v.song_step {
                    vec![
                        t(format!("{current}{:02} ", i + 1)),
                        f(Field::SongSeq, format!("Sq:{:02}", seq + 1)),
                        t(format!("-Sequence{:02}{used}", seq + 1)),
                    ]
                } else {
                    vec![t(format!("{current}{:02} Sq:{:02}-Sequence{:02}{used}", i + 1, seq + 1, seq + 1))]
                };
                let right = if i == v.song_step {
                    vec![t("Reps:"), f(Field::SongReps, reps.to_string())]
                } else {
                    vec![t(format!("Reps:{reps}"))]
                };
                rows.push(row(left, right));
            }
            if song.steps.is_empty() {
                rows.push(row(vec![t(" (empty: INSERT adds a step)")], vec![]));
            }
            msg(&mut rows);
            rows
        }
        Screen::Step => {
            let seq = s.seq();
            let tr = s.t.track;
            let notes = &seq.tracks[tr].notes;
            let mut rows = vec![row(
                vec![
                    t("STEP EDIT  Tr:"),
                    f(Field::Track, format!("{:02}", tr + 1)),
                    t(format!("  {} notes", notes.len())),
                ],
                vec![t(format!("Now:{}", seq.bbt(s.now())))],
            )];
            let name = |pad: u8| {
                s.pads[pad as usize].sound.as_ref().map_or(String::new(), |snd| snd.name.chars().take(8).collect())
            };
            let start = v.step.saturating_sub(1).min(notes.len().saturating_sub(4));
            for (i, n) in notes.iter().enumerate().skip(start).take(4) {
                let (pos, pad, vel, dur) =
                    (seq.bbt(n.tick), pad_name(n.pad as usize), format!("{:3}", n.velocity), n.duration.to_string());
                let left = if i == v.step {
                    vec![
                        f(Field::NotePos, pos),
                        t(" "),
                        f(Field::NotePad, pad),
                        t(format!(" {:<8}", name(n.pad))),
                        t(" Vel:"),
                        f(Field::NoteVel, vel),
                        t(" Dur:"),
                        f(Field::NoteDur, dur),
                    ]
                } else {
                    vec![t(format!("{pos} {pad} {:<8}  Vel:{vel} Dur:{dur}", name(n.pad)))]
                };
                rows.push(row(left, vec![]));
            }
            if notes.is_empty() {
                rows.push(row(vec![t("(no notes)")], vec![]));
            }
            rows.resize_with(5, Row::default);
            let hint = if v.message.is_empty() { "Pads add a note at Now" } else { v.message };
            rows.push(row(vec![t(hint)], vec![t(status(s))]));
            rows
        }
        Screen::Program => {
            let p = &s.pads[v.pad];
            let mut rows = vec![
                row(
                    vec![t("PROGRAM  Pad:"), f(Field::Pad, pad_name(v.pad)), t(format!("  {}", sound_name(v)))],
                    vec![],
                ),
                row(
                    vec![
                        t("Tune:"),
                        f(Field::Tune, format!("{:+.1}", p.tune as f32 / 10.0)),
                        t("  Attack:"),
                        f(Field::Attack, format!("{:3}", p.attack)),
                        t("  Decay:"),
                        f(Field::Decay, format!("{:3}", p.decay)),
                    ],
                    vec![t("Dcy md:"), f(Field::DecayMode, if p.decay_from_start { "START" } else { "END" })],
                ),
                row(
                    vec![
                        t("Filter:"),
                        f(Field::Cutoff, format!("{:3}", p.cutoff)),
                        t("  Reso:"),
                        f(Field::Reso, format!("{:3}", p.resonance)),
                    ],
                    vec![],
                ),
                row(
                    vec![
                        t("Overlap:"),
                        f(Field::Overlap, p.overlap.label()),
                        t("  Mutes:"),
                        f(Field::Mute1, mute_text(p.mutes[0])),
                        t(" "),
                        f(Field::Mute2, mute_text(p.mutes[1])),
                    ],
                    vec![],
                ),
                row(
                    vec![
                        t("Level:"),
                        f(Field::Level, format!("{:3}", p.level)),
                        t("  Pan:"),
                        f(Field::Pan, pan_text(p.pan)),
                    ],
                    vec![],
                ),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Mixer => {
            let p = &s.pads[v.pad];
            vec![
                row(
                    vec![t(format!("MIXER  Bank:{}", ["A", "B", "C", "D"][v.pad / 16]))],
                    vec![t(format!("{} {}", pad_name(v.pad), sound_name(v)))],
                ),
                row(
                    vec![
                        t("Level:"),
                        f(Field::Level, format!("{:3}", p.level)),
                        t("  Pan:"),
                        f(Field::Pan, pan_text(p.pan)),
                    ],
                    vec![t(v.message)],
                ),
            ]
        }
        Screen::Trim => {
            let secs = |frames: usize| format!("{:.3}s", frames as f32 / s.rate as f32);
            let (st, end, len) =
                s.pads[v.pad].sound.as_ref().map_or((0, 0, 0), |snd| (snd.start, snd.end, snd.frames.len()));
            vec![
                row(
                    vec![t("TRIM  Pad:"), f(Field::Pad, pad_name(v.pad)), t(format!("  {}", sound_name(v)))],
                    vec![t(format!("Len:{}", secs(len)))],
                ),
                row(
                    vec![t("St:"), f(Field::Start, secs(st)), t("  End:"), f(Field::End, secs(end))],
                    vec![t(if v.message.is_empty() {
                        format!("Play:{}", secs(end.saturating_sub(st)))
                    } else {
                        v.message.to_owned()
                    })],
                ),
            ]
        }
        Screen::Load(b) | Screen::Save(b) => {
            let saving = matches!(v.screen, Screen::Save(_));
            let dir = b.dir.to_string_lossy();
            let tail: String = dir.chars().rev().take(58).collect::<Vec<_>>().into_iter().rev().collect();
            let mut rows = vec![row(vec![t(format!("{} {tail}", if saving { "SAVE" } else { "LOAD" }))], vec![])];
            let start = b.cursor.saturating_sub(1);
            for (i, entry) in b.entries.iter().enumerate().skip(start).take(3) {
                let name = entry.file_name().unwrap_or_default().to_string_lossy();
                let (name, kind) = if crate::project::is_project(entry) {
                    (name.into_owned(), "PROJECT")
                } else if entry.is_dir() {
                    (format!("{name}/"), "")
                } else {
                    (name.into_owned(), "")
                };
                rows.push(Row { left: vec![t(format!(" {name}"))], right: vec![t(kind)], inverted: i == b.cursor });
            }
            if b.entries.is_empty() {
                rows.push(row(vec![t(if saving { " (no folders)" } else { " (no folders or .wav files)" })], vec![]));
            }
            rows.resize_with(4, Row::default);
            if saving {
                let cursor = if v.editing { "_" } else { "" };
                rows.push(row(
                    vec![t("Name:"), t(format!("{}{cursor}", v.name))],
                    vec![t(if v.editing { "ENTER: done" } else { "NAME: type" })],
                ));
                let hint = if v.message.is_empty() { "DO IT: save here" } else { v.message };
                rows.push(row(vec![t(hint)], vec![]));
            } else {
                rows.push(row(vec![t(format!("To pad:{} {}", pad_name(v.pad), sound_name(v)))], vec![]));
                let hint = if v.message.is_empty() { "ENTER: load" } else { v.message };
                rows.push(row(vec![t(hint)], vec![]));
            }
            rows
        }
    }
}

/// Editable fields of a screen in cursor order.
pub fn fields(rows: &[Row]) -> Vec<(usize, Field)> {
    rows.iter()
        .enumerate()
        .flat_map(|(i, r)| r.left.iter().chain(&r.right).filter_map(move |s| s.field.map(|f| (i, f))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_labels() {
        assert_eq!(pan_text(0), "MID");
        assert_eq!(pan_text(-50), "L50");
        assert_eq!(pan_text(12), "R12");
    }
}
