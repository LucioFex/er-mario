//! Picking enemies up like SM64's Bob-ombs: L2 + R3 lifts a regular (not boss, not too big) enemy,
//! he carries it between his hands (SM64's own carrying: walk, jump), and B throws it far: it flies
//! like the hook's home run (bat.rs), bowling over whoever it meets, and lands alive, hardly hurt.
//! The fun is in the throw, not the damage.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::log;

/// SM64's throwing actions (on the ground, in the air)
const ACT_THROWING: u32 = 0x8000_0588;
const ACT_AIR_THROW: u32 = 0x8300_08AB;
/// Biggest enemy Mario can lift (m): about a soldier
const MAX_RADIUS: f32 = 0.8;
const MAX_HEIGHT: f32 = 2.4;
/// Throw: speed forward and up (m/s): far, a long arc (bat.rs flies it)
const THROW_SPEED: f32 = 18.0;
const THROW_UP: f32 = 6.5;
/// its landing's damage (% of max HP): next to nothing
const LANDING_PCT: f32 = 1.5;

enum Phase {
    Idle,
    /// Mario lifts / carries it: (enemy, its height in m)
    Held { mob: FieldInsHandle, height: f32, since: Instant },
}

static PHASE: Mutex<Phase> = Mutex::new(Phase::Idle);
/// A pickup just started: the SM64 tick puts Mario into SM64's pickup
static START: AtomicBool = AtomicBool::new(false);
/// Mario must let go (the enemy is gone or died)
static DROP: AtomicBool = AtomicBool::new(false);

fn key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

