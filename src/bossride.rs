//! The boss ride (Monster Hunter's wirebug mount): hooked to a big enemy with R2 held, A jumps
//! Mario onto its back. Up there the stick steers it (it's pushed along where it points, turned
//! that way), R2 let go or A jumps off; driven into a wall at speed it crashes there, staggered and
//! left open (its stance broken: grab its tail). A few seconds at most, and a long wait after.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// how long a ride lasts at most (s), the wait before the next (s)
const TIME: f32 = 4.0;
const RECHARGE: f32 = 40.0;
/// how fast it's driven (m/s), how quickly it turns that way (rad/s), how fast into a wall counts
/// as a crash (m/s: it built up speed)
const DRIVE: f32 = 5.5;
const TURN: f32 = 2.5;
const CRASH_SPEED: f32 = 3.5;
/// the stance a crash cracks (swing.rs's meter, as poise: about 70% of a 100-poise boss's, two
/// crashes open it), and the ride it takes first (s: a run-up, not a nudge into the wall)
const CRASH_STANCE: f32 = 70.0;
const CRASH_AFTER: f32 = 1.0;

pub const ACT_ER_BOSS_RIDE: u32 = 0x0000_035C;
const ACT_DOUBLE_JUMP: u32 = 0x0300_0881;

const SOUND_RIDE: i32 = 0x2404_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_CRASH: i32 = 0x312F_0081;

struct Ride {
    mob: FieldInsHandle,
    since: Instant,
    speed: f32,
}

struct State {
    ride: Option<Ride>,
    ready_at: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { ride: None, ready_at: None });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Mario is up on a big one.
pub fn active() -> bool {
    state().ride.is_some()
}

/// The enemy he rides: where Mario sits on it (Elden Ring: on top of its body, a little behind).
fn seat(h: &FieldInsHandle) -> Option<(Vec3, Vec3, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let ph = &chr.modules.physics;
    let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let o = ph.orientation;
    let fwd = Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(Vec3::new(0.0, 0.0, -1.0)).with_y(0.0).normalize_or(Vec3::Z);
    let h = ph.hit_height.max(ph.chr_hit_height).clamp(1.0, 12.0);
    let r = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.3, 6.0);
    Some((p + Vec3::Y * h * 0.92 - fwd * r * 0.25, fwd, r))
}

/// Game thread (hook.rs): A while hooked to a big one with R2 held: up he jumps. True if he's on.
pub fn mount(h: &FieldInsHandle, id: i32) -> bool {
    let mut s = state();
    let ready = s.ready_at.is_none_or(|t| Instant::now() >= t) && s.ride.is_none();
    // (co-op guest: the host owns the enemies; a boss in a scripted moment isn't ridden)
    if !ready || crate::coop::guest() || combat::hands_off(h) || seat(h).is_none() {
        drop(s);
        worker::call("ride buzz", |_| unsafe { sm64::sm64_play_sound_global(SOUND_BUZZ) });
        return false;
    }
    s.ride = Some(Ride { mob: *h, since: Instant::now(), speed: 0.0 });
    drop(s);
    log("boss ride: on its back");
    crate::style::callout("RIDE!", None, [1.0, 0.8, 0.35], true, 3.0);
    worker::call("boss ride", move |_| unsafe {
        sm64::sm64_set_mario_action(id, ACT_ER_BOSS_RIDE);
        sm64::sm64_play_sound_global(SOUND_RIDE);
    });
    true
}

