//! The trap kit (LT held while Mario crouches or slides, plus a button): real 3D things
//! (assets/kit.rs, drawn bigger than life) he tosses, kicks, lobs and sets down, that work on their
//! own once out. Out of a slide or a run they carry some of his speed: a toss on the move goes far.
//! - X, a banana: tossed a little in front, it lies there; a regular enemy stepping on it slips and
//!   falls (the game's own knockdown), a big one only staggers, a boss walks over it. Two at most
//!   on the floor, each back 10 s after it's used.
//! - Y, the green shell: kicked straight along the floor, it follows the ground, bounces off walls;
//!   off every enemy it hits (a knockdown for the regular ones) it ricochets to the next one in
//!   reach, the one it hit the fewest times (three: one after the other, two: back and forth),
//!   gone after five. Y held: the red shell, slower, homing in on the nearest enemy from the start.
//! - B, a Bob-omb: lobbed, it bounces and rolls, its fuse blinking faster, and goes off after 2 s
//!   (or on the first enemy it touches): a blast all around that sends the regular ones up in the
//!   air (an air combo's start).
//! - RB, a spring: a wide trampoline set down in front, it stays a while; an enemy stepping on it is
//!   sent up (juggle), Mario landing on it is sent up far higher than any jump.
//! Not on bosses: a banana or a spring is nothing to them, a shell or a blast only its damage.

use std::time::Instant;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use glam::{Quat, Vec3};

use crate::assets::kit;
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
const AWAY: f32 = 20.0;
/// Y held this long (ticks) with LT: the red shell
const RED_HOLD: u32 = 8;
/// a shell's speed (m/s), its hits before it's gone, how fast the red one turns (rad/s)
const GREEN_SPEED: f32 = 10.0;
const RED_SPEED: f32 = 8.0;
const GREEN_HITS: u32 = 5;
const RED_HITS: u32 = 5;
const RED_TURN: f32 = 3.5;
/// a shell's ricochet: the next enemy it goes for within this of it (m, and this far up or down),
/// its speed up while it does (m/s), how hard it keeps on it (rad/s), how long it tries (s)
const RICOCHET_RADIUS: f32 = 9.0;
const RICOCHET_RISE: f32 = 2.5;
const RICOCHET_FAST: f32 = 3.0;
const RICOCHET_TURN: f32 = 12.0;
const RICOCHET_TRY: f32 = 2.5;
/// a shell hits the same enemy again after this long (the one it's going for: soon, two enemies
/// back and forth), the spring sends the same one up again (s)
const SHELL_AGAIN: f32 = 1.6;
const LOCKED_AGAIN: f32 = 0.3;
const SPRING_AGAIN: f32 = 4.0;
/// how much bigger than their meshes they're drawn (and touch): the banana, the shells, the
/// Bob-omb; the spring, wider across than tall (a trampoline to land on)
const KIT_LOOK: f32 = 1.4;
const SPRING_WIDE: f32 = 2.4;
const SPRING_TALL: f32 = 1.25;
/// Mario's own motion the things take (share), at most (m/s); a shell's share of it on top of its
/// speed, and how fast that wears off (m/s each s)
const CARRY: f32 = 0.6;
const CARRY_MAX: f32 = 12.0;
const SHELL_CARRY: f32 = 0.4;
const BOOST_FADE: f32 = 3.0;
/// ...and a shell off the kart's (karting.rs): its speed lasts
const KART_FADE: f32 = 1.0;
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
    /// enemies it hit lately (key, its age then), and every enemy it hit: how often, last when (the
    /// ricochet's pick, "CAROM!")
    hit: Vec<(u64, f32)>,
    tally: Vec<(u64, u32, f32)>,
    /// the enemy a shell ricochets to (key, its age then), and its speed on top (Mario's, wearing off)
    lock: Option<(u64, f32)>,
    boost: f32,
    /// the spring's squash (0 at rest, 1 pressed flat), and its springing back
    squash: f32,
    squash_vel: f32,
    gone: bool,
    /// how far from Mario it lasts (m), how fast a shell's speed on top wears off (m/s each s): the
    /// kart's things (karting.rs) farther, longer
    away: f32,
    fade: f32,
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
    /// Mario's motion before the throw slowed him (Elden Ring, m/s): what a toss carries
    moving: Vec3,
}

