//! Blown away: a heavy blow from a boss or a big brute sometimes sends Mario flying far off (SM64's
//! thrown-backward, Bowser's toss), screaming; if a wall's in the way he smacks into it (stars, a
//! bonk, a shake) and drops. Only sometimes (a roll of the dice, and never twice in a row): every
//! hit sending him off would wear thin.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Vec2, Vec3};

use crate::{sm64, worker};

const ACT_THROWN_BACKWARD: u32 = 0x010208BE;
/// the odds of a heavy blow (two wedges) and of a lighter one (one wedge) sending him off, and
/// the time after one before the next can (s)
const CHANCE_HEAVY: f32 = 0.4;
const CHANCE_LIGHT: f32 = 0.1;
const COOLDOWN: f32 = 9.0;
/// enemies this close count as the one who hit him (m)
const NEAR: f32 = 5.0;
/// how fast he flies off (SM64 units per tick: back and up)
const SPEED: f32 = 58.0;
const UP: f32 = 46.0;
/// a wall: his speed flipped round from at least this fast (units per tick)
const SPLAT_SPEED: f32 = 20.0;

// SM64 sounds: his scream and oof, a bonk and a hit, a big pound
const SOUND_OOOF: i32 = 0x240B_D081;
const SOUND_DOH: i32 = 0x2430_8081;
const SOUND_BONK: i32 = 0x0445_A081;
const SOUND_HIT: i32 = 0x0444_A081;
const SOUND_POUND: i32 = 0x3044_0081;

struct State {
    last: Option<Instant>,
    /// flying since, his last speed sideways, whether he hit a wall already
    flying: Option<(Instant, Vec2, bool)>,
}

static STATE: Mutex<State> = Mutex::new(State { last: None, flying: None });

/// Flying off now (another blow meanwhile only costs health: SM64's knockback would cut it short).
pub fn flying() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).flying.is_some_and(|(t, ..)| t.elapsed().as_secs_f32() < 3.0)
}

/// Game thread, Mario was hit for `wedges`: whether this blow sends him flying (a boss or a big
/// one near him dealt it, and the dice say so). `at`: the Tarnished's feet.
pub fn roll(wedges: u32, player: &ChrIns, at: Vec3) -> bool {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.last.is_some_and(|t| t.elapsed().as_secs_f32() < COOLDOWN) || s.flying.is_some() {
        return false;
    }
    let near = crate::combat::in_the_way(at, NEAR);
    let by = crate::combat::handle_key(&player.last_hit_by);
    // (the one who hit him, if the game says and it's close: an archer's arrow from afar doesn't
    // count; else any big one close by)
    let known = unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.chr_ins_by_handle(&player.last_hit_by)).is_some();
    let big = match near.iter().find(|(_, k, _)| *k == by) {
        Some((_, _, big)) => *big,
        None if known => false,
        None => near.iter().any(|(_, _, big)| *big),
    };
    let chance = if wedges >= 2 { CHANCE_HEAVY } else { CHANCE_LIGHT };
    if !big || crate::rand01() >= chance {
        return false;
    }
    s.last = Some(Instant::now());
    s.flying = Some((Instant::now(), Vec2::ZERO, false));
    crate::log(format!("blown: a heavy blow sends Mario flying ({wedges} wedges)"));
    true
}

/// Worker thread, right after SM64 took the blow: off he goes, away from `from` (SM64 units).
pub fn launch(id: i32, mario: [f32; 3], from: [f32; 3]) {
    let (dx, dz) = (from[0] - mario[0], from[2] - mario[2]);
    // (facing the one who hit him, flying back from it; kept off ±pi, which the SM64 angle can't hold)
    let yaw = if dx.abs() + dz.abs() > 1.0 { dx.atan2(dz).clamp(-3.14, 3.14) } else { 0.0 };
    unsafe {
        sm64::sm64_set_mario_faceangle(id, yaw);
        sm64::sm64_set_mario_action(id, ACT_THROWN_BACKWARD);
        sm64::sm64_set_mario_forward_velocity(id, -SPEED);
        sm64::sm64_set_mario_velocity(id, yaw.sin() * -SPEED, UP, yaw.cos() * -SPEED);
        // (SM64's throw already screams)
        sm64::sm64_play_sound_volume(SOUND_POUND, 0.5);
    }
}

/// Game thread, every SM64 tick: while he flies, streaks behind him; a wall in the way flips his
/// speed round: the smack. `feet`: Elden Ring.
pub fn tick(id: i32, state: &sm64::SM64MarioState, feet: Vec3) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some((since, last, hit)) = s.flying else { return };
    let age = since.elapsed().as_secs_f32();
    // (the blow lands a tick or two after the roll: give it time to become the throw)
    if state.action != ACT_THROWN_BACKWARD {
        if age > 0.5 {
            s.flying = None;
        }
        return;
    }
    let v = Vec2::new(state.velocity[0], state.velocity[2]);
    crate::impact::dash_point(feet, last == Vec2::ZERO);
    let splat = !hit && last.length() > SPLAT_SPEED && v.dot(last) < 0.0;
    s.flying = Some((since, v, hit || splat));
    if splat {
        drop(s);
        let at = feet + Vec3::Y * 0.6 - Vec3::new(last.x, 0.0, last.y).normalize_or_zero() * 0.3;
        crate::impact::splat(at);
        crate::lakitu::shake(1.3);
        let voice = if crate::rand01() < 0.5 { SOUND_DOH } else { SOUND_OOOF };
        let fall = state.velocity[1].min(0.0);
        worker::call("splat", move |_| unsafe {
            // (SM64 bounces a knockback off a wall at full speed, back at the boss: he drops instead)
            sm64::sm64_set_mario_forward_velocity(id, -4.0);
            sm64::sm64_set_mario_velocity(id, 0.0, fall, 0.0);
            sm64::sm64_er_hitstop(5);
            sm64::sm64_play_sound_global(SOUND_BONK);
            sm64::sm64_play_sound_volume(SOUND_HIT, 0.8);
            sm64::sm64_play_sound_global(voice);
        });
        crate::log("blown: Mario smacked into a wall");
    }
}

pub fn clear() {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).flying = None;
}
