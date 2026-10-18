//! Co-op (Seamless Co-op, docs/coop): each player runs their own Mario (or Luigi) and the game
//! only syncs the Tarnished under him, so the mod sends the rest itself over net.rs:
//! - the pose: the 31 parts of the Mario model, ~30 times a second; the partner's character is
//!   posed with it (engine_mario::apply_remote), so each sees the other as Mario or Luigi;
//! - the guest's blows: the host owns the enemies (a health the guest writes is overwritten by
//!   the host's), so the guest's hits go to the host, who takes them off (Combat::remote_hits);
//! - shoves: a blow on the partner pushes them a little (no damage).
//! Outside a session nothing here runs: single player is untouched.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eldenring::cs::{CSSessionManager, FieldInsHandle, LobbyState, PlayerIns, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::engine_mario::{PARTS, PartPose};
use crate::{explore, log, net};

const MAGIC: [u8; 3] = *b"ERM";
/// bumped when a packet changes: the other side's older ones are ignored (and logged)
/// (2: the pose carries the player's health and character, for the partner's HUD; 3: the trap kit's
/// parts; 4: the status, for the partner's HUD and the BROS!; 5: the pose's parts in 2 mm steps (out
/// to 65 m: a boomerang far off), its clock and place always, a place in a dungeon too, and the
/// effects around the player after it)
/// From 4 on, a kind of packet a build doesn't know is skipped, and one longer than it expects is
/// read as far as it knows: a later build of the same version can add kinds (grabbing and throwing
/// the partner: the status's CARRYING and CARRIED are kept for it) or fields at the end without
/// breaking an older one's co-op.
const VERSION: u8 = 5;
const KIND_POSE: u8 = 1;
const KIND_HIT: u8 = 2;
const KIND_SHOVE: u8 = 3;
/// nothing but "here I am": Steam takes a player's messages only once we've sent them something,
/// so this goes out every second even while we send nothing else (e.g. out of Mario mode)
const KIND_HELLO: u8 = 4;
/// how the player is doing (Status), a few times a second and when it changes
const KIND_STATUS: u8 = 5;
/// the sounds Mario's body made (his voice, steps, blows), for the partner to hear
const KIND_SOUNDS: u8 = 6;
/// at most this many sounds in one packet
const SOUNDS_MAX: usize = 32;
const HEADER: usize = 5;
/// one part: rotation i16 x4, position i16 x3 in POS_STEP, scale and stretch u16 /1000 x4
const PART_BYTES: usize = 22;
/// a part's position in steps of this (m)
const POS_STEP: f32 = 0.002;
/// seq u32, health u16 (SM64's: wedges in its high byte), character u8 (1 Luigi), then the parts
const POSE_BYTES: usize = HEADER + 4 + 3 + PARTS * PART_BYTES;
const PARTS_AT: usize = HEADER + 7;
/// after the parts: the sender's clock (ms), the place's kind (u8: 0 none, 1 map chunk, 2 in its
/// block), its block i32, its position f32 x3, yaw f32; then the effects around them (fx.rs)
const POSE_EXT_BYTES: usize = 4 + 1 + 4 + 12 + 4;
const HIT_BYTES: usize = HEADER + 8 + 4 + 12 + 1 + 4;
const SHOVE_BYTES: usize = HEADER + 8;
/// seq u16, health u16, character u8, coins u16, power u8, weapon u8, rank u8 (0xFF none), hits
/// u16, flags u8, last enemy hit: handle key u64 and ms ago u16 (0xFFFF none), the coin bag: coins
/// u16 (0 none), its origin block i32, its chunk position f32 x3
const STATUS_BYTES: usize = HEADER + 2 + 2 + 1 + 2 + 1 + 1 + 1 + 2 + 1 + 8 + 2 + 2 + 4 + 12;

/// how often the pose goes out (a frame early is close enough: at 60 fps every second frame, not
/// now two and now three)
const POSE_EVERY: Duration = Duration::from_millis(33);
const POSE_SLACK: Duration = Duration::from_millis(5);
/// the partner is shown this far back in time (s), so there are always two poses to go between:
/// at least INTERP_BASE, more the more their packets' trips vary, at most INTERP_MAX; past the
/// last pose they're carried on along their way for at most EXTRAPOLATE (s)
const INTERP_BASE: f64 = 0.05;
const INTERP_MAX: f64 = 0.25;
const EXTRAPOLATE: f64 = 0.1;
/// how quickly the clocks' difference creeps up to a slower trip (per pose: they drift apart a
/// little), and a jump this big (s) means they restarted
const OFFSET_DRIFT: f64 = 0.002;
const OFFSET_RESET: f64 = 2.0;
/// poses kept
const SAMPLES: usize = 12;
/// their own place is trusted within this far (m) of where the game has their Tarnished
const PLACE_TRUST: f32 = 40.0;
/// their sounds: at most this loud next to us (ours are 1: theirs never drown ours out), fading
/// with the distance at this rate (per m past PARTNER_NEAR), none past PARTNER_FAR (m)
const PARTNER_LOUD: f32 = 0.55;
const PARTNER_NEAR: f32 = 2.0;
const PARTNER_FADE: f32 = 0.15;
const PARTNER_FAR: f32 = 50.0;
/// how often the session is looked at
const SESSION_EVERY: Duration = Duration::from_millis(500);
/// a blow pushes the partner this close (m, times Mario's size), at most this often
const SHOVE_REACH: f32 = 1.4;
const SHOVE_EVERY: Duration = Duration::from_millis(1000);
const HELLO_EVERY: Duration = Duration::from_millis(1000);
/// a partner's pose with nothing new for this long is let go (they left Mario mode or the area)
const POSE_STALE: f32 = 1.0;
/// a guest's blow older than this when the host gets to it is dropped (not all at once later)
const HIT_STALE: f32 = 0.5;

pub struct Hit {
    pub handle: FieldInsHandle,
    pub chr_id: u32,
    pub pos: [f32; 3],
    /// Attack::code (combat::IMPACT_CODE for an impact)
    pub attack: u8,
    /// the share of max HP the guest worked out (their caps, Mega and counters in it)
    pub pct: f32,
}

/// Status flags: in a fight, just hurt, down (out of health)
pub const FIGHTING: u8 = 1;
pub const HURT: u8 = 2;
pub const DOWN: u8 = 4;
/// (kept for grabbing and throwing the partner: holding them, held)
#[allow(dead_code)]
pub const CARRYING: u8 = 8;
#[allow(dead_code)]
pub const CARRIED: u8 = 16;

/// Status powers: what the player is using
pub const POWER_NONE: u8 = 0;
pub const POWER_WING: u8 = 1;
pub const POWER_METAL: u8 = 2;
pub const POWER_VANISH: u8 = 3;
pub const POWER_ROCK: u8 = 4;
pub const POWER_MEGA: u8 = 5;
pub const POWER_YOSHI: u8 = 6;
pub const POWER_CAPTURE: u8 = 7;

/// How a player is doing, for their partner's HUD.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Status {
    /// SM64's health (wedges in its high byte)
    pub health: u16,
    pub luigi: bool,
    pub coins: u16,
    pub power: u8,
    pub weapon: u8,
    /// their style string's rank (None: no string), its blows
    pub rank: Option<u8>,
    pub hits: u16,
    pub flags: u8,
    /// the enemy they last hit (handle key) and how long ago (ms)
    pub hit: Option<(u64, u16)>,
    /// their coin bag: its coins, its map origin block, its chunk position
    pub bag: Option<(u16, i32, [f32; 3])>,
}

/// Where a player's Tarnished is and faces (the pose's parts are around it), in map chunk
/// coordinates (the same on both games, physics positions aren't): its origin block, its chunk
/// position, its yaw (radians).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Place {
    pub origin: i32,
    pub chunk: Vec3,
    pub yaw: f32,
    /// off the open world's chunks (a dungeon, a building): `origin` is its map block and `chunk`
    /// its physics position there
    pub local: bool,
}

