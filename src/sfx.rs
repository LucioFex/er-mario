//! The mod's own sounds (v4 Part 10), synthesized here: the Star Sword's swing, its hit and its
//! heavy hit (a bright cut over a deep thud), a thunderclap, the hook's gas, the dark beam's hit.
//! The sword's can be recordings instead (the user, 2026-10-24: the synthesized ones sounded like a
//! little bell, and quiet): WAV files in `sounds/` in the mod's folder (`sword_swing_*.wav`,
//! `sword_swing_heavy_*.wav`, `sword_hit_*.wav`, `sword_hit_heavy_*.wav`; one picked at random
//! each time), never in the repository; a hit's recording goes over the synthesized thud. Each
//! sound is made once as it's asked for (mono at RATE) and played by audio.rs's stream, mixed with
//! SM64's where Mario is; never twice the same (a seed shifts its pitch and grain a little).

use std::f32::consts::TAU;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU32, Ordering};

pub const RATE: f32 = 32_000.0;
/// at most this many playing at once (the oldest let go)
const VOICES: usize = 12;
/// the sword's sounds this much louder than asked (they were far under SM64's own), at most full
const SWORD_GAIN: f32 = 1.8;
const THUNDER_GAIN: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    /// a blade's swing through the air, by weight (0 light .. 2 heavy)
    Swing(u8),
    /// the blade striking: a bright cut over a thud, by weight
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
        Sfx::Hit(w) => 0.3 + 0.12 * w.min(2) as f32,
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
                // the cut (a burst of bright noise, its band falling) over a deep thud and a click;
                // no ringing tones (they made it a little bell)
                let w = w.min(2) as f32;
                let (_, cut) = a.step(noise.next(), (5200.0 - 2600.0 * (t / 0.08).min(1.0)) * detune, 1.2);
                let cut = cut * (-t / (0.035 + 0.015 * w)).exp() * (1.9 - 0.4 * w);
                let thud_f = 55.0 - 8.0 * w + 85.0 * (-t / 0.045).exp();
                let thud = (TAU * thud_f * t).sin() * (-t / (0.06 * (1.0 + 0.6 * w))).exp() * (0.4 + 0.7 * w);
                let (low, _) = b.step(noise.next(), 700.0, 0.9);
                let body = low * (-t / (0.03 + 0.02 * w)).exp() * (0.6 + 0.5 * w);
                let click = noise.next() * (-t / 0.003).exp() * 0.6;
                cut + thud + body + click
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

/// Brought up to near full scale (0.95 at its peak).
fn loud(mut out: Vec<f32>) -> Vec<f32> {
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 1e-4 {
        let k = 0.95 / peak;
        out.iter_mut().for_each(|s| *s *= k);
    }
    out
}

/// Reads a WAV file (PCM 8/16/24/32-bit or 32-bit float, any channels and rate): mono at RATE,
/// brought up to near full scale. None if it's no WAV or has no sound.
pub fn read_wav(bytes: &[u8]) -> Option<Vec<f32>> {
    let u16_at = |i: usize| bytes.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if bytes.get(0..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let (id, len) = (&bytes[at..at + 4], u32_at(at + 4)? as usize);
        let body = at + 8;
        match id {
            b"fmt " => fmt = Some((u16_at(body)?, u16_at(body + 2)?, u32_at(body + 4)?, u16_at(body + 14)?)),
            b"data" => data = Some(&bytes[body..(body + len).min(bytes.len())]),
            _ => {}
        }
        at = body + len + (len & 1);
    }
    let ((format, channels, rate, bits), data) = (fmt?, data?);
    let (channels, width) = (channels.max(1) as usize, (bits as usize).div_ceil(8));
    // (0xFFFE: the extensible header; its samples as its bits say)
    // (0xFFFE, the extensible header: taken as integer PCM, its usual kind)
    let float = format == 3;
    if width == 0 || rate == 0 || !(matches!(format, 1 | 3 | 0xFFFE)) {
        return None;
    }
    let frame = channels * width;
    let sample = |b: &[u8]| -> f32 {
        match (float, width) {
            (true, 4) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            (_, 1) => (b[0] as f32 - 128.0) / 128.0,
            (_, 2) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
            (_, 3) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (_, 4) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            _ => 0.0,
        }
    };
    let mono: Vec<f32> = data.chunks_exact(frame).map(|f| f.chunks_exact(width).map(sample).sum::<f32>() / channels as f32).collect();
    if mono.len() < 2 {
        return None;
    }
    // (to our rate, straight lines between its samples)
    let step = rate as f32 / RATE;
    let n = ((mono.len() - 1) as f32 / step) as usize;
    let out: Vec<f32> = (0..n)
        .map(|i| {
            let p = i as f32 * step;
            let k = p as usize;
            let f = p - k as f32;
            mono[k] + (mono[(k + 1).min(mono.len() - 1)] - mono[k]) * f
        })
        .filter(|x| x.is_finite())
        .collect();
    (out.iter().any(|x| x.abs() > 1e-4)).then(|| loud(out))
}

