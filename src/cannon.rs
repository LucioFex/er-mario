//! The portable cannon (v4 Part 7): D-pad up held half a second plants an SM64 cannon where Mario
//! stands (under open sky, out of a boss fight, a few coins, once a minute); he hops in, the stick
//! aims it (SM64's way: up aims lower; the right stick the camera's way), A held fills its power and
//! let go fires him 100-300 m in SM64's cannon flight, its arc and landing shown first. The wing cap
//! (a tap of D-pad up, now on the button's release) opens as he falls; LT on the way is a ground
//! pound that lands as a shockwave on the enemies round him. A far landing the game hasn't loaded
//! yet caps the shot.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{CSCamExt, CSCamera, ChrIns};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::{Vec2, Vec3};

use crate::colossus;
use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

// ---- pure ------------------------------------------------------------------------------------

/// D-pad up held this long plants the cannon (ticks); its ring shows from this tick on
pub const HOLD_TICKS: u32 = 15;
const HOLD_SHOWS: u32 = 4;
/// SM64's cannon flight in metres: gravity (1 unit a tick²), the fall's terminal speed (75 units),
/// the forward speed lost (0.05 units a tick²) down to this
pub const GRAVITY: f32 = 9.0;
pub const TERMINAL: f32 = 22.5;
const DRAG: f32 = 0.45;
const SLOWEST: f32 = 3.0;
/// out of the barrel by power (m/s): about 100 m at 45° on flat ground, 300 m
pub const SPEED: (f32, f32) = (31.0, 52.0);
/// the preview: rays between points this many ticks apart, at most this long a flight (s), a dot
/// every this long (s)
const ARC_STEP: u32 = 6;
pub const ARC_MOST: f32 = 14.0;
const DOT_EVERY: f32 = 0.2;
/// the governor's halvings
const HALVINGS: u32 = 5;
/// open sky: nothing this far straight up (m), nor up most of these four tilted ones (m, radians)
const SKY_UP: f32 = 50.0;
const SKY_SIDE: f32 = 40.0;
const SKY_TILT: f32 = 0.35;
/// the barrel's pitch (radians) and how fast the sticks turn it (radians a second)
pub const PITCH_MIN: f32 = 0.14;
pub const PITCH_MAX: f32 = 1.31;
pub const PITCH_START: f32 = 0.7;
const YAW_RATE: f32 = 1.6;
const PITCH_RATE: f32 = 0.9;
/// A held fills the power in this long (s)
const POWER_FILL: f32 = 1.0;
const TICK: f32 = 1.0 / 30.0;

/// D-pad up: a tap (the wing cap, as it's let go), or held long enough to plant the cannon.
#[derive(Default)]
pub struct Hold {
    ticks: u32,
    planted: bool,
}

/// What D-pad up did this tick: a tap ended, the hold planted, how far the hold is (0-1, shown).
#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub struct Press {
    pub tap: bool,
    pub plant: bool,
    pub charge: Option<f32>,
}

impl Hold {
    /// Every tick with the button's state.
    pub fn step(&mut self, down: bool) -> Press {
        if !down {
            let tap = self.ticks > 0 && !self.planted;
            *self = Hold::default();
            return Press { tap, ..Press::default() };
        }
        self.ticks += 1;
        let plant = !self.planted && self.ticks >= HOLD_TICKS;
        self.planted |= plant;
        let charge = (!self.planted && self.ticks >= HOLD_SHOWS).then(|| self.ticks as f32 / HOLD_TICKS as f32);
        Press { tap: false, plant, charge }
    }
}

/// Out of the barrel at `power` (0-1): its speed (m/s).
pub fn launch_speed(power: f32) -> f32 {
    SPEED.0 + (SPEED.1 - SPEED.0) * power.clamp(0.0, 1.0)
}

/// The shot's velocity: along `dir` (flat), up at `pitch`, at `speed`.
pub fn launch(dir: Vec3, pitch: f32, speed: f32) -> Vec3 {
    dir.with_y(0.0).normalize_or(Vec3::Z) * (pitch.cos() * speed) + Vec3::Y * (pitch.sin() * speed)
}

/// One step of SM64's cannon flight: where he is and his speed after `dt`.
pub fn fly(pos: Vec3, vel: Vec3, dt: f32) -> (Vec3, Vec3) {
    let flat = vel.with_y(0.0);
    let s = flat.length();
    let slower = (s - DRAG * dt).max(SLOWEST.min(s));
    let flat = if s > 1e-4 { flat * (slower / s) } else { flat };
    let v = Vec3::new(flat.x, (vel.y - GRAVITY * dt).max(-TERMINAL), flat.z);
    (pos + v * dt, v)
}

/// The flight ahead: its dots, where and when it lands (None: nothing within ARC_MOST s).
#[derive(Default, Clone)]
pub struct Arc {
    pub dots: Vec<Vec3>,
    pub landing: Option<(Vec3, f32)>,
}

