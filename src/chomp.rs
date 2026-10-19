//! Mario's Chain Chomp (weapons.rs): held on its chain, it lunges, sweeps and crashes where
//! er_moves.c's moves put it (degrees around his facing, how far out, how high), and its body is
//! what they hit with (combat.rs, as the hammer's head). Between blows it hops along beside him,
//! straining at the chain. It's a real model (assets/gear.rs, on bones of its own like the hammer):
//! the ball with its eyes and teeth, its jaw biting, and the chain's links from his hand, sagging
//! when slack and taut in a lash. With the Chomp in hand, the hook (hook.rs) is the Chomp itself:
//! it flies out on its chain, jaws open, and bites what it catches (Kratos' blades pulling him in).

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

use glam::{Mat3, Quat};

use crate::assets::gear::{CHOMP_R, HINGE, PAIR_LEN};
use crate::engine_mario::{CHAIN, CHAIN_LINKS, CHOMP, JAW, PARTS, PartPose};
use crate::hammer::Swing;
use crate::sm64;

/// SM64 parts: the right hand
const R_HAND: usize = 9;
/// its reach for the hits: the hammer head's reach times this (combat.rs)
const HIT_SCALE: f32 = 0.72;
/// beside him between blows: degrees around his facing (behind, to his right), how far (SM64
/// units), how fast it gets there (share of the way per tick), its hops (units high, per second)
const REST_YAW: f32 = -140.0;
const REST_REACH: f32 = 120.0;
const FOLLOW: f32 = 0.3;
const HOP: f32 = 30.0;
const HOP_RATE: f32 = 7.0;

/// the ball sits this much above the place the moves give (m: that's low, its bottom on the floor)
const LIFT: f32 = 0.2;
/// its jaw: open this far at most (radians), how fast it opens and shuts (share per tick)
const JAW_OPEN: f32 = 0.8;
const JAW_RATE: f32 = 0.55;
/// it turns to where it lunges this fast (share per tick)
const TURN: f32 = 0.45;
/// the chain sags this much when slack (m), taut from this long (m)
const SAG: f32 = 0.35;
const TAUT: f32 = 5.6;
/// it pops out (s)
const POP_TIME: f32 = 0.25;
/// its squash and stretch: flying fast it stretches out the way it looks (at this speed, m per
/// tick, all the way, this much longer, thinner to keep its size), and when it stops dead after
/// flying (a crash, a bite landing) it squashes flat for a few ticks; its links stretch when the
/// chain is taut
const STRETCH_SPEED: f32 = 0.35;
const STRETCH: f32 = 0.55;
const SQUASH_TICKS: u32 = 5;
const LINK_STRETCH: f32 = 0.3;
/// a jump further than this in a tick (m) is the hook letting go, not flight (the cyclone flies
/// it round metres a tick)
const JUMP: f32 = 8.0;
/// swung round him (the cyclone, a sweep; its speed round him in degrees a tick, eased this much
/// a tick): from this fast the chain trails behind it in a curve, his hand leading the ball by a
/// lag (degrees, and degrees per degree a tick, as er_moves.c's cyclone), bowed back along its
/// middle (m, and m per degree a tick); from that fast its teeth lead, turned into the way it goes,
/// and it stretches along it (all the way at the last speed)
const SPIN_FROM: f32 = 3.0;
const SPIN_FULL: f32 = 15.0;
const SPIN_EASE: f32 = 0.5;
const SPIN_LAG: f32 = 10.0;
const SPIN_LAG_PER_SPEED: f32 = 0.55;
const SPIN_BOW: f32 = 0.12;
const SPIN_BOW_PER_SPEED: f32 = 0.006;
const LEAD_FROM: f32 = 15.0;
const LEAD: f32 = 0.8;
const SPIN_STRETCH: f32 = 0.35;
const SPIN_STRETCH_SPEED: f32 = 40.0;
/// the chain only sags this slack (share of its length) and more
const SAG_FROM: f32 = 0.1;

