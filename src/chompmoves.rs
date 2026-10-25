//! The Chain Chomp's new moves as data (v4 Part 3): script.rs plays them (MOVES from id CHOMP_RUSH
//! on), weaponplay.rs picks them (charge.rs the ram). The Chomp stays the fast, wide, lighter
//! weapon: it lunges ahead dragging Mario along, rams through a line of them, bites in a frenzy. Its place comes
//! from the frame's weapon pose (lib.rs): `blade_yaw` round his facing (+ left), `out` and `up` in
//! SM64 units from his feet, `bite` its mouth (0 shut, 1 wide).

use crate::combat::Attack;
use crate::moves::FX_LAUNCH;
use crate::script::Ease::*;
use crate::script::{k, Approach, Cancel, Clip, Move, Shape, Window, ALL, BASE, DASH, SWAP};

const FIRST_PUNCH: i32 = 0x67;
const SECOND_PUNCH: i32 = 0x68;
const SECOND_PUNCH_FAST: i32 = 0x6A;
const FAST_LONGJUMP: i32 = 0x13;
const GENERAL_LAND: i32 = 0x57;
const GRAB_HEAVY_OBJECT: i32 = 0x59;
const HEAVY_THROW: i32 = 0xB9;
const GROUND_POUND: i32 = 0x3D;

/// (SM64's: the soft ones at that volume, the voice at his body; the chain's clink, not verified)
const SWISH: u32 = 0x305C_0081;
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

/// X held and let go (charge.rs): the ram, the Chomp flung ahead biting and Mario dragged along
/// behind it 4 m, through everyone in the way (the lasso's pull was the hook's job already)
pub const RAM: Move = Move {
    name: "Chomp ram",
    len: 26,
    clips: &[still(0, FIRST_PUNCH), clip(3, FAST_LONGJUMP), clip(12, GENERAL_LAND)],
    twist: &[k(0, -30.0, Linear), k(3, 10.0, Out), k(12, 0.0, InOut)],
    pitch: &[k(0, -6.0, Linear), k(3, 20.0, Out), k(11, 12.0, Linear), k(16, 0.0, InOut)],
    aim: &[0],
    approach: Some(Approach { from: 3, to: 11, max: 4.0, gap: 1.2 }),
    squash: &[k(0, 0.9, Linear), k(3, 1.12, Out), k(11, 1.0, InOut), k(13, 0.92, Out), k(18, 1.0, InOut)],
    dip: &[k(0, 8.0, Linear), k(3, 0.0, Out), k(13, 6.0, Out), k(18, 0.0, InOut)],
    reach: &[k(0, 0.3, Linear), k(3, 1.0, Out), k(12, 1.0, Linear), k(20, 0.4, InOut)],
    // (flung out ahead of him biting all the way, back by him as he lands)
    blade_yaw: &[k(0, -150.0, Linear), k(3, 0.0, Out), k(14, 0.0, Linear), k(24, -90.0, InOut)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 26),
    out: &[k(0, 130.0, Linear), k(4, 420.0, Out), k(11, 300.0, Linear), k(16, 160.0, InOut), k(24, 120.0, InOut)],
    up: &[k(0, 60.0, Linear), k(4, 70.0, Out), k(11, 60.0, Linear), k(24, 30.0, InOut)],
    bite: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(5, 0.0, In), k(7, 1.0, Out), k(8, 0.0, In), k(10, 1.0, Out), k(11, 0.0, In), k(16, 0.0, Linear)],
    windows: &[w(3, 12, Shape::Weapon, Attack::ChompRush, 0)],
    cancels: &[c(DASH | SWAP, 12, 10), c(ALL, 18, 14)],
    sounds: &[(0, YAH, 0.0), (1, SWISH, 0.45), (5, CHOMP_BITE, 0.3), (8, CHOMP_BITE, 0.28), (11, CHOMP_BITE, 0.3)],
    fx: &[(3, FX_LAUNCH)],
    locked: (0, 3),
    ..BASE
};

