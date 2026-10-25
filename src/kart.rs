//! The kart's physics (v4 Part 4, plan task 4.1), pure: whatever carries it (Torrent, plan A, or
//! Mario himself, plan B) is put where this says. A rigid chassis (mass, a box's inertia) on four
//! wheels, each a spring and a damper on a ray down the chassis' up from its mount: the springs
//! hold it up and tip it over bumps, the tyres push it along (throttle, brake) and grip sideways
//! (a friction circle, the rear ones loose in a drift); gravity, airtime off ramps, landings that
//! bounce on the springs, walls it bounces off, tumbles (put right after 2 s upside down). Weighty
//! like a sandbox car (GTA, BeamNG, Garry's Mod: no soft body), Mario Kart's drift and boosts on
//! top. Metres, seconds, the game's world (Y up); `ray(from, to)` gives the map's first hit.

use glam::{Quat, Vec3};

/// the chassis: mass (kg), its box (m: width, height, length), the wheels' mounts in it (m: right,
/// up, forward), level with its middle: a kart sits low, its weight between long axles (with the
/// mounts high and the wheelbase short, the throttle's own pull reared it up and over)
const MASS: f32 = 180.0;
pub const BOX: Vec3 = Vec3::new(1.4, 0.6, 2.2);
pub const MOUNTS: [Vec3; 4] = [Vec3::new(0.65, 0.0, 1.0), Vec3::new(-0.65, 0.0, 1.0), Vec3::new(0.65, 0.0, -1.0), Vec3::new(-0.65, 0.0, -1.0)];
/// the suspension: its length at rest with no load (m), the wheels' radius (m), its spring (N/m)
/// and damper (N s/m): long and soft enough for Elden Ring's ground, which is nowhere flat (a
/// kart's short firm one bounced off every bump, its wheels in the air half the time)
pub const SUSPENSION: f32 = 0.40;
pub const WHEEL_R: f32 = 0.22;
const SPRING: f32 = 4500.0;
const DAMPER: f32 = 520.0;
/// the wheels' rays start this far above their mounts (m): the game's rays find nothing from
/// under the ground, and a bump coming up over a mount took its wheel off the ground
const RAY_UP: f32 = 0.3;
/// ...and reach this far past the spring's full length (m): with the ground there the tyre still
/// touches (it drives, it steers) and its corner is held down to it with this share of its quarter
/// of the weight. Mario Kart's karts hug the ground; a free sprung one flew off every crest.
pub const SNAP: f32 = 0.3;
const STICK: f32 = 1.0;
/// a hop or a launch lets go of the ground for this long (s)
const LIFT: f32 = 0.4;
/// a spring pushes at most this many of its quarter of the weight (up a step it's pressed right in)
const PUSH_MOST: f32 = 10.0;
/// ...and its damper works off its spring's pace up to this (m/s)
const RATE_MOST: f32 = 8.0;
/// a tyre's pull across leans the chassis only this share of what it would (Mario Kart's never
/// roll over in a turn; it still leans into it)
const TYRE_LEAN: f32 = 0.35;
const GRAVITY: f32 = 9.81;
/// the drive (N, all four: under what the tyres hold, no wheelspin), the brake (N), the air and
/// road's drag (N per (m/s)²; N per m/s: TOP at full throttle), a boost's thrust (N, on the chassis
/// itself, Mario Kart's: past what the tyres could), a coin's drive (N, up to COINS)
const DRIVE: f32 = 2600.0;
const BRAKE: f32 = 3400.0;
const DRAG: f32 = 1.98;
const ROLLING: f32 = 18.0;
const BOOST_DRIVE: f32 = 2400.0;
const COIN_DRIVE: f32 = 34.0;
pub const COINS: u32 = 10;
/// the tyres' grip (μ), the rear ones' in a drift; how hard they hold against sliding sideways
const GRIP: f32 = 1.5;
/// a tyre on the ground holds at least as if it bore this share of its quarter of the weight
/// (Mario Kart's: cresting a bump, its spring nearly let out, it still drives and steers)
const TOUCH_LOAD: f32 = 0.8;
/// on the ground, pressed down with its speed (N per (m/s)²): over the crests at speed it stays
/// on them, as Mario Kart's do (off a ramp, its wheels off the ground, none)
const DOWNFORCE: f32 = 0.9;
const DRIFT_GRIP: f32 = 0.85;
const SLIP_HOLD: f32 = 0.5;
/// a drift is Mario Kart's, held round its way (not a spin-out): it turns at this rate (radians a
/// second), the stick opening or tightening it by this much, held there this firmly (per s)
const DRIFT_TURN: f32 = 1.1;
const DRIFT_TURN_STICK: f32 = 0.5;
const DRIFT_HOLD: f32 = 6.0;
/// ...keeping its speed (losing this share a second at most) and sliding at most this far off its
/// way (radians): the scrub of a real slide would stop it, Mario Kart's doesn't
const DRIFT_LOSS: f32 = 0.08;
const DRIFT_SLIP: f32 = 0.52;
/// the front wheels turn this far at most (degrees) slow, this far at the top speed
const STEER_SLOW: f32 = 30.0;
const STEER_FAST: f32 = 12.0;
pub const TOP: f32 = 32.0;
/// a hop (m/s up); a drift from this fast (m/s), steered at least this much; its levels' times (s)
/// and the boosts they give (s)
pub const HOP: f32 = 3.5;
const DRIFT_FROM: f32 = 8.0;
const DRIFT_STEER: f32 = 0.3;
const DRIFT_LEVELS: [f32; 3] = [0.6, 1.2, 2.0];
const DRIFT_BOOST: [f32; 4] = [0.0, 0.6, 1.0, 1.5];
/// a mushroom's boost (s)
const MUSHROOM: f32 = 1.2;
/// the bumpers: reach past the box (m), how much a wall gives back (0-1)
const BUMPER: f32 = 0.15;
/// ...looked for this high over its middle (m)
const BUMPER_UP: f32 = 0.5;
const BOUNCE: f32 = 0.3;
/// the chassis' own corners on the ground (on its roof, its side, a landing too hard for the
/// springs): pushed out stiffly (N/m, N s/m), rubbing (μ)
const BODY_SPRING: f32 = 20000.0;
const BODY_DAMPER: f32 = 1500.0;
const BODY_GRIP: f32 = 0.25;
/// in the air, a pull upright (N m per unit tilt; Mario Kart's level themselves off a jump), its
/// tumble damped (per s) and all its spin a little
const AIR_RIGHT: f32 = 900.0;
const AIR_TILT_DAMP: f32 = 4.0;
const AIR_SPIN_DAMP: f32 = 0.6;
/// on its wheels: its roll against the ground under them held (N m per radian, an anti-roll bar's
/// work, arcade-strong) and its roll spin damped (N m s): Mario Kart's karts lean, they don't roll
/// over (on rough ground a drift rolled it onto its roof, one wheel's spring and grip at a time)
const ROLL_HOLD: f32 = 4000.0;
const ROLL_DAMP: f32 = 500.0;
/// A on its roof or its side: a hop up (m/s) and it's turned back onto its wheels in this long (s)
const RIGHT_HOP: f32 = 4.0;
const RIGHT_TIME: f32 = 0.3;
/// upside down (or on its side) this long, slow, it's put right where it lies (s)
const TUMBLE_RESET: f32 = 2.0;
/// its up this far from the sky's at most (its y): on its roof or its side
const UPSET: f32 = 0.3;
/// the steps it's worked out in (s): two a game tick
const STEP: f32 = 1.0 / 60.0;

