//! Mario's boomerang (weapons.rs, the Boomerang Bros.'): thrown from his hand (er_moves.c's throws
//! say when), it flies out curving, turned to the nearest enemy in front, and comes back to him,
//! hitting on the way out and again on the way back. Three can be out at once.
//! - X: one, from one hand then the other (in the air too, each holding him up a moment)
//! - Y: three in a fan; held, the big one: it flies to the enemy and hangs there spinning a while,
//!   hitting everyone around it again and again, then comes back
//! - X, X, Y: the launcher, thrown up in an arc: whoever it catches flies up (juggle.rs)
//! Its blows keep enemies up in the air up there (juggle::shot): launch one, then keep it floating
//! from afar. Walls send it back early. Each is a real model (assets/gear.rs) on a bone of its own,
//! spinning flat and banking into its curve; hud.rs only draws the streak of air behind it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use glam::{Mat3, Quat, Vec3};

use crate::collision::er_to_sm;
use crate::combat::{self, Attack, Combat};
use crate::engine_mario::{PARTS, PartPose, RANG, RANGS};
use crate::{log, moves, sm64, worker};

/// at most this many out at once (each has a model of its own)
pub const MAX_OUT: u32 = 3;
const _: () = assert!(MAX_OUT as usize == RANGS);
/// how far one flies (m), the launcher's and the big one's at most, how long the way out takes
/// (s), how far it curves to the side (m), how high the launcher's arc goes (m)
const RANGE: f32 = 18.0;
const UPPER_RANGE: f32 = 9.0;
const BIG_RANGE: f32 = 36.0;
const OUT_TIME: f32 = 0.6;
const BIG_OUT_TIME: f32 = 1.05;
const CURVE: f32 = 1.3;
const UPPER_ARC: f32 = 2.6;
/// it flies a little past the enemy it was turned to (m)
const PAST: f32 = 2.5;
/// the big one hangs there this long (s), hitting within this reach (m), following its enemy at up
/// to this speed (m/s); with it gone, it looks for another this close (m)
const HOVER: f32 = 3.0;
const HOVER_CHASE: f32 = 9.0;
const RETARGET: f32 = 8.0;
/// the big one hits an enemy at most this often while it hovers (s) and this many times a throw;
/// it stops following one after this many (it hits again and again: it mustn't stunlock)
const HOVER_AGAIN: f32 = 0.5;
const BIG_HITS: u32 = 4;
const CHASE_HITS: u32 = 4;
const BIG_REACH: f32 = 1.7;
/// coming back: from this speed up to that one (m/s), caught this close to his chest (m), and gone
/// for good this long after the throw (s)
const BACK_FROM: f32 = 6.0;
const BACK_TO: f32 = 32.0;
const CATCH: f32 = 0.8;
const LIFE: f32 = 9.0;
/// the size of its hits (SM64 units) and of the sprite (m), and the big one's
const RADIUS: f32 = 40.0;
const SIZE: f32 = 0.55;
const BIG_SIZE: f32 = 1.5;
/// turned to the nearest enemy within this range (m) and cone (cos 50°) in front
const AIM_RANGE: f32 = 30.0;
const AIM_CONE_COS: f32 = 0.64;
/// the big one's: farther and wider (cos 70°)
const BIG_AIM_RANGE: f32 = 45.0;
const BIG_AIM_CONE_COS: f32 = 0.34;
/// the fan's three: this far apart (radians)
const FAN: f32 = 0.38;

// SM64 sounds: its whir (two different whooshes; the big one's a spin), the catch, a wall
const SOUND_WHIR: [i32; 2] = [0x305C_0081, 0x045A_8081];
const SOUND_SPIN: i32 = 0x0437_8081;
const SOUND_CATCH: i32 = 0x0436_8081;
const SOUND_WALL: i32 = 0x0444_A081;
/// one whir every this many ticks at most, whatever the number out (three whirring at once droned)
const WHIR_EVERY: u32 = 6;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Normal,
    Upper,
    Big,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Out,
    Hover(f32),
    Back,
}

