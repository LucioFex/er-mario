//! The Star Sword's B (Part 12.6a): the Star Step on foot, the Star Trick in the air. A short step
//! whose first ticks are a parry window: an attack that has just begun, close and facing Mario, is
//! met by its size. A small enemy (not a boss): the Phantom Cut, Mario vanishes in a star streak
//! and is behind it, sheathing, the cut line flashing across it a moment later. A medium or big
//! one: the parry, a blade clash, it staggers, Mario slides back and the next blows are the
//! Riposte (the perfect dodge's counter window, x1.6). A colossus: the perfect dodge itself
//! (Witch Time). No attack: just the step. In the air, with no blow coming, the Trick blinks him
//! 1.5 m over the enemy he fights. All of it pure down here, the game's side below.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// the dash's action arg for the short step (libsm64's act_er_dash scales its speeds)
pub const STEP_ARG: u32 = 1;
/// the step's first ticks parry
pub const WINDOW: u32 = 6;
/// an attacker this close (m, off its body), its attack this old (s), facing Mario (cos 60°)
pub const NEAR: f32 = 3.0;
pub const AGE: (f32, f32) = (0.15, 0.9);
/// Mario lands this far behind a small one's body (m), the cut flashes this many ticks later
pub const BEHIND_GAP: f32 = 0.8;
pub const CUT_AFTER: u32 = 10;
/// the parry throws Mario back this far (m)
pub const SLIDE: f32 = 0.5;
/// the Trick: the enemy this far at most, Mario this high over its head (m), 2 an airtime
pub const BLINK_RANGE: f32 = 8.0;
pub const BLINK_OVER: f32 = 1.5;
pub const TRICKS: u8 = 2;

/// what the enemy is, as the hook sorts them (juggle.rs) and the colossus climbing (colossus.rs)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Class {
    Small,
    Medium,
    Big,
    Colossus,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Verdict {
    Phantom,
    Parry,
    Witch,
}

/// What the step does against an attacker of `class`; `boss`: a boss or one the game's script
/// holds (`hands_off`); `spot`: the Phantom Cut found its landing spot.
pub fn verdict(class: Class, boss: bool, hands_off: bool, spot: bool) -> Verdict {
    match class {
        Class::Colossus => Verdict::Witch,
        Class::Small if !boss && !hands_off && spot => Verdict::Phantom,
        _ => Verdict::Parry,
    }
}

type Ray<'a> = &'a dyn Fn(Vec3, Vec3) -> Option<Vec3>;

/// Where the Phantom Cut lands (Elden Ring): behind the enemy at its radius + BEHIND_GAP, else at
/// its sides. Each place needs a floor within 1 m below, sight from Mario and from the enemy's
/// centre, and no wall right behind it. `ray(from, to)`: the first thing hit, if any.
pub fn phantom_spot(me: Vec3, foe: Vec3, forward: Vec3, radius: f32, bodies: &[Body], ray: Ray) -> Option<Vec3> {
    let fwd = forward.with_y(0.0).normalize_or_zero();
    if fwd == Vec3::ZERO {
        return None;
    }
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    let gap = radius + BEHIND_GAP;
    // (behind first, then the side nearer Mario)
    let (near, far) = if (me - foe).dot(right) >= 0.0 { (right, -right) } else { (-right, right) };
    [-fwd, near, far].into_iter().find_map(|dir| {
        let c = foe + dir * gap;
        // (a floor within a metre below, from a little above)
        let spot = ray(c + Vec3::Y * 0.6, c - Vec3::Y)?;
        if crowded(spot, bodies) {
            return None;
        }
        let eye = Vec3::Y;
        let clear = ray(me + eye, spot + eye).is_none()
            && ray(foe + eye, spot + eye).is_none()
            && ray(spot + eye, spot + eye + dir * 0.5).is_none();
        clear.then_some(spot)
    })
}

