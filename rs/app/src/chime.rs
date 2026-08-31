//! Two sounds, modelled rather than sampled.
//!
//! A recording would mean a licence and a megabyte. These are physics: the
//! shapes real objects make, written as arithmetic and rendered once into the
//! app's own directory. A pure sine is what makes an app sound like a machine;
//! what makes it sound natural is the *shape* — inharmonic partials for wood,
//! a rising pitch for a droplet, a noise transient at the moment of contact.
//!
//! - **press** — a struck wooden bar. Its partials sit at the free-bar ratios
//!   (1, 2.76, 5.40), which is why wood sounds like wood and not like a bell,
//!   and the higher ones die first, as they do in a real bar.
//! - **share** — a drop of water. The pitch *rises* as the cavity it leaves
//!   behind closes; that rise is the whole reason a droplet is recognisable,
//!   and getting it right means integrating the frequency rather than
//!   multiplying it by time.
//!
//! Both are quiet on purpose. An app you leave running for days must never
//! make a noise you grow to resent.

use std::path::PathBuf;

const RATE: u32 = 44_100;

/// A tiny deterministic noise source. Real contact starts with a broadband
/// click; without it a struck object sounds like a tone generator pretending.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

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

/// A struck wooden bar: inharmonic partials at the free-bar ratios, the high
/// ones decaying first, over a filtered noise transient for the contact.
fn press_samples() -> Vec<i16> {
    let n = (RATE as f32 * 0.16) as usize;
    let mut rng = Noise(0x5eed_1234);
    let mut lp = 0.0f32; // one-pole lowpass: a knock is dull, not bright
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f0 = 392.0; // G4 — low enough to be a knock, not a beep
            // free-bar modes; each higher one is quieter and dies sooner
            let modes = [(1.0f32, 1.0f32, 9.0f32), (2.76, 0.42, 15.0), (5.40, 0.16, 26.0)];
            let mut v = 0.0;
            for (ratio, amp, decay) in modes {
                v += (t * f0 * ratio * std::f32::consts::TAU).sin()
                    * amp
                    * (-t * decay).exp();
            }
            // the click of contact: a few milliseconds of dull noise
            let raw = rng.next();
            lp += (raw - lp) * 0.12;
            v += lp * 1.6 * (-t * 420.0).exp();
            // never start on a step
            let attack = 1.0 - (-t * 1_500.0).exp();
            (v * attack * 3_400.0).clamp(-30_000.0, 30_000.0) as i16
        })
        .collect()
}

/// A drop of water. The pitch rises as the air cavity it leaves behind
/// collapses — that rise is what makes a droplet recognisable as one. The
/// phase is the *integral* of the rising frequency; multiplying frequency by
/// time instead is the usual mistake, and it sounds like a slide whistle.
fn share_samples() -> Vec<i16> {
    let total = (RATE as f32 * 0.75) as usize;
    // two drops, the second smaller and a little later: one plink is an
    // accident, two is something arriving.
    let drops = [(0.00f32, 620.0f32, 0.052f32, 1.00f32), (0.19, 880.0, 0.040, 0.55)];
    (0..total)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let mut v = 0.0f32;
            for (start, f0, tau_f, amp) in drops {
                if t < start {
                    continue;
                }
                let u = t - start;
                // φ(u) = 2π f0 τ (e^{u/τ} − 1)
                let phase =
                    std::f32::consts::TAU * f0 * tau_f * ((u / tau_f).exp() - 1.0);
                let env = (-u * 26.0).exp() * (1.0 - (-u * 700.0).exp());
                v += phase.sin() * env * amp;
            }
            (v * 4_200.0).clamp(-30_000.0, 30_000.0) as i16
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
