//! The pad's rumble for Mario's world: his blows are the mod's own, not the game's attacks, so
//! the game never shook the pad for them (nor for his hurts, his ground pounds, the shakes of the
//! camera). A pulse on each, its own weight: the heavy (low) motor for weight, the light (high)
//! one for crispness, longer for bigger moments.
//!
//! Through the game's own XInputSetState (its import table, lib.rs: Steam Input's hook, if any,
//! stays in the chain, the way a PlayStation pad through Steam gets its rumble), and mixed with
//! the game's: the game sets the pad's motors itself (its own hits, often zero), and a straight
//! call of ours was undone by its next one. Now each motor is the stronger of the game's and ours.
//!
//! Strength: ini `rumble` (0 off .. 1, default 1) times the game's Vibration option (0-10; at 0,
//! the ini's alone: the game's own may be off while Mario's is wanted). Written by a thread of
//! its own, every 15 ms, only when it changes (never a wireless pad's call on the game's frame).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::UI::Input::XboxController::XINPUT_VIBRATION;

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

/// the game's XInputSetState (what was in its import table), 0 none: xinput1_4's own
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// the motors as the game last set them, and as ours are now (low << 16 | high)
static GAME: AtomicU32 = AtomicU32::new(0);
static OURS: AtomicU32 = AtomicU32::new(0);
type SetStateFn = unsafe extern "system" fn(u32, *mut XINPUT_VIBRATION) -> u32;

fn pack(low: u16, high: u16) -> u32 {
    (low as u32) << 16 | high as u32
}
fn unpack(v: u32) -> (u16, u16) {
    ((v >> 16) as u16, v as u16)
}

/// lib.rs: the game's XInputSetState, swapped in its import table for `game_set_state`.
pub fn set_original(f: usize) {
    ORIGINAL.store(f, Ordering::Relaxed);
}

/// Sends the motors (each the stronger of the game's and ours) to pad `index`: the result code
/// (0 done, 1167 no pad there).
fn send(index: u32, ours: (u16, u16)) -> u32 {
    let game = unpack(GAME.load(Ordering::Relaxed));
    let mut vib = XINPUT_VIBRATION { wLeftMotorSpeed: game.0.max(ours.0), wRightMotorSpeed: game.1.max(ours.1) };
    match ORIGINAL.load(Ordering::Relaxed) {
        0 => unsafe { windows::Win32::UI::Input::XboxController::XInputSetState(index, &mut vib) },
        f => unsafe { std::mem::transmute::<usize, SetStateFn>(f)(index, &mut vib) },
    }
}

/// The game sets a pad's motors (its import table points here): kept, and sent mixed with ours.
pub unsafe extern "system" fn game_set_state(index: u32, vib: *mut XINPUT_VIBRATION) -> u32 {
    if index != 0 || vib.is_null() {
        return match ORIGINAL.load(Ordering::Relaxed) {
            0 => 1167,
            f => unsafe { std::mem::transmute::<usize, SetStateFn>(f)(index, vib) },
        };
    }
    let v = unsafe { *vib };
    let was = GAME.swap(pack(v.wLeftMotorSpeed, v.wRightMotorSpeed), Ordering::Relaxed);
    static SEEN: AtomicBool = AtomicBool::new(false);
    if !SEEN.swap(true, Ordering::Relaxed) || (was == 0 && (v.wLeftMotorSpeed > 0 || v.wRightMotorSpeed > 0) && !SEEN_ON.swap(true, Ordering::Relaxed)) {
        crate::log(format!("rumble: the game sets the pad's motors itself ({}, {})", v.wLeftMotorSpeed, v.wRightMotorSpeed));
    }
    send(index, unpack(OURS.load(Ordering::Relaxed)))
}
static SEEN_ON: AtomicBool = AtomicBool::new(false);

/// The rumble's strength 0..1: the ini's times the game's option (the ini's alone with it at 0).
fn strength() -> (f32, u8) {
    use fromsoftware_shared::FromStatic;
    let game = unsafe { eldenring::cs::GameDataMan::instance() }.map_or(10, |g| g.game_settings.controller_rumble_strength).min(10);
    let ini = match crate::paths::config("rumble").as_deref().map(str::trim) {
        Some("off") | Some("0") => 0.0,
        Some(v) => v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0),
        None => 1.0,
    };
    (ini * if game == 0 { 1.0 } else { game as f32 / 10.0 }, game)
}

