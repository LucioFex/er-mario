//! The boomerang's new moves as data (v4 Part 3): script.rs plays them (MOVES from id RANG_SKIM
//! on), weaponplay.rs picks them. The boomerang stays the ranged weapon: a skimming throw Mario runs
//! after, two curving round the enemy from both sides, one steered with the stick. Each throw is
//! the frame's FX_RANG (lib.rs casts the move's kind with boomerang.rs).

use crate::moves::FX_LAUNCH;
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, ALL, BASE, DASH, FX_RANG, JUMP, SWAP};

const FIRST_PUNCH: i32 = 0x67;
const FIRST_PUNCH_FAST: i32 = 0x69;
const SECOND_PUNCH_FAST: i32 = 0x6A;
const THROW_LIGHT_OBJECT: i32 = 0x52;
const FAST_LONGJUMP: i32 = 0x13;
const GENERAL_LAND: i32 = 0x57;

/// (SM64's: the soft ones at that volume, the voice at his body)
const SWISH: u32 = 0x305C_0081;
const YAH: u32 = 0x241E_8081;
const HOO: u32 = 0x241F_8081;
const UH: u32 = 0x2405_8081;

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

/// back, forward + X: thrown low and straight, skimming the ground, and Mario dashing after it up
/// to the enemy; it comes back to his hand there
pub const SKIM: Move = Move {
    name: "boomerang skim",
    len: 28,
    clips: &[still(0, FIRST_PUNCH), clip(2, SECOND_PUNCH_FAST), clip(8, FAST_LONGJUMP), clip(19, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(2, -30.0, Out), k(5, 25.0, Out), k(10, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(2, 10.0, Out), k(5, 4.0, Out), k(9, 16.0, Out), k(18, 10.0, Linear), k(24, 0.0, InOut)],
    aim: &[0, 3],
    approach: Some(Approach { from: 8, to: 18, max: 8.0, gap: 1.6 }),
    squash: &[k(0, 1.0, Linear), k(2, 0.9, Out), k(5, 1.05, Out), k(9, 1.1, Out), k(18, 1.0, InOut), k(20, 0.92, Out), k(25, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(2, 12.0, Out), k(6, 4.0, Out), k(9, 0.0, Out)],
    reach: &[k(0, 0.4, Linear), k(2, 0.3, Out), k(4, 1.0, Out), k(8, 0.6, InOut), k(20, 0.8, InOut), k(26, 0.3, InOut)],
    reach_pitch: -10.0,
    cancels: &[c(DASH | SWAP, 8, 8), c(ALL, 20, 18)],
    sounds: &[(0, UH, 0.4), (3, SWISH, 0.4), (8, HOO, 0.0)],
    fx: &[(4, FX_RANG), (8, FX_LAUNCH)],
    locked: (0, 4),
    ..BASE
};

/// X, a beat, X: two boomerangs at once, one from each hand, curving out opposite ways and
/// closing on the enemy from both sides
pub const PINCER: Move = Move {
    name: "boomerang pincer",
    len: 22,
    clips: &[still(0, FIRST_PUNCH), clip(3, THROW_LIGHT_OBJECT)],
    twist: &[k(0, 0.0, Linear), k(3, -20.0, Out), k(5, 20.0, Out), k(12, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(3, -6.0, Out), k(5, 10.0, Out), k(12, 0.0, InOut)],
    aim: &[0, 3],
    squash: &[k(0, 1.0, Linear), k(3, 0.92, Out), k(5, 1.08, Out), k(11, 1.0, InOut)],
    reach: &[k(0, 0.4, Linear), k(3, 0.3, Out), k(5, 1.0, Out), k(12, 1.0, Linear), k(20, 0.3, InOut)],
    reach_pitch: 15.0,
    cancels: &[c(ALL, 12, 12)],
    sounds: &[(3, SWISH, 0.4), (4, YAH, 0.0)],
    fx: &[(5, FX_RANG)],
    locked: (0, 5),
    ..BASE
};

/// X held: one thrown out and steered with the stick for two seconds, his arm out after it; X let
/// go (or the time up), it comes back
pub const STEER: Move = Move {
    name: "steered boomerang",
    len: 66,
    clips: &[still(0, FIRST_PUNCH), clip(2, FIRST_PUNCH_FAST), still(8, FIRST_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(2, -20.0, Out), k(5, 15.0, Out), k(10, 5.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(5, 6.0, Out)],
    squash: &[k(0, 1.0, Linear), k(2, 0.94, Out), k(5, 1.04, Out), k(10, 1.0, InOut)],
    reach: &[k(0, 0.4, Linear), k(4, 1.0, Out), k(60, 1.0, Linear), k(65, 0.5, InOut)],
    reach_pitch: 10.0,
    cancels: &[c(DASH | JUMP | SWAP, 8, 8)],
    sounds: &[(2, SWISH, 0.4), (3, YAH, 0.0)],
    fx: &[(4, FX_RANG)],
    locked: (0, 4),
    ..BASE
};

/// The steered boomerang let go (X up): his hand out for it as it comes back
pub const CATCH: Move = Move {
    name: "boomerang let go",
    len: 12,
    clips: &[still(0, FIRST_PUNCH)],
    twist: &[k(0, 5.0, Linear), k(6, 0.0, InOut)],
    reach: &[k(0, 1.0, Linear), k(10, 0.3, InOut)],
    reach_pitch: 10.0,
    cancels: &[c(ALL, 2, 2)],
    ..BASE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::*;

    fn rang(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn the_boomerangs_moves_sit_at_their_ids() {
        assert_eq!(rang(RANG_SKIM).name, "boomerang skim");
        assert_eq!(rang(RANG_PINCER).name, "boomerang pincer");
        assert_eq!(rang(RANG_STEER).name, "steered boomerang");
        assert_eq!(rang(RANG_CATCH).name, "boomerang let go");
    }

    #[test]
    fn each_throws_once() {
        for id in [RANG_SKIM, RANG_PINCER, RANG_STEER] {
            assert_eq!(rang(id).fx.iter().filter(|f| f.1 & FX_RANG != 0).count(), 1, "{}", rang(id).name);
        }
    }
}
