//! The kart in the game (v4 Part 4): L3 and Mario hops into Mario Kart's kart where he stands (plan
//! B: a mode of his own, Torrent isn't summoned; kartproto.rs is plan A's test). kart.rs drives it
//! by the game's rays and kartplay.rs says with what; this carries it: the body in the seat (Mario,
//! put there every tick), the items (the trap kit's shell and bananas, the Fire Flower, the
//! mushrooms), runs into enemies, blows on him, coins, falls, the racing camera, the engine's voice,
//! the sounds, the model's pose (engine_mario: the legs piece's bones) and the HUD's.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use glam::{Quat, Vec3};

use crate::combat::{self, Attack, Combat, Target};
use crate::engine_mario::{KART_PARTS, PARTS, PartPose};
use crate::fireball::Fire;
use crate::kart::Kart;
use crate::kartplay::{self, Banana, Driver, KartNow, Model, Pads, RamKind, Size};
use crate::traps::{Kind, Traps};
use crate::{colossus, log, sm64, worker};

const TICK: f32 = 1.0 / 30.0;
/// the hop in (ticks: the kart pops up meanwhile); the kart's middle this high over his feet as it
/// comes (m: about where its springs hold it)
const HOP_IN: u32 = 10;
/// room for it: nothing within this ahead and behind, to each side (m), at knee height, nor over
/// his head this high
const ROOM_LONG: f32 = 1.6;
const ROOM_SIDE: f32 = 1.0;
const ROOM_UP: f32 = 1.8;
/// out of it: a hop up (m/s); in again no sooner than this (s)
const HOP_OUT: f32 = 6.5;
const OUT_WAIT: f32 = 1.0;
/// the poof as it goes (s)
const POOF: f32 = 0.25;
/// the things it puts out last this far from Mario (m: the kit's own is 20)
const KART_AWAY: f32 = 60.0;
/// a shell leaves this much faster than the kart (m/s); a banana thrown ahead this much faster and
/// up (m/s)
const SHELL_AHEAD: f32 = 8.0;
const BANANA_AHEAD: (f32, f32) = (9.0, 5.0);
/// the same enemy run into again after this long (ticks; a boss or a big one)
const RAM_EVERY: u32 = 30;
const BIG_RAM_EVERY: u32 = 90;
/// a small one flung lands with this much of its health (%)
const FLING_LANDING: f32 = 2.0;
/// the throw from the seat (ticks)
const THROW_TICKS: u32 = 8;
/// in the air this long (ticks): no ground under it, he's let fall
const FALL_LONG: u32 = 180;
/// a landing after this long in the air is heard (ticks); a hard one from this high (m)
const LANDING_HEARD: u32 = 8;
const LANDING_HARD: f32 = 4.0;
/// a wall: this much speed lost in a tick (m/s)
const WALL_LOSS: f32 = 6.0;
/// the governor: every this many ticks a ray down this far (m: no ground at all there is the map not
/// loaded yet, not a cliff: off one it flies, and a fall past 22 m kills as on foot), this far ahead
/// in time (s), from this fast
const AHEAD_EVERY: u32 = 3;
const AHEAD_DOWN: f32 = 600.0;
const AHEAD_TIME: f32 = 1.2;
const AHEAD_FROM: f32 = 8.0;
/// a line in the log while driving every this long (s)
const REPORT: f32 = 10.0;
/// ACT_ER_CLING's poses (libsm64): the hop in, seated, a throw from the seat
const POSE_LEAP: i32 = 2;
const POSE_SEAT: i32 = 14;
const POSE_THROW: i32 = 15;
const ACT_FREEFALL: u32 = 0x0100_088C;

/// SM64's sounds (the ROM's)
const SOUND_POP: i32 = 0x502C_8081;
const SOUND_HOP: i32 = 0x0400_8081;
const SOUND_LAND: i32 = 0x0408_8081;
const SOUND_LAND_HARD: i32 = 0x0460_8081;
const SOUND_DRIFT_UP: i32 = 0x3016_0091;
const SOUND_BOOST: i32 = 0x0456_8081;
const SOUND_HOOHOO: i32 = 0x2403_8081;
const SOUND_YAHOO: i32 = 0x2404_8081;
const SOUND_MAMA_MIA: i32 = 0x2420_8081;
const SOUND_COINS: i32 = 0x3030_0081;
const SOUND_BONK: i32 = 0x0445_A081;
const SOUND_BOING: i32 = 0x306C_4081;
const SOUND_FLAME: i32 = 0x5055_8081;
const SOUND_THROW: i32 = 0x0435_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;

