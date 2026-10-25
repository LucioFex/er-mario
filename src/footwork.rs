//! Footwork (v4 Part 12, the user's "pasitos de un lado a otro del enemigo"): the Star Sword's
//! blows are fought on the feet. `step` plans one short step per blow (pure: the cut's side, the
//! enemy's body, walls), `walk` is how the body covers it, `Feet` plants each foot in the world and
//! swings it to the plan while the body turns (leg IK, ik.rs, puts the ankles there; lib.rs drives
//! it). All in SM64's horizontal frame, metres: x, z; yaw 0 faces +z, his left is +x.

use std::sync::Mutex;

use glam::{Vec2, Vec3};

use crate::engine_mario::{PartPose, PARTS};
use crate::{ik, sm64};

/// a step is this long (m) on the lighter blows, this on the heavy ones; never more than MAX_STEP
const LIGHT_STEP: f32 = 0.28;
const HEAVY_STEP: f32 = 0.34;
pub const MAX_STEP: f32 = 0.35;
/// the feet stay this far from the enemy's centre, past its radius (m): the blade's reach band
const BAND_NEAR: f32 = 0.6;
const BAND_FAR: f32 = 1.3;
/// bodies this big (m), or a colossus, are only closed on and backed off from
const BIG_RADIUS: f32 = 1.4;
/// most a step orbits round the enemy
const MAX_ORBIT: f32 = 25.0 * std::f32::consts::PI / 180.0;
/// the body covers a step in this many ticks, eased out
pub const STEP_TICKS: u16 = 4;
/// the lead foot's arc (m), and the back foot's delay (ticks)
const LIFT: f32 = 0.06;
const TRAIL_DELAY: i32 = 3;
/// the lead foot lands this much past its stance (m)
const LEAD_REACH: f32 = 0.05;
/// the hips shift to the lead foot this far (m) and drop a little, for this many ticks
const SHIFT: f32 = 0.03;
const SHIFT_DROP: f32 = 0.015;
const SHIFT_TICKS: i32 = 8;
/// a heavy blow: the stance widens, the knees bend (hips down, never past HIP_DROP_MAX) a while
const WIDEN: f32 = 1.25;
const CROUCH: f32 = 0.075;
const CROUCH_TICKS: i32 = 8;
pub const HIP_DROP_MAX: f32 = 0.08;
/// a planted foot farther than this share of its leg's length from its hip re-steps
const REACH_SHARE: f32 = 0.97;

/// An enemy to fight round: its centre and body radius (m, horizontal), and whether it's a
/// colossus (climbed, not circled).
#[derive(Clone, Copy, Debug)]
pub struct Foe {
    pub pos: Vec2,
    pub radius: f32,
    pub colossus: bool,
}

/// What a step is planned from: his feet (the middle of them) and facing (the yaw he's turned to),
/// the enemy if any, the cut's side (+1 right, -1 left, 0 none: the other side than last time),
/// and whether the blow is a heavy one.
#[derive(Clone, Copy, Debug)]
pub struct Ask {
    pub feet: Vec2,
    pub facing: f32,
    pub foe: Option<Foe>,
    pub side: f32,
    pub last_side: f32,
    pub heavy: bool,
}

/// The planned step: where his feet end up, the yaw he ends facing (the enemy), and the foot that
/// leads (right when the step went to his right).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub to: Vec2,
    pub face: f32,
    pub lead_right: bool,
}

/// (forward, his left) for a yaw
fn basis(yaw: f32) -> (Vec2, Vec2) {
    let (s, c) = yaw.sin_cos();
    (Vec2::new(s, c), Vec2::new(c, -s))
}

/// The yaw from `from` toward `to`.
pub fn yaw_to(from: Vec2, to: Vec2) -> f32 {
    (to.x - from.x).atan2(to.y - from.y)
}

/// Plans a step. `blocked(from, to)`: a wall in the way. Diagonally forward toward the cut's side;
/// round the enemy at most MAX_ORBIT a blow; always inside the blade's reach band (out of its
/// body); the other side if a wall is in the way, else in place; with no enemy a short step ahead.
pub fn step(a: &Ask, blocked: impl Fn(Vec2, Vec2) -> bool) -> Step {
    let first = if a.side != 0.0 {
        a.side.signum()
    } else if a.last_side != 0.0 {
        -a.last_side.signum()
    } else {
        1.0
    };
    let len = if a.heavy { HEAVY_STEP } else { LIGHT_STEP };
    let face = |to: Vec2| a.foe.map_or(a.facing, |f| yaw_to(to, f.pos));
    let Some(foe) = a.foe else {
        let to = a.feet + basis(a.facing).0 * len;
        let to = if blocked(a.feet, to) { a.feet } else { to };
        return Step { to, face: a.facing, lead_right: first > 0.0 };
    };
    for s in [first, -first] {
        let to = toward(a, &foe, s, len);
        if (to - a.feet).length_squared() < 1e-8 || !blocked(a.feet, to) {
            return Step { to, face: face(to), lead_right: s > 0.0 };
        }
    }
    Step { to: a.feet, face: face(a.feet), lead_right: first > 0.0 }
}

