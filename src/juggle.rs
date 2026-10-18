//! Air combos (Devil May Cry's juggle): the super jump punch (X, X, Y) lifts a small enemy with
//! Mario, a regular one not too big, or a boss or strong enemy only while it's stunned (its poise
//! broken, or its stance broken for the tail grab, swing.rs) and not too big either. While Mario's
//! blows keep landing it hangs in front of him; left alone a moment it falls back down (no fall
//! damage: the blows did the work). A few at most at a time. The hammer's launcher (X, Y) sends
//! them flying high up instead, where they hang a moment for Mario to jump after them: his first
//! blow up there catches them and the air string goes on. Falling back down they bounce off the
//! ground once, and a blow then sends them up again.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::log;

/// Biggest enemy lifted (m): a regular one about a soldier's size, a stunned strong one a bit more
const MAX_RADIUS: f32 = 0.9;
const MAX_HEIGHT: f32 = 2.6;
const MAX_RADIUS_STUNNED: f32 = 1.4;
const MAX_HEIGHT_STUNNED: f32 = 3.4;
/// at most this many up at a time
const MAX_UP: usize = 3;
/// it hangs this long after the last blow (s), then falls
const HANG: f32 = 0.9;
/// how far in front of Mario it hangs (m), and how fast it gets there
const NEAR: f32 = 0.6;
const FAR: f32 = 1.3;
const FOLLOW: f32 = 20.0;
/// how fast it swings round to where Mario faces (1/s): it hung where it was caught, and once he
/// turned or drifted on his blows went past it
const FACE: f32 = 6.0;
/// falling back down (m/s²), and the longest fall before the game takes over
const GRAVITY: f32 = 16.0;
const MAX_FALL: f32 = 3.0;
/// the launcher: how high they fly (m) and how far away from Mario, then how long they hang up
/// there, sinking slowly (s, m/s) so that a jump reaches them; Mario's blows only catch them a
/// moment after the launch (s; the launcher's own blow isn't one)
const LAUNCH_HEIGHT: f32 = 4.0;
const LAUNCH_AWAY: f32 = 2.0;
const HOVER: f32 = 2.0;
const SINK: f32 = 0.8;
const CATCH_AFTER: f32 = 0.25;
/// the bounce off the ground (m/s up, and the share of the fall's sideways speed kept), and how
/// high a blow on the bounce sends it again (m)
const BOUNCE_UP: f32 = 4.5;
const BOUNCE_KEEP: f32 = 0.3;
const RELAUNCH_HEIGHT: f32 = 2.6;
/// a shot from afar lifts one hanging up there this much (m)
const SHOT_POP: f32 = 0.18;
/// the hook (hook.rs) drags it in this fast (m/s), to this far in front of Mario (m), for this long
/// at most (s)
const PULL_SPEED: f32 = 24.0;
const PULL_NEAR: f32 = 1.0;
const PULL_MAX: f32 = 1.5;

enum Phase {
    /// up with Mario: where it is, its direction and distance from him, when it was last hit
    Up { dir: Vec3, dist: f32, last_hit: Instant },
    Falling { vel: Vec3, since: Instant },
    /// sent up by the launcher, and hanging at the top of it
    Flying { vel: Vec3, since: Instant },
    Hover { since: Instant },
    /// off the ground after a fall, once
    Bounce { vel: Vec3 },
    /// dragged in by the hook: hung in front of him once there if he's in the air (`air`), set down
    /// otherwise
    Pulled { since: Instant, air: bool, near: f32 },
}

struct Juggled {
    mob: FieldInsHandle,
    pos: Vec3,
    phase: Phase,
}

static UP: Mutex<Vec<Juggled>> = Mutex::new(Vec::new());

fn key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

pub fn is_juggled(h: &FieldInsHandle) -> bool {
    UP.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| key(&j.mob) == key(h))
}

/// Some enemy hangs in the air with Mario, or was launched up there for him (his air blows lift
/// him to stay up, and X goes on with the air string: libsm64's er_juggle).
pub fn active() -> bool {
    UP.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| !matches!(j.phase, Phase::Falling { .. } | Phase::Bounce { .. }))
}

/// Mario `mario` caught `j` up there: it hangs in front of him from now on.
fn catch(j: &mut Juggled, mario: Vec3) {
    let d = Vec3::new(j.pos.x - mario.x, 0.0, j.pos.z - mario.z);
    j.phase = Phase::Up { dir: d.normalize_or(Vec3::Z), dist: d.length().clamp(NEAR, FAR), last_hit: Instant::now() };
}

