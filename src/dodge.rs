//! The perfect dodge (Bayonetta's Witch Time): B's dash started while an enemy close by, facing
//! Mario, has just started one of its attacks slows the world down for a moment (Mario keeps his
//! own speed, style.rs), with a blue flash, a ring and a "waha!"; his blows up close in that moment
//! are counters and hit harder (combat.rs). Once in a while, not on every dash near an enemy.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::{log, sm64, worker};

/// an enemy this close (m, off its body) in an attack: the dash's first this many ticks dodge it
const NEAR: f32 = 2.8;
const WINDOW: u32 = 6;
/// the world slowed this long (s); the counters last this long (s) and hit this much harder; the
/// next perfect dodge at least this long after (s)
const SLOW_TIME: f32 = 1.0;
const COUNTER_TIME: f32 = 1.6;
pub const COUNTER_DAMAGE: f32 = 1.6;
const COOLDOWN: f32 = 2.5;
/// enemies' attacks: their animations (a000_003000 on, Elden Ring's enemy attacks)
const ATTACKS: std::ops::Range<i32> = 3000..4000;
/// ...started at most this long ago (s: a slow wind-up's blow comes late, its recovery later), by
/// an enemy facing Mario (cos 60°)
const FRESH: f32 = 1.1;
const FACING_COS: f32 = 0.5;

const SOUND_WAHA: i32 = 0x242B_8081 + (1 << 16);
const SOUND_SHORT_STAR: i32 = 0x3016_0091;

struct State {
    dash_since: Option<u32>,
    last: Option<Instant>,
    counter_until: Option<Instant>,
    /// the Star Sword's Flurry Rush of this counter was used (one a perfect dodge)
    flurried: bool,
}

static STATE: Mutex<State> = Mutex::new(State { dash_since: None, last: None, counter_until: None, flurried: false });

/// Mario's blows are counters right now (a perfect dodge a moment ago).
pub fn countering() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).counter_until.is_some_and(|t| Instant::now() < t)
}

/// The counter time is on and its Flurry Rush not used yet (the sword's: one a perfect dodge, the
/// counter's blows stay counters).
pub fn flurry_ready() -> bool {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    !s.flurried && s.counter_until.is_some_and(|t| Instant::now() < t)
}

/// The Flurry Rush started: none more until the next perfect dodge.
pub fn spend_flurry() {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).flurried = true;
}

/// Game thread, every SM64 tick: Mario's action and feet (Elden Ring). True when it was a perfect
/// dodge.
pub fn tick(action: u32, feet: Vec3, tick: u32) -> bool {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !crate::moves::dodging(action) {
        s.dash_since = None;
        return false;
    }
    let since = *s.dash_since.get_or_insert(tick);
    if tick.wrapping_sub(since) > WINDOW || s.last.is_some_and(|t| t.elapsed().as_secs_f32() < COOLDOWN) {
        return false;
    }
    // (the Star Sword's step has its own parry, starstep.rs)
    if crate::starstep::stepping() {
        return false;
    }
    let attacker = attacker(feet, NEAR, (0.0, FRESH), Some(since)).map(|f| (f.id, f.anim));
    let Some((id, anim)) = attacker else { return false };
    // (once: the rest of this dash can't dodge again)
    s.dash_since = Some(tick.wrapping_sub(WINDOW + 1));
    drop(s);
    grant(feet, id, anim);
    true
}

/// A perfect dodge of `id`'s attack `anim` (B's dash, or the Star Step against a colossus): the
/// counter window, Witch Time, the flash and the "waha!". `feet`: Mario's (Elden Ring).
pub fn grant(feet: Vec3, id: i32, anim: i32) {
    open_counter(true);
    spend();
    log(format!("dodge: perfect dodge (c{id:04}, attack {anim}), witch time"));
    crate::style::witch_time(SLOW_TIME);
    crate::impact::perfect_dodge(feet);
    worker::call("perfect dodge", |_| unsafe {
        sm64::sm64_play_sound_volume(SOUND_WAHA, 0.55);
        sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.35);
    });
}

