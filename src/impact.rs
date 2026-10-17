//! A blow that lands: Mario holds still for a moment (hit-stop), SM64's hit sound, and a little
//! star burst where it struck (drawn by hud.rs). A blow Mario takes: the same, in red on him, and
//! a red flash at the screen's edges.
//!
//! The combos (moves.rs) add their own: shockwave rings on the ground, stars and coins flying off,
//! the hammer's swing leaving a ribbon, and a puff with sparkles when it comes out. Few at a time:
//! the rings and the stars and coins are capped, the rest fade within a third of a second.

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

use crate::combat::Attack;
use crate::{sm64, worker};

// SM64's sounds
const SOUND_ACTION_HIT: i32 = 0x0444_C081;
const SOUND_HEAVY_LANDING: i32 = 0x0460_8081;
const SOUND_ACTION_HIT_3: i32 = 0x0444_A081;
const SOUND_GENERAL_COIN: i32 = 0x3811_8081;
const SOUND_GENERAL_POUND_WOOD_POST: i32 = 0x3065_C081;
const SOUND_GENERAL_SHORT_STAR: i32 = 0x3016_0091;
const SOUND_ACTION_SPIN: i32 = 0x0437_8081;
const SOUND_GENERAL_SWISH_AIR: i32 = 0x305C_0081;

/// how long a burst shows (s), and the red flash when Mario is hurt
pub const SPARK_TIME: f32 = 0.22;
pub const HURT_TIME: f32 = 0.4;
/// a hit Mario lands, one he takes
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const RED: [f32; 4] = [1.0, 0.25, 0.2, 1.0];

/// A star burst to draw: Elden Ring position, when it struck, size (m).
#[derive(Clone, Copy)]
pub struct Spark {
    pub pos: Vec3,
    pub born: Instant,
    pub size: f32,
    pub tint: [f32; 4],
}

static SPARKS: Mutex<Vec<Spark>> = Mutex::new(Vec::new());
static HURT: Mutex<Option<Instant>> = Mutex::new(None);
/// the dash's path (Mario's feet) and the puffs where each dash began
static DASH: Mutex<Vec<(Vec3, Instant)>> = Mutex::new(Vec::new());
static PUFFS: Mutex<Vec<(Vec3, Instant)>> = Mutex::new(Vec::new());
/// how long the dash's streaks and puffs last (s)
const TRAIL_TIME: f32 = 0.3;
const PUFF_TIME: f32 = 0.35;

fn spark(pos: Vec3, size: f32, tint: [f32; 4]) {
    let mut sparks = SPARKS.lock().unwrap_or_else(|e| e.into_inner());
    sparks.retain(|s| s.born.elapsed().as_secs_f32() < SPARK_TIME);
    sparks.push(Spark { pos, born: Instant::now(), size, tint });
}

