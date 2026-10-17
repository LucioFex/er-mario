//! Mario's fireball, on the pad's RB (X stays SM64's B: punch, kick, dive). Mega Man's buster with
//! SM64's own flame (its 8 frames from the ROM, hud.rs):
//! - a tap throws a small one (SM64's throw, standing or in the air; running he just shoots)
//! - held, it charges: sparks of flame gather at Mario's hand, bigger at 1 s, 2 s and 4 s, each
//!   step with its own SM64 chime (the last, SM64's secret jingle, and the flame flashes)
//! - each step hits harder; the full one goes through enemies and knocks them back
//! - held on for 10 s it keeps growing into a giant one, lifted over his head: it hits twice as
//!   hard as the full one and blows regular enemies away (knock.rs), in a shower of flames
//! - after a shot, a short cooldown (released too early: Lakitu's buzz)
//! - thrown with a move of its own (er_moves.c's throws): a palm thrust standing, pushing him back
//!   the harder the shot, the giant one from over his head; in the air it holds him up a moment;
//!   running he shoots on the move. The flame leaves his hand when the throw lets go.
//! Shots fly straight, turned to the nearest enemy in front, and burst on walls and bodies. The
//! damage goes through combat.rs like Mario's other hits.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::collision::sm_to_er;
use crate::{log, scale, sm64, worker};

/// held this many SM64 ticks (30 a second): charge levels 1, 2, 3 and the giant one
const CHARGE_TICKS: [u32; 4] = [30, 60, 120, 300];
const GIANT: u8 = 4;
/// a tap shows no charge (the sparks start after this)
const SHOW_CHARGE_AFTER: u32 = 7;
/// ticks between shots, by level
const COOLDOWN: [u32; 5] = [36, 40, 44, 54, 80];
/// how far every shot flies before it fizzles (SM64 units: 48 m), the small ones fast and the big
/// ones slow
const RANGE: f32 = 4800.0;
/// by level: hit radius (SM64 units), speed (units per tick), drawn size (m)
const RADIUS: [f32; 5] = [26.0, 40.0, 58.0, 100.0, 170.0];
const SPEED: [f32; 5] = [80.0, 66.0, 54.0, 44.0, 30.0];
const SIZE: [f32; 5] = [0.5, 0.8, 1.15, 2.0, 4.2];
/// turned to the nearest enemy within this range (m) and cone (cos 35°) in front
const AIM_RANGE: f32 = 35.0;
const AIM_CONE_COS: f32 = 0.82;
/// the full shot knocks regular enemies back this far (m), over this many ticks
const PUSH: f32 = 3.0;
const PUSH_TICKS: u32 = 9;
/// a burst's ticks (the giant one's)
const BURST_TICKS: [u32; 2] = [10, 22];
/// running faster than this (SM64 units per tick), Mario shoots without stopping to throw
const RUN_SHOT_SPEED: f32 = 14.0;
/// the giant one: it clings to a boss this long (ticks), burns whoever it carries every this many
/// ticks, and blows up reaching this far (m)
const STICK_TICKS: u32 = 48;
const BURN_EVERY: u32 = 8;
const EXPLODE_REACH: f32 = 5.0;

// SM64 sounds
const SOUND_FLAME_BLOWN: i32 = 0x5055_8081;
const SOUND_VOLCANO: i32 = 0x300C_8081;
const SOUND_FLAME_OUT: i32 = 0x3003_8081;
const SOUND_BOMB: i32 = 0x802E_2081_u32 as i32;
/// charge steps: menu tick, short star, the secret jingle
const SOUND_CHARGE: [i32; 4] = [0x7000_F881_u32 as i32, 0x3016_0091, 0x7030_2081, SOUND_VOLCANO];
const SOUND_BUZZ: i32 = 0x700E_0081;

/// running, he throws it with a punch on the move
const ACT_MOVE_PUNCHING: u32 = 0x0080_0457;
/// how long that punch is only a throw (hits no one, combat.rs), ticks
const THROW_TICKS: u32 = 12;
const ACT_GROUND_POUND: u32 = 0x0080_08A9;
const ACT_FLAG_STATIONARY: u32 = 0x200;
const ACT_FLAG_MOVING: u32 = 0x400;
const ACT_FLAG_AIR: u32 = 0x800;
const ACT_FLAG_INVULNERABLE: u32 = 0x0002_0000;
/// swimming, hanging, on a pole, riding a shell
const ACT_NO_THROW: u32 = 0x2000 | 0x0020_0000 | 0x0010_0000 | 0x0001_0000;
const ACT_GROUP_MASK: u32 = 0x1C0;
const ACT_GROUP_CUTSCENE: u32 = 0x100;

/// flame colours: deep orange, hot orange, red-orange with a white core, and the full charge's
/// flashing (Mega Man's buster cycles its colours)
const TINT: [[f32; 4]; 3] = [[1.0, 0.62, 0.25, 1.0], [1.0, 0.5, 0.18, 1.0], [1.0, 0.35, 0.12, 1.0]];
const FLASH: [[f32; 4]; 3] = [[1.0, 1.0, 1.0, 1.0], [1.0, 0.85, 0.3, 1.0], [1.0, 0.3, 0.1, 1.0]];

