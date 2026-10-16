//! SAMPLE mode: recording a new sound from the audio input.

use std::sync::{Arc, Mutex, MutexGuard};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idle,
    /// Waiting for the input to reach the threshold.
    Armed,
    Recording,
    /// Finished; waiting for KEEP or RETRY.
    Done,
}

pub struct Capture {
    /// Input peak per channel, decaying, after the gain.
    pub peak: [f32; 2],
    pub state: State,
    /// Linear level that starts the recording; 0 starts at once.
    pub threshold: f32,
    pub gain: f32,
    pub mono: bool,
    pub max_frames: usize,
    pub frames: Vec<[f32; 2]>,
    pub rate: u32,
}

impl Capture {
    fn new(rate: u32) -> Self {
        Self {
            peak: [0.0; 2],
            state: State::Idle,
            threshold: 0.0,
            gain: 1.0,
            mono: false,
            max_frames: rate as usize * 4,
            frames: Vec::new(),
            rate,
        }
    }

    /// Feeds one block of interleaved input.
    fn feed(&mut self, data: &[f32], channels: usize) {
        self.peak = self.peak.map(|p| p * 0.9);
        for f in data.chunks_exact(channels) {
            let l = f[0] * self.gain;
            let r = f.get(1).map_or(l, |r| r * self.gain);
            let frame = if self.mono { [(l + r) / 2.0; 2] } else { [l, r] };
            self.peak = [self.peak[0].max(frame[0].abs()), self.peak[1].max(frame[1].abs())];
            if self.state == State::Armed && frame[0].abs().max(frame[1].abs()) >= self.threshold {
                self.state = State::Recording;
            }
            if self.state == State::Recording {
                self.frames.push(frame);
                if self.frames.len() >= self.max_frames {
                    self.state = State::Done;
                }
            }
        }
    }

    pub fn arm(&mut self) {
        self.frames.clear();
        self.state = State::Armed;
    }

    /// Stops a recording early; a recording that never started is cancelled.
    pub fn stop(&mut self) {
        self.state = match self.state {
            State::Recording if !self.frames.is_empty() => State::Done,
            State::Done => State::Done,
            _ => State::Idle,
        };
    }
}

/// An open input device. Dropping it closes the device.
pub struct Input {
    _stream: cpal::Stream,
    pub capture: Arc<Mutex<Capture>>,
    pub name: String,
}

impl Input {
    pub fn open() -> Result<Self, String> {
        let device = cpal::default_host().default_input_device().ok_or("No audio input")?;
        let name = device.description().map(|d| d.name().to_owned()).unwrap_or_else(|_| "Input".to_owned());
        let config = device.default_input_config().map_err(|e| e.to_string())?.config();
        let channels = config.channels as usize;
        let capture = Arc::new(Mutex::new(Capture::new(config.sample_rate)));
        let shared = capture.clone();
        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| lock(&shared).feed(data, channels),
                |err| eprintln!("audio input: {err}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream, capture, name })
    }

    pub fn lock(&self) -> MutexGuard<'_, Capture> {
        lock(&self.capture)
    }
}

fn lock(capture: &Mutex<Capture>) -> MutexGuard<'_, Capture> {
    capture.lock().unwrap_or_else(|e| e.into_inner())
}

/// Level in dBFS for the meters and the threshold.
pub fn db(level: f32) -> f32 {
    20.0 * level.max(1e-6).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_starts_and_length_stops_the_recording() {
        let mut c = Capture::new(100);
        c.threshold = 0.5;
        c.max_frames = 3;
        c.arm();
        c.feed(&[0.1, 0.1, 0.2, 0.2], 2);
        assert_eq!(c.state, State::Armed);
        c.feed(&[0.6, 0.0, 0.3, 0.3, 0.1, 0.1, 0.9, 0.9], 2);
        assert_eq!(c.state, State::Done);
        assert_eq!(c.frames, [[0.6, 0.0], [0.3, 0.3], [0.1, 0.1]]);
    }

    #[test]
    fn mono_input_and_gain() {
        let mut c = Capture::new(100);
        c.gain = 2.0;
        c.arm();
        c.feed(&[0.25, -0.25], 1);
        assert_eq!(c.frames, [[0.5, 0.5], [-0.5, -0.5]]);
        c.mono = true;
        c.arm();
        c.feed(&[0.25, 0.0], 2);
        assert_eq!(c.frames, [[0.25, 0.25]]);
        c.stop();
        assert_eq!(c.state, State::Done);
    }

    #[test]
    fn stopping_before_the_threshold_cancels() {
        let mut c = Capture::new(100);
        c.threshold = 1.0;
        c.arm();
        c.feed(&[0.1, 0.1], 2);
        c.stop();
        assert_eq!(c.state, State::Idle);
    }
}