/// The flight from `from` at `vel`, by the game's rays.
pub fn arc(from: Vec3, vel: Vec3, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Arc {
    let (mut p, mut v) = (from, vel);
    let mut dots: Vec<(Vec3, f32)> = Vec::new();
    let (mut start, mut next_dot) = (from, DOT_EVERY);
    let most = (ARC_MOST / TICK) as u32;
    for tick in 1..=most {
        (p, v) = fly(p, v, TICK);
        let t = tick as f32 * TICK;
        if t >= next_dot {
            dots.push((p, t));
            next_dot += DOT_EVERY;
        }
        if tick % ARC_STEP == 0 || tick == most {
            if let Some(h) = ray(start, p) {
                // (when along the segment: as far along it as the hit is)
                let k = ((h - start).length() / (p - start).length().max(1e-4)).clamp(0.0, 1.0);
                let seg = (tick - (tick - 1) / ARC_STEP * ARC_STEP) as f32 * TICK;
                let at = t - seg * (1.0 - k);
                return Arc { dots: dots.into_iter().filter(|d| d.1 < at).map(|d| d.0).collect(), landing: Some((h, at)) };
            }
            start = p;
        }
    }
    Arc { dots: dots.into_iter().map(|d| d.0).collect(), landing: None }
}

/// The streaming governor: the most power up to `wanted` whose shot lands on something the game has
/// loaded; None: not even the weakest does.
pub fn cap_power(from: Vec3, dir: Vec3, pitch: f32, wanted: f32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Option<f32> {
    let mut s = Search::new(wanted);
    while let Some(p) = s.next() {
        s.step(arc(from, launch(dir, pitch, launch_speed(p)), ray).landing.is_some());
    }
    s.found.flatten()
}

/// The governor a flight at a time (its rays spread over ticks): `next` is the power to try,
/// `step` takes whether it landed; `found` once done (None inside: not even the weakest lands).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Search {
    pub found: Option<Option<f32>>,
    wanted: f32,
    lo: f32,
    hi: f32,
    stage: u32,
}

impl Search {
    pub fn new(wanted: f32) -> Search {
        Search { found: None, wanted, lo: 0.0, hi: wanted, stage: 0 }
    }

    pub fn next(&self) -> Option<f32> {
        match (self.found, self.stage) {
            (Some(_), _) => None,
            (None, 0) => Some(self.wanted),
            (None, 1) => Some(0.0),
            _ => Some((self.lo + self.hi) * 0.5),
        }
    }

    pub fn step(&mut self, landed: bool) {
        let Some(p) = self.next() else { return };
        match self.stage {
            // (as wanted, then the weakest)
            0 if landed => self.found = Some(Some(self.wanted)),
            1 if !landed => self.found = Some(None),
            0 | 1 => {}
            // (the halvings between)
            k => {
                if landed {
                    self.lo = p;
                } else {
                    self.hi = p;
                }
                if k - 1 >= HALVINGS {
                    self.found = Some(Some(self.lo));
                }
            }
        }
        self.stage += 1;
    }
}

/// Nothing over his head: no roof straight up, nor over most of the sky round it.
pub fn open_sky(head: Vec3, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> bool {
    if ray(head, head + Vec3::Y * SKY_UP).is_some() {
        return false;
    }
    // (a wall or a cliff beside him takes one of them: a roof or an overhang most)
    let blocked = (0..4)
        .filter(|&k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_2;
            let d = Vec3::new(a.cos() * SKY_TILT.sin(), SKY_TILT.cos(), a.sin() * SKY_TILT.sin());
            ray(head, head + d * SKY_SIDE).is_some()
        })
        .count();
    blocked < 3
}

/// The sticks to the barrel's turn (towards the screen's right) and lift (up): the left stick SM64's
/// (up aims lower), the right stick the camera's (up aims higher).
pub fn sticks(left: Vec2, right: Vec2) -> (f32, f32) {
    ((left.x + right.x).clamp(-1.0, 1.0), (right.y - left.y).clamp(-1.0, 1.0))
}

/// The barrel turned: yaw by `turn`, pitch by `lift` (each -1..1), for `dt`.
pub fn aim(yaw: f32, pitch: f32, turn: f32, lift: f32, dt: f32) -> (f32, f32) {
    let yaw = (yaw + turn.clamp(-1.0, 1.0) * YAW_RATE * dt).rem_euclid(std::f32::consts::TAU);
    (yaw, (pitch + lift.clamp(-1.0, 1.0) * PITCH_RATE * dt).clamp(PITCH_MIN, PITCH_MAX))
}

/// The power, A `held` for `dt` more.
pub fn power_step(power: f32, held: bool, dt: f32) -> f32 {
    if held { (power + dt / POWER_FILL).min(1.0) } else { power }
}

// ---- in the game -----------------------------------------------------------------------------

/// once a minute (s, from the shot); its price (coins, back if he gets out with B)
const RECHARGE: f32 = 60.0;
pub const COST: u32 = 4;
/// the cannon: its pivot this high over the ground (m), the barrel this far behind and ahead of it;
/// it rises in this many ticks, he's in by this tick (hidden from two before)
pub const PIVOT: f32 = 1.0;
pub const BREECH: f32 = 0.7;
pub const MUZZLE: f32 = 1.5;
const RISE_TICKS: u32 = 8;
const IN_BY: u32 = 14;
/// his hop in: this high over the pivot at its top (m)
const HOP: f32 = 1.2;
/// after the shot it stays this long (s), then sinks over this long (s)
const STAYS: f32 = 1.5;
const SINKS: f32 = 0.5;
/// his feet this far under the pivot in the barrel (m)
const INSIDE_DROP: f32 = 0.5;
/// the preview again every this many ticks; the governor's cap again when the aim moved this far
/// (radians) or every this many ticks
const PREVIEW_EVERY: u32 = 3;
const CAP_MOVED: f32 = 0.04;
const CAP_EVERY: u32 = 15;
/// a flight this much longer (s) than the landing it expected: he's let fall
const FLIGHT_SPARE: f32 = 2.0;
/// the landing's dive slide at most this fast (SM64 units a tick); a wall's knockback (SM64's)
const SLIDE_MOST: f32 = 40.0;
const WALL_KB: f32 = -16.0;
/// out of the cannon's hands (landed, a wall, the wing cap, let fall): a fall never counts from
/// higher than this over him (m), until he's on the ground, in SM64's wing cap flight (its own), or
/// this long has gone (ticks: a fall from the highest shot takes about 8 s)
const GRACE_DROP: f32 = 8.0;
const GRACE_TICKS: u32 = 600;
/// the pound: its spin (ticks), then straight down this fast (m/s), at most this long (ticks); its
/// shockwave reaches bodies this far off (m), feet this far under / over his
const POUND_SPIN: u32 = 10;
const POUND_SPEED: f32 = 25.0;
const POUND_MOST: u32 = 240;
pub const POUND_REACH: f32 = 6.0;
const POUND_BELOW: f32 = 6.0;
const POUND_ABOVE: f32 = 3.0;
/// his body's middle over his feet in the flight (where its rays go from)
const BODY: f32 = 0.4;
/// out with B: a hop up (m/s)
const HOP_OUT: f32 = 7.0;
/// ACT_ER_CLING's poses (libsm64's er_cling_mode): the hop in, in the barrel, shot, the pound
const LEAP: i32 = 2;
const IN_BARREL: i32 = 11;
const SHOT: i32 = 12;
const POUND: i32 = 13;
/// SM64's actions he's handed back to
const ACT_DIVE_SLIDE: u32 = 0x0088_0456;
const ACT_BACKWARD_AIR_KB: u32 = 0x0102_08B0;
const ACT_FLYING: u32 = 0x1088_0899;
const ACT_GROUND_POUND_LAND: u32 = 0x0080_023C;
const ACT_FREEFALL: u32 = 0x0100_088C;
const ACT_FLAG_AIR: u32 = 0x800;

/// SM64's sounds (the ROM's): the cannon up (SOUND_GENERAL_CANNON_UP), its aim turning
/// (SOUND_MOVING_AIM_CANNON, played while it turns), the shot (SOUND_OBJ_POUNDING_CANNON,
/// SOUND_ACTION_FLYING_FAST, SOUND_MARIO_YAHOO), the pound's landing
/// (SOUND_ACTION_TERRAIN_HEAVY_LANDING), Lakitu's buzz
const SOUND_UP: i32 = 0x3047_8081;
const SOUND_AIM: i32 = 0x1D19_2001;
const SOUND_BOOM: i32 = 0x501A_5081;
const SOUND_WHOOSH: i32 = 0x0456_8081;
const SOUND_YAHOO: i32 = 0x2404_8081;
const SOUND_HEAVY: i32 = 0x0460_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    /// planted at `base` (his feet then), the barrel turned to `yaw`, since this tick: it rises, he
    /// hops in
    Rising { base: Vec3, yaw: f32, since: u32 },
    /// in it: the aim, the power (filling while A's held since a press: `charging`), the governor's
    /// last cap (None: no landing at all), its search going on and the tick and aim it's for
    Inside { base: Vec3, yaw: f32, pitch: f32, power: f32, charging: bool, cap: Option<f32>, search: Search, aim_of: (u32, f32, f32), since: u32 },
    /// shot from `from`: his feet, speed, since this tick, the landing it expected (s after)
    Flight { pos: Vec3, vel: Vec3, since: u32, expect: f32, from: Vec3 },
    /// LT in the flight: the spin from `since`, then straight down
    Pound { pos: Vec3, since: u32, from: Vec3 },
    /// out of its hands: no fall from higher than GRACE_DROP until he's down or `until`
    Grace { until: u32 },
}

