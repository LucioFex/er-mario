//! Moves as data (v4 Part 1): a move is a few ticks of keyframed curves (the SM64 animation and
//! the frame of it to show, his body's twist / lean / roll, his speed and the aimed lunge to the
//! enemy, a hop, squash and dip, the hand's reach), hit windows (combat.rs measures who's in them),
//! cancel windows (what may cut the move short, sooner once a blow of it landed) and sounds. Rust
//! works out each tick (`Player::frame`) and libsm64's ACT_ER_SCRIPT / ACT_ER_SCRIPT_AIR play it
//! with SM64's own ground and air steps.

use crate::combat::Attack;
use crate::sm64::SM64ErScript;

pub const ACT_ER_SCRIPT: u32 = 0x0080_039C;
pub const ACT_ER_SCRIPT_AIR: u32 = 0x0080_089E;

/// A scripted move's action, on the ground or in the air.
pub fn is_script(action: u32) -> bool {
    action == ACT_ER_SCRIPT || action == ACT_ER_SCRIPT_AIR
}

/// How a curve comes into a key from the one before.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    /// slow then fast (a wind-up's pull)
    In,
    /// fast then slow (a strike settling)
    Out,
    InOut,
    /// the value before it stays until the key's tick, then jumps
    Hold,
}

/// A curve's value `v` at the move's tick `t`.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub t: u16,
    pub v: f32,
    pub ease: Ease,
}

pub const fn k(t: u16, v: f32, ease: Ease) -> Key {
    Key { t, v, ease }
}

/// The curve at tick `t` (a fraction of a tick too); before its first key its first value, past
/// its last its last; `empty` with no keys at all.
pub fn eval(keys: &[Key], t: f32, empty: f32) -> f32 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return empty };
    if t <= first.t as f32 {
        return first.v;
    }
    if t >= last.t as f32 {
        return last.v;
    }
    let Some(i) = keys.windows(2).position(|w| t < w[1].t as f32) else { return last.v };
    let (a, b) = (keys[i], keys[i + 1]);
    let span = b.t.saturating_sub(a.t) as f32;
    if span <= 0.0 {
        return b.v;
    }
    let p = ((t - a.t as f32) / span).clamp(0.0, 1.0);
    let e = match b.ease {
        Ease::Linear => p,
        Ease::In => p * p,
        Ease::Out => 1.0 - (1.0 - p) * (1.0 - p),
        Ease::InOut => p * p * (3.0 - 2.0 * p),
        Ease::Hold => 0.0,
    };
    a.v + (b.v - a.v) * e
}

/// An animation from tick `from` on, the frame of it shown by `frames`.
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub from: u16,
    pub anim: i32,
    pub frames: &'static [Key],
}

/// Where a hit window reaches (SM64 units; `ahead`: its centre that far in front of him).
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)] // (the sword's and the other weapons' moves, Parts 2-3)
pub enum Shape {
    /// in front, within a cone (a punch's)
    Front { reach: f32 },
    /// a slice of a circle round a point ahead: `half_deg` each side of his facing
    Arc { radius: f32, half_deg: f32, ahead: f32 },
    /// all round a point ahead
    Around { radius: f32, ahead: f32 },
    /// round him, out to the held weapon's reach beyond the radius
    Ring { radius: f32 },
    /// along the held weapon (its head's path and its length: the hammer's, the sword's blade)
    Weapon,
    /// all round him, wider at full health (the Great Spin's, Zelda's)
    Burst { radius: f32 },
}

/// Ticks `from..=to` hit what `shape` reaches with `attack` (the same enemy again only after the
/// attack's own cooldown, combat.rs); `fx` shown while it's open (moves::FX_*).
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub from: u16,
    pub to: u16,
    pub shape: Shape,
    pub attack: Attack,
    pub fx: u32,
    /// only once he's on the ground (a plunge's landing ring)
    pub ground: bool,
}

/// The window if it's open for Mario now (`air`: he's in the air).
pub fn window_open(w: &Window, air: bool) -> Option<&Window> {
    (!(w.ground && air)).then_some(w)
}

/// What may cut a move short (bits), from tick `from` (`on_hit` once a blow of it landed).
pub const DASH: u8 = 1;
pub const JUMP: u8 = 2;
/// the string's next step (X)
pub const NEXT: u8 = 4;
/// a weapon swap (L1): the swap strike
pub const SWAP: u8 = 8;
pub const ALL: u8 = DASH | JUMP | NEXT | SWAP;

#[derive(Clone, Copy, Debug)]
pub struct Cancel {
    pub into: u8,
    pub from: u16,
    pub on_hit: u16,
}

