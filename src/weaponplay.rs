//! Which of the other weapons' new moves (v4 Part 3, script.rs) the buttons ask for, as
//! swordplay.rs does for the Star Sword. The fists: mashing X as the Bullet Bill punch lands (the
//! hundred-hand flurry), X, a beat, X (the heavy hook), and up in the air the stick's way with X
//! (forward: the meteor punch; back: the dropkick). X held is charge.rs's (every weapon's charge
//! while he walks) but the boomerang's (steered). Pure: moves.rs gathers what it reads and starts
//! what comes out; er_moves.c's own moves take the rest.

use crate::combo::Dir;
use crate::script::*;

/// What the buttons and the stick did this tick.
#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    pub x: bool,
    /// ticks X has been held (0: up), and when it came up this tick, how long it was
    pub x_held: u32,
    pub x_released: Option<u32>,
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
    /// ticks since the Chain Chomp's bite (X, Y) began
    pub bite_ago: Option<u32>,
    /// boomerangs in hand (boomerang.rs)
    pub rangs: u32,
}

/// X held this long: the boomerang steered
pub const CHARGE_AFTER: u32 = 18;
/// X this long after the first blow ended: the beat's (ticks)
pub const BEAT: (u32, u32) = (8, 30);
/// mashing: this many X presses in MASH_WINDOW ticks
pub const MASH: u32 = 3;
pub const MASH_WINDOW: u32 = 14;

/// The move to start now, if any, with weapon `weapon` (weapons.rs) in hand.
pub fn choose(weapon: u8, i: &Input, s: &State) -> Option<u16> {
    use crate::weapons::{CHOMP, FISTS, HAMMER, RANG};
    if s.mega {
        return None;
    }
    match weapon {
        FISTS => fists(i, s),
        HAMMER => hammer(i, s),
        CHOMP => chomp(i, s),
        RANG => rang(i, s),
        _ => None,
    }
}

/// X came up (how long it was held): this tick, or while moves weren't read (a menu): it's up now.
fn let_go(i: &Input) -> Option<u32> {
    i.x_released.or((i.x_held == 0).then_some(0))
}

/// The scripted move playing can be cut now (or none plays).
fn free(s: &State) -> bool {
    s.playing.is_none_or(|p| p.1)
}

/// X held this long isn't the Dodge Offset's (held through a dash) and isn't in the dash.
fn after_dash(i: &Input, s: &State) -> bool {
    !s.dashing && s.dashed.is_none_or(|d| i.x_held <= d)
}

fn held_x(i: &Input, s: &State) -> bool {
    i.x_held >= CHARGE_AFTER && i.x_released.is_none() && free(s) && after_dash(i, s)
}

fn beat(s: &State) -> bool {
    s.first_blow_ended.is_some_and(|ago| (BEAT.0..=BEAT.1).contains(&ago))
}

fn hammer(i: &Input, s: &State) -> Option<u16> {
    if !free(s) || !i.x {
        return None;
    }
    if s.air {
        return (i.dir == Some(Dir::Forward)).then_some(HAMMER_COMET);
    }
    if i.back_forward {
        return Some(HAMMER_CHARGE);
    }
    beat(s).then_some(HAMMER_VOLLEY)
}

fn chomp(i: &Input, s: &State) -> Option<u16> {
    if s.air {
        return None;
    }
    // mashing X as the bite lands (out of the bite itself: not again in the frenzy)
    if s.playing.is_none() && i.x && i.mash >= MASH && s.bite_ago.is_some_and(|ago| (FRENZY_FROM..=FRENZY_TO).contains(&ago)) {
        return Some(CHOMP_FRENZY);
    }
    (free(s) && i.x && i.back_forward).then_some(CHOMP_RUSH)
}

fn rang(i: &Input, s: &State) -> Option<u16> {
    let playing = s.playing.map(|p| p.0);
    if playing == Some(RANG_STEER) {
        return let_go(i).map(|_| RANG_CATCH);
    }
    if s.air {
        return None;
    }
    if held_x(i, s) && s.rangs >= 1 {
        return Some(RANG_STEER);
    }
    if !free(s) || !i.x {
        return None;
    }
    if i.back_forward {
        return (s.rangs >= 1).then_some(RANG_SKIM);
    }
    (beat(s) && s.rangs >= 2).then_some(RANG_PINCER)
}

