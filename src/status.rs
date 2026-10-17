//! Elden Ring's ailments and burns on Mario. They build up as they do on the Tarnished (his resist
//! gauges, which lib.rs used to keep full), and the game does their damage (the HP he loses turns
//! into Mario's wedges as always); Mario shows them the SM64 way:
//! - poison, scarlet rot: he coughs (SM64's toxic gas), green or orange puffs while it drains him
//! - blood loss: a burst of red, the wedge costs no knockback
//! - frostbite: frozen stiff a moment (no control), then shivering, icy rings (no knockback)
//! - sleep: he drops off to sleep (SM64's), buttons wake him sooner
//! - madness: shocked (SM64's electrocution), a yellow flash (no knockback)
//! - a fire on him (a burn the game puts on the Tarnished): SM64's burning run, flames off him
//! Deathblight stays off (its instant death isn't a wedge's worth).
//! An ailment sets in when its gauge runs out: the game fills it back up, so a jump of most of it in
//! one frame is the sign; but a cure or a rest at a grace fills it up too, so it's only taken once
//! the ailment's own damage comes with it (an HP loss for the bursts, an effect that drains for the
//! poisons). Mario's reaction waits for a calm moment (not mid-air, mid-move or knocked back).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::ChrIns;
use glam::Vec3;

use crate::{log, sm64, worker};

/// the gauges in the order of the resistances (the resist module, see lib.rs no_ailments)
pub const POISON: usize = 0;
pub const ROT: usize = 1;
pub const BLEED: usize = 2;
pub const BLIGHT: usize = 3;
pub const FROST: usize = 4;
pub const SLEEP: usize = 5;
pub const MADNESS: usize = 6;
const NAMES: [&str; 7] = ["poison", "scarlet rot", "blood loss", "deathblight", "frostbite", "sleep", "madness"];

/// frozen stiff (s), asleep at most (s) and how much each button press wakes him (s), the poison's
/// puffs (every s, for s after it sets in), the burning run (SM64 ticks), coughing or shivering at
/// most (s: SM64 loops them until the stick moves)
const FROZEN: f32 = 1.1;
const ASLEEP: f32 = 3.0;
const WAKE_PER_PRESS: f32 = 0.35;
const PUFF_EVERY: f32 = 0.5;
const PUFF_FOR: f32 = 8.0;
const BURN_TICKS: i32 = 45;
const IDLE_ACT_FOR: f32 = 2.5;
/// a gauge jump and its damage this close together count as the ailment setting in (s); a
/// reaction is tried this long for a calm moment (s); a burn: a new effect on the Tarnished that
/// hurts over time, lasting this long at most (s), no ailment's this close to it (s)
const CONFIRM: f32 = 0.6;
const REACT_FOR: f32 = 1.5;
const BURN_MAX_LIFE: f32 = 20.0;
const NOT_AN_AILMENT: f32 = 1.0;
/// lava's burns (SM64's lava boost already answers them)
const LAVA_BURNS: [i32; 2] = [4100, 4101];
/// a gap this long between ticks (s: following a door, a grace, a cutscene) and the gauges are read
/// afresh (a rest refilled them meanwhile)
const STALE: f32 = 0.5;

const SOUND_MAMA_MIA: i32 = 0x2420_8081;

/// sm64_er_status kinds
const COUGH: i32 = 1;
const SHIVER: i32 = 2;
const DOZE: i32 = 3;
const WAKE: i32 = 4;
const SHOCK: i32 = 5;
const BURN: i32 = 6;
const STOP_IDLE: i32 = 7;

