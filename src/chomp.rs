//! Mario's Chain Chomp (weapons.rs): held on its chain, it lunges, sweeps and crashes where
//! er_moves.c's moves put it (degrees around his facing, how far out, how high), and its body is
//! what they hit with (combat.rs, as the hammer's head). Between blows it hops along beside him,
//! straining at the chain. Drawn by hud.rs: the ball with its eye and teeth, the chain's links from
//! his hand.

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

use crate::engine_mario::{PARTS, PartPose};
use crate::hammer::Swing;
use crate::sm64;

/// SM64 parts: the right hand
const R_HAND: usize = 9;
/// its reach for the hits: the hammer head's reach times this (combat.rs)
const HIT_SCALE: f32 = 0.7;
/// beside him between blows: degrees around his facing (behind, to his right), how far (SM64
/// units), how fast it gets there (share of the way per tick), its hops (units high, per second)
const REST_YAW: f32 = -140.0;
const REST_REACH: f32 = 120.0;
const FOLLOW: f32 = 0.3;
const HOP: f32 = 30.0;
const HOP_RATE: f32 = 7.0;

struct State {
    out: bool,
    /// the Chomp's last place (SM64), and whether a move held it
    pos: Option<Vec3>,
    scripted: bool,
    born: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { out: false, pos: None, scripted: false, born: None });

/// The Chain Chomp is in hand or not (weapons.rs).
pub fn set_out(out: bool) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.out != out {
        s.out = out;
        s.pos = None;
        s.born = Some(Instant::now());
    }
}

/// Worker thread, after each SM64 tick: where the Chomp is now (SM64 units) and its sweep for the
/// hits, if it's out. `parts`: his part poses (the hand holds the chain).
pub fn pose(parts: &[PartPose; PARTS], state: &sm64::SM64MarioState, mv: &sm64::SM64ErMove, visible: bool) -> Option<Swing> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.out || !visible {
        s.pos = None;
        return None;
    }
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
    Some(Swing { grip, head: now, prev: if scripted { prev } else { now }, scale: HIT_SCALE })
}

/// What hud.rs draws: the Chomp (Elden Ring), his hand, its mouth open (0-1).
#[derive(Clone, Copy)]
pub struct Sprite {
    pub chomp: Vec3,
    pub hand: Vec3,
    pub bite: f32,
    pub vel: Vec3,
}

static SPRITE: Mutex<Option<(Sprite, Instant)>> = Mutex::new(None);

/// Game thread, after each tick: where to draw it (None: not out).
pub fn publish(sprite: Option<Sprite>) {
    let mut s = SPRITE.lock().unwrap_or_else(|e| e.into_inner());
    // (velocity from the last tick's place: drawn moving on between ticks)
    let sprite = sprite.map(|mut n| {
        if let Some((old, t)) = *s {
            let dt = t.elapsed().as_secs_f32().max(1.0 / 60.0);
            n.vel = ((n.chomp - old.chomp) / dt).clamp_length_max(40.0);
        }
        n
    });
    *s = sprite.map(|n| (n, Instant::now()));
}

/// The Chomp to draw now.
pub fn sprite() -> Option<Sprite> {
    let s = SPRITE.lock().unwrap_or_else(|e| e.into_inner());
    let (n, t) = (*s)?;
    let dt = t.elapsed().as_secs_f32();
    (dt < 0.25).then(|| Sprite { chomp: n.chomp + n.vel * dt.min(1.0 / 30.0), ..n })
}

pub fn clear() {
    *SPRITE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    STATE.lock().unwrap_or_else(|e| e.into_inner()).pos = None;
}