/// Where a step of `len` toward side `s` (+ right) ends, before the walls.
fn toward(a: &Ask, f: &Foe, s: f32, len: f32) -> Vec2 {
    let (lo, hi) = (f.radius + BAND_NEAR, f.radius + BAND_FAR);
    let rad = a.feet - f.pos;
    let dist = rad.length();
    let u = if dist > 1e-4 { rad / dist } else { -basis(a.facing).0 };
    let p = if f.radius > BIG_RADIUS || f.colossus {
        // (no orbit: closer, or back toward the middle of the band)
        let want = (lo + hi) * 0.5;
        f.pos + u * (dist + (want - dist).clamp(-len, len))
    } else {
        let fu = -u;
        let left = Vec2::new(fu.y, -fu.x);
        let p = a.feet + (fu - left * s) * (len * std::f32::consts::FRAC_1_SQRT_2);
        // (round him by 25 degrees at most)
        let q = p - f.pos;
        let ang = (rad.x * q.y - rad.y * q.x).atan2(rad.dot(q));
        if ang.abs() > MAX_ORBIT {
            let (sn, cs) = (ang.signum() * MAX_ORBIT).sin_cos();
            let r = rad / dist.max(1e-4);
            f.pos + Vec2::new(r.x * cs - r.y * sn, r.x * sn + r.y * cs) * q.length()
        } else {
            p
        }
    };
    // (into the reach band, and no longer than a step)
    let q = p - f.pos;
    let n = q.length();
    let p = if n > 1e-4 { f.pos + q / n * n.clamp(lo, hi) } else { f.pos + u * lo };
    let d = p - a.feet;
    if d.length() > MAX_STEP {
        a.feet + d.normalize() * MAX_STEP
    } else {
        p
    }
}

/// The share of what is left of a step that the body covers on tick `t` of it (eased out; all of
/// it on the last tick; 0 after).
pub fn share(t: u16) -> f32 {
    if t >= STEP_TICKS {
        return 0.0;
    }
    let n = STEP_TICKS as f32;
    let done = |k: f32| 1.0 - (1.0 - k / n) * (1.0 - k / n);
    let (a, b) = (done(t as f32), done(t as f32 + 1.0));
    ((b - a) / (1.0 - a)).clamp(0.0, 1.0)
}

/// What the body moves on tick `t` of a step to `to`, from `pos` with the `facing` it has then:
/// (forward, to his left) in m. Re-aimed at what is left every tick: a wall or a hit-stop
/// can't make it overshoot.
pub fn walk(pos: Vec2, to: Vec2, facing: f32, t: u16) -> (f32, f32) {
    let d = (to - pos) * share(t);
    let (f, l) = basis(facing);
    (d.dot(f), d.dot(l))
}

/// The legs' rig this tick, in SM64's frame relative to Mario (x, z horizontal; y up), metres:
/// each hip and ankle as the clip has them ([left, right]) and the legs' lengths.
#[derive(Clone, Copy)]
pub struct Rig {
    pub hip: [Vec3; 2],
    pub ankle: [Vec3; 2],
    pub reach: [f32; 2],
}

/// What to do with the legs this tick: by what weight, the ankles' places relative to Mario (the
/// lift in y is on top of the clip's ankle height), and how the hips move (x, z along the world,
/// y down).
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub weight: f32,
    pub ankle: [Vec3; 2],
    pub pelvis: Vec3,
}

/// One foot: planted at `plant` (x, z world; y relative to Mario) or swinging to `to` from `from`
/// (`t`: ticks into the swing; negative: waiting; STEP_TICKS: planted); `rest` is where it
/// stands in his own frame (left, y, forward).
#[derive(Clone, Copy)]
struct Foot {
    plant: Vec3,
    from: Vec3,
    to: Vec3,
    cur: Vec3,
    lift: f32,
    t: i32,
    rest: Vec3,
}

impl Default for Foot {
    fn default() -> Self {
        Foot { plant: Vec3::ZERO, from: Vec3::ZERO, to: Vec3::ZERO, cur: Vec3::ZERO, lift: 0.0, t: STEP_TICKS as i32, rest: Vec3::ZERO }
    }
}

