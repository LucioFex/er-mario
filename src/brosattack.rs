//! Co-op: the Bros. Attack (Mario & Luigi's). A streak of BROS! (both on one enemy, coop.rs) READY
//! long and that enemy is marked for a while (both brothers' faces over its head). A Bros. Jump off
//! the partner's head then (lib.rs) doesn't just bounce: it flips up and over to the marked enemy,
//! the world slows at the top, and it comes down on it in a ground pound, homing in: a big blow
//! (Attack::BrosAttack), rings in both brothers' colours, stars, a hard shake. The partner, the one
//! jumped off, gets BROS POWER for a few seconds (their blows up close hit harder). Then a wait
//! before the next one. Alone none of this runs: the mark needs a partner's blows.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::sm64::SM64MarioState;
use crate::{coop, log, moves, worker};

/// a streak of this many BROS! marks the enemy
pub const READY: u32 = 3;
/// the mark lasts this long (s); the leap reaches an enemy this far from where it starts (m, flat)
pub const MARK_TIME: f32 = 8.0;
const REACH: f32 = 12.0;
/// the next one only this long after one (s)
const COOLDOWN: f32 = 25.0;
/// the leap: up this fast (SM64 units per tick; SM64's gravity takes 4 a tick: its top 17 ticks
/// on, 5 m up), so many ticks to its top
const LEAP_UP: f32 = 66.0;
const LEAP_TICKS: u32 = 17;
/// the most it goes forward a tick (SM64 units: ~12 m in all)
const LEAP_FORWARD_MAX: f32 = 110.0;
/// the world slowed at its top, and as it lands (s)
const SLOW_TOP: f32 = 0.35;
const SLOW_HIT: f32 = 0.5;
/// coming down: this share of the way to the enemy (flat) a tick, from this close (m)
const HOME: f32 = 0.35;
const HOME_REACH: f32 = 6.0;
/// the blow lands on the enemy this close to where he comes down (m, its own width on top)
const HIT_REACH: f32 = 2.4;
/// a leap that hasn't landed in this long is over (s)
const FLIGHT_MAX: f32 = 3.5;
/// BROS POWER: the partner's blows up close this much harder for this long (s)
pub const POWER: f32 = 1.3;
const POWER_TIME: f32 = 5.0;
/// style points for the attack, and the partner's for the power
const POINTS: f32 = 6.0;
const POWER_POINTS: f32 = 2.0;

/// SM64's actions
const ACT_GROUND_POUND: u32 = 0x0080_08A9;
const ACT_FLAG_AIR: u32 = 0x0000_0800;
/// a star's twinkle (the mark, the power), a quake's thud and "here we go!" (the blow)
const SOUND_SHORT_STAR: i32 = 0x3016_0091;
const SOUND_POUND: i32 = 0x312F_0081;
const SOUND_HERE_WE_GO: i32 = 0x240C_8081;

struct Mark {
    key: u64,
    handle: FieldInsHandle,
    at: Instant,
    /// the top of its head now (the game thread follows it)
    head: Option<Vec3>,
}

#[derive(PartialEq)]
enum Phase {
    Up,
    Down,
}

struct Flight {
    key: u64,
    handle: FieldInsHandle,
    since: Instant,
    phase: Phase,
    /// seen going up (the leap's taken: its top is when that stops)
    rose: bool,
    /// the partner jumped off (their BROS POWER)
    base: u64,
}

struct State {
    mark: Option<Mark>,
    flight: Option<Flight>,
    /// none before this (the wait after one)
    next: Option<Instant>,
    power_until: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { mark: None, flight: None, next: None, power_until: None });

fn handle_of(key: u64) -> FieldInsHandle {
    unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) }
}

/// An enemy alive: the top of its head and its chest (Elden Ring), its width (m).
fn body(h: &FieldInsHandle) -> Option<(Vec3, Vec3, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 1 {
        return None;
    }
    let ph = &chr.modules.physics;
    let height = ph.hit_height.max(ph.chr_hit_height).clamp(0.5, 12.0);
    let radius = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.2, 6.0);
    let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    p.is_finite().then(|| (p + Vec3::Y * height, p + Vec3::Y * height * 0.55, radius))
}

