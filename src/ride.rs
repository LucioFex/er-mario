//! Riding Yoshi into a fight (Torrent is Yoshi, yoshi.rs). The game moves him (A jumps, B
//! dashes), the rest is Yoshi's:
//! - X: his tongue, lashed out ahead: it hits whoever it reaches, and each enemy it licks makes him
//!   an egg (Yoshi's Island)
//! - Y or RB: an egg thrown at the nearest enemy ahead (held: one after the other), arcing and
//!   bouncing once off the ground; he has a few, more with every lick and a new one now and then
//! - L2 in the air: a stomp, a shockwave all around where he lands
//! - LB: a burst of speed (his stride faster, dust behind him), then a rest
//! Mario's world doesn't tick while he rides (lib.rs follows the game), so the hits are dealt here
//! with combat.rs's, from Yoshi's place and facing. hud.rs draws the tongue, the eggs, the stock.

use std::sync::Mutex;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// the tongue: how far (m), how wide (m), out and back (s), before the next (s)
const TONGUE_REACH: f32 = 4.5;
const TONGUE_WIDTH: f32 = 0.7;
const TONGUE_TIME: f32 = 0.3;
const TONGUE_REST: f32 = 0.12;
/// eggs: at most, at the start, a new one every (s); thrown this fast (m/s), falling (m/s²), gone
/// after (s), at most this far (m) to the enemy it's aimed at, between two (s)
const EGGS_MAX: u32 = 6;
const EGGS_START: u32 = 3;
const EGG_EVERY: f32 = 3.0;
const EGG_SPEED: f32 = 24.0;
const EGG_GRAVITY: f32 = 14.0;
const EGG_LIFE: f32 = 2.5;
const EGG_AIM: f32 = 28.0;
const EGG_REST: f32 = 0.28;
/// how near an egg hits (m, plus the enemy's body)
const EGG_RADIUS: f32 = 0.5;
/// the stomp's reach (m), and before the next (s)
const STOMP_REACH: f32 = 4.5;
const STOMP_REST: f32 = 1.5;
/// the burst of speed: how much faster, how long, before the next (s)
pub const BOOST: f32 = 1.6;
const BOOST_TIME: f32 = 2.5;
const BOOST_REST: f32 = 6.0;
/// enemies within this cone ahead (cos 45°)
const AHEAD_COS: f32 = 0.7;

// SM64's sounds: Yoshi's call, a swish for the tongue, the gulp of a new egg (his 1-up), the throw,
// an egg bursting, the stomp's thud, Lakitu's buzz
const SOUND_YOSHI: i32 = 0x3070_3081;
const SOUND_LICK: i32 = 0x305C_0081;
const SOUND_EGG_MADE: i32 = 0x7015_0081;
const SOUND_THROW: i32 = 0x0435_8081;
const SOUND_POP: i32 = 0x502C_8081;
const SOUND_STOMP: i32 = 0x5068_4081;
const SOUND_BUZZ: i32 = 0x700E_0081;

struct Egg {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    bounced: bool,
    hit: Vec<u64>,
}

struct State {
    acc: f32,
    stock: u32,
    refill: f32,
    /// the tongue: since when (s), where it was going
    tongue: Option<(f32, Vec3)>,
    licked: Vec<u64>,
    tongue_ready: f32,
    egg_ready: f32,
    eggs: Vec<Egg>,
    /// the stomp, armed in the air; when it may come again
    stomp: bool,
    stomp_ready: f32,
    air: bool,
    boost_until: f32,
    boost_ready: f32,
    /// time riding (s)
    t: f32,
    held: [bool; 5],
    sprites: Sprites,
}

/// What hud.rs draws: the tongue (from Yoshi's mouth to its tip), the eggs flying, the stock.
#[derive(Clone, Default)]
pub struct Sprites {
    pub tongue: Option<(Vec3, Vec3)>,
    pub eggs: Vec<(Vec3, f32)>,
    pub stock: u32,
    pub riding: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn sprites() -> Sprites {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.sprites.clone()).unwrap_or_default()
}

/// How much faster Yoshi goes now (the burst of speed, LB).
pub fn boost() -> f32 {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(1.0, |s| if s.t < s.boost_until { BOOST } else { 1.0 })
}

