//! The Chain Chomp's new moves as data (v4 Part 3): script.rs plays them (MOVES from id CHOMP_RUSH
//! on), weaponplay.rs picks them. The Chomp stays the fast, wide, lighter weapon: it lunges ahead
//! dragging Mario along, is whirled overhead like a lasso, and bites in a frenzy. Its place comes
//! from the frame's weapon pose (lib.rs): `blade_yaw` round his facing (+ left), `out` and `up` in
//! SM64 units from his feet, `bite` its mouth (0 shut, 1 wide).

use crate::combat::Attack;
use crate::moves::{FX_CHARGE, FX_LAUNCH, FX_SPIN};
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, Shape, Window, ALL, BASE, DASH, JUMP, SWAP};

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const SECOND_PUNCH_FAST: i32 = 0x6A;
const FAST_LONGJUMP: i32 = 0x13;
const GENERAL_LAND: i32 = 0x57;
const THROW_LIGHT_OBJECT: i32 = 0x52;

/// (SM64's: the soft ones at that volume, the voice at his body; the chain's clink, not verified)
const SWISH: u32 = 0x305C_0081;
const SPIN: u32 = 0x0437_8081;
const YAH: u32 = 0x241E_8081;
const WAH: u32 = 0x2424_8081;
const HOO: u32 = 0x241F_8081;
const CHOMP_BITE: u32 = 0x3039_0081;

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

const fn bite_at(t: u16) -> Window {
    w(t, t + 1, Shape::Weapon, Attack::Frenzy, 0)
}

