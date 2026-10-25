//! The kart's rules (v4 Part 4), pure: what the driver's buttons do this tick (kart.rs drives,
//! this says with what: the mushrooms, the Fire Flower, the shell, the banana, the trick off a
//! lip, the super hop, the spin-out that takes the wheel from him), what a run into an enemy does
//! (a small one flung, a medium one bumped, a big one or a boss spins the kart out), what a blow on
//! him does, where the kart's six parts are (the body, the four wheels turning and rolling, the
//! steering wheel), where Mario sits and holds it, the flat floor SM64 keeps under him, the
//! engine's voice, the racing camera's mark. karting.rs puts it in the game.

use glam::{Quat, Vec2, Vec3};

use crate::kart::{self, Kart};

/// the mushrooms: so many, one back every this long (s)
pub const MUSHROOMS: u32 = 3;
const MUSHROOM_BACK: f32 = 10.0;
/// a Fire Flower shot at most this often (s)
const FIRE_EVERY: f32 = 0.6;
/// Y held this many ticks: the banana thrown ahead as it's let go (a tap drops it behind)
const BANANA_HOLD: u32 = 8;
/// the spin-out: how long (s), how many turns, the brake meanwhile
pub const SPIN_TIME: f32 = 0.9;
const SPIN_TURNS: f32 = 2.0;
const SPIN_BRAKE: f32 = 0.6;
/// the trick: A within this long (s) of leaving the ground going up at least this fast (m/s);
/// its roll's time (s) and the landing's boost (s)
const TRICK_WINDOW: f32 = 0.3;
const TRICK_UP: f32 = 2.0;
pub const TRICK_TIME: f32 = 0.5;
pub const TRICK_BOOST: f32 = 0.6;
/// the super hop (A while a boost is on): this much more up than a hop (m/s), once every this long
pub const SUPER_HOP: f32 = 4.5;
const SUPER_HOP_EVERY: f32 = 2.0;
/// a run into an enemy counts from this fast into it (m/s), its worth is full at this (and at most
/// RAM_MOST of it, at least RAM_LEAST)
const RAM_FROM: f32 = 4.0;
const RAM_FULL: f32 = 20.0;
const RAM_LEAST: f32 = 0.3;
const RAM_MOST: f32 = 1.5;
/// a small one flung: this share of the speed into it (m/s, at most FLING_MOST), this much up
const FLING_SHARE: f32 = 0.8;
const FLING_MOST: f32 = 18.0;
const FLING_UP: f32 = 6.0;
/// kart coins lost to a blow (Mario Kart's)
pub const COINS_LOST: u32 = 3;
/// a fall from this high over where it lands kills (the mod's lethal fall, m)
pub const LETHAL_FALL: f32 = 22.0;

/// Where things are on the kart (its own axes: x across, y up, z ahead, from its middle; kart.rs's
/// `right()` is its −x). Mario's seat (his feet in his seated pose).
pub const SEAT: Vec3 = Vec3::new(0.0, 0.16, -0.32);
/// the steering wheel's middle, its tilt back up toward him (radians), its rim (m), how far it
/// turns for the front wheels' turn
pub const WHEEL_AT: Vec3 = Vec3::new(0.0, 0.42, 0.22);
pub const WHEEL_TILT: f32 = 0.6;
pub const WHEEL_RIM: f32 = 0.17;
pub const WHEEL_GEAR: f32 = 4.0;
/// the two exhausts' mouths, the nose (shells and fireballs leave it), the tail (a banana's drop)
pub const EXHAUSTS: [Vec3; 2] = [Vec3::new(0.24, 0.14, -1.2), Vec3::new(-0.24, 0.14, -1.2)];
pub const NOSE: Vec3 = Vec3::new(0.0, 0.25, 1.35);
pub const TAIL: Vec3 = Vec3::new(0.0, 0.1, -1.45);
/// the body as enemies meet it: half its width, height and length (m)
pub const HALF: Vec3 = Vec3::new(0.75, 0.45, 1.2);
/// the parts: the body, the wheels (kart.rs's order: +x front, −x front, +x rear, −x rear), the
/// steering wheel
pub const PARTS: usize = 6;
pub const BODY: usize = 0;
pub const WHEELS: usize = 1;
pub const STEERING: usize = 5;
/// the wheels drawn turning at most this fast (rad/s: past it they'd seem to stand still)
const WHEEL_RATE_MOST: f32 = 30.0;

/// The driver's pad this tick: triggers 0-1, the stick (x right), the buttons held.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pads {
    pub rt: f32,
    pub lt: f32,
    pub stick: Vec2,
    pub a: bool,
    pub x: bool,
    pub y: bool,
    pub lb: bool,
    pub rb: bool,
}

/// The kart as the rules need it: wheels on the ground, its speed up, the boost left (s), the
/// drift's level.
#[derive(Clone, Copy, Debug, Default)]
pub struct KartNow {
    pub grounded: usize,
    pub vy: f32,
    pub boost: f32,
    pub drift: u8,
    /// on its roof or its side (A puts it right: kart.rs)
    pub upset: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Banana {
    /// dropped behind
    Back,
    /// thrown ahead
    Ahead,
}

/// What this tick's buttons do.
#[derive(Clone, Copy, Debug, Default)]
pub struct Acts {
    pub input: kart::Input,
    pub mushroom: bool,
    /// LB with none left
    pub empty: bool,
    pub shell: bool,
    pub banana: Option<Banana>,
    pub fire: bool,
    pub trick: bool,
    /// down off a trick: its boost
    pub landed_trick: bool,
    pub super_hop: bool,
    /// the drift went up to this level
    pub drift_up: Option<u8>,
}

pub struct Driver {
    prev: Pads,
    mushrooms: u32,
    refill: f32,
    fire_wait: f32,
    y_ticks: Option<u32>,
    spin: Option<f32>,
    trick: Option<f32>,
    trick_boost: bool,
    /// in the air: how long, and whether it left the ground going up
    air: Option<(f32, bool)>,
    /// how fast it went up on its last tick on the ground (m/s)
    ground_vy: f32,
    hop_wait: f32,
    drift_was: u8,
}

impl Driver {
    pub fn new() -> Self {
        Driver {
            prev: Pads::default(),
            mushrooms: MUSHROOMS,
            refill: MUSHROOM_BACK,
            fire_wait: 0.0,
            y_ticks: None,
            spin: None,
            trick: None,
            trick_boost: false,
            air: None,
            ground_vy: 0.0,
            hop_wait: 0.0,
            drift_was: 0,
        }
    }

