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
/// spiked down (the meteor punch): it falls this fast (m/s) from the start
const SPIKE_SPEED: f32 = 14.0;
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
    /// heaved over Mario by the Chomp Slam (chompslam.rs): along `arc`, `t` from -BITE/FLIGHT (the
    /// Chomp's bite: still where it was) to 1 (landed); `lurch`: the yank of a big one, a short
    /// step toward him that ends on its feet
    Slung { arc: crate::chompslam::Arc, t: f32, lurch: bool },
}

struct Juggled {
    mob: FieldInsHandle,
    pos: Vec3,
    phase: Phase,
    /// its body (m), and whether it's a small regular one (the air tricks are for those)
    radius: f32,
    height: f32,
    small: bool,
    /// air: it bounced off a wall this string; it had its OTG bounce since it last touched the ground
    wall_used: bool,
    otg_used: bool,
}

impl Juggled {
    fn new(mob: FieldInsHandle, pos: Vec3, phase: Phase, radius: f32, height: f32, small: bool) -> Self {
        Juggled { mob, pos, phase, radius, height, small, wall_used: false, otg_used: false }
    }
}

static UP: Mutex<Vec<Juggled>> = Mutex::new(Vec::new());

/// Slams that landed since the last `take_landed`: the enemy and where (Elden Ring).
static LANDED: Mutex<Vec<(FieldInsHandle, Vec3)>> = Mutex::new(Vec::new());

/// A sling at progress `t` (negative: still held, waiting for the Chomp's bite) advanced by `dt` s
/// over `flight` s: its new progress, and whether it landed.
pub fn sling_step(t: f32, dt: f32, flight: f32) -> (f32, bool) {
    let t = t + dt / flight.max(1e-3);
    (t.min(1.0), t >= 1.0)
}

fn key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

pub fn is_juggled(h: &FieldInsHandle) -> bool {
    UP.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| key(&j.mob) == key(h))
}

/// Some enemy hangs in the air with Mario, or was launched up there for him (his air blows lift
/// him to stay up, and X goes on with the air string: libsm64's er_juggle).
pub fn active() -> bool {
    UP.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| !matches!(j.phase, Phase::Falling { .. } | Phase::Bounce { .. } | Phase::Slung { .. }))
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
    lift(h, mario, radius_m, height_m, regular, stunned, high.then_some(LAUNCH_HEIGHT))
}

/// `try_launch`, `launch`: the height it flies to (the launcher's), or None to hang up with Mario.
fn lift(h: &FieldInsHandle, mario: Vec3, radius_m: f32, height_m: f32, regular: bool, stunned: bool, launch: Option<f32>) -> bool {
    let high = launch.is_some();
    let fly = launch.unwrap_or(LAUNCH_HEIGHT);
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
            j.phase = launched(j.pos, mario, fly);
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
    let phase = if high { launched(pos, mario, fly) } else { Phase::Up { dir: d.normalize_or(Vec3::Z), dist: d.length().clamp(NEAR, FAR), last_hit: Instant::now() } };
    up.push(Juggled::new(*h, pos, phase, radius_m, height_m, regular && refusal(radius_m, height_m, true, false).is_none()));
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
    up.push(Juggled::new(*h, pos, phase, radius_m, 2.0, !medium));
    true
}

/// The Chomp Slam (chompslam.rs) takes `h` along `arc`: after the Chomp's bite (BITE s) it flies
/// FLIGHT s and lands (`take_landed`), bouncing off the ground for a blow to send it up again;
/// `lurch`: a big one's yank, the arc a short step on the ground. False if it can't be moved now.
pub fn sling(h: &FieldInsHandle, arc: crate::chompslam::Arc, lurch: bool) -> bool {
    // (co-op guest: the host owns the enemies)
    if crate::coop::guest() || crate::stopwatch::frozen(h) {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(h) else { return false };
    if chr.modules.data.hp <= 0 || crate::carry::is_carried(h) || crate::knock::is_flying(h) {
        return false;
    }
    let phase = Phase::Slung { arc, t: -crate::chompslam::BITE / crate::chompslam::FLIGHT, lurch };
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) {
        j.pos = arc.start;
        j.phase = phase;
        return true;
    }
    up.push(Juggled::new(*h, arc.start, phase, 0.6, 2.0, false));
    true
}

