//! The Capture (Super Mario Odyssey's, with the hook for Cappy; Monster Hunter's wirebug mount):
//! hooked to a big enemy with R2 held, A jumps Mario onto its back and it's his for a while. The
//! stick steers it (it's pushed along where it points, turned that way), X and Y make it attack
//! with its own moves (light and heavy), B charges, A or R2 pressed again hops him off behind it.
//! Charmed meanwhile (the game's own team for it): its blows hurt the other enemies and they go
//! for it; blows meant for Mario land on it instead. Driven into a wall at speed it crashes there,
//! staggered and left open (its stance broken: grab its tail). Worn out, or its time up, he hops
//! off and it comes to its senses a little dazed (its poise gone). A long wait after. Not a boss
//! with its health bar up: those are fought, not driven.
//!
//! Its attacks: the game asks an enemy's moves for an action by id (its AI's `GOAL_COMMON_Attack`
//! ids, 3000 on, each playing the animation of that number). The id is written where the AI's
//! would be (CSChrActionRequestModule::npc_action_id) after the AI ran and before its behaviour
//! reads it (`pre_behavior`, from the ChrIns_PreBehaviorSafe task). An id it hasn't got starts
//! nothing: it's noted for that kind of enemy and the next one is tried.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// how long a capture lasts at most (s), the wait before the next (s)
const TIME: f32 = 10.0;
const RECHARGE: f32 = 45.0;
/// how fast it's driven (m/s), how quickly it turns that way (rad/s), how fast into a wall counts
/// as a crash (m/s: it built up speed)
const DRIVE: f32 = 5.5;
const TURN: f32 = 2.5;
const CRASH_SPEED: f32 = 3.5;
/// the stance a crash cracks (swing.rs's meter, as poise), and the ride it takes first (s: a
/// run-up, not a nudge into the wall)
const CRASH_STANCE: f32 = 70.0;
const CRASH_AFTER: f32 = 1.0;
/// how fast he hops off behind it (SM64 units a frame: well clear of its back)
const HOP_OFF: f32 = 26.0;
/// the charge (B): how long (s), how fast (m/s), the wait before the next (s)
const CHARGE_TIME: f32 = 0.7;
const CHARGE_SPEED: f32 = 11.0;
const CHARGE_EVERY: f32 = 1.6;
/// Elden Ring's "charmed" team: fights for the player, the enemies' enemy
const CHARMED: u8 = 15;
/// a blow meant for Mario costs it this share of its health a wedge; this much left, he hops off
const SHIELD_SHARE: f32 = 0.035;
const WORN_OUT: f32 = 0.15;
/// its attacks by the AI's ids: light ones (X) and heavy ones (Y), tried in this order
const LIGHT: [i32; 6] = [3000, 3001, 3002, 3003, 3004, 3005];
const HEAVY: [i32; 10] = [3010, 3011, 3012, 3013, 3014, 3015, 3006, 3007, 3008, 3009];
/// an asked-for attack that hasn't started within this (s) it hasn't got; how many ids one press
/// tries; once one runs, the next press is taken this far into it (s)
const START_WITHIN: f32 = 0.35;
const START_FRAMES: u32 = 10;
const TRIES: u32 = 4;
const NEXT_AFTER: f32 = 0.55;
/// an id is only taken for missing after this many asks it didn't answer while it stood idle
const MISSES: u32 = 2;
/// blows meant for Mario: at most one on his mount in this long (s; his own grace after a hit)
const SHIELD_EVERY: f32 = 0.7;

pub const ACT_ER_BOSS_RIDE: u32 = 0x0000_035C;
const ACT_DOUBLE_JUMP: u32 = 0x0300_0881;

const SOUND_RIDE: i32 = 0x2404_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_CRASH: i32 = 0x312F_0081;
const SOUND_CHARGE: i32 = 0x305C_0081;
const SOUND_SHIELD: i32 = 0x0442_8081;

/// An attack asked of it, not started yet.
struct Ask {
    id: i32,
    heavy: bool,
    since: Instant,
    tries: u32,
    /// frames it's been written; the animation it was in when first written (None: not yet)
    frames: u32,
    start_anim: Option<i32>,
}