    /// Every SM64 tick: the pad now `p`, the kart as it is before this tick's step.
    pub fn tick(&mut self, p: &Pads, k: &KartNow, dt: f32) -> Acts {
        let mut acts = Acts::default();
        let pressed = |now: bool, was: bool| now && !was;
        self.fire_wait = (self.fire_wait - dt).max(0.0);
        self.hop_wait = (self.hop_wait - dt).max(0.0);
        // (one mushroom back at a time, while some are gone)
        if self.mushrooms < MUSHROOMS {
            self.refill -= dt;
            if self.refill <= 0.0 {
                self.mushrooms += 1;
                self.refill = MUSHROOM_BACK;
            }
        }
        // (the spin-out's ticks all its own: its time runs on after this one's)
        let spinning = self.spin.is_some();
        if let Some(t) = self.spin.as_mut() {
            *t += dt;
            if *t >= SPIN_TIME {
                self.spin = None;
            }
        }
        if let Some(t) = self.trick.as_mut() {
            *t += dt;
            if *t >= TRICK_TIME {
                self.trick = None;
            }
        }
        // off the ground: since when, and going up as it left (a lip, a ramp, a hop); down again: a
        // trick's boost
        if k.grounded == 0 {
            // (going up as it left it: by its first tick off the ground gravity, and the hold to
            // the ground over a crest, took some of it)
            match self.air.as_mut() {
                Some((t, _)) => *t += dt,
                None => self.air = Some((0.0, k.vy.max(self.ground_vy) >= TRICK_UP)),
            }
        } else {
            self.ground_vy = k.vy;
            if k.grounded >= 2 && self.air.take().is_some() && std::mem::take(&mut self.trick_boost) {
                acts.landed_trick = true;
            }
        }
        let a_pressed = pressed(p.a, self.prev.a);
        if k.upset {
            // (on its roof or its side: A is only the hop that puts it right)
            acts.input = kart::Input { hop: p.a, ..Default::default() };
            self.y_ticks = None;
        } else if spinning {
            // (the wheel's not his while it spins: no items either)
            acts.input = kart::Input { throttle: 0.0, brake: SPIN_BRAKE, steer: 0.0, hop: false, drift: false, mushroom: false };
            self.y_ticks = None;
        } else {
            acts.input = kart::Input { throttle: p.rt.clamp(0.0, 1.0), brake: p.lt.clamp(0.0, 1.0), steer: p.stick.x.clamp(-1.0, 1.0), hop: p.a, drift: p.a, mushroom: false };
            if a_pressed && k.grounded == 0 && self.trick.is_none() && self.air.is_some_and(|(t, up)| up && t <= TRICK_WINDOW) {
                self.trick = Some(0.0);
                self.trick_boost = true;
                acts.trick = true;
            }
            if a_pressed && k.grounded >= 2 && k.boost > 0.0 && self.hop_wait <= 0.0 {
                acts.super_hop = true;
                self.hop_wait = SUPER_HOP_EVERY;
            }
            if pressed(p.lb, self.prev.lb) {
                if self.mushrooms > 0 {
                    if self.mushrooms == MUSHROOMS {
                        self.refill = MUSHROOM_BACK;
                    }
                    self.mushrooms -= 1;
                    acts.mushroom = true;
                    acts.input.mushroom = true;
                } else {
                    acts.empty = true;
                }
            }
            if pressed(p.rb, self.prev.rb) && self.fire_wait <= 0.0 {
                acts.fire = true;
                self.fire_wait = FIRE_EVERY;
            }
            acts.shell = pressed(p.x, self.prev.x);
            // Y: counted from its press, what it does as it's let go
            match (p.y, self.y_ticks) {
                (true, None) if !self.prev.y => self.y_ticks = Some(1),
                (true, Some(n)) => self.y_ticks = Some(n + 1),
                (false, Some(n)) => {
                    acts.banana = Some(if n >= BANANA_HOLD { Banana::Ahead } else { Banana::Back });
                    self.y_ticks = None;
                }
                _ => {}
            }
        }
        if k.drift > self.drift_was {
            acts.drift_up = Some(k.drift);
        }
        self.drift_was = k.drift;
        self.prev = *p;
        acts
    }

    /// A spin-out starts (none on top of one going on). Whether it started.
    pub fn spin_out(&mut self) -> bool {
        if self.spin.is_some() {
            return false;
        }
        self.spin = Some(0.0);
        true
    }

    pub fn spinning(&self) -> bool {
        self.spin.is_some()
    }

    /// The spin-out's turn now (radians round the kart's up; 0 when none): fast, then slowing.
    pub fn spin(&self) -> f32 {
        self.spin.map_or(0.0, |t| {
            let k = (t / SPIN_TIME).clamp(0.0, 1.0);
            SPIN_TURNS * std::f32::consts::TAU * (1.0 - (1.0 - k) * (1.0 - k))
        })
    }

    /// The trick's roll now (radians round the kart's way; 0 when none): a whole turn, eased.
    pub fn roll(&self) -> f32 {
        self.trick.map_or(0.0, |t| {
            let k = (t / TRICK_TIME).clamp(0.0, 1.0);
            std::f32::consts::TAU * k * k * (3.0 - 2.0 * k)
        })
    }

    /// The mushrooms left, and how far the next one is (0-1; 1 when all are there).
    pub fn mushrooms(&self) -> (u32, f32) {
        if self.mushrooms >= MUSHROOMS { (MUSHROOMS, 1.0) } else { (self.mushrooms, (1.0 - self.refill / MUSHROOM_BACK).clamp(0.0, 1.0)) }
    }
}

/// How big the one run into is (juggle.rs's pull sizes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Small,
    Medium,
    Big,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RamKind {
    /// a small one flung away (KartRam)
    Fling,
    /// a medium one knocked back, staggering (KartBump)
    Bump,
    /// a big one or a boss: a little of it (KartBump), the kart spins out
    Glance,
}

/// What a run into an enemy does: its blow and worth (Combat::deal_scaled), the fling (speed, up:
/// m/s), the share of the kart's speed into it kept (below 0: it bounces back), a spin-out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ram {
    pub kind: RamKind,
    pub worth: f32,
    pub fling: Option<(f32, f32)>,
    pub keep: f32,
    pub spin: bool,
}

pub fn ram(size: Size, boss: bool, into: f32) -> Option<Ram> {
    if into < RAM_FROM {
        return None;
    }
    let worth = (into / RAM_FULL).clamp(RAM_LEAST, RAM_MOST);
    Some(match (size, boss) {
        (Size::Small, false) => Ram { kind: RamKind::Fling, worth, fling: Some(((into * FLING_SHARE).min(FLING_MOST), FLING_UP)), keep: 0.85, spin: false },
        (Size::Medium, false) => Ram { kind: RamKind::Bump, worth, fling: None, keep: -0.2, spin: false },
        _ => Ram { kind: RamKind::Glance, worth: worth * 0.5, fling: None, keep: -0.3, spin: true },
    })
}

/// The kart at `pos` turned `rot` going `vel` touching an enemy whose feet are at `feet` (body
/// `radius`, `height`, m): how fast it runs into it (m/s), if it does.
pub fn contact(pos: Vec3, rot: Quat, vel: Vec3, feet: Vec3, radius: f32, height: f32) -> Option<f32> {
    let l = rot.inverse() * (feet - pos);
    if l.x.abs() > HALF.x + radius || l.z.abs() > HALF.z + radius {
        return None;
    }
    // (its feet under the kart's top with Mario on it, its head over the kart's bottom)
    if l.y > HALF.y + 0.8 || l.y + height < -HALF.y {
        return None;
    }
    let into = vel.dot((feet - pos).with_y(0.0).normalize_or_zero());
    (into > 0.0).then_some(into)
}