/// Game thread: a blow landed at `at` (the target's chest) from Mario at `from`.
pub fn hit(at: Vec3, from: Vec3, attack: Attack) {
    // the super jump punch knocks coins out (Super Smash Bros.), the finishers stars
    let (coins, stars) = match attack {
        Attack::Uppercut => (2, 0),
        Attack::UppercutTop => (5, 3),
        Attack::FlurryEnd => (3, 5),
        _ if attack.finisher() => (0, 3),
        _ => (0, 0),
    };
    if coins > 0 {
        burst(at, coins, Bit::Coin);
    }
    if stars > 0 {
        burst(at, stars, Bit::Star);
    }
    let wood = matches!(attack, Attack::Hammer | Attack::HammerAir | Attack::Flurry | Attack::Giro);
    // (hit-stop in SM64 ticks, burst size, a heavy thud too)
    let (stop, size, heavy) = match attack {
        Attack::Uppercut | Attack::Tornado | Attack::Flurry | Attack::Shell => (1, 0.8, false),
        Attack::Meteor | Attack::HammerAir | Attack::Giro | Attack::Chain | Attack::AirHit => (2, 1.0, false),
        Attack::ChainEnd => (5, 1.4, true),
        Attack::Hammer => (3, 1.1, false),
        Attack::UppercutTop | Attack::Quake | Attack::Launch => (5, 1.4, true),
        Attack::TornadoEnd => (4, 1.3, true),
        Attack::HammerSmash | Attack::GiroEnd => (5, 1.5, true),
        Attack::MeteorLand => (6, 1.6, true),
        Attack::FlurryEnd => (7, 1.8, true),
        Attack::GroundPound => (4, 1.3, true),
        Attack::Shockwave => (3, 1.0, true),
        // (a short one: bouncing up a boss, it held him in mid-air)
        Attack::Stomp => (1, 0.9, false),
        Attack::Sweep | Attack::Spin => (2, 0.9, false),
        Attack::Dash => (2, 0.8, false),
        Attack::Punch | Attack::Kick => (2, 0.8, false),
        Attack::Rush => (2, 0.9, false),
        Attack::RushKick => (5, 1.4, true),
        // the boomerang from afar: no hold for Mario, a short tick of it for the launcher
        Attack::Rang | Attack::RangBig => (0, 0.7, false),
        Attack::RangUpper => (2, 1.1, true),
        // the Chain Chomp: light blows on many, a heavy crash
        Attack::Chomp | Attack::ChompAir => (2, 1.0, false),
        Attack::ChompSpin => (1, 0.8, false),
        Attack::ChompLaunch => (4, 1.3, true),
        Attack::ChompEnd => (5, 1.5, true),
        Attack::Fire(_) => return,
    };
    // Mega Mario: every blow lands like a heavy one, held longer, a bigger burst
    let mega = crate::mega::active();
    let (stop, size, heavy) = if mega { (stop + stop / 2 + 1, size * 1.6, true) } else { (stop, size, heavy) };
    // on the near side of the target, a little towards Mario
    spark(at + (from - at).normalize_or_zero() * 0.35, size, WHITE);
    if heavy {
        crate::lakitu::shake(if mega { size * 0.6 } else { size - 0.4 });
    }
    worker::call("impact", move |_| unsafe {
        sm64::sm64_er_hitstop(stop);
        sm64::sm64_play_sound_global(SOUND_ACTION_HIT);
        if heavy {
            sm64::sm64_play_sound_global(SOUND_HEAVY_LANDING);
        }
        if wood {
            sm64::sm64_play_sound_global(SOUND_GENERAL_POUND_WOOD_POST);
        }
        if coins > 0 {
            sm64::sm64_play_sound_global(SOUND_GENERAL_COIN);
        }
    });
}

/// A shockwave ring on the ground: centre, how wide it gets (m), when, how long (s), colour.
#[derive(Clone, Copy)]
pub struct Ring {
    pub pos: Vec3,
    pub radius: f32,
    pub born: Instant,
    pub life: f32,
    pub tint: [f32; 4],
}

/// What flies off a blow: a star or a coin.
#[derive(Clone, Copy, PartialEq)]
pub enum Bit {
    Star,
    Coin,
}

/// A star or a coin thrown from `pos` at `vel` (m/s), falling.
#[derive(Clone, Copy)]
pub struct Flying {
    pub bit: Bit,
    pub pos: Vec3,
    pub vel: Vec3,
    pub born: Instant,
    pub size: f32,
}

static RINGS: Mutex<Vec<Ring>> = Mutex::new(Vec::new());
static BITS: Mutex<Vec<Flying>> = Mutex::new(Vec::new());
/// the hammer's swing: its head and the middle of its handle, a few ticks back
static SWING: Mutex<Vec<(Vec3, Vec3, Instant)>> = Mutex::new(Vec::new());
const MAX_RINGS: usize = 8;
const MAX_BITS: usize = 40;
pub const BIT_TIME: f32 = 0.7;
pub const BIT_GRAVITY: f32 = 14.0;
const SWING_TIME: f32 = 0.16;
const GOLD: [f32; 4] = [1.0, 0.85, 0.35, 1.0];
const DUST: [f32; 4] = [0.9, 0.85, 0.75, 1.0];

fn ring(pos: Vec3, radius: f32, life: f32, tint: [f32; 4]) {
    let mut rings = RINGS.lock().unwrap_or_else(|e| e.into_inner());
    rings.retain(|r| r.born.elapsed().as_secs_f32() < r.life);
    if rings.len() >= MAX_RINGS {
        rings.remove(0);
    }
    rings.push(Ring { pos, radius, born: Instant::now(), life, tint });
}