/// L2 + R3 on `h` (a regular enemy): lifts it if it isn't too big. True: picked up.
fn try_pick_up(h: &FieldInsHandle, radius_m: f32, height_m: f32) -> bool {
    // (co-op guest: no grabs, the host owns the enemies)
    if crate::coop::guest() {
        return false;
    }
    let mut phase = PHASE.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(*phase, Phase::Idle) || radius_m > MAX_RADIUS || height_m > MAX_HEIGHT {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(h) else { return false };
    if chr.modules.data.hp <= 0 {
        return false;
    }
    if crate::bat::is_flying(h) || crate::stopwatch::frozen(h) {
        return false;
    }
    log(format!("carry: picked up an enemy ({radius_m:.1} x {height_m:.1} m)"));
    *phase = Phase::Held { mob: *h, height: height_m, since: Instant::now() };
    START.store(true, Ordering::Relaxed);
    true
}

/// L2 + R3: Mario grabs the nearest enemy in front of him within arm's reach: a small one (carried,
/// B throws it) from any side, or a big one whose stance is broken (by the tail: swing.rs, spin
/// and B); one bigger than a man worn down to half is ridden instead (bossride.rs). `feet`, `face`:
/// his place and the way he looks (Elden Ring); `ray` the map's (none through a wall). True if he
/// took one.
pub fn grab_nearest(feet: Vec3, face: Vec3, origin: [f32; 3], id: i32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> bool {
    let here = HavokPosition(feet.x, feet.y, feet.z, 0.0);
    // (measured to their bodies: a wide one's centre is far off; a medium or big one's capsule is
    // narrower than its body, a hound's or a beast's long one most of all (by its flank, its rump,
    // Mario was "nobody within reach"): its body taken out to almost twice its radius, or half its
    // height; nearest first, the next if that one can't be taken)
    let near = crate::combat::nearby(&here, 9.0, origin);
    let mut closest: Option<(f32, f32, u32)> = None;
    let mut found: Vec<_> = near
        .iter()
        .filter_map(|t| {
            let h = *t.shootable()?;
            let (radius, height) = (t.radius * crate::scale(), t.height * crate::scale());
            let big = crate::juggle::pull_size(&h, radius, height) != crate::juggle::PullSize::Small;
            let body = if big { (radius * 1.8).max(height * 0.5).min(radius + 2.5) } else { radius };
            let d = Vec3::new(t.er.0 - feet.x, 0.0, t.er.2 - feet.z);
            let gap = (d.length() - body).max(0.0);
            let facing = d.normalize_or(face).dot(face);
            // (at his height: not one far below a ledge or up on one; and none through a wall)
            if feet.y < t.er.1 - 1.0 || feet.y > t.er.1 + height + 1.0 {
                return None;
            }
            let chest = Vec3::new(t.er.0, t.er.1 + (height * 0.5).min(1.5), t.er.2);
            let from = feet + Vec3::Y * 0.8;
            let edge = chest - (chest - from).with_y(0.0).normalize_or_zero() * body * 0.8;
            if ray(from, edge).is_some() {
                return None;
            }
            if closest.is_none_or(|c| gap < c.0) {
                closest = Some((gap, facing, unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.chr_ins_by_handle(&h)).map_or(0, |c| c.character_id)));
            }
            (gap <= REACH && (gap < if big { 1.2 } else { 0.8 } || facing >= 0.3)).then_some((gap, h, radius, height))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = found.len();
    for (_, h, radius, height) in found {
        // (a medium or big one worn down to half: up on its back, the capture; not one with its
        // stance broken, that's the tail grab's moment)
        if crate::bossride::mountable(&h) && !crate::swing::is_open(&h) && crate::bossride::mount(&h, id) {
            return true;
        }
        if crate::combat::liftable(&h) && !crate::combat::hands_off(&h) && try_pick_up(&h, radius, height) {
            return true;
        }
        // (one up in a juggle is let go of first: two moving it at once would tear it about)
        if crate::juggle::is_juggled(&h) && crate::swing::is_open(&h) && !crate::coop::guest() {
            crate::juggle::release(&h);
        }
        if crate::swing::try_grab(&h, radius) {
            return true;
        }
        log(format!(
            "carry: L2 + R3, one {radius:.1} x {height:.1} m not taken (liftable {}, scripted {}, stance open {})",
            crate::combat::liftable(&h),
            crate::combat::hands_off(&h),
            crate::swing::is_open(&h)
        ));
    }
    if n == 0 {
        match closest {
            Some((gap, facing, c)) => log(format!("carry: L2 + R3, nobody within reach (nearest c{c:04}, {gap:.1} m off its body, facing {facing:.2})")),
            None => log("carry: L2 + R3, nobody within reach"),
        }
    }
    false
}

/// how far (m, from its body) Mario's grab reaches
const REACH: f32 = 1.8;

/// SM64 tick: whether to start SM64's pickup now.
pub fn take_start() -> bool {
    START.swap(false, Ordering::Relaxed)
}

/// SM64 tick: whether Mario must let go.
pub fn take_drop() -> bool {
    DROP.swap(false, Ordering::Relaxed)
}


/// The enemy Mario carries or threw last, and when it was last carried or flying.
static LAST_MOB: Mutex<Option<(u64, Instant)>> = Mutex::new(None);
/// How long after that it still can't hurt Mario (running after a throw ran him into its hitbox).
const SAFE_AFTER: f32 = 1.0;

/// The last carried enemy's handle key (diagnostics), 0 if none.
pub fn last_mob_key() -> u64 {
    LAST_MOB.lock().unwrap_or_else(|e| e.into_inner()).map_or(0, |(k, _)| k)
}

/// Whether a hit Mario (at `mario`, game coordinates) just took came from the enemy he holds, the
/// one in the air, or the one he let go of a moment ago, so it's ignored. The game doesn't say who
/// hit the player (its last-attacker field stays empty), so: that enemy is within reach and no
/// other enemy is. Everyone else still hurts him.
pub fn harmless(mario: Vec3) -> bool {
    let phase = PHASE.lock().unwrap_or_else(|e| e.into_inner());
    let mut last = LAST_MOB.lock().unwrap_or_else(|e| e.into_inner());
    if let Phase::Held { mob, .. } = &*phase {
        *last = Some((key(mob), Instant::now()));
    }
    // (in the air after the throw, it can't hurt him either)
    if let Some((k, _)) = *last {
        let h = unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&k) };
        if crate::bat::is_flying(&h) {
            *last = Some((k, Instant::now()));
        }
    }
    let Some((mob, t)) = *last else { return false };
    if t.elapsed().as_secs_f32() >= SAFE_AFTER {
        return false;
    }
    let (mut mob_near, mut other_near) = (false, false);
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &eldenring::cs::ChrIns = chr;
            if chr.modules.data.hp <= 0 || crate::combat::own_side(chr.team_type) || crate::combat::helper(chr) {
                continue;
            }
            let q = chr.modules.physics.position;
            let d = Vec3::new(q.0 - mario.x, q.1 - mario.y, q.2 - mario.z).length();
            if d > HIT_RANGE {
                continue;
            }
            if key(&chr.field_ins_handle) == mob {
                mob_near = true;
            } else if crate::combat::hittable(chr.chr_type) {
                other_near = true;
            }
        }
    }
    mob_near && !other_near
}