fn tint(level: u8, tick: u32) -> [f32; 4] {
    match level {
        GIANT.. => FLASH[(tick % 3) as usize],
        3 => FLASH[(tick / 2 % 3) as usize],
        _ => TINT[level as usize],
    }
}

fn burst_ticks(level: u8) -> u32 {
    BURST_TICKS[(level >= GIANT) as usize]
}

struct Shot {
    /// SM64 coordinates and velocity (units per tick)
    pos: [f32; 3],
    vel: [f32; 3],
    level: u8,
    age: u32,
    /// enemies it went through (the full shot hits each once)
    hit: Vec<u64>,
    /// the giant one: the enemies it sweeps along, the big one it stuck to (its key) and for how
    /// many ticks, the tick of its last burn
    swept: Vec<FieldInsHandle>,
    stuck: Option<(u64, u32)>,
    last_burn: u32,
}

struct Burst {
    pos: Vec3,
    level: u8,
    born: u32,
}

struct Push {
    handle: FieldInsHandle,
    dir: Vec3,
    left: u32,
}

pub struct Fire {
    /// tick RB went down (None: not held)
    held_since: Option<u32>,
    /// no shot before this tick
    ready_at: u32,
    /// tick of the last shot thrown with a running punch
    run_throw: Option<u32>,
    /// a shot waiting for its throw to let go (its level, when it was asked for)
    pending: Option<(u8, u32)>,
    level: u8,
    shots: Vec<Shot>,
    bursts: Vec<Burst>,
    pushes: Vec<Push>,
}

/// A flame to draw (hud.rs): Elden Ring position, velocity (m/s, to move it between ticks), size
/// (m), frame (0-7 SM64's flame, then FRAME_GLOW and the FRAME_ORB ones), tint.
#[derive(Clone, Copy)]
pub struct Sprite {
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub frame: usize,
    pub tint: [f32; 4],
}

/// a soft round glow (halos, embers, smoke), and the giant shot's ball of fire (4 frames of its
/// surface churning)
pub const FRAME_GLOW: usize = 8;
pub const FRAME_ORB: usize = 9;

static SPRITES: Mutex<(Vec<Sprite>, Option<Instant>)> = Mutex::new((Vec::new(), None));

/// The flames to draw now, moved on since the last tick.
pub fn sprites() -> Vec<Sprite> {
    let guard = SPRITES.lock().unwrap_or_else(|e| e.into_inner());
    let Some(at) = guard.1 else { return Vec::new() };
    let dt = at.elapsed().as_secs_f32();
    // (the SM64 ticks stopped: menus, loading)
    if dt > 0.25 {
        return Vec::new();
    }
    let dt = dt.min(1.0 / 30.0);
    guard.0.iter().map(|s| Sprite { pos: s.pos + s.vel * dt, ..*s }).collect()
}

pub fn clear() {
    *SPRITES.lock().unwrap_or_else(|e| e.into_inner()) = (Vec::new(), None);
}

fn sound(id: i32) {
    worker::call("fireball sound", move |_| unsafe { sm64::sm64_play_sound_global(id) });
}

fn er(origin: [f32; 3], p: [f32; 3]) -> Vec3 {
    let h = sm_to_er(origin, p);
    Vec3::new(h.0, h.1, h.2)
}

/// SM64 velocity (units per tick) as Elden Ring's (m/s): mirrored on X
fn er_vel(v: [f32; 3]) -> Vec3 {
    Vec3::new(-v[0], v[1], v[2]) * scale() * 30.0
}

impl Fire {
    pub fn new() -> Self {
        Fire { held_since: None, ready_at: 0, run_throw: None, pending: None, level: 0, shots: Vec::new(), bursts: Vec::new(), pushes: Vec::new() }
    }