/// The aimed lunge: over ticks `from..to` he covers the way to the enemy he aims at, short of it
/// by `gap` (m), at most `max` (m); with no one aimed at, `max`. Eased out (fast start).
#[derive(Clone, Copy, Debug)]
pub struct Approach {
    pub from: u16,
    pub to: u16,
    pub max: f32,
    pub gap: f32,
}

/// One move.
#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub name: &'static str,
    pub len: u16,
    pub clips: &'static [Clip],
    /// his body drawn turned (+ left), leaning forward, rolled (degrees)
    pub twist: &'static [Key],
    pub pitch: &'static [Key],
    pub roll: &'static [Key],
    /// his facing turning (degrees a tick)
    pub turn: &'static [Key],
    /// ticks he turns to the aimed enemy at
    pub aim: &'static [u16],
    /// forward speed (m/s), on top of the lunge
    pub speed: &'static [Key],
    pub approach: Option<Approach>,
    /// vertical speed set at these ticks (SM64 units a tick: up off the ground into the air action)
    pub vel_y: &'static [(u16, f32)],
    /// ticks `from..to` with no gravity in the air
    pub hold: (u16, u16),
    /// his body squashed (under 1) / stretched (over 1), and drawn low (SM64 units)
    pub squash: &'static [Key],
    pub dip: &'static [Key],
    /// the right hand's reach (0-1, arm IK): straight out ahead at shoulder height, tipped up by
    /// `reach_pitch` degrees (90: overhead)
    pub reach: &'static [Key],
    pub reach_pitch: f32,
    /// the held weapon (the sword's blade) in ticks `blade_held` (from, to): degrees round his
    /// facing (+ left), up from level, rolled round itself
    pub blade_yaw: &'static [Key],
    pub blade_pitch: &'static [Key],
    pub blade_roll: &'static [Key],
    pub blade_held: (u16, u16),
    pub windows: &'static [Window],
    pub cancels: &'static [Cancel],
    /// (tick, sound, volume: 0 at his body), one a tick
    pub sounds: &'static [(u16, u32, f32)],
    /// (tick, moves::FX_*)
    pub fx: &'static [(u16, u32)],
    /// ticks `from..to` it can't be left; ticks `from..to` no blow lands on him
    pub locked: (u16, u16),
    pub invulnerable: (u16, u16),
    /// it starts in the air action; in the air, landing ends it
    pub air: bool,
    pub land_ends: bool,
    /// the stick steers him (his facing toward it, a little speed): the Hurricane
    pub steer: bool,
    /// a blow of it landing bounces him up this fast (SM64 units a tick) out of it: the pogo
    pub bounce: f32,
    /// X still held at this tick: he goes up at this vertical speed (with the enemy it launched)
    pub held_rise: Option<(u16, f32)>,
    /// (every, fx): shown every that many ticks from the start (a charge's glow)
    pub pulse: Option<(u16, u32)>,
}

pub const BASE: Move = Move {
    name: "",
    len: 1,
    clips: &[],
    twist: &[],
    pitch: &[],
    roll: &[],
    turn: &[],
    aim: &[],
    speed: &[],
    approach: None,
    vel_y: &[],
    hold: (0, 0),
    squash: &[],
    dip: &[],
    reach: &[],
    reach_pitch: 0.0,
    blade_yaw: &[],
    blade_pitch: &[],
    blade_roll: &[],
    blade_held: (0, 0),
    windows: &[],
    cancels: &[],
    sounds: &[],
    fx: &[],
    locked: (0, 0),
    invulnerable: (0, 0),
    air: false,
    land_ends: false,
    steer: false,
    bounce: 0.0,
    held_rise: None,
    pulse: None,
};

/// The lunge's whole way (m): to the enemy short of `gap`, at most `max`; `max` with no one aimed.
pub fn approach_total(a: &Approach, target_m: Option<f32>) -> f32 {
    target_m.map_or(a.max, |d| (d - a.gap).clamp(0.0, a.max))
}

/// The lunge's share of its whole way covered during tick `t` (0 outside it).
pub fn approach_step(a: &Approach, t: u16) -> f32 {
    if t < a.from || t >= a.to {
        return 0.0;
    }
    let n = (a.to - a.from) as f32;
    let out = |p: f32| 1.0 - (1.0 - p) * (1.0 - p);
    out((t + 1 - a.from) as f32 / n) - out((t - a.from) as f32 / n)
}

// ---- the moves ------------------------------------------------------------------------------

use Ease::*;

