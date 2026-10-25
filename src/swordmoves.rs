//! The Star Sword's moves as data (v4 Part 2; Part 10: Dante's Rebellion, DMC3, the user's call,
//! and no spins): script.rs plays them (MOVES from id SW_S1 on), swordplay.rs picks them from the
//! buttons, charge.rs the Drive. Each move points the blade (yaw round his facing + left, pitch up
//! from level, roll round itself) and the hand follows it (arm IK); its cuts hit along the blade
//! itself (`Shape::Weapon`), its landings once (ground windows). Rebellion's feel: a wind-up two
//! or three times the cut (the weight is in it and in the pose held after), the cut itself two
//! ticks, the follow-through frozen a few ticks (longer on the finishers), a hard stop where a
//! lunge meets its enemy. Its swings' sounds are the mod's own (sfx.rs), not SM64's swish.

use crate::combat::Attack;
use crate::moves::{FX_BANG, FX_LAUNCH, FX_SLAM};
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, Shape, Window, ALL, BASE, DASH, VOICE_LIGHT, FX_BEAM, FX_CROSS, FX_SPARKLE, FX_WAVE, FX_WAVE3, FX_ZAP, JUMP, NEXT, SWAP};
use crate::sfx::{SWING, SWING_HEAVY, SWING_LIGHT};

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const FIRST_PUNCH_FAST: i32 = 0x69;
const SECOND_PUNCH_FAST: i32 = 0x6A;
const DOUBLE_JUMP_RISE: i32 = 0x50;
const GENERAL_LAND: i32 = 0x57;
const FAST_LONGJUMP: i32 = 0x13;
const DIVE: i32 = 0x88;
const SLOW_LAND_FROM_DIVE: i32 = 0x5A;

/// (SM64's: the voice at his body; the swings are sfx.rs's)
const UH: u32 = 0x2405_8081;
const YAH: u32 = 0x241E_8081;
const HOO: u32 = 0x241F_8081;
const WAH: u32 = 0x2424_8081;
const YAH_WAH_HOO: u32 = 0x2400_8081;
const GROUND_POUND_WAH: u32 = 0x2422_8081;
const HEAVY_LANDING: u32 = 0x0460_8081;
/// (bolt.rs's, not verified in game)
const THUNDER: u32 = 0x300C_8081;

/// out of a blow that landed into the dash, a jump, a swap
const DJS: u8 = DASH | JUMP | SWAP;

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

/// (a pose held: the punch's first frame, his body coiled by the curves)
const FIRST_FRAME: &[crate::script::Key] = &[k(0, 0.0, Linear)];
const fn still(from: u16, anim: i32) -> Clip {
    Clip { from, anim, frames: FIRST_FRAME }
}

// ---- the string: X, X, X, X ------------------------------------------------------------------

