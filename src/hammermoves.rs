//! The hammer's new moves as data (v4 Part 3): script.rs plays them (MOVES from id HAMMER_CHARGE
//! on), weaponplay.rs picks them. The hammer stays the slow, heavy, committed weapon: its new
//! moves close in or reach out (a shoulder charge, a volley of small hammers, a dive from the air)
//! and end in its heavy blows. The hammer is held as er_moves.c holds it (yaw round his facing, +
//! to his left; pitch up from level; its size), through the frame's weapon pose (lib.rs).

use crate::combat::Attack;
use crate::moves::{FX_GIANT, FX_GROUND, FX_LAUNCH, FX_QUAKE, FX_SLAM};
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, Shape, Window, ALL, BASE, DASH, FX_VOLLEY, SWAP};

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const GENERAL_LAND: i32 = 0x57;
const FAST_LONGJUMP: i32 = 0x13;
const GROUND_POUND: i32 = 0x3D;
const THROW_LIGHT_OBJECT: i32 = 0x52;
const SINGLE_JUMP: i32 = 0x4D;
const SOFT_BACK_KB: i32 = 0x74;

/// (SM64's: the soft ones at that volume, the voice at his body)
const SWISH: u32 = 0x305C_0081;
const UH: u32 = 0x2405_8081;
const HOOHOO: u32 = 0x2403_8081;
const WAH: u32 = 0x2424_8081;
const YAH: u32 = 0x241E_8081;
const HEAVY_LANDING: u32 = 0x0460_8081;

const fn w(from: u16, to: u16, shape: Shape, attack: Attack, fx: u32) -> Window {
    Window { from, to, shape, attack, fx, ground: false }
}

/// (only once he's on the ground: a landing's ring)
const fn wg(from: u16, to: u16, shape: Shape, attack: Attack, fx: u32) -> Window {
    Window { from, to, shape, attack, fx, ground: true }
}

const fn c(into: u8, from: u16, on_hit: u16) -> Cancel {
    Cancel { into, from, on_hit }
}

const fn clip(from: u16, anim: i32) -> Clip {
    Clip { from, anim, frames: &[] }
}

const FIRST_FRAME: &[crate::script::Key] = &[k(0, 0.0, Linear)];
const fn still(from: u16, anim: i32) -> Clip {
    Clip { from, anim, frames: FIRST_FRAME }
}