/// The launcher's start: straight up to `height`, drifting LAUNCH_AWAY from Mario.
fn launched(pos: Vec3, mario: Vec3, height: f32) -> Phase {
    let up = (2.0 * GRAVITY * height).sqrt();
    let away = Vec3::new(pos.x - mario.x, 0.0, pos.z - mario.z).normalize_or(Vec3::Z) * LAUNCH_AWAY / (up / GRAVITY);
    Phase::Flying { vel: away + Vec3::Y * up, since: Instant::now() }
}

/// `refusal` for a boss or strong enemy that isn't stunned
pub const NOT_STUNNED: &str = "a boss, not stunned";

/// Why an enemy this big (m) can't be lifted, if it can't: `regular`, `stunned` as in `try_launch`
pub fn refusal(radius_m: f32, height_m: f32, regular: bool, stunned: bool) -> Option<&'static str> {
    let (max_r, max_h) = if regular { (MAX_RADIUS, MAX_HEIGHT) } else { (MAX_RADIUS_STUNNED, MAX_HEIGHT_STUNNED) };
    if !(regular || stunned) {
        Some(NOT_STUNNED)
    } else if radius_m > max_r {
        Some("too wide")
    } else if height_m > max_h {
        Some("too tall")
    } else {
        None
    }
}

/// The super jump punch hit `h` (combat.rs): lifts it if it may go. `stunned`: a boss or strong
/// enemy that's stunned (a regular one is `regular`); `high`: the hammer's launcher, sending it
/// flying high up. True if it's up now.
pub fn try_launch(h: &FieldInsHandle, mario: Vec3, radius_m: f32, height_m: f32, regular: bool, stunned: bool, high: bool) -> bool {
    // (co-op guest: the host owns the enemies, moving one here would only make it jump)
    if crate::coop::guest() {
        return false;
    }
    if refusal(radius_m, height_m, regular, stunned).is_some() || crate::stopwatch::frozen(h) {
        return false;
    }
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) {
        // (falling already: back up; up with him: a while longer)
        if high {
            j.phase = launched(j.pos, mario, LAUNCH_HEIGHT);
        } else if let Phase::Up { last_hit, .. } = &mut j.phase {
            *last_hit = Instant::now();
        } else if let Phase::Falling { .. } | Phase::Bounce { .. } = j.phase {
            catch(j, mario);
        }
        return true;
    }
    if up.iter().filter(|j| matches!(j.phase, Phase::Up { .. })).count() >= MAX_UP {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(h) else { return false };
    if chr.modules.data.hp <= 0 {
        return false;
    }
    let p = chr.modules.physics.position;
    let pos = Vec3::new(p.0, p.1, p.2);
    let d = Vec3::new(pos.x - mario.x, 0.0, pos.z - mario.z);
    log(format!("juggle: c{:04} {} ({:.1} x {:.1} m{})", chr.character_id, if high { "launched" } else { "up" }, radius_m, height_m, if stunned { ", stunned" } else { "" }));
    if high {
        crate::impact::launched(pos);
    }
    let phase = if high { launched(pos, mario, LAUNCH_HEIGHT) } else { Phase::Up { dir: d.normalize_or(Vec3::Z), dist: d.length().clamp(NEAR, FAR), last_hit: Instant::now() } };
    up.push(Juggled { mob: *h, pos, phase });
    true
}

/// An enemy the hook may drag in (m): a regular one not too big (the others: Mario goes to them).
pub fn draggable(h: &FieldInsHandle, radius_m: f32, height_m: f32) -> bool {
    pull_size(h, radius_m, height_m) != PullSize::Big
}

/// How the hook (with L2) treats an enemy: a small one, a medium one (a stronger or wider regular
/// enemy), a big one (a boss, anything too big to drag).
#[derive(Clone, Copy, PartialEq)]
pub enum PullSize {
    Small,
    Medium,
    Big,
}

const MEDIUM_RADIUS: f32 = 1.4;
const MEDIUM_HEIGHT: f32 = 3.4;

pub fn pull_size(h: &FieldInsHandle, radius_m: f32, height_m: f32) -> PullSize {
    if crate::coop::guest() || !crate::combat::liftable(h) || crate::combat::hands_off(h) {
        PullSize::Big
    } else if radius_m <= MAX_RADIUS && height_m <= MAX_HEIGHT {
        PullSize::Small
    } else if radius_m <= MEDIUM_RADIUS && height_m <= MEDIUM_HEIGHT {
        PullSize::Medium
    } else {
        PullSize::Big
    }
}

