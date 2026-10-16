mod audio;
mod lcd;
mod panel;
mod program;
mod project;
mod sample;
mod seq;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, Key, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use audio::{Engine, Hit, RecMode, Shared};
use lcd::{Browser, Field, Row, Screen, View};
use panel::*;
use program::{Overlap, PADS, Sound};
use seq::TIMING;

/// Keys for pads 1–16 by physical position, bottom row first: pad 1 is bottom-left, like on the
/// hardware, whatever the keyboard layout.
const PAD_KEYS: [Key; 16] = [
    Key::Z,
    Key::X,
    Key::C,
    Key::V,
    Key::A,
    Key::S,
    Key::D,
    Key::F,
    Key::Q,
    Key::W,
    Key::E,
    Key::R,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
];

const BANKS: [&str; 4] = ["A", "B", "C", "D"];
/// Numeric keypad, top row first, with the mode each key opens.
const KEYPAD: [(&str, &str); 10] = [
    ("7", "SETUP"),
    ("8", "MIXER"),
    ("9", "OTHER"),
    ("4", "SAMPLE"),
    ("5", "TRIM"),
    ("6", "PROGRAM"),
    ("1", "SONG"),
    ("2", "MISC"),
    ("3", "LOAD"),
    ("0", ""),
];
/// Velocity for pads hit from the keyboard, which has no pressure.
const KEY_VELOCITY: u8 = 101;
/// DATA wheel travel (design units of drag or scroll) per step.
const WHEEL_STEP: f32 = 12.0;
const MESSAGE_TIME: Duration = Duration::from_secs(4);

/// LCD text area in design units.
const LCD_X: f32 = 116.0;
const LCD_Y: f32 = 56.0;
const LCD_W: f32 = 338.0;
const ROW_H: f32 = 10.5;

struct App {
    engine: Engine,
    screen: Screen,
    /// Index into the current screen's editable fields.
    focus: usize,
    /// Selected program pad, 0..64: the last one hit.
    pad: usize,
    /// Selected note on the STEP EDIT screen.
    step: usize,
    /// Selected step on the SONG screen.
    song_step: usize,
    /// NOTE VARIATION: the pad the slider affects, what it changes, and the slider (0..=127).
    var_pad: usize,
    var_param: program::VarParam,
    var_value: u8,
    velocity: u8,
    message: String,
    message_at: Instant,
    load_dir: PathBuf,
    bank: usize,
    mouse_held: [bool; 16],
    key_held: [bool; 16],
    /// Program pad each physical pad last sent, so its release goes to the same pad.
    sent: [u8; 16],
    wheel_drag: f32,
    wheel_angle: f32,
    full_level: bool,
    sixteen_levels: bool,
    levels_pad: usize,
    shift: bool,
    taps: Vec<Instant>,
    /// Folder of the open project, once saved or loaded.
    project: Option<PathBuf>,
    save_name: String,
    /// Typing the project name on the SAVE screen.
    editing_name: bool,
    /// The first key typed replaces the whole name, like a selected text field.
    replace_name: bool,
    /// A replace waiting for its second press of ENTER or DO IT.
    confirm: Option<PathBuf>,
    title_changed: bool,
    /// Fingerprint of the project as last saved or loaded, to spot unsaved work.
    saved: u64,
    /// When closing was refused because of unsaved work; closing again soon after quits.
    close_warned: Option<Instant>,
    /// The audio input, open only while the SAMPLE screen is shown.
    input: Option<sample::Input>,
    sample_mono: bool,
    /// dBFS; below −60 means off.
    sample_threshold: i32,
    sample_time: u32,
    /// REC GAIN knob, 0..1 for −12..+12 dB.
    rec_gain: f32,
    samples_taken: u32,
}

/// Something the keyboard asked for this frame.
enum Action {
    Pad(usize, bool),
    PlayStop,
    PlayStart,
    Rec(RecMode),
    Undo,
    Cursor(i32, i32),
    Data(i32),
    Enter,
    Main,
    Back,
    Soft(usize),
    Mode(&'static str),
    Step(i32),
    Bar(i32),
    Start,
    Tap,
    Save,
}

impl App {
    fn new() -> Self {
        let (engine, error) = Engine::start();
        {
            let mut s = audio::lock(&engine.shared);
            let rate = s.rate;
            for (pad, sound) in audio::builtin_kit(rate).into_iter().enumerate() {
                s.pads[pad].sound = Some(sound);
            }
        }
        let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let music = home.join("Music");
        let saved = project::fingerprint(&audio::lock(&engine.shared));
        Self {
            engine,
            screen: Screen::Main,
            focus: 0,
            pad: 0,
            step: 0,
            song_step: 0,
            var_pad: 0,
            var_param: program::VarParam::Tune,
            var_value: 64,
            velocity: 0,
            message: error.unwrap_or_default(),
            message_at: Instant::now(),
            load_dir: if music.is_dir() { music } else { home },
            bank: 0,
            mouse_held: [false; 16],
            key_held: [false; 16],
            sent: [0; 16],
            wheel_drag: 0.0,
            wheel_angle: 0.0,
            full_level: false,
            sixteen_levels: false,
            levels_pad: 0,
            shift: false,
            taps: Vec::new(),
            project: None,
            save_name: "Project01".to_owned(),
            editing_name: false,
            replace_name: false,
            confirm: None,
            title_changed: false,
            saved,
            close_warned: None,
            input: None,
            sample_mono: false,
            sample_threshold: -40,
            sample_time: 4,
            rec_gain: 0.5,
            samples_taken: 0,
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut Shared) -> R) -> R {
        f(&mut audio::lock(&self.engine.shared))
    }

    fn say(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_at = Instant::now();
    }

    fn pad_press(&mut self, n: usize, velocity: u8) {
        let (pad, velocity) = if self.sixteen_levels {
            // 16 LEVELS: every pad plays the chosen sound, from velocity 7 on pad 1 to 127 on pad 16.
            (self.levels_pad, (n * 8 + 7) as u8)
        } else {
            (self.bank * 16 + n, if self.full_level { 127 } else { velocity })
        };
        self.sent[n] = pad as u8;
        self.velocity = velocity;
        if !self.sixteen_levels {
            self.pad = pad;
        }
        let var = (pad == self.var_pad).then_some((self.var_param, self.var_value));
        self.with(|s| s.hits.push(Hit::Press { pad: pad as u8, velocity, var }));
        if matches!(self.screen, Screen::Step) {
            self.step_record(pad as u8, velocity, var);
        }
    }

    /// STEP EDIT: a pad hit while stopped adds a note at the current position.
    fn step_record(&mut self, pad: u8, velocity: u8, var: program::Variation) {
        let added = self.with(|s| {
            if s.t.playing {
                return None;
            }
            let (tick, tr) = (s.now(), s.t.track);
            let duration = if s.grid() > 1 { s.grid() } else { seq::PPQ / 4 };
            let note = seq::Note { tick, pad, velocity, duration, var };
            let track = &mut s.seq_mut().tracks[tr];
            track.insert(note);
            track.notes.iter().rposition(|n| *n == note)
        });
        if let Some(i) = added {
            self.step = i;
        }
    }

    /// Selects the `i`th note of the current track on STEP EDIT and moves Now to it.
    fn select_note(&mut self, i: isize) {
        self.step = self.with(|s| {
            let tr = s.t.track;
            let notes = &s.seq().tracks[tr].notes;
            let i = i.clamp(0, notes.len().saturating_sub(1) as isize) as usize;
            if let Some(n) = notes.get(i) {
                let tick = n.tick;
                s.locate(tick);
            }
            i
        });
    }