/// What sits in the kart and is carried by it. Plan B (built): Mario himself, put in his seat
/// every tick (`MarioBody`). Plan A (if kartproto.rs's test finds Torrent can be driven): Torrent
/// would carry the kart's model with Mario riding him, a body writing Torrent's physics place and
/// turn instead (as kartproto.rs does) and lost once he's dismounted.
pub trait Body: Send {
    /// In the seat at `feet` facing `face` (Elden Ring), in pose `pose` (`start`: as it begins).
    fn place(&mut self, origin: [f32; 3], feet: Vec3, face: Vec3, pose: i32, start: bool);
    /// Gone from the seat (SM64 took him: a blow's knockback, a cutscene's action).
    fn lost(&self, action: u32) -> bool;
    /// Out of the seat at `feet` going `vel` (m/s), his fall counted from `peak` (Elden Ring height).
    fn leave(&mut self, origin: [f32; 3], feet: Vec3, vel: Vec3, peak: f32);
}

/// Mario in the seat: in ACT_ER_CLING's kart pose at the seat each tick, no fall counted from there.
struct MarioBody {
    id: i32,
}

impl Body for MarioBody {
    fn place(&mut self, origin: [f32; 3], feet: Vec3, face: Vec3, pose: i32, start: bool) {
        colossus::place(self.id, origin, feet, face, pose, start);
        let y = crate::collision::er_to_sm(origin, &HavokPosition(feet.x, feet.y, feet.z, 0.0))[1];
        let id = self.id;
        worker::call("kart peak", move |_| unsafe { sm64::sm64_er_set_peak(id, y) });
    }

    fn lost(&self, action: u32) -> bool {
        action != colossus::ACT_ER_CLING
    }

    fn leave(&mut self, origin: [f32; 3], feet: Vec3, vel: Vec3, peak: f32) {
        let sm = crate::collision::er_to_sm(origin, &HavokPosition(feet.x, feet.y, feet.z, 0.0));
        let top = crate::collision::er_to_sm(origin, &HavokPosition(feet.x, peak.max(feet.y), feet.z, 0.0))[1];
        let k = crate::scale() * 30.0;
        let v = Vec3::new(-vel.x, vel.y, vel.z) / k;
        let flat = v.with_y(0.0);
        let face = (flat.length() > 0.5).then(|| flat.x.atan2(flat.z));
        let id = self.id;
        worker::call("kart out", move |_| unsafe {
            sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]);
            sm64::sm64_set_mario_action(id, ACT_FREEFALL);
            if let Some(f) = face {
                sm64::sm64_set_mario_faceangle(id, f);
            }
            sm64::sm64_set_mario_forward_velocity(id, flat.length());
            sm64::sm64_set_mario_velocity(id, v.x, v.y, v.z);
            sm64::sm64_er_set_peak(id, top);
        });
    }
}

struct Ride {
    kart: Kart,
    driver: Driver,
    body: Box<dyn Body>,
    /// ticks in it, the hop in still to come, where he hopped from
    ticks: u32,
    hop_in: u32,
    from: Vec3,
    wheel_spin: [f32; 4],
    /// this tick's model and last one's (the frame draws between them)
    model: Model,
    prev: Model,
    /// the highest it's been since it was last on the ground, ticks off it
    peak: f32,
    air: u32,
    /// enemies run into lately (key, tick)
    rammed: Vec<(u64, u32)>,
    /// the last mini-turbo's level (its flames' colour; 0 a mushroom's)
    turbo: u8,
    throw_until: u32,
    /// no ground ahead (the governor)
    blind: bool,
    /// in the throw's pose from the seat this tick
    throwing: bool,
    report: Instant,
}

struct State {
    ride: Option<Ride>,
    /// gone: when (no kart again for a moment), and its last model shrinking into a puff
    out_at: Option<Instant>,
    leaving: Option<(Model, Instant)>,
    /// the flat floor SM64 has under him (its units), the stick's right as the kart's
    floor: Option<[f32; 3]>,
    sign: f32,
    /// coins picked up since the last tick
    coins: u32,
    /// Mario's libsm64 id (to let him go)
    id: i32,
}

static STATE: Mutex<State> = Mutex::new(State { ride: None, out_at: None, leaving: None, floor: None, sign: 1.0, coins: 0, id: -1 });
/// L3 (input_task): in or out, on the next tick
static TOGGLE: AtomicBool = AtomicBool::new(false);
/// in it (for the HUD thread and lib.rs, without the state's lock), and on its wheels
static HOLDS: AtomicBool = AtomicBool::new(false);
static GROUNDED: AtomicBool = AtomicBool::new(false);

/// The HUD's: the kart's coins, the mushrooms (left, the next's share), the shell's and the
/// banana's wait (0-1, 1 ready), a boost on (s left).
#[derive(Clone, Copy, Default)]
pub struct Hud {
    pub coins: u32,
    pub mushrooms: (u32, f32),
    pub shell: f32,
    pub banana: f32,
    pub boost: f32,
}

/// What the frame draws of it: its parts in the world (between two ticks), how big (popping in or
/// out), its exhausts' flames (mouth, the way out, colour, size).
#[derive(Clone, Copy)]
pub struct Look {
    pub parts: [(Vec3, Quat); KART_PARTS],
    pub size: f32,
    pub flames: [Option<(Vec3, Vec3, [f32; 3], f32)>; 2],
}

static HUD: Mutex<Option<Hud>> = Mutex::new(None);
static LOOK: Mutex<Option<Look>> = Mutex::new(None);