/// Game thread, every frame while he rides: `steer` the stick's way (Elden Ring, horizontal, its
/// length 0-1), `hold` R2 held, `jump` A pressed, SM64's origin. Moves the enemy along, keeps
/// Mario on its back (his SM64 position, for the next tick), crashes it into a wall.
#[allow(clippy::too_many_arguments)]
pub fn update(dt: f32, steer: Vec3, hold: bool, jump: bool, action: u32, id: i32, origin: [f32; 3], combat: &mut Combat, player: &eldenring::cs::ChrIns, tick: u32, ray: impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    let mut s = state();
    // (knocked off it: the ride's over, he isn't pinned to its back)
    if s.ride.as_ref().is_some_and(|r| action != ACT_ER_BOSS_RIDE && r.since.elapsed().as_secs_f32() > 0.15) {
        s.ride = None;
        s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
        drop(s);
        log("boss ride: knocked off");
        return;
    }
    let Some(r) = s.ride.as_mut() else { return };
    let Some((seat_at, fwd, radius)) = seat(&r.mob) else {
        drop(s);
        off(id, "it's gone");
        return;
    };
    if !hold || jump || r.since.elapsed().as_secs_f32() > TIME {
        drop(s);
        off(id, if jump { "jumped off" } else { "let go" });
        return;
    }
    // driven where the stick points: turned that way, pushed along
    let dt = dt.min(0.05);
    let want = steer.with_y(0.0);
    let mob = r.mob;
    let mut crashed = None;
    if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(&mob)) {
        let ph = &mut chr.modules.physics;
        let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
        if want.length() > 0.2 {
            let dir = want.normalize();
            let turn = fwd.angle_between(dir).min(TURN * dt) * fwd.cross(dir).y.signum();
            let o = ph.orientation;
            let q = (Quat::from_rotation_y(turn) * Quat::from_xyzw(o.0, o.1, o.2, o.3)).normalize();
            ph.orientation = eldenring::rotation::Quaternion(q.x, q.y, q.z, q.w);
            r.speed = (r.speed + DRIVE * 2.0 * dt).min(DRIVE);
            let next = p + dir * r.speed * dt;
            // into a wall: it crashes there at speed, else it just stops
            match ray(p + Vec3::Y * 1.0, next + dir * radius + Vec3::Y * 1.0) {
                Some(_) if r.speed >= CRASH_SPEED && r.since.elapsed().as_secs_f32() >= CRASH_AFTER => crashed = Some(p + dir * radius + Vec3::Y * 1.2),
                Some(_) => r.speed = 0.0,
                None => {
                    // (along the ground: the floor where it's going)
                    let ground = ray(next + Vec3::Y * 2.0, next - Vec3::Y * 3.0).unwrap_or(next);
                    ph.position = HavokPosition(next.x, ground.y, next.z, 0.0);
                    ph.chr_proxy_pos_update_requested = true;
                }
            }
        } else {
            r.speed = (r.speed - DRIVE * 3.0 * dt).max(0.0);
        }
    }
    drop(s);
    // Mario rides on its back
    let sm = crate::collision::er_to_sm(origin, &HavokPosition(seat_at.x, seat_at.y, seat_at.z, 0.0));
    let yaw = (-fwd.x).atan2(fwd.z);
    worker::call("ride seat", move |_| unsafe {
        sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]);
        sm64::sm64_set_mario_faceangle(id, yaw);
    });
    if let Some(at) = crashed {
        crash(&mob, at, origin, combat, player, tick);
        off(id, "crashed it into a wall");
    }
}

/// Off its back: a hop off, the wait starts.
fn off(id: i32, why: &str) {
    let mut s = state();
    if s.ride.take().is_none() {
        return;
    }
    s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
    drop(s);
    log(format!("boss ride: off ({why})"));
    worker::call("ride off", move |_| unsafe {
        sm64::sm64_set_mario_action(id, ACT_DOUBLE_JUMP);
        sm64::sm64_set_mario_velocity(id, 0.0, 44.0, 0.0);
    });
}

/// Driven into a wall: a heavy blow, its stance broken (swing.rs: open for the tail grab).
fn crash(mob: &FieldInsHandle, at: Vec3, origin: [f32; 3], combat: &mut Combat, player: &eldenring::cs::ChrIns, tick: u32) {
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), 8.0, origin);
    if let Some(i) = near.iter().position(|t| t.shootable().is_some_and(|h| combat::handle_key(h) == combat::handle_key(mob))) {
        combat.deal(player, &near, &[(i, Attack::RideCrash, false)], tick);
    }
    crate::swing::add_stance(mob, CRASH_STANCE);
    log("boss ride: crashed into a wall, left open");
    crate::style::callout("WALL SLAM!", Some(at + Vec3::Y), [1.0, 0.6, 0.3], true, 6.0);
    crate::impact::tether(at);
    crate::lakitu::shake(1.4);
    worker::call("ride crash", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_CRASH, 0.8) });
}

/// Mario's death, out of Mario mode: off at once.
pub fn clear() {
    state().ride = None;
}
