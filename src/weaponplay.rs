//! Which of the other weapons' new moves (v4 Part 3, script.rs) the buttons ask for, as
//! swordplay.rs does for the Star Sword. The fists: mashing X as the Bullet Bill punch lands (the
//! hundred-hand flurry), X held (the Super punch), X, a beat, X (the heavy hook), and up in the air
//! the stick's way with X (forward: the meteor punch; back: the dropkick). Pure: moves.rs gathers
//! what it reads and starts what comes out; er_moves.c's own moves take the rest.

use crate::combo::Dir;
use crate::script::*;

/// What the buttons and the stick did this tick.
#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    pub x: bool,
    /// ticks X has been held (0: up), and when it came up this tick, how long it was
    pub x_held: u32,
    pub x_released: Option<u32>,
    #[allow(dead_code)] // (the hammer's, the Chomp's and the boomerang's closers, Part 3b)
    pub back_forward: bool,
    pub dir: Option<Dir>,
    /// X presses in the last MASH_WINDOW ticks (since the move playing began)
    pub mash: u32,
}

/// What Mario is doing.
#[derive(Clone, Copy, Default, Debug)]
pub struct State {
    pub air: bool,
    /// the scripted move playing and whether its NEXT window is open, whether a blow of it landed
    pub playing: Option<(u16, bool)>,
    pub landed: bool,
    /// in the dash now; ticks since a dash ended
    pub dashing: bool,
    pub dashed: Option<u32>,
    /// ticks since the string's first blow ended with no second one (the fists' first punch)
    pub first_blow_ended: Option<u32>,
    /// Mega Mario: his own slam and fists
    pub mega: bool,
    /// in the air string, or an enemy up in the air with him (juggle.rs)
    pub air_string: bool,
}

/// X held this long: its charge
pub const CHARGE_AFTER: u32 = 18;
/// X this long after the first blow ended: the beat's (ticks)
pub const BEAT: (u32, u32) = (8, 30);
/// mashing: this many X presses in MASH_WINDOW ticks
pub const MASH: u32 = 3;
pub const MASH_WINDOW: u32 = 14;

/// The move to start now, if any, with weapon `weapon` (weapons.rs) in hand.
pub fn choose(weapon: u8, i: &Input, s: &State) -> Option<u16> {
    match weapon {
        crate::weapons::FISTS => fists(i, s),
        _ => None,
    }
}

