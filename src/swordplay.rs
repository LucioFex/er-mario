//! Which of the Star Sword's moves (script.rs) the buttons ask for (v4 Part 2; Part 10: Dante's
//! Rebellion, no spins): its string (X x4, Combo A, a two-handed chop to end), the pause strings
//! (X, beat, X: the thunder cleave; X, X, beat, X: the Million Stab, Combo B), the closers (back,
//! forward + X: the Stinger, X mashed as it lands the Million Stab; dash + X: the lightning iai; a
//! perfect dodge + X: the Flurry Rush), the launchers (back + X and Y: High Time, held he goes up
//! with them; X, X, Y and X + A: the launcher, up with them always), X, Y: the cross cut,
//! forward + Y: the Helm Splitter, and up in the air the Aerial Rave (X x4), the Helm Breaker (Y)
//! and the pogo (LT). X held is charge.rs's (the Drive). Pure: moves.rs gathers what it reads and
//! starts what comes out.

use crate::combo::Dir;
use crate::script::*;

/// What the buttons and the stick did this tick.
#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    pub x: bool,
    pub y: bool,
    pub lt: bool,
    pub back_forward: bool,
    pub chord_xa: bool,
    pub dir: Option<Dir>,
    /// X presses in the last MASH_WINDOW ticks
    pub mash: u32,
}

/// What Mario is doing.
#[derive(Clone, Copy, Default, Debug)]
pub struct State {
    pub air: bool,
    /// the scripted move playing (the sword's, or its RB's) and whether its NEXT window is open,
    /// how many ticks it has been going, whether a blow of it landed
    pub playing: Option<(u16, bool)>,
    pub playing_for: u32,
    pub landed: bool,
    /// the last sword move that ended, and how many ticks ago
    pub ended: Option<(u16, u32)>,
    /// ticks since a dash (on the ground) ended or is ending (0: in it now)
    pub dashed: Option<u32>,
    /// a perfect dodge's counter time (dodge.rs)
    pub countering: bool,
    /// pogo bounces left in this jump
    pub pogos: u8,
}

/// a beat after a string's blow ended: the pause strings' X (ticks)
pub const BEAT: (u32, u32) = (8, 30);
/// the Million Stab off the Stinger: this many X presses in MASH_WINDOW ticks
pub const MASH: u32 = 3;
pub const MASH_WINDOW: u32 = 14;
/// X this soon after the dash: the iai (ticks)
pub const DASH_X: u32 = 5;
/// X + A with X this many ticks first at most: the slash X began is the launcher's
pub const CHORD_LATE: u32 = 3;