impl Place {
    /// Between `self` and `b` (`t` 0-1): the shorter way round for the yaw; another block, `b`.
    fn lerp(&self, b: &Place, t: f32) -> Place {
        if self.origin != b.origin || self.local != b.local {
            return *b;
        }
        let turn = (b.yaw - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        Place { chunk: self.chunk.lerp(b.chunk, t), yaw: self.yaw + turn * t, ..*b }
    }
}

pub enum Packet {
    /// `ms`: the sender's clock; `place`: where its parts are; `fx`: the effects around them
    /// (encoded from the effects around `here`, their Tarnished; read: around it)
    Pose { seq: u32, health: u16, luigi: bool, parts: Box<[PartPose; PARTS]>, ms: Option<u32>, place: Option<Place>, fx: Option<std::sync::Arc<crate::fx::WorldFx>> },
    Sounds(Vec<u32>),
    Hit(Hit),
    Shove { dir: [f32; 2] },
    Hello,
    Status { seq: u16, status: Status },
}

fn header(kind: u8, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(kind);
    out
}

fn q16(v: f32, k: f32) -> [u8; 2] {
    ((v * k).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16).to_le_bytes()
}

fn u16k(v: f32) -> [u8; 2] {
    ((v * 1000.0).round().clamp(0.0, u16::MAX as f32) as u16).to_le_bytes()
}

pub fn encode(p: &Packet) -> Vec<u8> {
    match p {
        Packet::Pose { seq, health, luigi, parts, ms, place, fx } => {
            let mut out = header(KIND_POSE, POSE_BYTES + POSE_EXT_BYTES + 256);
            out.extend_from_slice(&seq.to_le_bytes());
            out.extend_from_slice(&health.to_le_bytes());
            out.push(*luigi as u8);
            for part in parts.iter() {
                let r = part.rot.normalize();
                for v in [r.x, r.y, r.z, r.w] {
                    out.extend_from_slice(&q16(v, 32767.0));
                }
                for v in [part.pos.x, part.pos.y, part.pos.z] {
                    out.extend_from_slice(&q16(v, 1.0 / POS_STEP));
                }
                for v in [part.scale, part.stretch.x, part.stretch.y, part.stretch.z] {
                    out.extend_from_slice(&u16k(v));
                }
            }
            out.extend_from_slice(&ms.unwrap_or(0).to_le_bytes());
            let p = place.unwrap_or(Place { origin: 0, chunk: Vec3::ZERO, yaw: 0.0, local: false });
            out.push(match place {
                None => 0,
                Some(p) if p.local => 2,
                Some(_) => 1,
            });
            out.extend_from_slice(&p.origin.to_le_bytes());
            for v in [p.chunk.x, p.chunk.y, p.chunk.z, p.yaw] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            // (the effects are written by frame(), around our Tarnished: see encode_pose)
            if let Some(fx) = fx {
                fx.encode(Vec3::ZERO, &mut out);
            }
            out
        }
        Packet::Sounds(ids) => {
            let ids = &ids[..ids.len().min(SOUNDS_MAX)];
            let mut out = header(KIND_SOUNDS, HEADER + 1 + ids.len() * 4);
            out.push(ids.len() as u8);
            for id in ids {
                out.extend_from_slice(&id.to_le_bytes());
            }
            out
        }
        Packet::Hit(h) => {
            let mut out = header(KIND_HIT, HIT_BYTES);
            out.extend_from_slice(&crate::combat::handle_key(&h.handle).to_le_bytes());
            out.extend_from_slice(&h.chr_id.to_le_bytes());
            for v in h.pos {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.push(h.attack);
            out.extend_from_slice(&h.pct.to_le_bytes());
            out
        }
        Packet::Shove { dir } => {
            let mut out = header(KIND_SHOVE, SHOVE_BYTES);
            out.extend_from_slice(&dir[0].to_le_bytes());
            out.extend_from_slice(&dir[1].to_le_bytes());
            out
        }
        Packet::Hello => header(KIND_HELLO, HEADER),
        Packet::Status { seq, status: s } => {
            let mut out = header(KIND_STATUS, STATUS_BYTES);
            out.extend_from_slice(&seq.to_le_bytes());
            out.extend_from_slice(&s.health.to_le_bytes());
            out.push(s.luigi as u8);
            out.extend_from_slice(&s.coins.to_le_bytes());
            out.push(s.power);
            out.push(s.weapon);
            out.push(s.rank.unwrap_or(0xFF));
            out.extend_from_slice(&s.hits.to_le_bytes());
            out.push(s.flags);
            let (key, ago) = s.hit.unwrap_or((0, u16::MAX));
            out.extend_from_slice(&key.to_le_bytes());
            out.extend_from_slice(&ago.to_le_bytes());
            let (coins, block, chunk) = s.bag.unwrap_or((0, 0, [0.0; 3]));
            out.extend_from_slice(&coins.to_le_bytes());
            out.extend_from_slice(&block.to_le_bytes());
            for v in chunk {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out
        }
    }
}

pub fn decode(b: &[u8]) -> Option<Packet> {
    if b.len() < HEADER || b[..3] != MAGIC || b[3] != VERSION {
        return None;
    }
    let i16at = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]) as f32;
    let u16at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as f32 / 1000.0;
    let f32at = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    // (longer than expected: a later build's fields at the end, skipped)
    match b[4] {
        KIND_POSE if b.len() >= POSE_BYTES => {
            let seq = u32::from_le_bytes([b[5], b[6], b[7], b[8]]);
            let health = u16::from_le_bytes([b[9], b[10]]);
            let luigi = b[11] == 1;
            let mut parts = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
            for (k, p) in parts.iter_mut().enumerate() {
                let o = PARTS_AT + k * PART_BYTES;
                let q = Quat::from_xyzw(i16at(o), i16at(o + 2), i16at(o + 4), i16at(o + 6));
                p.rot = if q.length_squared() > 0.0 { q.normalize() } else { Quat::IDENTITY };
                p.pos = Vec3::new(i16at(o + 8), i16at(o + 10), i16at(o + 12)) * POS_STEP;
                p.scale = u16at(o + 14);
                p.stretch = Vec3::new(u16at(o + 16), u16at(o + 18), u16at(o + 20));
            }
            // the clock and the place, then the effects
            let (ms, place, fx) = if b.len() >= POSE_BYTES + POSE_EXT_BYTES {
                let o = POSE_BYTES;
                let ms = u32::from_le_bytes(b[o..o + 4].try_into().ok()?);
                let chunk = Vec3::new(f32at(o + 9), f32at(o + 13), f32at(o + 17));
                let yaw = f32at(o + 21);
                let kind = b[o + 4];
                let ok = matches!(kind, 1 | 2) && chunk.is_finite() && yaw.is_finite();
                let place = ok.then(|| Place { origin: i32::from_le_bytes([b[o + 5], b[o + 6], b[o + 7], b[o + 8]]), chunk, yaw, local: kind == 2 });
                let fx = crate::fx::WorldFx::decode(&b[o + POSE_EXT_BYTES..]).filter(|f| !f.is_empty()).map(std::sync::Arc::new);
                (Some(ms), place, fx)
            } else {
                (None, None, None)
            };
            Some(Packet::Pose { seq, health, luigi, parts: Box::new(parts), ms, place, fx })
        }
        KIND_SOUNDS if b.len() > HEADER => {
            let n = (b[HEADER] as usize).min(SOUNDS_MAX);
            (b.len() >= HEADER + 1 + n * 4).then(|| Packet::Sounds((0..n).map(|k| u32::from_le_bytes([b[HEADER + 1 + k * 4], b[HEADER + 2 + k * 4], b[HEADER + 3 + k * 4], b[HEADER + 4 + k * 4]])).collect()))
        }
        KIND_HIT if b.len() >= HIT_BYTES => {
            let key = u64::from_le_bytes(b[5..13].try_into().ok()?);
            let (pos, pct) = ([f32at(17), f32at(21), f32at(25)], f32at(30));
            if !pos.iter().all(|v| v.is_finite()) || !pct.is_finite() {
                return None;
            }
            Some(Packet::Hit(Hit {
                handle: unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) },
                chr_id: u32::from_le_bytes(b[13..17].try_into().ok()?),
                pos,
                attack: b[29],
                pct,
            }))
        }
        KIND_SHOVE if b.len() >= SHOVE_BYTES => {
            let dir = [f32at(5), f32at(9)];
            dir.iter().all(|v| v.is_finite()).then_some(Packet::Shove { dir })
        }
        KIND_HELLO => Some(Packet::Hello),
        KIND_STATUS if b.len() >= STATUS_BYTES => {
            let u16at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
            let key = u64::from_le_bytes(b[18..26].try_into().ok()?);
            let ago = u16at(26);
            let bag_coins = u16at(28);
            let chunk = [f32at(34), f32at(38), f32at(42)];
            let status = Status {
                health: u16at(7),
                luigi: b[9] == 1,
                coins: u16at(10),
                power: b[12],
                weapon: b[13],
                rank: (b[14] != 0xFF).then_some(b[14]),
                hits: u16at(15),
                flags: b[17],
                hit: (ago != u16::MAX).then_some((key, ago)),
                bag: (bag_coins > 0 && chunk.iter().all(|v| v.is_finite())).then_some((bag_coins, i32::from_le_bytes(b[30..34].try_into().ok()?), chunk)),
            };
            Some(Packet::Status { seq: u16at(5), status })
        }
        // (a kind of a later build's: skipped)
        _ => None,
    }
}

/// One pose of a partner's: their clock's time for it (s), the parts, where they are.
struct Sample {
    t: f64,
    parts: [PartPose; PARTS],
    place: Option<Place>,
    fx: Option<std::sync::Arc<crate::fx::WorldFx>>,
}

/// A partner's poses as they come in, shown a little back in time between the two around it
/// (snapshot interpolation): packets arriving unevenly no longer make them stutter.
struct Remote {
    steam: u64,
    samples: std::collections::VecDeque<Sample>,
    seq: u32,
    /// when the last one came in (ours)
    at: Instant,
    health: u16,
    luigi: bool,
    /// our clock minus theirs (s: the smallest seen, the quickest trip), how much the trips vary (s)
    offset: Option<f64>,
    jitter: f64,
}

/// Our clock (s, since the mod started).
fn clock() -> f64 {
    static T0: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
    T0.elapsed().as_secs_f64()
}

fn clock_ms() -> u32 {
    (clock() * 1000.0) as u64 as u32
}

impl Remote {
    fn new(steam: u64, seq: u32, health: u16, luigi: bool) -> Self {
        Remote { steam, samples: std::collections::VecDeque::with_capacity(SAMPLES + 1), seq, at: Instant::now(), health, luigi, offset: None, jitter: 0.01 }
    }

    /// A pose came in: `t` their clock's time for it (s; None, an older build's: when it came).
    fn add(&mut self, t: Option<f64>, parts: [PartPose; PARTS], place: Option<Place>, fx: Option<std::sync::Arc<crate::fx::WorldFx>>) {
        let local = clock();
        let t = t.unwrap_or(local);
        let d = local - t;
        match self.offset {
            // (a jump: they restarted; or the first)
            Some(o) if (d - o).abs() < OFFSET_RESET => {
                let o = if d < o { d } else { o + (d - o) * OFFSET_DRIFT };
                self.jitter += ((d - o).abs() - self.jitter) * 0.05;
                self.offset = Some(o);
            }
            _ => {
                self.offset = Some(d);
                self.samples.clear();
            }
        }
        let at = self.samples.iter().position(|s| s.t > t).unwrap_or(self.samples.len());
        self.samples.insert(at, Sample { t, parts, place, fx });
        while self.samples.len() > SAMPLES {
            self.samples.pop_front();
        }
        self.at = Instant::now();
    }