/// ...let go full: 8 m, and a last crunching bite where he stops (blood)
pub const RAM_FULL: Move = Move {
    name: "Chomp ram (full)",
    len: 32,
    clips: &[still(0, FIRST_PUNCH), clip(3, FAST_LONGJUMP), clip(15, SECOND_PUNCH_FAST), clip(19, GENERAL_LAND)],
    approach: Some(Approach { from: 3, to: 14, max: 8.0, gap: 1.2 }),
    blade_yaw: &[k(0, -150.0, Linear), k(3, 0.0, Out), k(18, 0.0, Linear), k(30, -90.0, InOut)],
    blade_held: (0, 32),
    out: &[k(0, 130.0, Linear), k(4, 520.0, Out), k(14, 360.0, Linear), k(16, 240.0, Out), k(22, 160.0, InOut), k(30, 120.0, InOut)],
    up: &[k(0, 60.0, Linear), k(4, 70.0, Out), k(14, 60.0, Linear), k(16, 90.0, Out), k(22, 40.0, InOut), k(30, 30.0, InOut)],
    bite: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(5, 0.0, In), k(7, 1.0, Out), k(8, 0.0, In), k(10, 1.0, Out), k(11, 0.0, In), k(13, 1.0, Out), k(15, 1.0, Linear), k(17, 0.0, In), k(22, 0.0, Linear)],
    windows: &[w(3, 14, Shape::Weapon, Attack::ChompRush, 0), w(16, 18, Shape::Weapon, Attack::ChompEnd, crate::moves::FX_BANG)],
    cancels: &[c(DASH | SWAP, 19, 17), c(ALL, 24, 19)],
    sounds: &[(0, WAH, 0.0), (1, SWISH, 0.48), (5, CHOMP_BITE, 0.3), (8, CHOMP_BITE, 0.28), (11, CHOMP_BITE, 0.3), (16, CHOMP_BITE, 0.4)],
    locked: (0, 3),
    ..RAM
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

