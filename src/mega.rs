//! The Mega Mushroom (D-pad right; it took the Vanish cap's place): Mario grows to SIZE times his
//! height for LAST seconds, then needs RECHARGE seconds before the next one. Big, he's slow for his
//! size and heavy: every blow hits harder (DAMAGE) and shakes the screen, enemies' hits hurt him less
//! (HURT), his steps and landings shake the ground; Y is a two-fisted slam that shakes everything
//! around (held: charged, like the hammer's). Fists only while he's big.
//! He grows the way SM64's power-ups flash, in steps up and down (frozen meanwhile), with SM64's
//! power-up music. His whole SM64 world is scaled (lib.rs scale()), so his collisions grow with him:
//! a low ceiling or a narrow passage stops him like it would a giant. With no room for him there
//! (a ceiling, walls close by) he doesn't grow: Lakitu's buzz.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use glam::Vec3;

use crate::{log, sm64, worker};

/// how big, how long (s), how long before the next one (s, from when it ends)
pub const SIZE: f32 = 4.0;
const LAST: f32 = 20.0;
const RECHARGE: f32 = 75.0;
/// growing and shrinking (s), the steps shown on the way (times his size before)
const GROW_TIME: f32 = 1.3;
const SHRINK_TIME: f32 = 0.8;
const STEPS: [f32; 7] = [1.0, 1.6, 1.25, 2.4, 1.8, 3.1, SIZE];
/// his blows hit this much harder, enemies' hurt him this much
pub const DAMAGE: f32 = 1.35;
pub const HURT: f32 = 0.5;
/// his room: his height (m) and how wide he is (m) at SIZE
const HEIGHT: f32 = 1.6 * SIZE + 0.3;
const WIDTH: f32 = 0.5 * SIZE + 0.15;

const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_POWER_UP: i32 = 0x3058_FF81;
const SOUND_YAHOO: i32 = 0x2404_8081;
const SOUND_SHRINK: i32 = 0x305C_0081;
const SOUND_HEAVY_LANDING: i32 = 0x0460_8081;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Small,
    Growing(Instant),
    Big(Instant),
    Shrinking(Instant),
}

struct State {
    phase: Phase,
    ready_at: Option<Instant>,
    held: bool,
    /// a step of his: the tick of the last one, and whether he was in the air last tick
    last_step: u32,
    air: bool,
}

static STATE: Mutex<State> = Mutex::new(State { phase: Phase::Small, ready_at: None, held: false, last_step: 0, air: false });
/// how big he's drawn on top of his world's scale (f32 bits): the steps while he grows and shrinks
static VISUAL: AtomicU32 = AtomicU32::new(0x3F80_0000);

/// Mega Mario now (grown, his world scaled).
pub fn active() -> bool {
    matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Big(_))
}

/// Growing or shrinking (he holds still).
pub fn changing() -> bool {
    matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Growing(_) | Phase::Shrinking(_))
}

/// How big he's drawn on top of his world's scale.
pub fn visual() -> f32 {
    f32::from_bits(VISUAL.load(Ordering::Relaxed))
}

/// For the HUD: the share of the recharge done (1: ready), and whether he's big (or changing).
pub fn hud() -> (f32, bool) {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let ready = s.ready_at.map_or(1.0, |t| 1.0 - t.saturating_duration_since(Instant::now()).as_secs_f32() / RECHARGE);
    (ready, s.phase != Phase::Small)
}

/// How much of his time big is left (1 just grown, 0 at its end).
pub fn left() -> Option<f32> {
    match STATE.lock().unwrap_or_else(|e| e.into_inner()).phase {
        Phase::Big(t) => Some((1.0 - t.elapsed().as_secs_f32() / LAST).max(0.0)),
        _ => None,
    }
}

fn sounds(list: Vec<i32>) {
    worker::call("mega sound", move |_| unsafe {
        for s in list {
            sm64::sm64_play_sound_global(s);
        }
    });
}

/// What lib.rs does on this tick: change his world's scale (and re-centre it on him).
pub enum Change {
    /// to this size (times his own)
    Scale(f32),
}