/// back, forward + X: the hammer hauled back over his right shoulder, a shoulder-first charge up to
/// 6 m that bowls into the first enemy, then the overhead slam where he stops (Hammer Bro's
/// charge into a Bowser smash)
pub const CHARGE: Move = Move {
    name: "hammer charge",
    len: 42,
    clips: &[still(0, FIRST_PUNCH), clip(5, FAST_LONGJUMP), still(16, SECOND_PUNCH), clip(20, SECOND_PUNCH), clip(26, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(4, -35.0, Out), k(15, -30.0, Linear), k(19, 10.0, Out), k(22, 0.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(4, -6.0, Out), k(7, 14.0, Out), k(15, 12.0, Linear), k(18, -10.0, Out), k(21, 22.0, In), k(30, 0.0, InOut)],
    aim: &[0, 4, 16],
    approach: Some(Approach { from: 5, to: 15, max: 6.0, gap: 1.2 }),
    squash: &[k(0, 1.0, Linear), k(4, 0.88, Out), k(7, 1.1, Out), k(15, 1.05, Linear), k(18, 1.1, Out), k(21, 0.82, In), k(28, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 10.0, Out), k(7, 2.0, Out), k(18, 0.0, Out), k(21, 16.0, In), k(32, 0.0, InOut)],
    reach: &[k(0, 0.5, Linear), k(4, 0.4, Out), k(16, 0.4, Linear), k(19, 1.0, Out), k(30, 1.0, Linear), k(38, 0.5, InOut)],
    reach_pitch: 40.0,
    // (over his right shoulder for the charge, up over his head, down in front of him)
    blade_yaw: &[k(0, -20.0, Linear), k(4, -150.0, Out), k(16, -150.0, Linear), k(19, 0.0, Out), k(36, -10.0, InOut)],
    blade_pitch: &[k(0, 30.0, Linear), k(4, 40.0, Out), k(16, 40.0, Linear), k(19, 165.0, Out), k(22, -45.0, In), k(30, -40.0, Linear), k(38, 20.0, InOut)],
    blade_held: (0, 42),
    weapon_scale: &[k(0, 1.0, Linear), k(19, 1.0, Linear), k(21, 1.35, In), k(30, 1.35, Linear), k(38, 1.0, InOut)],
    windows: &[
        w(8, 15, Shape::Front { reach: 170.0 }, Attack::HammerCharge, 0),
        w(21, 23, Shape::Around { radius: 230.0, ahead: 130.0 }, Attack::HammerChargeSlam, FX_QUAKE | FX_GROUND),
    ],
    // (committed, as the hammer's heavy blows are)
    cancels: &[c(ALL, 34, 28)],
    sounds: &[(0, UH, 0.5), (5, WAH, 0.0), (6, SWISH, 0.4), (17, HOOHOO, 0.0), (20, SWISH, 0.45), (22, HEAVY_LANDING, 0.4)],
    fx: &[(5, FX_LAUNCH), (22, FX_SLAM)],
    locked: (0, 26),
    ..BASE
};

/// X, a beat, X: three small hammers thrown overhand one after the other, each arcing onto an
/// enemy in front (volley.rs); the big one stays on his back (a Hammer Bro's)
pub const VOLLEY: Move = Move {
    name: "Hammer Bros. volley",
    len: 34,
    clips: &[still(0, FIRST_PUNCH), clip(2, THROW_LIGHT_OBJECT), still(9, FIRST_PUNCH), clip(10, THROW_LIGHT_OBJECT), still(17, FIRST_PUNCH), clip(18, THROW_LIGHT_OBJECT)],
    twist: &[k(0, 0.0, Linear), k(2, -25.0, Out), k(4, 15.0, Out), k(9, -25.0, Out), k(12, 15.0, Out), k(17, -25.0, Out), k(20, 15.0, Out), k(28, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(2, -8.0, Out), k(4, 10.0, Out), k(9, -8.0, Out), k(12, 10.0, Out), k(17, -8.0, Out), k(20, 10.0, Out), k(28, 0.0, InOut)],
    aim: &[0, 9, 17],
    squash: &[k(0, 1.0, Linear), k(2, 0.94, Out), k(4, 1.05, Out), k(9, 0.94, Out), k(12, 1.05, Out), k(17, 0.94, Out), k(20, 1.05, Out), k(26, 1.0, InOut)],
    reach: &[k(0, 0.3, Linear), k(2, 0.3, Linear), k(4, 1.0, Out), k(8, 0.3, InOut), k(10, 0.3, Linear), k(12, 1.0, Out), k(16, 0.3, InOut), k(18, 0.3, Linear), k(20, 1.0, Out), k(30, 0.0, InOut)],
    reach_pitch: 55.0,
    cancels: &[c(DASH | SWAP, 8, 8), c(ALL, 26, 22)],
    sounds: &[(2, SWISH, 0.3), (3, YAH, 0.0), (10, SWISH, 0.3), (18, SWISH, 0.3), (19, HOOHOO, 0.0)],
    fx: &[(4, FX_VOLLEY), (12, FX_VOLLEY), (20, FX_VOLLEY)],
    locked: (0, 4),
    ..BASE
};

/// forward + X in the air: a moment's hang with the hammer raised over his head, an angled dive at
/// the enemy, the hammer first, and a ring shaking the ground where he lands (a comet)
pub const COMET: Move = Move {
    name: "diving comet",
    len: 34,
    clips: &[still(0, FIRST_PUNCH), clip(5, GROUND_POUND), clip(12, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(4, -20.0, Out), k(8, 10.0, Out), k(16, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(4, -12.0, Out), k(8, 30.0, In), k(12, 20.0, Linear), k(20, 0.0, InOut)],
    aim: &[0, 4],
    approach: Some(Approach { from: 4, to: 12, max: 4.5, gap: 0.9 }),
    vel_y: &[(0, 6.0), (5, -55.0)],
    hold: (0, 5),
    squash: &[k(0, 1.0, Linear), k(4, 1.08, Out), k(8, 1.12, Linear), k(12, 0.8, Out), k(20, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(12, 14.0, Out), k(24, 0.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(26, 1.0, Linear), k(32, 0.5, InOut)],
    reach_pitch: 60.0,
    blade_yaw: &[k(0, 0.0, Linear)],
    blade_pitch: &[k(0, 60.0, Linear), k(4, 160.0, Out), k(8, -30.0, In), k(12, -45.0, Linear), k(26, -40.0, Linear), k(32, 20.0, InOut)],
    blade_held: (0, 34),
    weapon_scale: &[k(0, 1.0, Linear), k(4, 1.2, Out), k(12, 1.3, Linear), k(26, 1.3, Linear), k(32, 1.0, InOut)],
    windows: &[
        w(5, 9, Shape::Weapon, Attack::Comet, 0),
        wg(10, 33, Shape::Around { radius: 240.0, ahead: 70.0 }, Attack::CometRing, FX_QUAKE | FX_GROUND | FX_SLAM),
    ],
    cancels: &[c(ALL, 26, 20)],
    sounds: &[(0, SWISH, 0.25), (4, HOOHOO, 0.0), (5, SWISH, 0.45)],
    locked: (0, 12),
    air: true,
    ..BASE
};

/// X held and let go (charge.rs): the leap, a jump up and ahead (3 m) with the hammer swung up over
/// his head and growing, brought down where he lands: a heavy slam
pub const LEAP: Move = Move {
    name: "hammer leap",
    len: 40,
    clips: &[still(0, FIRST_PUNCH), clip(3, SINGLE_JUMP), still(14, SECOND_PUNCH), clip(26, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(3, -15.0, Out), k(14, -10.0, Linear), k(17, 10.0, In), k(28, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(2, -8.0, Out), k(5, -12.0, Out), k(14, -10.0, Linear), k(18, 26.0, In), k(30, 0.0, InOut)],
    aim: &[0, 3],
    approach: Some(Approach { from: 3, to: 20, max: 3.0, gap: 1.2 }),
    vel_y: &[(3, 40.0)],
    squash: &[k(0, 0.82, Linear), k(3, 1.14, Out), k(10, 1.0, InOut), k(19, 0.8, In), k(26, 1.0, InOut)],
    dip: &[k(0, 14.0, Linear), k(3, 0.0, Out), k(19, 18.0, In), k(30, 0.0, InOut)],
    reach: &[k(0, 0.5, Linear), k(3, 1.0, Out), k(30, 1.0, Linear), k(38, 0.5, InOut)],
    reach_pitch: 50.0,
    // (behind him as he crouches, up over his head in the jump, down in front as he lands)
    blade_yaw: &[k(0, -150.0, Linear), k(4, -10.0, Out), k(34, -10.0, Linear)],
    blade_pitch: &[k(0, 30.0, Linear), k(4, 120.0, Out), k(14, 150.0, Linear), k(19, -45.0, In), k(30, -40.0, Linear), k(38, 20.0, InOut)],
    blade_held: (0, 40),
    weapon_scale: &[k(0, 1.1, Linear), k(12, 1.9, Out), k(28, 1.9, Linear), k(36, 1.0, InOut)],
    windows: &[wg(14, 34, Shape::Around { radius: 260.0, ahead: 140.0 }, Attack::QuakeBig, FX_QUAKE | FX_GROUND)],
    cancels: &[c(ALL, 34, 30)],
    sounds: &[(0, UH, 0.5), (3, HOOHOO, 0.0), (4, SWISH, 0.42), (18, SWISH, 0.5)],
    fx: &[(3, FX_LAUNCH)],
    locked: (0, 30),
    ..BASE
};

/// ...let go full: higher (6 m ahead), the hammer three times its size, a slam that shakes the
/// ground (the Titan's, frost's crater)
pub const LEAP_FULL: Move = Move {
    name: "hammer leap (full)",
    len: 46,
    clips: &[still(0, FIRST_PUNCH), clip(3, SINGLE_JUMP), still(16, SECOND_PUNCH), clip(30, GENERAL_LAND)],
    approach: Some(Approach { from: 3, to: 24, max: 6.0, gap: 1.2 }),
    vel_y: &[(3, 52.0)],
    pitch: &[k(0, 0.0, Linear), k(2, -8.0, Out), k(5, -12.0, Out), k(16, -10.0, Linear), k(21, 28.0, In), k(34, 0.0, InOut)],
    squash: &[k(0, 0.8, Linear), k(3, 1.16, Out), k(10, 1.0, InOut), k(22, 0.78, In), k(30, 1.0, InOut)],
    dip: &[k(0, 16.0, Linear), k(3, 0.0, Out), k(22, 20.0, In), k(34, 0.0, InOut)],
    blade_pitch: &[k(0, 30.0, Linear), k(4, 120.0, Out), k(16, 155.0, Linear), k(22, -45.0, In), k(34, -40.0, Linear), k(44, 20.0, InOut)],
    blade_held: (0, 46),
    weapon_scale: &[k(0, 1.1, Linear), k(14, 2.8, Out), k(32, 2.8, Linear), k(42, 1.0, InOut)],
    windows: &[wg(16, 40, Shape::Around { radius: 330.0, ahead: 160.0 }, Attack::Titan, FX_QUAKE | FX_GROUND | FX_GIANT)],
    cancels: &[c(ALL, 40, 36)],
    sounds: &[(0, UH, 0.5), (3, YAH, 0.0), (4, SWISH, 0.45), (20, SWISH, 0.55), (23, HEAVY_LANDING, 0.45)],
    locked: (0, 34),
    ..LEAP
};

/// The guard broke (guard.rs, Part 12.6b): staggered a second, leaning back, the hammer drooping at
/// his side. No way out of it, no one to hit; the guard is shut two seconds in all.
pub const STUN: Move = Move {
    name: "guard broken",
    len: 30,
    clips: &[clip(0, SOFT_BACK_KB)],
    pitch: &[k(0, -14.0, Out), k(8, -20.0, InOut), k(22, -8.0, InOut), k(30, 0.0, InOut)],
    squash: &[k(0, 0.94, Out), k(10, 0.97, InOut), k(30, 1.0, InOut)],
    dip: &[k(0, 4.0, Out), k(30, 0.0, InOut)],
    reach: &[k(0, 0.25, Linear)],
    blade_yaw: &[k(0, -40.0, Linear)],
    blade_pitch: &[k(0, -60.0, Linear)],
    blade_held: (0, 30),
    sounds: &[(0, UH, 0.5)],
    locked: (0, 30),
    ..BASE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::*;

    fn hammer(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn the_hammers_moves_sit_at_their_ids() {
        assert_eq!(hammer(HAMMER_CHARGE).name, "hammer charge");
        assert_eq!(hammer(HAMMER_VOLLEY).name, "Hammer Bros. volley");
        assert_eq!(hammer(HAMMER_COMET).name, "diving comet");
        assert!(hammer(HAMMER_COMET).air && !hammer(HAMMER_CHARGE).air && !hammer(HAMMER_VOLLEY).air);
    }

    /// The broken guard stuns him as long as the guard says, with no way out and nothing hit.
    #[test]
    fn a_broken_guard_is_a_second_with_no_way_out() {
        let m = hammer(HAMMER_STUN);
        assert_eq!(m.len as u32, crate::guard::STUN_TICKS);
        assert!(m.cancels.is_empty() && m.windows.is_empty() && !m.air);
        assert_eq!(m.locked, (0, m.len));
        // (no immunity: the stun is the price)
        assert_eq!(m.invulnerable, (0, 0));
        assert!(m.pitch.iter().all(|k| k.v <= 0.0), "leaning back, never forward");
        assert!(crate::guard::LOCK_TICKS > crate::guard::STUN_TICKS);
    }

    #[test]
    fn the_volley_throws_three() {
        assert_eq!(hammer(HAMMER_VOLLEY).fx.iter().filter(|f| f.1 & FX_VOLLEY != 0).count(), 3);
        // (the big hammer stays on his back meanwhile)
        assert_eq!(hammer(HAMMER_VOLLEY).blade_held, (0, 0));
    }

    #[test]
    fn its_heavy_blows_stay_committed() {
        // (the hammer's role: no dash out of the charge's slam before it lands)
        let m = hammer(HAMMER_CHARGE);
        let slam = m.windows.iter().find(|w| w.attack == Attack::HammerChargeSlam).unwrap();
        assert!(m.locked.1 > slam.from && m.cancels.iter().all(|c| c.on_hit.min(c.from) > slam.to));
    }
}
