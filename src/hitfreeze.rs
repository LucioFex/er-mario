//! The enemy's half of a blow's hit-stop: while Mario's move holds still on a blow that lands
//! (impact.rs, SM64's hit-stop), the one he struck holds too, its animation all but stopped, then
//! runs on. Both standing still a few frames is what gives a blow its weight (Devil May Cry's,
//! God of War's crunch); before, Mario alone froze and the enemy slid on through it.
//!
//! By its animation speed (the swamp's slow, carpet.rs, uses the same): set for the hold, given
//! back after, only if nothing else changed it meanwhile. Bosses hold less and more briefly (a
//! blow shouldn't stop a boss's attack: a taste of it only). Not on whoever is stopped in time
//! (stopwatch.rs), flying (juggle, bat, knock), carried or down, nor as a co-op guest (the host's
//! enemies).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use fromsoftware_shared::FromStatic;

use crate::combat;

/// the animation's speed while held: a regular enemy, a boss
const HELD: f32 = 0.04;
const HELD_BOSS: f32 = 0.35;
/// the longest hold (SM64 ticks, 30 a second): a regular enemy, a boss
const MOST: u32 = 7;
const MOST_BOSS: u32 = 3;
/// at most this many held at once (a sweep through a crowd)
const MOST_HELD: usize = 8;

struct Held {
    handle: FieldInsHandle,
    until: Instant,
    speed: f32,
    before: f32,
}

static HELD_NOW: Mutex<Vec<Held>> = Mutex::new(Vec::new());

/// How long (ticks) and how slow a blow of `stop` ticks holds an enemy: None, not at all.
pub fn hold_for(stop: u32, boss: bool) -> Option<(u32, f32)> {
    if stop == 0 {
        return None;
    }
    Some(if boss { (stop.min(MOST_BOSS), HELD_BOSS) } else { (stop.min(MOST), HELD) })
}

/// Game thread: Mario's blow with a hit-stop of `stop` SM64 ticks landed on `h`.
pub fn struck(h: &FieldInsHandle, stop: u32) {
    if crate::coop::guest() || crate::stopwatch::frozen(h) || crate::juggle::is_juggled(h) || crate::bat::is_flying(h) || crate::knock::is_flying(h) || crate::carry::is_carried(h) || crate::swing::is_down(h) {
        return;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle_mut(h) else { return };
    if chr.modules.data.hp <= 0 {
        return;
    }
    let boss = combat::is_boss_now(h) || combat::boss_class(h, chr);
    let Some((ticks, speed)) = hold_for(stop, boss) else { return };
    let until = Instant::now() + std::time::Duration::from_secs_f32(ticks as f32 / 30.0);
    let mut held = HELD_NOW.lock().unwrap_or_else(|e| e.into_inner());
    let key = combat::handle_key(h);
    if let Some(e) = held.iter_mut().find(|e| combat::handle_key(&e.handle) == key) {
        // (held already: a little longer, its own speed kept to give back)
        e.until = e.until.max(until);
        return;
    }
    if held.len() >= MOST_HELD {
        return;
    }
    let before = chr.modules.behavior.animation_speed;
    // (someone else's speed: a swamp's slow is fine, anything odd is left alone)
    if !before.is_finite() || !(0.1..=2.0).contains(&before) {
        return;
    }
    chr.modules.behavior.animation_speed = speed.min(before);
    held.push(Held { handle: *h, until, speed: speed.min(before), before });
}

/// Game thread, every frame: the holds over run on.
pub fn update() {
    let mut held = HELD_NOW.lock().unwrap_or_else(|e| e.into_inner());
    if held.is_empty() {
        return;
    }
    let now = Instant::now();
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    held.retain(|e| {
        if now < e.until {
            return true;
        }
        if let Some(chr) = wcm.chr_ins_by_handle_mut(&e.handle) {
            // (only if it's still ours: the stopwatch or the swamp may have set their own)
            if (chr.modules.behavior.animation_speed - e.speed).abs() < 1e-4 {
                chr.modules.behavior.animation_speed = e.before;
            }
        }
        false
    });
}

/// Everyone held runs on at once (a death, a load, Mario off).
pub fn clear() {
    let mut held = HELD_NOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
        for e in held.iter() {
            if let Some(chr) = wcm.chr_ins_by_handle_mut(&e.handle) {
                if (chr.modules.behavior.animation_speed - e.speed).abs() < 1e-4 {
                    chr.modules.behavior.animation_speed = e.before;
                }
            }
        }
    }
    held.clear();
}

#[cfg(test)]
mod tests {
    use super::hold_for;

    #[test]
    fn a_blow_holds_the_enemy_as_long_as_mario_a_boss_less() {
        assert_eq!(hold_for(0, false), None);
        assert_eq!(hold_for(2, false), Some((2, super::HELD)));
        // (the heaviest blows' long hit-stops capped: no stun lock)
        assert_eq!(hold_for(10, false).unwrap().0, super::MOST);
        let (t, s) = hold_for(8, true).unwrap();
        assert!(t <= super::MOST_BOSS && s > super::HELD);
    }
}
