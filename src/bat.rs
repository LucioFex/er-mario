//! Enemies sent flying by the hook's moves (hook.rs):
//! - the home run: batted away along the camera, bowling over whoever it meets on the way (the
//!   game's knockdown), a thud where it lands
//! - the tether: two enemies yanked into each other (a small one into a big one, or both to the
//!   middle), a crash when they meet
//! Never a kill by itself: their landing and their crash hurt like a blow (combat.rs).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::log;

/// the home run: speed along the camera and up (m/s), its gravity, the longest flight (s), how
/// close (m, off a body) it bowls one over, its landing's damage (% of max HP)
const BAT_SPEED: f32 = 17.0;
const BAT_UP: f32 = 5.5;
const BAT_GRAVITY: f32 = 9.0;
const BAT_MAX: f32 = 2.2;
const BOWL_REACH: f32 = 0.7;
const LANDING_PCT: f32 = 6.0;
/// the tether: how fast they're yanked together (m/s), how close they meet (m), the longest pull (s)
const YANK_SPEED: f32 = 20.0;
const MEET: f32 = 0.9;
const YANK_MAX: f32 = 1.2;

enum Kind {
    HomeRun { vel: Vec3, bowled: Vec<u64> },
    /// pulled to `to` (the other's body, or the middle), the other one's key
    Yank { to: Vec3, other: u64 },
}

struct Flight {
    mob: FieldInsHandle,
    pos: Vec3,
    since: Instant,
    kind: Kind,
}

static FLYING: Mutex<Vec<Flight>> = Mutex::new(Vec::new());

fn key(h: &FieldInsHandle) -> u64 {
    combat::handle_key(h)
}

fn feet_of(h: &FieldInsHandle) -> Option<Vec3> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let p = chr.modules.physics.position;
    Some(Vec3::new(p.0, p.1, p.2))
}

/// One of them is flying now (juggle.rs, knock.rs leave it alone).
pub fn is_flying(h: &FieldInsHandle) -> bool {
    FLYING.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|f| key(&f.mob) == key(h))
}

/// The home run: `h` batted away along `dir` (horizontal). False if it can't go.
pub fn home_run(h: &FieldInsHandle, dir: Vec3) -> bool {
    // (co-op guest: the host owns the enemies)
    if crate::coop::guest() || is_flying(h) || !combat::liftable(h) || combat::hands_off(h) {
        return false;
    }
    let Some(pos) = feet_of(h) else { return false };
    crate::juggle::release(h);
    let dir = dir.with_y(0.0).normalize_or(Vec3::Z);
    log("bat: home run");
    FLYING.lock().unwrap_or_else(|e| e.into_inner()).push(Flight {
        mob: *h,
        pos,
        since: Instant::now(),
        kind: Kind::HomeRun { vel: dir * BAT_SPEED + Vec3::Y * BAT_UP, bowled: vec![key(h)] },
    });
    crate::style::callout("HOME RUN!", Some(pos + Vec3::Y * 2.0), [1.0, 0.85, 0.3], true, 4.0);
    true
}

/// The tether: `a` (hooked) and `b` (hooked next) yanked into each other. The draggable ones fly:
/// both to the middle, or the small one into the big one. False if neither can.
pub fn tether(a: &FieldInsHandle, b: &FieldInsHandle) -> bool {
    if crate::coop::guest() || key(a) == key(b) || is_flying(a) || is_flying(b) {
        return false;
    }
    let (Some(pa), Some(pb)) = (feet_of(a), feet_of(b)) else { return false };
    let can = |h: &FieldInsHandle| combat::liftable(h) && !combat::hands_off(h) && !combat::is_boss_now(h);
    let (ma, mb) = (can(a), can(b));
    let flights: Vec<(FieldInsHandle, Vec3, u64)> = match (ma, mb) {
        (true, true) => {
            let mid = (pa + pb) * 0.5;
            vec![(*a, mid, key(b)), (*b, mid, key(a))]
        }
        (true, false) => vec![(*a, pb, key(b))],
        (false, true) => vec![(*b, pa, key(a))],
        (false, false) => return false,
    };
    let mut flying = FLYING.lock().unwrap_or_else(|e| e.into_inner());
    for (mob, to, other) in flights {
        crate::juggle::release(&mob);
        let pos = if key(&mob) == key(a) { pa } else { pb };
        flying.push(Flight { mob, pos, since: Instant::now(), kind: Kind::Yank { to, other } });
    }
    log(format!("bat: tether ({})", if ma && mb { "both pulled together" } else { "the small one into the big one" }));
    true
}

pub fn clear() {
    let mut flying = FLYING.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
        for f in flying.iter() {
            if let Some(chr) = wcm.chr_ins_by_handle_mut(&f.mob) {
                chr.modules.physics.gravity_disabled = false;
            }
        }
    }
    flying.clear();
}