struct State {
    gauges: Option<[u32; 7]>,
    last_tick: Option<Instant>,
    /// a gauge's jump waiting for its damage, and the ailments that did set in
    jumps: [Option<Instant>; 7],
    procs: [Option<Instant>; 7],
    /// the last HP loss, the last new draining effect
    hp_loss: Option<Instant>,
    new_dot: Option<Instant>,
    /// frozen or asleep until then; asleep, button presses shave it
    held_until: Option<Instant>,
    asleep: bool,
    pressed: bool,
    /// reactions waiting for a calm moment (sm64_er_status kind, since); a cough or shiver going on
    pending: Vec<(i32, Instant)>,
    idle_act: Option<Instant>,
    last_puff: Option<Instant>,
    /// the effects on the Tarnished last tick, and the ones logged once
    effects: Vec<i32>,
    seen: std::collections::HashSet<i32>,
    /// a burn seen (when), waiting a moment to be sure no ailment set in with it
    burn: Option<Instant>,
    burning_since: Option<Instant>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| State {
        gauges: None,
        last_tick: None,
        jumps: [None; 7],
        procs: [None; 7],
        hp_loss: None,
        new_dot: None,
        held_until: None,
        asleep: false,
        pressed: false,
        pending: Vec::new(),
        idle_act: None,
        last_puff: None,
        effects: Vec::new(),
        seen: std::collections::HashSet::new(),
        burn: None,
        burning_since: None,
    }))
}

fn within(t: Option<Instant>, s: f32) -> bool {
    t.is_some_and(|t| t.elapsed().as_secs_f32() < s)
}

/// Blood loss, frostbite or madness set in a moment ago: their wedge costs no knockback (it would
/// knock the reaction away).
pub fn just_set_in() -> bool {
    with(|s| [BLEED, FROST, MADNESS].iter().any(|&i| within(s.procs[i], 0.4)))
}

/// Game thread, with Mario's inputs: frozen stiff or asleep, he doesn't move (true: drop the
/// inputs). `pressed`: a button down this tick (asleep, presses wake him sooner).
pub fn holds(pressed: bool, id: i32) -> bool {
    let woke = with(|s| {
        let Some(mut until) = s.held_until else {
            s.pressed = pressed;
            return None;
        };
        if s.asleep && pressed && !s.pressed {
            until -= std::time::Duration::from_secs_f32(WAKE_PER_PRESS);
        }
        s.pressed = pressed;
        if Instant::now() >= until {
            s.held_until = None;
            return Some(std::mem::take(&mut s.asleep));
        }
        s.held_until = Some(until);
        None
    });
    match woke {
        None => with(|s| s.held_until.is_some()),
        Some(asleep) => {
            if asleep {
                log("status: woke up");
                worker::call("wake", move |_| unsafe { sm64::sm64_er_status(id, WAKE, 0) });
            }
            false
        }
    }
}

/// A hit on Mario: asleep, it wakes him.
pub fn hit() {
    with(|s| {
        if s.asleep {
            s.held_until = Some(Instant::now());
        }
    });
}

/// An ailment's gauge going from `prev` to `now` (of `max`) in a frame: maybe it set in (run out,
/// or filled back up by most of it at once; wearing off, it creeps back up).
fn sets_in(prev: u32, now: u32, max: u32) -> bool {
    (now == 0 && prev > 0) || (now > prev && now - prev > max / 2)
}

/// The ailments' gauges (what's left of each resistance) and their maxima, from the resist module.
fn gauges(player: &ChrIns) -> Option<([u32; 7], [u32; 7])> {
    const GAUGES: usize = 0x10;
    const MAXIMA: usize = 0x2c;
    let module = unsafe { *((&*player.modules as *const _ as usize + 0x20) as *const usize) };
    if !crate::explore::readable(module, 0x50) {
        return None;
    }
    let now = unsafe { *((module + GAUGES) as *const [u32; 7]) };
    let max = unsafe { *((module + MAXIMA) as *const [u32; 7]) };
    (max[0] > 0).then_some((now, max))
}

