//! Mario's fireball, on the pad's RB (X stays SM64's B: punch, kick, dive). Mega Man's buster with
//! SM64's own flame (its 8 frames from the ROM, hud.rs):
//! - a tap throws a small one (SM64's throw, standing or in the air; running he just shoots)
//! - held, it charges: sparks of flame gather at Mario's hand, bigger at 1 s, 2 s and 4 s, each
//!   step with its own SM64 chime (the last, SM64's secret jingle, and the flame flashes)
//! - each step hits harder; the full one goes through enemies and knocks them back
//! - held on for 10 s it keeps growing into a giant one, lifted over his head: it hits twice as
//!   hard as the full one and blows regular enemies away (knock.rs), in a shower of flames
//! - held on for 20 s it grows into the sun: immense and slow, it burns harder, blows up far wider
//!   (a blast in waves that flings the regular enemies near its heart) and leaves lava embers on
//!   the ground for 2-3 s that burn whoever steps in them
//! - a blow on Mario (or a knockback, a grab) puts the charge out: RB has to come up again
//! - after a shot, a short cooldown (released too early: Lakitu's buzz)
//! - thrown with a move of its own (er_moves.c's throws): a palm thrust standing, pushing him back
//!   the harder the shot, the giant one from over his head; in the air it holds him up a moment;
//!   running he shoots on the move. The flame leaves his hand when the throw lets go.
//! Shots leave turned to the nearest enemy in front and home in on it a little on the way (a gentle
//! curve, never a U-turn), and burst on walls and bodies. The
//! damage goes through combat.rs like Mario's other hits.
//! With another weapon in hand RB is that weapon's element's own move instead (v4 Part 3c,
//! rbmoves.rs; element.rs), each with a 10 s giant: the hammer's ice rocket punch (its Glacier fist
//! flies from here: a huge fist of ice straight through everyone in its way, a ring of ice spikes
//! where it stops), the Chain Chomp's dark beam (beam.rs), the boomerang's poison sweep and its
//! carpets (carpet.rs), the Star Sword's lightning (bolt.rs). The charge waits in `cast` for its
//! move to let it go (`release`).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::collision::sm_to_er;
use crate::element::Element;
use crate::fx::{Blast, FireFx, Orb, Pool, BLAST_LAUNCH, ORB_CHARGE, ORB_REFLECTED, ORB_STUCK};
use crate::{log, scale, sm64, worker};

/// held this many SM64 ticks (30 a second): charge levels 1, 2, 3, the giant one and the sun
const CHARGE_TICKS: [u32; 5] = [30, 60, 120, 300, 600];
pub const GIANT: u8 = 4;
pub const SUN: u8 = 5;
/// a tap shows no charge (the sparks start after this)
const SHOW_CHARGE_AFTER: u32 = 7;
/// ticks between shots, by level
const COOLDOWN: [u32; 6] = [36, 40, 44, 54, 80, 120];
/// how far every shot flies before it fizzles (SM64 units: 48 m), the small ones fast and the big
/// ones slow
const RANGE: f32 = 4800.0;
/// by level: hit radius (SM64 units), speed (units per tick), drawn size (m)
const RADIUS: [f32; 6] = [26.0, 40.0, 58.0, 100.0, 170.0, 260.0];
const SPEED: [f32; 6] = [80.0, 66.0, 54.0, 44.0, 30.0, 22.0];
const SIZE: [f32; 6] = [0.5, 0.8, 1.15, 2.0, 4.2, 7.0];
/// turned to the nearest enemy within this range (m) and cone (cos 35°) in front
const AIM_RANGE: f32 = 35.0;
const AIM_CONE_COS: f32 = 0.82;
/// homing on the way: the nearest enemy within this range (m) and cone (cos 60°) ahead of the shot,
/// looked for again every this many ticks; the most it turns a tick (radians, by level: the big
/// ones turn slower); past this angle (cos 80°) it lets him go
const HOME_RANGE: f32 = 22.0;
const HOME_CONE_COS: f32 = 0.5;
const HOME_EVERY: u32 = 6;
const HOME_TURN: [f32; 6] = [0.055, 0.05, 0.045, 0.04, 0.025, 0.02];
const HOME_LOSE_COS: f32 = 0.17;
/// the Glacier fist (the hammer's 10 s RB, rbmoves.rs): its speed (units per tick), hit radius
/// (units), drawn size (m), how far it flies (units); its ring of spikes where it stops (m)
const GLACIER_SPEED: f32 = 45.0;
const GLACIER_RADIUS: f32 = 160.0;
const GLACIER_SIZE: f32 = 3.2;
const GLACIER_RANGE: f32 = 2500.0;
const GLACIER_SPIKES: f32 = 4.5;
/// the poison carpets (carpet.rs) by the sweep's charge: how wide (m), how long (ticks); the
/// spray's pool in the air; the swamp
const CARPET_RADIUS: [f32; 4] = [3.0, 4.0, 5.0, 6.0];
const CARPET_TICKS: [u32; 4] = [180, 225, 270, 300];
const POOL_RADIUS: [f32; 2] = [2.5, 5.0];
const POOL_TICKS: u32 = 240;
const SWAMP_RADIUS: f32 = 10.0;
const SWAMP_TICKS: u32 = 360;
/// the ice rocket's block on his fist, by charge (m)
const FIST_SIZE: [f32; 4] = [0.45, 0.55, 0.7, 0.85];

