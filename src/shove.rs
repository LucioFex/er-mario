//! Shoves: an enemy's blow that costs no health (a push, a shield bash, a kick that only staggers)
//! still throws the Tarnished into one of his damage animations. Mario reacted only to HP loss, so
//! such a blow did nothing: now he reels back too (SM64's soft knockback, no wedge lost).
//! The damage animations' range is a guess (Elden Ring's player: 8xxx), so every new animation of
//! the Tarnished's with an enemy near is logged ("shove:") to check it.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Instant;

use crate::log;

/// the Tarnished's damage animations (without the weapon section: anim % 1_000_000)
const DAMAGE: std::ops::Range<i32> = 8000..9000;
/// a real hit's own damage animation comes a frame or two after its HP loss: not a shove (s)
const AFTER_HURT: f32 = 0.5;
/// no second shove before this (s)
const COOLDOWN: f32 = 0.8;
/// how many different animations to log in a session
const LOG_MAX: usize = 60;

struct State {
    anim: i32,
    hurt: Option<Instant>,
    shoved: Option<Instant>,
    logged: HashSet<i32>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// A real hit (HP lost) on Mario just now.
pub fn hurt() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = s.as_mut() {
        s.hurt = Some(Instant::now());
    }
}

/// Game thread, every frame: the Tarnished's animation now, how far the nearest enemy is (m), and
/// whether Mario can reel back now. True when it just turned into a damage animation that no HP
/// loss explains: a shove.
pub fn check(anim: i32, nearest: Option<f32>, free: bool) -> bool {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.get_or_insert_with(|| State { anim, hurt: None, shoved: None, logged: HashSet::new() });
    let last = std::mem::replace(&mut s.anim, anim);
    if last == anim {
        return false;
    }
    let a = anim % 1_000_000;
    let near = nearest.filter(|&d| d <= 8.0);
    if let Some(d) = near {
        if a > 0 && s.logged.len() < LOG_MAX && s.logged.insert(anim) {
            log(format!("shove: Tarnished anim {anim} (was {last}), an enemy {d:.1} m away"));
        }
    }
    if !free || !DAMAGE.contains(&a) || DAMAGE.contains(&(last % 1_000_000)) || near.is_none() {
        return false;
    }
    if s.hurt.is_some_and(|t| t.elapsed().as_secs_f32() < AFTER_HURT) || s.shoved.is_some_and(|t| t.elapsed().as_secs_f32() < COOLDOWN) {
        return false;
    }
    s.shoved = Some(Instant::now());
    log(format!("shove: no damage, Mario reels back (anim {anim})"));
    true
}
