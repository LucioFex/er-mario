//! The Chain Chomp's B (Part 12.6b): the Chomp Slam (God of War's harpy swing). The Chomp bites the
//! enemy he fights and Mario heaves it over his head onto the other side, into the ground (and
//! whoever stands there); B again within a moment slams it back the other way, three at most in a
//! string. It bounces off the ground, ready for X or the launcher. Only small ones, or stunned
//! medium ones (juggle.rs' sizes), fly; a big one is only yanked (a lurch toward him, stance
//! damage). In the air the Chomp bites an enemy that is up there and spikes it down (Orion's
//! Harpoon). The pure parts are here (the plan, the arc, the landing, the count); the game's side
//! is lib.rs and juggle.rs (the flight).

use std::sync::Mutex;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::juggle::PullSize;
use crate::log;

/// the chain's reach: the enemy this far at most (m, off its body)
pub const RANGE: f32 = 5.0;
/// the arc passes this high over Mario's feet (m)
pub const APEX: f32 = 2.5;
/// the flight (s), after the Chomp's bite (s)
pub const FLIGHT: f32 = 0.5;
pub const BITE: f32 = 0.12;
/// the landing: as far past him as it was before him, between these (m); short in front: this far
pub const LAND_MIN: f32 = 2.0;
pub const LAND_MAX: f32 = 4.5;
pub const SHORT_FRONT: f32 = 1.5;
/// three slams a string; B again within this many ticks of the last (its flight and a beat) keeps
/// the string
pub const MAX_SLAMS: u8 = 3;
pub const STRING_TICKS: u32 = 15 + 36;
/// the yank: a big one lurches this far toward him at most, and stops this far off him (m)
pub const YANK: f32 = 1.2;
pub const YANK_STOP: f32 = 1.2;
/// yanks count in the string (they are slams of a kind) and wait this many ticks between them
pub const YANK_COOLDOWN: u32 = 60;
/// the chain takes an enemy no more than this far above or below his feet (m) on the ground
pub const MAX_DY: f32 = 2.5;
/// a low ceiling lowers the arc, to this high at least (m); the arc leaves this share of the body
/// below the ceiling
pub const MIN_APEX: f32 = 1.2;
const CEILING_CLEAR: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Plan {
    Slam,
    /// a stance yank: the Chomp bites, it lurches toward Mario
    Yank,
    /// in the air: bites one that is up there and spikes it down
    Harpoon,
    /// nothing to bite: the plain dash
    Dash,
}

/// What B does against an enemy of `size` (`stunned`: its stance is broken) when Mario is in the
/// air (`air`) and the enemy is up in the air too (`up`: juggled or off the ground).
pub fn plan(size: PullSize, stunned: bool, air: bool, up: bool) -> Plan {
    let flies = size == PullSize::Small || (size == PullSize::Medium && stunned);
    match (air, flies, up) {
        (false, true, _) => Plan::Slam,
        (false, false, _) => Plan::Yank,
        (true, _, false) => Plan::Dash,
        (true, true, true) => Plan::Harpoon,
        (true, false, true) => Plan::Yank,
    }
}

/// A quadratic arc from `start` over a point `apex` high, to `end`.
#[derive(Clone, Copy, Debug)]
pub struct Arc {
    pub start: Vec3,
    pub ctrl: Vec3,
    pub end: Vec3,
}

impl Arc {
    /// The place at `t` (0..1).
    pub fn at(&self, t: f32) -> Vec3 {
        let (t, u) = (t.clamp(0.0, 1.0), 1.0 - t.clamp(0.0, 1.0));
        self.start * (u * u) + self.ctrl * (2.0 * u * t) + self.end * (t * t)
    }
}

/// The arc of an enemy at `start` heaved over Mario (his feet at `me`) to `end`: over his head at
/// the middle.
#[cfg(test)]
pub fn arc(start: Vec3, me: Vec3, end: Vec3) -> Arc {
    arc_with(start, me, end, APEX)
}

