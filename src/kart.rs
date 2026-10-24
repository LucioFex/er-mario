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
/// and damper (N s/m): short and firm (a kart's)
pub const SUSPENSION: f32 = 0.28;
pub const WHEEL_R: f32 = 0.22;
const SPRING: f32 = 6500.0;
const DAMPER: f32 = 380.0;
const GRAVITY: f32 = 9.81;
/// the drive (N, all four: under what the tyres hold, no wheelspin), the brake (N), the air and
/// road's drag (N per (m/s)²; N per m/s: TOP at full throttle), a boost's thrust (N, on the chassis
/// itself, Mario Kart's: past what the tyres could), a coin's drive (N, up to COINS)
const DRIVE: f32 = 2000.0;
const BRAKE: f32 = 3000.0;
const DRAG: f32 = 2.48;
const ROLLING: f32 = 18.0;
const BOOST_DRIVE: f32 = 1800.0;
const COIN_DRIVE: f32 = 26.0;
pub const COINS: u32 = 10;
/// the tyres' grip (μ), the rear ones' in a drift; how hard they hold against sliding sideways
const GRIP: f32 = 1.2;
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
pub const TOP: f32 = 25.0;
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
const BOUNCE: f32 = 0.3;
/// the chassis' own corners on the ground (on its roof, its side, a landing too hard for the
/// springs): pushed out stiffly (N/m, N s/m), rubbing (μ)
const BODY_SPRING: f32 = 20000.0;
const BODY_DAMPER: f32 = 1500.0;
const BODY_GRIP: f32 = 0.6;
/// in the air, a soft pull upright (N m per unit tilt) and the spin damped (per s)
const AIR_RIGHT: f32 = 60.0;
const AIR_SPIN_DAMP: f32 = 0.6;
/// upside down (or on its side) this long, slow, it's put right where it lies (s)
const TUMBLE_RESET: f32 = 2.0;
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
/// it's turned (radians) and rolled round (radians).
#[derive(Clone, Copy, Debug, Default)]
pub struct Wheel {
    pub compression: f32,
    pub contact: Option<Vec3>,
    pub turn: f32,
    pub roll: f32,
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
}

impl Kart {
    /// A kart at `pos`, turned `yaw` (radians round Y; 0 faces +Z).
    pub fn new(pos: Vec3, yaw: f32) -> Kart {
        Kart { pos, rot: Quat::from_rotation_y(yaw), vel: Vec3::ZERO, spin: Vec3::ZERO, wheels: [Wheel::default(); 4], coins: 0, drifting: None, boost: 0.0, upset: 0.0, hop_was: false }
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
        for k in 0..n {
            self.substep(i, hop && k == 0, h, ray);
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
        let contacts = self.grounded().max(1) as f32;
        let quarter = MASS / 4.0;
        for (w, mount) in self.wheels.iter_mut().zip(MOUNTS) {
            let at = self.pos + self.rot * mount;
            let reach = SUSPENSION + WHEEL_R;
            let hit = ray(at, at - up * reach);
            let front = mount.z > 0.0;
            w.turn = if front { steer } else { 0.0 };
            let Some(p) = hit else {
                w.compression = 0.0;
                w.contact = None;
                continue;
            };
            let length = (at - p).length() - WHEEL_R;
            let compression = (SUSPENSION - length).max(0.0);
            let rate = (compression - w.compression) / h;
            w.compression = compression;
            w.contact = Some(p);
            // the spring and its damper, along the chassis' up
            let push = (SPRING * compression + DAMPER * rate).max(0.0);
            let spring = up * push;
            // the tyre: the way it rolls, and across it
            let wheel_fwd = Quat::from_axis_angle(up, -w.turn) * fwd;
            let wheel_side = wheel_fwd.cross(up);
            let r = p - self.pos;
            let v = self.vel + self.spin.cross(r);
            let (v_fwd, v_side) = (v.dot(wheel_fwd), v.dot(wheel_side));
            // pushed along (or braked: at a standstill the brake backs it up), dragged
            let mut along = drive_total / contacts;
            if i.brake > 0.05 {
                along -= if speed > 0.5 { i.brake * BRAKE } else { i.brake * DRIVE * 0.4 } / contacts;
            }
            along -= v_fwd * ROLLING / 4.0;
            // held against sliding sideways, the rear ones loose in a drift
            let grip = GRIP * if self.drifting.is_some() && !front { DRIFT_GRIP } else { 1.0 };
            let across = -v_side * quarter / h * SLIP_HOLD;
            // (the friction circle: no more than the tyre's load allows, both together)
            let most = grip * push;
            let tyre = Vec3::new(along, across, 0.0);
            let tyre = if tyre.length() > most { tyre * (most / tyre.length().max(1e-6)) } else { tyre };
            let f = spring + wheel_fwd * tyre.x + wheel_side * tyre.y;
            force += f;
            torque += r.cross(f);
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
        // the drift held round its way: its turn kept to a rate, the stick opening or tightening it
        if let Some((way, _)) = self.drifting {
            let pace = (speed / 15.0).clamp(0.5, 1.2);
            let want = -way * (DRIFT_TURN + DRIFT_TURN_STICK * i.steer.clamp(-1.0, 1.0) * way) * pace;
            let now = self.spin.dot(up);
            torque += up * ((want - now) * Self::inertia().y * DRIFT_HOLD);
        }
        // the air's drag
        let v2 = self.vel.length();
        force -= self.vel * (DRAG * v2);
        // a hop off the ground
        if hop && on_ground {
            self.vel += up * HOP;
        }
        // in the air: a soft pull upright, the spin calmed (it doesn't flip on its own)
        if self.grounded() == 0 && !body_contact {
            torque += up.cross(Vec3::Y) * AIR_RIGHT;
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
        // the bumpers: a wall ahead or behind (where it's going), it bounces off
        let way = self.vel.with_y(0.0);
        if way.length() > 0.1 {
            let d = way.normalize();
            let reach = BOX.z * 0.5 + BUMPER;
            let from = self.pos + up * 0.2;
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
        self.boost = (self.boost - h).max(0.0);
        // upside down or on its side on the ground, slow: put right where it lies after a moment
        if self.up().y < 0.3 && self.vel.length() < 2.0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The map as a height over (x, z), and walls: a ray's first hit (marched, then halved down).
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
                return Some(a);
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
        run(&mut k, Input { throttle: 1.0, ..Default::default() }, 12.0, &ray);
        assert!(k.speed() > 21.0 && k.speed() < 27.0, "{}", k.speed());
        assert!(k.pos.z > 100.0 && k.pos.x.abs() < 2.0, "{}", k.pos);
        // (a boost: faster)
        let top = k.speed();
        run(&mut k, Input { throttle: 1.0, mushroom: true, ..Default::default() }, 0.5, &ray);
        assert!(k.speed() > top + 1.0, "{} {top}", k.speed());
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
