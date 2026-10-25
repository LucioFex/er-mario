//! The Star Sword (v4 Part 2): the kit's last thing in the gauntlets (assets/kit.rs). Across
//! Mario's back with its hilt over his right shoulder while he walks; drawn with a flourish (a turn
//! over his hand) at its first blow, held where its moves point it (script.rs), back after a moment.
//! Its blade (grip to tip, now and the tick before) is what its blows reach (combat.rs).

use std::sync::Mutex;
use std::time::Instant;

use glam::{Mat3, Quat, Vec3};

use crate::engine_mario::{PartPose, KIT_SWORD, PARTS};
use crate::sm64;

const PELVIS: usize = 1;
const HEAD: usize = 3;
const L_UPPER_ARM: usize = 4;
const R_UPPER_ARM: usize = 7;
const R_HAND: usize = 9;

/// on his back: the grip this far up his spine from the shoulder and behind him (m); the blade
/// lies in the plane of his back, down toward his left at this angle from level (a 1.80 m
/// greatsword on a 1.25 m Mario: steeper and its tip would be in the ground; it points at the
/// camera if it sticks out behind)
const BACK_UP: f32 = 0.25;
const BACK_OUT: f32 = 0.13;
const BACK_DROP: f32 = 35.0;
/// to his hand and back (share a tick: the draw takes ~5 ticks, a flourish of a turn in it)
const TO_HAND: f32 = 0.22;
const TO_BACK: f32 = 0.18;
const FLOURISH: f32 = 360.0;
/// ticks it stays in hand after its last blow
const HOLD: u32 = 10;
/// held between blows: down and out ahead of him (degrees round his facing, up from level)
pub const READY: (f32, f32, f32) = (-25.0, -15.0, 0.0);
/// in a move the blade turns this far a tick at most (degrees): its cuts' fastest (the JUST's and
/// the Flurry Rush's flicks, swordmoves.rs), so it never lags its own cut; a jump from one move's
/// pose to the next's still takes a couple of ticks
pub const MOVE_TURN: f32 = 115.0;
/// the pop when it comes out (s), and the shrink when it goes
const POP_TIME: f32 = 0.22;
const AWAY_TIME: f32 = 0.12;

struct State {
    out: bool,
    since: Instant,
    /// 0 on his back .. 1 in his hand, the ticks since its last blow
    in_hand: f32,
    idle: u32,
    /// where it was held last (blade direction, across it), to settle between blows
    dir: Vec3,
    across: Vec3,
    /// its grip and tip last tick (SM64), for the swept blade
    grip: Option<Vec3>,
    tip: Option<Vec3>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let since = Instant::now().checked_sub(std::time::Duration::from_secs(1)).unwrap_or_else(Instant::now);
    f(s.get_or_insert_with(|| State { out: false, since, in_hand: 0.0, idle: HOLD, dir: Vec3::Y, across: Vec3::Z, grip: None, tip: None }))
}

/// L1: the sword comes out (on his back) or goes away.
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
            if p < 0.6 { p / 0.6 * 1.15 } else { 1.15 - 0.15 * (p - 0.6) / 0.4 }
        } else {
            (1.0 - t / AWAY_TIME).max(0.0)
        }
    })
}

/// SM64 direction -> the part poses' (mirrored) axes, and back
fn mirror(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.y, v.z)
}

/// The blade's direction held `yaw` degrees round his facing `face` (SM64 radians, + to his left)
/// and `pitch` up from level, in the parts' axes.
pub fn held_dir(face: f32, yaw: f32, pitch: f32) -> Vec3 {
    let y = face + yaw.to_radians();
    let p = pitch.to_radians();
    Vec3::new(-y.sin() * p.cos(), p.sin(), y.cos() * p.cos())
}

/// With the sword in hand X is its moves' (swordplay.rs), never SM64's punches, kicks and dives:
/// on his feet, in the air and in its moves. SM64 keeps its B where it's something else (a stroke
/// in the water, letting go of a pole or a ledge, a cutscene, a shell).
pub fn owns_b(action: u32) -> bool {
    // (the Rock Mushroom's boulder rams on B, whatever is in hand)
    if action == crate::rock::ACT_ER_ROCK {
        return false;
    }
    const GROUP_MASK: u32 = 0x1C0;
    const SWIMMING: u32 = 0x2000;
    const RIDING_SHELL: u32 = 0x0001_0000;
    const ON_POLE: u32 = 0x0010_0000;
    const HANGING: u32 = 0x0020_0000;
    // (stationary, moving, airborne, object: the script actions are object and airborne ones)
    matches!(action & GROUP_MASK, 0x000 | 0x040 | 0x080 | 0x180) && action & (SWIMMING | RIDING_SHELL | ON_POLE | HANGING) == 0
}