/// The same over his head at `apex` m.
pub fn arc_with(start: Vec3, me: Vec3, end: Vec3, apex: f32) -> Arc {
    // (a quadratic arc's middle is a quarter of each end and half its control: the control that
    // puts the middle over his head)
    Arc { start, ctrl: (me + Vec3::Y * apex) * 2.0 - (start + end) * 0.5, end }
}

type Ray<'a> = &'a dyn Fn(Vec3, Vec3) -> Option<Vec3>;

/// Whether the chain reaches the enemy at `foe` (its feet) from `me`: nothing of the map between
/// his chest and its chest, and on the ground not more than MAX_DY above or below him.
pub fn sight(me: Vec3, foe: Vec3, air: bool, ray: Ray) -> bool {
    (air || (foe.y - me.y).abs() <= MAX_DY) && ray(me + Vec3::Y, foe + Vec3::Y * 0.9).is_none()
}

/// How high the arc may go over `me` with a body `body_h` m tall heaved along it: APEX, lower under
/// a low ceiling, None when less than MIN_APEX fits.
pub fn slam_apex(me: Vec3, body_h: f32, ray: Ray) -> Option<f32> {
    let Some(roof) = ray(me + Vec3::Y, me + Vec3::Y * (APEX + 1.5)) else { return Some(APEX) };
    let room = (roof.y - me.y - body_h * CEILING_CLEAR).min(APEX);
    (room >= MIN_APEX).then_some(room)
}

/// Where the slam lands (Elden Ring): the far side of Mario from the enemy at `foe`, as far past
/// him as it was before him (LAND_MIN..LAND_MAX), 70% of that if the full way is no good; each
/// needs a floor within reach below and nothing in the way from his chest. None of them: short in
/// front of him (SHORT_FRONT toward the enemy). `ray(from, to)`: the first thing hit, if any.
pub fn landing(me: Vec3, foe: Vec3, ray: Ray) -> Vec3 {
    let away = (me - foe).with_y(0.0).normalize_or(Vec3::NEG_Z);
    let dist = (foe - me).with_y(0.0).length().clamp(LAND_MIN, LAND_MAX);
    for share in [1.0, 0.7] {
        let c = me + away * dist * share;
        // (a floor in reach below, and sight from his chest)
        let Some(spot) = ray(c + Vec3::Y * 2.0, c - Vec3::Y * 3.0) else { continue };
        if ray(me + Vec3::Y, spot + Vec3::Y).is_none() {
            return spot;
        }
    }
    let front = me - away * SHORT_FRONT;
    ray(front + Vec3::Y * 2.0, front - Vec3::Y * 3.0).unwrap_or(front)
}

/// Where a yank leaves an enemy at `foe`: YANK toward Mario, no nearer than YANK_STOP to him.
pub fn yank_to(me: Vec3, foe: Vec3) -> Vec3 {
    let flat = (foe - me).with_y(0.0);
    let d = flat.length();
    let want = (d - YANK).max(YANK_STOP).min(d);
    let at = me + flat.normalize_or_zero() * want;
    Vec3::new(at.x, foe.y, at.z)
}

/// An enemy the chain could take: how far its body is (m), how squarely in front of him it is
/// (cos), whether it is the one he fights (combat focus) or the one the last slam threw.
#[derive(Clone, Copy, Debug)]
pub struct Cand {
    pub dist: f32,
    pub front: f32,
    pub focus: bool,
    pub prefer: bool,
}

/// beside him or behind is out of the chain's way unless it is the one he fights
const FRONT_MIN: f32 = 0.3;