    /// How far back they're shown (s).
    fn delay(&self) -> f64 {
        (INTERP_BASE + 2.5 * self.jitter).clamp(INTERP_BASE, INTERP_MAX)
    }

    /// Their pose and place now.
    fn now(&self) -> ([PartPose; PARTS], Option<Place>) {
        sample_at(&self.samples, self.target())
    }

    /// The time they're shown at (their clock).
    fn target(&self) -> f64 {
        clock() - self.offset.unwrap_or(0.0) - self.delay()
    }

    /// Their effects now: the last pose's at or before then (effects aren't blended).
    fn fx(&self) -> Option<std::sync::Arc<crate::fx::WorldFx>> {
        let t = self.target();
        self.samples.iter().rev().find(|s| s.t <= t).or(self.samples.front()).and_then(|s| s.fx.clone())
    }
}

/// The pose at time `t` (their clock) from poses in order: between the two around it, the first
/// before them all, the last after them (its place carried on its way a little).
fn sample_at(s: &std::collections::VecDeque<Sample>, t: f64) -> ([PartPose; PARTS], Option<Place>) {
    let (Some(first), Some(last)) = (s.front(), s.back()) else {
        return ([PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS], None);
    };
    if t <= first.t {
        return (first.parts, first.place);
    }
    for (a, b) in s.iter().zip(s.iter().skip(1)) {
        if t < b.t {
            let u = ((t - a.t) / (b.t - a.t).max(1e-6)) as f32;
            let place = match (a.place, b.place) {
                (Some(pa), Some(pb)) => Some(pa.lerp(&pb, u)),
                (_, p) => p,
            };
            return (crate::engine_mario::blend(&a.parts, &b.parts, u), place);
        }
    }
    // past the last: on along the way it was going, briefly
    let place = match (s.len().checked_sub(2).and_then(|i| s.get(i)), last.place) {
        (Some(prev), Some(pl)) if prev.place.is_some_and(|pp| pp.origin == pl.origin) && last.t > prev.t => {
            let pp = prev.place.unwrap_or(pl);
            let v = (pl.chunk - pp.chunk) / (last.t - prev.t) as f32;
            Some(Place { chunk: pl.chunk + v * (t - last.t).min(EXTRAPOLATE) as f32, ..pl })
        }
        (_, p) => p,
    };
    (last.parts, place)
}

/// Their pose put where they really are (their own place, smoothed), not where the game shows
/// their Tarnished (its own updates come slower and jerkier, and Mario moves faster than it):
/// the parts turned and moved from that Tarnished's place to theirs. Also where their feet are
/// shown (Elden Ring), and which way they face.
fn placed(steam: u64, chr: &PlayerIns, parts: &[PartPose; PARTS], place: Option<Place>) -> ([PartPose; PARTS], Vec3, Quat) {
    let c = &chr.chr_ins;
    let ph = &c.modules.physics;
    let here = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let o = ph.orientation;
    let rot = Quat::from_xyzw(o.0, o.1, o.2, o.3).normalize();
    // (no orientation yet, a character still loading: as the game has it)
    if !rot.is_finite() || !here.is_finite() {
        return (*parts, here, Quat::IDENTITY);
    }
    // their place in our physics position: in the open world by its chunk position (through their
    // Tarnished's own on our game, else ours); in a dungeon or a building, the same block's
    // physics position as is
    let at = place.and_then(|pl| {
        let at = if pl.local {
            (i32::from(c.block_id) == pl.origin).then_some(pl.chunk)
        } else {
            let ck = Vec3::new(c.chunk_position.0, c.chunk_position.1, c.chunk_position.2);
            if i32::from(c.block_origin) == pl.origin && ck.length_squared() > 1e-6 { Some(pl.chunk - (ck - here)) } else { crate::coins::place(pl.origin, pl.chunk) }
        };
        at.filter(|a| a.is_finite() && a.distance(here) < PLACE_TRUST).map(|a| (a - here, Quat::from_rotation_y(pl.yaw)))
    });
    // (between their own place and where the game has them, a moment's blend: no jump)
    let (shift, face) = {
        let mut all = HANDOVER.lock().unwrap_or_else(|e| e.into_inner());
        let goal = at.unwrap_or((Vec3::ZERO, rot));
        let i = match all.iter().position(|h| h.steam == steam) {
            Some(i) => i,
            None => {
                all.push(Handover { steam, own: at.is_some(), from: goal, last: goal, since: None });
                all.len() - 1
            }
        };
        let h = &mut all[i];
        if h.own != at.is_some() {
            h.own = at.is_some();
            h.from = h.last;
            h.since = Some(Instant::now());
        }
        let u = h.since.map_or(1.0, |t| (t.elapsed().as_secs_f32() / HANDOVER_TIME).min(1.0));
        let u = u * u * (3.0 - 2.0 * u);
        let now = (h.from.0.lerp(goal.0, u), h.from.1.slerp(goal.1, u).normalize());
        h.last = now;
        now
    };
    let turn = (rot.inverse() * face).normalize();
    let moved = rot.inverse() * shift;
    (parts.map(|p| PartPose { rot: (turn * p.rot).normalize(), pos: turn * p.pos + moved, ..p }), here + shift, face)
}

/// Each partner shown at their own place or where the game has them, and the blend between the
/// two when that changes (a far zip, into a dungeon): where they're shown (the shift from the
/// game's place, the facing) as last worked out, the blend's start.
struct Handover {
    steam: u64,
    own: bool,
    from: (Vec3, Quat),
    last: (Vec3, Quat),
    since: Option<Instant>,
}

static HANDOVER: Mutex<Vec<Handover>> = Mutex::new(Vec::new());
/// the blend's length (s)
const HANDOVER_TIME: f32 = 0.35;

/// Our pose for the partner: the parts, our place, and our effects around our Tarnished.
fn encode_pose(seq: u32, parts: [PartPose; PARTS]) -> Vec<u8> {
    let mut out = encode(&Packet::Pose { seq, health: own_health(), luigi: crate::equip::luigi(), parts: Box::new(parts), ms: Some(clock_ms()), place: own_place(), fx: None });
    let here = unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.main_player.as_ref().map(|p| p.chr_ins.modules.physics.position)).map(|p| Vec3::new(p.0, p.1, p.2));
    if let Some(here) = here.filter(|h| h.is_finite()) {
        crate::fx::WorldFx::local().encode(here, &mut out);
    }
    out
}

/// Where our Tarnished is and faces (the pose's parts are around it), for the partner.
fn own_place() -> Option<Place> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let c = &wcm.main_player.as_ref()?.chr_ins;
    let ck = Vec3::new(c.chunk_position.0, c.chunk_position.1, c.chunk_position.2);
    let o = c.modules.physics.orientation;
    let (yaw, _, _) = Quat::from_xyzw(o.0, o.1, o.2, o.3).normalize().to_euler(glam::EulerRot::YXZ);
    if !yaw.is_finite() {
        return None;
    }
    // (off the open world's chunks, in a dungeon: its block and the physics position there)
    if ck.length_squared() < 1e-6 || !ck.is_finite() {
        let p = c.modules.physics.position;
        let p = Vec3::new(p.0, p.1, p.2);
        return p.is_finite().then(|| Place { origin: i32::from(c.block_id), chunk: p, yaw, local: true });
    }
    Some(Place { origin: i32::from(c.block_origin), chunk: ck, yaw, local: false })
}

#[derive(Default)]
struct State {
    checked: Option<Instant>,
    /// the session's other players (Steam ids), the host's, whether that's us
    peers: Vec<u64>,
    host_id: u64,
    host: bool,
    /// each partner's character this frame (Steam id, PlayerIns address)
    chrs: Vec<(u64, usize)>,
    remotes: Vec<Remote>,
    sent: Option<Instant>,
    hello: Option<Instant>,
    seq: u32,
    hits: Vec<(Instant, Hit)>,
    shove: Option<Vec3>,
    shoved: Option<Instant>,
    old_version: bool,
    unpaired: bool,
    /// the partners' status as it comes in
    mates: Vec<Mate>,
    /// our status: last sent (what, when), its seq
    status_sent: Option<(Status, Instant)>,
    status_seq: u16,
    /// the partners' sounds come in, to play (who, which)
    sounds_in: Vec<(u64, u32)>,
}