struct Rang {
    /// which of the models it is (engine_mario::RANG + slot)
    slot: usize,
    kind: Kind,
    phase: Phase,
    start: Vec3,
    dir: Vec3,
    side: Vec3,
    range: f32,
    out_time: f32,
    pos: Vec3,
    vel: Vec3,
    /// since the throw (s)
    age: f32,
    /// enemies hit on this pass (out, or back), the big one's by when
    hit: Vec<(u64, f32)>,
    spin: f32,
    /// a few points back, for its trail
    trail: std::collections::VecDeque<Vec3>,
    /// the enemy it was turned to (its key): the big one homes in on it and follows it
    target: Option<u64>,
    /// the big one's hits on each enemy this throw
    counts: Vec<(u64, u32)>,
}

/// A boomerang to draw (hud.rs): Elden Ring position, velocity (m/s), size (m), its turn (radians),
/// the trail behind it (oldest first).
#[derive(Clone)]
pub struct Sprite {
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub spin: f32,
    pub spin_rate: f32,
    pub trail: Vec<Vec3>,
}

pub struct Boomerang {
    out: Vec<Rang>,
    /// the tick of the last whir
    whir: u32,
    /// 0 on his back .. 1 in his hand, the ticks since he last threw one, the ticks the big one has
    /// been winding up
    in_hand: f32,
    idle: u32,
    winding: u32,
}

/// slung on his back while he isn't throwing (the big one, flat across his back, its bend up and
/// to his left): up from the pelvis and out behind (m), its tilt (radians), its size; in his hand
/// for a throw: how fast it gets there and goes back (share of the way per tick), how long it stays
/// after one (ticks); wound up for the big throw it grows from the hand's size to this
const BACK_RISE: f32 = 0.24;
const BACK_OUT: f32 = 0.26;
const BACK_TILT: f32 = 0.6;
const BACK_SIZE: f32 = 1.4;
const TO_HAND: f32 = 0.5;
const TO_BACK: f32 = 0.2;
const HOLD: u32 = 10;
const WOUND_SIZE: f32 = 2.2;
/// (er_moves.c ER_BIG_CHARGE_MAX: ticks to wind up fully)
const WIND_TICKS: f32 = 40.0;

static READY: AtomicU32 = AtomicU32::new(MAX_OUT);
static SPRITES: Mutex<(Vec<Sprite>, Option<Vec3>, Option<Instant>)> = Mutex::new((Vec::new(), None, None));

/// Boomerangs in hand (not out flying).
pub fn ready() -> u32 {
    READY.load(Ordering::Relaxed)
}

/// The boomerangs to draw now (moved on since the last tick), and the one in his hand, if any.
pub fn sprites() -> (Vec<Sprite>, Option<Vec3>) {
    let guard = SPRITES.lock().unwrap_or_else(|e| e.into_inner());
    let Some(at) = guard.2 else { return (Vec::new(), None) };
    let dt = at.elapsed().as_secs_f32();
    if dt > 0.25 {
        return (Vec::new(), None);
    }
    let dt = dt.min(1.0 / 30.0);
    (guard.0.iter().map(|s| Sprite { pos: s.pos + s.vel * dt, spin: s.spin + s.spin_rate * dt, ..s.clone() }).collect(), guard.1)
}

pub fn clear() {
    *SPRITES.lock().unwrap_or_else(|e| e.into_inner()) = (Vec::new(), None, None);
    READY.store(MAX_OUT, Ordering::Relaxed);
}