/// The enemy B bites: the one the last slam threw while the string lasts, else the one he fights,
/// else the nearest in front; all within RANGE.
pub fn choose(c: &[Cand]) -> Option<usize> {
    c.iter()
        .enumerate()
        .filter(|(_, c)| c.dist <= RANGE && (c.front >= FRONT_MIN || c.focus || c.prefer))
        .min_by(|(_, a), (_, b)| (!a.prefer, !a.focus).cmp(&(!b.prefer, !b.focus)).then(a.dist.total_cmp(&b.dist)))
        .map(|(i, _)| i)
}

/// Where the Chomp goes to hold the enemy at `target` (SM64 units) from Mario at `mario` facing
/// `face` (radians): its yaw round his facing (degrees, as the moves' curves), reach out and height
/// up (units). Right over his head it has no side: `keep` is the yaw it had.
pub fn grip_pose(mario: [f32; 3], face: f32, target: [f32; 3], keep: f32) -> (f32, f32, f32) {
    let (dx, dz) = (target[0] - mario[0], target[2] - mario[2]);
    let reach = dx.hypot(dz);
    let yaw = if reach < 40.0 {
        keep
    } else {
        ((dx.atan2(dz) - face).to_degrees() + 180.0).rem_euclid(360.0) - 180.0
    };
    (yaw, reach, target[1] - mario[1])
}

/// The string of slams.
#[derive(Default)]
pub struct Slams {
    count: u8,
    last: Option<u32>,
    /// no yank before this tick
    yank_ready: u32,
}

impl Slams {
    /// A yank started at `tick`: it counts in the string and the next waits.
    pub fn yanked(&mut self, tick: u32) {
        self.used(tick);
        self.yank_ready = tick + YANK_COOLDOWN;
    }

    /// Another yank may start.
    pub fn can_yank(&self, tick: u32) -> bool {
        self.can(tick) && tick.wrapping_sub(self.yank_ready) < u32::MAX / 2
    }

    /// Slams in the string going at `tick`.
    pub fn count(&self, tick: u32) -> u8 {
        match self.last {
            Some(l) if tick.wrapping_sub(l) <= STRING_TICKS => self.count,
            _ => 0,
        }
    }

    /// Another slam may start.
    pub fn can(&self, tick: u32) -> bool {
        self.count(tick) < MAX_SLAMS
    }

    /// A slam started at `tick`.
    pub fn used(&mut self, tick: u32) {
        self.count = self.count(tick) + 1;
        self.last = Some(tick);
    }

    pub fn clear(&mut self) {
        *self = Slams::default();
    }
}

// ---- the game's side ------------------------------------------------------------------------

/// the blast where a slam lands: radius (m)
const BLAST: f32 = 2.5;

/// The enemy the Chomp holds (its place for the chain's end, `grip`), until this tick; the one the
/// last slam threw and when (B again within the string slams it back)
struct State {
    held: Option<(FieldInsHandle, u32)>,
    last: Option<(u64, u32)>,
    yaw: f32,
}

static STATE: Mutex<State> = Mutex::new(State { held: None, last: None, yaw: 0.0 });

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.held = None;
    s.last = None;
}

pub struct In<'a> {
    pub origin: [f32; 3],
    pub tick: u32,
    /// Mario's feet (Elden Ring), SM64's facing, in the air
    pub feet: Vec3,
    pub face_angle: f32,
    pub air: bool,
    /// the enemy he fights (combat focus), by key
    pub focus: Option<u64>,
    pub targets: &'a [combat::Target],
    /// the yank is off cooldown (and the string has room)
    pub yank_ok: bool,
}

/// The enemy B bites and what comes of it.
pub struct Picked {
    pub handle: FieldInsHandle,
    pub id: i32,
    /// its feet (Elden Ring)
    pub pos: Vec3,
    pub plan: Plan,
    /// its height (m)
    pub height_m: f32,
    /// the yank may move it (not a boss, not one the game's script holds)
    pub movable: bool,
    pub key: u64,
}