/// Game thread, every frame: ailments setting in (and burns) on the Tarnished, and Mario's answer.
/// `feet`: Elden Ring; `calm`: Mario may be made to cough, sleep... (not dead, riding, held, big);
/// `hurt`: the Tarnished lost HP this frame.
pub fn tick(player: &ChrIns, id: i32, feet: Vec3, calm: bool, hurt: bool) {
    let now_g = gauges(player);
    burns(player);
    let set_in: Vec<usize> = with(|s| {
        let stale = !within(s.last_tick, STALE);
        s.last_tick = Some(Instant::now());
        if hurt {
            s.hp_loss = Some(Instant::now());
        }
        if let Some((now, max)) = now_g {
            if let (Some(prev), false) = (s.gauges, stale) {
                for i in 0..7 {
                    if i != BLIGHT && sets_in(prev[i], now[i], max[i]) && !within(s.procs[i], 1.5) {
                        s.jumps[i] = Some(Instant::now());
                    }
                }
            }
            s.gauges = Some(now);
        }
        // a jump confirmed by its damage (the poisons by their draining effect; sleep has none)
        let mut out = Vec::new();
        for i in 0..7 {
            let Some(j) = s.jumps[i] else { continue };
            let sign = match i {
                POISON | ROT => s.new_dot,
                SLEEP => Some(j),
                _ => s.hp_loss,
            };
            let close = sign.is_some_and(|t| (t.elapsed().as_secs_f32() - j.elapsed().as_secs_f32()).abs() < CONFIRM);
            if close {
                s.jumps[i] = None;
                s.procs[i] = Some(Instant::now());
                out.push(i);
            } else if j.elapsed().as_secs_f32() > CONFIRM {
                s.jumps[i] = None;
                log(format!("status: {} gauge refilled with no damage (a cure or a rest)", NAMES[i]));
            }
        }
        out
    });
    for &i in &set_in {
        log(format!("status: {} set in", NAMES[i]));
        react(i, feet);
    }
    ignite(feet);
    if calm {
        attempt(id);
    }
    flair(feet, calm);
}

/// What Mario does about an ailment: its flash now, his reaction when he's calm.
fn react(i: usize, feet: Vec3) {
    let queue = |kind: i32| with(|s| s.pending.push((kind, Instant::now())));
    match i {
        POISON | ROT => {
            crate::impact::status_puff(feet + Vec3::Y * 0.9, if i == POISON { [0.45, 0.9, 0.3] } else { [1.0, 0.45, 0.2] });
            queue(COUGH);
        }
        BLEED => {
            crate::impact::bleed(feet + Vec3::Y * 0.8);
            worker::call("bleed", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_MAMA_MIA, 0.6) });
        }
        FROST => {
            crate::impact::frost(feet);
            queue(SHIVER);
        }
        SLEEP => queue(DOZE),
        MADNESS => {
            crate::impact::flash([1.0, 0.85, 0.2], 0.35, 0.4);
            queue(SHOCK);
        }
        _ => {}
    }
}

/// The reactions waiting for a calm moment: tried each tick until they take, for a while.
fn attempt(id: i32) {
    let pending = with(|s| {
        s.pending.retain(|(_, t)| t.elapsed().as_secs_f32() < REACT_FOR);
        s.pending.clone()
    });
    for (kind, since) in pending {
        let took = worker::call("status", move |_| unsafe { sm64::sm64_er_status(id, kind, BURN_TICKS) }).unwrap_or(0) != 0;
        if !took {
            continue;
        }
        with(|s| {
            s.pending.retain(|p| p.1 != since || p.0 != kind);
            let now = Instant::now();
            match kind {
                SHIVER => {
                    s.held_until = Some(now + std::time::Duration::from_secs_f32(FROZEN));
                    s.idle_act = Some(now);
                }
                DOZE => {
                    s.held_until = Some(now + std::time::Duration::from_secs_f32(ASLEEP));
                    s.asleep = true;
                }
                COUGH => s.idle_act = Some(now),
                BURN => s.burning_since = Some(now),
                _ => {}
            }
        });
    }
    // (SM64 coughs and shivers until the stick moves: enough after a while)
    let stop = with(|s| s.idle_act.is_some_and(|t| t.elapsed().as_secs_f32() > IDLE_ACT_FOR) && s.idle_act.take().is_some());
    if stop {
        worker::call("status idle", move |_| unsafe { sm64::sm64_er_status(id, STOP_IDLE, 0) });
    }
}