/// the hand on a blow: out along the blade's way (its sweep a little narrower, a little low) at
/// this much of the arm's length
const ARM_YAW: f32 = 0.8;
const ARM_PITCH: f32 = 0.8;
const ARM_DROP: f32 = -12.0;
const ARM_OUT: f32 = 0.92;

/// Where the right hand goes when a move points the blade (`blade`: yaw, pitch, roll degrees round
/// his facing `face`; his body drawn turned `twist` degrees): out from the shoulder along it, the
/// arm's reach `arm` (parts' axes). (Its sweep narrower than the blade's round his body, however
/// many turns a spin has put in both.)
pub fn hand_goal(shoulder: Vec3, arm: f32, face: f32, blade: (f32, f32, f32), twist: f32) -> Vec3 {
    let off = (blade.0 - twist + 180.0).rem_euclid(360.0) - 180.0;
    shoulder + held_dir(face, twist + off * ARM_YAW, blade.1 * ARM_PITCH + ARM_DROP) * arm * ARM_OUT
}

/// Across the blade (its edge) `roll` degrees round it from level.
fn across_of(dir: Vec3, roll: f32) -> Vec3 {
    let level = Vec3::Y.cross(dir).try_normalize().unwrap_or(Vec3::X);
    Quat::from_axis_angle(dir, roll.to_radians()) * level
}

/// On his back (parts' axes, relative to him): the grip over his right shoulder, the blade slung
/// down across it toward his left (BACK_DROP), its flat toward his back. Grip, direction, across.
fn slung(parts: &[PartPose; PARTS], fwd: Vec3) -> (Vec3, Vec3, Vec3) {
    let spine = (parts[HEAD].pos - parts[PELVIS].pos).normalize_or(Vec3::Y);
    let side = (parts[R_UPPER_ARM].pos - parts[L_UPPER_ARM].pos).normalize_or(fwd.cross(Vec3::Y));
    let side = (side - spine * side.dot(spine)).normalize_or(fwd.cross(Vec3::Y));
    let mut back = spine.cross(side);
    if back.dot(fwd) > 0.0 {
        back = -back;
    }
    let grip = parts[R_UPPER_ARM].pos + spine * BACK_UP + back * BACK_OUT;
    let drop = BACK_DROP.to_radians();
    let dir = (-side * drop.cos() - spine * drop.sin()).normalize();
    let across = dir.cross(back).normalize_or(side);
    (grip, dir, across)
}

/// `from` turned toward `to` (unit directions) by at most `max` radians.
fn turn_toward(from: Vec3, to: Vec3, max: f32) -> Vec3 {
    let angle = from.angle_between(to);
    if angle <= max || angle.is_nan() {
        return to;
    }
    // (straight back: round any side)
    let axis = from.cross(to).try_normalize().unwrap_or_else(|| from.any_orthonormal_vector());
    (Quat::from_axis_angle(axis, max) * from).normalize()
}

fn rot_of(dir: Vec3, across: Vec3) -> Quat {
    let across = (across - dir * across.dot(dir)).normalize_or(dir.any_orthonormal_vector());
    Quat::from_mat3(&Mat3::from_cols(dir.cross(across), dir, across)).normalize()
}

