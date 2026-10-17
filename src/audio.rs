//! Audio output, voices and the sequencer clock. The sequencer runs inside the audio callback so
//! playback is sample-accurate; the UI talks to it through [`Shared`].

use std::error::Error;
use std::f32::consts::{PI, TAU};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::program::{DRUMS, Frames, Overlap, PADS, Pad, Program, Sound, Variation, Velo};
use crate::seq::{KEYS, Note, SEQUENCES, Sequence, TIMING, quantize};

const MAX_VOICES: usize = 64;

#[derive(Debug, PartialEq)]
pub enum Hit {
    Press {
        pad: u8,
        velocity: u8,
        var: Variation,
    },
    Release {
        pad: u8,
    },
    /// PLAY X: plays part of a pad's sound as it is, once.
    Audition {
        pad: u8,
        range: std::ops::Range<usize>,
    },
    /// Stops PLAY X and LOAD's PLAY.
    StopAudition,
    /// Ends a sequence note still sounding when playback stops.
    Off {
        key: u16,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecMode {
    Off,
    Rec,
    Overdub,
}

/// Transport and sequencer state.
pub struct Transport {
    pub seq: usize,
    /// Sequence to switch to at the end of the current one, chosen while playing.
    pub next_seq: Option<usize>,
    pub track: usize,
    pub solo: Option<usize>,
    pub playing: bool,
    /// Armed while stopped, active while playing.
    pub rec: RecMode,
    /// Position in ticks; negative during the count-in.
    pub tick: f64,
    next_tick: i64,
    pub timing: usize,
    pub swing: u32,
    pub count_in: bool,
    pub click: bool,
    /// ERASE held: pads hit now delete their notes on the current track as the playhead passes.
    pub erase: bool,
    erasing: [bool; KEYS],
    /// NOTE REPEAT held: held pads play again on every Timing grid line.
    pub repeat: bool,
    /// Pads held down, with the velocity and variation they were hit with.
    held: [Option<(u8, Variation)>; KEYS],
    /// Notes recorded this pass, with the unquantized press position while the pad is still
    /// held; merged into the track when the loop wraps or playback stops, so recording in REC
    /// mode cannot erase them.
    pending: Vec<(Note, Option<u32>)>,
    /// Pads playing NOTE OFF notes from the sequence, and the tick to release them.
    note_offs: Vec<(u16, u32)>,
    pub undo: Option<(usize, Sequence)>,
    /// Playing the song: the step, and how many times it has repeated so far.
    pub song: Option<(usize, u32)>,
    /// Send MIDI clock, start and stop.
    pub clock_out: bool,
    /// Follow an external MIDI clock instead of the sequence tempo.
    pub sync: bool,
    pub ext: ExtClock,
    /// MIDI notes sent from the sequence that still need their note off: status, note, tick.
    midi_offs: Vec<(u8, u8, u32)>,
    pub punch: Punch,
    /// 2ND SEQ: another sequence playing along, when on.
    pub second: (bool, usize),
    /// TRANS: when on, MIDI notes of a track (or all, `None`) move by this many semitones.
    pub transpose: (bool, Option<usize>, i32),
}

/// MISC's PUNCH: with `on`, recording only happens inside the punch times.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Punch {
    pub on: bool,
    pub mode: PunchMode,
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PunchMode {
    /// From `from` on.
    In,
    /// Until `to`.
    Out,
    /// From `from` until `to`.
    InOut,
}

impl PunchMode {
    pub const ALL: [PunchMode; 3] = [PunchMode::In, PunchMode::Out, PunchMode::InOut];

    pub fn label(self) -> &'static str {
        match self {
            PunchMode::In => "PUNCH IN ONLY",
            PunchMode::Out => "PUNCH OUT ONLY",
            PunchMode::InOut => "AUTO IN/OUT",
        }
    }
}

impl Punch {
    /// Whether a recording at `tick` is kept.
    pub fn allows(&self, tick: u32) -> bool {
        !self.on
            || match self.mode {
                PunchMode::In => tick >= self.from,
                PunchMode::Out => tick < self.to,
                PunchMode::InOut => tick >= self.from && tick < self.to,
            }
    }
}

/// State of an incoming MIDI clock.
#[derive(Default)]
pub struct ExtClock {
    /// Smoothed time between clock pulses, in microseconds; 0 until two pulses arrived.
    interval_us: f64,
    last_us: Option<u64>,
    /// Position of the last pulse; playback may run at most one pulse (4 ticks) past it.
    clock_tick: f64,
}

impl ExtClock {
    pub fn tempo(&self) -> Option<f32> {
        (self.interval_us > 0.0).then(|| (60e6 / (self.interval_us * 24.0)) as f32)
    }
}

/// Song mode: sequences played in order, each repeated a number of times.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub steps: Vec<(usize, u32)>,
    pub looping: bool,
}

pub struct Shared {
    pub programs: Vec<Program>,
    /// The program each DRUM bus plays.
    pub drums: [usize; DRUMS],
    pub seqs: Vec<Sequence>,
    pub song: Song,
    pub t: Transport,
    pub master: f32,
    /// Pad hits from the UI, consumed by the audio callback.
    pub hits: Vec<Hit>,
    pub rate: u32,
    /// MIDI output, if one could be opened. Messages are [status, data, data]; real-time
    /// messages (clock, start, stop) only use the status byte.
    pub midi_out: Option<mpsc::Sender<[u8; 3]>>,
    /// Velocity of pads hit from the computer keyboard.
    pub key_velocity: u8,
    /// A sound file to listen to from the LOAD screen, consumed by the audio callback.
    pub preview: Option<Sound>,
    /// The MIDI out port chosen in SETUP, saved with the project; the UI opens it.
    pub out_port: String,
    pub fx: crate::fx::FxSettings,
}

impl Shared {
    fn new(rate: u32) -> Self {
        Self {
            programs: vec![Program::new("NewPgm-A")],
            drums: [0; DRUMS],
            seqs: (0..SEQUENCES).map(|_| Sequence::default()).collect(),
            song: Song::default(),
            t: Transport {
                seq: 0,
                next_seq: None,
                track: 0,
                solo: None,
                playing: false,
                rec: RecMode::Off,
                tick: 0.0,
                next_tick: 0,
                timing: 3,
                swing: 50,
                count_in: true,
                click: false,
                erase: false,
                erasing: [false; KEYS],
                repeat: false,
                held: [None; KEYS],
                pending: Vec::new(),
                note_offs: Vec::new(),
                undo: None,
                song: None,
                clock_out: false,
                sync: false,
                ext: ExtClock::default(),
                midi_offs: Vec::new(),
                punch: Punch { on: false, mode: PunchMode::InOut, from: 0, to: 4 * crate::seq::PPQ * 4 },
                second: (false, 1),
                transpose: (false, None, 0),
            },
            master: 0.8,
            hits: Vec::new(),
            rate,
            midi_out: None,
            key_velocity: 101,
            preview: None,
            out_port: crate::midi::OUT_OFF.to_owned(),
            fx: crate::fx::FxSettings::default(),
        }
    }

    /// The program a track plays, or `None` for a MIDI track.
    pub fn program_of(&self, track: &crate::seq::Track) -> Option<usize> {
        let bus = track.bus.checked_sub(1)? as usize;
        Some(self.drums[bus.min(DRUMS - 1)].min(self.programs.len() - 1))
    }

    /// The program the pads play and the PROGRAM, MIXER and TRIM screens edit: the selected
    /// track's, or DRUM1's when the track is a MIDI track.
    pub fn program(&self) -> usize {
        self.program_of(&self.seq().tracks[self.t.track]).unwrap_or(self.drums[0].min(self.programs.len() - 1))
    }

    pub fn pads(&self) -> &[Pad] {
        &self.programs[self.program()].pads
    }

    pub fn pads_mut(&mut self) -> &mut [Pad] {
        let p = self.program();
        &mut self.programs[p].pads
    }

    /// The tempo playing: the sequence's, or the external clock's when synced.
    pub fn tempo(&self) -> f32 {
        match (self.t.sync, self.t.ext.tempo()) {
            (true, Some(ext)) => ext,
            _ => self.seq().tempo,
        }
    }

    pub fn seq(&self) -> &Sequence {
        &self.seqs[self.t.seq]
    }

