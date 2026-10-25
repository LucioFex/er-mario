//! Which of the Star Sword's moves (script.rs) the buttons ask for (v4 Part 2; Part 10: Dante's
//! Rebellion, no spins): its string (X x4, Combo A, a two-handed chop to end), the pause strings
//! (X, beat, X: the thunder cleave; X, X, beat, X: the Million Stab, Combo B; X, X, X, beat, X:
//! the Leap Splitter), the closers (back,
//! forward + X: the Stinger, X mashed as it lands the Million Stab; dash + X: the lightning iai; a
//! perfect dodge + X: the Flurry Rush), the launchers (back + X and Y: High Time, held he goes up
//! with them; X, X, Y and X + A: the launcher, up with them always), X, Y: the cross cut,
//! forward + Y: the Helm Splitter, and up in the air the Aerial Rave (X x4), the Helm Breaker (Y)
//! and the pogo (LT), and forward + X up there the Star Calibur. X held is charge.rs's (the Drive). Pure: moves.rs gathers what it reads and
//! starts what comes out.

use crate::combat::Attack;
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
    /// the Star Calibur was used in this jump (once an airtime)
    pub calibur_used: bool,
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
            // (forward + X out of the string: the Star Calibur, once a jump)
            if i.dir == Some(Dir::Forward) && !s.calibur_used && !is(&[SW_RAVE, SW_RAVE + 1, SW_RAVE + 2, SW_RAVE + 3]) {
                return Some(SW_CALIBUR);
            }
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
                // (X X X, a beat, X: the hop and the cleave)
                SW_S3 => return Some(SW_LEAP),
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

// ---- Star Charge (Part 12.5: Nero's Exceed, DMC4) -------------------------------------------

/// Stars on the blade at most; the next heavy blow spends them all, each worth this much more; they
/// go out after this many ticks (6 s) without a new one; a spent set's other hits (the same blow on
/// another enemy, the Leap Splitter's wave after its cleave) get its bonus within this many ticks.
pub const STAR_MAX: u8 = 3;
pub const STAR_BONUS: f32 = 0.15;
pub const STAR_IDLE: u32 = 180;
const STAR_HOLD: u32 = 12;

/// The stars a NICE! lit on the blade.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stars {
    n: u8,
    lit_at: u32,
    /// the bonus (a factor) the last spend gave, and when
    spent: Option<(f32, u32)>,
}

impl Stars {
    pub const fn new() -> Self {
        Stars { n: 0, lit_at: 0, spent: None }
    }

    /// The stars lit as of `tick` (the idle ones gone).
    pub fn count(&self, tick: u32) -> u8 {
        if self.n > 0 && tick.wrapping_sub(self.lit_at) > STAR_IDLE {
            0
        } else {
            self.n
        }
    }

    /// A NICE! at `tick`: one more star (3 at most). How many now.
    pub fn light(&mut self, tick: u32) -> u8 {
        self.n = (self.count(tick) + 1).min(STAR_MAX);
        self.lit_at = tick;
        self.n
    }

    /// A sword blow landed at `tick`, `heavy` or not: its damage factor (1 + 0.15 a star), and
    /// whether this blow spent the stars (the first hit of it). A heavy blow takes them all; a light
    /// one leaves them; a hit right after a spend (same blow, other enemy) gets the same factor.
    pub fn blow(&mut self, tick: u32, heavy: bool) -> (f32, bool) {
        if heavy {
            if let Some((f, _)) = self.spent.filter(|(_, at)| tick.wrapping_sub(*at) <= STAR_HOLD) {
                return (f, false);
            }
            let n = self.count(tick);
            if n > 0 {
                let f = 1.0 + STAR_BONUS * n as f32;
                self.n = 0;
                self.spent = Some((f, tick));
                return (f, true);
            }
        }
        (1.0, false)
    }

    /// Mario was hit, or put the sword away: they go out.
    pub fn lose(&mut self) {
        self.n = 0;
        self.spent = None;
    }
}

/// A blow the stars are spent on: the Rebellion chop, the Stinger (Bullet Bill), the Leap Splitter.
pub fn star_heavy(attack: Attack) -> bool {
    matches!(attack, Attack::SwordSlash(3) | Attack::SwordThrust | Attack::SwordLeap | Attack::SwordLeapRing)
}

static STARS: std::sync::Mutex<Stars> = std::sync::Mutex::new(Stars::new());

fn stars() -> std::sync::MutexGuard<'static, Stars> {
    STARS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Game thread: a NICE! landed with the sword out (timing.rs): a star lights.
pub fn star_lit(tick: u32) {
    if crate::weapons::current() == crate::weapons::SWORD {
        let n = stars().light(tick);
        crate::log(format!("swordplay: star {n} of {STAR_MAX} lit"));
    }
}

/// Game thread (combat.rs): a sword blow landed. Its factor, and the stars spent by it (0 if none).
pub fn star_blow(attack: Attack, tick: u32) -> (f32, u8) {
    if !crate::impact::sword_blow(attack) {
        return (1.0, 0);
    }
    let mut s = stars();
    let n = s.count(tick);
    let (f, spent) = s.blow(tick, star_heavy(attack));
    (f, if spent { n } else { 0 })
}

/// The stars to draw on the blade now.
pub fn stars_shown(tick: u32) -> u8 {
    stars().count(tick)
}