/// The slams that landed since the last call.
pub fn take_landed() -> Vec<(FieldInsHandle, Vec3)> {
    std::mem::take(&mut *LANDED.lock().unwrap_or_else(|e| e.into_inner()))
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
        Phase::Falling { .. } | Phase::Pulled { .. } | Phase::Slung { .. } => false,
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
        Phase::Flying { .. } | Phase::Pulled { .. } | Phase::Slung { .. } => {}
    }
}

/// A blow drove `h` down (the fists' meteor punch): up there, it's slammed into the ground and
/// bounces off it once (a blow then sends it up again). False if it isn't up there.
pub fn spike(h: &FieldInsHandle) -> bool {
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) else { return false };
    if matches!(j.phase, Phase::Bounce { .. } | Phase::Pulled { .. } | Phase::Slung { .. }) {
        return false;
    }
    j.phase = Phase::Falling { vel: Vec3::new(0.0, -SPIKE_SPEED, 0.0), since: Instant::now() };
    log("juggle: spiked down");
    true
}

/// How high a small enemy slammed into the ground bounces off it (m): the Leap Splitter's OTG.
pub const OTG_HEIGHT: f32 = 1.5;

/// The speed up (m/s) that takes a body `height` m high under the juggle's gravity.
pub fn bounce_speed(height: f32) -> f32 {
    (2.0 * GRAVITY * height).sqrt()
}

/// A blow slammed `h` into the ground (the Leap Splitter's wave): a small enemy (one the string's
/// launcher could lift) bounces OTG_HEIGHT m off it, ready for a blow to send it up again. False
/// for a bigger one, or when it's not here to bounce.
pub fn ground_bounce(h: &FieldInsHandle, radius_m: f32, height_m: f32, regular: bool) -> bool {
    // (co-op guest: the host owns the enemies)
    if crate::coop::guest() || refusal(radius_m, height_m, regular, false).is_some() || crate::stopwatch::frozen(h) {
        return false;
    }
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    let phase = Phase::Bounce { vel: Vec3::Y * bounce_speed(OTG_HEIGHT) };
    if let Some(j) = up.iter_mut().find(|j| key(&j.mob) == key(h)) {
        // (one already in the air is up there for the string, not slammed: only a fall or a bounce
        // goes round again)
        if !matches!(j.phase, Phase::Falling { .. } | Phase::Bounce { .. }) {
            return false;
        }
        // (one OTG bounce until it has touched the ground: the ring hits on and on)
        let (now, used) = once(j.otg_used, false, true);
        j.otg_used = used;
        if !now {
            return false;
        }
        j.phase = phase;
        return true;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(h) else { return false };
    if chr.modules.data.hp <= 0 {
        return false;
    }
    let p = chr.modules.physics.position;
    log(format!("juggle: c{:04} bounced off the ground ({:.1} x {:.1} m)", chr.character_id, radius_m, height_m));
    let mut j = Juggled::new(*h, Vec3::new(p.0, p.1, p.2), phase, radius_m, height_m, true);
    j.otg_used = true;
    up.push(j);
    true
}

// Air (v4 Part 12.7): a wall bounces a flight back, a fast one billiards into the next enemy

/// the wall bounce: it leaves the wall this fast (m/s), the flights it checks for a wall are at least
/// this fast (m/s), and the share of its max HP the knock costs (%)
pub const WALL_SPEED: f32 = 3.0;
const WALL_MIN_SPEED: f32 = 2.0;
pub const WALL_SHARE: f32 = 3.0;
/// billiards: the speed that carries it into the next (m/s), how high the next flies (m), the share
/// each takes (%), how many it chains in a string, and how far a body may be to be looked at (m)
const BILLIARD_SPEED: f32 = 4.0;
const BILLIARD_HEIGHT: f32 = 2.5;
pub const BILLIARD_SHARE: f32 = 3.0;
pub const BILLIARD_CHAIN: u32 = 3;
const BILLIARD_LOOK: f32 = 1.6;

/// Where a flight along `dir` (horizontal) hit a surface: `p0` on its line, `side` a ray 0.4 m to the
/// side and `up` one 0.5 m higher, both along it (None: missed). The wall's normal facing the flight,
/// horizontal; None for a floor or a ceiling. With the side rays missing: head on.
pub fn wall_normal(dir: Vec3, p0: Vec3, side: Option<Vec3>, up: Option<Vec3>) -> Option<Vec3> {
    let back = -Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::Z);
    let (Some(a), Some(b)) = (side, up) else { return Some(back) };
    let n = (a - p0).cross(b - p0).normalize_or_zero();
    if n == Vec3::ZERO {
        return Some(back);
    }
    let n = if n.dot(dir) > 0.0 { -n } else { n };
    if n.y.abs() > 0.5 {
        return None;
    }
    Some(Vec3::new(n.x, 0.0, n.z).normalize_or(back))
}