    /// Changes a field of the selected note on STEP EDIT, keeping the track sorted.
    fn edit_note(&mut self, field: Field, d: i32) {
        let (sel, fine) = (self.step, self.shift);
        let moved = self.with(|s| {
            let (tr, len, grid) = (s.t.track, s.seq().len(), s.grid());
            let track = &mut s.seq_mut().tracks[tr];
            let mut n = *track.notes.get(sel)?;
            let unit = if fine { 1 } else { grid } as i64;
            match field {
                Field::NotePos => n.tick = (n.tick as i64 + d as i64 * unit).rem_euclid(len as i64) as u32,
                Field::NotePad => n.pad = (n.pad as i32 + d).clamp(0, PADS as i32 - 1) as u8,
                Field::NoteVel => n.velocity = (n.velocity as i32 + d).clamp(1, 127) as u8,
                _ => n.duration = (n.duration as i64 + d as i64 * unit).clamp(1, len as i64) as u32,
            }
            track.notes.remove(sel);
            track.insert(n);
            let i = track.notes.iter().rposition(|m| *m == n)?;
            s.locate(n.tick);
            Some(i)
        });
        if let Some(i) = moved {
            self.step = i;
        }
    }

    fn pad_release(&mut self, n: usize) {
        let pad = self.sent[n];
        self.with(|s| s.hits.push(Hit::Release { pad }));
    }

    fn set_screen(&mut self, screen: Screen) {
        if let Screen::Load(b) | Screen::Save(b) = &self.screen {
            self.load_dir = b.dir.clone();
        }
        self.screen = screen;
        if !matches!(self.screen, Screen::Sample) {
            // Closes the input device, so it is only in use on the SAMPLE screen.
            self.input = None;
        }
        self.editing_name = false;
        self.confirm = None;
        self.focus = 0;
        self.message.clear();
    }

    fn rows(&self, s: &Shared) -> Vec<Row> {
        lcd::rows(&View {
            s,
            screen: &self.screen,
            pad: self.pad,
            velocity: self.velocity,
            message: &self.message,
            name: &self.save_name,
            editing: self.editing_name,
            step: self.step,
            song_step: self.song_step,
            var: (self.var_pad, self.var_param, self.var_value),
            sample: self.input.as_ref().map(|input| {
                let c = input.lock();
                lcd::SampleView {
                    input: input.name.clone(),
                    state: c.state,
                    seconds: c.frames.len() as f32 / c.rate as f32,
                    mono: self.sample_mono,
                    threshold: self.sample_threshold,
                    time: self.sample_time,
                    gain_db: self.rec_gain * 24.0 - 12.0,
                }
            }),
        })
    }

    fn fields(&self) -> Vec<(usize, Field)> {
        lcd::fields(&self.with(|s| self.rows(s)))
    }

    fn cursor(&mut self, dx: i32, dy: i32) {
        match &mut self.screen {
            Screen::Load(b) | Screen::Save(b) => {
                self.confirm = None;
                if dy != 0 {
                    b.cursor = b.cursor.saturating_add_signed(dy as isize).min(b.entries.len().saturating_sub(1));
                }
                if dx < 0 {
                    self.parent_dir();
                } else if dx > 0 {
                    self.enter();
                }
            }
            Screen::Mixer if dx != 0 => {
                self.pad = self.pad / 16 * 16 + (self.pad as i32 % 16 + dx).rem_euclid(16) as usize
            }
            // Level and Pan share one row; up selects Level, down selects Pan.
            Screen::Mixer => self.focus = (dy > 0) as usize,
            Screen::Song if dy != 0 => {
                let len = self.with(|s| s.song.steps.len());
                self.song_step = (self.song_step as i32 + dy).clamp(0, len.saturating_sub(1) as i32) as usize;
                self.focus = self.focus.max(1);
            }
            // Up and down pick the note; left and right move between its fields.
            Screen::Step if dy != 0 => {
                self.select_note(self.step as isize + dy as isize);
                self.focus = self.focus.max(1);
            }
            _ => {
                let fields = self.fields();
                if fields.is_empty() {
                    return;
                }
                let cur = self.focus.min(fields.len() - 1);
                self.focus = if dy != 0 {
                    // Jump to the first field of the next row with fields in that direction.
                    let row = fields[cur].0 as i32;
                    let target = fields
                        .iter()
                        .map(|(r, _)| *r as i32)
                        .filter(|r| (r - row).signum() == dy.signum())
                        .min_by_key(|r| (r - row).abs());
                    target.and_then(|t| fields.iter().position(|(r, _)| *r as i32 == t)).unwrap_or(cur)
                } else {
                    (cur as i32 + dx).clamp(0, fields.len() as i32 - 1) as usize
                };
            }
        }
    }

    /// Turns the DATA wheel `steps` clicks.
    fn data(&mut self, steps: i32) {
        if steps == 0 {
            return;
        }
        if let Screen::Load(_) | Screen::Save(_) = self.screen {
            return self.cursor(0, steps);
        }
        let fields = self.fields();
        if let Some(&(_, field)) = fields.get(self.focus.min(fields.len().saturating_sub(1))) {
            self.adjust(field, steps);
        }
    }

    fn adjust(&mut self, field: Field, d: i32) {
        let fine = self.shift;
        let pad = self.pad;
        let step = |v: u32, lo: u32, hi: u32| (v as i32 + d).clamp(lo as i32, hi as i32) as u32;
        if field == Field::Pad {
            self.pad = (pad as i32 + d).clamp(0, PADS as i32 - 1) as usize;
            return;
        }
        if matches!(field, Field::NotePos | Field::NotePad | Field::NoteVel | Field::NoteDur) {
            return self.edit_note(field, d);
        }
        if field == Field::Track {
            self.step = 0;
        }
        let song_step = self.song_step;
        match field {
            Field::VarPad => return self.var_pad = (self.var_pad as i32 + d).clamp(0, PADS as i32 - 1) as usize,
            Field::VarParam => {
                let all = program::VarParam::ALL;
                let i = all.iter().position(|p| *p == self.var_param).unwrap_or(0) as i32;
                return self.var_param = all[(i + d).rem_euclid(all.len() as i32) as usize];
            }
            Field::SongLoop => return self.with(|s| s.song.looping ^= true),
            Field::SongSeq | Field::SongReps => {
                return self.with(|s| {
                    if let Some(step) = s.song.steps.get_mut(song_step) {
                        if field == Field::SongSeq {
                            step.0 = (step.0 as i32 + d).clamp(0, seq::SEQUENCES as i32 - 1) as usize;
                        } else {
                            step.1 = (step.1 as i32 + d).clamp(1, 99) as u32;
                        }
                    }
                });
            }
            Field::SampleMode => return self.sample_mono ^= true,
            Field::Threshold => return self.sample_threshold = (self.sample_threshold + d).clamp(-61, 0),
            Field::SampleTime => return self.sample_time = (self.sample_time as i32 + d).clamp(1, 60) as u32,
            _ => {}
        }
        self.with(|s| {
            let grid = s.grid();
            let p = &mut s.pads[pad];
            match field {
                Field::Seq => s.select_seq(step(s.t.seq as u32, 0, seq::SEQUENCES as u32 - 1) as usize),
                Field::Tempo => {
                    let tempo = s.seq().tempo + d as f32 * if fine { 0.1 } else { 1.0 };
                    s.seq_mut().tempo = (tempo.clamp(30.0, 300.0) * 10.0).round() / 10.0;
                }
                Field::Timing => s.t.timing = step(s.t.timing as u32, 0, TIMING.len() as u32 - 1) as usize,
                Field::Swing => s.t.swing = step(s.t.swing, 50, 75),
                Field::Count => s.t.count_in = !s.t.count_in,
                Field::Click => s.t.click = !s.t.click,
                Field::Loop => s.seq_mut().looping = !s.seq().looping,
                Field::Bars => {
                    s.checkpoint();
                    let bars = step(s.seq().bars, 1, 999);
                    s.seq_mut().set_bars(bars);
                    s.locate(s.now());
                }
                Field::Beats => {
                    s.checkpoint();
                    let (beats, unit) = (step(s.seq().beats, 1, 32), s.seq().beat_unit);
                    s.seq_mut().set_signature(beats, unit);
                    s.locate(s.now());
                }
                Field::BeatUnit => {
                    s.checkpoint();
                    const UNITS: [u32; 4] = [2, 4, 8, 16];
                    let i = UNITS.iter().position(|&u| u == s.seq().beat_unit).unwrap_or(1);
                    let unit = UNITS[(i as i32 + d).clamp(0, 3) as usize];
                    let beats = s.seq().beats;
                    s.seq_mut().set_signature(beats, unit);
                    s.locate(s.now());
                }
                Field::Track => s.t.track = step(s.t.track as u32, 0, seq::TRACKS as u32 - 1) as usize,
                Field::TrackOn => {
                    let tr = s.t.track;
                    s.seq_mut().tracks[tr].on ^= true;
                }
                Field::Now if !s.t.playing => {
                    let now = (s.now() as i64 + d as i64 * grid as i64).max(0) as u32;
                    s.locate(now);
                }
                Field::Now => {}
                Field::Tune => p.tune = (p.tune + d * if fine { 1 } else { 10 }).clamp(-120, 120),
                Field::Attack => p.attack = step(p.attack, 0, 100),
                Field::Decay => p.decay = step(p.decay, 0, 100),
                Field::DecayMode => p.decay_from_start ^= true,
                Field::Cutoff => p.cutoff = step(p.cutoff, 0, 100),
                Field::Reso => p.resonance = step(p.resonance, 0, 100),
                Field::Overlap => {
                    const ORDER: [Overlap; 3] = [Overlap::Poly, Overlap::Mono, Overlap::NoteOff];
                    let i = ORDER.iter().position(|&o| o == p.overlap).unwrap_or(0) as i32;
                    p.overlap = ORDER[(i + d).rem_euclid(3) as usize];
                }
                Field::Mute1 | Field::Mute2 => {
                    let m = &mut p.mutes[(field == Field::Mute2) as usize];
                    // −1 stands for "off", before A01.
                    let v = (m.map_or(-1, |p| p as i32) + d).clamp(-1, PADS as i32 - 1);
                    *m = (v >= 0 && v as usize != pad).then_some(v as u8);
                }
                Field::Level => p.level = step(p.level, 0, 100),
                Field::Pan => p.pan = (p.pan + d).clamp(-50, 50),
                Field::Start | Field::End => {
                    if let Some(snd) = &mut p.sound {
                        let len = snd.frames.len();
                        let unit = if fine { 1 } else { (len / 500).max(1) } as i64;
                        let moved = |v: usize| (v as i64 + d as i64 * unit).clamp(0, len as i64) as usize;
                        if field == Field::Start {
                            snd.start = moved(snd.start).min(snd.end.saturating_sub(1));
                        } else {
                            snd.end = moved(snd.end).max(snd.start + 1);
                        }
                    }
                }
                Field::Pad
                | Field::NotePos
                | Field::NotePad
                | Field::NoteVel
                | Field::NoteDur
                | Field::SampleMode
                | Field::Threshold
                | Field::SampleTime
                | Field::SongLoop
                | Field::SongSeq
                | Field::SongReps
                | Field::VarPad
                | Field::VarParam => {}
            }
        });
    }