/// Both feet planted in the world while a move plays: the state between ticks.
#[derive(Default)]
pub struct Feet {
    live: bool,
    w: f32,
    feet: [Foot; 2],
    body: Vec3,
    lead: usize,
    shift: i32,
    crouch: i32,
    widen: f32,
    pelvis: Vec3,
    /// a step planned before the feet are planted: fired as they are
    pending: Option<(Step, bool)>,
}

impl Feet {
    /// Where foot `i` stands for a body at `body`, turned to `yaw` (the stance widened or not).
    fn stance(&self, i: usize, body: Vec2, yaw: f32) -> Vec3 {
        let (f, l) = basis(yaw);
        let r = self.feet[i].rest;
        let at = body + l * (r.x * self.widen) + f * r.z;
        Vec3::new(at.x, r.y, at.y)
    }

    /// A planned step fires: the lead foot lifts and lands at its new place over STEP_TICKS, the
    /// trailing one TRAIL_DELAY ticks after it; a heavy blow widens the stance and bends the knees.
    pub fn step(&mut self, s: &Step, heavy: bool) {
        if !self.live {
            self.pending = Some((*s, heavy));
            return;
        }
        for f in self.feet.iter_mut().filter(|f| f.t < STEP_TICKS as i32) {
            f.plant = f.to;
            f.t = STEP_TICKS as i32;
        }
        self.widen = if heavy { WIDEN } else { 1.0 };
        let lead = s.lead_right as usize;
        let dir = (s.to - Vec2::new(self.body.x, self.body.z)).try_normalize().unwrap_or_else(|| basis(s.face).0);
        for i in 0..2 {
            let mut to = self.stance(i, s.to, s.face);
            if i == lead {
                to += Vec3::new(dir.x, 0.0, dir.y) * LEAD_REACH;
            }
            let f = &mut self.feet[i];
            f.from = f.plant;
            f.to = to;
            f.t = if i == lead { 0 } else { -TRAIL_DELAY };
        }
        self.lead = lead;
        self.shift = SHIFT_TICKS;
        if heavy {
            self.crouch = CROUCH_TICKS;
        }
    }

    /// One tick: `on` while a move that uses the footwork plays (else it blends out over 3 ticks);
    /// `body` and `yaw` as Mario is now; `rig`: the clip's legs. None when it's all off.
    pub fn tick(&mut self, on: bool, body: Vec3, yaw: f32, rig: &Rig) -> Option<Pose> {
        const BLEND: f32 = 1.0 / 3.0;
        if on {
            if !self.live {
                let pending = self.pending.take();
                self.engage(body, yaw, rig);
                if let Some((s, heavy)) = pending {
                    self.step(&s, heavy);
                }
            }
            self.w = (self.w + BLEND).min(1.0);
        } else if self.live {
            self.w -= BLEND;
            if self.w <= 0.001 {
                *self = Feet::default();
                return None;
            }
        } else {
            return None;
        }
        self.body = body;
        let b2 = Vec2::new(body.x, body.z);
        let (_, left) = basis(yaw);
        // (a planted foot the body has left too far behind steps to its stance, the second after the first)
        let mut late = 0;
        for i in 0..2 {
            let f = self.feet[i];
            if f.t < STEP_TICKS as i32 {
                continue;
            }
            let hip = rig.hip[i] + self.pelvis;
            let goal = Vec3::new(f.plant.x - body.x, f.plant.y, f.plant.z - body.z);
            if (goal - hip).length() > rig.reach[i] * REACH_SHARE {
                let to = self.stance(i, b2, yaw);
                let f = &mut self.feet[i];
                f.from = f.plant;
                f.to = to;
                f.t = -late * TRAIL_DELAY;
                late += 1;
            }
        }
        let n = STEP_TICKS as i32;
        for f in self.feet.iter_mut() {
            f.lift = 0.0;
            f.cur = f.plant;
            if f.t < 0 {
                f.t += 1;
            } else if f.t < n {
                let u = (f.t + 1) as f32 / n as f32;
                f.cur = f.from.lerp(f.to, u * u * (3.0 - 2.0 * u));
                f.lift = LIFT * (std::f32::consts::PI * u).sin();
                f.t += 1;
                if f.t >= n {
                    f.plant = f.to;
                }
            }
        }
        // the hips: toward the lead foot and a little down while it lands, down on a heavy blow
        let mut want = Vec3::ZERO;
        if self.shift > 0 {
            let side = if self.lead == 0 { 1.0 } else { -1.0 };
            want += Vec3::new(left.x * side * SHIFT, -SHIFT_DROP, left.y * side * SHIFT);
            self.shift -= 1;
        }
        if self.crouch > 0 {
            want.y -= CROUCH;
            self.crouch -= 1;
        }
        want.y = want.y.max(-HIP_DROP_MAX);
        self.pelvis += (want - self.pelvis) * 0.5;
        self.pelvis.y = self.pelvis.y.clamp(-HIP_DROP_MAX, 0.0);
        let ankle = |f: &Foot| Vec3::new(f.cur.x - body.x, f.cur.y + f.lift, f.cur.z - body.z);
        Some(Pose { weight: self.w, ankle: [ankle(&self.feet[0]), ankle(&self.feet[1])], pelvis: self.pelvis })
    }