/// The counter window opens: Mario's blows are counters for COUNTER_TIME; `flurry`: the sword's
/// Flurry Rush is ready too (a perfect dodge's, not the parry's Riposte).
pub fn open_counter(flurry: bool) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.counter_until = Some(Instant::now() + std::time::Duration::from_secs_f32(COUNTER_TIME));
    s.flurried = !flurry;
}

/// A perfect dodge, a parry or a Phantom Cut was just spent: none of them for COOLDOWN s.
pub fn spend() {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).last = Some(Instant::now());
}

/// COOLDOWN is over: a perfect dodge, a parry or a Phantom Cut may come.
pub fn ready() -> bool {
    !STATE.lock().unwrap_or_else(|e| e.into_inner()).last.is_some_and(|t| t.elapsed().as_secs_f32() < COOLDOWN)
}

/// An animation (`anim`, its id) that has played `age` s is an attack begun within `ages` (s).
pub fn attacking(anim: i32, age: f32, ages: (f32, f32)) -> bool {
    ATTACKS.contains(&anim) && (ages.0..=ages.1).contains(&age)
}

/// An enemy in an attack facing Mario.
pub struct Found {
    pub handle: FieldInsHandle,
    pub id: i32,
    pub anim: i32,
    /// its feet and the way it faces (Elden Ring)
    pub pos: Vec3,
    pub forward: Vec3,
}

/// The nearest enemy within `reach` (off its body) of `feet` in an attack begun within `ages` s ago
/// that faces Mario (cos 60°). `said`: the dash's start, to log once an attack that faces away.
pub fn attacker(feet: Vec3, reach: f32, ages: (f32, f32), said: Option<u32>) -> Option<Found> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return None };
    crate::combat::in_the_way(feet, reach).into_iter().find_map(|(handle, ..)| {
        let chr = wcm.chr_ins_by_handle(&handle)?;
        let t = &chr.modules.time_act;
        let a = &t.anim_queue[(t.read_idx % 10) as usize];
        let anim = a.anim_id % 1_000_000;
        let ph = &chr.modules.physics;
        let (o, p) = (ph.orientation, ph.position);
        let ahead = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(Vec3::new(0.0, 0.0, -1.0)).with_y(0.0).normalize_or_zero();
        let to = (feet - Vec3::new(p.0, p.1, p.2)).with_y(0.0).normalize_or_zero();
        let facing = ahead.dot(to) >= FACING_COS;
        let attacking = attacking(anim, a.play_time, ages);
        static SAID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
        if let Some(since) = said.filter(|_| attacking && !facing) {
            if SAID.swap(since, std::sync::atomic::Ordering::Relaxed) != since {
                // (which way an enemy's forward is was a guess: logged to check)
                log(format!("dodge: c{:04} attacks ({anim}) but faces away from Mario ({:.2})", chr.character_id, ahead.dot(to)));
            }
        }
        (attacking && facing).then_some(Found { handle, id: chr.character_id as i32, anim, pos: Vec3::new(p.0, p.1, p.2), forward: ahead })
    })
}

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.dash_since = None;
    s.counter_until = None;
    s.flurried = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attack_is_judged_by_its_animation_and_age() {
        // the perfect dodge: any attack up to FRESH old; the Star Step: not before 0.15 s, not after 0.9
        assert!(attacking(3010, 0.05, (0.0, FRESH)));
        assert!(attacking(3010, 1.1, (0.0, FRESH)));
        assert!(!attacking(3010, 1.2, (0.0, FRESH)));
        assert!(!attacking(3010, 0.1, crate::starstep::AGE));
        assert!(attacking(3010, 0.15, crate::starstep::AGE));
        assert!(attacking(3999, 0.9, crate::starstep::AGE));
        assert!(!attacking(3010, 0.95, crate::starstep::AGE));
        // (idle, walking and the like are not attacks)
        assert!(!attacking(2000, 0.5, crate::starstep::AGE));
        assert!(!attacking(4000, 0.5, crate::starstep::AGE));
    }
}