    pub fn seq_mut(&mut self) -> &mut Sequence {
        &mut self.seqs[self.t.seq]
    }

    /// Current position, never negative (0 during the count-in).
    pub fn now(&self) -> u32 {
        self.t.tick.max(0.0) as u32
    }

    /// PLAY (from the current position) or PLAY START (from the first bar).
    pub fn play(&mut self, from_start: bool) {
        if self.t.playing {
            return;
        }
        let start = if from_start { 0.0 } else { self.t.tick.max(0.0) };
        self.t.tick = start;
        if self.t.rec != RecMode::Off {
            self.begin_recording();
            if self.t.count_in {
                self.t.tick -= self.seq().ticks_per_bar() as f64;
            }
        }
        self.t.next_tick = self.t.tick.ceil() as i64;
        self.t.playing = true;
        if self.t.clock_out {
            if start == 0.0 {
                self.send([0xFA, 0, 0]);
            } else {
                // Song position pointer, in sixteenths (24 ticks), then continue from there.
                let sixteenths = start as u32 / 24;
                self.send([0xF2, (sixteenths & 0x7F) as u8, (sixteenths >> 7 & 0x7F) as u8]);
                self.send([0xFB, 0, 0]);
            }
        }
    }

    fn send(&self, msg: [u8; 3]) {
        if let Some(out) = &self.midi_out {
            let _ = out.send(msg);
        }
    }

    /// MIDI channel of the selected track, for passing pad hits through.
    fn thru(&self) -> Option<u8> {
        self.seq().tracks[self.t.track].out
    }

    /// Whether the selected track is a MIDI track, whose notes are MIDI notes rather than pads.
    pub fn midi_track(&self) -> bool {
        self.seq().tracks[self.t.track].bus == 0
    }

    /// The MIDI note a note of the selected track goes out as: pads A01–D16 are 36–99.
    fn out_note(&self, pad: u8) -> u8 {
        if self.midi_track() { pad } else { pad + crate::midi::FIRST_NOTE }
    }

    /// Plays the song from `step`. Song mode does not record.
    pub fn play_song(&mut self, step: usize) -> bool {
        if self.t.playing || self.song.steps.is_empty() {
            return false;
        }
        let step = step.min(self.song.steps.len() - 1);
        self.t.song = Some((step, 0));
        self.t.seq = self.song.steps[step].0;
        self.t.rec = RecMode::Off;
        self.t.next_seq = None;
        self.t.tick = 0.0;
        self.t.next_tick = 0;
        self.t.playing = true;
        if self.t.clock_out {
            self.send([0xFA, 0, 0]);
        }
        true
    }

    pub fn stop(&mut self) {
        self.hits.push(Hit::StopAudition);
        self.t.song = None;
        if !self.t.playing {
            // Pressing STOP while stopped returns to the start, like the hardware.
            self.locate(0);
            self.t.rec = RecMode::Off;
            return;
        }
        self.commit_pending();
        for (key, _) in self.t.note_offs.drain(..) {
            self.hits.push(Hit::Off { key });
        }
        for (status, note, _) in std::mem::take(&mut self.t.midi_offs) {
            self.send([status, note, 0]);
        }
        if self.t.clock_out {
            self.send([0xFC, 0, 0]);
        }
        self.t.playing = false;
        self.t.rec = RecMode::Off;
        self.t.next_seq = None;
        self.t.tick = self.t.tick.max(0.0);
    }

    /// REC or OVERDUB pressed: arms while stopped, punches in or out while playing.
    pub fn toggle_rec(&mut self, mode: RecMode) {
        if self.t.rec == mode {
            self.commit_pending();
            self.t.rec = RecMode::Off;
        } else {
            let was_recording = self.t.rec != RecMode::Off;
            self.t.rec = mode;
            if self.t.playing && !was_recording {
                self.begin_recording();
            }
        }
    }

    fn begin_recording(&mut self) {
        self.t.undo = Some((self.t.seq, self.seq().clone()));
    }

    /// UNDO SEQ: swaps the sequence with its state before the last recording or edit.
    pub fn undo(&mut self) {
        if self.t.playing {
            return;
        }
        if let Some((seq, before)) = self.t.undo.take() {
            let after = std::mem::replace(&mut self.seqs[seq], before);
            self.t.undo = Some((seq, after));
        }
    }

    /// Saves the current sequence for UNDO SEQ before a destructive edit.
    pub fn checkpoint(&mut self) {
        self.begin_recording();
    }

    pub fn locate(&mut self, tick: u32) {
        let tick = tick.min(self.seq().len().saturating_sub(1));
        self.t.tick = tick as f64;
        self.t.next_tick = tick as i64;
    }

    pub fn select_seq(&mut self, seq: usize) {
        if self.t.playing {
            self.t.next_seq = Some(seq);
        } else {
            self.t.seq = seq;
            self.locate(self.now());
        }
    }

    pub fn grid(&self) -> u32 {
        TIMING[self.t.timing].1
    }

    fn commit_pending(&mut self) {
        let (seq, track) = (self.t.seq, self.t.track);
        let now = self.now();
        let len = self.seqs[seq].len();
        for (mut note, held_from) in self.t.pending.drain(..) {
            if let Some(from) = held_from {
                note.duration = (now + len - from) % len;
            }
            note.duration = note.duration.max(1);
            self.seqs[seq].tracks[track].insert(note);
        }
    }