/// The hook caught `h` (hook.rs): dragged in to Mario, `air`: he's in the air.
/// True if it's coming.
pub fn pull(h: &FieldInsHandle, air: bool, radius_m: f32, medium: bool) -> bool {
    // (co-op guest: the host owns the enemies, moving one here would only make it jump)
    if crate::coop::guest() {
        return false;
    }
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    // (a wider body stops farther off, not inside him)
    // (a medium one isn't held up in the air: it drops in front of him, else it could be juggled
    // on and on without ever being stunned)
    let phase = Phase::Pulled { since: Instant::now(), air: air && !medium, near: PULL_NEAR + (radius_m - MAX_RADIUS).max(0.0) };
    if let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) {
        j.phase = phase;
        return true;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(h) else { return false };
    if chr.modules.data.hp <= 0 || crate::carry::is_carried(h) || crate::knock::is_flying(h) {
        return false;
    }
    let p = chr.modules.physics.position;
    let pos = Vec3::new(p.0, p.1, p.2);
    log(format!("juggle: c{:04} pulled in{}", chr.character_id, if air { " (up there)" } else { "" }));
    up.push(Juggled { mob: *h, pos, phase });
    true
}

/// `h` is being dragged in by the hook.
pub fn is_pulled(h: &FieldInsHandle) -> bool {
    UP.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| key(&j.mob) == key(h) && matches!(j.phase, Phase::Pulled { .. }))
}

/// A blow of Mario's (at `mario`) landed on `h`: if it's up, it stays up a while longer; launched
/// up there, he caught it.
pub fn hit(h: &FieldInsHandle, mario: Vec3) {
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) else { return };
    let caught = match &mut j.phase {
        Phase::Up { last_hit, .. } => {
            *last_hit = Instant::now();
            false
        }
        Phase::Flying { since, .. } => since.elapsed().as_secs_f32() >= CATCH_AFTER,
        Phase::Hover { .. } => true,
        Phase::Falling { .. } | Phase::Pulled { .. } => false,
        // on the bounce: up again
        Phase::Bounce { .. } => {
            log("juggle: off the bounce, up again");
            crate::impact::launched(j.pos);
            j.phase = launched(j.pos, mario, RELAUNCH_HEIGHT);
            false
        }
    };
    if caught {
        log("juggle: caught up there");
        crate::impact::caught(j.pos + Vec3::Y * 0.8);
        catch(j, mario);
    }
}

/// A shot from afar (a boomerang, a fireball) hit `h`: if it's up in the air it stays up where it is,
/// a little higher (Devil May Cry's guns keep them floating); falling, it's caught up there again.
pub fn shot(h: &FieldInsHandle) {
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) else { return };
    match &mut j.phase {
        Phase::Up { last_hit, .. } => *last_hit = Instant::now(),
        Phase::Hover { since } => {
            *since = Instant::now();
            j.pos += Vec3::Y * SHOT_POP;
        }
        Phase::Falling { .. } => {
            j.phase = Phase::Hover { since: Instant::now() };
            j.pos += Vec3::Y * SHOT_POP;
        }
        Phase::Bounce { .. } => {
            log("juggle: shot off the bounce, up again");
            crate::impact::launched(j.pos);
            j.phase = launched(j.pos, j.pos, RELAUNCH_HEIGHT * 0.7);
        }
        Phase::Flying { .. } | Phase::Pulled { .. } => {}
    }
}

/// `h` isn't juggled any more (another move takes it: bat.rs), its gravity back.
pub fn release(h: &FieldInsHandle) {
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = up.iter().position(|j| key(&j.mob) == key(h)) {
        up.remove(i);
        if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(h)) {
            chr.modules.physics.gravity_disabled = false;
        }
    }
}

pub fn clear() {
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
        for j in up.iter() {
            if let Some(chr) = wcm.chr_ins_by_handle_mut(&j.mob) {
                chr.modules.physics.gravity_disabled = false;
            }
        }
    }
    up.clear();
}

