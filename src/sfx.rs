//! The mod's own sounds (v4 Part 10), synthesized here, nothing recorded and nothing third-party:
//! the Star Sword's swing, its hit and its heavy hit (a metallic ring over a thud), a thunderclap,
//! the hook's gas, the dark beam's hit. Each is made once as it's asked for (`render`, mono at
//! RATE) and played by audio.rs's stream, mixed with SM64's where Mario is. Soft, and never twice
//! the same: a seed shifts each one's pitch and grain a little.

use std::f32::consts::TAU;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

pub const RATE: f32 = 32_000.0;
/// at most this many playing at once (the oldest let go)
const VOICES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    /// a blade's swing through the air, by weight (0 light .. 2 heavy)
    Swing(u8),
    /// the blade striking: a ring of steel over a thud, by weight
    Hit(u8),
    /// a bolt striking (true: a big one, its rumble longer)
    Thunder(bool),
    /// a burst of the hook's gas
    Gas,
    /// the dark beam burning into someone
    BeamHit,
}

/// A script move's sound (script.rs `sounds`) that's one of these, not SM64's: the tag in its top
/// bits, the kind and weight under it (lib.rs plays it here and gives SM64 none).
const TAG: u32 = 0xE5F0_0000;
pub const SWING_LIGHT: u32 = TAG | 0x10;
pub const SWING: u32 = TAG | 0x11;
pub const SWING_HEAVY: u32 = TAG | 0x12;

/// The sound a script move's `id` stands for, if it's one of these.
pub fn from_script(id: u32) -> Option<Sfx> {
    if id & 0xFFF0_0000 != TAG {
        return None;
    }
    match id & 0xFF {
        w @ 0x10..=0x12 => Some(Sfx::Swing((w - 0x10) as u8)),
        _ => None,
    }
}

/// A little noise of its own (xorshift).
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// A state-variable filter (the trapezoidal kind: steady at any frequency): its low and band
/// passes.
#[derive(Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// One sample through it at `freq` (Hz), damped `q` (0.5 sharp .. 2 broad): (low, band).
    fn step(&mut self, x: f32, freq: f32, q: f32) -> (f32, f32) {
        let g = (std::f32::consts::PI * (freq / RATE).clamp(1e-4, 0.49)).tan();
        let a1 = 1.0 / (1.0 + g * (g + q));
        let (a2, a3) = (g * a1, g * g * a1);
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1)
    }
}