/// the full shot knocks regular enemies back this far (m), over this many ticks
const PUSH: f32 = 3.0;
const PUSH_TICKS: u32 = 9;
/// a burst's ticks (the giant one's, the sun's)
const BURST_TICKS: [u32; 3] = [10, 22, 34];
/// running faster than this (SM64 units per tick), Mario shoots without stopping to throw
const RUN_SHOT_SPEED: f32 = 14.0;
/// the giant one: it clings to a boss this long (ticks), burns whoever it carries every this many
/// ticks, and blows up reaching this far (m); the giant one's, then the sun's
const STICK_TICKS: [u32; 2] = [48, 60];
const BURN_EVERY: u32 = 8;
pub const EXPLODE_REACH: [f32; 2] = [5.0, 9.0];
/// the sun's blast: regular enemies this close to its heart (share of the reach) are flung away;
/// its waves, this many ticks apart
const SUN_FLING: f32 = 0.6;
const SUN_WAVES: [u32; 2] = [5, 11];
/// its lava embers: how many, how wide (m), how long (ticks: 2.5 s and up to 0.4 s more), how
/// often they burn whoever stands in them (ticks)
const EMBERS: u32 = 14;
const EMBER_RADIUS: f32 = 1.3;
const EMBER_TICKS: u32 = 75;
const EMBER_BURN_EVERY: u32 = 15;

// SM64 sounds
const SOUND_FLAME_BLOWN: i32 = 0x5055_8081;
const SOUND_VOLCANO: i32 = 0x300C_8081;
const SOUND_FLAME_OUT: i32 = 0x3003_8081;
const SOUND_BOMB: i32 = 0x802E_2081_u32 as i32;
/// charge steps: menu tick, short star, the secret jingle, the volcano, a boom
const SOUND_CHARGE: [i32; 5] = [0x7000_F881_u32 as i32, 0x3016_0091, 0x7030_2081, SOUND_VOLCANO, SOUND_BOMB];
const SOUND_BUZZ: i32 = 0x700E_0081;
/// the Glacier fist: a whoosh out, the snowman's burst and a box breaking where it stops
const SOUND_SWISH: i32 = 0x305C_0081;
const SOUND_SNOW: i32 = 0x5064_C081;
const SOUND_BREAK: i32 = 0x3041_C081;

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
/// SM64's bumps into a wall (soft bonk, ground bonk, the air knockback off a wall): no blow
const BONKS: [u32; 3] = [0x0102_08B6, 0x0002_0466, 0x0102_08B0];
const ACT_GROUP_CUTSCENE: u32 = 0x100;

pub fn burst_ticks(level: u8) -> u32 {
    BURST_TICKS[(level >= GIANT) as usize + big(level)]
}

/// How long another element's burst is kept for its looks (ticks): the ice's shards falling, the
/// dark orb's spray, the glob's splash.
pub fn element_burst_ticks(el: Element, level: u8) -> u32 {
    let l = level.min(3) as u32;
    match el {
        Element::Frost => 26 + 3 * l,
        Element::Blood => 18 + 2 * l,
        Element::Poison => 20 + 2 * l,
        Element::Fire | Element::Lightning => burst_ticks(level),
    }
}

/// A shot's speed (units per tick), hit radius (units) and drawn size (m) by its element and level.
/// A shot's speed (units per tick), hit radius (units): fire's by level; the hammer's Glacier fist
/// (the only other element that throws one, rbmoves.rs).
fn shot_speed(el: Element, level: u8) -> f32 {
    if el == Element::Frost { GLACIER_SPEED } else { SPEED[level.min(SUN) as usize] }
}

fn shot_radius(el: Element, level: u8) -> f32 {
    if el == Element::Frost { GLACIER_RADIUS } else { RADIUS[level.min(SUN) as usize] }
}

/// A shot's drawn size (m), and a puff's of another element (a charge put out).
pub fn shot_size(el: Element, level: u8) -> f32 {
    match el {
        Element::Fire | Element::Lightning => SIZE[level.min(SUN) as usize],
        Element::Frost if level >= GIANT => GLACIER_SIZE,
        _ => 0.6,
    }
}

/// The blow a shot of `el` at `level` lands.
fn shot_attack(el: Element, level: u8) -> Attack {
    if el == Element::Frost { Attack::Glacier } else { Attack::Fire(level) }
}


/// how long a burst is kept for its looks (ticks): the big ones' smoke and shock wave outlast
/// their flames
pub fn blast_life(level: u8) -> u32 {
    match level {
        SUN.. => 96,
        GIANT => 66,
        _ => burst_ticks(level),
    }
}