/// `n` stars or coins flying off `at`, up and out all around.
fn burst(at: Vec3, n: usize, bit: Bit) {
    let mut bits = BITS.lock().unwrap_or_else(|e| e.into_inner());
    bits.retain(|b| b.born.elapsed().as_secs_f32() < BIT_TIME);
    // (a different turn each burst)
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let turn = nanos as f32 * 1e-9 * std::f32::consts::TAU;
    for k in 0..n {
        if bits.len() >= MAX_BITS {
            bits.remove(0);
        }
        let a = turn + k as f32 / n as f32 * std::f32::consts::TAU;
        let out = if bit == Bit::Coin { 2.2 } else { 3.5 };
        let vel = Vec3::new(a.cos() * out, if bit == Bit::Coin { 6.5 } else { 4.0 } + (k % 2) as f32, a.sin() * out);
        bits.push(Flying { bit, pos: at, vel, born: Instant::now(), size: if bit == Bit::Coin { 0.35 } else { 0.4 } });
    }
}

fn puffs(at: Vec3, n: usize, spread: f32) {
    let mut puffs = PUFFS.lock().unwrap_or_else(|e| e.into_inner());
    puffs.retain(|(_, t)| t.elapsed().as_secs_f32() < PUFF_TIME);
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        puffs.push((at + Vec3::new(a.cos() * spread, 0.15, a.sin() * spread), Instant::now()));
    }
}

/// Game thread: what a combo's move shows this tick (moves::FX_*): around Mario's feet `feet`, or
/// where the hammer's head is `head` (Elden Ring positions).
pub fn combo(fx: u32, feet: Vec3, head: Option<Vec3>) {
    use crate::moves::*;
    // (a blow on the ground: where the head is, at the ground's height)
    let ground = head.map_or(feet, |h| Vec3::new(h.x, feet.y.max(h.y - 0.5).min(h.y), h.z));
    if fx & FX_GROUND != 0 {
        ring(ground + Vec3::Y * 0.05, 1.8, 0.35, DUST);
        puffs(ground, 3, 0.3);
    }
    if fx & FX_QUAKE != 0 {
        ring(ground + Vec3::Y * 0.05, 3.8, 0.5, DUST);
        ring(ground + Vec3::Y * 0.1, 2.2, 0.35, WHITE);
        puffs(ground, 6, 0.8);
        burst(ground + Vec3::Y * 0.3, 4, Bit::Star);
    }
    if fx & (FX_QUAKE | FX_METEOR) != 0 {
        crate::lakitu::shake(1.2);
    }
    if fx & FX_GIANT != 0 {
        crate::lakitu::shake(2.0);
        ring(ground + Vec3::Y * 0.05, 6.5, 0.7, DUST);
        ring(ground + Vec3::Y * 0.1, 4.0, 0.5, GOLD);
        puffs(ground, 8, 1.8);
        burst(ground + Vec3::Y * 0.6, 7, Bit::Star);
    }
    if fx & FX_METEOR != 0 {
        ring(feet + Vec3::Y * 0.05, 4.0, 0.5, GOLD);
        ring(feet + Vec3::Y * 0.1, 2.4, 0.35, WHITE);
        puffs(feet, 6, 0.9);
        burst(feet + Vec3::Y * 0.3, 5, Bit::Star);
    }
    if fx & FX_SPIN != 0 {
        ring(feet + Vec3::Y * 0.6, 2.2, 0.25, [0.9, 0.95, 1.0, 0.8]);
    }
    if fx & FX_LAUNCH != 0 {
        puffs(feet, 4, 0.35);
    }
    if fx & FX_BANG != 0 {
        puffs(ground, 1, 0.0);
        ring(ground + Vec3::Y * 0.05, 1.0, 0.2, DUST);
    }
    if fx & FX_CHARGE != 0 {
        // charging the slam: light gathering at the head, dust at his feet
        if let Some(h) = head {
            ring(h, 0.9, 0.22, GOLD);
            burst(h, 1, Bit::Star);
        }
        puffs(feet, 2, 0.45);
    }
    if fx & FX_GROW != 0 {
        if let Some(h) = head {
            burst(h, 4, Bit::Star);
        }
    }
}

