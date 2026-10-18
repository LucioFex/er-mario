//! The Rock Mushroom (D-pad left, Super Mario Galaxy 2's): Mario as a boulder for a while, his head
//! peeking out of it. libsm64's ACT_ER_ROCK rolls it on the real ground: down slopes faster, up
//! them slower, off walls, a slam after a long drop; A hops, X rams. What it rolls into it hits:
//! regular enemies are bowled over (the game's knockdown), three at once a "STRIKE!", props shatter,
//! bosses and big bodies take a little and bounce him off. Hits cost him half and don't knock him
//! out of it. No weapons or moves meanwhile.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use glam::{Quat, Vec3};

use crate::engine_mario::{KIT_ROCK, PARTS, PartPose, SM64_PARTS};
use crate::{log, sm64, worker};

pub const ACT_ER_ROCK: u32 = 0x0080_047C;
/// how long it lasts, and the wait before the next (s)
pub const TIME: f32 = 12.0;
const RECHARGE: f32 = 75.0;
/// three enemies bowled over within this long (s): a strike
const STRIKE_WITHIN: f32 = 1.5;
/// bouncing off a big body: the speed kept, and pushed out of it (SM64 units per tick)
const BOUNCE_KEEP: f32 = 0.55;
const BOUNCE_OUT: f32 = 14.0;

const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_POUND_ROCK: i32 = 0x3056_0081;
const SOUND_SHORT_STAR: i32 = 0x3016_0091;
const SOUND_BREAK: i32 = 0x3041_C081;
const SOUND_HERE_WE_GO: i32 = 0x240C_8081;

static ON: AtomicBool = AtomicBool::new(false);

struct State {
    since: Option<Instant>,
    ready_at: Option<Instant>,
    held: bool,
    /// the boulder's roll so far, where it was last tick, its pop in (s since it came)
    roll: Quat,
    last: Option<Vec3>,
    /// enemies bowled over lately (for the strike), a strike called out for them already
    bowled: Vec<(u64, Instant)>,
    struck: bool,
    /// bounced off something big: the way out (ER, horizontal), for the next tick
    bounce: Option<Vec3>,
}