/// What the driver does this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// 0-1
    pub throttle: f32,
    pub brake: f32,
    /// -1 left .. 1 right
    pub steer: f32,
    /// A pressed now: a hop; A held: the drift
    pub hop: bool,
    pub drift: bool,
    /// a mushroom used now
    pub mushroom: bool,
}

/// One wheel: how far its spring is pressed in (m), where it touches the ground (or not), how far
/// it's turned (radians) and rolled round (radians), how far under its mount it is (m: its spring's
/// length, or the ground's, past it, in the reach that holds it to the ground).
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub compression: f32,
    pub contact: Option<Vec3>,
    pub turn: f32,
    pub roll: f32,
    pub reach: f32,
}

impl Default for Wheel {
    fn default() -> Wheel {
        Wheel { compression: 0.0, contact: None, turn: 0.0, roll: 0.0, reach: SUSPENSION }
    }
}

#[derive(Clone, Debug)]
pub struct Kart {
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    /// angular velocity (world, radians a second)
    pub spin: Vec3,
    pub wheels: [Wheel; 4],
    /// coins picked up (each a little more top speed, up to COINS)
    pub coins: u32,
    /// in a drift (its way: -1 left, 1 right) for how long (s); a boost left (s)
    pub drifting: Option<(f32, f32)>,
    pub boost: f32,
    /// upside down this long (s)
    upset: f32,
    /// the hop's A last tick
    hop_was: bool,
    /// a hop or a launch: this long (s) it isn't held to the ground (nor pressed onto it)
    lift: f32,
    /// A on its roof: turning back onto its wheels for this long yet (s), to this
    righting: Option<(f32, Quat)>,
}

impl Kart {
    /// A kart at `pos`, turned `yaw` (radians round Y; 0 faces +Z).
    pub fn new(pos: Vec3, yaw: f32) -> Kart {
        Kart { pos, rot: Quat::from_rotation_y(yaw), vel: Vec3::ZERO, spin: Vec3::ZERO, wheels: [Wheel::default(); 4], coins: 0, drifting: None, boost: 0.0, upset: 0.0, hop_was: false, lift: 0.0, righting: None }
    }

    pub fn forward(&self) -> Vec3 {
        self.rot * Vec3::Z
    }

    pub fn up(&self) -> Vec3 {
        self.rot * Vec3::Y
    }