    /// Handles everything due at `tick`: metronome, count-in, recording erase and note playback.
    fn on_tick(&mut self, tick: i64, voices: &mut Voices) {
        // MIDI clock runs at 24 per quarter note, every 4 of our ticks.
        if self.t.clock_out && tick.rem_euclid(4) == 0 {
            self.send([0xF8, 0, 0]);
        }
        let seq = self.seq();
        let tpb = seq.ticks_per_beat() as i64;
        let recording = self.t.rec != RecMode::Off;
        if tick.rem_euclid(tpb) == 0 && (tick < 0 || (self.t.click && (recording || self.t.playing))) {
            let accent = tick.rem_euclid(seq.ticks_per_bar() as i64) == 0;
            voices.click(accent);
        }
        if tick < 0 {
            return;
        }
        let tick = tick as u32;

        let (seqi, track) = (self.t.seq, self.t.track);
        let recording = recording && self.t.punch.allows(tick);
        if self.t.rec == RecMode::Rec && recording {
            self.seqs[seqi].tracks[track].erase(tick, tick + 1, None);
        }
        if self.t.erase {
            for pad in (0..KEYS).filter(|&p| self.t.erasing[p]) {
                self.seqs[seqi].tracks[track].erase(tick, tick + 1, Some(pad as u8));
            }
        }

        let mut i = 0;
        while i < self.t.midi_offs.len() {
            if self.t.midi_offs[i].2 == tick {
                let (status, note, _) = self.t.midi_offs.swap_remove(i);
                self.send([status, note, 0]);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.t.note_offs.len() {
            if self.t.note_offs[i].1 == tick {
                let (key, _) = self.t.note_offs.swap_remove(i);
                voices.release(key);
            } else {
                i += 1;
            }
        }

        let len = self.seqs[seqi].len();
        // NOTE REPEAT plays the held pads on the grid; with Timing OFF it uses 1/16.
        let grid = if self.grid() > 1 { self.grid() } else { 24 };
        if self.t.repeat && tick.is_multiple_of(grid) {
            for pad in 0..KEYS {
                if let Some((velocity, var)) = self.t.held[pad] {
                    if let Some(p) = self.program_of(&self.seq().tracks[self.t.track]) {
                        voices.trigger(&self.programs[p].pads, p, pad as u8, velocity, var);
                    }
                    if recording {
                        let note = Note { tick, pad: pad as u8, velocity, duration: grid / 2, var };
                        self.t.pending.push((note, None));
                    }
                }
            }
        }
        // 2ND SEQ plays along from the start, looping on its own length.
        let second =
            Some(self.t.second).filter(|(on, s)| *on && *s != seqi).map(|(_, s)| (s, tick % self.seqs[s].len()));
        for (si, at, main) in std::iter::once((seqi, tick, true)).chain(second.map(|(s, at)| (s, at, false))) {
            for (ti, t) in self.seqs[si].tracks.iter().enumerate() {
                if !t.on || (main && self.t.solo.is_some_and(|s| s != ti)) {
                    continue;
                }
                let shift = match self.t.transpose {
                    (true, None, by) => by,
                    (true, Some(tr), by) if tr == ti => by,
                    _ => 0,
                };
                for n in t.range(at, at + 1) {
                    if let (Some(ch), Some(out)) = (t.out, &self.midi_out) {
                        let base = if t.bus == 0 { 0 } else { crate::midi::FIRST_NOTE as i32 };
                        let note = (n.pad as i32 + base + shift).clamp(0, 127) as u8;
                        let _ = out.send([0x90 | ch, note, n.velocity]);
                        self.t.midi_offs.push((0x80 | ch, note, (tick + n.duration.max(1)) % len));
                    }
                    // MIDI tracks only send their notes out.
                    let Some(p) = t.bus.checked_sub(1).map(|b| self.drums[b as usize].min(self.programs.len() - 1))
                    else {
                        continue;
                    };
                    voices.trigger(&self.programs[p].pads, p, n.pad, n.velocity, n.var);
                    if self.programs[p].pads.get(n.pad as usize).is_some_and(Pad::note_off) {
                        self.t.note_offs.push((key(p, n.pad), (tick + n.duration) % len));
                    }
                }
            }
        }
    }

    /// Advances one frame of playback.
    fn advance(&mut self, voices: &mut Voices) {
        if !self.t.playing {
            return;
        }
        while self.t.next_tick as f64 <= self.t.tick {
            let tick = self.t.next_tick;
            if tick >= self.seq().len() as i64 {
                let before = self.t.tick;
                self.end_of_sequence();
                // Wrapping moves the playhead back; the external clock's position moves with it.
                self.t.ext.clock_tick -= before - self.t.tick;
                if !self.t.playing {
                    return;
                }
                continue;
            }
            self.on_tick(tick, voices);
            self.t.next_tick += 1;
        }
        let rate = self.rate as f64;
        let tempo = self.tempo();
        self.t.tick += tempo as f64 * crate::seq::PPQ as f64 / 60.0 / rate;
        if self.t.sync {
            // Never run more than one pulse ahead of the clock; the next pulse lets it go on.
            self.t.tick = self.t.tick.min(self.t.ext.clock_tick + 4.0);
        }
    }

    /// MIDI real-time and song position messages, followed when `sync` is on. `stamp_us` is the
    /// arrival time in microseconds.
    pub fn midi_sync(&mut self, msg: &[u8], stamp_us: u64) {
        if !self.t.sync {
            return;
        }
        match *msg {
            [0xF8] => {
                let ext = &mut self.t.ext;
                if let Some(last) = ext.last_us {
                    let dt = stamp_us.saturating_sub(last) as f64;
                    // Ignore gaps, such as the pause before a start.
                    if dt > 0.0 && dt < 250_000.0 {
                        ext.interval_us = if ext.interval_us == 0.0 { dt } else { ext.interval_us * 0.9 + dt * 0.1 };
                    }
                }
                ext.last_us = Some(stamp_us);
                if self.t.playing {
                    ext.clock_tick += 4.0;
                    // Behind the clock: catch up; the skipped ticks still play.
                    self.t.tick = self.t.tick.max(ext.clock_tick);
                }
            }
            [0xFA] => {
                self.locate(0);
                self.start_synced();
            }
            [0xFB] => self.start_synced(),
            [0xFC] => self.stop(),
            [0xF2, lsb, msb] if !self.t.playing => {
                let sixteenths = (lsb as u32 & 0x7F) | (msb as u32 & 0x7F) << 7;
                self.locate(sixteenths * 24);
            }
            _ => {}
        }
    }

    /// Starts on an external start or continue: no count-in, and the first pulse marks `Now`.
    fn start_synced(&mut self) {
        if self.t.playing {
            return;
        }
        let count_in = std::mem::replace(&mut self.t.count_in, false);
        self.play(false);
        self.t.count_in = count_in;
        self.t.ext.clock_tick = self.t.tick - 4.0;
    }

    fn end_of_sequence(&mut self) {
        let len = self.seq().len() as f64;
        self.commit_pending();
        if let Some((step, rep)) = self.t.song {
            let (mut step, mut rep) = (step, rep + 1);
            if rep >= self.song.steps.get(step).map_or(0, |s| s.1) {
                (step, rep) = (step + 1, 0);
            }
            if step >= self.song.steps.len() && self.song.looping {
                step = 0;
            }
            match self.song.steps.get(step) {
                Some(&(seq, _)) => {
                    self.t.song = Some((step, rep));
                    self.t.seq = seq;
                    self.t.tick -= len;
                    self.t.next_tick = 0;
                }
                None => {
                    self.stop();
                    self.locate(0);
                }
            }
        } else if let Some(next) = self.t.next_seq.take() {
            self.t.seq = next;
            self.t.tick -= len;
            self.t.next_tick = 0;
        } else if self.seq().looping {
            // The MPC switches REC to OVERDUB after the first pass, so the take just recorded
            // is not erased on the next one.
            if self.t.rec == RecMode::Rec {
                self.t.rec = RecMode::Overdub;
            }
            self.t.tick -= len;
            self.t.next_tick = 0;
        } else {
            self.t.playing = false;
            self.t.rec = RecMode::Off;
            self.t.tick = len - 1.0;
        }
    }

    fn handle_hit(&mut self, hit: Hit, voices: &mut Voices) {
        match hit {
            Hit::Press { pad, velocity, var } => {
                self.t.held[pad as usize] = Some((velocity, var));
                if self.t.erase && self.t.playing {
                    self.t.erasing[pad as usize] = true;
                    return;
                }
                if let Some(p) = self.program_of(&self.seq().tracks[self.t.track]) {
                    voices.trigger(&self.programs[p].pads, p, pad, velocity, var);
                }
                if let Some(ch) = self.thru() {
                    self.send([0x90 | ch, self.out_note(pad), velocity]);
                }
                if self.t.playing && self.t.rec != RecMode::Off && self.t.tick >= 0.0 && self.t.punch.allows(self.now())
                {
                    let (now, len) = (self.now(), self.seq().len());
                    let tick = quantize(now, self.grid(), self.t.swing, len);
                    self.t.pending.push((Note { tick, pad, velocity, duration: 0, var }, Some(now)));
                }
            }
            Hit::Audition { pad, range } => {
                if let Some(sound) = &self.pads()[pad as usize].sound {
                    voices.audition(sound, range);
                }
            }
            Hit::StopAudition => voices.cut(AUDITION),
            Hit::Off { key } => voices.release(key),
            Hit::Release { pad } => {
                self.t.held[pad as usize] = None;
                self.t.erasing[pad as usize] = false;
                if let Some(ch) = self.thru() {
                    self.send([0x80 | ch, self.out_note(pad), 0]);
                }
                // Only NOTE OFF sounds stop when the pad is let go; the others play out.
                // With SIMULT or VEL SW the hit may be sounding on the other pads too.
                if let Some(p) = self.program_of(&self.seq().tracks[self.t.track]) {
                    let pads = &self.programs[p].pads;
                    let also = pads.get(pad as usize).map_or([None; 2], |x| x.also);
                    for target in std::iter::once(pad).chain(also.into_iter().flatten()) {
                        if pads.get(target as usize).is_some_and(Pad::note_off) {
                            voices.release(key(p, target));
                        }
                    }
                }
                let len = self.seq().len();
                let now = self.now();
                // The duration is how long the pad was held, wherever quantizing moved the start.
                for (note, held_from) in self.t.pending.iter_mut().filter(|(n, h)| h.is_some() && n.pad == pad) {
                    note.duration = ((now + len - held_from.take().unwrap_or(now)) % len).max(1);
                }
            }
        }
    }
}

pub struct Engine {
    _stream: Option<cpal::Stream>,
    pub shared: Arc<Mutex<Shared>>,
}

impl Engine {
    /// Opens the default output device. Without one the app still runs, silently, and the
    /// error is returned for display.
    pub fn start() -> (Self, Option<String>) {
        match Self::open() {
            Ok(engine) => (engine, None),
            Err(e) => {
                (Self { _stream: None, shared: Arc::new(Mutex::new(Shared::new(48_000))) }, Some(format!("Audio: {e}")))
            }
        }
    }

    fn open() -> Result<Self, Box<dyn Error>> {
        let device = cpal::default_host().default_output_device().ok_or("no audio output device")?;
        let mut config = device.default_output_config()?.config();
        let shared = Arc::new(Mutex::new(Shared::new(config.sample_rate)));

        // A small buffer keeps pad latency low; fall back to the device default if refused.
        config.buffer_size = cpal::BufferSize::Fixed(256);
        let stream = match Self::stream(&device, config, shared.clone()) {
            Ok(stream) => stream,
            Err(_) => {
                config.buffer_size = cpal::BufferSize::Default;
                Self::stream(&device, config, shared.clone())?
            }
        };
        stream.play()?;
        Ok(Self { _stream: Some(stream), shared })
    }

    // ponytail: always asks for f32 output, which PipeWire/Pulse/WASAPI accept; match on
    // `sample_format()` if a device turns up that only takes integer samples.
    fn stream(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        shared: Arc<Mutex<Shared>>,
    ) -> Result<cpal::Stream, cpal::Error> {
        let channels = config.channels as usize;
        let mut voices = Voices::new(config.sample_rate);
        // ponytail: one mutex shared with the UI, held for a buffer at a time; the UI only holds
        // it briefly. Move to lock-free queues if this ever causes dropouts.
        device.build_output_stream(
            config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut s = lock(&shared);
                for hit in std::mem::take(&mut s.hits) {
                    s.handle_hit(hit, &mut voices);
                }
                if let Some(sound) = s.preview.take() {
                    let len = sound.frames.len();
                    voices.audition(&sound, 0..len);
                }
                for frame in out.chunks_mut(channels) {
                    s.advance(&mut voices);
                    let [l, r] = voices.render(&s.fx, s.tempo());
                    let (l, r) = (soft_clip(l * s.master), soft_clip(r * s.master));
                    for (i, x) in frame.iter_mut().enumerate() {
                        *x = if i % 2 == 0 { l } else { r };
                    }
                }
            },
            |err| eprintln!("audio: {err}"),
            None,
        )
    }
}

/// Linear up to −2 dBFS, then bends smoothly towards full scale instead of clipping hard when
/// many pads hit at once.
fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let a = x.abs();
    if a <= KNEE { x } else { x.signum() * (KNEE + (1.0 - KNEE) * ((a - KNEE) / (1.0 - KNEE)).tanh()) }
}