/// The stance of a strong enemy is broken (swing.rs): the hammer's, the hook's "stunned".
fn stunned(h: &FieldInsHandle) -> bool {
    crate::swing::is_open(h) || crate::swing::toughness_of(h).is_some_and(|(now, max)| max > 0.0 && now <= 0.0)
}

/// Chooses the enemy (within the chain's reach, RANGE m off its body) and the plan against it.
pub fn pick(i: &In) -> Option<Picked> {
    let k = crate::scale();
    let facing = Vec3::new(-i.face_angle.sin(), 0.0, i.face_angle.cos());
    let prefer = {
        let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.last.filter(|(_, at)| i.tick.wrapping_sub(*at) <= STRING_TICKS).map(|(key, _)| key)
    };
    let list: Vec<&combat::Target> = i.targets.iter().filter(|t| !t.friendly() && !t.is_prop() && t.shootable().is_some()).collect();
    let cands: Vec<Cand> = list
        .iter()
        .map(|t| {
            let to = t.feet() - i.feet;
            let flat = to.with_y(0.0);
            Cand {
                dist: (flat.length() - t.radius * k).max(0.0),
                front: flat.normalize_or_zero().dot(facing),
                focus: Some(t.key()) == i.focus,
                prefer: Some(t.key()) == prefer,
            }
        })
        .collect();
    let t = list[choose(&cands)?];
    let h = *t.shootable()?;
    let (r_m, h_m) = (t.radius * k, t.height * k);
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(&h)?;
    let up = crate::juggle::is_juggled(&h) || t.feet().y > i.feet.y + 1.2;
    let plan = plan(crate::juggle::pull_size(&h, r_m, h_m), stunned(&h), i.air, up);
    Some(Picked { handle: h, id: chr.character_id as i32, pos: t.feet(), plan, height_m: h_m, movable: combat::liftable(&h) && !combat::hands_off(&h), key: t.key() })
}

fn strike(i: &In, combat: &mut Combat, player: &ChrIns, p: &Picked, attack: Attack) {
    let near = combat::nearby(&HavokPosition(p.pos.x, p.pos.y, p.pos.z, 0.0), 6.0, i.origin);
    if let Some(n) = near.iter().position(|t| t.key() == p.key) {
        combat.deal(player, &near, &[(n, attack, false)], i.tick);
    }
}

/// B bites `p`: the move to play and what it came to (None: nothing came of it, the dash).
/// `ray(from, to)`: the map's first hit. Not through a wall or from a far ledge; a low ceiling
/// lowers the slam's arc, or makes it a yank.
pub fn begin(i: &In, p: &Picked, combat: &mut Combat, player: &ChrIns, ray: Ray) -> Option<(u16, Plan)> {
    if p.plan != Plan::Dash && !sight(i.feet, p.pos, i.air, ray) {
        log(format!("chomp: c{:04} out of reach (a wall or a ledge)", p.id));
        return None;
    }
    let mut plan = p.plan;
    let mut apex = APEX;
    if plan == Plan::Slam {
        match slam_apex(i.feet, p.height_m, ray) {
            Some(a) => apex = a,
            None => {
                log(format!("chomp: no room over him to heave c{:04}, a yank", p.id));
                plan = Plan::Yank;
            }
        }
    }
    if plan == Plan::Yank && !i.yank_ok {
        return None;
    }
    let id = match plan {
        Plan::Slam => {
            let end = landing(i.feet, p.pos, ray);
            if !crate::juggle::sling(&p.handle, arc_with(p.pos, i.feet, end, apex), false) {
                return None;
            }
            log(format!("chomp: slam c{:04} ({:.1} m over to {:.1} m the other side)", p.id, (p.pos - i.feet).with_y(0.0).length(), (end - i.feet).with_y(0.0).length()));
            STATE.lock().unwrap_or_else(|e| e.into_inner()).last = Some((p.key, i.tick));
            crate::script::CHOMP_SLAM
        }
        Plan::Yank => {
            // (a stance's worth; a boss only that, anything else lurches a step toward him)
            strike(i, combat, player, p, Attack::ChompYank);
            if p.movable {
                let to = yank_to(i.feet, p.pos);
                crate::juggle::sling(&p.handle, Arc { start: p.pos, ctrl: (p.pos + to) * 0.5, end: to }, true);
            }
            log(format!("chomp: yank c{:04}{}", p.id, if p.movable { "" } else { " (held fast)" }));
            crate::script::CHOMP_YANK
        }
        Plan::Harpoon => {
            if !crate::juggle::spike(&p.handle) {
                return None;
            }
            // (the spike is a slam's blow)
            strike(i, combat, player, p, Attack::ChompSlam);
            log(format!("chomp: harpoon c{:04}", p.id));
            crate::script::CHOMP_HARPOON
        }
        Plan::Dash => return None,
    };
    STATE.lock().unwrap_or_else(|e| e.into_inner()).held = Some((p.handle, i.tick + 22));
    Some((id, plan))
}

