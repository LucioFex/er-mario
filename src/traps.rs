//! The trap kit (LT held while Mario crouches, plus a button): real 3D things (assets/kit.rs) he
//! tosses, kicks, lobs and sets down, that work on their own once out:
//! - X, a banana: tossed a little in front, it lies there; a regular enemy stepping on it slips and
//!   falls (the game's own knockdown), a big one only staggers, a boss walks over it. Two at most
//!   on the floor, each back 10 s after it's used.
//! - Y, the green shell: kicked along the floor, it follows the ground, bounces off walls and off
//!   every enemy it hits (a knockdown for the regular ones), gone after four. Y held: the red
//!   shell, slower, homing in on the nearest enemy, gone after two.
//! - B, a Bob-omb: lobbed, it bounces and rolls, its fuse blinking faster, and goes off after 2 s
//!   (or on the first enemy it touches): a blast all around that sends the regular ones up in the
//!   air (an air combo's start).
//! - RB, a spring: set down in front, it stays a while; an enemy stepping on it is sent up (juggle),
//!   Mario landing on it is sent up far higher than any jump.
//! Not on bosses: a banana or a spring is nothing to them, a shell or a blast only its damage.

use std::time::Instant;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use glam::{Quat, Vec3};

use crate::combat::{self, Attack, Combat};
use crate::engine_mario::{KIT_BANANA, KIT_BOMB, KIT_GREEN_SHELL, KIT_RED_SHELL, KIT_SPRING, PARTS, PartPose};
use crate::{log, moves, sm64, worker};

const DT: f32 = 1.0 / 30.0;
const GRAVITY: f32 = 16.0;
/// how long until each comes back (s): a banana each, the shells, the Bob-omb, the spring
const BANANA_BACK: f32 = 10.0;
const SHELL_BACK: f32 = 8.0;
const BOMB_BACK: f32 = 12.0;
const SPRING_BACK: f32 = 15.0;
/// how long they last out there (s), and the farthest from Mario (m) before one is gone
const BANANA_LIFE: f32 = 45.0;
const SHELL_LIFE: f32 = 8.0;
const SPRING_LIFE: f32 = 15.0;
const FUSE: f32 = 2.0;
const AWAY: f32 = 30.0;
/// Y held this long (ticks) with LT: the red shell
const RED_HOLD: u32 = 8;
/// a shell's speed (m/s), its hits before it's gone, how fast the red one turns (rad/s)
const GREEN_SPEED: f32 = 10.0;
const RED_SPEED: f32 = 8.0;
const GREEN_HITS: u32 = 4;
const RED_HITS: u32 = 2;
const RED_TURN: f32 = 3.5;
/// the blast's reach (m); the spring's super jump (SM64 units per tick up)
const BLAST: f32 = 3.5;
const SPRING_JUMP: f32 = 76.0;
/// the throw is let go this soon after it was asked for (ticks), or it's dropped
const THROW_WAIT: u32 = 30;

// SM64's sounds (audio_defines.h)
const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_BOING: i32 = 0x306C_4081;
const SOUND_BOING2: i32 = 0x306D_4081;
const SOUND_EXPLOSION: i32 = 0x802E_2081u32 as i32;
const SOUND_FUSE: i32 = 0x6008_6001;
const SOUND_BONK: i32 = 0x0445_A081;
const SOUND_BODY_HIT_GROUND: i32 = 0x0418_8081;
const SOUND_HIT: i32 = 0x0444_C081;
const SOUND_BREAK: i32 = 0x3041_C081;
const SOUND_POUND_ROCK: i32 = 0x3056_0081;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Banana,
    Green,
    Red,
    Bomb,
    Spring,
}