const MARIO_ANIM_FIRST_PUNCH: i32 = 0x67;
const MARIO_ANIM_DIVE: i32 = 0x88;
const MARIO_ANIM_SLOW_LAND_FROM_DIVE: i32 = 0x5A;
const SOUND_MARIO_PUNCH_HOO: u32 = 0x241F_8081;
const SOUND_MARIO_UH: u32 = 0x2405_8081;
const SOUND_ACTION_THROW: u32 = 0x0435_8081;
const SOUND_GENERAL_SWISH_AIR: u32 = 0x305C_0081;

pub const BULLET_PUNCH: u16 = 0;
pub const SKY_STRIKE: u16 = 1;
pub const ZEUS_THROW: u16 = 2;
/// the Star Sword's moves (swordplay.rs picks them)
pub const SW_S1: u16 = 3;
pub const SW_S2: u16 = 4;
#[allow(dead_code)] // (reached as SW_S2 + 1: swordplay.rs walks the string by id)
pub const SW_S3: u16 = 5;
pub const SW_S4: u16 = 6;
pub const SW_WHIRL: u16 = 7;
pub const SW_CLEAVE: u16 = 8;
pub const SW_THRUST: u16 = 9;
pub const SW_STABS: u16 = 10;
pub const SW_RISE: u16 = 11;
pub const SW_SPIN_JUMP: u16 = 12;
pub const SW_WAVE_CHARGE: u16 = 13;
pub const SW_WAVE: u16 = 14;
pub const SW_WAVE_JUST: u16 = 15;
pub const SW_SPIN: u16 = 16;
pub const SW_SPIN_CHARGE: u16 = 17;
/// the Great Spin by charge level: this, + 1, + 2
pub const SW_GREAT_SPIN: u16 = 18;
pub const SW_HURRICANE: u16 = 21;
pub const SW_CROSS: u16 = 22;
pub const SW_LAUNCHER: u16 = 23;
pub const SW_HELM: u16 = 24;
pub const SW_IAI: u16 = 25;
pub const SW_FLURRY: u16 = 26;
/// the Aerial Rave's four: this to + 3
pub const SW_RAVE: u16 = 27;
pub const SW_BREAKER: u16 = 31;
pub const SW_POGO: u16 = 32;
pub const SW_DRAW: u16 = 33;
pub const SW_DRAW_AIR: u16 = 34;

/// One of the Star Sword's moves.
pub fn is_sword(id: u16) -> bool {
    (SW_S1..=SW_DRAW_AIR).contains(&id)
}

/// The move `id`'s data, if there is any (a name for the log).
pub fn named(id: u16) -> &'static str {
    MOVES.get(id as usize).map_or("(a move with no data)", |m| m.name)
}

/// A frame's fx bit (beyond moves::FX_*): what the move casts leaves now (the sword's bolt)
pub const FX_RELEASE: u32 = 0x1000;
/// the sword's crescents let go now (wave.rs): the star wave, the JUST three, a finisher's sword
/// beam (only at full health: lib.rs)
pub const FX_WAVE: u32 = 0x2000;
pub const FX_WAVE3: u32 = 0x4000;
pub const FX_BEAM: u32 = 0x8000;
/// a crack of lightning where the blade's tip comes down (the thunder cleave, the rings), a gleam
/// on the blade (a charge's JUST moment, the draw), the cross cut's X left hanging (wave.rs)
pub const FX_ZAP: u32 = 0x1_0000;
pub const FX_SPARKLE: u32 = 0x2_0000;
pub const FX_CROSS: u32 = 0x4_0000;
const SOUND_MARIO_PUNCH_YAH: u32 = 0x241E_8081;

/// The fists' Bullet Bill punch (back, forward + X; DMC's Stinger): a coil back, then he flies at
/// the enemy fist first like a Bullet Bill, up to 7 m, and lands out of it. Landed, it can be cut
/// at once into the dash, a jump, the string or a swap.
const BULLET: Move = Move {
    name: "Bullet Bill punch",
    len: 26,
    clips: &[
        Clip { from: 0, anim: MARIO_ANIM_FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(4, 1.0, In)] },
        Clip { from: 5, anim: MARIO_ANIM_DIVE, frames: &[k(5, 0.0, Linear), k(9, 5.0, Out), k(12, 6.0, Linear)] },
        Clip { from: 13, anim: MARIO_ANIM_SLOW_LAND_FROM_DIVE, frames: &[k(13, 0.0, Linear), k(25, 14.0, Out)] },
    ],
    twist: &[k(0, 0.0, Linear), k(4, -30.0, Out), k(7, 0.0, Out)],
    pitch: &[k(0, 0.0, Linear), k(4, -6.0, InOut), k(8, 4.0, Out), k(13, 0.0, InOut)],
    aim: &[0, 4],
    speed: &[k(0, 0.0, Linear), k(12, 0.0, Hold), k(13, 3.0, Linear), k(19, 0.0, Out)],
    approach: Some(Approach { from: 5, to: 12, max: 7.0, gap: 0.7 }),
    vel_y: &[(5, 14.0)],
    squash: &[k(0, 1.0, Linear), k(4, 0.86, Out), k(6, 1.12, Out), k(11, 1.0, InOut), k(13, 0.9, Out), k(17, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 8.0, Out), k(6, 0.0, Out), k(13, 6.0, Out), k(18, 0.0, InOut)],
    reach: &[k(5, 0.0, Linear), k(8, 1.0, Out), k(13, 1.0, Hold), k(17, 0.0, InOut)],
    windows: &[Window { from: 8, to: 13, shape: Shape::Front { reach: 150.0 }, attack: Attack::BulletPunch, fx: 0, ground: false }],
    cancels: &[Cancel { into: ALL, from: 19, on_hit: 13 }],
    sounds: &[(0, SOUND_MARIO_UH, 0.5), (5, SOUND_MARIO_PUNCH_HOO, 0.0), (6, SOUND_ACTION_THROW, 0.0), (7, SOUND_GENERAL_SWISH_AIR, 0.45)],
    fx: &[(5, crate::moves::FX_LAUNCH)],
    locked: (0, 5),
    ..BASE
};

