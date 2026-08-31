//! Two sounds, synthesised rather than sampled.
//!
//! A recording would mean a licence and a megabyte; these are a few hundred
//! lines of arithmetic that render once into the app's own directory and are
//! then played by `afplay`. Both are deliberately soft: an app you leave
//! running for days must never make a noise you grow to resent.
//!
//! - **press** — a wooden pluck, two harmonics under a fast decay.
//! - **share** — a bird: two rising chirps with vibrato, the sound of
//!   something small arriving, which is exactly what a share is.

use std::path::PathBuf;

const RATE: u32 = 44_100;

fn dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/ai.cyber.erga/sound"))
}

/// A 16-bit mono WAV around the samples.
fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM header size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// A soft pluck: a fundamental and its fifth, decaying quickly. Wooden rather
/// than glassy, so it reads as a button and not as an alert.
fn press_samples() -> Vec<i16> {
    let n = (RATE as f32 * 0.11) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let env = (-t * 34.0).exp() * (1.0 - (-t * 900.0).exp());
            let v = (t * 523.25 * std::f32::consts::TAU).sin() * 0.6
                + (t * 783.99 * std::f32::consts::TAU).sin() * 0.25;
            (v * env * 8_000.0) as i16
        })
        .collect()
}

/// A bird: two short rising sweeps with a little vibrato, the second higher
/// than the first. Something small has arrived.
fn share_samples() -> Vec<i16> {
    let total = (RATE as f32 * 0.42) as usize;
    (0..total)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            // two chirps: 0.00–0.13 s and 0.18–0.33 s
            let (start, base, rise) = if t < 0.15 {
                (0.0, 1_760.0, 900.0)
            } else if t >= 0.18 && t < 0.36 {
                (0.18, 2_093.0, 1_150.0)
            } else {
                return 0;
            };
            let u = t - start;
            let span = if start == 0.0 { 0.13 } else { 0.16 };
            if u > span {
                return 0;
            }
            let k = u / span;
            // rise, then ease off at the very end — a real chirp bends
            let freq = base + rise * (k * 1.35).min(1.0) + 40.0 * (u * 90.0).sin();
            let env = (k * std::f32::consts::PI).sin().powf(0.7);
            let v = (u * freq * std::f32::consts::TAU).sin();
            (v * env * 5_200.0) as i16
        })
        .collect()
}

/// Render both sounds once. Cheap enough to do at startup and idempotent.
pub fn ensure() {
    let Some(d) = dir() else { return };
    let _ = std::fs::create_dir_all(&d);
    for (name, samples) in [("press.wav", press_samples()), ("share.wav", share_samples())] {
        let p = d.join(name);
        if !p.exists() {
            let _ = std::fs::write(p, wav(&samples));
        }
    }
}

/// Play one, without blocking the frame. A failure to make a noise is never
/// worth interrupting mining over.
fn play(name: &str) {
    let Some(d) = dir() else { return };
    let p = d.join(name);
    if !p.exists() {
        return;
    }
    let _ = std::process::Command::new("afplay")
        .arg(p)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

pub fn press() {
    play("press.wav");
}

pub fn share() {
    play("share.wav");
}