/// Worker thread, after each SM64 tick: puts the sword's part on his back or in his hand (`blade`:
/// where the move playing points it, degrees yaw / pitch / roll round his facing; None between
/// blows) and returns where it is (SM64 units) for its blows. `visible`: his hands are free.
pub fn pose(parts: &mut [PartPose; PARTS], state: &sm64::SM64MarioState, blade: Option<(f32, f32, f32)>, visible: bool) -> Option<crate::hammer::Swing> {
    let hand = parts[R_HAND];
    let size = if visible { shown() } else { 0.0 };
    if size <= 0.01 {
        parts[KIT_SWORD] = PartPose { scale: 0.01, ..hand };
        with(|s| {
            s.grip = None;
            s.tip = None;
        });
        return None;
    }
    let face = state.face_angle;
    let fwd = Vec3::new(-face.sin(), 0.0, face.cos());
    let (t, drawing) = with(|s| {
        s.idle = if blade.is_some() { 0 } else { s.idle.saturating_add(1) };
        let want = if s.idle <= HOLD { 1.0 } else { 0.0 };
        let was = s.in_hand;
        s.in_hand += (want - s.in_hand).clamp(-TO_BACK, TO_HAND);
        (s.in_hand, s.in_hand > was)
    });
    let e = t * t * (3.0 - 2.0 * t);
    // in his hand: where the move points it, or held ready between blows (settling there)
    let (yaw, pitch, roll) = blade.unwrap_or(READY);
    let want_dir = held_dir(face, yaw, pitch);
    let want_across = across_of(want_dir, roll);
    let (dir, across) = with(|s| {
        if blade.is_none() {
            s.dir = s.dir.lerp(want_dir, 0.35).normalize_or(want_dir);
            s.across = s.across.lerp(want_across, 0.35).normalize_or(want_across);
        } else {
            // (as fast as a cut goes, no faster: one move into the next doesn't pop)
            let dir = turn_toward(s.dir, want_dir, MOVE_TURN.to_radians());
            let carried = Quat::from_rotation_arc(s.dir, dir) * s.across;
            s.dir = dir;
            s.across = carried.lerp(want_across, 0.5).normalize_or(want_across);
        }
        (s.dir, s.across)
    });
    let mut held = rot_of(dir, across);
    // (drawn: a turn over his hand on the way)
    if drawing && t < 1.0 {
        held = (Quat::from_axis_angle(dir, (FLOURISH * (1.0 - e)).to_radians()) * held).normalize();
    }
    let (back_grip, back_dir, back_across) = slung(parts, fwd);
    let rot = rot_of(back_dir, back_across).slerp(held, e).normalize();
    let grip = back_grip.lerp(hand.pos, e);
    parts[KIT_SWORD] = PartPose { rot, pos: grip, scale: size, stretch: Vec3::ONE };
    let to_sm64 = |rel: Vec3| Vec3::from(state.position) + mirror(rel) / crate::scale();
    let (grip_sm, tip_sm) = (to_sm64(grip), to_sm64(grip + rot * Vec3::Y * crate::assets::kit::SWORD_TIP * size));
    let (prev_grip, prev) = with(|s| (s.grip.replace(grip_sm).unwrap_or(grip_sm), s.tip.replace(tip_sm).unwrap_or(tip_sm)));
    Some(crate::hammer::Swing { grip: grip_sm, head: tip_sm, prev, scale: size, prev_grip, blade: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_where_the_move_points_it() {
        assert!(held_dir(0.0, 0.0, 0.0).distance(Vec3::Z) < 1e-5);
        assert!(held_dir(0.0, 0.0, 90.0).distance(Vec3::Y) < 1e-5);
        assert!(held_dir(0.0, 90.0, 0.0).dot(Vec3::Z).abs() < 1e-5);
        // (turned with his facing)
        let face = 1.0f32;
        assert!(held_dir(face, 0.0, 0.0).distance(Vec3::new(-face.sin(), 0.0, face.cos())) < 1e-5);
        // the edge across the blade, rolled round it
        let d = held_dir(0.0, 0.0, 0.0);
        let a = across_of(d, 90.0);
        assert!(a.dot(d).abs() < 1e-5 && (a.length() - 1.0).abs() < 1e-5);
        assert!(a.dot(across_of(d, 0.0)).abs() < 1e-5);
    }

    #[test]
    fn x_is_the_swords_on_foot_and_in_the_air() {
        const ACT_IDLE: u32 = 0x0C40_0201;
        const ACT_WALKING: u32 = 0x0400_0440;
        const ACT_JUMP: u32 = 0x0300_0880;
        const ACT_ER_DASH: u32 = 0x0002_047B;
        for a in [ACT_IDLE, ACT_WALKING, ACT_JUMP, ACT_ER_DASH, crate::script::ACT_ER_SCRIPT, crate::script::ACT_ER_SCRIPT_AIR] {
            assert!(owns_b(a), "{a:#x}");
        }
        // (SM64 keeps its B: a stroke in the water, on a pole, on a ledge, in a cutscene, on a shell)
        const ACT_WATER_IDLE: u32 = 0x3800_22C0;
        const ACT_HOLDING_POLE: u32 = 0x0810_0340;
        const ACT_LEDGE_GRAB: u32 = 0x0800_034B;
        const ACT_READING_SIGN: u32 = 0x0000_1308;
        const ACT_RIDING_SHELL_GROUND: u32 = 0x2081_0446;
        // (and the Rock Mushroom's boulder: its ram)
        for a in [ACT_WATER_IDLE, ACT_HOLDING_POLE, ACT_LEDGE_GRAB, ACT_READING_SIGN, ACT_RIDING_SHELL_GROUND, crate::rock::ACT_ER_ROCK] {
            assert!(!owns_b(a), "{a:#x}");
        }
    }

    #[test]
    fn the_hand_follows_the_blade() {
        let (shoulder, arm) = (Vec3::new(-0.15, 0.9, 0.0), 0.5);
        for face in [0.0f32, 1.2] {
            // a thrust: straight out ahead, a little low, nearly the arm's length
            let g = hand_goal(shoulder, arm, face, (0.0, 0.0, 0.0), 0.0) - shoulder;
            assert!(g.normalize().dot(held_dir(face, 0.0, 0.0)) > 0.95, "{g:?}");
            assert!(g.y < 0.0 && (g.length() - arm * 0.92).abs() < 1e-3);
            // overhead: up over his shoulder
            assert!(hand_goal(shoulder, arm, face, (0.0, 90.0, 0.0), 0.0).y > shoulder.y + 0.6 * arm);
            // a cut across to his left: the hand goes over there too (less far than the blade)
            let g = hand_goal(shoulder, arm, face, (80.0, 0.0, 0.0), 0.0) - shoulder;
            assert!(g.normalize().dot(held_dir(face, 90.0, 0.0)) > 0.5);
            assert!(g.normalize().dot(held_dir(face, 0.0, 0.0)) > 0.0);
            // in a spin (his body drawn turned round and round): the hand by the blade, whatever the
            // turns
            let g = hand_goal(shoulder, arm, face, (540.0, 0.0, 0.0), 450.0) - shoulder;
            assert!(g.normalize().dot(held_dir(face, 180.0, -12.0)) > 0.9, "{g:?}");
        }
    }

    #[test]
    fn the_blade_turns_at_most_so_far_a_tick() {
        let max = 75f32.to_radians();
        // (a small turn: all of it)
        let to = Vec3::new(0.0, 0.3, 1.0).normalize();
        assert!(turn_toward(Vec3::Z, to, max).distance(to) < 1e-5);
        // (a big one: that far toward it, in the same plane)
        let got = turn_toward(Vec3::Z, Vec3::X, 30f32.to_radians());
        assert!((got.angle_between(Vec3::Z) - 30f32.to_radians()).abs() < 1e-4 && got.y.abs() < 1e-5 && got.x > 0.0);
        // (straight back: some way round, not stuck)
        let got = turn_toward(Vec3::Z, Vec3::NEG_Z, max);
        assert!((got.angle_between(Vec3::Z) - max).abs() < 1e-3);
    }

    #[test]
    fn slung_down_across_his_back() {
        let mut p = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
        p[PELVIS].pos = Vec3::new(0.0, 0.45, 0.0);
        p[HEAD].pos = Vec3::new(0.0, 1.1, 0.0);
        p[L_UPPER_ARM].pos = Vec3::new(0.15, 0.9, 0.0);
        p[R_UPPER_ARM].pos = Vec3::new(-0.15, 0.9, 0.0);
        let fwd = Vec3::Z;
        let (grip, dir, across) = slung(&p, fwd);
        // behind him, its hilt up over his right shoulder, pointing down across toward his left
        assert!(grip.z < -0.05 && grip.x < 0.0 && grip.y > 1.05, "{grip:?}");
        assert!(dir.y < -0.5 && dir.z.abs() < 0.1 && dir.x > 0.7, "{dir:?}");
        // (a greatsword lying in the plane of his back: ~35 degrees from level, its tip out to his
        // left over the ground he stands on, no more than 25 cm behind his back plane, the whole
        // blade behind his back and not in him)
        let tip = grip + dir * crate::assets::kit::SWORD_TIP;
        assert!(tip.y > 0.10, "its tip at {tip:?}");
        let rise = (-dir.y).asin().to_degrees();
        assert!(rise > 30.0 && rise < 40.0, "{rise} degrees from level");
        assert!(tip.x > 0.5 && tip.z < -0.02 && tip.z > -0.25, "its tip at {tip:?}");
        for k in 0..=18 {
            let q = grip + dir * (0.1 * k as f32);
            assert!(q.z < -0.04 && q.z > -0.25 && q.y > 0.05, "{q:?} in his body or the ground");
        }
        // its flat toward his back
        let flat = dir.cross(across.normalize());
        assert!(flat.z.abs() > 0.9, "{flat:?}");
    }
}