/// B (Part 12.6b, chompslam.rs): the Chomp bites the enemy and Mario heaves it over his head onto the
/// other side: two hands overhead (the right hand's reach, tipped straight up), the torso back, then
/// forward as it lands. The enemy's flight is juggle.rs'; lib.rs puts the Chomp (its yaw, its reach,
/// its height) on the enemy it holds, so the curves for them are only the rest pose. No blow of its
/// own: the landing and its blast are lib.rs'.
pub const SLAM: Move = Move {
    name: "Chomp Slam",
    len: 32,
    clips: &[clip(0, GRAB_HEAVY_OBJECT), clip(12, HEAVY_THROW), clip(24, GENERAL_LAND)],
    pitch: &[k(0, 0.0, Linear), k(4, -8.0, Out), k(9, -26.0, InOut), k(15, 4.0, In), k(19, 30.0, Out), k(25, 10.0, InOut), k(32, 0.0, InOut)],
    aim: &[0],
    squash: &[k(0, 1.0, Linear), k(9, 1.08, Out), k(19, 0.88, Out), k(26, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(19, 8.0, Out), k(28, 0.0, InOut)],
    reach: &[k(0, 0.8, Linear), k(4, 1.0, Out), k(24, 1.0, Linear), k(30, 0.5, InOut)],
    reach_pitch: 90.0,
    blade_yaw: &[k(0, 0.0, Linear)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 32),
    out: &[k(0, 150.0, Linear)],
    up: &[k(0, 70.0, Linear)],
    bite: &[k(0, 0.0, Linear), k(3, 1.0, Out), k(5, 0.1, In), k(18, 0.1, Linear), k(20, 1.0, Out), k(24, 0.0, InOut)],
    // (B again once it has landed: moves.rs slams back the other way)
    cancels: &[c(DASH | SWAP, 22, 20)],
    sounds: &[(0, HOO, 0.0), (4, CHOMP_BITE, 0.35), (9, SWISH, 0.4)],
    locked: (0, 20),
    ..BASE
};

/// In the air (Orion's Harpoon): the Chomp bites an enemy that is up there and he drives it down,
/// hanging a moment, leaning into the throw.
pub const HARPOON: Move = Move {
    name: "Chomp harpoon",
    len: 24,
    clips: &[still(0, FIRST_PUNCH), clip(3, GROUND_POUND)],
    pitch: &[k(0, 0.0, Linear), k(3, -12.0, Out), k(8, 28.0, In), k(16, 10.0, InOut), k(24, 0.0, InOut)],
    aim: &[0, 3],
    hold: (0, 10),
    squash: &[k(0, 1.0, Linear), k(3, 1.06, Out), k(8, 0.9, Out), k(16, 1.0, InOut)],
    reach: &[k(0, 0.8, Linear), k(4, 1.0, Out), k(18, 1.0, Linear), k(23, 0.5, InOut)],
    reach_pitch: 60.0,
    blade_yaw: &[k(0, 0.0, Linear)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 24),
    out: &[k(0, 150.0, Linear)],
    up: &[k(0, 70.0, Linear)],
    bite: &[k(0, 0.0, Linear), k(3, 1.0, Out), k(6, 0.1, In), k(14, 0.1, Linear), k(16, 1.0, Out), k(20, 0.0, InOut)],
    cancels: &[c(ALL, 16, 12)],
    sounds: &[(0, HOO, 0.0), (4, CHOMP_BITE, 0.35), (8, SWISH, 0.45)],
    locked: (0, 8),
    air: true,
    land_ends: true,
    ..BASE
};

/// A big one (or a boss): the Chomp bites and he hauls on the chain, a stance yank; it lurches
/// toward him a step, nothing more.
pub const YANK: Move = Move {
    name: "Chomp yank",
    len: 18,
    clips: &[clip(0, GRAB_HEAVY_OBJECT), clip(10, GENERAL_LAND)],
    pitch: &[k(0, 0.0, Linear), k(3, -6.0, Out), k(7, -20.0, InOut), k(10, 10.0, In), k(18, 0.0, InOut)],
    aim: &[0],
    squash: &[k(0, 1.0, Linear), k(7, 0.94, Out), k(12, 1.0, InOut)],
    reach: &[k(0, 0.6, Linear), k(4, 1.0, Out), k(14, 1.0, Linear), k(17, 0.5, InOut)],
    blade_yaw: &[k(0, 0.0, Linear)],
    blade_pitch: &[k(0, 0.0, Linear)],
    blade_held: (0, 18),
    out: &[k(0, 150.0, Linear)],
    up: &[k(0, 70.0, Linear)],
    bite: &[k(0, 0.0, Linear), k(3, 1.0, Out), k(5, 0.1, In), k(11, 0.1, Linear), k(13, 1.0, Out), k(16, 0.0, InOut)],
    cancels: &[c(DASH | SWAP, 12, 10)],
    sounds: &[(0, HOO, 0.0), (4, CHOMP_BITE, 0.35)],
    locked: (0, 9),
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
        assert_eq!(chomp(CHOMP_RAM).name, "Chomp ram");
        assert_eq!(chomp(CHOMP_RAM_FULL).name, "Chomp ram (full)");
        assert_eq!(chomp(CHOMP_FRENZY).name, "Chomp frenzy");
    }

    /// The Chomp Slam (Part 12.6b): the torso goes back, then forward as the enemy lands (its flight
    /// is chompslam's: bite + flight seconds), B again opens only after it landed.
    #[test]
    fn the_slam_heaves_back_then_slams_forward_as_it_lands() {
        let m = chomp(CHOMP_SLAM);
        assert!(!m.air && m.windows.is_empty());
        let land = ((crate::chompslam::BITE + crate::chompslam::FLIGHT) * 30.0).round() as u16;
        let at = |f: fn(f32, f32) -> bool| m.pitch.iter().copied().reduce(|a, b| if f(b.v, a.v) { b } else { a }).unwrap();
        let (back, fwd) = (at(|b, a| b < a), at(|b, a| b > a));
        assert!(back.v <= -20.0 && fwd.v >= 25.0, "{back:?} {fwd:?}");
        assert!(back.t < fwd.t, "back before forward");
        assert!(fwd.t.abs_diff(land) <= 2, "forward peak {} vs landing {land}", fwd.t);
        // (overhead, both hands: the reach tipped straight up)
        assert_eq!(m.reach_pitch, 90.0);
        // (no dash, no second slam, before it has landed)
        assert!(m.cancels.iter().all(|c| c.from.min(c.on_hit) >= land), "{:?}", m.cancels);
        assert!(m.locked.1 >= land - 2);
        assert!(m.cancels.iter().any(|c| c.into & DASH != 0));
    }

    #[test]
    fn the_harpoon_hangs_and_drives_down_and_the_yank_is_short() {
        let h = chomp(CHOMP_HARPOON);
        assert!(h.air && h.land_ends && h.hold.1 >= 6);
        assert!(h.pitch.iter().any(|k| k.v >= 25.0));
        let y = chomp(CHOMP_YANK);
        assert!(!y.air && y.len <= 20 && y.windows.is_empty());
        assert_eq!(chomp(CHOMP_SLAM).name, "Chomp Slam");
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

    /// The ram drags him 4 m (8 m full), biting all the way; the full one crunches at the end.
    #[test]
    fn the_ram_drags_him_through_them() {
        assert_eq!(chomp(CHOMP_RAM).approach.map(|a| a.max), Some(4.0));
        assert_eq!(chomp(CHOMP_RAM_FULL).approach.map(|a| a.max), Some(8.0));
        assert!(chomp(CHOMP_RAM_FULL).windows.iter().any(|w| w.attack == Attack::ChompEnd));
    }
}