/// The Star Sword's RB on the ground (Zelda's Skyward Strike): the sword raised overhead, the bolt
/// off its tip, then down again. Its charge was RB's (bolt.rs makes the bolt).
const SKY: Move = Move {
    name: "skyward strike",
    len: 20,
    clips: &[Clip { from: 0, anim: MARIO_ANIM_FIRST_PUNCH, frames: &[k(0, 0.0, Linear)] }],
    pitch: &[k(0, 0.0, Linear), k(4, -6.0, Out), k(7, -3.0, InOut), k(14, 0.0, InOut)],
    aim: &[0],
    squash: &[k(0, 1.0, Linear), k(4, 0.92, Out), k(6, 1.08, Out), k(12, 1.0, InOut)],
    dip: &[k(0, 0.0, Linear), k(4, 5.0, Out), k(6, 0.0, Out)],
    reach: &[k(0, 0.0, Linear), k(4, 1.0, Out), k(12, 1.0, Hold), k(18, 0.0, InOut)],
    reach_pitch: 85.0,
    blade_yaw: &[k(0, -20.0, Linear), k(5, 0.0, Out)],
    blade_pitch: &[k(0, 10.0, Linear), k(5, 92.0, Out), k(12, 92.0, Hold), k(18, 20.0, InOut)],
    blade_held: (0, 18),
    cancels: &[Cancel { into: ALL, from: 10, on_hit: 10 }],
    sounds: &[(0, SOUND_GENERAL_SWISH_AIR, 0.4), (6, SOUND_MARIO_PUNCH_HOO, 0.0)],
    fx: &[(6, FX_RELEASE)],
    locked: (0, 7),
    ..BASE
};

/// The Star Sword's RB in the air (Zeus's throw): held up there a moment, the arm cocked back over
/// his head, the bolt javelin hurled down at the enemy.
const ZEUS: Move = Move {
    name: "Zeus throw",
    len: 16,
    clips: &[Clip { from: 0, anim: MARIO_ANIM_FIRST_PUNCH, frames: &[k(0, 0.0, Linear), k(6, 3.0, Out)] }],
    twist: &[k(0, 0.0, Linear), k(4, -25.0, Out), k(7, 15.0, In), k(12, 0.0, InOut)],
    pitch: &[k(0, 0.0, Linear), k(4, -8.0, Out), k(7, 12.0, In), k(12, 0.0, InOut)],
    aim: &[0, 5],
    vel_y: &[(0, 0.0)],
    hold: (0, 9),
    reach: &[k(0, 0.5, Linear), k(4, 1.0, Out), k(9, 1.0, Hold), k(12, 0.0, InOut)],
    reach_pitch: 60.0,
    blade_pitch: &[k(0, 40.0, Linear), k(4, 140.0, Out), k(7, -35.0, In), k(12, -20.0, InOut)],
    blade_held: (0, 12),
    cancels: &[Cancel { into: ALL, from: 9, on_hit: 9 }],
    sounds: &[(0, SOUND_GENERAL_SWISH_AIR, 0.4), (6, SOUND_MARIO_PUNCH_YAH, 0.0)],
    fx: &[(6, FX_RELEASE)],
    air: true,
    land_ends: true,
    ..BASE
};

use crate::swordmoves as sw;