/// Game thread: an enemy the launcher sent flying (juggle.rs): dust where it left from; hanging up
/// there for Mario, a twinkle now and then; caught by his blow up there, stars.
pub fn launched(feet: Vec3) {
    ring(feet + Vec3::Y * 0.05, 1.6, 0.35, DUST);
    puffs(feet, 4, 0.5);
}

pub fn waiting(at: Vec3) {
    burst(at, 1, Bit::Star);
}

pub fn caught(at: Vec3) {
    burst(at, 4, Bit::Star);
    ring(at, 1.4, 0.25, GOLD);
}

/// Game thread, every SM64 tick a hammer swing hits: its head and handle's middle (the ribbon).
pub fn swing_point(head: Vec3, mid: Vec3) {
    let mut swing = SWING.lock().unwrap_or_else(|e| e.into_inner());
    swing.retain(|(_, _, t)| t.elapsed().as_secs_f32() < SWING_TIME);
    swing.push((head, mid, Instant::now()));
}

/// A cap goes on or comes off (caps.rs): a puff all over Mario (it hides the model swap), a ring
/// in the cap's colour (silver, gold, blue; white back to his own), sparkles. `feet`: Elden Ring.
pub fn cap_change(feet: Vec3, cap: Option<u8>) {
    let tint = match cap {
        Some(crate::caps::METAL) => [0.8, 0.85, 0.95, 1.0],
        Some(crate::caps::WING) => GOLD,
        Some(_) => [0.45, 0.6, 1.0, 1.0],
        None => WHITE,
    };
    for up in [0.0, 0.45, 0.9] {
        puffs(feet + Vec3::Y * up, 4, 0.35);
    }
    ring(feet + Vec3::Y * 0.7, 1.6, 0.35, tint);
    ring(feet + Vec3::Y * 0.05, 2.2, 0.45, tint);
    burst(feet + Vec3::Y * 1.0, 5, Bit::Star);
    if cap.is_none() {
        worker::call("cap off", |_| unsafe { sm64::sm64_play_sound_global(SOUND_GENERAL_SWISH_AIR) });
    }
}

/// The Mega Mushroom (mega.rs): a burst of puffs all over him, golden rings, stars, as he grows
/// (`grow`) or shrinks back. `feet`: Elden Ring.
pub fn mega_change(feet: Vec3, grow: bool) {
    let k = if grow { crate::mega::SIZE * 0.75 } else { 1.8 };
    for up in [0.0, 0.8, 1.6, 2.6] {
        puffs(feet + Vec3::Y * up * k * 0.5, 5, 0.5 * k);
    }
    ring(feet + Vec3::Y * 0.05, 2.5 * k, 0.55, GOLD);
    ring(feet + Vec3::Y * 1.0 * k, 1.6 * k, 0.4, WHITE);
    burst(feet + Vec3::Y * 1.2 * k, 8, Bit::Star);
    flash([1.0, 0.95, 0.8], if grow { 0.22 } else { 0.12 }, 0.35);
}

/// Mega Mario's step (or landing, `heavy`): dust around his feet, a ring of it.
pub fn mega_step(feet: Vec3, heavy: bool) {
    let k = crate::mega::SIZE * 0.75;
    if heavy {
        ring(feet + Vec3::Y * 0.05, 3.0 * k, 0.5, DUST);
        puffs(feet, 8, 0.9 * k);
    } else {
        ring(feet + Vec3::Y * 0.05, 1.2 * k, 0.3, DUST);
        puffs(feet, 3, 0.45 * k);
    }
}

/// L1: the hammer pops out (or away) in Mario's hand: a puff, sparkles, SM64's sparkle and whoosh.
pub fn summon(hand: Vec3, out: bool) {
    puffs(hand - Vec3::Y * 0.15, 3, 0.2);
    if out {
        burst(hand, 4, Bit::Star);
    }
    worker::call("hammer", move |_| unsafe {
        sm64::sm64_play_sound_global(if out { SOUND_GENERAL_SHORT_STAR } else { SOUND_GENERAL_SWISH_AIR });
        sm64::sm64_play_sound_global(SOUND_ACTION_SPIN);
    });
}