/// How much of a ball at `p` (radius `r`) the camera at `eye` sees past the map: its heart and
/// four points round its rim across the view (0-1). A wall cutting off half a big one leaves half.
/// (`grounded`: one lying on the ground, its rim's lowest point under it isn't looked at)
fn visibility(eye: Vec3, p: Vec3, r: f32, grounded: bool, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> f32 {
    let open = |q: Vec3, slack: f32| ray(eye, q).is_none_or(|h| (h - q).length() < slack);
    if r < 0.6 {
        return open(p, r + 0.2) as u8 as f32;
    }
    let d = (p - eye).normalize_or(Vec3::Z);
    let side = d.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = side.cross(d);
    let mut seen = 2.0 * open(p, r + 0.2) as u8 as f32;
    let mut of = 2.0;
    for q in [side, -side, up, -up] {
        if grounded && q.y < -0.3 {
            continue;
        }
        seen += open(p + q * r * 0.8, r * 0.3 + 0.2) as u8 as f32;
        of += 1.0;
    }
    seen / of
}

/// A burst's or a pool's visibility, looked at every few ticks (fewer rays) and eased; at once
/// the first time.
fn eased(vis: &mut f32, at: &mut u32, tick: u32, look: impl FnOnce() -> f32) -> f32 {
    if *vis < 0.0 || tick.wrapping_sub(*at) >= VIS_EVERY {
        let seen = look();
        *vis = if *vis < 0.0 { seen } else { *vis + (seen - *vis) * 0.5 };
        *at = tick;
    }
    *vis
}

/// bursts and pools are looked at every this many ticks, the shots' ground under them too
const VIS_EVERY: u32 = 3;

/// how high over Mario's feet a shot leaves (SM64 units): the big ones over his head
fn hand_height(level: u8) -> f32 {
    match level {
        SUN.. => 300.0,
        GIANT => 150.0,
        _ => 75.0,
    }
}

/// the giant one's (0) or the sun's (1) row in the tables above
pub fn big(level: u8) -> usize {
    (level >= SUN) as usize
}

struct Shot {
    /// SM64 coordinates and velocity (units per tick)
    pos: [f32; 3],
    vel: [f32; 3],
    level: u8,
    /// what it is (element.rs): a fireball, the ice block, the dark orb, the poison glob
    el: Element,
    age: u32,
    /// enemies it went through (the full shot hits each once)
    hit: Vec<u64>,
    /// the giant one: the enemies it sweeps along, the big one it stuck to (its key) and for how
    /// many ticks, the tick of its last burn
    swept: Vec<FieldInsHandle>,
    stuck: Option<(u64, u32)>,
    last_burn: u32,
    /// the enemy it homes in on (its key)
    homing: Option<u64>,
    /// an enemy's projectile sent back (parry.rs): drawn blue-white
    reflected: bool,
    /// how much of it the camera sees (eased; below 0: not looked yet), the ground under it
    /// (looked for every few ticks)
    vis: f32,
    ground: Option<f32>,
}

struct Burst {
    pos: Vec3,
    level: u8,
    el: Element,
    born: u32,
    /// the ground under it (the big ones': None, not looked for yet)
    ground: Option<Option<f32>>,
    /// the flames at his hand as a big one leaves it (no explosion of its own)
    launch: bool,
    /// how much of it the camera sees (eased; below 0: not looked yet) and when it was looked at
    vis: f32,
    vis_at: u32,
}

impl Burst {
    fn new(pos: Vec3, level: u8, born: u32) -> Self {
        Burst { pos, level, el: Element::Fire, born, ground: None, launch: false, vis: -1.0, vis_at: 0 }
    }

    fn of(el: Element, pos: Vec3, level: u8, born: u32) -> Self {
        Burst { el, ..Burst::new(pos, level, born) }
    }

    /// (ticks it's kept)
    fn life(&self) -> u32 {
        match self.el {
            Element::Fire if self.launch => burst_ticks(self.level),
            Element::Fire => blast_life(self.level),
            el => element_burst_ticks(el, self.level),
        }
    }
}

/// a pool of lava the sun left on the ground (Elden Ring)
struct Ember {
    pos: Vec3,
    born: u32,
    life: u32,
    vis: f32,
    vis_at: u32,
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
    /// a shot waiting for its throw to let go (its level and element, when it was asked for)
    pending: Option<(u8, Element, u32)>,
    level: u8,
    /// the charge was put out (a blow on Mario): nothing more until RB comes up
    doused: bool,
    /// the charge's element: the weapon in hand when RB went down (element.rs)
    el: Element,
    shots: Vec<Shot>,
    bursts: Vec<Burst>,
    pushes: Vec<Push>,
    embers: Vec<Ember>,
    last_ember_burn: u32,
    /// a charge of the other weapons' (the sword's lightning, the hammer's frost, the Chomp's
    /// blood, the boomerang's poison) asked for (its element, level, in the air, when) until its
    /// move lets go (rbmoves.rs); the sword's bolts (bolt.rs) and crescents (wave.rs), the hammer's
    /// thrown small ones (volley.rs), the dark beam (beam.rs), the poison carpets (carpet.rs)
    cast: Option<(Element, u8, bool, u32)>,
    cast_asked: bool,
    pub bolts: crate::bolt::Bolts,
    pub waves: crate::wave::Waves,
    pub volley: crate::volley::Volley,
    pub beam: crate::beam::Beam,
    pub carpets: crate::carpet::Carpets,
    /// the scripted move playing (lib.rs): the beam pours while its move plays, the ice rocket's
    /// fist is in its block
    playing: Option<u16>,
}

/// a sword cast whose move didn't let go within this (ticks: hurt, cut short) is lost
const CAST_WAIT: u32 = 30;

static LOOKS: Mutex<(FireFx, Option<Instant>)> = Mutex::new((FireFx::EMPTY, None));

/// The fireballs to draw now, the balls moved on since the last tick.
pub fn looks() -> FireFx {
    let guard = LOOKS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(at) = guard.1 else { return FireFx::default() };
    let dt = at.elapsed().as_secs_f32();
    // (the SM64 ticks stopped: menus, loading)
    if dt > 0.25 {
        return FireFx::default();
    }
    let dt = dt.min(1.0 / 30.0);
    let mut fx = guard.0.clone();
    for o in &mut fx.orbs {
        o.pos += o.vel * dt;
    }
    fx
}

pub fn clear() {
    *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = (FireFx::default(), None);
    crate::bolt::clear();
    crate::wave::clear();
    crate::volley::clear();
    crate::beam::clear();
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
        Fire {
            held_since: None,
            ready_at: 0,
            run_throw: None,
            pending: None,
            level: 0,
            doused: false,
            el: Element::Fire,
            shots: Vec::new(),
            bursts: Vec::new(),
            pushes: Vec::new(),
            embers: Vec::new(),
            last_ember_burn: 0,
            cast: None,
            cast_asked: false,
            bolts: Default::default(),
            waves: Default::default(),
            volley: Default::default(),
            beam: Default::default(),
            carpets: Default::default(),
            playing: None,
        }
    }

    /// Every tick, before `tick`: the scripted move playing (script.rs), if any.
    pub fn set_playing(&mut self, id: Option<u16>) {
        self.playing = id;
    }

    /// A charge wants its move (rbmoves.rs `cast_move`): its element, level, in the air; once, as
    /// it's asked for.
    pub fn take_cast(&mut self) -> Option<(Element, u8, bool)> {
        let (el, level, air, _) = self.cast?;
        (!std::mem::replace(&mut self.cast_asked, true)).then_some((el, level, air))
    }

    /// The charge's move lets it go now, from his hand `hand` facing `facing` (Elden Ring): the
    /// sword's bolts, the hammer's Glacier fist (its rocket punch and dive are the move's own
    /// blows), the Chomp's beam, the boomerang's carpet.
    pub fn release(&mut self, hand: Vec3, facing: Vec3, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let Some((el, level, air, _)) = self.cast.take() else { return };
        let feet = er(origin, state.position);
        match el {
            Element::Lightning => self.bolts.cast(level, air, hand, facing, origin, tick),
            Element::Frost if level >= GIANT => self.spawn(GIANT, Element::Frost, state, origin, tick),
            Element::Frost | Element::Fire => {}
            Element::Blood => {
                let ticks = self.playing.and_then(crate::rbmoves::beam_ticks).unwrap_or(crate::rbmoves::BEAM_TICKS[0]);
                self.beam.start(level, ticks as u32, hand, facing, origin, tick);
            }
            Element::Poison => {
                let f = facing.with_y(0.0).normalize_or(Vec3::Z);
                if air {
                    // (a pool on the ground below him)
                    let ground = ray(feet + Vec3::Y * 0.3, feet - Vec3::Y * 12.0).unwrap_or(feet);
                    self.carpets.lay(ground, POOL_RADIUS[(level >= GIANT) as usize], f, std::f32::consts::PI, POOL_TICKS, false, tick);
                } else if level >= GIANT {
                    self.carpets.lay(feet, SWAMP_RADIUS, f, std::f32::consts::PI, SWAMP_TICKS, true, tick);
                } else {
                    let l = level.min(3) as usize;
                    let half = (crate::rbmoves::SWEEP_SPAN[l] * 0.5).to_radians();
                    self.carpets.lay(feet, CARPET_RADIUS[l], f, half, CARPET_TICKS[l], false, tick);
                }
            }
        }
    }

    /// A charge is going on in his hand (RB held, not put out).
    pub fn charging(&self) -> bool {
        self.held_since.is_some()
    }

    /// A projectile turned back (parry.rs): a level-3 shot from `from` (Elden Ring) along `aim`,
    /// homing on the one who fired it (`target`, its key).
    pub fn reflect(&mut self, from: Vec3, aim: Vec3, target: Option<u64>, origin: [f32; 3], tick: u32) {
        let a = crate::collision::er_to_sm(origin, &HavokPosition(from.x, from.y, from.z, 0.0));
        let tip = from + aim;
        let b = crate::collision::er_to_sm(origin, &HavokPosition(tip.x, tip.y, tip.z, 0.0));
        let dir = Vec3::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]).normalize_or(Vec3::Z);
        let level = 3u8;
        self.shots.push(Shot { pos: a, vel: (dir * SPEED[level as usize]).to_array(), level, el: Element::Fire, age: 0, hit: Vec::new(), swept: Vec::new(), stuck: None, last_burn: tick, homing: target, reflected: true, vis: -1.0, ground: None });
        self.bursts.push(Burst::new(from, 0, tick));
    }

    /// Another attack pressed while RB charges (lib.rs): the charge goes out, a shot already let go
    /// of (waiting for the throw) still flies.
    pub fn put_out(&mut self, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32) {
        let pending = self.pending.take();
        self.douse(state, origin, tick);
        self.pending = pending;
    }

    /// Mario was hit (lib.rs): a charge in his hand goes out with a fizzle.
    pub fn douse(&mut self, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32) {
        self.pending = None;
        let Some(since) = self.held_since.take() else { return };
        let level = std::mem::take(&mut self.level);
        self.doused = true;
        // (not for a tap's first ticks: nothing showed yet)
        if tick.saturating_sub(since.max(self.ready_at)) < SHOW_CHARGE_AFTER && level == 0 {
            return;
        }
        let m = state.position;
        let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
        self.bursts.push(Burst::of(self.el, er(origin, [m[0] + fx * 45.0, m[1] + 75.0, m[2] + fz * 45.0]), 0, tick));
        worker::call("fireball doused", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_FLAME_OUT, 0.6) });
        log(format!("fireball: charge put out by a blow (level {level})"));
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
        // (knocked back, grabbed, burning...: the charge goes out; not bumping into a wall)
        if busy && action & ACT_GROUP_MASK != ACT_GROUP_CUTSCENE && !BONKS.contains(&action) {
            self.douse(state, origin, tick);
        }
        if !b {
            self.doused = false;
        }
        if !allowed {
            self.held_since = None;
            self.level = 0;
        } else if b && self.doused {
            // (put out: RB has to come up first)
        } else if b {
            // (the weapon in hand as RB goes down: its element)
            if self.held_since.is_none() {
                self.el = crate::element::current();
            }
            let since = *self.held_since.get_or_insert(tick);
            let held = tick.saturating_sub(since.max(self.ready_at));
            let level = CHARGE_TICKS.iter().filter(|&&t| held >= t).count() as u8;
            // (the sun is the fists' own: the others stop at their 10 s giant)
            let level = if self.el == Element::Fire { level } else { level.min(GIANT) };
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
        if let Some((level, el, asked)) = self.pending {
            let thrown = mv.throw_kind > 0 && (crate::moves::THROW_FIRE..crate::moves::THROW_HOOK).contains(&(mv.throw_kind as u32 - 1));
            if thrown || tick.wrapping_sub(asked) > 9 {
                self.pending = None;
                self.spawn(level, el, state, origin, tick);
            }
        }

        // (a cast whose move never let go: lost)
        if self.cast.is_some_and(|(_, _, _, at)| tick.wrapping_sub(at) > CAST_WAIT) {
            self.cast = None;
            log("fireball: the cast's move didn't let go: lost");
        }
        // the other weapons' (in the game's physics coordinates: the map rays are those)
        {
            let er_ray = |a: Vec3, b: Vec3| ray(a, b);
            self.bolts.tick(player, combat, origin, tick, &er_ray);
            self.waves.tick(player, combat, origin, tick, &er_ray);
            self.volley.tick(player, combat, origin, tick);
            // (the dark beam from his hands, the way he faces, while its move plays)
            let m = state.position;
            let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
            let hand = er(origin, [m[0] + fx * 45.0, m[1] + 75.0, m[2] + fz * 45.0]);
            let casting = self.playing.and_then(crate::rbmoves::beam_ticks).is_some();
            self.beam.tick(hand, Vec3::new(-fx, 0.0, fz), casting, player, combat, origin, tick, &er_ray);
            self.carpets.tick(player, combat, origin, tick);
        }
        self.fly(origin, tick, player, combat, &ray);
        self.push(&ray);
        self.burn_embers(origin, tick, player, combat);
        // (the sun's later waves wait their turn)
        self.bursts.retain(|b| b.born > tick || tick - b.born < b.life());
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
        // the other weapons: their own moves (the cast waits for its move to let go: `release`)
        if self.el != Element::Fire {
            self.cast = Some((self.el, level, action_air(state.action), tick));
            self.cast_asked = false;
            return;
        }
        let (dir, _) = self.aim(state, origin, level);
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
            self.pending = Some((level, self.el, tick));
            return;
        }
        // running: a punch on the move throws it, now
        if on_ground && !crate::moves::dodging(state.action) {
            self.run_throw = Some(tick);
            worker::call("fireball run throw", move |_| unsafe {
                sm64::sm64_er_keep_run();
                sm64::sm64_set_mario_action(id, ACT_MOVE_PUNCHING);
            });
        }
        self.spawn(level, self.el, state, origin, tick);
    }

    /// Where a shot from his hand goes: the nearest enemy in front (its chest), else straight ahead;
    /// and that chest (SM64), if one was found.
    fn aim(&self, state: &sm64::SM64MarioState, origin: [f32; 3], level: u8) -> (Vec3, Option<[f32; 3]>) {
        let m = state.position;
        let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
        let hand = [m[0] + fx * 45.0, m[1] + hand_height(level), m[2] + fz * 45.0];
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
            Some(p) => (Vec3::new(p[0] - hand[0], p[1] - hand[1], p[2] - hand[2]).normalize_or(Vec3::new(fx, 0.0, fz)), Some(p)),
            None => (Vec3::new(fx, 0.0, fz), None),
        }
    }

    /// The flame (or the weapon's element: the ice block, the dark orb, the poison glob) leaves his
    /// hand.
    fn spawn(&mut self, level: u8, el: Element, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32) {
        let m = state.position;
        let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
        let hand = [m[0] + fx * 45.0, m[1] + hand_height(level), m[2] + fz * 45.0];
        let (dir, target) = self.aim(state, origin, level);
        let speed = shot_speed(el, level);
        // (the Glacier fist flies level, its enemy's height or not)
        let dir = if el == Element::Frost { dir.with_y(0.0).normalize_or(Vec3::new(fx, 0.0, fz)) } else { dir };
        let vel = dir * speed;
        self.shots.push(Shot { pos: hand, vel: vel.to_array(), level, el, age: 0, hit: Vec::new(), swept: Vec::new(), stuck: None, last_burn: tick, homing: None, reflected: false, vis: -1.0, ground: None });
        log(format!("fireball: {} level {level} shot{}", el.name(), if target.is_some() { ", aimed" } else { "" }));
        // a puff of it at his hand as it leaves
        self.bursts.push(Burst::of(el, er(origin, hand), 0, tick));
        if el != Element::Fire {
            // (the Glacier fist: a whoosh, a boom, Mario holding still a moment)
            worker::call("glacier shot", move |_| unsafe {
                sm64::sm64_play_sound_volume(SOUND_SWISH, 0.7);
                sm64::sm64_play_sound_volume(SOUND_VOLCANO, 0.45);
                sm64::sm64_er_hitstop(4);
            });
            crate::lakitu::shake(0.8);
            return;
        }
        worker::call("fireball shot", move |_| unsafe {
            sm64::sm64_play_sound_global(SOUND_FLAME_BLOWN);
            if level >= 3 {
                sm64::sm64_play_sound_global(SOUND_VOLCANO);
            }
            // the giant one's recoil: a boom, and Mario holds still a moment
            if level >= GIANT {
                sm64::sm64_play_sound_global(SOUND_BOMB);
                sm64::sm64_er_hitstop(if level >= SUN { 7 } else { 4 });
            }
        });
        if level >= SUN {
            crate::lakitu::shake(1.0);
        }
        if level >= GIANT {
            self.bursts.push(Burst { launch: true, ..Burst::new(er(origin, hand), level, tick) });
        }
    }

    fn fly(&mut self, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|shot| {
            home(shot, origin);
            if shot.level >= GIANT && shot.el == Element::Fire {
                return self.giant(shot, origin, tick, player, combat, ray);
            }
            let prev = shot.pos;
            let next = [prev[0] + shot.vel[0], prev[1] + shot.vel[1], prev[2] + shot.vel[2]];
            shot.age += 1;
            let r = shot_radius(shot.el, shot.level);
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
                    combat.deal(player, &near, &[(i, shot_attack(shot.el, shot.level), false)], tick);
                }
                let at = er(origin, [prev[0] + (next[0] - prev[0]) * k, prev[1] + (next[1] - prev[1]) * k, prev[2] + (next[2] - prev[2]) * k]);
                self.bursts.push(Burst::of(shot.el, at, shot.level, tick));
                // (the Glacier fist goes on through them all)
                if shot.el != Element::Fire {
                    continue;
                }
                if shot.level >= 3 {
                    sound(SOUND_BOMB);
                    // (co-op guest: no push, the host owns the enemies)
                    if let Some(h) = t.shootable().filter(|_| !blown && !crate::coop::guest()) {
                        self.pushes.push(Push { handle: *h, dir: Vec3::new(v.x, 0.0, v.z).normalize_or_zero(), left: PUSH_TICKS });
                    }
                } else {
                    sound(SOUND_FLAME_OUT);
                    return false;
                }
            }
            // the map: a burst where it hits
            if let Some(p) = ray(er(origin, prev), er(origin, next)) {
                self.bursts.push(Burst::of(shot.el, p, shot.level, tick));
                if shot.el == Element::Fire {
                    sound(if shot.level >= 3 { SOUND_BOMB } else { SOUND_FLAME_OUT });
                } else {
                    self.glacier_end(p, origin, tick, player, combat, ray);
                }
                return false;
            }
            shot.pos = next;
            let range = if shot.el == Element::Frost { GLACIER_RANGE } else { RANGE };
            if shot.age as f32 * shot_speed(shot.el, shot.level) >= range {
                if shot.el == Element::Frost {
                    self.glacier_end(er(origin, next), origin, tick, player, combat, ray);
                } else {
                    self.bursts.push(Burst::of(shot.el, er(origin, next), 0, tick));
                }
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
                Some(chest) if *ticks < STICK_TICKS[big(shot.level)] => {
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
                worker::call("fireball sweep", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_BOMB, 0.4) });
                self.bursts.push(Burst::new(center, 3, tick));
                if combat::blowable(&h) && !crate::coop::guest() {
                    shot.swept.push(h);
                } else {
                    log(format!("fireball: the giant one clings to a big one ({} carried)", shot.swept.len()));
                    shot.stuck = Some((t.key(), 0));
                    crate::lakitu::shake(if shot.level >= SUN { 1.4 } else { 0.8 });
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
            // (soft: it comes again and again)
            worker::call("fireball burn", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_FLAME_BLOWN, 0.25) });
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
        let ground = ray(at, at - Vec3::Y * 8.0).unwrap_or(at - Vec3::Y * reach.min(2.0));
        let blast = EXPLODE_REACH[big(shot.level)];
        let n = combat.blast(player, ground, blast, Attack::Fire(shot.level), origin, tick);
        let away = er_vel(shot.vel).with_y(0.0).normalize_or(Vec3::Z);
        for h in shot.swept.drain(..) {
            crate::knock::launch(&h, away);
        }
        self.bursts.push(Burst::new(at, shot.level, tick));
        if shot.level >= SUN {
            self.sun_blast(ground, blast, origin, tick, ray);
        } else {
            crate::impact::fire_blast(ground, blast);
            crate::lakitu::shake(1.6);
        }
        sound(SOUND_BOMB);
        sound(SOUND_VOLCANO);
        log(format!("fireball: the {} blows up ({n} caught in it)", if shot.level >= SUN { "sun" } else { "giant one" }));
        false
    }

    /// The sun's blast on top of the explosion: regular enemies near its heart flung away, waves
    /// of fire rolling out after it, and lava embers left on the ground all round.
    fn sun_blast(&mut self, ground: Vec3, reach: f32, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let here = HavokPosition(ground.x, ground.y, ground.z, 0.0);
        let mut flung = 0;
        for t in combat::nearby(&here, reach + 4.0, origin) {
            let Some(h) = t.shootable() else { continue };
            let off = Vec3::new(t.er.0, t.er.1, t.er.2) - ground;
            if off.with_y(0.0).length() <= reach * SUN_FLING && off.y.abs() <= 2.0 && combat::blowable(h) && crate::knock::launch(h, off.with_y(0.0).normalize_or(Vec3::Z)) {
                flung += 1;
            }
        }
        // (the waves: smaller bursts rolling out, a ring each)
        for (k, &delay) in SUN_WAVES.iter().enumerate() {
            let n = 5 + 2 * k;
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32 + k as f32 * 0.6;
                let d = reach * (0.45 + 0.3 * k as f32);
                self.bursts.push(Burst::new(ground + Vec3::new(a.cos() * d, 0.6, a.sin() * d), 3, tick + delay));
            }
        }
        // the embers: pools of lava on the ground, the nearest to its heart first
        let before = self.embers.len();
        for i in 0..EMBERS {
            let a = i as f32 * 2.399;
            let d = reach * 0.85 * ((i as f32 + 0.5) / EMBERS as f32).sqrt();
            let top = ground + Vec3::new(a.cos() * d, 2.5, a.sin() * d);
            // (on the ground, and not through a wall from the blast's heart)
            let Some(p) = ray(top, top - Vec3::Y * 6.0) else { continue };
            if ray(ground + Vec3::Y * 1.0, p + Vec3::Y * 0.5).is_some() {
                continue;
            }
            self.embers.push(Ember { pos: p, born: tick, life: EMBER_TICKS + (i % 3) * 6, vis: -1.0, vis_at: 0 });
        }
        crate::impact::sun_blast(ground, reach);
        crate::lakitu::shake(2.6);
        log(format!("fireball: the sun flings {flung} away, {} embers on the ground", self.embers.len() - before));
    }

    /// The embers burn whoever stands in them, every EMBER_BURN_EVERY ticks (once each however
    /// many he stands in), and go out after their time.
    fn burn_embers(&mut self, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat) {
        self.embers.retain(|e| tick.wrapping_sub(e.born) < e.life);
        if self.embers.is_empty() || tick.wrapping_sub(self.last_ember_burn) < EMBER_BURN_EVERY {
            return;
        }
        self.last_ember_burn = tick;
        let (lo, hi) = self.embers.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), e| (lo.min(e.pos), hi.max(e.pos)));
        let mid = (lo + hi) * 0.5;
        let near = combat::nearby(&HavokPosition(mid.x, mid.y, mid.z, 0.0), (hi - lo).length() * 0.5 + EMBER_RADIUS + 4.0, origin);
        let burning: Vec<(usize, Attack, bool)> = near
            .iter()
            .enumerate()
            .filter(|(_, t)| t.shootable().is_some())
            .filter(|(_, t)| {
                let p = Vec3::new(t.er.0, t.er.1, t.er.2);
                let r = t.radius * scale();
                self.embers.iter().any(|e| (p - e.pos).with_y(0.0).length() <= EMBER_RADIUS + r && (p.y - e.pos.y).abs() <= 0.8)
            })
            .map(|(i, _)| (i, Attack::Ember, false))
            .collect();
        if !burning.is_empty() {
            combat.deal(player, &near, &burning, tick);
            // (soft: it comes again and again)
            worker::call("fireball ember", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_FLAME_OUT, 0.2) });
        }
    }

    /// The Glacier fist stops at `at` (Elden Ring: a wall, the end of its way): a ring of ice spikes
    /// bursting from the ground round it.
    fn glacier_end(&mut self, at: Vec3, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let ground = ray(at + Vec3::Y * 0.5, at - Vec3::Y * 8.0).unwrap_or(at - Vec3::Y * 1.0);
        let n = combat.blast(player, ground, GLACIER_SPIKES, Attack::GlacierSpikes, origin, tick);
        self.bursts.push(Burst::of(Element::Frost, at, GIANT, tick));
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            self.bursts.push(Burst::of(Element::Frost, ground + Vec3::new(a.cos() * GLACIER_SPIKES * 0.7, 0.4, a.sin() * GLACIER_SPIKES * 0.7), 2, tick + 2 + k % 3));
        }
        crate::lakitu::shake(1.2);
        worker::call("glacier spikes", |_| unsafe {
            sm64::sm64_play_sound_volume(SOUND_SNOW, 0.6);
            sm64::sm64_play_sound_volume(SOUND_BREAK, 0.45);
        });
        log(format!("fireball: the Glacier fist's spikes ({n} caught in them)"));
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

    /// The fireballs as hud.rs draws them (flame.rs gives them their looks): the charge in his
    /// hand, the shots, their bursts and the lava pools, with how much of each the camera sees
    /// past the map and the ground under the big ones.
    fn publish(&mut self, state: &sm64::SM64MarioState, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let mut out = FireFx::default();
        let eye = camera();
        // Mega Mario's fireballs are a giant's (their reach and speed are in his scaled units already)
        let k = crate::size_factor();
        // the charge: the flame in his hand growing
        if let Some(since) = self.held_since {
            let held = tick.saturating_sub(since.max(self.ready_at));
            if held >= SHOW_CHARGE_AFTER || self.level > 0 {
                let m = state.position;
                let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
                let hand = er(origin, [m[0] + fx * 45.0, m[1] + 75.0, m[2] + fz * 45.0]);
                let level = self.level as usize;
                let pulse = 1.0 + 0.12 * (tick as f32 * 0.9).sin();
                let fire = self.el == Element::Fire;
                let grow = match level {
                    0 => (held as f32 / CHARGE_TICKS[0] as f32).clamp(0.3, 1.0),
                    // from 4 s on it keeps growing towards the giant one, from 10 s towards the sun
                    3 | 4 if fire => {
                        let k = (held - CHARGE_TICKS[level - 1]) as f32 / (CHARGE_TICKS[level] - CHARGE_TICKS[level - 1]) as f32;
                        1.0 + k.clamp(0.0, 1.0) * (SIZE[level + 1] / SIZE[level] - 1.0) * 0.85
                    }
                    _ => 1.0,
                };
                let size = shot_size(self.el, self.level) * if fire { 0.7 } else { 0.8 } * pulse * grow;
                // (a big one is held up over his head, not around him)
                // (the sun, higher still: he holds it up, not stands in it)
                let hand = hand + Vec3::Y * ((size * 0.5 - 0.3).max(0.0) + (size - 3.0).max(0.0) * 0.5) * k;
                let ground = (self.level >= 3).then(|| ray(hand, hand - Vec3::Y * size * k * 3.0).map(|g| g.y)).flatten();
                out.orbs.push(Orb { pos: hand, vel: er_vel(state.velocity), size: size * k, level: self.level, age: held, flags: ORB_CHARGE, ground, vis: 1.0, element: self.el as u8 });
            }
        }
        for shot in &mut self.shots {
            let pos = er(origin, shot.pos);
            // (clinging to a boss it stands still, churning bigger)
            let stuck = shot.stuck.is_some();
            let vel = if stuck { Vec3::ZERO } else { er_vel(shot.vel) };
            let size = k * shot_size(shot.el, shot.level) * if stuck { 1.12 + 0.12 * (tick as f32 * 2.1).sin() } else { 1.0 + 0.08 * (tick as f32 * 1.3 + shot.age as f32).sin() };
            // (eased: a ball passing behind a post fades, it doesn't blink; hidden at once if it
            // leaves his hand out of sight)
            let seen = eye.map_or(1.0, |e| visibility(e, pos, size * 0.5, false, ray));
            shot.vis = if shot.vis < 0.0 { seen } else { shot.vis + (seen - shot.vis) * 0.4 };
            if shot.vis < 0.02 {
                continue;
            }
            if shot.level >= 3 && shot.age % VIS_EVERY == 0 {
                shot.ground = ray(pos, pos - Vec3::Y * size * 3.0).map(|g| g.y);
            }
            let ground = shot.ground.filter(|_| shot.level >= 3);
            let flags = if stuck { ORB_STUCK } else { 0 } | if shot.reflected { ORB_REFLECTED } else { 0 };
            out.orbs.push(Orb { pos, vel, size, level: shot.level, age: shot.age, flags, ground, vis: shot.vis, element: shot.el as u8 });
        }
        for b in &mut self.bursts {
            if b.born > tick {
                continue;
            }
            let size = shot_size(b.el, b.level) * k;
            let (pos, r) = (b.pos, size * 0.5);
            let vis = eased(&mut b.vis, &mut b.vis_at, tick, || eye.map_or(1.0, |e| visibility(e, pos, r, b.level >= 3, ray)));
            if vis < 0.02 {
                continue;
            }
            // (the big ones' ground, looked for once)
            if b.level >= GIANT && b.ground.is_none() {
                b.ground = Some(ray(b.pos + Vec3::Y * 0.5, b.pos - Vec3::Y * 8.0).map(|g| g.y));
            }
            let flags = if b.launch { BLAST_LAUNCH } else { 0 };
            out.blasts.push(Blast { pos: b.pos, level: b.level, age: tick - b.born, size, ground: b.ground.flatten(), flags, vis, element: b.el as u8 });
        }
        // the ice rocket punch's fist in its block of ice (rbmoves.rs), bigger by charge
        if let Some(l) = self.playing.and_then(|id| match id {
            crate::script::ICE_ROCKET..=crate::script::ICE_DIVE => Some((id - crate::script::ICE_ROCKET).min(3) as usize),
            _ => None,
        }) {
            let m = state.position;
            let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
            let fist = er(origin, [m[0] + fx * 70.0, m[1] + 70.0, m[2] + fz * 70.0]);
            out.orbs.push(Orb { pos: fist, vel: er_vel(state.velocity), size: FIST_SIZE[l] * k, level: l as u8, age: tick, flags: ORB_CHARGE, ground: None, vis: 1.0, element: Element::Frost as u8 });
        }
        out.beams = crate::beam::looks();
        out.carpets = crate::carpet::looks();
        for e in &mut self.embers {
            let pos = e.pos + Vec3::Y * 0.3;
            let vis = eased(&mut e.vis, &mut e.vis_at, tick, || eye.map_or(1.0, |c| visibility(c, pos, EMBER_RADIUS * k, true, ray)));
            if vis >= 0.02 {
                out.pools.push(Pool { pos: e.pos, age: tick.wrapping_sub(e.born), life: e.life, radius: EMBER_RADIUS * k, vis });
            }
        }
        *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = (out, Some(Instant::now()));
    }
}

