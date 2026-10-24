//! The Star Sword's moves as data (v4 Part 2, the table the user approved 2026-10-24): script.rs
//! plays them (MOVES from id SW_S1 on), swordplay.rs picks them from the buttons. Each move points
//! the blade (yaw round his facing + left, pitch up from level, roll round itself) and the hand
//! follows it (arm IK); its cuts hit along the blade itself (`Shape::Weapon`), its spins in short
//! windows (one blow a turn), its landings once (ground windows). Spins turn his body as drawn
//! (`twist`), not his facing: he keeps facing the enemy, and the Hurricane's stick steers him.

use crate::combat::Attack;
use crate::moves::{FX_CHARGE, FX_LAUNCH, FX_METEOR, FX_SLAM, FX_SPIN};
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, Shape, Window, ALL, BASE, DASH, FX_BEAM, FX_CROSS, FX_SPARKLE, FX_WAVE, FX_WAVE3, FX_ZAP, JUMP, NEXT, SWAP};

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const FIRST_PUNCH_FAST: i32 = 0x69;
const SECOND_PUNCH_FAST: i32 = 0x6A;
const SWINGING_BOWSER: i32 = 0xB6;
const TWIRL: i32 = 0x94;
const DOUBLE_JUMP_RISE: i32 = 0x50;
const GENERAL_LAND: i32 = 0x57;
const FAST_LONGJUMP: i32 = 0x13;
const DIVE: i32 = 0x88;
const SLOW_LAND_FROM_DIVE: i32 = 0x5A;

/// (SM64's: the soft ones at that volume, the voice at his body)
const SWISH: u32 = 0x305C_0081;
const SPIN: u32 = 0x0437_8081;
const YAH: u32 = 0x241E_8081;
const HOO: u32 = 0x241F_8081;
const WAH: u32 = 0x2424_8081;
const YAH_WAH_HOO: u32 = 0x2400_8081;
const GROUND_POUND_WAH: u32 = 0x2422_8081;
const SHORT_STAR: u32 = 0x3016_0091;
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

const fn around(radius: f32) -> Shape {
    Shape::Around { radius, ahead: 0.0 }
}

// ---- the string: X, X, X, X ------------------------------------------------------------------

