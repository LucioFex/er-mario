//! The kart's engine (v4 Part 4), synthesised by the mod (no samples): a buzzy pulse and its octave
//! at the engine's pitch, put-putting at half of it, softened by a low-pass that opens with the
//! throttle; the tyres' skid a hiss of filtered noise. Its settings (kartplay::Tone) are eased
//! sample by sample: a jump in them never clicks. audio.rs mixes it in where Mario is.

use crate::kartplay::Tone;

/// the settings ease to new ones over about this long (s); the low-pass's corner closed and open
/// (Hz); the octave's and the put's share; the skid's loudness at full
const EASE: f32 = 0.03;
const CORNER: (f32, f32) = (700.0, 2400.0);
const OCTAVE: f32 = 0.35;
const PUTT: f32 = 0.35;
const SKID: f32 = 0.1;

/// the pulse's duty (its high share of a beat), the hiss's band (Hz), the DC blocker's corner (Hz)
const DUTY: f32 = 0.3;
const HISS: (f32, f32) = (600.0, 4000.0);
const DC: f32 = 20.0;

pub struct Engine {
    /// the beat's and the put's phases (0-1)
    phase: f32,
    putt: f32,
    /// the settings as eased so far
    freq: f32,
    volume: f32,
    load: f32,
    skid: f32,
    /// the low-pass and the DC blocker
    lp: f32,
    dc: f32,
    noise: u32,
    hiss: [f32; 2],
}

/// A saw's corner smoothed over one sample (polyBLEP): its step doesn't alias into a whine.
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

fn saw(t: f32, dt: f32) -> f32 {
    2.0 * t - 1.0 - blep(t, dt)
}

/// a one-pole low-pass's step at `corner` Hz
fn pole(corner: f32, rate: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * corner / rate).exp()
}

impl Engine {
    pub fn new() -> Self {
        Engine { phase: 0.0, putt: 0.0, freq: 0.0, volume: 0.0, load: 0.0, skid: 0.0, lp: 0.0, dc: 0.0, noise: 0x2545_F491, hiss: [0.0; 2] }
    }