fn sound(id: i32, volume: f32) {
    // (a little louder or softer each time)
    let volume = volume * (0.8 + 0.35 * crate::rand01());
    worker::call("boomerang sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

impl Boomerang {
    pub fn new() -> Self {
        Boomerang { out: Vec::new(), whir: 0, in_hand: 0.0, idle: HOLD + 1, winding: 0 }
    }

    /// Game thread, after every SM64 tick: what the throws did (`mv`), Mario's state, feet and hand
    /// (Elden Ring), whether the boomerang is in hand. Throws, flies them, lands their hits.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        mv: &sm64::SM64ErMove,
        state: &sm64::SM64MarioState,
        feet: Vec3,
        hand: Vec3,
        held: bool,
        origin: [f32; 3],
        tick: u32,
        player: &ChrIns,
        combat: &mut Combat,
        ray: impl Fn(Vec3, Vec3) -> Option<Vec3>,
    ) {
        const DT: f32 = 1.0 / 30.0;
        if held && mv.throw_kind > 0 && (mv.throw_kind as u32 - 1) <= moves::THROW_BIG {
            self.throw(mv.throw_kind as u32 - 1, mv.throw_charge, state, feet, hand, origin);
        }
        let chest = feet + Vec3::Y * 0.8;
        let mut caught = 0;
        let mut out = std::mem::take(&mut self.out);
        out.retain_mut(|r| {
            r.age += DT;
            r.spin += DT * if r.kind == Kind::Big { 18.0 } else { 26.0 };
            let prev = r.pos;
            match r.phase {
                Phase::Out => {
                    // (the big one homes in: its way out bends to where its enemy is now)
                    // (less and less as it gets there, and its reach at most a metre a tick: late in
                    // the flight a big change made it jump)
                    if r.kind == Kind::Big {
                        let left = 1.0 - (r.age / r.out_time).min(1.0);
                        if let Some(at) = r.target.and_then(|k| chest_of(k, r.pos, origin)) {
                            let d = at - r.start;
                            if d.length() > 1.0 {
                                r.dir = r.dir.lerp(d.normalize(), 0.25 * left).normalize_or(r.dir);
                                r.range += ((d.length().min(BIG_RANGE) - r.range) * 0.25).clamp(-1.0, 1.0) * left;
                            }
                        }
                    }
                    let p = (r.age / r.out_time).min(1.0);
                    let along = r.range * (1.0 - (1.0 - p) * (1.0 - p));
                    let (sideways, up) = match r.kind {
                        Kind::Upper => (0.0, (p * std::f32::consts::PI).sin() * UPPER_ARC),
                        _ => ((p * std::f32::consts::PI).sin() * CURVE, 0.0),
                    };
                    r.pos = r.start + r.dir * along + r.side * sideways + Vec3::Y * up;
                    if p >= 1.0 {
                        r.hit.clear();
                        r.phase = if r.kind == Kind::Big { Phase::Hover(0.0) } else { Phase::Back };
                    }
                    // a wall: back early, with a clink
                    if ray(prev, r.pos).is_some() {
                        r.pos = prev;
                        r.hit.clear();
                        r.phase = Phase::Back;
                        r.vel = Vec3::ZERO;
                        sound(SOUND_WALL, 0.5);
                    }
                }
                Phase::Hover(t) => {
                    r.phase = if t + DT >= HOVER { Phase::Back } else { Phase::Hover(t + DT) };
                    // following its enemy (or the nearest other one), bobbing a little; not through
                    // walls, and not for good: after a few hits on it, it stays where it is (it
                    // must not hold one enemy down)
                    let mut at = r.target.and_then(|k| chest_of(k, r.pos, origin));
                    if at.is_none() && ((t / DT).round() as u32) % 6 == 0 {
                        r.target = nearest(r.pos, RETARGET, origin, None);
                        at = r.target.and_then(|k| chest_of(k, r.pos, origin));
                    }
                    let tired = r.target.is_some_and(|k| r.counts.iter().any(|(c, n)| *c == k && *n >= CHASE_HITS));
                    if let Some(at) = at.filter(|_| !tired) {
                        let next = r.pos + (at - r.pos).clamp_length_max(HOVER_CHASE * DT);
                        if ray(r.pos, next).is_none() {
                            r.pos = next;
                        }
                    }
                    r.pos += Vec3::Y * ((t * 7.0).cos() * 0.4 * DT);
                    if matches!(r.phase, Phase::Back) {
                        r.hit.clear();
                    }
                }
                Phase::Back => {
                    let to = chest - r.pos;
                    let k = ((r.age - r.out_time) / 0.6).clamp(0.0, 1.0);
                    let speed = BACK_FROM + (BACK_TO - BACK_FROM) * k;
                    let want = to.normalize_or_zero() * speed;
                    r.vel = r.vel.lerp(want, 0.35);
                    r.pos += r.vel * DT;
                    if to.length() < CATCH.max(speed * DT) {
                        caught += 1;
                        return false;
                    }
                }
            }
            r.vel = if matches!(r.phase, Phase::Back) { r.vel } else { (r.pos - prev) / DT };
            if tick.wrapping_sub(self.whir) >= WHIR_EVERY && crate::rand01() < 0.6 {
                self.whir = tick;
                match r.kind {
                    Kind::Big => sound(SOUND_SPIN, 0.3),
                    _ => sound(SOUND_WHIR[(crate::rand01() * 2.0) as usize % 2], 0.17),
                }
            }
            hits(r, prev, origin, tick, player, combat);
            r.age < LIFE
        });
        self.out = out;
        if caught > 0 {
            sound(SOUND_CATCH, 0.4);
        }
        READY.store(MAX_OUT.saturating_sub(self.out.len() as u32), Ordering::Relaxed);
        self.publish(held.then_some(hand));
    }

    fn throw(&mut self, kind: u32, charge: i32, state: &sm64::SM64MarioState, feet: Vec3, hand: Vec3, origin: [f32; 3]) {
        let free = MAX_OUT.saturating_sub(self.out.len() as u32) as usize;
        if free == 0 {
            return;
        }
        let face = Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
        // the nearest enemy in front, at its chest
        let here = HavokPosition(feet.x, feet.y, feet.z, 0.0);
        let (aim_range, cone) = if kind == moves::THROW_BIG { (BIG_AIM_RANGE, BIG_AIM_CONE_COS) } else { (AIM_RANGE, AIM_CONE_COS) };
        let found = combat::nearby(&here, aim_range, origin)
            .iter()
            .filter(|t| t.shootable().is_some())
            .filter_map(|t| {
                let at = chest(t, origin);
                let d = at - hand;
                let flat = d.with_y(0.0);
                (flat.length() > 0.5 && flat.normalize().dot(face) >= cone && combat::aim_pitch_ok(d)).then_some((flat.length(), at, t.key()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let aim = found.map(|(_, p, _)| p);
        let target = found.map(|(_, _, k)| k);
        let toward = aim.map_or(face, |p| (p - hand).normalize_or(face));
        let reach = aim.map(|p| (p - hand).length());
        let (kind, n, range, out_time) = match kind {
            moves::THROW_STORM => (Kind::Normal, 3, RANGE, OUT_TIME),
            moves::THROW_UPPER => (Kind::Upper, 1, reach.map_or(UPPER_RANGE, |r| (r + 1.0).min(UPPER_RANGE)), OUT_TIME * 0.9),
            // (with the MAX ready, three of them, a fan: style.rs, maxfin.rs)
            moves::THROW_BIG => {
                let max = free >= 2 && crate::style::take_max();
                if max {
                    log("max: three big boomerangs");
                    crate::style::callout("MAX!", Some(hand + Vec3::Y * 0.5), [1.0, 0.75, 0.2], true, 4.0);
                    crate::impact::max_burst(hand + face * 0.8);
                }
                (Kind::Big, if max { 3 } else { 1 }, reach.map_or(BIG_RANGE * 0.75, |r| r.min(BIG_RANGE)), BIG_OUT_TIME)
            }
            _ => (Kind::Normal, 1, reach.map_or(RANGE, |r| (r + PAST).min(RANGE + PAST)), OUT_TIME),
        };
        // (off it goes: a whoosh)
        sound(SOUND_WHIR[(crate::rand01() * 2.0) as usize % 2], if kind == Kind::Big { 0.45 } else { 0.3 });
        static FLIP: AtomicU32 = AtomicU32::new(0);
        let flip = FLIP.fetch_add(1, Ordering::Relaxed) % 2 == 0;
        for k in 0..n.min(free) {
            let Some(slot) = (0..RANGS).find(|s| !self.out.iter().any(|r| r.slot == *s)) else { break };
            // the fan: one ahead, one to each side, curving outwards
            let turn = if n == 3 { (k as f32 - 1.0) * FAN } else { 0.0 };
            let dir = glam::Quat::from_rotation_y(turn) * toward;
            let side = Vec3::Y.cross(dir).normalize_or(Vec3::X) * if n == 3 { if k == 0 { -1.0 } else { 1.0 } } else if flip { 1.0 } else { -1.0 };
            self.out.push(Rang {
                slot,
                kind,
                phase: Phase::Out,
                start: hand,
                dir,
                side,
                range,
                out_time,
                pos: hand,
                vel: dir * (range * 2.0 / out_time),
                age: 0.0,
                hit: Vec::new(),
                spin: k as f32,
                trail: Default::default(),
                target,
                counts: Vec::new(),
            });
        }
        log(format!("boomerang: {} x{}{}{}", ["one", "fan", "launcher", "big"][kind_index(kind, n)], n.min(free), if aim.is_some() { ", aimed" } else { "" }, if charge > 0 { format!(", charged {charge}") } else { String::new() }));
    }

    /// Puts the boomerangs on their models: the ones out where they fly (`feet`: Mario's, Elden
    /// Ring), spinning flat and banking into their curve; one in his right hand or on his back while he has one
    /// (`held`: the boomerang is his weapon now), slung on his back between throws.
    pub fn pose(&mut self, parts: &mut [PartPose; PARTS], feet: Vec3, face_angle: f32, held: bool, mv: &sm64::SM64ErMove, action: u32) {
        for r in &self.out {
            let size = if r.kind == Kind::Big { BIG_SIZE / SIZE } else { 1.0 };
            // (tilted towards the side it curves to, and up a little in the launcher's arc)
            let bank = match (r.kind, r.phase) {
                (Kind::Upper, Phase::Out) => Quat::from_axis_angle(r.dir.cross(Vec3::Y).normalize_or(Vec3::X), 0.5),
                (_, Phase::Back) => Quat::IDENTITY,
                _ => Quat::from_axis_angle(r.dir.normalize_or(Vec3::Z), 0.3 * r.side.dot(Vec3::Y.cross(r.dir).normalize_or(Vec3::X)).signum()),
            };
            let rot = (bank * Quat::from_rotation_y(-r.spin)).normalize();
            // (popping out of his hand: past full size, then settling; flat as it flies, a little
            // wider along its flight, a cartoon's smear)
            let pop = if r.age < 0.08 { 0.6 + 0.65 * (r.age / 0.08) } else { 1.0 + 0.25 * (1.0 - ((r.age - 0.08) / 0.12).min(1.0)) };
            let speed = (r.vel.length() / 30.0).clamp(0.0, 1.0);
            let stretch = Vec3::new(1.0 + 0.12 * speed, 1.0 - 0.15 * speed, 1.0 + 0.12 * speed);
            parts[RANG + r.slot] = PartPose { rot, pos: r.pos - feet, scale: size * pop, stretch };
        }
        if !held {
            self.in_hand = 0.0;
            self.idle = HOLD + 1;
            self.winding = 0;
            return;
        }
        // (none while the big one is out: it's that one)
        if self.out.iter().any(|r| r.kind == Kind::Big) {
            self.winding = 0;
            return;
        }
        let Some(slot) = (0..RANGS).find(|s| !self.out.iter().any(|r| r.slot == *s)) else { return };
        // in his hand while he throws (and a moment after), on his back otherwise; the big one
        // grows in his hand as he winds it up
        let throwing = matches!(action, moves::ACT_ER_THROW | moves::ACT_ER_AIR_THROW) && mv.action_arg <= moves::THROW_BIG;
        let winding = action == moves::ACT_ER_THROW && mv.action_arg == moves::THROW_BIG && mv.action_timer == 2;
        // (let go, it keeps its wound-up size until it leaves his hand)
        let big_throw = action == moves::ACT_ER_THROW && mv.action_arg == moves::THROW_BIG;
        self.winding = if winding { self.winding + 1 } else if big_throw { self.winding } else { 0 };
        self.idle = if throwing { 0 } else { self.idle.saturating_add(1) };
        let want = if self.idle <= HOLD { 1.0 } else { 0.0 };
        self.in_hand += (want - self.in_hand).clamp(-TO_BACK, TO_HAND);
        let t = self.in_hand * self.in_hand * (3.0 - 2.0 * self.in_hand);
        // in his fist by one tip, upright, its bend back over his forearm (along his arm while he
        // whirls: it swings round with him)
        let hand = parts[9].pos;
        let arm = (hand - parts[7].pos).with_y(0.0).normalize_or_zero();
        let ahead = if winding && arm != Vec3::ZERO { arm } else { Vec3::new(-face_angle.sin(), 0.0, face_angle.cos()) };
        let right = Vec3::Y.cross(ahead).normalize_or(Vec3::X);
        let face = Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, ahead));
        // (wound up it's held out level, swung round like a hammer thrower's)
        let lift = if winding { -0.35 } else { -1.25 };
        let held_rot = (face * Quat::from_rotation_x(lift) * Quat::from_rotation_y(0.4)).normalize();
        let wound = (self.winding as f32 / WIND_TICKS).min(1.0);
        let full = self.winding as f32 >= WIND_TICKS;
        let held_size = 0.85 + (WOUND_SIZE - 0.85) * wound * wound * (3.0 - 2.0 * wound) * (if full { 1.0 + 0.04 * (self.winding as f32 * 2.1).sin() } else { 1.0 });
        let tip = Vec3::new(crate::assets::gear::RANG_SPAN * 0.5, 0.0, -0.12);
        let held_pos = hand - held_rot * tip * held_size;
        // on his back, following his body (pelvis to head, shoulder to shoulder)
        let spine = (parts[3].pos - parts[1].pos).normalize_or(Vec3::Y);
        let side = (parts[7].pos - parts[4].pos).normalize_or(right);
        let side = (side - spine * side.dot(spine)).normalize_or(right);
        let mut back = spine.cross(side);
        if back.dot(ahead) > 0.0 {
            back = -back;
        }
        let across = back.cross(spine).normalize_or(side);
        let back_rot = (Quat::from_mat3(&Mat3::from_cols(across, back, spine)) * Quat::from_rotation_y(BACK_TILT)).normalize();
        let back_pos = parts[1].pos + spine * BACK_RISE + back * BACK_OUT;
        let rot = back_rot.slerp(held_rot, t).normalize();
        let pos = back_pos.lerp(held_pos, t);
        let scale = BACK_SIZE + (held_size - BACK_SIZE) * t;
        parts[RANG + slot] = PartPose { rot, pos, scale, stretch: Vec3::ONE };
    }

    fn publish(&mut self, hand: Option<Vec3>) {
        let mut sprites = Vec::with_capacity(self.out.len());
        for r in self.out.iter_mut() {
            r.trail.push_back(r.pos);
            while r.trail.len() > 6 {
                r.trail.pop_front();
            }
            let trail = &r.trail;
            let size = if r.kind == Kind::Big { BIG_SIZE } else { SIZE };
            sprites.push(Sprite { pos: r.pos, vel: r.vel, size, spin: r.spin, spin_rate: if r.kind == Kind::Big { 18.0 } else { 26.0 }, trail: trail.iter().copied().collect() });
        }
        // (in his hand while he has one)
        let held = hand.filter(|_| self.out.len() < MAX_OUT as usize);
        *SPRITES.lock().unwrap_or_else(|e| e.into_inner()) = (sprites, held, Some(Instant::now()));
    }
}

/// An enemy's chest (Elden Ring position).
fn chest(t: &combat::Target, origin: [f32; 3]) -> Vec3 {
    let at = crate::collision::sm_to_er(origin, [t.sm[0], t.sm[1] + t.height.min(300.0) * 0.5, t.sm[2]]);
    Vec3::new(at.0, at.1, at.2)
}

/// Where the enemy `key` is now, if it's still up and within reach of `near`.
fn chest_of(key: u64, near: Vec3, _origin: [f32; 3]) -> Option<Vec3> {
    combat::chest_by_key(key).filter(|p| (*p - near).length() < BIG_AIM_RANGE)
}

/// The nearest enemy within `range` of `at` (its key), other than `but`.
fn nearest(at: Vec3, range: f32, origin: [f32; 3], but: Option<u64>) -> Option<u64> {
    let here = HavokPosition(at.x, at.y, at.z, 0.0);
    combat::nearby(&here, range, origin)
        .iter()
        .filter(|t| t.shootable().is_some() && Some(t.key()) != but && combat::aim_pitch_ok(chest(t, origin) - at))
        .min_by(|a, b| (chest(a, origin) - at).length_squared().total_cmp(&(chest(b, origin) - at).length_squared()))
        .map(|t| t.key())
}

fn kind_index(kind: Kind, n: usize) -> usize {
    match kind {
        Kind::Normal if n == 3 => 1,
        Kind::Normal => 0,
        Kind::Upper => 2,
        Kind::Big => 3,
    }
}

/// Who `r` hits on its way from `prev` this tick.
fn hits(r: &mut Rang, prev: Vec3, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat) {
    let center = HavokPosition(r.pos.x, r.pos.y, r.pos.z, 0.0);
    let reach = if r.kind == Kind::Big && matches!(r.phase, Phase::Hover(_)) { BIG_REACH } else { 0.0 };
    let near = combat::nearby(&center, 3.0 + reach, origin);
    let (a, b) = (er_to_sm(origin, &HavokPosition(prev.x, prev.y, prev.z, 0.0)), er_to_sm(origin, &center));
    let (a, b) = (Vec3::from(a), Vec3::from(b));
    let radius = RADIUS + reach / crate::scale();
    let attack = match (r.kind, r.phase) {
        (Kind::Big, _) => Attack::RangBig,
        (Kind::Upper, Phase::Out) => Attack::RangUpper,
        _ => Attack::Rang,
    };
    // the big one hangs there hitting again and again (every few ticks, combat.rs), the others
    // each enemy once a pass
    let again = if r.kind == Kind::Big { HOVER_AGAIN } else { f32::MAX };
    let age = r.age;
    for (i, t) in near.iter().enumerate() {
        if t.shootable().is_none() {
            continue;
        }
        if r.hit.iter().any(|(k, when)| *k == t.key() && age - *when < again) {
            continue;
        }
        if r.kind == Kind::Big && r.counts.iter().any(|(k, n)| *k == t.key() && *n >= BIG_HITS) {
            continue;
        }
        let foot = Vec3::from(t.sm);
        let ab = (b - a).with_y(0.0);
        let k = if ab.length_squared() > 1.0 { ((foot - a).with_y(0.0).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 1.0 };
        let at = a.lerp(b, k);
        let near_axis = (foot - at).with_y(0.0).length() <= t.radius + radius;
        let level = at.y + radius >= foot.y && at.y - radius <= foot.y + t.height;
        if near_axis && level {
            r.hit.retain(|(key, _)| *key != t.key());
            r.hit.push((t.key(), age));
            if r.kind == Kind::Big {
                match r.counts.iter_mut().find(|(k, _)| *k == t.key()) {
                    Some((_, n)) => *n += 1,
                    None => r.counts.push((t.key(), 1)),
                }
            }
            combat.deal(player, &near, &[(i, attack, false)], tick);
        }
    }
}
