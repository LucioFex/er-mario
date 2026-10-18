//! Timing: three ways for good timing to pay off.
//! - The Mikiri stomp (Sekiro's Mikiri counter): A just before an enemy's blow lands (it's facing
//!   Mario, close, and its attack is under way) and Mario somersaults over the blow onto its head,
//!   stomps it (it staggers, a boss's stance cracks a little: swing.rs) and bounces off, his air
//!   string ready. Too early, or with no blow coming, A is just a jump.
//! - The action command (Paper Mario's): X right as one of his blows connects (on the impact or
//!   up to three ticks after it, while the hit-stop holds him) is a "NICE!": that blow hits harder.
//!   A third in a row is a "GREAT!". Mashing doesn't count: the press must come after a moment
//!   without one.
//! - The Dash Bonk (Hades' dash strike) lives in er_moves.c and moves.rs: X at the end of the dash.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{FieldInsHandle, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::Attack;
use crate::{log, sm64, worker};

/// an enemy this close (m, off its body) whose attack is between these times (s into its
/// animation: the blow is on its way) can be countered; the next one at least this long after (s)
const MIKIRI_NEAR: f32 = 3.2;
const MIKIRI_FROM: f32 = 0.2;
const MIKIRI_TO: f32 = 0.9;
const MIKIRI_COOLDOWN: f32 = 2.5;
/// enemies' attacks: their animations (a000_003000 on, as dodge.rs), facing Mario (cos 60°)
const ATTACKS: std::ops::Range<i32> = 3000..4000;
const FACING_COS: f32 = 0.5;
/// the stomp lands at most this high on a big body (m): a giant's knee, not its head
const MIKIRI_TOP: f32 = 3.0;
/// the world slows this long after a Mikiri (s), and a boss's stance takes this much (poise)
pub const MIKIRI_SLOW: f32 = 0.45;
pub const MIKIRI_POSTURE: f32 = 30.0;

/// the action command: X this many ticks after a blow landed (0 = on the same tick), with no X in
/// the ticks before it; at most one this often (ticks); a streak ends this long after the last (s)
const NICE_LATE: u32 = 3;
const NICE_CLEAN: u32 = 8;
const NICE_EVERY: u32 = 10;
const NICE_STREAK: f32 = 2.0;
/// the blow's extra share: a "NICE!", a "GREAT!" (the third and on)
pub const NICE: f32 = 0.3;
pub const GREAT: f32 = 0.5;

const SOUND_SHORT_STAR: i32 = 0x3016_0091;
const SOUND_RED_COIN: i32 = 0x7828_9081;

struct State {
    a_held: bool,
    x_held: bool,
    /// the last two X presses (ticks)
    x_at: Option<u32>,
    x_before: Option<u32>,
    /// the last blow that landed: on whom, what, when, where (Elden Ring), and whether it already
    /// got its "NICE!"
    landed: Option<(FieldInsHandle, Attack, u32, Vec3, bool)>,
    nice_at: Option<u32>,
    streak: u32,
    streak_at: Option<Instant>,
    /// the Mikiri: the next one allowed, the enemy it's on (its key) while it lasts
    mikiri_last: Option<Instant>,
    mikiri_on: Option<u64>,
    /// where Mario started it (Elden Ring feet)
    mikiri_from: Vec3,
    misses: u32,
}