/// X: drawn up over his right shoulder from the ready pose (the wind-up, two ticks), then a
/// diagonal cleave down to his left in two, held low a moment (Rebellion's Combo A, 1)
pub const S1: Move = Move {
    name: "star slash",
    len: 14,
    clips: &[still(0, FIRST_PUNCH), clip(2, FIRST_PUNCH_FAST)],
    twist: &[k(0, 0.0, Linear), k(2, -35.0, InOut), k(4, 28.0, Out), k(7, 26.0, Linear), k(13, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(2, -5.0, InOut), k(4, 10.0, Out), k(7, 9.0, Linear), k(13, 0.0, InOut)],
    aim: &[0, 6],
    approach: Some(Approach { from: 1, to: 4, max: 1.6, gap: 1.0 }),
    squash: &[k(0, 1.0, Linear), k(2, 0.9, InOut), k(4, 1.08, Out), k(7, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(2, 1.0, InOut), k(4, 4.0, Out), k(7, 3.0, Linear), k(12, 0.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(2, 1.0, InOut), k(8, 1.0, Linear), k(13, 0.5, InOut)],
    // (from the ready pose, -25 / -35, to the draw)
    blade_yaw: &[k(0, -25.0, Linear), k(2, -85.0, InOut), k(3, -10.0, Linear), k(4, 60.0, Out), k(7, 58.0, Linear), k(13, 20.0, InOut)],
    blade_pitch: &[k(0, -35.0, Linear), k(2, 65.0, InOut), k(3, 20.0, Linear), k(4, -32.0, Out), k(7, -30.0, Linear), k(13, -20.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(2, -40.0, InOut), k(4, -34.0, Out), k(7, -34.0, Linear), k(13, 0.0, InOut)],
    blade_held: (0, 14),
    windows: &[w(2, 4, Shape::Weapon, Attack::SwordSlash(0), 0)],
    cancels: &[c(NEXT, 6, 4), c(DJS, 6, 4)],
    sounds: &[(0, VOICE_LIGHT, 0.25), (1, SWING_LIGHT, 0.4)],
    follow: (5, 12),
    locked: (0, 2),
    ..BASE
};

/// X, X: a dip with the blade low on his right, then a rising backhand up to his left, his back
/// half turned to the camera, held up there a moment (Combo A, 2); it starts where the slash was
pub const S2: Move = Move {
    name: "star backhand",
    len: 13,
    clips: &[still(0, SECOND_PUNCH), clip(2, SECOND_PUNCH_FAST)],
    twist: &[k(0, 27.0, Linear), k(2, 40.0, InOut), k(4, -35.0, Out), k(6, -32.0, Linear), k(12, 0.0, InOut)],
    pitch: &[k(0, 9.0, Linear), k(2, 4.0, InOut), k(4, -6.0, Out), k(6, -5.0, Linear), k(12, 0.0, InOut)],
    aim: &[0, 5],
    approach: Some(Approach { from: 1, to: 4, max: 1.2, gap: 1.0 }),
    squash: &[k(0, 1.06, Linear), k(2, 0.92, InOut), k(4, 1.07, Out), k(7, 1.0, InOut)],
    dip: &[k(0, 3.0, Linear), k(2, 4.0, InOut), k(4, 0.0, Out)],
    reach: &[k(0, 1.0, Linear), k(9, 1.0, Linear), k(12, 0.5, InOut)],
    blade_yaw: &[k(0, 59.0, Linear), k(2, 80.0, InOut), k(3, 10.0, Linear), k(4, -70.0, Out), k(6, -68.0, Linear), k(12, -35.0, InOut)],
    blade_pitch: &[k(0, -31.0, Linear), k(2, -40.0, InOut), k(3, 5.0, Linear), k(4, 45.0, Out), k(6, 44.0, Linear), k(12, -10.0, InOut)],
    blade_roll: &[k(0, -34.0, Linear), k(2, 30.0, InOut), k(12, 0.0, InOut)],
    blade_held: (0, 13),
    windows: &[w(2, 4, Shape::Weapon, Attack::SwordSlash(1), 0)],
    cancels: &[c(NEXT, 5, 4), c(DJS, 6, 4)],
    sounds: &[(0, VOICE_LIGHT, 0.25), (1, SWING_LIGHT, 0.42)],
    follow: (5, 11),
    locked: (0, 2),
    ..BASE
};

/// X, X, X: the blade low on his right, a rising diagonal up to his left (Combo A, 3); it starts
/// where the backhand was
pub const S3: Move = Move {
    name: "star rising slash",
    len: 14,
    clips: &[still(0, FIRST_PUNCH), clip(3, FIRST_PUNCH_FAST)],
    twist: &[k(0, -33.0, Linear), k(3, -38.0, InOut), k(5, 32.0, Out), k(8, 28.0, Linear), k(13, 0.0, InOut)],
    pitch: &[k(0, -5.0, Linear), k(3, 8.0, InOut), k(5, -9.0, Out), k(8, -8.0, Linear), k(13, 0.0, InOut)],
    aim: &[0, 6],
    approach: Some(Approach { from: 1, to: 5, max: 1.2, gap: 1.0 }),
    squash: &[k(0, 1.05, Linear), k(3, 0.88, InOut), k(5, 1.1, Out), k(9, 1.0, InOut)],
    dip: &[k(0, 3.0, Linear), k(3, 4.0, InOut), k(5, 0.0, Out)],
    reach: &[k(0, 0.95, Linear), k(3, 1.0, InOut), k(10, 1.0, Linear), k(13, 0.5, InOut)],
    blade_yaw: &[k(0, -68.0, Linear), k(3, -72.0, InOut), k(4, -10.0, Linear), k(5, 50.0, Out), k(8, 48.0, Linear), k(13, 25.0, InOut)],
    blade_pitch: &[k(0, 44.0, Linear), k(3, -48.0, InOut), k(4, 10.0, Linear), k(5, 72.0, Out), k(8, 70.0, Linear), k(13, 15.0, InOut)],
    blade_roll: &[k(0, 24.0, Linear), k(3, 40.0, InOut), k(13, 0.0, InOut)],
    blade_held: (0, 14),
    windows: &[w(3, 5, Shape::Weapon, Attack::SwordSlash(2), 0)],
    cancels: &[c(NEXT, 6, 4), c(DJS, 6, 4)],
    sounds: &[(1, SWING, 0.45), (4, HOO, 0.0)],
    follow: (6, 12),
    locked: (0, 3),
    ..BASE
};

/// X x4: Combo A's end, no spin: the blade cocked level behind his head in both hands, hunched
/// over it (held), then a two-handed chop from high on his right to low on his left with a deep
/// lunge, frozen there (it knocks them back; at full health a sword beam)
pub const S4: Move = Move {
    name: "Rebellion chop",
    len: 28,
    clips: &[still(0, SECOND_PUNCH), clip(4, FIRST_PUNCH), clip(18, GENERAL_LAND)],
    twist: &[k(0, 31.0, Linear), k(4, -45.0, InOut), k(6, 30.0, Out), k(16, 28.0, Linear), k(24, 0.0, InOut)],
    pitch: &[k(0, -9.0, Linear), k(4, -8.0, InOut), k(6, 26.0, Out), k(16, 24.0, Linear), k(24, 0.0, InOut)],
    aim: &[0, 2, 16],
    approach: Some(Approach { from: 3, to: 6, max: 1.8, gap: 1.0 }),
    squash: &[k(0, 1.08, Linear), k(4, 0.86, InOut), k(6, 1.14, Out), k(10, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 3.0, InOut), k(6, 4.0, Out), k(16, 4.0, Linear), k(24, 0.0, InOut)],
    reach: &[k(0, 1.0, Linear), k(4, 0.8, InOut), k(6, 1.0, Out), k(16, 1.0, Linear), k(24, 0.4, InOut)],
    // (behind his head, level, both hands at his right shoulder; down across him in two ticks)
    blade_yaw: &[k(0, 50.0, Linear), k(4, -120.0, InOut), k(5, -30.0, Linear), k(6, 55.0, Out), k(16, 52.0, Linear), k(25, 15.0, InOut)],
    blade_pitch: &[k(0, 72.0, Linear), k(4, 90.0, InOut), k(5, 30.0, Linear), k(6, -45.0, Out), k(16, -42.0, Linear), k(25, -25.0, InOut)],
    blade_roll: &[k(0, 36.0, Linear), k(4, -60.0, InOut), k(6, -20.0, Out), k(25, 0.0, InOut)],
    blade_held: (0, 28),
    windows: &[w(4, 6, Shape::Weapon, Attack::SwordSlash(3), FX_BANG)],
    cancels: &[c(DJS, 12, 9), c(NEXT, 16, 12)],
    sounds: &[(0, WAH, 0.0), (1, SWING_HEAVY, 0.55)],
    fx: &[(6, FX_BEAM)],
    follow: (7, 24),
    locked: (0, 6),
    ..BASE
};

/// X, a beat, X: both hands overhead and down, a crack of lightning where it lands; it shakes a
/// stance
pub const CLEAVE: Move = Move {
    name: "thunder cleave",
    len: 28,
    clips: &[still(0, FIRST_PUNCH), clip(8, SECOND_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(6, -15.0, Out), k(10, 10.0, Out), k(18, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(6, -10.0, Out), k(10, 18.0, Out), k(18, 12.0, Linear), k(24, 0.0, InOut)],
    aim: &[0, 7],
    approach: Some(Approach { from: 2, to: 10, max: 2.0, gap: 1.0 }),
    squash: &[k(0, 1.0, Linear), k(6, 1.06, Out), k(10, 0.86, In), k(15, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(6, 0.0, Linear), k(10, 4.0, Out), k(20, 0.0, InOut)],
    reach: &[k(0, 0.5, Linear), k(5, 1.0, Out), k(18, 1.0, Linear), k(26, 0.4, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(6, 0.0, Out), k(26, -20.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(6, 125.0, Out), k(8, 125.0, Linear), k(12, -55.0, Out), k(20, -50.0, Linear), k(26, -30.0, InOut)],
    blade_held: (0, 28),
    windows: &[w(9, 12, Shape::Weapon, Attack::SwordCleave, 0)],
    cancels: &[c(DJS, 21, 16), c(NEXT, 23, 18)],
    sounds: &[(8, GROUND_POUND_WAH, 0.0), (9, SWING_HEAVY, 0.5), (12, HEAVY_LANDING, 0.3)],
    fx: &[(12, FX_ZAP)],
    locked: (0, 12),
    ..BASE
};

// ---- closers and launchers -------------------------------------------------------------------

/// back, forward + X: DMC's Stinger: a deep coil (the point low at his hip), then he shoots at the
/// enemy point first, almost flat (up to 8 m), and stops dead on it, the thrust held out; X mashed
/// as it lands: the Million Stab
pub const THRUST: Move = Move {
    name: "Stinger",
    len: 30,
    clips: &[
        Clip { from: 0, anim: FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(6, 1.0, In)] },
        Clip { from: 6, anim: DIVE, frames: &[k(6, 0.0, Linear), k(10, 5.0, Out), k(14, 6.0, Linear)] },
        Clip { from: 15, anim: SLOW_LAND_FROM_DIVE, frames: &[k(15, 0.0, Linear), k(29, 14.0, Out)] },
    ],
    twist: &[k(0, 0.0, Linear), k(6, -35.0, InOut), k(8, 0.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(6, -8.0, InOut), k(9, 6.0, Out), k(15, 4.0, Linear), k(20, 0.0, InOut)],
    aim: &[0, 6],
    speed: &[k(0, 0.0, Linear), k(14, 0.0, Linear), k(15, 2.0, Linear), k(19, 0.0, Out)],
    approach: Some(Approach { from: 6, to: 14, max: 8.0, gap: 0.8 }),
    vel_y: &[(6, 12.0)],
    squash: &[k(0, 1.0, Linear), k(6, 0.84, InOut), k(8, 1.14, Out), k(13, 1.0, InOut), k(15, 0.88, Out), k(20, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(6, 4.0, InOut), k(8, 0.0, Out), k(15, 2.7, Out), k(22, 0.0, InOut)],
    // (the hand pulled in for the coil, then out with the point, held out past the stop)
    reach: &[k(0, 0.3, Linear), k(6, 0.15, InOut), k(9, 1.0, Out), k(22, 1.0, Linear), k(28, 0.4, InOut)],
    blade_yaw: &[k(0, -15.0, Linear), k(6, -25.0, InOut), k(9, 0.0, Out), k(22, 0.0, Linear), k(28, -15.0, InOut)],
    blade_pitch: &[k(0, -15.0, Linear), k(6, -12.0, InOut), k(9, -4.0, Out), k(22, -4.0, Linear), k(28, -25.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear)],
    blade_held: (0, 30),
    windows: &[w(8, 15, Shape::Front { reach: 200.0 }, Attack::SwordThrust, 0)],
    cancels: &[c(ALL, 22, 15)],
    sounds: &[(0, UH, 0.0), (6, YAH, 0.0), (7, SWING, 0.5)],
    fx: &[(6, FX_LAUNCH)],
    locked: (0, 7),
    ..BASE
};

/// X mashed on the Stinger, or X, X, a beat, X (Combo B): a flash, then nine stabs in 1.2 s rooted in
/// a wide stance, a last full-length lunge that blows them back (DMC's Million Stab)
pub const STABS: Move = Move {
    name: "Million Stab",
    len: 46,
    clips: &[
        clip(0, FIRST_PUNCH_FAST),
        clip(4, SECOND_PUNCH_FAST),
        clip(8, FIRST_PUNCH_FAST),
        clip(12, SECOND_PUNCH_FAST),
        clip(16, FIRST_PUNCH_FAST),
        clip(20, SECOND_PUNCH_FAST),
        clip(24, FIRST_PUNCH_FAST),
        clip(28, SECOND_PUNCH_FAST),
        clip(32, FIRST_PUNCH_FAST),
        clip(38, FIRST_PUNCH),
    ],
    twist: &[k(0, 0.0, Linear), k(36, 0.0, Linear), k(38, -25.0, Out), k(40, 10.0, Out), k(45, 0.0, InOut)],
    pitch: &[k(0, 4.0, Linear), k(36, 4.0, Linear), k(38, -4.0, Out), k(40, 10.0, Out), k(45, 0.0, InOut)],
    aim: &[0, 38],
    approach: Some(Approach { from: 0, to: 6, max: 0.8, gap: 0.9 }),
    // (each stab: the arm out, back in)
    reach: &[
        k(0, 0.5, Linear),
        k(2, 1.0, Out),
        k(4, 0.5, In),
        k(6, 1.0, Out),
        k(8, 0.5, In),
        k(10, 1.0, Out),
        k(12, 0.5, In),
        k(14, 1.0, Out),
        k(16, 0.5, In),
        k(18, 1.0, Out),
        k(20, 0.5, In),
        k(22, 1.0, Out),
        k(24, 0.5, In),
        k(26, 1.0, Out),
        k(28, 0.5, In),
        k(30, 1.0, Out),
        k(32, 0.5, In),
        k(34, 1.0, Out),
        k(38, 0.3, In),
        k(40, 1.0, Out),
        k(45, 0.5, InOut),
    ],
    blade_yaw: &[
        k(0, 0.0, Linear),
        k(2, -10.0, Out),
        k(6, 8.0, Out),
        k(10, -6.0, Out),
        k(14, 12.0, Out),
        k(18, -12.0, Out),
        k(22, 6.0, Out),
        k(26, -8.0, Out),
        k(30, 10.0, Out),
        k(34, -4.0, Out),
        k(40, 0.0, Out),
        k(45, -20.0, InOut),
    ],
    blade_pitch: &[
        k(0, -5.0, Linear),
        k(2, 0.0, Out),
        k(6, -8.0, Out),
        k(10, 4.0, Out),
        k(14, -6.0, Out),
        k(18, 2.0, Out),
        k(22, -10.0, Out),
        k(26, 5.0, Out),
        k(30, -3.0, Out),
        k(34, 0.0, Out),
        k(38, -5.0, Out),
        k(40, -2.0, Out),
        k(45, -25.0, InOut),
    ],
    blade_roll: &[k(0, 90.0, Linear)],
    blade_held: (0, 46),
    windows: &[
        w(2, 2, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(6, 6, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(10, 10, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(14, 14, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(18, 18, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(22, 22, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(26, 26, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(30, 30, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(34, 34, Shape::Front { reach: 190.0 }, Attack::SwordStab, 0),
        w(39, 41, Shape::Front { reach: 210.0 }, Attack::SwordStabEnd, 0),
    ],
    cancels: &[c(DASH | SWAP, 12, 12), c(ALL, 43, 42)],
    sounds: &[(0, WAH, 0.0), (2, SWING_LIGHT, 0.22), (10, SWING_LIGHT, 0.2), (18, SWING_LIGHT, 0.2), (26, SWING_LIGHT, 0.2), (34, SWING_LIGHT, 0.2), (38, YAH, 0.0), (39, SWING_HEAVY, 0.5)],
    fx: &[(0, FX_SPARKLE), (40, FX_BEAM)],
    locked: (0, 2),
    ..BASE
};

/// back + X: DMC's High Time: a deep coil, the point near the floor, then a rising cut from the
/// ground straight up over his head that launches small and medium enemies; X still held, he goes
/// up with them
pub const RISE: Move = Move {
    name: "High Time",
    len: 28,
    clips: &[still(0, FIRST_PUNCH), clip(6, FIRST_PUNCH_FAST), clip(11, DOUBLE_JUMP_RISE)],
    twist: &[k(0, 0.0, Linear), k(5, -22.0, InOut), k(8, 12.0, Out), k(18, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(5, 16.0, InOut), k(8, -14.0, Out), k(13, -12.0, Linear), k(18, 0.0, InOut)],
    aim: &[0],
    squash: &[k(0, 1.0, Linear), k(5, 0.82, InOut), k(8, 1.16, Out), k(14, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(5, 4.0, InOut), k(8, 0.0, Out)],
    reach: &[k(0, 0.5, Linear), k(5, 0.8, InOut), k(8, 1.0, Out), k(18, 1.0, Linear), k(24, 0.5, InOut)],
    blade_yaw: &[k(0, -25.0, Linear), k(5, -45.0, InOut), k(8, 10.0, Out), k(18, 0.0, Linear), k(24, -20.0, InOut)],
    blade_pitch: &[k(0, -35.0, Linear), k(5, -65.0, InOut), k(6, -10.0, Linear), k(8, 88.0, Out), k(16, 86.0, Linear), k(24, -10.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(5, 90.0, InOut), k(18, 90.0, Linear), k(24, 0.0, InOut)],
    blade_held: (0, 28),
    windows: &[w(6, 8, Shape::Weapon, Attack::SwordRise, 0)],
    // (risen with them: the air string's X from 16; held at the top a moment for it)
    held_rise: Some((10, 52.0)),
    hold: (22, 27),
    cancels: &[c(NEXT, 16, 12), c(DJS, 18, 12)],
    sounds: &[(5, UH, 0.0), (6, SWING, 0.48), (7, HOO, 0.0)],
    fx: &[(6, FX_LAUNCH)],
    locked: (0, 7),
    ..BASE
};

/// Y: High Time on its own button (DMC's), the same rising cut; Y still held, he goes up with them
pub const HIGH_TIME: Move = Move { name: "High Time (Y)", rise_on_y: true, ..RISE };

// ---- X held: the Drive (charge.rs) ---------------------------------------------------------

/// X held and let go: the blade cocked high behind his right shoulder (as he charged) swung down and
/// forward in two ticks into a low lunge, a wave of lightning sent along the ground, 10 m (wave.rs;
/// DMC's Drive)
pub const DRIVE: Move = Move {
    name: "Drive",
    len: 22,
    clips: &[clip(0, FIRST_PUNCH_FAST), clip(12, GENERAL_LAND)],
    twist: &[k(0, -30.0, Linear), k(2, 30.0, Out), k(10, 28.0, Linear), k(16, 0.0, InOut)],
    pitch: &[k(0, -6.0, Linear), k(2, 20.0, Out), k(10, 18.0, Linear), k(16, 0.0, InOut)],
    approach: Some(Approach { from: 0, to: 3, max: 1.0, gap: 1.0 }),
    aim: &[0],
    squash: &[k(0, 0.92, Linear), k(2, 1.1, Out), k(9, 1.0, InOut)],
    dip: &[k(0, 1.1, Linear), k(2, 4.0, Out), k(10, 3.7, Linear), k(17, 0.0, InOut)],
    reach: &[k(0, 0.9, Linear), k(2, 1.0, Out), k(12, 1.0, Linear), k(20, 0.5, InOut)],
    blade_yaw: &[k(0, -150.0, Linear), k(1, -50.0, Linear), k(2, 35.0, Out), k(10, 32.0, Linear), k(20, -10.0, InOut)],
    blade_pitch: &[k(0, 110.0, Linear), k(1, 40.0, Linear), k(2, -40.0, Out), k(10, -38.0, Linear), k(20, -25.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(10, 90.0, Linear), k(18, 0.0, InOut)],
    blade_held: (0, 22),
    windows: &[w(1, 3, Shape::Weapon, Attack::SwordWave, 0)],
    cancels: &[c(DASH | SWAP, 6, 6), c(ALL, 12, 10)],
    sounds: &[(0, YAH, 0.0), (1, SWING_HEAVY, 0.5)],
    fx: &[(2, FX_WAVE)],
    locked: (0, 2),
    ..BASE
};

/// ...full: the same with all of him in it, the wave and a crack of lightning where the blade hits
/// the ground
pub const DRIVE_FULL: Move = Move {
    name: "Drive (full)",
    len: 26,
    windows: &[w(1, 3, Shape::Weapon, Attack::SwordSlash(3), FX_BANG)],
    dip: &[k(0, 1.3, Linear), k(2, 4.0, Out), k(12, 3.6, Linear), k(20, 0.0, InOut)],
    squash: &[k(0, 0.88, Linear), k(2, 1.14, Out), k(10, 1.0, InOut)],
    cancels: &[c(DASH | SWAP, 8, 8), c(ALL, 14, 12)],
    sounds: &[(0, WAH, 0.0), (1, SWING_HEAVY, 0.55)],
    fx: &[(2, FX_WAVE | FX_ZAP)],
    ..DRIVE
};

/// ...let go right as it filled (JUST): three Drives, three waves (DMC5's Quick Drive, Vergil's
/// cuts)
pub const DRIVE_JUST: Move = Move {
    name: "Drive JUST",
    len: 26,
    clips: &[clip(0, FIRST_PUNCH_FAST), clip(5, SECOND_PUNCH_FAST), clip(10, FIRST_PUNCH)],
    twist: &[k(0, -30.0, Linear), k(2, 30.0, Out), k(6, -28.0, Out), k(10, 0.0, Out), k(12, 25.0, Out), k(20, 0.0, InOut)],
    pitch: &[k(0, 6.0, Linear), k(10, -6.0, Out), k(12, 18.0, Out), k(20, 0.0, InOut)],
    dip: &[k(0, 2.9, Linear), k(2, 1.7, Out), k(10, 0.0, Out), k(12, 4.0, Out), k(20, 0.0, InOut)],
    reach: &[k(0, 0.8, Linear), k(2, 1.0, Out), k(16, 1.0, Linear), k(24, 0.5, InOut)],
    blade_yaw: &[k(0, -150.0, Linear), k(1, -50.0, Linear), k(2, 40.0, Out), k(5, -10.0, InOut), k(6, -60.0, Out), k(9, -20.0, InOut), k(10, 0.0, Out), k(14, 0.0, Linear), k(22, -20.0, InOut)],
    blade_pitch: &[k(0, 110.0, Linear), k(1, 40.0, Linear), k(2, -30.0, Out), k(5, 30.0, InOut), k(6, -25.0, Out), k(9, 70.0, InOut), k(10, 115.0, Out), k(11, 40.0, Linear), k(12, -40.0, Out), k(22, -30.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(14, 0.0, InOut)],
    blade_held: (0, 26),
    windows: &[w(1, 3, Shape::Weapon, Attack::SwordWaveJust, 0)],
    cancels: &[c(ALL, 18, 18)],
    sounds: &[(0, WAH, 0.0), (1, SWING, 0.45), (6, SWING, 0.42), (11, SWING_HEAVY, 0.5)],
    fx: &[(2, FX_WAVE3)],
    locked: (0, 13),
    ..BASE
};

/// X, Y: two crossing cuts, their X left glowing in the air to burst 0.4 s later (wave.rs)
pub const CROSS: Move = Move {
    name: "cross cut",
    len: 20,
    clips: &[clip(0, FIRST_PUNCH_FAST), clip(5, SECOND_PUNCH_FAST)],
    twist: &[k(0, 0.0, Linear), k(1, -25.0, Out), k(4, 25.0, Out), k(6, 25.0, Linear), k(9, -25.0, Out), k(16, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 0, to: 4, max: 1.2, gap: 1.1 }),
    reach: &[k(0, 0.7, Linear), k(1, 1.0, Out), k(12, 1.0, Linear), k(18, 0.5, InOut)],
    blade_yaw: &[k(0, -40.0, Linear), k(1, -70.0, Out), k(4, 60.0, Out), k(6, 70.0, Out), k(9, -60.0, Out), k(16, -25.0, InOut)],
    blade_pitch: &[k(0, -10.0, Linear), k(1, 55.0, Out), k(4, -40.0, Out), k(6, 55.0, Out), k(9, -40.0, Out), k(16, -30.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(1, -45.0, Out), k(4, -45.0, Linear), k(6, 45.0, Out), k(9, 45.0, Linear), k(16, 0.0, InOut)],
    blade_held: (0, 20),
    windows: &[w(2, 9, Shape::Weapon, Attack::SwordCross, 0)],
    cancels: &[c(ALL, 14, 11)],
    sounds: &[(1, SWING, 0.42), (5, SWING, 0.42), (9, HOO, 0.0)],
    fx: &[(9, FX_CROSS)],
    locked: (0, 9),
    ..BASE
};

/// X, X, Y (and X + A): the rising cut, Mario going up with them every time (the other weapons'
/// X, X, Y)
pub const LAUNCHER: Move = Move {
    name: "star launcher",
    len: 26,
    clips: &[still(0, FIRST_PUNCH), clip(4, FIRST_PUNCH_FAST), clip(8, DOUBLE_JUMP_RISE)],
    twist: &[k(0, 0.0, Linear), k(3, -20.0, Out), k(7, 15.0, Out), k(16, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, 10.0, Out), k(7, -12.0, Out), k(16, 0.0, InOut)],
    aim: &[0],
    vel_y: &[(7, 50.0)],
    hold: (19, 25),
    squash: &[k(0, 1.0, Linear), k(3, 0.85, Out), k(7, 1.14, Out), k(12, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(3, 4.0, Out), k(7, 0.0, Out)],
    reach: &[k(0, 0.5, Linear), k(3, 0.8, Out), k(7, 1.0, Out), k(18, 1.0, Linear), k(24, 0.5, InOut)],
    blade_yaw: &[k(0, -25.0, Linear), k(3, -50.0, Out), k(7, 10.0, Out), k(16, 0.0, Linear), k(24, -20.0, InOut)],
    blade_pitch: &[k(0, -35.0, Linear), k(3, -55.0, Out), k(7, 85.0, Out), k(14, 80.0, Linear), k(24, -10.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(3, 90.0, Out), k(16, 90.0, Linear), k(24, 0.0, InOut)],
    blade_held: (0, 26),
    windows: &[w(4, 7, Shape::Weapon, Attack::SwordLauncher, 0)],
    cancels: &[c(NEXT | DASH | SWAP, 12, 10)],
    sounds: &[(3, SWING, 0.45), (5, YAH, 0.0)],
    fx: &[(4, FX_LAUNCH), (7, FX_LAUNCH)],
    locked: (0, 7),
    ..BASE
};

/// forward + Y: a leap onto the enemy, the blade raised, a plunging cleave and a ring of
/// lightning where he lands (Twilight Princess's Helm Splitter, a jump attack)
pub const HELM: Move = Move {
    name: "Helm Splitter",
    len: 36,
    clips: &[still(0, FIRST_PUNCH), clip(4, DOUBLE_JUMP_RISE), clip(15, SECOND_PUNCH), clip(20, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(12, -10.0, InOut), k(16, 10.0, Out), k(26, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, 8.0, Out), k(6, -6.0, Out), k(12, -12.0, Out), k(16, 20.0, Out), k(26, 10.0, Linear), k(32, 0.0, InOut)],
    aim: &[0, 4, 12],
    approach: Some(Approach { from: 4, to: 14, max: 7.0, gap: 0.7 }),
    vel_y: &[(4, 38.0), (15, -55.0)],
    hold: (12, 15),
    squash: &[k(0, 1.0, Linear), k(3, 0.86, Out), k(6, 1.1, Out), k(12, 1.04, Linear), k(16, 1.12, Out), k(20, 0.88, Out), k(26, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(3, 4.0, Out), k(6, 0.0, Out), k(18, 0.0, Linear), k(20, 4.0, Out), k(28, 0.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(30, 1.0, Linear), k(35, 0.5, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(4, 0.0, Out), k(30, 0.0, Linear), k(34, -20.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(4, 95.0, Out), k(12, 130.0, InOut), k(14, 130.0, Linear), k(17, -55.0, Out), k(26, -50.0, Linear), k(34, -30.0, InOut)],
    blade_held: (0, 36),
    windows: &[
        w(15, 17, Shape::Weapon, Attack::SwordHelm, 0),
        wg(18, 32, Shape::Around { radius: 210.0, ahead: 40.0 }, Attack::SwordHelmRing, FX_SLAM | FX_ZAP),
    ],
    cancels: &[c(ALL, 30, 24)],
    sounds: &[(4, YAH_WAH_HOO, 0.0), (15, SWING_HEAVY, 0.55), (16, GROUND_POUND_WAH, 0.0)],
    locked: (0, 17),
    ..BASE
};

/// X out of the dash: through the enemy and past it with one cut, the cut's line cracking with
/// lightning a beat later behind him (Vergil's Rapid Slash, an iai)
pub const IAI: Move = Move {
    name: "lightning iai",
    len: 24,
    clips: &[still(0, FIRST_PUNCH), clip(2, FAST_LONGJUMP), clip(9, GENERAL_LAND)],
    twist: &[k(0, -20.0, Linear), k(3, 30.0, Out), k(9, 20.0, Linear), k(16, 0.0, InOut)],
    pitch: &[k(0, 6.0, Linear), k(2, 14.0, Out), k(8, 10.0, Linear), k(10, -4.0, Out), k(16, 0.0, InOut)],
    aim: &[0],
    speed: &[k(0, 0.0, Linear), k(8, 0.0, Linear), k(9, 3.0, Linear), k(14, 0.0, Out)],
    // (through: past its body this far)
    approach: Some(Approach { from: 2, to: 8, max: 6.0, gap: -1.6 }),
    invulnerable: (2, 9),
    squash: &[k(0, 0.92, Linear), k(2, 1.1, Out), k(8, 1.05, Linear), k(10, 0.9, Out), k(16, 1.0, InOut)],
    dip: &[k(0, 4.0, Linear), k(2, 2.4, Out), k(9, 4.0, Out), k(16, 0.0, InOut)],
    reach: &[k(0, 0.7, Linear), k(4, 1.0, Out), k(16, 1.0, Linear), k(22, 0.5, InOut)],
    // (out, then back down behind him, as if sheathed: the crack comes then)
    blade_yaw: &[k(0, -150.0, Linear), k(4, 60.0, Out), k(9, 60.0, Linear), k(12, -160.0, InOut), k(22, -30.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(4, 5.0, Out), k(9, 0.0, Linear), k(12, -25.0, InOut), k(22, -30.0, Linear)],
    blade_roll: &[k(0, 90.0, Linear), k(9, 90.0, Linear), k(14, 0.0, InOut)],
    blade_held: (0, 24),
    windows: &[w(11, 12, Shape::Around { radius: 260.0, ahead: -120.0 }, Attack::SwordIai, 0)],
    cancels: &[c(ALL, 16, 13)],
    sounds: &[(3, SWING, 0.5), (11, THUNDER, 0.25)],
    fx: &[(11, FX_ZAP)],
    locked: (0, 9),
    ..BASE
};

/// X in a perfect dodge's slow time: in at the enemy, six cuts, a last wide one (Breath of the
/// Wild's Flurry Rush); no blow lands on him meanwhile
pub const FLURRY: Move = Move {
    name: "Flurry Rush",
    len: 40,
    clips: &[
        clip(0, FAST_LONGJUMP),
        clip(5, FIRST_PUNCH_FAST),
        clip(9, SECOND_PUNCH_FAST),
        clip(13, FIRST_PUNCH_FAST),
        clip(17, SECOND_PUNCH_FAST),
        clip(21, FIRST_PUNCH_FAST),
        clip(25, SECOND_PUNCH_FAST),
        clip(29, FIRST_PUNCH),
    ],
    twist: &[
        k(0, 0.0, Linear),
        k(4, -20.0, Out),
        k(6, 20.0, Out),
        k(8, -20.0, Out),
        k(10, 20.0, Out),
        k(12, -20.0, Out),
        k(14, 20.0, Out),
        k(16, -20.0, Out),
        k(18, 20.0, Out),
        k(20, -20.0, Out),
        k(22, 20.0, Out),
        k(24, -20.0, Out),
        k(26, 20.0, Out),
        k(29, -35.0, Out),
        k(32, 30.0, Out),
        k(38, 0.0, InOut),
    ],
    aim: &[0, 5, 29],
    approach: Some(Approach { from: 0, to: 5, max: 9.0, gap: 0.9 }),
    invulnerable: (0, 34),
    reach: &[k(0, 1.0, Linear), k(36, 1.0, Linear), k(39, 0.5, InOut)],
    blade_yaw: &[
        k(0, -30.0, Linear),
        k(4, -70.0, Out),
        k(6, 60.0, Out),
        k(8, 70.0, Out),
        k(10, -60.0, Out),
        k(12, -70.0, Out),
        k(14, 60.0, Out),
        k(16, 70.0, Out),
        k(18, -60.0, Out),
        k(20, -70.0, Out),
        k(22, 60.0, Out),
        k(24, 70.0, Out),
        k(26, -60.0, Out),
        k(29, -120.0, Out),
        k(32, 70.0, Out),
        k(38, -20.0, InOut),
    ],
    blade_pitch: &[
        k(0, -20.0, Linear),
        k(4, 40.0, Out),
        k(6, -20.0, Out),
        k(8, 40.0, Out),
        k(10, -20.0, Out),
        k(12, 40.0, Out),
        k(14, -20.0, Out),
        k(16, 40.0, Out),
        k(18, -20.0, Out),
        k(20, 40.0, Out),
        k(22, -20.0, Out),
        k(24, 40.0, Out),
        k(26, -20.0, Out),
        k(29, 10.0, Out),
        k(32, 0.0, Out),
        k(38, -30.0, InOut),
    ],
    blade_roll: &[k(0, 0.0, Linear), k(29, 0.0, Linear), k(31, 90.0, Out), k(36, 90.0, Linear), k(39, 0.0, InOut)],
    blade_held: (0, 40),
    windows: &[
        w(6, 6, Shape::Front { reach: 190.0 }, Attack::SwordFlurry, 0),
        w(10, 10, Shape::Front { reach: 190.0 }, Attack::SwordFlurry, 0),
        w(14, 14, Shape::Front { reach: 190.0 }, Attack::SwordFlurry, 0),
        w(18, 18, Shape::Front { reach: 190.0 }, Attack::SwordFlurry, 0),
        w(22, 22, Shape::Front { reach: 190.0 }, Attack::SwordFlurry, 0),
        w(26, 26, Shape::Front { reach: 190.0 }, Attack::SwordFlurry, 0),
        w(31, 33, Shape::Arc { radius: 210.0, half_deg: 75.0, ahead: 0.0 }, Attack::SwordFlurryEnd, 0),
    ],
    cancels: &[c(ALL, 36, 35)],
    sounds: &[(0, SWING_LIGHT, 0.3), (6, SWING_LIGHT, 0.3), (14, SWING_LIGHT, 0.3), (22, SWING_LIGHT, 0.3), (29, YAH, 0.0), (31, SWING_HEAVY, 0.5)],
    fx: &[(32, FX_BEAM)],
    locked: (0, 29),
    ..BASE
};

// ---- in the air ------------------------------------------------------------------------------

/// X x4 in the air: three cuts that hold him up there, a turn to end that throws them off (DMC's
/// Aerial Rave)
pub const RAVE: [Move; 4] = [
    Move {
        name: "Aerial Rave",
        len: 13,
        clips: &[clip(0, FIRST_PUNCH_FAST)],
        twist: &[k(0, 0.0, Linear), k(2, -30.0, Out), k(5, 25.0, Out), k(12, 0.0, InOut)],
        aim: &[0],
        approach: Some(Approach { from: 0, to: 4, max: 1.2, gap: 1.0 }),
        vel_y: &[(0, 0.0)],
        hold: (0, 11),
        reach: &[k(0, 0.7, Linear), k(2, 1.0, Out), k(9, 1.0, Linear), k(12, 0.6, InOut)],
        blade_yaw: &[k(0, -30.0, Linear), k(2, -80.0, Out), k(5, 60.0, Out), k(12, 20.0, InOut)],
        blade_pitch: &[k(0, -10.0, Linear), k(2, 55.0, Out), k(5, -30.0, Out)],
        blade_roll: &[k(0, 0.0, Linear), k(2, -40.0, Out), k(12, 0.0, InOut)],
        blade_held: (0, 13),
        windows: &[w(2, 5, Shape::Weapon, Attack::SwordRave(0), 0)],
        cancels: &[c(NEXT, 6, 5), c(DASH | SWAP, 7, 5)],
        sounds: &[(2, SWING_LIGHT, 0.4)],
        air: true,
        land_ends: true,
        ..BASE
    },
    Move {
        name: "Aerial Rave 2",
        len: 13,
        clips: &[clip(0, SECOND_PUNCH_FAST)],
        twist: &[k(0, 10.0, Linear), k(2, 35.0, Out), k(5, -30.0, Out), k(12, 0.0, InOut)],
        aim: &[0],
        approach: Some(Approach { from: 0, to: 4, max: 1.0, gap: 1.0 }),
        vel_y: &[(0, 0.0)],
        hold: (0, 11),
        reach: &[k(0, 0.8, Linear), k(2, 1.0, Out), k(9, 1.0, Linear), k(12, 0.6, InOut)],
        blade_yaw: &[k(0, 40.0, Linear), k(2, 85.0, Out), k(5, -70.0, Out), k(12, -35.0, InOut)],
        blade_pitch: &[k(0, -10.0, Linear), k(2, 15.0, Out), k(5, 0.0, Out), k(12, -25.0, InOut)],
        blade_roll: &[k(0, 0.0, Linear), k(2, 30.0, Out), k(12, 0.0, InOut)],
        blade_held: (0, 13),
        windows: &[w(2, 5, Shape::Weapon, Attack::SwordRave(1), 0)],
        cancels: &[c(NEXT, 6, 5), c(DASH | SWAP, 7, 5)],
        sounds: &[(2, SWING_LIGHT, 0.4)],
        air: true,
        land_ends: true,
        ..BASE
    },
    Move {
        name: "Aerial Rave 3",
        len: 14,
        clips: &[clip(0, FIRST_PUNCH_FAST)],
        twist: &[k(0, -5.0, Linear), k(2, -35.0, Out), k(6, 30.0, Out), k(13, 0.0, InOut)],
        aim: &[0],
        approach: Some(Approach { from: 0, to: 4, max: 1.0, gap: 1.0 }),
        vel_y: &[(0, 0.0)],
        hold: (0, 12),
        reach: &[k(0, 0.8, Linear), k(2, 1.0, Out), k(10, 1.0, Linear), k(13, 0.6, InOut)],
        blade_yaw: &[k(0, -40.0, Linear), k(2, -70.0, Out), k(6, 50.0, Out), k(13, 25.0, InOut)],
        blade_pitch: &[k(0, -25.0, Linear), k(2, -45.0, Out), k(6, 70.0, Out), k(13, 15.0, InOut)],
        blade_roll: &[k(0, 0.0, Linear), k(2, 40.0, Out), k(13, 0.0, InOut)],
        blade_held: (0, 14),
        windows: &[w(2, 6, Shape::Weapon, Attack::SwordRave(2), 0)],
        cancels: &[c(NEXT, 7, 6), c(DASH | SWAP, 8, 6)],
        sounds: &[(2, SWING, 0.42), (4, HOO, 0.0)],
        air: true,
        land_ends: true,
        ..BASE
    },
    // (the last cut no spin: the blade raised high, down through them in two ticks, and he drops
    // with it, near the ground: Rebellion's)
    Move {
        name: "Aerial Rave finisher",
        len: 18,
        clips: &[still(0, SECOND_PUNCH), clip(4, FIRST_PUNCH)],
        twist: &[k(0, 0.0, Linear), k(4, -25.0, InOut), k(6, 20.0, Out), k(12, 0.0, InOut)],
        pitch: &[k(0, 0.0, Linear), k(4, -10.0, InOut), k(6, 24.0, Out), k(12, 10.0, Linear), k(17, 0.0, InOut)],
        aim: &[0],
        vel_y: &[(0, 0.0), (6, -32.0)],
        hold: (0, 6),
        squash: &[k(0, 1.0, Linear), k(4, 1.08, InOut), k(6, 0.9, Out), k(12, 1.0, InOut)],
        reach: &[k(0, 0.8, Linear), k(4, 1.0, InOut), k(12, 1.0, Linear), k(17, 0.5, InOut)],
        blade_yaw: &[k(0, -20.0, Linear), k(4, -10.0, InOut), k(6, 15.0, Out), k(16, 0.0, InOut)],
        blade_pitch: &[k(0, 20.0, Linear), k(4, 125.0, InOut), k(5, 40.0, Linear), k(6, -55.0, Out), k(12, -52.0, Linear), k(16, -30.0, InOut)],
        blade_held: (0, 18),
        windows: &[w(4, 7, Shape::Weapon, Attack::SwordRave(3), FX_BANG)],
        // (the air string ends here: he drops, the dash and a swap still open)
        cancels: &[c(DASH | SWAP, 12, 9)],
        sounds: &[(2, WAH, 0.0), (4, SWING_HEAVY, 0.5)],
        fx: &[(7, FX_BEAM)],
        air: true,
        land_ends: true,
        ..BASE
    },
];

/// Y in the air: a moment's hang with the blade raised, then a fast plunging cut and a ring of
/// lightning where he lands (DMC's Helm Breaker)
pub const BREAKER: Move = Move {
    name: "Helm Breaker",
    len: 30,
    clips: &[still(0, FIRST_PUNCH), clip(4, SECOND_PUNCH), clip(9, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(3, -15.0, Out), k(6, 10.0, Out), k(14, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, -10.0, Out), k(6, 22.0, Out), k(14, 10.0, Linear), k(22, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 0, to: 4, max: 1.5, gap: 0.8 }),
    vel_y: &[(0, 0.0), (4, -70.0)],
    hold: (0, 4),
    squash: &[k(0, 1.0, Linear), k(3, 1.08, Out), k(7, 1.1, Linear), k(9, 0.82, Out), k(19, 0.86, Linear), k(24, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(8, 0.0, Linear), k(10, 4.0, Out), k(19, 3.8, Linear), k(26, 0.0, InOut)],
    reach: &[k(0, 0.7, Linear), k(3, 1.0, Out), k(24, 1.0, Linear), k(29, 0.5, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(3, 0.0, Out), k(24, 0.0, Linear), k(29, -20.0, InOut)],
    blade_pitch: &[k(0, -10.0, Linear), k(3, 120.0, Out), k(7, -60.0, Out), k(20, -50.0, Linear), k(28, -30.0, InOut)],
    blade_held: (0, 30),
    windows: &[
        w(4, 7, Shape::Weapon, Attack::SwordBreaker, 0),
        wg(8, 29, Shape::Around { radius: 220.0, ahead: 30.0 }, Attack::SwordBreakerRing, FX_SLAM | FX_ZAP),
    ],
    cancels: &[c(ALL, 18, 12)],
    sounds: &[(3, HOO, 0.0), (4, SWING_HEAVY, 0.5)],
    locked: (0, 7),
    air: true,
    ..BASE
};

/// LT in the air: the point straight down under him, dropping; on an enemy it bounces him up
/// again, three times a jump (Zelda II's down-thrust)
pub const POGO: Move = Move {
    name: "pogo",
    len: 24,
    clips: &[still(0, FIRST_PUNCH)],
    pitch: &[k(0, 0.0, Linear), k(3, -8.0, Out)],
    vel_y: &[(0, 4.0), (3, -60.0)],
    hold: (1, 3),
    squash: &[k(0, 1.0, Linear), k(2, 0.9, Out), k(4, 1.1, Out)],
    reach: &[k(0, 0.8, Linear), k(3, 1.0, Out)],
    blade_pitch: &[k(0, -30.0, Linear), k(3, -88.0, Out)],
    blade_held: (0, 24),
    windows: &[w(3, 23, Shape::Around { radius: 80.0, ahead: 0.0 }, Attack::SwordPogo, 0)],
    bounce: 44.0,
    cancels: &[c(DASH | SWAP, 6, 4)],
    sounds: &[(0, SWING_LIGHT, 0.3), (3, HOO, 0.0)],
    locked: (0, 3),
    air: true,
    land_ends: true,
    ..BASE
};

// ---- the swap into it ------------------------------------------------------------------------

/// L1 into the sword mid-fight: drawn as an iai cut in front (Mortal Draw)
pub const DRAW: Move = Move {
    name: "quick draw",
    len: 16,
    clips: &[clip(0, FIRST_PUNCH_FAST)],
    twist: &[k(0, -30.0, Linear), k(3, 30.0, Out), k(10, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 0, to: 4, max: 1.0, gap: 1.0 }),
    reach: &[k(0, 0.8, Linear), k(3, 1.0, Out), k(10, 1.0, Linear), k(15, 0.5, InOut)],
    blade_yaw: &[k(0, -160.0, Linear), k(4, 60.0, Out), k(10, 30.0, InOut), k(15, -20.0, InOut)],
    blade_pitch: &[k(0, -30.0, Linear), k(4, 5.0, Out), k(15, -30.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(10, 90.0, Linear), k(15, 0.0, InOut)],
    blade_held: (0, 16),
    windows: &[w(1, 5, Shape::Weapon, Attack::SwordDraw, 0)],
    cancels: &[c(NEXT, 7, 6), c(DJS, 8, 6)],
    sounds: &[(0, SWING, 0.5), (1, YAH, 0.0)],
    fx: &[(0, FX_SPARKLE)],
    locked: (0, 2),
    ..BASE
};

/// L1 into the sword in the air: drawn in a cut down across him
pub const DRAW_AIR: Move = Move {
    name: "quick draw (air)",
    len: 14,
    clips: &[clip(0, FIRST_PUNCH_FAST)],
    twist: &[k(0, -25.0, Linear), k(3, 25.0, Out), k(10, 0.0, InOut)],
    vel_y: &[(0, 0.0)],
    hold: (0, 8),
    reach: &[k(0, 0.9, Linear), k(2, 1.0, Out), k(9, 1.0, Linear), k(13, 0.6, InOut)],
    blade_yaw: &[k(0, -100.0, Linear), k(2, -20.0, Linear), k(3, 55.0, Out), k(13, 20.0, InOut)],
    blade_pitch: &[k(0, 40.0, Linear), k(2, 10.0, Linear), k(3, -35.0, Out), k(13, -25.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(9, 90.0, Linear), k(13, 0.0, InOut)],
    blade_held: (0, 14),
    windows: &[w(1, 4, Shape::Weapon, Attack::SwordDraw, 0)],
    cancels: &[c(NEXT | DASH | SWAP, 9, 8)],
    sounds: &[(0, SWING, 0.5)],
    fx: &[(0, FX_SPARKLE)],
    air: true,
    land_ends: true,
    ..BASE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::*;

    fn sword(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn every_sword_id_has_its_move() {
        let want: [(u16, &str); 26] = [
            (SW_S1, "star slash"),
            (SW_S2, "star backhand"),
            (SW_S3, "star rising slash"),
            (SW_S4, "Rebellion chop"),
            (SW_CLEAVE, "thunder cleave"),
            (SW_THRUST, "Stinger"),
            (SW_STABS, "Million Stab"),
            (SW_RISE, "High Time"),
            (SW_DRIVE, "Drive"),
            (SW_DRIVE_FULL, "Drive (full)"),
            (SW_DRIVE_JUST, "Drive JUST"),
            (SW_HIGH_TIME, "High Time (Y)"),
            (SW_CROSS, "cross cut"),
            (SW_LAUNCHER, "star launcher"),
            (SW_HELM, "Helm Splitter"),
            (SW_IAI, "lightning iai"),
            (SW_FLURRY, "Flurry Rush"),
            (SW_RAVE, "Aerial Rave"),
            (SW_RAVE + 1, "Aerial Rave 2"),
            (SW_RAVE + 2, "Aerial Rave 3"),
            (SW_RAVE + 3, "Aerial Rave finisher"),
            (SW_BREAKER, "Helm Breaker"),
            (SW_POGO, "pogo"),
            (SW_DRAW, "quick draw"),
            (SW_DRAW_AIR, "quick draw (air)"),
            (SW_S2 + 1, "star rising slash"),
        ];
        // (the other weapons' moves come after the sword's)
        assert!(MOVES.len() > SW_DRAW_AIR as usize);
        for (id, name) in want {
            assert_eq!(sword(id).name, name, "id {id}");
            assert!(is_sword(id));
        }
        assert!(!is_sword(BULLET_PUNCH) && !is_sword(SKY_STRIKE) && !is_sword(ZEUS_THROW));
    }

    /// No spins (the user: the sword never turns him round, Dante's Rebellion doesn't): no move
    /// twists his body or swings the blade past half a turn.
    #[test]
    fn the_sword_never_spins() {
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            let most = |keys: &[crate::script::Key]| keys.iter().map(|k| k.v.abs()).fold(0.0f32, f32::max);
            assert!(most(m.twist) < 90.0, "{}: twists {}", m.name, most(m.twist));
            assert!(most(m.blade_yaw) <= 180.0, "{}: blade round {}", m.name, most(m.blade_yaw));
        }
    }

    /// Rebellion's feel (Part 12): the blows land fast (2 / 2 / 3 / 4 ticks to the hit, the pose
    /// moving from the first tick, the swish at tick 1), then hold the cut pose; the next blow
    /// and the dodge open soon (a landed blow sooner); the chop's frozen lunge lasts.
    #[test]
    fn the_cuts_land_fast_and_chain_soon() {
        let want = [(SW_S1, 2, 6, 4), (SW_S2, 2, 5, 4), (SW_S3, 3, 6, 4), (SW_S4, 4, 16, 12)];
        for (id, hit, next, next_landed) in want {
            let m = sword(id);
            let w = m.windows[0];
            assert_eq!(w.from, hit, "{}: ticks to the hit", m.name);
            assert!(w.to - w.from <= 2, "{}", m.name);
            // (moving from tick 0: the blade's pose differs from tick 0 to tick 1, and it starts
            // from the ready pose or the blow before's)
            let eval = |keys: &[Key], t: f32| crate::script::eval(keys, t, 0.0);
            assert!(eval(m.blade_yaw, 0.0) != eval(m.blade_yaw, 1.0) || eval(m.blade_pitch, 0.0) != eval(m.blade_pitch, 1.0), "{}: still at the start", m.name);
            // the swish at tick 1, with the voice (or the heavy cut's) before it
            assert!(m.sounds.iter().any(|s| s.0 == 1 && crate::sfx::from_script(s.1).is_some()), "{}: the swish at tick 1", m.name);
            let nexts = m.cancels.iter().find(|c| c.into & NEXT != 0).unwrap();
            assert_eq!((nexts.from, nexts.on_hit), (next, next_landed), "{}: NEXT", m.name);
            assert!(nexts.on_hit > w.from, "{}: not before the cut starts", m.name);
        }
        // (the dash, jump and swap: from 6, 4 landed after the string's three; the chop 12, 9)
        for id in [SW_S1, SW_S2, SW_S3] {
            let djs = sword(id).cancels.iter().find(|c| c.into & DASH != 0).unwrap();
            assert_eq!((djs.from, djs.on_hit), (6, 4), "{}", sword(id).name);
        }
        let djs = sword(SW_S4).cancels.iter().find(|c| c.into & DASH != 0).unwrap();
        assert_eq!((djs.from, djs.on_hit), (12, 9));
        // (the finisher's frozen lunge: a few ticks past its cut still)
        assert!(djs.from >= sword(SW_S4).windows[0].to + 6);
    }

    /// The string's blows re-aim as the next one's window opens (a chain re-faces a moved enemy).
    #[test]
    fn a_chain_re_aims_at_its_next_window() {
        for id in [SW_S1, SW_S2, SW_S3, SW_S4] {
            let m = sword(id);
            let next = m.cancels.iter().find(|c| c.into & NEXT != 0).unwrap().from;
            assert!(m.aim.contains(&0) && m.aim.contains(&next), "{}: aims {:?}, next at {next}", m.name, m.aim);
        }
    }

    /// The knees do the crouch (pitch, squash), not the floor: the model sinks 4 units at most.
    #[test]
    fn no_sword_move_sinks_him_in_the_floor() {
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            let most = m.dip.iter().map(|k| k.v).fold(0.0f32, f32::max);
            assert!(most <= 4.0, "{}: dip {most}", m.name);
        }
    }

    /// Each blow of the string starts where the one before was when it let the next in (its
    /// cancel ticks): the same twist, lean, blade and hand, so nothing pops.
    #[test]
    fn a_blow_starts_where_the_last_one_was() {
        use crate::script::eval;
        for (a, b) in [(SW_S1, SW_S2), (SW_S2, SW_S3), (SW_S3, SW_S4)] {
            let (prev, next) = (sword(a), sword(b));
            let nc = prev.cancels.iter().find(|c| c.into & NEXT != 0).unwrap();
            // (a landed cancel inside the cut itself is mid-swing: the carry-in blends that one)
            for t in [nc.on_hit.max(prev.windows[0].to), nc.from] {
                let t = t as f32;
                let diffs = [
                    ("twist", eval(prev.twist, t, 0.0), eval(next.twist, 0.0, 0.0), 6.0),
                    ("pitch", eval(prev.pitch, t, 0.0), eval(next.pitch, 0.0, 0.0), 4.0),
                    ("blade yaw", eval(prev.blade_yaw, t, 0.0), eval(next.blade_yaw, 0.0, 0.0), 10.0),
                    ("blade pitch", eval(prev.blade_pitch, t, 0.0), eval(next.blade_pitch, 0.0, 0.0), 10.0),
                    ("blade roll", eval(prev.blade_roll, t, 0.0), eval(next.blade_roll, 0.0, 0.0), 12.0),
                    ("squash", eval(prev.squash, t, 1.0) * 100.0, eval(next.squash, 0.0, 1.0) * 100.0, 8.0),
                    ("reach", eval(prev.reach, t, 0.0) * 100.0, eval(next.reach, 0.0, 0.0) * 100.0, 15.0),
                ];
                for (what, x, y, tol) in diffs {
                    assert!((x - y).abs() <= tol, "{} -> {}: {what} {x:.1} then {y:.1} (cancel at {t})", prev.name, next.name);
                }
            }
        }
    }

    /// After the cut, the stick turns him and drifts him along it (not planted): the string's
    /// blows say when.
    #[test]
    fn the_follow_through_follows_the_stick() {
        for id in [SW_S1, SW_S2, SW_S3, SW_S4] {
            let m = sword(id);
            let w = m.windows[0];
            assert!(m.follow.0 > w.to && m.follow.1 > m.follow.0 && m.follow.1 <= m.len, "{}: {:?}", m.name, m.follow);
        }
    }

    /// The swings sound the mod's own (sfx.rs), not SM64's soft swish.
    #[test]
    fn the_swings_are_the_mods_sounds() {
        const SWISH: u32 = 0x305C_0081;
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            assert!(!m.sounds.iter().any(|s| s.1 == SWISH), "{}", m.name);
            if !m.windows.is_empty() {
                assert!(m.sounds.iter().any(|s| crate::sfx::from_script(s.1).is_some()), "{}: no swing heard", m.name);
            }
        }
    }

    #[test]
    fn the_sword_hits_with_its_own_blows() {
        let sword_blow = |a: Attack| {
            matches!(
                a,
                Attack::SwordSlash(_)
                    | Attack::SwordCleave
                    | Attack::SwordThrust
                    | Attack::SwordStab
                    | Attack::SwordStabEnd
                    | Attack::SwordRise
                    | Attack::SwordWave
                    | Attack::SwordWaveJust
                    | Attack::SwordCross
                    | Attack::SwordLauncher
                    | Attack::SwordHelm
                    | Attack::SwordHelmRing
                    | Attack::SwordIai
                    | Attack::SwordFlurry
                    | Attack::SwordFlurryEnd
                    | Attack::SwordRave(_)
                    | Attack::SwordBreaker
                    | Attack::SwordBreakerRing
                    | Attack::SwordPogo
                    | Attack::SwordDraw
            )
        };
        for id in SW_S1..=SW_DRAW_AIR {
            for w in sword(id).windows {
                assert!(sword_blow(w.attack), "{}: {:?}", sword(id).name, w.attack);
            }
        }
    }

    #[test]
    fn the_tables_counts_of_blows() {
        let count = |id: u16, a: Attack| sword(id).windows.iter().filter(|w| w.attack == a).count();
        assert_eq!(count(SW_FLURRY, Attack::SwordFlurry), 6);
        assert_eq!(count(SW_STABS, Attack::SwordStab), 9);
        // (a window strikes each enemy once, script::Player::first_strike; a repeat of the same
        // blow starts at least its cooldown after the one before, so each lands: the hit-stops only
        // add real ticks in between)
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            for (i, pair) in m.windows.windows(2).enumerate() {
                if pair[0].attack == pair[1].attack {
                    assert!((pair[1].from - pair[0].from) as u32 >= pair[1].attack.cooldown(), "{} window {}", m.name, i + 1);
                }
            }
        }
    }

    #[test]
    fn any_landed_blow_can_be_dashed_out_of() {
        // (the dynamic pillar: but the charges, which are no blow, and the air finishers' ends)
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            if m.windows.is_empty() {
                continue;
            }
            let dash = m.cancels.iter().filter(|c| c.into & DASH != 0).map(|c| c.on_hit.min(c.from)).min();
            assert!(dash.is_some_and(|t| t < m.len), "{}: no dash out", m.name);
        }
    }

    #[test]
    fn spins_end_facing_where_they_began() {
        // (a body turned a whole number of turns: the move after it doesn't snap round)
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            if let Some(last) = m.twist.last() {
                if last.v.abs() >= 180.0 {
                    assert!((last.v % 360.0).abs() < 0.01, "{} ends turned {}", m.name, last.v);
                }
            }
        }
    }

    #[test]
    fn the_air_moves_are_air_moves() {
        for id in [SW_RAVE, SW_RAVE + 1, SW_RAVE + 2, SW_RAVE + 3, SW_BREAKER, SW_POGO, SW_DRAW_AIR] {
            assert!(sword(id).air, "{}", sword(id).name);
        }
        for id in [SW_S1, SW_S4, SW_HELM, SW_RISE, SW_HIGH_TIME, SW_LAUNCHER, SW_DRAW, SW_DRIVE] {
            assert!(!sword(id).air, "{}", sword(id).name);
        }
        // the pogo bounces, High Time follows its enemy up with its button held (Y's by Y)
        assert!(sword(SW_POGO).bounce > 0.0);
        assert!(sword(SW_RISE).held_rise.is_some() && !sword(SW_RISE).rise_on_y);
        assert!(sword(SW_HIGH_TIME).held_rise.is_some() && sword(SW_HIGH_TIME).rise_on_y);
    }

    #[test]
    fn the_blade_never_turns_faster_than_it_can_follow() {
        // (sword.rs turns the blade at most MOVE_TURN a tick: where a move points it, tick to tick,
        // stays under it, or the blade lags behind its cut)
        use crate::script::eval;
        let mut fast = Vec::new();
        for id in SW_S1..=SW_DRAW_AIR {
            let m = sword(id);
            let dir = |t: f32| crate::sword::held_dir(0.0, eval(m.blade_yaw, t, 0.0), eval(m.blade_pitch, t, 0.0));
            for t in m.blade_held.0..m.blade_held.1.saturating_sub(1) {
                let turn = dir(t as f32).angle_between(dir(t as f32 + 1.0)).to_degrees();
                if turn > crate::sword::MOVE_TURN {
                    fast.push(format!("{} {turn:.0} at {t}", m.name));
                }
            }
        }
        assert!(fast.is_empty(), "degrees a tick: {fast:?}");
    }
}