/// A partner's status, and what their HUD card makes of it.
struct Mate {
    steam: u64,
    st: Status,
    seq: u16,
    at: Instant,
    /// the enemy they last hit and when (on our clock)
    hit: Option<(u64, Instant)>,
    hurt_at: Option<Instant>,
    /// "LUIGI!" said (low health) when, and them down
    low_said: Option<Instant>,
    down: bool,
    /// their card: how shown (0-1), and when it last had a reason to show
    alpha: f32,
    wanted: Option<Instant>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static GUEST: AtomicBool = AtomicBool::new(false);

/// `coop_test = ...` in er_mario.ini (developer tests with one PC): `loopback` sends every kind of
/// packet to ourselves over Steam and logs what comes back; `guest` plays as a guest would (no
/// grabs or moving enemies, their health never below 1).
fn test_mode(what: &str) -> bool {
    static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    MODE.get_or_init(|| {
        let mode = crate::paths::config("coop_test").unwrap_or_default().to_ascii_lowercase();
        if !mode.is_empty() {
            log(format!("coop: test mode '{mode}'"));
        }
        mode
    })
    .contains(what)
}

/// The loopback test: sent and received counts, when last sent, kinds seen.
struct Loopback {
    sent: u32,
    got: u32,
    at: Option<Instant>,
    summary: Option<Instant>,
    seen: Vec<u8>,
    no_id: bool,
}
static LOOPBACK: Mutex<Loopback> = Mutex::new(Loopback { sent: 0, got: 0, at: None, summary: None, seen: Vec::new(), no_id: false });

fn loopback() {
    let mut t = LOOPBACK.lock().unwrap_or_else(|e| e.into_inner());
    net::receive(|from, data| {
        t.got += 1;
        let kind = data.get(4).copied().unwrap_or(0);
        if !t.seen.contains(&kind) {
            t.seen.push(kind);
            let what = match decode(data) {
                Some(Packet::Pose { seq, health, luigi, parts, ms, place, fx }) => format!("pose seq {seq}, health {health:#x}, luigi {luigi}, head at {:.2} m, clock {ms:?}, place {place:?}, effects {}", parts[3].pos.length(), fx.is_some()),
                Some(Packet::Sounds(ids)) => format!("{} sounds", ids.len()),
                Some(Packet::Hit(h)) => format!("hit c{:04} code {} {:.1}%", h.chr_id, h.attack, h.pct),
                Some(Packet::Shove { dir }) => format!("shove {dir:?}"),
                Some(Packet::Hello) => "hello".into(),
                Some(Packet::Status { seq, status }) => format!("status seq {seq}, {status:?}"),
                None => "UNREADABLE".into(),
            };
            log(format!("coop test: loopback got {what} from {from:x} ({} B)", data.len()));
        }
    });
    if t.at.is_some_and(|a| a.elapsed() < Duration::from_secs(1)) {
        return;
    }
    t.at = Some(Instant::now());
    let Some(me) = net::my_id() else {
        if !std::mem::replace(&mut t.no_id, true) {
            log("coop test: no Steam id of our own");
        }
        return;
    };
    let pose = (*crate::engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()))
        .unwrap_or([PartPose { rot: Quat::IDENTITY, pos: Vec3::Y, scale: 1.0, stretch: Vec3::ONE }; PARTS]);
    let key = 0x0001_0002_0000_0003u64;
    let packets = [
        (encode(&Packet::Hello), false),
        (encode_pose(t.sent + 1, pose), false),
        (encode(&Packet::Sounds(vec![0x2404_8081])), false),
        (encode(&Packet::Hit(Hit { handle: unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) }, chr_id: 4750, pos: [1.0, 2.0, 3.0], attack: 0, pct: 4.5 })), true),
        (encode(&Packet::Shove { dir: [0.6, 0.8] }), true),
        (encode(&Packet::Status { seq: t.sent as u16, status: own_status() }), false),
    ];
    for (bytes, reliable) in packets {
        if net::send(me, &bytes, reliable) {
            t.sent += 1;
        }
    }
    if t.summary.is_none_or(|s| s.elapsed() >= Duration::from_secs(10)) {
        t.summary = Some(Instant::now());
        log(format!("coop test: loopback to {me:x}: {} sent, {} received", t.sent, t.got));
    }
}

/// In a session with someone.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// In someone else's world: the host owns the enemies (no grabs, no moving them).
pub fn guest() -> bool {
    GUEST.load(Ordering::Relaxed)
}

/// The session's players: (us, other players, the host's id, whether we're the host if the
/// lobby says so).
fn session() -> Option<(u64, Vec<u64>, u64, Option<bool>)> {
    let sm = unsafe { CSSessionManager::instance() }.ok()?;
    let lobby = match sm.lobby_state {
        LobbyState::Host => Some(true),
        LobbyState::Client => Some(false),
        _ => None,
    };
    let mut me = 0;
    let mut host = sm.host_player.as_option().map_or(0, |h| h.steam_id);
    let mut others = Vec::new();
    for p in sm.players.iter() {
        let id = p.base.steam_id;
        if id == 0 {
            continue;
        }
        if p.is_local_player {
            me = id;
        } else if !others.contains(&id) {
            others.push(id);
        }
        if p.is_host {
            host = id;
        }
    }
    (!others.is_empty()).then_some((me, others, host, lobby))
}

/// The Steam id a partner's character is played from.
fn remote_identity(p: &PlayerIns) -> Option<u64> {
    // PlayerSessionHolder +0x18 PlayerNetworkSession, its +0x8 the Steam id
    let holder = &p.player_session_holder as *const _ as usize;
    let session = explore::read_u64(holder + 0x18)? as usize;
    explore::read_u64(session + 8).filter(|&id| id != 0)
}

/// The other players' characters: (Steam id, PlayerIns address).
fn partner_chrs(peers: &[u64], unpaired: &mut bool) -> Vec<(u64, usize)> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
    let main = wcm.main_player.as_ref().map(|p| p.as_ptr() as usize);
    let others: Vec<&mut PlayerIns> = wcm.player_chr_set.characters().filter(|p| Some(*p as *const PlayerIns as usize) != main).collect();
    let mut out: Vec<(u64, usize)> = others
        .iter()
        .filter_map(|p| remote_identity(p).filter(|id| peers.contains(id)).map(|id| (id, &**p as *const PlayerIns as usize)))
        .collect();
    // (the Steam id not where it's expected: with one partner and one other character, that's them)
    let lone = out.is_empty() && peers.len() == 1 && others.len() == 1;
    if lone {
        out.push((peers[0], &*others[0] as *const PlayerIns as usize));
    }
    if lone != *unpaired {
        *unpaired = lone;
        if lone {
            log("coop: partner's character paired by elimination (no Steam id on it)");
        }
    }
    out
}

fn refresh(s: &mut State) {
    let (me, peers, host_id, lobby) = session().unwrap_or_default();
    // (the lobby's word first; else by the ids; with no word at all we're the host: as in single
    // player, the enemies are ours)
    let host = peers.is_empty() || lobby.unwrap_or(host_id == 0 || host_id == me);
    if peers != s.peers || host != s.host || host_id != s.host_id {
        log(format!(
            "coop: session {:x?}, host {:x} ({}), me {me:x}",
            peers,
            host_id,
            if peers.is_empty() { "none" } else if host { "us" } else { "them" }
        ));
        s.remotes.retain(|r| peers.contains(&r.steam));
        s.mates.retain(|m| peers.contains(&m.steam));
        s.peers = peers;
        s.host = host;
        s.host_id = host_id;
    }
    // (who's who changes rarely: looked up with the session, checked for life where used)
    s.chrs = if s.peers.is_empty() { Vec::new() } else { partner_chrs(&s.peers, &mut s.unpaired) };
    ACTIVE.store(!s.peers.is_empty(), Ordering::Relaxed);
    GUEST.store((!s.peers.is_empty() && !s.host) || test_mode("guest"), Ordering::Relaxed);
}

/// The packets that came in (sender, bytes), into the state.
fn take_in(s: &mut State, batch: Vec<(u64, Vec<u8>)>) {
    for (from, data) in batch {
        if !s.peers.contains(&from) {
            continue;
        }
        let Some(p) = decode(&data) else {
            // (our version but a kind we don't know: a later build's, fine)
            if data.len() > 3 && data[..3] == MAGIC && data[3] != VERSION && !s.old_version {
                s.old_version = true;
                log(format!("coop: packets of version {} from {from:x} (ours {VERSION}): both need the same ER Mario", data[3]));
            }
            continue;
        };
        match p {
            Packet::Pose { seq, health, luigi, parts, ms, place, fx } => {
                let r = match s.remotes.iter().position(|r| r.steam == from) {
                    Some(i) => &mut s.remotes[i],
                    None => {
                        log(format!(
                            "coop: pose from {from:x} ({}, {} wedges, {}): partner HUD on",
                            if luigi { "Luigi" } else { "Mario" },
                            health >> 8,
                            if place.is_some() { "with its place" } else { "no place: shown where the game has them" }
                        ));
                        s.remotes.push(Remote::new(from, seq.wrapping_sub(1), health, luigi));
                        s.remotes.last_mut().expect("just pushed")
                    }
                };
                // (one seen already: dropped; an older one, out of order, still goes in its place)
                if seq != r.seq && seq.wrapping_sub(r.seq).min(r.seq.wrapping_sub(seq)) < 1000 {
                    if seq.wrapping_sub(r.seq) < u32::MAX / 2 {
                        r.seq = seq;
                        r.health = health;
                        r.luigi = luigi;
                    }
                    r.add(ms.map(|m| m as f64 / 1000.0), *parts, place, fx);
                }
            }
            Packet::Sounds(ids) => {
                for id in ids {
                    if s.sounds_in.len() < SOUNDS_MAX * 2 {
                        s.sounds_in.push((from, id));
                    }
                }
            }
            Packet::Hit(h) if s.host => {
                if s.hits.len() < 64 {
                    s.hits.push((Instant::now(), h));
                }
            }
            Packet::Hit(_) | Packet::Hello => {}
            Packet::Shove { dir } => s.shove = Some(Vec3::new(dir[0], 0.0, dir[1])),
            Packet::Status { seq, status } => mate_status(s, from, seq, status),
        }
    }
    // (a partner's status stops while they load, after a death too: kept a while as down)
    s.mates.retain(|m| {
        let age = m.at.elapsed().as_secs_f32();
        age < MATE_STALE || (m.down && age < MATE_DOWN_STALE)
    });
    // (they stopped sending: out of Mario mode, gone)
    s.remotes.retain(|r| {
        let fresh = r.at.elapsed().as_secs_f32() < POSE_STALE;
        if !fresh {
            log(format!("coop: no pose from {:x} lately", r.steam));
        }
        fresh
    });
}