/// Mario is crouching, crawling or sliding (a crouch slide, on his bottom or his belly, out of a
/// dive or a slide kick, and its stop): LT and a button are the trap kit then.
pub fn crouching(action: u32) -> bool {
    const ACT_CROUCHING: u32 = 0x0C00_8220;
    const ACT_START_CROUCHING: u32 = 0x0C00_8221;
    const ACT_STOP_CROUCHING: u32 = 0x0C00_8222;
    const ACT_START_CRAWLING: u32 = 0x0C00_8223;
    const ACT_STOP_CRAWLING: u32 = 0x0C00_8224;
    const ACT_CRAWLING: u32 = 0x0400_8448;
    const ACT_CROUCH_SLIDE: u32 = 0x0480_8459;
    const ACT_BUTT_SLIDE: u32 = 0x0084_0452;
    const ACT_BUTT_SLIDE_STOP: u32 = 0x0C00_023E;
    const ACT_STOMACH_SLIDE: u32 = 0x008C_0453;
    const ACT_DIVE_SLIDE: u32 = 0x0088_0456;
    const ACT_SLIDE_KICK_SLIDE: u32 = 0x0080_045A;
    matches!(
        action,
        ACT_CROUCHING
            | ACT_START_CROUCHING
            | ACT_STOP_CROUCHING
            | ACT_START_CRAWLING
            | ACT_STOP_CRAWLING
            | ACT_CRAWLING
            | ACT_CROUCH_SLIDE
            | ACT_BUTT_SLIDE
            | ACT_BUTT_SLIDE_STOP
            | ACT_STOMACH_SLIDE
            | ACT_DIVE_SLIDE
            | ACT_SLIDE_KICK_SLIDE
    )
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
            moving: Vec3::ZERO,
        }
    }

    fn now(&self) -> f32 {
        self.born.elapsed().as_secs_f32()
    }

    /// What's ready (for the HUD): each kit button's share of its wait done (1 ready): X, Y, B, RB.
    pub fn ready(&self) -> [f32; 4] {
        let now = self.now();
        let left = |back: f32, at: f32| if at <= now { 1.0 } else { 1.0 - (at - now) / back };
        let out = self.items.iter().filter(|i| i.kind == Kind::Banana).count();
        let banana = if out >= 2 { 0.0 } else { self.bananas.iter().map(|&b| left(BANANA_BACK, b)).fold(0.0f32, f32::max) };
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
            sound(SOUND_BUZZ, 0.5);
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
        can_bounce: bool,
        ray: impl Fn(Vec3, Vec3) -> Option<Vec3>,
    ) {
        let ahead = Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
        // his motion (SM64 units a tick, mirrored on X), kept from before the throw: it slows him
        let moving = Vec3::new(-state.velocity[0], 0.0, state.velocity[2]) * crate::scale() * 30.0;
        if state.action != moves::ACT_ER_THROW {
            self.moving = moving;
        }
        // (a bounce nobody took last tick is gone)
        self.bounce = None;
        if let Some((kind, _)) = self.pending {
            if mv.throw_kind > 0 && mv.throw_kind as u32 - 1 == kind.throw() {
                self.pending = None;
                let carry = if self.moving.length() > moving.length() { self.moving } else { moving };
                self.spawn(kind, feet, hand, ahead, carry.clamp_length_max(CARRY_MAX), &ray);
            } else if state.action != moves::ACT_ER_THROW && !crouching(state.action) {
                // (the throw never came: hurt, jumped out of it)
                self.pending = None;
            }
        }
        let now = self.now();
        // (who's around, once a tick for all of them)
        let near: Vec<combat::Target> = if self.items.is_empty() { Vec::new() } else { combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), AWAY + RICOCHET_RADIUS, origin) };
        let mut items = std::mem::take(&mut self.items);
        for it in items.iter_mut() {
            it.age += DT;
            // (a shell hits the same enemy again only after a while, the spring sends one up every
            // few seconds: no stunlock against a wall, no juggling one on it for good)
            let again = match it.kind {
                Kind::Spring => SPRING_AGAIN,
                Kind::Green | Kind::Red => SHELL_AGAIN,
                _ => 0.5,
            };
            it.hit.retain(|(_, at)| it.age - *at < again);
            let life = match it.kind {
                Kind::Banana => BANANA_LIFE,
                Kind::Green | Kind::Red => SHELL_LIFE,
                Kind::Bomb => FUSE,
                Kind::Spring => SPRING_LIFE,
            };
            if (it.pos - feet).length() > it.away && it.kind != Kind::Bomb {
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
                    if fly(it, &ray, Landing::Stop) == Step::Wall {
                        it.vel.x *= -0.3;
                        it.vel.z *= -0.3;
                    }
                    if it.state == State::Down {
                        banana_trip(it, &near, player, combat, tick);
                    }
                }
                Kind::Green | Kind::Red => shell(it, &ray, &near, player, combat, tick),
                Kind::Bomb => {
                    if fly(it, &ray, Landing::Bounce(0.35)) == Step::Wall {
                        it.vel.x *= -0.5;
                        it.vel.z *= -0.5;
                    }
                    // rolling on: slowing down
                    if it.state == State::Down {
                        it.vel *= 0.93;
                    }
                    it.spin += it.vel.with_y(0.0).length() * DT / (kit::BOMB_R * KIT_LOOK);
                    // (a touch is enough: it goes off on the first enemy)
                    if it.age > 0.15 && near.iter().any(|t| t.shootable().is_some() && touching(t, it.pos, kit::BOMB_R * KIT_LOOK * 1.25, origin)) {
                        explode(it, player, combat, origin, tick);
                        it.gone = true;
                    }
                }
                Kind::Spring => {
                    fly(it, &ray, Landing::Stop);
                    // springing back to shape
                    it.squash_vel += (-it.squash * 260.0 - it.squash_vel * 9.0) * DT;
                    it.squash += it.squash_vel * DT;
                    if it.state == State::Down {
                        let top = it.pos + Vec3::Y * kit::SPRING_H * SPRING_TALL;
                        // Mario coming down on it out of a jump or a fall, anywhere on its wide top
                        // (and a little past its rim): up he goes
                        let d = (feet - it.pos).with_y(0.0).length();
                        let falling = state.velocity[1] <= 0.0 && can_bounce;
                        if d < spring_reach() && feet.y > it.pos.y - 0.1 && feet.y < top.y + 0.35 && falling && now - self.bounced_at > 0.5 {
                            self.bounced_at = now;
                            self.bounce = Some(SPRING_JUMP);
                            it.squash = 0.6;
                            it.squash_vel = -2.0;
                            log("traps: Mario off the spring");
                            crate::impact::boing(top);
                            sound(SOUND_BOING, 0.6);
                        }
                        spring_launch(it, &near, player, combat, tick, feet);
                    }
                }
            }
        }
        items.retain(|i| !i.gone);
        self.items = items;
    }

    /// Puts one out: from his hand (the banana, the Bob-omb), at his feet (the shells), in front of
    /// him on the floor (the spring). `carry`: his own motion (m/s), some of it theirs.
    fn spawn(&mut self, kind: Kind, feet: Vec3, hand: Vec3, ahead: Vec3, carry: Vec3, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let now = self.now();
        // (a shell takes his speed along its way, on top of its own)
        let boost = (carry.dot(ahead).max(0.0) * SHELL_CARRY).min(4.0);
        let kick = feet + ahead * (0.25 + kit::SHELL_R * KIT_LOOK) + Vec3::Y * 0.05;
        let (part, pos, vel) = match kind {
            Kind::Banana => {
                let slot = (0..2).find(|&k| !self.items.iter().any(|i| i.part == KIT_BANANA + k)).unwrap_or(0);
                if let Some(b) = self.bananas.iter_mut().filter(|b| **b <= now).next() {
                    *b = now + BANANA_BACK;
                }
                (KIT_BANANA + slot, hand, ahead * 3.2 + carry * CARRY + Vec3::Y * 2.8)
            }
            Kind::Green => {
                self.shell = now + SHELL_BACK;
                (KIT_GREEN_SHELL, kick, ahead * (GREEN_SPEED + boost))
            }
            Kind::Red => {
                self.shell = now + SHELL_BACK;
                (KIT_RED_SHELL, kick, ahead * (RED_SPEED + boost))
            }
            Kind::Bomb => {
                self.bomb = now + BOMB_BACK;
                sound(SOUND_FUSE, 0.35);
                (KIT_BOMB, hand, ahead * 6.5 + carry * CARRY + Vec3::Y * 5.0)
            }
            Kind::Spring => {
                self.spring = now + SPRING_BACK;
                // (one at a time: the old one goes)
                for i in self.items.iter_mut().filter(|i| i.kind == Kind::Spring) {
                    i.gone = true;
                }
                // (far enough out for its wide top to clear him, not through a wall: short of it)
                let out = 0.6 + kit::SPRING_R * SPRING_WIDE;
                let at = match ray(feet + Vec3::Y * 0.5, feet + ahead * out + Vec3::Y * 0.5) {
                    Some(w) => feet + ahead * ((w - feet).with_y(0.0).length() - kit::SPRING_R * SPRING_WIDE - 0.1).max(0.0),
                    None => feet + ahead * out,
                };
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
            // (the spring on its floor, or falling to one: set down by a ledge)
            state: if kind == Kind::Spring && ray(pos + Vec3::Y * 0.3, pos - Vec3::Y * 0.3).is_some() { State::Down } else { State::Flying },
            hits: 0,
            hit: Vec::new(),
            tally: Vec::new(),
            lock: None,
            boost: if matches!(kind, Kind::Green | Kind::Red) { boost } else { 0.0 },
            squash: if kind == Kind::Spring { 0.5 } else { 0.0 },
            squash_vel: 0.0,
            gone: false,
            away: AWAY,
            fade: BOOST_FADE,
        });
    }

    /// One of the kit's things put out with no throw of Mario's (the kart's X and Y, karting.rs):
    /// `kind` at `from` going `vel` (m/s), lasting until it's `away` m from Mario, a shell's speed
    /// past its own wearing off slowly. Its stock and wait as the kit's (a buzz when it's not ready).
    pub fn launch(&mut self, kind: Kind, from: Vec3, vel: Vec3, away: f32) -> bool {
        if !self.available(kind) {
            sound(SOUND_BUZZ, 0.5);
            log(format!("traps: {kind:?} not ready"));
            return false;
        }
        let now = self.now();
        let part = match kind {
            Kind::Banana => {
                let slot = (0..2).find(|&k| !self.items.iter().any(|i| i.part == KIT_BANANA + k)).unwrap_or(0);
                if let Some(b) = self.bananas.iter_mut().find(|b| **b <= now) {
                    *b = now + BANANA_BACK;
                }
                KIT_BANANA + slot
            }
            Kind::Green | Kind::Red => {
                self.shell = now + SHELL_BACK;
                if kind == Kind::Green { KIT_GREEN_SHELL } else { KIT_RED_SHELL }
            }
            Kind::Bomb => {
                self.bomb = now + BOMB_BACK;
                KIT_BOMB
            }
            Kind::Spring => {
                self.spring = now + SPRING_BACK;
                KIT_SPRING
            }
        };
        let own = match kind {
            Kind::Green => GREEN_SPEED,
            Kind::Red => RED_SPEED,
            _ => 0.0,
        };
        let shell = matches!(kind, Kind::Green | Kind::Red);
        log(format!("traps: {kind:?} out (the kart's)"));
        crate::impact::kit_out(from);
        self.items.push(Item {
            kind,
            part,
            pos: from,
            vel,
            yaw: vel.x.atan2(vel.z),
            spin: 0.0,
            age: 0.0,
            state: State::Flying,
            hits: 0,
            hit: Vec::new(),
            tally: Vec::new(),
            lock: None,
            boost: if shell { (vel.with_y(0.0).length() - own).max(0.0) } else { 0.0 },
            squash: 0.0,
            squash_vel: 0.0,
            gone: false,
            away,
            fade: KART_FADE,
        });
        true
    }

    /// Puts the things on their models: where each is, turned and squashed as it goes; the rest
    /// hidden in his hand (as relative_parts left them).
    pub fn pose(&self, parts: &mut [PartPose; PARTS], feet: Vec3) {
        for it in &self.items {
            let k = kit::BOMB_R * KIT_LOOK;
            let (rot, stretch, scale) = match it.kind {
                // (a pop as it lands, then lying still, a slow wobble)
                Kind::Banana => (Quat::from_rotation_y(it.yaw), Vec3::ONE, pop(it.age) * KIT_LOOK),
                // spinning as it slides
                Kind::Green | Kind::Red => (Quat::from_rotation_y(it.spin), Vec3::ONE, pop(it.age) * KIT_LOOK),
                Kind::Bomb => {
                    // rolling along its way, swelling in time with the fuse as it runs out
                    let dir = it.vel.with_y(0.0).normalize_or(Vec3::new(it.yaw.sin(), 0.0, it.yaw.cos()));
                    let axis = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                    let left = (FUSE - it.age).max(0.0);
                    let beat = (it.age * (6.0 + 22.0 * (1.0 - left / FUSE))).sin().abs();
                    let swell = 1.0 + 0.12 * beat * (1.0 - left / FUSE);
                    let rot = Quat::from_axis_angle(axis, it.spin) * Quat::from_rotation_y(it.yaw);
                    (rot, Vec3::splat(swell), pop(it.age) * KIT_LOOK)
                }
                Kind::Spring => {
                    // (a wide trampoline, its squash on top: pressed flat it spreads)
                    let s = it.squash.clamp(-0.4, 0.8);
                    let wide = SPRING_WIDE * (1.0 + 0.25 * s);
                    (Quat::from_rotation_y(it.yaw), Vec3::new(wide, SPRING_TALL * (1.0 - s), wide), 1.0)
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

    /// The world's origin moved by `d` (Elden Ring's floating origin, lib.rs): the things with it.
    pub fn shift(&mut self, d: Vec3) {
        for it in &mut self.items {
            it.pos += d;
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

/// An enemy's feet (Elden Ring), as the touches above take them.
fn foot(t: &combat::Target) -> Vec3 {
    Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2)
}

/// How far from the spring's middle a landing is on it (m): its wide top, and a little past the rim.
fn spring_reach() -> f32 {
    kit::SPRING_R * SPRING_WIDE + 0.3
}

/// How a thing meets the floor: it stops there (a banana, the spring), slides on (a shell), or
/// bounces with this share of its fall (the Bob-omb).
#[derive(Clone, Copy, PartialEq)]
enum Landing {
    Stop,
    Slide,
    Bounce(f32),
}

/// What it did this tick.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Moved,
    Wall,
}

/// A wall in the way from `a` to `b` (Elden Ring), not just the slope it goes up: a hit, with no
/// floor to stand on a little up from it.
fn wall(ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>, a: Vec3, b: Vec3) -> bool {
    if ray(a + Vec3::Y * 0.3, b + Vec3::Y * 0.3).is_none() {
        return false;
    }
    // (up a slope: there's floor at its far end not much above)
    ray(b + Vec3::Y * 0.7, b - Vec3::Y * 0.3).is_none_or(|g| g.y > a.y + 0.45)
}

/// Falls under gravity until it meets the floor, then lies or slides on it, following it down
/// slopes and off ledges.
fn fly(it: &mut Item, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>, landing: Landing) -> Step {
    match it.state {
        State::Flying => {
            it.vel.y -= GRAVITY * DT;
            let next = it.pos + it.vel * DT;
            // into a wall on the way: back off it
            if it.vel.with_y(0.0).length() > 0.5 && wall(ray, it.pos, next) {
                return Step::Wall;
            }
            match ray(it.pos + Vec3::Y * 0.3, next - Vec3::Y * 0.05).filter(|_| it.vel.y <= 0.0) {
                Some(ground) => {
                    it.pos = ground;
                    match landing {
                        Landing::Bounce(k) if it.vel.y < -3.0 => {
                            it.vel.y = -it.vel.y * k;
                            it.vel.x *= 0.7;
                            it.vel.z *= 0.7;
                            sound(SOUND_BODY_HIT_GROUND, 0.25);
                        }
                        _ => {
                            it.state = State::Down;
                            it.vel.y = 0.0;
                            if landing == Landing::Stop {
                                it.vel = Vec3::ZERO;
                            }
                            crate::impact::thud_small(ground);
                        }
                    }
                }
                None => it.pos = next,
            }
            Step::Moved
        }
        State::Down => {
            let flat = it.vel.with_y(0.0);
            // (lying still: the floor checked again now and then, a lift moves, a ledge crumbles)
            if flat.length() < 0.05 {
                if (it.age * 30.0) as u32 % 15 == 0 {
                    match ray(it.pos + Vec3::Y * 0.5, it.pos - Vec3::Y * 1.0) {
                        Some(g) => it.pos = g,
                        None => it.state = State::Flying,
                    }
                }
                return Step::Moved;
            }
            let next = it.pos + flat * DT;
            if wall(ray, it.pos, next) {
                return Step::Wall;
            }
            // (the floor under where it's going: down a slope, up a step, off a ledge)
            match ray(next + Vec3::Y * 0.5, next - Vec3::Y * 0.8) {
                Some(ground) => it.pos = ground,
                None => {
                    it.pos = next;
                    it.state = State::Flying;
                }
            }
            Step::Moved
        }
    }
}

/// A regular enemy steps on the banana: down it goes. A big one staggers, a boss walks on.
fn banana_trip(it: &mut Item, near: &[combat::Target], player: &ChrIns, combat: &mut Combat, tick: u32) {
    for (i, t) in near.iter().enumerate() {
        let Some(h) = t.shootable() else { continue };
        let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
        let r = t.radius * crate::scale();
        if (foot - it.pos).with_y(0.0).length() > kit::BANANA_LEN * 0.85 * KIT_LOOK + r * 0.6 || (foot.y - it.pos.y).abs() > 0.6 {
            continue;
        }
        // (a boss walks over it, and so does anything huge)
        if combat::is_boss_now(h) || r > 1.4 {
            continue;
        }
        log(format!("traps: an enemy slipped on the banana ({})", if combat::liftable(h) { "down" } else { "staggered" }));
        combat.deal(player, near, &[(i, Attack::Slip, false)], tick);
        crate::style::callout("SLIP!", Some(foot + Vec3::Y * 1.2), [1.0, 0.9, 0.25], false, 2.0);
        crate::impact::slip(foot);
        sound(SOUND_BODY_HIT_GROUND, 0.5);
        it.gone = true;
        return;
    }
}

/// The shell's slide: along the floor, homing (the red one), off the walls, into the enemies.
fn shell(it: &mut Item, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>, near: &[combat::Target], player: &ChrIns, combat: &mut Combat, tick: u32) {
    let red = it.kind == Kind::Red;
    it.spin += DT * 18.0;
    it.boost = (it.boost - it.fade * DT).max(0.0);
    // (the one it ricochets to: still there to hit, still in reach, not chased for good)
    if let Some((key, since)) = it.lock {
        let there = near
            .iter()
            .find(|t| t.key() == key && t.shootable().is_some())
            .is_some_and(|t| (foot(t) - it.pos).with_y(0.0).length() < RICOCHET_RADIUS * 1.5);
        if !there || it.age - since > RICOCHET_TRY {
            it.lock = None;
        }
    }
    let speed = shell_speed(it);
    // after the one it ricochets to, hard (it moves); else the red one turns to the nearest enemy
    // it hasn't just hit
    let target = match it.lock {
        Some((key, _)) => near.iter().find(|t| t.key() == key).map(|t| (foot(t), RICOCHET_TURN)),
        None if red => near
            .iter()
            .filter(|t| t.shootable().is_some() && !it.hit.iter().any(|(k, _)| *k == t.key()))
            .filter(|t| (Vec3::new(t.er.0, t.er.1, t.er.2) - it.pos).length() < 12.0)
            .map(foot)
            .min_by(|a, b| (*a - it.pos).length_squared().total_cmp(&(*b - it.pos).length_squared()))
            .map(|to| (to, RED_TURN)),
        None => None,
    };
    if let Some((to, turn)) = target {
        let want = (to - it.pos).with_y(0.0).normalize_or(it.vel.normalize_or(Vec3::Z));
        let have = it.vel.with_y(0.0).normalize_or(want);
        let angle = have.angle_between(want).min(turn * DT);
        let side = have.cross(want).y.signum();
        let turned = Quat::from_rotation_y(angle * side) * have;
        it.vel = turned * speed + Vec3::Y * it.vel.y;
    }
    // (on the floor it keeps its speed: it's a Koopa shell)
    if it.state == State::Down {
        let flat = it.vel.with_y(0.0).normalize_or(Vec3::Z) * speed;
        it.vel = Vec3::new(flat.x, it.vel.y, flat.z);
    }
    let before = it.vel;
    if fly(it, ray, Landing::Slide) == Step::Wall {
        // off a wall: back the way it came, a little to one side
        let back = Quat::from_rotation_y((crate::rand01() - 0.5) * 0.7) * -before.with_y(0.0);
        it.vel = back.normalize_or(Vec3::Z) * speed;
        // (a wall between it and the one it went for: let go of it, no bonking at the wall for good)
        it.lock = None;
        sound(SOUND_BONK, 0.35);
        crate::impact::thud_small(it.pos);
    }
    let middle = it.pos + Vec3::Y * kit::SHELL_H * 0.5 * KIT_LOOK;
    for (i, t) in near.iter().enumerate() {
        if t.shootable().is_none() {
            continue;
        }
        let key = t.key();
        // (the one it ricochets to is hit again soon after, two enemies back and forth; any other
        // only after a while)
        let again = if it.lock.is_some_and(|(k, _)| k == key) { LOCKED_AGAIN } else { SHELL_AGAIN };
        if it.hit.iter().any(|(k, at)| *k == key && it.age - *at < again) || !touching(t, middle, kit::SHELL_R * KIT_LOOK, [0.0; 3]) {
            continue;
        }
        it.hit.retain(|(k, _)| *k != key);
        it.hit.push((key, it.age));
        let first = match it.tally.iter_mut().find(|(k, ..)| *k == key) {
            Some(n) => {
                n.1 += 1;
                n.2 = it.age;
                false
            }
            None => {
                it.tally.push((key, 1, it.age));
                true
            }
        };
        it.hits += 1;
        combat.deal(player, near, &[(i, Attack::ShellHit, false)], tick);
        sound(SOUND_HIT, 0.4);
        let over = foot(t) + Vec3::Y * 1.4;
        // three enemies by one shell: a carom
        if first && it.tally.len() == 3 {
            crate::style::callout("CAROM!", Some(over), [0.4, 1.0, 0.45], true, 4.0);
            log("traps: carom (a shell hit three enemies)");
        }
        if it.hits >= if red { RED_HITS } else { GREEN_HITS } {
            // (all five off more than one enemy: a pinball)
            if it.tally.len() >= 2 {
                crate::style::callout("PINBALL!", Some(over + Vec3::Y * 0.4), [0.4, 1.0, 0.45], true, 5.0);
                log(format!("traps: pinball (a shell's {} hits off {} enemies)", it.hits, it.tally.len()));
            }
            crate::impact::shell_break(it.pos + Vec3::Y * 0.15 * KIT_LOOK, red);
            sound(SOUND_BREAK, 0.4);
            it.gone = true;
            return;
        }
        match ricochet(it, near, key, ray) {
            // on to the next one: straight at it at once, a little faster
            Some((next, to)) => {
                it.lock = Some((next, it.age));
                let at = (to - it.pos).with_y(0.0).normalize_or(-it.vel.with_y(0.0).normalize_or(Vec3::Z));
                it.vel = at * shell_speed(it) + Vec3::Y * it.vel.y;
            }
            // nobody else in reach: off the enemy, back the way it came
            None => {
                it.lock = None;
                let back = Quat::from_rotation_y((crate::rand01() - 0.5) * 1.2) * -it.vel.with_y(0.0);
                it.vel = back.normalize_or(Vec3::Z) * shell_speed(it);
            }
        }
        // (one hit a tick: the next is the one it's going for)
        return;
    }
}

/// A shell's speed (m/s): its own, Mario's share on top, faster while it ricochets.
fn shell_speed(it: &Item) -> f32 {
    let own = if it.kind == Kind::Red { RED_SPEED } else { GREEN_SPEED };
    own + it.boost + if it.lock.is_some() { RICOCHET_FAST } else { 0.0 }
}

/// The enemy a shell ricochets to after hitting `just`: in reach and in sight, never the one just
/// hit; of those the one it hit the fewest times, then the one hit longest ago, then the nearest
/// (three enemies: one after the other, A B C A B; two: back and forth, A B A B A). None when there's
/// nobody else: it bounces off as ever.
fn ricochet(it: &Item, near: &[combat::Target], just: u64, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Option<(u64, Vec3)> {
    let tally = |key: u64| it.tally.iter().find(|(k, ..)| *k == key).map_or((0, f32::MIN), |&(_, n, at)| (n, at));
    near.iter()
        .filter(|t| t.shootable().is_some() && t.key() != just)
        .map(|t| (t.key(), foot(t)))
        .filter(|(_, f)| (*f - it.pos).with_y(0.0).length() < RICOCHET_RADIUS && (f.y - it.pos.y).abs() < RICOCHET_RISE)
        // (not through a wall: a clear line from the shell to it, or one that ends at it)
        .filter(|(_, f)| ray(it.pos + Vec3::Y * 0.5, *f + Vec3::Y * 0.4).is_none_or(|w| (w - *f).with_y(0.0).length() < 1.0))
        .min_by(|(a, fa), (b, fb)| {
            let ((na, ta), (nb, tb)) = (tally(*a), tally(*b));
            na.cmp(&nb).then(ta.total_cmp(&tb)).then((*fa - it.pos).length_squared().total_cmp(&(*fb - it.pos).length_squared()))
        })
}

/// The Bob-omb goes off: a blast all around, the regular enemies in it sent up in the air.
fn explode(it: &Item, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
    let at = it.pos + Vec3::Y * kit::BOMB_R * KIT_LOOK;
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
fn spring_launch(it: &mut Item, near: &[combat::Target], player: &ChrIns, combat: &mut Combat, tick: u32, mario: Vec3) {
    let top = it.pos + Vec3::Y * kit::SPRING_H * SPRING_TALL;
    for (i, t) in near.iter().enumerate() {
        let Some(h) = t.shootable() else { continue };
        let foot = Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2);
        if it.hit.iter().any(|(k, _)| *k == t.key()) || (foot - it.pos).with_y(0.0).length() > spring_reach() || (foot.y - top.y).abs() > 0.5 || !combat::liftable(h) {
            continue;
        }
        it.hit.push((t.key(), it.age));
        if crate::juggle::try_launch(h, mario, t.radius * crate::scale(), t.height * crate::scale(), true, false, true) {
            combat.deal(player, near, &[(i, Attack::Boing, false)], tick);
            crate::style::callout("BOING!", Some(foot + Vec3::Y * 1.4), [1.0, 0.5, 0.4], false, 2.0);
            crate::impact::boing(top);
            sound(SOUND_BOING2, 0.5);
            it.squash = 0.7;
            it.squash_vel = -2.0;
            log("traps: an enemy sent up by the spring");
        }
    }
}
