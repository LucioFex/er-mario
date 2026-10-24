//! Mario's hammer (L1): a big, solid, dark mallet in his right hand (Mario & Luigi's, Donkey
//! Kong's), a mesh of its own in his model (assets/model.rs) on a bone of its own, shown only while
//! it's out. Its swings come from libsm64 (er_moves.c): during them it points where the move says
//! (degrees around Mario's facing and up from level); otherwise it's slung across his back, following
//! his body (on his shoulder it looked afloat), and it comes to his hand quickly for a swing. Its
//! head is what the swings hit with (combat.rs).

use std::sync::Mutex;
use std::time::Instant;

use glam::{Mat3, Quat, Vec3};

use crate::engine_mario::{HAMMER, PARTS, PartPose};
use crate::sm64;

/// SM64 parts: the pelvis, the head, the left upper arm, the right upper arm and hand
const PELVIS: usize = 1;
const HEAD: usize = 3;
const L_UPPER_ARM: usize = 4;
const R_UPPER_ARM: usize = 7;
const R_HAND: usize = 9;
/// the head's centre from the grip, along the handle (m, at scale 1; assets/model.rs): Donkey
/// Kong's, near as tall as Mario
pub const HEAD_AT: f32 = 1.0;
/// how far the hand reaches from the shoulder when a swing places the hammer (m)
const ARM: f32 = 0.22;
/// slung on his back, across it: the grip by his right hip (up from the pelvis, out to the side,
/// out behind), the handle leaning over to his left shoulder and a little away from his back (the
/// head is fat), all in metres
const BACK_GRIP: (f32, f32, f32) = (0.02, 0.1, 0.32);
const BACK_LEAN: f32 = 0.55;
const BACK_TILT: f32 = 0.16;
/// in his hand for a swing: how fast it gets there and goes back (share of the way per tick), and
/// how long it stays after one (ticks: a string's swings don't bounce it back between them)
const TO_HAND: f32 = 0.45;
const TO_BACK: f32 = 0.22;
const HOLD: u32 = 8;
/// held between swings: the handle up and a little forward and out, settling there
const READY_SETTLE: f32 = 0.3;
/// its squash and stretch (a cartoon's): swung fast it stretches out along the handle and smears
/// along the way its head moves (at this head speed, m per tick, all the way), and a heavy blow
/// squashes it flat for a few ticks
const STRETCH_SPEED: f32 = 0.45;
const STRETCH_HANDLE: f32 = 0.2;
const STRETCH_SMEAR: f32 = 0.4;
const SQUASH_TICKS: u32 = 5;
/// it pops out, and shrinks away (s)
const POP_TIME: f32 = 0.22;
const AWAY_TIME: f32 = 0.12;
/// a new move that points it this far (degrees) from where it was: it gets there over a few
/// ticks, this share of the way each (it jumped across in one); and the head's face turns at most
/// this much a tick (it flipped over at once when the swing turned back)
const JUMP_ANGLE: f32 = 50.0;
const JUMP_TICKS: u32 = 2;
const JUMP_SHARE: f32 = 0.6;
const ACROSS_TURN: f32 = 60.0;

struct State {
    out: bool,
    since: Instant,
    /// the head's last direction across the handle (it faces where it moves)
    across: Vec3,
    /// the head's last position (SM64), for the swept hit
    head: Option<Vec3>,
    /// the handle's last direction in his hand (settling after a swing)
    dir: Vec3,
    /// 0 on his back .. 1 in his hand, and the ticks since the last swing
    in_hand: f32,
    idle: u32,
    /// ticks of squash left (a heavy blow), and the stretch now (eased)
    squash: u32,
    stretch: Vec3,
    /// Mario's action last tick, and the ticks left easing a jump in where it points
    action: u32,
    easing: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    // (put away long ago: it doesn't shrink away on the first tick)
    let since = Instant::now().checked_sub(std::time::Duration::from_secs(1)).unwrap_or_else(Instant::now);
    f(s.get_or_insert_with(|| State { out: false, since, across: Vec3::Z, head: None, dir: Vec3::Y, in_hand: 0.0, idle: HOLD, squash: 0, stretch: Vec3::ONE, action: 0, easing: 0 }))
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
    /// the grip the tick before, and whether its whole length cuts (the sword's blade: combat.rs
    /// Reach::Blade) rather than its head alone
    pub prev_grip: Vec3,
    pub blade: bool,
}

/// SM64 direction -> the part poses' (mirrored) axes, and back
fn mirror(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.y, v.z)
}