/// The rings to draw, with how far along each is (0-1).
pub fn rings() -> Vec<(Ring, f32)> {
    let rings = RINGS.lock().unwrap_or_else(|e| e.into_inner());
    rings.iter().map(|r| (*r, r.born.elapsed().as_secs_f32() / r.life)).filter(|(_, t)| *t < 1.0).collect()
}

/// The stars and coins to draw: where each is now, its size, how far along (0-1), its age (s).
pub fn bits() -> Vec<(Bit, Vec3, f32, f32, f32)> {
    let bits = BITS.lock().unwrap_or_else(|e| e.into_inner());
    bits.iter()
        .filter_map(|b| {
            let age = b.born.elapsed().as_secs_f32();
            let pos = b.pos + b.vel * age - Vec3::Y * (0.5 * BIT_GRAVITY * age * age);
            (age < BIT_TIME).then_some((b.bit, pos, b.size, age / BIT_TIME, age))
        })
        .collect()
}

/// The hammer's ribbon, oldest first: head, handle's middle, how faded (0-1).
pub fn swing_trail() -> Vec<(Vec3, Vec3, f32)> {
    let swing = SWING.lock().unwrap_or_else(|e| e.into_inner());
    swing.iter().map(|(h, m, t)| (*h, *m, t.elapsed().as_secs_f32() / SWING_TIME)).filter(|(.., t)| *t < 1.0).collect()
}

/// Enemies flying through the air (juggle.rs): their streaks, by enemy (points and when).
static STREAKS: Mutex<Vec<(u64, Vec3, Instant)>> = Mutex::new(Vec::new());
const STREAK_TIME: f32 = 0.25;

/// Game thread: an enemy `key` flying fast is at `at` (its chest).
pub fn enemy_streak(key: u64, at: Vec3) {
    let mut s = STREAKS.lock().unwrap_or_else(|e| e.into_inner());
    s.retain(|(_, _, t)| t.elapsed().as_secs_f32() < STREAK_TIME);
    if s.len() < 120 {
        s.push((key, at, Instant::now()));
    }
}

/// The streaks to draw, each enemy's points oldest first, with how faded (0-1).
pub fn enemy_streaks() -> Vec<Vec<(Vec3, f32)>> {
    let s = STREAKS.lock().unwrap_or_else(|e| e.into_inner());
    let mut by: Vec<(u64, Vec<(Vec3, f32)>)> = Vec::new();
    for (k, p, t) in s.iter() {
        let f = t.elapsed().as_secs_f32() / STREAK_TIME;
        if f >= 1.0 {
            continue;
        }
        match by.iter_mut().find(|(key, _)| key == k) {
            Some((_, v)) => v.push((*p, f)),
            None => by.push((*k, vec![(*p, f)])),
        }
    }
    by.into_iter().map(|(_, v)| v).collect()
}

/// Game thread: an enemy came down hard after a juggle, at its feet `at`: dust, a ring, a thud.
pub fn thud(at: Vec3) {
    ring(at + Vec3::Y * 0.05, 2.2, 0.4, DUST);
    puffs(at, 5, 0.6);
    crate::lakitu::shake(0.5);
    worker::call("thud", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_HEAVY_LANDING, 0.6) });
}

/// The screen's flash: when, its colour, how strong at first (0-1), how long (s).
static FLASH: Mutex<Option<(Instant, [f32; 3], f32, f32)>> = Mutex::new(None);

/// A flash over the screen (a star finish's gold), fading.
pub fn flash(tint: [f32; 3], strength: f32, life: f32) {
    *FLASH.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), tint, strength, life));
}

/// The screen's flash now: its colour and strength (0 when there's none).
pub fn screen_flash() -> ([f32; 3], f32) {
    let f = *FLASH.lock().unwrap_or_else(|e| e.into_inner());
    f.map_or(([1.0; 3], 0.0), |(t, tint, s, life)| {
        let p = t.elapsed().as_secs_f32() / life;
        (tint, if p >= 1.0 { 0.0 } else { s * (1.0 - p) * (1.0 - p) })
    })
}