/// The move to start now, if any.
pub fn choose(i: &Input, s: &State) -> Option<u16> {
    let playing = s.playing.map(|p| p.0);
    let next = s.playing.is_some_and(|p| p.1);
    let free = s.playing.is_none() || next;
    let is = |ids: &[u16]| playing.is_some_and(|p| ids.contains(&p));
    if s.air {
        if !free {
            return None;
        }
        if i.y {
            return Some(SW_BREAKER);
        }
        if i.lt && s.pogos > 0 {
            return Some(SW_POGO);
        }
        if i.x {
            return Some(match playing {
                Some(p) if (SW_RAVE..SW_RAVE + 3).contains(&p) => p + 1,
                _ => SW_RAVE,
            });
        }
        return None;
    }
    // the Million Stab: mashing X as the Stinger lands (its lunge first: mashed from the start it
    // would stab the air where it began)
    if is(&[SW_THRUST]) && s.landed && i.x && i.mash >= MASH {
        return Some(SW_STABS);
    }
    // X a tick or two before A: the first slash it began gives way to the launcher
    if i.chord_xa && is(&[SW_S1]) && s.playing_for <= CHORD_LATE {
        return Some(SW_LAUNCHER);
    }
    if !free {
        return None;
    }
    if i.chord_xa {
        return Some(SW_LAUNCHER);
    }
    if i.x {
        if s.countering {
            return Some(SW_FLURRY);
        }
        if s.dashed.is_some_and(|t| t <= DASH_X) {
            return Some(SW_IAI);
        }
        if i.back_forward {
            return Some(SW_THRUST);
        }
        if i.dir == Some(Dir::Back) {
            return Some(SW_RISE);
        }
        // the string, and its pause strings after a beat
        if let Some(p) = playing.filter(|p| (SW_S1..SW_S4).contains(p)) {
            return Some(p + 1);
        }
        if let Some((m, _)) = s.ended.filter(|(_, ago)| (BEAT.0..=BEAT.1).contains(ago)) {
            match m {
                SW_S1 => return Some(SW_CLEAVE),
                SW_S2 => return Some(SW_STABS),
                _ => {}
            }
        }
        return Some(SW_S1);
    }
    if i.y {
        if i.dir == Some(Dir::Forward) && playing.is_none() {
            return Some(SW_HELM);
        }
        return Some(match playing {
            Some(SW_S1) => SW_CROSS,
            Some(SW_S2) => SW_LAUNCHER,
            _ => SW_HIGH_TIME,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x() -> Input {
        Input { x: true, ..Default::default() }
    }

    fn y() -> Input {
        Input { y: true, ..Default::default() }
    }

    fn in_move(id: u16, next: bool) -> State {
        State { playing: Some((id, next)), ..Default::default() }
    }

    #[test]
    fn the_string_and_its_pauses() {
        assert_eq!(choose(&x(), &State::default()), Some(SW_S1));
        assert_eq!(choose(&x(), &in_move(SW_S1, true)), Some(SW_S2));
        assert_eq!(choose(&x(), &in_move(SW_S3, true)), Some(SW_S4));
        // (the chop ends Combo A: X after it starts it over)
        assert_eq!(choose(&x(), &in_move(SW_S4, true)), Some(SW_S1));
        // (too early in a blow: nothing, the buffer waits)
        assert_eq!(choose(&x(), &in_move(SW_S1, false)), None);
        // a beat after the first blow: the thunder cleave; after the second: the Million Stab
        let after = |m: u16, ago: u32| State { ended: Some((m, ago)), ..Default::default() };
        assert_eq!(choose(&x(), &after(SW_S1, 12)), Some(SW_CLEAVE));
        assert_eq!(choose(&x(), &after(SW_S2, 20)), Some(SW_STABS));
        // (too soon or too late: the string again)
        assert_eq!(choose(&x(), &after(SW_S2, 2)), Some(SW_S1));
        assert_eq!(choose(&x(), &after(SW_S2, 60)), Some(SW_S1));
    }

    #[test]
    fn closers_and_launchers() {
        assert_eq!(choose(&Input { back_forward: true, dir: Some(Dir::Forward), ..x() }, &State::default()), Some(SW_THRUST));
        assert_eq!(choose(&Input { dir: Some(Dir::Back), ..x() }, &State::default()), Some(SW_RISE));
        assert_eq!(choose(&x(), &State { dashed: Some(3), ..Default::default() }), Some(SW_IAI));
        assert_eq!(choose(&x(), &State { countering: true, ..Default::default() }), Some(SW_FLURRY));
        assert_eq!(choose(&Input { chord_xa: true, ..x() }, &State::default()), Some(SW_LAUNCHER));
        // mashing as the Stinger lands: the Million Stab
        let landed = State { landed: true, ..in_move(SW_THRUST, false) };
        assert_eq!(choose(&Input { mash: 3, ..x() }, &landed), Some(SW_STABS));
        assert_eq!(choose(&Input { mash: 1, ..x() }, &landed), None);
    }

    /// Y: High Time, never a spin (the user's call); its strings.
    #[test]
    fn y_and_its_strings() {
        assert_eq!(choose(&y(), &State::default()), Some(SW_HIGH_TIME));
        assert_eq!(choose(&y(), &in_move(SW_S1, true)), Some(SW_CROSS));
        assert_eq!(choose(&y(), &in_move(SW_S2, true)), Some(SW_LAUNCHER));
        assert_eq!(choose(&Input { dir: Some(Dir::Forward), ..y() }, &State::default()), Some(SW_HELM));
        // (Y, then X: just the string, no Hurricane)
        assert_eq!(choose(&x(), &in_move(SW_HIGH_TIME, true)), Some(SW_S1));
    }

    #[test]
    fn its_rb_moves_are_moves_too() {
        // (the skyward strike and the Zeus throw before they let the bolt go: X or LT wait for
        // their NEXT, or the cast is lost)
        assert_eq!(choose(&x(), &in_move(SKY_STRIKE, false)), None);
        assert_eq!(choose(&Input { lt: true, ..x() }, &State { air: true, pogos: 3, ..in_move(ZEUS_THROW, false) }), None);
        assert_eq!(choose(&x(), &in_move(SKY_STRIKE, true)), Some(SW_S1));
    }

    #[test]
    fn the_stabs_come_once_the_stinger_lands() {
        let mash = Input { mash: 3, ..x() };
        assert_eq!(choose(&mash, &in_move(SW_THRUST, false)), None);
        assert_eq!(choose(&mash, &State { landed: true, ..in_move(SW_THRUST, false) }), Some(SW_STABS));
    }

    #[test]
    fn x_then_a_is_the_launcher_too() {
        // (X a tick before A: the first slash it began gives way)
        let chord = Input { chord_xa: true, ..Default::default() };
        assert_eq!(choose(&chord, &State { playing_for: 1, ..in_move(SW_S1, false) }), Some(SW_LAUNCHER));
        assert_eq!(choose(&chord, &State { playing_for: 8, ..in_move(SW_S1, false) }), None);
    }

    #[test]
    fn up_in_the_air() {
        let air = State { air: true, pogos: 3, ..Default::default() };
        assert_eq!(choose(&x(), &air), Some(SW_RAVE));
        assert_eq!(choose(&x(), &State { playing: Some((SW_RAVE, true)), ..air }), Some(SW_RAVE + 1));
        assert_eq!(choose(&x(), &State { playing: Some((SW_RAVE + 3, true)), ..air }), Some(SW_RAVE));
        assert_eq!(choose(&y(), &air), Some(SW_BREAKER));
        assert_eq!(choose(&Input { lt: true, ..Default::default() }, &air), Some(SW_POGO));
        assert_eq!(choose(&Input { lt: true, ..Default::default() }, &State { pogos: 0, ..air }), None);
    }
}