/// What a blow on him while he drives does: out of the kart (`blown`: a heavy one that sends him
/// flying), else a spin-out and coins lost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Taken {
    pub unseat: bool,
    pub spin: bool,
    pub coins: u32,
}

pub fn hit(blown: bool) -> Taken {
    if blown { Taken { unseat: true, spin: false, coins: 0 } } else { Taken { unseat: false, spin: true, coins: COINS_LOST } }
}

/// The kart's six parts in the world: (place, turn) each.
#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub parts: [(Vec3, Quat); PARTS],
}

/// The kart's turn as drawn: its own, the spin-out's round its up, the trick's round its way.
pub fn visual(k: &Kart, spin: f32, roll: f32) -> Quat {
    (k.rot * Quat::from_rotation_y(spin) * Quat::from_rotation_z(roll)).normalize()
}

/// The parts of kart `k` (spun `spin`, rolled `roll`), its wheels rolled round `wheel_spin`.
pub fn model(k: &Kart, spin: f32, roll: f32, wheel_spin: &[f32; 4]) -> Model {
    let vis = visual(k, spin, roll);
    let mut parts = [(k.pos, vis); PARTS];
    for (i, w) in k.wheels.iter().enumerate() {
        // (under its mount where it touches: the spring's length, or past it on the ground it's
        // held to; all of it in the air)
        let local = kart::MOUNTS[i] - Vec3::Y * w.reach;
        let rot = vis * Quat::from_rotation_y(-w.turn) * Quat::from_rotation_x(wheel_spin[i]);
        parts[WHEELS + i] = (k.pos + vis * local, rot.normalize());
    }
    let turn = k.wheels[0].turn;
    let rot = vis * Quat::from_rotation_x(WHEEL_TILT) * Quat::from_rotation_z(turn * WHEEL_GEAR);
    parts[STEERING] = (k.pos + vis * WHEEL_AT, rot.normalize());
    Model { parts }
}

/// The drawn tyres' bottoms over the ground straight under their middles (m; under 0 in it), or
/// none found.
pub fn tyre_gaps(m: &Model, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> [Option<f32>; 4] {
    std::array::from_fn(|i| {
        let c = m.parts[WHEELS + i].0;
        ray(c + Vec3::Y * 1.0, c - Vec3::Y * 1.5).map(|g| c.y - kart::WHEEL_R - g.y)
    })
}

/// Where Mario sits: his feet, the way he faces (flat), and the turn of the kart off his upright
/// self (world: his parts are turned by it round his feet).
pub fn seat(k: &Kart, spin: f32, roll: f32) -> (Vec3, Vec3, Quat) {
    let vis = visual(k, spin, roll);
    let feet = k.pos + vis * SEAT;
    let face = (vis * Vec3::Z).with_y(0.0).normalize_or(Vec3::Z);
    let upright = Quat::from_rotation_y(face.x.atan2(face.z));
    (feet, face, (vis * upright.inverse()).normalize())
}

/// The steering wheel's two grips (its rim at ten and two), on its +x side and its −x side.
pub fn grips(m: &Model) -> [Vec3; 2] {
    let (c, rot) = m.parts[STEERING];
    let (s, co) = 0.35f32.sin_cos();
    [c + rot * Vec3::new(co, s, 0.0) * WHEEL_RIM, c + rot * Vec3::new(-co, s, 0.0) * WHEEL_RIM]
}

/// How fast the wheels are drawn turning at `speed` (rad/s).
pub fn wheel_rate(speed: f32) -> f32 {
    (speed / kart::WHEEL_R).clamp(-WHEEL_RATE_MOST, WHEEL_RATE_MOST)
}

/// SM64's floor under him while he drives (units): this far under his feet when it's put there;
/// put again once he's this far off it, this little over it or this much over it.
pub const FLOOR_DROP: f32 = 50.0;
const FLOOR_REACH: f32 = 1200.0;
const FLOOR_OVER_LEAST: f32 = 20.0;
const FLOOR_OVER_MOST: f32 = 3000.0;

/// The floor at `floor` (None: none yet) has to be put again for Mario at `mario` (SM64 units).
pub fn floor_reload(floor: Option<[f32; 3]>, mario: [f32; 3]) -> bool {
    let Some(f) = floor else { return true };
    let flat = ((mario[0] - f[0]).powi(2) + (mario[2] - f[2]).powi(2)).sqrt();
    let over = mario[1] - f[1];
    flat > FLOOR_REACH || !(FLOOR_OVER_LEAST..=FLOOR_OVER_MOST).contains(&over)
}

/// The stick's right as the kart's `right()` (+1) or the other way (−1): by the camera's axes
/// (the game's world is mirrored against glam's cross product, or not).
pub fn steer_sign(fwd: Vec3, up: Vec3, right: Vec3) -> f32 {
    if fwd.cross(up).dot(right) >= 0.0 { 1.0 } else { -1.0 }
}

/// The engine's voice (kartengine.rs): its pitch (Hz), how hard it works (0-1), how loud, the
/// tyres' skid (0-1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tone {
    pub freq: f32,
    pub load: f32,
    pub volume: f32,
    pub skid: f32,
}

/// the engine's pitch at a standstill and at the top speed (Hz), the throttle's rev on top, a
/// boost's lift; how loud with the throttle off and full
const IDLE_HZ: f32 = 46.0;
const TOP_HZ: f32 = 170.0;
const REV_HZ: f32 = 8.0;
const BOOST_PITCH: f32 = 1.15;
const VOLUME: (f32, f32) = (0.16, 0.24);

pub fn engine_tone(speed: f32, throttle: f32, boost: bool, skid: f32) -> Tone {
    let s = (speed.abs() / kart::TOP).clamp(0.0, 1.4);
    let rev = throttle.clamp(0.0, 1.0);
    // (past the top speed, a boost's: higher still)
    let freq = IDLE_HZ + (TOP_HZ - IDLE_HZ) * s.min(1.0) + 60.0 * (s - 1.0).max(0.0) + REV_HZ * rev;
    Tone { freq: freq * if boost { BOOST_PITCH } else { 1.0 }, load: rev, volume: VOLUME.0 + (VOLUME.1 - VOLUME.0) * rev, skid: skid.clamp(0.0, 1.0) }
}

/// A landing at `at` after the highest point `peak` (m) kills.
pub fn lethal(peak: f32, at: f32) -> bool {
    peak - at > LETHAL_FALL
}