/// The motors' mix of the pulses on now (each motor its strongest).
fn mix(pulses: &[Pulse], now: Instant) -> (f32, f32) {
    pulses.iter().filter(|p| p.until > now).fold((0.0f32, 0.0f32), |(l, h), p| (l.max(p.low), h.max(p.high)))
}

/// A pulse: `low` / `high` motors 0..1 for `secs`.
pub fn pulse(low: f32, high: f32, secs: f32) {
    let (k, game) = strength();
    static TOLD: AtomicBool = AtomicBool::new(false);
    if !TOLD.swap(true, Ordering::Relaxed) {
        crate::log(format!(
            "rumble: first pulse: strength {k:.2} (the game's Vibration {game}/10{}, er_mario.ini rumble), XInput pad {}, through {}",
            if game == 0 { ": off there, the ini's alone" } else { "" },
            if crate::pads::xinput_pad() { "seen" } else { "NOT seen" },
            if ORIGINAL.load(Ordering::Relaxed) != 0 { "the game's XInputSetState" } else { "xinput1_4's XInputSetState" }
        ));
    }
    if k <= 0.0 {
        return;
    }
    start();
    let p = Pulse { low: (low * k).clamp(0.0, 1.0), high: (high * k).clamp(0.0, 1.0), until: Instant::now() + Duration::from_secs_f32(secs.clamp(0.02, 1.5)) };
    let mut v = PULSES.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    v.retain(|q| q.until > now);
    if v.len() >= MOST {
        v.remove(0);
    }
    v.push(p);
    WAKE.notify_one();
}

/// A blow Mario lands: its hit-stop (SM64 ticks) is its weight; a finisher thuds longer.
pub fn blow(stop: u32, heavy: bool, finisher: bool) {
    let w = (stop as f32 / 8.0).clamp(0.1, 1.0);
    if finisher {
        pulse(0.6 + 0.4 * w, 0.45, 0.18 + 0.12 * w);
    } else if heavy {
        pulse(0.35 + 0.5 * w, 0.25 + 0.3 * w, 0.08 + 0.1 * w);
    } else {
        // (a light jab: a short buzz of the light motor, a touch of the heavy one)
        pulse(0.12 + 0.2 * w, 0.3 + 0.3 * w, 0.05 + 0.04 * w);
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
        pulse(1.0, 0.6, 0.32);
    } else {
        pulse(0.65, 0.35, 0.18);
    }
}

/// Mario lands hard (a ground pound, a long fall): `k` 0..1.
pub fn land(k: f32) {
    let k = k.clamp(0.0, 1.0);
    pulse(0.3 + 0.55 * k, 0.12, 0.07 + 0.1 * k);
}

/// A crisp tick: a perfect dodge, a parry's ring (`strong`), a blow turned on the guard.
pub fn tick(strong: bool) {
    if strong {
        pulse(0.45, 0.9, 0.12);
    } else {
        pulse(0.0, 0.6, 0.06);
    }
}

/// A big moment: Mega Mario growing, a guard broken, a boss's star (`k` 0..1, how big).
pub fn rumble(k: f32) {
    let k = k.clamp(0.0, 1.0);
    pulse(0.5 + 0.5 * k, 0.3 + 0.3 * k, 0.25 + 0.6 * k);
}

/// A coin: the faintest tick.
pub fn coin() {
    pulse(0.0, 0.2, 0.04);
}

/// Mario's death: a long, heavy fade.
pub fn death() {
    pulse(1.0, 0.4, 0.35);
    pulse(0.5, 0.0, 0.9);
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
        let mut last = (0u16, 0u16);
        let mut told_fail = false;
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
                OURS.store(pack(now.0, now.1), Ordering::Relaxed);
                let rc = send(0, now);
                if rc != 0 && !told_fail {
                    told_fail = true;
                    crate::log(format!("rumble: the pad's XInputSetState said {rc} (1167: no pad in slot 0)"));
                }
                last = now;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{mix, pack, unpack, Pulse};
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

    #[test]
    fn the_motors_pack_both_ways() {
        assert_eq!(unpack(pack(65535, 12)), (65535, 12));
        assert_eq!(unpack(pack(0, 0)), (0, 0));
    }
}
