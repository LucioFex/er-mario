//! Situational criticals (v4 Part 3d; Mortal Kombat 11's Krushing Blows): the right blow at the
//! right moment hits ×1.6 with a moment of slow-mo, the camera punched in and "CRITICAL", at most
//! once an enemy every 20 s (combat.rs keeps the wait). The moments:
//! - a launcher on an enemy in the middle of one of its attacks
//! - the hammer's heavy blows on an enemy whose stance is broken
//! - the sword's thrust, iai or stabs in his back
//! - a Chain Chomp bite on one up in the air
//! - a boomerang striking him both on its way out and on its way back

use std::sync::Mutex;
use std::time::Instant;

use crate::combat::Attack;

/// a critical's worth, the wait before the next on the same enemy (ticks)
pub const BONUS: f32 = 1.6;
pub const EVERY: u32 = 600;

/// What the enemy a blow lands on is doing (combat.rs reads it as the blow lands).
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    /// in one of its attacks, freshly begun
    pub attacking: bool,
    /// its stance broken (swing.rs)
    pub stance_open: bool,
    /// Mario behind it
    pub from_behind: bool,
    /// up in the air (juggle.rs)
    pub juggled: bool,
    /// a boomerang that struck it on its way out strikes it again coming back (boomerang.rs)
    pub both_ways: bool,
}

fn launcher(a: Attack) -> bool {
    matches!(
        a,
        Attack::Uppercut | Attack::UppercutTop | Attack::Launch | Attack::ChompLaunch | Attack::RangUpper | Attack::SwordRise | Attack::SwordLauncher | Attack::FistFlurryEnd | Attack::HammerRise | Attack::Rise
    )
}

fn hammer_heavy(a: Attack) -> bool {
    matches!(
        a,
        Attack::HammerSmash | Attack::Quake | Attack::QuakeBig | Attack::Titan | Attack::FlurryEnd | Attack::Golf | Attack::HammerChargeSlam | Attack::Comet | Attack::CometRing
    )
}

fn thrust(a: Attack) -> bool {
    matches!(a, Attack::SwordThrust | Attack::SwordIai | Attack::SwordStab | Attack::SwordStabEnd)
}

fn bite(a: Attack) -> bool {
    matches!(a, Attack::ChompLaunch | Attack::Frenzy | Attack::FrenzyEnd | Attack::ChompRush)
}

fn rang(a: Attack) -> bool {
    matches!(a, Attack::Rang | Attack::RangSkim)
}

/// A blow that can be a critical at all (what's worth reading the enemy for).
pub fn candidate(a: Attack) -> bool {
    launcher(a) || hammer_heavy(a) || thrust(a) || bite(a) || rang(a)
}

/// The critical `attack` is on an enemy doing `ctx`, if it is one (its name for the log).
pub fn kind(attack: Attack, ctx: &Ctx) -> Option<&'static str> {
    if launcher(attack) && ctx.attacking {
        return Some("a launcher in the middle of its attack");
    }
    if hammer_heavy(attack) && ctx.stance_open {
        return Some("a heavy blow on a broken stance");
    }
    if thrust(attack) && ctx.from_behind {
        return Some("a thrust in the back");
    }
    if bite(attack) && ctx.juggled {
        return Some("a bite up in the air");
    }
    if rang(attack) && ctx.both_ways {
        return Some("a boomerang both ways");
    }
    None
}

/// The enemies a boomerang struck on its way out and now strikes coming back (a moment).
static BOTH_WAYS: Mutex<Vec<(u64, Instant)>> = Mutex::new(Vec::new());

/// boomerang.rs: the boomerang that struck `key` on its way out strikes it on its way back.
pub fn both_ways(key: u64) {
    let mut b = BOTH_WAYS.lock().unwrap_or_else(|e| e.into_inner());
    b.retain(|(_, at)| at.elapsed().as_secs_f32() < 1.0);
    b.push((key, Instant::now()));
}

/// combat.rs: whether the boomerang blow landing on `key` is one struck both ways (once).
pub fn take_both_ways(key: u64) -> bool {
    let mut b = BOTH_WAYS.lock().unwrap_or_else(|e| e.into_inner());
    b.retain(|(_, at)| at.elapsed().as_secs_f32() < 1.0);
    match b.iter().position(|(k, _)| *k == key) {
        Some(i) => {
            b.remove(i);
            true
        }
        None => false,
    }
}

/// An enemy's last critical at `last` (ticks): whether one can come again at `tick`.
pub fn ready(last: Option<u32>, tick: u32) -> bool {
    last.is_none_or(|t| tick.wrapping_sub(t) >= EVERY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_right_blow_at_the_right_moment() {
        let ctx = |f: fn(&mut Ctx)| {
            let mut c = Ctx::default();
            f(&mut c);
            c
        };
        assert!(kind(Attack::SwordLauncher, &ctx(|c| c.attacking = true)).is_some());
        assert!(kind(Attack::SwordLauncher, &Ctx::default()).is_none());
        assert!(kind(Attack::Titan, &ctx(|c| c.stance_open = true)).is_some());
        assert!(kind(Attack::Hammer, &ctx(|c| c.stance_open = true)).is_none());
        assert!(kind(Attack::SwordThrust, &ctx(|c| c.from_behind = true)).is_some());
        assert!(kind(Attack::SwordThrust, &ctx(|c| c.attacking = true)).is_none());
        assert!(kind(Attack::Frenzy, &ctx(|c| c.juggled = true)).is_some());
        assert!(kind(Attack::Chomp, &ctx(|c| c.juggled = true)).is_none());
        assert!(kind(Attack::Rang, &ctx(|c| c.both_ways = true)).is_some());
        assert!(kind(Attack::Rang, &ctx(|c| c.from_behind = true)).is_none());
        // (a plain punch never)
        assert!(!candidate(Attack::Punch) && kind(Attack::Punch, &ctx(|c| c.attacking = true)).is_none());
    }

    #[test]
    fn once_an_enemy_every_twenty_seconds() {
        assert!(ready(None, 5));
        assert!(!ready(Some(100), 100 + EVERY - 1));
        assert!(ready(Some(100), 100 + EVERY));
    }

    #[test]
    fn a_boomerang_both_ways_counts_once() {
        both_ways(99);
        assert!(take_both_ways(99));
        assert!(!take_both_ways(99));
        assert!(!take_both_ways(98));
    }
}