/// The velocity (horizontal, WALL_SPEED m/s) after a flight at `vel` hits a wall with `normal`: its
/// reflection, pushed off the wall a bit (one running along it still leaves it), and turned to
/// Mario's side (`toward`, horizontal) if it would go the other way.
pub fn wall_bounce(vel: Vec3, normal: Vec3, toward: Vec3) -> Vec3 {
    let n = Vec3::new(normal.x, 0.0, normal.z).normalize_or(Vec3::X);
    let v = Vec3::new(vel.x, 0.0, vel.z);
    let r = v - 2.0 * v.dot(n) * n;
    let mut d = (r.normalize_or(n) + n * 0.5).normalize_or(n);
    if d.dot(toward) < 0.0 {
        d = (d + toward).normalize_or(toward);
    }
    d * WALL_SPEED
}

/// A bounce that happens once until something resets it (the string's wall bounce, the OTG bounce
/// until the ground is touched): (it bounces now, used from now on). The reset comes first.
pub fn once(used: bool, reset: bool, asked: bool) -> (bool, bool) {
    let used = used && !reset;
    if asked && !used {
        (true, true)
    } else {
        (false, used)
    }
}

/// A body the air tricks play on: small or medium (a boss or a big body never; the string's own
/// limits for the rest).
pub fn air_size(radius_m: f32, height_m: f32) -> bool {
    radius_m <= MEDIUM_RADIUS && height_m <= MEDIUM_HEIGHT
}

pub fn billiard_fast(speed: f32) -> bool {
    speed > BILLIARD_SPEED
}

pub fn chain_ok(chained: u32) -> bool {
    chained < BILLIARD_CHAIN
}

/// Two bodies (feet `a`, `b`; radius, height in m) touch: close sideways and overlapping in height.
pub fn touches(a: Vec3, ar: f32, ah: f32, b: Vec3, br: f32, bh: f32) -> bool {
    let d = Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
    d < ar + br && a.y < b.y + bh && b.y < a.y + ah
}

/// An enemy on the ground (or anywhere) a flight may hit: its handle, feet, body (m).
pub struct Body {
    pub handle: FieldInsHandle,
    pub feet: Vec3,
    pub radius: f32,
    pub height: f32,
}

/// billiards this string
static CHAINED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Extra shares (the wall knock, the billiards): the enemy and the % of its max HP.
static SHARES: Mutex<Vec<(FieldInsHandle, f32)>> = Mutex::new(Vec::new());

fn share(h: FieldInsHandle, pct: f32) {
    SHARES.lock().unwrap_or_else(|e| e.into_inner()).push((h, pct));
}

