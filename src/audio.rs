use std::error::Error;
use std::f32::consts::TAU;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, mpsc};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Mono audio at the output device's sample rate.
pub type Sample = Arc<[f32]>;

struct Voice {
    data: Sample,
    pos: usize,
    gain: f32,
}

pub struct Engine {
    _stream: cpal::Stream,
    voices: mpsc::Sender<Voice>,
    volume: Arc<AtomicU32>,
    pub rate: u32,
}

impl Engine {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = cpal::default_host().default_output_device().ok_or("no audio output device")?;
        let config = device.default_output_config()?.config();
        let rate = config.sample_rate;
        let channels = config.channels as usize;

        let (tx, rx) = mpsc::channel::<Voice>();
        let volume = Arc::new(AtomicU32::new(0.8f32.to_bits()));
        let master = volume.clone();
        let mut voices: Vec<Voice> = Vec::with_capacity(64);

        // ponytail: always asks for f32 output, which PipeWire/Pulse/WASAPI accept; match on
        // `sample_format()` if a device turns up that only takes integer samples.
        let stream = device.build_output_stream(
            config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                voices.extend(rx.try_iter());
                let master = f32::from_bits(master.load(Ordering::Relaxed));
                for frame in out.chunks_mut(channels) {
                    let mut mix = 0.0;
                    for v in &mut voices {
                        if let Some(x) = v.data.get(v.pos) {
                            mix += x * v.gain;
                            v.pos += 1;
                        }
                    }
                    frame.fill((mix * master).clamp(-1.0, 1.0));
                }
                voices.retain(|v| v.pos < v.data.len());
            },
            |err| eprintln!("audio: {err}"),
            None,
        )?;
        stream.play()?;
        Ok(Self { _stream: stream, voices: tx, volume, rate })
    }

    pub fn play(&self, data: &Sample, gain: f32) {
        let _ = self.voices.send(Voice { data: data.clone(), pos: 0, gain });
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume.to_bits(), Ordering::Relaxed);
    }
}

/// Loads a WAV file, mixed down to mono and resampled to `rate`.
pub fn load_wav(path: &Path, rate: u32) -> Result<Sample, hound::Error> {
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
    let mono: Vec<f32> = raw.chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect();
    Ok(resample(&mono, spec.sample_rate, rate).into())
}

// ponytail: linear interpolation aliases when downsampling; switch to a windowed-sinc
// resampler (e.g. rubato) if high-rate samples sound harsh.
fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let step = from as f64 / to as f64;
    let len = (x.len() as f64 / step) as usize;
    (0..len)
        .map(|i| {
            let t = i as f64 * step;
            let j = t as usize;
            let a = x[j];
            let b = x.get(j + 1).copied().unwrap_or(a);
            a + (b - a) * (t - j as f64) as f32
        })
        .collect()
}

/// A small synthesized kit so the pads make sound before any sample is loaded.
pub fn builtin_kit(rate: u32) -> [(&'static str, Sample); 4] {
    let r = rate as f32;
    let render = |secs: f32, f: &mut dyn FnMut(f32) -> f32| -> Sample {
        (0..(secs * r) as usize).map(|i| f(i as f32 / r)).collect()
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
    [("KICK", kick), ("SNARE", snare), ("HAT", hat), ("CLAP", clap)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_keeps_duration_and_values() {
        let up = resample(&[0.0, 1.0, 0.0, -1.0], 24_000, 48_000);
        assert_eq!(up.len(), 8);
        assert_eq!(up[2], 1.0);
        assert!((up[1] - 0.5).abs() < 1e-6);
        assert_eq!(resample(&[0.5; 10], 48_000, 48_000), vec![0.5; 10]);
    }

    #[test]
    fn wav_roundtrip_is_mono_and_scaled() {
        let path = std::env::temp_dir().join("rsampler-test.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for s in [16384i16, 16384, -32768, 0] {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        let sample = load_wav(&path, 48_000).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(&*sample, &[0.5, -0.5]);
    }
}