struct State {
    phase: Phase,
    hold: Hold,
    /// when it can be planted again
    ready_at: Option<Instant>,
    /// the cannon left behind after a shot (or once he's out): its base, yaw, pitch, since when
    left: Option<(Vec3, f32, f32, Instant)>,
    /// the flight ahead in the barrel
    preview: Arc,
    /// the hold's ring: over his head, how far (0-1)
    holding: Option<(Vec3, f32)>,
    /// buttons last tick: A, B, LT
    prev: [bool; 3],
    /// the right stick (x right, y up, -1..1), every frame
    right: Vec2,
    /// Mario's libsm64 id
    id: i32,
}

static STATE: Mutex<State> = Mutex::new(State {
    phase: Phase::Idle,
    hold: Hold { ticks: 0, planted: false },
    ready_at: None,
    left: None,
    preview: Arc { dots: Vec::new(), landing: None },
    holding: None,
    prev: [false; 3],
    right: Vec2::ZERO,
    id: -1,
});

/// The barrel's way flat, from its yaw (lakitu.rs's convention).
fn flat_of(yaw: f32) -> Vec3 {
    Vec3::new(yaw.cos(), 0.0, yaw.sin())
}

/// The cannon as hud.rs draws it: its base (on the ground), the barrel's way (flat) and pitch, how
/// big (0-1: rising, sinking).
#[derive(Clone, Copy)]
pub struct Look {
    pub base: Vec3,
    pub dir: Vec3,
    pub pitch: f32,
    pub size: f32,
}

/// What hud.rs draws: the cannon, the flight's dots and its landing (how far off), the power and
/// the governor's cap while in it (None: out of it), no landing at all, the hold's ring.
#[derive(Clone, Default)]
pub struct Hud {
    pub cannon: Option<Look>,
    pub dots: Vec<Vec3>,
    pub landing: Option<Vec3>,
    pub metres: f32,
    pub power: Option<(f32, f32)>,
    pub blocked: bool,
    pub hold: Option<(Vec3, f32)>,
}

pub fn hud() -> Option<Hud> {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = Hud { hold: s.holding, ..Hud::default() };
    match s.phase {
        Phase::Rising { base, yaw, since } => {
            let t = (TICKS.load(std::sync::atomic::Ordering::Relaxed).wrapping_sub(since) as f32 / RISE_TICKS as f32).min(1.0);
            // (a pop: up a little past its size and back)
            let size = 1.0 - (1.0 - t).powi(3) + 0.12 * (t * std::f32::consts::PI).sin();
            h.cannon = Some(Look { base, dir: flat_of(yaw), pitch: PITCH_START, size });
        }
        Phase::Inside { base, yaw, pitch, power, cap, .. } => {
            h.cannon = Some(Look { base, dir: flat_of(yaw), pitch, size: 1.0 });
            h.dots = s.preview.dots.clone();
            h.landing = s.preview.landing.map(|l| l.0);
            h.metres = h.landing.map_or(0.0, |l| (l - base).with_y(0.0).length());
            h.power = Some((power, cap.unwrap_or(0.0)));
            h.blocked = cap.is_none();
        }
        _ => {}
    }
    if let (None, Some((base, yaw, pitch, at))) = (h.cannon, s.left) {
        let t = at.elapsed().as_secs_f32();
        if t < STAYS + SINKS {
            let size = 1.0 - ((t - STAYS) / SINKS).clamp(0.0, 1.0);
            h.cannon = Some(Look { base, dir: flat_of(yaw), pitch, size: size * size * (3.0 - 2.0 * size) });
        }
    }
    (h.cannon.is_some() || h.hold.is_some()).then_some(h)
}

/// the tick the cannon last ran on (for the HUD's rise)
static TICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// In the barrel: Mario's not drawn (lib.rs's opacity).
pub fn hidden() -> bool {
    match STATE.lock().unwrap_or_else(|e| e.into_inner()).phase {
        Phase::Inside { .. } => true,
        Phase::Rising { since, .. } => TICKS.load(std::sync::atomic::Ordering::Relaxed).wrapping_sub(since) + 2 >= IN_BY,
        _ => false,
    }
}

/// The cannon has him (in it, shot, pounding): the stuck check and the other powers stand aside.
pub fn holds() -> bool {
    matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Rising { .. } | Phase::Inside { .. } | Phase::Flight { .. } | Phase::Pound { .. })
}

/// In it the right stick aims (not the camera's then).
pub fn holds_camera() -> bool {
    matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Inside { .. })
}

/// The right stick now (XInput's, every frame), and Elden Ring's "reverse camera X axis" (its
/// left and right the other way, as Lakitu's turn).
pub fn right_stick(x: i16, y: i16, reversed: bool) {
    let axis = |v: i16| {
        let f = v as f32 / 32767.0;
        if f.abs() < 0.2 { 0.0 } else { f.clamp(-1.0, 1.0) }
    };
    let x = if reversed { -axis(x) } else { axis(x) };
    STATE.lock().unwrap_or_else(|e| e.into_inner()).right = Vec2::new(x, axis(y));
}

