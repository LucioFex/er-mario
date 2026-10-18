//! Enemies blown away (the fireball's giant shot, fireball.rs): like a thrown enemy (carry.rs) they
//! fly off a moment under guidance, then go limp (the game's death ragdoll) and whatever they hit
//! first finishes them.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::log;

/// speed away from the blast and up (m/s), gravity (m/s²)
const SPEED: f32 = 17.0;
const UP: f32 = 9.0;
const GRAVITY: f32 = 16.0;
/// guided flight before it goes limp, and the longest flight
const LIMP_AFTER: f32 = 0.12;
const MAX_FLIGHT: f32 = 3.0;
/// damage on landing: all of it (the finishing blow is the game's, combat::impact)
const IMPACT_PCT: f32 = 100.0;

struct Flight {
    mob: FieldInsHandle,
    pos: Vec3,
    vel: Vec3,
    since: Instant,
    limp: bool,
    frames: u32,
}

static FLYING: Mutex<Vec<Flight>> = Mutex::new(Vec::new());

fn key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

/// Sends `h` flying along `dir` (horizontal). False if it can't go (gone, dead, already flying).
pub fn launch(h: &FieldInsHandle, dir: Vec3) -> bool {
    // (co-op guest: the host owns the enemies, moving one here would only make it jump)
    if crate::coop::guest() {
        return false;
    }
    let mut flying = FLYING.lock().unwrap_or_else(|e| e.into_inner());
    if flying.iter().any(|f| key(&f.mob) == key(h)) {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(h) else { return false };
    if chr.modules.data.hp <= 0 {
        return false;
    }
    let p = chr.modules.physics.position;
    let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::Z);
    log(format!("knock: c{:04} blown away", chr.character_id));
    flying.push(Flight { mob: *h, pos: Vec3::new(p.0, p.1, p.2), vel: dir * SPEED + Vec3::Y * UP, since: Instant::now(), limp: false, frames: 0 });
    true
}

pub fn is_flying(h: &FieldInsHandle) -> bool {
    FLYING.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|f| key(&f.mob) == key(h))
}

pub fn clear() {
    FLYING.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// Every frame: moves the flights on. `hit` is a map ray. Returns the landings: (enemy, damage share
/// of its max HP in %).
pub fn update(dt: f32, hit: impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Vec<(FieldInsHandle, f32)> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return Vec::new() };
    let mut flying = FLYING.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();
    flying.retain_mut(|f| {
        let Some(chr) = wcm.chr_ins_by_handle_mut(&f.mob) else { return false };
        let age = f.since.elapsed().as_secs_f32();
        if f.limp {
            // the ragdoll carries it: it lands when the map is right under or ahead of it
            let p = chr.modules.physics.position;
            let now = Vec3::new(p.0, p.1, p.2);
            let dir = (now - f.pos).normalize_or_zero();
            let settle = f.frames < 3;
            let ground = !settle && now.y <= f.pos.y && hit(now + Vec3::Y * 0.3, now - Vec3::Y * 0.35).is_some();
            let wall = !settle && hit(f.pos + Vec3::Y * 0.5, now + dir * 0.6 + Vec3::Y * 0.5).is_some();
            f.pos = now;
            f.frames += 1;
            if ground || wall || age > MAX_FLIGHT {
                out.push((f.mob, IMPACT_PCT));
                return false;
            }
            return true;
        }
        f.vel.y -= GRAVITY * dt;
        let next = f.pos + f.vel * dt;
        let ph = &mut chr.modules.physics;
        if age > 0.05 && hit(f.pos + Vec3::Y * 0.5, next + Vec3::Y * 0.5).is_some() {
            ph.gravity_disabled = false;
            out.push((f.mob, IMPACT_PCT));
            return false;
        }
        if chr.chr_ctrl.ragdoll_ins != 0 && age > LIMP_AFTER {
            chr.chr_ctrl.chr_ragdoll_state = 2;
            chr.modules.physics.gravity_disabled = false;
            f.limp = true;
            return true;
        }
        if age > MAX_FLIGHT {
            ph.gravity_disabled = false;
            out.push((f.mob, IMPACT_PCT));
            return false;
        }
        ph.position = HavokPosition(next.x, next.y, next.z, 0.0);
        ph.chr_proxy_pos_update_requested = true;
        ph.gravity_disabled = true;
        chr.modules.fall.fall_timer = 0.0;
        f.pos = next;
        true
    });
    out
}
