//! RB with the hammer, the Chain Chomp and the boomerang (v4 Part 3c): each element its own move,
//! never a projectile in another colour. Script.rs plays them (MOVES from id ICE_ROCKET on);
//! `cast_move` picks the one a charge asks for (fireball.rs holds the charge, its move's
//! FX_RELEASE lets the element's effect go: the Glacier fist, the dark beam, the poison carpet).
//! - frost (hammer): the ice rocket punch (the fist in an ice block, a lunge into the enemy), the
//!   ice dive in the air, the 10 s Glacier fist
//! - blood (Chain Chomp): the dark beam (beam.rs), held up in the air, the 10 s Abyss beam steered
//!   with the stick
//! - poison (boomerang): the arm swept spraying a carpet of poison (carpet.rs), sprayed down in the
//!   air, the 10 s toxic swamp

use crate::combat::Attack;
use crate::element::Element;
use crate::moves::{FX_BANG, FX_CHARGE, FX_GROUND, FX_LAUNCH, FX_SLAM, FX_SPIN};
use crate::script::Ease::*;
use crate::script::*;

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const DIVE: i32 = 0x88;
const SLOW_LAND_FROM_DIVE: i32 = 0x5A;
const GENERAL_LAND: i32 = 0x57;
const SWINGING_BOWSER: i32 = 0xB6;

/// (SM64's: the soft ones at that volume, the voice at his body)
const SWISH: u32 = 0x305C_0081;
const SPIN: u32 = 0x0437_8081;
const UH: u32 = 0x2405_8081;
const WAH: u32 = 0x2424_8081;
const HOO: u32 = 0x241F_8081;
const HOOHOO: u32 = 0x2403_8081;
const SHORT_STAR: u32 = 0x3016_0091;

/// the dark beam's length by charge (ticks: 0.4 to 1.2 s), the Abyss beam's (2.5 s); its moves
/// let it go at BEAM_FROM
pub const BEAM_TICKS: [u16; 4] = [12, 20, 28, 36];
pub const ABYSS_TICKS: u16 = 75;
pub const BEAM_FROM: u16 = 6;
/// the arm sweep's arc by charge (degrees)
pub const SWEEP_SPAN: [f32; 4] = [120.0, 180.0, 270.0, 360.0];

const fn w(from: u16, to: u16, shape: Shape, attack: Attack, fx: u32) -> Window {
    Window { from, to, shape, attack, fx, ground: false }
}

const fn wg(from: u16, to: u16, shape: Shape, attack: Attack, fx: u32) -> Window {
    Window { from, to, shape, attack, fx, ground: true }
}

const fn c(into: u8, from: u16, on_hit: u16) -> Cancel {
    Cancel { into, from, on_hit }
}

const fn clip(from: u16, anim: i32) -> Clip {
    Clip { from, anim, frames: &[] }
}

const FIRST_FRAME: &[Key] = &[k(0, 0.0, Linear)];
const fn still(from: u16, anim: i32) -> Clip {
    Clip { from, anim, frames: FIRST_FRAME }
}

const ROCKET_REACH: Shape = Shape::Front { reach: 190.0 };

// ---- frost (the hammer) ---------------------------------------------------------------------

