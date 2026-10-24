//! The fists' new moves as data (v4 Part 3): script.rs plays them (MOVES from id FIST_FLURRY on),
//! weaponplay.rs picks them from the buttons. Fists stay the balanced weapon: quick strings that
//! reach a heavier blow by a charge, a beat or a mash, and air blows by the stick's way.

use crate::combat::Attack;
use crate::moves::{FX_BANG, FX_LAUNCH};
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, Shape, Window, ALL, BASE, DASH, SWAP};

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const FIRST_PUNCH_FAST: i32 = 0x69;
const SECOND_PUNCH_FAST: i32 = 0x6A;
const SINGLE_JUMP: i32 = 0x4D;
const GENERAL_LAND: i32 = 0x57;
const SLIDE_KICK: i32 = 0x8C;
const AIR_KICK: i32 = 0x4F;

/// (SM64's: the soft ones at that volume, the voice at his body)
const SWISH: u32 = 0x305C_0081;
const YAH: u32 = 0x241E_8081;
const HOO: u32 = 0x241F_8081;
const WAH: u32 = 0x2424_8081;
const UH: u32 = 0x2405_8081;
const PUNCH_HIT: u32 = 0x0444_C081;

const fn w(from: u16, to: u16, shape: Shape, attack: Attack, fx: u32) -> Window {
    Window { from, to, shape, attack, fx, ground: false }
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

const fn jab(t: u16) -> Window {
    w(t, t, Shape::Front { reach: 150.0 }, Attack::FistFlurry, 0)
}

/// mashed X as the Bullet Bill punch lands: ten jabs from both fists in a second (Kenshiro's,
/// Jotaro's), an uppercut off a little hop that launches small and medium enemies
pub const FLURRY: Move = Move {
    name: "hundred-hand flurry",
    len: 44,
    clips: &[
        clip(0, FIRST_PUNCH_FAST),
        clip(3, SECOND_PUNCH_FAST),
        clip(6, FIRST_PUNCH_FAST),
        clip(9, SECOND_PUNCH_FAST),
        clip(12, FIRST_PUNCH_FAST),
        clip(15, SECOND_PUNCH_FAST),
        clip(18, FIRST_PUNCH_FAST),
        clip(21, SECOND_PUNCH_FAST),
        clip(24, FIRST_PUNCH_FAST),
        clip(27, SECOND_PUNCH_FAST),
        still(30, FIRST_PUNCH),
        clip(32, SINGLE_JUMP),
        clip(38, GENERAL_LAND),
    ],
    // (each jab: his shoulders thrown into it, one side then the other)
    twist: &[
        k(0, 0.0, Linear),
        k(1, -14.0, Out),
        k(4, 14.0, Out),
        k(7, -14.0, Out),
        k(10, 14.0, Out),
        k(13, -14.0, Out),
        k(16, 14.0, Out),
        k(19, -14.0, Out),
        k(22, 14.0, Out),
        k(25, -14.0, Out),
        k(28, 14.0, Out),
        k(31, -30.0, Out),
        k(33, 15.0, Out),
        k(40, 0.0, InOut),
    ],
    pitch: &[k(0, 0.0, Linear), k(2, 5.0, Out), k(29, 5.0, Linear), k(31, 10.0, Out), k(33, -14.0, Out), k(40, 0.0, InOut)],
    aim: &[0, 30],
    approach: Some(Approach { from: 0, to: 6, max: 0.6, gap: 0.8 }),
    vel_y: &[(32, 24.0)],
    squash: &[k(0, 1.0, Linear), k(30, 1.0, Linear), k(31, 0.86, Out), k(33, 1.14, Out), k(38, 0.9, Out), k(42, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(30, 0.0, Linear), k(31, 10.0, Out), k(33, 0.0, Out)],
    reach: &[
        k(0, 0.5, Linear),
        k(1, 1.0, Out),
        k(3, 0.5, In),
        k(4, 1.0, Out),
        k(6, 0.5, In),
        k(7, 1.0, Out),
        k(9, 0.5, In),
        k(10, 1.0, Out),
        k(12, 0.5, In),
        k(13, 1.0, Out),
        k(15, 0.5, In),
        k(16, 1.0, Out),
        k(18, 0.5, In),
        k(19, 1.0, Out),
        k(21, 0.5, In),
        k(22, 1.0, Out),
        k(24, 0.5, In),
        k(25, 1.0, Out),
        k(27, 0.5, In),
        k(28, 1.0, Out),
        k(31, 0.2, In),
        k(33, 1.0, Out),
        k(38, 0.0, InOut),
    ],
    reach_pitch: 20.0,
    windows: &[
        jab(1),
        jab(4),
        jab(7),
        jab(10),
        jab(13),
        jab(16),
        jab(19),
        jab(22),
        jab(25),
        jab(28),
        w(32, 34, Shape::Front { reach: 170.0 }, Attack::FistFlurryEnd, 0),
    ],
    cancels: &[c(DASH | SWAP, 10, 10), c(ALL, 40, 37)],
    sounds: &[(0, WAH, 0.0), (1, SWISH, 0.15), (7, SWISH, 0.15), (13, SWISH, 0.15), (19, SWISH, 0.15), (25, SWISH, 0.15), (31, HOO, 0.0), (32, SWISH, 0.4)],
    fx: &[(32, FX_LAUNCH)],
    locked: (0, 2),
    ..BASE
};

/// X held and let go (charge.rs): the bullet straight, the charged fist driven out as he shoots in
/// behind it, 3 m (sends them back)
pub const STRAIGHT: Move = Move {
    name: "bullet straight",
    len: 22,
    clips: &[
        Clip { from: 0, anim: FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(2, 1.0, In)] },
        Clip { from: 3, anim: SECOND_PUNCH, frames: &[k(3, 2.0, Linear), k(8, 5.0, Out)] },
        clip(12, GENERAL_LAND),
    ],
    twist: &[k(0, -40.0, Linear), k(3, 30.0, Out), k(12, 0.0, InOut)],
    pitch: &[k(0, -6.0, Linear), k(3, 16.0, Out), k(9, 10.0, Linear), k(14, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 1, to: 7, max: 3.0, gap: 0.7 }),
    squash: &[k(0, 0.88, Linear), k(3, 1.16, Out), k(9, 1.0, InOut)],
    dip: &[k(0, 12.0, Linear), k(3, 0.0, Out)],
    reach: &[k(0, 0.2, Linear), k(3, 1.0, Out), k(11, 1.0, Linear), k(18, 0.0, InOut)],
    windows: &[w(3, 8, Shape::Front { reach: 170.0 }, Attack::SuperPunch(0), 0)],
    cancels: &[c(DASH | SWAP, 9, 7), c(ALL, 14, 9)],
    sounds: &[(0, YAH, 0.0), (2, SWISH, 0.42)],
    fx: &[(2, FX_LAUNCH)],
    locked: (0, 3),
    ..BASE
};

/// ...let go full: the flaming straight, 6 m in, a bang (it flings them, splats them on a wall)
pub const STRAIGHT_FULL: Move = Move {
    name: "bullet straight (full)",
    len: 28,
    clips: &[
        Clip { from: 0, anim: FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(2, 1.0, In)] },
        Clip { from: 3, anim: SECOND_PUNCH, frames: &[k(3, 2.0, Linear), k(10, 5.0, Out)] },
        clip(15, GENERAL_LAND),
    ],
    twist: &[k(0, -45.0, Linear), k(3, 35.0, Out), k(16, 0.0, InOut)],
    pitch: &[k(0, -8.0, Linear), k(3, 20.0, Out), k(11, 12.0, Linear), k(16, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 1, to: 9, max: 6.0, gap: 0.7 }),
    squash: &[k(0, 0.86, Linear), k(3, 1.2, Out), k(11, 1.0, InOut)],
    dip: &[k(0, 14.0, Linear), k(3, 0.0, Out)],
    reach: &[k(0, 0.2, Linear), k(3, 1.0, Out), k(14, 1.0, Linear), k(22, 0.0, InOut)],
    windows: &[w(3, 10, Shape::Front { reach: 190.0 }, Attack::SuperPunch(1), FX_BANG)],
    cancels: &[c(DASH | SWAP, 12, 9), c(ALL, 18, 12)],
    sounds: &[(0, WAH, 0.0), (2, SWISH, 0.48), (4, PUNCH_HIT, 0.4)],
    fx: &[(2, FX_LAUNCH)],
    locked: (0, 4),
    ..BASE
};