    fn parent_dir(&mut self) {
        let wavs = matches!(self.screen, Screen::Load(_));
        if let Screen::Load(b) | Screen::Save(b) = &mut self.screen
            && let Some(parent) = b.dir.parent().map(PathBuf::from)
        {
            let from = std::mem::replace(b, Browser::open(parent, wavs)).dir;
            b.cursor = b.entries.iter().position(|e| *e == from).unwrap_or(0);
        }
    }

    /// ENTER: on the LOAD screen opens a folder, loads a project, or loads a WAV into the selected
    /// pad; on the SAVE screen opens a folder or picks an existing project's name.
    fn enter(&mut self) {
        let saving = matches!(self.screen, Screen::Save(_));
        let (Screen::Load(b) | Screen::Save(b)) = &mut self.screen else { return };
        let Some(path) = b.entries.get(b.cursor).cloned() else { return };
        if project::is_project(&path) {
            if saving {
                self.save_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            } else {
                self.load_project(path);
            }
            return;
        }
        if path.is_dir() {
            *b = Browser::open(path, !saving);
            return;
        }
        let pad = self.pad;
        let rate = self.with(|s| s.rate);
        match audio::load_wav(&path, rate) {
            Ok(frames) if frames.is_empty() => self.say("Empty file"),
            Ok(frames) => {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                self.with(|s| {
                    s.pads[pad].sound = Some(Sound::new(name, frames));
                    s.hits.push(Hit::Press { pad: pad as u8, velocity: KEY_VELOCITY, var: None });
                });
                self.say("Loaded");
            }
            Err(e) => self.say(e.to_string()),
        }
    }

    /// Loading replaces everything, so it takes a second ENTER to confirm.
    fn load_project(&mut self, path: PathBuf) {
        if self.confirm.as_ref() != Some(&path) {
            self.say("Replaces everything. ENTER again to load");
            self.confirm = Some(path);
            return;
        }
        self.confirm = None;
        match self.with(|s| project::load(s, &path)) {
            Ok(()) => {
                self.saved = self.with(|s| project::fingerprint(s));
                self.save_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                self.project = Some(path);
                self.title_changed = true;
                (self.pad, self.bank, self.focus) = (0, 0, 0);
                self.set_screen(Screen::Main);
                self.say(format!("Loaded {}", self.save_name));
            }
            Err(e) => self.say(e),
        }
    }

    /// Ctrl+S: saves over the open project, or opens the SAVE screen for a new one.
    fn quick_save(&mut self) {
        match self.project.clone() {
            Some(root) => {
                let dir = root.parent().map(PathBuf::from).unwrap_or_default();
                let result = self.with(|s| project::save(s, &dir, &self.save_name).map(|_| project::fingerprint(s)));
                match result {
                    Ok(saved) => {
                        self.saved = saved;
                        self.say(format!("Saved {}", self.save_name));
                    }
                    Err(e) => self.say(e),
                }
            }
            None => self.open_save(),
        }
    }

    fn open_save(&mut self) {
        let dir =
            self.project.as_ref().and_then(|p| p.parent()).map(PathBuf::from).unwrap_or_else(|| self.load_dir.clone());
        self.set_screen(Screen::Save(Browser::open(dir, false)));
    }

    /// DO IT on the SAVE screen. Overwriting an existing project asks for a second press.
    fn save_here(&mut self) {
        let Screen::Save(b) = &self.screen else { return };
        let name = self.save_name.trim().to_owned();
        if name.is_empty() {
            return self.say("Type a name first (NAME)");
        }
        let dir = b.dir.clone();
        let target = dir.join(&name);
        if project::is_project(&target) && self.confirm.as_ref() != Some(&target) {
            self.say("Exists. DO IT again to replace");
            self.confirm = Some(target);
            return;
        }
        self.confirm = None;
        let result = self.with(|s| project::save(s, &dir, &name).map(|root| (root, project::fingerprint(s))));
        match result {
            Ok((root, saved)) => {
                self.saved = saved;
                self.project = Some(root);
                self.save_name = name;
                self.title_changed = true;
                let Screen::Save(b) = &mut self.screen else { return };
                *b = Browser::open(dir, false);
                self.say(format!("Saved {}", self.save_name));
            }
            Err(e) => self.say(e),
        }
    }

    /// Typing the project name: letters go to the name instead of the pads.
    fn type_name(&mut self, events: &[egui::Event]) {
        for e in events {
            match e {
                egui::Event::Text(text) => {
                    if std::mem::take(&mut self.replace_name) {
                        self.save_name.clear();
                    }
                    let allowed = text.chars().filter(|c| c.is_alphanumeric() || " -_.()".contains(*c));
                    self.save_name.extend(allowed);
                    self.save_name
                        .truncate(self.save_name.char_indices().nth(40).map_or(self.save_name.len(), |(i, _)| i));
                }
                egui::Event::Key { key: Key::Backspace, pressed: true, .. } => {
                    self.replace_name = false;
                    self.save_name.pop();
                }
                egui::Event::Key { key: Key::Enter | Key::Escape, pressed: true, .. } => self.editing_name = false,
                _ => {}
            }
        }
    }