impl Ask {
    fn new(id: i32, heavy: bool, tries: u32) -> Self {
        Ask { id, heavy, since: Instant::now(), tries, frames: 0, start_anim: None }
    }
}

/// An enemy's animation that's one of its attacks (as the AI's ids go).
fn is_attack(anim: i32) -> bool {
    (3000..4000).contains(&anim)
}

struct Ride {
    mob: FieldInsHandle,
    /// its kind (NpcParam's character id): which attack ids it has
    kind: i32,
    since: Instant,
    speed: f32,
    /// R2 came up since he got on (pressed again after that, he hops off)
    let_go: bool,
    /// what it was before: its team, its own attacks and walking off
    team: u8,
    no_attacks: bool,
    no_moving: bool,
    ask: Option<Ask>,
    /// the attack it's in (its id) and since when
    attacking: Option<(i32, Instant)>,
    next: [usize; 2],
    charge_until: Option<Instant>,
    charge_hit: Vec<u64>,
    last_charge: Option<Instant>,
    last_shield: Option<Instant>,
    keys: [bool; 3],
}

struct State {
    ride: Option<Ride>,
    ready_at: Option<Instant>,
    /// per kind of enemy: the attack ids it turned out not to have, those it has, and the asks
    /// each id didn't answer (idle)
    missing: HashMap<i32, HashSet<i32>>,
    works: HashMap<i32, HashSet<i32>>,
    misses: HashMap<(i32, i32), u32>,
    /// the HUD's: the share of its time left and of its health (set on the game thread)
    hud: Option<(f32, f32)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(|| State { ride: None, ready_at: None, missing: HashMap::new(), works: HashMap::new(), misses: HashMap::new(), hud: None }))
}

/// Mario is up on one.
pub fn active() -> bool {
    with(|s| s.ride.is_some())
}

/// For the HUD while he rides: the share of its time left and of its health (0-1). (The render
/// thread's: what the game thread last saw, the characters aren't read there.)
pub fn hud() -> Option<(f32, f32)> {
    with(|s| s.ride.as_ref().and(s.hud))
}

/// The enemy he rides: where Mario sits on it (Elden Ring: on top of its body, a little behind).
fn seat(h: &FieldInsHandle) -> Option<(Vec3, Vec3, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let ph = &chr.modules.physics;
    let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let o = ph.orientation;
    let fwd = Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(Vec3::new(0.0, 0.0, -1.0)).with_y(0.0).normalize_or(Vec3::Z);
    let h = ph.hit_height.max(ph.chr_hit_height).clamp(1.0, 12.0);
    let r = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.3, 6.0);
    Some((p + Vec3::Y * h * 0.92 - fwd * r * 0.25, fwd, r))
}

/// Game thread (hook.rs): A while hooked to a big one with R2 held: up he jumps. True if he's on.
pub fn mount(h: &FieldInsHandle, id: i32) -> bool {
    let ready = with(|s| s.ready_at.is_none_or(|t| Instant::now() >= t) && s.ride.is_none());
    // (co-op guest: the host owns the enemies; a boss in a scripted moment isn't ridden)
    let chr = (ready && !crate::coop::guest() && !combat::hands_off(h) && !combat::is_boss_now(h) && seat(h).is_some())
        .then(|| unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(h)))
        .flatten();
    let Some(chr) = chr else {
        worker::call("ride buzz", |_| unsafe { sm64::sm64_play_sound_global(SOUND_BUZZ) });
        return false;
    };
    // charmed: on his side, its own attacks and wandering off held back (the stick and X/Y are its
    // will now)
    let (team, kind) = (chr.team_type, chr.character_id as i32);
    let (no_attacks, no_moving) = (chr.debug_flags.disabled_secondary_actions(), chr.debug_flags.disabled_movement());
    chr.team_type = CHARMED;
    chr.debug_flags.set_disabled_secondary_actions(true);
    chr.debug_flags.set_disabled_movement(true);
    with(|s| {
        s.ride = Some(Ride {
            mob: *h,
            kind,
            since: Instant::now(),
            speed: 0.0,
            let_go: false,
            team,
            no_attacks,
            no_moving,
            ask: None,
            attacking: None,
            next: [0, 0],
            charge_until: None,
            charge_hit: Vec::new(),
            last_charge: None,
            last_shield: None,
            keys: [true; 3],
        })
    });
    log(format!("boss ride: on its back, c{kind:04} captured (team {team} -> {CHARMED})"));
    crate::style::callout("CAPTURE!", None, [1.0, 0.8, 0.35], true, 3.0);
    worker::call("boss ride", move |_| unsafe {
        sm64::sm64_set_mario_action(id, ACT_ER_BOSS_RIDE);
        sm64::sm64_play_sound_global(SOUND_RIDE);
    });
    true
}