/// Homing: the shot picks the nearest enemy ahead of it (again every HOME_EVERY ticks) and turns
/// a little towards his chest each tick, keeping its speed. Not while clinging to a boss.
fn home(shot: &mut Shot, origin: [f32; 3]) {
    if shot.stuck.is_some() {
        return;
    }
    // (the Glacier fist flies straight on)
    if shot.el == Element::Frost {
        return;
    }
    let vel = Vec3::from(shot.vel);
    let speed = vel.length();
    if speed < 1e-3 {
        return;
    }
    let dir = vel / speed;
    let p = shot.pos;
    // (a turned-back shot keeps to its shooter while he's there)
    if shot.homing.is_none() || (shot.age % HOME_EVERY == 0 && !shot.reflected) {
        let here = sm_to_er(origin, p);
        shot.homing = combat::nearby(&here, HOME_RANGE, origin)
            .iter()
            .filter(|t| t.shootable().is_some() && !shot.hit.contains(&t.key()))
            .filter_map(|t| {
                let to = Vec3::new(t.sm[0] - p[0], t.sm[1] + t.height.min(300.0) * 0.45 - p[1], t.sm[2] - p[2]);
                let d = to.length();
                (d > 1.0 && to.dot(dir) / d >= HOME_CONE_COS).then_some((d, t.key()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, k)| k);
    }
    let Some(key) = shot.homing else { return };
    let Some(chest) = combat::chest_by_key(key) else {
        shot.homing = None;
        return;
    };
    let c = crate::collision::er_to_sm(origin, &HavokPosition(chest.x, chest.y, chest.z, 0.0));
    let want = Vec3::new(c[0] - p[0], c[1] - p[1], c[2] - p[2]).normalize_or_zero();
    let cos = want.dot(dir);
    if want == Vec3::ZERO || cos < HOME_LOSE_COS {
        shot.homing = None;
        return;
    }
    let angle = cos.clamp(-1.0, 1.0).acos();
    let turn = HOME_TURN[shot.level.min(SUN) as usize];
    let new = if angle <= turn { want } else { dir.lerp(want, turn / angle).normalize_or(dir) };
    shot.vel = (new * speed).to_array();
}

/// Where the game's render camera is (Lakitu's, when on).
fn camera() -> Option<Vec3> {
    let cam = unsafe { eldenring::cs::CSCamera::instance() }.ok()?;
    let m = &cam.pers_cam_1.matrix;
    Some(Vec3::new(m.3.0, m.3.1, m.3.2))
}

/// In the air, in an action SM64's air throw can take over (not a ground pound, not hurt).
fn action_air(action: u32) -> bool {
    action & ACT_FLAG_AIR != 0 && action != ACT_GROUND_POUND && action & (ACT_FLAG_INVULNERABLE | ACT_NO_THROW) == 0
}