pub fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    // A panic while holding the lock leaves plain data behind, so keep going with it.
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// Simplified TPT state-variable low-pass, one per channel.
#[derive(Clone, Copy)]
struct Svf {
    a1: f32,
    a2: f32,
    a3: f32,
    ic: [[f32; 2]; 2],
}

impl Svf {
    fn new(cutoff_hz: f32, resonance: f32, rate: f32) -> Self {
        let mut f = Self { a1: 0.0, a2: 0.0, a3: 0.0, ic: [[0.0; 2]; 2] };
        f.tune(cutoff_hz, resonance, rate);
        f
    }

    /// Moves the cutoff, keeping the filter's state so a sweep has no clicks.
    fn tune(&mut self, cutoff_hz: f32, resonance: f32, rate: f32) {
        let g = (PI * cutoff_hz.min(rate * 0.45) / rate).tan();
        let k = 2.0 - 1.9 * resonance;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        (self.a2, self.a3) = (g * self.a1, g * g * self.a1);
    }

    fn process(&mut self, ch: usize, x: f32) -> f32 {
        let [ic1, ic2] = &mut self.ic[ch];
        let v3 = x - *ic2;
        let v1 = self.a1 * *ic1 + self.a2 * v3;
        let v2 = *ic2 + self.a2 * *ic1 + self.a3 * v3;
        *ic1 = 2.0 * v1 - *ic1;
        *ic2 = 2.0 * v2 - *ic2;
        v2
    }
}

struct Voice {
    key: u16,
    frames: Frames,
    pos: f64,
    end: f64,
    loop_to: Option<f64>,
    rate: f64,
    gain: [f32; 2],
    /// Share of the output that goes to the effects.
    send: f32,
    age: u32,
    attack: u32,
    /// START mode: decay time constant in frames after the attack.
    decay_tau: Option<f32>,
    /// END mode: source position where the fade-out before the end begins.
    fade_from: Option<f64>,
    release: Option<(f32, u32, u32)>,
    release_len: u32,
    /// Short fade when cut off by a MONO retrigger, a mute assign or voice stealing.
    cut: Option<u32>,
    filter: Option<Svf>,
    sweep: Option<Sweep>,
}

/// A filter envelope in motion: the cutoff (0..=100) rises by `amount` over `attack` frames, then
/// falls back over `decay` frames.
struct Sweep {
    base: f32,
    amount: f32,
    attack: f32,
    decay: f32,
    resonance: f32,
    rate: f32,
}

/// Cutoff 0..=100 to Hz, 20 Hz to 20 kHz evenly in pitch.
fn cutoff_hz(cutoff: f32) -> f32 {
    20.0 * 1000f32.powf(cutoff / 100.0)
}

/// Frames between filter envelope updates.
const SWEEP_STEP: u32 = 32;

const CUT_FRAMES: u32 = 128;
/// Voice owner for PLAY X, which is no pad.
const AUDITION: u16 = u16::MAX - 1;

/// What a voice belongs to: a pad of a program, so pads of different programs never cut or
/// release each other.
fn key(program: usize, pad: u8) -> u16 {
    (program * PADS + pad as usize) as u16
}

impl Voice {
    fn level(&mut self) -> f32 {
        let mut g = if self.age < self.attack { self.age as f32 / self.attack as f32 } else { 1.0 };
        if let Some(tau) = self.decay_tau {
            g *= (-(self.age.saturating_sub(self.attack) as f32) / tau).exp();
        }
        if let Some(from) = self.fade_from
            && self.pos > from
        {
            g *= ((self.end - self.pos) / (self.end - from)) as f32;
        }
        if let Some((start, len, ref mut i)) = self.release {
            g *= start * (1.0 - *i as f32 / len as f32);
            *i += 1;
        }
        if let Some(ref mut left) = self.cut {
            g *= *left as f32 / CUT_FRAMES as f32;
            *left = left.saturating_sub(1);
        }
        g
    }

    fn done(&self) -> bool {
        (self.pos >= self.end && self.loop_to.is_none())
            || self.cut == Some(0)
            || self.release.is_some_and(|(_, len, i)| i >= len)
            || self.decay_tau.is_some_and(|tau| self.age > self.attack + (tau * 7.0) as u32)
    }

    fn next(&mut self) -> [f32; 2] {
        let i = self.pos as usize;
        let frac = (self.pos - i as f64) as f32;
        let a = self.frames[i];
        let b = self.frames.get(i + 1).copied().unwrap_or(a);
        let g = self.level();
        if let (Some(s), Some(f)) = (&self.sweep, &mut self.filter)
            && self.age.is_multiple_of(SWEEP_STEP)
        {
            let t = self.age as f32;
            let env = if t < s.attack { t / s.attack } else { (1.0 - (t - s.attack) / s.decay).max(0.0) };
            f.tune(cutoff_hz((s.base + s.amount * env).min(100.0)), s.resonance, s.rate);
        }
        let mut out = [0.0; 2];
        for ch in 0..2 {
            let x = a[ch] + (b[ch] - a[ch]) * frac;
            let x = match &mut self.filter {
                Some(f) => f.process(ch, x),
                None => x,
            };
            out[ch] = x * g * self.gain[ch];
        }
        self.pos += self.rate;
        if let Some(to) = self.loop_to
            && self.pos >= self.end
        {
            self.pos = to + (self.pos - self.end);
        }
        self.age += 1;
        out
    }
}

pub struct Voices {
    voices: Vec<Voice>,
    rate: u32,
    clicks: [Frames; 2],
    fx: crate::fx::Fx,
}

impl Voices {
    fn new(rate: u32) -> Self {
        let click = |freq: f32| -> Frames {
            let r = rate as f32;
            (0..(0.03 * r) as usize)
                .map(|i| {
                    let t = i as f32 / r;
                    let x = (TAU * freq * t).sin() * (-t * 150.0).exp() * 0.6;
                    [x, x]
                })
                .collect()
        };
        Self {
            voices: Vec::with_capacity(MAX_VOICES + 8),
            rate,
            clicks: [click(1000.0), click(1600.0)],
            fx: crate::fx::Fx::new(rate),
        }
    }