    /// Game thread, every SM64 tick: RB held now (`b`, and whether Mario may shoot at all: not on
    /// Yoshi, not holding an enemy), Mario's state. Charges, shoots, moves the shots and lands
    /// their hits. `ray` gives the first map hit between two Elden Ring points.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        id: i32,
        b: bool,
        allowed: bool,
        mv: &sm64::SM64ErMove,
        state: &sm64::SM64MarioState,
        origin: [f32; 3],
        tick: u32,
        player: &ChrIns,
        combat: &mut Combat,
        ray: impl Fn(Vec3, Vec3) -> Option<Vec3>,
    ) {
        let action = state.action;
        // hurt, or in one of SM64's cutscene actions: the charge waits (not in the dash)
        let busy = (action & ACT_FLAG_INVULNERABLE != 0 && !crate::moves::dodging(action)) || action & ACT_GROUP_MASK == ACT_GROUP_CUTSCENE;
        if !allowed {
            self.held_since = None;
            self.level = 0;
        } else if b {
            let since = *self.held_since.get_or_insert(tick);
            let held = tick.saturating_sub(since.max(self.ready_at));
            let level = CHARGE_TICKS.iter().filter(|&&t| held >= t).count() as u8;
            if level > self.level {
                sound(SOUND_CHARGE[level as usize - 1]);
            }
            self.level = level;
        } else if self.held_since.take().is_some() {
            let level = std::mem::take(&mut self.level);
            if busy {
                // (let go while knocked back: the charge is lost)
            } else if tick < self.ready_at {
                sound(SOUND_BUZZ);
            } else {
                self.shoot(id, level, state, origin, tick);
            }
        }
        // the throw let go (or never came, cut short): the flame leaves his hand
        if let Some((level, asked)) = self.pending {
            let thrown = mv.throw_kind > 0 && (crate::moves::THROW_FIRE..crate::moves::THROW_HOOK).contains(&(mv.throw_kind as u32 - 1));
            if thrown || tick.wrapping_sub(asked) > 9 {
                self.pending = None;
                self.spawn(level, state, origin, tick);
            }
        }

        self.fly(origin, tick, player, combat, &ray);
        self.push(&ray);
        self.bursts.retain(|b| tick.wrapping_sub(b.born) < burst_ticks(b.level));
        self.publish(state, origin, tick, &ray);
    }

    /// Mario's world was re-scaled (mega.rs): shots in flight, kept in its old units, go.
    pub fn drop_shots(&mut self) {
        self.shots.clear();
        self.pending = None;
    }

    /// Mario's punch is only throwing a shot (combat.rs: it hits no one).
    pub fn throwing(&self, tick: u32) -> bool {
        self.run_throw.is_some_and(|t| tick.wrapping_sub(t) < THROW_TICKS)
    }

    /// Starts a shot: its throw (the flame comes out when it lets go: `spawn`), or straight away
    /// running or dashing.
    fn shoot(&mut self, id: i32, level: u8, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32) {
        self.ready_at = tick + COOLDOWN[level as usize];
        let (dir, _) = self.aim(state, origin);
        let on_ground = state.action & (ACT_FLAG_STATIONARY | ACT_FLAG_MOVING) != 0 && state.action & ACT_NO_THROW == 0;
        // the throw's kind: by charge level (er_moves.c ER_THROW_FIRE*)
        let kind = crate::moves::THROW_FIRE + match level {
            0 => 0,
            1 | 2 => 1,
            3 => 2,
            _ => 3,
        };
        let throw = if action_air(state.action) {
            Some((crate::moves::ACT_ER_AIR_THROW, kind))
        } else if on_ground && (state.forward_velocity.abs() < RUN_SHOT_SPEED || level >= GIANT) && !crate::moves::dodging(state.action) {
            Some((crate::moves::ACT_ER_THROW, kind))
        } else {
            None
        };
        let face = (dir.x.abs() + dir.z.abs() > 0.01).then(|| dir.x.atan2(dir.z));
        if let Some((action, kind)) = throw {
            worker::call("fireball throw", move |_| unsafe {
                if let Some(f) = face {
                    sm64::sm64_set_mario_faceangle(id, f);
                }
                sm64::sm64_set_mario_action_arg(id, action, kind);
            });
            self.pending = Some((level, tick));
            return;
        }
        // running: a punch on the move throws it, now
        if on_ground && !crate::moves::dodging(state.action) {
            self.run_throw = Some(tick);
            worker::call("fireball run throw", move |_| unsafe { sm64::sm64_set_mario_action(id, ACT_MOVE_PUNCHING) });
        }
        self.spawn(level, state, origin, tick);
    }

    /// Where a shot from his hand goes: the nearest enemy in front (its chest), else straight ahead;
    /// and whether one was found.
    fn aim(&self, state: &sm64::SM64MarioState, origin: [f32; 3]) -> (Vec3, bool) {
        let m = state.position;
        let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
        let hand = [m[0] + fx * 45.0, m[1] + 75.0, m[2] + fz * 45.0];
        let here = sm_to_er(origin, m);
        let aim = combat::nearby(&here, AIM_RANGE, origin)
            .iter()
            .filter(|t| t.shootable().is_some())
            .filter_map(|t| {
                let (dx, dz) = (t.sm[0] - m[0], t.sm[2] - m[2]);
                let d = (dx * dx + dz * dz).sqrt();
                let dy = t.sm[1] + t.height.min(300.0) * 0.45 - hand[1];
                let pitch_ok = combat::aim_pitch_ok(Vec3::new(dx, dy, dz) * crate::scale());
                (d > 1.0 && (dx * fx + dz * fz) / d >= AIM_CONE_COS && pitch_ok).then(|| (d, [t.sm[0], t.sm[1] + t.height.min(300.0) * 0.45, t.sm[2]]))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p);
        match aim {
            Some(p) => (Vec3::new(p[0] - hand[0], p[1] - hand[1], p[2] - hand[2]).normalize_or(Vec3::new(fx, 0.0, fz)), true),
            None => (Vec3::new(fx, 0.0, fz), false),
        }
    }

    /// The flame leaves his hand.
    fn spawn(&mut self, level: u8, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32) {
        let m = state.position;
        let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
        let hand = [m[0] + fx * 45.0, m[1] + if level >= GIANT { 150.0 } else { 75.0 }, m[2] + fz * 45.0];
        let (dir, aimed) = self.aim(state, origin);
        let vel = dir * SPEED[level as usize];
        self.shots.push(Shot { pos: hand, vel: vel.to_array(), level, age: 0, hit: Vec::new(), swept: Vec::new(), stuck: None, last_burn: tick });
        log(format!("fireball: level {level} shot{}", if aimed { ", aimed" } else { "" }));
        // a puff of flame at his hand as it leaves
        self.bursts.push(Burst { pos: er(origin, hand), level: 0, born: tick });
        worker::call("fireball shot", move |_| unsafe {
            sm64::sm64_play_sound_global(SOUND_FLAME_BLOWN);
            if level >= 3 {
                sm64::sm64_play_sound_global(SOUND_VOLCANO);
            }
            // the giant one's recoil: a boom, and Mario holds still a moment
            if level >= GIANT {
                sm64::sm64_play_sound_global(SOUND_BOMB);
                sm64::sm64_er_hitstop(4);
            }
        });
        if level >= GIANT {
            self.bursts.push(Burst { pos: er(origin, hand), level, born: tick });
        }
    }

    fn fly(&mut self, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|shot| {
            if shot.level >= GIANT {
                return self.giant(shot, origin, tick, player, combat, ray);
            }
            let prev = shot.pos;
            let next = [prev[0] + shot.vel[0], prev[1] + shot.vel[1], prev[2] + shot.vel[2]];
            shot.age += 1;
            let r = RADIUS[shot.level as usize];
            // enemies first (one standing against a wall still takes the hit)
            let center = sm_to_er(origin, next);
            let near = combat::nearby(&center, 3.0 + r * scale(), origin);
            let mut hits: Vec<(usize, f32)> = near
                .iter()
                .enumerate()
                .filter(|(_, t)| t.shootable().is_some() && !shot.hit.contains(&t.key()))
                .filter_map(|(i, t)| {
                    // the segment's nearest point to the body's axis (horizontally)
                    let (sx, sz) = (next[0] - prev[0], next[2] - prev[2]);
                    let len2 = (sx * sx + sz * sz).max(1e-3);
                    let k = (((t.sm[0] - prev[0]) * sx + (t.sm[2] - prev[2]) * sz) / len2).clamp(0.0, 1.0);
                    let (px, py, pz) = (prev[0] + sx * k, prev[1] + (next[1] - prev[1]) * k, prev[2] + sz * k);
                    let d = ((t.sm[0] - px).powi(2) + (t.sm[2] - pz).powi(2)).sqrt();
                    (d <= t.radius + r && py + r >= t.sm[1] && py - r <= t.sm[1] + t.height).then_some((i, k))
                })
                .collect();
            hits.sort_by(|a, b| a.1.total_cmp(&b.1));
            for &(i, k) in &hits {
                let t = &near[i];
                shot.hit.push(t.key());
                let v = er_vel(shot.vel);
                // the giant one blows regular enemies away (the landing finishes them)
                let blown = shot.level >= GIANT && t.shootable().is_some_and(|h| combat::blowable(h) && crate::knock::launch(h, v));
                if !blown {
                    combat.deal(player, &near, &[(i, Attack::Fire(shot.level), false)], tick);
                }
                let at = [prev[0] + shot.vel[0] * k, prev[1] + shot.vel[1] * k, prev[2] + shot.vel[2] * k];
                self.bursts.push(Burst { pos: er(origin, at), level: shot.level, born: tick });
                if shot.level >= 3 {
                    sound(SOUND_BOMB);
                    if let Some(h) = t.shootable().filter(|_| !blown) {
                        self.pushes.push(Push { handle: *h, dir: Vec3::new(v.x, 0.0, v.z).normalize_or_zero(), left: PUSH_TICKS });
                    }
                } else {
                    sound(SOUND_FLAME_OUT);
                    return false;
                }
            }
            // the map: a burst where it hits
            if let Some(p) = ray(er(origin, prev), er(origin, next)) {
                self.bursts.push(Burst { pos: p, level: shot.level, born: tick });
                sound(if shot.level >= 3 { SOUND_BOMB } else { SOUND_FLAME_OUT });
                return false;
            }
            shot.pos = next;
            if shot.age as f32 * SPEED[shot.level as usize] >= RANGE {
                self.bursts.push(Burst { pos: er(origin, next), level: 0, born: tick });
                return false;
            }
            true
        });
        self.shots = shots;
    }

    /// The giant one, like a kamehameha: whoever it runs into is swept along inside it, burning a
    /// little at a time; a boss or a big body stops it and it clings to him, burning; it blows up at
    /// the end (a wall, the end of its range, a while on a boss), hurting all around, and whoever
    /// it carried flies off. False once it's gone.
    fn giant(&mut self, shot: &mut Shot, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> bool {
        let prev = shot.pos;
        shot.age += 1;
        let r = RADIUS[shot.level as usize];
        let reach = r * scale();
        // stuck to a boss: it follows his chest (in front of him, where it hit), burning
        let mut blow_up = false;
        if let Some((key, ticks)) = shot.stuck.as_mut() {
            *ticks += 1;
            match combat::chest_by_key(*key) {
                Some(chest) if *ticks < STICK_TICKS => {
                    let c = crate::collision::er_to_sm(origin, &HavokPosition(chest.x, chest.y, chest.z, 0.0));
                    let dir = Vec3::from(shot.vel).with_y(0.0).normalize_or(Vec3::Z);
                    shot.pos = [c[0] - dir.x * r * 0.5, c[1], c[2] - dir.z * r * 0.5];
                }
                _ => blow_up = true,
            }
        }
        let next = if shot.stuck.is_some() { shot.pos } else { [prev[0] + shot.vel[0], prev[1] + shot.vel[1], prev[2] + shot.vel[2]] };
        let center = er(origin, next);
        // whoever it runs into now: swept along, or (a boss, a big body) it sticks to him
        if shot.stuck.is_none() && !blow_up {
            let near = combat::nearby(&HavokPosition(center.x, center.y, center.z, 0.0), 3.0 + reach, origin);
            let fresh: Vec<&combat::Target> = near.iter().filter(|t| t.shootable().is_some() && !shot.hit.contains(&t.key())).collect();
            for t in fresh {
                let d = ((t.sm[0] - next[0]).powi(2) + (t.sm[2] - next[2]).powi(2)).sqrt();
                if d > t.radius + r || next[1] + r < t.sm[1] || next[1] - r > t.sm[1] + t.height {
                    continue;
                }
                let Some(h) = t.shootable().copied() else { continue };
                shot.hit.push(t.key());
                sound(SOUND_BOMB);
                self.bursts.push(Burst { pos: center, level: 3, born: tick });
                if combat::blowable(&h) {
                    shot.swept.push(h);
                } else {
                    log(format!("fireball: the giant one clings to a big one ({} carried)", shot.swept.len()));
                    shot.stuck = Some((t.key(), 0));
                    crate::lakitu::shake(0.8);
                    break;
                }
            }
        }
        // the ones it carries go along with it (unless a wall stops them), and they all burn
        let step = center - er(origin, prev);
        if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
            shot.swept.retain(|h| {
                let Some(chr) = wcm.chr_ins_by_handle_mut(h) else { return false };
                if chr.modules.data.hp <= 0 || crate::carry::is_carried(h) || crate::knock::is_flying(h) {
                    return false;
                }
                let ph = &mut chr.modules.physics;
                let at = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
                let flat = step.with_y(0.0);
                let chest = at + Vec3::Y * 0.8;
                if flat.length() > 1e-3 && ray(chest, chest + flat + flat.normalize() * 0.4).is_none() {
                    let to = at + flat;
                    ph.position = HavokPosition(to.x, to.y, to.z, 0.0);
                    ph.chr_proxy_pos_update_requested = true;
                }
                true
            });
        }
        if tick.wrapping_sub(shot.last_burn) >= BURN_EVERY && (!shot.swept.is_empty() || shot.stuck.is_some()) {
            shot.last_burn = tick;
            let near = combat::nearby(&HavokPosition(center.x, center.y, center.z, 0.0), 6.0 + reach, origin);
            let burning: Vec<(usize, Attack, bool)> = near
                .iter()
                .enumerate()
                .filter(|(_, t)| shot.swept.iter().any(|h| combat::handle_key(h) == t.key()) || shot.stuck.is_some_and(|(k, _)| k == t.key()))
                .map(|(i, _)| (i, Attack::FireSweep, false))
                .collect();
            if !burning.is_empty() {
                combat.deal(player, &near, &burning, tick);
            }
            sound(SOUND_FLAME_BLOWN);
        }
        // the end: a wall, its range, long enough on a boss
        if !blow_up && shot.stuck.is_none() {
            if let Some(p) = ray(er(origin, prev), center) {
                shot.pos = crate::collision::er_to_sm(origin, &HavokPosition(p.x, p.y, p.z, 0.0));
                blow_up = true;
            } else if shot.age as f32 * SPEED[shot.level as usize] >= RANGE {
                blow_up = true;
            }
        }
        if !blow_up {
            shot.pos = next;
            return true;
        }
        // the explosion: everyone around it, and the ones it carried fly off
        let at = er(origin, shot.pos);
        let ground = ray(at, at - Vec3::Y * 6.0).unwrap_or(at - Vec3::Y * reach.min(2.0));
        let n = combat.blast(player, ground, EXPLODE_REACH, Attack::Fire(shot.level), origin, tick);
        let away = er_vel(shot.vel).with_y(0.0).normalize_or(Vec3::Z);
        for h in shot.swept.drain(..) {
            crate::knock::launch(&h, away);
        }
        self.bursts.push(Burst { pos: at, level: shot.level, born: tick });
        crate::impact::fire_blast(ground, EXPLODE_REACH);
        crate::lakitu::shake(1.6);
        sound(SOUND_BOMB);
        sound(SOUND_VOLCANO);
        log(format!("fireball: the giant one blows up ({n} caught in it)"));
        false
    }

    /// The full shot's knockback: regular enemies slide away from Mario (bosses stand their ground)
    fn push(&mut self, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else {
            self.pushes.clear();
            return;
        };
        self.pushes.retain_mut(|p| {
            let Some(chr) = wcm.chr_ins_by_handle_mut(&p.handle) else { return false };
            if chr.modules.data.hp <= 0 || combat::boss_class(&p.handle, chr) || crate::carry::is_carried(&p.handle) || crate::swing::is_down(&p.handle) {
                return false;
            }
            // fast first, easing out
            let step = p.dir * (PUSH * 2.0 / PUSH_TICKS as f32) * (p.left as f32 / PUSH_TICKS as f32);
            let ph = &mut chr.modules.physics;
            let at = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
            let chest = at + Vec3::Y * 0.8;
            if ray(chest, chest + step + p.dir * 0.4).is_some() {
                return false;
            }
            let to = at + step;
            ph.position = HavokPosition(to.x, to.y, to.z, 0.0);
            ph.chr_proxy_pos_update_requested = true;
            p.left -= 1;
            p.left > 0
        });
    }

    fn publish(&self, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let mut out = Vec::new();
        // (the camera: a shot behind a wall isn't drawn over it, and the flames are drawn back to
        // front)
        let eye = camera();
        let seen = |p: Vec3, size: f32| eye.is_none_or(|e| ray(e, p).is_none_or(|h| (h - p).length() < size * 0.5 + 0.2));
        // the charge: sparks of flame gathering at Mario's hand, the flame there growing
        if let Some(since) = self.held_since {
            let held = tick.saturating_sub(since.max(self.ready_at));
            if held >= SHOW_CHARGE_AFTER || self.level > 0 {
                let m = state.position;
                let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
                let hand = er(origin, [m[0] + fx * 45.0, m[1] + 75.0, m[2] + fz * 45.0]);
                let vel = er_vel(state.velocity);
                let level = self.level as usize;
                let pulse = 1.0 + 0.12 * (tick as f32 * 0.9).sin();
                let grow = match level {
                    0 => (held as f32 / CHARGE_TICKS[0] as f32).clamp(0.3, 1.0),
                    // from 4 s on it keeps growing towards the giant one
                    3 => {
                        let k = (held - CHARGE_TICKS[2]) as f32 / (CHARGE_TICKS[3] - CHARGE_TICKS[2]) as f32;
                        1.0 + k.clamp(0.0, 1.0) * (SIZE[4] / SIZE[3] - 1.0) * 0.85
                    }
                    _ => 1.0,
                };
                let size = SIZE[level] * 0.7 * pulse * grow;
                // (a big one is held up over his head, not around him)
                let hand = hand + Vec3::Y * (size * 0.5 - 0.3).max(0.0);
                if self.level >= GIANT {
                    giant(&mut out, hand, vel, size, tick, tick);
                } else {
                    // (held still in his hand, its flames rise)
                    ball(&mut out, hand, vel, size, self.level, tick, tick, Vec3::Y);
                }
                // (more, faster sparks with each level)
                let (count, period) = [(4, 10), (6, 8), (8, 7), (10, 6), (18, 5)][level];
                for i in 0..count {
                    let phase = ((tick + i * period / count) % period) as f32 / period as f32;
                    let angle = i as f32 * 2.399 + (tick / period) as f32 * 0.9;
                    let reach = (1.1 * (1.0 - phase) + 0.1) * size.max(1.0);
                    let off = Vec3::new(angle.cos() * reach, (angle * 1.7).sin() * reach * 0.6, angle.sin() * reach);
                    out.push(Sprite {
                        pos: hand + off,
                        vel,
                        size: 0.12 + 0.1 * level as f32 * phase,
                        frame: (i as usize + tick as usize) % 8,
                        tint: { let mut c = tint(self.level, tick + i); c[3] = 0.35 + 0.65 * phase; c },
                    });
                }
            }
        }
        for shot in &self.shots {
            let pos = er(origin, shot.pos);
            // (clinging to a boss it stands still, churning bigger)
            let stuck = shot.stuck.is_some();
            let vel = if stuck { Vec3::ZERO } else { er_vel(shot.vel) };
            let level = shot.level as usize;
            let size = SIZE[level] * if stuck { 1.12 + 0.12 * (tick as f32 * 2.1).sin() } else { 1.0 + 0.08 * (tick as f32 * 1.3 + shot.age as f32).sin() };
            if !seen(pos, size) {
                continue;
            }
            if shot.level >= GIANT {
                giant(&mut out, pos, vel, size, shot.age, tick);
                continue;
            }
            ball(&mut out, pos, vel, size, shot.level, shot.age, tick, (-vel).normalize_or(Vec3::Y));
        }
        for b in &self.bursts {
            if !seen(b.pos, SIZE[b.level as usize]) {
                continue;
            }
            let age = tick.wrapping_sub(b.born) as f32 / burst_ticks(b.level) as f32;
            let level = b.level as usize;
            let spread = SIZE[level] * (0.4 + 1.2 * age);
            let mut c = tint(b.level, tick);
            c[3] = 1.0 - age;
            out.push(Sprite { pos: b.pos, vel: Vec3::ZERO, size: SIZE[level] * (1.2 - 0.6 * age), frame: (age * 8.0) as usize % 8, tint: c });
            // (the giant one: a bigger ring and a column of fire rising)
            let ring = if b.level >= GIANT { 16 } else { 6 };
            if b.level >= GIANT {
                out.push(Sprite { pos: b.pos, vel: Vec3::ZERO, size: SIZE[level] * (1.5 + 2.0 * age), frame: FRAME_GLOW, tint: [1.0, 0.55, 0.15, 0.7 * (1.0 - age)] });
                for i in 0..14 {
                    let a = i as f32 * 2.399;
                    let dir = Vec3::new(a.cos(), 0.4 + 0.6 * ((a * 3.1).sin() * 0.5 + 0.5), a.sin());
                    out.push(Sprite { pos: b.pos + dir * spread * 1.4, vel: Vec3::ZERO, size: 0.18, frame: FRAME_GLOW, tint: [1.0, 0.85, 0.4, 1.0 - age] });
                }
                for i in 0..8 {
                    let up = Vec3::Y * spread * (0.2 + 0.25 * i as f32) * (0.5 + age);
                    out.push(Sprite { pos: b.pos + up, vel: Vec3::ZERO, size: SIZE[level] * 0.5 * (1.0 - age), frame: (i + (age * 8.0) as usize) % 8, tint: c });
                }
            }
            for i in 0..ring {
                let a = i as f32 * std::f32::consts::TAU / ring as f32 + level as f32;
                let off = Vec3::new(a.cos(), 0.5 + 0.4 * (a * 2.0).sin(), a.sin()) * spread * 0.6;
                out.push(Sprite { pos: b.pos + off, vel: Vec3::ZERO, size: SIZE[level] * 0.45 * (1.0 - age * 0.5), frame: (i + (age * 8.0) as usize) % 8, tint: c });
            }
        }
        // Mega Mario's fireballs are a giant's (their reach and speed are in his scaled units already)
        let k = crate::size_factor();
        if k != 1.0 {
            for s in &mut out {
                s.size *= k;
            }
        }
        // back to front (a halo a little behind its fire), so the flames licking round a ball's far
        // side go behind it and the near ones over it: it reads as a ball, not a flat picture
        if let Some(e) = eye {
            let depth = |s: &Sprite| (s.pos - e).length() + if s.frame == FRAME_GLOW && s.tint[3] < 0.6 { s.size * 0.5 } else { 0.0 };
            out.sort_by(|a, b| depth(b).total_cmp(&depth(a)));
        }
        *SPRITES.lock().unwrap_or_else(|e| e.into_inner()) = (out, Some(Instant::now()));
    }
}