/// the Chomp's bite this many ticks along (its jaws close at 6, er_moves.c): mashing X then is its
/// frenzy
const FRENZY_FROM: u32 = 7;
const FRENZY_TO: u32 = 40;

fn fists(i: &Input, s: &State) -> Option<u16> {
    let playing = s.playing.map(|p| p.0);
    let free = free(s);
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
    if !free || !i.x {
        return None;
    }
    // X, a beat, X: the heavy hook
    beat(s).then_some(HEAVY_HOOK)
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

    /// X held is charge.rs's now: nothing here for the fists, the hammer or the Chomp.
    #[test]
    fn x_held_is_the_charges() {
        use crate::weapons::CHOMP;
        let held = Input { x_held: CHARGE_AFTER + 10, ..Default::default() };
        for w in [FISTS, HAMMER, CHOMP] {
            assert_eq!(choose(w, &held, &State::default()), None, "weapon {w}");
        }
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
        // (the hammer's X held is nothing new: its Y held charges)
        assert_eq!(choose(HAMMER, &Input { x_held: CHARGE_AFTER, ..Default::default() }, &State::default()), None);
        assert_eq!(choose(crate::weapons::SWORD, &x(), &State::default()), None);
    }

    fn closer() -> Input {
        Input { back_forward: true, dir: Some(Dir::Forward), ..x() }
    }

    #[test]
    fn the_hammers_closer_beat_and_dive() {
        assert_eq!(choose(HAMMER, &closer(), &State::default()), Some(HAMMER_CHARGE));
        assert_eq!(choose(HAMMER, &x(), &State { first_blow_ended: Some(15), ..Default::default() }), Some(HAMMER_VOLLEY));
        assert_eq!(choose(HAMMER, &x(), &State::default()), None);
        let air = State { air: true, ..Default::default() };
        assert_eq!(choose(HAMMER, &Input { dir: Some(Dir::Forward), ..x() }, &air), Some(HAMMER_COMET));
        assert_eq!(choose(HAMMER, &Input { dir: Some(Dir::Neutral), ..x() }, &air), None);
    }

    #[test]
    fn the_chomps_closer_and_frenzy() {
        use crate::weapons::CHOMP;
        assert_eq!(choose(CHOMP, &closer(), &State::default()), Some(CHOMP_RUSH));
        // (mashing as the bite lands; not before it bites)
        let mash = Input { mash: 3, ..x() };
        assert_eq!(choose(CHOMP, &mash, &State { bite_ago: Some(8), ..Default::default() }), Some(CHOMP_FRENZY));
        // (the bite's jaws close at its tick 6: not before it bites)
        assert_eq!(choose(CHOMP, &mash, &State { bite_ago: Some(5), ..Default::default() }), None);
        assert_eq!(choose(CHOMP, &mash, &State::default()), None);
        // (in the frenzy, mashing on: it plays out, it doesn't start over)
        assert_eq!(choose(CHOMP, &mash, &State { bite_ago: Some(12), playing: Some((CHOMP_FRENZY, false)), ..Default::default() }), None);
    }

    #[test]
    fn the_boomerangs_need_boomerangs_in_hand() {
        use crate::weapons::RANG;
        let rangs = |n: u32| State { rangs: n, ..Default::default() };
        assert_eq!(choose(RANG, &closer(), &rangs(1)), Some(RANG_SKIM));
        assert_eq!(choose(RANG, &closer(), &rangs(0)), None);
        let beat = |n: u32| State { first_blow_ended: Some(12), ..rangs(n) };
        assert_eq!(choose(RANG, &x(), &beat(2)), Some(RANG_PINCER));
        assert_eq!(choose(RANG, &x(), &beat(1)), None);
        assert_eq!(choose(RANG, &Input { x_held: CHARGE_AFTER, ..Default::default() }, &rangs(1)), Some(RANG_STEER));
        let steering = State { playing: Some((RANG_STEER, false)), ..rangs(0) };
        assert_eq!(choose(RANG, &Input { x_released: Some(40), ..Default::default() }, &steering), Some(RANG_CATCH));
        assert_eq!(choose(RANG, &Input { x_held: 40, ..Default::default() }, &steering), None);
    }
}