/// The poison's puffs, the flames off him while he burns.
fn flair(feet: Vec3, calm: bool) {
    with(|s| {
        let tint = [(POISON, [0.45, 0.9, 0.3]), (ROT, [1.0, 0.45, 0.2])].into_iter().find(|(i, _)| within(s.procs[*i], PUFF_FOR)).map(|(_, c)| c);
        if let Some(c) = tint {
            if !within(s.last_puff, PUFF_EVERY) {
                s.last_puff = Some(Instant::now());
                crate::impact::status_puff(feet + Vec3::Y * 0.9, c);
            }
        }
        if calm && within(s.burning_since, BURN_TICKS as f32 / 30.0) && !within(s.last_puff, 0.12) {
            s.last_puff = Some(Instant::now());
            crate::impact::flame_lick(feet + Vec3::Y * (0.4 + crate::rand01() * 0.6));
        }
    });
}

/// New effects on the Tarnished (each id logged once, with what it does): one that hurts over time
/// marks a poison setting in, or (with none) a burn.
fn burns(player: &ChrIns) {
    let mut log_lines = Vec::new();
    with(|s| {
        let mut fresh_dot = false;
        let mut now_ids = std::mem::take(&mut s.effects);
        let before = now_ids.clone();
        now_ids.clear();
        for e in player.special_effect.entries() {
            now_ids.push(e.param_id);
            if before.contains(&e.param_id) {
                continue;
            }
            let Some(p) = e.param_data.map(|p| unsafe { p.as_ref() }) else { continue };
            let hurts = p.change_hp_point() > 0 || p.change_hp_rate() > 0.0;
            let life = p.effect_endurance();
            if (hurts || p.state_info() != 0) && s.seen.len() < 4096 && s.seen.insert(e.param_id) {
                log_lines.push(format!(
                    "status: effect {} (state {}, hp {} / {:.1}%, {:.1} s, vfx {})",
                    e.param_id,
                    p.state_info(),
                    p.change_hp_point(),
                    p.change_hp_rate(),
                    life,
                    p.vfx_id()
                ));
            }
            if hurts && life > 0.0 && life <= BURN_MAX_LIFE && !LAVA_BURNS.contains(&e.param_id) {
                fresh_dot = true;
            }
        }
        s.effects = now_ids;
        if fresh_dot {
            s.new_dot = Some(Instant::now());
            if s.burn.is_none() {
                s.burn = Some(Instant::now());
            }
        }
    });
    for l in log_lines {
        log(l);
    }
}

/// A draining effect that no ailment came with, a moment later: Mario's on fire.
fn ignite(feet: Vec3) {
    let lit = with(|s| {
        let Some(t) = s.burn else { return false };
        if t.elapsed().as_secs_f32() < CONFIRM {
            return false;
        }
        s.burn = None;
        let ailment = [POISON, ROT, BLEED, FROST, MADNESS].iter().any(|&i| within(s.procs[i], NOT_AN_AILMENT) || s.jumps[i].is_some());
        if !ailment {
            s.pending.push((BURN, Instant::now()));
        }
        !ailment
    });
    if lit {
        log("status: burning");
        crate::impact::fire_pop(feet + Vec3::Y * 0.6);
    }
}

pub fn clear() {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::sets_in;

    #[test]
    fn ailment_sets_in() {
        // building up, wearing off: no
        assert!(!sets_in(300, 250, 300));
        assert!(!sets_in(100, 104, 300));
        // run out, or refilled at once: yes
        assert!(sets_in(20, 0, 300));
        assert!(sets_in(20, 300, 300));
        // at rest: no
        assert!(!sets_in(0, 0, 300));
        assert!(!sets_in(300, 300, 300));
    }
}