/// Its current animation (as the AI's ids go: without the variant's millions) and how far in (s).
fn anim_of(chr: &eldenring::cs::ChrIns) -> (i32, f32) {
    let t = &chr.modules.time_act;
    let a = &t.anim_queue[(t.read_idx % 10) as usize];
    (a.anim_id % 1_000_000, a.play_time)
}

/// The next attack id to ask of it for `heavy` or not: one it has, or one not tried yet (light ones
/// stand in for heavy ones it hasn't got, and the other way round).
fn pick(s: &mut State, kind: i32, heavy: bool) -> Option<i32> {
    let missing = s.missing.get(&kind).cloned().unwrap_or_default();
    let r = s.ride.as_mut()?;
    let order: [(&[i32], usize); 2] = if heavy { [(&HEAVY, 1), (&LIGHT, 0)] } else { [(&LIGHT, 0), (&HEAVY, 1)] };
    for (list, slot) in order {
        for k in 0..list.len() {
            let id = list[(r.next[slot] + k) % list.len()];
            if !missing.contains(&id) {
                r.next[slot] = (r.next[slot] + k + 1) % list.len();
                return Some(id);
            }
        }
    }
    None
}

/// Game thread, the ChrIns_PreBehaviorSafe task (after the AI, before its behaviour): an attack
/// asked of it goes in where the AI's would; one that started, or didn't, is noted.
pub fn pre_behavior() {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = guard.as_mut() else { return };
    let Some(r) = s.ride.as_mut() else { return };
    let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(&r.mob)) else { return };
    let (anim, at) = anim_of(chr);
    if r.attacking.is_some_and(|(id, _)| anim != id) {
        r.attacking = None;
    }
    let Some(ask) = r.ask.as_mut() else {
        // (nothing asked: its own attacks and its walking stay held back)
        chr.debug_flags.set_disabled_secondary_actions(true);
        chr.debug_flags.set_disabled_movement(true);
        return;
    };
    let start_anim = *ask.start_anim.get_or_insert(anim);
    // started: that one, or another of its attacks it went into instead (a variant of it)
    if (anim == ask.id && at < 0.5) || (anim != start_anim && is_attack(anim)) {
        let (id, kind) = (ask.id, r.kind);
        r.attacking = Some((anim, Instant::now()));
        r.ask = None;
        chr.debug_flags.set_disabled_secondary_actions(true);
        chr.modules.action_request.npc_action_id = -1;
        if anim == id && s.works.entry(kind).or_default().insert(id) {
            log(format!("boss ride: c{kind:04} attacks with {id}"));
        }
        return;
    }
    if ask.since.elapsed().as_secs_f32() < START_WITHIN || ask.frames < START_FRAMES {
        ask.frames += 1;
        chr.debug_flags.set_disabled_secondary_actions(false);
        chr.debug_flags.set_disabled_movement(true);
        chr.modules.action_request.npc_action_id = ask.id;
        return;
    }
    // nothing came of it. Taken for one it hasn't got only if it stood idle all along (in an
    // attack, staggered, turning, it couldn't take a new one) and more than once; the next one
    // tried (a few a press)
    let (id, heavy, tries, kind) = (ask.id, ask.heavy, ask.tries, r.kind);
    // (a step from walking to standing as the stick came up isn't a reason to give up)
    let idle = !is_attack(anim);
    if idle && !s.works.get(&kind).is_some_and(|w| w.contains(&id)) {
        let n = s.misses.entry((kind, id)).or_default();
        *n += 1;
        if *n >= MISSES && s.missing.entry(kind).or_default().insert(id) {
            log(format!("boss ride: c{kind:04} has no attack {id} (anim {anim} at {at:.2} s)"));
        }
    }
    let next = if idle && tries < TRIES { pick(s, kind, heavy) } else { None };
    let Some(r) = s.ride.as_mut() else { return };
    r.ask = next.map(|id| Ask::new(id, heavy, tries + 1));
    chr.modules.action_request.npc_action_id = -1;
    if r.ask.is_none() {
        chr.debug_flags.set_disabled_secondary_actions(true);
        if idle {
            log(format!("boss ride: c{kind:04}, no attack started after {tries} tries"));
        }
    }
}