    /// to its driver's right
    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up())
    }

    /// How fast it goes along its way (m/s; backwards below 0).
    pub fn speed(&self) -> f32 {
        self.vel.dot(self.forward())
    }

    /// Thrown up off the ground at `speed` (m/s, along its up: the super hop).
    pub fn launch(&mut self, speed: f32) {
        self.vel += self.up() * speed;
        self.lift = LIFT;
    }

    /// How high its middle rests over flat ground (m): on its springs pressed in by its weight.
    pub fn rest_height() -> f32 {
        SUSPENSION - MASS * GRAVITY / 4.0 / SPRING + WHEEL_R
    }

    /// Flipping back onto its wheels now (A on its roof).
    pub fn righting(&self) -> bool {
        self.righting.is_some()
    }

    /// On its roof or its side (A puts it right).
    pub fn upset(&self) -> bool {
        self.up().y < UPSET
    }

    /// Wheels on the ground.
    pub fn grounded(&self) -> usize {
        self.wheels.iter().filter(|w| w.contact.is_some()).count()
    }

    /// The drift's level now (0 none, 1 blue, 2 orange, 3 purple sparks).
    pub fn drift_level(&self) -> u8 {
        self.drifting.map_or(0, |(_, t)| DRIFT_LEVELS.iter().filter(|&&l| t >= l).count() as u8)
    }

    /// The chassis' box' inertia (kg m², its own axes).
    fn inertia() -> Vec3 {
        let (w, h, l) = (BOX.x, BOX.y, BOX.z);
        Vec3::new(h * h + l * l, w * w + l * l, w * w + h * h) * (MASS / 12.0)
    }

    /// One game tick of `dt` s (worked out in steps of STEP).
    pub fn step(&mut self, i: &Input, dt: f32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let dt = dt.clamp(0.0, 0.1);
        let n = (dt / STEP).ceil().max(1.0) as usize;
        let h = dt / n as f32;
        // (a hop and a mushroom once a press, not each step)
        let hop = i.hop && !self.hop_was;
        self.hop_was = i.hop;
        if i.mushroom {
            self.boost = self.boost.max(MUSHROOM);
        }
        // A on its roof or its side: a hop and it flips back onto its wheels, its way kept
        if hop && self.upset() && self.righting.is_none() {
            let f = self.forward().with_y(0.0);
            let f = if f.length() > 0.2 { f.normalize() } else { self.up().with_y(0.0).normalize_or(Vec3::Z) };
            self.righting = Some((RIGHT_TIME, Quat::from_rotation_y(f.x.atan2(f.z))));
            self.vel = self.vel.with_y(self.vel.y.max(0.0) + RIGHT_HOP);
            self.lift = LIFT;
            self.upset = 0.0;
        }
        for k in 0..n {
            self.substep(i, hop && k == 0, h, ray);
        }
        self.place_wheels(ray);
    }

    /// The wheels as drawn: each under its mount where the chassis is now, on the ground there.
    /// (Their springs' lengths were measured before the last step moved the chassis on: at speed
    /// half a metre back, so over bumps the drawn tyres sank into the rise ahead.)
    fn place_wheels(&mut self, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let up = self.up();
        for (w, mount) in self.wheels.iter_mut().zip(MOUNTS) {
            let at = self.pos + self.rot * mount;
            // (one off the ground hangs, or rests on ground the step brought within its reach)
            let snap = if w.contact.is_some() { SNAP } else { 0.0 };
            w.reach = match ray(at + up * RAY_UP, at - up * (SUSPENSION + WHEEL_R + snap)) {
                Some(p) => ((p - at).dot(-up) - WHEEL_R).clamp(0.0, SUSPENSION + snap),
                None if w.contact.is_some() => w.reach,
                None => SUSPENSION,
            };
        }
    }

    fn substep(&mut self, i: &Input, hop: bool, h: f32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let up = self.up();
        let fwd = self.forward();
        let speed = self.speed();
        let flat_speed = self.vel.with_y(0.0).length();
        let mut force = Vec3::new(0.0, -GRAVITY * MASS, 0.0);
        let mut torque = Vec3::ZERO;
        // the front wheels turned with the stick, less the faster it goes
        let k = (speed.abs() / TOP).clamp(0.0, 1.0);
        let steer = i.steer.clamp(-1.0, 1.0) * (STEER_SLOW + (STEER_FAST - STEER_SLOW) * k).to_radians();
        // the drift: A held, steered, quick enough, on the ground
        let on_ground = self.grounded() >= 2;
        match self.drifting {
            Some((way, t)) if i.drift && on_ground && speed > DRIFT_FROM * 0.6 && i.steer * way > -0.2 => self.drifting = Some((way, t + h)),
            Some(_) => {
                // let go: its boost, by how long it went
                self.boost = self.boost.max(DRIFT_BOOST[self.drift_level() as usize]);
                self.drifting = None;
            }
            None if i.drift && on_ground && speed > DRIFT_FROM && i.steer.abs() > DRIFT_STEER => self.drifting = Some((i.steer.signum(), 0.0)),
            None => {}
        }
        let drive_total = i.throttle.clamp(0.0, 1.0) * (DRIVE + self.coins.min(COINS) as f32 * COIN_DRIVE);
        // (a boost pushes the chassis itself, on the ground or not)
        if self.boost > 0.0 {
            force += fwd * BOOST_DRIVE;
        }
        let quarter = MASS / 4.0;
        for (w, mount) in self.wheels.iter_mut().zip(MOUNTS) {
            let at = self.pos + self.rot * mount;
            let reach = SUSPENSION + WHEEL_R;
            let hit = ray(at + up * RAY_UP, at - up * (reach + SNAP));
            let front = mount.z > 0.0;
            w.turn = if front { steer } else { 0.0 };
            let Some(p) = hit else {
                w.compression = 0.0;
                w.contact = None;
                w.reach = SUSPENSION;
                continue;
            };
            // (the ground over the mount: fully pressed in and more, it pushes back hard)
            let length = (p - at).dot(-up) - WHEEL_R;
            let compression = (SUSPENSION - length).max(0.0);
            // (no faster than this: up a step, its spring pressed in at once, the damper alone
            // threw it over backwards)
            let rate = ((compression - w.compression) / h).clamp(-RATE_MOST, RATE_MOST);
            w.compression = compression;
            w.contact = Some(p);
            w.reach = length.clamp(0.0, SUSPENSION + SNAP);
            // the spring and its damper, along the chassis' up; past its full length, held down
            // (through its middle: pulled down at its corner, over a dip, it dived nose first
            // into the next bump)
            let push = (SPRING * compression + DAMPER * rate).clamp(0.0, quarter * GRAVITY * PUSH_MOST);
            let stick = if length > SUSPENSION && self.lift <= 0.0 { quarter * GRAVITY * STICK } else { 0.0 };
            let spring = up * push;
            force -= up * stick;
            // the tyre: the way it rolls, and across it
            let wheel_fwd = Quat::from_axis_angle(up, -w.turn) * fwd;
            let wheel_side = wheel_fwd.cross(up);
            let r = p - self.pos;
            let v = self.vel + self.spin.cross(r);
            let (v_fwd, v_side) = (v.dot(wheel_fwd), v.dot(wheel_side));
            // dragged along (the drive and the brake are the chassis', below)
            let along = -v_fwd * ROLLING / 4.0;
            // held against sliding sideways, the rear ones loose in a drift
            let grip = GRIP * if self.drifting.is_some() && !front { DRIFT_GRIP } else { 1.0 };
            let across = -v_side * quarter / h * SLIP_HOLD;
            // (the friction circle: no more than the tyre's load allows, both together)
            let most = grip * push.max(quarter * GRAVITY * TOUCH_LOAD);
            let tyre = Vec3::new(along, across, 0.0);
            let tyre = if tyre.length() > most { tyre * (most / tyre.length().max(1e-6)) } else { tyre };
            let grip_f = wheel_fwd * tyre.x + wheel_side * tyre.y;
            force += spring + grip_f;
            torque += r.cross(spring) + (r - up * (r.dot(up) * (1.0 - TYRE_LEAN))).cross(grip_f);
            w.roll += v_fwd / WHEEL_R * h;
        }
        // its corners on the ground: pushed out, rubbing along
        let mut body_contact = false;
        for c in 0..8 {
            let corner = Vec3::new(if c & 1 == 0 { 0.5 } else { -0.5 }, if c & 2 == 0 { 0.5 } else { -0.5 }, if c & 4 == 0 { 0.5 } else { -0.5 }) * BOX;
            let at = self.pos + self.rot * corner;
            let Some(g) = ray(at + Vec3::Y * 0.6, at) else { continue };
            let depth = g.y - at.y;
            if depth <= 0.0 {
                continue;
            }
            body_contact = true;
            let r = at - self.pos;
            let v = self.vel + self.spin.cross(r);
            let push = (BODY_SPRING * depth - BODY_DAMPER * v.y).max(0.0);
            let slide = v.with_y(0.0);
            let rub = if slide.length() > 1e-3 { -slide.normalize() * (BODY_GRIP * push).min(slide.length() * MASS / 8.0 / h) } else { Vec3::ZERO };
            let f = Vec3::Y * push + rub;
            force += f;
            torque += r.cross(f);
        }
        // driven (or braked: at a standstill the brake backs it up) through its middle while a wheel
        // touches: split among the wheels down, with the two of one side on a bump the whole drive
        // went through them and spun it round
        if self.grounded() > 0 {
            let mut along = drive_total;
            if i.brake > 0.05 {
                along -= if speed > 0.5 { i.brake * BRAKE } else { i.brake * DRIVE * 0.4 };
            }
            force += fwd * along;
        }
        // the drift held round its way: its turn kept to a rate, the stick opening or tightening it
        if let Some((way, _)) = self.drifting {
            let pace = (speed / 15.0).clamp(0.5, 1.2);
            let want = -way * (DRIFT_TURN + DRIFT_TURN_STICK * i.steer.clamp(-1.0, 1.0) * way) * pace;
            let now = self.spin.dot(up);
            torque += up * ((want - now) * Self::inertia().y * DRIFT_HOLD);
        }
        // on its wheels: its roll against their ground held, its roll spin damped
        if self.grounded() >= 2 {
            torque -= fwd * (ROLL_DAMP * self.spin.dot(fwd));
            // (three or four wheels down: against their ground; two, one side up, the tipping:
            // upright, half as hard)
            let (n, k) = ground_normal(&self.wheels, up).map_or((Vec3::Y, 0.5), |n| (n, 1.0));
            torque += fwd * (ROLL_HOLD * k * up.cross(n).dot(fwd));
        }
        // the air's drag
        let v2 = self.vel.length();
        force -= self.vel * (DRAG * v2);
        // on the ground: pressed down onto it with its speed
        if on_ground && self.lift <= 0.0 {
            force -= up * (DOWNFORCE * flat_speed * flat_speed);
        }
        // a hop off the ground
        if hop && on_ground {
            self.vel += up * HOP;
            self.lift = LIFT;
        }
        self.lift = (self.lift - h).max(0.0);
        // in the air: pulled upright, its tumble calmed (it doesn't flip on its own)
        if self.grounded() == 0 && !body_contact {
            torque += up.cross(Vec3::Y) * AIR_RIGHT;
            let tumble = self.spin - Vec3::Y * self.spin.y;
            self.spin -= tumble * (AIR_TILT_DAMP * h).min(1.0);
            self.spin *= 1.0 - (AIR_SPIN_DAMP * h).min(1.0);
        }
        // move it
        self.vel += force / MASS * h;
        // (in a drift: its speed kept, its slide off its way held to DRIFT_SLIP)
        if self.drifting.is_some() && i.throttle > 0.1 {
            let f = fwd.with_y(0.0).normalize_or(Vec3::Z);
            let flat = self.vel.with_y(0.0);
            let keep = flat_speed * (1.0 - DRIFT_LOSS * h);
            let mut v = if flat.length() < keep { flat.normalize_or(f) * keep } else { flat };
            let off = v.angle_between(f);
            if off > DRIFT_SLIP && v.length() > 0.5 {
                let axis = v.cross(f).normalize_or(Vec3::Y);
                v = Quat::from_axis_angle(axis, off - DRIFT_SLIP) * v;
            }
            self.vel = v + Vec3::Y * self.vel.y;
        }
        let body_torque = self.rot.inverse() * torque;
        let body_spin = self.rot.inverse() * self.spin + body_torque / Self::inertia() * h;
        self.spin = self.rot * body_spin;
        let next = self.pos + self.vel * h;
        // the bumpers: a wall ahead or behind (where it's going), it bounces off (looked for at its
        // roof's height: lower down, the next bump's or a hill's rise was a wall, and it stopped
        // dead on rough ground; the springs and the chassis take what's lower)
        let way = self.vel.with_y(0.0);
        if way.length() > 0.1 {
            let d = way.normalize();
            let reach = BOX.z * 0.5 + BUMPER;
            let from = self.pos + up * BUMPER_UP;
            if ray(from, from + d * (reach + way.length() * h)).is_some() {
                let into = self.vel.dot(d);
                if into > 0.0 {
                    self.vel -= d * into * (1.0 + BOUNCE);
                    self.spin *= 0.5;
                }
            }
        }
        if !self.vel.is_finite() || !self.spin.is_finite() {
            self.vel = Vec3::ZERO;
            self.spin = Vec3::ZERO;
        }
        self.pos = if next.is_finite() { self.pos + self.vel * h } else { self.pos };
        let turn = self.spin * h;
        if turn.length_squared() > 1e-12 {
            self.rot = (Quat::from_scaled_axis(turn) * self.rot).normalize();
        }
        // (flipping back onto its wheels: turned there over RIGHT_TIME, its spin let go)
        if let Some((left, to)) = self.righting {
            self.rot = self.rot.slerp(to, (h / left.max(h)).min(1.0)).normalize();
            self.spin = Vec3::ZERO;
            self.righting = (left - h > 1e-4).then_some((left - h, to));
        }
        self.boost = (self.boost - h).max(0.0);
        // upside down or on its side on the ground, slow: put right where it lies after a moment
        if self.upset() && self.vel.length() < 2.0 {
            self.upset += h;
            if self.upset >= TUMBLE_RESET {
                let f = self.forward().with_y(0.0).normalize_or(Vec3::Z);
                self.rot = Quat::from_rotation_y(f.x.atan2(f.z));
                self.pos += Vec3::Y * 0.8;
                self.spin = Vec3::ZERO;
                self.upset = 0.0;
            }
        } else {
            self.upset = 0.0;
        }
    }
}