/// The ice rocket punch (Overwatch's Doomfist): the fist drawn back while the ice grows on it, then
/// he rockets fist first into the enemy (6 / 8 / 11 / 14 m by charge) and lands out of it.
const ROCKET: Move = Move {
    name: "ice rocket punch",
    len: 30,
    clips: &[
        Clip { from: 0, anim: FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(5, 1.0, In)] },
        Clip { from: 6, anim: DIVE, frames: &[k(6, 0.0, Linear), k(10, 5.0, Out), k(14, 6.0, Linear)] },
        Clip { from: 15, anim: SLOW_LAND_FROM_DIVE, frames: &[k(15, 0.0, Linear), k(29, 14.0, Out)] },
    ],
    twist: &[k(0, 0.0, Linear), k(5, -35.0, Out), k(8, 0.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(5, -8.0, InOut), k(9, 6.0, Out), k(15, 0.0, InOut)],
    aim: &[0, 5],
    speed: &[k(0, 0.0, Linear), k(14, 0.0, Linear), k(15, 3.0, Linear), k(21, 0.0, Out)],
    approach: Some(Approach { from: 6, to: 14, max: 6.0, gap: 0.8 }),
    vel_y: &[(6, 12.0)],
    squash: &[k(0, 1.0, Linear), k(5, 0.84, Out), k(7, 1.14, Out), k(12, 1.0, InOut), k(15, 0.9, Out), k(19, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(5, 10.0, Out), k(7, 0.0, Out), k(15, 6.0, Out), k(20, 0.0, InOut)],
    reach: &[k(0, 0.4, Linear), k(5, 0.2, Out), k(8, 1.0, Out), k(15, 1.0, Linear), k(21, 0.0, InOut)],
    windows: &[w(8, 15, ROCKET_REACH, Attack::IceRocket(0), 0)],
    cancels: &[c(ALL, 22, 16)],
    sounds: &[(0, UH, 0.45), (6, WAH, 0.0), (7, SWISH, 0.45)],
    fx: &[(0, FX_RELEASE), (6, FX_LAUNCH)],
    locked: (0, 6),
    ..BASE
};

pub const ICE_ROCKETS: [Move; 4] = [
    ROCKET,
    Move { name: "ice rocket punch (charged)", approach: Some(Approach { from: 6, to: 14, max: 8.0, gap: 0.8 }), windows: &[w(8, 15, ROCKET_REACH, Attack::IceRocket(1), 0)], ..ROCKET },
    Move { name: "ice rocket punch (charged 2)", approach: Some(Approach { from: 6, to: 14, max: 11.0, gap: 0.8 }), windows: &[w(8, 15, ROCKET_REACH, Attack::IceRocket(2), 0)], ..ROCKET },
    Move { name: "ice rocket punch (full)", approach: Some(Approach { from: 6, to: 14, max: 14.0, gap: 0.8 }), windows: &[w(8, 15, ROCKET_REACH, Attack::IceRocket(3), FX_BANG)], ..ROCKET },
];

/// RB in the air with the hammer: a moment's hang, a diving punch down at the enemy in its ice
/// block, a wave of frost where he lands
pub const ICE_DIVE_MOVE: Move = Move {
    name: "ice dive",
    len: 34,
    clips: &[still(0, FIRST_PUNCH), clip(5, DIVE), clip(12, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(4, -25.0, Out), k(8, 0.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(4, -10.0, Out), k(8, 30.0, In), k(12, 20.0, Linear), k(20, 0.0, InOut)],
    aim: &[0, 4],
    approach: Some(Approach { from: 4, to: 12, max: 5.0, gap: 0.8 }),
    vel_y: &[(0, 6.0), (5, -55.0)],
    hold: (0, 5),
    squash: &[k(0, 1.0, Linear), k(4, 1.08, Out), k(8, 1.12, Linear), k(12, 0.8, Out), k(20, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(12, 14.0, Out), k(24, 0.0, InOut)],
    reach: &[k(0, 0.4, Linear), k(4, 0.3, Out), k(7, 1.0, Out), k(14, 1.0, Linear), k(22, 0.2, InOut)],
    reach_pitch: -35.0,
    windows: &[
        w(5, 9, ROCKET_REACH, Attack::IceDive, 0),
        wg(10, 33, Shape::Around { radius: 230.0, ahead: 60.0 }, Attack::FrostWave, FX_SLAM | FX_GROUND),
    ],
    cancels: &[c(ALL, 26, 20)],
    sounds: &[(0, SWISH, 0.25), (4, HOOHOO, 0.0), (5, SWISH, 0.45)],
    fx: &[(0, FX_RELEASE)],
    locked: (0, 12),
    air: true,
    ..BASE
};

/// The Glacier fist (10 s): both fists drawn back, the cold gathering round him, then thrust out:
/// a huge fist of ice flies 25 m through everyone in its way (fireball.rs), a ring of ice spikes
/// bursting from the ground where it stops
pub const GLACIER_MOVE: Move = Move {
    name: "Glacier fist",
    len: 42,
    clips: &[still(0, FIRST_PUNCH), clip(15, SECOND_PUNCH), clip(30, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(8, -40.0, Out), k(14, -45.0, Linear), k(17, 30.0, Out), k(30, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(8, -10.0, Out), k(14, -12.0, Linear), k(17, 14.0, Out), k(30, 0.0, InOut)],
    aim: &[0, 14],
    squash: &[k(0, 1.0, Linear), k(8, 0.88, Out), k(14, 0.86, Linear), k(17, 1.16, Out), k(26, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(8, 14.0, Out), k(14, 16.0, Linear), k(17, 0.0, Out)],
    reach: &[k(0, 0.4, Linear), k(8, 0.15, Out), k(14, 0.15, Linear), k(17, 1.0, Out), k(28, 1.0, Linear), k(36, 0.2, InOut)],
    cancels: &[c(ALL, 30, 30)],
    sounds: &[(0, UH, 0.45), (8, SHORT_STAR, 0.22), (15, WAH, 0.0), (16, SWISH, 0.5)],
    fx: &[(16, FX_RELEASE | FX_BANG)],
    pulse: Some((5, FX_CHARGE)),
    locked: (0, 18),
    ..BASE
};

pub const GLACIER_AIR_MOVE: Move = Move { name: "Glacier fist (air)", vel_y: &[(0, 0.0)], hold: (0, 26), air: true, ..GLACIER_MOVE };

// ---- blood (the Chain Chomp) ----------------------------------------------------------------

/// The dark beam (a small, evil kamehameha): cupped hands drawn to his side, then thrust out, the
/// beam pouring from them (beam.rs) for as long as its charge allows; he stands (or hangs) braced.
const BEAM: Move = Move {
    name: "dark beam",
    len: 6 + 12 + 10,
    clips: &[still(0, FIRST_PUNCH), still(BEAM_FROM, SECOND_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(5, -35.0, Out), k(7, 5.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(5, -6.0, Out), k(7, 8.0, Out)],
    aim: &[0, 5],
    squash: &[k(0, 1.0, Linear), k(5, 0.9, Out), k(7, 1.06, Out), k(12, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(5, 10.0, Out), k(8, 6.0, Out)],
    reach: &[k(0, 0.4, Linear), k(5, 0.2, Out), k(7, 1.0, Out)],
    cancels: &[c(DASH, 10, 10), c(ALL, 6 + 12 + 4, 6 + 12 + 4)],
    sounds: &[(0, SWISH, 0.25), (5, HOO, 0.0)],
    fx: &[(BEAM_FROM, FX_RELEASE)],
    locked: (0, BEAM_FROM),
    ..BASE
};

pub const DARK_BEAMS: [Move; 4] = [
    BEAM,
    Move { name: "dark beam (charged)", len: 6 + 20 + 10, cancels: &[c(DASH, 10, 10), c(ALL, 6 + 20 + 4, 6 + 20 + 4)], ..BEAM },
    Move { name: "dark beam (charged 2)", len: 6 + 28 + 10, cancels: &[c(DASH, 10, 10), c(ALL, 6 + 28 + 4, 6 + 28 + 4)], ..BEAM },
    Move { name: "dark beam (full)", len: 6 + 36 + 10, cancels: &[c(DASH, 10, 10), c(ALL, 6 + 36 + 4, 6 + 36 + 4)], ..BEAM },
];

pub const DARK_BEAMS_AIR: [Move; 4] = [
    Move { name: "dark beam (air)", vel_y: &[(0, 0.0)], hold: (0, 6 + 12), air: true, ..DARK_BEAMS[0] },
    Move { name: "dark beam (air, charged)", vel_y: &[(0, 0.0)], hold: (0, 6 + 20), air: true, ..DARK_BEAMS[1] },
    Move { name: "dark beam (air, charged 2)", vel_y: &[(0, 0.0)], hold: (0, 6 + 28), air: true, ..DARK_BEAMS[2] },
    Move { name: "dark beam (air, full)", vel_y: &[(0, 0.0)], hold: (0, 6 + 36), air: true, ..DARK_BEAMS[3] },
];

/// The Abyss beam (10 s): a huge beam for two and a half seconds, swept round by the stick
pub const ABYSS_MOVE: Move = Move {
    name: "Abyss beam",
    len: 6 + ABYSS_TICKS + 12,
    cancels: &[c(DASH, 20, 20), c(ALL, 6 + ABYSS_TICKS + 4, 6 + ABYSS_TICKS + 4)],
    sounds: &[(0, SWISH, 0.25), (5, HOOHOO, 0.0)],
    squash: &[k(0, 1.0, Linear), k(5, 0.86, Out), k(7, 1.08, Out), k(14, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(5, 14.0, Out), k(8, 10.0, Out)],
    steer: true,
    ..BEAM
};

pub const ABYSS_AIR_MOVE: Move = Move { name: "Abyss beam (air)", vel_y: &[(0, 0.0)], hold: (0, 6 + ABYSS_TICKS), air: true, ..ABYSS_MOVE };

// ---- poison (the boomerang) -----------------------------------------------------------------

/// The arm sweep: his arm swung out from his left round to his right, spraying poison over the
/// ground in front of him (carpet.rs: its arc by the charge)
const SWEEP: Move = Move {
    name: "poison sweep",
    len: 24,
    clips: &[still(0, FIRST_PUNCH), clip(4, SECOND_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(4, 60.0, Out), k(12, -60.0, Linear), k(20, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(4, 6.0, Out), k(12, 8.0, Linear), k(20, 0.0, InOut)],
    aim: &[0],
    reach: &[k(0, 0.5, Linear), k(4, 1.0, Out), k(14, 1.0, Linear), k(22, 0.3, InOut)],
    reach_pitch: -25.0,
    windows: &[w(5, 12, Shape::Arc { radius: 300.0, half_deg: 60.0, ahead: 0.0 }, Attack::Spray, 0)],
    cancels: &[c(DASH | SWAP, 13, 13), c(ALL, 18, 16)],
    sounds: &[(3, SWISH, 0.35), (4, WAH, 0.0), (8, SWISH, 0.25)],
    fx: &[(4, FX_RELEASE)],
    locked: (0, 4),
    ..BASE
};

pub const POISON_SWEEPS: [Move; 4] = [
    SWEEP,
    Move {
        name: "poison sweep (charged)",
        twist: &[k(0, 0.0, Linear), k(4, 90.0, Out), k(13, -90.0, Linear), k(21, 0.0, InOut)],
        windows: &[w(5, 13, Shape::Arc { radius: 400.0, half_deg: 90.0, ahead: 0.0 }, Attack::Spray, 0)],
        ..SWEEP
    },
    Move {
        name: "poison sweep (charged 2)",
        len: 28,
        twist: &[k(0, 0.0, Linear), k(4, 135.0, Out), k(15, -135.0, Linear), k(24, 0.0, InOut)],
        windows: &[w(5, 15, Shape::Arc { radius: 500.0, half_deg: 135.0, ahead: 0.0 }, Attack::Spray, 0)],
        cancels: &[c(DASH | SWAP, 16, 16), c(ALL, 22, 18)],
        ..SWEEP
    },
    Move {
        name: "poison sweep (full)",
        len: 30,
        clips: &[still(0, FIRST_PUNCH), clip(4, SWINGING_BOWSER)],
        twist: &[k(0, 0.0, Linear), k(16, -340.0, In), k(26, -360.0, Out)],
        windows: &[w(5, 16, Shape::Around { radius: 600.0, ahead: 0.0 }, Attack::Spray, 0)],
        cancels: &[c(DASH | SWAP, 17, 17), c(ALL, 24, 20)],
        sounds: &[(3, SPIN, 0.35), (4, WAH, 0.0), (10, SWISH, 0.25)],
        ..SWEEP
    },
];

/// RB in the air with the boomerang: a moment's hang, poison sprayed down: a pool where it lands
pub const POISON_AIR_MOVE: Move = Move {
    name: "poison spray (air)",
    len: 20,
    clips: &[still(0, FIRST_PUNCH)],
    pitch: &[k(0, 0.0, Linear), k(4, 25.0, Out), k(14, 25.0, Linear), k(19, 0.0, InOut)],
    vel_y: &[(0, 0.0)],
    hold: (0, 12),
    reach: &[k(0, 0.5, Linear), k(4, 1.0, Out), k(14, 1.0, Linear), k(19, 0.3, InOut)],
    reach_pitch: -60.0,
    cancels: &[c(ALL, 14, 14)],
    sounds: &[(2, SWISH, 0.3), (4, HOO, 0.0)],
    fx: &[(6, FX_RELEASE)],
    air: true,
    land_ends: true,
    ..BASE
};

/// The toxic swamp (10 s): two turns spraying all round him, a swamp 10 m wide for twelve seconds
pub const SWAMP_MOVE: Move = Move {
    name: "toxic swamp",
    len: 48,
    clips: &[still(0, FIRST_PUNCH), clip(6, SWINGING_BOWSER), clip(38, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(6, 30.0, Out), k(36, -690.0, Linear), k(44, -720.0, InOut)],
    reach: &[k(0, 0.5, Linear), k(6, 1.0, Out), k(36, 1.0, Linear), k(44, 0.3, InOut)],
    reach_pitch: -20.0,
    windows: &[w(8, 30, Shape::Around { radius: 420.0, ahead: 0.0 }, Attack::Spray, 0)],
    cancels: &[c(ALL, 40, 40)],
    sounds: &[(4, HOOHOO, 0.0), (6, SPIN, 0.35), (18, SPIN, 0.3), (30, SPIN, 0.3)],
    fx: &[(8, FX_RELEASE), (8, FX_SPIN), (20, FX_SPIN)],
    locked: (0, 36),
    ..BASE
};

/// The move a charge of `el` at `level` (4: the 10 s giant) asks for, in the air or not (None:
/// the fists' fire throws its shots, fireball.rs).
pub fn cast_move(el: Element, level: u8, air: bool) -> Option<u16> {
    let l = level.min(3) as u16;
    let giant = level >= 4;
    Some(match el {
        Element::Fire => return None,
        Element::Lightning => {
            if air {
                ZEUS_THROW
            } else {
                SKY_STRIKE
            }
        }
        Element::Frost => match (giant, air) {
            (true, false) => GLACIER,
            (true, true) => GLACIER_AIR,
            (false, true) => ICE_DIVE,
            (false, false) => ICE_ROCKET + l,
        },
        Element::Blood => match (giant, air) {
            (true, false) => ABYSS,
            (true, true) => ABYSS_AIR,
            (false, true) => DARK_BEAM_AIR + l,
            (false, false) => DARK_BEAM + l,
        },
        Element::Poison => match (giant, air) {
            (_, true) => POISON_AIR,
            (true, false) => SWAMP,
            (false, false) => POISON_SWEEP + l,
        },
    })
}

/// How long a dark beam move's beam lasts (ticks), None if it isn't one.
pub fn beam_ticks(id: u16) -> Option<u16> {
    match id {
        ABYSS | ABYSS_AIR => Some(ABYSS_TICKS),
        _ if (DARK_BEAM..DARK_BEAM + 4).contains(&id) => Some(BEAM_TICKS[(id - DARK_BEAM) as usize]),
        _ if (DARK_BEAM_AIR..DARK_BEAM_AIR + 4).contains(&id) => Some(BEAM_TICKS[(id - DARK_BEAM_AIR) as usize]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rb(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn each_charge_its_elements_move() {
        assert_eq!(cast_move(Element::Fire, 2, false), None);
        assert_eq!(cast_move(Element::Lightning, 1, false), Some(SKY_STRIKE));
        assert_eq!(cast_move(Element::Lightning, 4, true), Some(ZEUS_THROW));
        for l in 0..4u8 {
            assert_eq!(cast_move(Element::Frost, l, false), Some(ICE_ROCKET + l as u16));
            assert_eq!(cast_move(Element::Blood, l, false), Some(DARK_BEAM + l as u16));
            assert_eq!(cast_move(Element::Blood, l, true), Some(DARK_BEAM_AIR + l as u16));
            assert_eq!(cast_move(Element::Poison, l, false), Some(POISON_SWEEP + l as u16));
        }
        assert_eq!(cast_move(Element::Frost, 1, true), Some(ICE_DIVE));
        assert_eq!(cast_move(Element::Frost, 4, false), Some(GLACIER));
        assert_eq!(cast_move(Element::Frost, 4, true), Some(GLACIER_AIR));
        assert_eq!(cast_move(Element::Blood, 4, false), Some(ABYSS));
        assert_eq!(cast_move(Element::Blood, 4, true), Some(ABYSS_AIR));
        assert_eq!(cast_move(Element::Poison, 4, false), Some(SWAMP));
        assert_eq!(cast_move(Element::Poison, 2, true), Some(POISON_AIR));
    }

    #[test]
    fn every_charge_has_a_move_of_its_place() {
        // (in the air an air move, on the ground a ground one: SM64's action follows the move)
        for el in [Element::Lightning, Element::Frost, Element::Blood, Element::Poison] {
            for level in 0..=4u8 {
                for air in [false, true] {
                    let id = cast_move(el, level, air).expect("a move");
                    assert_eq!(MOVES[id as usize].air, air, "{el:?} level {level} air {air}: {}", MOVES[id as usize].name);
                }
            }
        }
    }

    #[test]
    fn the_moves_sit_at_their_ids() {
        assert_eq!(rb(ICE_ROCKET).name, "ice rocket punch");
        assert_eq!(rb(ICE_ROCKET + 3).name, "ice rocket punch (full)");
        assert_eq!(rb(ICE_DIVE).name, "ice dive");
        assert_eq!(rb(GLACIER).name, "Glacier fist");
        assert_eq!(rb(GLACIER_AIR).name, "Glacier fist (air)");
        assert_eq!(rb(DARK_BEAM + 2).name, "dark beam (charged 2)");
        assert_eq!(rb(DARK_BEAM_AIR + 3).name, "dark beam (air, full)");
        assert_eq!(rb(ABYSS).name, "Abyss beam");
        assert_eq!(rb(ABYSS_AIR).name, "Abyss beam (air)");
        assert_eq!(rb(POISON_SWEEP + 3).name, "poison sweep (full)");
        assert_eq!(rb(POISON_AIR).name, "poison spray (air)");
        assert_eq!(rb(SWAMP).name, "toxic swamp");
    }

    #[test]
    fn every_cast_lets_its_charge_go() {
        // (the charge waits for its move's FX_RELEASE: one that never lets go loses it)
        for id in ICE_ROCKET..=SWAMP {
            assert_eq!(rb(id).fx.iter().filter(|f| f.1 & FX_RELEASE != 0).count(), 1, "{}", rb(id).name);
        }
    }

    #[test]
    fn the_beams_last_their_charge() {
        for l in 0..4u16 {
            let (ground, air) = (rb(DARK_BEAM + l), rb(DARK_BEAM_AIR + l));
            let t = BEAM_TICKS[l as usize];
            assert_eq!(beam_ticks(DARK_BEAM + l), Some(t));
            assert_eq!(beam_ticks(DARK_BEAM_AIR + l), Some(t));
            assert!(ground.len > BEAM_FROM + t && air.len == ground.len, "{}", ground.name);
            // (held up in the air while it pours)
            assert!(air.air && air.hold.1 >= BEAM_FROM + t);
        }
        assert_eq!(beam_ticks(ABYSS), Some(ABYSS_TICKS));
        assert!(rb(ABYSS).steer && rb(ABYSS).len > BEAM_FROM + ABYSS_TICKS);
        assert_eq!(beam_ticks(ICE_ROCKET), None);
    }

    #[test]
    fn the_sweeps_spray_their_arcs() {
        for (l, span) in SWEEP_SPAN.iter().enumerate() {
            let m = rb(POISON_SWEEP + l as u16);
            let (lo, hi) = m.twist.iter().fold((f32::MAX, f32::MIN), |(lo, hi), k| (lo.min(k.v), hi.max(k.v)));
            assert!((hi - lo - span).abs() < 1.0, "{}: {}", m.name, hi - lo);
        }
    }

    #[test]
    fn any_landed_blow_can_be_dashed_out_of() {
        for id in ICE_ROCKET..=SWAMP {
            let m = rb(id);
            if m.windows.is_empty() {
                continue;
            }
            let dash = m.cancels.iter().filter(|c| c.into & DASH != 0).map(|c| c.on_hit.min(c.from)).min();
            assert!(dash.is_some_and(|t| t < m.len), "{}: no dash out", m.name);
        }
    }
}