/// Game thread, every frame: the session, the partners' packets, our pose out. (Steam is called
/// with the state unlocked: the pose tasks wait on it.)
pub fn frame() {
    let peers = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let s = guard.get_or_insert_with(State::default);
        if s.checked.is_none_or(|t| t.elapsed() >= SESSION_EVERY) {
            s.checked = Some(Instant::now());
            refresh(s);
        }
        s.peers.clone()
    };
    if peers.is_empty() {
        if test_mode("loopback") {
            loopback();
        }
        return;
    }
    let mut batch = Vec::new();
    net::receive(|from, data| {
        if batch.len() < 256 {
            batch.push((from, data.to_vec()));
        }
    });
    let pose = *crate::engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner());
    let status = own_status();
    let sounds: Vec<u32> = OUT_SOUNDS.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect();
    let out = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        take_in(s, batch);
        let mut out = Vec::new();
        if let Some(parts) = pose.filter(|_| s.sent.is_none_or(|t| t.elapsed() + POSE_SLACK >= POSE_EVERY)) {
            s.sent = Some(Instant::now());
            s.seq = s.seq.wrapping_add(1);
            out.push(encode_pose(s.seq, parts));
        }
        if !sounds.is_empty() && pose.is_some() {
            out.push(encode(&Packet::Sounds(sounds)));
        }
        // the status: a few times a second, sooner when something changed (a blow landed since)
        let due = match &s.status_sent {
            None => true,
            Some((last, t)) => {
                let e = t.elapsed();
                let hit_since = status.hit.is_some_and(|(_, ago)| (ago as f32 / 1000.0) < e.as_secs_f32());
                e >= STATUS_EVERY || (e >= STATUS_MIN && (hit_since || status_changed(last, &status)))
            }
        };
        // (only in Mario mode: out of it there's no Mario to show)
        if due && pose.is_some() {
            s.status_sent = Some((status, Instant::now()));
            s.status_seq = s.status_seq.wrapping_add(1);
            out.push(encode(&Packet::Status { seq: s.status_seq, status }));
        }
        if s.hello.is_none_or(|t| t.elapsed() >= HELLO_EVERY) {
            s.hello = Some(Instant::now());
            out.push(encode(&Packet::Hello));
        }
        out
    };
    for bytes in out {
        for &peer in &peers {
            net::send(peer, &bytes, false);
        }
    }
    play_partner_sounds();
    if CHIME.swap(false, Ordering::Relaxed) {
        crate::worker::call("bros chime", |_| unsafe { crate::sm64::sm64_play_sound_volume(SOUND_BROS, 0.09) });
    }
}

/// Every pose task: each partner's character in their Mario pose.
pub fn apply_remote() {
    if !active() {
        return;
    }
    // (only while the partner row has the other character's model: out of Mario mode their
    // Tarnished is left alone)
    if !crate::equip::partner_row_on() {
        unfade(&[]);
        return;
    }
    let poses: Vec<(u64, usize, [PartPose; PARTS], Option<Place>, bool)> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_ref() else { return };
        s.remotes
            .iter()
            .filter_map(|r| {
                let vanish = s.mates.iter().any(|m| m.steam == r.steam && m.st.power == POWER_VANISH);
                s.chrs.iter().find(|c| c.0 == r.steam).map(|c| {
                    let (parts, place) = r.now();
                    (r.steam, c.1, parts, place, vanish)
                })
            })
            .collect()
    };
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    // (only characters still there: one may have left since the session was looked at)
    let live: Vec<usize> = wcm.player_chr_set.characters().map(|p| p as *const PlayerIns as usize).collect();
    let mut vanished = Vec::new();
    for (steam, chr, parts, place, vanish) in poses.iter().filter(|p| live.contains(&p.1)) {
        let player = unsafe { &mut *(*chr as *mut PlayerIns) };
        let (pose, ..) = placed(*steam, player, parts, *place);
        // the Vanish cap: see-through, as they see themselves
        if *vanish {
            let c = &mut player.chr_ins;
            c.opacity_keyframes_multiplier = crate::caps::VANISH_OPACITY;
            c.opacity_keyframes_multiplier_previous = crate::caps::VANISH_OPACITY;
            vanished.push(*chr);
        }
        let ins = &player.chr_ins as *const _ as usize;
        crate::engine_mario::apply_remote(ins, &pose);
    }
    unfade(&vanished);
}

/// The partners made see-through by their Vanish cap (PlayerIns addresses).
static FADED: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Every partner see-through before and not in `now` back to full (the cap off, their pose
/// stopped, out of Mario mode: the game never puts it back itself), if still there.
fn unfade(now: &[usize]) {
    let mut faded = FADED.lock().unwrap_or_else(|e| e.into_inner());
    if faded.is_empty() && now.is_empty() {
        return;
    }
    let live: Vec<usize> = unsafe { WorldChrMan::instance() }.map(|w| w.player_chr_set.characters().map(|p| p as *const PlayerIns as usize).collect()).unwrap_or_default();
    for &chr in faded.iter().filter(|c| !now.contains(c) && live.contains(c)) {
        let c = unsafe { &mut (*(chr as *mut PlayerIns)).chr_ins };
        c.opacity_keyframes_multiplier = 1.0;
        c.opacity_keyframes_multiplier_previous = 1.0;
    }
    *faded = now.to_vec();
}

/// Our Mario's health (SM64's), for the partner's HUD (lib.rs sets it every tick).
static OWN_HEALTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x880);

pub fn set_own_health(health: i16) {
    let now = health.max(0) as u32;
    let was = OWN_HEALTH.swap(now, Ordering::Relaxed);
    // (a wedge less: hurt, for the status)
    if now >> 8 < was >> 8 && active() {
        *OWN_HURT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }
}

static OWN_HURT: Mutex<Option<Instant>> = Mutex::new(None);
/// what the player is using (a POWER_), whether he's in a fight: lib.rs sets them
static POWER_NOW: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(POWER_NONE);
static FIGHTING_NOW: AtomicBool = AtomicBool::new(false);

pub fn set_power(power: u8) {
    POWER_NOW.store(power, Ordering::Relaxed);
}

pub fn set_fighting(fighting: bool) {
    FIGHTING_NOW.store(fighting, Ordering::Relaxed);
}

/// how often the status goes out, and at most how often when it changes
const STATUS_EVERY: Duration = Duration::from_millis(250);
const STATUS_MIN: Duration = Duration::from_millis(100);
/// a partner whose status stopped this long ago is let go (s); one down is kept longer (the
/// loading screen after their death)
const MATE_STALE: f32 = 3.0;
const MATE_DOWN_STALE: f32 = 60.0;
/// "LUIGI!" (low health) at most this often (s)
const LOW_EVERY: f32 = 8.0;
/// the BROS!: both players' blows on one enemy this close in time (s); the same enemy again only
/// this much later (s); a streak of them lasts this long after the last (s); style points each
const BROS_WINDOW: f32 = 1.5;
const BROS_EVERY: f32 = 0.8;
pub const BROS_KEEP: f32 = 6.0;
const BROS_POINTS: f32 = 1.5;
/// the partner's card: up while there's a reason and this long after (s), in and out this fast (s)
const CARD_LINGER: f32 = 5.0;
const CARD_IN: f32 = 0.25;
const CARD_OUT: f32 = 0.8;
/// ...a reason: hurt this recently (s), this far off (m), this few wedges
const CARD_HURT: f32 = 4.0;
const CARD_FAR: f32 = 30.0;
const LOW_WEDGES: u8 = 2;
/// a red coin's chime, soft (the BROS!)
const SOUND_BROS: i32 = 0x7828_9081;

/// Our last blows (the enemy's handle key, where it was, when), for the BROS!
struct Mine {
    hits: [Option<(u64, Vec3, Instant)>; 8],
    next: usize,
}

static MINE: Mutex<Mine> = Mutex::new(Mine { hits: [None; 8], next: 0 });

/// Our latest blow on `key`: where, when.
fn own_hit_on(key: u64) -> Option<(Vec3, Instant)> {
    let m = MINE.lock().unwrap_or_else(|e| e.into_inner());
    m.hits.iter().flatten().filter(|h| h.0 == key).max_by_key(|h| h.2).map(|h| (h.1, h.2))
}

fn last_own_hit() -> Option<(u64, Instant)> {
    let m = MINE.lock().unwrap_or_else(|e| e.into_inner());
    m.hits.iter().flatten().max_by_key(|h| h.2).map(|h| (h.0, h.2))
}

/// Mario's blow landed on an enemy (combat.rs; `at` where it is, Elden Ring): a BROS! if the
/// partner hit it a moment ago too.
pub fn own_hit(key: u64, at: Vec3) {
    if !active() {
        return;
    }
    let now = Instant::now();
    {
        let mut m = MINE.lock().unwrap_or_else(|e| e.into_inner());
        let i = m.next;
        m.hits[i] = Some((key, at, now));
        m.next = (i + 1) % m.hits.len();
    }
    let theirs: Vec<Instant> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map_or(Vec::new(), |s| s.mates.iter().filter_map(|m| m.hit.filter(|h| h.0 == key).map(|h| h.1)).collect())
    };
    for t in theirs {
        bros(key, at, now, t);
    }
}

/// The streak of BROS! (blows by both on one enemy).
struct Bros {
    key: u64,
    at: Option<Instant>,
    /// the two blows the last one was made of (each blow counts once: one of theirs and a string
    /// of ours is one BROS!, not one per blow)
    used: Option<(Instant, Instant)>,
    streak: u32,
    streak_at: Option<Instant>,
}

impl Bros {
    /// Our blow at `a` and theirs at `b` on `key`, seen at `now`: the streak's count if it's a
    /// BROS! (close enough in time, and not the same enemy's again too soon).
    fn add(&mut self, key: u64, a: Instant, b: Instant, now: Instant) -> Option<u32> {
        let gap = if a > b { a - b } else { b - a };
        if gap.as_secs_f32() > BROS_WINDOW {
            return None;
        }
        if self.key == key && self.at.is_some_and(|t| (now - t).as_secs_f32() < BROS_EVERY) {
            return None;
        }
        if self.used.is_some_and(|(x, y)| x == a || y == b) {
            return None;
        }
        self.key = key;
        self.at = Some(now);
        self.used = Some((a, b));
        self.streak = if self.streak_at.is_some_and(|t| (now - t).as_secs_f32() < BROS_KEEP) { self.streak + 1 } else { 1 };
        self.streak_at = Some(now);
        Some(self.streak)
    }
}