/// Where the Trick lands: BLINK_OVER over the enemy's head top, the ceiling there clear (an up-ray),
/// in sight of Mario, a floor under it within GROUND_SLACK of the enemy's ground `ground` (the Trick
/// ends in a drop: never over a chasm) and no other body there.
pub fn blink_spot(me: Vec3, head: Vec3, ground: f32, bodies: &[Body], ray: Ray) -> Option<Vec3> {
    let spot = head + Vec3::Y * BLINK_OVER;
    let clear = ray(head, spot + Vec3::Y * 0.4).is_none() && ray(me + Vec3::Y * 0.8, spot).is_none();
    let floor = ray(spot, Vec3::new(spot.x, ground - GROUND_SLACK, spot.z)).is_some_and(|f| (f.y - ground).abs() <= GROUND_SLACK);
    (clear && floor && !crowded(spot, bodies)).then_some(spot)
}

/// Another body's feet, radius and height (m).
pub type Body = (Vec3, f32, f32);

/// The Trick's floor may be this far (m) off the enemy's ground; a spot this close to another body
/// (m, off its side) is in it.
pub const GROUND_SLACK: f32 = 1.5;
const BODY_GAP: f32 = 0.3;

/// `spot` is inside one of `bodies`.
pub fn crowded(spot: Vec3, bodies: &[Body]) -> bool {
    bodies.iter().any(|(feet, r, h)| (spot - *feet).with_y(0.0).length() < r + BODY_GAP && spot.y > feet.y - 0.5 && spot.y < feet.y + h)
}

/// Whether the Trick can blink over an enemy `dist` m off whose head is `above` m over Mario's
/// feet: any but a colossus (its head is a climb) or one the game's script holds. Bosses too, it is
/// only a move.
pub fn blinkable(colossus: bool, hands_off: bool, dist: f32, above: f32) -> bool {
    !colossus && !hands_off && dist <= BLINK_RANGE && above <= 6.0
}

/// Whether a foe whose floor is lava (`on_lava`, its own floor material: a ray tells no material)
/// is no place for the Phantom Cut or the Trick to drop Mario; a lava boss's fight makes it harmless.
pub fn molten(on_lava: bool, lava_fight: bool) -> bool {
    on_lava && !lava_fight
}

/// Mario thrown back from `foe` by the parry: SLIDE, less when a wall is in the way.
pub fn slide_back(me: Vec3, foe: Vec3, ray: Ray) -> Vec3 {
    let away = (me - foe).with_y(0.0).normalize_or_zero();
    if away == Vec3::ZERO {
        return me;
    }
    // (a wall 0.3 m short of where he'd end up stops him there)
    let from = me + Vec3::Y * 0.7;
    let way = match ray(from, from + away * (SLIDE + 0.3)) {
        Some(hit) => ((hit - from).length() - 0.3).clamp(0.0, SLIDE),
        None => SLIDE,
    };
    me + away * way
}

// ---- the game's side ------------------------------------------------------------------------

/// untouchable this long after a Phantom Cut's vanish, a parry or a blink (s)
const VEIL_CUT: f32 = 0.6;
const VEIL_PARRY: f32 = 0.35;
const VEIL_BLINK: f32 = 0.3;

struct Step {
    since: u32,
    air: bool,
}

/// the Phantom Cut's line, due at `due`
struct Cut {
    mob: FieldInsHandle,
    due: u32,
    radius: f32,
}

struct State {
    step: Option<Step>,
    cut: Option<Cut>,
    veil: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { step: None, cut: None, veil: None });

/// The Star Step (or the Star Trick, `air`) began this tick.
pub fn begin(tick: u32, air: bool) {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).step = Some(Step { since: tick, air });
}

/// A Star Step's parry window is open: the perfect dodge leaves the dash alone then (dodge.rs).
pub fn stepping() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).step.is_some()
}

/// Mario is gone a moment (the Phantom Cut's vanish, a parry's clash, the blink): blows go through.
pub fn veiled() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).veil.is_some_and(|t| Instant::now() < t)
}

/// A Phantom Cut's line is due yet.
#[cfg(test)]
pub fn pending() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).cut.is_some()
}

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.step = None;
    s.cut = None;
    s.veil = None;
}

fn veil(secs: f32) {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).veil = Some(Instant::now() + std::time::Duration::from_secs_f32(secs));
}

