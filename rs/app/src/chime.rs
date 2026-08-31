//! Two sounds, modelled rather than sampled.
//!
//! A recording would mean a licence and a megabyte. These are physics —
//! rendered once into the app's own directory and played with `afplay`.
//!
//! The method matters more than the numbers. Adding sine partials together
//! and fading them out is how an app ends up sounding like a tone generator:
//! real objects are not sums of sines, they are *bodies that ring when
//! something hits them*. So each sound here is a short burst of noise — the
//! contact — poured through resonators tuned to the modes of the body. The
//! attack is noisy and the tail is tonal, which is the shape an ear reads as
//! a physical event.
//!
//! - **press** — a fingertip on a wooden bar. The partials sit at the free-bar
//!   ratios (1, 2.76, 5.40, 8.93), which is why wood sounds like wood and not
//!   like a bell, and the higher ones die first, as they do in a real bar.
//! - **share** — a drop of water into a bowl. The pitch *rises* as the cavity
//!   the drop leaves behind collapses; that rise is the whole reason a droplet
//!   is recognisable, and getting it right means integrating the frequency
//!   rather than multiplying it by time. Under it, the bowl rings from the
//!   splash.
//!
//! Nothing in nature repeats exactly, and an identical sample played for the
//! hundredth time is precisely what reads as *machine*. Each sound is rendered
//! in three slightly different strikes and they are played in rotation, so no
//! two in a row are the same.
//!
//! Both are quiet on purpose. An app you leave running for days must never
//! make a noise you grow to resent.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const RATE: u32 = 44_100;
/// Bump when the sounds change, so a new build replaces the old renders
/// instead of playing whatever an earlier version left behind.
const DESIGN: u32 = 2;
/// How many strikes of each sound are rendered and rotated through.
const VARIANTS: usize = 3;

/// A tiny deterministic noise source. Real contact starts broadband; without
/// it a struck object sounds like a tone generator pretending.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// One mode of a body: a two-pole resonator that rings at `f` and fades over
/// `tau`. Feed it noise and it sings; feed it silence and it dies away.
///
/// y[n] = x[n] + 2r·cos(ω)·y[n−1] − r²·y[n−2]
struct Mode {
    b1: f32,
    b2: f32,
    gain: f32,
    y1: f32,
    y2: f32,
}