/// a BROS! chime to play
static CHIME: AtomicBool = AtomicBool::new(false);

static BROS: Mutex<Bros> = Mutex::new(Bros { key: 0, at: None, used: None, streak: 0, streak_at: None });

/// Both hit `key` (at `pos`), we at `a`, they at `b`: the BROS! over it (the first of a streak and
/// every fifth: the rest add to the count and the style quietly).
fn bros(key: u64, pos: Vec3, a: Instant, b: Instant) {
    let Some(n) = BROS.lock().unwrap_or_else(|e| e.into_inner()).add(key, a, b, Instant::now()) else { return };
    if n == 1 || n % 5 == 0 {
        if n == 1 {
            log("coop: BROS! (both on one enemy)");
        }
        crate::style::callout("BROS!", Some(pos), [0.45, 1.0, 0.5], true, BROS_POINTS);
        // (played by frame(), out of the state's lock: the call waits for the SM64 thread)
        CHIME.store(true, Ordering::Relaxed);
    } else {
        crate::style::team(BROS_POINTS);
    }
}

/// For the HUD: the streak of BROS! going (2 or more), and how long since the last (s).
pub fn bros_streak() -> Option<(u32, f32)> {
    let b = BROS.lock().unwrap_or_else(|e| e.into_inner());
    let age = b.streak_at?.elapsed().as_secs_f32();
    (b.streak >= 2 && age < BROS_KEEP && active()).then_some((b.streak, age))
}

/// Our status, for the partner.
fn own_status() -> Status {
    let show = crate::style::show();
    let health = own_health();
    let mut flags = 0;
    if FIGHTING_NOW.load(Ordering::Relaxed) || show.is_some() {
        flags |= FIGHTING;
    }
    if OWN_HURT.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < 2.0) {
        flags |= HURT;
    }
    if health >> 8 == 0 {
        flags |= DOWN;
    }
    let hit = last_own_hit().filter(|h| h.1.elapsed().as_secs_f32() < BROS_WINDOW + 0.5).map(|(k, t)| (k, t.elapsed().as_millis().min(60_000) as u16));
    let bag = crate::coins::bag_chunk().map(|(block, chunk, n)| (n.clamp(1, u16::MAX as u32) as u16, block, chunk.to_array()));
    Status {
        health,
        luigi: crate::equip::luigi(),
        coins: crate::stats::get().coins.min(u16::MAX as u32) as u16,
        power: POWER_NOW.load(Ordering::Relaxed),
        weapon: crate::weapons::current(),
        rank: show.as_ref().and_then(|s| s.rank).map(|r| r as u8),
        hits: show.map_or(0, |s| s.count.min(u16::MAX as u32) as u16),
        flags,
        hit,
        bag,
    }
}

/// Whether a status says something new (the blow's age aside).
fn status_changed(a: &Status, b: &Status) -> bool {
    let (mut a, mut b) = (*a, *b);
    let keys = (a.hit.map(|h| h.0), b.hit.map(|h| h.0));
    a.hit = None;
    b.hit = None;
    a != b || keys.0 != keys.1
}

/// A partner's status came in: their card, the warnings ("LUIGI!", down, back), the BROS!
fn mate_status(s: &mut State, from: u64, seq: u16, st: Status) {
    let now = Instant::now();
    let i = match s.mates.iter().position(|m| m.steam == from) {
        // (an old one, out of order: dropped)
        // (unless nothing came for a while: they restarted, their count with it)
        Some(i) if (seq.wrapping_sub(s.mates[i].seq) >= u16::MAX / 2 || seq == s.mates[i].seq) && s.mates[i].at.elapsed().as_secs_f32() < 1.0 => return,
        Some(i) => i,
        None => {
            log(format!("coop: status from {from:x} ({}, {} coins): partner card on", if st.luigi { "Luigi" } else { "Mario" }, st.coins));
            s.mates.push(Mate { steam: from, st, seq, at: now, hit: None, hurt_at: None, low_said: None, down: false, alpha: 0.0, wanted: None });
            s.mates.len() - 1
        }
    };
    let m = &mut s.mates[i];
    let (was, wedges) = ((m.st.health >> 8) as u8, (st.health >> 8) as u8);
    if wedges < was || (st.flags & HURT != 0 && m.st.flags & HURT == 0) {
        m.hurt_at = Some(now);
    }
    let (low, down_text, back) = if st.luigi { ("LUIGI!", "LUIGI DOWN", "LUIGI IS BACK") } else { ("MARIO!", "MARIO DOWN", "MARIO IS BACK") };
    let down = st.flags & DOWN != 0 || wedges == 0;
    if down && !m.down {
        log(format!("coop: partner {from:x} is down"));
        crate::style::callout(down_text, None, [0.85, 0.35, 0.3], true, 0.0);
    } else if !down && m.down {
        log(format!("coop: partner {from:x} is back"));
        crate::style::callout(back, None, [0.45, 1.0, 0.5], false, 0.0);
    } else if !down && wedges <= LOW_WEDGES && was > LOW_WEDGES && m.low_said.is_none_or(|t| t.elapsed().as_secs_f32() >= LOW_EVERY) {
        m.low_said = Some(now);
        crate::style::callout(low, None, [1.0, 0.35, 0.3], false, 0.0);
    }
    m.down = down;
    m.st = st;
    m.seq = seq;
    m.at = now;
    // their last blow (on our clock): a BROS! if we hit it a moment ago too
    if let Some((key, ago)) = st.hit {
        let at = now.checked_sub(Duration::from_millis(ago as u64)).unwrap_or(now);
        // (the same blow comes in several statuses, its time a little off each: a new one only
        // well after)
        let fresh = m.hit.is_none_or(|(k, t)| k != key || at > t + Duration::from_millis(300));
        if fresh {
            m.hit = Some((key, at));
            if let Some((pos, mine)) = own_hit_on(key) {
                bros(key, pos, mine, at);
            }
        }
    }
}

/// The card's fade: towards shown (or not) at its pace.
fn card_step(alpha: f32, shown: bool, dt: f32) -> f32 {
    if shown { (alpha + dt / CARD_IN).min(1.0) } else { (alpha - dt / CARD_OUT).max(0.0) }
}

/// Game thread, every frame: the partners' cards for the HUD. `me`: our feet (Elden Ring), `dt`
/// the frame's time (s). Shown while something's going on (a fight, either hurt, them low, down
/// or far off), gone a while after it's quiet.
pub fn hud(me: Vec3, dt: f32) -> Vec<crate::hud::PartnerHud> {
    if !active() {
        return Vec::new();
    }
    let near = partners();
    update_partner_fx(&near);
    let me_fighting = FIGHTING_NOW.load(Ordering::Relaxed) || OWN_HURT.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < CARD_HURT);
    let now = Instant::now();
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = guard.as_mut() else { return Vec::new() };
    let mut out = Vec::with_capacity(s.mates.len());
    for m in s.mates.iter_mut() {
        let p = near.iter().find(|p| p.steam == m.steam);
        let wedges = (m.st.health >> 8).min(8) as u8;
        let hurt = m.hurt_at.map_or(f32::MAX, |t| t.elapsed().as_secs_f32());
        let far = p.is_some_and(|p| (p.feet - me).length() > CARD_FAR);
        if m.st.flags & FIGHTING != 0 || me_fighting || hurt < CARD_HURT || wedges <= LOW_WEDGES || m.down || far {
            m.wanted = Some(now);
        }
        let shown = m.wanted.is_some_and(|t| (now - t).as_secs_f32() < CARD_LINGER);
        m.alpha = card_step(m.alpha, shown, dt);
        let bag = m.st.bag.and_then(|(coins, block, chunk)| crate::coins::place(block, Vec3::from(chunk)).map(|at| (at, coins)));
        out.push(crate::hud::PartnerHud {
            head: p.map(|p| p.head),
            wedges,
            luigi: m.st.luigi,
            power: m.st.power,
            coins: m.st.coins,
            rank: m.st.rank,
            alpha: m.alpha,
            danger: !m.down && (wedges <= LOW_WEDGES || hurt < 2.0),
            down: m.down,
            bag,
            // (their wing cap's wings: their model has none, they're drawn over it)
            wings: p.filter(|_| m.st.power == POWER_WING).map(|p| (p.head_part, p.face, p.size)),
        });
    }
    out
}

fn own_health() -> u16 {
    OWN_HEALTH.load(Ordering::Relaxed).min(0x880) as u16
}

/// A partner as the HUD and the Bros. Jump see them.
#[derive(Clone, Copy)]
pub struct Partner {
    pub steam: u64,
    /// their feet and the top of their head (Elden Ring), where they're shown
    pub feet: Vec3,
    pub head: Vec3,
    pub luigi: bool,
    /// their head part (Elden Ring: its centre), which way they face, their size there
    pub head_part: Vec3,
    pub face: Quat,
    pub size: f32,
}

/// Game thread: the partners in Mario mode near us (their pose coming in), where they are.
pub fn partners() -> Vec<Partner> {
    if !active() {
        return Vec::new();
    }
    let list: Vec<(u64, usize, [PartPose; PARTS], Option<Place>, bool)> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_ref() else { return Vec::new() };
        s.remotes
            .iter()
            .filter_map(|r| {
                s.chrs.iter().find(|c| c.0 == r.steam).map(|c| {
                    let (parts, place) = r.now();
                    (r.steam, c.1, parts, place, r.luigi)
                })
            })
            .collect()
    };
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
    let live: Vec<usize> = wcm.player_chr_set.characters().map(|p| p as *const PlayerIns as usize).collect();
    list.into_iter()
        .filter(|(_, chr, ..)| live.contains(chr))
        .map(|(steam, chr, parts, place, luigi)| {
            let player = unsafe { &*(chr as *const PlayerIns) };
            let (_, feet, face) = placed(steam, player, &parts, place);
            // (the head part's centre, and a little over it: its top)
            let head = parts[3];
            let head_y = head.pos.y.clamp(0.4, 4.0) + 0.25 * head.scale.clamp(0.5, 3.0);
            Partner { steam, feet, head: feet + Vec3::Y * head_y, luigi, head_part: feet + face * head.pos, face, size: head.scale.clamp(0.3, 4.0) }
        })
        .collect()
}