/// back, forward + X: the Chomp flung ahead on its chain, Mario dragged along behind it up to 9 m,
/// its jaws on the first enemy in the way
pub const RUSH: Move = Move {
    name: "Chomp rush",
    len: 30,
    clips: &[still(0, FIRST_PUNCH), clip(5, FAST_LONGJUMP), clip(16, GENERAL_LAND)],
    twist: &[k(0, 0.0, Linear), k(4, -30.0, Out), k(7, 10.0, Out), k(14, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(4, -6.0, Out), k(7, 20.0, Out), k(14, 12.0, Linear), k(20, 0.0, InOut)],
    aim: &[0, 4],
    approach: Some(Approach { from: 6, to: 15, max: 9.0, gap: 1.4 }),
    squash: &[k(0, 1.0, Linear), k(4, 0.9, Out), k(7, 1.12, Out), k(15, 1.0, InOut), k(17, 0.92, Out), k(22, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 8.0, Out), k(7, 0.0, Out), k(17, 6.0, Out), k(22, 0.0, InOut)],
    reach: &[k(0, 0.5, Linear), k(4, 0.3, Out), k(7, 1.0, Out), k(16, 1.0, Linear), k(24, 0.4, InOut)],
    // (pulled back behind him, flung out ahead, its chain shortening as he catches up, back by him)
    blade_yaw: &[k(0, -120.0, Linear), k(4, -160.0, Out), k(8, 0.0, Out), k(18, 0.0, Linear), k(28, -90.0, InOut)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 30),
    out: &[k(0, 120.0, Linear), k(4, 130.0, Out), k(9, 620.0, Out), k(15, 170.0, InOut), k(22, 150.0, Linear), k(28, 120.0, InOut)],
    up: &[k(0, 30.0, Linear), k(4, 60.0, Out), k(9, 70.0, Out), k(15, 60.0, Linear), k(28, 30.0, InOut)],
    bite: &[k(0, 0.0, Linear), k(5, 0.2, Linear), k(8, 1.0, Out), k(11, 0.0, In), k(14, 0.3, Out), k(20, 0.0, InOut)],
    windows: &[w(6, 14, Shape::Weapon, Attack::ChompRush, 0)],
    cancels: &[c(ALL, 22, 16)],
    sounds: &[(0, SWISH, 0.25), (5, YAH, 0.0), (6, SWISH, 0.45), (11, CHOMP_BITE, 0.35)],
    fx: &[(6, FX_LAUNCH)],
    locked: (0, 6),
    ..BASE
};

/// X held: the Chomp whirled round over his head on its chain, faster and faster (a lasso)
pub const LASSO_CHARGE: Move = Move {
    name: "Chomp lasso charge",
    len: 300,
    clips: &[still(0, SECOND_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(6, -10.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(6, -6.0, Out)],
    squash: &[k(0, 1.0, Linear), k(6, 0.96, Out)],
    reach: &[k(0, 0.6, Linear), k(6, 1.0, Out)],
    reach_pitch: 82.0,
    blade_yaw: &[k(0, 0.0, Linear), k(12, 240.0, In), k(300, 7440.0, Linear)],
    // (his hand up over his head with the chain)
    blade_pitch: &[k(0, 20.0, Linear), k(8, 75.0, Out)],
    blade_held: (0, 300),
    out: &[k(0, 120.0, Linear), k(8, 150.0, Out)],
    up: &[k(0, 60.0, Linear), k(8, 235.0, Out)],
    bite: &[k(0, 0.0, Linear), k(8, 0.5, Out)],
    cancels: &[c(DASH | JUMP | SWAP, 4, 4)],
    sounds: &[(0, SWISH, 0.25), (8, SPIN, 0.22), (20, SPIN, 0.2), (32, SPIN, 0.2), (44, SPIN, 0.2)],
    pulse: Some((12, FX_CHARGE | FX_SPIN)),
    ..BASE
};

/// X let go: the whirled Chomp let fly straight ahead, 12 m on its chain; a small enemy it bites is
/// dragged in to him (combat.rs: juggle::pull)
pub const LASSO: Move = Move {
    name: "Chomp lasso",
    len: 28,
    clips: &[clip(0, THROW_LIGHT_OBJECT), clip(12, SECOND_PUNCH_FAST)],
    twist: &[k(0, -15.0, Linear), k(4, 20.0, Out), k(14, 0.0, InOut)],
    pitch: &[k(0, -6.0, Linear), k(4, 14.0, Out), k(14, 0.0, InOut)],
    aim: &[0],
    squash: &[k(0, 1.0, Linear), k(4, 1.08, Out), k(10, 1.0, InOut)],
    reach: &[k(0, 1.0, Linear), k(12, 1.0, Linear), k(20, 0.6, InOut)],
    reach_pitch: 30.0,
    blade_yaw: &[k(0, 0.0, Linear), k(20, 0.0, Linear), k(27, -90.0, InOut)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 28),
    out: &[k(0, 120.0, Linear), k(4, 160.0, Linear), k(10, 1200.0, Out), k(13, 1150.0, Linear), k(22, 160.0, InOut), k(27, 120.0, InOut)],
    up: &[k(0, 235.0, Linear), k(4, 200.0, Linear), k(10, 90.0, Out), k(22, 50.0, InOut), k(27, 30.0, InOut)],
    bite: &[k(0, 0.5, Linear), k(5, 1.0, Out), k(12, 0.0, In), k(20, 0.0, Linear)],
    windows: &[w(5, 12, Shape::Weapon, Attack::Lasso, 0)],
    cancels: &[c(DASH | SWAP, 14, 13), c(ALL, 22, 18)],
    sounds: &[(0, WAH, 0.0), (1, SWISH, 0.45), (12, CHOMP_BITE, 0.3)],
    locked: (0, 5),
    ..BASE
};

/// X mashed as the bite lands: five quick bites in front of him, the last one tossing it up
pub const FRENZY: Move = Move {
    name: "Chomp frenzy",
    len: 34,
    clips: &[still(0, SECOND_PUNCH), clip(22, FIRST_PUNCH)],
    twist: &[k(0, 0.0, Linear), k(2, 12.0, Out), k(6, -12.0, Out), k(10, 12.0, Out), k(14, -12.0, Out), k(18, 12.0, Out), k(22, -25.0, Out), k(26, 15.0, Out), k(32, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(2, 8.0, Out), k(20, 8.0, Linear), k(24, -14.0, Out), k(32, 0.0, InOut)],
    aim: &[0, 12],
    approach: Some(Approach { from: 0, to: 6, max: 0.8, gap: 1.2 }),
    reach: &[k(0, 0.8, Linear), k(2, 1.0, Out), k(26, 1.0, Linear), k(32, 0.5, InOut)],
    blade_yaw: &[k(0, 0.0, Linear), k(2, 10.0, Out), k(6, -10.0, Out), k(10, 10.0, Out), k(14, -10.0, Out), k(18, 10.0, Out), k(22, 0.0, Out), k(32, -80.0, InOut)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 34),
    out: &[k(0, 140.0, Linear), k(2, 170.0, Out), k(20, 170.0, Linear), k(26, 150.0, Out), k(32, 120.0, InOut)],
    up: &[k(0, 50.0, Linear), k(22, 50.0, Linear), k(26, 230.0, Out), k(32, 40.0, InOut)],
    // (its jaws: open, snapped shut on each bite)
    bite: &[
        k(0, 0.6, Linear),
        k(2, 1.0, Out),
        k(4, 0.0, In),
        k(6, 1.0, Out),
        k(8, 0.0, In),
        k(10, 1.0, Out),
        k(12, 0.0, In),
        k(14, 1.0, Out),
        k(16, 0.0, In),
        k(18, 1.0, Out),
        k(20, 0.0, In),
        k(23, 1.0, Out),
        k(25, 0.0, In),
        k(32, 0.0, Linear),
    ],
    windows: &[bite_at(3), bite_at(7), bite_at(11), bite_at(15), bite_at(19), w(24, 26, Shape::Weapon, Attack::FrenzyEnd, 0)],
    cancels: &[c(DASH | SWAP, 8, 8), c(ALL, 28, 26)],
    sounds: &[(0, HOO, 0.0), (4, CHOMP_BITE, 0.22), (8, CHOMP_BITE, 0.2), (12, CHOMP_BITE, 0.2), (16, CHOMP_BITE, 0.2), (20, CHOMP_BITE, 0.2), (25, CHOMP_BITE, 0.32)],
    fx: &[(25, FX_LAUNCH)],
    locked: (0, 2),
    ..BASE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::*;

    fn chomp(id: u16) -> &'static Move {
        &MOVES[id as usize]
    }

    #[test]
    fn the_chomps_moves_sit_at_their_ids() {
        assert_eq!(chomp(CHOMP_RUSH).name, "Chomp rush");
        assert_eq!(chomp(CHOMP_LASSO_CHARGE).name, "Chomp lasso charge");
        assert_eq!(chomp(CHOMP_LASSO).name, "Chomp lasso");
        assert_eq!(chomp(CHOMP_FRENZY).name, "Chomp frenzy");
    }

    #[test]
    fn the_frenzy_bites_five_times_and_tosses() {
        let m = chomp(CHOMP_FRENZY);
        assert_eq!(m.windows.iter().filter(|w| w.attack == Attack::Frenzy).count(), 5);
        assert_eq!(m.windows.iter().filter(|w| w.attack == Attack::FrenzyEnd).count(), 1);
        for pair in m.windows.windows(2) {
            if pair[0].attack == pair[1].attack {
                assert!((pair[1].from - pair[0].from) as u32 >= pair[1].attack.cooldown());
            }
        }
    }

    #[test]
    fn the_lasso_reaches_12_m() {
        let far = chomp(CHOMP_LASSO).out.iter().map(|k| k.v).fold(0.0f32, f32::max);
        assert!((far / 100.0 - 12.0).abs() < 0.5, "{far}");
    }
}