impl Kind {
    fn throw(self) -> u32 {
        match self {
            Kind::Banana => moves::THROW_BANANA,
            Kind::Green | Kind::Red => moves::THROW_SHELL,
            Kind::Bomb => moves::THROW_BOMB,
            Kind::Spring => moves::THROW_SPRING,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// in the air (tossed, lobbed, off a ledge)
    Flying,
    /// on the floor: lying (banana, spring) or sliding / rolling (shells, Bob-omb)
    Down,
}

struct Item {
    kind: Kind,
    part: usize,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    spin: f32,
    age: f32,
    state: State,
    hits: u32,
    /// enemies it hit lately (key, its age then), and every enemy it hit (for "CAROM!")
    hit: Vec<(u64, f32)>,
    struck: Vec<u64>,
    /// the spring's squash (0 at rest, 1 pressed flat), and its springing back
    squash: f32,
    squash_vel: f32,
    gone: bool,
}

pub struct Traps {
    items: Vec<Item>,
    /// when each comes back (s since `born`): the two bananas, the shells, the Bob-omb, the spring
    bananas: [f32; 2],
    shell: f32,
    bomb: f32,
    spring: f32,
    born: Instant,
    held: [bool; 4],
    y_since: Option<u32>,
    /// a throw asked for: what, since when (it goes out when Mario's throw lets go)
    pending: Option<(Kind, u32)>,
    /// Mario landed on the spring: his jump, for the next tick
    bounce: Option<f32>,
    bounced_at: f32,
}

/// Mario is crouching (or crawling): LT and a button are the trap kit then.
pub fn crouching(action: u32) -> bool {
    const ACT_CROUCHING: u32 = 0x0C00_8220;
    const ACT_START_CROUCHING: u32 = 0x0C00_8221;
    const ACT_STOP_CROUCHING: u32 = 0x0C00_8222;
    const ACT_START_CRAWLING: u32 = 0x0C00_8223;
    const ACT_STOP_CRAWLING: u32 = 0x0C00_8224;
    const ACT_CRAWLING: u32 = 0x0400_8448;
    matches!(action, ACT_CROUCHING | ACT_START_CROUCHING | ACT_STOP_CROUCHING | ACT_START_CRAWLING | ACT_STOP_CRAWLING | ACT_CRAWLING)
}

fn sound(id: i32, volume: f32) {
    worker::call("trap sound", move |_| unsafe {
        if volume < 1.0 {
            sm64::sm64_play_sound_volume(id, volume);
        } else {
            sm64::sm64_play_sound_global(id);
        }
    });
}

impl Traps {
    pub fn new() -> Self {
        Traps {
            items: Vec::new(),
            bananas: [0.0; 2],
            shell: 0.0,
            bomb: 0.0,
            spring: 0.0,
            born: Instant::now(),
            held: [false; 4],
            y_since: None,
            pending: None,
            bounce: None,
            bounced_at: -10.0,
        }
    }

    fn now(&self) -> f32 {
        self.born.elapsed().as_secs_f32()
    }

    /// What's ready (for the HUD): each kit button's share of its wait done (1 ready): X, Y, B, RB.
    pub fn ready(&self) -> [f32; 4] {
        let now = self.now();
        let left = |back: f32, at: f32| if at <= now { 1.0 } else { 1.0 - (at - now) / back };
        let banana = self.bananas.iter().map(|&b| left(BANANA_BACK, b)).fold(0.0f32, f32::max);
        [banana, left(SHELL_BACK, self.shell), left(BOMB_BACK, self.bomb), left(SPRING_BACK, self.spring)]
    }

    fn available(&self, kind: Kind) -> bool {
        let now = self.now();
        match kind {
            Kind::Banana => self.bananas.iter().any(|&b| b <= now) && self.items.iter().filter(|i| i.kind == Kind::Banana).count() < 2,
            Kind::Green | Kind::Red => self.shell <= now && !self.items.iter().any(|i| matches!(i.kind, Kind::Green | Kind::Red)),
            Kind::Bomb => self.bomb <= now && !self.items.iter().any(|i| i.kind == Kind::Bomb),
            Kind::Spring => self.spring <= now,
        }
    }

    /// Game thread, every SM64 tick: whether the kit is up (LT held, crouching, hands free), X, Y,
    /// B and RB held. The throw to start, if any.
    pub fn input(&mut self, on: bool, buttons: [bool; 4], tick: u32) -> Option<moves::Request> {
        let pressed: Vec<bool> = (0..4).map(|k| buttons[k] && !self.held[k]).collect();
        let released_y = !buttons[1] && self.held[1];
        self.held = buttons;
        if !on {
            self.y_since = None;
            return None;
        }
        if self.pending.is_some_and(|(_, at)| tick.wrapping_sub(at) > THROW_WAIT) {
            self.pending = None;
        }
        if self.pending.is_some() {
            return None;
        }
        if pressed[1] {
            self.y_since = Some(tick);
        }
        let kind = if pressed[0] {
            Some(Kind::Banana)
        } else if pressed[2] {
            Some(Kind::Bomb)
        } else if pressed[3] {
            Some(Kind::Spring)
        } else if let Some(since) = self.y_since {
            // a tap: the green shell; held: the red
            if released_y {
                self.y_since = None;
                Some(Kind::Green)
            } else if tick.wrapping_sub(since) >= RED_HOLD {
                self.y_since = None;
                Some(Kind::Red)
            } else {
                None
            }
        } else {
            None
        };
        let kind = kind?;
        if !self.available(kind) {
            sound(SOUND_BUZZ, 1.0);
            log(format!("traps: {kind:?} not ready"));
            return None;
        }
        self.pending = Some((kind, tick));
        Some(moves::Request::Action(moves::ACT_ER_THROW, kind.throw()))
    }

    /// The spring's super jump, if Mario just landed on it (applied before the next SM64 tick).
    pub fn take_bounce(&mut self) -> Option<f32> {
        self.bounce.take()
    }

    /// Game thread, after every SM64 tick: what Mario's throw did (`mv`), his state, feet and hand
    /// (Elden Ring). Puts the things out, moves them, lands their hits.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        mv: &sm64::SM64ErMove,
        state: &sm64::SM64MarioState,
        feet: Vec3,
        hand: Vec3,
        origin: [f32; 3],
        tick: u32,
        player: &ChrIns,
        combat: &mut Combat,
        ray: impl Fn(Vec3, Vec3) -> Option<Vec3>,
    ) {
        let ahead = Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
        if let Some((kind, _)) = self.pending {
            if mv.throw_kind > 0 && mv.throw_kind as u32 - 1 == kind.throw() {
                self.pending = None;
                self.spawn(kind, feet, hand, ahead, &ray);
            } else if state.action != moves::ACT_ER_THROW && !crouching(state.action) {
                // (the throw never came: hurt, jumped out of it)
                self.pending = None;
            }
        }
        let now = self.now();
        let mut items = std::mem::take(&mut self.items);
        for it in items.iter_mut() {
            it.age += DT;
            it.hit.retain(|(_, at)| it.age - *at < 0.5);
            let life = match it.kind {
                Kind::Banana => BANANA_LIFE,
                Kind::Green | Kind::Red => SHELL_LIFE,
                Kind::Bomb => FUSE,
                Kind::Spring => SPRING_LIFE,
            };
            if (it.pos - feet).length() > AWAY && it.kind != Kind::Bomb {
                it.gone = true;
            }
            if it.age >= life {
                if it.kind == Kind::Bomb {
                    explode(it, player, combat, origin, tick);
                } else {
                    crate::impact::poof(it.pos + Vec3::Y * 0.2);
                }
                it.gone = true;
                continue;
            }
            match it.kind {
                Kind::Banana => {
                    fly(it, &ray, 0.0);
                    if it.state == State::Down {
                        banana_trip(it, player, combat, origin, tick);
                    }
                }
                Kind::Green | Kind::Red => shell(it, &ray, player, combat, origin, tick),
                Kind::Bomb => {
                    fly(it, &ray, 0.35);
                    // rolling on: slowing down
                    if it.state == State::Down {
                        it.vel *= 0.93;
                    }
                    it.spin += it.vel.with_y(0.0).length() * DT / crate::assets::kit::BOMB_R;
                    // (a touch is enough: it goes off on the first enemy)
                    let near = combat::nearby(&HavokPosition(it.pos.x, it.pos.y, it.pos.z, 0.0), 2.0, origin);
                    if it.age > 0.15 && near.iter().any(|t| t.shootable().is_some() && touching(t, it.pos, 0.3, origin)) {
                        explode(it, player, combat, origin, tick);
                        it.gone = true;
                    }
                }
                Kind::Spring => {
                    fly(it, &ray, 0.0);
                    // springing back to shape
                    it.squash_vel += (-it.squash * 260.0 - it.squash_vel * 9.0) * DT;
                    it.squash += it.squash_vel * DT;
                    if it.state == State::Down {
                        let top = it.pos + Vec3::Y * crate::assets::kit::SPRING_H;
                        // Mario coming down on it (or walking onto it): up he goes
                        let d = (feet - it.pos).with_y(0.0).length();
                        let falling = state.velocity[1] <= 0.0;
                        if d < 0.45 && feet.y > it.pos.y - 0.1 && feet.y < top.y + 0.35 && falling && now - self.bounced_at > 0.5 {
                            self.bounced_at = now;
                            self.bounce = Some(SPRING_JUMP);
                            it.squash = 0.6;
                            it.squash_vel = -2.0;
                            log("traps: Mario off the spring");
                            crate::impact::boing(top);
                            sound(SOUND_BOING, 1.0);
                        }
                        spring_launch(it, player, combat, origin, tick, feet);
                    }
                }
            }
        }
        items.retain(|i| !i.gone);
        self.items = items;
    }

    /// Puts one out: from his hand (the banana, the Bob-omb), at his feet (the shells), in front of
    /// him on the floor (the spring).
    fn spawn(&mut self, kind: Kind, feet: Vec3, hand: Vec3, ahead: Vec3, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let now = self.now();
        let (part, pos, vel) = match kind {
            Kind::Banana => {
                let slot = (0..2).find(|&k| !self.items.iter().any(|i| i.part == KIT_BANANA + k)).unwrap_or(0);
                if let Some(b) = self.bananas.iter_mut().filter(|b| **b <= now).next() {
                    *b = now + BANANA_BACK;
                }
                (KIT_BANANA + slot, hand, ahead * 3.2 + Vec3::Y * 2.8)
            }
            Kind::Green => {
                self.shell = now + SHELL_BACK;
                (KIT_GREEN_SHELL, feet + ahead * 0.55 + Vec3::Y * 0.05, ahead * GREEN_SPEED)
            }
            Kind::Red => {
                self.shell = now + SHELL_BACK;
                (KIT_RED_SHELL, feet + ahead * 0.55 + Vec3::Y * 0.05, ahead * RED_SPEED)
            }
            Kind::Bomb => {
                self.bomb = now + BOMB_BACK;
                sound(SOUND_FUSE, 0.35);
                (KIT_BOMB, hand, ahead * 6.5 + Vec3::Y * 5.0)
            }
            Kind::Spring => {
                self.spring = now + SPRING_BACK;
                // (one at a time: the old one goes)
                for i in self.items.iter_mut().filter(|i| i.kind == Kind::Spring) {
                    i.gone = true;
                }
                let at = feet + ahead * 1.0;
                let ground = ray(at + Vec3::Y * 1.0, at - Vec3::Y * 2.0).unwrap_or(at);
                sound(SOUND_POUND_ROCK, 0.4);
                (KIT_SPRING, ground, Vec3::ZERO)
            }
        };
        log(format!("traps: {kind:?} out"));
        crate::impact::kit_out(pos);
        self.items.push(Item {
            kind,
            part,
            pos,
            vel,
            yaw: ahead.x.atan2(ahead.z),
            spin: 0.0,
            age: 0.0,
            state: if kind == Kind::Spring { State::Down } else { State::Flying },
            hits: 0,
            hit: Vec::new(),
            struck: Vec::new(),
            squash: if kind == Kind::Spring { 0.5 } else { 0.0 },
            squash_vel: 0.0,
            gone: false,
        });
    }

    /// Puts the things on their models: where each is, turned and squashed as it goes; the rest
    /// hidden in his hand (as relative_parts left them).
    pub fn pose(&self, parts: &mut [PartPose; PARTS], feet: Vec3) {
        for it in &self.items {
            let k = crate::assets::kit::BOMB_R;
            let (rot, stretch, scale) = match it.kind {
                // (a pop as it lands, then lying still, a slow wobble)
                Kind::Banana => (Quat::from_rotation_y(it.yaw), Vec3::ONE, pop(it.age)),
                // spinning as it slides
                Kind::Green | Kind::Red => (Quat::from_rotation_y(it.spin), Vec3::ONE, pop(it.age)),
                Kind::Bomb => {
                    // rolling along its way, swelling in time with the fuse as it runs out
                    let dir = it.vel.with_y(0.0).normalize_or(Vec3::new(it.yaw.sin(), 0.0, it.yaw.cos()));
                    let axis = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                    let left = (FUSE - it.age).max(0.0);
                    let beat = (it.age * (6.0 + 22.0 * (1.0 - left / FUSE))).sin().abs();
                    let swell = 1.0 + 0.12 * beat * (1.0 - left / FUSE);
                    let rot = Quat::from_axis_angle(axis, it.spin) * Quat::from_rotation_y(it.yaw);
                    (rot, Vec3::splat(swell), pop(it.age))
                }
                Kind::Spring => {
                    let s = it.squash.clamp(-0.4, 0.8);
                    (Quat::from_rotation_y(it.yaw), Vec3::new(1.0 + 0.25 * s, 1.0 - s, 1.0 + 0.25 * s), 1.0)
                }
            };
            // (the Bob-omb rolls about its middle: its pose point stays at its feet)
            let pos = if it.kind == Kind::Bomb {
                let c = it.pos + Vec3::Y * k;
                c - rot * (Vec3::Y * k)
            } else {
                it.pos
            };
            parts[it.part] = PartPose { rot, pos: pos - feet, scale, stretch };
        }
    }

    /// Everything out there is gone (Mario's death, out of Mario mode).
    pub fn clear(&mut self) {
        self.items.clear();
        self.pending = None;
        self.bounce = None;
    }
}

/// Out of nothing to full size with a little overshoot.
fn pop(age: f32) -> f32 {
    if age < 0.1 {
        0.4 + 0.75 * age / 0.1
    } else if age < 0.22 {
        1.15 - 0.15 * (age - 0.1) / 0.12
    } else {
        1.0
    }
}

/// An enemy's body touches a point (Elden Ring) within `reach` (m).
fn touching(t: &combat::Target, p: Vec3, reach: f32, _origin: [f32; 3]) -> bool {
    let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
    let (r, h) = (t.radius * crate::scale(), t.height * crate::scale());
    (foot - p).with_y(0.0).length() <= r + reach && p.y >= foot.y - reach - 0.3 && p.y <= foot.y + h + reach
}

/// Falls under gravity until it meets the floor (`bounce`: the share of its fall it bounces back
/// up with), then lies or slides on it, following it down slopes and off ledges.
fn fly(it: &mut Item, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>, bounce: f32) {
    match it.state {
        State::Flying => {
            it.vel.y -= GRAVITY * DT;
            let next = it.pos + it.vel * DT;
            // into a wall on the way: back off it
            if let Some(_wall) = ray(it.pos + Vec3::Y * 0.2, next + Vec3::Y * 0.2).filter(|_| it.vel.with_y(0.0).length() > 0.5) {
                it.vel.x *= -0.4;
                it.vel.z *= -0.4;
                return;
            }
            match ray(it.pos + Vec3::Y * 0.3, next - Vec3::Y * 0.05).filter(|_| it.vel.y <= 0.0) {
                Some(ground) => {
                    it.pos = ground;
                    if bounce > 0.0 && it.vel.y < -3.0 {
                        it.vel.y = -it.vel.y * bounce;
                        it.vel.x *= 0.7;
                        it.vel.z *= 0.7;
                        sound(SOUND_BODY_HIT_GROUND, 0.25);
                    } else {
                        it.state = State::Down;
                        it.vel.y = 0.0;
                        if bounce == 0.0 {
                            it.vel = Vec3::ZERO;
                        }
                        crate::impact::thud_small(ground);
                    }
                }
                None => it.pos = next,
            }
        }
        State::Down => {
            let flat = it.vel.with_y(0.0);
            if flat.length() < 0.05 {
                return;
            }
            let next = it.pos + flat * DT;
            if ray(it.pos + Vec3::Y * 0.2, next + Vec3::Y * 0.2).is_some() {
                it.vel.x *= -0.5;
                it.vel.z *= -0.5;
                return;
            }
            // (the floor under where it's going: down a slope, up a step, off a ledge)
            match ray(next + Vec3::Y * 0.5, next - Vec3::Y * 0.8) {
                Some(ground) => it.pos = ground,
                None => {
                    it.pos = next;
                    it.state = State::Flying;
                }
            }
        }
    }
}

/// A regular enemy steps on the banana: down it goes. A big one staggers, a boss walks on.
fn banana_trip(it: &mut Item, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
    let near = combat::nearby(&HavokPosition(it.pos.x, it.pos.y, it.pos.z, 0.0), 2.5, origin);
    for (i, t) in near.iter().enumerate() {
        let Some(h) = t.shootable() else { continue };
        let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
        let r = t.radius * crate::scale();
        if (foot - it.pos).with_y(0.0).length() > 0.35 + r * 0.6 || (foot.y - it.pos.y).abs() > 0.6 {
            continue;
        }
        // (a boss walks over it, and so does anything huge)
        if combat::is_boss_now(h) || r > 1.4 {
            continue;
        }
        log(format!("traps: an enemy slipped on the banana ({})", if combat::liftable(h) { "down" } else { "staggered" }));
        combat.deal(player, &near, &[(i, Attack::Slip, false)], tick);
        crate::style::callout("SLIP!", Some(foot + Vec3::Y * 1.2), [1.0, 0.9, 0.25], false, 2.0);
        crate::impact::slip(foot);
        sound(SOUND_BODY_HIT_GROUND, 0.6);
        it.gone = true;
        return;
    }
}

/// The shell's slide: along the floor, homing (the red one), off the walls, into the enemies.
fn shell(it: &mut Item, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
    let red = it.kind == Kind::Red;
    let speed = if red { RED_SPEED } else { GREEN_SPEED };
    it.spin += DT * 18.0;
    let near = combat::nearby(&HavokPosition(it.pos.x, it.pos.y, it.pos.z, 0.0), 12.0, origin);
    // the red one turns to the nearest enemy it hasn't just hit
    if red {
        let target = near
            .iter()
            .filter(|t| t.shootable().is_some() && !it.hit.iter().any(|(k, _)| *k == t.key()))
            .map(|t| Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2))
            .min_by(|a, b| (*a - it.pos).length_squared().total_cmp(&(*b - it.pos).length_squared()));
        if let Some(to) = target {
            let want = (to - it.pos).with_y(0.0).normalize_or(it.vel.normalize_or(Vec3::Z));
            let have = it.vel.with_y(0.0).normalize_or(want);
            let angle = have.angle_between(want).min(RED_TURN * DT);
            let side = have.cross(want).y.signum();
            let turned = Quat::from_rotation_y(angle * side) * have;
            it.vel = turned * speed + Vec3::Y * it.vel.y;
        }
    }
    // (on the floor it keeps its speed: it's a Koopa shell)
    if it.state == State::Down {
        let flat = it.vel.with_y(0.0).normalize_or(Vec3::Z) * speed;
        it.vel = Vec3::new(flat.x, it.vel.y, flat.z);
    }
    let before = it.vel;
    fly(it, ray, 0.0);
    if it.state == State::Down && it.vel.with_y(0.0).length() < before.with_y(0.0).length() * 0.9 {
        // off a wall: back the way it came, a little to one side
        let back = Quat::from_rotation_y((crate::rand01() - 0.5) * 0.7) * -before.with_y(0.0);
        it.vel = back.normalize_or(Vec3::Z) * speed;
        sound(SOUND_BONK, 0.35);
        crate::impact::thud_small(it.pos);
    }
    for (i, t) in near.iter().enumerate() {
        let Some(h) = t.shootable() else { continue };
        if it.hit.iter().any(|(k, _)| *k == t.key()) || !touching(t, it.pos + Vec3::Y * 0.12, crate::assets::kit::SHELL_R, origin) {
            continue;
        }
        it.hit.push((t.key(), it.age));
        if !it.struck.contains(&t.key()) {
            it.struck.push(t.key());
        }
        it.hits += 1;
        let _ = h;
        combat.deal(player, &near, &[(i, Attack::ShellHit, false)], tick);
        sound(SOUND_HIT, 0.5);
        // three enemies by one shell: a carom
        if it.struck.len() == 3 {
            crate::style::callout("CAROM!", Some(Vec3::new(t.er.0, t.er.1 + 2.0, t.er.2)), [0.4, 1.0, 0.45], true, 4.0);
            log("traps: carom (a shell hit three enemies)");
        }
        // off the enemy, back the way it came
        let back = Quat::from_rotation_y((crate::rand01() - 0.5) * 1.2) * -it.vel.with_y(0.0);
        it.vel = back.normalize_or(Vec3::Z) * speed;
        if it.hits >= if red { RED_HITS } else { GREEN_HITS } {
            crate::impact::shell_break(it.pos + Vec3::Y * 0.15, red);
            sound(SOUND_BREAK, 0.5);
            it.gone = true;
            return;
        }
    }
}