/// Game thread, every frame: moves the flights on, lands their blows. `ray`: the map.
pub fn update(dt: f32, origin: [f32; 3], player: &ChrIns, combat: &mut Combat, tick: u32, ray: impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    let mut flying = std::mem::take(&mut *FLYING.lock().unwrap_or_else(|e| e.into_inner()));
    if flying.is_empty() {
        return;
    }
    let dt = dt.min(0.05);
    // (who crashed into whom this frame: each tether's pair once)
    let mut crashed: Vec<u64> = Vec::new();
    let mut landed: Vec<(FieldInsHandle, f32)> = Vec::new();
    let positions: Vec<(u64, Vec3)> = flying.iter().map(|f| (key(&f.mob), f.pos)).collect();
    flying.retain_mut(|f| {
        if feet_of(&f.mob).is_none() {
            return false;
        }
        let age = f.since.elapsed().as_secs_f32();
        let (next, keep) = match &mut f.kind {
            Kind::HomeRun { vel, bowled } => {
                vel.y -= BAT_GRAVITY * dt;
                let next = f.pos + *vel * dt;
                // whoever it meets on the way goes down too
                let near = combat::nearby(&HavokPosition(next.x, next.y, next.z, 0.0), 3.0, origin);
                for (i, t) in near.iter().enumerate() {
                    let Some(h) = t.shootable() else { continue };
                    if bowled.contains(&t.key()) || is_flying_in(&positions, t.key()) {
                        continue;
                    }
                    let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
                    let (r, ht) = (t.radius * crate::scale(), t.height * crate::scale());
                    if (foot - next).with_y(0.0).length() <= r + BOWL_REACH && next.y + 0.5 >= foot.y && next.y <= foot.y + ht {
                        bowled.push(t.key());
                        let _ = h;
                        combat.deal(player, &near, &[(i, Attack::Bowled, false)], tick);
                        crate::impact::slip(foot);
                        if bowled.len() == 4 {
                            crate::style::callout("STRIKE!", Some(foot + Vec3::Y * 2.0), [1.0, 0.85, 0.3], true, 5.0);
                        }
                    }
                }
                // into the map: down it comes there
                match ray(f.pos + Vec3::Y * 0.6, next + Vec3::Y * 0.6).or_else(|| ray(f.pos + Vec3::Y * 0.3, next - Vec3::Y * 0.05)) {
                    Some(hit) => {
                        landed.push((f.mob, LANDING_PCT));
                        crate::impact::thud(hit);
                        (Vec3::new(hit.x, hit.y.min(next.y), hit.z) - vel.with_y(0.0).normalize_or_zero() * 0.3, false)
                    }
                    None => (next, age < BAT_MAX),
                }
            }
            Kind::Yank { to, other } => {
                // (towards where the other one is now, if it flies too: the middle moves with them)
                let other_now = positions.iter().find(|(k, _)| *k == *other).map(|(_, p)| *p);
                let goal = match other_now {
                    Some(p) => (p + f.pos) * 0.5,
                    None => feet_of_key(*other).unwrap_or(*to),
                };
                *to = goal;
                let d = goal - f.pos;
                let step = YANK_SPEED * dt;
                let next = if d.length() <= step { goal } else { f.pos + d / d.length() * step };
                let met = (next - goal).length() < MEET * 0.5 || other_now.is_some_and(|p| (p - next).length() < MEET) || feet_of_key(*other).is_some_and(|p| (p - next).with_y(0.0).length() < MEET + 0.4);
                if met && !crashed.contains(&key(&f.mob)) {
                    crashed.push(key(&f.mob));
                    crashed.push(*other);
                    crash(f.mob, *other, next, origin, player, combat, tick);
                    (next, false)
                } else if met {
                    (next, false)
                } else {
                    (next, age < YANK_MAX && ray(f.pos + Vec3::Y * 0.6, next + Vec3::Y * 0.6).is_none())
                }
            }
        };
        if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(&f.mob)) {
            let ph = &mut chr.modules.physics;
            ph.position = HavokPosition(next.x, next.y, next.z, 0.0);
            ph.chr_proxy_pos_update_requested = true;
            ph.gravity_disabled = keep;
            crate::swing::clear_fall(chr);
        }
        // (a streak behind it while it flies)
        if keep {
            crate::impact::enemy_streak(key(&f.mob), next + Vec3::Y * 0.8);
        }
        f.pos = next;
        keep
    });
    {
        let mut guard = FLYING.lock().unwrap_or_else(|e| e.into_inner());
        flying.append(&mut guard);
        *guard = flying;
    }
    for (mob, pct) in landed {
        combat::impact(combat, &mob, pct, tick);
    }
}

fn is_flying_in(positions: &[(u64, Vec3)], k: u64) -> bool {
    positions.iter().any(|(p, _)| *p == k)
}

fn feet_of_key(k: u64) -> Option<Vec3> {
    let h = unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&k) };
    feet_of(&h)
}

/// The tether's two met: both hit hard (a regular one knocked down), a crash between them.
fn crash(a: FieldInsHandle, b: u64, at: Vec3, origin: [f32; 3], player: &ChrIns, combat: &mut Combat, tick: u32) {
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), 6.0, origin);
    let hits: Vec<(usize, Attack, bool)> = near.iter().enumerate().filter(|(_, t)| t.key() == key(&a) || t.key() == b).map(|(i, _)| (i, Attack::Tether, false)).collect();
    log(format!("bat: tether crash ({} hit)", hits.len()));
    if !hits.is_empty() {
        combat.deal(player, &near, &hits, tick);
    }
    crate::impact::tether(at + Vec3::Y * 1.0);
    crate::style::callout("TETHER!", Some(at + Vec3::Y * 2.2), [0.6, 0.85, 1.0], true, 4.0);
}