/// Game thread, every frame: the lifted ones follow Mario (his feet `mario`, Elden Ring), or fall.
/// `hit` casts a ray (from, to) against the map.
pub fn update(dt: f32, mario: Vec3, face_angle: f32, hit: impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    let facing = Vec3::new(-face_angle.sin(), 0.0, face_angle.cos());
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if up.is_empty() {
        return;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    up.retain_mut(|j| {
        let Some(chr) = wcm.chr_ins_by_handle_mut(&j.mob) else { return false };
        // dead: the game's death (and its ragdoll) takes it from here
        if chr.modules.data.hp <= 0 || crate::carry::is_carried(&j.mob) || crate::knock::is_flying(&j.mob) || crate::bat::is_flying(&j.mob) {
            chr.modules.physics.gravity_disabled = false;
            return false;
        }
        let (next, keep) = match j.phase {
            Phase::Up { dir, dist, last_hit } => {
                let dir = dir.lerp(facing, 1.0 - (-FACE * dt).exp()).normalize_or(facing);
                j.phase = if last_hit.elapsed().as_secs_f32() > HANG {
                    Phase::Falling { vel: Vec3::ZERO, since: Instant::now() }
                } else {
                    Phase::Up { dir, dist, last_hit }
                };
                // in front of Mario, its feet a little above his
                let want = mario + dir * dist + Vec3::Y * 0.15;
                (j.pos.lerp(want, 1.0 - (-FOLLOW * dt).exp()), true)
            }
            Phase::Pulled { since, air, near } => {
                // straight to just in front of him, stopped by the map on the way
                let dir = Vec3::new(j.pos.x - mario.x, 0.0, j.pos.z - mario.z).normalize_or(Vec3::Z);
                let want = mario + dir * near + Vec3::Y * if air { 0.15 } else { 0.0 };
                let to = want - j.pos;
                let step = PULL_SPEED * dt;
                let next = if to.length() <= step { want } else { j.pos + to / to.length() * step };
                let blocked = hit(j.pos + Vec3::Y * 0.5, next + Vec3::Y * 0.5).is_some();
                if to.length() <= step || blocked || since.elapsed().as_secs_f32() > PULL_MAX {
                    let now = Instant::now();
                    j.phase = if air && !blocked { Phase::Up { dir, dist: near, last_hit: now } } else { Phase::Falling { vel: Vec3::ZERO, since: now } };
                    (if blocked { j.pos } else { next }, true)
                } else {
                    (next, true)
                }
            }
            Phase::Flying { vel, since } => {
                let vel = vel - Vec3::Y * GRAVITY * dt;
                let next = j.pos + vel * dt;
                // at the top (or into a ceiling or a wall): it hangs there
                if vel.y <= 0.0 || hit(j.pos + Vec3::Y * 0.5, next + Vec3::Y * 0.5).is_some() {
                    j.phase = Phase::Hover { since: Instant::now() };
                    (j.pos, true)
                } else {
                    j.phase = Phase::Flying { vel, since };
                    (next, true)
                }
            }
            Phase::Hover { since } => {
                // (a twinkle now and then: here, jump up to it)
                let t = since.elapsed().as_secs_f32();
                if (t / 0.35).floor() != ((t - dt) / 0.35).floor() {
                    crate::impact::waiting(j.pos + Vec3::Y * 0.9);
                }
                if since.elapsed().as_secs_f32() > HOVER {
                    j.phase = Phase::Falling { vel: Vec3::ZERO, since: Instant::now() };
                }
                (j.pos - Vec3::Y * SINK * dt, true)
            }
            Phase::Bounce { vel } => {
                let vel = vel - Vec3::Y * GRAVITY * dt;
                let next = j.pos + vel * dt;
                match hit(j.pos + Vec3::Y * 0.5, next - Vec3::Y * 0.05).filter(|_| vel.y < 0.0) {
                    Some(ground) => (Vec3::new(next.x, ground.y, next.z), false),
                    None => {
                        j.phase = Phase::Bounce { vel };
                        (next, true)
                    }
                }
            }
            Phase::Falling { vel, since } => {
                let vel = vel - Vec3::Y * GRAVITY * dt;
                let next = j.pos + vel * dt;
                if let Some(ground) = hit(j.pos + Vec3::Y * 0.5, next - Vec3::Y * 0.05) {
                    // (a real fall bounces once: a blow then sends it back up)
                    let landed = Vec3::new(next.x, ground.y, next.z);
                    if vel.y < -3.0 {
                        crate::impact::launched(landed);
                        j.phase = Phase::Bounce { vel: Vec3::new(vel.x * BOUNCE_KEEP, BOUNCE_UP, vel.z * BOUNCE_KEEP) };
                        (landed, true)
                    } else {
                        (landed, false)
                    }
                } else if since.elapsed().as_secs_f32() > MAX_FALL {
                    (j.pos, false)
                } else {
                    j.phase = Phase::Falling { vel, since };
                    (next, true)
                }
            }
        };
        // flying fast (sent up, falling, bouncing): a streak behind it (impact.rs)
        let speed = (next - j.pos).length() / dt.max(1e-3);
        if speed > 3.0 && !matches!(j.phase, Phase::Up { .. } | Phase::Hover { .. }) {
            crate::impact::enemy_streak(key(&j.mob), next + Vec3::Y * 0.8);
        }
        // back on the ground after a fall: a thud and its dust
        if !keep && speed > 2.0 {
            crate::impact::thud(next);
        }
        j.pos = next;
        let ph = &mut chr.modules.physics;
        ph.position = HavokPosition(next.x, next.y, next.z, 0.0);
        ph.chr_proxy_pos_update_requested = true;
        ph.gravity_disabled = keep;
        // (no fall as far as the game is concerned: no fall damage, no "fell off the world")
        crate::swing::clear_fall(chr);
        if !keep {
            log("juggle: back on the ground");
        }
        keep
    });
}
