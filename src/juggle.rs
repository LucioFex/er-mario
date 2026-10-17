//! Air combos (Devil May Cry's juggle): the super jump punch (X, X, Y) lifts a small enemy with
//! Mario, a regular one not too big, or a boss or strong enemy only while it's stunned (its poise
//! broken, or its stance broken for the tail grab, swing.rs) and not too big either. While Mario's
//! blows keep landing it hangs in front of him; left alone a moment it falls back down (no fall
//! damage: the blows did the work). A few at most at a time. The hammer's launcher (X, Y) sends
//! them flying high up instead, where they hang a moment for Mario to jump after them: his first
//! blow up there catches them and the air string goes on.

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
const FOLLOW: f32 = 14.0;
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

enum Phase {
    /// up with Mario: where it is, its direction and distance from him, when it was last hit
    Up { dir: Vec3, dist: f32, last_hit: Instant },
    Falling { vel: Vec3, since: Instant },
    /// sent up by the launcher, and hanging at the top of it
    Flying { vel: Vec3, since: Instant },
    Hover { since: Instant },
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
    UP.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| !matches!(j.phase, Phase::Falling { .. }))
}

/// Mario `mario` caught `j` up there: it hangs in front of him from now on.
fn catch(j: &mut Juggled, mario: Vec3) {
    let d = Vec3::new(j.pos.x - mario.x, 0.0, j.pos.z - mario.z);
    j.phase = Phase::Up { dir: d.normalize_or(Vec3::Z), dist: d.length().clamp(NEAR, FAR), last_hit: Instant::now() };
}

/// The launcher's start: straight up to LAUNCH_HEIGHT, drifting LAUNCH_AWAY from Mario.
fn launched(pos: Vec3, mario: Vec3) -> Phase {
    let up = (2.0 * GRAVITY * LAUNCH_HEIGHT).sqrt();
    let away = Vec3::new(pos.x - mario.x, 0.0, pos.z - mario.z).normalize_or(Vec3::Z) * LAUNCH_AWAY / (up / GRAVITY);
    Phase::Flying { vel: away + Vec3::Y * up, since: Instant::now() }
}

/// The super jump punch hit `h` (combat.rs): lifts it if it may go. `stunned`: a boss or strong
/// enemy that's stunned (a regular one is `regular`); `high`: the hammer's launcher, sending it
/// flying high up. True if it's up now.
pub fn try_launch(h: &FieldInsHandle, mario: Vec3, radius_m: f32, height_m: f32, regular: bool, stunned: bool, high: bool) -> bool {
    let (max_r, max_h) = if regular { (MAX_RADIUS, MAX_HEIGHT) } else { (MAX_RADIUS_STUNNED, MAX_HEIGHT_STUNNED) };
    if !(regular || stunned) || radius_m > max_r || height_m > max_h {
        return false;
    }
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) {
        // (falling already: back up; up with him: a while longer)
        if high {
            j.phase = launched(j.pos, mario);
        } else if let Phase::Up { last_hit, .. } = &mut j.phase {
            *last_hit = Instant::now();
        } else if let Phase::Falling { .. } = j.phase {
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
    let phase = if high { launched(pos, mario) } else { Phase::Up { dir: d.normalize_or(Vec3::Z), dist: d.length().clamp(NEAR, FAR), last_hit: Instant::now() } };
    up.push(Juggled { mob: *h, pos, phase });
    true
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
        Phase::Falling { .. } => false,
    };
    if caught {
        log("juggle: caught up there");
        crate::impact::caught(j.pos + Vec3::Y * 0.8);
        catch(j, mario);
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
pub fn update(dt: f32, mario: Vec3, hit: impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if up.is_empty() {
        return;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    up.retain_mut(|j| {
        let Some(chr) = wcm.chr_ins_by_handle_mut(&j.mob) else { return false };
        // dead: the game's death (and its ragdoll) takes it from here
        if chr.modules.data.hp <= 0 || crate::carry::is_carried(&j.mob) || crate::knock::is_flying(&j.mob) {
            chr.modules.physics.gravity_disabled = false;
            return false;
        }
        let (next, keep) = match j.phase {
            Phase::Up { dir, dist, last_hit } => {
                if last_hit.elapsed().as_secs_f32() > HANG {
                    j.phase = Phase::Falling { vel: Vec3::ZERO, since: Instant::now() };
                }
                // in front of Mario, its feet a little above his
                let want = mario + dir * dist + Vec3::Y * 0.15;
                (j.pos.lerp(want, 1.0 - (-FOLLOW * dt).exp()), true)
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
            Phase::Falling { vel, since } => {
                let vel = vel - Vec3::Y * GRAVITY * dt;
                let next = j.pos + vel * dt;
                if let Some(ground) = hit(j.pos + Vec3::Y * 0.5, next - Vec3::Y * 0.05) {
                    (Vec3::new(next.x, ground.y, next.z), false)
                } else if since.elapsed().as_secs_f32() > MAX_FALL {
                    (j.pos, false)
                } else {
                    j.phase = Phase::Falling { vel, since };
                    (next, true)
                }
            }
        };
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