    fn soft_key(&mut self, i: usize) {
        let pad = self.pad;
        match (&self.screen, i) {
            (Screen::Main, 0) => {
                // Edits on this screen can be undone together with UNDO SEQ.
                let first = self.with(|s| {
                    s.checkpoint();
                    let now = s.now();
                    s.seq().tracks[s.t.track].notes.partition_point(|n| n.tick < now)
                });
                self.set_screen(Screen::Step);
                self.select_note(first as isize);
                self.focus = 1;
            }
            (Screen::Step, 0) => {
                let sel = self.step;
                self.with(|s| {
                    let tr = s.t.track;
                    let notes = &mut s.seq_mut().tracks[tr].notes;
                    if sel < notes.len() {
                        notes.remove(sel);
                    }
                });
                self.select_note(sel as isize);
            }
            (Screen::Step, 4) => {
                self.with(|s| s.t.track = s.t.track.saturating_sub(1));
                self.select_note(0);
            }
            (Screen::Step, 5) => {
                self.with(|s| s.t.track = (s.t.track + 1).min(seq::TRACKS - 1));
                self.select_note(0);
            }
            (Screen::Main, 1) => self.with(|s| s.undo()),
            (Screen::Main, 2) => self.with(|s| {
                let tr = s.t.track;
                s.seq_mut().tracks[tr].on ^= true;
            }),
            (Screen::Main, 3) => {
                self.with(|s| s.t.solo = if s.t.solo == Some(s.t.track) { None } else { Some(s.t.track) })
            }
            (Screen::Main, 4) => self.with(|s| s.t.track = s.t.track.saturating_sub(1)),
            (Screen::Main, 5) => self.with(|s| s.t.track = (s.t.track + 1).min(seq::TRACKS - 1)),
            (Screen::Load(_) | Screen::Save(_), 0) => self.parent_dir(),
            (Screen::Load(_), 1) => self.open_save(),
            (Screen::Load(_), 5) => self.enter(),
            (Screen::Save(_), 4) => (self.editing_name, self.replace_name) = (true, true),
            (Screen::Save(_), 5) => self.save_here(),
            (Screen::Program, 0) => self.mode("LOAD"),
            (Screen::Program, 1) => self.mode("TRIM"),
            (Screen::Program, 2) => self.mode("MIXER"),
            (Screen::Program, 5) => {
                self.with(|s| s.pads[pad].sound = None);
                self.say("Sound removed");
            }
            (Screen::Sample, 4 | 5) => self.sample_key(i),
            (Screen::Song, 0) => {
                // A new step after the selected one, playing the sequence of the MAIN screen.
                let at = self.with(|s| {
                    let at = (self.song_step + 1).min(s.song.steps.len());
                    let seq = s.t.seq;
                    s.song.steps.insert(at, (seq, 1));
                    at
                });
                self.song_step = at;
                self.focus = 1;
            }
            (Screen::Song, 1) => {
                let sel = self.song_step;
                let left = self.with(|s| {
                    if s.t.song.is_none() && sel < s.song.steps.len() {
                        s.song.steps.remove(sel);
                    }
                    s.song.steps.len()
                });
                self.song_step = sel.min(left.saturating_sub(1));
            }
            (Screen::Mixer, 5) => self.with(|s| (s.pads[pad].level, s.pads[pad].pan) = (100, 0)),
            (Screen::Trim, 0) => self.with(|s| s.hits.push(Hit::Press { pad: pad as u8, velocity: 127, var: None })),
            (Screen::Trim, 5) => self.with(|s| {
                if let Some(snd) = &mut s.pads[pad].sound {
                    (snd.start, snd.end) = (0, snd.frames.len());
                }
            }),
            _ => {}
        }
    }

    /// SAMPLE soft keys: REC arms and STOP ends a take; then RETRY (F5) or KEEP (F6).
    fn sample_key(&mut self, i: usize) {
        use sample::State;
        let Some(input) = &self.input else { return };
        let mut c = input.lock();
        match (c.state, i) {
            (State::Idle, 5) => c.arm(),
            (State::Armed | State::Recording, 5) => c.stop(),
            (State::Done, 4) => c.arm(),
            (State::Done, 5) => {
                let (frames, from) = (std::mem::take(&mut c.frames), c.rate);
                c.state = State::Idle;
                drop(c);
                self.keep_sample(&frames, from);
            }
            _ => {}
        }
    }

    fn keep_sample(&mut self, frames: &[[f32; 2]], from: u32) {
        self.samples_taken += 1;
        let name = format!("SAMPLE{:02}", self.samples_taken);
        let pad = self.pad;
        self.with(|s| {
            let frames = audio::resample(frames, from, s.rate);
            s.pads[pad].sound = Some(Sound::new(name.clone(), frames.into()));
            s.hits.push(Hit::Press { pad: pad as u8, velocity: KEY_VELOCITY, var: None });
        });
        self.say(format!("Kept {name} on {}", program::pad_name(pad)));
    }

    /// Keeps the input's settings in step with the screen and the REC GAIN knob.
    fn sync_input(&self) {
        if let Some(input) = &self.input {
            let mut c = input.lock();
            c.gain = 10f32.powf((self.rec_gain * 24.0 - 12.0) / 20.0);
            c.mono = self.sample_mono;
            c.threshold =
                if self.sample_threshold < -60 { 0.0 } else { 10f32.powf(self.sample_threshold as f32 / 20.0) };
            c.max_frames = self.sample_time as usize * c.rate as usize;
        }
    }

    /// PLAY / PLAY START. On the SONG screen they play the song, from the selected step or the
    /// first one.
    fn play(&mut self, from_start: bool) {
        if matches!(self.screen, Screen::Song) {
            let step = if from_start { 0 } else { self.song_step };
            if !self.with(|s| s.play_song(step)) && !self.with(|s| s.t.playing) {
                self.say("The song is empty: INSERT adds a step");
            }
        } else {
            self.with(|s| s.play(from_start));
        }
    }

    /// T/C: quantizes the current track to the Timing setting.
    fn timing_correct(&mut self) {
        let msg = self.with(|s| {
            if s.t.playing {
                return "Stop first".to_owned();
            }
            s.checkpoint();
            let (grid, swing, len, tr) = (s.grid(), s.t.swing, s.seq().len(), s.t.track);
            let track = &mut s.seq_mut().tracks[tr];
            let notes = std::mem::take(&mut track.notes);
            for mut n in notes {
                n.tick = seq::quantize(n.tick, grid, swing, len);
                track.insert(n);
            }
            format!("Track {:02} corrected to {}", tr + 1, TIMING[s.t.timing].0)
        });
        self.say(msg);
    }

    fn mode(&mut self, name: &str) {
        match name {
            // SHIFT + LOAD is SAVE.
            "LOAD" if self.shift => self.open_save(),
            "LOAD" => self.set_screen(Screen::Load(Browser::open(self.load_dir.clone(), true))),
            "TRIM" => self.set_screen(Screen::Trim),
            "PROGRAM" => self.set_screen(Screen::Program),
            "MIXER" => self.set_screen(Screen::Mixer),
            "SONG" => {
                self.set_screen(Screen::Song);
                self.focus = 1;
            }
            "SAMPLE" => {
                self.set_screen(Screen::Sample);
                match sample::Input::open() {
                    Ok(input) => self.input = Some(input),
                    Err(e) => self.say(e),
                }
            }
            "" => {}
            other => self.say(format!("{other} is not available yet")),
        }
    }

    fn tap_tempo(&mut self) {
        let now = Instant::now();
        if self.taps.last().is_some_and(|t| now - *t > Duration::from_secs(2)) {
            self.taps.clear();
        }
        self.taps.push(now);
        if self.taps.len() > 4 {
            self.taps.remove(0);
        }
        if let (Some(first), Some(last)) = (self.taps.first(), self.taps.last())
            && self.taps.len() >= 2
        {
            let beat = (*last - *first).as_secs_f32() / (self.taps.len() - 1) as f32;
            let tempo = ((60.0 / beat).clamp(30.0, 300.0) * 10.0).round() / 10.0;
            self.with(|s| s.seq_mut().tempo = tempo);
        }
    }