/// the racing camera: Mario's own camera's distance (a share of it, lakitu.rs's zoom) and height
/// (and this much higher), this much farther at the top speed and in a boost (m); behind the way
/// the kart goes, its yaw on a damped spring this quick (rad/s: a turn shows, the shiver of rough
/// ground doesn't), its height eased at this rate (per s: the springs' bounce ridden out); the
/// way it moves counts from this fast to this fast (m/s) for this share against its nose (a
/// drift's slide); it looks this far ahead and this high (m); the stick's look eased at this rate
const CHASE_ZOOM: f32 = 0.95;
const CHASE_LIFT: f32 = 0.3;
/// farther back at speed (m: at its top speed going forward, at BACKING_FAST backing up), in a
/// boost; higher by this much a metre farther (third test: 1.4 / 1.0 m and level was too near to
/// see the way ahead)
const CHASE_FAST: f32 = 4.0;
const CHASE_BACKING: f32 = 3.0;
const BACKING_FAST: f32 = 12.0;
const CHASE_BOOST: f32 = 1.8;
const CHASE_RISE: f32 = 0.3;
const CHASE_TURN: f32 = 5.0;
const CHASE_HEIGHT: f32 = 2.5;
const TRAVEL: (f32, f32) = (3.0, 10.0);
const TRAVEL_SHARE: f32 = 0.6;
const LOOK_AHEAD: f32 = 3.0;
const LOOK_UP: f32 = 0.9;
const CHASE_LOOK: f32 = 6.0;

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Lakitu's racing chase (lakitu.rs keeps one while Mario drives).
#[derive(Clone, Copy, Debug, Default)]
pub struct ChaseCam {
    started: bool,
    yaw: f32,
    yaw_vel: f32,
    y: f32,
    back: f32,
    look: f32,
}

impl ChaseCam {
    /// A frame of `dt` s: the kart at `pos` with its nose `fwd`, going `vel`; Mario's zoom's
    /// (distance, height) in m; the stick's look round it (radians); a boost on. Where the eye
    /// wants to be and where it looks.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, pos: Vec3, fwd: Vec3, vel: Vec3, zoom: (f32, f32), look: f32, boost: bool, dt: f32) -> (Vec3, Vec3) {
        let dt = dt.clamp(0.0, 0.1);
        let f = fwd.with_y(0.0).normalize_or(Vec3::Z);
        let nose = f.x.atan2(f.z);
        let flat = vel.with_y(0.0);
        let along = flat.dot(f);
        // (backing up: behind its nose; going forward: toward the way it goes, a drift's slide)
        let travel = if along > 0.0 { flat.x.atan2(flat.z) } else { nose };
        let w = ((along - TRAVEL.0) / (TRAVEL.1 - TRAVEL.0)).clamp(0.0, 1.0) * TRAVEL_SHARE;
        let want = nose + wrap(travel - nose) * w;
        let fast = if along >= 0.0 { CHASE_FAST * (along / kart::TOP).clamp(0.0, 1.2) } else { CHASE_BACKING * (-along / BACKING_FAST).clamp(0.0, 1.0) };
        if !self.started {
            *self = ChaseCam { started: true, yaw: want, yaw_vel: 0.0, y: pos.y, back: fast, look };
        }
        // (the yaw's spring in steps of a 60th of a second at most)
        let n = (dt * 60.0).ceil().max(1.0) as usize;
        let h = dt / n as f32;
        for _ in 0..n {
            let acc = CHASE_TURN * CHASE_TURN * wrap(want - self.yaw) - 2.0 * CHASE_TURN * self.yaw_vel;
            self.yaw_vel += acc * h;
            self.yaw = wrap(self.yaw + self.yaw_vel * h);
        }
        let rate = |r: f32| 1.0 - (-r * dt).exp();
        self.y += (pos.y - self.y) * rate(CHASE_HEIGHT);
        self.back += (fast + if boost { CHASE_BOOST } else { 0.0 } - self.back) * rate(2.0);
        self.look += wrap(look - self.look) * rate(CHASE_LOOK);
        let yaw = self.yaw + self.look;
        let dir = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let base = pos.with_y(self.y);
        let eye = base - dir * (zoom.0 * CHASE_ZOOM + self.back) + Vec3::Y * (zoom.1 + CHASE_LIFT + self.back * CHASE_RISE);
        (eye, base + dir * LOOK_AHEAD + Vec3::Y * LOOK_UP)
    }
}

/// The drift's sparks and the exhausts' flames: their colour by the drift's level (blue, orange,
/// purple), a boost's (a mini-turbo's level's colour, a mushroom's fiery orange) and how big (m).
pub const SPARKS: [[f32; 3]; 3] = [[0.35, 0.62, 1.0], [1.0, 0.58, 0.15], [0.78, 0.38, 1.0]];
const BOOST_FLAME: [f32; 3] = [1.0, 0.45, 0.1];