/// Game thread: a blow meant for Mario while he rides (`halves` of his wedges): it lands on his
/// mount.
pub fn shield(halves: u32) {
    let Some(mob) = with(|s| {
        let r = s.ride.as_mut()?;
        if r.last_shield.is_some_and(|t| t.elapsed().as_secs_f32() < SHIELD_EVERY) {
            return None;
        }
        r.last_shield = Some(Instant::now());
        Some(r.mob)
    }) else {
        return;
    };
    let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(&mob)) else { return };
    let d = &mut chr.modules.data;
    let cost = (d.max_hp as f32 * SHIELD_SHARE * halves.max(1) as f32 * 0.5) as i32;
    d.hp = (d.hp - cost).max(1);
    let p = chr.modules.physics.position;
    crate::impact::hurt(Vec3::new(p.0, p.1 + 1.4, p.2));
    worker::call("ride shield", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_SHIELD, 0.45) });
}

/// Game thread, every SM64 tick while he rides: `steer` the stick's way (Elden Ring, horizontal,
/// its length 0-1), `hold` R2 held, `jump` A pressed, `keys` X, Y and B held, SM64's origin.
/// Moves it along, keeps Mario on its back (his SM64 position, for the next tick), asks it for
/// attacks, charges, crashes it into a wall.
#[allow(clippy::too_many_arguments)]
pub fn update(dt: f32, steer: Vec3, hold: bool, jump: bool, keys: [bool; 3], action: u32, id: i32, origin: [f32; 3], combat: &mut Combat, player: &eldenring::cs::ChrIns, tick: u32, ray: impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    // (knocked off it: the capture's over, he isn't pinned to its back)
    if with(|s| s.ride.as_ref().is_some_and(|r| action != ACT_ER_BOSS_RIDE && r.since.elapsed().as_secs_f32() > 0.15)) {
        off(id, "knocked off", None);
        return;
    }
    let Some((mob, since)) = with(|s| s.ride.as_ref().map(|r| (r.mob, r.since))) else { return };
    let Some((seat_at, fwd, radius)) = seat(&mob) else {
        off(id, "it's gone", None);
        return;
    };
    let health = unsafe { WorldChrMan::instance() }
        .ok()
        .and_then(|w| w.chr_ins_by_handle(&mob))
        .map_or(1.0, |c| c.modules.data.hp as f32 / c.modules.data.max_hp.max(1) as f32);
    let worn = health < WORN_OUT;
    with(|s| s.hud = Some((1.0 - since.elapsed().as_secs_f32() / TIME, health)));
    // (R2 needn't stay held up there: letting it go on getting on ended the ride at once, and his
    // hop off landed on the enemy's head, a ground pound)
    let again = with(|s| {
        let r = s.ride.as_mut().unwrap();
        r.let_go |= !hold;
        hold && r.let_go
    });
    if again || jump || worn || since.elapsed().as_secs_f32() > TIME {
        off(id, if jump { "jumped off" } else if again { "R2 again" } else if worn { "it's worn out" } else { "time" }, Some(fwd));
        return;
    }
    // X and Y: its attacks; B: a charge
    let (charging, charged) = with(|s| {
        let kind = s.ride.as_ref().unwrap().kind;
        let r = s.ride.as_mut().unwrap();
        let pressed = [keys[0] && !r.keys[0], keys[1] && !r.keys[1], keys[2] && !r.keys[2]];
        r.keys = keys;
        let free = r.ask.is_none() && r.charge_until.is_none_or(|t| Instant::now() >= t) && r.attacking.is_none_or(|(_, t)| t.elapsed().as_secs_f32() >= NEXT_AFTER);
        let mut charged = false;
        if free && (pressed[0] || pressed[1]) {
            let heavy = pressed[1];
            if let Some(a) = pick(s, kind, heavy) {
                s.ride.as_mut().unwrap().ask = Some(Ask::new(a, heavy, 1));
            }
        } else if free && pressed[2] && r.last_charge.is_none_or(|t| t.elapsed().as_secs_f32() >= CHARGE_EVERY) {
            r.charge_until = Some(Instant::now() + std::time::Duration::from_secs_f32(CHARGE_TIME));
            r.last_charge = Some(Instant::now());
            r.charge_hit.clear();
            charged = true;
        }
        let r = s.ride.as_ref().unwrap();
        (r.charge_until.is_some_and(|t| Instant::now() < t), charged)
    });
    if charged {
        log("boss ride: charge");
        worker::call("ride charge", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_CHARGE, 0.6) });
    }
    // driven where the stick points (in a charge, straight on): turned that way, pushed along; in
    // one of its attacks its own moves carry it
    let dt = dt.min(0.05);
    let want = if charging { fwd } else { steer.with_y(0.0) };
    let in_attack = with(|s| s.ride.as_ref().is_some_and(|r| r.attacking.is_some() || r.ask.is_some()));
    let mut crashed = None;
    let mut bowl = None;
    if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(&mob)) {
        let ph = &mut chr.modules.physics;
        let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
        let mut speed = with(|s| s.ride.as_ref().map_or(0.0, |r| r.speed));
        if want.length() > 0.2 && !in_attack {
            let dir = want.normalize();
            let turn = fwd.angle_between(dir).min(TURN * dt) * fwd.cross(dir).y.signum();
            let o = ph.orientation;
            let q = (Quat::from_rotation_y(turn) * Quat::from_xyzw(o.0, o.1, o.2, o.3)).normalize();
            ph.orientation = eldenring::rotation::Quaternion(q.x, q.y, q.z, q.w);
            speed = if charging { CHARGE_SPEED } else { (speed + DRIVE * 2.0 * dt).min(DRIVE) };
            let next = p + dir * speed * dt;
            // into a wall: it crashes there at speed, else it just stops
            match ray(p + Vec3::Y * 1.0, next + dir * radius + Vec3::Y * 1.0) {
                Some(_) if speed >= CRASH_SPEED && since.elapsed().as_secs_f32() >= CRASH_AFTER => crashed = Some(p + dir * radius + Vec3::Y * 1.2),
                Some(_) => speed = 0.0,
                None => {
                    // (along the ground: the floor where it's going)
                    let ground = ray(next + Vec3::Y * 2.0, next - Vec3::Y * 3.0).unwrap_or(next);
                    ph.position = HavokPosition(next.x, ground.y, next.z, 0.0);
                    ph.chr_proxy_pos_update_requested = true;
                }
            }
            if charging {
                bowl = Some((p, dir));
            }
        } else {
            speed = (speed - DRIVE * 3.0 * dt).max(0.0);
        }
        with(|s| {
            if let Some(r) = s.ride.as_mut() {
                r.speed = speed;
            }
        });
    }
    // the charge bowls over whoever's in its way (each once a charge)
    if let Some((p, dir)) = bowl {
        let near = combat::nearby(&HavokPosition(p.x, p.y, p.z, 0.0), radius + 3.0, origin);
        let me = combat::handle_key(&mob);
        let hits: Vec<(usize, Attack, bool)> = with(|s| {
            let r = s.ride.as_mut().unwrap();
            let mut hits = Vec::new();
            for (i, t) in near.iter().enumerate() {
                if !t.shootable().is_some_and(|h| combat::handle_key(h) != me) || t.friendly() || t.is_prop() || r.charge_hit.contains(&t.key()) {
                    continue;
                }
                let d = Vec3::new(t.er.0, t.er.1, t.er.2) - p;
                let flat = d.with_y(0.0);
                if flat.length() <= radius + t.radius * crate::scale() + 0.6 && flat.normalize_or_zero().dot(dir) > 0.3 && d.y.abs() < 2.5 {
                    r.charge_hit.push(t.key());
                    hits.push((i, Attack::Bowled, false));
                }
            }
            hits
        });
        if !hits.is_empty() {
            for (i, _, _) in &hits {
                let t = &near[*i];
                crate::impact::tether(Vec3::new(t.er.0, t.er.1 + 1.0, t.er.2));
            }
            combat.deal(player, &near, &hits, tick);
            crate::lakitu::shake(0.5);
        }
    }
    // Mario rides on its back
    let sm = crate::collision::er_to_sm(origin, &HavokPosition(seat_at.x, seat_at.y, seat_at.z, 0.0));
    let yaw = (-fwd.x).atan2(fwd.z);
    worker::call("ride seat", move |_| unsafe {
        sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]);
        sm64::sm64_set_mario_faceangle(id, yaw);
    });
    // (let go first: charmed, it isn't one of Mario's enemies to hit)
    if let Some(at) = crashed {
        off(id, "crashed it into a wall", Some(fwd));
        crash(&mob, at, origin, combat, player, tick);
    }
}