    /// One sample at `rate` Hz of `tone` (None: it fades out).
    pub fn sample(&mut self, tone: Option<&Tone>, rate: f32) -> f32 {
        let rate = rate.max(1000.0);
        let (freq, volume, load, skid) = match tone {
            Some(t) => (t.freq.clamp(10.0, 600.0), t.volume.clamp(0.0, 1.0), t.load.clamp(0.0, 1.0), t.skid.clamp(0.0, 1.0)),
            None => (self.freq, 0.0, 0.0, 0.0),
        };
        // (silent: it starts at the pitch it's given, no sweep up from nothing)
        if self.volume < 1e-6 && self.skid < 1e-6 {
            if tone.is_none_or(|t| t.volume <= 0.0 && t.skid <= 0.0) {
                self.freq = freq;
                self.lp = 0.0;
                self.dc = 0.0;
                self.hiss = [0.0; 2];
                return 0.0;
            }
            if self.freq <= 0.0 {
                self.freq = freq;
            }
        }
        let k = 1.0 - (-1.0 / (EASE * rate)).exp();
        self.freq += (freq - self.freq) * k;
        self.volume += (volume - self.volume) * k;
        self.load += (load - self.load) * k;
        self.skid += (skid - self.skid) * k;
        // the beat: a pulse (two saws a duty apart) and its octave, put-putting every other beat
        let dt = self.freq / rate;
        self.phase = (self.phase + dt).fract();
        self.putt = (self.putt + dt * 0.5).fract();
        let pulse = saw(self.phase, dt) - saw((self.phase + DUTY).fract(), dt);
        let octave = (std::f32::consts::TAU * 2.0 * self.phase).sin();
        let putt = 1.0 - PUTT * (0.5 + 0.5 * (std::f32::consts::TAU * self.putt).sin());
        let raw = (pulse * 0.5 + octave * OCTAVE) * putt;
        // softened: the low-pass opens as it works harder; its DC taken out
        self.lp += (raw - self.lp) * pole(CORNER.0 + (CORNER.1 - CORNER.0) * self.load, rate);
        self.dc += (self.lp - self.dc) * pole(DC, rate);
        let engine = (self.lp - self.dc) * self.volume * 1.4;
        // the skid: noise between the hiss's band
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let n = self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0;
        self.hiss[0] += (n - self.hiss[0]) * pole(HISS.1, rate);
        self.hiss[1] += (self.hiss[0] - self.hiss[1]) * pole(HISS.0, rate);
        let hiss = (self.hiss[0] - self.hiss[1]) * SKID * 2.0 * self.skid;
        (engine + hiss).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn run(e: &mut Engine, t: Option<&Tone>, secs: f32) -> Vec<f32> {
        (0..(secs * RATE) as usize).map(|_| e.sample(t, RATE)).collect()
    }

    fn tone(freq: f32, volume: f32, skid: f32) -> Tone {
        Tone { freq, load: 0.5, volume, skid }
    }

    /// The waveform's repeat (s): the lag (20-400 Hz) it matches itself best at.
    fn period(x: &[f32]) -> f32 {
        let n = x.len() / 2;
        let lags = (RATE / 400.0) as usize..(RATE / 20.0) as usize;
        let score = |lag: usize| (0..n).map(|i| x[i] * x[i + lag]).sum::<f32>();
        let scores: Vec<(usize, f32)> = lags.map(|l| (l, score(l))).collect();
        let best = scores.iter().map(|s| s.1).fold(f32::MIN, f32::max);
        // (the first peak about as good as the best: the later ones are its multiples)
        scores.windows(3).find(|w| w[1].1 >= best * 0.9 && w[1].1 >= w[0].1 && w[1].1 >= w[2].1).map_or(0.0, |w| w[1].0 as f32 / RATE)
    }

    #[test]
    fn silent_without_a_tone_and_after_one_fades() {
        let mut e = Engine::new();
        assert!(run(&mut e, None, 0.2).iter().all(|x| x.abs() < 1e-6));
        run(&mut e, Some(&tone(80.0, 0.2, 0.0)), 0.3);
        let tail = run(&mut e, None, 0.5);
        assert!(tail[tail.len() - 2000..].iter().all(|x| x.abs() < 1e-3));
    }

    #[test]
    fn it_never_goes_past_full_scale() {
        let mut e = Engine::new();
        let x = run(&mut e, Some(&Tone { freq: 200.0, load: 1.0, volume: 1.0, skid: 1.0 }), 1.0);
        assert!(x.iter().all(|v| v.abs() <= 1.0 && v.is_finite()));
        assert!(x.iter().any(|v| v.abs() > 0.1), "it plays");
    }

    #[test]
    fn its_pitch_follows_the_engine() {
        let mut low = Engine::new();
        let mut high = Engine::new();
        run(&mut low, Some(&tone(60.0, 0.2, 0.0)), 0.2);
        run(&mut high, Some(&tone(150.0, 0.2, 0.0)), 0.2);
        let (a, b) = (period(&run(&mut low, Some(&tone(60.0, 0.2, 0.0)), 0.3)), period(&run(&mut high, Some(&tone(150.0, 0.2, 0.0)), 0.3)));
        // (its repeat is the pitch's, or twice it: the put-put comes every other beat)
        let near = |p: f32, f: f32| [1.0 / f, 2.0 / f].iter().any(|q| (p - q).abs() < q * 0.08);
        assert!(near(a, 60.0) && near(b, 150.0), "{a} {b}");
        let k = a / b;
        assert!(k > 2.2 && k < 2.8, "{a} {b}");
    }

    #[test]
    fn a_jump_in_its_settings_does_not_click() {
        let mut e = Engine::new();
        let steady = run(&mut e, Some(&tone(150.0, 0.24, 0.0)), 0.5);
        let most = |x: &[f32]| x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        let normal = most(&steady[steady.len() / 2..]);
        let mut e = Engine::new();
        run(&mut e, Some(&tone(50.0, 0.0, 0.0)), 0.5);
        let jumped = run(&mut e, Some(&tone(150.0, 0.24, 0.0)), 0.3);
        assert!(most(&jumped) <= normal * 1.5 + 1e-3, "{} vs {normal}", most(&jumped));
    }

    #[test]
    fn the_skid_alone_is_a_hiss() {
        let mut e = Engine::new();
        let x = run(&mut e, Some(&tone(80.0, 0.0, 1.0)), 0.5);
        let rms = (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        assert!(rms > 0.005 && rms < 0.2, "{rms}");
    }
}