/// Game thread, every SM64 tick: D-pad right held, whether he may grow now (on the ground, not
/// riding or in a cap, alive) and stay big (alive, no menu, not riding), whether there's room for a
/// giant where he stands, his action, Elden Ring feet. What lib.rs must do.
#[allow(clippy::too_many_arguments)]
pub fn tick(right: bool, can_start: bool, can_stay: bool, room: impl Fn() -> bool, _id: i32, action: u32, feet: Vec3, tick: u32) -> Option<Change> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let pressed = right && !s.held;
    s.held = right;
    let mut change = None;
    let step = |p: f32| STEPS[((p * STEPS.len() as f32) as usize).min(STEPS.len() - 1)];
    match s.phase {
        Phase::Small => {
            VISUAL.store(1.0f32.to_bits(), Ordering::Relaxed);
            if pressed {
                let ready = s.ready_at.is_none_or(|t| Instant::now() >= t);
                if !ready || !can_start {
                    sounds(vec![SOUND_BUZZ]);
                } else if !room() {
                    log("mega: no room for a giant here");
                    sounds(vec![SOUND_BUZZ]);
                } else {
                    log("mega: growing");
                    s.phase = Phase::Growing(Instant::now());
                    let freeze = (GROW_TIME * 30.0) as i32 + 2;
                    worker::call("mega grow", move |_| unsafe {
                        sm64::sm64_er_hitstop(freeze);
                        sm64::sm64_play_sound_global(SOUND_POWER_UP);
                    });
                    crate::impact::mega_change(feet, true);
                }
            }
        }
        Phase::Growing(t) => {
            let p = t.elapsed().as_secs_f32() / GROW_TIME;
            if p >= 1.0 {
                // grown: his world scales, he's drawn at his size again
                s.phase = Phase::Big(Instant::now());
                VISUAL.store(1.0f32.to_bits(), Ordering::Relaxed);
                change = Some(Change::Scale(SIZE));
                worker::call("mega on", |_| unsafe {
                    sm64::sm64_er_set_mega(1);
                    sm64::sm64_play_sound_global(SOUND_YAHOO);
                });
                crate::impact::mega_change(feet, true);
                crate::lakitu::shake(1.5);
            } else {
                VISUAL.store(step(p).to_bits(), Ordering::Relaxed);
            }
        }
        Phase::Big(t) => {
            // over: his time is up, D-pad right again, or he can't be big (a menu, his death)
            if t.elapsed().as_secs_f32() >= LAST || pressed || !allowed_big(can_stay, action) {
                log("mega: shrinking");
                s.phase = Phase::Shrinking(Instant::now());
                // (back to his size in the world at once, drawn big and stepping down)
                change = Some(Change::Scale(1.0));
                VISUAL.store(SIZE.to_bits(), Ordering::Relaxed);
                let freeze = (SHRINK_TIME * 30.0) as i32 + 2;
                worker::call("mega off", move |_| unsafe {
                    sm64::sm64_er_set_mega(0);
                    sm64::sm64_er_hitstop(freeze);
                    sm64::sm64_play_sound_global(SOUND_SHRINK);
                });
                crate::impact::mega_change(feet, false);
            } else {
                // his steps and landings shake the ground
                let air = action & 0x800 != 0;
                let landed = s.air && !air;
                s.air = air;
                if landed {
                    crate::lakitu::shake(1.3);
                    crate::impact::mega_step(feet, true);
                    sounds(vec![SOUND_HEAVY_LANDING]);
                } else if !air && action & 0x400 != 0 && tick.wrapping_sub(s.last_step) >= 11 {
                    s.last_step = tick;
                    crate::lakitu::shake(0.3);
                    crate::impact::mega_step(feet, false);
                }
            }
        }
        Phase::Shrinking(t) => {
            let p = t.elapsed().as_secs_f32() / SHRINK_TIME;
            if p >= 1.0 {
                s.phase = Phase::Small;
                s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
                VISUAL.store(1.0f32.to_bits(), Ordering::Relaxed);
            } else {
                // (his own size is 1 now: the steps back down, shown as times that)
                VISUAL.store(step(1.0 - p).to_bits(), Ordering::Relaxed);
            }
        }
    }
    change
}

/// He can stay big (no menu, alive, not swimming or riding: `allowed` says).
fn allowed_big(allowed: bool, action: u32) -> bool {
    // (swimming: 0x2000; cutscenes: group 0x100)
    allowed && action & 0x2000 == 0 && action & 0x1C0 != 0x100
}

/// Whether a giant fits where he stands: nothing over his head up to a giant's height, no walls
/// close around him. `ray` gives the first map hit between two Elden Ring points.
pub fn room(feet: Vec3, ray: impl Fn(Vec3, Vec3) -> Option<Vec3>) -> bool {
    if ray(feet + Vec3::Y * 0.4, feet + Vec3::Y * HEIGHT).is_some() {
        return false;
    }
    for up in [1.2, HEIGHT * 0.5, HEIGHT * 0.85] {
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU;
            let from = feet + Vec3::Y * up;
            if ray(from, from + Vec3::new(a.cos(), 0.0, a.sin()) * WIDTH).is_some() {
                return false;
            }
        }
    }
    true
}

/// Back to his size at once (Mario mode off, his death): lib.rs resets the scale.
pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.phase != Phase::Small {
        s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
    }
    s.phase = Phase::Small;
    VISUAL.store(1.0f32.to_bits(), Ordering::Relaxed);
    worker::call("mega clear", |_| unsafe { sm64::sm64_er_set_mega(0) });
}