/// Within this distance (m) an enemy could have just hit Mario.
const HIT_RANGE: f32 = 3.0;

/// Whether this enemy is in Mario's hands (no normal hits on it; once thrown, bat.rs flies it).
pub fn is_carried(h: &FieldInsHandle) -> bool {
    match &*PHASE.lock().unwrap_or_else(|e| e.into_inner()) {
        Phase::Held { mob, .. } => key(mob) == key(h),
        Phase::Idle => false,
    }
}

/// Mario holds an enemy (B throws it).
pub fn holding() -> bool {
    matches!(*PHASE.lock().unwrap_or_else(|e| e.into_inner()), Phase::Held { .. })
}

pub fn reset() {
    *PHASE.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Idle;
}

/// Every frame in Mario mode. `hands`: SM64's held-object point (game coordinates) and whether
/// Mario still holds it; `face` Mario's SM64 face angle; `action` his SM64 action.
pub fn update(hands: Option<Vec3>, face: f32, action: u32) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let mut phase = PHASE.lock().unwrap_or_else(|e| e.into_inner());
    // Mario's forward in game coordinates (SM64 is mirrored on X)
    let fwd = Vec3::new(-face.sin(), 0.0, face.cos());
    let Phase::Held { mob, height, since } = *phase else { return };
    let Some(chr) = wcm.chr_ins_by_handle_mut(&mob) else {
        *phase = Phase::Idle;
        DROP.store(true, Ordering::Relaxed);
        return;
    };
    if chr.modules.data.hp <= 0 {
        chr.modules.physics.gravity_disabled = false;
        *phase = Phase::Idle;
        DROP.store(true, Ordering::Relaxed);
        return;
    }
    let ph = &mut chr.modules.physics;
    match hands {
        // (the pickup's first frames: SM64 hasn't placed the hands yet, keep it where it is)
        Some(p) if p != Vec3::ZERO => {
            // held around its middle, facing away from Mario like a Bob-omb
            let at = p - Vec3::Y * (height * 0.45) + fwd * 0.25;
            ph.position = HavokPosition(at.x, at.y, at.z, 0.0);
            ph.chr_proxy_pos_update_requested = true;
            ph.gravity_disabled = true;
            let q = glam::Quat::from_rotation_y(std::f32::consts::PI - face);
            ph.orientation = eldenring::rotation::Quaternion(q.x, q.y, q.z, q.w);
            chr.modules.fall.fall_timer = 0.0;
        }
        Some(_) => {}
        // (SM64 starts the pickup on its next tick: not holding yet isn't letting go)
        None if START.load(Ordering::Relaxed) || since.elapsed().as_secs_f32() < 0.25 => {}
        None if action == ACT_THROWING || action == ACT_AIR_THROW => {
            ph.gravity_disabled = false;
            *phase = Phase::Idle;
            drop(phase);
            *LAST_MOB.lock().unwrap_or_else(|e| e.into_inner()) = Some((key(&mob), Instant::now()));
            // far along his facing, a long arc: the home run's flight (bat.rs), landing alive
            let flies = crate::bat::throw(&mob, fwd, THROW_SPEED, THROW_UP, LANDING_PCT);
            log(format!("carry: thrown{}", if flies { "" } else { " (it can't fly: dropped)" }));
        }
        None => {
            // let go some other way (Mario got hurt, fell): just drop it
            ph.gravity_disabled = false;
            log(format!("carry: dropped without a throw (action {action:#x})"));
            *phase = Phase::Idle;
        }
    }
}