/// Our Mario's body's sounds this tick (lib.rs, after his tick), to go out with the next frame.
static OUT_SOUNDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// The SM64 thread, each tick: the sounds our Mario's body made (only in a session).
pub fn own_sounds(ids: &[u32]) {
    if ids.is_empty() || !active() {
        return;
    }
    let mut out = OUT_SOUNDS.lock().unwrap_or_else(|e| e.into_inner());
    for &id in ids {
        if out.len() < SOUNDS_MAX {
            out.push(id);
        }
    }
}

/// How loud a partner's sound is `d` m from us: always under ours, softer the farther (0: not at all).
fn partner_volume(d: f32) -> f32 {
    if !d.is_finite() || d > PARTNER_FAR {
        return 0.0;
    }
    let v = PARTNER_LOUD / (1.0 + (d - PARTNER_NEAR).max(0.0) * PARTNER_FADE);
    // (and fading out to nothing towards PARTNER_FAR, no cut)
    v * (1.0 - (d / PARTNER_FAR).powi(2)).max(0.0)
}

/// Game thread: the partners' sounds that came in, played as far as they are from the camera.
fn play_partner_sounds() {
    let sounds = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_mut() {
            Some(s) if !s.sounds_in.is_empty() => std::mem::take(&mut s.sounds_in),
            _ => return,
        }
    };
    let near = partners();
    let ear = crate::lakitu::position().or_else(|| {
        let wcm = unsafe { WorldChrMan::instance() }.ok()?;
        let p = wcm.main_player.as_ref()?.chr_ins.modules.physics.position;
        Some(Vec3::new(p.0, p.1, p.2))
    });
    let Some(ear) = ear else { return };
    // (each partner's from a spot of their own; under SM64's quietest, not at all)
    let play: Vec<(u32, u32, f32)> = sounds
        .into_iter()
        .filter_map(|(from, id)| {
            let slot = near.iter().position(|p| p.steam == from)?;
            let v = partner_volume(near[slot].head_part.distance(ear));
            (v >= 0.05).then_some((id, slot as u32, v))
        })
        .collect();
    if !play.is_empty() {
        crate::worker::call("partner sounds", move |_| {
            for (id, slot, v) in play {
                unsafe { crate::sm64::sm64_er_play_partner(id as i32, slot, v) };
            }
        });
    }
}

/// The partners' effects around where they're shown (hud.rs draws them; set by `hud` on the game
/// thread, characters aren't read on the render thread).
static PARTNER_FX: Mutex<Vec<std::sync::Arc<crate::fx::WorldFx>>> = Mutex::new(Vec::new());

pub fn partner_fx() -> Vec<std::sync::Arc<crate::fx::WorldFx>> {
    if !active() {
        return Vec::new();
    }
    PARTNER_FX.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Game thread: each partner's latest effects, moved to around where they're shown.
fn update_partner_fx(near: &[Partner]) {
    let list: Vec<(u64, std::sync::Arc<crate::fx::WorldFx>)> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_ref() else { return };
        s.remotes.iter().filter_map(|r| r.fx().map(|f| (r.steam, f))).collect()
    };
    let shown: Vec<std::sync::Arc<crate::fx::WorldFx>> = list
        .into_iter()
        .filter_map(|(steam, fx)| near.iter().find(|p| p.steam == steam).map(|p| std::sync::Arc::new((*fx).clone().shifted(p.feet))))
        .collect();
    *PARTNER_FX.lock().unwrap_or_else(|e| e.into_inner()) = shown;
}

/// Players in the session (1 out of one).
pub fn players() -> usize {
    if !active() {
        return 1;
    }
    STATE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(1, |s| s.peers.len() + 1)
}