pub fn hud() -> Option<Hud> {
    *HUD.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn look() -> Option<Look> {
    *LOOK.lock().unwrap_or_else(|e| e.into_inner())
}

/// L3 was pressed (input_task): in or out of the kart.
pub fn toggle() {
    TOGGLE.store(true, Ordering::Relaxed);
}

/// Mario is in the kart (or hopping in): his buttons are its, SM64's collision isn't read.
pub fn holds() -> bool {
    HOLDS.load(Ordering::Relaxed)
}

/// In it, its wheels on the ground (lib.rs: a spot he stood on, for the coin bag and the warp back).
pub fn grounded() -> bool {
    holds() && GROUNDED.load(Ordering::Relaxed)
}

/// Coins picked up this tick (lib.rs): each a kart coin too (Mario Kart's: more top speed).
pub fn coins(n: u32) {
    if n > 0 && holds() {
        STATE.lock().unwrap_or_else(|e| e.into_inner()).coins += n;
    }
}

/// SM64's world was re-centred (lib.rs): its floor under him is put again.
pub fn recentred() {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).floor = None;
}

/// The world's origin moved by `d` (lib.rs): the kart with it.
pub fn shift(d: Vec3) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let move_model = |m: &mut Model| m.parts.iter_mut().for_each(|p| p.0 += d);
    if let Some(r) = s.ride.as_mut() {
        r.kart.pos += d;
        r.from += d;
        r.peak += d.y;
        move_model(&mut r.model);
        move_model(&mut r.prev);
    }
    if let Some((m, _)) = s.leaving.as_mut() {
        move_model(m);
    }
}