/// The sword's recordings in sounds/ (empty: synthesized), read once.
struct Bank {
    swing: Vec<Vec<f32>>,
    swing_heavy: Vec<Vec<f32>>,
    hit: Vec<Vec<f32>>,
    hit_heavy: Vec<Vec<f32>>,
}

fn bank() -> &'static Bank {
    static BANK: OnceLock<Bank> = OnceLock::new();
    BANK.get_or_init(|| {
        let mut b = Bank { swing: Vec::new(), swing_heavy: Vec::new(), hit: Vec::new(), hit_heavy: Vec::new() };
        let dir = crate::paths::mod_dir().join("sounds");
        let mut names: Vec<_> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        names.sort();
        for path in names {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
            if !name.ends_with(".wav") {
                continue;
            }
            let into = if name.starts_with("sword_swing_heavy") {
                &mut b.swing_heavy
            } else if name.starts_with("sword_swing") {
                &mut b.swing
            } else if name.starts_with("sword_hit_heavy") {
                &mut b.hit_heavy
            } else if name.starts_with("sword_hit") {
                &mut b.hit
            } else {
                continue;
            };
            match std::fs::read(&path).ok().and_then(|bytes| read_wav(&bytes)) {
                Some(s) => into.push(s),
                None => crate::log(format!("sfx: {name} is no WAV we can read")),
            }
        }
        let count = b.swing.len() + b.swing_heavy.len() + b.hit.len() + b.hit_heavy.len();
        crate::log(format!("sfx: {count} sword recordings from {} ({} swings, {} heavy, {} hits, {} heavy)", dir.display(), b.swing.len(), b.swing_heavy.len(), b.hit.len(), b.hit_heavy.len()));
        b
    })
}

/// One of `set` by `seed`, falling back on `or` (the light ones for the heavy, and back).
fn pick<'a>(set: &'a [Vec<f32>], or: &'a [Vec<f32>], seed: u32) -> Option<&'a Vec<f32>> {
    let from = if set.is_empty() { or } else { set };
    (!from.is_empty()).then(|| &from[(seed >> 8) as usize % from.len()])
}

/// `top` laid over `under` from the start, brought up to near full scale.
fn layered(under: Vec<f32>, top: &[f32], top_share: f32) -> Vec<f32> {
    let mut out = under;
    if out.len() < top.len() {
        out.resize(top.len(), 0.0);
    }
    for (o, t) in out.iter_mut().zip(top) {
        *o = *o * (1.0 - top_share * 0.5) + t * top_share;
    }
    loud(out)
}