    fn cut(&mut self, key: u16) {
        for v in self.voices.iter_mut().filter(|v| v.key == key && v.cut.is_none()) {
            v.cut = Some(CUT_FRAMES);
        }
    }

    fn release(&mut self, key: u16) {
        for v in self.voices.iter_mut().filter(|v| v.key == key && v.release.is_none()) {
            let level = if v.attack > 0 { (v.age as f32 / v.attack as f32).min(1.0) } else { 1.0 };
            v.release = Some((level, v.release_len, 0));
        }
    }

    /// Plays `pad` of program number `program`, whose pads are `pads`.
    fn trigger(&mut self, pads: &[Pad], program: usize, pad: u8, velocity: u8, var: Variation) {
        // A note past the 64 pads (one recorded on a MIDI track) plays nothing.
        let Some(hit) = pads.get(pad as usize) else { return };
        for target in hit.targets(pad, velocity) {
            // The NOTE VARIATION belongs to the pad that was hit.
            self.play(pads, program, target, velocity, if target == pad { var } else { None });
        }
    }

    fn play(&mut self, pads: &[Pad], program: usize, pad: u8, velocity: u8, var: Variation) {
        let Some(p) = pads.get(pad as usize) else { return };
        let Some(sound) = &p.sound else { return };
        // Mute assigns work within the program.
        for m in p.mutes.iter().flatten() {
            self.cut(key(program, *m));
        }
        if p.overlap != Overlap::Poly || p.note_off() {
            self.cut(key(program, pad));
        }
        if self.voices.len() >= MAX_VOICES
            && let Some(oldest) = self.voices.iter_mut().find(|v| v.cut.is_none())
        {
            oldest.cut = Some(CUT_FRAMES);
        }
        self.voices.push(voice(&p.varied(var), sound, key(program, pad), velocity, self.rate));
    }

    fn audition(&mut self, sound: &Sound, range: std::ops::Range<usize>) {
        self.cut(AUDITION);
        let end = range.end.min(sound.frames.len());
        let part = Sound { start: range.start.min(end.saturating_sub(1)), end, looping: false, ..sound.clone() };
        let plain = Pad { decay: 0, ..Pad::default() };
        self.voices.push(voice(&plain, &part, AUDITION, 127, self.rate));
    }

    fn click(&mut self, accent: bool) {
        let frames = self.clicks[accent as usize].clone();
        let end = frames.len() as f64;
        self.voices.push(Voice {
            key: u16::MAX,
            frames,
            pos: 0.0,
            end,
            loop_to: None,
            rate: 1.0,
            gain: [1.0, 1.0],
            send: 0.0,
            age: 0,
            attack: 0,
            decay_tau: None,
            fade_from: None,
            release: None,
            release_len: 1,
            cut: None,
            filter: None,
            sweep: None,
        });
    }

    /// One frame of output: the voices, plus the effects on what they send.
    fn render(&mut self, fx: &crate::fx::FxSettings, tempo: f32) -> [f32; 2] {
        let (mut mix, mut send) = ([0.0; 2], [0.0; 2]);
        for v in &mut self.voices {
            let [l, r] = v.next();
            (mix[0], mix[1]) = (mix[0] + l, mix[1] + r);
            (send[0], send[1]) = (send[0] + l * v.send, send[1] + r * v.send);
        }
        self.voices.retain(|v| !v.done());
        let [l, r] = self.fx.process(send, fx, tempo);
        [mix[0] + l, mix[1] + r]
    }

    /// Nothing sounding any more, effect tails included.
    fn quiet(&self) -> bool {
        self.voices.is_empty() && self.fx.level < 1e-4
    }
}

fn voice(p: &Pad, sound: &Sound, key: u16, velocity: u8, rate: u32) -> Voice {
    let r = rate as f32;
    let end = sound.end.min(sound.frames.len()) as f64;
    // VELOCITY MODULATION, with the MPC's numbers: softer hits start up to (v² + 1) / 2 ms later,
    // fade in more slowly and are quieter; harder hits open the filter.
    let late = Velo::softness(velocity, p.velo.start);
    let late = if late == 0 { 0.0 } else { (late * late + 1) as f64 / 2.0 / 1000.0 * r as f64 };
    let start = (sound.start as f64 + late).min(end - 1.0).max(sound.start as f64);
    let soft_attack = Velo::softness(velocity, p.velo.attack);
    let cutoff = (p.cutoff + velocity as u32 * p.velo.filter / 127).min(100);
    let velocity_level = (127.0 + (velocity as f32 - 127.0) * p.velo.level as f32 / 100.0) / 127.0;
    let secs = |v: u32, max: f32| (v as f32 / 100.0).powi(2) * max * r;
    // Decay is a time, as on the MPC: START decays from the end of the attack, END fades out over
    // that long before the end of the sound (0 stops it abruptly).
    let decay = secs(p.decay, 2.0);
    let decay_tau = p.decay_from_start.then(|| decay.max(r * 0.005));
    let loop_to = sound.looping.then(|| (sound.loop_to as f64).clamp(start, end - 1.0));
    // A loop never reaches its end, so it only fades when let go.
    let fade_from = (!p.decay_from_start && decay >= 1.0 && loop_to.is_none()).then(|| (end - decay as f64).max(start));
    // Equal-power pan, normalised so the centre stays at full level.
    let angle = (p.pan + 50) as f32 / 100.0 * PI / 2.0;
    let level = p.level as f32 / 100.0 * velocity_level * std::f32::consts::SQRT_2;
    let resonance = p.resonance as f32 / 100.0;
    let filter = (cutoff < 100).then(|| Svf::new(cutoff_hz(cutoff as f32), resonance, r));
    // The MPC's envelope times: (v² + 1) / 2 ms, up to five seconds.
    let ms = |v: u32| (v * v).div_ceil(2).max(1) as f32 / 1000.0 * r;
    let sweep = (cutoff < 100 && p.fenv.amount > 0).then(|| Sweep {
        base: cutoff as f32,
        amount: p.fenv.amount as f32,
        attack: ms(p.fenv.attack),
        decay: ms(p.fenv.decay),
        resonance,
        rate: r,
    });
    Voice {
        key,
        frames: sound.frames.clone(),
        pos: start,
        end,
        loop_to,
        rate: 2f64.powf(p.tune as f64 / 120.0),
        gain: [angle.cos() * level, angle.sin() * level],
        send: p.fx_send as f32 / 100.0,
        age: 0,
        attack: (secs(p.attack, 1.0) + secs(soft_attack, 1.0)) as u32,
        decay_tau,
        fade_from,
        release: None,
        release_len: (r * 0.005 + secs(p.decay, 1.0)) as u32,
        cut: None,
        filter,
        sweep,
    }
}

/// A copy of what [`render`] needs, taken quickly so the audio lock is not held while rendering.
pub fn bounce(live: &Shared, song: bool) -> Shared {
    let mut s = Shared::new(live.rate);
    (s.programs, s.drums, s.fx) = (live.programs.clone(), live.drums, live.fx);
    s.seqs = live.seqs.clone();
    s.master = live.master;
    (s.t.seq, s.t.solo, s.t.second) = (live.t.seq, live.t.solo, live.t.second);
    s.song = Song { steps: live.song.steps.clone(), looping: false };
    if song {
        s.play_song(0);
    } else {
        let seq = s.t.seq;
        s.seqs[seq].looping = false;
        s.play(true);
    }
    s
}

/// Plays a [`bounce`] copy without the sound card and returns the mixed output: what PLAY START
/// would sound like (the sequence once, or the song), plus the sounds ringing out afterwards.
pub fn render(mut s: Shared) -> Vec<[f32; 2]> {
    let rate = s.rate;
    let mut v = Voices::new(rate);
    let mut out = Vec::new();
    let mix = |s: &mut Shared, v: &mut Voices, out: &mut Vec<[f32; 2]>| {
        s.advance(v);
        let [l, r] = v.render(&s.fx, s.tempo());
        out.push([soft_clip(l * s.master), soft_clip(r * s.master)]);
    };
    // ponytail: renders everything into memory; an hour at 48 kHz is about 1.4 GB, so it stops
    // there. Stream to the file if longer bounces are ever needed.
    let limit = rate as usize * 3600;
    while s.t.playing && out.len() < limit {
        mix(&mut s, &mut v, &mut out);
    }
    // Held notes end with the sequence; the rest ring out, for at most ten seconds.
    for (key, _) in std::mem::take(&mut s.t.note_offs) {
        v.release(key);
    }
    for _ in 0..rate as usize * 10 {
        if v.quiet() {
            break;
        }
        mix(&mut s, &mut v, &mut out);
    }
    out
}