/// Out of Mario mode, a load, dead: no kart (nothing of it left).
pub fn clear() {
    TOGGLE.store(false, Ordering::Relaxed);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.ride.take().is_some() {
        log("kart: out (cleared)");
    }
    s.leaving = None;
    s.floor = None;
    s.coins = 0;
    drop(s);
    gone();
    *crate::engine_mario::KART.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *LOOK.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// He's put elsewhere (a warp) or the game moves him (a door, a grace, a grab: follow mode): out of
/// it where he is now (off the seat, falling), the kart poofs.
pub fn release() {
    TOGGLE.store(false, Ordering::Relaxed);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = s.ride.take() {
        log("kart: out (put elsewhere, or the game moves him)");
        s.leaving = Some((r.model, Instant::now()));
        s.out_at = Some(Instant::now());
        s.floor = None;
        let id = s.id;
        drop(s);
        gone();
        if id >= 0 {
            colossus::let_go(id, Vec3::ZERO, false);
        }
    }
}

/// (the camera, the engine, the HUD: none of it)
fn gone() {
    HOLDS.store(false, Ordering::Relaxed);
    crate::lakitu::set_kart(None);
    crate::audio::set_engine(None);
    *HUD.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn sound(id: i32, volume: f32) {
    worker::call("kart sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

fn buzz(why: &str, callout: Option<&'static str>) {
    log(format!("kart: not here ({why})"));
    sound(SOUND_BUZZ, 0.5);
    if let Some(c) = callout {
        crate::style::callout(c, None, [1.0, 0.8, 0.3], false, 0.0);
    }
}

/// er_mario.ini `kart_engine = 0..2`: the engine's loudness (1 as it is).
fn engine_volume() -> f32 {
    crate::paths::config("kart_engine").and_then(|v| v.trim().parse::<f32>().ok()).map_or(1.0, |v| v.clamp(0.0, 2.0))
}

/// What lib.rs gives the kart every SM64 tick: its buttons (taken from everyone else while it
/// holds), Mario's feet and facing (Elden Ring), SM64's action, dead, a menu up, whether it can
/// start now (on foot on the ground, nothing else going on) and he can stay in it, the open world,
/// a blow on him this tick (its half wedges) and whether it sends him flying, his libsm64 id, the
/// SM64 world's origin, the tick, the game camera's axes (forward, up, right).
pub struct In {
    pub pads: Pads,
    pub feet: Vec3,
    pub face: Vec3,
    pub action: u32,
    pub dead: bool,
    pub paused: bool,
    pub can_start: bool,
    pub can_stay: bool,
    pub overworld: bool,
    pub hurt: Option<u32>,
    /// who hit him (its character id), if the game says
    pub by: Option<u32>,
    pub blown: bool,
    pub id: i32,
    pub origin: [f32; 3],
    pub tick: u32,
    pub cam: Option<(Vec3, Vec3, Vec3)>,
}

/// What the tick did: the kart has his buttons; it took this tick's blow (its health only, no
/// knockback: lib.rs); SM64's floor under him to put now (its units); out of the kart just now.
#[derive(Default)]
pub struct Out {
    pub consumed: bool,
    pub soaked: bool,
    pub floor: Option<[f32; 3]>,
    pub released: bool,
}

/// Game thread, every SM64 tick before SM64's (after the blow on him is known): in, out, the drive.
#[allow(clippy::too_many_arguments)]
pub fn update(inp: &In, targets: &[Target], combat: &mut Combat, player: &ChrIns, traps: &mut Traps, fire: &mut Fire, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Out {
    let _span = crate::perf::span(crate::perf::KART);
    let toggle = TOGGLE.swap(false, Ordering::Relaxed);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.id = inp.id;
    if let Some((cam_fwd, cam_up, cam_right)) = inp.cam {
        s.sign = kartplay::steer_sign(cam_fwd, cam_up, cam_right);
    }
    if s.leaving.is_some_and(|(_, t)| t.elapsed().as_secs_f32() > POOF) {
        s.leaving = None;
    }
    let mut out = Out::default();
    let Some(mut ride) = s.ride.take() else {
        s.coins = 0;
        if toggle && !inp.dead {
            summon(&mut s, inp, ray);
        }
        return out;
    };
    // out: dead, can't stay, SM64 took him, L3 (a hop out)
    let lost = ride.hop_in == 0 && ride.ticks > 2 && ride.body.lost(inp.action);
    let why = if inp.dead {
        Some(("dead", false))
    } else if !inp.can_stay {
        Some(("he can't stay in it", false))
    } else if lost {
        Some(("knocked off", false))
    } else if toggle && ride.hop_in == 0 {
        Some(("L3", true))
    } else {
        None
    };
    if let Some((why, hop)) = why {
        if hop {
            let (feet, face, _) = kartplay::seat(&ride.kart, ride.driver.spin(), ride.driver.roll());
            let side = face.cross(Vec3::Y).normalize_or(Vec3::X);
            ride.body.leave(inp.origin, feet + Vec3::Y * 0.1, ride.kart.vel * 0.6 + Vec3::Y * HOP_OUT + side * 1.5, feet.y + 1.0);
        }
        end(&mut s, ride, why);
        out.released = true;
        return out;
    }
    out.consumed = true;
    ride.ticks += 1;
    let tick = inp.tick;
    // a blow on him: off it if it sends him flying (lib.rs gives him SM64's knockback then), else a
    // spin-out and coins lost, the health taken with no knockback
    if let Some(w) = inp.hurt {
        let taken = kartplay::hit(inp.blown);
        if taken.unseat {
            log(format!("kart: knocked out of the kart ({w} half wedges)"));
            let (feet, _, _) = kartplay::seat(&ride.kart, ride.driver.spin(), ride.driver.roll());
            ride.body.leave(inp.origin, feet + Vec3::Y * 0.2, ride.kart.vel * 0.5, ride.peak.max(feet.y));
            end(&mut s, ride, "a heavy blow");
            out.released = true;
            return out;
        }
        out.soaked = true;
        if taken.spin && ride.driver.spin_out() {
            sound(SOUND_MAMA_MIA, 0.5);
        }
        let lost = taken.coins.min(ride.kart.coins);
        ride.kart.coins -= lost;
        if lost > 0 {
            sound(SOUND_COINS, 0.3);
            crate::impact::kart_coins(ride.kart.pos, lost as usize);
        }
        log(format!("kart: hit by {}, {lost} coins lost ({} left)", inp.by.map_or("nobody known".to_string(), |id| format!("c{id:04}")), ride.kart.coins));
    }
    // coins picked up: kart coins (more top speed, up to ten)
    let picked = std::mem::take(&mut s.coins);
    ride.kart.coins = (ride.kart.coins + picked).min(crate::kart::COINS);
    // a menu: it waits as it is, Mario in his seat
    if inp.paused {
        let (feet, face, _) = kartplay::seat(&ride.kart, ride.driver.spin(), ride.driver.roll());
        ride.body.place(inp.origin, feet, face, POSE_SEAT, false);
        ride.prev = ride.model;
        s.ride = Some(ride);
        return out;
    }
    // the hop in: from where he stood up and into the seat, the kart settling on its springs
    if ride.hop_in > 0 {
        ride.kart.step(&crate::kart::Input::default(), TICK, ray);
        ride.prev = ride.model;
        ride.model = kartplay::model(&ride.kart, 0.0, 0.0, &ride.wheel_spin);
        let (feet, face, _) = kartplay::seat(&ride.kart, 0.0, 0.0);
        let k = 1.0 - (ride.hop_in - 1) as f32 / HOP_IN as f32;
        let e = k * k * (3.0 - 2.0 * k);
        let at = ride.from.lerp(feet, e) + Vec3::Y * (1.2 * 4.0 * k * (1.0 - k));
        ride.hop_in -= 1;
        ride.body.place(inp.origin, if ride.hop_in == 0 { feet } else { at }, face, if ride.hop_in == 0 { POSE_SEAT } else { POSE_LEAP }, ride.ticks == 1 || ride.hop_in == 0);
        put_floor(&mut s, &mut out, inp.origin, feet);
        publish(&ride, traps);
        s.ride = Some(ride);
        return out;
    }
    // (the stick's right as the kart's right(): the world's hands, by the camera)
    let mut pads = inp.pads;
    pads.stick.x *= s.sign;
    drive(&mut ride, &pads, inp, targets, combat, player, traps, fire, ray, tick);
    // a fall from too high: SM64's own fall from where it began (it kills as on foot)
    let grounded = ride.kart.grounded();
    GROUNDED.store(grounded >= 3, Ordering::Relaxed);
    if grounded == 0 {
        ride.air += 1;
        ride.peak = ride.peak.max(ride.kart.pos.y);
    } else if grounded >= 2 {
        if ride.air > 0 {
            let drop = ride.peak - ride.kart.pos.y;
            if kartplay::lethal(ride.peak, ride.kart.pos.y) {
                log(format!("kart: fell {drop:.0} m: lethal"));
                let (feet, _, _) = kartplay::seat(&ride.kart, 0.0, 0.0);
                ride.body.leave(inp.origin, feet + Vec3::Y * 0.3, Vec3::new(0.0, -8.0, 0.0), ride.peak);
                end(&mut s, ride, "a lethal fall");
                out.released = true;
                return out;
            }
            if ride.air >= LANDING_HEARD {
                sound(if drop > LANDING_HARD { SOUND_LAND_HARD } else { SOUND_LAND }, if drop > LANDING_HARD { 0.4 } else { 0.3 });
                crate::impact::thud_small(ride.kart.pos - Vec3::Y * 0.4);
                crate::lakitu::shake((drop * 0.08).clamp(0.1, 0.6));
            }
        }
        ride.air = 0;
        ride.peak = ride.kart.pos.y;
    }
    if ride.air > FALL_LONG {
        log("kart: in the air for six seconds: let fall");
        let (feet, _, _) = kartplay::seat(&ride.kart, 0.0, 0.0);
        ride.body.leave(inp.origin, feet, ride.kart.vel, ride.peak);
        end(&mut s, ride, "a fall into nothing");
        out.released = true;
        return out;
    }
    // Mario in his seat (a throw's pose a moment), SM64's floor under him
    let (feet, face, _) = kartplay::seat(&ride.kart, ride.driver.spin(), ride.driver.roll());
    ride.throwing = tick < ride.throw_until;
    ride.body.place(inp.origin, feet, face, if ride.throwing { POSE_THROW } else { POSE_SEAT }, false);
    put_floor(&mut s, &mut out, inp.origin, feet);
    // the camera, the engine, the HUD
    let k = &ride.kart;
    crate::lakitu::set_kart(Some(crate::lakitu::KartView { off: k.pos - feet, fwd: k.forward(), vel: k.vel, boost: k.boost > 0.0 }));
    let slide = k.vel.dot(k.right()).abs();
    // (the tyres screech round a spin-out, hiss through a drift, and when it slides sideways)
    let skid = if ride.driver.spinning() { 1.0 } else if k.drifting.is_some() { 0.6 + 0.13 * k.drift_level() as f32 } else if grounded >= 2 { ((slide - 4.0) / 6.0).clamp(0.0, 1.0) } else { 0.0 };
    let mut tone = kartplay::engine_tone(k.speed(), inp.pads.rt, k.boost > 0.0, skid);
    tone.volume *= engine_volume();
    tone.skid *= engine_volume().min(1.0);
    crate::audio::set_engine(Some(tone));
    if ride.report.elapsed().as_secs_f32() >= REPORT {
        ride.report = Instant::now();
        // (the drawn tyres' bottoms over the map's ground straight under them, cm: near 0 where
        // the ground's collision is where it's drawn)
        let gaps = kartplay::tyre_gaps(&ride.model, ray).map(|g| g.map_or("-".to_string(), |g| format!("{:.0}", g * 100.0)));
        log(format!(
            "kart: {:.1} m/s, coins {}, wheels down {grounded}, drift {}, mushrooms {}, tyres over the ground [{}] cm",
            k.speed(),
            k.coins,
            k.drift_level(),
            ride.driver.mushrooms().0,
            gaps.join(", ")
        ));
    }
    publish(&ride, traps);
    s.ride = Some(ride);
    out
}

/// The tick's drive: the buttons, the step, walls, items, the drift's and the boosts' looks, runs
/// into enemies.
#[allow(clippy::too_many_arguments)]
fn drive(ride: &mut Ride, pads: &Pads, inp: &In, targets: &[Target], combat: &mut Combat, player: &ChrIns, traps: &mut Traps, fire: &mut Fire, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>, tick: u32) {
    let k = &mut ride.kart;
    let now = KartNow { grounded: k.grounded(), vy: k.vel.y, boost: k.boost, drift: k.drift_level(), upset: k.upset() };
    let acts = ride.driver.tick(pads, &now, TICK);
    let mut input = acts.input;
    // the governor: no ground ahead at speed, it slows (the map may not be loaded there yet)
    if tick % AHEAD_EVERY == 0 {
        let speed = k.speed();
        let was = ride.blind;
        ride.blind = speed > AHEAD_FROM && {
            let ahead = k.pos + k.forward().with_y(0.0).normalize_or(k.forward()) * speed * AHEAD_TIME;
            ray(ahead + Vec3::Y * 4.0, ahead - Vec3::Y * AHEAD_DOWN).is_none()
        };
        if ride.blind && !was {
            log("kart: no ground ahead, slowing");
        }
    }
    if ride.blind {
        input.throttle = 0.0;
        input.brake = input.brake.max(0.5);
    }
    let (boost_was, vel_was, drift_was, righting_was) = (k.boost, k.vel, k.drift_level(), k.righting());
    let drifting_was = k.drifting.is_some();
    k.step(&input, TICK, ray);
    // a wall: its bump
    let lost = vel_was.with_y(0.0).length() - k.vel.with_y(0.0).length();
    if lost > WALL_LOSS && k.vel.dot(vel_was) < vel_was.length_squared() * 0.5 {
        sound(SOUND_BONK, 0.4);
        crate::impact::cannon_wall(k.pos + k.rot * kartplay::NOSE);
        crate::lakitu::shake(0.4);
    }
    // (on its roof, A: back onto its wheels with a hop)
    if !righting_was && k.righting() {
        sound(SOUND_BOING, 0.35);
        log("kart: put back on its wheels (A)");
    }
    if acts.super_hop {
        k.launch(kartplay::SUPER_HOP);
        sound(SOUND_BOING, 0.4);
        log("kart: super hop");
    } else if now.grounded >= 2 && k.vel.y - vel_was.y > 2.5 {
        // (a hop off the ground)
        sound(SOUND_HOP, 0.3);
    }
    if acts.landed_trick {
        k.boost = k.boost.max(kartplay::TRICK_BOOST);
        ride.turbo = 0;
        sound(SOUND_BOOST, 0.3);
        log("kart: trick landed, boost");
    }
    if acts.trick {
        sound(SOUND_YAHOO, 0.5);
        log("kart: trick");
    }
    if acts.mushroom {
        ride.turbo = 0;
        sound(SOUND_BOOST, 0.35);
        sound(SOUND_HOOHOO, 0.45);
        log(format!("kart: mushroom ({} left)", ride.driver.mushrooms().0));
    } else if acts.empty {
        sound(SOUND_BUZZ, 0.4);
    }
    if let Some(level) = acts.drift_up {
        sound(SOUND_DRIFT_UP, 0.12);
        crate::dlog(format!("kart: drift level {level}"));
    }
    if drifting_was && k.drifting.is_none() && k.boost > boost_was + 0.3 {
        ride.turbo = drift_was;
        sound(SOUND_BOOST, 0.3);
        log(format!("kart: mini-turbo {drift_was}"));
    }
    // the wheels drawn turning
    let rate = kartplay::wheel_rate(k.speed());
    for w in ride.wheel_spin.iter_mut() {
        *w = (*w + rate * TICK).rem_euclid(std::f32::consts::TAU);
    }
    // the drift's sparks off the rear wheels, the boost's streak
    let level = k.drift_level();
    if level > 0 && tick % 2 == 0 {
        for w in &k.wheels[2..] {
            if let Some(c) = w.contact {
                crate::impact::kart_spark(c + Vec3::Y * 0.05, kartplay::SPARKS[level as usize - 1], 0.18 + 0.06 * level as f32);
            }
        }
    } else if k.drifting.is_some() && tick % 3 == 0 {
        if let Some(c) = k.wheels[2].contact {
            crate::impact::kart_dust(c);
        }
    }
    if k.boost > 0.0 {
        crate::impact::dash_point(k.pos, boost_was <= 0.0);
    }
    // the items: off the nose, the tail (the throw's pose from the seat a moment)
    let vis = kartplay::visual(k, ride.driver.spin(), ride.driver.roll());
    let ahead = (vis * Vec3::Z).with_y(0.0).normalize_or(Vec3::Z);
    let speed = k.speed().max(0.0);
    if acts.shell && traps.launch(Kind::Green, k.pos + vis * kartplay::NOSE, ahead * (speed + SHELL_AHEAD), KART_AWAY) {
        ride.throw_until = tick + THROW_TICKS;
        sound(SOUND_THROW, 0.4);
        log("kart: green shell");
    }
    match acts.banana {
        Some(Banana::Back) => {
            if traps.launch(Kind::Banana, k.pos + vis * kartplay::TAIL, -ahead * 1.5 + Vec3::Y * 2.0, KART_AWAY) {
                log("kart: banana dropped");
            }
        }
        Some(Banana::Ahead) => {
            if traps.launch(Kind::Banana, k.pos + vis * kartplay::NOSE + Vec3::Y * 0.3, k.vel.with_y(0.0) + ahead * BANANA_AHEAD.0 + Vec3::Y * BANANA_AHEAD.1, KART_AWAY) {
                ride.throw_until = tick + THROW_TICKS;
                sound(SOUND_THROW, 0.4);
                log("kart: banana thrown ahead");
            }
        }
        None => {}
    }
    if acts.fire {
        fire.kart_shot(k.pos + vis * kartplay::NOSE + Vec3::Y * 0.2, vis * Vec3::Z, k.vel, inp.origin, tick);
        ride.throw_until = tick + THROW_TICKS;
        sound(SOUND_FLAME, 0.4);
    }
    // runs into enemies
    ride.rammed.retain(|(_, at)| tick.wrapping_sub(*at) < BIG_RAM_EVERY);
    for (i, t) in targets.iter().enumerate() {
        let Some(h) = t.shootable() else { continue };
        if t.friendly() || t.is_prop() {
            continue;
        }
        let (r, height) = (t.radius * crate::scale(), t.height * crate::scale());
        let Some(into) = kartplay::contact(k.pos, k.rot, k.vel, t.feet(), r, height) else { continue };
        let boss = !combat::liftable(h);
        let key = t.key();
        let every = if boss { BIG_RAM_EVERY } else { RAM_EVERY };
        if ride.rammed.iter().any(|(k2, at)| *k2 == key && tick.wrapping_sub(*at) < every) {
            continue;
        }
        let size = match crate::juggle::pull_size(h, r, height) {
            crate::juggle::PullSize::Small => Size::Small,
            crate::juggle::PullSize::Medium => Size::Medium,
            crate::juggle::PullSize::Big => Size::Big,
        };
        let Some(ram) = kartplay::ram(size, boss, into) else { continue };
        ride.rammed.push((key, tick));
        let attack = if ram.kind == RamKind::Fling { Attack::KartRam } else { Attack::KartBump };
        combat.deal_scaled(player, targets, &[(i, attack, false)], tick, ram.worth);
        let dir = (t.feet() - k.pos).with_y(0.0).normalize_or(ahead);
        if let Some((v, up)) = ram.fling {
            crate::bat::throw(h, ahead.lerp(dir, 0.5).normalize_or(ahead), v, up, FLING_LANDING);
        }
        let along = k.vel.dot(dir);
        k.vel += dir * (along * ram.keep - along);
        if ram.spin && ride.driver.spin_out() {
            sound(SOUND_MAMA_MIA, 0.5);
        }
        log(format!("kart: rammed {key:#x} ({size:?}{}, {into:.1} m/s, x{:.2}){}", if boss { ", a boss or a big one" } else { "" }, ram.worth, if ram.spin { ", spun out" } else { "" }));
    }
    ride.prev = ride.model;
    ride.model = kartplay::model(k, ride.driver.spin(), ride.driver.roll(), &ride.wheel_spin);
}

/// SM64's flat floor under the seat at `feet` again, if he's gone off the last one.
fn put_floor(s: &mut State, out: &mut Out, origin: [f32; 3], feet: Vec3) {
    let sm = crate::collision::er_to_sm(origin, &HavokPosition(feet.x, feet.y, feet.z, 0.0));
    if kartplay::floor_reload(s.floor, sm) {
        let f = [sm[0], sm[1] - kartplay::FLOOR_DROP, sm[2]];
        s.floor = Some(f);
        out.floor = Some(f);
    }
}

/// The HUD's numbers from this tick.
fn publish(ride: &Ride, traps: &Traps) {
    let ready = traps.ready();
    *HUD.lock().unwrap_or_else(|e| e.into_inner()) = Some(Hud {
        coins: ride.kart.coins,
        mushrooms: ride.driver.mushrooms(),
        shell: ready[1],
        banana: ready[0],
        boost: ride.kart.boost,
    });
}

/// L3 out of it: the kart where he stands, if it can be, and he hops in.
fn summon(s: &mut State, inp: &In, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    if s.out_at.is_some_and(|t| t.elapsed().as_secs_f32() < OUT_WAIT) {
        return;
    }
    if !inp.can_start {
        return buzz("busy, or off the ground", None);
    }
    if !inp.overworld {
        return buzz("not in the open world", Some("NOT HERE"));
    }
    let face = inp.face.with_y(0.0).normalize_or(Vec3::Z);
    let side = face.cross(Vec3::Y).normalize_or(Vec3::X);
    let knee = inp.feet + Vec3::Y * 0.5;
    let blocked = [face * ROOM_LONG, -face * ROOM_LONG, side * ROOM_SIDE, -side * ROOM_SIDE].iter().any(|d| ray(knee, knee + *d).is_some()) || ray(inp.feet + Vec3::Y * 0.3, inp.feet + Vec3::Y * ROOM_UP).is_some();
    if blocked {
        return buzz("no room", Some("NO ROOM"));
    }
    let kart = Kart::new(inp.feet + Vec3::Y * Kart::rest_height(), face.x.atan2(face.z));
    let model = kartplay::model(&kart, 0.0, 0.0, &[0.0; 4]);
    s.ride = Some(Ride {
        kart,
        driver: Driver::new(),
        body: Box::new(MarioBody { id: inp.id }),
        ticks: 0,
        hop_in: HOP_IN,
        from: inp.feet,
        wheel_spin: [0.0; 4],
        model,
        prev: model,
        peak: inp.feet.y,
        air: 0,
        rammed: Vec::new(),
        turbo: 0,
        throw_until: 0,
        blind: false,
        throwing: false,
        report: Instant::now(),
    });
    s.leaving = None;
    s.floor = None;
    s.coins = 0;
    HOLDS.store(true, Ordering::Relaxed);
    sound(SOUND_POP, 0.4);
    crate::impact::kart_in(inp.feet);
    log(format!("kart: in at {:.1?} (yaw {:.0} deg)", inp.feet, face.x.atan2(face.z).to_degrees()));
}

/// Out of it (`why`): the kart poofs.
fn end(s: &mut State, ride: Ride, why: &str) {
    log(format!("kart: out ({why}) after {:.0} s", ride.ticks as f32 * TICK));
    sound(SOUND_POP, 0.35);
    crate::impact::poof(ride.kart.pos);
    s.leaving = Some((ride.model, Instant::now()));
    s.out_at = Some(Instant::now());
    s.floor = None;
    s.coins = 0;
    gone();
}

/// Game thread, after SM64's tick: Mario's parts (relative to his feet, the seat's) turned with the
/// kart, his hands on the wheel.
pub fn pose_rider(parts: &mut [PartPose; PARTS]) {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(r) = s.ride.as_ref().filter(|r| r.hop_in == 0) else { return };
    let (feet, _, tilt) = kartplay::seat(&r.kart, r.driver.spin(), r.driver.roll());
    for p in parts.iter_mut() {
        p.pos = tilt * p.pos;
        p.rot = (tilt * p.rot).normalize();
    }
    // (a throw from the seat: its own arms)
    if r.throwing {
        return;
    }
    let [plus, minus] = kartplay::grips(&r.model);
    let body = r.model.parts[kartplay::BODY].1;
    let across = body * Vec3::X;
    let down = -(body * Vec3::Y);
    // (which of his hands is on the kart's +x side: SM64's are mirrored)
    let right_on_plus = (parts[9].pos - parts[6].pos).dot(across) > 0.0;
    let (right, left) = if right_on_plus { (plus, minus) } else { (minus, plus) };
    let out = |plus_side: bool| if plus_side { across } else { -across };
    crate::ik::arm(parts, true, right - feet, down + out(right_on_plus) * 0.6, 1.0);
    crate::ik::arm(parts, false, left - feet, down + out(!right_on_plus) * 0.6, 1.0);
}

/// Every frame (lib.rs, as the Tarnished is put at `at` turned `q`, `alpha` of the way from the
/// last SM64 tick to the next): the kart's parts on his skeleton (character space) and its look for
/// the HUD.
pub fn model_pose(alpha: f32, q: Quat, at: Vec3) {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let alpha = alpha.clamp(0.0, 1.0);
    let now = match (&s.ride, &s.leaving) {
        (Some(r), _) => {
            // (popping up as he hops in: a little past its size and back)
            let t = ((r.ticks as f32 + alpha) / HOP_IN as f32).min(1.0);
            let size = 1.0 - (1.0 - t).powi(3) + 0.12 * (t * std::f32::consts::PI).sin();
            let flames = flames(r);
            Some((r.prev, r.model, size.max(0.02), flames))
        }
        (None, Some((m, t))) => {
            let k = 1.0 - (t.elapsed().as_secs_f32() / POOF).clamp(0.0, 1.0);
            (k > 0.0).then_some((*m, *m, k.max(0.02), [None; 2]))
        }
        _ => None,
    };
    drop(s);
    let Some((prev, cur, size, flames)) = now else {
        *crate::engine_mario::KART.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *LOOK.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return;
    };
    let mut parts = [(Vec3::ZERO, Quat::IDENTITY); KART_PARTS];
    for i in 0..KART_PARTS {
        let (a, b) = (prev.parts[i], cur.parts[i]);
        // (a jump: the origin's shift, a re-centre: no sliding across)
        parts[i] = if a.0.distance(b.0) < 3.0 { (a.0.lerp(b.0, alpha), a.1.slerp(b.1, alpha).normalize()) } else { b };
    }
    let body = parts[kartplay::BODY].0;
    for p in parts.iter_mut().skip(1) {
        p.0 = body + (p.0 - body) * size;
    }
    let inv = q.inverse();
    let pose = parts.map(|(p, r)| PartPose { rot: (inv * r).normalize(), pos: inv * (p - at), scale: size, stretch: Vec3::ONE });
    *crate::engine_mario::KART.lock().unwrap_or_else(|e| e.into_inner()) = Some(pose);
    // the exhausts' flames where the body is now
    let (bp, br) = parts[kartplay::BODY];
    let flames = flames.map(|f| f.map(|(local, c, k)| (bp + br * (local * size), br * -Vec3::Z, c, k * size)));
    *LOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Look { parts, size, flames });
}

/// The exhausts' flames now: (mouth in the kart's axes, colour, size), by the drift and the boost.
fn flames(r: &Ride) -> [Option<(Vec3, [f32; 3], f32)>; 2] {
    let f = kartplay::flame_of(r.kart.drift_level(), r.kart.boost, r.turbo);
    kartplay::EXHAUSTS.map(|m| f.map(|(c, k)| (m, c, k)))
}