/// It comes to its senses: its team and its own will back, its poise gone a moment (dazed).
fn release(r: &Ride) {
    if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(&r.mob)) {
        chr.team_type = r.team;
        chr.debug_flags.set_disabled_secondary_actions(r.no_attacks);
        chr.debug_flags.set_disabled_movement(r.no_moving);
        chr.modules.action_request.npc_action_id = -1;
        chr.modules.super_armor.sa_durability = 0.0;
    }
}

/// Off its back: a hop off (behind it, `fwd` its facing in Elden Ring, so he doesn't come down on
/// its head), it's let go, the wait starts.
fn off(id: i32, why: &str, fwd: Option<Vec3>) {
    let Some(r) = with(|s| {
        let r = s.ride.take()?;
        s.hud = None;
        s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
        Some(r)
    }) else {
        return;
    };
    release(&r);
    log(format!("boss ride: off ({why})"));
    // (SM64's axes: x the other way round)
    let away = fwd.map(|f| Vec3::new(f.x, 0.0, -f.z).normalize_or_zero());
    worker::call("ride off", move |_| unsafe {
        sm64::sm64_set_mario_action(id, ACT_DOUBLE_JUMP);
        match away {
            Some(a) if a != Vec3::ZERO => {
                sm64::sm64_set_mario_faceangle(id, a.x.atan2(a.z));
                sm64::sm64_set_mario_forward_velocity(id, HOP_OFF);
                sm64::sm64_set_mario_velocity(id, a.x * HOP_OFF, 44.0, a.z * HOP_OFF);
            }
            _ => sm64::sm64_set_mario_velocity(id, 0.0, 44.0, 0.0),
        }
    });
}

/// Driven into a wall: a heavy blow, its stance broken (swing.rs: open for the tail grab).
fn crash(mob: &FieldInsHandle, at: Vec3, origin: [f32; 3], combat: &mut Combat, player: &eldenring::cs::ChrIns, tick: u32) {
    let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), 8.0, origin);
    if let Some(i) = near.iter().position(|t| t.shootable().is_some_and(|h| combat::handle_key(h) == combat::handle_key(mob))) {
        combat.deal(player, &near, &[(i, Attack::RideCrash, false)], tick);
    }
    crate::swing::add_stance(mob, CRASH_STANCE);
    log("boss ride: crashed into a wall, left open");
    crate::style::callout("WALL SLAM!", Some(at + Vec3::Y), [1.0, 0.6, 0.3], true, 6.0);
    crate::impact::tether(at);
    crate::lakitu::shake(1.4);
    worker::call("ride crash", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_CRASH, 0.8) });
}

/// Mario's death, out of Mario mode: off at once, the enemy itself again.
pub fn clear() {
    if let Some(r) = with(|s| {
        s.hud = None;
        s.ride.take()
    }) {
        release(&r);
    }
}
