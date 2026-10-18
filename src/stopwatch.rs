//! The Stopwatch (Zelda's Stasis, Super Mario Bros. 2's): LT held while crouching + L1 stops the
//! enemy in front of Mario for a moment (the game's own "no updates" on it: it stands frozen mid
//! move). Blows on it meanwhile build up its momentum (each the way Mario hit it); when time runs
//! again a regular enemy flies off with all of it (bat.rs, bowling over whoever it meets), a big
//! one or a boss staggers. Bosses stop for a shorter while; it comes back slowly.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat;
use crate::{log, sm64, worker};

/// how long it holds a regular enemy, a big one, a boss (s); the wait before the next (s)
const HOLD: f32 = 3.0;
const HOLD_BIG: f32 = 2.0;
const HOLD_BOSS: f32 = 1.2;
const RECHARGE: f32 = 25.0;
/// the enemy it takes: in front (cos 60°) within this far (m)
const REACH: f32 = 12.0;
/// a blow's momentum (m/s of flight) by how heavy it is, the most it flies at, the least to fly
const PUSH: f32 = 2.2;
const PUSH_HEAVY: f32 = 5.0;
const FLY_MAX: f32 = 24.0;
const FLY_MIN: f32 = 4.0;

const SOUND_TICK: i32 = 0x8055_F011u32 as i32;
const SOUND_TICK_FAST: i32 = 0x8054_F011u32 as i32;
const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_STOP: i32 = 0x3016_0091;

struct Frozen {
    mob: FieldInsHandle,
    since: Instant,
    hold: f32,
    push: Vec3,
    boss: bool,
    last_tick: f32,
}

struct State {
    frozen: Option<Frozen>,
    ready_at: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { frozen: None, ready_at: None });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn key(h: &FieldInsHandle) -> u64 {
    combat::handle_key(h)
}

fn set_frozen(h: &FieldInsHandle, on: bool) {
    if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(h)) {
        chr.debug_flags.set_disabled_updates(on);
    }
}

/// For the HUD: the share of its recharge done (1 ready).
pub fn ready() -> f32 {
    state().ready_at.map_or(1.0, |t| 1.0 - t.saturating_duration_since(Instant::now()).as_secs_f32() / RECHARGE)
}

/// `h` is stopped in time now (its blows build up momentum, it isn't finished meanwhile).
pub fn frozen(h: &FieldInsHandle) -> bool {
    state().frozen.as_ref().is_some_and(|f| key(&f.mob) == key(h))
}

/// Game thread: LT + L1 while crouching. Stops the enemy in front of Mario (feet and facing, Elden
/// Ring) if there's one and it's ready; a buzz otherwise.
pub fn use_it(feet: Vec3, facing: Vec3, origin: [f32; 3]) {
    let mut s = state();
    // (co-op guest: the host owns the enemies, one stopped here would only be stopped here)
    let ready = s.ready_at.is_none_or(|t| Instant::now() >= t) && s.frozen.is_none() && !crate::coop::guest();
    let target = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), REACH, origin)
        .iter()
        .filter_map(|t| {
            let h = *t.shootable()?;
            let d = Vec3::new(t.er.0 - feet.x, 0.0, t.er.2 - feet.z);
            (d.length() > 0.2 && d.normalize().dot(facing) >= 0.5 && !combat::hands_off(&h) && !crate::bat::is_flying(&h)).then_some((d.length(), h, t.radius * crate::scale(), Vec3::new(t.er.0, t.er.1 + 1.6, t.er.2)))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let (Some((_, mob, radius, chest)), true) = (target, ready) else {
        drop(s);
        worker::call("stopwatch buzz", |_| unsafe { sm64::sm64_play_sound_global(SOUND_BUZZ) });
        return;
    };
    let boss = combat::is_boss_now(&mob) || !combat::liftable(&mob);
    let hold = if combat::is_boss_now(&mob) { HOLD_BOSS } else if boss || radius > 0.9 { HOLD_BIG } else { HOLD };
    crate::juggle::release(&mob);
    set_frozen(&mob, true);
    s.frozen = Some(Frozen { mob, since: Instant::now(), hold, push: Vec3::ZERO, boss, last_tick: 0.0 });
    drop(s);
    log(format!("stopwatch: an enemy stopped for {hold} s{}", if boss { " (big or a boss)" } else { "" }));
    crate::style::callout("STOP!", Some(chest + Vec3::Y * 0.4), [0.6, 0.8, 1.0], true, 2.0);
    crate::impact::stopwatch(chest, true);
    worker::call("stopwatch", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_STOP, 0.4) });
}

/// Game thread (combat.rs): a blow on the stopped enemy, from `from` (Mario's feet, Elden Ring):
/// its momentum builds up.
pub fn struck(h: &FieldInsHandle, from: Vec3, at: Vec3, heavy: bool) {
    let mut s = state();
    let Some(f) = s.frozen.as_mut().filter(|f| key(&f.mob) == key(h)) else { return };
    let dir = (at - from).with_y(0.0).normalize_or(Vec3::Z);
    f.push += dir * if heavy { PUSH_HEAVY } else { PUSH };
    // (each blow a ring on it: the momentum showing)
    crate::impact::stopwatch(at, false);
}

/// Game thread, every frame: the clock ticking (faster at the end), and time running again.
pub fn update() {
    let mut s = state();
    let Some(f) = s.frozen.as_mut() else { return };
    let t = f.since.elapsed().as_secs_f32();
    let alive = unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.chr_ins_by_handle(&f.mob)).is_some_and(|c| c.modules.data.hp > 0);
    // (tick... tock, quicker in its last second)
    let every = if f.hold - t < 1.0 { 0.17 } else { 0.4 };
    if t - f.last_tick >= every {
        f.last_tick = t;
        let fast = f.hold - t < 1.0;
        worker::call("stopwatch tick", move |_| unsafe { sm64::sm64_play_sound_volume(if fast { SOUND_TICK_FAST } else { SOUND_TICK }, 0.18) });
    }
    if t < f.hold && alive {
        return;
    }
    let f = s.frozen.take().unwrap();
    s.ready_at = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE));
    drop(s);
    set_frozen(&f.mob, false);
    let speed = f.push.length().min(FLY_MAX);
    let chest = unsafe { WorldChrMan::instance() }
        .ok()
        .and_then(|w| w.chr_ins_by_handle(&f.mob))
        .map(|c| {
            let p = c.modules.physics.position;
            Vec3::new(p.0, p.1 + 1.2, p.2)
        })
        .unwrap_or(Vec3::ZERO);
    crate::impact::stopwatch(chest, true);
    if !alive {
        return;
    }
    if !f.boss && speed >= FLY_MIN {
        log(format!("stopwatch: time runs again, it flies off at {speed:.0} m/s"));
        crate::bat::launch(&f.mob, f.push, speed);
        crate::style::callout("RELEASE!", Some(chest + Vec3::Y * 0.6), [0.6, 0.8, 1.0], true, 3.0);
    } else {
        log("stopwatch: time runs again");
        if f.boss && speed >= FLY_MIN {
            // (a boss doesn't fly: its stance cracks with what built up)
            crate::swing::add_stance(&f.mob, speed * 2.5);
        }
    }
}

/// Mario's death, out of Mario mode, a load: the enemy runs again at once.
pub fn clear() {
    let mut s = state();
    if let Some(f) = s.frozen.take() {
        drop(s);
        set_frozen(&f.mob, false);
    }
}