    fn locate_step(&mut self, dir: i32) {
        self.with(|s| {
            let grid = s.grid() as i64;
            let now = s.now() as i64;
            // Snap to the grid first, so stepping from between grid lines lands on one.
            let target = if dir < 0 { (now - 1).div_euclid(grid) * grid } else { (now / grid + 1) * grid };
            s.locate(target.max(0) as u32);
        });
    }

    fn locate_bar(&mut self, dir: i32) {
        self.with(|s| {
            let bar = s.seq().ticks_per_bar() as i64;
            let now = s.now() as i64;
            let target = if dir < 0 { (now - 1).div_euclid(bar) * bar } else { (now / bar + 1) * bar };
            s.locate(target.max(0) as u32);
        });
    }

    fn run(&mut self, action: Action) {
        match action {
            Action::Pad(n, true) => self.pad_press(n, KEY_VELOCITY),
            Action::Pad(n, false) => self.pad_release(n),
            Action::PlayStop => {
                if self.with(|s| s.t.playing) {
                    self.with(|s| s.stop());
                } else {
                    self.play(false);
                }
            }
            Action::PlayStart => self.play(true),
            Action::Rec(mode) => self.with(|s| s.toggle_rec(mode)),
            Action::Undo => self.with(|s| s.undo()),
            Action::Cursor(dx, dy) => self.cursor(dx, dy),
            Action::Data(d) => self.data(d),
            Action::Enter => self.enter(),
            Action::Main => self.set_screen(Screen::Main),
            Action::Back => self.parent_dir(),
            Action::Soft(i) => self.soft_key(i),
            Action::Mode(m) => self.mode(m),
            Action::Step(d) => self.locate_step(d),
            Action::Bar(d) => self.locate_bar(d),
            Action::Start => self.with(|s| s.locate(0)),
            Action::Tap => {
                if !self.with(|s| s.t.playing) {
                    self.tap_tempo();
                }
            }
            Action::Save => self.quick_save(),
        }
    }

    fn keyboard(&mut self, ui: &Ui) {
        let (events, focused, erase, repeat) =
            ui.input(|i| (i.events.clone(), i.focused, i.key_down(Key::Delete), i.key_down(Key::T)));
        self.shift = ui.input(|i| i.modifiers.shift);
        if self.editing_name {
            return self.type_name(&events);
        }
        let mut actions = Vec::new();
        for e in events {
            let egui::Event::Key { key, physical_key, pressed, repeat, modifiers } = e else { continue };
            let command = modifiers.ctrl || modifiers.command || modifiers.alt;
            // Pads follow physical positions, so QWERTZ and AZERTY keyboards get the same grid.
            // With a modifier held the key is a shortcut or a symbol instead (Shift+3 is `+` on a
            // Hungarian keyboard), but a pad still gets its release.
            if let Some(n) = PAD_KEYS.iter().position(|k| *k == physical_key.unwrap_or(key))
                && ((!command && !modifiers.shift) || (!pressed && self.key_held[n]))
            {
                if !repeat && pressed != self.key_held[n] {
                    self.key_held[n] = pressed;
                    actions.push(Action::Pad(n, pressed));
                }
                continue;
            }
            if !pressed {
                continue;
            }
            let action = match key {
                Key::Space if modifiers.shift => Action::PlayStart,
                Key::Space if !repeat => Action::PlayStop,
                Key::R if modifiers.ctrl => Action::Rec(RecMode::Rec),
                Key::O if modifiers.ctrl => Action::Rec(RecMode::Overdub),
                Key::Z if modifiers.ctrl => Action::Undo,
                Key::S if modifiers.ctrl => Action::Save,
                Key::Num1 if command => Action::Mode("SONG"),
                Key::Num3 if command => Action::Mode("LOAD"),
                Key::Num4 if command => Action::Mode("SAMPLE"),
                Key::Num5 if command => Action::Mode("TRIM"),
                Key::Num6 if command => Action::Mode("PROGRAM"),
                Key::Num8 if command => Action::Mode("MIXER"),
                Key::ArrowLeft => Action::Cursor(-1, 0),
                Key::ArrowRight => Action::Cursor(1, 0),
                Key::ArrowUp => Action::Cursor(0, -1),
                Key::ArrowDown => Action::Cursor(0, 1),
                Key::Plus | Key::Equals => Action::Data(1),
                Key::Minus => Action::Data(-1),
                Key::Enter => Action::Enter,
                Key::Escape => Action::Main,
                Key::Backspace => Action::Back,
                Key::F1 => Action::Soft(0),
                Key::F2 => Action::Soft(1),
                Key::F3 => Action::Soft(2),
                Key::F4 => Action::Soft(3),
                Key::F5 => Action::Soft(4),
                Key::F6 => Action::Soft(5),
                Key::Comma => Action::Step(-1),
                Key::Period => Action::Step(1),
                Key::PageUp => Action::Bar(-1),
                Key::PageDown => Action::Bar(1),
                Key::Home => Action::Start,
                Key::T if !repeat => Action::Tap,
                _ => continue,
            };
            actions.push(action);
        }
        // Keys released while the window was unfocused never arrive; let go of them.
        if !focused {
            for n in 0..16 {
                if std::mem::take(&mut self.key_held[n]) {
                    actions.push(Action::Pad(n, false));
                }
            }
        }
        for a in actions {
            self.run(a);
        }
        self.with(|s| {
            s.t.erase = erase;
            // While playing, holding T is NOTE REPEAT; while stopped it taps the tempo.
            s.t.repeat = repeat && s.t.playing;
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if !self.message.is_empty() && self.message_at.elapsed() > MESSAGE_TIME {
            self.message.clear();
        }
        self.keyboard(ui);
        self.guard_close(ui);
        if std::mem::take(&mut self.title_changed) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(format!("rsampler - {}", self.save_name)));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BACKDROP)).show(ui, |ui| self.panel(ui));
        // Keep the LCD position and LEDs moving while the sequencer runs.
        self.sync_input();
        if self.with(|s| s.t.playing) || !self.message.is_empty() || self.input.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(30));
        }
    }
}

impl App {
    /// Closing with unsaved work is refused once with a warning; closing again quits.
    fn guard_close(&mut self, ui: &Ui) {
        if !ui.input(|i| i.viewport().close_requested()) {
            return;
        }
        let warned = self.close_warned.is_some_and(|t| t.elapsed() < Duration::from_secs(10));
        if !warned && self.with(|s| project::fingerprint(s)) != self.saved {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_warned = Some(Instant::now());
            self.say("Unsaved! Close again to quit, Ctrl+S saves");
        }
    }

    fn panel(&mut self, ui: &mut Ui) {
        let area = ui.max_rect();
        let scale = (area.width() / W).min(area.height() / H);
        let p = Panel { origin: area.center() - vec2(W, H) * scale / 2.0, scale };
        let s = scale;
        let painter = ui.painter().clone();

        painter.rect_filled(p.rect(15.0, 15.0, 970.0, 700.0), 6.0 * s, BODY_EDGE);
        painter.rect_filled(p.rect(15.0, 15.0, 970.0, 694.0), 6.0 * s, BODY);

        self.lcd(ui, &painter, &p);
        for i in 0..6 {
            if key(ui, &p, p.rect(167.0 + i as f32 * 41.0, 212.0, 28.0, 14.0), &WHITE, &format!("F{}", i + 1), false)
                .clicked()
            {
                self.soft_key(i);
            }
        }

        self.left_controls(ui, &painter, &p);
        self.transport(ui, &painter, &p);
        self.upper_panel(ui, &painter, &p);
        self.pads(ui, &painter, &p);
    }