struct State {
    out: bool,
    /// the Chomp's last place (SM64), and whether a move held it
    pos: Option<Vec3>,
    scripted: bool,
    born: Option<Instant>,
    /// where it looks (part axes), how open its jaw is
    look: Vec3,
    jaw: f32,
    /// its last centre (part axes), how far it moved last tick (m), the ticks of squash left
    last_centre: Option<Vec3>,
    step: f32,
    squash: u32,
    /// how fast it goes round him (degrees a tick, part axes, eased), and its last angle round him
    spin: f32,
    round: Option<f32>,
}

static STATE: Mutex<State> = Mutex::new(State { out: false, pos: None, scripted: false, born: None, look: Vec3::Z, jaw: 0.0, last_centre: None, step: 0.0, squash: 0, spin: 0.0, round: None });
/// the Chomp is out and drawn, so it's the hook's head (hud.rs draws its chain, not the cap)
static HOOKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn hooking() -> bool {
    HOOKING.load(std::sync::atomic::Ordering::Relaxed)
}

/// The Chain Chomp is in hand or not (weapons.rs).
pub fn set_out(out: bool) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !out {
        HOOKING.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    if s.out != out {
        s.out = out;
        s.pos = None;
        s.born = Some(Instant::now());
    }
}

/// Worker thread, after each SM64 tick: where the Chomp is now (SM64 units) and its sweep for the
/// hits, if it's out; puts its ball, jaw and chain on their parts. `parts`: his part poses (the
/// hand holds the chain).
pub fn pose(parts: &mut [PartPose; PARTS], state: &sm64::SM64MarioState, mv: &sm64::SM64ErMove, visible: bool) -> Option<Swing> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.out || !visible {
        s.pos = None;
        s.last_centre = None;
        s.step = 0.0;
        s.squash = 0;
        s.spin = 0.0;
        s.round = None;
        HOOKING.store(false, std::sync::atomic::Ordering::Relaxed);
        return None;
    }
    let hook = crate::hook::live();
    HOOKING.store(true, std::sync::atomic::Ordering::Relaxed);
    let mario = Vec3::from(state.position);
    let face = state.face_angle;
    let scripted = mv.chomp_scripted != 0;
    let at = |yaw_deg: f32, reach: f32, height: f32| {
        let yaw = face + yaw_deg.to_radians();
        mario + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * reach + Vec3::Y * height
    };
    let t = s.born.get_or_insert_with(Instant::now).elapsed().as_secs_f32();
    let want = if scripted {
        at(mv.chomp_yaw, mv.chomp_reach, mv.chomp_height)
    } else {
        // hopping beside him, a little higher when he's moving
        let hop = (t * HOP_RATE).sin().abs() * HOP * if state.forward_velocity.abs() > 4.0 { 1.3 } else { 0.6 };
        at(REST_YAW, REST_REACH, 20.0 + hop)
    };
    let prev = s.pos.unwrap_or(want);
    // (a move puts it exactly where it hits; between blows it follows, trailing behind)
    let now = if scripted { want } else { prev.lerp(want, FOLLOW) };
    s.pos = Some(now);
    s.scripted = scripted;
    let hand = parts[R_HAND].pos;
    let grip = mario + Vec3::new(-hand.x, hand.y, hand.z) / crate::scale();
    // the model: SM64 -> part axes (mirrored, metres from Mario)
    let rel = |p: Vec3| {
        let d = (p - mario) * crate::scale();
        Vec3::new(-d.x, d.y, d.z)
    };
    // (the hook's head: where the hook is, from his hand; part axes are Elden Ring's)
    let hooked = hook.map(|hk| hand + (hk.head - hk.hand));
    let centre = hooked.unwrap_or(rel(now) + Vec3::Y * LIFT);
    if hooked.is_some() {
        // (back from the hook it hops in from where it was)
        s.pos = Some(mario + Vec3::new(-centre.x, centre.y - LIFT, centre.z) / crate::scale());
    }
    let moved = rel(now) - rel(prev);
    let size = {
        let p = (t / POP_TIME).min(1.0);
        (if p < 0.6 { p / 0.6 * 1.2 } else { 1.2 - 0.2 * (p - 0.6) / 0.4 }).max(0.01)
    };
    // how fast it goes round him (swung by a move, not on the hook)
    let round = centre.x.atan2(centre.z);
    let turn = match s.round {
        Some(r) if scripted && hooked.is_none() => (round - r + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI,
        _ => 0.0,
    };
    s.round = Some(round);
    // (a hit-stop holds the ball where it is: its swing goes on after it, no squash, no slowing)
    let held = scripted && hooked.is_none() && s.last_centre.is_some_and(|c| (centre - c).length() < 1e-4);
    if !held {
        s.spin += (turn.to_degrees() - s.spin) * SPIN_EASE;
    }
    let (speed, sense) = (s.spin.abs(), s.spin.signum());
    let swung = ((speed - SPIN_FROM) / (SPIN_FULL - SPIN_FROM)).clamp(0.0, 1.0);
    let lead = ((speed - LEAD_FROM) / LEAD_FROM).clamp(0.0, 1.0);
    // it looks away from his hand (straining), and up or down the way it flies (swung fast round
    // him, its teeth turned into the way it goes)
    let face = state.face_angle;
    let ahead = Vec3::new(-face.sin(), 0.0, face.cos());
    let away = (centre - hand).with_y(0.0);
    let flat = if away.length() > 0.3 { away.normalize() } else { ahead };
    let flat = (flat + Vec3::new(flat.z, 0.0, -flat.x) * sense * lead * LEAD).normalize_or(flat);
    // (on the hook it looks along the chain, up or down, jaws wide)
    let (flat, scripted) = match hooked {
        Some(c) => ((c - hand).normalize_or(flat), true),
        None => (flat, scripted),
    };
    let pitch = if scripted { (moved.y / moved.with_y(0.0).length().max(0.05)).clamp(-1.2, 1.2) * 0.35 } else { 0.0 };
    let want = (flat * pitch.cos() + Vec3::Y * pitch.sin()).normalize_or(flat);
    let look = s.look.lerp(want, TURN).normalize_or(want);
    s.look = look;
    // its jaw: what the move says, or a snap now and then between blows
    let bite = if hooked.is_some() { 1.0 } else if scripted { mv.chomp_bite } else { ((t * 3.3) as i32 % 4 == 0) as u8 as f32 };
    s.jaw += (bite.clamp(0.0, 1.0) - s.jaw) * JAW_RATE;
    let jaw = s.jaw * JAW_OPEN;
    let right = Vec3::Y.cross(look).normalize_or(Vec3::X);
    let up = look.cross(right);
    let body = Quat::from_mat3(&Mat3::from_cols(right, up, look)).normalize();
    // squash and stretch (its own axes: Z the way it looks)
    let step = s.last_centre.map_or(0.0, |c| (centre - c).length());
    s.last_centre = Some(centre);
    if !held {
        if s.step > STRETCH_SPEED * 0.6 && step < s.step * 0.3 && step < JUMP {
            s.squash = SQUASH_TICKS;
        }
        // (a jump of metres is the hook letting go, not flight)
        s.step = if step < JUMP { step } else { 0.0 };
    }
    let stretch = if s.squash > 0 {
        s.squash -= 1;
        let p = s.squash as f32 / SQUASH_TICKS as f32;
        Vec3::new(1.0 + 0.24 * p, 1.0 + 0.24 * p, 1.0 - 0.35 * p)
    } else {
        // (swung round, stretched along the way it goes, its teeth leading: by its speed round
        // him, not its whole speed, a ball pulled long at the cyclone's metres a tick)
        let flying = 1.0 + STRETCH * (s.step / STRETCH_SPEED).clamp(0.0, 1.0);
        let swinging = 1.0 + SPIN_STRETCH * (speed / SPIN_STRETCH_SPEED).clamp(0.0, 1.0);
        let z = flying + (swinging - flying) * lead;
        Vec3::new(1.0 / z.sqrt(), 1.0 / z.sqrt(), z)
    };
    // (a little wobble as it hops)
    let body = if scripted { body } else { body * Quat::from_rotation_z((t * 7.0).sin() * 0.12) };
    parts[CHOMP] = PartPose { rot: body, pos: centre, scale: size, stretch };
    // its jaw is the wedge of the ball its mouth is, so it squashes and stretches with it: a part
    // only stretches along its own axes, turned open with the jaw, so it takes the ball's stretch
    // along those (as near as they get), opens less the more lopsided the ball is (or they'd shear
    // apart), and turns about the hinge where it is on the stretched ball
    let jaw = jaw * (1.0 - (stretch.z - stretch.y).abs() * 1.2).clamp(0.3, 1.0);
    let (sin, cos) = jaw.sin_cos();
    let jaw_stretch = Vec3::new(stretch.x, stretch.y * cos * cos + stretch.z * sin * sin, stretch.y * sin * sin + stretch.z * cos * cos);
    let open = Quat::from_rotation_x(jaw);
    let pos = centre + body * (HINGE * stretch - open * (HINGE * jaw_stretch)) * size;
    parts[JAW] = PartPose { rot: (body * open).normalize(), pos, scale: size, stretch: jaw_stretch };
    // the chain: from his hand to the back of the ball, its pairs of links spread along it (a
    // little bigger when it's long, so it reads as a chain), sagging when slack (and swaying a
    // little); a pair's second link is turned a quarter, so the next pair's first hooks into it.
    // Swung round him it trails the ball in a curve: his hand leads it round by a lag, the links
    // turned less and less of it the nearer the ball, bowed back along the middle
    let anchor = centre - look * CHOMP_R * 0.85 * size;
    let span = anchor - hand;
    let slack = 1.0 - (span.length() / TAUT).min(1.0);
    let sag = SAG * ((slack - SAG_FROM) / (1.0 - SAG_FROM)).max(0.0);
    let side = Vec3::Y.cross(span).normalize_or(right);
    let links = (span.length() / (CHAIN_LINKS as f32 * PAIR_LEN)).clamp(1.0, 2.1) * size.min(1.0);
    let (reach, height, toward) = (span.with_y(0.0).length(), span.y, span.x.atan2(span.z));
    let lag = (SPIN_LAG + SPIN_LAG_PER_SPEED * speed).to_radians() * sense * swung;
    let bow = (SPIN_BOW + SPIN_BOW_PER_SPEED * speed) * swung;
    let curve = |u: f32| {
        let yaw = toward + lag * (1.0 - u.max(0.0).powf(0.8));
        let behind = -Vec3::new(yaw.cos(), 0.0, -yaw.sin()) * sense;
        hand + Vec3::new(yaw.sin() * reach * u, height * u, yaw.cos() * reach * u) + behind * (bow * 4.0 * u * (1.0 - u))
            - Vec3::Y * (sag * 4.0 * u * (1.0 - u))
    };
    for k in 0..CHAIN_LINKS {
        let u = (k as f32 + 0.5) / CHAIN_LINKS as f32;
        let sway = (t * 6.0 + k as f32 * 0.9).sin() * 0.05 * slack * (u * (1.0 - u) * 4.0);
        let p = curve(u) + side * sway;
        let along = (curve(u + 0.05) - curve(u - 0.05)).normalize_or(Vec3::Y);
        // (turned about the chain from a steady side, not the vertical: a chain going straight up
        // in a slam didn't twist about)
        let x = (side - along * side.dot(along)).try_normalize().unwrap_or(Vec3::X);
        let rot = Quat::from_mat3(&Mat3::from_cols(x, along, x.cross(along))).normalize();
        // (on a long hook the chain is drawn as a line, hud.rs: four pairs of links can't span it)
        let links = if hooked.is_some() && span.length() > CHAIN_LINKS as f32 * PAIR_LEN * 2.1 { 0.0001 } else { links };
        // (taut: each link pulled long and thin)
        let pull = ((1.0 - slack - 0.8) / 0.2).clamp(0.0, 1.0) * LINK_STRETCH;
        parts[CHAIN + k] = PartPose { rot, pos: p, scale: links, stretch: Vec3::new(1.0 - pull * 0.4, 1.0 + pull, 1.0 - pull * 0.4) };
    }
    // (on the hook the hook's own catch hits, not its body)
    if hooked.is_some() {
        return None;
    }
    Some(Swing { grip, head: now, prev: if scripted { prev } else { now }, scale: HIT_SCALE })
}

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.pos = None;
    s.last_centre = None;
    s.step = 0.0;
    s.squash = 0;
    s.spin = 0.0;
    s.round = None;
}
