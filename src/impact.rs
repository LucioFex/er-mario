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
/// the light blows' hit sounds: one of these, louder or softer each time (one sound on every blow of
/// a string droned); the Chain Chomp's bites clank on its iron
const SOUND_ACTION_PAT_BACK: i32 = 0x043F_8081;
const SOUND_METAL_STEP: i32 = 0x042A_9081;
const SOUND_METAL_BONK: i32 = 0x0442_8081;

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
        Attack::Hammer => (4, 1.2, false),
        Attack::UppercutTop | Attack::Quake | Attack::Launch => (5, 1.4, true),
        Attack::TornadoEnd => (4, 1.3, true),
        Attack::HammerSmash | Attack::GiroEnd => (5, 1.5, true),
        Attack::MeteorLand => (6, 1.6, true),
        Attack::FlurryEnd => (7, 1.8, true),
        Attack::GroundPound => (4, 1.3, true),
        Attack::Shockwave => (3, 1.0, true),
        // (the giant fireball's burn and Mega's steps: no hold, they come again and again)
        Attack::FireSweep => (0, 0.7, false),
        Attack::Ember => (0, 0.5, false),
        Attack::Trample => (1, 0.9, false),
        // (a short one: bouncing up a boss, it held him in mid-air)
        Attack::Stomp => (1, 0.9, false),
        Attack::Sweep | Attack::Spin => (2, 0.9, false),
        Attack::Dash => (2, 0.8, false),
        Attack::Punch | Attack::Kick => (2, 0.8, false),
        Attack::Rush => (2, 0.9, false),
        Attack::RushKick => (5, 1.4, true),
        // the boomerang from afar: no hold for Mario, a short tick of it for the launcher
        Attack::Rang | Attack::RangBig => (0, 0.7, false),
        Attack::Hook => (1, 0.6, false),
        Attack::Tongue => (1, 0.7, false),
        Attack::Egg => (1, 0.9, false),
        Attack::YoshiStomp => (3, 1.0, true),
        Attack::RangUpper => (2, 1.1, true),
        // the Chain Chomp: light blows on many, a heavy crash
        Attack::Chomp | Attack::ChompAir => (2, 1.0, false),
        Attack::ChompSpin => (1, 0.8, false),
        Attack::ChompLaunch => (4, 1.3, true),
        Attack::ChompEnd => (5, 1.5, true),
        // timing: the Mikiri lands hard (its own flash: impact::mikiri), the dash strike a solid one
        Attack::Mikiri => (4, 1.3, true),
        Attack::Bonk => (3, 1.1, false),
        Attack::Fire(_) => return,
    };
    // Mega Mario: every blow lands like a heavy one, held longer, a bigger burst
    let mega = crate::mega::active();
    let (stop, size, heavy) = if mega { (stop + stop / 2 + 1, size * 1.6, true) } else { (stop, size, heavy) };
    // on the near side of the target, a little towards Mario
    spark(at + (from - at).normalize_or_zero() * 0.35, size, WHITE);
    if heavy {
        crate::lakitu::shake(if mega { size * 0.6 } else { size - 0.4 });
    } else if matches!(attack, Attack::Hammer | Attack::HammerAir) {
        // (the hammer's every blow has some weight)
        crate::lakitu::shake(0.35);
    }
    let chomp = matches!(attack, Attack::Chomp | Attack::ChompAir | Attack::ChompSpin | Attack::ChompLaunch | Attack::ChompEnd);
    let light = !heavy && !matches!(attack, Attack::Hammer);
    let (pick, loud) = (crate::rand01(), 0.6 + 0.4 * crate::rand01());
    worker::call("impact", move |_| unsafe {
        sm64::sm64_er_hitstop(stop);
        if light {
            // (SM64's hit, now and then with a softer thump under it)
            sm64::sm64_play_sound_volume(SOUND_ACTION_HIT, loud);
            if pick < 0.45 {
                sm64::sm64_play_sound_volume(SOUND_ACTION_PAT_BACK, loud * 0.6);
            }
        } else {
            sm64::sm64_play_sound_global(SOUND_ACTION_HIT);
        }
        if chomp {
            sm64::sm64_play_sound_volume(if pick < 0.5 { SOUND_METAL_STEP } else { SOUND_METAL_BONK }, 0.25 + 0.2 * loud);
        }
        if heavy {
            sm64::sm64_play_sound_global(SOUND_HEAVY_LANDING);
        }
        if wood {
            sm64::sm64_play_sound_global(SOUND_GENERAL_POUND_WOOD_POST);
        }
        if coins > 0 {
            sm64::sm64_play_sound_volume(SOUND_GENERAL_COIN, 0.45);
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

/// A Yoshi egg bursting (ride.rs): shell bits (puffs), a green ring, a star or two.
pub fn egg_burst(at: Vec3) {
    puffs(at, 4, 0.35);
    ring(at, 1.1, 0.25, [0.45, 0.95, 0.35, 0.9]);
    burst(at, 2, Bit::Star);
}

/// Mario's fireball from Yoshi's saddle going out (ride.rs): an orange ring, a puff, a twinkle.
pub fn fire_pop(at: Vec3) {
    ring(at, 0.9, 0.22, [1.0, 0.55, 0.15, 0.9]);
    puffs(at, 2, 0.3);
    burst(at, 1, Bit::Star);
}

/// Mario's ailments (status.rs): a puff of the poison's colour round him; blood loss's red burst;
/// frostbite's icy rings and a cold flash; a lick of flame off him while he burns.
pub fn status_puff(at: Vec3, tint: [f32; 3]) {
    let c = [tint[0], tint[1], tint[2], 0.8];
    ring(at, 0.5, 0.35, c);
    spark(at + Vec3::new(crate::rand01() - 0.5, crate::rand01() * 0.4, crate::rand01() - 0.5) * 0.5, 0.45, c);
}

pub fn bleed(at: Vec3) {
    spark(at, 1.4, RED);
    ring(at, 1.2, 0.3, RED);
    burst(at, 3, Bit::Star);
    flash([1.0, 0.1, 0.1], 0.3, 0.35);
}

pub fn frost(feet: Vec3) {
    const ICE: [f32; 4] = [0.6, 0.85, 1.0, 1.0];
    ring(feet + Vec3::Y * 0.1, 1.6, 0.6, ICE);
    ring(feet + Vec3::Y * 0.9, 1.0, 0.5, WHITE);
    spark(feet + Vec3::Y * 0.9, 1.2, ICE);
    flash([0.5, 0.75, 1.0], 0.3, 0.5);
}

pub fn flame_lick(at: Vec3) {
    spark(at, 0.5, [1.0, 0.5 + crate::rand01() * 0.3, 0.15, 0.9]);
}

/// Yoshi's stomp landing (ride.rs): a wide ring of dust, a green one, puffs all round.
pub fn yoshi_stomp(feet: Vec3) {
    ring(feet + Vec3::Y * 0.05, 4.5, 0.45, DUST);
    ring(feet + Vec3::Y * 0.3, 3.0, 0.35, [0.45, 0.95, 0.35, 0.9]);
    puffs(feet, 8, 1.6);
    burst(feet + Vec3::Y * 0.6, 4, Bit::Star);
}

/// The hook caught something (hook.rs): a little gold ring and a twinkle where it bit.
pub fn hook_latch(at: Vec3) {
    ring(at, 0.9, 0.25, GOLD);
    burst(at, 3, Bit::Star);
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

/// The giant fireball's explosion (fireball.rs), reaching `reach` m: rings of fire on the ground
/// and in the air, smoke, a hot flash.
pub fn fire_blast(at: Vec3, reach: f32) {
    const FIRE: [f32; 4] = [1.0, 0.5, 0.15, 1.0];
    ring(at + Vec3::Y * 0.1, reach, 0.6, FIRE);
    ring(at + Vec3::Y * 1.2, reach * 0.7, 0.45, GOLD);
    puffs(at, 10, reach * 0.4);
    flash([1.0, 0.55, 0.2], 0.35, 0.45);
}

/// The 20 s fireball's (the sun's) explosion, reaching `reach` m: rings of fire rolling out on the
/// ground one inside the other, a ring of gold up high, a wall of smoke, a white-hot flash.
pub fn sun_blast(at: Vec3, reach: f32) {
    const FIRE: [f32; 4] = [1.0, 0.5, 0.15, 1.0];
    const LAVA: [f32; 4] = [0.9, 0.2, 0.05, 1.0];
    ring(at + Vec3::Y * 0.1, reach, 0.9, FIRE);
    ring(at + Vec3::Y * 0.1, reach * 0.6, 0.7, LAVA);
    ring(at + Vec3::Y * 0.4, reach * 1.2, 1.1, LAVA);
    ring(at + Vec3::Y * 2.0, reach * 0.8, 0.6, GOLD);
    ring(at + Vec3::Y * 4.0, reach * 0.5, 0.5, WHITE);
    puffs(at, 16, reach * 0.45);
    puffs(at, 12, reach * 0.8);
    flash([1.0, 0.85, 0.6], 0.7, 0.8);
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

/// A perfect dodge (dodge.rs): a cold blue flash at the screen's edges, rings round Mario and
/// stars. `feet`: Elden Ring.
pub fn perfect_dodge(feet: Vec3) {
    flash([0.35, 0.6, 1.0], 0.55, 0.45);
    ring(feet + Vec3::Y * 0.8, 1.6, 0.35, [0.5, 0.75, 1.0, 1.0]);
    ring(feet + Vec3::Y * 0.1, 2.6, 0.5, [0.5, 0.75, 1.0, 1.0]);
    burst(feet + Vec3::Y * 1.0, 4, Bit::Star);
}

/// The action command (timing.rs): a gold burst on the blow, a quick ring, a few stars; a "GREAT!"
/// bigger, with a little flash. `at`: the enemy's chest (Elden Ring).
pub fn nice(at: Vec3, great: bool) {
    spark(at, if great { 1.5 } else { 1.1 }, GOLD);
    ring(at, if great { 1.6 } else { 1.0 }, 0.25, GOLD);
    burst(at, if great { 4 } else { 2 }, Bit::Star);
    if great {
        flash([1.0, 0.8, 0.35], 0.12, 0.25);
    }
}

/// The Mikiri (timing.rs): Mario landed on the attacker's head. A white-blue flash, a ring at
/// his feet up there, stars. `at`: where he stomped (Elden Ring).
pub fn mikiri(at: Vec3) {
    flash([0.7, 0.85, 1.0], 0.3, 0.35);
    spark(at, 1.6, WHITE);
    ring(at, 1.4, 0.3, [0.75, 0.9, 1.0, 1.0]);
    ring(at - Vec3::Y * 0.3, 2.4, 0.45, WHITE);
    burst(at, 5, Bit::Star);
    crate::lakitu::shake(0.6);
}

/// The Bros. Jump (co-op): off the partner's head, a puff and a ring there, SM64's bounce.
pub fn bros_jump(head: Vec3) {
    ring(head, 0.8, 0.25, WHITE);
    puffs(head, 3, 0.25);
    burst(head + Vec3::Y * 0.2, 2, Bit::Star);
    worker::call("bros jump", |_| unsafe {
        sm64::sm64_play_sound_global(SOUND_BOUNCE_OFF_OBJECT);
        sm64::sm64_play_sound_global(SOUND_YAHOO);
    });
}
const SOUND_BOUNCE_OFF_OBJECT: i32 = 0x0459_B081;
const SOUND_YAHOO: i32 = 0x2404_8081;

/// A weapon switched in the middle of a fight (moves.rs's swap strike): a golden ring round Mario
/// and a flash of stars at his chest. `feet`: Elden Ring.
pub fn swap(feet: Vec3) {
    ring(feet + Vec3::Y * 0.8, 1.3, 0.22, GOLD);
    ring(feet + Vec3::Y * 0.1, 1.8, 0.3, GOLD);
    burst(feet + Vec3::Y * 0.9, 3, Bit::Star);
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

/// The Chain Chomp's swing (Kratos' Blades of Chaos): a ribbon of fire behind its body, longer
/// lived than the hammer's, and sparks thrown off its chain. Game thread, every tick it hits:
/// its body and a point on the chain near it, and the chain's ends (hand, body).
static CHOMP_TRAIL: Mutex<Vec<(Vec3, Vec3, Instant)>> = Mutex::new(Vec::new());
pub const CHOMP_TRAIL_TIME: f32 = 0.3;
/// sparks flying: from, velocity (m/s), when
static FLYING_SPARKS: Mutex<Vec<(Vec3, Vec3, Instant)>> = Mutex::new(Vec::new());
const FLYING_SPARK_TIME: f32 = 0.35;
const FLYING_SPARK_GRAVITY: f32 = 9.0;

pub fn chomp_point(head: Vec3, mid: Vec3, hand: Vec3) {
    let mut trail = CHOMP_TRAIL.lock().unwrap_or_else(|e| e.into_inner());
    trail.retain(|(.., t)| t.elapsed().as_secs_f32() < CHOMP_TRAIL_TIME);
    trail.push((head, mid, Instant::now()));
    drop(trail);
    // (two or three sparks off the taut chain, each tick, flying out sideways)
    let mut sparks = FLYING_SPARKS.lock().unwrap_or_else(|e| e.into_inner());
    sparks.retain(|(.., t)| t.elapsed().as_secs_f32() < FLYING_SPARK_TIME);
    let along = head - hand;
    for _ in 0..(2 + (crate::rand01() * 2.0) as usize) {
        let at = hand + along * (0.25 + 0.75 * crate::rand01());
        let side = Vec3::new(crate::rand01() - 0.5, crate::rand01() * 0.8, crate::rand01() - 0.5).normalize_or(Vec3::Y);
        sparks.push((at, side * (3.0 + 4.0 * crate::rand01()), Instant::now()));
    }
}

/// The Chomp's fire ribbon, oldest first: body, chain point, how faded (0-1).
pub fn chomp_trail() -> Vec<(Vec3, Vec3, f32)> {
    let trail = CHOMP_TRAIL.lock().unwrap_or_else(|e| e.into_inner());
    trail.iter().map(|(h, m, t)| (*h, *m, t.elapsed().as_secs_f32() / CHOMP_TRAIL_TIME)).filter(|(.., t)| *t < 1.0).collect()
}

/// The sparks flying: where each is, where it was a moment ago (a streak), how far along (0-1).
pub fn flying_sparks() -> Vec<(Vec3, Vec3, f32)> {
    let sparks = FLYING_SPARKS.lock().unwrap_or_else(|e| e.into_inner());
    sparks
        .iter()
        .filter_map(|(p, v, t)| {
            let age = t.elapsed().as_secs_f32();
            let at = |a: f32| *p + *v * a - Vec3::Y * (0.5 * FLYING_SPARK_GRAVITY * a * a);
            (age < FLYING_SPARK_TIME).then(|| (at(age), at((age - 0.03).max(0.0)), age / FLYING_SPARK_TIME))
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

/// Game thread: Mario, blown away, smacked into a wall (blown.rs): stars all round, a red spark,
/// a ring of dust on the wall.
pub fn splat(at: Vec3) {
    spark(at, 1.6, GOLD);
    spark(at, 0.9, RED);
    burst(at, 6, Bit::Star);
    ring(at, 1.3, 0.3, DUST);
    puffs(at - Vec3::Y * 0.5, 3, 0.4);
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
        sm64::sm64_play_sound_volume(SOUND_GENERAL_COIN, 0.45);
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
    CHOMP_TRAIL.lock().unwrap_or_else(|e| e.into_inner()).clear();
    FLYING_SPARKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *HURT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *FLASH.lock().unwrap_or_else(|e| e.into_inner()) = None;
    STREAKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