    /// Plants both feet where the clip has them.
    fn engage(&mut self, body: Vec3, yaw: f32, rig: &Rig) {
        let (f, l) = basis(yaw);
        *self = Feet { live: true, widen: 1.0, body, ..Feet::default() };
        for i in 0..2 {
            let a = rig.ankle[i];
            let h = Vec2::new(a.x, a.z);
            self.feet[i].rest = Vec3::new(h.dot(l), a.y, h.dot(f));
            self.feet[i].plant = Vec3::new(body.x + a.x, a.y, body.z + a.z);
        }
    }
}

// ---- driving it: the body's step and the legs' IK (lib.rs) ----

/// The footwork between ticks: the step the body is walking and the feet.
#[derive(Default)]
struct Footing {
    feet: Feet,
    to: Option<Vec2>,
    t: u16,
    foe: Option<Vec2>,
    last_side: f32,
}

static FOOTING: Mutex<Option<Footing>> = Mutex::new(None);

/// A footwork move is playing on the ground (lib.rs): whether it lunges on its own (then the step
/// only goes sideways), and on its first tick what the step is planned from.
#[derive(Clone, Copy)]
pub struct Plan {
    pub lunge: bool,
    pub ask: Option<Ask>,
}

/// A wall, or no floor, where a step would end (SM64 units: `y` his height, `k` m a unit).
fn blocked(from: Vec2, to: Vec2, y: f32, k: f32) -> bool {
    let at = |p: Vec2| {
        let (mut x, mut wy, mut z) = (p.x / k, y + 30.0, p.y / k);
        // (Mario's own width, as the walk's walls see him)
        let wall = unsafe { sm64::sm64_surface_find_wall_collision(&mut x, &mut wy, &mut z, 30.0, 50.0) } != 0;
        let floor = unsafe { sm64::sm64_surface_find_floor_height(p.x / k, y + 80.0, p.y / k) };
        wall || floor < y - 120.0
    };
    at(to) || at((from + to) * 0.5)
}

