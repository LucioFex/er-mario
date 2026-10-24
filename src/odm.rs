//! The Attack on Titan hook (v4 Part 5), the omni-directional gear Mario's way: R2 on a big enemy
//! anchors the cap to the bone nearest the screen's centre (its neck, head, an arm, a leg, its tail:
//! colossus.rs's canonical bones), a ring on it, the camera framing Mario and the bone. On the line:
//! - the left stick held swings him round the anchor a moment (at most 90°/s, 3 s a line)
//! - let go, the line reels him in, faster and faster (12 → 30 m/s); B is a burst of gas (+40%; with
//!   the stick to a side, a sidestep round the anchor that way with a hiss; three charges, all back
//!   on a hit or on landing)
//! - X in the last quarter second before he gets there is the strike: harder the faster he comes
//!   (× speed / 20 m/s, 0.3 to 1.8), half again on the nape or the head from behind (NAPE!); no X,
//!   a glancing hit at half. The right stick flicked moves the anchor to the next bone that way.
//! - after any hit he rebounds up and back; A (or R2) re-anchors
//! - on a huge one (a colossus), A as he gets there grabs on instead (colossus.rs climbs it)
//! A medium enemy has no line: the hook's tap is a zip-kick, held a vault over it (hook.rs).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{CSCamExt, CSCamera, ChrIns, FieldInsHandle};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::{Vec2, Vec3};

use crate::colossus::{self, Input, Mario, Part, Titan};
use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// the reel: from, to (m/s), how fast it gets there (m/s²)
pub const REEL_FROM: f32 = 12.0;
pub const REEL_TO: f32 = 30.0;
const REEL_RAMP: f32 = 30.0;
/// a burst of gas: its push, how long it fades over (s), charges
const GAS_BOOST: f32 = 1.4;
const GAS_FADE: f32 = 0.8;
pub const GAS_CHARGES: u8 = 3;
/// B with the stick across the line: a sidestep round the anchor, this fast on the whole (m/s,
/// quickest at once and dying away) for this long (s); the stick that far across the line at least
const DASH_SPEED: f32 = 26.0;
const DASH_TIME: f32 = 0.4;
const DASH_ACROSS: f32 = 0.35;
/// the orbit: its fastest turn (rad/s, full stick), how long a line may orbit (s)
const ORBIT_RATE: f32 = std::f32::consts::FRAC_PI_2;
const ORBIT_MAX: f32 = 3.0;
/// the strike's window: the last this long before contact (s)
pub const STRIKE_WINDOW: f32 = 0.25;
/// the speed the strike is worth ×1 at (m/s), its least and most
const SPEED_UNIT: f32 = 20.0;
const FACTOR_MIN: f32 = 0.3;
const FACTOR_MAX: f32 = 1.8;
/// the nape (or head) from behind; a boss's most, all together
const NAPE: f32 = 1.5;
const BOSS_CAP: f32 = 1.5;
/// from behind: coming along its facing at least this much (cos)
const BEHIND_COS: f32 = 0.3;
/// the speed tiers' edges (m/s): below the first a "tink"
const TIERS: [f32; 3] = [12.0, 20.0, 28.0];
/// the line bends his speed into its way this much a tick (a swoop out of an orbit, not a corner)
const BEND: f32 = 0.35;
/// the rebound: back, up (m/s); its arc driven this long (ticks: SM64's air drag takes a backward
/// speed down to 16 units a tick at once, a backflip of SM64's went half as far), then a fall on
/// with what's left of it, facing the way he goes; SM64's gravity (m/s²: 4 units a tick²)
const REBOUND_BACK: f32 = 10.5;
const REBOUND_UP: f32 = 12.0;
pub const REBOUND_ARC: u32 = 16;
const GRAVITY: f32 = 36.0;
/// the stick orbits from this far out, and on down to this once it does (no flicker between the
/// two poses at the edge)
const ORBIT_FROM: f32 = 0.25;
const ORBIT_TO: f32 = 0.15;
/// a bone on screen: within this of the view's middle (cos), a flick's way within this (cos)
const SCREEN_COS: f32 = 0.6;
const FLICK_COS: f32 = 0.5;
/// the scenery rope's cone: its rays' turns either way and pitches (degrees), the pitch it likes
/// best, its nearest and farthest (m), how high above his feet at the least (m)
const CONE_YAWS: [f32; 5] = [0.0, -20.0, 20.0, -40.0, 40.0];
const CONE_PITCHES: [f32; 3] = [35.0, 50.0, 65.0];
const CONE_BEST: f32 = 50.0;
const CONE_NEAR: f32 = 6.0;
const CONE_FAR: f32 = 25.0;
const CONE_ABOVE: f32 = 4.0;

