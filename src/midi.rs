//! MIDI input: notes 36–99 (C1–D#6) play pads A01–D16 with their velocity. rsampler connects to
//! every MIDI input it finds and, on Linux, also offers its own "rsampler" input port.
//!
//! MIDI output goes to an "rsampler" output port on Linux, for synths and DAWs to connect to.

use std::sync::{Arc, Mutex, mpsc};

use midir::{Ignore, MidiInput, MidiInputConnection};

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

/// Opens the output port and returns where to send messages; `None` on Windows, which has no
/// virtual MIDI ports.
pub fn start_out() -> Option<mpsc::Sender<[u8; 3]>> {
    #[cfg(unix)]
    {
        use midir::os::unix::VirtualOutput;
        let mut conn = midir::MidiOutput::new("rsampler").ok()?.create_virtual("rsampler").ok()?;
        let (tx, rx) = mpsc::channel::<[u8; 3]>();
        // The audio thread must not wait on the MIDI system, so a thread does the sending.
        std::thread::spawn(move || {
            for msg in rx {
                let len = if msg[0] >= 0xF8 { 1 } else { 3 };
                let _ = conn.send(&msg[..len]);
            }
        });
        Some(tx)
    }
    #[cfg(not(unix))]
    None
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