/// coop.rs: a BROS! streak of `n` on the enemy `key`: from READY on, it's marked (if none is on its
/// way and the wait is over).
pub fn streak(key: u64, n: u32) {
    if n < READY || !coop::active() {
        return;
    }
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.flight.is_some() || s.next.is_some_and(|t| Instant::now() < t) {
        return;
    }
    let fresh = s.mark.as_ref().is_none_or(|m| m.key != key);
    s.mark = Some(Mark { key, handle: handle_of(key), at: Instant::now(), head: None });
    if fresh {
        log(format!("bros: BROS ATTACK ready on {key:x} (a Bros. Jump off the partner's head)"));
        // (played by frame(): this comes in under coop.rs's lock, and the call waits for the SM64
        // thread)
        CHIME.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// the mark's chime to play
static CHIME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Game thread, every frame: the mark follows its enemy's head, and goes when it's down or old.
pub fn frame() {
    if CHIME.swap(false, std::sync::atomic::Ordering::Relaxed) {
        worker::call("bros ready", |_| unsafe { crate::sm64::sm64_play_sound_local(SOUND_SHORT_STAR, 0.25) });
    }
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(m) = s.mark.as_mut() else { return };
    match body(&m.handle) {
        Some((head, ..)) if m.at.elapsed().as_secs_f32() < MARK_TIME && coop::active() => m.head = Some(head),
        _ => s.mark = None,
    }
}

/// For the HUD: the marked enemy's head (Elden Ring) and how long it's been marked (s).
pub fn mark() -> Option<(Vec3, f32)> {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.mark.as_ref().filter(|_| coop::active()).and_then(|m| m.head.map(|h| (h, m.at.elapsed().as_secs_f32())))
}

/// An enemy is marked: the Bros. Jump takes its partner's head more readily.
pub fn marked() -> bool {
    coop::active() && STATE.lock().unwrap_or_else(|e| e.into_inner()).mark.as_ref().is_some_and(|m| m.head.is_some())
}

/// lib.rs, the Bros. Jump off the partner's head (`head`, Elden Ring), Mario's feet at `feet`: with
/// an enemy marked within reach, the leap at it instead of the bounce.
pub fn leap(feet: Vec3, head: Vec3, base: u64, flip: bool) -> Option<moves::Request> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let m = s.mark.as_ref()?;
    let (top, ..) = body(&m.handle)?;
    let d = (top - feet).with_y(0.0);
    let flat = d.length();
    if flat > REACH {
        log(format!("bros: the marked enemy is {flat:.1} m off: just the Bros. Jump"));
        return None;
    }
    let (key, handle) = (m.key, m.handle.clone());
    s.mark = None;
    s.flight = Some(Flight { key, handle, since: Instant::now(), phase: Phase::Up, rose: false, base });
    s.next = Some(Instant::now() + std::time::Duration::from_secs_f32(COOLDOWN));
    drop(s);
    // (SM64 is mirrored on X: its yaw from the flat way there)
    let yaw = (-d.x).atan2(d.z);
    let forward = (leap_speed(flat / crate::scale())).min(LEAP_FORWARD_MAX);
    log(format!("bros: BROS ATTACK! leap at {key:x}, {flat:.1} m off (forward {forward:.0})"));
    crate::impact::bros_leap(head);
    crate::style::callout("BROS ATTACK!", Some(top + Vec3::Y * 0.4), [1.0, 0.85, 0.35], true, 0.0);
    coop::send_bros(coop::BROS_LAUNCH, key, top, base);
    Some(moves::Request::Leap { up: LEAP_UP, forward, yaw, flip })
}

/// How far a leap at `f0` forward goes by its top (SM64 units): SM64's air drag takes 0.35 a tick
/// off, and 1 more off a speed over 32, before each step.
fn covered(f0: f32) -> f32 {
    (0..LEAP_TICKS)
        .fold((0.0, f0), |(x, f), _| {
            let f = (f - 0.35).max(0.0);
            let f = if f > 32.0 { f - 1.0 } else { f };
            (x + f, f)
        })
        .0
}

/// The forward speed (SM64 units a tick) that covers `dist` units by the leap's top.
fn leap_speed(dist: f32) -> f32 {
    let (mut lo, mut hi) = (0.0f32, 200.0f32);
    for _ in 0..30 {
        let mid = (lo + hi) * 0.5;
        if covered(mid) < dist { lo = mid } else { hi = mid }
    }
    hi
}

/// Game thread, every SM64 tick (lib.rs, after the blows): a leap on its way: at its top the
/// ground pound, coming down homing in on the enemy, and on landing the blow.
pub fn tick(id: i32, state: &SM64MarioState, origin: [f32; 3], combat: &mut Combat, player: &ChrIns, tick: u32) {
    enum Do {
        Nothing,
        Pound,
        Home([f32; 3]),
        Land(u64, FieldInsHandle, u64),
    }
    let feet = {
        let k = crate::scale();
        Vec3::new(origin[0] - state.position[0] * k, origin[1] + state.position[1] * k, origin[2] + state.position[2] * k)
    };
    let (todo, target) = {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(f) = s.flight.as_mut() else { return };
        let target = body(&f.handle);
        let air = state.action & ACT_FLAG_AIR != 0;
        let age = f.since.elapsed().as_secs_f32();
        let todo = match f.phase {
            Phase::Up if age > FLIGHT_MAX || (f.rose && !air) || target.is_none() || (!f.rose && age > 0.4) => {
                log(format!("bros: the leap ended on its way up (action {:#x}, enemy {})", state.action, if target.is_some() { "there" } else { "gone" }));
                s.flight = None;
                Do::Nothing
            }
            Phase::Up if state.velocity[1] > 10.0 => {
                f.rose = true;
                Do::Nothing
            }
            Phase::Up if f.rose && state.velocity[1] <= 0.0 => {
                f.phase = Phase::Down;
                Do::Pound
            }
            Phase::Up => Do::Nothing,
            // (the ground pound over: on the ground, or onto the enemy's body, a stomp's bounce;
            // where he is says whether it struck)
            Phase::Down if state.action != ACT_GROUND_POUND || age > FLIGHT_MAX => {
                let f = s.flight.take().expect("in flight");
                Do::Land(f.key, f.handle, f.base)
            }
            Phase::Down => match target {
                // (homing in: a share of the way to the enemy, flat, each tick)
                Some((top, ..)) if (top - feet).with_y(0.0).length() > 0.15 && (top - feet).with_y(0.0).length() < HOME_REACH => {
                    let step = (top - feet).with_y(0.0) * HOME;
                    let k = crate::scale();
                    Do::Home([state.position[0] - step.x / k, state.position[1], state.position[2] + step.z / k])
                }
                _ => Do::Nothing,
            },
        };
        (todo, target)
    };
    // (out of the lock: these wait for the SM64 thread)
    match todo {
        Do::Nothing => {}
        Do::Pound => {
            crate::style::slow_down(SLOW_TOP);
            worker::call("bros pound", move |_| unsafe { crate::sm64::sm64_set_mario_action(id, ACT_GROUND_POUND) });
        }
        Do::Home([x, y, z]) => {
            worker::call("bros home", move |_| unsafe { crate::sm64::sm64_set_mario_position(id, x, y, z) });
        }
        Do::Land(key, handle, base) => land(key, &handle, base, feet, target, state.action, combat, player, origin, tick),
    }
}

/// The ground pound came down at `feet`: the blow on the marked enemy if it's there.
#[allow(clippy::too_many_arguments)]
fn land(key: u64, handle: &FieldInsHandle, base: u64, feet: Vec3, target: Option<(Vec3, Vec3, f32)>, action: u32, combat: &mut Combat, player: &ChrIns, origin: [f32; 3], tick: u32) {
    let Some((top, chest, radius)) = target else {
        log("bros: landed, the enemy's gone");
        return;
    };
    let flat = (chest - feet).with_y(0.0).length();
    let below = chest.y - (top.y - chest.y) / 0.45 * 0.55;
    if flat > HIT_REACH + radius || feet.y < below - 1.5 || feet.y > top.y + 1.5 {
        log(format!("bros: came down {flat:.1} m off the enemy ({:.1} m over its feet, action {action:#x}): missed", feet.y - below));
        return;
    }
    let near = combat::nearby(&HavokPosition(chest.x, chest.y, chest.z, 0.0), 3.0, origin);
    let hits: Vec<(usize, Attack, bool)> = near.iter().enumerate().filter(|(_, t)| t.key() == key && t.shootable().is_some()).map(|(i, _)| (i, Attack::BrosAttack, false)).collect();
    if hits.is_empty() {
        log("bros: landed on it, but it can't be hit now");
        return;
    }
    combat.deal(player, &near, &hits, tick);
    log(format!("bros: BROS ATTACK landed on {key:x}"));
    crate::impact::bros_attack(chest, feet);
    crate::style::slow_down(SLOW_HIT);
    crate::style::team(POINTS);
    coop::bros_spent();
    coop::send_bros(coop::BROS_HIT, combat::handle_key(handle), chest, base);
    worker::call("bros hit", |_| unsafe {
        crate::sm64::sm64_play_sound_volume(SOUND_POUND, 0.8);
        crate::sm64::sm64_play_sound_volume(SOUND_HERE_WE_GO, 0.7);
    });
}

/// coop.rs, game thread: a partner's Bros. Attack: its leap (the callout over the enemy, our mark
/// gone: it's theirs), its blow (the world slowed for us too, a shake; BROS POWER if `ours`: we
/// were the springboard).
pub fn partner_phase(from: u64, phase: u8, key: u64, pos: Vec3, ours: bool) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.mark = None;
    s.next = Some(Instant::now() + std::time::Duration::from_secs_f32(COOLDOWN));
    match phase {
        coop::BROS_LAUNCH => {
            drop(s);
            log(format!("bros: partner {from:x} leaps off us at {key:x}"));
            crate::style::callout("BROS ATTACK!", Some(pos + Vec3::Y * 0.4), [1.0, 0.85, 0.35], true, 0.0);
        }
        coop::BROS_HIT if !ours => {
            drop(s);
            crate::style::slow_down(SLOW_HIT * 0.5);
            coop::bros_spent();
        }
        coop::BROS_HIT => {
            s.power_until = Some(Instant::now() + std::time::Duration::from_secs_f32(POWER_TIME));
            drop(s);
            log(format!("bros: partner {from:x} landed the Bros. Attack: BROS POWER for {POWER_TIME} s"));
            crate::style::slow_down(SLOW_HIT * 0.7);
            crate::lakitu::shake(0.8);
            crate::style::callout("BROS POWER!", None, [0.45, 1.0, 0.5], true, POWER_POINTS);
            worker::call("bros power", |_| unsafe { crate::sm64::sm64_play_sound_local(SOUND_SHORT_STAR, 0.35) });
            coop::bros_spent();
        }
        _ => {}
    }
}

/// BROS POWER on: our blows up close hit POWER times harder (combat.rs).
pub fn powered() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).power_until.is_some_and(|t| Instant::now() < t)
}

/// Out of a session, Mario mode off, a death: nothing marked, nothing on its way.
pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.mark = None;
    s.flight = None;
    s.power_until = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_leap_gets_there_by_its_top() {
        for dist in [100.0, 500.0, 900.0] {
            let f = leap_speed(dist);
            assert!((covered(f) - dist).abs() < 2.0, "{dist}: {f} covers {}", covered(f));
        }
        // a slow one, under the drag of 1: distance over time, with the 0.35 a tick on top
        let f = leap_speed(170.0);
        assert!((f - (10.0 + 0.35 * 9.0)).abs() < 0.1, "{f}");
    }

    #[test]
    fn alone_nothing_is_marked() {
        streak(0x1234, 9);
        assert!(mark().is_none());
        assert!(!marked());
        assert!(!powered());
    }
}