/// Every move by its id: the fists' and the sword's RB moves, then the Star Sword's (SW_S1 on,
/// swordmoves.rs)
pub const MOVES: &[Move] = &[
    BULLET,
    SKY,
    ZEUS,
    sw::S1,
    sw::S2,
    sw::S3,
    sw::S4,
    sw::WHIRL,
    sw::CLEAVE,
    sw::THRUST,
    sw::STABS,
    sw::RISE,
    sw::SPIN_JUMP,
    sw::WAVE_CHARGE,
    sw::WAVE,
    sw::WAVE_JUST,
    sw::SPIN_CUT,
    sw::SPIN_CHARGE,
    sw::GREAT_SPIN[0],
    sw::GREAT_SPIN[1],
    sw::GREAT_SPIN[2],
    sw::HURRICANE,
    sw::CROSS,
    sw::LAUNCHER,
    sw::HELM,
    sw::IAI,
    sw::FLURRY,
    sw::RAVE[0],
    sw::RAVE[1],
    sw::RAVE[2],
    sw::RAVE[3],
    sw::BREAKER,
    sw::POGO,
    sw::DRAW,
    sw::DRAW_AIR,
];

// ---- playing them ---------------------------------------------------------------------------

/// What a move does on one tick: libsm64's part, the hit window open (its index), the hand's reach,
/// what to show (what may cut it short now: `Player::cancel`).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub id: u16,
    pub t: u16,
    pub c: SM64ErScript,
    pub window: Option<(usize, &'static Window)>,
    pub reach: f32,
    pub reach_pitch: f32,
    /// where the held weapon points (yaw, pitch, roll degrees), in its held ticks
    pub blade: Option<(f32, f32, f32)>,
    pub fx: u32,
}

/// The move playing: which, its lunge's whole way, whether a blow of it landed.
#[derive(Default)]
pub struct Player {
    cur: Option<u16>,
    starting: Option<(u16, Option<f32>)>,
    approach_m: f32,
    landed: bool,
    /// what may cut it short as of its last frame
    cancel: u8,
    /// the ground windows whose look was shown (by index: once, as he lands)
    ground_shown: u32,
    /// (window, enemy) blows that landed: each window strikes an enemy once
    struck: Vec<(usize, u64)>,
    /// moves started so far (the same move twice in a row is a new one)
    starts: u32,
}

impl Player {
    /// Move `id` starts this tick (its request goes in with it), the enemy aimed at `target_m` away.
    pub fn start(&mut self, id: u16, target_m: Option<f32>) {
        self.starting = Some((id, target_m));
    }

    /// What may cut the move playing short now (as of its last frame; everything with none playing).
    pub fn cancel(&self) -> u8 {
        if self.cur.is_some() { self.cancel } else { ALL }
    }

    /// A blow of the move playing landed: its cancels open sooner.
    pub fn landed(&mut self) {
        self.landed = true;
    }

    /// The move playing (as of its last frame).
    pub fn active(&self) -> Option<u16> {
        self.cur
    }

    /// Moves started so far (a new one when it changes).
    pub fn starts(&self) -> u32 {
        self.starts
    }

    /// A blow of the move playing landed.
    pub fn has_landed(&self) -> bool {
        self.landed
    }

    /// Window `window` of the move playing reaches the enemy `key`: whether that's its first blow on
    /// him (and it's counted). A hit-stop holds the move's tick while the real ticks, and the blow's
    /// cooldown, run on: without this a window two ticks long hit a big body again and again.
    pub fn first_strike(&mut self, window: usize, key: u64) -> bool {
        if self.struck.contains(&(window, key)) {
            return false;
        }
        self.struck.push((window, key));
        true
    }

    /// Before an SM64 tick: Mario's action, its arg and tick count after the last tick; the frame
    /// to play now (None: no move is playing). `units_per_m`: SM64 units in a metre.
    pub fn frame(&mut self, action: u32, arg: u32, timer: i32, units_per_m: f32) -> Option<Frame> {
        self.frame_in(MOVES, action, arg, timer, units_per_m)
    }