/// Writes stereo frames as a 24-bit WAV file.
pub fn write_wav(path: &Path, frames: &[[f32; 2]], rate: u32) -> Result<(), hound::Error> {
    let spec =
        hound::WavSpec { channels: 2, sample_rate: rate, bits_per_sample: 24, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec)?;
    for x in frames.iter().flatten() {
        w.write_sample((x.clamp(-1.0, 1.0) * 8_388_607.0) as i32)?;
    }
    w.finalize()
}

/// Loads a WAV file as stereo frames resampled to `rate`.
pub fn load_wav(path: &Path, rate: u32) -> Result<Frames, hound::Error> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|v| v as f32 * scale)).collect::<Result<_, _>>()?
        }
    };
    let channels = spec.channels.max(1) as usize;
    let frames: Vec<[f32; 2]> =
        raw.chunks_exact(channels).map(|f| if channels == 1 { [f[0], f[0]] } else { [f[0], f[1]] }).collect();
    Ok(resample(&frames, spec.sample_rate, rate).into())
}

// ponytail: linear interpolation aliases when downsampling; switch to a windowed-sinc
// resampler (e.g. rubato) if high-rate samples sound harsh.
pub fn resample(x: &[[f32; 2]], from: u32, to: u32) -> Vec<[f32; 2]> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let step = from as f64 / to as f64;
    let len = (x.len() as f64 / step) as usize;
    (0..len)
        .map(|i| {
            let t = i as f64 * step;
            let j = t as usize;
            let (a, b) = (x[j], x.get(j + 1).copied().unwrap_or(x[j]));
            let f = (t - j as f64) as f32;
            [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
        })
        .collect()
}

/// A small synthesized kit so the pads make sound before any sample is loaded.
pub fn builtin_kit(rate: u32) -> [Sound; 4] {
    let r = rate as f32;
    let render = |secs: f32, f: &mut dyn FnMut(f32) -> f32| -> Frames {
        (0..(secs * r) as usize)
            .map(|i| {
                let x = f(i as f32 / r);
                [x, x]
            })
            .collect()
    };
    let mut seed = 0x2545_f491u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };

    let mut phase = 0.0;
    let kick = render(0.45, &mut |t| {
        phase += TAU * (45.0 + 110.0 * (-t * 30.0).exp()) / r;
        phase.sin() * (-t * 9.0).exp()
    });
    let snare =
        render(0.25, &mut |t| noise() * 0.6 * (-t * 18.0).exp() + (TAU * 185.0 * t).sin() * 0.5 * (-t * 25.0).exp());
    let mut prev = 0.0;
    let hat = render(0.08, &mut |t| {
        let n = noise();
        let hp = n - prev;
        prev = n;
        hp * 0.4 * (-t * 60.0).exp()
    });
    let clap = render(0.3, &mut |t| {
        // Three short bursts, then a decaying tail.
        let env = if t < 0.03 { (-(t % 0.01) * 300.0).exp() } else { (-(t - 0.03) * 14.0).exp() * 0.8 };
        noise() * 0.6 * env
    });
    [Sound::new("KICK", kick), Sound::new("SNARE", snare), Sound::new("HAT", hat), Sound::new("CLAP", clap)]
}

