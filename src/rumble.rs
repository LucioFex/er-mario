//! The pad's rumble for Mario's world: his blows are the mod's own, not the game's attacks, so
//! the game never shook the pad for them (nor for his hurts, his ground pounds, the shakes of the
//! camera). A pulse on each (heavy low motor, light high motor), as strong as the game's own
//! "Vibration" option says (0 off), times ini `rumble` (0 off .. 1, default 1).
//!
//! XInput only (the pad the game reads: pads.rs); written by a thread of its own, every 15 ms, only
//! when it changes (a wireless pad's call can take a moment: never on the game's frame).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// A pulse: the low (heavy) and high (light) motors 0..1, until when.
#[derive(Clone, Copy)]
struct Pulse {
    low: f32,
    high: f32,
    until: Instant,
}

static PULSES: Mutex<Vec<Pulse>> = Mutex::new(Vec::new());
static WAKE: Condvar = Condvar::new();
static STARTED: AtomicBool = AtomicBool::new(false);
/// at most this many pulses kept (the strongest win anyway)
const MOST: usize = 8;

/// The rumble's strength 0..1: the game's option (0-10) times the ini's.
fn strength() -> f32 {
    use fromsoftware_shared::FromStatic;
    let game = unsafe { eldenring::cs::GameDataMan::instance() }.map_or(5, |g| g.game_settings.controller_rumble_strength).min(10) as f32 / 10.0;
    let ini = match crate::paths::config("rumble").as_deref().map(str::trim) {
        Some("off") | Some("0") => 0.0,
        Some(v) => v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0),
        None => 1.0,
    };
    game * ini
}

/// The motors' mix of the pulses on now (each motor its strongest).
fn mix(pulses: &[Pulse], now: Instant) -> (f32, f32) {
    pulses.iter().filter(|p| p.until > now).fold((0.0f32, 0.0f32), |(l, h), p| (l.max(p.low), h.max(p.high)))
}

/// A pulse: `low` / `high` motors 0..1 for `secs`.
pub fn pulse(low: f32, high: f32, secs: f32) {
    let k = strength();
    if k <= 0.0 || !crate::pads::xinput_pad() {
        return;
    }
    start();
    let p = Pulse { low: (low * k).clamp(0.0, 1.0), high: (high * k).clamp(0.0, 1.0), until: Instant::now() + Duration::from_secs_f32(secs.clamp(0.02, 1.0)) };
    let mut v = PULSES.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    v.retain(|q| q.until > now);
    if v.len() >= MOST {
        v.remove(0);
    }
    v.push(p);
    WAKE.notify_one();
}

/// A blow Mario lands: its hit-stop (SM64 ticks) is its weight.
pub fn blow(stop: u32, heavy: bool) {
    let w = (stop as f32 / 8.0).clamp(0.1, 1.0);
    if heavy {
        pulse(0.35 + 0.5 * w, 0.25 + 0.3 * w, 0.08 + 0.1 * w);
    } else {
        pulse(0.0, 0.18 + 0.25 * w, 0.05 + 0.03 * w);
    }
}

/// Lakitu's camera shakes (impact.rs, bolts, beams, rides...): the pad with it.
pub fn shake(strength: f32) {
    let s = strength.clamp(0.0, 2.0) / 2.0;
    if s > 0.05 {
        pulse(0.25 + 0.6 * s, 0.15 + 0.3 * s, 0.1 + 0.25 * s);
    }
}

/// Mario is hurt (`wedges`: 1 half a wedge, 2 a whole one).
pub fn hurt(wedges: u32) {
    if wedges >= 2 {
        pulse(0.85, 0.5, 0.25);
    } else {
        pulse(0.55, 0.3, 0.15);
    }
}

/// Mario lands hard (a ground pound, a long fall): `k` 0..1.
pub fn land(k: f32) {
    let k = k.clamp(0.0, 1.0);
    pulse(0.3 + 0.5 * k, 0.1, 0.06 + 0.08 * k);
}

/// Everything off now (Mario off, a load, the game closing).
pub fn stop() {
    PULSES.lock().unwrap_or_else(|e| e.into_inner()).clear();
    WAKE.notify_one();
}

fn start() {
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let _ = std::thread::Builder::new().name("er_mario rumble".into()).spawn(|| {
        use windows::Win32::UI::Input::XboxController::{XInputSetState, XINPUT_VIBRATION};
        let mut last = (0u16, 0u16);
        loop {
            let (low, high) = {
                let v = PULSES.lock().unwrap_or_else(|e| e.into_inner());
                let v = if v.iter().all(|p| p.until <= Instant::now()) && last == (0, 0) {
                    // (nothing on and nothing to turn off: sleep until a pulse)
                    WAKE.wait(v).unwrap_or_else(|e| e.into_inner())
                } else {
                    v
                };
                mix(&v, Instant::now())
            };
            let now = ((low * 65535.0) as u16, (high * 65535.0) as u16);
            if now != last {
                let mut vib = XINPUT_VIBRATION { wLeftMotorSpeed: now.0, wRightMotorSpeed: now.1 };
                let _ = unsafe { XInputSetState(0, &mut vib) };
                last = now;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{mix, Pulse};
    use std::time::{Duration, Instant};

    #[test]
    fn each_motor_takes_its_strongest_pulse_on_now() {
        let now = Instant::now();
        let later = now + Duration::from_millis(100);
        let gone = now - Duration::from_millis(1);
        let v = [Pulse { low: 0.3, high: 0.9, until: later }, Pulse { low: 0.8, high: 0.1, until: later }, Pulse { low: 1.0, high: 1.0, until: gone }];
        assert_eq!(mix(&v, now), (0.8, 0.9));
        assert_eq!(mix(&[], now), (0.0, 0.0));
    }
}