pub struct In<'a> {
    pub id: i32,
    pub origin: [f32; 3],
    pub tick: u32,
    /// Mario's action after this tick
    pub action: u32,
    /// his feet (Elden Ring)
    pub feet: Vec3,
    /// the enemy he fights (combat focus), by key
    pub focus: Option<u64>,
    pub targets: &'a [combat::Target],
}

/// A scripted move to start next tick (script id, in the air).
#[derive(Default)]
pub struct Out {
    pub queue: Option<(u16, bool)>,
}

/// SM64's yaw that faces `d` (Elden Ring axes, x flipped: his forward is (-sin, 0, cos))
fn yaw_of(d: Vec3) -> f32 {
    (-d.x).atan2(d.z)
}

/// The other bodies near `at` (not `skip`'s): feet, radius and height (m).
fn bodies_near(i: &In, at: Vec3, skip: u64) -> Vec<Body> {
    let k = crate::scale();
    i.targets.iter().filter(|t| t.key() != skip && (t.feet() - at).length() < 6.0).map(|t| (t.feet(), t.radius * k, t.height * k)).collect()
}

/// Mario to `to` (if any), facing `yaw`, still, in `action` with this vertical speed: after the
/// checks of the pure functions above (SM64 puts him where it is; its floor finds him next tick).
fn put(i: &In, to: Option<Vec3>, yaw: f32, action: u32, up: f32) {
    let (id, sm) = (i.id, to.map(|p| crate::collision::er_to_sm(i.origin, &HavokPosition(p.x, p.y, p.z, 0.0))));
    worker::call("star step", move |_| unsafe {
        if let Some(sm) = sm {
            sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]);
        }
        sm64::sm64_set_mario_faceangle(id, yaw);
        sm64::sm64_set_mario_forward_velocity(id, 0.0);
        sm64::sm64_set_mario_velocity(id, 0.0, up, 0.0);
        sm64::sm64_set_mario_action(id, action);
    });
}

/// The enemy stands on lava that would burn Mario (its own floor material, as Mario's is read).
fn on_lava(chr: &ChrIns) -> bool {
    molten(crate::is_lava(chr.modules.physics.material_info.hit_material), combat::lava_fight())
}

/// `attack` on enemy `mob` (its radius `radius_m`).
pub(crate) fn strike(i: &In, combat: &mut Combat, player: &ChrIns, mob: &FieldInsHandle, at: Vec3, radius_m: f32, attack: Attack) {
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), 4.0 + radius_m, i.origin);
    let key = combat::handle_key(mob);
    if let Some(n) = near.iter().position(|t| t.shootable().is_some_and(|h| combat::handle_key(h) == key)) {
        combat.deal(player, &near, &[(n, attack, false)], i.tick);
    }
}

/// Game thread, every SM64 tick after Mario's: the Phantom Cut's line when it is due, and the
/// step's parry window (the first WINDOW ticks, an air Trick at once).
pub fn tick(i: &In, combat: &mut Combat, player: &ChrIns, ray: Ray) -> Out {
    let mut out = Out::default();
    // (the cut flashes: the enemy it was laid on, where it is now)
    let due = {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if s.cut.as_ref().is_some_and(|c| i.tick.wrapping_sub(c.due) < u32::MAX / 2) { s.cut.take() } else { None }
    };
    if let Some(cut) = due {
        cut_flash(i, combat, player, &cut);
    }
    let step = STATE.lock().unwrap_or_else(|e| e.into_inner()).step.as_ref().map(|s| (s.since, s.air));
    let Some((since, air)) = step else { return out };
    let age = i.tick.wrapping_sub(since);
    if age >= if air { 1 } else { WINDOW } || !crate::moves::dodging(i.action) {
        STATE.lock().unwrap_or_else(|e| e.into_inner()).step = None;
        return out;
    }
    let found = if crate::dodge::ready() { crate::dodge::attacker(i.feet, NEAR, AGE, Some(since)) } else { None };
    let done = if let Some(f) = found {
        resolve(i, combat, player, ray, f, air, &mut out);
        true
    } else if air {
        blink(i, ray, &mut out);
        true
    } else {
        false
    };
    if done {
        STATE.lock().unwrap_or_else(|e| e.into_inner()).step = None;
    }
    out
}