/// Before the tick (in the SM64 worker): plans the step on a move's first tick and walks the body
/// through it, on top of the move's own speed: sideways by `side_vel`, a little forward when the
/// move doesn't lunge itself, his facing kept on the enemy meanwhile. `pos`, `face`: Mario as
/// the last tick left him; `k`: metres a unit.
pub fn drive(c: &mut sm64::SM64ErScript, plan: &Plan, pos: [f32; 3], face: f32, k: f32) {
    let mut g = FOOTING.lock().unwrap_or_else(|e| e.into_inner());
    let ft = g.get_or_insert_with(Footing::default);
    let here = Vec2::new(pos[0] * k, pos[2] * k);
    let mut facing = face;
    if let Some(mut ask) = plan.ask {
        ask.last_side = ft.last_side;
        let s = step(&ask, |a, b| blocked(a, b, pos[1], k));
        ft.feet.step(&s, ask.heavy);
        (ft.to, ft.t, ft.foe) = (Some(s.to), 0, ask.foe.map(|f| f.pos));
        ft.last_side = if s.lead_right { 1.0 } else { -1.0 };
        // (the move's own aim turns him to the enemy on this tick)
        facing = ask.facing;
    }
    let Some(to) = ft.to else { return };
    let mut after = facing + c.turn.to_radians();
    if let (Some(foe), true) = (ft.foe, plan.ask.is_none()) {
        let off = (yaw_to(here, foe) - after + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let extra = off.to_degrees().clamp(-TURN_RATE, TURN_RATE);
        c.turn += extra;
        after += extra.to_radians();
    }
    let (fwd, side) = walk(here, to, after, ft.t);
    if !plan.lunge {
        c.forward_vel += fwd / k;
    }
    c.side_vel += side / k;
    ft.t += 1;
    if ft.t >= STEP_TICKS {
        ft.to = None;
    }
}

/// most the step turns him back to the enemy a tick (degrees)
const TURN_RATE: f32 = 12.0;

/// After the tick (in the worker), on Mario's poses: the feet planted in the world, the hips
/// shifted, the legs' IK (ik.rs). `on`: a footwork move is playing; else it blends out over 3
/// ticks. `pos`, `face`: Mario now; `k`: metres a unit.
pub fn legs(parts: &mut [PartPose; PARTS], on: bool, pos: [f32; 3], face: f32, k: f32) {
    let mut g = FOOTING.lock().unwrap_or_else(|e| e.into_inner());
    if !on && !g.as_ref().is_some_and(|f| f.feet.live) {
        return;
    }
    let ft = g.get_or_insert_with(Footing::default);
    // (the parts' x is mirrored; left/right as he stands: the thigh that is on his left is first)
    let mirror = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
    let (fwd, left) = basis(face);
    let lat = |v: Vec3| mirror(v).x * left.x + mirror(v).z * left.y;
    let legs: [[usize; 3]; 2] = if lat(parts[10].pos) >= lat(parts[13].pos) { [[10, 11, 12], [13, 14, 15]] } else { [[13, 14, 15], [10, 11, 12]] };
    let len = |l: &[usize; 3]| (parts[l[1]].pos - parts[l[0]].pos).length() + (parts[l[2]].pos - parts[l[1]].pos).length();
    let rig = Rig {
        hip: [mirror(parts[legs[0][0]].pos), mirror(parts[legs[1][0]].pos)],
        ankle: [mirror(parts[legs[0][2]].pos), mirror(parts[legs[1][2]].pos)],
        reach: [len(&legs[0]), len(&legs[1])],
    };
    let body = Vec3::new(pos[0] * k, pos[1] * k, pos[2] * k);
    let Some(out) = ft.feet.tick(on, body, face, &rig) else {
        (ft.to, ft.last_side) = (None, 0.0);
        return;
    };
    // the weight shifts: all of him but the legs moves with the hips
    let shift = mirror(out.pelvis) * out.weight;
    for (i, p) in parts.iter_mut().enumerate().skip(1) {
        if !matches!(i, 11 | 12 | 14 | 15) {
            p.pos += shift;
        }
    }
    // the knees bend forward
    let pole = Vec3::new(-fwd.x, 0.0, fwd.y);
    for (i, l) in legs.iter().enumerate() {
        ik::leg(parts, l[0] == 13, mirror(out.ankle[i]), pole, out.weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEG: f32 = std::f32::consts::PI / 180.0;

    fn wrap(a: f32) -> f32 {
        (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    }

    /// Mario at `feet` facing the foe at the origin; `r` its radius.
    fn ask(feet: Vec2, r: f32, side: f32) -> Ask {
        let facing = (-feet.x).atan2(-feet.y);
        Ask { feet, facing, foe: Some(Foe { pos: Vec2::ZERO, radius: r, colossus: false }), side, last_side: 0.0, heavy: false }
    }

    fn at(dist: f32, deg: f32) -> Vec2 {
        Vec2::new((deg * DEG).sin(), (deg * DEG).cos()) * dist
    }

    /// The signed angle about the origin from `a` to `b` (degrees).
    fn orbit(a: Vec2, b: Vec2) -> f32 {
        wrap(b.x.atan2(b.y) - a.x.atan2(a.y)) / DEG
    }

    fn free(_: Vec2, _: Vec2) -> bool {
        false
    }

    #[test]
    fn the_feet_stay_in_the_blades_reach_band_and_out_of_the_body() {
        for r in [0.3f32, 0.5, 0.9, 1.3] {
            for dist in [r + 0.6, r + 0.8, r + 1.0, r + 1.3] {
                for deg in [0.0, 70.0, 180.0, -120.0] {
                    for side in [-1.0f32, 1.0] {
                        for heavy in [false, true] {
                            let mut a = ask(at(dist, deg), r, side);
                            a.heavy = heavy;
                            let s = step(&a, free);
                            let d = s.to.length();
                            assert!(d >= r + 0.6 - 1e-3 && d <= r + 1.3 + 1e-3, "r {r} from {dist} side {side}: ended {d} from its centre");
                            assert!((s.to - a.feet).length() <= MAX_STEP + 1e-4, "stepped {}", (s.to - a.feet).length());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_step_walks_diagonally_forward_to_the_cuts_side() {
        // (out at the band's far edge: room to come in; right = +1)
        let a = ask(at(1.8, 0.0), 0.5, 1.0);
        let s = step(&a, free);
        let d = s.to - a.feet;
        assert!(d.length() >= 0.25 && d.length() <= 0.35, "step {}", d.length());
        assert!(s.to.length() < a.feet.length() - 0.05, "not forward");
        // (he faces the origin from +z, so his right is toward +x)
        assert!(d.x > 0.1, "not to his right: {d:?}");
        assert!(s.lead_right);
        let l = step(&ask(at(1.8, 0.0), 0.5, -1.0), free);
        assert!((l.to - a.feet).x < -0.1 && !l.lead_right);
    }

    #[test]
    fn he_ends_facing_the_enemy() {
        for deg in [0.0, 33.0, -90.0, 170.0] {
            for side in [-1.0f32, 1.0] {
                let a = ask(at(1.5, deg), 0.5, side);
                let s = step(&a, free);
                let want = (-s.to.x).atan2(-s.to.y);
                assert!(wrap(s.face - want).abs() < 2.0 * DEG, "off by {}", wrap(s.face - want) / DEG);
            }
        }
    }

    #[test]
    fn a_chain_alternates_sides_and_orbits_at_most_25_degrees_a_blow() {
        let mut feet = at(1.5, 0.0);
        let mut last = 0.0;
        let mut dirs = Vec::new();
        for _ in 0..4 {
            let mut a = ask(feet, 0.5, 0.0);
            a.last_side = last;
            let s = step(&a, free);
            let o = orbit(feet, s.to);
            assert!(o.abs() <= 25.0 + 0.01, "orbited {o}");
            assert!(o.abs() > 2.0, "no orbit at all: {o}");
            dirs.push(o.signum());
            last = if s.lead_right { 1.0 } else { -1.0 };
            feet = s.to;
        }
        assert!(dirs.windows(2).all(|w| w[0] != w[1]), "sides {dirs:?}");
        // (a close enemy: the orbit is the biggest, still capped)
        let s = step(&ask(at(1.1, 0.0), 0.5, 1.0), free);
        assert!(orbit(at(1.1, 0.0), s.to).abs() <= 25.0 + 0.01);
    }

    #[test]
    fn a_wall_sends_the_step_to_the_other_side_or_keeps_it_in_place() {
        let a = ask(at(1.5, 0.0), 0.5, 1.0);
        // (a wall on his right, toward +x of where he stands)
        let right_wall = |_: Vec2, to: Vec2| to.x > a.feet.x + 0.02;
        let s = step(&a, right_wall);
        assert!(s.to.x < a.feet.x - 0.05, "went {:?}", s.to);
        assert!(!s.lead_right);
        let walled = |_: Vec2, to: Vec2| (to - a.feet).length() > 0.01;
        let s = step(&a, walled);
        assert_eq!(s.to, a.feet);
        let want = (-a.feet.x).atan2(-a.feet.y);
        assert!(wrap(s.face - want).abs() < 2.0 * DEG);
    }

    #[test]
    fn big_bodies_get_no_orbit_only_forward_and_back() {
        for (r, colossus) in [(2.0f32, false), (0.5, true)] {
            for (dist, want) in [(r + 1.3, -1.0f32), (r + 0.6, 1.0), (r + 0.95, 0.0)] {
                let mut a = ask(at(dist, 40.0), r, 1.0);
                a.foe = Some(Foe { pos: Vec2::ZERO, radius: r, colossus });
                let s = step(&a, free);
                assert!(orbit(a.feet, s.to).abs() < 0.01, "orbited {}", orbit(a.feet, s.to));
                let moved = s.to.length() - dist;
                assert!(moved * want >= -1e-3 && (s.to - a.feet).length() <= MAX_STEP + 1e-4, "r {r} dist {dist}: moved {moved}");
                assert!(s.to.length() >= r + 0.6 - 1e-3 && s.to.length() <= r + 1.3 + 1e-3);
            }
        }
    }

    #[test]
    fn with_no_enemy_it_is_a_short_step_forward() {
        let facing = 0.7f32;
        let a = Ask { feet: Vec2::new(3.0, -2.0), facing, foe: None, side: -1.0, last_side: 0.0, heavy: false };
        let s = step(&a, free);
        let d = s.to - a.feet;
        assert!(d.length() > 0.2 && d.length() <= 0.35 + 1e-4, "{}", d.length());
        assert!(wrap(d.x.atan2(d.y) - facing).abs() < 0.01);
        assert!(wrap(s.face - facing).abs() < 0.01);
        let s = step(&a, |_, _| true);
        assert_eq!(s.to, a.feet);
    }

    #[test]
    fn the_step_is_covered_over_four_ticks_and_exactly() {
        let mut left = 1.0f32;
        for t in 0..STEP_TICKS {
            let s = share(t);
            assert!(s > 0.0 && s <= 1.0);
            left *= 1.0 - s;
        }
        assert!(left.abs() < 1e-5, "left {left}");
        assert_eq!(share(STEP_TICKS), 0.0);
        // (eased out: the first tick covers the most)
        assert!(share(0) * 1.0 > 0.4);
    }

    #[test]
    fn the_body_walks_the_planned_step_exactly() {
        let (mut pos, to) = (Vec2::new(1.0, 2.0), Vec2::new(1.2, 2.25));
        let mut facing = 0.4f32;
        for t in 0..STEP_TICKS {
            // (turning meanwhile: he moves along the facing he has by then)
            facing += 0.05;
            let (fwd, side) = walk(pos, to, facing, t);
            let (f, l) = basis(facing);
            pos += f * fwd + l * side;
        }
        assert!((pos - to).length() < 1e-4, "ended at {pos:?}");
        assert_eq!(walk(pos, to, facing, STEP_TICKS), (0.0, 0.0));
    }

    // ---- planting ----

    /// Hips 0.75 m over their ankles, ankles 0.1 m either side of him (left first), legs 0.8 m.
    fn rig() -> Rig {
        Rig { hip: [Vec3::new(0.1, 0.75, 0.0), Vec3::new(-0.1, 0.75, 0.0)], ankle: [Vec3::new(0.1, 0.0, 0.0), Vec3::new(-0.1, 0.0, 0.0)], reach: [0.8, 0.8] }
    }

    fn world(body: Vec3, o: &Pose, i: usize) -> Vec2 {
        Vec2::new(body.x + o.ankle[i].x, body.z + o.ankle[i].z)
    }

    fn plan(to: Vec2, face: f32, right: bool) -> Step {
        Step { to, face, lead_right: right }
    }

    #[test]
    fn a_planted_foot_does_not_skate_while_the_body_turns_and_drifts() {
        let mut f = Feet::default();
        let first = f.tick(true, Vec3::ZERO, 0.0, &rig()).unwrap();
        let (l0, r0) = (world(Vec3::ZERO, &first, 0), world(Vec3::ZERO, &first, 1));
        for k in 1..8 {
            let body = Vec3::new(0.012 * k as f32, 0.0, -0.01 * k as f32);
            let yaw = 12.0 * DEG * k as f32;
            let o = f.tick(true, body, yaw, &rig()).unwrap();
            assert!((world(body, &o, 0) - l0).length() < 1e-4, "left skated at {k}");
            assert!((world(body, &o, 1) - r0).length() < 1e-4, "right skated at {k}");
        }
    }

    #[test]
    fn the_lead_foot_lifts_an_arc_of_6_cm_and_lands_in_4_ticks() {
        let mut f = Feet::default();
        f.tick(true, Vec3::ZERO, 0.0, &rig());
        f.step(&plan(Vec2::new(0.0, 0.3), 0.0, true), false);
        let mut ys = Vec::new();
        let mut last = Vec2::ZERO;
        for _ in 0..4 {
            let o = f.tick(true, Vec3::new(0.0, 0.0, 0.3), 0.0, &rig()).unwrap();
            ys.push(o.ankle[1].y);
            last = world(Vec3::new(0.0, 0.0, 0.3), &o, 1);
        }
        let peak = ys.iter().cloned().fold(0.0, f32::max);
        assert!(peak > 0.05 && peak <= 0.0605, "peak {peak} of {ys:?}");
        assert!(ys[3].abs() < 1e-4 && ys.iter().all(|y| *y >= -1e-5), "{ys:?}");
        // (landed forward of where it stood, at the new stance)
        assert!(last.y > 0.25, "landed at {last:?}");
        let o = f.tick(true, Vec3::new(0.0, 0.0, 0.3), 0.0, &rig()).unwrap();
        assert!((world(Vec3::new(0.0, 0.0, 0.3), &o, 1) - last).length() < 1e-4);
    }

    #[test]
    fn the_trailing_foot_follows_three_ticks_later() {
        let mut f = Feet::default();
        let body0 = Vec3::ZERO;
        let o = f.tick(true, body0, 0.0, &rig()).unwrap();
        let left0 = world(body0, &o, 0);
        f.step(&plan(Vec2::new(0.0, 0.3), 0.0, true), false);
        let body = Vec3::new(0.0, 0.0, 0.3);
        for k in 0..3 {
            let o = f.tick(true, body, 0.0, &rig()).unwrap();
            assert!((world(body, &o, 0) - left0).length() < 1e-4 && o.ankle[0].y.abs() < 1e-5, "left moved at tick {k}");
        }
        let o = f.tick(true, body, 0.0, &rig()).unwrap();
        assert!((world(body, &o, 0) - left0).length() > 1e-3, "left still at tick 3");
    }

    #[test]
    fn a_foot_left_too_far_behind_re_steps() {
        let mut f = Feet::default();
        f.tick(true, Vec3::ZERO, 0.0, &rig());
        // (the body ran 0.7 m on: both planted feet are out of the legs' reach)
        let body = Vec3::new(0.0, 0.0, 0.7);
        let mut o = f.tick(true, body, 0.0, &rig()).unwrap();
        assert!(o.ankle[0].y > 0.0 || o.ankle[1].y > 0.0, "no foot lifted");
        for _ in 0..6 {
            o = f.tick(true, body, 0.0, &rig()).unwrap();
        }
        for i in 0..2 {
            assert!((o.ankle[i].x - rig().ankle[i].x).abs() < 0.03 && o.ankle[i].z.abs() < 0.05, "foot {i} at {:?}", o.ankle[i]);
            assert!(o.ankle[i].y.abs() < 1e-4);
        }
    }

    #[test]
    fn the_hips_shift_to_the_lead_foot_and_never_sink_below_the_safe_floor() {
        let mut f = Feet::default();
        f.tick(true, Vec3::ZERO, 0.0, &rig());
        f.step(&plan(Vec2::new(0.0, 0.3), 0.0, false), false);
        let body = Vec3::new(0.0, 0.0, 0.3);
        let mut shift = 0.0f32;
        for _ in 0..8 {
            let o = f.tick(true, body, 0.0, &rig()).unwrap();
            shift = shift.max(o.pelvis.x);
            assert!(o.pelvis.y <= 1e-5 && o.pelvis.y >= -HIP_DROP_MAX - 1e-5, "hips at {}", o.pelvis.y);
        }
        // (the left foot led: toward +x, a few cm)
        assert!(shift > 0.01 && shift <= 0.045, "shift {shift}");
        // (heavy blows, again and again: never lower than the floor)
        let mut low = 0.0f32;
        for n in 0..12 {
            f.step(&plan(Vec2::new(0.0, 0.3 + 0.3 * n as f32), 0.0, n % 2 == 0), true);
            for _ in 0..6 {
                let b = Vec3::new(0.0, 0.0, 0.3 + 0.3 * n as f32);
                let o = f.tick(true, b, 0.0, &rig()).unwrap();
                low = low.min(o.pelvis.y);
                assert!(o.pelvis.y >= -HIP_DROP_MAX - 1e-5);
            }
        }
        assert!(low < -0.05, "a heavy blow didn't bend the knees: {low}");
    }

    #[test]
    fn a_heavy_blow_widens_the_stance() {
        let mut f = Feet::default();
        f.tick(true, Vec3::ZERO, 0.0, &rig());
        f.step(&plan(Vec2::new(0.0, 0.3), 0.0, true), true);
        let body = Vec3::new(0.0, 0.0, 0.3);
        let mut o = f.tick(true, body, 0.0, &rig()).unwrap();
        for _ in 0..8 {
            o = f.tick(true, body, 0.0, &rig()).unwrap();
        }
        assert!(o.ankle[0].x > 0.1 * 1.2 && o.ankle[1].x < -0.1 * 1.2, "{:?} {:?}", o.ankle[0], o.ankle[1]);
    }

    #[test]
    fn a_step_planned_before_the_first_tick_fires_as_the_feet_plant() {
        let mut f = Feet::default();
        f.step(&plan(Vec2::new(0.0, 0.3), 0.0, true), false);
        let body = Vec3::new(0.0, 0.0, 0.3);
        let o = f.tick(true, Vec3::ZERO, 0.0, &rig()).unwrap();
        assert!(o.ankle[1].y > 0.01, "the lead foot didn't lift: {:?}", o.ankle[1]);
        let mut o = o;
        for _ in 0..8 {
            o = f.tick(true, body, 0.0, &rig()).unwrap();
        }
        assert!(world(body, &o, 1).y > 0.3, "landed at {:?}", world(body, &o, 1));
    }

    #[test]
    fn it_blends_in_and_out_over_3_ticks() {
        let mut f = Feet::default();
        let w: Vec<f32> = (0..4).map(|_| f.tick(true, Vec3::ZERO, 0.0, &rig()).unwrap().weight).collect();
        assert!((w[0] - 1.0 / 3.0).abs() < 1e-4 && w[2] > 0.99 && w[3] == 1.0, "{w:?}");
        let out: Vec<Option<f32>> = (0..4).map(|_| f.tick(false, Vec3::ZERO, 0.0, &rig()).map(|o| o.weight)).collect();
        assert!(out[0].unwrap() < 0.7 && out[1].unwrap() < out[0].unwrap(), "{out:?}");
        assert!(out[3].is_none(), "{out:?}");
        // (off for good: a fresh plant next time, where the clip's feet are)
        let o = f.tick(true, Vec3::new(5.0, 0.0, 5.0), 0.0, &rig()).unwrap();
        assert!((world(Vec3::new(5.0, 0.0, 5.0), &o, 0) - Vec2::new(5.1, 5.0)).length() < 1e-4);
    }
}
