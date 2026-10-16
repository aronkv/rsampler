//! MIDI input: notes 36–99 (C1–D#6) play pads A01–D16 with their velocity. rsampler connects to
//! every MIDI input it finds and, on Linux, also offers its own "rsampler" input port.

use std::sync::{Arc, Mutex};

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
            input().and_then(|i| i.create_virtual("rsampler", move |_, msg, _| forward(&shared, msg), ()).ok())
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
            if let Ok(conn) = input.connect(&port, "rsampler", move |_, msg, _| forward(&shared, msg), ()) {
                added.push(name.clone());
                self.connections.push((name, conn));
            }
        }
        added
    }
}

fn input() -> Option<MidiInput> {
    let mut input = MidiInput::new("rsampler").ok()?;
    input.ignore(Ignore::All);
    Some(input)
}

fn forward(shared: &Mutex<Shared>, msg: &[u8]) {
    if let Some(hit) = parse(msg) {
        lock(shared).hits.push(hit);
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
