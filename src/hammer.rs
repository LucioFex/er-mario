//! Mario's hammer (L1): a big, solid, dark mallet in his right hand (Mario & Luigi's, Donkey
//! Kong's), a mesh of its own in his model (assets/model.rs) on a bone of its own, shown only while
//! it's out. Its swings come from libsm64 (er_moves.c): during them it points where the move says
//! (degrees around Mario's facing and up from level); otherwise it rests on his shoulder, the head
//! behind him (along his swinging forearm it shook all over the place as he ran). Its head is what
//! the swings hit with (combat.rs).

use std::sync::Mutex;
use std::time::Instant;

use glam::{Mat3, Quat, Vec3};

use crate::engine_mario::{HAMMER, PARTS, PartPose};
use crate::sm64;

/// SM64 parts: the left upper arm, the right upper arm and hand
const L_UPPER_ARM: usize = 4;
const R_UPPER_ARM: usize = 7;
const R_HAND: usize = 9;
/// the head's centre from the grip, along the handle (m, at scale 1; assets/model.rs): a big one,
/// it reaches
pub const HEAD_AT: f32 = 0.8;
/// how far the hand reaches from the shoulder when a swing places the hammer (m)
const ARM: f32 = 0.22;
/// at rest on his shoulder: how much of the way to his hand the grip is, and how fast it settles
/// there after a swing (share of the way per tick)
const REST_TO_HAND: f32 = 0.25;
const REST_SETTLE: f32 = 0.3;
/// it pops out, and shrinks away (s)
const POP_TIME: f32 = 0.22;
const AWAY_TIME: f32 = 0.12;

struct State {
    out: bool,
    since: Instant,
    /// the head's last direction across the handle (it faces where it moves)
    across: Vec3,
    /// the head's last position (SM64), for the swept hit
    head: Option<Vec3>,
    /// the handle's last direction (settling onto the shoulder after a swing)
    dir: Vec3,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    // (put away long ago: it doesn't shrink away on the first tick)
    let since = Instant::now().checked_sub(std::time::Duration::from_secs(1)).unwrap_or_else(Instant::now);
    f(s.get_or_insert_with(|| State { out: false, since, across: Vec3::Z, head: None, dir: Vec3::Y }))
}

/// L1: the hammer comes out or goes away.
pub fn set_out(out: bool) {
    with(|s| {
        if s.out != out {
            s.out = out;
            s.since = Instant::now();
        }
    });
}

/// Its size now (0 = gone): it pops out a little too big and settles.
fn shown() -> f32 {
    with(|s| {
        let t = s.since.elapsed().as_secs_f32();
        if s.out {
            let p = (t / POP_TIME).min(1.0);
            if p < 0.6 { p / 0.6 * 1.2 } else { 1.2 - 0.2 * (p - 0.6) / 0.4 }
        } else {
            (1.0 - t / AWAY_TIME).max(0.0)
        }
    })
}

/// The hammer this tick, in SM64 units: where his hand holds it and where its head is, and the
/// head's position the tick before (the swing's sweep), and its size.
#[derive(Clone, Copy, Debug)]
pub struct Swing {
    pub grip: Vec3,
    pub head: Vec3,
    pub prev: Vec3,
    pub scale: f32,
}

/// SM64 direction -> the part poses' (mirrored) axes, and back
fn mirror(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.y, v.z)
}

/// Worker thread, after each SM64 tick: puts the hammer part in Mario's hand (or hides it there)
/// and returns where it is. `visible`: his hands are free (not riding, holding, swimming); it shows
/// while it's out, or shrinking away.
pub fn pose(parts: &mut [PartPose; PARTS], state: &sm64::SM64MarioState, mv: &sm64::SM64ErMove, visible: bool) -> Option<Swing> {
    let hand = parts[R_HAND];
    let size = if visible { shown() } else { 0.0 };
    if size <= 0.01 {
        parts[HAMMER] = PartPose { scale: 0.01, ..hand };
        with(|s| s.head = None);
        return None;
    }
    let up = Vec3::Y;
    let face = state.face_angle;
    let fwd = Vec3::new(-face.sin(), 0.0, face.cos());
    let scripted = mv.hammer_scripted != 0;
    let (dir, grip) = if scripted {
        let yaw = face + mv.hammer_yaw.to_radians();
        let pitch = mv.hammer_pitch.to_radians();
        let level = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
        let dir = (level * pitch.cos() + up * pitch.sin()).normalize_or(fwd);
        // between his hand and where a straight arm would hold it: the swing's arc stays round
        let reach = parts[R_UPPER_ARM].pos + dir * ARM;
        (dir, hand.pos.lerp(reach, 0.5))
    } else {
        // on his shoulder: the handle up over it and a little out, the head behind him, settling
        // there after a swing; held between the shoulder and his hand
        let side = (parts[R_UPPER_ARM].pos - parts[L_UPPER_ARM].pos).normalize_or(fwd.cross(up));
        let rest = (up - fwd * 0.6 + side * 0.15).normalize();
        let dir = with(|s| s.dir.lerp(rest, REST_SETTLE).normalize_or(rest));
        (dir, parts[R_UPPER_ARM].pos.lerp(hand.pos, REST_TO_HAND) + fwd * 0.06)
    };
    with(|s| s.dir = dir);
    let scale = size * if scripted { mv.hammer_scale.max(0.1) } else { 1.0 };
    let head_rel = grip + dir * HEAD_AT * scale;
    let to_sm64 = |rel: Vec3| Vec3::from(state.position) + mirror(rel) / crate::SCALE;
    let head = to_sm64(head_rel);
    // the head faces where it moves (its flat ends strike), kept across the handle
    let across = with(|s| {
        let moved = s.head.map_or(Vec3::ZERO, |h| mirror(head - h) * crate::SCALE);
        let want = s.across + moved * 6.0;
        let across = (want - dir * want.dot(dir)).try_normalize().unwrap_or_else(|| {
            let fallback = fwd.cross(up).cross(dir);
            fallback.try_normalize().unwrap_or(Vec3::X)
        });
        s.across = across;
        across
    });
    let rot = Quat::from_mat3(&Mat3::from_cols(dir.cross(across), dir, across)).normalize();
    parts[HAMMER] = PartPose { rot, pos: grip, scale };
    let prev = with(|s| s.head.replace(head)).unwrap_or(head);
    Some(Swing { grip: to_sm64(grip), head, prev, scale })
}