static STATE: Mutex<State> = Mutex::new(State {
    a_held: false,
    x_held: false,
    x_at: None,
    x_before: None,
    landed: None,
    nice_at: None,
    streak: 0,
    streak_at: None,
    mikiri_last: None,
    mikiri_on: None,
    mikiri_from: Vec3::ZERO,
    misses: 0,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Game thread, every SM64 tick before Mario's moves: A and X held now. Whether A was just pressed.
pub fn buttons(a: bool, x: bool, tick: u32) -> bool {
    let mut s = state();
    let a_pressed = a && !s.a_held;
    let x_pressed = x && !s.x_held;
    s.a_held = a;
    s.x_held = x;
    if x_pressed {
        s.x_before = s.x_at;
        s.x_at = Some(tick);
    }
    a_pressed
}

/// The press at `at` came after a moment without one (not mashing).
fn clean(s: &State, at: u32) -> bool {
    s.x_before.is_none_or(|b| at.wrapping_sub(b) >= NICE_CLEAN)
}

/// A "NICE!" now: its share, the streak counted. None if it's too soon after the last.
fn nice_now(s: &mut State, tick: u32) -> Option<f32> {
    if s.nice_at.is_some_and(|t| tick.wrapping_sub(t) < NICE_EVERY) {
        return None;
    }
    s.nice_at = Some(tick);
    if s.streak_at.is_none_or(|t| t.elapsed().as_secs_f32() > NICE_STREAK) {
        s.streak = 0;
    }
    s.streak += 1;
    s.streak_at = Some(Instant::now());
    Some(if s.streak >= 3 { GREAT } else { NICE })
}

/// Blows the action command counts: Mario's own up close (no shots from afar, no burn, no steps,
/// no hook, no cyclone's many little hits).
fn counts(attack: Attack) -> bool {
    !attack.ranged() && !matches!(attack, Attack::Trample | Attack::ChompSpin | Attack::Spin | Attack::Mikiri)
}

/// Game thread (combat.rs): one of Mario's blows landed on `handle` (its chest `at`). Its extra
/// share if X came right on it.
pub fn landed(handle: &FieldInsHandle, attack: Attack, at: Vec3, tick: u32) -> Option<f32> {
    if !counts(attack) {
        return None;
    }
    let mut s = state();
    let on_it = s.x_at == Some(tick) && clean(&s, tick);
    let extra = if on_it { nice_now(&mut s, tick) } else { None };
    s.landed = Some((*handle, attack, tick, at, extra.is_some()));
    if let Some(e) = extra {
        drop(s);
        cheer(e, at, attack);
    }
    extra
}

/// Game thread, every SM64 tick after `buttons`: an X pressed just after a blow landed, a "NICE!"
/// on it: the enemy, the blow and its extra share.
pub fn late_nice(tick: u32) -> Option<(FieldInsHandle, Attack, f32)> {
    let mut s = state();
    if s.x_at != Some(tick) || !clean(&s, tick) {
        return None;
    }
    let (handle, attack, at, pos, done) = s.landed?;
    let late = tick.wrapping_sub(at);
    if done || late == 0 || late > NICE_LATE {
        return None;
    }
    let extra = nice_now(&mut s, tick)?;
    if let Some(l) = s.landed.as_mut() {
        l.4 = true;
    }
    drop(s);
    cheer(extra, pos, attack);
    Some((handle, attack, extra))
}

fn cheer(extra: f32, at: Vec3, attack: Attack) {
    let great = extra >= GREAT;
    log(format!("timing: {} on {attack:?} (+{:.0}%)", if great { "GREAT" } else { "NICE" }, extra * 100.0));
    crate::style::nice(at, great);
    worker::call("nice", move |_| unsafe {
        sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.16);
        if great {
            sm64::sm64_play_sound_volume(SOUND_RED_COIN + (5 << 16), 0.1);
        }
    });
}

/// Mario was hurt: the streak of "NICE!"s is over.
pub fn hurt() {
    let mut s = state();
    s.streak = 0;
    s.streak_at = None;
}

/// Game thread, every SM64 tick: A was just pressed, Mario may counter now (on the ground, free,
/// not big, not on Yoshi, not holding anyone), his feet (Elden Ring) and SM64's origin. The
/// enemy's head (SM64 units) to somersault onto, if it's a Mikiri.
pub fn mikiri(a_pressed: bool, allowed: bool, feet: Vec3, origin: [f32; 3]) -> Option<[f32; 3]> {
    if !a_pressed || !allowed {
        return None;
    }
    let mut s = state();
    if s.mikiri_last.is_some_and(|t| t.elapsed().as_secs_f32() < MIKIRI_COOLDOWN) {
        return None;
    }
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let mut early = None;
    let found = crate::combat::in_the_way(feet, MIKIRI_NEAR).into_iter().find_map(|(handle, key, _)| {
        let chr = wcm.chr_ins_by_handle(&handle)?;
        let t = &chr.modules.time_act;
        let a = &t.anim_queue[(t.read_idx % 10) as usize];
        let anim = a.anim_id % 1_000_000;
        if !ATTACKS.contains(&anim) || crate::juggle::is_juggled(&handle) || crate::combat::hands_off(&handle) {
            return None;
        }
        let ph = &chr.modules.physics;
        let (o, p) = (ph.orientation, ph.position);
        let ahead = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(Vec3::new(0.0, 0.0, -1.0)).with_y(0.0).normalize_or_zero();
        let to = (feet - Vec3::new(p.0, p.1, p.2)).with_y(0.0).normalize_or_zero();
        if ahead.dot(to) < FACING_COS {
            return None;
        }
        if !(MIKIRI_FROM..=MIKIRI_TO).contains(&a.play_time) {
            early = Some((chr.character_id, anim, a.play_time));
            return None;
        }
        Some((key, chr.character_id, anim, a.play_time, crate::collision::er_to_sm(origin, &landing(chr, feet))))
    });
    let Some((key, id, anim, at, head)) = found else {
        if let Some((id, anim, t)) = early {
            if s.misses < 20 {
                s.misses += 1;
                log(format!("timing: A near c{id:04}'s attack {anim} at {t:.2} s, outside the Mikiri window ({MIKIRI_FROM}-{MIKIRI_TO} s): a plain jump"));
            }
        }
        return None;
    };
    s.mikiri_last = Some(Instant::now());
    s.mikiri_on = Some(key);
    s.mikiri_from = feet;
    log(format!("timing: Mikiri on c{id:04} (attack {anim} at {at:.2} s)"));
    Some(head)
}

/// Game thread, every SM64 tick while the Mikiri lasts: where its enemy's head is now (SM64 units).
pub fn mikiri_target(origin: [f32; 3]) -> Option<[f32; 3]> {
    let key = state().mikiri_on?;
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &eldenring::cs::ChrIns = chr;
            if crate::combat::handle_key(&chr.field_ins_handle) != key || chr.modules.data.hp <= 0 {
                continue;
            }
            return Some(crate::collision::er_to_sm(origin, &landing(chr, state().mikiri_from)));
        }
    }
    None
}

/// Where the Mikiri lands on `chr`: on its head, at most MIKIRI_TOP up; on a big body (wider than
/// a man) on its near side, toward `from` (Mario's feet, Elden Ring), not inside it.
fn landing(chr: &eldenring::cs::ChrIns, from: Vec3) -> eldenring::position::HavokPosition {
    let ph = &chr.modules.physics;
    let h = ph.hit_height.max(ph.chr_hit_height);
    let h = if h.is_finite() && h > 0.3 { h.min(MIKIRI_TOP) } else { 1.7 };
    let r = ph.hit_radius.max(ph.chr_hit_radius);
    let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let toward = (from - p).with_y(0.0).normalize_or_zero();
    let p = if r.is_finite() && r > BIG_BODY { p + toward * (r - 0.4).min(3.0) } else { p };
    eldenring::position::HavokPosition(p.x, p.y + h, p.z, 0.0)
}
/// wider than this (m), the Mikiri lands on its near side
const BIG_BODY: f32 = 0.8;

/// The Mikiri is over (Mario out of it).
pub fn mikiri_done() {
    state().mikiri_on = None;
}

pub fn clear() {
    let mut s = state();
    s.landed = None;
    s.streak = 0;
    s.streak_at = None;
    s.mikiri_on = None;
}