/// X, a beat, X: a wide heavy hook from his right, his whole body turned into it
pub const HOOK: Move = Move {
    name: "heavy hook",
    len: 24,
    clips: &[still(0, SECOND_PUNCH), clip(5, SECOND_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(4, 40.0, Out), k(8, -40.0, Out), k(16, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(4, -4.0, Out), k(8, 8.0, Out), k(16, 0.0, InOut)],
    aim: &[0, 4],
    approach: Some(Approach { from: 0, to: 6, max: 1.2, gap: 0.8 }),
    squash: &[k(0, 1.0, Linear), k(4, 0.92, Out), k(8, 1.08, Out), k(14, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 8.0, Out), k(8, 0.0, Out)],
    reach: &[k(0, 0.5, Linear), k(4, 0.4, Out), k(7, 1.0, Out), k(12, 1.0, Linear), k(20, 0.0, InOut)],
    windows: &[w(6, 9, Shape::Arc { radius: 170.0, half_deg: 70.0, ahead: 40.0 }, Attack::HeavyHook, 0)],
    cancels: &[c(ALL, 16, 12)],
    sounds: &[(0, UH, 0.4), (5, WAH, 0.0), (6, SWISH, 0.42), (7, PUNCH_HIT, 0.35)],
    fx: &[(7, FX_BANG)],
    locked: (0, 7),
    ..BASE
};

/// forward + X in the air: a punch driven down and ahead that spikes a small enemy into the
/// ground (it bounces up off it: juggle.rs)
pub const METEOR: Move = Move {
    name: "meteor punch",
    len: 20,
    clips: &[still(0, FIRST_PUNCH), clip(3, FIRST_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(2, -25.0, Out), k(5, 15.0, Out), k(12, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(2, -10.0, Out), k(5, 28.0, Out), k(14, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 0, to: 4, max: 1.0, gap: 0.7 }),
    vel_y: &[(0, 0.0), (6, -20.0)],
    hold: (0, 6),
    squash: &[k(0, 1.0, Linear), k(2, 0.9, Out), k(5, 1.12, Out), k(10, 1.0, InOut)],
    reach: &[k(0, 0.4, Linear), k(2, 0.3, Out), k(5, 1.0, Out), k(10, 1.0, Linear), k(16, 0.3, InOut)],
    reach_pitch: -35.0,
    windows: &[w(3, 6, Shape::Front { reach: 170.0 }, Attack::MeteorPunch, 0)],
    cancels: &[c(ALL, 12, 8)],
    sounds: &[(0, SWISH, 0.25), (2, YAH, 0.0), (3, SWISH, 0.4)],
    fx: &[(4, FX_LAUNCH)],
    locked: (0, 5),
    air: true,
    land_ends: true,
    ..BASE
};

/// back + X in the air: both feet flat into the enemy, knocking it away (he springs back off it)
pub const DROPKICK: Move = Move {
    name: "dropkick",
    len: 24,
    clips: &[still(0, AIR_KICK), clip(3, SLIDE_KICK)],
    pitch: &[k(0, 0.0, Linear), k(3, -15.0, Out), k(10, -10.0, Linear), k(18, 0.0, InOut)],
    aim: &[0],
    speed: &[k(0, 0.0, Linear), k(3, 10.0, Out), k(8, 8.0, Linear), k(12, -4.0, Out), k(18, 0.0, InOut)],
    vel_y: &[(0, 8.0), (9, 18.0)],
    hold: (2, 9),
    squash: &[k(0, 1.0, Linear), k(3, 1.1, Out), k(9, 0.9, Out), k(14, 1.0, InOut)],
    windows: &[w(3, 8, Shape::Front { reach: 180.0 }, Attack::Dropkick, 0)],
    cancels: &[c(DASH | SWAP, 10, 9), c(ALL, 16, 12)],
    sounds: &[(0, HOO, 0.0), (3, SWISH, 0.4)],
    fx: &[(9, FX_BANG)],
    locked: (0, 6),
    air: true,
    land_ends: true,
    ..BASE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::*;

    fn fist(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn the_fists_moves_sit_at_their_ids() {
        assert_eq!(fist(FIST_FLURRY).name, "hundred-hand flurry");
        assert_eq!(fist(FIST_STRAIGHT).name, "bullet straight");
        assert_eq!(fist(FIST_STRAIGHT_FULL).name, "bullet straight (full)");
        assert_eq!(fist(HEAVY_HOOK).name, "heavy hook");
        assert_eq!(fist(METEOR_PUNCH).name, "meteor punch");
        assert_eq!(fist(FIST_DROPKICK).name, "dropkick");
    }

    #[test]
    fn the_flurry_is_ten_jabs_and_an_uppercut() {
        let m = fist(FIST_FLURRY);
        assert_eq!(m.windows.iter().filter(|w| w.attack == Attack::FistFlurry).count(), 10);
        assert_eq!(m.windows.iter().filter(|w| w.attack == Attack::FistFlurryEnd).count(), 1);
        for pair in m.windows.windows(2) {
            if pair[0].attack == pair[1].attack {
                assert!((pair[1].from - pair[0].from) as u32 >= pair[1].attack.cooldown());
            }
        }
    }

    /// The charge let go closes in: 3 m, 6 m full.
    #[test]
    fn the_straights_close_in() {
        assert_eq!(fist(FIST_STRAIGHT).approach.map(|a| a.max), Some(3.0));
        assert_eq!(fist(FIST_STRAIGHT_FULL).approach.map(|a| a.max), Some(6.0));
    }

    #[test]
    fn any_landed_fist_blow_can_be_dashed_out_of() {
        for id in FIST_FLURRY..=FIST_DROPKICK {
            let m = fist(id);
            if m.windows.is_empty() {
                continue;
            }
            let dash = m.cancels.iter().filter(|c| c.into & DASH != 0).map(|c| c.on_hit.min(c.from)).min();
            assert!(dash.is_some_and(|t| t < m.len), "{}: no dash out", m.name);
        }
    }

    #[test]
    fn the_air_ones_are_air_moves() {
        assert!(fist(METEOR_PUNCH).air && fist(FIST_DROPKICK).air);
        for id in [FIST_FLURRY, FIST_STRAIGHT, FIST_STRAIGHT_FULL, HEAVY_HOOK] {
            assert!(!fist(id).air, "{}", fist(id).name);
        }
    }
}