fn fists(i: &Input, s: &State) -> Option<u16> {
    if s.mega {
        return None;
    }
    let playing = s.playing.map(|p| p.0);
    let free = s.playing.is_none_or(|p| p.1);
    // (charging: only the release counts)
    if playing == Some(FIST_CHARGE) {
        return i.x_released.map(|held| if held >= crate::fistmoves::SUPER_FULL { SUPER_PUNCH + 1 } else { SUPER_PUNCH });
    }
    // mashing X as the Bullet Bill punch lands
    if playing == Some(BULLET_PUNCH) && s.landed && i.x && i.mash >= MASH {
        return Some(FIST_FLURRY);
    }
    if s.air {
        if !free || !i.x {
            return None;
        }
        return match i.dir {
            // (jumping in at an enemy, forward + X stays the air string's first kick: the spike is
            // for one up there)
            Some(Dir::Forward) if s.air_string => Some(METEOR_PUNCH),
            Some(Dir::Back) => Some(FIST_DROPKICK),
            _ => None,
        };
    }
    // X held: the charge (not in the dash, nor held from before it: the Dodge Offset's)
    let after_dash = !s.dashing && s.dashed.is_none_or(|d| i.x_held <= d);
    if i.x_held >= CHARGE_AFTER && i.x_released.is_none() && free && after_dash {
        return Some(FIST_CHARGE);
    }
    if !free || !i.x {
        return None;
    }
    // X, a beat, X: the heavy hook
    if s.first_blow_ended.is_some_and(|ago| (BEAT.0..=BEAT.1).contains(&ago)) {
        return Some(HEAVY_HOOK);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::{FISTS, HAMMER};

    fn x() -> Input {
        Input { x: true, x_held: 1, ..Default::default() }
    }

    #[test]
    fn the_flurry_comes_once_the_bullet_bill_punch_lands() {
        let mash = Input { mash: 3, ..x() };
        let punch = |landed: bool| State { playing: Some((BULLET_PUNCH, false)), landed, ..Default::default() };
        assert_eq!(choose(FISTS, &mash, &punch(false)), None);
        assert_eq!(choose(FISTS, &mash, &punch(true)), Some(FIST_FLURRY));
        assert_eq!(choose(FISTS, &Input { mash: 1, ..x() }, &punch(true)), None);
    }

    #[test]
    fn x_held_charges_the_super_punch() {
        let held = Input { x_held: CHARGE_AFTER, ..Default::default() };
        assert_eq!(choose(FISTS, &held, &State::default()), Some(FIST_CHARGE));
        let charging = State { playing: Some((FIST_CHARGE, false)), ..Default::default() };
        assert_eq!(choose(FISTS, &Input { x_released: Some(30), ..Default::default() }, &charging), Some(SUPER_PUNCH));
        assert_eq!(choose(FISTS, &Input { x_released: Some(50), ..Default::default() }, &charging), Some(SUPER_PUNCH + 1));
        assert_eq!(choose(FISTS, &Input { x_held: 40, ..Default::default() }, &charging), None);
        // (not in the dash, nor held through it)
        assert_eq!(choose(FISTS, &held, &State { dashing: true, dashed: Some(0), ..Default::default() }), None);
        assert_eq!(choose(FISTS, &Input { x_held: 30, ..Default::default() }, &State { dashed: Some(5), ..Default::default() }), None);
    }

    #[test]
    fn a_beat_after_the_first_punch_is_the_heavy_hook() {
        let after = |ago: u32| State { first_blow_ended: Some(ago), ..Default::default() };
        assert_eq!(choose(FISTS, &x(), &after(12)), Some(HEAVY_HOOK));
        // (too soon: SM64's own string; too late: a new one)
        assert_eq!(choose(FISTS, &x(), &after(3)), None);
        assert_eq!(choose(FISTS, &x(), &after(60)), None);
        assert_eq!(choose(FISTS, &x(), &State::default()), None);
    }

    #[test]
    fn up_in_the_air_the_stick_picks_the_blow() {
        let air = State { air: true, air_string: true, ..Default::default() };
        assert_eq!(choose(FISTS, &Input { dir: Some(Dir::Forward), ..x() }, &air), Some(METEOR_PUNCH));
        // (jumping in at an enemy, forward + X is the air string's first kick: the spike is for one
        // up there)
        assert_eq!(choose(FISTS, &Input { dir: Some(Dir::Forward), ..x() }, &State { air_string: false, ..air }), None);
        assert_eq!(choose(FISTS, &Input { dir: Some(Dir::Back), ..x() }, &State { air_string: false, ..air }), Some(FIST_DROPKICK));
        assert_eq!(choose(FISTS, &Input { dir: Some(Dir::Back), ..x() }, &air), Some(FIST_DROPKICK));
        // (neutral: the air string stays er_moves.c's)
        assert_eq!(choose(FISTS, &Input { dir: Some(Dir::Neutral), ..x() }, &air), None);
        // (in a move that can't be cut yet: nothing)
        assert_eq!(choose(FISTS, &Input { dir: Some(Dir::Forward), ..x() }, &State { playing: Some((METEOR_PUNCH, false)), ..air }), None);
    }

    #[test]
    fn mega_mario_and_the_other_weapons_keep_their_own() {
        assert_eq!(choose(FISTS, &Input { x_held: CHARGE_AFTER, ..Default::default() }, &State { mega: true, ..Default::default() }), None);
        assert_eq!(choose(HAMMER, &Input { x_held: CHARGE_AFTER, ..Default::default() }, &State::default()), None);
    }
}