/// A slam landed at `at` (Elden Ring): the one thrown takes the slam, everyone it landed beside the
/// blast.
pub fn landed(i: &In, combat: &mut Combat, player: &ChrIns, thrown: &FieldInsHandle, at: Vec3) {
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), BLAST + 4.0, i.origin);
    let key = combat::handle_key(thrown);
    let k = crate::scale();
    let hits: Vec<(usize, Attack, bool)> = near
        .iter()
        .enumerate()
        .filter(|(_, t)| t.shootable().is_some())
        .filter_map(|(n, t)| {
            if t.key() == key {
                return Some((n, Attack::ChompSlam, false));
            }
            let p = t.feet();
            let beside = (p - at).with_y(0.0).length() <= BLAST + t.radius * k && at.y + 1.5 >= p.y && at.y - 1.5 <= p.y + t.height * k;
            beside.then_some((n, Attack::ChompSlamLand, false))
        })
        .collect();
    let beside = hits.iter().filter(|h| h.1 == Attack::ChompSlamLand).count();
    log(format!("chomp: slam landed, {beside} beside it"));
    crate::impact::thud(at);
    if !hits.is_empty() {
        combat.deal(player, &near, &hits, i.tick);
    }
}

/// The Chomp's place while it holds an enemy (yaw round his facing in degrees, reach, height: SM64
/// units from his feet), if it does.
pub fn grip(mario: [f32; 3], face: f32, origin: [f32; 3], tick: u32) -> Option<(f32, f32, f32)> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let (h, until) = s.held?;
    if until.wrapping_sub(tick) > u32::MAX / 2 {
        s.held = None;
        return None;
    }
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(&h)?;
    let (p, ph) = (chr.modules.physics.position, &chr.modules.physics);
    let body = ph.hit_height.max(ph.chr_hit_height);
    let chest = if body.is_finite() && body > 0.3 { (body * 0.5).min(1.5) } else { 0.9 };
    let sm = crate::collision::er_to_sm(origin, &HavokPosition(p.0, p.1 + chest, p.2, 0.0));
    let (yaw, reach, up) = grip_pose(mario, face, sm, s.yaw);
    s.yaw = yaw;
    Some((yaw, reach, up))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a flat floor at y 0, and optionally a wall: the plane z = `wall` (hit from either side) at
    /// chest height and a cliff beyond z = `cliff` (no floor there)
    fn world(wall: Option<f32>, cliff: Option<f32>) -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        move |a: Vec3, b: Vec3| {
            let d = b - a;
            let mut best: Option<f32> = None;
            if (a.y > 0.0) != (b.y > 0.0) {
                let t = a.y / -d.y;
                let hit = a + d * t;
                if cliff.is_none_or(|c| hit.z > c) {
                    best = Some(t);
                }
            }
            if let Some(z) = wall {
                if (a.z < z) != (b.z < z) && d.z != 0.0 {
                    let t = (z - a.z) / d.z;
                    if (a + d * t).y > 0.3 {
                        best = Some(best.map_or(t, |s: f32| s.min(t)));
                    }
                }
            }
            best.map(|t| a + d * t)
        }
    }

    #[test]
    fn small_ones_and_stunned_mediums_fly_big_ones_are_yanked() {
        use PullSize::*;
        assert_eq!(plan(Small, false, false, false), Plan::Slam);
        assert_eq!(plan(Medium, true, false, false), Plan::Slam);
        assert_eq!(plan(Medium, false, false, false), Plan::Yank);
        assert_eq!(plan(Big, false, false, false), Plan::Yank);
        // (a big one stunned is still too big; bosses are Big)
        assert_eq!(plan(Big, true, false, false), Plan::Yank);
    }

    #[test]
    fn in_the_air_it_spikes_what_is_up_there() {
        use PullSize::*;
        assert_eq!(plan(Small, false, true, true), Plan::Harpoon);
        assert_eq!(plan(Medium, true, true, true), Plan::Harpoon);
        assert_eq!(plan(Medium, false, true, true), Plan::Yank);
        assert_eq!(plan(Big, false, true, true), Plan::Yank);
        // (one on the ground below: nothing to spike, the air dash)
        assert_eq!(plan(Small, false, true, false), Plan::Dash);
    }

    #[test]
    fn the_arc_passes_over_his_head() {
        let me = Vec3::new(1.0, 2.0, 1.0);
        let (start, end) = (Vec3::new(1.0, 2.0, 4.0), Vec3::new(1.0, 2.0, -2.0));
        let a = arc(start, me, end);
        assert!((a.at(0.0) - start).length() < 1e-4 && (a.at(1.0) - end).length() < 1e-4);
        let top = a.at(0.5);
        assert!((top - (me + Vec3::Y * APEX)).length() < 1e-3, "{top}");
        // (the middle of the path is the highest; it never dips under the floor it starts on)
        assert!((0..=20).all(|i| a.at(i as f32 / 20.0).y >= 2.0 - 1e-4 && a.at(i as f32 / 20.0).y <= top.y + 1e-3));
        // (a start not opposite the end still passes over him at the middle)
        let a = arc(Vec3::new(2.0, 2.0, 3.0), me, Vec3::new(3.0, 2.0, -1.0));
        let top = a.at(0.5);
        assert!((top.x - me.x).abs() < 1e-3 && (top.z - me.z).abs() < 1e-3 && (top.y - (me.y + APEX)).abs() < 1e-3, "{top}");
    }

    #[test]
    fn it_lands_on_the_far_side_as_far_as_it_was() {
        let w = world(None, None);
        let (me, foe) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0));
        let spot = landing(me, foe, &w);
        assert!((spot - Vec3::new(0.0, 0.0, -3.0)).length() < 0.01, "{spot}");
        // (never closer than LAND_MIN nor farther than LAND_MAX)
        let near = landing(me, Vec3::new(0.0, 0.0, 1.0), &w);
        assert!((near.z + LAND_MIN).abs() < 0.01, "{near}");
        let far = landing(me, Vec3::new(0.0, 0.0, 5.0), &w);
        assert!((far.z + LAND_MAX).abs() < 0.01, "{far}");
    }

    #[test]
    fn a_wall_or_a_cliff_behind_him_shortens_it_or_it_lands_short_in_front() {
        let (me, foe) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0));
        // a wall 4 m behind him: the 3 m spot is in sight
        let w = world(Some(-4.0), None);
        let spot = landing(me, foe, &w);
        assert!((spot.z + 3.0).abs() < 0.01, "{spot}");
        // a wall 2.5 m behind him: 3 m is past it, 70% (2.1 m) is not
        let w = world(Some(-2.5), None);
        let spot = landing(me, foe, &w);
        assert!((spot.z + 2.1).abs() < 0.01, "{spot}");
        // a wall right behind him: no room at all, short in front of him
        let w = world(Some(-0.5), None);
        let spot = landing(me, foe, &w);
        assert!((spot - Vec3::new(0.0, 0.0, SHORT_FRONT)).length() < 0.01, "{spot}");
        // a cliff behind him (no floor past z = -1): short in front too
        let w = world(None, Some(-1.0));
        let spot = landing(me, foe, &w);
        assert!((spot - Vec3::new(0.0, 0.0, SHORT_FRONT)).length() < 0.01, "{spot}");
    }

    #[test]
    fn the_chain_takes_the_fought_one_else_the_nearest_in_front() {
        let c = |dist, front, focus, prefer| Cand { dist, front, focus, prefer };
        // the nearest in front within the chain's reach
        assert_eq!(choose(&[c(4.0, 0.9, false, false), c(2.0, 0.8, false, false)]), Some(1));
        // (behind him, beside him, too far: none)
        assert_eq!(choose(&[c(2.0, -0.5, false, false), c(2.0, 0.1, false, false), c(5.5, 1.0, false, false)]), None);
        // the one he fights, even a farther one, even to the side
        assert_eq!(choose(&[c(2.0, 0.9, false, false), c(4.0, 0.2, true, false)]), Some(1));
        // (but not out of reach)
        assert_eq!(choose(&[c(2.0, 0.9, false, false), c(6.0, 1.0, true, false)]), Some(0));
        // the one the last slam threw, while the string lasts, before the fought one
        assert_eq!(choose(&[c(2.0, 0.9, true, false), c(3.0, -0.4, false, true)]), Some(1));
        assert_eq!(choose(&[]), None);
    }

    #[test]
    fn the_chain_runs_to_the_enemy_it_holds() {
        // (ahead 3 m, chest height 1 m)
        let (yaw, reach, up) = grip_pose([0.0, 0.0, 0.0], 0.0, [0.0, 100.0, 300.0], 7.0);
        assert!(yaw.abs() < 1e-3 && (reach - 300.0).abs() < 1e-3 && (up - 100.0).abs() < 1e-3);
        // (to his side, whichever way he faces: round his facing like the moves' curves)
        let (yaw, ..) = grip_pose([0.0; 3], 0.0, [300.0, 0.0, 0.0], 0.0);
        assert!((yaw - 90.0).abs() < 1e-3);
        let (yaw, ..) = grip_pose([0.0; 3], std::f32::consts::FRAC_PI_2, [300.0, 0.0, 0.0], 0.0);
        assert!(yaw.abs() < 1e-3, "{yaw}");
        // (behind him, either side of the wrap)
        let (yaw, ..) = grip_pose([0.0; 3], 0.0, [0.0, 0.0, -300.0], 0.0);
        assert!((yaw.abs() - 180.0).abs() < 1e-3);
        // (right over his head, high: it keeps the side it had)
        let (yaw, reach, up) = grip_pose([0.0; 3], 0.0, [5.0, 250.0, -3.0], 33.0);
        assert_eq!(yaw, 33.0);
        assert!(reach < 40.0 && up == 250.0);
    }

    #[test]
    fn a_yank_lurches_it_toward_him_and_stops_short() {
        let me = Vec3::ZERO;
        let to = yank_to(me, Vec3::new(0.0, 0.0, 4.0));
        assert!((to - Vec3::new(0.0, 0.0, 4.0 - YANK)).length() < 1e-4, "{to}");
        // (already close: it stops YANK_STOP off him)
        let to = yank_to(me, Vec3::new(0.0, 0.0, 2.0));
        assert!((to.z - YANK_STOP).abs() < 1e-4, "{to}");
        // (closer than that: it stays)
        let to = yank_to(me, Vec3::new(0.0, 0.0, 1.0));
        assert!((to.z - 1.0).abs() < 1e-4, "{to}");
    }

    #[test]
    fn three_slams_a_string_and_a_pause_ends_it() {
        let mut s = Slams::default();
        assert!(s.can(100));
        s.used(100);
        assert!(s.can(130));
        s.used(130);
        assert!(s.can(160));
        s.used(160);
        assert_eq!(s.count(170), 3);
        assert!(!s.can(170), "the fourth");
        // (a pause longer than the string's beat: a new string)
        assert!(s.can(160 + STRING_TICKS + 1));
        assert_eq!(s.count(160 + STRING_TICKS + 1), 0);
        s.used(160 + STRING_TICKS + 1);
        assert_eq!(s.count(160 + STRING_TICKS + 2), 1);
    }

    /// a ceiling at y = `c` over the floor y = 0, a wall at z = `wall`
    fn room(c: f32, wall: Option<f32>) -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        let w = world(wall, None);
        move |a: Vec3, b: Vec3| {
            let d = b - a;
            if (a.y < c) != (b.y < c) && d.y != 0.0 {
                return Some(a + d * ((c - a.y) / d.y));
            }
            w(a, b)
        }
    }

    #[test]
    fn a_wall_between_or_a_far_ledge_is_out_of_the_chains_reach() {
        let (me, foe) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 4.0));
        assert!(sight(me, foe, false, &world(None, None)));
        // a wall at 2 m: no
        assert!(!sight(me, foe, false, &world(Some(2.0), None)));
        // a ledge 3 m up or a pit 3 m down: no, 2 m: yes (on the ground only)
        assert!(!sight(me, foe + Vec3::Y * 3.0, false, &world(None, None)));
        assert!(!sight(me, foe - Vec3::Y * 3.0, false, &world(None, None)));
        assert!(sight(me, foe + Vec3::Y * 2.0, false, &world(None, None)));
        // (in the air the harpoon takes what is up high)
        assert!(sight(me, foe + Vec3::Y * 5.0, true, &world(None, None)));
        assert!(!sight(me, foe + Vec3::Y * 5.0, true, &world(Some(2.0), None)));
    }

    #[test]
    fn a_low_ceiling_lowers_the_arc_or_there_is_no_slam() {
        let me = Vec3::ZERO;
        assert_eq!(slam_apex(me, 1.8, &world(None, None)), Some(APEX));
        // a ceiling far up changes nothing
        assert_eq!(slam_apex(me, 1.0, &room(6.0, None)), Some(APEX));
        // 3 m with a 1.4 m body: 3 - 0.7 = 2.3 m of room, a lower arc
        let a = slam_apex(me, 1.4, &room(3.0, None)).unwrap();
        assert!(a < APEX && a >= MIN_APEX, "{a}");
        // 1.8 m: nothing fits
        assert_eq!(slam_apex(me, 1.4, &room(1.8, None)), None);
    }

    #[test]
    fn the_arc_can_be_lower() {
        let me = Vec3::ZERO;
        let a = arc_with(Vec3::new(0.0, 0.0, 3.0), me, Vec3::new(0.0, 0.0, -3.0), 1.6);
        assert!((a.at(0.5).y - 1.6).abs() < 1e-3);
    }

    #[test]
    fn yanks_count_in_the_string_and_wait_a_second_or_two() {
        let mut s = Slams::default();
        assert!(s.can_yank(100));
        s.yanked(100);
        assert_eq!(s.count(101), 1);
        assert!(!s.can_yank(100 + YANK_COOLDOWN - 1));
        assert!(s.can_yank(100 + YANK_COOLDOWN));
        // the cap of three covers yanks and slams together
        let mut s = Slams::default();
        s.used(100);
        s.used(130);
        s.yanked(160);
        assert_eq!(s.count(161), 3);
        assert!(!s.can(161) && !s.can_yank(190));
        assert!(s.can_yank(160 + YANK_COOLDOWN));
    }
}