/// What `kind` sounds like this time (`seed`): the sword's recordings if there are some.
fn sound_of(kind: Sfx, seed: u32) -> Vec<f32> {
    let b = bank();
    match kind {
        Sfx::Swing(w) => {
            let (set, or) = if w >= 2 { (&b.swing_heavy, &b.swing) } else { (&b.swing, &b.swing_heavy) };
            pick(set, or, seed).cloned().unwrap_or_else(|| render(kind, seed))
        }
        Sfx::Hit(w) => {
            let (set, or) = if w >= 2 { (&b.hit_heavy, &b.hit) } else { (&b.hit, &b.hit_heavy) };
            match pick(set, or, seed) {
                // (the recording's cut over our thud: the blow felt as well as heard)
                Some(rec) => layered(render(kind, seed), rec, 0.75),
                None => render(kind, seed),
            }
        }
        _ => render(kind, seed),
    }
}

/// Sounds asked for, waiting for the audio thread to take them (samples, volume).
static QUEUE: Mutex<Vec<(Vec<f32>, f32)>> = Mutex::new(Vec::new());
static SEED: AtomicU32 = AtomicU32::new(0x9E37);

/// Plays `kind` at `volume` (0-1) where Mario is.
pub fn play(kind: Sfx, volume: f32) {
    let seed = SEED.fetch_add(0x6F4F_2A31, Ordering::Relaxed);
    let samples = sound_of(kind, seed);
    let volume = match kind {
        Sfx::Swing(_) | Sfx::Hit(_) => volume * SWORD_GAIN,
        // (the thunder too: far under SM64's own, the user said)
        Sfx::Thunder(_) => volume * THUNDER_GAIN,
        _ => volume,
    };
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

    /// A little WAV file of `samples` (16-bit or float), `channels`, at `rate`.
    fn wav(samples: &[f32], channels: u16, rate: u32, float: bool) -> Vec<u8> {
        let bytes = if float { 4 } else { 2 };
        let data = (samples.len() * bytes) as u32;
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&(if float { 3u16 } else { 1u16 }).to_le_bytes());
        w.extend_from_slice(&channels.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * channels as u32 * bytes as u32).to_le_bytes());
        w.extend_from_slice(&(channels * bytes as u16).to_le_bytes());
        w.extend_from_slice(&(bytes as u16 * 8).to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data.to_le_bytes());
        for &x in samples {
            if float {
                w.extend_from_slice(&x.to_le_bytes());
            } else {
                w.extend_from_slice(&((x * 32767.0) as i16).to_le_bytes());
            }
        }
        w
    }

    /// A recording (sounds/ in the mod's folder) is read whatever its rate and channels: mono at
    /// RATE, brought up to near full scale (the user: the sword was far quieter than the rest).
    #[test]
    fn a_wav_is_read_at_our_rate_loud() {
        // (0.1 s of a 440 Hz tone, quiet, stereo at 44.1 kHz)
        let n = 4410;
        let tone: Vec<f32> = (0..n).flat_map(|i| {
            let x = 0.2 * (TAU * 440.0 * i as f32 / 44_100.0).sin();
            [x, x]
        }).collect();
        for float in [false, true] {
            let got = read_wav(&wav(&tone, 2, 44_100, float)).expect("read");
            assert!((got.len() as f32 - 0.1 * RATE).abs() < 40.0, "{}", got.len());
            let peak = got.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.85 && peak <= 0.951, "{peak}");
        }
        assert!(read_wav(b"not a wav").is_none());
        assert!(read_wav(&wav(&[], 1, 32_000, false)).is_none());
    }

    /// The sword's hit is a cut and a thud, not a bell (the user: "una campanita"): no long ring of
    /// steady tones; its energy dies within a quarter second.
    #[test]
    fn the_sword_hit_is_no_bell() {
        for w in 0..3u8 {
            let s = render(Sfx::Hit(w), 5);
            let q = (0.25 * RATE) as usize;
            let (early, late) = (rms(&s[..q.min(s.len())]), rms(&s[q.min(s.len())..]));
            assert!(late < early * 0.2, "weight {w}: it rings on ({late} after {early})");
        }
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