/// Where the game's render camera is (Lakitu's, when on).
fn camera() -> Option<Vec3> {
    let cam = unsafe { eldenring::cs::CSCamera::instance() }.ok()?;
    let m = &cam.pers_cam_1.matrix;
    Some(Vec3::new(m.3.0, m.3.1, m.3.2))
}

/// A shot (or the charge in his hand): a lit ball of fire in its glow with a white-hot heart,
/// flames licking all round it on a sphere turning about its flight (some pass in front of it,
/// some behind: publish draws them back to front), a tail of flames shed behind it (`tail`: the way
/// back), cooling as they go (yellow, orange, red, a wisp of smoke), and embers flying off.
#[allow(clippy::too_many_arguments)]
fn ball(out: &mut Vec<Sprite>, pos: Vec3, vel: Vec3, size: f32, level: u8, age: u32, tick: u32, tail: Vec3) {
    let l = level.min(3) as usize;
    let flicker = 1.0 + 0.07 * (tick as f32 * 2.3).sin();
    let hot = tint(level, tick);
    out.push(Sprite { pos, vel, size: size * 2.3 * flicker, frame: FRAME_GLOW, tint: [hot[0], hot[1] * 0.75, hot[2] * 0.6, 0.28] });
    out.push(Sprite { pos, vel, size: size * 1.45 * flicker, frame: FRAME_GLOW, tint: [1.0, 0.68, 0.28, 0.5] });
    out.push(Sprite { pos, vel, size: size * 0.8, frame: FRAME_ORB + (age / 2) as usize % 4, tint: [1.0, 1.0, 1.0, 1.0] });
    out.push(Sprite { pos, vel, size: size * 0.5, frame: (age / 2 + 4) as usize % 8, tint: [1.0, 0.97, 0.82, 0.9] });
    // (a frame about the way back: the licks turn round it)
    let axis = tail.normalize_or(Vec3::Y);
    let u = axis.any_orthonormal_vector();
    let w = axis.cross(u);
    let licks = [6, 7, 8, 10][l];
    for i in 0..licks {
        let a = i as f32 * std::f32::consts::TAU / licks as f32 + age as f32 * 0.5;
        let lat = (i as f32 * 2.399).sin() * 0.7;
        let dir = (u * a.cos() + w * a.sin()) * lat.cos() + axis * (lat.sin() * 0.6);
        let reach = size * (0.36 + 0.06 * ((age + i) as f32 * 1.7).sin());
        out.push(Sprite {
            pos: pos + dir * reach + axis * size * 0.18,
            vel,
            size: size * (0.38 + 0.08 * (i % 3) as f32),
            frame: (i as usize + age as usize / 2) % 8,
            tint: [1.0, 0.72 + 0.1 * (i % 2) as f32, 0.32, 0.88],
        });
    }
    // the tail: older flames farther back, smaller, cooler, rising a little and swaying
    let step = (vel.length() / 30.0).max(size * 0.3);
    let tail_n = [5, 6, 7, 8][l];
    for k in 1..=tail_n {
        let t = k as f32 / tail_n as f32;
        let sway = (u * ((age + k) as f32 * 0.9).sin() + w * ((age + k) as f32 * 1.3).cos()) * size * 0.12 * t;
        let p = pos + axis * step * k as f32 * 0.8 + Vec3::Y * t * t * size * 0.5 + sway;
        let c = [1.0, 0.85 - 0.55 * t, 0.45 - 0.37 * t, 0.85 * (1.0 - t) + 0.1];
        out.push(Sprite { pos: p, vel, size: size * (0.85 - 0.5 * t), frame: (age as usize + k as usize * 3) % 8, tint: c });
    }
    if l >= 2 {
        let p = pos + axis * step * (tail_n as f32 + 1.5) * 0.8 + Vec3::Y * size * 0.5;
        out.push(Sprite { pos: p, vel: vel * 0.5, size: size * 0.9, frame: FRAME_GLOW, tint: [0.12, 0.08, 0.06, 0.25] });
    }
    // embers
    for i in 0..(3 + 2 * l as u32) {
        let t = ((age + i * 7) % 18) as f32 / 18.0;
        let a = i as f32 * 2.399;
        let side = (u * a.cos() + w * a.sin()) * size * 0.6 * (0.4 + t);
        out.push(Sprite {
            pos: pos + axis * step * t * 4.0 + side + Vec3::Y * t * size * 0.7,
            vel: vel * 0.4,
            size: 0.05 + 0.04 * (1.0 - t) * (1.0 + l as f32 * 0.5),
            frame: FRAME_GLOW,
            tint: [1.0, 0.85 - 0.35 * t, 0.4 - 0.25 * t, 1.0 - t],
        });
    }
}