/// The Bob-omb goes off: a blast all around, the regular enemies in it sent up in the air.
fn explode(it: &Item, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
    let at = it.pos + Vec3::Y * crate::assets::kit::BOMB_R;
    let n = combat.blast(player, it.pos, BLAST, Attack::Bomb, origin, tick);
    // (the regular ones close in fly up: an air combo's start, not a kill by itself)
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), BLAST, origin);
    let mut up = 0;
    for t in &near {
        let Some(h) = t.shootable() else { continue };
        let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
        if (foot - it.pos).with_y(0.0).length() > BLAST * 0.6 || !combat::liftable(h) {
            continue;
        }
        if crate::juggle::try_launch(h, it.pos, t.radius * crate::scale(), t.height * crate::scale(), true, false, true) {
            up += 1;
        }
    }
    log(format!("traps: Bob-omb went off, {n} hit, {up} sent up"));
    crate::impact::bomb(at);
    sound(SOUND_EXPLOSION, 1.0);
}

/// A regular enemy stepping on the spring is sent up (once in a while each).
fn spring_launch(it: &mut Item, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32, mario: Vec3) {
    let top = it.pos + Vec3::Y * crate::assets::kit::SPRING_H;
    let near = combat::nearby(&HavokPosition(it.pos.x, it.pos.y, it.pos.z, 0.0), 2.5, origin);
    for (i, t) in near.iter().enumerate() {
        let Some(h) = t.shootable() else { continue };
        let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
        if it.hit.iter().any(|(k, _)| *k == t.key()) || (foot - it.pos).with_y(0.0).length() > 0.5 || (foot.y - top.y).abs() > 0.5 || !combat::liftable(h) {
            continue;
        }
        it.hit.push((t.key(), it.age));
        if crate::juggle::try_launch(h, mario, t.radius * crate::scale(), t.height * crate::scale(), true, false, true) {
            combat.deal(player, &near, &[(i, Attack::Boing, false)], tick);
            crate::style::callout("BOING!", Some(foot + Vec3::Y * 1.4), [1.0, 0.5, 0.4], false, 2.0);
            crate::impact::boing(top);
            sound(SOUND_BOING2, 0.8);
            it.squash = 0.7;
            it.squash_vel = -2.0;
            log("traps: an enemy sent up by the spring");
        }
    }
}
