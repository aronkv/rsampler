//! What the LCD shows on each screen, as rows of text segments; some segments are editable fields.

use std::fs;
use std::path::{Path, PathBuf};

use crate::audio::{RecMode, Shared};
use crate::program::{Generation, pad_name};
use crate::seq::{EventOp, TIMING, ValueMode};

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
            .filter(|p| visible(p) && (p.is_dir() || (wavs && is_sound(p))))
            .collect();
        entries.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|n| n.to_string_lossy().to_lowercase())));
        Self { dir, entries, cursor: 0 }
    }
}

/// Sound and music files, the MPC2000XL's own .SND sounds, .PGM and .APS programs and .ALL
/// sequences, and MIDI files.
fn is_sound(path: &Path) -> bool {
    let mpc = ["snd", "pgm", "aps", "all", "mid"];
    path.extension()
        .is_some_and(|e| crate::audio::AUDIO_EXTENSIONS.iter().chain(&mpc).any(|x| e.eq_ignore_ascii_case(x)))
}

pub enum Screen {
    Main,
    Load(Browser),
    Save(Browser),
    Program,
    Mixer,
    Trim,
    Loop,
    Zone,
    /// The EDIT window, working on the section of the screen it was opened from.
    Edit(Section),
    Step,
    /// EDIT on the MAIN screen: copying and changing notes (EVENTS) and copying bars (BARS).
    Events,
    Bars,
    Sample,
    Song,
    /// NOTE VARIATION assign.
    Assign,
    /// 16 LEVELS assign.
    Levels,
    /// PROGRAM's VELOCITY MODULATION window.
    Velo,
    /// The effects on the FX SEND bus.
    Fx,
    /// TRACK MUTE: the pads mute tracks; `solo` while SOLO waits for a pad.
    TrMute {
        solo: bool,
    },
    /// NEXT SEQ: the pads pick the next sequence.
    NextSeq,
    /// OPEN WINDOW on the sequence and track fields: name, copy, delete.
    SeqWin,
    /// OPEN WINDOW on PROGRAM's `Pgm`: new, copy, delete, name.
    PgmWin,
    TrWin,
    /// MISC: PUNCH, TRANS and 2ND SEQ.
    Punch,
    Trans,
    SecondSeq,
    Setup,
    Other,
}

/// Which part of the sound EDIT works on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    /// St–End.
    Trim,
    /// Loop point to End.
    Loop,
    /// The selected zone.
    Zone,
}

impl Section {
    /// The section of the TRIM, LOOP or ZONE screen.
    pub fn of(screen: &Screen) -> Section {
        match screen {
            Screen::Loop => Section::Loop,
            Screen::Zone => Section::Zone,
            _ => Section::Trim,
        }
    }

    pub fn screen(self) -> Screen {
        match self {
            Section::Trim => Screen::Trim,
            Section::Loop => Screen::Loop,
            Section::Zone => Screen::Zone,
        }
    }
}

impl Screen {
    pub fn soft_keys(&self) -> [&'static str; 6] {
        match self {
            Screen::Main => ["STEP", "EDIT", "TR.ON", "SOLO", "TR -", "TR +"],
            Screen::Events | Screen::Bars => ["EVENTS", "BARS", "", "", "", "DO IT"],
            Screen::Step => ["DELETE", "", "", "", "TR -", "TR +"],
            Screen::Load(_) => ["UP DIR", "SAVE", "", "", "PLAY", "DO IT"],
            Screen::Save(_) => ["UP DIR", "SONG", "WAV", "MIDI", "NAME", "DO IT"],
            Screen::Program => ["LOAD", "TRIM", "MIXER", "VELO", "", "CLEAR"],
            Screen::Velo => ["PROGRAM", "", "", "", "", ""],
            Screen::Mixer => ["STEREO", "FX SEND", "", "", "FX", "RESET"],
            Screen::Fx => ["MIXER", "", "", "", "", ""],
            Screen::Trim | Screen::Loop | Screen::Zone => ["TRIM", "LOOP", "ZONE", "RESET", "EDIT", "PLAY X"],
            Screen::Edit(_) => ["", "", "", "CANCEL", "DO IT", ""],
            Screen::Sample => ["", "", "", "", "", "REC"],
            Screen::Song => ["INSERT", "DELETE", "", "", "", ""],
            Screen::Assign | Screen::Levels | Screen::NextSeq | Screen::Setup | Screen::Other => {
                ["", "", "", "", "", ""]
            }
            Screen::TrMute { .. } => ["", "", "", "", "", "SOLO"],
            Screen::SeqWin | Screen::TrWin => ["", "", "COPY", "DELETE", "NAME", "CLOSE"],
            Screen::PgmWin => ["", "NEW", "COPY", "DELETE", "NAME", "CLOSE"],
            Screen::Punch | Screen::Trans | Screen::SecondSeq => ["PUNCH", "TRANS", "2ND SEQ", "", "", "ON/OFF"],
        }
    }