pub fn flame_of(drift: u8, boost: f32, turbo: u8) -> Option<([f32; 3], f32)> {
    if boost > 0.0 {
        let c = if (1..=3).contains(&turbo) { SPARKS[turbo as usize - 1] } else { BOOST_FLAME };
        Some((c, 0.32 + 0.3 * boost.min(1.0)))
    } else if (1..=3).contains(&drift) {
        Some((SPARKS[drift as usize - 1], 0.16 + 0.03 * drift as f32))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    fn ground() -> KartNow {
        KartNow { grounded: 4, vy: 0.0, boost: 0.0, drift: 0, upset: false }
    }

    /// One tick with `p` held after a tick of nothing (a fresh press).
    fn press(d: &mut Driver, p: Pads, k: &KartNow) -> Acts {
        d.tick(&Pads::default(), k, DT);
        d.tick(&p, k, DT)
    }

    #[test]
    fn three_mushrooms_then_none_one_back_in_ten_seconds() {
        let mut d = Driver::new();
        let k = ground();
        for n in 0..MUSHROOMS {
            let a = press(&mut d, Pads { lb: true, ..Default::default() }, &k);
            assert!(a.mushroom && a.input.mushroom && !a.empty, "mushroom {n}");
        }
        let a = press(&mut d, Pads { lb: true, ..Default::default() }, &k);
        assert!(!a.mushroom && !a.input.mushroom && a.empty);
        assert_eq!(d.mushrooms().0, 0);
        // (one back after 10 s since the first went)
        for _ in 0..(10.0 / DT) as usize {
            d.tick(&Pads::default(), &k, DT);
        }
        assert!(d.mushrooms().0 >= 1, "{:?}", d.mushrooms());
        // (all back, the bar full)
        for _ in 0..(25.0 / DT) as usize {
            d.tick(&Pads::default(), &k, DT);
        }
        assert_eq!(d.mushrooms(), (MUSHROOMS, 1.0));
        // (held, one only)
        let mut used = 0;
        for _ in 0..30 {
            used += d.tick(&Pads { lb: true, ..Default::default() }, &k, DT).mushroom as u32;
        }
        assert_eq!(used, 1);
    }

    #[test]
    fn the_fire_flower_every_so_often() {
        let mut d = Driver::new();
        let k = ground();
        let mut shots = 0;
        // (mashed for a second: a press every other tick)
        for t in 0..30 {
            shots += d.tick(&Pads { rb: t % 2 == 0, ..Default::default() }, &k, DT).fire as u32;
        }
        assert!((1..=2).contains(&shots), "{shots}");
    }

    #[test]
    fn a_banana_tapped_falls_behind_held_it_is_thrown_ahead() {
        let mut d = Driver::new();
        let k = ground();
        let y = Pads { y: true, ..Default::default() };
        let mut got = Vec::new();
        // a tap: three ticks
        for p in [y, y, y, Pads::default()] {
            got.extend(d.tick(&p, &k, DT).banana);
        }
        assert_eq!(got, vec![Banana::Back]);
        // held 8 ticks: ahead, once, on its release
        got.clear();
        for _ in 0..BANANA_HOLD {
            got.extend(d.tick(&y, &k, DT).banana);
        }
        assert!(got.is_empty());
        got.extend(d.tick(&Pads::default(), &k, DT).banana);
        assert_eq!(got, vec![Banana::Ahead]);
        // (the shell: X, a press each)
        assert!(press(&mut d, Pads { x: true, ..Default::default() }, &k).shell);
        assert!(!d.tick(&Pads { x: true, ..Default::default() }, &k, DT).shell);
    }

    #[test]
    fn a_spin_out_turns_twice_and_takes_the_wheel() {
        let mut d = Driver::new();
        let k = ground();
        assert!(d.spin_out());
        assert!(!d.spin_out(), "one on top of another");
        let full = Pads { rt: 1.0, stick: Vec2::new(1.0, 0.0), a: true, ..Default::default() };
        let mut last = 0.0;
        let mut ticks = 0;
        while d.spinning() {
            let a = d.tick(&full, &k, DT);
            assert!(a.input.throttle == 0.0 && a.input.steer == 0.0 && !a.input.drift && a.input.brake > 0.0);
            if d.spinning() {
                let s = d.spin();
                assert!(s >= last - 1e-4, "it turns one way");
                last = s;
            }
            ticks += 1;
            assert!(ticks < 60);
        }
        assert!((ticks as f32 * DT - SPIN_TIME).abs() < 0.1, "{ticks}");
        assert!(last > 3.5 * std::f32::consts::PI, "{last}");
        assert_eq!(d.spin(), 0.0);
        // (the wheel back)
        let a = d.tick(&full, &k, DT);
        assert!(a.input.throttle == 1.0 && a.input.steer == 1.0);
    }

    #[test]
    fn a_trick_off_a_lip_rolls_once_and_boosts_its_landing() {
        let mut d = Driver::new();
        let up = KartNow { grounded: 0, vy: 5.0, ..ground() };
        let a_press = Pads { a: true, ..Default::default() };
        // (off the ground going up: A within 0.3 s)
        d.tick(&Pads::default(), &ground(), DT);
        d.tick(&Pads::default(), &up, DT);
        let a = d.tick(&a_press, &up, DT);
        assert!(a.trick);
        let mut most = 0.0f32;
        for _ in 0..(TRICK_TIME / DT) as usize - 2 {
            d.tick(&a_press, &KartNow { vy: -2.0, ..up }, DT);
            most = most.max(d.roll());
        }
        assert!(most > 5.0, "{most}");
        // landing: its boost, once
        let a = d.tick(&Pads::default(), &ground(), DT);
        assert!(a.landed_trick);
        assert!(!d.tick(&Pads::default(), &ground(), DT).landed_trick);
        // too late (0.5 s in the air), or off the ground falling: none
        let mut d = Driver::new();
        d.tick(&Pads::default(), &ground(), DT);
        for _ in 0..15 {
            d.tick(&Pads::default(), &up, DT);
        }
        assert!(!d.tick(&a_press, &up, DT).trick);
        let mut d = Driver::new();
        d.tick(&Pads::default(), &ground(), DT);
        d.tick(&Pads::default(), &KartNow { vy: -3.0, ..up }, DT);
        assert!(!d.tick(&a_press, &KartNow { vy: -3.0, ..up }, DT).trick);
        // going up as it left the ground counts, though gravity took some by its first tick off it
        let mut d = Driver::new();
        d.tick(&Pads::default(), &KartNow { vy: 2.6, ..ground() }, DT);
        d.tick(&Pads::default(), &KartNow { vy: 1.4, ..up }, DT);
        assert!(d.tick(&a_press, &KartNow { vy: 1.1, ..up }, DT).trick);
    }

    /// On its roof, A is the hop that puts it right (kart.rs): no trick, even in a spin-out.
    #[test]
    fn on_its_roof_a_is_the_hop() {
        let mut d = Driver::new();
        let a = Pads { a: true, ..Default::default() };
        let roof = KartNow { grounded: 0, vy: 3.0, upset: true, ..ground() };
        d.tick(&Pads::default(), &roof, DT);
        let acts = d.tick(&a, &roof, DT);
        assert!(acts.input.hop && !acts.trick && !acts.super_hop, "{acts:?}");
        let mut d = Driver::new();
        d.spin_out();
        d.tick(&Pads::default(), &roof, DT);
        assert!(d.tick(&a, &roof, DT).input.hop);
    }

    #[test]
    fn the_super_hop_needs_a_boost_and_waits() {
        let mut d = Driver::new();
        let a = Pads { a: true, ..Default::default() };
        assert!(!press(&mut d, a, &ground()).super_hop);
        let boosting = KartNow { boost: 0.8, ..ground() };
        assert!(press(&mut d, a, &boosting).super_hop);
        assert!(!press(&mut d, a, &boosting).super_hop, "again at once");
        for _ in 0..70 {
            d.tick(&Pads::default(), &boosting, DT);
        }
        assert!(press(&mut d, a, &boosting).super_hop);
    }

    #[test]
    fn the_drift_going_up_is_told_once() {
        let mut d = Driver::new();
        let mut ups = Vec::new();
        for level in [0u8, 0, 1, 1, 2, 2, 3, 3, 0] {
            ups.extend(d.tick(&Pads::default(), &KartNow { drift: level, ..ground() }, DT).drift_up);
        }
        assert_eq!(ups, vec![1, 2, 3]);
    }

    #[test]
    fn rams_by_size_and_speed() {
        assert_eq!(ram(Size::Small, false, 3.0), None);
        let small = ram(Size::Small, false, 20.0).unwrap();
        assert_eq!(small.kind, RamKind::Fling);
        assert!(small.fling.is_some_and(|(v, up)| v > 10.0 && v <= FLING_MOST && up > 0.0));
        assert!(small.keep > 0.5 && !small.spin && (small.worth - 1.0).abs() < 1e-4);
        // (the worth clamped)
        assert_eq!(ram(Size::Small, false, 5.0).unwrap().worth, RAM_LEAST);
        assert_eq!(ram(Size::Small, false, 60.0).unwrap().worth, RAM_MOST);
        let medium = ram(Size::Medium, false, 15.0).unwrap();
        assert!(medium.kind == RamKind::Bump && medium.fling.is_none() && medium.keep < 0.0 && !medium.spin);
        for (size, boss) in [(Size::Big, false), (Size::Small, true), (Size::Medium, true)] {
            let r = ram(size, boss, 20.0).unwrap();
            assert!(r.kind == RamKind::Glance && r.spin && r.keep < 0.0 && r.worth < 1.0, "{size:?} {boss}");
        }
    }

    #[test]
    fn contact_in_front_at_speed_only() {
        let (pos, rot) = (Vec3::new(0.0, 0.5, 0.0), Quat::IDENTITY);
        let ahead = Vec3::new(0.0, 0.0, 1.6);
        assert!(contact(pos, rot, Vec3::Z * 12.0, ahead, 0.5, 1.8).is_some_and(|v| (v - 12.0).abs() < 1e-3));
        // (going away from it, or too far, or way above)
        assert!(contact(pos, rot, -Vec3::Z * 12.0, ahead, 0.5, 1.8).is_none());
        assert!(contact(pos, rot, Vec3::Z * 12.0, Vec3::new(0.0, 0.0, 3.0), 0.5, 1.8).is_none());
        assert!(contact(pos, rot, Vec3::Z * 12.0, Vec3::new(0.0, 4.0, 1.6), 0.5, 1.8).is_none());
        // (a big one's wide body reaches it from farther off)
        assert!(contact(pos, rot, Vec3::Z * 12.0, Vec3::new(0.0, 0.0, 3.0), 2.0, 5.0).is_some());
        // (turned: in its own axes)
        let turned = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        assert!(contact(pos, turned, Vec3::X * 12.0, Vec3::new(1.6, 0.0, 0.0), 0.5, 1.8).is_some());
    }

    #[test]
    fn a_blow_spins_him_out_a_heavy_one_unseats() {
        assert_eq!(hit(false), Taken { unseat: false, spin: true, coins: COINS_LOST });
        assert!(hit(true).unseat && !hit(true).spin);
    }

    /// A kart settled on flat ground (kart.rs's own floor).
    fn settled(steer: f32) -> Kart {
        let ray = |a: Vec3, b: Vec3| (a.y >= 0.0 && b.y < 0.0).then(|| a + (b - a) * (a.y / (a.y - b.y)));
        let mut k = Kart::new(Vec3::new(0.0, 0.8, 0.0), 0.0);
        for _ in 0..90 {
            k.step(&kart::Input { steer, ..Default::default() }, DT, &ray);
        }
        k
    }

    /// The ground as a height over (x, z): a ray's first hit (marched, then halved), none from under it.
    fn heightfield(h: impl Fn(f32, f32) -> f32 + Copy) -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        move |a: Vec3, b: Vec3| {
            let below = |p: Vec3| p.y <= h(p.x, p.z);
            if below(a) {
                return None;
            }
            let n = 24;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                if below(a.lerp(b, t)) {
                    let (mut lo, mut hi) = ((k - 1) as f32 / n as f32, t);
                    for _ in 0..20 {
                        let mid = (lo + hi) * 0.5;
                        if below(a.lerp(b, mid)) { hi = mid } else { lo = mid }
                    }
                    return Some(a.lerp(b, hi));
                }
            }
            None
        }
    }

    /// How deep the drawn kart is in the ground at worst and on average (m): its tyres all round
    /// their rims and its tub's belly (kartmesh.rs: 0.28 m under its middle), driven over `h`.
    fn sunk(h: impl Fn(f32, f32) -> f32 + Copy, lane: f32, throttle: f32, secs: f32) -> (f32, f32) {
        let ray = heightfield(h);
        let mut k = Kart::new(Vec3::new(lane, h(lane, 0.0) + 0.8, 0.0), 0.0);
        for _ in 0..60 {
            k.step(&kart::Input::default(), DT, &ray);
        }
        let (mut worst, mut sum, mut n) = (0.0f32, 0.0f32, 0usize);
        for _ in 0..(secs / DT) as usize {
            k.step(&kart::Input { throttle, ..Default::default() }, DT, &ray);
            let m = model(&k, 0.0, 0.0, &[0.0; 4]);
            let mut deepest = 0.0f32;
            for i in 0..4 {
                let c = m.parts[WHEELS + i].0;
                for a in -6..=6 {
                    let a = a as f32 * 0.2;
                    let p = c + k.rot * Vec3::new(0.0, -a.cos(), a.sin()) * kart::WHEEL_R;
                    deepest = deepest.max(h(p.x, p.z) - p.y);
                }
            }
            for x in [-0.4f32, 0.0, 0.4] {
                for z in [-0.6f32, 0.0, 0.45] {
                    let p = k.pos + k.rot * Vec3::new(x, -0.28, z);
                    deepest = deepest.max(h(p.x, p.z) - p.y);
                }
            }
            worst = worst.max(deepest);
            sum += deepest;
            n += 1;
        }
        (worst, sum / n as f32)
    }

    /// The user's photos (2026-10-24/25): the tyres sunk in the ground. Driven over bumps, a wheel
    /// was drawn where the step had moved its mount on (half a metre at speed), at the spring's
    /// length measured before: up to 0.29 m into the rise ahead (0.14 m over 12 cm bumps).
    #[test]
    fn the_drawn_kart_stays_out_of_the_ground() {
        for lane in [0.0f32, 3.1, 7.7] {
            for (amp, worst_most, mean_most) in [(0.0f32, 0.001f32, 0.001f32), (0.12, 0.03, 0.01), (0.25, 0.12, 0.03)] {
                let h = move |x: f32, z: f32| amp * ((x * 1.3).sin() * (z * 0.9).cos() + (z * 2.1 + x * 0.4).sin() * 0.5);
                for throttle in [0.0f32, 1.0] {
                    let (worst, mean) = sunk(h, lane, throttle, 6.0);
                    assert!(worst <= worst_most && mean <= mean_most, "bumps {amp} lane {lane} throttle {throttle}: {worst:.3} m deep at worst, {mean:.3} on average");
                }
            }
        }
        // parked on a 20 degree slope, across it and up it
        for (sx, sz) in [(0.36f32, 0.0f32), (0.0, 0.36)] {
            let (worst, _) = sunk(move |x, z| x * sx + z * sz, 0.0, 0.0, 3.0);
            assert!(worst < 0.01, "slope {sx} {sz}: {worst:.3}");
        }
    }

    #[test]
    fn the_tyre_gaps_read_the_ground_under_them() {
        let ray = heightfield(|_, _| 0.0);
        let k = settled(0.0);
        let gaps = tyre_gaps(&model(&k, 0.0, 0.0, &[0.0; 4]), &ray);
        assert!(gaps.iter().all(|g| g.is_some_and(|g| g.abs() < 0.01)), "{gaps:?}");
        let none = |_: Vec3, _: Vec3| None;
        assert!(tyre_gaps(&model(&k, 0.0, 0.0, &[0.0; 4]), &none).iter().all(|g| g.is_none()));
    }

    #[test]
    fn the_model_hangs_its_wheels_and_turns_the_front_ones() {
        let k = settled(0.0);
        let m = model(&k, 0.0, 0.0, &[0.0; 4]);
        assert!(m.parts[BODY].0.distance(k.pos) < 1e-5);
        for i in 0..4 {
            let (p, _) = m.parts[WHEELS + i];
            let mount = k.pos + k.rot * kart::MOUNTS[i];
            // (under its mount by the spring, its bottom on the ground)
            assert!(p.y < mount.y - 0.05 && (p.y - kart::WHEEL_R).abs() < 0.05, "wheel {i} at {p}");
            assert!((p.with_y(0.0) - mount.with_y(0.0)).length() < 1e-3);
        }
        // steered: the front wheels turned (their axles off the body's), the rear not, the steering
        // wheel turned the front wheels' way
        let k = settled(1.0);
        let m = model(&k, 0.0, 0.0, &[0.0; 4]);
        let axle = |i: usize| m.parts[WHEELS + i].1 * Vec3::X;
        let body_x = m.parts[BODY].1 * Vec3::X;
        assert!(axle(0).dot(body_x) < 0.995 && axle(1).dot(body_x) < 0.995);
        assert!(axle(2).dot(body_x) > 0.9999 && axle(3).dot(body_x) > 0.9999);
        // (the front wheel's way and the rim's top both lean to the kart's right())
        let ahead = m.parts[WHEELS].1 * Vec3::Z;
        let top = m.parts[STEERING].1 * Vec3::Y;
        assert!(ahead.dot(k.right()) > 0.05 && top.dot(k.right()) > 0.05, "{} {}", ahead.dot(k.right()), top.dot(k.right()));
        // rolled: round their axles
        let m2 = model(&k, 0.0, 0.0, &[1.0; 4]);
        let up = |m: &Model, i: usize| m.parts[WHEELS + i].1 * Vec3::Y;
        assert!(up(&m2, 2).dot(up(&m, 2)) < 0.6);
        assert!((m2.parts[WHEELS + 2].1 * Vec3::X).dot(axle(2)) > 0.9999);
    }

    #[test]
    fn the_seat_turns_with_the_spin_and_the_roll() {
        let k = settled(0.0);
        let (feet, face, tilt) = seat(&k, 0.0, 0.0);
        assert!(feet.distance(k.pos + k.rot * SEAT) < 1e-4);
        assert!(face.dot(k.forward().with_y(0.0).normalize()) > 0.9999);
        assert!(tilt.angle_between(Quat::IDENTITY) < 0.01);
        // a half turn of the spin: he faces back
        let (_, back, _) = seat(&k, std::f32::consts::PI, 0.0);
        assert!(back.dot(face) < -0.999);
        // the roll: on his side, the way he faces kept
        let (_, f2, t2) = seat(&k, 0.0, std::f32::consts::FRAC_PI_2);
        assert!(f2.dot(face) > 0.999 && (t2 * Vec3::Y).y.abs() < 0.01);
    }

    #[test]
    fn the_grips_turn_with_the_wheel() {
        let m = model(&settled(0.0), 0.0, 0.0, &[0.0; 4]);
        let [a, b] = grips(&m);
        let (c, rot) = m.parts[STEERING];
        assert!((a.distance(c) - WHEEL_RIM).abs() < 1e-3 && (b.distance(c) - WHEEL_RIM).abs() < 1e-3);
        assert!((a - c).dot(rot * Vec3::X) > 0.1 && (b - c).dot(rot * Vec3::X) < -0.1);
        // (both above its middle: ten and two)
        assert!((a - c).dot(rot * Vec3::Y) > 0.0 && (b - c).dot(rot * Vec3::Y) > 0.0);
        let m2 = model(&settled(1.0), 0.0, 0.0, &[0.0; 4]);
        assert!(grips(&m2)[0].distance(m2.parts[STEERING].0 + (a - c)) > 0.02, "they turn with it");
    }

    #[test]
    fn wheels_drawn_no_faster_than_the_eye_follows() {
        assert!((wheel_rate(2.2) - 10.0).abs() < 1e-3);
        assert_eq!(wheel_rate(100.0), WHEEL_RATE_MOST);
        assert_eq!(wheel_rate(-100.0), -WHEEL_RATE_MOST);
    }

    #[test]
    fn the_floor_follows_him() {
        let f = [0.0, 0.0, 0.0];
        assert!(floor_reload(None, f));
        assert!(!floor_reload(Some(f), [100.0, FLOOR_DROP, 0.0]));
        assert!(floor_reload(Some(f), [FLOOR_REACH + 1.0, FLOOR_DROP, 0.0]));
        assert!(floor_reload(Some(f), [0.0, 5.0, 0.0]), "nearly under it");
        assert!(floor_reload(Some(f), [0.0, FLOOR_OVER_MOST + 1.0, 0.0]));
    }

    #[test]
    fn the_steer_follows_the_cameras_hands() {
        let (fwd, up) = (Vec3::Z, Vec3::Y);
        assert_eq!(steer_sign(fwd, up, fwd.cross(up)), 1.0);
        assert_eq!(steer_sign(fwd, up, -fwd.cross(up)), -1.0);
    }

    #[test]
    fn the_engine_climbs_with_speed() {
        let idle = engine_tone(0.0, 0.0, false, 0.0);
        let half = engine_tone(12.0, 1.0, false, 0.0);
        let top = engine_tone(25.0, 1.0, false, 0.0);
        assert!(idle.freq > 30.0 && idle.freq < half.freq && half.freq < top.freq);
        assert!(engine_tone(25.0, 1.0, true, 0.0).freq > top.freq);
        assert!(half.volume > idle.volume && top.volume <= 0.3);
        assert_eq!(engine_tone(5.0, 0.0, false, 3.0).skid, 1.0);
        // (backwards too)
        assert!((engine_tone(-12.0, 1.0, false, 0.0).freq - half.freq).abs() < 1e-3);
    }

    #[test]
    fn falls_kill_past_twenty_two_metres() {
        assert!(!lethal(10.0, 0.0));
        assert!(lethal(30.0, 5.0));
    }

    const ZOOMS: [(f32, f32); 3] = [(5.0, 1.6), (7.0, 2.2), (9.5, 3.0)];

    /// Frames of 60 fps of a kart going `vel` from `pos`, its heading `heading(t)` (radians round
    /// +y, 0 along +z), its height `height(t)`; the camera's yaw (its eye to the kart, flat) and
    /// height over the frames after the first second.
    fn drive_cam(zoom: (f32, f32), secs: f32, speed: f32, heading: impl Fn(f32) -> f32, height: impl Fn(f32) -> f32) -> (Vec<f32>, Vec<f32>, ChaseCam, Vec3) {
        let mut cam = ChaseCam::default();
        let (mut pos, dt) = (Vec3::ZERO, 1.0 / 60.0);
        let (mut yaws, mut ys) = (Vec::new(), Vec::new());
        for f in 0..(secs * 60.0) as usize {
            let t = f as f32 * dt;
            let h = heading(t);
            let fwd = Vec3::new(h.sin(), 0.0, h.cos());
            pos += fwd * speed * dt;
            pos.y = height(t);
            let (eye, _) = cam.step(pos, fwd, fwd * speed, zoom, 0.0, false, dt);
            if t > 1.0 {
                let back = (pos - eye).with_y(0.0);
                yaws.push(back.x.atan2(back.z));
                ys.push(eye.y);
            }
        }
        (yaws, ys, cam, pos)
    }

    fn spread(v: &[f32]) -> f32 {
        v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
    }

    /// Mario's own camera's distance: near, middle, far (lakitu.rs's zoom), the kart's view too.
    #[test]
    fn the_kart_camera_keeps_marios_zoom() {
        let mut gaps = Vec::new();
        for zoom in ZOOMS {
            let mut cam = ChaseCam::default();
            let pos = Vec3::new(3.0, 1.0, 3.0);
            let mut eye = Vec3::ZERO;
            for _ in 0..120 {
                eye = cam.step(pos, Vec3::Z, Vec3::ZERO, zoom, 0.0, false, 1.0 / 60.0).0;
            }
            let flat = (pos - eye).with_y(0.0);
            assert!(flat.normalize().dot(Vec3::Z) > 0.99, "behind it: {eye}");
            assert!((flat.length() - zoom.0).abs() < zoom.0 * 0.2, "zoom {zoom:?}: {} m back", flat.length());
            assert!(eye.y - pos.y > zoom.1 * 0.8, "zoom {zoom:?}: {} m up", eye.y - pos.y);
            gaps.push(flat.length());
        }
        assert!(gaps[0] < gaps[1] && gaps[1] < gaps[2], "{gaps:?}");
    }

    /// Where the eye settles behind a kart going `speed` along its nose for 3 s: (back, up) from it.
    fn settled_view(speed: f32) -> (f32, f32) {
        let mut cam = ChaseCam::default();
        let (mut pos, dt) = (Vec3::ZERO, 1.0 / 60.0);
        let mut eye = Vec3::ZERO;
        for _ in 0..180 {
            pos += Vec3::Z * speed * dt;
            eye = cam.step(pos, Vec3::Z, Vec3::Z * speed, ZOOMS[1], 0.0, false, dt).0;
        }
        ((pos - eye).with_y(0.0).length(), eye.y - pos.y)
    }

    /// The user (third test): going fast forward or backing up fast, the camera should pull back
    /// much farther (before: 1.4 m more at the top speed, 0.4 m backing up at 10 m/s).
    #[test]
    fn the_kart_camera_pulls_back_at_speed() {
        let (rest, rest_up) = settled_view(0.0);
        let (half, _) = settled_view(kart::TOP * 0.5);
        let (top, top_up) = settled_view(kart::TOP);
        let (backing, _) = settled_view(-12.0);
        assert!(half - rest > 1.5, "half speed: {:.2} m more", half - rest);
        assert!(top - rest > 3.2, "top speed: {:.2} m more", top - rest);
        assert!(top_up - rest_up > 0.6, "top speed: {:.2} m higher", top_up - rest_up);
        assert!(backing - rest > 2.5, "backing up: {:.2} m more", backing - rest);
    }

    /// Its springs bouncing it 10 cm at 6 Hz and its heading shivering 6 degrees at 8 Hz (rough
    /// ground): the camera barely moves (before: it shook with the chassis).
    #[test]
    fn the_kart_camera_rides_out_the_bumps() {
        let (yaws, ys, _, _) = drive_cam(ZOOMS[0], 4.0, 20.0, |t| (6.0f32.to_radians()) * (t * 8.0 * std::f32::consts::TAU).sin(), |t| 0.1 * (t * 6.0 * std::f32::consts::TAU).sin());
        assert!(spread(&ys) < 0.06, "height shakes {:.3} m", spread(&ys));
        assert!(spread(&yaws).to_degrees() < 3.0, "yaw shakes {:.1} deg", spread(&yaws).to_degrees());
    }

    /// A steady turn: it swings round behind, a little late (the turn shows), never left far off.
    #[test]
    fn the_kart_camera_follows_a_turn() {
        let (yaws, _, _, _) = drive_cam(ZOOMS[1], 4.0, 18.0, |t| t * 1.0, |_| 0.0);
        let (last, heading) = (*yaws.last().unwrap(), 4.0 - 1.0 / 60.0);
        let lag = (heading - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        assert!(lag > 0.02 && lag < 0.6, "lag {:.1} deg", lag.to_degrees());
    }

    /// Backing up: it stays behind the kart's nose' way, it doesn't swing round to face it.
    #[test]
    fn the_kart_camera_stays_behind_when_backing_up() {
        let (yaws, _, _, _) = drive_cam(ZOOMS[0], 3.0, -6.0, |_| 0.0, |_| 0.0);
        assert!(yaws.iter().all(|y| y.abs() < 0.2), "{:?}", yaws.last());
    }

    /// In a drift (its nose 35 degrees in, sliding along its way): between the two, nearer its way.
    #[test]
    fn the_kart_camera_looks_down_a_drift() {
        let mut cam = ChaseCam::default();
        let nose = Quat::from_rotation_y(35f32.to_radians()) * Vec3::Z;
        let mut eye = Vec3::ZERO;
        for _ in 0..240 {
            eye = cam.step(Vec3::ZERO, nose, Vec3::Z * 20.0, ZOOMS[0], 0.0, false, 1.0 / 60.0).0;
        }
        let back = (-eye).with_y(0.0);
        let yaw = back.x.atan2(back.z).to_degrees();
        assert!(yaw > 2.0 && yaw < 17.5, "{yaw:.1}");
    }

    /// The right stick looks round it, and the view comes back behind once let go.
    #[test]
    fn the_kart_camera_looks_round_with_the_stick() {
        let mut cam = ChaseCam::default();
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            cam.step(Vec3::ZERO, Vec3::Z, Vec3::ZERO, ZOOMS[0], 1.2, false, dt);
        }
        let (eye, _) = cam.step(Vec3::ZERO, Vec3::Z, Vec3::ZERO, ZOOMS[0], 1.2, false, dt);
        let b = (-eye).with_y(0.0);
        assert!(b.x.atan2(b.z).abs() > 0.8, "{eye}");
    }

    #[test]
    fn the_flames_take_the_charges_colour() {
        assert_eq!(flame_of(0, 0.0, 0), None);
        assert_eq!(flame_of(2, 0.0, 0).map(|f| f.0), Some(SPARKS[1]));
        let (c, big) = flame_of(0, 1.0, 3).unwrap();
        assert_eq!(c, SPARKS[2]);
        assert_eq!(flame_of(0, 1.0, 0).unwrap().0, BOOST_FLAME);
        assert!(big > flame_of(1, 0.0, 0).unwrap().1);
    }
}