/// X: a diagonal from over his right shoulder down to his left (Rebellion's Combo I)
pub const S1: Move = Move {
    name: "star slash",
    len: 15,
    clips: &[clip(0, FIRST_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(3, -30.0, Out), k(7, 25.0, Out), k(13, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, -4.0, Out), k(7, 8.0, Out), k(13, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 1, to: 6, max: 1.6, gap: 1.0 }),
    squash: &[k(0, 1.0, Linear), k(3, 0.93, Out), k(6, 1.06, Out), k(10, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(3, 4.0, Out), k(7, 0.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(3, 1.0, Out), k(10, 1.0, Linear), k(14, 0.5, InOut)],
    blade_yaw: &[k(0, -30.0, Linear), k(3, -80.0, Out), k(7, 60.0, Out), k(13, 20.0, InOut)],
    blade_pitch: &[k(0, -10.0, Linear), k(3, 60.0, Out), k(7, -30.0, Out)],
    blade_roll: &[k(0, 0.0, Linear), k(3, -40.0, Out), k(13, 0.0, InOut)],
    blade_held: (0, 15),
    windows: &[w(4, 7, Shape::Weapon, Attack::SwordSlash(0), 0)],
    cancels: &[c(NEXT, 8, 6), c(DJS, 9, 6)],
    sounds: &[(3, SWISH, 0.32)],
    locked: (0, 3),
    ..BASE
};

/// X, X: a backhand, level, left to right
pub const S2: Move = Move {
    name: "star backhand",
    len: 15,
    clips: &[clip(0, SECOND_PUNCH)],
    twist: &[k(0, 10.0, Linear), k(3, 35.0, Out), k(7, -30.0, Out), k(13, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, -3.0, Out), k(7, 6.0, Out), k(13, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 1, to: 6, max: 1.2, gap: 1.0 }),
    squash: &[k(0, 1.0, Linear), k(3, 0.94, Out), k(6, 1.05, Out), k(10, 1.0, InOut)],
    reach: &[k(0, 0.8, Linear), k(3, 1.0, Out), k(10, 1.0, Linear), k(14, 0.5, InOut)],
    blade_yaw: &[k(0, 40.0, Linear), k(3, 85.0, Out), k(7, -70.0, Out), k(13, -35.0, InOut)],
    blade_pitch: &[k(0, -10.0, Linear), k(3, 15.0, Out), k(7, 0.0, Out), k(13, -30.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(3, 30.0, Out), k(13, 0.0, InOut)],
    blade_held: (0, 15),
    windows: &[w(4, 7, Shape::Weapon, Attack::SwordSlash(1), 0)],
    cancels: &[c(NEXT, 8, 6), c(DJS, 9, 6)],
    sounds: &[(3, SWISH, 0.32)],
    locked: (0, 3),
    ..BASE
};

/// X, X, X: a rising diagonal, low on his right up to his left
pub const S3: Move = Move {
    name: "star rising slash",
    len: 16,
    clips: &[clip(0, FIRST_PUNCH_FAST)],
    twist: &[k(0, -5.0, Linear), k(3, -35.0, Out), k(8, 30.0, Out), k(14, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, 7.0, Out), k(8, -8.0, Out), k(14, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 1, to: 6, max: 1.2, gap: 1.0 }),
    squash: &[k(0, 1.0, Linear), k(3, 0.9, Out), k(8, 1.08, Out), k(12, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(3, 8.0, Out), k(8, 0.0, Out)],
    reach: &[k(0, 0.7, Linear), k(3, 1.0, Out), k(11, 1.0, Linear), k(15, 0.5, InOut)],
    blade_yaw: &[k(0, -40.0, Linear), k(3, -70.0, Out), k(8, 50.0, Out), k(14, 25.0, InOut)],
    blade_pitch: &[k(0, -25.0, Linear), k(3, -45.0, Out), k(8, 70.0, Out), k(14, 15.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(3, 40.0, Out), k(14, 0.0, InOut)],
    blade_held: (0, 16),
    windows: &[w(4, 8, Shape::Weapon, Attack::SwordSlash(2), 0)],
    cancels: &[c(NEXT, 9, 7), c(DJS, 10, 7)],
    sounds: &[(3, SWISH, 0.35), (5, HOO, 0.0)],
    locked: (0, 3),
    ..BASE
};

/// X x4: a full turn with the blade out (knocks them back); at full health a sword beam
pub const S4: Move = Move {
    name: "star spin finisher",
    len: 24,
    clips: &[clip(0, FIRST_PUNCH), clip(3, SWINGING_BOWSER), clip(14, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(3, -40.0, Out), k(12, 360.0, Linear)],
    pitch: &[k(0, 0.0, Linear), k(3, -5.0, Out), k(5, 6.0, Out), k(12, 4.0, Linear), k(18, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 0, to: 4, max: 1.2, gap: 1.1 }),
    squash: &[k(0, 1.0, Linear), k(3, 0.9, Out), k(6, 1.06, Out), k(13, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(3, 6.0, Out), k(6, 0.0, Out), k(14, 6.0, Out), k(20, 0.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(15, 1.0, Linear), k(22, 0.4, InOut)],
    // (out to his right as he turns left: it sweeps round through the front)
    blade_yaw: &[k(0, -60.0, Linear), k(3, -120.0, Out), k(12, 280.0, Linear), k(16, 280.0, Linear), k(22, 335.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(3, 10.0, Out), k(5, 0.0, Out), k(14, 0.0, Linear), k(22, -35.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(5, 90.0, Out), k(16, 90.0, Linear), k(22, 0.0, InOut)],
    blade_held: (0, 24),
    windows: &[w(5, 12, Shape::Weapon, Attack::SwordSlash(3), 0)],
    cancels: &[c(DJS, 17, 14), c(NEXT, 19, 16)],
    sounds: &[(2, WAH, 0.0), (5, SPIN, 0.35)],
    fx: &[(5, FX_SPIN), (9, FX_SPIN), (13, FX_BEAM)],
    locked: (0, 12),
    ..BASE
};

/// X, X, a beat, X: two turns cutting all round him (three blows), an upward flick to end
/// (Rebellion's Combo II)
pub const WHIRL: Move = Move {
    name: "star whirl",
    len: 30,
    clips: &[clip(0, SWINGING_BOWSER), clip(21, FIRST_PUNCH_FAST)],
    twist: &[k(0, 0.0, Linear), k(2, -30.0, Out), k(20, 690.0, Linear), k(24, 720.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(2, 4.0, Out), k(20, 4.0, Linear), k(22, 8.0, Out), k(25, -10.0, Out), k(29, 0.0, InOut)],
    aim: &[0, 20],
    approach: Some(Approach { from: 0, to: 4, max: 1.4, gap: 1.1 }),
    squash: &[k(0, 1.0, Linear), k(20, 1.0, Linear), k(22, 0.9, Out), k(25, 1.1, Out), k(29, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(20, 0.0, Linear), k(22, 6.0, Out), k(25, 0.0, Out)],
    reach: &[k(0, 0.6, Linear), k(2, 1.0, Out), k(26, 1.0, Linear), k(29, 0.5, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(2, 45.0, Out), k(20, 765.0, Linear), k(22, 720.0, Out), k(29, 700.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(2, -5.0, Out), k(20, -5.0, Linear), k(22, -35.0, Out), k(25, 70.0, Out), k(29, 10.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(2, 90.0, Out), k(20, 90.0, Linear), k(22, 0.0, Out)],
    blade_held: (0, 30),
    windows: &[
        w(4, 6, around(180.0), Attack::SwordWhirl, 0),
        w(10, 12, around(180.0), Attack::SwordWhirl, 0),
        w(16, 18, around(180.0), Attack::SwordWhirl, 0),
        w(23, 25, Shape::Arc { radius: 190.0, half_deg: 70.0, ahead: 0.0 }, Attack::SwordWhirlEnd, 0),
    ],
    cancels: &[c(DJS, 27, 25), c(NEXT, 28, 26)],
    sounds: &[(2, SPIN, 0.3), (9, SWISH, 0.25), (15, SWISH, 0.25), (22, HOO, 0.0), (23, SWISH, 0.4)],
    fx: &[(4, FX_SPIN), (10, FX_SPIN), (16, FX_SPIN), (23, FX_LAUNCH), (24, FX_BEAM)],
    locked: (0, 20),
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
    dip: &[k(0, 0.0, Linear), k(6, 0.0, Linear), k(10, 12.0, Out), k(20, 0.0, InOut)],
    reach: &[k(0, 0.5, Linear), k(5, 1.0, Out), k(18, 1.0, Linear), k(26, 0.4, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(6, 0.0, Out), k(26, -20.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(6, 125.0, Out), k(8, 125.0, Linear), k(12, -55.0, Out), k(20, -50.0, Linear), k(26, -30.0, InOut)],
    blade_held: (0, 28),
    windows: &[w(9, 12, Shape::Weapon, Attack::SwordCleave, 0)],
    cancels: &[c(DJS, 21, 16), c(NEXT, 23, 18)],
    sounds: &[(3, SWISH, 0.2), (8, GROUND_POUND_WAH, 0.0), (9, SWISH, 0.42), (12, HEAVY_LANDING, 0.3)],
    fx: &[(12, FX_ZAP)],
    locked: (0, 12),
    ..BASE
};

// ---- closers and launchers -------------------------------------------------------------------

/// back, forward + X: a coil, then he flies at the enemy point first (up to 8 m; DMC's Stinger);
/// X mashed as it lands: the thousand stabs
pub const THRUST: Move = Move {
    name: "Bullet Bill thrust",
    len: 26,
    clips: &[
        Clip { from: 0, anim: FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(4, 1.0, In)] },
        Clip { from: 5, anim: DIVE, frames: &[k(5, 0.0, Linear), k(9, 5.0, Out), k(12, 6.0, Linear)] },
        Clip { from: 13, anim: SLOW_LAND_FROM_DIVE, frames: &[k(13, 0.0, Linear), k(25, 14.0, Out)] },
    ],
    twist: &[k(0, 0.0, Linear), k(4, -30.0, Out), k(7, 0.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(4, -6.0, InOut), k(8, 4.0, Out), k(13, 0.0, InOut)],
    aim: &[0, 4],
    speed: &[k(0, 0.0, Linear), k(12, 0.0, Linear), k(13, 3.0, Linear), k(19, 0.0, Out)],
    approach: Some(Approach { from: 5, to: 12, max: 8.0, gap: 0.8 }),
    vel_y: &[(5, 14.0)],
    squash: &[k(0, 1.0, Linear), k(4, 0.86, Out), k(6, 1.12, Out), k(11, 1.0, InOut), k(13, 0.9, Out), k(17, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 8.0, Out), k(6, 0.0, Out), k(13, 6.0, Out), k(18, 0.0, InOut)],
    // (the hand pulled in for the coil, then out with the point)
    reach: &[k(0, 0.3, Linear), k(4, 0.2, Out), k(7, 1.0, Out), k(13, 1.0, Linear), k(20, 0.4, InOut)],
    blade_yaw: &[k(0, -15.0, Linear), k(4, -20.0, Out), k(7, 0.0, Out), k(20, -15.0, InOut)],
    blade_pitch: &[k(0, -15.0, Linear), k(4, -5.0, Out), k(7, -4.0, Out), k(20, -25.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear)],
    blade_held: (0, 26),
    windows: &[w(7, 13, Shape::Front { reach: 200.0 }, Attack::SwordThrust, 0)],
    cancels: &[c(ALL, 19, 13)],
    sounds: &[(0, SWISH, 0.2), (5, YAH, 0.0), (6, SWISH, 0.45)],
    fx: &[(5, FX_LAUNCH)],
    locked: (0, 5),
    ..BASE
};

/// X mashed on the thrust: nine stabs in 1.2 s, a last thrust that knocks back (Million Stab)
pub const STABS: Move = Move {
    name: "thousand stabs",
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
    sounds: &[(0, WAH, 0.0), (2, SWISH, 0.18), (10, SWISH, 0.18), (18, SWISH, 0.18), (26, SWISH, 0.18), (34, SWISH, 0.18), (38, YAH, 0.0), (40, SWISH, 0.4)],
    fx: &[(40, FX_BEAM)],
    locked: (0, 2),
    ..BASE
};

/// back + X: a rising cut that launches small and medium enemies; X still held, he goes up with
/// them (DMC's High Time)
pub const RISE: Move = Move {
    name: "rising cut",
    len: 26,
    clips: &[still(0, FIRST_PUNCH), clip(4, FIRST_PUNCH_FAST), clip(9, DOUBLE_JUMP_RISE)],
    twist: &[k(0, 0.0, Linear), k(3, -20.0, Out), k(7, 15.0, Out), k(16, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, 10.0, Out), k(7, -12.0, Out), k(16, 0.0, InOut)],
    aim: &[0],
    squash: &[k(0, 1.0, Linear), k(3, 0.85, Out), k(7, 1.12, Out), k(12, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(3, 12.0, Out), k(7, 0.0, Out)],
    reach: &[k(0, 0.5, Linear), k(3, 0.8, Out), k(7, 1.0, Out), k(16, 1.0, Linear), k(22, 0.5, InOut)],
    blade_yaw: &[k(0, -25.0, Linear), k(3, -50.0, Out), k(7, 10.0, Out), k(16, 0.0, Linear), k(22, -20.0, InOut)],
    blade_pitch: &[k(0, -35.0, Linear), k(3, -55.0, Out), k(7, 85.0, Out), k(14, 80.0, Linear), k(22, -10.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(3, 90.0, Out), k(16, 90.0, Linear), k(22, 0.0, InOut)],
    blade_held: (0, 26),
    windows: &[w(4, 7, Shape::Weapon, Attack::SwordRise, 0)],
    // (risen with them: the air string's X from 14; held at the top a moment for it)
    held_rise: Some((8, 52.0)),
    hold: (20, 25),
    cancels: &[c(NEXT, 14, 10), c(DJS, 16, 10)],
    sounds: &[(3, SWISH, 0.3), (4, HOO, 0.0)],
    fx: &[(4, FX_LAUNCH)],
    locked: (0, 5),
    ..BASE
};

/// X + A: up off the ground spinning twice, the blade out (Link's spin jump), ends in the air
pub const SPIN_JUMP: Move = Move {
    name: "spin jump",
    len: 26,
    clips: &[clip(0, TWIRL)],
    twist: &[k(0, 0.0, Linear), k(17, 720.0, Linear)],
    vel_y: &[(1, 46.0)],
    reach: &[k(0, 1.0, Linear), k(18, 1.0, Linear), k(25, 0.6, InOut)],
    blade_yaw: &[k(0, 85.0, Linear), k(17, 805.0, Linear), k(24, 700.0, InOut)],
    blade_pitch: &[k(0, 5.0, Linear), k(17, 10.0, Linear), k(24, -30.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(17, 90.0, Linear), k(24, 0.0, InOut)],
    blade_held: (0, 26),
    windows: &[
        w(2, 4, around(170.0), Attack::SwordSpinJump, 0),
        w(6, 8, around(170.0), Attack::SwordSpinJump, 0),
        w(10, 12, around(170.0), Attack::SwordSpinJump, 0),
        w(14, 16, around(170.0), Attack::SwordSpinJump, 0),
        w(18, 20, around(200.0), Attack::SwordSpinJumpEnd, 0),
    ],
    cancels: &[c(ALL, 21, 19)],
    sounds: &[(1, SPIN, 0.35), (9, SPIN, 0.3), (17, SPIN, 0.3)],
    fx: &[(1, FX_LAUNCH), (2, FX_SPIN), (10, FX_SPIN), (19, FX_SPIN)],
    locked: (0, 17),
    ..BASE
};

// ---- X held: the star wave ------------------------------------------------------------------

/// X held past a blow: low in an iai stance, the blade drawn back, light gathering on it; it
/// gleams when a let go would be JUST (swordplay.rs JUST_FROM..JUST_TO ticks held)
pub const WAVE_CHARGE: Move = Move {
    name: "star wave charge",
    len: 300,
    clips: &[still(0, FIRST_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(6, -25.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(6, 6.0, Out)],
    squash: &[k(0, 1.0, Linear), k(6, 0.93, Out)],
    dip: &[k(0, 0.0, Linear), k(6, 10.0, Out)],
    reach: &[k(0, 0.6, Linear), k(6, 0.8, Out)],
    blade_yaw: &[k(0, -30.0, Linear), k(6, -150.0, Out)],
    blade_pitch: &[k(0, -30.0, Linear), k(6, -20.0, Out)],
    blade_roll: &[k(0, 0.0, Linear), k(6, 90.0, Out)],
    blade_held: (0, 300),
    cancels: &[c(DASH | JUMP, 4, 4)],
    sounds: &[(0, SWISH, 0.2), (12, SHORT_STAR, 0.22)],
    fx: &[(12, FX_SPARKLE), (15, FX_SPARKLE), (18, FX_SPARKLE)],
    pulse: Some((10, FX_CHARGE)),
    ..BASE
};

/// X let go: an iai slash that sends a crescent of light along the ground, 10 m (wave.rs)
pub const WAVE: Move = Move {
    name: "star wave",
    len: 18,
    clips: &[clip(0, FIRST_PUNCH_FAST)],
    twist: &[k(0, -25.0, Linear), k(3, 30.0, Out), k(10, 0.0, InOut)],
    pitch: &[k(0, 6.0, Linear), k(3, 10.0, Out), k(10, 0.0, InOut)],
    squash: &[k(0, 0.93, Linear), k(3, 1.08, Out), k(9, 1.0, InOut)],
    dip: &[k(0, 10.0, Linear), k(3, 4.0, Out), k(10, 0.0, InOut)],
    reach: &[k(0, 0.8, Linear), k(3, 1.0, Out), k(12, 1.0, Linear), k(17, 0.5, InOut)],
    blade_yaw: &[k(0, -150.0, Linear), k(4, 55.0, Out), k(12, 30.0, InOut), k(17, -20.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(4, 5.0, Out), k(17, -25.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(4, 90.0, Linear), k(17, 0.0, InOut)],
    blade_held: (0, 18),
    cancels: &[c(ALL, 9, 9)],
    sounds: &[(1, SWISH, 0.45), (2, YAH, 0.0)],
    fx: &[(3, FX_WAVE)],
    locked: (0, 3),
    ..BASE
};

/// X let go on the gleam: JUST, three cuts and three crescents (Vergil's Judgement Cut)
pub const WAVE_JUST: Move = Move {
    name: "star wave JUST",
    len: 24,
    clips: &[clip(0, FIRST_PUNCH_FAST), clip(4, SECOND_PUNCH_FAST), clip(8, FIRST_PUNCH)],
    twist: &[k(0, -25.0, Linear), k(3, 30.0, Out), k(7, -25.0, Out), k(9, 0.0, Out), k(14, 0.0, Linear)],
    pitch: &[k(0, 6.0, Linear), k(9, -6.0, Out), k(12, 14.0, Out), k(18, 0.0, InOut)],
    dip: &[k(0, 10.0, Linear), k(3, 4.0, Out), k(9, 0.0, Out), k(12, 8.0, Out), k(18, 0.0, InOut)],
    reach: &[k(0, 0.8, Linear), k(2, 1.0, Out), k(14, 1.0, Linear), k(22, 0.5, InOut)],
    blade_yaw: &[k(0, -150.0, Linear), k(3, 55.0, Out), k(7, -60.0, Out), k(9, 0.0, Out), k(12, 0.0, Linear), k(20, -20.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(3, 25.0, Out), k(7, -20.0, Out), k(9, 120.0, Out), k(12, -40.0, Out), k(20, -30.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(12, 0.0, InOut)],
    blade_held: (0, 24),
    cancels: &[c(ALL, 16, 16)],
    sounds: &[(1, SWISH, 0.42), (2, WAH, 0.0), (5, SWISH, 0.38), (9, SWISH, 0.38)],
    fx: &[(2, FX_WAVE3)],
    locked: (0, 11),
    ..BASE
};

// ---- Y: the spins ----------------------------------------------------------------------------

/// Y: a quick full turn, the blade out (Zelda's spin)
pub const SPIN_CUT: Move = Move {
    name: "star spin",
    len: 18,
    clips: &[clip(0, SWINGING_BOWSER)],
    twist: &[k(0, 0.0, Linear), k(1, -25.0, Out), k(9, 360.0, Linear)],
    squash: &[k(0, 1.0, Linear), k(1, 0.92, Out), k(3, 1.04, Out), k(9, 1.0, InOut)],
    reach: &[k(0, 0.7, Linear), k(2, 1.0, Out), k(12, 1.0, Linear), k(17, 0.5, InOut)],
    blade_yaw: &[k(0, -30.0, Linear), k(1, -105.0, Out), k(9, 280.0, Linear), k(15, 335.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(2, 0.0, Out), k(10, 0.0, Linear), k(16, -30.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(2, 90.0, Out), k(11, 90.0, Linear), k(16, 0.0, InOut)],
    blade_held: (0, 18),
    windows: &[w(2, 9, Shape::Weapon, Attack::SwordSpin, 0)],
    cancels: &[c(NEXT, 10, 8), c(DJS, 11, 8)],
    sounds: &[(1, SPIN, 0.35)],
    fx: &[(2, FX_SPIN), (6, FX_SPIN)],
    locked: (0, 2),
    ..BASE
};

/// Y held: the blade out low behind him, light gathering; it gleams at each level
/// (swordplay.rs SPIN_LEVELS)
pub const SPIN_CHARGE: Move = Move {
    name: "Great Spin charge",
    len: 300,
    clips: &[still(0, FIRST_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(6, -35.0, Out)],
    squash: &[k(0, 1.0, Linear), k(6, 0.94, Out)],
    dip: &[k(0, 0.0, Linear), k(6, 8.0, Out)],
    reach: &[k(0, 0.7, Linear), k(6, 1.0, Out)],
    blade_yaw: &[k(0, -40.0, Linear), k(6, -110.0, Out)],
    blade_pitch: &[k(0, -20.0, Linear), k(6, -15.0, Out)],
    blade_roll: &[k(0, 0.0, Linear), k(6, 90.0, Out)],
    blade_held: (0, 300),
    cancels: &[c(DASH | JUMP, 4, 4)],
    sounds: &[(0, SWISH, 0.18), (18, SHORT_STAR, 0.18), (57, SHORT_STAR, 0.24)],
    fx: &[(18, FX_SPARKLE), (57, FX_SPARKLE)],
    pulse: Some((10, FX_CHARGE)),
    ..BASE
};

/// The Great Spin let go, by charge level: one turn, then two with a lightning ring, then three
/// with a quake; its blow all round him at once, wider at full health (`Shape::Burst`)
pub const GREAT_SPIN: [Move; 3] = [
    Move {
        name: "Great Spin",
        len: 20,
        clips: &[clip(0, SWINGING_BOWSER)],
        twist: &[k(0, -35.0, Linear), k(9, 360.0, Linear)],
        squash: &[k(0, 0.94, Linear), k(3, 1.06, Out), k(8, 1.0, InOut)],
        dip: &[k(0, 8.0, Linear), k(3, 0.0, Out)],
        reach: &[k(0, 1.0, Linear), k(14, 1.0, Linear), k(19, 0.5, InOut)],
        blade_yaw: &[k(0, -110.0, Linear), k(9, 285.0, Linear), k(16, 335.0, InOut)],
        blade_pitch: &[k(0, -15.0, Linear), k(3, 0.0, Out), k(12, 0.0, Linear), k(18, -30.0, InOut)],
        blade_roll: &[k(0, 90.0, Linear), k(12, 90.0, Linear), k(18, 0.0, InOut)],
        blade_held: (0, 20),
        windows: &[w(3, 3, Shape::Burst { radius: 200.0 }, Attack::SwordGreatSpin(0), 0)],
        cancels: &[c(ALL, 14, 12)],
        sounds: &[(0, SPIN, 0.38), (1, YAH, 0.0)],
        fx: &[(3, FX_SPIN), (7, FX_SPIN)],
        locked: (0, 9),
        ..BASE
    },
    Move {
        name: "Great Spin 2",
        len: 24,
        clips: &[clip(0, SWINGING_BOWSER)],
        twist: &[k(0, -35.0, Linear), k(14, 720.0, Linear)],
        squash: &[k(0, 0.94, Linear), k(3, 1.08, Out), k(8, 1.0, InOut)],
        dip: &[k(0, 8.0, Linear), k(3, 0.0, Out)],
        reach: &[k(0, 1.0, Linear), k(18, 1.0, Linear), k(23, 0.5, InOut)],
        blade_yaw: &[k(0, -110.0, Linear), k(14, 645.0, Linear), k(20, 695.0, InOut)],
        blade_pitch: &[k(0, -15.0, Linear), k(3, 0.0, Out), k(16, 0.0, Linear), k(22, -30.0, InOut)],
        blade_roll: &[k(0, 90.0, Linear), k(16, 90.0, Linear), k(22, 0.0, InOut)],
        blade_held: (0, 24),
        windows: &[w(3, 3, Shape::Burst { radius: 250.0 }, Attack::SwordGreatSpin(1), 0)],
        cancels: &[c(ALL, 18, 16)],
        sounds: &[(0, SPIN, 0.42), (1, WAH, 0.0), (3, THUNDER, 0.22)],
        fx: &[(3, FX_SPIN | FX_ZAP), (8, FX_SPIN), (13, FX_SPIN)],
        locked: (0, 14),
        ..BASE
    },
    Move {
        name: "Great Spin 3",
        len: 30,
        clips: &[clip(0, SWINGING_BOWSER)],
        twist: &[k(0, -35.0, Linear), k(20, 1080.0, Linear)],
        squash: &[k(0, 0.92, Linear), k(3, 1.1, Out), k(8, 1.0, InOut)],
        dip: &[k(0, 10.0, Linear), k(3, 0.0, Out)],
        reach: &[k(0, 1.0, Linear), k(24, 1.0, Linear), k(29, 0.5, InOut)],
        blade_yaw: &[k(0, -110.0, Linear), k(20, 1005.0, Linear), k(26, 1055.0, InOut)],
        blade_pitch: &[k(0, -15.0, Linear), k(3, 0.0, Out), k(22, 0.0, Linear), k(28, -30.0, InOut)],
        blade_roll: &[k(0, 90.0, Linear), k(22, 90.0, Linear), k(28, 0.0, InOut)],
        blade_held: (0, 30),
        windows: &[w(3, 3, Shape::Burst { radius: 300.0 }, Attack::SwordGreatSpin(2), 0)],
        cancels: &[c(ALL, 24, 22)],
        sounds: &[(0, SPIN, 0.45), (1, YAH_WAH_HOO, 0.0), (3, THUNDER, 0.28)],
        fx: &[(3, FX_SPIN | FX_ZAP | FX_METEOR), (9, FX_SPIN), (15, FX_SPIN), (20, FX_BEAM)],
        locked: (0, 20),
        ..BASE
    },
];

/// Y, X: two seconds of spinning, the stick steering him about (Wind Waker's Hurricane Spin):
/// six blows each, a last that throws them off; a short dizzy wobble after
pub const HURRICANE: Move = Move {
    name: "Hurricane",
    len: 86,
    clips: &[clip(0, SWINGING_BOWSER), clip(68, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(64, 2520.0, Linear)],
    roll: &[k(0, 0.0, Linear), k(66, 0.0, Linear), k(70, 10.0, Out), k(74, -8.0, InOut), k(78, 6.0, InOut), k(82, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(66, 0.0, Linear), k(70, -6.0, Out), k(80, 0.0, InOut)],
    speed: &[k(0, 0.0, Linear), k(4, 2.5, Out), k(64, 2.5, Linear), k(68, 0.0, Out)],
    steer: true,
    reach: &[k(0, 1.0, Linear), k(66, 1.0, Linear), k(74, 0.4, InOut)],
    blade_yaw: &[k(0, 90.0, Linear), k(64, 2610.0, Linear), k(70, 2540.0, InOut), k(80, 2495.0, InOut)],
    blade_pitch: &[k(0, 0.0, Linear), k(64, 0.0, Linear), k(70, -30.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(64, 90.0, Linear), k(70, 0.0, InOut)],
    blade_held: (0, 86),
    windows: &[
        w(8, 9, around(180.0), Attack::SwordHurricane, 0),
        w(18, 19, around(180.0), Attack::SwordHurricane, 0),
        w(28, 29, around(180.0), Attack::SwordHurricane, 0),
        w(38, 39, around(180.0), Attack::SwordHurricane, 0),
        w(48, 49, around(180.0), Attack::SwordHurricane, 0),
        w(58, 59, around(180.0), Attack::SwordHurricane, 0),
        w(64, 66, around(210.0), Attack::SwordHurricaneEnd, 0),
    ],
    cancels: &[c(DJS, 20, 10), c(ALL, 80, 74)],
    sounds: &[(0, HOO, 0.0), (1, SPIN, 0.3), (11, SPIN, 0.22), (21, SPIN, 0.22), (31, SPIN, 0.22), (41, SPIN, 0.22), (51, SPIN, 0.22), (64, SWISH, 0.4)],
    fx: &[(8, FX_SPIN), (18, FX_SPIN), (28, FX_SPIN), (38, FX_SPIN), (48, FX_SPIN), (58, FX_SPIN), (65, FX_SPIN | FX_BEAM)],
    locked: (0, 8),
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
    sounds: &[(1, SWISH, 0.35), (5, SWISH, 0.35), (9, HOO, 0.0)],
    fx: &[(9, FX_CROSS)],
    locked: (0, 9),
    ..BASE
};

/// X, X, Y: the rising cut, Mario going up with them every time (the other weapons' X, X, Y)
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
    dip: &[k(0, 0.0, Linear), k(3, 12.0, Out), k(7, 0.0, Out)],
    reach: &[k(0, 0.5, Linear), k(3, 0.8, Out), k(7, 1.0, Out), k(18, 1.0, Linear), k(24, 0.5, InOut)],
    blade_yaw: &[k(0, -25.0, Linear), k(3, -50.0, Out), k(7, 10.0, Out), k(16, 0.0, Linear), k(24, -20.0, InOut)],
    blade_pitch: &[k(0, -35.0, Linear), k(3, -55.0, Out), k(7, 85.0, Out), k(14, 80.0, Linear), k(24, -10.0, InOut)],
    blade_roll: &[k(0, 0.0, Linear), k(3, 90.0, Out), k(16, 90.0, Linear), k(24, 0.0, InOut)],
    blade_held: (0, 26),
    windows: &[w(4, 7, Shape::Weapon, Attack::SwordLauncher, 0)],
    cancels: &[c(NEXT | DASH | SWAP, 12, 10)],
    sounds: &[(3, SWISH, 0.32), (5, YAH, 0.0)],
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
    dip: &[k(0, 0.0, Linear), k(3, 10.0, Out), k(6, 0.0, Out), k(18, 0.0, Linear), k(20, 10.0, Out), k(28, 0.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(30, 1.0, Linear), k(35, 0.5, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(4, 0.0, Out), k(30, 0.0, Linear), k(34, -20.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(4, 95.0, Out), k(12, 130.0, InOut), k(14, 130.0, Linear), k(17, -55.0, Out), k(26, -50.0, Linear), k(34, -30.0, InOut)],
    blade_held: (0, 36),
    windows: &[
        w(15, 17, Shape::Weapon, Attack::SwordHelm, 0),
        wg(18, 32, Shape::Around { radius: 210.0, ahead: 40.0 }, Attack::SwordHelmRing, FX_SLAM | FX_ZAP),
    ],
    cancels: &[c(ALL, 30, 24)],
    sounds: &[(3, SWISH, 0.2), (4, YAH_WAH_HOO, 0.0), (15, SWISH, 0.45), (16, GROUND_POUND_WAH, 0.0)],
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
    dip: &[k(0, 10.0, Linear), k(2, 6.0, Out), k(9, 10.0, Out), k(16, 0.0, InOut)],
    reach: &[k(0, 0.7, Linear), k(4, 1.0, Out), k(16, 1.0, Linear), k(22, 0.5, InOut)],
    // (out, then back down behind him, as if sheathed: the crack comes then)
    blade_yaw: &[k(0, -150.0, Linear), k(4, 60.0, Out), k(9, 60.0, Linear), k(12, -160.0, InOut), k(22, -30.0, InOut)],
    blade_pitch: &[k(0, -20.0, Linear), k(4, 5.0, Out), k(9, 0.0, Linear), k(12, -25.0, InOut), k(22, -30.0, Linear)],
    blade_roll: &[k(0, 90.0, Linear), k(9, 90.0, Linear), k(14, 0.0, InOut)],
    blade_held: (0, 24),
    windows: &[w(11, 12, Shape::Around { radius: 260.0, ahead: -120.0 }, Attack::SwordIai, 0)],
    cancels: &[c(ALL, 16, 13)],
    sounds: &[(1, SWISH, 0.2), (3, SWISH, 0.45), (11, THUNDER, 0.25)],
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
    sounds: &[(0, SWISH, 0.3), (6, SWISH, 0.28), (14, SWISH, 0.28), (22, SWISH, 0.28), (29, YAH, 0.0), (31, SWISH, 0.45)],
    fx: &[(31, FX_SPIN), (32, FX_BEAM)],
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
        sounds: &[(2, SWISH, 0.3)],
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
        sounds: &[(2, SWISH, 0.3)],
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
        sounds: &[(2, SWISH, 0.32), (4, HOO, 0.0)],
        air: true,
        land_ends: true,
        ..BASE
    },
    Move {
        name: "Aerial Rave finisher",
        len: 18,
        clips: &[clip(0, SWINGING_BOWSER)],
        twist: &[k(0, 0.0, Linear), k(1, -30.0, Out), k(10, 360.0, Linear)],
        aim: &[0],
        vel_y: &[(0, 0.0)],
        hold: (0, 12),
        reach: &[k(0, 0.8, Linear), k(2, 1.0, Out), k(12, 1.0, Linear), k(17, 0.5, InOut)],
        blade_yaw: &[k(0, -30.0, Linear), k(1, -110.0, Out), k(10, 280.0, Linear), k(16, 335.0, InOut)],
        blade_pitch: &[k(0, -10.0, Linear), k(2, 0.0, Out), k(11, 0.0, Linear), k(16, -30.0, InOut)],
        blade_roll: &[k(0, 0.0, Linear), k(2, 90.0, Out), k(11, 90.0, Linear), k(16, 0.0, InOut)],
        blade_held: (0, 18),
        windows: &[w(2, 10, Shape::Weapon, Attack::SwordRave(3), 0)],
        // (the air string ends here: he drops, the dash and a swap still open)
        cancels: &[c(DASH | SWAP, 12, 11)],
        sounds: &[(1, SPIN, 0.32), (2, WAH, 0.0)],
        fx: &[(2, FX_SPIN), (10, FX_BEAM)],
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
    squash: &[k(0, 1.0, Linear), k(3, 1.08, Out), k(7, 1.1, Linear), k(9, 0.85, Out), k(16, 1.0, InOut)],
    reach: &[k(0, 0.7, Linear), k(3, 1.0, Out), k(24, 1.0, Linear), k(29, 0.5, InOut)],
    blade_yaw: &[k(0, -20.0, Linear), k(3, 0.0, Out), k(24, 0.0, Linear), k(29, -20.0, InOut)],
    blade_pitch: &[k(0, -10.0, Linear), k(3, 120.0, Out), k(7, -60.0, Out), k(20, -50.0, Linear), k(28, -30.0, InOut)],
    blade_held: (0, 30),
    windows: &[
        w(4, 7, Shape::Weapon, Attack::SwordBreaker, 0),
        wg(8, 29, Shape::Around { radius: 220.0, ahead: 30.0 }, Attack::SwordBreakerRing, FX_SLAM | FX_ZAP),
    ],
    cancels: &[c(ALL, 18, 12)],
    sounds: &[(0, SWISH, 0.25), (3, HOO, 0.0), (4, SWISH, 0.4)],
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
    sounds: &[(0, SWISH, 0.25), (3, HOO, 0.0)],
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
    sounds: &[(0, SWISH, 0.45), (1, YAH, 0.0)],
    fx: &[(0, FX_SPARKLE)],
    locked: (0, 2),
    ..BASE
};

/// L1 into the sword in the air: drawn in a spinning cut
pub const DRAW_AIR: Move = Move {
    name: "quick draw (air)",
    len: 14,
    clips: &[clip(0, SWINGING_BOWSER)],
    twist: &[k(0, 0.0, Linear), k(8, 360.0, Linear)],
    vel_y: &[(0, 0.0)],
    hold: (0, 8),
    reach: &[k(0, 0.9, Linear), k(2, 1.0, Out), k(9, 1.0, Linear), k(13, 0.6, InOut)],
    blade_yaw: &[k(0, -80.0, Linear), k(8, 280.0, Linear), k(13, 335.0, InOut)],
    blade_pitch: &[k(0, 0.0, Linear), k(9, 0.0, Linear), k(13, -25.0, InOut)],
    blade_roll: &[k(0, 90.0, Linear), k(9, 90.0, Linear), k(13, 0.0, InOut)],
    blade_held: (0, 14),
    windows: &[w(1, 8, Shape::Weapon, Attack::SwordDraw, 0)],
    cancels: &[c(NEXT | DASH | SWAP, 9, 8)],
    sounds: &[(0, SPIN, 0.35)],
    fx: &[(0, FX_SPARKLE), (1, FX_SPIN)],
    air: true,
    land_ends: true,
    ..BASE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::*;
    use crate::swordplay::{CHARGE_AFTER, JUST_FROM, JUST_TO, SPIN_LEVELS};

    fn sword(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn every_sword_id_has_its_move() {
        let want: [(u16, &str); 32] = [
            (SW_S1, "star slash"),
            (SW_S2, "star backhand"),
            (SW_S3, "star rising slash"),
            (SW_S4, "star spin finisher"),
            (SW_WHIRL, "star whirl"),
            (SW_CLEAVE, "thunder cleave"),
            (SW_THRUST, "Bullet Bill thrust"),
            (SW_STABS, "thousand stabs"),
            (SW_RISE, "rising cut"),
            (SW_SPIN_JUMP, "spin jump"),
            (SW_WAVE_CHARGE, "star wave charge"),
            (SW_WAVE, "star wave"),
            (SW_WAVE_JUST, "star wave JUST"),
            (SW_SPIN, "star spin"),
            (SW_SPIN_CHARGE, "Great Spin charge"),
            (SW_GREAT_SPIN, "Great Spin"),
            (SW_GREAT_SPIN + 1, "Great Spin 2"),
            (SW_GREAT_SPIN + 2, "Great Spin 3"),
            (SW_HURRICANE, "Hurricane"),
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
        ];
        // (the other weapons' moves come after the sword's)
        assert!(MOVES.len() > SW_DRAW_AIR as usize);
        for (id, name) in want {
            assert_eq!(sword(id).name, name, "id {id}");
            assert!(is_sword(id));
        }
        assert!(!is_sword(BULLET_PUNCH) && !is_sword(SKY_STRIKE) && !is_sword(ZEUS_THROW));
    }

    #[test]
    fn the_gleams_tell_the_charges_moments() {
        // (a charge starts with the button held CHARGE_AFTER ticks: its tick t is t + CHARGE_AFTER held)
        let gleams = |id: u16| sword(id).fx.iter().filter(|f| f.1 & FX_SPARKLE != 0).map(|f| f.0 as u32 + CHARGE_AFTER).collect::<Vec<u32>>();
        let wave = gleams(SW_WAVE_CHARGE);
        assert_eq!(wave.first(), Some(&JUST_FROM));
        assert!(wave.iter().all(|t| (JUST_FROM..=JUST_TO).contains(t)), "{wave:?}");
        assert_eq!(gleams(SW_SPIN_CHARGE), SPIN_LEVELS.to_vec());
        // (and their chimes with them)
        let chime = |id: u16, t: u32| sword(id).sounds.iter().any(|s| s.0 as u32 + CHARGE_AFTER == t && s.1 == SHORT_STAR);
        assert!(chime(SW_WAVE_CHARGE, JUST_FROM));
        assert!(SPIN_LEVELS.iter().all(|&t| chime(SW_SPIN_CHARGE, t)));
    }

    #[test]
    fn the_sword_hits_with_its_own_blows() {
        let sword_blow = |a: Attack| {
            matches!(
                a,
                Attack::SwordSlash(_)
                    | Attack::SwordWhirl
                    | Attack::SwordWhirlEnd
                    | Attack::SwordCleave
                    | Attack::SwordThrust
                    | Attack::SwordStab
                    | Attack::SwordStabEnd
                    | Attack::SwordRise
                    | Attack::SwordSpinJump
                    | Attack::SwordSpinJumpEnd
                    | Attack::SwordSpin
                    | Attack::SwordGreatSpin(_)
                    | Attack::SwordHurricane
                    | Attack::SwordHurricaneEnd
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
        assert_eq!(count(SW_WHIRL, Attack::SwordWhirl), 3);
        assert_eq!(count(SW_SPIN_JUMP, Attack::SwordSpinJump), 4);
        assert_eq!(count(SW_HURRICANE, Attack::SwordHurricane), 6);
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
        // the Great Spin wider by level
        let radius = |id: u16| match sword(id).windows[0].shape {
            Shape::Burst { radius } => radius,
            _ => 0.0,
        };
        assert!(radius(SW_GREAT_SPIN) < radius(SW_GREAT_SPIN + 1) && radius(SW_GREAT_SPIN + 1) < radius(SW_GREAT_SPIN + 2));
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
        for id in [SW_S1, SW_SPIN_JUMP, SW_HELM, SW_RISE, SW_LAUNCHER, SW_DRAW] {
            assert!(!sword(id).air, "{}", sword(id).name);
        }
        // the pogo bounces, the rising cut follows its enemy up with X held
        assert!(sword(SW_POGO).bounce > 0.0);
        assert!(sword(SW_RISE).held_rise.is_some());
        assert!(sword(SW_HURRICANE).steer);
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