/// The attacker `f` meets the step: its size decides.
fn resolve(i: &In, combat: &mut Combat, player: &ChrIns, ray: Ray, f: crate::dodge::Found, air: bool, out: &mut Out) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle(&f.handle) else { return };
    let key = combat::handle_key(&f.handle);
    let k = crate::scale();
    let (r_m, h_m) = i.targets.iter().find(|t| t.key() == key).map_or((0.6, 1.8), |t| (t.radius * k, t.height * k));
    let class = if crate::colossus::is_colossus(r_m, h_m) {
        Class::Colossus
    } else {
        match crate::juggle::pull_size(&f.handle, r_m, h_m) {
            crate::juggle::PullSize::Small => Class::Small,
            crate::juggle::PullSize::Medium => Class::Medium,
            crate::juggle::PullSize::Big => Class::Big,
        }
    };
    let (boss, hands) = (combat::boss_class(&f.handle, chr), combat::hands_off(&f.handle));
    let lava = class == Class::Small && !boss && !hands && !air && on_lava(chr);
    if lava {
        log(format!("sword: phantom cut refused (lava under c{:04})", f.id));
    }
    let spot = (class == Class::Small && !boss && !hands && !air && !lava).then(|| phantom_spot(i.feet, f.pos, f.forward, r_m, &bodies_near(i, f.pos, key), ray)).flatten();
    let toward = (f.pos - i.feet).with_y(0.0);
    match verdict(class, boss, hands, spot.is_some()) {
        Verdict::Witch => {
            log(format!("sword: star step dodge c{:04}", f.id));
            crate::dodge::grant(i.feet, f.id, f.anim);
        }
        Verdict::Phantom => {
            let Some(spot) = spot else { return };
            log(format!("sword: phantom cut on c{:04}", f.id));
            crate::dodge::spend();
            crate::impact::phantom_streak(i.feet + Vec3::Y * 0.9, spot + Vec3::Y * 0.9);
            // (BEHIND! from the landing: the cut, a moment on, has it already)
            crate::odm::mark_behind(&f.handle, f.pos);
            put(i, Some(spot), yaw_of(f.pos - spot), crate::moves::ACT_IDLE, 0.0);
            veil(VEIL_CUT);
            STATE.lock().unwrap_or_else(|e| e.into_inner()).cut = Some(Cut { mob: f.handle, due: i.tick.wrapping_add(CUT_AFTER), radius: r_m });
            out.queue = Some((crate::script::SW_SHEATH, false));
        }
        Verdict::Parry => {
            log(format!("sword: parry c{:04}", f.id));
            crate::dodge::spend();
            // (the Riposte: the next blows are counters, no Flurry Rush of it)
            crate::dodge::open_counter(false);
            let dir = toward.normalize_or_zero();
            crate::impact::clash(i.feet + Vec3::Y * 1.0 + dir * 0.7);
            strike(i, combat, player, &f.handle, f.pos, r_m, Attack::SwordParry);
            let yaw = yaw_of(toward);
            if air {
                put(i, None, yaw, crate::moves::ACT_FREEFALL, 12.0);
            } else {
                put(i, Some(slide_back(i.feet, f.pos, ray)), yaw, crate::moves::ACT_IDLE, 0.0);
            }
            veil(VEIL_PARRY);
        }
    }
}

/// The Phantom Cut's line across the enemy (it may have moved on, or died: then nothing).
fn cut_flash(i: &In, combat: &mut Combat, player: &ChrIns, cut: &Cut) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle(&cut.mob) else { return };
    // (the sword put away meanwhile: no cut)
    if chr.modules.data.hp <= 0 || crate::weapons::current() != crate::weapons::SWORD {
        return;
    }
    let p = chr.modules.physics.position;
    let at = Vec3::new(p.0, p.1, p.2);
    let chest = at + Vec3::Y * (0.5 + cut.radius.min(1.0));
    let way = (chest - i.feet).with_y(0.0).normalize_or_zero();
    // (a slash slanting across it, from high on his left to low on his right)
    let slant = (Vec3::new(-way.z, 0.0, way.x) + Vec3::Y * 0.9).normalize();
    crate::impact::phantom_cut(chest, slant);
    strike(i, combat, player, &cut.mob, at, cut.radius, Attack::SwordPhantom);
}