/// The ground's up under the wheels touching it (at least three; with two, none), turned to
/// `up`'s side.
fn ground_normal(wheels: &[Wheel; 4], up: Vec3) -> Option<Vec3> {
    // (round the chassis: front left, front right, rear right, rear left)
    let ring: Vec<Vec3> = [0usize, 1, 3, 2].iter().filter_map(|&i| wheels[i].contact).collect();
    if ring.len() < 3 {
        return None;
    }
    let mut n = Vec3::ZERO;
    for (k, a) in ring.iter().enumerate() {
        let b = ring[(k + 1) % ring.len()];
        n += Vec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
    }
    let n = n.try_normalize()?;
    Some(if n.dot(up) < 0.0 { -n } else { n })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The map as a height over (x, z), and walls: a ray's first hit (marched, then halved down).
    /// As the game's rays: one that starts under the ground finds nothing (no face from inside).
    fn world(height: impl Fn(f32, f32) -> f32 + Copy, wall_z: Option<f32>) -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        move |a: Vec3, b: Vec3| {
            if let Some(z) = wall_z {
                if (a.z - z) * (b.z - z) <= 0.0 && a.z != b.z {
                    let t = (z - a.z) / (b.z - a.z);
                    let p = a.lerp(b, t);
                    if p.y < height(p.x, p.z) + 3.0 {
                        return Some(p);
                    }
                }
            }
            let below = |p: Vec3| p.y <= height(p.x, p.z);
            if below(a) {
                return None;
            }
            let n = 24;
            let mut prev = 0.0;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                if below(a.lerp(b, t)) {
                    let (mut lo, mut hi) = (prev, t);
                    for _ in 0..20 {
                        let mid = (lo + hi) * 0.5;
                        if below(a.lerp(b, mid)) {
                            hi = mid;
                        } else {
                            lo = mid;
                        }
                    }
                    return Some(a.lerp(b, hi));
                }
                prev = t;
            }
            None
        }
    }

    fn flat() -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        world(|_, _| 0.0, None)
    }

    fn run(k: &mut Kart, i: Input, secs: f32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        for _ in 0..(secs * 30.0) as usize {
            k.step(&i, 1.0 / 30.0, ray);
            assert!(k.pos.is_finite() && k.vel.is_finite() && k.rot.is_finite(), "{k:?}");
        }
    }

    #[test]
    fn it_settles_on_its_springs() {
        let ray = flat();
        let mut k = Kart::new(Vec3::new(0.0, 1.2, 0.0), 0.0);
        run(&mut k, Input::default(), 30.0, &ray);
        assert_eq!(k.grounded(), 4);
        assert!(k.vel.length() < 0.05 && k.up().y > 0.99, "{k:?}");
        // (resting on its springs, pressed in by its weight)
        let c = k.wheels[0].compression;
        assert!(c > 0.04 && c < 0.12, "{c}");
    }

    #[test]
    fn the_throttle_takes_it_to_its_top_speed() {
        let ray = flat();
        let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
        run(&mut k, Input::default(), 1.0, &ray);
        // (quick off the line: Mario Kart's, 20 m/s in under 2.5 s)
        run(&mut k, Input { throttle: 1.0, ..Default::default() }, 2.5, &ray);
        assert!(k.speed() > 20.0, "{}", k.speed());
        run(&mut k, Input { throttle: 1.0, ..Default::default() }, 9.5, &ray);
        assert!(k.speed() > 29.0 && k.speed() < 34.0, "{}", k.speed());
        assert!(k.pos.z > 100.0 && k.pos.x.abs() < 2.0, "{}", k.pos);
        // (a boost: faster)
        let top = k.speed();
        run(&mut k, Input { throttle: 1.0, mushroom: true, ..Default::default() }, 0.5, &ray);
        assert!(k.speed() > top + 1.0, "{} {top}", k.speed());
    }

    /// Elden Ring's ground is nowhere flat: bumps of 12 and 25 cm every metre or two, driven over
    /// in four lanes.
    #[test]
    fn on_rough_ground_it_keeps_its_wheels_down_and_its_speed() {
        let mut runs = Vec::new();
        for (amp, lane) in [0.12f32, 0.25].into_iter().flat_map(|a| [0.0f32, 3.1, 7.7, 12.4].map(|l| (a, l))) {
            let h = move |x: f32, z: f32| amp * ((x * 1.3).sin() * (z * 0.9).cos() + (z * 2.1 + x * 0.4).sin() * 0.5);
            let ray = world(h, None);
            let mut k = Kart::new(Vec3::new(lane, 1.0, 0.0), 0.0);
            run(&mut k, Input::default(), 2.0, &ray);
            let (mut down, mut ticks, mut scraped) = (0usize, 0usize, 0usize);
            for _ in 0..(6.0 * 30.0) as usize {
                k.step(&Input { throttle: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
                down += k.grounded();
                ticks += 1;
                // (its chassis' bottom corners in the ground)
                let corners = [Vec3::new(0.5, -0.5, 0.5), Vec3::new(-0.5, -0.5, 0.5), Vec3::new(0.5, -0.5, -0.5), Vec3::new(-0.5, -0.5, -0.5)];
                if corners.iter().any(|&c| {
                    let at = k.pos + k.rot * (c * BOX);
                    at.y < h(at.x, at.z)
                }) {
                    scraped += 1;
                }
            }
            runs.push((amp, lane, down as f32 / ticks as f32, k.speed(), scraped as f32 / ticks as f32));
        }
        // 12 cm: every lane on its wheels, fast, never on its belly
        for &(_, lane, wheels, speed, scraped) in runs.iter().filter(|r| r.0 < 0.2) {
            assert!(wheels >= 3.5 && speed > 27.0 && scraped == 0.0, "12 cm lane {lane}: {wheels:.2} wheels down, {speed:.1} m/s, in the ground {:.0}%", scraped * 100.0);
        }
        // 25 cm (half a metre crest to dip, every metre or two: rougher than most of the map): on
        // the whole still on its wheels and quick, a corner's touch now and then
        let rough: Vec<_> = runs.iter().filter(|r| r.0 > 0.2).collect();
        let mean = |f: fn(&(f32, f32, f32, f32, f32)) -> f32| rough.iter().map(|r| f(r)).sum::<f32>() / rough.len() as f32;
        let (wheels, speed, scraped) = (mean(|r| r.2), mean(|r| r.3), mean(|r| r.4));
        assert!(wheels >= 2.8 && speed > 22.0 && scraped <= 0.2, "25 cm: {wheels:.2} wheels down, {speed:.1} m/s, in the ground {:.0}% ({rough:?})", scraped * 100.0);
    }

    /// A hop (A) and a launch (the super hop) leave the ground: holding it to the ground over the
    /// crests mustn't keep it from jumping.
    #[test]
    fn a_hop_and_a_launch_leave_the_ground() {
        let ray = flat();
        for launch in [false, true] {
            let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
            run(&mut k, Input { throttle: 1.0, ..Default::default() }, 2.0, &ray);
            if launch {
                k.launch(4.5);
            } else {
                k.step(&Input { throttle: 1.0, hop: true, ..Default::default() }, 1.0 / 30.0, &ray);
            }
            let mut off = 0;
            for _ in 0..30 {
                k.step(&Input { throttle: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
                off += usize::from(k.grounded() == 0);
            }
            assert!(off >= 3, "launch {launch}: {off} ticks off the ground");
            assert!(k.up().y > 0.9);
        }
    }

    /// Up a step (a rock, a kerb) at speed: it rides up onto it, no backflip off its springs.
    #[test]
    fn up_a_step_it_rides_onto_it() {
        for step in [0.35f32, 0.6] {
            let ray = world(move |_, z| if z > 25.0 { step } else { 0.0 }, None);
            let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
            let mut lowest = 1.0f32;
            for _ in 0..(6.0 * 30.0) as usize {
                k.step(&Input { throttle: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
                lowest = lowest.min(k.up().y);
            }
            assert!(lowest > 0.5, "step {step}: tipped to {lowest:.2}");
            assert!(k.pos.z > 40.0 && k.up().y > 0.9 && k.grounded() >= 3, "step {step}: {k:?}");
        }
    }

    #[test]
    fn it_steers_both_ways() {
        let ray = flat();
        for way in [1.0f32, -1.0] {
            let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
            run(&mut k, Input { throttle: 1.0, ..Default::default() }, 2.0, &ray);
            let right = k.right();
            let start = k.pos;
            run(&mut k, Input { throttle: 0.6, steer: way, ..Default::default() }, 1.5, &ray);
            assert!((k.pos - start).dot(right) * way > 2.0, "way {way}: {}", (k.pos - start).dot(right));
            assert!(k.up().y > 0.9, "rolled over in a turn");
        }
    }

    #[test]
    fn the_brake_stops_it() {
        let ray = flat();
        let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
        run(&mut k, Input { throttle: 1.0, ..Default::default() }, 3.0, &ray);
        assert!(k.speed() > 10.0);
        let mut stopped = false;
        for _ in 0..90 {
            k.step(&Input { brake: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
            stopped |= k.speed().abs() < 0.5;
        }
        assert!(stopped, "{}", k.speed());
    }

    #[test]
    fn it_lands_bouncing_and_settles() {
        let ray = flat();
        let mut k = Kart::new(Vec3::new(0.0, 5.0, 0.0), 0.0);
        let mut touched = false;
        let mut bounced = false;
        for _ in 0..240 {
            k.step(&Input::default(), 1.0 / 30.0, &ray);
            touched |= k.grounded() > 0;
            bounced |= touched && k.vel.y > 0.2;
        }
        assert!(touched && bounced, "touched {touched} bounced {bounced}");
        assert!(k.vel.length() < 0.1 && k.up().y > 0.99);
    }

    #[test]
    fn off_a_ramp_it_flies_and_lands_on_its_wheels() {
        // (up a kicker from z 10 to 20, then a drop back to the ground)
        let ray = world(|_, z| if (10.0..20.0).contains(&z) { (z - 10.0) * 0.35 } else { 0.0 }, None);
        let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
        let mut airborne = 0;
        for _ in 0..150 {
            k.step(&Input { throttle: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
            if k.grounded() == 0 && k.pos.z > 20.0 {
                airborne += 1;
            }
        }
        assert!(airborne >= 6, "{airborne}");
        run(&mut k, Input::default(), 4.0, &ray);
        assert!(k.up().y > 0.8 && k.grounded() >= 3, "{k:?}");
    }

    #[test]
    fn a_wall_stops_it_and_it_bounces_off() {
        let ray = world(|_, _| 0.0, Some(30.0));
        let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
        let mut back = false;
        for _ in 0..240 {
            k.step(&Input { throttle: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
            assert!(k.pos.z < 30.0, "through the wall: {}", k.pos.z);
            back |= k.vel.z < -0.5;
        }
        assert!(back, "no bounce");
    }

    #[test]
    fn upside_down_it_is_put_right() {
        let ray = flat();
        let mut k = Kart::new(Vec3::new(0.0, 0.6, 0.0), 0.0);
        k.rot = Quat::from_rotation_z(std::f32::consts::PI);
        run(&mut k, Input::default(), 5.0, &ray);
        assert!(k.up().y > 0.9, "{}", k.up());
    }

    /// Elden Ring's rough ground (25 cm bumps), turning and drifting on it, a ramp under one side's
    /// wheels: Mario Kart's karts lean and bounce but stay on their wheels (before: a drift on the
    /// bumps rolled it onto its roof, its roll at 6 rad/s off one wheel's spring and grip).
    #[test]
    fn rough_ground_turns_and_drifts_dont_roll_it_over() {
        fn ground(case: usize, x: f32, z: f32) -> f32 {
            match case {
                0 => 0.25 * ((x * 1.3).sin() * (z * 0.9).cos() + (z * 2.1 + x * 0.4).sin() * 0.5),
                _ => if x > 0.2 && (15.0..20.0).contains(&z) { (z - 15.0) * 0.12 } else { 0.0 },
            }
        }
        for (case, name, steer, drift) in [(0, "turning on bumps", 0.6, false), (0, "a drift on bumps", 1.0, true), (0, "a drift the other way", -1.0, true), (1, "a ramp under one side", 0.0, false)] {
            let ray = world(move |x, z| ground(case, x, z), None);
            let mut k = Kart::new(Vec3::new(0.0, 3.0, 0.0), 0.0);
            run(&mut k, Input::default(), 2.0, &ray);
            let mut lowest = 1.0f32;
            for t in 0..(6.0 * 30.0) as usize {
                let on = t > 30;
                k.step(&Input { throttle: 1.0, steer: if on { steer } else { 0.0 }, drift: drift && on, ..Default::default() }, 1.0 / 30.0, &ray);
                lowest = lowest.min(k.up().y);
            }
            assert!(lowest > 0.6, "{name}: tipped to {lowest:.2}");
            assert!(k.up().y > 0.85, "{name}: ended at {:.2}", k.up().y);
        }
    }

    /// On its roof or its side, A (the hop) puts it back on its wheels at once, with a little hop.
    #[test]
    fn upside_down_a_hop_puts_it_right() {
        let ray = flat();
        for over in [std::f32::consts::PI, std::f32::consts::FRAC_PI_2] {
            let mut k = Kart::new(Vec3::new(0.0, 0.6, 0.0), 0.3);
            k.rot = Quat::from_rotation_z(over) * k.rot;
            run(&mut k, Input::default(), 0.5, &ray);
            assert!(k.up().y < 0.3, "it should still lie there: {}", k.up());
            assert!(k.upset(), "upset");
            k.step(&Input { hop: true, ..Default::default() }, 1.0 / 30.0, &ray);
            run(&mut k, Input::default(), 0.6, &ray);
            assert!(k.up().y > 0.9, "over {over}: {}", k.up());
            assert!(!k.upset());
            // (its way kept: it faces where it did, round the up)
            let f = k.forward().with_y(0.0).normalize();
            assert!(f.x.atan2(f.z).abs() < 1.2, "{f}");
        }
    }

    #[test]
    fn a_long_drift_gives_a_big_boost() {
        let ray = flat();
        let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
        run(&mut k, Input { throttle: 1.0, ..Default::default() }, 2.5, &ray);
        let mut most = 0;
        for _ in 0..80 {
            k.step(&Input { throttle: 1.0, steer: 1.0, drift: true, ..Default::default() }, 1.0 / 30.0, &ray);
            most = most.max(k.drift_level());
        }
        assert_eq!(most, 3);
        k.step(&Input { throttle: 1.0, ..Default::default() }, 1.0 / 30.0, &ray);
        assert!(k.drifting.is_none() && k.boost > 1.0, "boost {}", k.boost);
    }
}
