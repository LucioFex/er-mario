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
    /// the right hand's reach toward the aimed enemy (0-1, arm IK)
    pub reach: &'static [Key],
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
    windows: &[],
    cancels: &[],
    sounds: &[],
    fx: &[],
    locked: (0, 0),
    invulnerable: (0, 0),
    air: false,
    land_ends: false,
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
    windows: &[Window { from: 8, to: 13, shape: Shape::Front { reach: 150.0 }, attack: Attack::BulletPunch, fx: 0 }],
    cancels: &[Cancel { into: ALL, from: 19, on_hit: 13 }],
    sounds: &[(0, SOUND_MARIO_UH, 0.5), (5, SOUND_MARIO_PUNCH_HOO, 0.0), (6, SOUND_ACTION_THROW, 0.0), (7, SOUND_GENERAL_SWISH_AIR, 0.45)],
    fx: &[(5, crate::moves::FX_LAUNCH)],
    locked: (0, 5),
    ..BASE
};

pub const MOVES: &[Move] = &[BULLET];

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

    /// Before an SM64 tick: Mario's action, its arg and tick count after the last tick; the frame
    /// to play now (None: no move is playing). `units_per_m`: SM64 units in a metre.
    pub fn frame(&mut self, action: u32, arg: u32, timer: i32, units_per_m: f32) -> Option<Frame> {
        let (id, t) = if let Some((id, target)) = self.starting.take() {
            let Some(m) = MOVES.get(id as usize) else {
                self.cur = None;
                return None;
            };
            self.cur = Some(id);
            self.landed = false;
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
        let m = &MOVES[id as usize];
        if t >= m.len {
            self.cur = None;
            let c = SM64ErScript { active: 1, end: 1, anim_id: -1, anim_frame: -1.0, squash: 1.0, ..Default::default() };
            return Some(Frame { id, t, c, window: None, reach: 0.0, fx: 0 });
        }
        let tf = t as f32;
        let within = |(from, to): (u16, u16)| from <= t && t < to;
        let (anim_id, anim_frame) = m.clips.iter().rev().find(|c| c.from <= t).map_or((-1, -1.0), |c| (c.anim, eval(c.frames, tf, -1.0)));
        // (m a tick: its speed and its share of the lunge)
        let way = eval(m.speed, tf, 0.0) / 30.0 + m.approach.map_or(0.0, |a| approach_step(&a, t) * self.approach_m);
        let vel_y = m.vel_y.iter().find(|v| v.0 == t).map(|v| v.1);
        let sound = m.sounds.iter().find(|s| s.0 == t);
        let window = m.windows.iter().enumerate().find(|(_, w)| w.from <= t && t <= w.to);
        let fx = m.fx.iter().filter(|f| f.0 == t).fold(window.map_or(0, |w| w.1.fx), |a, f| a | f.1);
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
        Some(Frame { id, t, c, window, reach: eval(m.reach, tf, 0.0), fx })
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
    fn every_move_is_well_formed() {
        for m in MOVES {
            assert!(m.len > 0, "{}", m.name);
            for keys in [m.twist, m.pitch, m.roll, m.turn, m.speed, m.squash, m.dip, m.reach] {
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