    fn lcd(&mut self, ui: &mut Ui, painter: &egui::Painter, p: &Panel) {
        let s = p.scale;
        painter.rect_filled(p.rect(78.0, 28.0, 412.0, 152.0), 6.0 * s, HOUSING);
        painter.rect_filled(p.rect(108.0, 44.0, 354.0, 106.0), 3.0 * s, GLASS);
        let header = "16 BIT DRUM SAMPLER / MIDI SEQUENCER";
        painter.text(p.pos(285.0, 50.0), Align2::CENTER_CENTER, header, p.prop(5.5), Color32::WHITE);
        let screen = p.rect(LCD_X, LCD_Y, LCD_W, 88.0);
        painter.rect_filled(screen, s, LCD_BG);

        let shared = self.engine.shared.clone();
        let st = audio::lock(&shared);
        let rows = self.rows(&st);
        let fields = lcd::fields(&rows);
        let focus = (!matches!(self.screen, Screen::Load(_) | Screen::Save(_)) && !fields.is_empty())
            .then(|| fields[self.focus.min(fields.len() - 1)].1);
        let font = p.mono(7.5);
        let mut clicked = None;

        for (i, row) in rows.iter().take(self.screen.text_rows()).enumerate() {
            let y = LCD_Y + 2.0 + i as f32 * ROW_H;
            if row.inverted {
                painter.rect_filled(p.rect(LCD_X + 1.0, y, LCD_W - 2.0, ROW_H), 0.0, LCD_INK);
            }
            let base = if row.inverted { LCD_BG } else { LCD_INK };
            let galleys = |segs: &[lcd::Seg]| -> Vec<_> {
                segs.iter()
                    .map(|seg| (seg.field, painter.layout_no_wrap(seg.text.clone(), font.clone(), base)))
                    .collect()
            };
            let left = galleys(&row.left);
            let right = galleys(&row.right);
            let right_w: f32 = right.iter().map(|(_, g)| g.size().x).sum();
            let mut x = p.pos(LCD_X + 3.0, 0.0).x;
            let mut right_x = p.pos(LCD_X + LCD_W - 3.0, 0.0).x - right_w;
            for (segs, x) in [(left, &mut x), (right, &mut right_x)] {
                for (field, galley) in segs {
                    let rect = Rect::from_min_size(egui::pos2(*x, p.pos(0.0, y).y), galley.size());
                    if field.is_some() && field == focus {
                        painter.rect_filled(rect.expand(0.5 * s), 0.0, LCD_INK);
                        painter.text(rect.min, Align2::LEFT_TOP, galley.text(), font.clone(), LCD_BG);
                    } else {
                        painter.galley(rect.min, galley.clone(), base);
                    }
                    if let Some(field) = field
                        && ui
                            .interact(rect, ui.scope_id().with(("field", i, rect.min.x as i32)), Sense::click())
                            .clicked()
                    {
                        clicked = fields.iter().position(|(_, f)| *f == field);
                    }
                    *x += galley.size().x;
                }
            }
        }

        match self.screen {
            Screen::Mixer => self.mixer(&st, painter, p),
            Screen::Trim => trim(&st, self.pad, painter, p),
            Screen::Sample => self.meters(painter, p),
            _ => {}
        }

        let keys = match (&self.screen, self.input.as_ref().map(|i| i.lock().state)) {
            (Screen::Sample, Some(sample::State::Armed | sample::State::Recording)) => ["", "", "", "", "", "STOP"],
            (Screen::Sample, Some(sample::State::Done)) => ["", "", "", "", "RETRY", "KEEP"],
            (Screen::Sample, None) => [""; 6],
            (screen, _) => screen.soft_keys(),
        };
        for (i, label) in keys.iter().enumerate() {
            let tab = p.rect(163.0 + i as f32 * 41.0, 133.0, 36.0, 9.5);
            if !label.is_empty() {
                painter.rect_filled(tab, 0.0, LCD_INK);
                painter.text(tab.center(), Align2::CENTER_CENTER, *label, p.mono(6.0), LCD_BG);
            }
            let legend = format!("F{}", i + 1);
            let at = p.pos(181.0 + i as f32 * 41.0, 163.0);
            painter.text(at, Align2::CENTER_CENTER, legend, p.prop(7.0), Color32::from_gray(0xc8));
        }
        drop(st);

        if let Some(i) = clicked {
            self.focus = i;
        }
        // Scrolling over the LCD turns the DATA wheel.
        let resp = ui.interact(screen, ui.scope_id().with("lcd"), Sense::hover());
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y) / s;
            self.turn(scroll);
        }
    }

    fn mixer(&self, st: &Shared, painter: &egui::Painter, p: &Panel) {
        let s = p.scale;
        let bank = self.pad / 16 * 16;
        let w = (LCD_W - 6.0) / 16.0;
        for i in 0..16 {
            let pad = &st.pads[bank + i];
            let x = LCD_X + 3.0 + i as f32 * w;
            let selected = bank + i == self.pad;
            // Pan marker over the strip, level bar below it.
            let cx = x + w / 2.0;
            painter.hline(
                p.pos(x + 2.0, 0.0).x..=p.pos(x + w - 2.0, 0.0).x,
                p.pos(0.0, 82.0).y,
                Stroke::new(0.5 * s, LCD_INK),
            );
            painter.rect_filled(
                p.rect(cx + pad.pan as f32 / 50.0 * (w / 2.0 - 2.0) - 1.0, 79.5, 2.0, 5.0),
                0.0,
                LCD_INK,
            );
            let bar = p.rect(x + 4.0, 87.0, w - 8.0, 32.0);
            painter.rect_stroke(bar, 0.0, Stroke::new(0.7 * s, LCD_INK), StrokeKind::Inside);
            let h = 32.0 * pad.level as f32 / 100.0;
            painter.rect_filled(p.rect(x + 4.0, 87.0 + 32.0 - h, w - 8.0, h), 0.0, LCD_INK);
            let label = p.rect(x + 1.0, 121.0, w - 2.0, 8.0);
            if selected {
                painter.rect_filled(label, 0.0, LCD_INK);
            }
            let color = if selected { LCD_BG } else { LCD_INK };
            painter.text(label.center(), Align2::CENTER_CENTER, format!("{:02}", i + 1), p.mono(6.0), color);
        }
    }

    /// SAMPLE screen input meters, −60..0 dBFS, with the threshold marked.
    fn meters(&self, painter: &egui::Painter, p: &Panel) {
        let Some(input) = &self.input else { return };
        let peak = input.lock().peak;
        let s = p.scale;
        let (x0, w) = (LCD_X + 14.0, LCD_W - 20.0);
        let pos = |db: f32| x0 + w * ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        for (ch, level) in peak.iter().enumerate() {
            let y = LCD_Y + 3.0 * ROW_H + 2.0 + ch as f32 * 9.0;
            painter.text(p.pos(LCD_X + 3.0, y + 3.5), Align2::LEFT_CENTER, ["L", "R"][ch], p.mono(7.0), LCD_INK);
            painter.rect_stroke(p.rect(x0, y, w, 7.0), 0.0, Stroke::new(0.7 * s, LCD_INK), StrokeKind::Inside);
            let x1 = pos(sample::db(*level));
            painter.rect_filled(p.rect(x0, y, x1 - x0, 7.0), 0.0, LCD_INK);
        }
        if self.sample_threshold >= -60 {
            let x = p.pos(pos(self.sample_threshold as f32), 0.0).x;
            let (y0, y1) = (p.pos(0.0, LCD_Y + 3.0 * ROW_H).y, p.pos(0.0, LCD_Y + 3.0 * ROW_H + 20.0).y);
            painter.vline(x, y0..=y1, Stroke::new(1.5 * s, LCD_INK));
        }
    }

    fn turn(&mut self, amount: f32) {
        self.wheel_angle += amount * 0.03;
        self.wheel_drag += amount;
        let steps = (self.wheel_drag / WHEEL_STEP).trunc();
        self.wheel_drag -= steps * WHEEL_STEP;
        self.data(steps as i32);
    }

    fn left_controls(&mut self, ui: &mut Ui, painter: &egui::Painter, p: &Panel) {
        let s = p.scale;

        for (i, (num, mode)) in KEYPAD.iter().enumerate() {
            let (x, y) =
                if i == 9 { (141.0, 402.0) } else { (77.0 + (i % 3) as f32 * 64.0, 286.0 + (i / 3) as f32 * 36.0) };
            p.label(painter, x + 12.0, y - 7.0, num, 8.0);
            if key(ui, p, p.rect(x, y, 24.0, 10.0), &BLUE, "", false).clicked() {
                self.mode(mode);
            }
            p.label(painter, x + 12.0, y + 16.0, mode, 6.0);
        }
        painter.rect_filled(p.rect(66.0, 386.0, 40.0, 30.0), 2.0 * s, PANEL);
        p.label(painter, 86.0, 393.0, "SHIFT", 7.0);
        if key(ui, p, p.rect(72.0, 401.0, 28.0, 12.0), &WHITE, "", self.shift).is_pointer_button_down_on() {
            self.shift = true;
        }
        p.label(painter, 217.0, 393.0, "ENTER", 7.0);
        if key(ui, p, p.rect(203.0, 401.0, 28.0, 12.0), &WHITE, "", false).clicked() {
            // SHIFT + ENTER is SAVE, as printed under the key.
            if self.shift { self.open_save() } else { self.enter() }
        }

        p.label(painter, 304.0, 262.0, "MAIN SCREEN", 7.0);
        if key(ui, p, p.rect(288.0, 271.0, 32.0, 15.0), &WHITE, "", false).clicked() {
            self.set_screen(Screen::Main);
        }
        p.label(painter, 398.0, 262.0, "OPEN WINDOW", 7.0);
        if key(ui, p, p.rect(382.0, 271.0, 32.0, 15.0), &ORANGE, "", false).clicked() {
            let fields = self.fields();
            match fields.get(self.focus).map(|f| f.1) {
                Some(Field::Timing | Field::Swing) => self.timing_correct(),
                _ => self.say("No window for this field"),
            }
        }

        p.label(painter, 350.0, 304.0, "DATA", 9.0);
        let center = p.pos(350.0, 378.0);
        let resp = ui.interact(
            Rect::from_center_size(center, vec2(124.0, 124.0) * s),
            ui.scope_id().with("data"),
            Sense::drag(),
        );
        let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        // Dragging up or scrolling up turns the wheel clockwise, which increases the value.
        self.turn((-resp.drag_delta().y + scroll) / s);
        wheel(painter, p, center, 62.0, self.wheel_angle);

        p.label(painter, 350.0, 467.0, "CURSOR", 9.0);
        painter.rect_filled(p.rect(308.0, 476.0, 86.0, 66.0), 4.0 * s, BODY_EDGE);
        let arrows = [
            (p.rect(331.0, 479.0, 40.0, 29.0), "︿", (0, -1)),
            (p.rect(331.0, 510.0, 40.0, 29.0), "﹀", (0, 1)),
            (p.rect(311.0, 479.0, 18.0, 60.0), "‹", (-1, 0)),
            (p.rect(373.0, 479.0, 18.0, 60.0), "›", (1, 0)),
        ];
        for (rect, glyph, (dx, dy)) in arrows {
            if key(ui, p, rect, &WHITE, glyph, false).clicked() {
                self.cursor(dx, dy);
            }
        }
        p.label(painter, 350.0, 551.0, "DIGIT", 6.0);

        painter.rect_stroke(p.rect(44.0, 430.0, 84.0, 244.0), 0.0, Stroke::new(s, INK), StrokeKind::Inside);
        p.label(painter, 86.0, 440.0, "NOTE", 7.0);
        p.label(painter, 86.0, 449.0, "VARIATION", 7.0);
        p.label(painter, 86.0, 462.0, "AFTER", 7.0);
        led(painter, p, 86.0, 470.0, matches!(self.screen, Screen::Assign), RED_ON, RED_OFF);
        // The hardware's AFTER uses pad pressure, which a keyboard and mouse do not have; the key
        // opens the slider's ASSIGN screen (SHIFT + AFTER on the MPC).
        if key(ui, p, p.rect(70.0, 477.0, 32.0, 16.0), &WHITE, "", false).clicked() {
            self.set_screen(Screen::Assign);
            self.focus = 0;
        }
        let track = p.rect(58.0, 510.0, 54.0, 150.0);
        let resp = ui.interact(track, ui.scope_id().with("variation"), Sense::click_and_drag());
        if let Some(pos) = resp.interact_pointer_pos()
            && (resp.dragged() || resp.clicked())
        {
            // The cap's centre runs from y 650 (0) to 520 (127).
            let v = (p.pos(0.0, 650.0).y - pos.y) / (130.0 * s);
            self.var_value = (v.clamp(0.0, 1.0) * 127.0).round() as u8;
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y) / s / 4.0;
            self.var_value = (self.var_value as f32 + scroll).clamp(0.0, 127.0).round() as u8;
        }
        if resp.double_clicked() {
            self.var_value = 64;
        }
        fader(painter, p, self.var_value as f32 / 127.0);

        p.label(painter, 205.0, 436.0, "TAP TEMPO", 7.0);
        p.label(painter, 205.0, 445.0, "NOTE REPEAT", 7.0);
        let playing = self.with(|s| s.t.playing);
        let tap = key(ui, p, p.rect(185.0, 454.0, 40.0, 30.0), &WHITE, "", false);
        if playing && tap.is_pointer_button_down_on() {
            self.with(|s| s.t.repeat = true);
        } else if !playing && tap.clicked() {
            self.tap_tempo();
        }

        let (undo, erase) = self.with(|s| (s.t.undo.is_some() && !s.t.playing, s.t.erase));
        p.label(painter, 176.0, 509.0, "UNDO SEQ", 7.0);
        led(painter, p, 176.0, 518.0, undo, RED_ON, RED_OFF);
        if key(ui, p, p.rect(160.0, 525.0, 32.0, 16.0), &WHITE, "", false).clicked() {
            self.with(|s| s.undo());
        }
        p.label(painter, 234.0, 509.0, "ERASE", 7.0);
        if key(ui, p, p.rect(218.0, 525.0, 32.0, 16.0), &WHITE, "", erase).is_pointer_button_down_on() {
            self.with(|s| s.t.erase = true);
        }

        p.section(painter, 160.0, 425.0, 563.0, "LOCATE");
        let legends = [
            (176.0, "<"),
            (205.0, "STEP"),
            (234.0, ">"),
            (293.0, "GO TO"),
            (351.0, "<<"),
            (380.0, "BAR"),
            (409.0, ">>"),
        ];
        for (x, text) in legends {
            p.label(painter, x, 574.0, text, 7.0);
        }
        let locate = [Action::Step(-1), Action::Step(1), Action::Start, Action::Bar(-1), Action::Bar(1)];
        for (x, action) in [160.0, 218.0, 277.0, 335.0, 393.0].into_iter().zip(locate) {
            if key(ui, p, p.rect(x, 582.0, 32.0, 16.0), &WHITE, "", false).clicked() {
                self.run(action);
            }
        }
    }

    fn transport(&mut self, ui: &mut Ui, painter: &egui::Painter, p: &Panel) {
        let (rec, playing) = self.with(|s| (s.t.rec, s.t.playing));
        let buttons: [(&str, &str, &KeyStyle, Option<bool>); 5] = [
            ("REC", "", &RED, Some(rec == RecMode::Rec)),
            ("OVER", "DUB", &RED, Some(rec == RecMode::Overdub)),
            ("STOP", "", &WHITE, None),
            ("PLAY", "", &WHITE, Some(playing)),
            ("PLAY", "START", &WHITE, None),
        ];
        for (i, (top, bottom, style, lit)) in buttons.into_iter().enumerate() {
            let x = [155.0, 213.0, 272.0, 331.0, 389.0][i];
            if bottom.is_empty() {
                p.label(painter, x + 21.0, 630.0, top, 7.0);
            } else {
                p.label(painter, x + 23.0, 624.0, top, 7.0);
                p.label(painter, x + 23.0, 632.0, bottom, 7.0);
            }
            if let Some(on) = lit {
                led(painter, p, x + 6.0, 627.0, on, RED_ON, RED_OFF);
            }
            if key(ui, p, p.rect(x, 640.0, 42.0, 32.0), style, "", false).clicked() {
                match i {
                    0 => self.with(|s| s.toggle_rec(RecMode::Rec)),
                    1 => self.with(|s| s.toggle_rec(RecMode::Overdub)),
                    2 => self.with(|s| s.stop()),
                    3 => self.play(false),
                    _ => self.play(true),
                }
            }
        }
    }

    fn upper_panel(&mut self, ui: &mut Ui, painter: &egui::Painter, p: &Panel) {
        let s = p.scale;
        painter.text(p.pos(553.0, 86.0), Align2::LEFT_CENTER, "rsampler", p.prop(38.0), LOGO);
        painter.text(p.pos(965.0, 72.0), Align2::RIGHT_CENTER, "DRUM SAMPLER", p.prop(20.0), INK);
        painter.hline(p.pos(760.0, 0.0).x..=p.pos(965.0, 0.0).x, p.pos(0.0, 88.0).y, Stroke::new(2.0 * s, INK));
        painter.text(p.pos(965.0, 100.0), Align2::RIGHT_CENTER, "MIDI SEQUENCER", p.prop(11.0), INK);

        painter.rect_filled(p.rect(545.0, 123.0, 430.0, 140.0), 3.0 * s, PANEL);
        painter.rect_filled(p.rect(568.0, 141.0, 135.0, 54.0), 2.0 * s, PANEL_INSET);
        painter.rect_filled(p.rect(568.0, 203.0, 135.0, 54.0), 2.0 * s, PANEL_INSET);

        let track_on = self.with(|s| s.seq().tracks[s.t.track].on);
        let toggles = [
            (602.0, 151.0, "FULL LEVEL", self.full_level),
            (664.0, 151.0, "16 LEVELS", self.sixteen_levels),
            (602.0, 213.0, "NEXT SEQ", false),
            (664.0, 213.0, "TRACK MUTE", !track_on),
        ];
        for (i, (x, y, text, on)) in toggles.into_iter().enumerate() {
            p.label(painter, x, y, text, 7.0);
            led(painter, p, x, y + 10.0, on, RED_ON, RED_OFF);
            if key(ui, p, p.rect(x - 14.0, y + 18.0, 28.0, 14.0), &WHITE, "", false).clicked() {
                match i {
                    0 => self.full_level ^= true,
                    1 => {
                        self.sixteen_levels ^= true;
                        self.levels_pad = self.pad;
                        if self.sixteen_levels {
                            self.say(format!("16 LEVELS: {} velocity", program::pad_name(self.pad)));
                        }
                    }
                    2 => self.with(|s| s.select_seq((s.t.next_seq.unwrap_or(s.t.seq) + 1).min(seq::SEQUENCES - 1))),
                    _ => self.with(|s| {
                        let tr = s.t.track;
                        s.seq_mut().tracks[tr].on ^= true;
                    }),
                }
            }
        }

        p.label(painter, 790.0, 135.0, "REC GAIN", 8.0);
        let center = p.pos(790.0, 170.0);
        let resp = ui.interact(
            Rect::from_center_size(center, vec2(52.0, 52.0) * s),
            ui.scope_id().with("gain"),
            Sense::drag(),
        );
        let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        self.rec_gain = (self.rec_gain + (-resp.drag_delta().y + scroll) / s / 120.0).clamp(0.0, 1.0);
        knob(painter, p, center, 20.0, Color32::from_rgb(0xe9, 0xa0, 0xa6), self.rec_gain);
        p.label(painter, 885.0, 135.0, "MAIN VOLUME", 8.0);
        let center = p.pos(885.0, 170.0);
        let resp = ui.interact(
            Rect::from_center_size(center, vec2(52.0, 52.0) * s),
            ui.scope_id().with("volume"),
            Sense::drag(),
        );
        let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        let delta = (-resp.drag_delta().y + scroll) / s / 120.0;
        let volume = self.with(|st| {
            st.master = (st.master + delta).clamp(0.0, 1.0);
            st.master
        });
        knob(painter, p, center, 20.0, WHITE.face, volume);
        for x in [790.0, 885.0] {
            p.label(painter, x - 22.0, 197.0, "MIN", 5.5);
            p.label(painter, x + 22.0, 197.0, "MAX", 5.5);
        }

        p.section(painter, 712.0, 965.0, 212.0, "PAD BANK");
        for (i, name) in BANKS.iter().enumerate() {
            let x = 714.0 + i as f32 * 63.0;
            p.label(painter, x + 14.0, 221.0, name, 7.0);
            led(painter, p, x + 14.0, 229.0, self.bank == i, GREEN_ON, GREEN_OFF);
            if key(ui, p, p.rect(x, 237.0, 28.0, 14.0), &WHITE, "", false).clicked() {
                self.bank = i;
                if matches!(self.screen, Screen::Mixer) {
                    self.pad = i * 16 + self.pad % 16;
                }
            }
        }
    }

    fn pads(&mut self, ui: &mut Ui, painter: &egui::Painter, p: &Panel) {
        painter.rect_filled(p.rect(545.0, 272.0, 430.0, 400.0), 3.0 * p.scale, PANEL);
        p.section(painter, 575.0, 945.0, 284.0, "DRUMS");
        for (n, key) in PAD_KEYS.iter().enumerate() {
            let (x, y) = (583.0 + (n % 4) as f32 * 92.0, 302.0 + (3 - n / 4) as f32 * 92.0);
            painter.text(p.pos(x, y - 7.0), Align2::LEFT_CENTER, format!("PAD {}", n + 1), p.prop(7.0), INK);
            let legend = if *key == Key::Z { "Z/Y" } else { key.name() };
            painter.text(p.pos(x + 76.0, y - 7.0), Align2::RIGHT_CENTER, legend, p.prop(7.0), INK);
            let rect = p.rect(x, y, 76.0, 74.0);
            let resp = ui.interact(rect, ui.scope_id().with(("pad", n)), Sense::click());
            let down = resp.is_pointer_button_down_on();
            if down && !self.mouse_held[n] {
                // With the mouse, hitting higher on the pad plays louder.
                let v = resp.interact_pointer_pos().map_or(0.8, |pos| 1.0 - (pos.y - rect.top()) / rect.height() * 0.8);
                self.pad_press(n, (v * 127.0).round() as u8);
            } else if !down && self.mouse_held[n] {
                self.pad_release(n);
            }
            self.mouse_held[n] = down;
            pad(painter, p, rect, down || self.key_held[n]);
        }
    }
}