/// Engine state without an audio device, for tests.
#[cfg(test)]
pub fn offline_shared(rate: u32) -> Shared {
    Shared::new(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Shared, v: &mut Voices, frames: usize) -> f32 {
        let mut peak: f32 = 0.0;
        for hit in std::mem::take(&mut s.hits) {
            s.handle_hit(hit, v);
        }
        for _ in 0..frames {
            s.advance(v);
            let [l, r] = v.render(&s.fx, 120.0);
            peak = peak.max(l.abs()).max(r.abs());
        }
        peak
    }

    fn with_kit(rate: u32) -> (Shared, Voices) {
        let (mut s, v) = (offline_shared(rate), Voices::new(rate));
        for (pad, sound) in builtin_kit(rate).into_iter().enumerate() {
            s.programs[0].pads[pad].sound = Some(sound);
        }
        (s, v)
    }

    #[test]
    fn midi_out_sends_clock_notes_and_stop() {
        let (mut s, mut v) = with_kit(48_000);
        let (tx, rx) = mpsc::channel();
        s.midi_out = Some(tx);
        s.t.clock_out = true;
        s.seqs[0].tracks[1].out = Some(9);
        s.seqs[0].tracks[1].insert(Note { tick: 0, pad: 2, velocity: 99, duration: 6, var: None });
        s.play(true);
        // Eight ticks: clocks at 0 and 4, the note at 0 and its note off at 6.
        run(&mut s, &mut v, 250 * 8);
        s.stop();
        let msgs: Vec<[u8; 3]> = rx.try_iter().collect();
        assert_eq!(msgs, [[0xFA, 0, 0], [0xF8, 0, 0], [0x99, 38, 99], [0xF8, 0, 0], [0x89, 38, 0], [0xFC, 0, 0]]);
    }

    #[test]
    fn play_from_the_middle_sends_song_position_then_continue() {
        let (mut s, _) = with_kit(48_000);
        let (tx, rx) = mpsc::channel();
        s.midi_out = Some(tx);
        s.t.clock_out = true;
        s.seqs[0].set_bars(64);
        s.locate(200 * 24);
        s.play(false);
        let msgs: Vec<[u8; 3]> = rx.try_iter().collect();
        assert_eq!(msgs, [[0xF2, 200 & 0x7F, 1], [0xFB, 0, 0]]);
    }

    #[test]
    fn external_clock_drives_tempo_and_position() {
        let (mut s, mut v) = with_kit(48_000);
        s.t.sync = true;
        s.seqs[0].tempo = 120.0;
        s.seqs[0].tracks[0].insert(Note { tick: 48, pad: 0, velocity: 100, duration: 10, var: None });
        // 90 BPM: 24 pulses per beat, one every 27 777 µs (1333 frames at 48 kHz).
        let pulse_us = 60_000_000 / (90 * 24);
        let mut stamp = 1_000_000;
        s.midi_sync(&[0xFA], stamp);
        assert!(s.t.playing);
        for _ in 0..12 {
            s.midi_sync(&[0xF8], stamp);
            run(&mut s, &mut v, 1333);
            stamp += pulse_us;
        }
        // Twelve pulses are 48 ticks; playback is held at most one pulse ahead.
        assert!((44.0..=52.0).contains(&s.t.tick), "tick {}", s.t.tick);
        assert!((s.t.ext.tempo().unwrap() - 90.0).abs() < 0.5);
        // Without pulses the playhead stops at one pulse past the last one.
        run(&mut s, &mut v, 48_000);
        assert!(s.t.tick <= 48.0);
        s.midi_sync(&[0xFC], stamp);
        assert!(!s.t.playing);
        // Song position: 8 sixteenths in.
        s.midi_sync(&[0xF2, 8, 0], stamp);
        assert_eq!(s.now(), 192);
    }

    #[test]
    fn soft_clip_is_transparent_below_the_knee_and_never_clips() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.8), -0.8);
        assert!(soft_clip(1.0) < 1.0 && soft_clip(1.0) > 0.95);
        assert!(soft_clip(100.0) <= 1.0 && soft_clip(-100.0) >= -1.0);
        let mut last = 0.0;
        for i in 0..400 {
            let y = soft_clip(i as f32 / 100.0);
            assert!(y >= last);
            last = y;
        }
    }

    #[test]
    fn resample_keeps_duration_and_values() {
        let up = resample(&[[0.0; 2], [1.0; 2], [0.0; 2], [-1.0; 2]], 24_000, 48_000);
        assert_eq!(up.len(), 8);
        assert_eq!(up[2], [1.0; 2]);
        assert!((up[1][0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn wav_roundtrip_keeps_stereo_and_scale() {
        let path = std::env::temp_dir().join(format!("rsampler-test-{}.wav", std::process::id()));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for s in [16384i16, -16384, -32768, 0] {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        let frames = load_wav(&path, 48_000).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(&*frames, &[[0.5, -0.5], [-1.0, 0.0]]);
    }

    #[test]
    fn recording_quantizes_and_plays_back() {
        let rate = 48_000;
        let (mut s, mut v) = with_kit(rate);
        s.t.count_in = false;
        s.toggle_rec(RecMode::Rec);
        s.play(true);
        assert_eq!(s.t.rec, RecMode::Rec);
        // At 120 BPM a tick is 250 frames; hit pad 0 just after the second sixteenth.
        run(&mut s, &mut v, 250 * 26);
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, 250 * 10);
        s.hits.push(Hit::Release { pad: 0 });
        run(&mut s, &mut v, 10);
        s.stop();
        let notes = &s.seq().tracks[0].notes;
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].tick, 24);
        assert_eq!(notes[0].velocity, 100);
        // Held from tick 26 to 36.
        assert!((9..=11).contains(&notes[0].duration), "duration {}", notes[0].duration);

        // Playing it back sounds at tick 24 and not before.
        let (mut s2, mut v2) = with_kit(rate);
        s2.seqs[0] = s.seq().clone();
        s2.play(true);
        assert_eq!(run(&mut s2, &mut v2, 250 * 23), 0.0);
        assert!(run(&mut s2, &mut v2, 250 * 4) > 0.1);
    }

    #[test]
    fn song_plays_steps_with_repeats_then_stops() {
        let (mut s, mut v) = with_kit(48_000);
        for seq in [3, 7] {
            s.seqs[seq].set_bars(1);
        }
        s.song.steps = vec![(3, 2), (7, 1)];
        assert!(s.play_song(0));
        // A one-bar sequence at 120 BPM is 384 ticks of 250 frames.
        let bar = 384 * 250;
        run(&mut s, &mut v, bar - 10);
        assert_eq!((s.t.seq, s.t.song), (3, Some((0, 0))));
        run(&mut s, &mut v, bar);
        assert_eq!((s.t.seq, s.t.song), (3, Some((0, 1))));
        run(&mut s, &mut v, bar);
        assert_eq!((s.t.seq, s.t.song), (7, Some((1, 0))));
        run(&mut s, &mut v, bar);
        assert!(!s.t.playing);
        assert_eq!(s.t.song, None);
    }

    #[test]
    fn note_repeat_records_on_the_grid_while_held() {
        let (mut s, mut v) = with_kit(48_000);
        s.t.count_in = false;
        s.toggle_rec(RecMode::Overdub);
        s.play(true);
        s.t.repeat = true;
        run(&mut s, &mut v, 250 * 10);
        s.hits.push(Hit::Press { pad: 1, velocity: 90, var: None });
        // Held from tick 10 to 100: the 1/16 lines 24, 48, 72 and 96 repeat it.
        run(&mut s, &mut v, 250 * 90);
        s.hits.push(Hit::Release { pad: 1 });
        run(&mut s, &mut v, 250 * 40);
        s.stop();
        let ticks: Vec<u32> = s.seq().tracks[0].notes.iter().map(|n| n.tick).collect();
        // The first hit itself is quantized to 0.
        assert_eq!(ticks, [0, 24, 48, 72, 96]);
    }

    #[test]
    fn note_variation_is_recorded_and_plays_back_retuned() {
        use crate::program::VarParam;
        let (mut s, mut v) = with_kit(48_000);
        s.t.count_in = false;
        s.toggle_rec(RecMode::Overdub);
        s.play(true);
        let var = Some((VarParam::Tune, 127));
        s.hits.push(Hit::Press { pad: 2, velocity: 100, var });
        run(&mut s, &mut v, 1);
        // An octave up plays the hi-hat at double speed.
        assert!(v.voices.iter().any(|v| v.key == 2 && (v.rate - 2.0).abs() < 0.01));
        s.hits.push(Hit::Release { pad: 2 });
        run(&mut s, &mut v, 1);
        s.stop();
        assert_eq!(s.seq().tracks[0].notes[0].var, var);
    }

    #[test]
    fn quick_tap_before_the_grid_line_stays_short() {
        let (mut s, mut v) = with_kit(48_000);
        s.t.count_in = false;
        s.toggle_rec(RecMode::Overdub);
        s.play(true);
        // Tap at tick 20, released at 22: quantized to 24, which is after the release.
        run(&mut s, &mut v, 250 * 20 + 1);
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, 250 * 2);
        s.hits.push(Hit::Release { pad: 0 });
        run(&mut s, &mut v, 1);
        s.stop();
        let n = s.seq().tracks[0].notes[0];
        assert_eq!((n.tick, n.duration), (24, 2));
    }

    #[test]
    fn rec_mode_replaces_the_old_take_then_overdubs() {
        let (mut s, mut v) = with_kit(48_000);
        s.t.count_in = false;
        s.seqs[0].set_bars(1);
        s.seqs[0].tracks[0].insert(Note { tick: 96, pad: 1, velocity: 90, duration: 10, var: None });
        s.toggle_rec(RecMode::Rec);
        s.play(true);
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, 250 * 10);
        s.hits.push(Hit::Release { pad: 0 });
        // The whole first pass: the old note at tick 96 is erased by REC.
        run(&mut s, &mut v, 250 * 390);
        assert_eq!(s.t.rec, RecMode::Overdub);
        let pads: Vec<u8> = s.seq().tracks[0].notes.iter().map(|n| n.pad).collect();
        assert_eq!(pads, [0]);
        s.stop();
        // UNDO SEQ brings the old take back.
        s.undo();
        let pads: Vec<u8> = s.seq().tracks[0].notes.iter().map(|n| n.pad).collect();
        assert_eq!(pads, [1]);
    }

    #[test]
    fn count_in_runs_one_bar_before_recording() {
        let (mut s, mut v) = with_kit(48_000);
        s.toggle_rec(RecMode::Overdub);
        s.play(true);
        assert_eq!(s.t.tick, -384.0);
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, 250 * 10);
        s.hits.push(Hit::Release { pad: 0 });
        run(&mut s, &mut v, 250 * 400);
        s.stop();
        // Hits during the count-in sound but are not recorded.
        assert!(s.seq().tracks[0].notes.is_empty());
    }

    #[test]
    fn mono_and_mute_assign_cut_voices() {
        let (mut s, mut v) = with_kit(48_000);
        s.programs[0].pads[0].overlap = Overlap::Mono;
        s.programs[0].pads[1].mutes = [Some(2), None];
        for pad in [0, 0, 2, 1] {
            s.hits.push(Hit::Press { pad, velocity: 100, var: None });
        }
        run(&mut s, &mut v, 1);
        // Pad 0's first hit and pad 2 are fading out; pad 0's second hit and pad 1 play on.
        run(&mut s, &mut v, CUT_FRAMES as usize + 2);
        let mut pads: Vec<u16> = v.voices.iter().map(|v| v.key).collect();
        pads.sort();
        assert_eq!(pads, [0, 1]);
    }

    #[test]
    fn note_off_stops_on_release() {
        let (mut s, mut v) = with_kit(48_000);
        s.programs[0].pads[0].overlap = Overlap::NoteOff;
        s.programs[0].pads[0].decay = 0;
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, 100);
        s.hits.push(Hit::Release { pad: 0 });
        run(&mut s, &mut v, 48_000 / 100);
        assert!(v.voices.is_empty());
    }

    #[test]
    fn letting_go_of_a_poly_pad_lets_it_ring() {
        let (mut s, mut v) = with_kit(48_000);
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, 10);
        s.hits.push(Hit::Release { pad: 0 });
        // Well past any release time, the kick is still sounding.
        assert!(run(&mut s, &mut v, 4_800) > 0.1);
    }

    #[test]
    fn a_loop_holds_until_release_and_play_x_plays_once() {
        let (mut s, mut v) = with_kit(48_000);
        let len = {
            let snd = s.programs[0].pads[0].sound.as_mut().unwrap();
            (snd.loop_to, snd.looping) = (snd.end / 2, true);
            snd.end
        };
        // Poly, but a looping sound still waits for its release.
        s.hits.push(Hit::Press { pad: 0, velocity: 100, var: None });
        run(&mut s, &mut v, len * 3);
        assert_eq!(v.voices.len(), 1);
        s.hits.push(Hit::Release { pad: 0 });
        run(&mut s, &mut v, 48_000);
        assert!(v.voices.is_empty());
        s.hits.push(Hit::Audition { pad: 0, range: 0..100 });
        run(&mut s, &mut v, 101);
        assert!(v.voices.is_empty());
    }

    #[test]
    fn bounce_plays_the_sequence_once_then_rings_out() {
        let (mut s, _) = with_kit(48_000);
        // One kick on the first beat of a 1-bar sequence at 120 BPM: two seconds.
        s.seqs[0].set_bars(1);
        s.seqs[0].tracks[0].insert(Note { tick: 0, pad: 0, velocity: 127, duration: 10, var: None });
        let out = render(bounce(&s, false));
        assert!(out.len() >= 96_000 && out.len() < 96_000 + 48_000);
        assert!(out[..4800].iter().any(|f| f[0].abs() > 0.1));
        assert!(out[48_000..].iter().all(|f| f[0].abs() < 1e-3));
        // The live sequence still loops; only the copy was played.
        assert!(s.seqs[0].looping);
        // An empty song gives nothing.
        assert!(render(bounce(&s, true)).is_empty());
    }

    #[test]
    fn velocity_modulation() {
        let (mut s, mut v) = with_kit(48_000);
        let peak = |s: &mut Shared, v: &mut Voices, velocity| {
            s.hits.push(Hit::Press { pad: 0, velocity, var: None });
            let p = run(s, v, 48_000);
            v.voices.clear();
            p
        };
        let (soft, hard) = (peak(&mut s, &mut v, 32), peak(&mut s, &mut v, 127));
        assert!((soft / hard - 32.0 / 127.0).abs() < 0.02);
        // Velo→Level 0: every hit at full level.
        s.programs[0].pads[0].velo.level = 0;
        assert!((peak(&mut s, &mut v, 32) - hard).abs() < 1e-3);
        // Velo→Start 100: the softest hit starts (100² + 1) / 2 ms = 5 s in, past the kick's end.
        s.programs[0].pads[0].velo.start = 100;
        s.hits.push(Hit::Press { pad: 0, velocity: 1, var: None });
        run(&mut s, &mut v, 1);
        assert!(v.voices.is_empty() || v.voices[0].pos >= v.voices[0].end - 2.0);
    }

    #[test]
    fn filter_envelope_opens_then_closes() {
        let rate = 48_000;
        let (mut s, mut v) = with_kit(rate);
        // Steady noise, so only the filter changes the level.
        let noise: Vec<[f32; 2]> =
            (0..rate as usize).map(|i| ((i * 7919 % 1000) as f32 / 500.0 - 1.0) * 0.5).map(|x| [x, x]).collect();
        s.programs[0].pads[0].sound = Some(Sound::new("NOISE", noise.into()));
        (s.programs[0].pads[0].cutoff, s.programs[0].pads[0].fenv) =
            (20, crate::program::FilterEnv { attack: 10, decay: 20, amount: 70 });
        s.hits.push(Hit::Press { pad: 0, velocity: 127, var: None });
        // Attack 50 ms, decay 200 ms: open around 50 ms, closed again after 250 ms.
        let rms = |s: &mut Shared, v: &mut Voices, frames: usize| {
            for hit in std::mem::take(&mut s.hits) {
                s.handle_hit(hit, v);
            }
            let sum: f32 = (0..frames).map(|_| v.render(&s.fx, 120.0)[0].powi(2)).sum();
            (sum / frames as f32).sqrt()
        };
        rms(&mut s, &mut v, 2_000);
        let open = rms(&mut s, &mut v, 1_000);
        rms(&mut s, &mut v, 12_000);
        let closed = rms(&mut s, &mut v, 1_000);
        assert!(open > closed * 3.0, "open {open}, closed {closed}");
    }

    #[test]
    fn punch_keeps_only_hits_inside_the_window() {
        let (mut s, mut v) = with_kit(48_000);
        s.t.count_in = false;
        s.t.punch = Punch { on: true, mode: PunchMode::InOut, from: 96, to: 192 };
        s.toggle_rec(RecMode::Overdub);
        s.play(true);
        // Hits at ticks 10, 110 and 210 (250 frames a tick at 120 BPM): only 110 is inside.
        for _ in 0..3 {
            run(&mut s, &mut v, 250 * 10);
            s.hits.push(Hit::Press { pad: 1, velocity: 100, var: None });
            run(&mut s, &mut v, 250 * 90);
            s.hits.push(Hit::Release { pad: 1 });
        }
        s.stop();
        let ticks: Vec<_> = s.seq().tracks[0].notes.iter().map(|n| n.tick).collect();
        // The hit at 110 lands on the 1/16 line at 120.
        assert_eq!(ticks, [120]);
    }

    #[test]
    fn second_sequence_plays_along() {
        let (mut s, mut v) = with_kit(48_000);
        // The main sequence is empty; sequence 2 has a kick at the start.
        s.seqs[1].tracks[0].insert(Note { tick: 0, pad: 0, velocity: 127, duration: 10, var: None });
        s.play(true);
        assert_eq!(run(&mut s, &mut v, 2_000), 0.0);
        s.stop();
        s.stop();
        s.t.second = (true, 1);
        s.play(true);
        assert!(run(&mut s, &mut v, 2_000) > 0.1);
    }

    #[test]
    fn tracks_play_the_program_of_their_drum_bus() {
        let (mut s, mut v) = with_kit(48_000);
        // Program 2 has the kit's snare on its pad A01, and DRUM2 plays it.
        let mut second = Program::new("TWO");
        second.pads[0].sound = s.programs[0].pads[1].sound.clone();
        s.programs.push(second);
        s.drums[1] = 1;
        let note = Note { tick: 0, pad: 0, velocity: 100, duration: 10, var: None };
        s.seqs[0].tracks[0].insert(note);
        s.seqs[0].tracks[1].insert(note);
        s.seqs[0].tracks[1].bus = 2;
        // A MIDI track makes no sound of its own.
        s.seqs[0].tracks[2].insert(note);
        s.seqs[0].tracks[2].bus = 0;
        s.play(true);
        run(&mut s, &mut v, 10);
        let mut keys: Vec<u16> = v.voices.iter().map(|v| v.key).collect();
        keys.sort();
        assert_eq!(keys, [0, 64]);
        // The pads play the selected track's program.
        s.stop();
        s.t.track = 1;
        assert_eq!(s.program(), 1);
        assert!(s.pads()[0].sound.as_ref().is_some_and(|x| x.name == "SNARE"));
    }

    #[test]
    fn midi_tracks_keep_whole_notes_and_make_no_sound() {
        let (mut s, mut v) = with_kit(48_000);
        let (tx, rx) = mpsc::channel();
        s.midi_out = Some(tx);
        s.t.count_in = false;
        let track = &mut s.seqs[0].tracks[0];
        (track.bus, track.out) = (0, Some(0));
        // Note 110 is past the pads; recorded, it stays 110 and goes out as 110.
        s.toggle_rec(RecMode::Overdub);
        s.play(true);
        run(&mut s, &mut v, 250 * 2);
        s.hits.push(Hit::Press { pad: 110, velocity: 80, var: None });
        assert_eq!(run(&mut s, &mut v, 250 * 3), 0.0);
        s.hits.push(Hit::Release { pad: 110 });
        run(&mut s, &mut v, 10);
        s.stop();
        assert_eq!(s.seq().tracks[0].notes[0].pad, 110);
        let msgs: Vec<[u8; 3]> = rx.try_iter().collect();
        assert!(msgs.contains(&[0x90, 110, 80]) && msgs.contains(&[0x80, 110, 0]));
        // Played back: out again, still silent.
        s.play(true);
        assert_eq!(run(&mut s, &mut v, 250 * 4), 0.0);
        assert!(rx.try_iter().any(|m| m == [0x90, 110, 80]));
    }

    #[test]
    fn tune_shortens_playback() {
        let (mut s, mut v) = with_kit(48_000);
        s.programs[0].pads[2].tune = 120; // one octave up halves the hi-hat's 0.08 s
        s.hits.push(Hit::Press { pad: 2, velocity: 100, var: None });
        run(&mut s, &mut v, 1925);
        assert!(v.voices.is_empty());
    }
}