/// Worker thread, after each SM64 tick: puts the hammer part in Mario's hand or on his back (or hides it)
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
    // in his hand while he swings it (and a moment after), on his back otherwise
    let t = with(|s| {
        s.idle = if scripted { 0 } else { s.idle.saturating_add(1) };
        let want = if s.idle <= HOLD { 1.0 } else { 0.0 };
        s.in_hand += (want - s.in_hand).clamp(-TO_BACK, TO_HAND);
        s.in_hand
    });
    let t = t * t * (3.0 - 2.0 * t);
    // in his hand: where the move points it, or held ready between swings
    let side = (parts[R_UPPER_ARM].pos - parts[L_UPPER_ARM].pos).normalize_or(fwd.cross(up));
    let (dir, grip) = if scripted {
        let yaw = face + mv.hammer_yaw.to_radians();
        let pitch = mv.hammer_pitch.to_radians();
        let level = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
        let dir = (level * pitch.cos() + up * pitch.sin()).normalize_or(fwd);
        // between his hand and where a straight arm would hold it: the swing's arc stays round
        let reach = parts[R_UPPER_ARM].pos + dir * ARM;
        (dir, hand.pos.lerp(reach, 0.5))
    } else {
        let ready = (up + fwd * 0.35 + side * 0.2).normalize();
        let dir = with(|s| s.dir.lerp(ready, READY_SETTLE).normalize_or(ready));
        (dir, hand.pos)
    };
    // (a new move pointing it far from where it was: eased there)
    let dir = with(|s| {
        if state.action != s.action && scripted && s.dir.angle_between(dir).to_degrees() > JUMP_ANGLE {
            s.easing = JUMP_TICKS;
        }
        s.action = state.action;
        if s.easing > 0 && scripted {
            s.easing -= 1;
            Quat::IDENTITY.slerp(Quat::from_rotation_arc(s.dir, dir), JUMP_SHARE) * s.dir
        } else {
            s.easing = 0;
            dir
        }
    });
    with(|s| s.dir = dir);
    let scale = size * if scripted { mv.hammer_scale.max(0.1) } else { 1.0 };
    // the head faces where it moves (its flat ends strike), kept across the handle
    let head_now = Vec3::from(state.position) + mirror(grip + dir * HEAD_AT * scale) / crate::scale();
    let across = with(|s| {
        let moved = s.head.map_or(Vec3::ZERO, |h| mirror(head_now - h) * crate::scale());
        let want = s.across + moved * 6.0;
        let across = (want - dir * want.dot(dir)).try_normalize().unwrap_or_else(|| {
            let fallback = fwd.cross(up).cross(dir);
            fallback.try_normalize().unwrap_or(Vec3::X)
        });
        // (turned over at most so far a tick, still across the handle)
        let turned = s.across.angle_between(across).to_degrees();
        if turned > ACROSS_TURN && turned.is_finite() {
            let toward = Quat::IDENTITY.slerp(Quat::from_rotation_arc(s.across, across), ACROSS_TURN / turned) * s.across;
            (toward - dir * toward.dot(dir)).try_normalize().unwrap_or(across)
        } else {
            across
        }
    });
    let held = Quat::from_mat3(&Mat3::from_cols(dir.cross(across), dir, across)).normalize();
    // on his back, following his body (pelvis to head, shoulder to shoulder): the head behind his
    // left shoulder, lying flat across his back
    let spine = (parts[HEAD].pos - parts[PELVIS].pos).normalize_or(up);
    let side_b = (side - spine * side.dot(spine)).normalize_or(fwd.cross(up));
    let mut back = spine.cross(side_b);
    if back.dot(fwd) > 0.0 {
        back = -back;
    }
    let (rise, out, behind) = BACK_GRIP;
    let slung_grip = parts[PELVIS].pos + spine * rise + side_b * out + back * behind;
    let slung_dir = (spine - side_b * BACK_LEAN + back * BACK_TILT).normalize();
    let slung_across = slung_dir.cross(back).normalize_or(side_b);
    let slung = Quat::from_mat3(&Mat3::from_cols(slung_dir.cross(slung_across), slung_dir, slung_across)).normalize();
    let rot = slung.slerp(held, t).normalize();
    let grip = slung_grip.lerp(grip, t);
    let scale = size + (scale - size) * t;
    let dir = rot * Vec3::Y;
    with(|s| s.across = rot * Vec3::Z);
    let to_sm64 = |rel: Vec3| Vec3::from(state.position) + mirror(rel) / crate::scale();
    let head = to_sm64(grip + dir * HEAD_AT * scale);
    let prev = with(|s| s.head.replace(head)).unwrap_or(head);
    // squash and stretch (its own axes: X across the head's face, Y along the handle, Z the way the
    // head strikes)
    let speed = (head - prev).length() * crate::scale() / scale.max(0.1);
    let heavy = scripted && mv.fx & (crate::moves::FX_GROUND | crate::moves::FX_SLAM | crate::moves::FX_QUAKE | crate::moves::FX_GIANT | crate::moves::FX_BANG) != 0;
    let stretch = with(|s| {
        if heavy {
            s.squash = SQUASH_TICKS;
        }
        let want = if s.squash > 0 {
            s.squash -= 1;
            let p = s.squash as f32 / SQUASH_TICKS as f32;
            // (flattened against what it hit: short the way it struck, wide across)
            Vec3::new(1.0 + 0.25 * p, 1.0 + 0.1 * p, 1.0 - 0.3 * p)
        } else {
            let k = if scripted { (speed / STRETCH_SPEED).clamp(0.0, 1.0) } else { 0.0 };
            let (y, z) = (1.0 + STRETCH_HANDLE * k, 1.0 + STRETCH_SMEAR * k);
            Vec3::new(1.0 / (y * z).sqrt(), y, z)
        };
        // (a squash snaps in, a stretch eases)
        s.stretch = if heavy { want } else { s.stretch.lerp(want, 0.6) };
        s.stretch
    });
    parts[HAMMER] = PartPose { rot, pos: grip, scale, stretch };
    Some(Swing { grip: to_sm64(grip), head, prev, scale, prev_grip: to_sm64(grip), blade: false })
}