pub fn clear() {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn sound(id: i32, volume: f32) {
    worker::call("ride sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

/// The buttons this frame: X, Y, RB, L2, LB.
pub struct Buttons(pub [bool; 5]);

/// Game thread, every frame while Mario's on Yoshi (`saddle`: on him, not getting on or off, no
/// menu): the buttons, Yoshi's feet, velocity and facing, whether he's in the air; advances
/// `ticks` (combat.rs's cooldowns) at SM64's 30 Hz. `ray` the map between two points.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    dt: f32,
    saddle: bool,
    buttons: Buttons,
    yoshi: Option<(Vec3, Vec3, bool)>,
    facing: Vec3,
    origin: [f32; 3],
    ticks: &mut u32,
    player: &ChrIns,
    combat: &mut Combat,
    ray: impl Fn(Vec3, Vec3) -> Option<Vec3>,
) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let (Some((feet, vel, air)), true) = (yoshi, saddle) else {
        if let Some(s) = guard.as_mut() {
            s.sprites = Sprites::default();
            s.tongue = None;
            s.eggs.clear();
        }
        return;
    };
    let s = guard.get_or_insert_with(|| State {
        acc: 0.0,
        stock: EGGS_START,
        refill: 0.0,
        tongue: None,
        licked: Vec::new(),
        tongue_ready: 0.0,
        egg_ready: 0.0,
        eggs: Vec::new(),
        stomp: false,
        stomp_ready: 0.0,
        air: false,
        boost_until: 0.0,
        boost_ready: 0.0,
        t: 0.0,
        held: [false; 5],
        sprites: Sprites::default(),
    });
    let dt = dt.min(0.1);
    s.t += dt;
    s.acc += dt;
    while s.acc >= 1.0 / 30.0 {
        s.acc -= 1.0 / 30.0;
        *ticks = ticks.wrapping_add(1);
    }
    let tick = *ticks;
    let pressed: [bool; 5] = std::array::from_fn(|i| buttons.0[i] && !s.held[i]);
    s.held = buttons.0;
    let face = Vec3::new(facing.x, 0.0, facing.z).normalize_or(Vec3::Z);
    let mouth = feet + Vec3::Y * 1.45 + face * 1.1;
    let chest = |t: &combat::Target| {
        let c = crate::collision::sm_to_er(origin, [t.sm[0], t.sm[1] + t.height.min(300.0) * 0.5, t.sm[2]]);
        Vec3::new(c.0, c.1, c.2)
    };

    // new eggs now and then
    if s.stock < EGGS_MAX {
        s.refill += dt;
        if s.refill >= EGG_EVERY {
            s.refill = 0.0;
            s.stock += 1;
        }
    }

    // X: the tongue
    if pressed[0] && s.tongue.is_none() && s.t >= s.tongue_ready {
        // (at the nearest enemy ahead, else straight ahead)
        let here = HavokPosition(mouth.x, mouth.y, mouth.z, 0.0);
        let to = combat::nearby(&here, TONGUE_REACH + 2.0, origin)
            .iter()
            .filter(|t| t.shootable().is_some())
            .map(&chest)
            .filter(|c| (*c - mouth).with_y(0.0).normalize_or(face).dot(face) >= AHEAD_COS && (*c - mouth).length() <= TONGUE_REACH + 1.0)
            .min_by(|a, b| (*a - mouth).length().total_cmp(&(*b - mouth).length()))
            .unwrap_or(mouth + face * TONGUE_REACH);
        let dir = (to - mouth).normalize_or(face);
        s.tongue = Some((s.t, dir));
        s.licked.clear();
        sound(SOUND_LICK, 0.5);
    }
    let mut tongue_tip = None;
    if let Some((since, dir)) = s.tongue {
        let p = (s.t - since) / TONGUE_TIME;
        if p >= 1.0 {
            s.tongue = None;
            s.tongue_ready = s.t + TONGUE_REST;
        } else {
            // out fast, back a little slower
            let k = if p < 0.4 { p / 0.4 } else { 1.0 - (p - 0.4) / 0.6 };
            let tip = mouth + dir * TONGUE_REACH * k;
            // (stopped by the map)
            let tip = ray(mouth, tip).unwrap_or(tip);
            tongue_tip = Some(tip);
            let here = HavokPosition(tip.x, tip.y, tip.z, 0.0);
            let near = combat::nearby(&here, TONGUE_REACH, origin);
            for (i, t) in near.iter().enumerate() {
                if t.shootable().is_none() || s.licked.contains(&t.key()) {
                    continue;
                }
                // along the tongue so far
                let c = chest(t);
                let along = (c - mouth).dot(dir).clamp(0.0, (tip - mouth).length());
                let off = (c - (mouth + dir * along)).with_y(0.0).length();
                let body = t.radius * crate::scale();
                let tall = t.height * crate::scale();
                let foot = c.y - tall * 0.5;
                let y = mouth.y + dir.y * along;
                if off <= TONGUE_WIDTH + body && y >= foot - 0.5 && y <= foot + tall + 0.5 {
                    s.licked.push(t.key());
                    combat.deal(player, &near, &[(i, Attack::Tongue, false)], tick);
                    // a lick, an egg
                    if s.stock < EGGS_MAX {
                        s.stock += 1;
                        sound(SOUND_EGG_MADE, 0.35);
                    }
                }
            }
        }
    }

    // Y or RB: an egg (held: one after another)
    let throw = buttons.0[1] || buttons.0[2];
    if throw && s.t >= s.egg_ready {
        if s.stock == 0 {
            if pressed[1] || pressed[2] {
                sound(SOUND_BUZZ, 0.35);
            }
        } else {
            s.stock -= 1;
            s.egg_ready = s.t + EGG_REST;
            let from = feet + Vec3::Y * 1.7 - face * 0.2;
            let here = HavokPosition(feet.x, feet.y, feet.z, 0.0);
            let aim = combat::nearby(&here, EGG_AIM, origin)
                .iter()
                .filter(|t| t.shootable().is_some())
                .map(&chest)
                .filter(|c| {
                    let d = (*c - from).with_y(0.0);
                    d.length() > 1.0 && d.normalize().dot(face) >= AHEAD_COS && ray(from, *c).is_none()
                })
                .min_by(|a, b| (*a - from).length().total_cmp(&(*b - from).length()));
            // aimed: an arc that lands on it; else ahead, a little up, with his speed
            let v = match aim {
                Some(c) => {
                    let d = c - from;
                    let flat = d.with_y(0.0);
                    let time = (flat.length() / EGG_SPEED).max(0.12);
                    flat / time + Vec3::Y * (d.y / time + 0.5 * EGG_GRAVITY * time)
                }
                None => face * EGG_SPEED + Vec3::Y * 4.0 + vel.with_y(0.0),
            };
            s.eggs.push(Egg { pos: from, vel: v, age: 0.0, bounced: false, hit: Vec::new() });
            sound(SOUND_THROW, 0.45);
        }
    }
    // the eggs: flying, bouncing once, bursting on whoever they hit
    let mut eggs = std::mem::take(&mut s.eggs);
    eggs.retain_mut(|e| {
        e.age += dt;
        e.vel -= Vec3::Y * EGG_GRAVITY * dt;
        let next = e.pos + e.vel * dt;
        if let Some(at) = ray(e.pos, next) {
            if e.bounced || e.vel.y > -2.0 {
                crate::impact::egg_burst(at);
                sound(SOUND_POP, 0.35);
                return false;
            }
            e.bounced = true;
            e.pos = at + Vec3::Y * 0.05;
            e.vel = Vec3::new(e.vel.x * 0.7, -e.vel.y * 0.5, e.vel.z * 0.7);
            return true;
        }
        let prev = e.pos;
        e.pos = next;
        let here = HavokPosition(e.pos.x, e.pos.y, e.pos.z, 0.0);
        let near = combat::nearby(&here, 4.0, origin);
        for (i, t) in near.iter().enumerate() {
            if t.shootable().is_none() || e.hit.contains(&t.key()) {
                continue;
            }
            let c = chest(t);
            let body = t.radius * crate::scale();
            let tall = t.height * crate::scale();
            let ab = e.pos - prev;
            let k = if ab.length_squared() > 1e-4 { ((c - prev).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 1.0 };
            let at = prev + ab * k;
            let foot = c.y - tall * 0.5;
            if (c - at).with_y(0.0).length() <= EGG_RADIUS + body && at.y >= foot - EGG_RADIUS && at.y <= foot + tall + EGG_RADIUS {
                e.hit.push(t.key());
                combat.deal(player, &near, &[(i, Attack::Egg, false)], tick);
                crate::impact::egg_burst(at);
                sound(SOUND_POP, 0.45);
                return false;
            }
        }
        e.age < EGG_LIFE
    });
    s.eggs = eggs;

    // L2 in the air: the stomp, on landing
    if pressed[3] && air && !s.stomp && s.t >= s.stomp_ready {
        s.stomp = true;
        sound(SOUND_YOSHI, 0.5);
    }
    if s.stomp && s.air && !air {
        s.stomp = false;
        s.stomp_ready = s.t + STOMP_REST;
        log("ride: Yoshi stomp");
        let here = HavokPosition(feet.x, feet.y, feet.z, 0.0);
        let near = combat::nearby(&here, STOMP_REACH + 3.0, origin);
        let hits: Vec<_> = near
            .iter()
            .enumerate()
            .filter(|(_, t)| t.shootable().is_some() && (chest(t) - feet).with_y(0.0).length() <= STOMP_REACH + t.radius * crate::scale())
            .map(|(i, _)| (i, Attack::Shockwave, false))
            .collect();
        combat.deal(player, &near, &hits, tick);
        crate::impact::yoshi_stomp(feet);
        crate::lakitu::shake(1.0);
        sound(SOUND_STOMP, 0.7);
    }
    if !air {
        s.stomp = false;
    }
    s.air = air;

    // LB: the burst of speed
    if pressed[4] {
        if s.t >= s.boost_ready {
            s.boost_until = s.t + BOOST_TIME;
            s.boost_ready = s.t + BOOST_TIME + BOOST_REST;
            sound(SOUND_YOSHI, 0.55);
            crate::impact::dash_point(feet, true);
            log("ride: Yoshi boost");
        } else {
            sound(SOUND_BUZZ, 0.35);
        }
    }
    if s.t < s.boost_until && !air {
        crate::impact::dash_point(feet + Vec3::Y * 0.2, false);
    }

    s.sprites = Sprites { tongue: tongue_tip.map(|tip| (mouth, tip)), eggs: s.eggs.iter().map(|e| (e.pos, e.age)).collect(), stock: s.stock, riding: true };
}