static STATE: Mutex<State> = Mutex::new(State {
    since: None,
    ready_at: None,
    held: false,
    roll: Quat::IDENTITY,
    last: None,
    bowled: Vec::new(),
    struck: false,
    bounce: None,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Mario is a boulder now.
pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}

/// For the HUD: the share of its recharge done (1 ready), and how much of it is left while on.
pub fn hud() -> (f32, Option<f32>) {
    let s = state();
    let ready = s.ready_at.map_or(1.0, |t| 1.0 - t.saturating_duration_since(Instant::now()).as_secs_f32() / RECHARGE);
    let left = s.since.map(|t| (1.0 - t.elapsed().as_secs_f32() / TIME).clamp(0.0, 1.0));
    (ready, left)
}

fn set(id: i32, on: bool) {
    ON.store(on, Ordering::Relaxed);
    worker::call("rock", move |_| unsafe { sm64::sm64_er_rock(id, on as i32) });
}

/// Game thread, every SM64 tick: D-pad left held, whether it may start now (on his feet or in the
/// air, free, not big, no cap on, not riding or holding anyone) and stay on, his feet (Elden Ring).
pub fn tick(left: bool, can_start: bool, can_stay: bool, id: i32, action: u32, feet: Vec3) {
    let mut s = state();
    let pressed = left && !s.held;
    s.held = left;
    if let Some(since) = s.since {
        // over: its time, D-pad left again, or he can't be one any more
        let over = since.elapsed().as_secs_f32() >= TIME || pressed || !can_stay || (since.elapsed().as_secs_f32() > 0.3 && action != ACT_ER_ROCK);
        if over {
            s.since = None;
            // (it never really started, he couldn't be one: no wait)
            s.ready_at = (since.elapsed().as_secs_f32() > 1.0).then(|| Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
            s.last = None;
            drop(s);
            set(id, false);
            log("rock: back to Mario");
            crate::impact::rock(feet, false);
            worker::call("rock off", |_| unsafe {
                sm64::sm64_play_sound_global(SOUND_BREAK);
            });
        }
        return;
    }
    if !pressed {
        return;
    }
    let ready = s.ready_at.is_none_or(|t| Instant::now() >= t);
    if !ready || !can_start {
        drop(s);
        worker::call("rock buzz", |_| unsafe { sm64::sm64_play_sound_global(SOUND_BUZZ) });
        return;
    }
    s.since = Some(Instant::now());
    s.roll = Quat::IDENTITY;
    s.last = None;
    s.bowled.clear();
    s.struck = false;
    s.bounce = None;
    drop(s);
    set(id, true);
    log("rock: Rock Mushroom on");
    crate::impact::rock(feet, true);
    worker::call("rock on", |_| unsafe {
        sm64::sm64_play_sound_global(SOUND_POUND_ROCK);
        sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.3);
        sm64::sm64_play_sound_global(SOUND_HERE_WE_GO);
    });
}

/// Game thread (combat.rs): the boulder rolled into an enemy at `at` (Elden Ring feet): `big` it
/// bounces him off, a regular one is bowled over (counted for the strike). `mario`: his feet.
pub fn rolled_into(key: u64, at: Vec3, big: bool, mario: Vec3) {
    let mut s = state();
    if big {
        s.bounce = Some((mario - at).with_y(0.0).normalize_or(Vec3::X));
        return;
    }
    s.bowled.retain(|(k, t)| t.elapsed().as_secs_f32() < STRIKE_WITHIN && *k != key);
    s.bowled.push((key, Instant::now()));
    if s.bowled.len() < 3 {
        s.struck = false;
    } else if !s.struck {
        s.struck = true;
        drop(s);
        log("rock: strike (three bowled over)");
        crate::style::callout("STRIKE!", Some(at + Vec3::Y * 2.0), [1.0, 0.85, 0.3], true, 5.0);
    }
}

/// Bounced off something big: Mario's new velocity (SM64 units per tick, SM64 axes) from his
/// velocity now, for the next tick.
pub fn take_bounce(velocity: [f32; 3]) -> Option<[f32; 3]> {
    let out = state().bounce.take()?;
    // (SM64's X is Elden Ring's mirrored)
    let n = Vec3::new(-out.x, 0.0, out.z);
    let v = Vec3::from(velocity).with_y(0.0);
    let d = v.dot(n);
    let back = if d < 0.0 { v - n * 2.0 * d } else { v };
    let back = back * BOUNCE_KEEP + n * BOUNCE_OUT;
    Some([back.x, velocity[1].max(18.0), back.z])
}

/// Mario's model as the boulder: his body hidden in it, his head peeking out of its top towards
/// where it rolls, the boulder rolling along the ground (`feet`: Elden Ring), popping in.
pub fn pose(parts: &mut [PartPose; PARTS], feet: Vec3, face_angle: f32) {
    let mut s = state();
    let Some(since) = s.since else { return };
    let r = crate::assets::kit::ROCK_R * crate::size_factor();
    // its roll: about the axis across the way it went, as far as it went over its radius
    if let Some(last) = s.last {
        let d = (feet - last).with_y(0.0);
        if d.length() > 1e-4 && d.length() < 3.0 {
            let axis = Vec3::Y.cross(d).normalize_or(Vec3::X);
            s.roll = (Quat::from_axis_angle(axis, d.length() / r) * s.roll).normalize();
        }
    }
    s.last = Some(feet);
    let age = since.elapsed().as_secs_f32();
    let pop = if age < 0.12 { 0.3 + 1.0 * age / 0.12 } else if age < 0.3 { 1.3 - 0.3 * (age - 0.12) / 0.18 } else { 1.0 };
    let centre = Vec3::Y * r;
    parts[KIT_ROCK] = PartPose { rot: s.roll, pos: centre, scale: pop * crate::size_factor(), stretch: Vec3::ONE };
    // his head out of its top, a little ahead, bobbing as it rolls; the rest of him inside
    let ahead = Vec3::new(-face_angle.sin(), 0.0, face_angle.cos());
    let head = parts[3];
    let want = centre + ahead * r * 0.42 + Vec3::Y * r * 0.78 + Vec3::Y * (age * 9.0).sin() * 0.015;
    let shift = want - head.pos;
    for (i, p) in parts.iter_mut().enumerate().take(SM64_PARTS + 4).skip(1) {
        // (the head and its eye variants move together; the body stays inside)
        if i == 3 || i >= SM64_PARTS {
            p.pos += shift;
        } else {
            p.scale = 0.01;
        }
    }
    // (the weapons on his back or in his hand inside it too; the boomerangs out flying stay)
    for p in parts.iter_mut().take(crate::engine_mario::KIT).skip(crate::engine_mario::HAMMER) {
        if p.pos.length() < 2.0 {
            p.scale = 0.01;
        }
    }
}

pub fn clear() {
    ON.store(false, Ordering::Relaxed);
    let mut s = state();
    s.since = None;
    s.last = None;
    s.bounce = None;
    s.bowled.clear();
}
