//! MPC2000XL `.SND` sound files: a 42-byte header, then 16-bit samples, with the right channel of
//! a stereo sound stored after the whole left channel.

use std::path::Path;

use crate::audio::resample;
use crate::program::Sound;

const HEADER: usize = 42;

pub struct Snd {
    pub name: String,
    pub frames: Vec<[f32; 2]>,
    pub rate: u32,
    pub start: usize,
    pub end: usize,
    /// Tenths of a semitone, like a pad's `Tune`.
    pub tune: i32,
}

pub fn parse(b: &[u8]) -> Result<Snd, String> {
    if b.len() < HEADER || b[..2] != [0x01, 0x04] {
        return Err("Not an MPC2000XL sound".to_owned());
    }
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
    let name = String::from_utf8_lossy(b[2..19].split(|c| *c == 0).next().unwrap_or_default()).trim().to_owned();
    let tune = b[20] as i8 as i32;
    let stereo = b[21] & 1 == 1;
    let (start, end, count) = (u32_at(22), u32_at(26), u32_at(30));
    let rate = u16::from_le_bytes([b[40], b[41]]) as u32;
    let channels = if stereo { 2 } else { 1 };
    let data = &b[HEADER..];
    if rate == 0 || data.len() < count * 2 * channels {
        return Err("Damaged sound file".to_owned());
    }
    let sample = |i: usize| i16::from_le_bytes([data[2 * i], data[2 * i + 1]]) as f32 / 32768.0;
    let frames = (0..count)
        .map(|i| {
            let l = sample(i);
            [l, if stereo { sample(count + i) } else { l }]
        })
        .collect();
    let end = end.clamp(1, count.max(1));
    Ok(Snd { name, frames, rate, start: start.min(end - 1), end, tune: tune.clamp(-120, 120) })
}

/// Loads a `.SND` file resampled to `rate`, keeping its trim in proportion, and returns its tune.
pub fn load(path: &Path, rate: u32) -> Result<(Sound, i32), String> {
    let snd = parse(&std::fs::read(path).map_err(|e| e.to_string())?)?;
    let frames = resample(&snd.frames, snd.rate, rate);
    let len = frames.len().max(1);
    let scale = |v: usize| (v as u64 * len as u64 / snd.frames.len().max(1) as u64) as usize;
    let end = scale(snd.end).clamp(1, len);
    let start = scale(snd.start).min(end - 1);
    let name = if snd.name.is_empty() {
        path.file_stem().unwrap_or_default().to_string_lossy().into_owned()
    } else {
        snd.name
    };
    Ok((Sound { name, frames: frames.into(), start, end }, snd.tune))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(stereo: bool, frames: u32, start: u32, end: u32, tune: i8) -> Vec<u8> {
        let mut b = vec![0x01, 0x04];
        b.extend(b"kick1\0\0\0\0\0\0\0\0\0\0\0\0");
        b.extend([100, tune as u8, stereo as u8]);
        for v in [start, end, frames, 0] {
            b.extend(v.to_le_bytes());
        }
        b.extend([0, 4]);
        b.extend(44_100u16.to_le_bytes());
        b
    }

    #[test]
    fn stereo_channels_follow_each_other() {
        let mut b = header(true, 3, 1, 3, -15);
        for s in [100i16, 200, 300, -100, -200, -300] {
            b.extend(s.to_le_bytes());
        }
        let snd = parse(&b).unwrap();
        assert_eq!(snd.name, "kick1");
        assert_eq!((snd.rate, snd.start, snd.end, snd.tune), (44_100, 1, 3, -15));
        let f = |v: i16| v as f32 / 32768.0;
        assert_eq!(snd.frames, [[f(100), f(-100)], [f(200), f(-200)], [f(300), f(-300)]]);
    }

    #[test]
    fn mono_and_damaged_files() {
        let mut b = header(false, 2, 0, 2, 0);
        b.extend([1, 0, 2, 0]);
        assert_eq!(parse(&b).unwrap().frames.len(), 2);
        assert!(parse(&b[..b.len() - 1]).is_err());
        assert!(parse(b"RIFF....").is_err());
    }
}