/// Game thread: a star finish (style.rs, a finisher from the S rank up) at `at`: a golden flash,
/// rings, stars and coins everywhere, more from each rank up.
pub fn star_finish(at: Vec3, rank: usize) {
    let more = rank.saturating_sub(4);
    flash([1.0, 0.85, 0.4], 0.28 + 0.06 * more as f32, 0.45);
    crate::lakitu::shake(1.4);
    let ground = at - Vec3::Y * 0.8;
    ring(ground, 5.0 + more as f32, 0.6, GOLD);
    ring(at, 2.6, 0.4, WHITE);
    ring(at, 1.6 + more as f32 * 0.5, 0.3, GOLD);
    burst(at, 6 + 2 * more, Bit::Star);
    burst(at, 4 + 2 * more, Bit::Coin);
    puffs(ground, 6, 1.0);
    worker::call("star finish", |_| unsafe {
        sm64::sm64_er_hitstop(4);
        sm64::sm64_play_sound_global(SOUND_GENERAL_COIN);
    });
}

/// Game thread: an enemy's blow hurt Mario, his chest at `at`.
pub fn hurt(at: Vec3) {
    spark(at, 1.1, RED);
    *HURT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    worker::call("hurt", |_| unsafe {
        sm64::sm64_er_hitstop(3);
        sm64::sm64_play_sound_global(SOUND_ACTION_HIT_3);
    });
}

/// The red flash at the screen's edges: how strong now (0-1).
pub fn hurt_flash() -> f32 {
    let t = HURT.lock().unwrap_or_else(|e| e.into_inner()).map_or(1.0, |t| t.elapsed().as_secs_f32() / HURT_TIME);
    (1.0 - t).max(0.0)
}

/// Game thread, every SM64 tick: Mario's feet while he dashes (`start` on its first tick).
pub fn dash_point(at: Vec3, start: bool) {
    let mut dash = DASH.lock().unwrap_or_else(|e| e.into_inner());
    dash.retain(|(_, t)| t.elapsed().as_secs_f32() < TRAIL_TIME);
    // (a new dash isn't joined to the last one)
    if start {
        dash.clear();
        let mut puffs = PUFFS.lock().unwrap_or_else(|e| e.into_inner());
        puffs.retain(|(_, t)| t.elapsed().as_secs_f32() < PUFF_TIME);
        for k in 0..3 {
            puffs.push((at + Vec3::Y * (0.15 + 0.2 * k as f32), Instant::now()));
        }
    }
    dash.push((at, Instant::now()));
}

/// The dash's path to draw, oldest first, with how faded each point is (0-1).
pub fn dash_trail() -> Vec<(Vec3, f32)> {
    let dash = DASH.lock().unwrap_or_else(|e| e.into_inner());
    dash.iter().map(|(p, t)| (*p, t.elapsed().as_secs_f32() / TRAIL_TIME)).filter(|(_, t)| *t < 1.0).collect()
}

/// The puffs to draw, with how far along each is (0-1).
pub fn dash_puffs() -> Vec<(Vec3, f32)> {
    let puffs = PUFFS.lock().unwrap_or_else(|e| e.into_inner());
    puffs.iter().map(|(p, t)| (*p, t.elapsed().as_secs_f32() / PUFF_TIME)).filter(|(_, t)| *t < 1.0).collect()
}

/// The bursts to draw now, with how far along each is (0-1).
pub fn sparks() -> Vec<(Spark, f32)> {
    let sparks = SPARKS.lock().unwrap_or_else(|e| e.into_inner());
    sparks
        .iter()
        .filter_map(|s| {
            let t = s.born.elapsed().as_secs_f32() / SPARK_TIME;
            (t < 1.0).then_some((*s, t))
        })
        .collect()
}

pub fn clear() {
    SPARKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    RINGS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    BITS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    SWING.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *HURT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *FLASH.lock().unwrap_or_else(|e| e.into_inner()) = None;
    STREAKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