/// The shares since the last call (lib.rs deals them: combat::impact_soft).
pub fn take_shares() -> Vec<(FieldInsHandle, f32)> {
    std::mem::take(&mut *SHARES.lock().unwrap_or_else(|e| e.into_inner()))
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
    CHAINED.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Game thread, every frame: the lifted ones follow Mario (his feet `mario`, Elden Ring), or fall.
/// `hit` casts a ray (from, to) against the map; `bodies` lists the enemies around a point (m).
pub fn update(dt: f32, mario: Vec3, face_angle: f32, hit: impl Fn(Vec3, Vec3) -> Option<Vec3>, bodies: impl Fn(Vec3, f32) -> Vec<Body>) {
    let facing = Vec3::new(-face_angle.sin(), 0.0, face_angle.cos());
    let mut up = UP.lock().unwrap_or_else(|e| e.into_inner());
    if up.is_empty() {
        CHAINED.store(0, std::sync::atomic::Ordering::Relaxed);
        return;
    }
    // the fast flights of this frame, for the billiards
    let mut fast: Vec<(FieldInsHandle, Vec3, f32, f32)> = Vec::new();
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    up.retain_mut(|j| {
        let Some(chr) = wcm.chr_ins_by_handle_mut(&j.mob) else { return false };
        // dead: the game's death (and its ragdoll) takes it from here
        if chr.modules.data.hp <= 0 || crate::carry::is_carried(&j.mob) || crate::knock::is_flying(&j.mob) || crate::bat::is_flying(&j.mob) {
            chr.modules.physics.gravity_disabled = false;
            return false;
        }
        // air: a wall in the way of a flight (sent up, falling, bouncing) throws it back toward Mario
        if j.small && !j.wall_used {
            let vel = match j.phase {
                Phase::Flying { vel, .. } | Phase::Falling { vel, .. } | Phase::Bounce { vel } => vel,
                _ => Vec3::ZERO,
            };
            let speed = Vec3::new(vel.x, 0.0, vel.z).length();
            if speed >= WALL_MIN_SPEED {
                let dir = Vec3::new(vel.x, 0.0, vel.z) / speed;
                let from = j.pos + Vec3::Y * 0.8;
                let reach = j.radius + speed * dt;
                if let Some(p0) = hit(from, from + dir * reach) {
                    let (side, high) = (Vec3::new(-dir.z, 0.0, dir.x) * 0.4, Vec3::Y * 0.5);
                    let far = reach + 0.6;
                    let p1 = hit(from + side, from + side + dir * far);
                    let p2 = hit(from + high, from + high + dir * far);
                    if let Some(n) = wall_normal(dir, p0, p1, p2) {
                        let toward = Vec3::new(mario.x - j.pos.x, 0.0, mario.z - j.pos.z).normalize_or(-dir);
                        let out = wall_bounce(vel, n, toward);
                        let (_, used) = once(j.wall_used, false, true);
                        j.wall_used = used;
                        j.phase = Phase::Falling { vel: Vec3::new(out.x, vel.y, out.z), since: Instant::now() };
                        share(j.mob, WALL_SHARE);
                        crate::impact::air_bounce(p0);
                        log(format!("air: wall bounce c{:04}", chr.character_id));
                    }
                }
            }
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
            Phase::Slung { arc, t, lurch } => {
                let (t, landed) = sling_step(t, dt, crate::chompslam::FLIGHT);
                if !landed {
                    j.phase = Phase::Slung { arc, t, lurch };
                    (arc.at(t.max(0.0)), true)
                } else if lurch {
                    // (the yank: set down, it falls the little that is left)
                    j.phase = Phase::Falling { vel: Vec3::ZERO, since: Instant::now() };
                    (arc.end, true)
                } else {
                    LANDED.lock().unwrap_or_else(|e| e.into_inner()).push((j.mob, arc.end));
                    j.phase = Phase::Bounce { vel: Vec3::Y * bounce_speed(OTG_HEIGHT) };
                    (arc.end, true)
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
                    // (it touched the ground: the OTG bounce is ready again)
                    j.otg_used = once(j.otg_used, true, false).1;
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
        // air: flying fast, it may knock into the next enemy (below, once the lists are free)
        if keep && j.small && billiard_fast(speed) && matches!(j.phase, Phase::Flying { .. } | Phase::Falling { .. } | Phase::Bounce { .. } | Phase::Slung { .. }) {
            fast.push((j.mob, next, j.radius, j.height));
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
    drop(up);
    // air billiards: a fast one that touches another enemy launches it too, a small share to both
    for (striker, pos, sr, sh) in fast {
        for b in bodies(pos + Vec3::Y * 0.9, sr + BILLIARD_LOOK) {
            if !chain_ok(CHAINED.load(std::sync::atomic::Ordering::Relaxed)) {
                return;
            }
            if is_juggled(&b.handle) || !air_size(b.radius, b.height) || !touches(pos, sr, sh, b.feet, b.radius, b.height) {
                continue;
            }
            let (regular, stunned) = match pull_size(&b.handle, b.radius, b.height) {
                PullSize::Small => (true, false),
                PullSize::Medium => (false, crate::swing::is_open(&b.handle) || crate::swing::toughness_of(&b.handle).is_some_and(|(now, max)| max > 0.0 && now <= 0.0)),
                PullSize::Big => continue,
            };
            if !(regular || stunned) || !lift(&b.handle, mario, b.radius, b.height, regular, stunned, Some(BILLIARD_HEIGHT)) {
                continue;
            }
            CHAINED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            share(striker, BILLIARD_SHARE);
            share(b.handle, BILLIARD_SHARE);
            crate::impact::air_clack((pos + b.feet) * 0.5 + Vec3::Y * 0.9);
            if let Ok(wcm) = unsafe { WorldChrMan::instance() } {
                let id = |h: &FieldInsHandle| wcm.chr_ins_by_handle(h).map_or(0, |c| c.character_id);
                log(format!("air: billiard c{:04} -> c{:04}", id(&striker), id(&b.handle)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slam's flight: held still for the bite (negative progress), then FLIGHT s to land, to the tick.
    #[test]
    fn a_sling_waits_for_the_bite_then_lands_after_its_flight() {
        let (bite, flight) = (crate::chompslam::BITE, crate::chompslam::FLIGHT);
        let mut t = -bite / flight;
        let mut ticks = 0;
        loop {
            let (next, landed) = sling_step(t, 1.0 / 30.0, flight);
            ticks += 1;
            if t < 0.0 && next < 0.0 {
                // (still held where it was)
                assert!(!landed);
            }
            t = next;
            if landed {
                break;
            }
            assert!(ticks < 100);
        }
        assert_eq!(t, 1.0);
        assert_eq!(ticks, ((bite + flight) * 30.0).ceil() as i32, "{ticks}");
    }

    /// The Leap Splitter's OTG bounce: about 1.5 m up off the ground.
    #[test]
    fn the_ground_bounce_rises_one_and_a_half_metres() {
        let v = bounce_speed(OTG_HEIGHT);
        let apex = v * v / (2.0 * GRAVITY);
        assert!((apex - 1.5).abs() < 1e-3, "{apex}");
        assert!(OTG_HEIGHT > BOUNCE_UP * BOUNCE_UP / (2.0 * GRAVITY), "higher than a fall's bounce");
        // (only a small one: the string's limits)
        assert!(refusal(0.5, 1.8, true, false).is_none());
        assert!(refusal(2.0, 5.0, true, false).is_some());
    }

    /// The wall bounce: the flight's horizontal velocity reflects about the wall's normal, at 3 m/s.
    #[test]
    fn a_wall_bounce_reflects_about_the_normal_at_three_metres_a_second() {
        let n = Vec3::new(-1.0, 0.0, 0.0);
        let v = wall_bounce(Vec3::new(8.0, 0.0, 0.0), n, Vec3::new(-1.0, 0.0, 0.0));
        assert!((v - Vec3::new(-WALL_SPEED, 0.0, 0.0)).length() < 1e-4, "{v}");
        // (a glancing blow keeps its slant, still 3 m/s)
        let v = wall_bounce(Vec3::new(4.0, 0.0, 4.0), n, Vec3::new(-1.0, 0.0, 0.0));
        assert!(v.x < 0.0 && v.z > 0.0 && (v.length() - WALL_SPEED).abs() < 1e-4, "{v}");
        // (one running along the wall, or no way back: off the normal, toward Mario's side)
        let v = wall_bounce(Vec3::new(0.0, 0.0, 5.0), n, Vec3::new(-1.0, 0.0, 0.0));
        assert!(v.x < 0.0 && (v.length() - WALL_SPEED).abs() < 1e-4, "{v}");
        // (never away from Mario: a reflection that points the wrong way is turned onto the normal)
        let v = wall_bounce(Vec3::new(-4.0, 0.0, 4.0), n, Vec3::new(1.0, 0.0, 0.0));
        assert!(v.x > 0.0, "{v}");
    }

    /// Floors and ceilings don't bounce, walls do: three rays along the flight give the surface.
    #[test]
    fn only_a_wall_bounces_not_a_floor_or_a_ceiling() {
        let dir = Vec3::X;
        // a wall at x = 5 facing -X: the three hits are on the plane
        let n = wall_normal(dir, Vec3::new(5.0, 1.0, 0.0), Some(Vec3::new(5.0, 1.0, 0.4)), Some(Vec3::new(5.0, 1.5, 0.0))).unwrap();
        assert!((n - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-4, "{n}");
        // a slanted wall: the normal turns with it (still horizontal)
        let n = wall_normal(dir, Vec3::new(5.0, 1.0, 0.0), Some(Vec3::new(5.4, 1.0, 0.4)), Some(Vec3::new(5.0, 1.5, 0.0))).unwrap();
        assert!(n.y.abs() < 1e-4 && n.x < 0.0 && n.z > 0.0, "{n}");
        // a floor sloping up to meet the flight: the points rise with the distance
        assert!(wall_normal(dir, Vec3::new(5.0, 1.0, 0.0), Some(Vec3::new(5.0, 1.0, 0.4)), Some(Vec3::new(5.5, 1.5, 0.0))).is_none());
        // a ceiling: the same, mirrored
        assert!(wall_normal(dir, Vec3::new(5.0, 1.0, 0.0), Some(Vec3::new(5.0, 1.0, 0.4)), Some(Vec3::new(5.5, 0.5, 0.0))).is_none());
        // (the side rays missing: head on, off the flight itself)
        let n = wall_normal(dir, Vec3::new(5.0, 1.0, 0.0), None, None).unwrap();
        assert!((n + Vec3::X).length() < 1e-4);
    }

    /// Once per enemy per string (the wall) and once per landing (the OTG): asked again, no.
    #[test]
    fn a_bounce_happens_once_until_the_ground_is_touched() {
        let (now, used) = once(false, false, true);
        assert!(now && used);
        let (now, used) = once(used, false, true);
        assert!(!now && used, "no second bounce in the air");
        let (now, used) = once(used, true, false);
        assert!(!now && !used, "the ground touched: ready again");
        let (now, _) = once(used, false, true);
        assert!(now);
        // (touching the ground and asked in the same breath: the ground came first)
        assert!(once(true, true, true).0);
    }

    /// The wall bounce and the billiards are for small ones, never bosses or big bodies.
    #[test]
    fn air_billiards_and_wall_bounces_are_for_small_and_medium_bodies() {
        assert!(air_size(0.5, 1.8));
        assert!(air_size(1.2, 3.0));
        assert!(!air_size(2.0, 5.0));
        assert!(!air_size(1.0, 4.0));
        assert!(!air_size(1.5, 3.0));
    }

    /// A billiard needs speed and a touch: bodies overlapping sideways and in height.
    #[test]
    fn a_billiard_needs_speed_and_a_touch() {
        assert!(!billiard_fast(3.9) && billiard_fast(4.1));
        let a = Vec3::new(0.0, 1.0, 0.0);
        // 1.0 m apart, radii 0.5 + 0.6: touching
        assert!(touches(a, 0.5, 1.8, Vec3::new(1.0, 1.0, 0.0), 0.6, 1.8));
        assert!(!touches(a, 0.5, 1.8, Vec3::new(1.2, 1.0, 0.0), 0.6, 1.8));
        // over its head: no
        assert!(!touches(a, 0.5, 1.8, Vec3::new(0.5, 3.0, 0.0), 0.6, 1.8));
        // under its feet: no
        assert!(!touches(a, 0.5, 1.8, Vec3::new(0.5, -1.0, 0.0), 0.6, 1.8));
        // its feet in his head: yes
        assert!(touches(a, 0.5, 1.8, Vec3::new(0.5, 2.5, 0.0), 0.6, 1.8));
    }

    /// The chain: up to three per string.
    #[test]
    fn a_billiard_chain_stops_at_three() {
        assert!((0..BILLIARD_CHAIN).all(chain_ok));
        assert!(!chain_ok(BILLIARD_CHAIN));
        assert_eq!(BILLIARD_CHAIN, 3);
    }
}