/// Out of Mario mode, a load, dead: all of it gone (the recharge kept).
pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.phase = Phase::Idle;
    s.left = None;
    s.holding = None;
    s.preview = Arc::default();
    drop(s);
    crate::lakitu::set_cannon(None);
}

/// He's put elsewhere (a warp, the world re-scaled): out of it, falling.
pub fn release() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(s.phase, Phase::Rising { .. } | Phase::Inside { .. } | Phase::Flight { .. } | Phase::Pound { .. }) {
        return;
    }
    s.phase = Phase::Idle;
    s.left = None;
    let id = s.id;
    drop(s);
    crate::lakitu::set_cannon(None);
    colossus::let_go(id, Vec3::ZERO, false);
}

/// The world's origin shifted by `d` (lib.rs): the places kept in its terms with it.
pub fn shift(d: Vec3) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    match &mut s.phase {
        Phase::Rising { base, .. } | Phase::Inside { base, .. } => *base += d,
        Phase::Flight { pos, from, .. } | Phase::Pound { pos, from, .. } => {
            *pos += d;
            *from += d;
        }
        _ => {}
    }
    if let Some((base, ..)) = s.left.as_mut() {
        *base += d;
    }
    for p in s.preview.dots.iter_mut() {
        *p += d;
    }
    if let Some((p, _)) = s.preview.landing.as_mut() {
        *p += d;
    }
}

/// What lib.rs gives the cannon every tick: D-pad up, A, B, LT, the left stick (x right, y up), his
/// feet and facing (Elden Ring), SM64's action, dead, a menu up, whether it can be planted now (on
/// foot on the ground, nothing else going on) and he can stay in it, a boss's health bar, the wing
/// cap on.
pub struct In {
    pub up: bool,
    pub a: bool,
    pub b: bool,
    pub lt: bool,
    pub left: Vec2,
    pub feet: Vec3,
    pub face: Vec3,
    pub action: u32,
    pub dead: bool,
    pub paused: bool,
    /// frozen stiff, asleep, down for the partner (status.rs, revive.rs): the D-pad's not his
    pub frozen: bool,
    pub free: bool,
    pub can_stay: bool,
    pub boss: bool,
    pub wing: bool,
    pub id: i32,
    pub origin: [f32; 3],
    pub tick: u32,
}

/// What the tick did: the cannon has his buttons (nobody else gets them); D-pad up was tapped (the
/// wing cap's, caps.rs, as a press this tick).
#[derive(Default)]
pub struct Out {
    pub consumed: bool,
    pub wing_tap: bool,
}

fn sound(id: i32, volume: f32) {
    worker::call("cannon sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

fn buzz(why: &str, callout: Option<&'static str>) {
    log(format!("cannon: not here ({why})"));
    sound(SOUND_BUZZ, 0.5);
    if let Some(c) = callout {
        crate::style::callout(c, None, [1.0, 0.8, 0.3], false, 0.0);
    }
}

/// Still in ACT_ER_CLING as the cannon lets go (he can't stay): off it falling, not left hanging.
fn let_him_go(inp: &In, vel: Vec3) {
    if inp.action == colossus::ACT_ER_CLING {
        colossus::let_go(inp.id, vel, false);
    }
}

/// The way the screen's right turns the yaw (+1 or -1): the game's camera's right against the yaw's
/// turn (its coordinates are mirrored against SM64's).
fn turn_sign(yaw: f32) -> f32 {
    let Ok(cam) = (unsafe { CSCamera::instance() }) else { return 1.0 };
    let r = cam.pers_cam_1.right();
    let along = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
    if Vec3::new(r.0, r.1, r.2).dot(along) < 0.0 { -1.0 } else { 1.0 }
}

/// Mario into an SM64 action at `feet` (Elden Ring), turned to `way`, `forward` units a tick ahead
/// and `up` units a tick up; his fall counted from `peak` metres over his feet.
fn hand_back(id: i32, origin: [f32; 3], feet: Vec3, way: Vec3, action: u32, forward: f32, up: f32, peak: f32) {
    let sm = crate::collision::er_to_sm(origin, &HavokPosition(feet.x, feet.y, feet.z, 0.0));
    let yaw = (way.x.abs() + way.z.abs() > 1e-3).then(|| (-way.x).atan2(way.z));
    let peak = sm[1] + peak / crate::scale();
    worker::call("cannon hand back", move |_| unsafe {
        sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]);
        sm64::sm64_set_mario_action(id, action);
        if let Some(f) = yaw {
            sm64::sm64_set_mario_faceangle(id, f);
        }
        sm64::sm64_set_mario_forward_velocity(id, forward);
        let (fx, fz) = yaw.map_or((0.0, 0.0), |f| (f.sin() * forward, f.cos() * forward));
        sm64::sm64_set_mario_velocity(id, fx, up, fz);
        sm64::sm64_er_set_peak(id, peak);
    });
}

/// m/s to SM64's units a tick.
fn units(v: f32) -> f32 {
    v / (crate::scale() * 30.0)
}

/// The barrel's mouth for this aim (and its way out, a unit vector).
fn mouth(base: Vec3, yaw: f32, pitch: f32) -> (Vec3, Vec3) {
    let out = launch(flat_of(yaw), pitch, 1.0);
    (base + Vec3::Y * PIVOT + out * MUZZLE, out)
}