fn smooth(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// The samples of `kind` (mono, RATE), its grain from `seed`.
pub fn render(kind: Sfx, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed.wrapping_mul(2_654_435_761).max(1));
    // (a few per cent up or down: no two alike)
    let detune = 1.0 + (noise.next() * 0.05);
    let secs = match kind {
        Sfx::Swing(w) => 0.16 + 0.06 * w.min(2) as f32,
        Sfx::Hit(w) => 0.42 + 0.14 * w.min(2) as f32,
        Sfx::Thunder(big) => if big { 2.2 } else { 1.3 },
        Sfx::Gas => 0.32,
        Sfx::BeamHit => 0.12,
    };
    let n = (secs * RATE) as usize;
    let mut out = Vec::with_capacity(n);
    let (mut a, mut b) = (Svf::default(), Svf::default());
    let mut brown = 0.0f32;
    let mut rumble = 0.0f32;
    for i in 0..n {
        let t = i as f32 / RATE;
        let k = t / secs;
        let s = match kind {
            Sfx::Swing(w) => {
                // air cut by the blade: noise through a band sweeping up and back down, swelling
                // in and dying away (heavier: lower and longer)
                let w = w.min(2) as f32;
                let peak = 0.38;
                let sweep = if k < peak { k / peak } else { 1.0 - (k - peak) / (1.0 - peak) };
                let freq = (480.0 + (2100.0 - 520.0 * w) * smooth(sweep)) * detune;
                let (_, band) = a.step(noise.next(), freq, 1.1);
                let env = smooth(k / peak) * (1.0 - smooth((k - peak) / (1.0 - peak)));
                let (low, _) = b.step(noise.next(), 220.0, 1.0);
                (band * 1.3 + low * 0.9 * w) * env * 0.55
            }
            Sfx::Hit(w) => {
                // steel ringing (bell-like partials, not harmonic) over a thud and a click
                let w = w.min(2) as f32;
                let f0 = (1500.0 - 260.0 * w) * detune;
                let ring: f32 = [(1.0, 1.0, 0.34), (2.76, 0.55, 0.2), (5.40, 0.3, 0.11), (8.93, 0.16, 0.06)]
                    .iter()
                    .map(|&(m, amp, decay)| amp * (TAU * f0 * m * t).sin() * (-t / (decay * (1.0 + 0.35 * w))).exp())
                    .sum();
                let thud_f = 60.0 + 90.0 * (-t / 0.05).exp();
                let thud = (TAU * thud_f * t).sin() * (-t / (0.07 * (1.0 + 0.6 * w))).exp() * (0.55 + 0.25 * w);
                let (low, _) = a.step(noise.next(), 900.0, 0.9);
                let click = noise.next() * (-t / 0.004).exp() * 0.5 + low * (-t / 0.03).exp() * 0.8;
                ring * 0.22 + thud + click
            }
            Sfx::Thunder(big) => {
                // the crack (bright noise, at once), then the rumble rolling on (deep noise, swelling
                // and falling at random)
                let crack = {
                    let (_, band) = a.step(noise.next(), 3200.0, 1.4);
                    band * (-t / 0.03).exp() * 1.4
                };
                brown = (brown + noise.next() * 0.05) * 0.996;
                let (low, _) = b.step(brown, if big { 120.0 } else { 160.0 }, 0.8);
                rumble += (noise.next().abs() - rumble) * 0.0004;
                let swell = smooth(t / 0.08) * (1.0 - smooth(k)) * (0.6 + 1.6 * rumble);
                crack + low * swell * if big { 9.0 } else { 7.0 }
            }
            Sfx::Gas => {
                // a hiss: bright noise, its band drifting down as the burst spends itself
                let (_, band) = a.step(noise.next(), (6200.0 - 2600.0 * k) * detune, 1.6);
                band * smooth(t / 0.012) * (-t / 0.11).exp() * 0.9
            }
            Sfx::BeamHit => {
                // a sizzle with a low pop under it
                let buzz = if (t * 210.0 * detune).fract() < 0.5 { 1.0 } else { -1.0 };
                let (low, _) = a.step(noise.next() * buzz, 2600.0, 1.0);
                let pop = (TAU * 85.0 * t).sin() * (-t / 0.03).exp();
                (low * 0.9 + pop * 0.6) * (1.0 - smooth(k))
            }
        };
        out.push(s);
    }
    // (never past full scale)
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.95 {
        let k = 0.95 / peak;
        out.iter_mut().for_each(|s| *s *= k);
    }
    out
}

/// Sounds asked for, waiting for the audio thread to take them (samples, volume).
static QUEUE: Mutex<Vec<(Vec<f32>, f32)>> = Mutex::new(Vec::new());
static SEED: AtomicU32 = AtomicU32::new(0x9E37);

/// Plays `kind` at `volume` (0-1) where Mario is.
pub fn play(kind: Sfx, volume: f32) {
    let seed = SEED.fetch_add(0x6F4F_2A31, Ordering::Relaxed);
    let samples = render(kind, seed);
    let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    if q.len() < VOICES {
        q.push((samples, volume.clamp(0.0, 1.0)));
    }
}

