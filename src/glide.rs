//! The boomerang's B in the air (Part 12.6b): the glide. Mario holds the boomerang overhead and it
//! spins like a propeller; he falls at 2.5 m/s at most and sails on at 8 m/s, steered by the stick
//! (gently), up to 3 s. Never any height out of it. Once an airtime: landing, an enemy step or the
//! hook give it back. The rule of each tick is here, er_moves.c's ACT_ER_GLIDE only applies it
//! (sm64_er_set_glide); X throws, L2 pounds, A lets go (the action and moves.rs).

/// fall speed cap and sailing speed (m/s)
pub const FALL: f32 = 2.5;
pub const SPEED: f32 = 8.0;
/// up to 3 s (SM64 ticks, 30 a second)
pub const MAX_TICKS: u32 = 90;
/// how he sinks into the cap and speeds up to the glide's pace (m/s per s)
const SINK: f32 = 14.0;
const ACCEL: f32 = 24.0;
/// the stick turns him this much a tick at most (degrees: a gentle 90 a second), above this push
const TURN: f32 = 3.0;
const STICK_MIN: f32 = 0.2;
const DT: f32 = 1.0 / 30.0;

/// What the glide does this tick (m/s, degrees), `done` it is over (time out).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Params {
    pub vy: f32,
    pub forward: f32,
    pub turn: f32,
    pub done: bool,
}

/// The tick `age` of the glide: his vertical speed `vy` and forward speed `forward` now (m/s), the
/// stick's push `stick` (0..1) and how far it points off the way he faces (degrees, + left).
pub fn step(vy: f32, forward: f32, age: u32, stick: f32, off_deg: f32) -> Params {
    // (the rise is cut at once: it starts from nothing; the fall settles into the cap)
    let vy = (vy.min(0.0) - SINK * DT).max(-FALL);
    let gap = SPEED - forward;
    let forward = if gap.abs() <= ACCEL * DT { SPEED } else { forward + ACCEL * DT * gap.signum() };
    let turn = if stick < STICK_MIN { 0.0 } else { off_deg.clamp(-TURN, TURN) };
    Params { vy, forward, turn, done: age >= MAX_TICKS }
}

#[derive(Default)]
pub struct Glide {
    used: bool,
    since: Option<u32>,
}

impl Glide {
    /// The glide may start now (in the air, not used this airtime, not going, a boomerang in hand
    /// to hold up: `in_hand`).
    pub fn ready(&self, air: bool, in_hand: u32) -> bool {
        air && !self.used && self.since.is_none() && in_hand > 0
    }

    pub fn start(&mut self, tick: u32) {
        self.used = true;
        self.since = Some(tick);
    }

    /// Ticks it has gone.
    pub fn age(&self, tick: u32) -> Option<u32> {
        self.since.map(|s| tick.wrapping_sub(s))
    }

    /// It is over (any way); it stays used.
    pub fn end(&mut self) {
        self.since = None;
    }

    /// Landing, an enemy step or the hook: it is ready again.
    pub fn give_back(&mut self) {
        self.used = false;
    }

    /// He is on the ground.
    pub fn landed(&mut self) {
        self.used = false;
        self.since = None;
    }

    pub fn clear(&mut self) {
        *self = Glide::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fall_is_capped_and_never_turns_into_a_climb() {
        // (a fast fall slows to the cap at once)
        assert_eq!(step(-12.0, 8.0, 5, 0.0, 0.0).vy, -FALL);
        // (rising: starts from nothing, then sinks into the cap, never above 0)
        let p = step(6.0, 8.0, 0, 0.0, 0.0);
        assert!(p.vy <= 0.0 && p.vy >= -FALL, "{p:?}");
        let mut vy = 6.0;
        for t in 0..20 {
            vy = step(vy, 8.0, t, 0.0, 0.0).vy;
            assert!((-FALL..=0.0).contains(&vy), "{vy}");
        }
        assert_eq!(vy, -FALL);
    }

    #[test]
    fn he_sails_at_eight_metres_a_second() {
        let mut fwd = 0.0;
        for t in 0..30 {
            let next = step(-2.5, fwd, t, 0.0, 0.0).forward;
            assert!(next >= fwd && next <= SPEED, "{next}");
            fwd = next;
        }
        assert_eq!(fwd, SPEED);
        // (faster than that, from a dash: eased down to it, not more)
        assert!(step(-2.5, 20.0, 3, 0.0, 0.0).forward < 20.0);
        assert!(step(-2.5, 20.0, 3, 0.0, 0.0).forward >= SPEED);
    }

    #[test]
    fn the_stick_turns_him_gently() {
        // (a push off to the left: the full turn; the right: back; a little: its share)
        assert_eq!(step(-2.5, 8.0, 5, 1.0, 90.0).turn, TURN);
        assert_eq!(step(-2.5, 8.0, 5, 1.0, -90.0).turn, -TURN);
        assert_eq!(step(-2.5, 8.0, 5, 1.0, 1.0).turn, 1.0);
        // (no push: straight on)
        assert_eq!(step(-2.5, 8.0, 5, 0.1, 90.0).turn, 0.0);
        assert!(TURN * 30.0 <= 120.0, "gentle: at most 120 degrees a second");
    }

    #[test]
    fn it_lasts_three_seconds() {
        assert!(!step(-2.5, 8.0, MAX_TICKS - 1, 0.0, 0.0).done);
        assert!(step(-2.5, 8.0, MAX_TICKS, 0.0, 0.0).done);
        assert_eq!(MAX_TICKS, 90);
    }

    #[test]
    fn once_an_airtime_and_landing_gives_it_back() {
        let mut g = Glide::default();
        // (not on the ground)
        assert!(!g.ready(false, 3));
        assert!(g.ready(true, 3));
        g.start(100);
        assert_eq!(g.age(105), Some(5));
        assert!(!g.ready(true, 3), "going");
        g.end();
        assert_eq!(g.age(106), None);
        assert!(!g.ready(true, 3), "used up this airtime");
        // (an enemy step or the hook)
        g.give_back();
        assert!(g.ready(true, 3));
        g.start(200);
        g.end();
        // (the ground: any tick on it gives it back)
        assert!(!g.ready(true, 3));
        g.landed();
        assert!(g.ready(true, 3));
    }

    #[test]
    fn no_glide_with_every_boomerang_out() {
        let g = Glide::default();
        assert!(!g.ready(true, 0), "nothing to hold up");
        assert!(g.ready(true, 1));
    }
}