    /// Mixer and TRIM draw graphics below their first rows.
    pub fn text_rows(&self) -> usize {
        match self {
            Screen::Mixer | Screen::Trim | Screen::Loop | Screen::Zone => 2,
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
    Sync,
    KeyVelocity,
    GenMode,
    Also1,
    Also2,
    Switch1,
    Switch2,
    LoopTo,
    LoopLen,
    Looping,
    ZoneCount,
    Zone,
    ZoneStart,
    ZoneEnd,
    EditOp,
    EditTo,
    EvOp,
    EvPad,
    EvFrom,
    EvTo,
    EvToSeq,
    EvToTrack,
    EvMerge,
    EvStart,
    EvCopies,
    EvMode,
    EvValue,
    FirstBar,
    LastBar,
    AfterBar,
    LevelsPad,
    LevelsParam,
    LevelsType,
    LevelsKey,
    VeloLevel,
    VeloAttack,
    VeloStart,
    VeloFilter,
    OutPort,
    EditRatio,
    EnvAttack,
    EnvDecay,
    EnvAmount,
    PunchMode,
    PunchFrom,
    PunchTo,
    TransTrack,
    TransAmount,
    SecondSeq,
    CopyTo,
    Bus,
    Pgm,
    FxSend,
    FxReverb,
    FxSize,
    FxDamp,
    FxDelay,
    FxTime,
    FxFeedback,
    EditRate,
    EditBits,
}

/// The settings of the sequence EDIT screens, kept between visits like on the MPC.
pub struct SeqEdit {
    pub op: EventOp,
    /// Only this pad's notes, or all.
    pub pad: Option<u8>,
    /// Time range in ticks, end excluded.
    pub from: u32,
    pub to: u32,
    pub to_seq: usize,
    pub to_track: usize,
    pub merge: bool,
    pub start: u32,
    pub copies: u32,
    pub mode: ValueMode,
    pub value: u32,
    /// BARS, counted from 1; `after_bar` 0 inserts before the first bar.
    pub first_bar: u32,
    pub last_bar: u32,
    pub after_bar: u32,
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

pub fn bus_name(bus: u8) -> String {
    if bus == 0 { "MIDI".to_owned() } else { format!("DRUM{bus}") }
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
    /// ZONE: the selected zone and the zone bounds of the selected pad's sound.
    pub zone: (usize, &'a [usize]),
    /// EDIT: the operation, the pad new sounds go to, TIME STRETCH's ratio in tenths of a
    /// percent, and RESAMPLE's rate and bits.
    pub edit: (crate::edit::Op, usize, u32, (u32, u32)),
    pub seq_edit: &'a SeqEdit,
    /// MIXER shows the FX SEND levels instead of level and pan.
    pub mixer_fx: bool,
    /// SEQUENCE and TRACK windows: where COPY copies to.
    pub copy_to: usize,
    /// Selected PAD BANK, 0..4.
    pub bank: usize,
    /// The MIDI out port.
    pub out_port: &'a str,
    /// 16 LEVELS: the pad, NOTE VAR's parameter (None = VELOCITY), and the original-key pad.
    pub levels: (usize, Option<crate::program::VarParam>, usize),
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
    sound_name_of(v.s, v.pad)
}

fn sound_name_of(s: &Shared, pad: usize) -> String {
    s.pads()[pad].sound.as_ref().map_or("(no sound)".to_owned(), |s| s.name.chars().take(20).collect())
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
                    vec![
                        f(Field::Seq, format!("Sq:{:02}", s.t.seq + 1)),
                        t(format!("-{}", crate::seq::seq_name(s.t.seq, seq))),
                    ],
                    vec![t("Now:"), f(Field::Now, seq.bbt(s.now()))],
                ),
                row(
                    vec![
                        t("Tempo:"),
                        f(
                            Field::Tempo,
                            match (s.t.sync, s.t.ext.tempo()) {
                                (true, Some(ext)) => format!("{ext:5.1} EXT"),
                                (true, None) => "  EXT".to_owned(),
                                _ => format!("{:5.1}", seq.tempo),
                            },
                        ),
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
                        t(format!("-{}", crate::seq::track_name(tr, track))),
                        t("  On:"),
                        f(Field::TrackOn, if track.on { "YES" } else { "NO" }),
                        t("  Bus:"),
                        f(Field::Bus, bus_name(track.bus)),
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
                row(
                    vec![t("Sync:"), f(Field::Sync, if s.t.sync { "MIDI CLOCK" } else { "INTERNAL" })],
                    vec![t(match (s.t.sync, s.t.ext.tempo()) {
                        (true, Some(tempo)) => format!("In: {tempo:.1} BPM"),
                        (true, None) => "No clock yet".to_owned(),
                        _ => String::new(),
                    })],
                ),
                row(vec![t("Keyboard pad velocity:"), f(Field::KeyVelocity, s.key_velocity.to_string())], vec![]),
                row(vec![t("MIDI out:"), f(Field::OutPort, v.out_port.chars().take(34).collect::<String>())], vec![]),
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
        Screen::TrMute { solo } => {
            let base = v.bank * 16;
            let mut rows = vec![row(
                vec![t(format!("TRACK MUTE  Sq:{:02}  Bank:{}", s.t.seq + 1, ["A", "B", "C", "D"][v.bank]))],
                vec![t(s.seq().bbt(s.now()))],
            )];
            // Laid out like the pads: tracks 13–16 on top.
            for r in (0..4).rev() {
                let cells = (0..4)
                    .map(|c| {
                        let tr = base + r * 4 + c;
                        let state = match s.t.solo {
                            Some(x) if x == tr => "SOLO",
                            Some(_) => "  --",
                            None if s.seq().tracks[tr].on => "  ON",
                            None => " off",
                        };
                        let used = if s.seq().tracks[tr].notes.is_empty() { ' ' } else { '*' };
                        format!("{:02}{used}{state}", tr + 1)
                    })
                    .collect::<Vec<_>>()
                    .join("   ");
                rows.push(row(vec![t(cells)], vec![]));
            }
            let hint = match (solo, s.t.solo) {
                (true, _) => "Hit the pad of the track to solo",
                (false, Some(_)) => "SOLO: back to all tracks",
                _ if v.message.is_empty() => "Pads mute tracks (* has notes)",
                _ => v.message,
            };
            rows.push(row(vec![t(hint)], vec![t(status(s))]));
            rows
        }
        Screen::NextSeq => {
            let base = v.bank * 16;
            let next = s.t.next_seq.map_or("--".to_owned(), |n| format!("{:02}", n + 1));
            let mut rows = vec![row(
                vec![t(format!("NEXT SEQ  Sq:{:02}  Next:{next}", s.t.seq + 1))],
                vec![t(s.seq().bbt(s.now()))],
            )];
            for r in (0..4).rev() {
                let cells = (0..4)
                    .map(|c| {
                        let sq = base + r * 4 + c;
                        let used = s.seqs[sq].tracks.iter().any(|t| !t.notes.is_empty());
                        let mark = if sq == s.t.seq {
                            '<'
                        } else if Some(sq) == s.t.next_seq {
                            '>'
                        } else if used {
                            '*'
                        } else {
                            ' '
                        };
                        format!("Sq{:02}{mark}", sq + 1)
                    })
                    .collect::<Vec<_>>()
                    .join("    ");
                rows.push(row(vec![t(cells)], vec![]));
            }
            msg(&mut rows);
            if rows[5].left[0].text.is_empty() {
                rows[5].left[0].text = "Pads pick the next sequence".to_owned();
            }
            rows
        }
        Screen::PgmWin => {
            let p = s.program();
            let name = if v.editing { format!("{}_", v.name) } else { s.programs[p].name.clone() };
            let drums =
                s.drums.iter().enumerate().map(|(i, d)| format!("DRUM{}:{:02}", i + 1, d + 1)).collect::<Vec<_>>();
            let mut rows = vec![
                row(
                    vec![
                        t("PROGRAM  Pgm:"),
                        f(Field::Pgm, format!("{:02}", p + 1)),
                        t(format!(" of {}", s.programs.len())),
                    ],
                    vec![],
                ),
                row(vec![t(format!("Name:{name}"))], vec![t(if v.editing { "ENTER: done  ESC: cancel" } else { "" })]),
                row(vec![t(drums.join("  "))], vec![]),
                row(vec![t("NEW: empty program  COPY: copy of this one")], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::SeqWin | Screen::TrWin => {
            use crate::seq::{seq_name, track_name};
            let seq_win = matches!(v.screen, Screen::SeqWin);
            let cursor = if v.editing { "_" } else { "" };
            let name = if v.editing {
                format!("{}{cursor}", v.name)
            } else if seq_win {
                seq_name(s.t.seq, s.seq())
            } else {
                track_name(s.t.track, &s.seq().tracks[s.t.track])
            };
            let target = if seq_win {
                let used = if s.seqs[v.copy_to].tracks.iter().any(|t| !t.notes.is_empty()) { "" } else { " (unused)" };
                format!("-{}{used}", seq_name(v.copy_to, &s.seqs[v.copy_to]))
            } else {
                format!("-{}", track_name(v.copy_to, &s.seq().tracks[v.copy_to]))
            };
            let mut rows = vec![
                if seq_win {
                    row(vec![t("SEQUENCE  Sq:"), f(Field::Seq, format!("{:02}", s.t.seq + 1))], vec![])
                } else {
                    row(
                        vec![
                            t(format!("TRACK  Sq:{:02}  Tr:", s.t.seq + 1)),
                            f(Field::Track, format!("{:02}", s.t.track + 1)),
                        ],
                        vec![],
                    )
                },
                row(vec![t(format!("Name:{name}"))], vec![t(if v.editing { "ENTER: done  ESC: cancel" } else { "" })]),
                row(
                    vec![
                        t(if seq_win { "Copy to sq:" } else { "Copy to tr:" }),
                        f(Field::CopyTo, format!("{:02}", v.copy_to + 1)),
                        t(target),
                    ],
                    vec![],
                ),
                row(
                    vec![t(if seq_win {
                        "COPY replaces it; DELETE clears this one"
                    } else {
                        "COPY replaces it; DELETE clears this track"
                    })],
                    vec![],
                ),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Punch => {
            use crate::audio::PunchMode;
            let p = s.t.punch;
            let seq = s.seq();
            let mut time = vec![t("Time:")];
            if p.mode != PunchMode::Out {
                time.push(f(Field::PunchFrom, seq.bbt(p.from)));
            }
            if p.mode == PunchMode::InOut {
                time.push(t(" - "));
            }
            if p.mode != PunchMode::In {
                time.push(f(Field::PunchTo, seq.bbt(p.to)));
            }
            let mut rows = vec![
                row(
                    vec![t("PUNCH  Auto punch:"), f(Field::PunchMode, p.mode.label())],
                    vec![t(format!("Punch:{}", on_off(p.on)))],
                ),
                row(time, vec![]),
                row(vec![t("Records only inside the punch time while on")], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Trans => {
            let (on, track, by) = s.t.transpose;
            let mut rows = vec![
                row(
                    vec![
                        t("TRANSPOSE  Tr:"),
                        f(Field::TransTrack, track.map_or("ALL".to_owned(), |t| format!("{:02}", t + 1))),
                    ],
                    vec![t(format!("Trans:{}", on_off(on)))],
                ),
                row(vec![t("Transpose:"), f(Field::TransAmount, format!("{by:+}"))], vec![]),
                row(vec![t("Moves the MIDI out notes of the track while playing")], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::SecondSeq => {
            let (on, sq) = s.t.second;
            let used = if s.seqs[sq].tracks.iter().any(|t| !t.notes.is_empty()) { "" } else { " (unused)" };
            let mut rows = vec![
                row(
                    vec![
                        t("2ND SEQUENCE  Sq:"),
                        f(Field::SecondSeq, format!("{:02}", sq + 1)),
                        t(format!("-{}{used}", crate::seq::seq_name(sq, &s.seqs[sq]))),
                    ],
                    vec![t(format!("2nd seq:{}", on_off(on)))],
                ),
                row(vec![t("Plays along with the sequence, at its tempo")], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Velo => {
            let (velo, env) = (s.pads()[v.pad].velo, s.pads()[v.pad].fenv);
            let num = |field, v: u32| f(field, format!("{v:3}"));
            let mut rows = vec![
                row(vec![t("VELO/ENV  Pad:"), f(Field::Pad, pad_name(v.pad))], vec![t(sound_name(v))]),
                row(
                    vec![t("Velo>Level: "), num(Field::VeloLevel, velo.level)],
                    vec![t("Velo>Attack:"), num(Field::VeloAttack, velo.attack)],
                ),
                row(
                    vec![t("Velo>Start: "), num(Field::VeloStart, velo.start)],
                    vec![t("Velo>Filter:"), num(Field::VeloFilter, velo.filter)],
                ),
                row(vec![t(format!("FILTER ENVELOPE  (Filter:{})", s.pads()[v.pad].cutoff))], vec![]),
                row(
                    vec![
                        t("Attack:"),
                        num(Field::EnvAttack, env.attack),
                        t("  Decay:"),
                        num(Field::EnvDecay, env.decay),
                        t("  Amount:"),
                        num(Field::EnvAmount, env.amount),
                    ],
                    vec![],
                ),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Levels => {
            let (pad, var, key) = v.levels;
            let mut rows = vec![
                row(vec![t("ASSIGN 16 LEVELS")], vec![]),
                row(
                    vec![t("Note:"), f(Field::LevelsPad, pad_name(pad)), t(format!("  {}", sound_name_of(s, pad)))],
                    vec![],
                ),
                row(
                    vec![t("Param:"), f(Field::LevelsParam, if var.is_some() { "NOTE VAR" } else { "VELOCITY" })],
                    vec![],
                ),
            ];
            if let Some(param) = var {
                let mut r = vec![t("Type:"), f(Field::LevelsType, param.label())];
                if param == crate::program::VarParam::Tune {
                    r.extend([t("  Original key pad:"), f(Field::LevelsKey, format!("{:2}", key + 1))]);
                }
                rows.push(row(r, vec![]));
            }
            msg(&mut rows);
            rows
        }
        Screen::Assign => {
            use crate::program::VarParam;
            let (pad, param, value) = v.var;
            let base = &s.pads()[pad];
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
                        t(format!("-{}{used}", crate::seq::seq_name(seq, &s.seqs[seq]))),
                    ]
                } else {
                    vec![t(format!(
                        "{current}{:02} Sq:{:02}-{}{used}",
                        i + 1,
                        seq + 1,
                        crate::seq::seq_name(seq, &s.seqs[seq])
                    ))]
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
            // A MIDI track's notes are MIDI notes, a DRUM track's are pads with their sounds.
            let midi = s.midi_track();
            let name = |pad: u8| {
                let sound = s.pads().get(pad as usize).and_then(|p| p.sound.as_ref());
                if midi { String::new() } else { sound.map_or(String::new(), |snd| snd.name.chars().take(8).collect()) }
            };
            let pad_name = |pad: usize| if midi { crate::seq::note_name(pad as u8) } else { pad_name(pad) };
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
            let p = &s.pads()[v.pad];
            let mut rows = vec![
                row(
                    vec![t("PROGRAM  Pad:"), f(Field::Pad, pad_name(v.pad)), t(format!("  {}", sound_name(v)))],
                    vec![
                        t("Pgm:"),
                        f(Field::Pgm, format!("{:02}", s.program() + 1)),
                        t(format!("-{}", s.programs[s.program()].name)),
                    ],
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
                    vec![t("Mode:"), f(Field::GenMode, p.generation.label())],
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
                    match p.generation {
                        Generation::Normal => vec![],
                        Generation::Simult => vec![
                            t("Also:"),
                            f(Field::Also1, mute_text(p.also[0])),
                            t(" "),
                            f(Field::Also2, mute_text(p.also[1])),
                        ],
                        // "Over 64:A02" plays A02 for velocities above 64.
                        Generation::VelSw => vec![
                            t("Over "),
                            f(Field::Switch1, p.switch[0].to_string()),
                            t(":"),
                            f(Field::Also1, mute_text(p.also[0])),
                            t(" "),
                            f(Field::Switch2, p.switch[1].to_string()),
                            t(":"),
                            f(Field::Also2, mute_text(p.also[1])),
                        ],
                    },
                ),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Mixer => {
            let p = &s.pads()[v.pad];
            vec![
                row(
                    vec![t(format!("MIXER  Bank:{}", ["A", "B", "C", "D"][v.pad / 16]))],
                    vec![t(format!("{} {}", pad_name(v.pad), sound_name(v)))],
                ),
                if v.mixer_fx {
                    row(vec![t("FX send:"), f(Field::FxSend, format!("{:3}", p.fx_send))], vec![t(v.message)])
                } else {
                    row(
                        vec![
                            t("Level:"),
                            f(Field::Level, format!("{:3}", p.level)),
                            t("  Pan:"),
                            f(Field::Pan, pan_text(p.pan)),
                        ],
                        vec![t(v.message)],
                    )
                },
            ]
        }
        Screen::Fx => {
            let x = &s.fx;
            let num = |field, v: u32| f(field, format!("{v:3}"));
            let mut rows = vec![
                row(vec![t("EFFECTS on the FX SEND bus")], vec![]),
                row(
                    vec![
                        t("Reverb  Level:"),
                        num(Field::FxReverb, x.reverb),
                        t("  Size:"),
                        num(Field::FxSize, x.size),
                        t("  Damp:"),
                        num(Field::FxDamp, x.damp),
                    ],
                    vec![],
                ),
                row(
                    vec![
                        t("Delay   Level:"),
                        num(Field::FxDelay, x.delay),
                        t("  Time:"),
                        f(Field::FxTime, format!("{:5}", crate::fx::DELAY_TIMES[x.time].0)),
                        t("  Feedback:"),
                        num(Field::FxFeedback, x.feedback),
                    ],
                    vec![],
                ),
                row(vec![t("Pads reach them with FX SEND on the MIXER")], vec![]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Trim => {
            let secs = |frames: usize| format!("{:.3}s", frames as f32 / s.rate as f32);
            let (st, end, len) =
                s.pads()[v.pad].sound.as_ref().map_or((0, 0, 0), |snd| (snd.start, snd.end, snd.frames.len()));
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
        Screen::Loop => {
            let secs = |frames: usize| format!("{:.3}s", frames as f32 / s.rate as f32);
            let (to, end, looping) =
                s.pads()[v.pad].sound.as_ref().map_or((0, 0, false), |snd| (snd.loop_to, snd.end, snd.looping));
            vec![
                row(
                    vec![t("LOOP  Pad:"), f(Field::Pad, pad_name(v.pad)), t(format!("  {}", sound_name(v)))],
                    vec![t("Loop:"), f(Field::Looping, on_off(looping))],
                ),
                row(
                    vec![
                        t("To:"),
                        f(Field::LoopTo, secs(to)),
                        t(" Lngth:"),
                        f(Field::LoopLen, secs(end.saturating_sub(to))),
                        t(" End:"),
                        f(Field::End, secs(end)),
                    ],
                    vec![t(v.message)],
                ),
            ]
        }
        Screen::Zone => {
            let secs = |frames: usize| format!("{:.3}s", frames as f32 / s.rate as f32);
            let (zone, bounds) = v.zone;
            let count = bounds.len().saturating_sub(1);
            let (st, end) = (bounds.get(zone).copied().unwrap_or(0), bounds.get(zone + 1).copied().unwrap_or(0));
            vec![
                row(
                    vec![t("ZONE  Pad:"), f(Field::Pad, pad_name(v.pad)), t(format!("  {}", sound_name(v)))],
                    vec![t("Zones:"), f(Field::ZoneCount, format!("{count:2}"))],
                ),
                row(
                    vec![
                        t("Zone:"),
                        f(Field::Zone, format!("{:2}", zone + 1)),
                        t("  St:"),
                        f(Field::ZoneStart, secs(st)),
                        t("  End:"),
                        f(Field::ZoneEnd, secs(end)),
                    ],
                    vec![t(v.message)],
                ),
            ]
        }
        Screen::Events => {
            let e = v.seq_edit;
            let seq = s.seq();
            let op = match e.op {
                EventOp::Copy => "COPY",
                EventOp::Duration => "DURATION",
                EventOp::Velocity => "VELOCITY",
            };
            let mut rows = vec![
                row(
                    vec![t("EDIT  Edit:"), f(Field::EvOp, op)],
                    vec![
                        t("Sq:"),
                        f(Field::Seq, format!("{:02}", s.t.seq + 1)),
                        t(" Tr:"),
                        f(Field::Track, format!("{:02}", s.t.track + 1)),
                    ],
                ),
                row(
                    vec![t("Notes:"), f(Field::EvPad, e.pad.map_or("ALL".to_owned(), |p| pad_name(p as usize)))],
                    vec![],
                ),
                row(
                    vec![t("Time:"), f(Field::EvFrom, seq.bbt(e.from)), t(" - "), f(Field::EvTo, seq.bbt(e.to))],
                    vec![],
                ),
            ];
            if e.op == EventOp::Copy {
                rows.push(row(
                    vec![
                        t("To sq:"),
                        f(Field::EvToSeq, format!("{:02}", e.to_seq + 1)),
                        t(" tr:"),
                        f(Field::EvToTrack, format!("{:02}", e.to_track + 1)),
                    ],
                    vec![t("Mode:"), f(Field::EvMerge, if e.merge { "MERGE" } else { "REPLACE" })],
                ));
                rows.push(row(
                    vec![t("Start:"), f(Field::EvStart, s.seqs[e.to_seq].bbt(e.start))],
                    vec![t("Copies:"), f(Field::EvCopies, format!("{:3}", e.copies))],
                ));
            } else {
                rows.push(row(vec![t("Mode:"), f(Field::EvMode, e.mode.label())], vec![]));
                rows.push(row(vec![t("Value:"), f(Field::EvValue, format!("{:4}", e.value))], vec![]));
            }
            msg(&mut rows);
            rows
        }
        Screen::Bars => {
            let e = v.seq_edit;
            let mut rows = vec![
                row(vec![t("EDIT BARS  Copy bars")], vec![]),
                row(
                    vec![t("From sq:"), f(Field::Seq, format!("{:02}", s.t.seq + 1))],
                    vec![
                        t("First bar:"),
                        f(Field::FirstBar, format!("{:03}", e.first_bar)),
                        t(" Last bar:"),
                        f(Field::LastBar, format!("{:03}", e.last_bar)),
                    ],
                ),
                row(
                    vec![t("To sq:"), f(Field::EvToSeq, format!("{:02}", e.to_seq + 1))],
                    vec![t("After bar:"), f(Field::AfterBar, format!("{:03}", e.after_bar))],
                ),
                row(vec![], vec![t("Copies:"), f(Field::EvCopies, format!("{:3}", e.copies))]),
            ];
            msg(&mut rows);
            rows
        }
        Screen::Edit(section) => {
            let (op, to, ratio, (rate, bits)) = v.edit;
            let what = match section {
                Section::Trim => "St to End",
                Section::Loop => "loop (To to End)",
                Section::Zone if op == crate::edit::Op::Slice => "every zone",
                Section::Zone => "the selected zone",
            };
            let mut rows = vec![
                row(vec![t(format!("EDIT SOUND  {} {}", pad_name(v.pad), sound_name(v)))], vec![]),
                row(vec![t(format!("Section: {what}"))], vec![]),
                row(vec![t("Edit:"), f(Field::EditOp, op.label())], vec![]),
            ];
            if op == crate::edit::Op::Insert {
                rows.push(row(
                    vec![t("Sound of pad:"), f(Field::EditTo, pad_name(to)), t(format!(" {}", sound_name_of(s, to)))],
                    vec![],
                ));
            }
            if op.makes_sounds() {
                let count = if op == crate::edit::Op::Slice { v.zone.1.len().saturating_sub(1) } else { 1 };
                let last = (to + count - 1).min(crate::program::PADS - 1);
                let range = if count > 1 { format!(" to {}", pad_name(last)) } else { String::new() };
                rows.push(row(vec![t("To pad:"), f(Field::EditTo, pad_name(to)), t(range)], vec![]));
            }
            if op == crate::edit::Op::Resample {
                rows.push(row(
                    vec![
                        t("New fs:"),
                        f(Field::EditRate, format!("{rate}Hz")),
                        t("  Bits:"),
                        f(Field::EditBits, bits.to_string()),
                    ],
                    vec![],
                ));
            }
            if op == crate::edit::Op::TimeStretch {
                let r = row(
                    vec![t("Ratio:"), f(Field::EditRatio, format!("{:.1}%", ratio as f32 / 10.0))],
                    vec![t("of the length, same pitch")],
                );
                rows.push(r);
            }
            msg(&mut rows);
            rows
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