/// The audio thread's side: the sounds playing.
pub struct Mixer {
    voices: Vec<(Vec<f32>, f32, f32)>,
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer { voices: Vec::new() }
    }

    /// Before a buffer: what's been asked for since (not waiting if the game thread holds it).
    pub fn take(&mut self) {
        if let Ok(mut q) = QUEUE.try_lock() {
            for (samples, volume) in q.drain(..) {
                if self.voices.len() >= VOICES {
                    self.voices.remove(0);
                }
                self.voices.push((samples, 0.0, volume));
            }
        }
    }

    /// The next output sample at the device's `rate`.
    pub fn sample(&mut self, rate: f32) -> f32 {
        if self.voices.is_empty() {
            return 0.0;
        }
        let step = RATE / rate;
        let mut sum = 0.0;
        for (samples, pos, volume) in &mut self.voices {
            let i = *pos as usize;
            if let (Some(&a), Some(&b)) = (samples.get(i), samples.get(i + 1)) {
                sum += (a + (b - a) * pos.fract()) * *volume;
            }
            *pos += step;
        }
        self.voices.retain(|(samples, pos, _)| (*pos as usize) + 1 < samples.len());
        // (many at once, a storm, a cut on five: limited softly, not clipped)
        sum.tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Sfx; 9] = [Sfx::Swing(0), Sfx::Swing(2), Sfx::Hit(0), Sfx::Hit(1), Sfx::Hit(2), Sfx::Thunder(false), Sfx::Thunder(true), Sfx::Gas, Sfx::BeamHit];

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
    }

    /// Each one sounds, never clips, and dies away by its end.
    #[test]
    fn every_sound_sounds_and_dies_away() {
        for kind in ALL {
            let s = render(kind, 7);
            assert!(s.len() > 1000 && s.iter().all(|x| x.is_finite()), "{kind:?}");
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.05 && peak <= 0.951, "{kind:?}: peak {peak}");
            let tail = &s[s.len() * 95 / 100..];
            assert!(rms(tail) < rms(&s) * 0.35, "{kind:?}: still loud at its end ({} of {})", rms(tail), rms(&s));
        }
    }

    /// A heavy hit is deeper than a light one (more of it under 200 Hz).
    #[test]
    fn a_heavy_hit_is_deeper() {
        let low = |s: &[f32]| {
            let mut f = Svf::default();
            let lows: Vec<f32> = s.iter().map(|&x| f.step(x, 200.0, 1.0).0).collect();
            rms(&lows) / rms(s)
        };
        assert!(low(&render(Sfx::Hit(2), 3)) > low(&render(Sfx::Hit(0), 3)) * 1.2);
        assert!(low(&render(Sfx::Swing(2), 3)) > low(&render(Sfx::Swing(0), 3)));
    }

    /// A move's tag is this module's sound, SM64's own ids aren't.
    #[test]
    fn script_tags_are_ours() {
        assert_eq!(from_script(SWING_HEAVY), Some(Sfx::Swing(2)));
        assert_eq!(from_script(SWING_LIGHT), Some(Sfx::Swing(0)));
        assert_eq!(from_script(0x305C_0081), None);
        assert_eq!(from_script(0x2405_8081), None);
    }

    /// Never twice the same: another seed, another grain.
    #[test]
    fn no_two_hits_alike() {
        assert_ne!(render(Sfx::Hit(1), 1), render(Sfx::Hit(1), 2));
    }

    /// (to listen to them: ER_SFX_OUT=<folder>, test.ps1 -Ignored -Filter write_the_sounds)
    #[test]
    #[ignore]
    fn write_the_sounds() {
        let Ok(dir) = std::env::var("ER_SFX_OUT") else { return };
        let names = ["swing_light", "swing_heavy", "hit_light", "hit", "hit_heavy", "thunder", "thunder_big", "gas", "beam_hit"];
        for (kind, name) in ALL.iter().zip(names) {
            let samples = render(*kind, 11);
            let mut wav = Vec::new();
            let data = (samples.len() * 2) as u32;
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + data).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&(RATE as u32).to_le_bytes());
            wav.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
            wav.extend_from_slice(&2u16.to_le_bytes());
            wav.extend_from_slice(&16u16.to_le_bytes());
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&data.to_le_bytes());
            for x in samples {
                wav.extend_from_slice(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
            }
            std::fs::write(format!("{dir}/{name}.wav"), wav).unwrap();
        }
    }

    /// The mixer plays what's queued at the device's rate, and lets it go once it's over.
    #[test]
    fn the_mixer_plays_and_lets_go() {
        let mut m = Mixer::new();
        m.voices.push((render(Sfx::BeamHit, 1), 0.0, 1.0));
        let n = (0.12 * 48_000.0) as usize + 10;
        let heard: f32 = (0..n).map(|_| m.sample(48_000.0).abs()).sum();
        assert!(heard > 1.0);
        assert!(m.voices.is_empty(), "still playing");
    }
}
