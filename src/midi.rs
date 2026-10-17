//! MIDI input: notes 36–99 (C1–D#6) play pads A01–D16 with their velocity. rsampler connects to
//! every MIDI input it finds and, on Linux, also offers its own "rsampler" input port.
//!
//! MIDI output goes to the port chosen in SETUP: on Linux by default rsampler's own "rsampler"
//! port, for synths and DAWs to connect to; or straight to a synth, interface or, on Windows, the
//! system's own synth.

use std::sync::{Arc, Mutex, mpsc};

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::audio::{Hit, Shared, lock};

/// The note that plays pad A01; the 64 pads follow chromatically.
pub const FIRST_NOTE: u8 = 36;

pub fn parse(msg: &[u8]) -> Option<Hit> {
    let [status, note, velocity] = *msg else { return None };
    let pad = note.checked_sub(FIRST_NOTE).filter(|p| *p < 64)?;
    match status & 0xF0 {
        0x90 if velocity > 0 => Some(Hit::Press { pad, velocity, var: None }),
        // Note on with velocity 0 is a note off.
        0x80 | 0x90 => Some(Hit::Release { pad }),
        _ => None,
    }
}

pub struct Midi {
    shared: Arc<Mutex<Shared>>,
    connections: Vec<(String, MidiInputConnection<()>)>,
    _virtual: Option<MidiInputConnection<()>>,
}

impl Midi {
    pub fn start(shared: Arc<Mutex<Shared>>) -> Self {
        #[cfg(unix)]
        let virtual_port = {
            use midir::os::unix::VirtualInput;
            let shared = shared.clone();
            input()
                .and_then(|i| i.create_virtual("rsampler", move |stamp, msg, _| forward(&shared, stamp, msg), ()).ok())
        };
        #[cfg(not(unix))]
        let virtual_port = None;
        Self { shared, connections: Vec::new(), _virtual: virtual_port }
    }

    /// Connects to inputs that appeared since the last scan and drops the ones that went away.
    /// Returns the names of new connections.
    pub fn scan(&mut self) -> Vec<String> {
        let Some(probe) = input() else { return Vec::new() };
        let ports: Vec<_> = probe
            .ports()
            .into_iter()
            .filter_map(|port| probe.port_name(&port).ok().map(|name| (name, port)))
            // Our own port and the loopback would only echo.
            .filter(|(name, _)| !name.contains("rsampler") && !name.contains("Midi Through"))
            .collect();
        self.connections.retain(|(name, _)| ports.iter().any(|(n, _)| n == name));
        let mut added = Vec::new();
        for (name, port) in ports {
            if self.connections.iter().any(|(n, _)| *n == name) {
                continue;
            }
            let shared = self.shared.clone();
            let Some(input) = input() else { break };
            if let Ok(conn) = input.connect(&port, "rsampler", move |stamp, msg, _| forward(&shared, stamp, msg), ()) {
                added.push(name.clone());
                self.connections.push((name, conn));
            }
        }
        added
    }
}

/// MIDI out port choices besides the system's ports.
pub const OUT_OFF: &str = "OFF";
/// rsampler's own port, which other programs connect to (not on Windows).
pub const OUT_VIRTUAL: &str = "rsampler (own port)";

/// Where MIDI output goes. A thread sends the messages, so the audio thread never waits on the
/// MIDI system.
pub struct MidiOut {
    conn: Arc<Mutex<Option<MidiOutputConnection>>>,
    pub port: String,
}

impl MidiOut {
    /// Starts on rsampler's own port where there is one, else off; returns where to send.
    pub fn start() -> (Self, mpsc::Sender<[u8; 3]>) {
        let conn: Arc<Mutex<Option<MidiOutputConnection>>> = Arc::default();
        let (tx, rx) = mpsc::channel::<[u8; 3]>();
        let sender = conn.clone();
        std::thread::spawn(move || {
            for msg in rx {
                let len = if msg[0] >= 0xF8 { 1 } else { 3 };
                if let Some(c) = sender.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                    let _ = c.send(&msg[..len]);
                }
            }
        });
        let mut out = Self { conn, port: OUT_OFF.to_owned() };
        if cfg!(unix) {
            let _ = out.select(OUT_VIRTUAL);
        }
        (out, tx)
    }

    /// The ports to choose from: OFF, our own port where there is one, then the system's.
    pub fn choices() -> Vec<String> {
        let mut list = vec![OUT_OFF.to_owned()];
        if cfg!(unix) {
            list.push(OUT_VIRTUAL.to_owned());
        }
        if let Ok(probe) = MidiOutput::new("rsampler") {
            list.extend(
                probe
                    .ports()
                    .iter()
                    .filter_map(|p| probe.port_name(p).ok())
                    .filter(|n| !n.contains("rsampler") && !n.contains("Midi Through")),
            );
        }
        list
    }

    pub fn select(&mut self, name: &str) -> Result<(), String> {
        let mut slot = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        // The old connection goes first, so a port can be reopened.
        *slot = None;
        self.port = OUT_OFF.to_owned();
        let out = MidiOutput::new("rsampler").map_err(|e| e.to_string())?;
        let conn = match name {
            OUT_OFF => return Ok(()),
            #[cfg(unix)]
            OUT_VIRTUAL => {
                use midir::os::unix::VirtualOutput;
                out.create_virtual("rsampler").map_err(|e| e.to_string())?
            }
            _ => {
                let port = out
                    .ports()
                    .into_iter()
                    .find(|p| out.port_name(p).is_ok_and(|n| n == name))
                    .ok_or("The port is gone")?;
                out.connect(&port, "rsampler").map_err(|e| e.to_string())?
            }
        };
        *slot = Some(conn);
        self.port = name.to_owned();
        Ok(())
    }
}

fn input() -> Option<MidiInput> {
    let mut input = MidiInput::new("rsampler").ok()?;
    // Clock and transport messages are needed for sync.
    input.ignore(Ignore::SysexAndActiveSense);
    Some(input)
}

fn forward(shared: &Mutex<Shared>, stamp_us: u64, msg: &[u8]) {
    if let Some(hit) = parse(msg) {
        lock(shared).hits.push(hit);
    } else if matches!(msg.first(), Some(0xF2 | 0xF8 | 0xFA | 0xFB | 0xFC)) {
        lock(shared).midi_sync(msg, stamp_us);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_map_to_pads() {
        assert_eq!(parse(&[0x90, 36, 100]), Some(Hit::Press { pad: 0, velocity: 100, var: None }));
        assert_eq!(parse(&[0x99, 99, 1]), Some(Hit::Press { pad: 63, velocity: 1, var: None }));
        assert_eq!(parse(&[0x80, 37, 64]), Some(Hit::Release { pad: 1 }));
        assert_eq!(parse(&[0x90, 37, 0]), Some(Hit::Release { pad: 1 }));
        assert_eq!(parse(&[0x90, 35, 100]), None);
        assert_eq!(parse(&[0x90, 100, 100]), None);
        assert_eq!(parse(&[0xB0, 36, 100]), None);
        assert_eq!(parse(&[0xF8]), None);
    }
}