/// Game thread, every SM64 tick before SM64's (before odm.rs's): the hold, the plant, the aim, the
/// shot, the flight, the pound.
pub fn update(inp: &In, combat: &mut Combat, player: &ChrIns, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Out {
    TICKS.store(inp.tick, std::sync::atomic::Ordering::Relaxed);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.id = inp.id;
    let pressed = [inp.a && !s.prev[0], inp.b && !s.prev[1], inp.lt && !s.prev[2]];
    s.prev = [inp.a, inp.b, inp.lt];
    // D-pad up: a tap is the wing cap's (on its release), held it's the cannon (frozen mid-hold:
    // neither)
    if inp.frozen {
        s.hold = Hold::default();
    }
    let press = s.hold.step(inp.up && !inp.paused && !inp.dead);
    let mut out = Out { consumed: false, wing_tap: press.tap };
    let idle = matches!(s.phase, Phase::Idle | Phase::Grace { .. });
    s.holding = press.charge.filter(|_| idle && inp.free).map(|k| (inp.feet + Vec3::Y * 1.9, k));
    if press.plant && idle {
        drop(s);
        plant(inp, ray);
        s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    }
    if inp.dead {
        if !matches!(s.phase, Phase::Idle) {
            s.phase = Phase::Idle;
            drop(s);
            crate::lakitu::set_cannon(None);
        }
        return out;
    }
    let tick = inp.tick;
    let id = inp.id;
    let phase = s.phase;
    match phase {
        Phase::Idle => {}
        Phase::Grace { until } => {
            // (a fall from the sky's height doesn't count: from GRACE_DROP over him at most)
            if inp.action & ACT_FLAG_AIR == 0 || inp.action == ACT_FLYING || tick >= until {
                s.phase = Phase::Idle;
            } else {
                let sm = crate::collision::er_to_sm(inp.origin, &HavokPosition(inp.feet.x, inp.feet.y, inp.feet.z, 0.0));
                let peak = sm[1] + GRACE_DROP / crate::scale();
                worker::call("cannon grace", move |_| unsafe { sm64::sm64_er_set_peak(id, peak) });
            }
        }
        Phase::Rising { base, yaw, since } => {
            out.consumed = true;
            let t = tick.wrapping_sub(since);
            // (a blow on him as he hops in: out, the cannon gone, no coins back)
            if (t > 2 && inp.action != colossus::ACT_ER_CLING) || !inp.can_stay {
                return knocked_out(s, inp, base, yaw, PITCH_START, out);
            }
            let inside = base + Vec3::Y * (PIVOT - INSIDE_DROP);
            if t >= IN_BY {
                s.phase = Phase::Inside { base, yaw, pitch: PITCH_START, power: 0.0, charging: false, cap: None, search: Search::new(1.0), aim_of: (0, f32::NAN, f32::NAN), since: tick };
                drop(s);
                colossus::place(id, inp.origin, inside, flat_of(yaw), IN_BARREL, true);
                return out;
            }
            // his hop up and into the barrel
            let k = t as f32 / IN_BY as f32;
            let e = k * k * (3.0 - 2.0 * k);
            let at = base.lerp(inside, e).with_y(base.y + (inside.y - base.y) * e + (PIVOT + HOP) * 4.0 * k * (1.0 - k));
            drop(s);
            colossus::place(id, inp.origin, at, flat_of(yaw), LEAP, true);
        }
        Phase::Inside { base, yaw, pitch, power, charging, cap, search, aim_of, since } => {
            out.consumed = true;
            if (tick.wrapping_sub(since) > 2 && inp.action != colossus::ACT_ER_CLING) || !inp.can_stay {
                return knocked_out(s, inp, base, yaw, pitch, out);
            }
            let inside = base + Vec3::Y * (PIVOT - INSIDE_DROP);
            // (a menu: he waits in it as he is)
            if inp.paused {
                return out;
            }
            // B: out of it, the coins back, no wait
            if pressed[1] {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                s.left = Some((base, yaw, pitch, Instant::now() - std::time::Duration::from_secs_f32(STAYS)));
                s.preview = Arc::default();
                drop(s);
                crate::coins::refund(COST, "the cannon");
                crate::lakitu::set_cannon(None);
                log("cannon: got out");
                colossus::place(id, inp.origin, base + Vec3::Y * (PIVOT + 0.3), -flat_of(yaw), LEAP, true);
                colossus::let_go(id, -flat_of(yaw) * 2.5 + Vec3::Y * HOP_OUT, false);
                return out;
            }
            // the aim: the left stick SM64's, the right stick the camera's
            let (turn, lift) = sticks(inp.left, s.right);
            // (the rays below without the lock: the HUD reads it from the render thread)
            drop(s);
            let (yaw2, pitch2) = aim(yaw, pitch, turn * turn_sign(yaw), lift, TICK);
            let turning = (yaw2 - yaw).abs() + (pitch2 - pitch).abs() > 1e-4;
            if turning {
                sound(SOUND_AIM, 0.3);
            }
            let (yaw, pitch) = (yaw2, pitch2);
            let (muzzle, way) = mouth(base, yaw, pitch);
            let lands = |yaw: f32, pitch: f32, p: f32| arc(mouth(base, yaw, pitch).0, launch(flat_of(yaw), pitch, launch_speed(p)), ray).landing.is_some();
            // the governor's cap for the HUD and the preview: a new search once the aim settles
            // somewhere new, and now and then (turning, the world streaming in); one flight's rays a
            // tick, the last cap kept meanwhile
            let (mut cap, mut search, mut aim_of) = (cap, search, aim_of);
            let moved = (aim_of.1 - yaw).abs() + (aim_of.2 - pitch).abs();
            if (!(moved <= CAP_MOVED) && !turning) || tick.wrapping_sub(aim_of.0) >= CAP_EVERY {
                search = Search::new(1.0);
                aim_of = (tick, yaw, pitch);
            }
            if let Some(p) = search.next() {
                search.step(lands(aim_of.1, aim_of.2, p));
                if let Some(found) = search.found {
                    cap = found;
                }
            }
            let most = cap.unwrap_or(0.0);
            // A: a press starts the power filling, let go it fires (the bar fills past the cap's
            // mark: the shot is checked for this very aim as it goes)
            let charging = charging || pressed[0];
            let power = if charging { power_step(power, inp.a, TICK) } else { power };
            if charging && !inp.a {
                let wanted = power;
                let power = if lands(yaw, pitch, wanted) { Some(wanted) } else { cap_power(muzzle, flat_of(yaw), pitch, wanted, ray) };
                let Some(power) = power else {
                    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
                    s.phase = Phase::Inside { base, yaw, pitch, power: 0.0, charging: false, cap: None, search, aim_of, since };
                    drop(s);
                    buzz("no landing", Some("NO LANDING"));
                    return out;
                };
                let vel = launch(flat_of(yaw), pitch, launch_speed(power));
                let flight = arc(muzzle, vel, ray);
                let expect = flight.landing.map_or(ARC_MOST, |l| l.1);
                let metres = flight.landing.map_or(0.0, |l| (l.0 - base).with_y(0.0).length());
                let feet = muzzle - Vec3::Y * BODY;
                let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
                s.phase = Phase::Flight { pos: feet, vel, since: tick, expect, from: base };
                s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
                s.left = Some((base, yaw, pitch, Instant::now()));
                s.preview = Arc::default();
                drop(s);
                log(format!("cannon: fired at {:.0}% ({:.1} m/s, {:.0} deg), landing {metres:.0} m off", power * 100.0, launch_speed(power), pitch.to_degrees()));
                if power < wanted {
                    log(format!("cannon: capped to {:.0}% of {:.0}% (the far landing isn't loaded)", power * 100.0, wanted * 100.0));
                }
                sound(SOUND_BOOM, 0.6);
                sound(SOUND_WHOOSH, 0.4);
                sound(SOUND_YAHOO, 0.6);
                crate::impact::cannon_fire(muzzle, way);
                crate::lakitu::set_cannon(Some(crate::lakitu::CannonView::Flight { vel }));
                colossus::place(id, inp.origin, feet, vel.with_y(0.0), SHOT, true);
                let tilt = vel.y.atan2(vel.with_y(0.0).length());
                worker::call("cannon tilt", move |_| unsafe { sm64::sm64_er_cling_pitch(tilt) });
                return out;
            }
            // the flight ahead, now and then (at the power it'd fire at)
            let preview = (tick.wrapping_sub(since) % PREVIEW_EVERY == 0).then(|| {
                let mut a = arc(muzzle, launch(flat_of(yaw), pitch, launch_speed(power.min(most))), ray);
                if cap.is_none() {
                    a.landing = None;
                }
                a
            });
            let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(a) = preview {
                s.preview = a;
            }
            s.phase = Phase::Inside { base, yaw, pitch, power, charging, cap, search, aim_of, since };
            drop(s);
            crate::lakitu::set_cannon(Some(crate::lakitu::CannonView::Aim { pivot: base + Vec3::Y * PIVOT, dir: way }));
            colossus::place(id, inp.origin, inside, flat_of(yaw), IN_BARREL, true);
        }
        Phase::Flight { pos, vel, since, expect, from } => {
            out.consumed = true;
            let t = tick.wrapping_sub(since);
            // (a blow on him in the air, or he can't go on: SM64 has him)
            if (t > 2 && inp.action != colossus::ACT_ER_CLING) || !inp.can_stay {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                crate::lakitu::set_cannon(None);
                log("cannon: knocked out of the flight");
                let_him_go(inp, vel * 0.5);
                return out;
            }
            // the wing cap on, and falling: SM64's flight on from here (its cannon's)
            if inp.wing && vel.y < 0.0 {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                crate::lakitu::set_cannon(None);
                log("cannon: wing cap, flying on");
                let flat = vel.with_y(0.0);
                hand_back(id, inp.origin, pos, flat, ACT_FLYING, units(vel.length()).min(100.0), 0.0, GRACE_DROP);
                return out;
            }
            // LT: the pound
            if pressed[2] {
                s.phase = Phase::Pound { pos, since: tick, from };
                drop(s);
                crate::lakitu::set_cannon(None);
                log(format!("cannon: pound from {:.0} m up", (pos.y - ray(pos, pos - Vec3::Y * 300.0).map_or(pos.y, |g| g.y)).max(0.0)));
                colossus::place(id, inp.origin, pos, vel.with_y(0.0), POUND, true);
                return out;
            }
            let (next, nv) = fly(pos, vel, TICK);
            if let Some(h) = ray(pos + Vec3::Y * BODY, next + Vec3::Y * BODY) {
                let flat = vel.with_y(0.0).normalize_or(Vec3::Z);
                // a floor (the ground right under where it hit) or a wall
                // (only coming down: up into a thin slab, its top is right over the hit)
                let floor = (vel.y <= 0.0).then(|| ray(h + Vec3::Y * 0.8 - flat * 0.3, h - Vec3::Y * 0.6)).flatten().filter(|g| (g.y - h.y).abs() < 0.7);
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                crate::lakitu::set_cannon(None);
                match floor {
                    Some(g) => {
                        log(format!("cannon: landed {:.0} m off after {:.1} s", (g - from).with_y(0.0).length(), t as f32 * TICK));
                        crate::impact::thud_small(g);
                        crate::lakitu::shake(0.4);
                        hand_back(id, inp.origin, g + Vec3::Y * 0.05, flat, ACT_DIVE_SLIDE, units(vel.with_y(0.0).length()).min(SLIDE_MOST), 0.0, 0.0);
                    }
                    None => {
                        log("cannon: hit a wall");
                        crate::impact::cannon_wall(h);
                        crate::lakitu::shake(0.5);
                        let back = h - flat * 0.6 - Vec3::Y * BODY;
                        hand_back(id, inp.origin, back, flat, ACT_BACKWARD_AIR_KB, WALL_KB, units(vel.y.min(0.0)), GRACE_DROP);
                    }
                }
                return out;
            }
            // (gone on past any landing: nothing's there to land on, he falls)
            if t as f32 * TICK > expect + FLIGHT_SPARE {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                crate::lakitu::set_cannon(None);
                log("cannon: no landing on the way, let fall");
                hand_back(id, inp.origin, next, nv.with_y(0.0), ACT_FREEFALL, units(nv.with_y(0.0).length()).min(SLIDE_MOST), units(nv.y), GRACE_DROP);
                return out;
            }
            s.phase = Phase::Flight { pos: next, vel: nv, since, expect, from };
            drop(s);
            crate::lakitu::set_cannon(Some(crate::lakitu::CannonView::Flight { vel: nv }));
            if nv.y > 0.0 && (t == 1 || t % 2 == 0) {
                crate::impact::dash_point(next, t == 1);
            }
            colossus::place(id, inp.origin, next, nv.with_y(0.0), SHOT, true);
            let tilt = nv.y.atan2(nv.with_y(0.0).length());
            worker::call("cannon tilt", move |_| unsafe { sm64::sm64_er_cling_pitch(tilt) });
        }
        Phase::Pound { pos, since, from } => {
            out.consumed = true;
            let t = tick.wrapping_sub(since);
            if (t > 2 && inp.action != colossus::ACT_ER_CLING) || !inp.can_stay {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                let_him_go(inp, Vec3::ZERO);
                return out;
            }
            // the spin, held up there
            if t < POUND_SPIN {
                drop(s);
                colossus::place(id, inp.origin, pos, Vec3::ZERO, POUND, true);
                return out;
            }
            let next = pos - Vec3::Y * POUND_SPEED * TICK;
            if let Some(g) = ray(pos + Vec3::Y * 0.3, next - Vec3::Y * 0.05) {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                hand_back(id, inp.origin, g + Vec3::Y * 0.02, Vec3::ZERO, ACT_GROUND_POUND_LAND, 0.0, 0.0, 0.0);
                let n = shockwave(g, combat, player, inp.origin, tick);
                sound(SOUND_HEAVY, 0.7);
                crate::impact::cannon_pound(g);
                log(format!("cannon: POUND on {n} ({:.0} m off the cannon)", (g - from).with_y(0.0).length()));
                return out;
            }
            if t > POUND_MOST {
                s.phase = Phase::Grace { until: tick + GRACE_TICKS };
                drop(s);
                log("cannon: the pound found no ground, let fall");
                hand_back(id, inp.origin, next, Vec3::ZERO, ACT_FREEFALL, 0.0, units(-POUND_SPEED), GRACE_DROP);
                return out;
            }
            s.phase = Phase::Pound { pos: next, since, from };
            drop(s);
            crate::impact::dash_point(next + Vec3::Y * 0.6, false);
            colossus::place(id, inp.origin, next, Vec3::ZERO, POUND, true);
        }
    }
    out
}

/// Knocked out of the cannon (a blow, he can't stay): it sinks, no coins back.
fn knocked_out(mut s: std::sync::MutexGuard<'_, State>, inp: &In, base: Vec3, yaw: f32, pitch: f32, out: Out) -> Out {
    s.phase = Phase::Grace { until: inp.tick + GRACE_TICKS };
    s.left = Some((base, yaw, pitch, Instant::now() - std::time::Duration::from_secs_f32(STAYS)));
    s.preview = Arc::default();
    drop(s);
    crate::lakitu::set_cannon(None);
    log("cannon: knocked out");
    let_him_go(inp, Vec3::ZERO);
    out
}

/// D-pad up held: plant it here, if it can be.
fn plant(inp: &In, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    if !inp.free {
        return buzz("busy, or off the ground", None);
    }
    if inp.boss {
        return buzz("a boss fight", Some("NOT NOW"));
    }
    let wait = STATE.lock().unwrap_or_else(|e| e.into_inner()).ready_at.map_or(0.0, |t| t.saturating_duration_since(Instant::now()).as_secs_f32());
    if wait > 0.0 {
        return buzz(&format!("ready in {wait:.0} s"), Some("WAIT"));
    }
    if !open_sky(inp.feet + Vec3::Y * 1.6, ray) {
        return buzz("no open sky", Some("NO SKY"));
    }
    if !crate::coins::pay(COST, "the cannon") {
        log("cannon: not here (coins)");
        sound(SOUND_BUZZ, 0.5);
        return;
    }
    let face = inp.face.with_y(0.0).normalize_or(Vec3::Z);
    let yaw = face.z.atan2(face.x);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.phase = Phase::Rising { base: inp.feet, yaw, since: inp.tick };
    s.left = None;
    drop(s);
    sound(SOUND_UP, 0.5);
    crate::impact::cannon_up(inp.feet);
    log(format!("cannon: planted at {:.1?} (yaw {:.0} deg)", inp.feet, yaw.to_degrees()));
}

/// The pound's shockwave at `at`: every enemy body near. How many it hit.
fn shockwave(at: Vec3, combat: &mut Combat, player: &ChrIns, origin: [f32; 3], tick: u32) -> usize {
    // (by its body, about his height or under him: a giant's centre far off when its leg is right
    // there, not one on a floor above)
    const BODY_MOST: f32 = 15.0;
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), POUND_REACH + BODY_MOST, origin);
    let hits: Vec<(usize, Attack, bool)> = near
        .iter()
        .enumerate()
        .filter(|(_, t)| t.shootable().is_some() && !t.friendly() && !t.is_prop())
        .filter(|(_, t)| {
            let f = t.feet();
            let gap = ((f - at).with_y(0.0).length() - t.radius * crate::scale()).max(0.0);
            gap <= POUND_REACH && (-POUND_BELOW..=POUND_ABOVE).contains(&(f.y - at.y))
        })
        .map(|(i, _)| (i, Attack::CannonPound, false))
        .collect();
    combat.deal(player, &near, &hits, tick);
    hits.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    /// flat ground at y = 0
    fn ground(a: Vec3, b: Vec3) -> Option<Vec3> {
        (a.y >= 0.0 && b.y < 0.0).then(|| a + (b - a) * (a.y / (a.y - b.y)))
    }

    /// ground at y = 0 only where x < `edge` (the far part not loaded)
    fn ground_to(edge: f32) -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        move |a, b| ground(a, b).filter(|h| h.x < edge)
    }

    fn range(power: f32, pitch: f32) -> f32 {
        let (mut p, mut v) = (Vec3::ZERO, launch(Vec3::X, pitch, launch_speed(power)));
        for _ in 0..3000 {
            let (np, nv) = fly(p, v, DT);
            if np.y < 0.0 {
                return np.x;
            }
            (p, v) = (np, nv);
        }
        f32::INFINITY
    }

    #[test]
    fn a_tap_is_the_wing_cap_as_it_is_let_go() {
        let mut h = Hold::default();
        assert_eq!(h.step(true), Press::default());
        for _ in 0..5 {
            assert!(!h.step(true).tap);
        }
        let up = h.step(false);
        assert!(up.tap && !up.plant);
        // (nothing while it's up)
        assert_eq!(h.step(false), Press::default());
        // 14 ticks is still a tap
        for _ in 0..HOLD_TICKS - 1 {
            assert!(!h.step(true).plant);
        }
        assert!(h.step(false).tap);
    }

    #[test]
    fn held_half_a_second_plants_once() {
        let mut h = Hold::default();
        let mut plants = 0;
        for t in 1..=40 {
            let p = h.step(true);
            plants += p.plant as u32;
            if p.plant {
                assert_eq!(t, HOLD_TICKS);
            }
            // (its ring from the 4th tick until it plants)
            assert_eq!(p.charge.is_some(), (HOLD_SHOWS..HOLD_TICKS).contains(&t), "tick {t}");
        }
        assert_eq!(plants, 1);
        // (and it was no tap)
        assert!(!h.step(false).tap);
        let p = h.step(true);
        assert!(!p.tap && !p.plant);
    }

    #[test]
    fn a_hundred_to_three_hundred_metres() {
        let low = range(0.0, std::f32::consts::FRAC_PI_4);
        let high = range(1.0, std::f32::consts::FRAC_PI_4);
        assert!((90.0..=110.0).contains(&low), "{low}");
        assert!((280.0..=320.0).contains(&high), "{high}");
        assert!(range(0.5, std::f32::consts::FRAC_PI_4) > low && range(0.5, std::f32::consts::FRAC_PI_4) < high);
        assert_eq!(launch_speed(-1.0), SPEED.0);
        assert_eq!(launch_speed(2.0), SPEED.1);
    }

    #[test]
    fn sm64s_cannon_flight() {
        let (mut p, mut v) = (Vec3::ZERO, launch(Vec3::Z, 0.3, SPEED.1));
        assert!((v.length() - SPEED.1).abs() < 1e-3 && v.y > 0.0 && v.x.abs() < 1e-4);
        let mut fastest_fall = 0.0f32;
        for _ in 0..30 * 120 {
            (p, v) = fly(p, v, DT);
            fastest_fall = fastest_fall.min(v.y);
            assert!(v.with_y(0.0).length() >= SLOWEST - 1e-3);
        }
        assert!((fastest_fall + TERMINAL).abs() < 1e-3, "{fastest_fall}");
        // (its way kept)
        assert!(v.x.abs() < 1e-3 && v.z > 0.0 && p.z > 0.0);
        // (a flat dir taken flat)
        let up = launch(Vec3::new(0.0, 5.0, 1.0), 0.0, 10.0);
        assert!((up - Vec3::Z * 10.0).length() < 1e-4);
    }

    #[test]
    fn the_arc_lands_where_it_crosses_the_ground() {
        let from = Vec3::new(0.0, 1.0, 0.0);
        let v = launch(Vec3::X, 0.7, launch_speed(0.0));
        let a = arc(from, v, &ground);
        let (at, t) = a.landing.expect("lands");
        assert!(at.y.abs() < 1e-3);
        let r = range(0.0, 0.7);
        assert!((at.x - r).abs() < 3.0, "{} vs {r}", at.x);
        assert!(t > 3.0 && t < 6.0, "{t}");
        // (its dots: up and over, none past the landing)
        assert!(a.dots.len() > 10);
        assert!(a.dots.iter().all(|d| d.y >= 0.0 && d.x <= at.x + 1e-3));
        assert!(a.dots.windows(2).all(|w| w[1].x > w[0].x));
        // over nothing: no landing
        assert!(arc(from, v, &|_, _| None).landing.is_none());
    }

    #[test]
    fn the_governor_caps_a_shot_over_the_unloaded() {
        let from = Vec3::new(0.0, 1.0, 0.0);
        let pitch = std::f32::consts::FRAC_PI_4;
        // all there: as wanted
        assert_eq!(cap_power(from, Vec3::X, pitch, 0.8, &ground), Some(0.8));
        // only the first 200 m loaded: under what reaches past it, more than the least
        let p = cap_power(from, Vec3::X, pitch, 1.0, &ground_to(200.0)).expect("some");
        assert!(p > 0.1 && p < 1.0, "{p}");
        let landed = arc(from, launch(Vec3::X, pitch, launch_speed(p)), &ground_to(200.0)).landing.unwrap().0;
        assert!(landed.x > 150.0 && landed.x < 200.0, "{landed}");
        // nothing loaded at all: no shot
        assert_eq!(cap_power(from, Vec3::X, pitch, 1.0, &ground_to(50.0)), None);
    }

    #[test]
    fn the_search_spreads_the_governor() {
        let from = Vec3::new(0.0, 1.0, 0.0);
        let pitch = std::f32::consts::FRAC_PI_4;
        let lands = |p: f32, ray: &dyn Fn(Vec3, Vec3) -> Option<Vec3>| arc(from, launch(Vec3::X, pitch, launch_speed(p)), &ray).landing.is_some();
        for (edge, wanted) in [(1e9, 0.8), (200.0, 1.0), (50.0, 1.0)] {
            let ray = ground_to(edge);
            let mut s = Search::new(wanted);
            let mut steps = 0;
            while let Some(p) = s.next() {
                s.step(lands(p, &ray));
                steps += 1;
                assert!(steps <= 2 + HALVINGS, "edge {edge}");
            }
            assert_eq!(s.found, Some(cap_power(from, Vec3::X, pitch, wanted, &ray)), "edge {edge}");
            // (what lands as wanted takes one flight)
            if edge > 1e8 {
                assert_eq!(steps, 1);
            }
        }
    }

    #[test]
    fn open_sky_and_a_roof() {
        let head = Vec3::new(0.0, 1.5, 0.0);
        assert!(open_sky(head, &|_, _| None));
        // a roof 4 m up
        let roof = |a: Vec3, b: Vec3| (b.y > 5.5).then(|| a + (b - a) * ((5.5 - a.y) / (b.y - a.y)));
        assert!(!open_sky(head, &roof));
        // a tall wall on one side (x > 3): still the open sky
        let wall = |a: Vec3, b: Vec3| (b.x > 3.0).then(|| a + (b - a) * ((3.0 - a.x) / (b.x - a.x)));
        assert!(open_sky(head, &wall));
        // an overhang over him (y > 10 and x > -2): the sky's mostly gone
        let over = |_: Vec3, b: Vec3| (b.y > 10.0 && b.x > -2.0).then_some(b);
        assert!(!open_sky(head, &over));
    }

    #[test]
    fn the_sticks_aim_it() {
        // the left stick's up aims lower (SM64's), the right stick's higher (the camera's)
        assert!(sticks(Vec2::new(0.0, 1.0), Vec2::ZERO).1 < 0.0);
        assert!(sticks(Vec2::ZERO, Vec2::new(0.0, 1.0)).1 > 0.0);
        assert!(sticks(Vec2::new(1.0, 0.0), Vec2::ZERO).0 > 0.0 && sticks(Vec2::ZERO, Vec2::new(1.0, 0.0)).0 > 0.0);
        // (both at once: no more than one full push)
        let (t, l) = sticks(Vec2::new(1.0, -1.0), Vec2::new(1.0, 1.0));
        assert!(t <= 1.0 && l <= 1.0);
        // the turn and the lift, clamped
        let (y, p) = aim(0.0, PITCH_START, 1.0, 1.0, 1.0);
        assert!((y - YAW_RATE).abs() < 1e-4 && (p - (PITCH_START + PITCH_RATE)).min(PITCH_MAX) - p < 1e-4);
        assert!(p <= PITCH_MAX);
        let (_, p) = aim(0.0, PITCH_START, 0.0, -1.0, 5.0);
        assert_eq!(p, PITCH_MIN);
        let (_, p) = aim(0.0, PITCH_START, 0.0, 1.0, 5.0);
        assert_eq!(p, PITCH_MAX);
    }

    #[test]
    fn the_power_fills_in_a_second() {
        let mut p = 0.0;
        for _ in 0..15 {
            p = power_step(p, true, DT);
        }
        assert!((p - 0.5).abs() < 0.02, "{p}");
        assert_eq!(power_step(p, false, DT), p);
        for _ in 0..30 {
            p = power_step(p, true, DT);
        }
        assert_eq!(p, 1.0);
    }
}