/// The Star Trick with no blow coming: over the enemy he fights, if there is one near.
fn blink(i: &In, ray: Ray, out: &mut Out) {
    let k = crate::scale();
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let pick = |t: &&combat::Target| {
        let Some(h) = t.shootable() else { return false };
        let (r_m, h_m) = (t.radius * k, t.height * k);
        // (a boss too, it is only a move; never a colossus, nor one the game's script holds)
        !t.is_prop()
            && !t.friendly()
            && blinkable(crate::colossus::is_colossus(r_m, h_m), combat::hands_off(h), (t.feet() - i.feet).length(), t.feet().y + h_m - i.feet.y)
            && !wcm.chr_ins_by_handle(h).is_some_and(on_lava)
    };
    let rank = |t: &combat::Target| (Some(t.key()) != i.focus, (t.feet() - i.feet).length());
    let target = i.targets.iter().filter(pick).min_by(|a, b| {
        let (a, b) = (rank(a), rank(b));
        a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
    });
    let Some(t) = target else { return };
    let head = t.feet() + Vec3::Y * (t.height * k);
    let Some(spot) = blink_spot(i.feet, head, t.feet().y, &bodies_near(i, t.feet(), t.key()), ray) else { return };
    let id = t.shootable().and_then(|h| wcm.chr_ins_by_handle(h)).map_or(0, |c| c.character_id as i32);
    log(format!("sword: star trick blink on c{id:04}"));
    crate::impact::blink(i.feet + Vec3::Y * 0.9, spot);
    put(i, Some(spot), yaw_of(t.feet() - spot), crate::moves::ACT_FREEFALL, 0.0);
    veil(VEIL_BLINK);
    out.queue = Some((crate::script::SW_TRICK, true));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a flat floor at y 0 and, if given, a wall: the plane x = wall (hit from either side)
    fn world(wall: Option<f32>, floor: bool) -> impl Fn(Vec3, Vec3) -> Option<Vec3> {
        move |a: Vec3, b: Vec3| {
            let d = b - a;
            let mut best: Option<f32> = None;
            if floor && (a.y > 0.0) != (b.y > 0.0) {
                best = Some(a.y / -d.y);
            }
            if let Some(x) = wall {
                if (a.x < x) != (b.x < x) && d.x != 0.0 {
                    let t = (x - a.x) / d.x;
                    best = Some(best.map_or(t, |s| s.min(t)));
                }
            }
            best.map(|t| a + d * t)
        }
    }

    #[test]
    fn a_small_enemy_is_cut_a_medium_one_parried_a_colossus_dodged() {
        assert_eq!(verdict(Class::Small, false, false, true), Verdict::Phantom);
        // (no landing spot: the parry)
        assert_eq!(verdict(Class::Small, false, false, false), Verdict::Parry);
        // (never on a boss, nor one the game's script holds)
        assert_eq!(verdict(Class::Small, true, false, true), Verdict::Parry);
        assert_eq!(verdict(Class::Small, false, true, true), Verdict::Parry);
        assert_eq!(verdict(Class::Medium, false, false, true), Verdict::Parry);
        assert_eq!(verdict(Class::Big, true, false, true), Verdict::Parry);
        assert_eq!(verdict(Class::Colossus, false, false, true), Verdict::Witch);
        assert_eq!(verdict(Class::Colossus, true, false, false), Verdict::Witch);
    }

    #[test]
    fn the_phantom_cut_lands_behind_it() {
        let w = world(None, true);
        let (me, foe) = (Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO);
        // it faces +z, toward him: behind it is -z
        let spot = phantom_spot(me, foe, Vec3::Z, 0.5, &[], &w).expect("a spot");
        assert!((spot - Vec3::new(0.0, 0.0, -1.3)).length() < 0.01, "{spot}");
    }

    #[test]
    fn the_phantom_cut_takes_a_side_when_behind_is_walled_in() {
        // a wall right behind it (z = -1): no room there, no sight to it at chest height
        let w = world(None, true);
        let walled = move |a: Vec3, b: Vec3| {
            if (a.z > -1.0) != (b.z > -1.0) && (b - a).z != 0.0 && a.y > 0.5 {
                let t = (-1.0 - a.z) / (b - a).z;
                return Some(a + (b - a) * t);
            }
            w(a, b)
        };
        let (me, foe) = (Vec3::new(2.0, 0.0, 3.0), Vec3::ZERO);
        let spot = phantom_spot(me, foe, Vec3::Z, 0.5, &[], &walled).expect("a side");
        assert!((spot.x.abs() - 1.3).abs() < 0.01 && spot.z.abs() < 0.01, "{spot}");
        // the side nearer Mario first
        assert!(spot.x > 0.0, "{spot}");
    }

    #[test]
    fn the_phantom_cut_needs_a_floor_and_sight() {
        let (me, foe) = (Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO);
        // no floor anywhere within a metre below: nowhere to land
        assert!(phantom_spot(me, foe, Vec3::Z, 0.5, &[], &world(None, false)).is_none());
        // Mario's sight stopped everywhere
        let blind = |a: Vec3, b: Vec3| {
            let floor = world(None, true);
            if (a - me - Vec3::Y).length() < 0.01 {
                return Some(a);
            }
            floor(a, b)
        };
        assert!(phantom_spot(me, foe, Vec3::Z, 0.5, &[], &blind).is_none());
        // a ledge: the floor 3 m down is no floor
        let ledge = |a: Vec3, b: Vec3| if b.y < 0.0 && (a.y > -3.0) != (b.y > -3.0) { Some(Vec3::new(b.x, -3.0, b.z)) } else { None };
        assert!(phantom_spot(me, foe, Vec3::Z, 0.5, &[], &ledge).is_none());
    }

    #[test]
    fn the_trick_lands_over_the_head_with_a_clear_ceiling() {
        let me = Vec3::new(0.0, 3.0, 5.0);
        let head = Vec3::new(0.0, 2.0, 0.0);
        let floor = |a: Vec3, b: Vec3| if a.y > 0.0 && b.y < 0.0 { Some(Vec3::new(b.x, 0.0, b.z)) } else { None };
        let spot = blink_spot(me, head, 0.0, &[], &floor).expect("a spot");
        assert!((spot - Vec3::new(0.0, 3.5, 0.0)).length() < 0.01, "{spot}");
        // a ceiling 1.8 m over the head: no room for him
        let low = |a: Vec3, b: Vec3| if a.y < 3.8 && b.y > 3.8 { Some(Vec3::new(b.x, 3.8, b.z)) } else { None };
        assert!(blink_spot(me, head, 0.0, &[], &low).is_none());
        // a wall between him and the spot
        let wall = |a: Vec3, b: Vec3| if a.z > 2.0 && b.z < 2.0 { Some(Vec3::new(a.x, a.y, 2.0)) } else { None };
        assert!(blink_spot(me, head, 0.0, &[], &wall).is_none());
    }

    #[test]
    fn the_phantom_cut_never_lands_inside_another_body() {
        let w = world(None, true);
        let (me, foe) = (Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO);
        // a second enemy standing where behind it is: the side nearer Mario instead
        let other = [(Vec3::new(0.0, 0.0, -1.3), 0.5, 1.8)];
        let spot = phantom_spot(me, foe, Vec3::Z, 0.5, &other, &w).expect("a side");
        assert!(spot.x.abs() > 1.0, "{spot}");
        // all three places taken: no cut (the parry)
        let all = [(Vec3::new(0.0, 0.0, -1.3), 0.5, 1.8), (Vec3::new(1.3, 0.0, 0.0), 0.5, 1.8), (Vec3::new(-1.3, 0.0, 0.0), 0.5, 1.8)];
        assert!(phantom_spot(me, foe, Vec3::Z, 0.5, &all, &w).is_none());
    }

    #[test]
    fn the_trick_needs_a_floor_near_the_enemys_and_no_body_there() {
        let me = Vec3::new(0.0, 3.0, 5.0);
        let head = Vec3::new(0.0, 2.0, 0.0);
        // over a chasm: no floor below the spot at all
        assert!(blink_spot(me, head, 0.0, &[], &|a: Vec3, b: Vec3| if a.y > b.y { None } else { None }).is_none());
        // a floor 3 m under the enemy's feet is not its ground
        let deep = |a: Vec3, b: Vec3| if a.y > b.y && b.y < -3.0 { Some(Vec3::new(b.x, -3.0, b.z)) } else { None };
        assert!(blink_spot(me, head, 0.0, &[], &deep).is_none());
        // its own ground: fine
        let flat = |a: Vec3, b: Vec3| if a.y > 0.0 && b.y < 0.0 { Some(Vec3::new(b.x, 0.0, b.z)) } else { None };
        assert!(blink_spot(me, head, 0.0, &[], &flat).is_some());
        // another enemy standing under the spot
        assert!(blink_spot(me, head, 0.0, &[(Vec3::new(0.2, 0.0, 0.0), 0.6, 4.0)], &flat).is_none());
    }

    #[test]
    fn the_trick_blinks_over_bosses_but_not_colossi_or_held_ones() {
        // (a boss is just an enemy here: no damage, only the move)
        assert!(blinkable(false, false, 5.0, 2.0));
        assert!(!blinkable(true, false, 5.0, 2.0));
        assert!(!blinkable(false, true, 5.0, 2.0));
        assert!(!blinkable(false, false, BLINK_RANGE + 0.1, 2.0));
        assert!(!blinkable(false, false, 5.0, 6.1));
    }

    #[test]
    fn a_pending_cut_goes_with_a_weapon_swap() {
        STATE.lock().unwrap().cut = Some(Cut { mob: unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&7u64) }, due: 5, radius: 0.5 });
        assert!(pending());
        crate::weapons::set(crate::weapons::HAMMER);
        assert!(!pending());
    }

    /// SM64's forward in Elden Ring terms is (-sin a, 0, cos a): after `yaw_of(d)` he looks along `d`.
    #[test]
    fn mario_faces_the_way_he_is_asked_to() {
        for d in [Vec3::Z, -Vec3::Z, Vec3::X, -Vec3::X, Vec3::new(1.0, 0.0, 2.0), Vec3::new(-3.0, 0.0, -0.5)] {
            let a = yaw_of(d);
            let face = Vec3::new(-a.sin(), 0.0, a.cos());
            assert!(face.dot(d.normalize()) > 0.99, "{d}: faces {face}");
        }
    }

    #[test]
    fn the_parry_throws_him_back_half_a_metre_less_at_a_wall() {
        let (me, foe) = (Vec3::new(0.0, 0.0, 2.0), Vec3::ZERO);
        let back = slide_back(me, foe, &|_, _| None);
        assert!((back - Vec3::new(0.0, 0.0, 2.5)).length() < 0.01, "{back}");
        // a wall 0.6 m behind him: he stops short of it
        let wall = |a: Vec3, b: Vec3| if a.z < 2.6 && b.z > 2.6 { Some(Vec3::new(a.x, a.y, 2.6)) } else { None };
        let back = slide_back(me, foe, &wall);
        assert!(back.z > 2.0 && back.z < 2.4, "{back}");
        // a wall right at his back: he stays
        let wall = |a: Vec3, b: Vec3| if a.z < 2.1 && b.z > 2.1 { Some(Vec3::new(a.x, a.y, 2.1)) } else { None };
        assert!((slide_back(me, foe, &wall) - me).length() < 0.01);
    }

    #[test]
    fn a_foe_on_lava_is_no_place_to_land_unless_the_lava_is_harmless() {
        assert!(molten(true, false));
        // (a lava boss's fight: the lava does nothing to him then)
        assert!(!molten(true, true));
        assert!(!molten(false, false));
        assert!(!molten(false, true));
    }
}