/// In the air, in an action SM64's air throw can take over (not a ground pound, not hurt).
fn action_air(action: u32) -> bool {
    action & ACT_FLAG_AIR != 0 && action != ACT_GROUND_POUND && action & (ACT_FLAG_INVULNERABLE | ACT_NO_THROW) == 0
}

/// The giant shot (or its charge): a churning ball of fire in a glow, flames licking round its
/// rim, embers shed behind it drifting up and a little smoke, Elden Ring's fire more than SM64's.
/// `age` drives its churn, `tick` the flicker.
fn giant(out: &mut Vec<Sprite>, pos: Vec3, vel: Vec3, size: f32, age: u32, tick: u32) {
    let flicker = 1.0 + 0.06 * (tick as f32 * 2.1).sin();
    // (a smudge of smoke behind it, then the glow, the ball, the flames on top)
    let back = (-vel).normalize_or(Vec3::Y);
    for i in 0..5u32 {
        let t = ((age + i * 4) % 20) as f32 / 20.0;
        let off = back * size * (0.4 + t * 1.6) + Vec3::Y * t * size * 0.6;
        out.push(Sprite { pos: pos + off, vel: vel * 0.6, size: size * (0.6 + t * 0.8), frame: FRAME_GLOW, tint: [0.12, 0.08, 0.06, 0.35 * (1.0 - t)] });
    }
    out.push(Sprite { pos, vel, size: size * 2.0 * flicker, frame: FRAME_GLOW, tint: [1.0, 0.45, 0.1, 0.35] });
    out.push(Sprite { pos, vel, size: size * 1.4 * flicker, frame: FRAME_GLOW, tint: [1.0, 0.65, 0.25, 0.55] });
    out.push(Sprite { pos, vel, size, frame: FRAME_ORB + (age / 3) as usize % 4, tint: [1.0, 1.0, 1.0, 1.0] });
    for i in 0..8u32 {
        let a = i as f32 * std::f32::consts::TAU / 8.0 + age as f32 * 0.35;
        let off = Vec3::new(a.cos(), (a * 1.3).sin() * 0.7, a.sin()) * size * 0.42;
        out.push(Sprite { pos: pos + off, vel, size: size * 0.32, frame: (i + age) as usize % 8, tint: [1.0, 0.7, 0.3, 0.85] });
    }
    for i in 0..16u32 {
        let t = ((age + i * 5) % 16) as f32 / 16.0;
        let a = i as f32 * 2.399;
        let side = Vec3::new(a.cos(), (a * 0.7).sin(), a.sin()) * size * 0.5;
        out.push(Sprite {
            pos: pos + back * size * t * 2.0 + side * (1.0 + t) + Vec3::Y * t * size * 0.8,
            vel: vel * 0.3,
            size: 0.07 + 0.08 * (1.0 - t),
            frame: FRAME_GLOW,
            tint: [1.0, 0.8 - 0.3 * t, 0.35 - 0.2 * t, 1.0 - t],
        });
    }
}