/// TRIM screen waveform with the start and end markers.
fn trim(st: &Shared, pad: usize, painter: &egui::Painter, p: &Panel) {
    let Some(snd) = &st.pads[pad].sound else { return };
    let s = p.scale;
    let (top, height) = (80.0, 48.0);
    let mid = top + height / 2.0;
    let cols = (LCD_W - 6.0) as usize;
    let len = snd.frames.len().max(1);
    // ponytail: rescans the whole sound every frame; cache the peaks if long sounds get slow.
    for c in 0..cols {
        let (a, b) = (c * len / cols, ((c + 1) * len / cols).max(c * len / cols + 1).min(len));
        let (lo, hi) = snd.frames[a..b]
            .iter()
            .map(|f| (f[0] + f[1]) / 2.0)
            .fold((0.0f32, 0.0f32), |(lo, hi), x| (lo.min(x), hi.max(x)));
        let inside = a >= snd.start && a < snd.end;
        let color = if inside { LCD_INK } else { LCD_INK.gamma_multiply(0.35) };
        let x = p.pos(LCD_X + 3.0 + c as f32, 0.0).x;
        let (y0, y1) =
            (p.pos(0.0, mid - hi.min(1.0) * height / 2.0).y, p.pos(0.0, mid - lo.max(-1.0) * height / 2.0).y);
        painter.vline(x, y0..=y1.max(y0 + 0.5 * s), Stroke::new(s, color));
    }
    for marker in [snd.start, snd.end] {
        let x = p.pos(LCD_X + 3.0 + marker as f32 / len as f32 * cols as f32, 0.0).x;
        painter.vline(x, p.pos(0.0, top).y..=p.pos(0.0, top + height).y, Stroke::new(s, LCD_INK));
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 730.0]).with_min_inner_size([650.0, 475.0]),
        ..Default::default()
    };
    eframe::run_native("rsampler", options, Box::new(|_| Ok(Box::new(App::new()))))
}