/// The enemies' extra health per extra player (%), regular and boss, that Mario's blows go by:
/// `coop_health_scaling` / `coop_boss_health_scaling` in er_mario.ini, else Seamless Co-op's own
/// defaults (35 / 100). Not read from Seamless's ersc_settings.ini: Reforged sets its scaling to 0
/// there, and with Mario's blows being shares of max HP, two Marios would melt everything twice as
/// fast.
fn health_scaling() -> (f32, f32) {
    static SCALING: std::sync::OnceLock<(f32, f32)> = std::sync::OnceLock::new();
    *SCALING.get_or_init(|| {
        let key = |k: &str, default: f32| crate::paths::config(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(default);
        let s = (key("coop_health_scaling", 35.0), key("coop_boss_health_scaling", 100.0));
        log(format!("coop: with two, enemies take blows as if {}% healthier per extra player (bosses {}%)", s.0, s.1));
        s
    })
}

/// What Mario's blows (shares of max HP) are worth with this many players (1 alone).
pub fn share_factor(boss: bool) -> f32 {
    let extra = players().saturating_sub(1) as f32;
    if extra == 0.0 {
        return 1.0;
    }
    let (regular, bosses) = health_scaling();
    1.0 / (1.0 + extra * if boss { bosses } else { regular } / 100.0).max(1.0)
}

/// Guest: a blow of ours on an enemy (its share of max HP, all bonuses in), for the host to take off.
pub fn send_hit(handle: &FieldInsHandle, attack: u8, pct: f32) {
    if !guest() {
        return;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle(handle) else { return };
    let p = chr.modules.physics.position;
    let bytes = encode(&Packet::Hit(Hit { handle: *handle, chr_id: chr.character_id, pos: [p.0, p.1, p.2], attack, pct }));
    // (the host's id unknown: everyone, only a host takes it)
    let to: Vec<u64> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_ref() else { return };
        if s.peers.contains(&s.host_id) { vec![s.host_id] } else { s.peers.clone() }
    };
    for peer in to {
        net::send(peer, &bytes, true);
    }
}

/// Host: the guests' blows since last time (not the ones that waited too long).
pub fn take_hits() -> Vec<Hit> {
    let hits = STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(|s| std::mem::take(&mut s.hits)).unwrap_or_default();
    hits.into_iter().filter(|(t, _)| t.elapsed().as_secs_f32() < HIT_STALE).map(|(_, h)| h).collect()
}

/// A partner pushed us: the way to go (ER axes, horizontal).
pub fn take_shove() -> Option<Vec3> {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut().and_then(|s| s.shove.take())
}

/// Game thread, each SM64 tick: Mario is punching or kicking at `me` (ER world): a partner within
/// reach is pushed away.
pub fn shove_partners(me: Vec3, attacking: bool) {
    if !attacking || !active() {
        return;
    }
    let chrs = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        if s.shoved.is_some_and(|t| t.elapsed() < SHOVE_EVERY) {
            return;
        }
        s.chrs.clone()
    };
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let live: Vec<usize> = wcm.player_chr_set.characters().map(|p| p as *const PlayerIns as usize).collect();
    let reach = SHOVE_REACH * crate::size_factor();
    let mut shoved = false;
    for &(steam, chr) in chrs.iter().filter(|c| live.contains(&c.1)) {
        let p = unsafe { &*(chr as *const PlayerIns) }.chr_ins.modules.physics.position;
        let off = Vec3::new(p.0 - me.x, 0.0, p.2 - me.z);
        if off.length() > reach || (p.1 - me.y).abs() > 1.5 {
            continue;
        }
        let dir = off.normalize_or(Vec3::Z);
        net::send(steam, &encode(&Packet::Shove { dir: [dir.x, dir.z] }), true);
        shoved = true;
        log(format!("coop: shoved partner {steam:x}"));
    }
    if shoved {
        if let Some(s) = STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            s.shoved = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose() -> [PartPose; PARTS] {
        let mut parts = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
        for (k, p) in parts.iter_mut().enumerate() {
            let f = k as f32;
            p.rot = Quat::from_euler(glam::EulerRot::YXZ, f * 0.3, -f * 0.1, f * 0.05);
            p.pos = Vec3::new(f * 0.05 - 0.7, f * 0.04, -f * 0.03);
            p.scale = if k % 7 == 0 { 0.01 } else { 1.0 + f * 0.02 };
            p.stretch = Vec3::new(1.0, 1.2, 0.8);
        }
        parts
    }

    #[test]
    fn pose_round_trip() {
        let a = pose();
        let place = Place { origin: 0x3C2C_2400, chunk: Vec3::new(-1234.5, 88.25, 4096.0), yaw: -2.5, local: false };
        let fx = crate::fx::WorldFx { fire: vec![crate::fx::Fire { pos: Vec3::new(1.0, 2.0, 3.0), size: 1.0, frame: 3, tint: [1.0; 4] }], ..Default::default() };
        let bytes = encode(&Packet::Pose { seq: 7, health: 0x580, luigi: true, parts: Box::new(a), ms: Some(123_456), place: Some(place), fx: Some(std::sync::Arc::new(fx.clone())) });
        assert!(bytes.len() > POSE_BYTES + POSE_EXT_BYTES);
        let Some(Packet::Pose { seq, health, luigi, parts, ms, place: got, fx: got_fx }) = decode(&bytes) else { panic!("no pose") };
        assert_eq!((seq, health, luigi, ms, got), (7, 0x580, true, Some(123_456), Some(place)));
        assert!(got_fx.is_some_and(|f| f.fire.len() == 1 && f.fire[0].pos.distance(fx.fire[0].pos) < 0.01));
        // in a dungeon: the block's own place
        let local = Place { local: true, ..place };
        let bytes = encode(&Packet::Pose { seq: 8, health: 0x580, luigi: true, parts: Box::new(a), ms: Some(1), place: Some(local), fx: None });
        let Some(Packet::Pose { place: got, fx, .. }) = decode(&bytes) else { panic!("no pose") };
        assert_eq!((got, fx.is_none()), (Some(local), true));
        // a boomerang 60 m off still fits
        let mut far = a;
        far[crate::engine_mario::RANG].pos = Vec3::new(60.0, -5.0, 0.0);
        let Some(Packet::Pose { parts: got, .. }) = decode(&encode(&Packet::Pose { seq: 9, health: 0, luigi: false, parts: Box::new(far), ms: None, place: None, fx: None })) else { panic!("no pose") };
        assert!(got[crate::engine_mario::RANG].pos.distance(Vec3::new(60.0, -5.0, 0.0)) < 0.002);
        for (x, y) in a.iter().zip(parts.iter()) {
            assert!(x.rot.angle_between(y.rot) < 0.001);
            assert!(x.pos.distance(y.pos) < 0.001);
            assert!((x.scale - y.scale).abs() < 0.001);
            assert!(x.stretch.distance(y.stretch) < 0.002);
        }
    }

    #[test]
    fn hit_and_shove_round_trip() {
        let key = 0x0012_3456_0000_00ABu64;
        let handle = unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) };
        let bytes = encode(&Packet::Hit(Hit { handle, chr_id: 4750, pos: [1.0, -2.5, 300.25], attack: 203, pct: 0.65 }));
        let Some(Packet::Hit(h)) = decode(&bytes) else { panic!("no hit") };
        assert_eq!(crate::combat::handle_key(&h.handle), key);
        assert_eq!((h.chr_id, h.pos, h.attack, h.pct), (4750, [1.0, -2.5, 300.25], 203, 0.65));
        let Some(Packet::Shove { dir }) = decode(&encode(&Packet::Shove { dir: [0.6, -0.8] })) else { panic!("no shove") };
        assert_eq!(dir, [0.6, -0.8]);
    }

    #[test]
    fn status_round_trip() {
        let st = Status {
            health: 0x380,
            luigi: true,
            coins: 1234,
            power: POWER_MEGA,
            weapon: 2,
            rank: Some(4),
            hits: 37,
            flags: FIGHTING | HURT,
            hit: Some((0x0012_3456_0000_00AB, 420)),
            bag: Some((17, 0x3C2C_2400, [-1234.5, 88.25, 4096.0])),
        };
        let bytes = encode(&Packet::Status { seq: 65535, status: st });
        assert_eq!(bytes.len(), STATUS_BYTES);
        let Some(Packet::Status { seq, status }) = decode(&bytes) else { panic!("no status") };
        assert_eq!((seq, status), (65535, st));
        // nothing: no rank, no blow, no bag
        let quiet = Status { health: 0x880, ..Status::default() };
        let Some(Packet::Status { status, .. }) = decode(&encode(&Packet::Status { seq: 1, status: quiet })) else { panic!("no status") };
        assert_eq!(status, quiet);
    }

    #[test]
    fn later_builds_packets_are_read_or_skipped() {
        // fields added at the end: read as far as we know
        let st = Status { health: 0x480, coins: 9, ..Status::default() };
        let mut bytes = encode(&Packet::Status { seq: 3, status: st });
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        assert!(matches!(decode(&bytes), Some(Packet::Status { seq: 3, status }) if status == st));
        let mut shove = encode(&Packet::Shove { dir: [0.0, 1.0] });
        shove.push(9);
        assert!(matches!(decode(&shove), Some(Packet::Shove { .. })));
        // a kind we don't know (grabbing the partner, say): skipped, no harm
        let mut other = header(42, HEADER + 4);
        other.extend_from_slice(&[0; 4]);
        assert!(decode(&other).is_none());
        // too short: not read
        let bytes = encode(&Packet::Status { seq: 3, status: st });
        assert!(decode(&bytes[..STATUS_BYTES - 1]).is_none());
    }

    #[test]
    fn bros_needs_both_close_in_time() {
        let t0 = Instant::now();
        let at = |s: f32| t0 + Duration::from_secs_f32(s);
        let mut b = Bros { key: 0, at: None, used: None, streak: 0, streak_at: None };
        // too far apart
        assert_eq!(b.add(7, at(0.0), at(2.0), at(2.0)), None);
        // close: the first of a streak
        assert_eq!(b.add(7, at(3.0), at(3.4), at(3.4)), Some(1));
        // the same enemy again right away: not counted twice (both sides see the same pair)
        assert_eq!(b.add(7, at(3.5), at(3.4), at(3.5)), None);
        // our string on their one blow: that blow's been counted
        assert_eq!(b.add(7, at(4.3), at(3.4), at(4.3)), None);
        // a fresh blow of theirs, on this one or another one: the streak goes on
        assert_eq!(b.add(7, at(4.3), at(4.6), at(4.6)), Some(2));
        assert_eq!(b.add(9, at(5.5), at(5.6), at(5.6)), Some(3));
        // after a long quiet: a new streak
        assert_eq!(b.add(9, at(20.0), at(20.1), at(20.1)), Some(1));
    }

    #[test]
    fn card_fades_in_quick_and_out_slow() {
        let mut a = 0.0;
        for _ in 0..8 {
            a = card_step(a, true, 1.0 / 30.0);
        }
        assert!(a >= 0.99, "in {a}");
        for _ in 0..12 {
            a = card_step(a, false, 1.0 / 30.0);
        }
        assert!(a > 0.4 && a < 0.6, "half out {a}");
        for _ in 0..30 {
            a = card_step(a, false, 1.0 / 30.0);
        }
        assert_eq!(a, 0.0);
    }

    #[test]
    fn status_change_ignores_the_blows_age() {
        let a = Status { health: 0x880, hit: Some((5, 100)), ..Status::default() };
        assert!(!status_changed(&a, &Status { hit: Some((5, 300)), ..a }));
        assert!(status_changed(&a, &Status { hit: Some((6, 0)), ..a }));
        assert!(status_changed(&a, &Status { health: 0x780, ..a }));
    }

    #[test]
    fn sounds_round_trip() {
        let ids = vec![0x2404_8081, 0x0442_A081, 7];
        let Some(Packet::Sounds(got)) = decode(&encode(&Packet::Sounds(ids.clone()))) else { panic!("no sounds") };
        assert_eq!(got, ids);
        let mut short = encode(&Packet::Sounds(ids));
        short.pop();
        assert!(decode(&short).is_none());
    }

    #[test]
    fn partner_sounds_softer_and_fading() {
        // never as loud as ours, loudest close by, softer farther, nothing far off
        assert!(partner_volume(0.0) <= PARTNER_LOUD && partner_volume(0.0) > 0.5);
        assert!(partner_volume(5.0) < partner_volume(1.0));
        assert!(partner_volume(20.0) < partner_volume(5.0));
        assert!(partner_volume(20.0) > 0.05);
        assert_eq!(partner_volume(60.0), 0.0);
        assert_eq!(partner_volume(f32::NAN), 0.0);
    }

    #[test]
    fn poses_shown_between_the_two_around_now() {
        let mut s = std::collections::VecDeque::new();
        let pose = |x: f32| [PartPose { rot: Quat::IDENTITY, pos: Vec3::new(x, 1.0, 0.0), scale: 1.0, stretch: Vec3::ONE }; PARTS];
        let at = |x: f32, yaw: f32| Some(Place { origin: 5, chunk: Vec3::new(x, 0.0, 0.0), yaw, local: false });
        s.push_back(Sample { t: 1.0, parts: pose(0.0), place: at(0.0, 3.0), fx: None });
        s.push_back(Sample { t: 1.1, parts: pose(0.1), place: at(1.0, -3.0), fx: None });
        // halfway: halfway there, the yaw the short way round (across +-pi)
        let (parts, place) = sample_at(&s, 1.05);
        assert!((parts[0].pos.x - 0.05).abs() < 1e-4);
        let p = place.expect("a place");
        assert!((p.chunk.x - 0.5).abs() < 1e-4);
        assert!(p.yaw.abs() > 3.0, "yaw {}", p.yaw);
        // before them: the first; after: the last, carried on a little (at most EXTRAPOLATE)
        assert_eq!(sample_at(&s, 0.5).1.expect("first").chunk.x, 0.0);
        let ahead = sample_at(&s, 5.0).1.expect("last").chunk.x;
        assert!((ahead - (1.0 + 10.0 * EXTRAPOLATE as f32)).abs() < 1e-3, "ahead {ahead}");
    }

    #[test]
    fn uneven_packets_still_play_smoothly() {
        // their clock 0.033 s apart, arriving bunched and late: shown from their own times
        let mut r = Remote::new(1, 0, 0x880, false);
        let pose = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
        for k in 0..6 {
            r.add(Some(100.0 + k as f64 * 0.033), pose, None, None);
        }
        assert_eq!(r.samples.len(), 6);
        assert!(r.samples.iter().zip(r.samples.iter().skip(1)).all(|(a, b)| a.t < b.t));
        // one out of order goes in its place
        r.add(Some(100.05), pose, None, None);
        assert!(r.samples.iter().zip(r.samples.iter().skip(1)).all(|(a, b)| a.t <= b.t));
        // they restarted (their clock jumped back): started over
        r.add(Some(1.0), pose, None, None);
        assert_eq!(r.samples.len(), 1);
    }

    #[test]
    fn alone_blows_are_worth_the_same() {
        // out of a session: single player, nothing scaled
        assert_eq!(share_factor(false), 1.0);
        assert_eq!(share_factor(true), 1.0);
    }

    #[test]
    fn rejects_foreign_or_broken() {
        let mut bytes = encode(&Packet::Shove { dir: [1.0, 0.0] });
        assert!(decode(&bytes[..bytes.len() - 1]).is_none());
        bytes[3] = VERSION + 1;
        assert!(decode(&bytes).is_none());
        assert!(decode(b"hello world").is_none());
        assert!(decode(&[]).is_none());
    }
}