    /// `frame`, the moves read from `moves` (by id).
    pub fn frame_in(&mut self, moves: &'static [Move], action: u32, arg: u32, timer: i32, units_per_m: f32) -> Option<Frame> {
        let (id, t) = if let Some((id, target)) = self.starting.take() {
            let Some(m) = moves.get(id as usize) else {
                self.cur = None;
                return None;
            };
            self.cur = Some(id);
            self.landed = false;
            self.ground_shown = 0;
            self.struck.clear();
            self.starts = self.starts.wrapping_add(1);
            self.approach_m = m.approach.map_or(0.0, |a| approach_total(&a, target));
            (id, 0)
        } else {
            let id = self.cur?;
            // (another action: a blow on him, a fall, a cancel; the move is over)
            if !is_script(action) || arg != id as u32 {
                self.cur = None;
                return None;
            }
            (id, timer.clamp(0, u16::MAX as i32) as u16)
        };
        let m = &moves[id as usize];
        if t >= m.len {
            self.cur = None;
            let c = SM64ErScript { active: 1, end: 1, anim_id: -1, anim_frame: -1.0, squash: 1.0, ..Default::default() };
            return Some(Frame { id, t, c, window: None, reach: 0.0, reach_pitch: 0.0, blade: None, fx: 0 });
        }
        let tf = t as f32;
        let within = |(from, to): (u16, u16)| from <= t && t < to;
        let (anim_id, anim_frame) = m.clips.iter().rev().find(|c| c.from <= t).map_or((-1, -1.0), |c| (c.anim, eval(c.frames, tf, -1.0)));
        // (m a tick: its speed and its share of the lunge)
        let way = eval(m.speed, tf, 0.0) / 30.0 + m.approach.map_or(0.0, |a| approach_step(&a, t) * self.approach_m);
        let vel_y = m.vel_y.iter().find(|v| v.0 == t).map(|v| v.1);
        let sound = m.sounds.iter().find(|s| s.0 == t);
        let window = m.windows.iter().enumerate().find(|(_, w)| w.from <= t && t <= w.to);
        // (a ground window's look once, as he's on the ground: the landing's ring)
        let window_fx = match window {
            Some((i, w)) if w.ground => {
                let bit = 1u32 << (i as u32).min(31);
                if action & 0x800 == 0 && self.ground_shown & bit == 0 {
                    self.ground_shown |= bit;
                    w.fx
                } else {
                    0
                }
            }
            Some((_, w)) => w.fx,
            None => 0,
        };
        let pulse = m.pulse.filter(|&(every, _)| every > 0 && t % every == 0).map_or(0, |p| p.1);
        let fx = m.fx.iter().filter(|f| f.0 == t).fold(window_fx | pulse, |a, f| a | f.1);
        let cancel = m.cancels.iter().filter(|c| t >= if self.landed { c.on_hit.min(c.from) } else { c.from }).fold(0, |a, c| a | c.into);
        let c = SM64ErScript {
            active: 1,
            end: 0,
            anim_id,
            anim_frame,
            twist: eval(m.twist, tf, 0.0),
            pitch: eval(m.pitch, tf, 0.0),
            roll: eval(m.roll, tf, 0.0),
            turn: eval(m.turn, tf, 0.0),
            aim: m.aim.contains(&t) as i32,
            forward_vel: way * units_per_m,
            set_vel_y: vel_y.is_some() as i32,
            vel_y: vel_y.unwrap_or(0.0),
            hold: within(m.hold) as i32,
            land_ends: m.land_ends as i32,
            squash: eval(m.squash, tf, 1.0),
            dip: eval(m.dip, tf, 0.0),
            invulnerable: within(m.invulnerable) as i32,
            locked: within(m.locked) as i32,
            sound: sound.map_or(0, |s| s.1),
            sound_volume: sound.map_or(0.0, |s| s.2),
        };
        self.cancel = cancel;
        let blade = within(m.blade_held).then(|| (eval(m.blade_yaw, tf, 0.0), eval(m.blade_pitch, tf, 0.0), eval(m.blade_roll, tf, 0.0)));
        Some(Frame { id, t, c, window, reach: eval(m.reach, tf, 0.0), reach_pitch: m.reach_pitch, blade, fx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: &[Key] = &[k(0, 0.0, Linear), k(10, 10.0, Linear), k(20, 0.0, Out), k(30, 5.0, Hold)];

    #[test]
    fn curves_ease_between_keys_and_clamp_outside() {
        assert_eq!(eval(KEYS, 5.0, 0.0), 5.0);
        assert_eq!(eval(KEYS, -3.0, 0.0), 0.0);
        assert_eq!(eval(KEYS, 99.0, 0.0), 5.0);
        // (out: most of the way early)
        assert!(eval(KEYS, 12.0, 0.0) < 10.0 - 2.0 * 1.0);
        assert!((eval(KEYS, 15.0, 0.0) - 2.5).abs() < 0.01);
        // (hold: as it was until the key)
        assert_eq!(eval(KEYS, 29.0, 0.0), 0.0);
        assert_eq!(eval(KEYS, 30.0, 0.0), 5.0);
        assert_eq!(eval(&[], 3.0, 1.0), 1.0);
        assert_eq!(eval(&[k(4, 2.0, In)], 0.0, 1.0), 2.0);
    }

    #[test]
    fn the_lunge_covers_the_way_to_the_enemy() {
        let a = Approach { from: 5, to: 12, max: 7.0, gap: 0.9 };
        assert!((approach_total(&a, Some(5.0)) - 4.1).abs() < 1e-5);
        assert_eq!(approach_total(&a, Some(20.0)), 7.0);
        assert_eq!(approach_total(&a, Some(0.5)), 0.0);
        assert_eq!(approach_total(&a, None), 7.0);
        let steps: Vec<f32> = (0..30).map(|t| approach_step(&a, t)).collect();
        assert!((steps.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert_eq!(steps[4], 0.0);
        assert_eq!(steps[12], 0.0);
        // (eased out: the first step is the biggest)
        assert!(steps[5] > steps[11]);
    }

    #[test]
    fn the_lunge_ends_well_inside_its_hit_window() {
        // (`start` gets the distance to the enemy's body; the punch reaches 150 units past it: with
        // room for an enemy stepping back a bit)
        let a = BULLET.approach.unwrap();
        let Shape::Front { reach } = BULLET.windows[0].shape else { panic!("a front window") };
        for d in [1.0f32, 2.5, 4.0, 6.0, 7.5] {
            let end = d - approach_total(&a, Some(d));
            if d - a.gap <= a.max {
                assert!(end * 100.0 <= reach - 70.0, "{d} m away: ends {end} m off its body");
            }
        }
    }

    fn player_at(id: u16) -> Player {
        let mut p = Player::default();
        p.start(id, Some(5.0));
        p
    }

    #[test]
    fn a_move_plays_by_its_tick_count() {
        let mut p = player_at(BULLET_PUNCH);
        let f = p.frame(0x0C40_0201, 0, 0, 100.0).expect("the start frame");
        assert_eq!((f.id, f.t), (BULLET_PUNCH, 0));
        assert_eq!(f.c.active, 1);
        assert_eq!(f.c.end, 0);
        assert_eq!(f.c.anim_id, MARIO_ANIM_FIRST_PUNCH);
        assert_eq!(f.c.aim, 1);
        let f = p.frame(ACT_ER_SCRIPT, BULLET_PUNCH as u32, 6, 100.0).unwrap();
        assert_eq!(f.t, 6);
        assert_eq!(f.c.anim_id, MARIO_ANIM_DIVE);
        // (in the lunge: 4.1 m over 7 ticks, eased out, in SM64 units)
        assert!(f.c.forward_vel > 0.0);
        let f = p.frame(ACT_ER_SCRIPT_AIR, BULLET_PUNCH as u32, 5, 100.0).unwrap();
        assert_eq!(f.c.set_vel_y, 1);
        assert_eq!(f.c.vel_y, 14.0);
        assert_eq!(f.c.sound, SOUND_MARIO_PUNCH_HOO);
    }

    #[test]
    fn the_lunge_adds_up_in_sm64_units() {
        let mut p = player_at(BULLET_PUNCH);
        let mut way = 0.0;
        for t in 0..26 {
            let action = if t == 0 { 0 } else { ACT_ER_SCRIPT };
            way += p.frame(action, BULLET_PUNCH as u32, t, 100.0).unwrap().c.forward_vel;
        }
        // 4.1 m of lunge plus the landing's slide (3 m/s easing out over 6 ticks)
        assert!(way > 410.0 && way < 410.0 + 3.0 / 30.0 * 6.0 * 100.0, "{way}");
    }

    #[test]
    fn a_move_ends_or_is_dropped() {
        let mut p = player_at(BULLET_PUNCH);
        p.frame(0, 0, 0, 100.0);
        let f = p.frame(ACT_ER_SCRIPT, BULLET_PUNCH as u32, 26, 100.0).unwrap();
        assert_eq!(f.c.end, 1);
        assert_eq!(p.cancel(), ALL);
        // (hurt: another action, the move is gone)
        let mut p = player_at(BULLET_PUNCH);
        p.frame(0, 0, 0, 100.0);
        assert!(p.frame(0x0002_0464, 0, 3, 100.0).is_none());
        assert!(p.frame(ACT_ER_SCRIPT, BULLET_PUNCH as u32, 4, 100.0).is_none());
        // (nothing started: nothing to play)
        assert!(Player::default().frame(ACT_ER_SCRIPT, 0, 3, 100.0).is_none());
    }

    #[test]
    fn hit_windows_and_cancels_by_tick() {
        let mut p = player_at(BULLET_PUNCH);
        p.frame(0, 0, 0, 100.0);
        let at = |p: &mut Player, t: i32| p.frame(ACT_ER_SCRIPT, BULLET_PUNCH as u32, t, 100.0).unwrap();
        assert!(at(&mut p, 7).window.is_none());
        assert_eq!(at(&mut p, 8).window.map(|w| w.0), Some(0));
        assert_eq!(at(&mut p, 13).window.map(|w| w.0), Some(0));
        assert!(at(&mut p, 14).window.is_none());
        assert_eq!(p.cancel(), 0);
        at(&mut p, 19);
        assert_eq!(p.cancel(), ALL);
        at(&mut p, 9);
        assert_eq!(p.cancel(), 0);
        assert_eq!(Player::default().cancel(), ALL);
        assert_eq!(at(&mut p, 2).c.locked, 1);
        assert_eq!(at(&mut p, 5).c.locked, 0);
        p.landed();
        at(&mut p, 14);
        assert_eq!(p.cancel(), ALL);
        assert!(at(&mut p, 10).reach > 0.9);
    }

    #[test]
    fn a_window_strikes_each_enemy_once() {
        // (a hit-stop holds the move's tick while the real ticks and the cooldown run on: the same
        // window again on the same enemy is no new blow)
        let mut p = player_at(BULLET_PUNCH);
        p.frame(0, 0, 0, 100.0);
        assert!(p.first_strike(0, 7));
        assert!(!p.first_strike(0, 7));
        assert!(p.first_strike(0, 8));
        assert!(p.first_strike(1, 7));
        // (the next move: all new)
        p.start(BULLET_PUNCH, None);
        p.frame(0, 0, 0, 100.0);
        assert!(p.first_strike(0, 7));
        assert!(!p.has_landed());
        p.landed();
        assert!(p.has_landed());
    }

    #[test]
    fn a_ground_window_waits_for_the_landing() {
        static W: &[Window] = &[Window { from: 0, to: 9, shape: Shape::Around { radius: 100.0, ahead: 0.0 }, attack: Attack::Punch, fx: 0, ground: true }];
        static M: Move = Move { name: "test", len: 10, windows: W, air: true, ..BASE };
        assert!(window_open(&M.windows[0], true).is_none());
        assert!(window_open(&M.windows[0], false).is_some());
    }

    #[test]
    fn a_ground_windows_look_shows_once_as_he_lands() {
        static W: &[Window] = &[Window { from: 2, to: 9, shape: Shape::Around { radius: 100.0, ahead: 0.0 }, attack: Attack::Punch, fx: 0x40, ground: true }];
        static M: Move = Move { name: "test", len: 12, windows: W, air: true, ..BASE };
        let mut p = Player::default();
        p.start(0, None);
        p.frame_in(std::slice::from_ref(&M), 0, 0, 0, 100.0);
        // (in the air: nothing yet)
        assert_eq!(p.frame_in(std::slice::from_ref(&M), ACT_ER_SCRIPT_AIR, 0, 3, 100.0).unwrap().fx & 0x40, 0);
        // on the ground: once
        assert_eq!(p.frame_in(std::slice::from_ref(&M), ACT_ER_SCRIPT, 0, 4, 100.0).unwrap().fx & 0x40, 0x40);
        assert_eq!(p.frame_in(std::slice::from_ref(&M), ACT_ER_SCRIPT, 0, 5, 100.0).unwrap().fx & 0x40, 0);
    }

    #[test]
    fn a_pulse_comes_back_every_so_often() {
        static M: Move = Move { name: "test", len: 40, pulse: Some((10, 0x100)), ..BASE };
        let mut p = Player::default();
        p.start(0, None);
        let fx: Vec<u32> = (0..25).map(|t| p.frame_in(std::slice::from_ref(&M), if t == 0 { 0 } else { ACT_ER_SCRIPT }, 0, t, 100.0).unwrap().fx & 0x100).collect();
        assert!(fx[0] != 0 && fx[10] != 0 && fx[20] != 0);
        assert_eq!(fx.iter().filter(|f| **f != 0).count(), 3);
    }

    #[test]
    fn every_move_is_well_formed() {
        for m in MOVES {
            assert!(m.len > 0, "{}", m.name);
            for keys in [m.twist, m.pitch, m.roll, m.turn, m.speed, m.squash, m.dip, m.reach, m.blade_yaw, m.blade_pitch, m.blade_roll] {
                assert!(keys.windows(2).all(|w| w[0].t <= w[1].t), "{}: keys out of order", m.name);
            }
            assert!(m.clips.windows(2).all(|w| w[0].from < w[1].from), "{}", m.name);
            for w in m.windows {
                assert!(w.from <= w.to && w.to < m.len, "{}: window outside the move", m.name);
            }
            assert!(m.sounds.windows(2).all(|w| w[0].0 < w[1].0), "{}: one sound a tick, in order", m.name);
            if let Some(a) = m.approach {
                assert!(a.from < a.to && a.to <= m.len, "{}", m.name);
            }
        }
    }
}