/// The bone nearest the middle of the screen (the camera at `eye` looking along `fwd`), in reach of
/// `from` (his hand); none behind the camera or off to the side.
pub fn centre_pick(bones: &[(u32, Vec3)], eye: Vec3, fwd: Vec3, from: Vec3, reach: f32) -> Option<u32> {
    let fwd = fwd.normalize_or(Vec3::Z);
    bones
        .iter()
        .filter_map(|&(id, p)| {
            let v = p - eye;
            let cos = v.normalize_or_zero().dot(fwd);
            if cos < SCREEN_COS || (p - from).length() > reach {
                return None;
            }
            // (the nearer of two about as central)
            Some((1.0 - cos + 0.001 * v.length(), id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Where `p` is on the screen (the view's slopes across and up).
fn on_screen(p: Vec3, eye: Vec3, fwd: Vec3, right: Vec3, up: Vec3) -> Option<Vec2> {
    let v = p - eye;
    let z = v.dot(fwd);
    (z > 0.1).then(|| Vec2::new(v.dot(right) / z, v.dot(up) / z))
}

/// The right stick flicked `dir` (x right, y up): the bone that way on screen from `current`, the
/// nearest; none that way.
pub fn flick_pick(bones: &[(u32, Vec3)], current: u32, eye: Vec3, fwd: Vec3, right: Vec3, up: Vec3, dir: Vec2) -> Option<u32> {
    let dir = dir.try_normalize()?;
    let c = bones.iter().find(|(id, _)| *id == current).and_then(|&(_, p)| on_screen(p, eye, fwd, right, up))?;
    bones
        .iter()
        .filter(|(id, _)| *id != current)
        .filter_map(|&(id, p)| {
            let d = on_screen(p, eye, fwd, right, up)? - c;
            let len = d.length();
            let cos = if len > 0.01 { d.dot(dir) / len } else { -1.0 };
            // (the nearest that way, the straighter the better)
            (cos >= FLICK_COS).then(|| (len * (2.0 - cos), id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// What the strike is worth for his speed (m/s).
pub fn speed_factor(speed: f32) -> f32 {
    (speed / SPEED_UNIT).clamp(FACTOR_MIN, FACTOR_MAX)
}

/// The speed's tier (0 a "tink" .. 3), for the line's colour and the blow's sound.
pub fn tier(speed: f32) -> u8 {
    TIERS.iter().filter(|&&t| speed >= t).count() as u8
}

/// All the strike's worth: its speed, the nape from behind, a boss's cap.
pub fn multiplier(speed: f32, nape: bool, boss: bool) -> f32 {
    let f = speed_factor(speed) * if nape { NAPE } else { 1.0 };
    if boss { f.min(BOSS_CAP) } else { f }
}

/// He comes at it from behind: his way (`approach`) along its facing.
pub fn from_behind(approach: Vec3, enemy_fwd: Vec3) -> bool {
    approach.with_y(0.0).normalize_or_zero().dot(enemy_fwd.with_y(0.0).normalize_or_zero()) >= BEHIND_COS
}

/// Mario on the line (Elden Ring, m and m/s): where, his speed, the reel's speed now, how long the
/// line orbited, the gas burst left (s), whether he orbits now.
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub pos: Vec3,
    pub vel: Vec3,
    pub reel: f32,
    pub orbit_used: f32,
    pub gas: f32,
    pub orbiting: bool,
    /// a sidestep's turning sense (the axis it goes round the anchor) and how long it has left (s)
    dash: Option<(Vec3, f32)>,
}

/// A step's outcome: whether he orbited, how long until contact at this speed (s).
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub orbiting: bool,
    pub to_contact: f32,
}

impl Line {
    pub fn new(pos: Vec3, vel: Vec3) -> Line {
        Line { pos, vel, reel: REEL_FROM, orbit_used: 0.0, gas: 0.0, orbiting: false, dash: None }
    }

    pub fn speed(&self) -> f32 {
        self.vel.length()
    }

    /// B: a burst of gas.
    pub fn burst(&mut self) {
        self.gas = GAS_FADE;
    }

    /// B with the stick held `way` (level), the line to `anchor`: across the line, a sidestep round
    /// the anchor that way; along it (or no stick), the burst. Whether it was a sidestep.
    pub fn dash(&mut self, way: Vec3, anchor: Vec3) -> bool {
        let r = (self.pos - anchor).normalize_or(Vec3::Z);
        let across = way - r * way.dot(r);
        if across.length() < DASH_ACROSS {
            self.burst();
            return false;
        }
        self.dash = Some((r.cross(across.normalize()).normalize_or(Vec3::Y), DASH_TIME));
        true
    }

    /// In a sidestep now.
    #[cfg(test)]
    pub fn dashing(&self) -> bool {
        self.dash.is_some()
    }

    /// One step (`dt` s) towards `anchor` (contact `contact` m off it), the stick (level, 0-1) held
    /// for the orbit.
    pub fn step(&mut self, anchor: Vec3, stick: Vec3, contact: f32, dt: f32) -> Step {
        let rel = self.pos - anchor;
        let dist = rel.length().max(1e-3);
        let r = rel / dist;
        // a sidestep: round the anchor its way, quickest at once, the line's length kept
        if let Some((axis, left)) = self.dash {
            // (round its axis: on and on the same way, past a quarter turn too)
            let across = axis.cross(r);
            let speed = DASH_SPEED * 2.0 * (left / DASH_TIME);
            let a = (speed * dt / dist).min(0.5);
            let next = anchor + (r * a.cos() + across.normalize_or_zero() * a.sin()) * dist;
            self.vel = (next - self.pos) / dt;
            self.pos = next;
            let left = left - dt;
            self.dash = (left > 1e-4).then_some((axis, left));
            self.orbiting = false;
            // (going round, not in: no strike window)
            return Step { orbiting: false, to_contact: f32::MAX };
        }
        // the stick across the line: round the anchor that way, the line's length kept (along the
        // line it's no orbit: the reel)
        let across = stick - r * stick.dot(r);
        let from = if self.orbiting { ORBIT_TO } else { ORBIT_FROM };
        let orbiting = stick.length() > from && across.length() > 0.1 && self.orbit_used < ORBIT_MAX - dt * 0.5;
        self.orbiting = orbiting;
        if orbiting {
            self.orbit_used += dt;
            let a = ORBIT_RATE * stick.length().min(1.0) * dt;
            let next = anchor + (r * a.cos() + across.normalize() * a.sin()) * dist;
            self.vel = (next - self.pos) / dt;
            self.pos = next;
        } else {
            // (the gas pushes the reel: a burst in an orbit waits for it)
            let boost = 1.0 + (GAS_BOOST - 1.0) * (self.gas / GAS_FADE).clamp(0.0, 1.0);
            self.gas = (self.gas - dt).max(0.0);
            self.reel = (self.reel + REEL_RAMP * dt).min(REEL_TO);
            let speed = self.reel * boost;
            let way = -r;
            let k = 1.0 - (1.0 - BEND).powf(dt * 30.0);
            let v = self.vel.lerp(way * speed, k).normalize_or(way) * speed;
            // (never past the contact: stopped on it)
            let next = self.pos + v * dt;
            self.pos = if (next - anchor).length() < contact { anchor + (next - anchor).normalize_or(r) * contact } else { next };
            self.vel = v;
        }
        // (stopped on the contact: there, not a rounding's breadth off it)
        let left = (self.pos - anchor).length() - contact;
        let left = if left < 1e-3 { 0.0 } else { left };
        Step { orbiting, to_contact: left / self.speed().max(1.0) }
    }
}

/// The rebound off a hit (m/s): away from it (`away`, level), and up.
pub fn rebound(away: Vec3) -> Vec3 {
    away.with_y(0.0).normalize_or(Vec3::Z) * REBOUND_BACK + Vec3::Y * REBOUND_UP
}

/// A step (`dt` s) of the rebound's arc: where he is, his speed, after it.
pub fn arc_step(at: Vec3, vel: Vec3, dt: f32) -> (Vec3, Vec3) {
    let v = vel - Vec3::Y * GRAVITY * dt;
    (at + (vel + v) * 0.5 * dt, v)
}

/// The scenery rope's rays: ahead (`fwd`, level) and up, in a cone.
pub fn cone_dirs(fwd: Vec3) -> Vec<Vec3> {
    let f = fwd.with_y(0.0).normalize_or(Vec3::Z);
    CONE_PITCHES
        .iter()
        .flat_map(|&pitch| {
            CONE_YAWS.iter().map(move |&yaw| {
                let h = glam::Quat::from_rotation_y(yaw.to_radians()).mul_vec3(f);
                h * pitch.to_radians().cos() + Vec3::Y * pitch.to_radians().sin()
            })
        })
        .collect()
}

/// The best of the cone's hits (each ray's way and what it hit, from `from`; his feet `feet`): the
/// one nearest CONE_BEST up, ahead, not too near or far, high enough.
pub fn cone_pick(hits: &[(Vec3, Option<Vec3>)], from: Vec3, feet: Vec3, fwd: Vec3) -> Option<Vec3> {
    let f = fwd.with_y(0.0).normalize_or(Vec3::Z);
    hits.iter()
        .filter_map(|&(dir, hit)| {
            let p = hit?;
            let d = (p - from).length();
            if !(CONE_NEAR..=CONE_FAR).contains(&d) || p.y < feet.y + CONE_ABOVE {
                return None;
            }
            let up = dir.y.clamp(-1.0, 1.0).asin().to_degrees();
            let ahead = dir.with_y(0.0).normalize_or_zero().dot(f);
            Some(((up - CONE_BEST).abs() / 15.0 + (1.0 - ahead) * 2.0 + (d - 14.0).abs() * 0.03, p))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

// ---- in the game ------------------------------------------------------------------------------

const DT: f32 = 1.0 / 30.0;
const ACT_FLAG_AIR: u32 = 0x800;
/// a line lasts at most this long (ticks); through the scenery this many ticks running, it drops;
/// a blow on him in the first ticks is the latch's own (SM64 still turning to it)
const LINE_MOST: u32 = 180;
const CUT_TICKS: u32 = 4;
const SETTLE: u32 = 6;
/// an X (or A) this long before the window still counts (s)
const PRESS_GRACE: f32 = 4.0 * DT;
/// after a hit, A re-anchors for this long (ticks)
const REBOUND_TICKS: u32 = 36;
/// his chest above his feet on the line (m)
const CHEST: f32 = 0.9;
/// contact this far off a bone (m); a central one by its body (share of its radius: the back, the
/// hips; the neck and head)
const CONTACT: f32 = 1.2;
const CONTACT_CORE: f32 = 0.8;
const CONTACT_NECK: f32 = 0.4;
/// the anchor follows its bone this much a tick (an animation's jolts eased)
const ANCHOR_EASE: f32 = 0.35;
/// the NAPE!'s stance
const NAPE_STANCE: f32 = 40.0;
/// speed lines past this (m/s)
const LINES_FROM: f32 = 18.0;
/// the vault: over its top by this (m), landing this far behind its body (m), its speed (m/s) and
/// shortest/longest (ticks); the blows on it from behind after it: worth this, this long (s)
const VAULT_OVER: f32 = 1.2;
const VAULT_BEHIND: f32 = 1.6;
const VAULT_SPEED: f32 = 14.0;
const VAULT_TICKS: (f32, f32) = (14.0, 30.0);
pub const BACKSTAB: f32 = 1.3;
const BACKSTAB_TIME: f32 = 1.0;
/// the right stick: a flick goes out past this and starts from under the other (0-1)
const FLICK_OUT: f32 = 0.7;
const FLICK_REST: f32 = 0.35;
/// ACT_ER_CLING's poses (libsm64's er_cling_mode): the leap's flip, the dive along the line, the
/// swing round it, the spin slash, the flying kick
const LEAP: i32 = 2;
const DIVE: i32 = 5;
const SWING: i32 = 6;
const SPIN: i32 = 8;
const KICK: i32 = 9;
/// (the rebound's backflip)
const FLIP: i32 = 10;

const SOUND_TINK: i32 = 0x0442_8081;
const SOUND_HEAVY: i32 = 0x312F_0081;
const SOUND_GAS: i32 = 0x305C_0081;
const SOUND_LATCH: i32 = 0x0436_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    /// on the line to bone `bone` (its part) of `mob`: `at` where it's anchored now (eased after the
    /// bone), since this tick; X came in the window (the strike), A did (a colossus's grab); ticks
    /// the line has run through the scenery
    Line { mob: FieldInsHandle, bone: u32, part: Part, line: Line, at: Vec3, since: u32, struck: bool, grab: bool, cut: u32 },
    /// off a hit: his feet, speed, the way he faces (at it), the pose the blow held (its hit-stop,
    /// until `since`), the arc from `since`, A re-anchoring until `until`
    Rebound { at: Vec3, vel: Vec3, face: Vec3, pose: i32, since: u32, until: u32 },
    /// over a medium enemy (`mob`) from `from` to `to`, its top `top` high, since this tick, for `dur`
    Vault { mob: FieldInsHandle, from: Vec3, to: Vec3, top: f32, since: u32, dur: u32 },
}

struct State {
    phase: Phase,
    gas: u8,
    /// buttons last tick: A, B, X, R2
    prev: [bool; 4],
    /// a flick of the right stick waiting for the line (x right, y up), and whether the stick is out
    flick: Option<Vec2>,
    stick_out: bool,
    /// Mario's libsm64 id (to let go of him from outside the tick)
    id: i32,
}

static STATE: Mutex<State> = Mutex::new(State { phase: Phase::Idle, gas: GAS_CHARGES, prev: [false; 4], flick: None, stick_out: false, id: -1 });
/// what hud.rs draws, in a lock of its own: the overlay (render thread) reads it every frame, and the
/// state's lock is held through the tick's rays and walks
static HUD: Mutex<Option<Hud>> = Mutex::new(None);
/// the enemies vaulted lately (handle key, when): blows on them from behind count for more
static BEHIND: Mutex<Vec<(u64, Instant)>> = Mutex::new(Vec::new());

/// What hud.rs draws: the ring on the bone, the line (his hand to it), his speed's tier, the
/// strike's window open, the gas left, where he is.
#[derive(Clone, Copy)]
pub struct Hud {
    pub anchor: Vec3,
    pub hand: Vec3,
    pub tier: u8,
    pub window: bool,
    pub gas: u8,
    pub mario: Vec3,
}

pub fn hud() -> Option<Hud> {
    *HUD.lock().unwrap_or_else(|e| e.into_inner())
}

/// On the line, or vaulting.
pub fn active() -> bool {
    matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Line { .. } | Phase::Vault { .. })
}

/// Where the line is anchored now (hook.rs keeps its cap there); None off it.
pub fn anchor() -> Option<Vec3> {
    match STATE.lock().unwrap_or_else(|e| e.into_inner()).phase {
        Phase::Line { at, .. } => Some(at),
        _ => None,
    }
}

/// On the line the right stick flicks the anchor from bone to bone: not the camera's then.
pub fn holds_camera() -> bool {
    matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Line { .. })
}

/// The right stick now (XInput's, every frame): pushed out from rest, a flick that way for the line.
pub fn right_stick(x: i16, y: i16) {
    let v = Vec2::new(x as f32, y as f32) / 32767.0;
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(s.phase, Phase::Line { .. }) {
        s.flick = None;
        s.stick_out = v.length() > FLICK_REST;
        return;
    }
    if v.length() >= FLICK_OUT && !s.stick_out {
        s.stick_out = true;
        s.flick = Some(v);
    } else if v.length() < FLICK_REST {
        s.stick_out = false;
    }
}

/// The game's camera now: where, its forward, right and up.
fn view() -> Option<(Vec3, Vec3, Vec3, Vec3)> {
    let cam = unsafe { CSCamera::instance() }.ok()?;
    let p = &cam.pers_cam_1;
    let v = |d: eldenring::position::PositionDelta| Vec3::new(d.0, d.1, d.2).normalize_or_zero();
    let c = p.position();
    Some((Vec3::new(c.0, c.1, c.2), v(p.forward()), v(p.right()), v(p.up()))).filter(|(_, f, ..)| f.length() > 0.5)
}

/// Its bones to anchor to: those in its body (one the reading puts off it hangs in the sky).
fn bones(t: &Titan) -> Vec<(u32, Vec3)> {
    t.spots.iter().filter(|s| t.in_body(s)).map(|s| (s.id, s.at)).collect()
}

/// The bone the hook goes for on big enemy `mob` (hook.rs): the one nearest the middle of the
/// screen, in reach of his hand. Its id and where it is.
pub fn aim(mob: &FieldInsHandle, hand: Vec3, reach: f32) -> Option<(u32, Vec3)> {
    let t = colossus::titan(mob)?;
    let (eye, fwd, ..) = view()?;
    let b = bones(&t);
    let id = centre_pick(&b, eye, fwd, hand, reach)?;
    let sp = t.spot(id)?;
    log(format!("odm: aimed at its {:?} ({:.1} m)", sp.part, (sp.at - hand).length()));
    Some((id, sp.at))
}

/// Where bone `bone` of `mob` is now (the hook flying to it).
pub fn bone_at(mob: &FieldInsHandle, bone: u32) -> Option<Vec3> {
    colossus::titan(mob)?.spot(bone).map(|s| s.at)
}

/// How near the bone he gets: off a limb a little, off its back or its neck by its body.
fn contact_for(part: Part, radius: f32) -> f32 {
    match part {
        Part::Back | Part::Hip | Part::Side => (radius * CONTACT_CORE).max(CONTACT),
        Part::Neck | Part::Head => (radius * CONTACT_NECK).max(CONTACT),
        _ => CONTACT,
    }
}

/// The hook caught bone `bone` of big enemy `mob` (hook.rs): onto the line. Mario's feet and speed
/// (Elden Ring, m/s), the tick. False: its body's gone.
pub fn start(mob: &FieldInsHandle, bone: u32, feet: Vec3, vel: Vec3, tick: u32) -> bool {
    let Some(t) = colossus::titan(mob) else { return false };
    let Some(sp) = t.spot(bone) else { return false };
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.phase = Phase::Line { mob: *mob, bone, part: sp.part, line: Line::new(feet + Vec3::Y * CHEST, vel), at: sp.at, since: tick, struck: false, grab: false, cut: 0 };
    // (the stick out already isn't a flick)
    s.flick = None;
    log(format!("odm: anchored on its {:?} ({:.1} m)", sp.part, (sp.at - feet).length()));
    true
}

/// The hook dropped (hook.rs: a warp, the world re-scaled, death): off the line, falling on.
pub fn cancel() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Phase::Line { line, .. } = s.phase {
        s.phase = Phase::Idle;
        *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let id = s.id;
        drop(s);
        crate::lakitu::set_anchor(None);
        log("odm: the line's gone");
        colossus::let_go(id, line.vel * 0.5, false);
    }
}

/// Out of Mario mode, dead: off it, nothing more.
pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.phase = Phase::Idle;
    *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
    s.gas = GAS_CHARGES;
    drop(s);
    crate::lakitu::set_anchor(None);
}

/// He's put elsewhere (a warp, the world re-scaled: lib.rs): off it, falling.
pub fn release() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(s.phase, Phase::Line { .. } | Phase::Vault { .. }) {
        return;
    }
    s.phase = Phase::Idle;
    *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let id = s.id;
    drop(s);
    crate::lakitu::set_anchor(None);
    colossus::let_go(id, Vec3::ZERO, false);
}

/// The world's origin shifted by `d` (lib.rs): the places kept in its terms with it.
pub fn shift(d: Vec3) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    match &mut s.phase {
        Phase::Line { line, at, .. } => {
            line.pos += d;
            *at += d;
        }
        Phase::Vault { from, to, top, .. } => {
            *from += d;
            *to += d;
            *top += d.y;
        }
        Phase::Rebound { at, .. } => *at += d,
        _ => {}
    }
}

/// R2 held on a medium enemy (hook.rs): a leap over it from his feet `feet`, landing behind it,
/// whose blows count for more a moment. False: no room, it's gone.
pub fn vault(mob: &FieldInsHandle, feet: Vec3, tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> bool {
    let Some(t) = colossus::titan(mob) else { return false };
    let dir = (t.feet - feet).with_y(0.0).normalize_or(t.fwd);
    let mut to = t.feet + dir * (t.radius + VAULT_BEHIND);
    // (a wall behind it: down short of it)
    if let Some(w) = ray(t.feet + Vec3::Y * 1.0, to + Vec3::Y * 1.0) {
        to = Vec3::new(w.x, to.y, w.z) - dir * 0.5;
    }
    if (to - feet).with_y(0.0).length() < t.radius + 0.5 {
        return false;
    }
    let top = t.feet.y + t.height + VAULT_OVER;
    // (a ceiling over it: no room for him up there)
    let head = Vec3::new(t.feet.x, t.feet.y + t.height, t.feet.z);
    if ray(head, Vec3::new(head.x, top + 1.8, head.z)).is_some() {
        return false;
    }
    let dur = ((to - feet).length() / VAULT_SPEED / DT).clamp(VAULT_TICKS.0, VAULT_TICKS.1) as u32;
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(s.phase, Phase::Idle | Phase::Rebound { .. }) {
        return false;
    }
    s.phase = Phase::Vault { mob: *mob, from: feet, to, top, since: tick, dur };
    log(format!("odm: vault over it ({:.1} m, {dur} ticks)", (to - feet).with_y(0.0).length()));
    true
}

/// The vault's backstab: blows on `h` a moment after count for more (combat.rs); 1 otherwise.
pub fn behind_bonus(h: &FieldInsHandle) -> f32 {
    let key = combat::handle_key(h);
    let mut list = BEHIND.lock().unwrap_or_else(|e| e.into_inner());
    list.retain(|(_, t)| t.elapsed().as_secs_f32() < BACKSTAB_TIME);
    if list.iter().any(|(k, _)| *k == key) { BACKSTAB } else { 1.0 }
}

fn sound(id: i32, volume: f32) {
    worker::call("odm sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

/// What the tick did: his buttons were taken (nobody else gets them); A asked for the hook again
/// (lib.rs presses R2 for it).
#[derive(Default)]
pub struct Out {
    pub consumed: bool,
    pub rethrow: bool,
}

/// Off the line now: why (the log), with what speed he falls on (Elden Ring, m/s).
fn off(mut s: std::sync::MutexGuard<'_, State>, why: &str, vel: Vec3, kick: bool) -> Out {
    s.phase = Phase::Idle;
    *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let id = s.id;
    drop(s);
    crate::lakitu::set_anchor(None);
    log(format!("odm: line dropped ({why})"));
    colossus::let_go(id, vel, kick);
    Out { consumed: true, rethrow: false }
}

/// Game thread, every SM64 tick before SM64's (before colossus.rs's): the line, the strike, the
/// rebound, the vault. `allowed` as colossus.rs's.
#[allow(clippy::too_many_arguments)]
pub fn update(inp: &Input, m: &Mario, allowed: bool, id: i32, origin: [f32; 3], combat: &mut Combat, player: &ChrIns, tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Out {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let pressed = [inp.a && !s.prev[0], inp.b && !s.prev[1], inp.x && !s.prev[2], inp.rt && !s.prev[3]];
    s.prev = [inp.a, inp.b, inp.x, inp.rt];
    s.id = id;
    let air = m.action & ACT_FLAG_AIR != 0;
    // (on his feet: the gas is back)
    if !air && m.action != colossus::ACT_ER_CLING {
        s.gas = GAS_CHARGES;
    }
    // (a menu: he hangs on as he is, nothing's read; what's held then is no press after it)
    if m.paused && !m.dead {
        return Out { consumed: matches!(s.phase, Phase::Line { .. } | Phase::Vault { .. } | Phase::Rebound { .. }), rethrow: false };
    }
    if !allowed {
        if let Phase::Line { line, .. } = s.phase {
            return if m.dead {
                s.phase = Phase::Idle;
                *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
                drop(s);
                crate::lakitu::set_anchor(None);
                Out::default()
            } else {
                off(s, "not allowed now", line.vel * 0.5, false)
            };
        }
        if let Phase::Rebound { vel, .. } = s.phase {
            if !m.dead {
                return off(s, "not allowed now", vel * 0.5, false);
            }
        }
        if matches!(s.phase, Phase::Vault { .. }) && !m.dead {
            return off(s, "not allowed now", Vec3::ZERO, false);
        }
        s.phase = Phase::Idle;
        *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Out::default();
    }
    let phase = s.phase;
    match phase {
        Phase::Idle => {
            *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
            Out::default()
        }
        Phase::Rebound { at, vel, face, pose, since, until } => {
            let t = tick as i64 - since as i64;
            // A: the hook again (to the bone nearest the middle of the screen); in the arc, falling
            // on with its speed as he throws it
            if pressed[0] {
                s.phase = Phase::Idle;
                drop(s);
                if t < REBOUND_ARC as i64 {
                    colossus::let_go(id, vel, false);
                }
                log("odm: re-anchor");
                return Out { consumed: true, rethrow: true };
            }
            // the blow's hold: still, in its pose
            if t < 0 {
                drop(s);
                colossus::place(id, origin, at, face, pose, true);
                return Out { consumed: true, rethrow: false };
            }
            if t < REBOUND_ARC as i64 {
                // (a blow on him: off)
                if t > 2 && m.action != colossus::ACT_ER_CLING {
                    s.phase = Phase::Idle;
                    return Out::default();
                }
                let (mut next, mut v) = arc_step(at, vel, DT);
                // (a wall: stopped against it; the ground: landed)
                if ray(at + Vec3::Y * 1.0, next + Vec3::Y * 1.0).is_some() {
                    next = Vec3::new(at.x, next.y, at.z);
                    v = Vec3::new(0.0, v.y, 0.0);
                }
                if v.y < 0.0 {
                    if let Some(g) = ray(next + Vec3::Y * 0.5, next - Vec3::Y * 0.05) {
                        s.phase = Phase::Idle;
                        drop(s);
                        colossus::place(id, origin, g, face, FLIP, true);
                        colossus::let_go(id, Vec3::ZERO, false);
                        return Out { consumed: true, rethrow: false };
                    }
                }
                s.phase = Phase::Rebound { at: next, vel: v, face, pose, since, until };
                drop(s);
                colossus::place(id, origin, next, face, FLIP, true);
                return Out { consumed: true, rethrow: false };
            }
            // the arc's end: a fall on with its speed (facing the way he goes: SM64 keeps a speed
            // ahead, not one backwards), A still re-anchoring a moment
            if t == REBOUND_ARC as i64 {
                drop(s);
                colossus::let_go(id, vel, false);
                return Out::default();
            }
            if tick >= until || !air {
                s.phase = Phase::Idle;
            }
            Out::default()
        }
        Phase::Vault { mob, from, to, top, since, dur } => {
            let t = tick.wrapping_sub(since);
            // (a blow on him mid-leap: off)
            if t > 2 && m.action != colossus::ACT_ER_CLING {
                s.phase = Phase::Idle;
                return Out::default();
            }
            let k = (t as f32 / dur as f32).min(1.0);
            let e = k * k * (3.0 - 2.0 * k);
            let base = from.lerp(to, e);
            let mid = (from.y + to.y) * 0.5;
            let at = base + Vec3::Y * (top - mid).max(0.0) * 4.0 * k * (1.0 - k);
            let way = (to - from).with_y(0.0);
            if k >= 1.0 {
                s.phase = Phase::Idle;
                drop(s);
                let mut list = BEHIND.lock().unwrap_or_else(|e| e.into_inner());
                list.retain(|(k, _)| *k != combat::handle_key(&mob));
                list.push((combat::handle_key(&mob), Instant::now()));
                drop(list);
                crate::style::callout("BEHIND!", Some(to + Vec3::Y * 2.0), [0.85, 0.9, 1.0], false, 2.0);
                // (down behind it, turned round over its top to face it)
                colossus::let_go(id, Vec3::ZERO, false);
                worker::call("odm vault turn", move |_| unsafe { sm64::sm64_set_mario_faceangle(id, way.x.atan2(-way.z)) });
                return Out { consumed: true, rethrow: false };
            }
            drop(s);
            crate::impact::dash_point(at, false);
            colossus::place(id, origin, at, way, LEAP, true);
            Out { consumed: true, rethrow: false }
        }
        Phase::Line { mob, bone, part, line, at, since, struck, grab, cut } => {
            let (mut bone, mut part, mut line, mut struck, mut grab, mut cut) = (bone, part, line, struck, grab, cut);
            let Some(t) = colossus::titan(&mob) else { return off(s, "its body's gone", line.vel * 0.5, false) };
            // (its bone gone from the reading: the nearest one instead)
            let Some(sp) = t.spot(bone).filter(|x| t.in_body(x)).or_else(|| {
                t.spots.iter().copied().filter(|x| t.in_body(x)).min_by(|a, b| (a.at - at).length().total_cmp(&(b.at - at).length()))
            }) else {
                return off(s, "no bone to hold", line.vel * 0.5, false);
            };
            if sp.id != bone {
                bone = sp.id;
                part = sp.part;
            }
            // (a blow on him, or SM64 took him elsewhere: off)
            if tick.wrapping_sub(since) > SETTLE && m.action != colossus::ACT_ER_CLING {
                s.phase = Phase::Idle;
                *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
                drop(s);
                crate::lakitu::set_anchor(None);
                log("odm: knocked off the line");
                return Out::default();
            }
            if tick.wrapping_sub(since) > LINE_MOST {
                return off(s, "too long", line.vel * 0.5, false);
            }
            // R2: let go, falling on along his way (no flying kick: let go right by it, a kick hit
            // harder than the strike, without its timing)
            if pressed[3] {
                log(format!("odm: let go at {:.1} m/s", line.speed()));
                return off(s, "let go", line.vel, false);
            }
            // the right stick flicked: the next bone that way on the screen
            if let Some(dir) = s.flick.take() {
                let next = view().and_then(|(eye, fwd, right, up)| flick_pick(&bones(&t), bone, eye, fwd, right, up, dir)).and_then(|b| t.spot(b));
                match next {
                    Some(n) => {
                        bone = n.id;
                        part = n.part;
                        crate::impact::hook_latch(n.at);
                        sound(SOUND_LATCH, 0.4);
                        log(format!("odm: flick to its {:?}", n.part));
                    }
                    None => sound(SOUND_BUZZ, 0.3),
                }
            }
            // B: a burst of gas (the stick to a side: a sidestep round it that way, hissing)
            if pressed[1] {
                if s.gas > 0 {
                    s.gas -= 1;
                    let side = line.dash(inp.steer, at);
                    crate::sfx::play(crate::sfx::Sfx::Gas, if side { 0.55 } else { 0.45 });
                    crate::impact::dash_point(line.pos - Vec3::Y * CHEST, true);
                    log(format!("odm: gas{} ({} left)", if side { ", a sidestep" } else { "" }, s.gas));
                } else {
                    sound(SOUND_BUZZ, 0.3);
                }
            }
            let target = t.spot(bone).map_or(sp.at, |x| x.at);
            let at = at.lerp(target, ANCHOR_EASE);
            let contact = contact_for(part, t.radius);
            let before = line.pos;
            let step = line.step(at, inp.steer, contact, DT);
            // (the ground under him, swung low round a leg: along it, never into it)
            if let Some(g) = ray(line.pos + Vec3::Y * 0.3, line.pos - Vec3::Y * (CHEST + 0.05)) {
                line.pos.y = g.y + CHEST + 0.05;
                line.vel.y = line.vel.y.max(0.0);
            }
            // (a wall in his way: off the line against it; the line through the scenery a moment: it
            // drops)
            if ray(before, line.pos).is_some() {
                crate::impact::thud_small(line.pos);
                return off(s, "into the scenery", -line.vel * 0.15, false);
            }
            cut = if ray(line.pos, at).is_some_and(|h| (h - at).length() > 1.5) { cut + 1 } else { 0 };
            if cut >= CUT_TICKS {
                return off(s, "caught on the scenery", line.vel * 0.6, false);
            }
            // X in the window (or just before it): the strike; A: a colossus's grab
            let late = step.to_contact <= STRIKE_WINDOW + PRESS_GRACE;
            if pressed[2] && late && !struck {
                struck = true;
            }
            if pressed[0] && late {
                grab = true;
            }
            let speed = line.speed();
            if step.to_contact <= 0.0 {
                // a colossus: A as he got there grabs on (colossus.rs climbs it)
                if grab && colossus::is_colossus(t.radius, t.height) && colossus::grab_at(&t, line.pos) {
                    s.phase = Phase::Idle;
                    *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    drop(s);
                    crate::lakitu::set_anchor(None);
                    log(format!("odm: grabbed on at {speed:.1} m/s"));
                    return Out { consumed: true, rethrow: false };
                }
                let nape = struck && matches!(part, Part::Neck | Part::Head) && from_behind(line.vel, t.fwd);
                let boss = !combat::liftable(&mob);
                let mult = multiplier(speed, nape, boss);
                let attack = if struck { Attack::OdmStrike } else { Attack::OdmGlance };
                let tier = tier(speed);
                let feet = line.pos - Vec3::Y * CHEST;
                let face = (target - line.pos).with_y(0.0);
                let pose = if struck { if crate::weapons::current() == crate::weapons::FISTS { KICK } else { SPIN } } else { DIVE };
                let since = tick + crate::impact::stop(attack);
                s.gas = GAS_CHARGES;
                s.phase = Phase::Rebound { at: feet, vel: rebound(line.pos - target), face, pose, since, until: since + REBOUND_ARC + REBOUND_TICKS };
                *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
                drop(s);
                crate::lakitu::set_anchor(None);
                let near = combat::nearby(&HavokPosition(t.feet.x, t.feet.y, t.feet.z, 0.0), t.radius + 6.0, origin);
                if let Some(i) = near.iter().position(|x| x.key() == t.key) {
                    combat.deal_scaled(player, &near, &[(i, attack, false)], tick, mult);
                }
                // the tier heard and seen: a tink below the lowest, a crash at the top
                match tier {
                    0 => {
                        sound(SOUND_TINK, 0.35);
                        crate::impact::thud_small(target);
                    }
                    3 => {
                        sound(SOUND_HEAVY, 0.4);
                        crate::lakitu::shake(0.6);
                    }
                    _ => {}
                }
                if nape {
                    crate::style::callout("NAPE!", Some(target + Vec3::Y * 0.8), [1.0, 0.35, 0.25], true, 6.0);
                    crate::swing::add_stance(&mob, NAPE_STANCE);
                    crate::style::slow_down(0.35);
                }
                log(format!(
                    "odm: {} at {speed:.1} m/s on its {part:?} (tier {tier}, x{mult:.2}{})",
                    if struck { "STRIKE" } else { "glancing hit" },
                    if nape { ", from behind" } else { "" }
                ));
                sound(SOUND_GAS, 0.3);
                return Out { consumed: true, rethrow: false };
            }
            let feet = line.pos - Vec3::Y * CHEST;
            let face = if step.orbiting { line.vel.with_y(0.0) } else { (at - line.pos).with_y(0.0) };
            let mode = if struck {
                if crate::weapons::current() == crate::weapons::FISTS { KICK } else { SPIN }
            } else if step.orbiting {
                SWING
            } else {
                DIVE
            };
            s.phase = Phase::Line { mob, bone, part, line, at, since, struck, grab, cut };
            *HUD.lock().unwrap_or_else(|e| e.into_inner()) = Some(Hud { anchor: at, hand: line.pos, tier: tier(speed), window: step.to_contact <= STRIKE_WINDOW, gas: s.gas, mario: feet });
            drop(s);
            crate::lakitu::set_anchor(Some(at));
            if speed > LINES_FROM {
                crate::impact::dash_point(feet, false);
            }
            colossus::place(id, origin, feet, face, mode, true);
            Out { consumed: true, rethrow: false }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    #[test]
    fn the_bone_nearest_the_screens_middle() {
        // the camera behind him looking +Z; the head far off but dead ahead, a hand near him off
        // to the side
        let eye = Vec3::new(0.0, 2.0, -4.0);
        let bones = [(1, Vec3::new(0.0, 6.0, 15.0)), (2, Vec3::new(4.0, 2.0, 3.0)), (3, Vec3::new(0.0, 2.0, -10.0))];
        assert_eq!(centre_pick(&bones, eye, Vec3::Z, Vec3::new(0.0, 1.0, 0.0), 30.0), Some(1));
        // (out of his reach: the next best)
        assert_eq!(centre_pick(&bones, eye, Vec3::Z, Vec3::new(0.0, 1.0, 0.0), 10.0), Some(2));
        // (behind the camera never; nothing near the middle: none)
        assert_eq!(centre_pick(&bones[2..], eye, Vec3::Z, Vec3::ZERO, 30.0), None);
        assert_eq!(centre_pick(&[(4, Vec3::new(30.0, 2.0, 1.0))], eye, Vec3::Z, Vec3::ZERO, 40.0), None);
    }

    #[test]
    fn a_flick_takes_the_bone_that_way_on_screen() {
        let (eye, fwd, right, up) = (Vec3::new(0.0, 2.0, -10.0), Vec3::Z, Vec3::X, Vec3::Y);
        // head in the middle, a hand to its right, a foot below, another hand far right
        let bones = [(1, Vec3::new(0.0, 6.0, 0.0)), (2, Vec3::new(2.0, 5.0, 0.0)), (3, Vec3::new(0.0, 0.5, 0.0)), (4, Vec3::new(5.0, 5.5, 0.0))];
        assert_eq!(flick_pick(&bones, 1, eye, fwd, right, up, Vec2::X), Some(2));
        assert_eq!(flick_pick(&bones, 1, eye, fwd, right, up, Vec2::NEG_Y), Some(3));
        assert_eq!(flick_pick(&bones, 2, eye, fwd, right, up, Vec2::X), Some(4));
        // (nothing up there)
        assert_eq!(flick_pick(&bones, 1, eye, fwd, right, up, Vec2::Y), None);
        assert_eq!(flick_pick(&bones, 4, eye, fwd, right, up, Vec2::X), None);
    }

    #[test]
    fn the_strike_is_worth_its_speed() {
        assert!((speed_factor(20.0) - 1.0).abs() < 1e-5);
        assert!((speed_factor(2.0) - 0.3).abs() < 1e-5);
        assert!((speed_factor(50.0) - 1.8).abs() < 1e-5);
        assert_eq!([tier(5.0), tier(11.9), tier(12.0), tier(19.9), tier(20.0), tier(28.0), tier(40.0)], [0, 0, 1, 1, 2, 3, 3]);
        assert!((multiplier(20.0, true, false) - 1.5).abs() < 1e-5);
        assert!((multiplier(40.0, true, false) - 2.7).abs() < 1e-5);
        // (a boss: all of it together at most 1.5)
        assert!((multiplier(40.0, true, true) - 1.5).abs() < 1e-5);
        assert!((multiplier(20.0, false, true) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn from_behind_is_along_its_facing() {
        assert!(from_behind(Vec3::new(0.0, -0.5, 1.0), Vec3::Z));
        assert!(!from_behind(Vec3::new(0.0, 0.0, -1.0), Vec3::Z));
        assert!(!from_behind(Vec3::X, Vec3::Z));
    }

    #[test]
    fn the_reel_ramps_up_and_the_gas_lifts_it() {
        let anchor = Vec3::new(0.0, 10.0, 60.0);
        let mut l = Line::new(Vec3::ZERO, Vec3::ZERO);
        l.step(anchor, Vec3::ZERO, 1.2, DT);
        assert!((l.speed() - (REEL_FROM + REEL_RAMP * DT)).abs() < 0.2, "{}", l.speed());
        for _ in 0..30 {
            l.step(anchor, Vec3::ZERO, 1.2, DT);
        }
        assert!((l.speed() - REEL_TO).abs() < 0.01, "{}", l.speed());
        // (along the line, towards the anchor)
        assert!(l.vel.normalize().dot((anchor - l.pos).normalize()) > 0.99);
        l.burst();
        l.step(anchor, Vec3::ZERO, 1.2, DT);
        assert!(l.speed() > REEL_TO * 1.35, "{}", l.speed());
        for _ in 0..30 {
            l.step(anchor, Vec3::ZERO, 1.2, DT);
        }
        assert!((l.speed() - REEL_TO).abs() < 0.01, "{}", l.speed());
    }

    /// B with the stick across the line: a burst of gas that way, round the anchor (the 3D
    /// maneuver gear's sidestep), its length kept; even with the orbit spent.
    #[test]
    fn a_sideways_burst_swings_him_round() {
        let anchor = Vec3::new(0.0, 5.0, 0.0);
        for spent in [false, true] {
            let mut l = Line::new(Vec3::new(0.0, 5.0, -10.0), Vec3::ZERO);
            if spent {
                l.orbit_used = ORBIT_MAX;
            }
            let start = l.pos - anchor;
            assert!(l.dash(Vec3::X, anchor));
            let mut ticks = 0;
            while l.dashing() && ticks < 60 {
                l.step(anchor, Vec3::ZERO, 1.2, DT);
                ticks += 1;
            }
            let now = l.pos - anchor;
            let turned = start.angle_between(now).to_degrees();
            assert!(turned > 50.0 && turned < 120.0, "spent {spent}: turned {turned}");
            assert!((now.length() - start.length()).abs() < 0.6, "spent {spent}: {} m", now.length());
            // (the way it was pushed: toward +x)
            assert!(now.x > 0.0, "{now}");
            assert!(ticks as f32 * DT <= DASH_TIME + 0.05);
        }
        // (on a short line the sidestep goes on round past a quarter turn, no shaking at its side)
        let mut l = Line::new(Vec3::new(0.0, 5.0, -4.0), Vec3::ZERO);
        assert!(l.dash(Vec3::X, anchor));
        let mut last = l.pos;
        let mut turned = 0.0;
        while l.dashing() {
            l.step(anchor, Vec3::ZERO, 1.2, DT);
            let s = (last - anchor).normalize().cross((l.pos - anchor).normalize()).y;
            assert!(s <= 1e-4, "it turned back: {s}");
            turned += (last - anchor).angle_between(l.pos - anchor);
            last = l.pos;
        }
        assert!(turned.to_degrees() > 100.0, "{}", turned.to_degrees());
        // (no strike window in a sidestep)
        let mut l = Line::new(Vec3::new(0.0, 5.0, -4.0), Vec3::ZERO);
        l.dash(Vec3::X, anchor);
        assert!(l.step(anchor, Vec3::ZERO, 1.2, DT).to_contact > STRIKE_WINDOW);
        // (with the stick along the line it's the old burst: the reel's)
        let mut l = Line::new(Vec3::new(0.0, 5.0, -10.0), Vec3::ZERO);
        let toward = (anchor - l.pos).normalize();
        assert!(!l.dash(toward, anchor));
        assert!(!l.dashing());
        l.step(anchor, Vec3::ZERO, 1.2, DT);
        assert!(l.speed() > REEL_FROM * 1.3, "{}", l.speed());
    }

    #[test]
    fn the_orbit_turns_at_most_90_a_second_and_keeps_its_radius() {
        let anchor = Vec3::new(0.0, 5.0, 0.0);
        let mut l = Line::new(Vec3::new(0.0, 5.0, -10.0), Vec3::ZERO);
        // (the stick right: across the line, as the camera behind him looking at the anchor keeps it)
        let right = |l: &Line| (l.pos - anchor).cross(Vec3::Y).normalize();
        let s = l.step(anchor, right(&l), 1.2, DT);
        assert!(s.orbiting);
        for _ in 0..29 {
            l.step(anchor, right(&l), 1.2, DT);
        }
        let rel = l.pos - anchor;
        assert!((rel.length() - 10.0).abs() < 0.01, "{}", rel.length());
        let turned = rel.x.atan2(-rel.z);
        // (a second at full stick: a quarter turn, towards the stick)
        assert!((turned - std::f32::consts::FRAC_PI_2).abs() < 0.05 && rel.x > 9.9, "{rel:?}");
        // (3 s a line: then the stick no longer orbits, the reel takes him in)
        for _ in 0..60 {
            l.step(anchor, right(&l), 1.2, DT);
        }
        let s = l.step(anchor, right(&l), 1.2, DT);
        assert!(!s.orbiting);
        assert!((l.pos - anchor).length() < 9.9);
    }

    #[test]
    fn contact_comes_nearer_as_he_reels_in() {
        let anchor = Vec3::new(0.0, 0.0, 20.0);
        let mut l = Line::new(Vec3::ZERO, Vec3::ZERO);
        let first = l.step(anchor, Vec3::ZERO, 1.2, DT).to_contact;
        let mut last = first;
        for _ in 0..40 {
            last = l.step(anchor, Vec3::ZERO, 1.2, DT).to_contact;
            if last <= 0.0 {
                break;
            }
        }
        assert!(first > 0.5 && last <= 0.0, "{first} {last}");
        // (never past the contact)
        assert!((l.pos - anchor).length() >= 1.2 - 1e-3, "{}", (l.pos - anchor).length());
    }

    #[test]
    fn the_rebound_goes_back_and_up_7_m_or_so() {
        let v = rebound(Vec3::new(0.0, 0.0, -2.0));
        assert!(v.z < 0.0 && v.y > 0.0 && v.x.abs() < 1e-5);
        // the arc driven, then SM64's fall (units: 0.01 m, a tick 1/30 s): its gravity 4 a tick²,
        // the speed ahead drawn 0.35 a tick to nothing and 1 more past 32 (he faces the way he goes)
        let (mut at, mut vel) = (Vec3::ZERO, v);
        for _ in 0..REBOUND_ARC {
            (at, vel) = arc_step(at, vel, DT);
        }
        assert!(at.y > 0.5, "{at:?}");
        let (mut y, mut vy, mut fwd, mut back) = (at.y * 100.0, vel.y * 100.0 / 30.0, vel.with_y(0.0).length() * 100.0 / 30.0, -at.z);
        while y > 0.0 {
            fwd = (fwd - 0.35).max(0.0);
            if fwd > 32.0 {
                fwd -= 1.0;
            }
            vy = (vy - 4.0).max(-75.0);
            y += vy;
            back += fwd / 100.0;
        }
        assert!((6.0..=8.0).contains(&back), "{back}");
    }

    #[test]
    fn a_burst_in_an_orbit_waits_for_the_reel() {
        let anchor = Vec3::new(0.0, 5.0, 0.0);
        let mut l = Line::new(Vec3::new(0.0, 5.0, -10.0), Vec3::ZERO);
        let right = |l: &Line| (l.pos - anchor).cross(Vec3::Y).normalize();
        l.burst();
        for _ in 0..30 {
            l.step(anchor, right(&l), 1.2, DT);
        }
        l.step(anchor, Vec3::ZERO, 1.2, DT);
        assert!(l.speed() > (REEL_FROM + REEL_RAMP * DT) * 1.35, "{}", l.speed());
        // (a stick at the orbit's edge: no flicker, once it orbits it holds on down to ORBIT_TO)
        let mut l = Line::new(Vec3::new(0.0, 5.0, -10.0), Vec3::ZERO);
        assert!(!l.step(anchor, right(&l) * 0.2, 1.2, DT).orbiting);
        assert!(l.step(anchor, right(&l) * 0.3, 1.2, DT).orbiting);
        assert!(l.step(anchor, right(&l) * 0.2, 1.2, DT).orbiting);
        assert!(!l.step(anchor, right(&l) * 0.1, 1.2, DT).orbiting);
    }

    #[test]
    fn the_cone_takes_a_spot_ahead_and_up() {
        let dirs = cone_dirs(Vec3::Z);
        assert_eq!(dirs.len(), 15);
        for d in &dirs {
            let up = d.y.asin().to_degrees();
            assert!((d.length() - 1.0).abs() < 1e-4 && (34.0..=66.0).contains(&up) && d.z > 0.0, "{d:?}");
        }
        let (from, feet) = (Vec3::new(0.0, 1.0, 0.0), Vec3::ZERO);
        let at = |d: Vec3, m: f32| (d, Some(from + d * m));
        let fifty = dirs.iter().copied().find(|d| d.x.abs() < 1e-4 && (d.y.asin().to_degrees() - 50.0).abs() < 0.1).unwrap();
        let steep = dirs.iter().copied().find(|d| d.x.abs() < 1e-4 && (d.y.asin().to_degrees() - 65.0).abs() < 0.1).unwrap();
        let hits = [at(steep, 12.0), at(fifty, 14.0), (dirs[3], None)];
        assert_eq!(cone_pick(&hits, from, feet, Vec3::Z), Some(from + fifty * 14.0));
        // (too near, too far, too low: none)
        assert_eq!(cone_pick(&[at(fifty, 3.0), at(fifty, 40.0)], from, feet, Vec3::Z), None);
        assert_eq!(cone_pick(&[(Vec3::Z, None)], from, feet, Vec3::Z), None);
    }
}
