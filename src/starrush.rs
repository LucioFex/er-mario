//! Star Rush (Devil May Cry's Devil Trigger, Mario's star): style fills a star meter; full, R3
//! sets it off for a while: super armour (hits cost half and don't knock him about), a little
//! faster (his whole SM64 clock: moves, blows, steps), blows a little harder, star sparks round him
//! and SM64's power-up music, low. Not with the Mega Mushroom or the Rock Mushroom.

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

use crate::{log, sm64, worker};

/// how long it lasts (s), the style points that fill it, how much faster and harder Mario is
pub const TIME: f32 = 10.0;
const FULL: f32 = 110.0;
pub const SPEED: f32 = 1.15;
pub const DAMAGE: f32 = 1.15;

const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_STAR: i32 = 0x701E_FF81;
const SOUND_HERE_WE_GO: i32 = 0x240C_8081;
const SOUND_SHORT_STAR: i32 = 0x3016_0091;

struct State {
    meter: f32,
    since: Option<Instant>,
    held: bool,
    /// the meter was full already (its chime once)
    told: bool,
}

static STATE: Mutex<State> = Mutex::new(State { meter: 0.0, since: None, held: false, told: false });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Star Rush is on.
pub fn active() -> bool {
    state().since.is_some()
}

/// For the HUD: how full the meter is (0-1), and how much of the rush is left while it's on.
pub fn hud() -> (f32, Option<f32>) {
    let s = state();
    (s.meter / FULL, s.since.map(|t| (1.0 - t.elapsed().as_secs_f32() / TIME).clamp(0.0, 1.0)))
}

/// Style points earned (style.rs): into the meter (not while it's on).
pub fn feed(points: f32) {
    let mut s = state();
    if s.since.is_some() {
        return;
    }
    s.meter = (s.meter + points).min(FULL);
    if s.meter >= FULL && !s.told {
        s.told = true;
        drop(s);
        log("star rush: the meter is full (R3)");
        crate::style::callout("STAR RUSH READY", None, [1.0, 0.9, 0.35], false, 0.0);
        worker::call("star ready", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.2) });
    }
}

/// Game thread, every SM64 tick: R3 held now, whether it may start (free, not big, not a boulder),
/// Mario's feet (Elden Ring), the tick. Starts it, ends it, keeps its sparkle going.
pub fn tick(r3: bool, can_start: bool, can_stay: bool, feet: Vec3, tick: u32) {
    let mut s = state();
    let pressed = r3 && !s.held;
    s.held = r3;
    if let Some(since) = s.since {
        if since.elapsed().as_secs_f32() >= TIME || !can_stay {
            s.since = None;
            s.meter = 0.0;
            s.told = false;
            drop(s);
            log("star rush: over");
            worker::call("star rush off", |_| unsafe { sm64::sm64_er_star_music(0) });
            crate::impact::star_rush(feet, false);
            return;
        }
        drop(s);
        // (stars round him, now and then a ring)
        if tick % 4 == 0 {
            crate::impact::star_spark(feet);
        }
        return;
    }
    if !pressed {
        return;
    }
    if s.meter < FULL || !can_start {
        drop(s);
        worker::call("star buzz", |_| unsafe { sm64::sm64_play_sound_global(SOUND_BUZZ) });
        return;
    }
    s.since = Some(Instant::now());
    drop(s);
    log("star rush: on");
    crate::style::callout("STAR RUSH!", None, [1.0, 0.9, 0.35], true, 0.0);
    crate::impact::star_rush(feet, true);
    worker::call("star rush on", |_| unsafe {
        sm64::sm64_er_star_music(1);
        sm64::sm64_play_sound_volume(SOUND_STAR, 0.35);
        sm64::sm64_play_sound_global(SOUND_HERE_WE_GO);
    });
}

/// Mario's death, out of Mario mode: over, the meter kept (it's earned).
pub fn clear() {
    let mut s = state();
    if s.since.take().is_some() {
        s.meter = 0.0;
        s.told = false;
        worker::call("star rush off", |_| unsafe { sm64::sm64_er_star_music(0) });
    }
    s.held = false;
}