/// Mario was hit, or swapped weapons: the stars go out.
pub fn stars_lose() {
    let mut s = stars();
    if s.n > 0 {
        crate::log("swordplay: the stars go out");
    }
    s.lose();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stars: each NICE! lights one (3 at most); a heavy blow spends them all for +15% each.
    #[test]
    fn nice_lights_stars_and_a_heavy_blow_spends_them() {
        let mut s = Stars::new();
        assert_eq!(s.count(0), 0);
        assert_eq!(s.light(10), 1);
        assert_eq!(s.light(40), 2);
        assert_eq!(s.light(60), 3);
        // (a fourth: still three)
        assert_eq!(s.light(80), 3);
        // a light cut leaves them
        assert_eq!(s.blow(90, false), (1.0, false));
        assert_eq!(s.count(90), 3);
        // the chop takes the three: +45%
        let (f, spent) = s.blow(100, true);
        assert!(spent);
        assert!((f - 1.45).abs() < 1e-5, "{f}");
        assert_eq!(s.count(100), 0);
        // (the next heavy one has none)
        assert_eq!(s.blow(200, true), (1.0, false));
        // one star: +15%
        let mut s = Stars::new();
        s.light(0);
        assert!((s.blow(5, true).0 - 1.15).abs() < 1e-5);
    }

    #[test]
    fn the_same_blow_on_the_next_enemy_gets_the_bonus_too() {
        let mut s = Stars::new();
        s.light(0);
        s.light(10);
        let (f, first) = s.blow(20, true);
        assert!(first && (f - 1.3).abs() < 1e-5);
        // (the chop's sweep reaching another enemy a tick later, the Leap Splitter's wave after its cleave)
        assert_eq!(s.blow(21, true), (f, false));
        assert_eq!(s.blow(20 + STAR_HOLD, true), (f, false));
        // (but not a whole blow later)
        assert_eq!(s.blow(20 + STAR_HOLD + 1, true), (1.0, false));
        // and not on a light cut
        assert_eq!(s.blow(21, false), (1.0, false));
    }

    #[test]
    fn stars_are_lost_idle_or_on_a_hit() {
        let mut s = Stars::new();
        s.light(0);
        s.light(100);
        // 6 s (180 ticks) after the last one they're out
        assert_eq!(s.count(100 + STAR_IDLE), 2);
        assert_eq!(s.count(100 + STAR_IDLE + 1), 0);
        assert_eq!(s.blow(100 + STAR_IDLE + 1, true), (1.0, false));
        // (a new one after the lapse starts over from one)
        assert_eq!(s.light(400), 1);
        // hit, or the sword put away
        s.light(401);
        s.lose();
        assert_eq!(s.count(402), 0);
        assert_eq!(s.blow(402, true), (1.0, false));
    }

    #[test]
    fn only_the_heavy_blows_spend_stars() {
        for a in [Attack::SwordSlash(3), Attack::SwordThrust, Attack::SwordLeap, Attack::SwordLeapRing] {
            assert!(star_heavy(a), "{a:?}");
        }
        for a in [Attack::SwordSlash(0), Attack::SwordSlash(2), Attack::SwordStab, Attack::SwordCalibur, Attack::SwordCleave, Attack::SwordRave(3), Attack::Punch] {
            assert!(!star_heavy(a), "{a:?}");
        }
    }

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

    /// X X X, a beat, X: the Leap Splitter (Combo A and the other pause strings stay as they were).
    #[test]
    fn the_leap_splitter_is_x_x_x_beat_x() {
        let after = |m: u16, ago: u32| State { ended: Some((m, ago)), ..Default::default() };
        assert_eq!(choose(&x(), &after(SW_S3, 12)), Some(SW_LEAP));
        assert_eq!(choose(&x(), &after(SW_S3, BEAT.0)), Some(SW_LEAP));
        assert_eq!(choose(&x(), &after(SW_S3, BEAT.1)), Some(SW_LEAP));
        // (too soon or too late: the string again)
        assert_eq!(choose(&x(), &after(SW_S3, 2)), Some(SW_S1));
        assert_eq!(choose(&x(), &after(SW_S3, 60)), Some(SW_S1));
        // X X X X is still the chop, X X beat X the Million Stab, X beat X the cleave
        assert_eq!(choose(&x(), &in_move(SW_S3, true)), Some(SW_S4));
        assert_eq!(choose(&x(), &after(SW_S2, 20)), Some(SW_STABS));
        assert_eq!(choose(&x(), &after(SW_S1, 12)), Some(SW_CLEAVE));
        // (the chop's end isn't a leap's start)
        assert_eq!(choose(&x(), &after(SW_S4, 12)), Some(SW_S1));
    }

    /// Forward + X in the air: the Star Calibur, once a jump; X alone is still the air string.
    #[test]
    fn the_star_calibur_is_forward_x_in_the_air() {
        let air = State { air: true, pogos: 3, ..Default::default() };
        let fwd = Input { dir: Some(Dir::Forward), ..x() };
        assert_eq!(choose(&fwd, &air), Some(SW_CALIBUR));
        // (once a jump: then it is the air string's X)
        assert_eq!(choose(&fwd, &State { calibur_used: true, ..air }), Some(SW_RAVE));
        assert_eq!(choose(&x(), &air), Some(SW_RAVE));
        assert_eq!(choose(&Input { dir: Some(Dir::Back), ..x() }, &air), Some(SW_RAVE));
        // (a string already going on: forward keeps it going)
        assert_eq!(choose(&fwd, &State { playing: Some((SW_RAVE, true)), ..air }), Some(SW_RAVE + 1));
        // (Y and LT stay the Helm Breaker and the pogo)
        assert_eq!(choose(&Input { dir: Some(Dir::Forward), ..y() }, &air), Some(SW_BREAKER));
        assert_eq!(choose(&Input { lt: true, dir: Some(Dir::Forward), ..Default::default() }, &air), Some(SW_POGO));
        // (on the ground forward + X is no Calibur)
        assert_eq!(choose(&fwd, &State::default()), Some(SW_S1));
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