impl Mode {
    fn new(f: f32, tau: f32, amp: f32) -> Mode {
        let r = (-1.0 / (tau * RATE as f32)).exp();
        let w = std::f32::consts::TAU * f / RATE as f32;
        Mode {
            b1: 2.0 * r * w.cos(),
            b2: r * r,
            // (1 − r) keeps a long tail from being louder than a short one
            gain: amp * (1.0 - r),
            y1: 0.0,
            y2: 0.0,
        }
    }
    fn tick(&mut self, x: f32) -> f32 {
        let y = x + self.b1 * self.y1 - self.b2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y * self.gain
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

/// Scale to a peak and clip nothing. Rendering to a fixed headroom is what
/// keeps one sound from being twice as loud as the other by accident.
fn normalize(mut v: Vec<f32>, peak: f32) -> Vec<i16> {
    let max = v.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 1e-6 {
        let k = peak / max;
        for s in &mut v {
            *s *= k;
        }
    }
    v.iter().map(|s| (s * 32_000.0).clamp(-32_000.0, 32_000.0) as i16).collect()
}

/// A fingertip on a wooden bar: a few milliseconds of dull contact noise,
/// poured into the bar's own modes.
///
/// `k` is which strike this is — no two are identical, because no two are in
/// life. It moves the pitch by a few parts in a hundred and re-rolls the
/// noise, which is the difference between a sound and a sample.
fn press_samples(k: usize) -> Vec<i16> {
    let n = (RATE as f32 * 0.14) as usize;
    let mut rng = Noise(0x5eed_1234 ^ ((k as u32 + 1) * 0x9e37_79b9));
    let detune = 1.0 + (k as f32 - 1.0) * 0.023;
    let f0 = 322.0 * detune;

    // free-bar modes: the higher ones are quieter and die sooner
    let mut modes: Vec<Mode> = [
        (1.00f32, 1.00f32, 0.058f32),
        (2.76, 0.46, 0.034),
        (5.40, 0.19, 0.019),
        (8.93, 0.07, 0.011),
    ]
    .iter()
    .map(|&(ratio, amp, tau)| Mode::new(f0 * ratio, tau, amp))
    .collect();

    let mut lp = 0.0f32; // a knock is dull, not bright
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        // the contact: about three milliseconds of it, and then the bar is
        // on its own
        let raw = rng.next();
        lp += (raw - lp) * 0.34;
        let hit = lp * (-t * 900.0).exp();
        let mut v = 0.0;
        for m in &mut modes {
            v += m.tick(hit);
        }
        out.push(v);
    }
    normalize(out, 0.24)
}

/// A drop of water into a bowl.
///
/// The plink is a rising tone: φ(u) = 2π f₀ τ (e^{u/τ} − 1). Multiplying the
/// frequency by time instead of integrating it is the usual mistake, and it
/// sounds like a slide whistle. Under it the bowl rings from the splash, which
/// is what places the drop somewhere rather than nowhere.
fn share_samples(k: usize) -> Vec<i16> {
    let total = (RATE as f32 * 0.62) as usize;
    let mut rng = Noise(0x1d0b_9f21 ^ ((k as u32 + 1) * 0x85eb_ca6b));
    let detune = 1.0 + (k as f32 - 1.0) * 0.031;

    // two drops: one plink is an accident, two is something arriving
    let drops = [
        (0.00f32, 545.0f32 * detune, 0.055f32, 1.00f32),
        (0.21, 762.0 * detune, 0.043, 0.52),
    ];
    // the water it falls into
    let mut bowl = [
        Mode::new(404.0 * detune, 0.085, 1.00),
        Mode::new(611.0 * detune, 0.055, 0.42),
    ];

    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(total);
    for i in 0..total {
        let t = i as f32 / RATE as f32;
        let mut v = 0.0f32;
        let mut splash = 0.0f32;
        for (start, f0, tau_f, amp) in drops {
            if t < start {
                continue;
            }
            let u = t - start;
            let phase = std::f32::consts::TAU * f0 * tau_f * ((u / tau_f).exp() - 1.0);
            let env = (-u * 27.0).exp() * (1.0 - (-u * 900.0).exp());
            v += phase.sin() * env * amp;
            // the moment of contact, feeding the bowl
            splash += (-u * 1_400.0).exp() * amp;
        }
        let raw = rng.next();
        lp += (raw - lp) * 0.5;
        let exc = lp * splash;
        for m in &mut bowl {
            v += m.tick(exc) * 0.5;
        }
        out.push(v);
    }
    normalize(out, 0.20)
}

fn file_name(kind: &str, k: usize) -> String {
    format!("{kind}-{DESIGN}-{k}.wav")
}

/// Render every strike once, and sweep away renders from an older design.
/// Cheap enough to do at startup and idempotent.
pub fn ensure() {
    let Some(d) = dir() else { return };
    let _ = std::fs::create_dir_all(&d);
    let mut wanted = Vec::new();
    for k in 0..VARIANTS {
        for (kind, samples) in
            [("press", press_samples(k)), ("share", share_samples(k))]
        {
            let name = file_name(kind, k);
            let p = d.join(&name);
            if !p.exists() {
                let _ = std::fs::write(&p, wav(&samples));
            }
            wanted.push(name);
        }
    }
    // Anything left is a previous design; a stale sound would outlive every
    // attempt to improve it.
    if let Ok(entries) = std::fs::read_dir(&d) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.ends_with(".wav") && !wanted.contains(&n) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Rotation, not randomness: rotation guarantees the next one differs, while
/// random would sometimes play the same strike twice in a row — which is the
/// thing being avoided.
static TURN: AtomicUsize = AtomicUsize::new(0);

/// Play one, without blocking the frame. A failure to make a noise is never
/// worth interrupting mining over.
fn play(kind: &str) {
    let Some(d) = dir() else { return };
    let k = TURN.fetch_add(1, Ordering::Relaxed) % VARIANTS;
    let p = d.join(file_name(kind, k));
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
    play("press");
}

pub fn share() {
    play("share");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A resonator with a feedback coefficient at or above one does not ring,
    /// it explodes — and the first anyone would know is a burst of noise at
    /// full scale. Every mode must decay.
    #[test]
    fn modes_decay() {
        for &(f, tau) in &[(322.0f32, 0.058f32), (2857.0, 0.011), (404.0, 0.085)] {
            let mut m = Mode::new(f, tau, 1.0);
            // A mode is spent after about five time constants: e^-5 is well
            // under a percent. Measuring against a fixed clock instead would
            // just be asking whether tau is small, which is not the question.
            let spent = (5.0 * tau * RATE as f32) as usize;
            let mut peak_early = 0.0f32;
            let mut peak_late = 0.0f32;
            for i in 0..spent * 2 {
                let x = if i == 0 { 1.0 } else { 0.0 };
                let y = m.tick(x).abs();
                if i < spent / 10 {
                    peak_early = peak_early.max(y);
                } else if i > spent {
                    peak_late = peak_late.max(y);
                }
            }
            assert!(peak_early > 0.0, "mode at {f} Hz never rang");
            assert!(
                peak_late < peak_early * 0.01,
                "mode at {f} Hz still at {:.3}% after five time constants",
                peak_late / peak_early * 100.0
            );
        }
    }

    /// Rendered sound must use its headroom and never clip: a sound that
    /// clips is the one that gets called ugly.
    #[test]
    fn renders_are_audible_and_unclipped() {
        for k in 0..VARIANTS {
            for (name, s) in [("press", press_samples(k)), ("share", share_samples(k))] {
                let peak = s.iter().map(|v| v.abs() as i32).max().unwrap_or(0);
                assert!(peak > 3_000, "{name} #{k} is inaudible (peak {peak})");
                assert!(peak < 32_000, "{name} #{k} clips (peak {peak})");
            }
        }
    }

    /// Two strikes of the same sound must differ, or the rotation is theatre.
    #[test]
    fn strikes_differ() {
        let a = press_samples(0);
        let b = press_samples(1);
        assert_ne!(a, b, "every strike rendered identically");
    }
}
