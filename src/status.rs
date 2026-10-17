//! Elden Ring's ailments and burns on Mario. They build up as they do on the Tarnished (his resist
//! gauges, which lib.rs used to keep full), and the game does their damage (the HP he loses turns
//! into Mario's wedges as always); Mario shows them the SM64 way:
//! - poison, scarlet rot: he coughs (SM64's toxic gas), green or orange puffs while it drains him
//! - blood loss: a burst of red, the wedge costs no knockback
//! - frostbite: frozen stiff a moment (no control), then shivering, icy rings
//! - sleep: he drops off to sleep (SM64's), buttons wake him sooner
//! - madness: shocked (SM64's electrocution), a yellow flash
//! - a fire on him (a burn the game puts on the Tarnished): SM64's burning run, flames behind him
//! Deathblight stays off (its instant death isn't a wedge's worth). An ailment sets in when its
//! gauge runs out: the game fills it back up, so a jump of most of it in one frame is the sign.

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
/// puffs (every s, for s after it sets in), the burning run (SM64 ticks)
const FROZEN: f32 = 1.1;
const ASLEEP: f32 = 3.0;
const WAKE_PER_PRESS: f32 = 0.35;
const PUFF_EVERY: f32 = 0.5;
const PUFF_FOR: f32 = 8.0;
const BURN_TICKS: i32 = 45;
/// a burn: a new effect on the Tarnished that hurts over time, lasting this long at most (s), and
/// none of the ailments set in this close to it (s)
const BURN_MAX_LIFE: f32 = 20.0;
const NOT_AN_AILMENT: f32 = 1.0;

const SOUND_MAMA_MIA: i32 = 0x2420_8081;
const SOUND_SNORE: i32 = 0x240E_0081;

struct State {
    gauges: Option<[u32; 7]>,
    procs: [Option<Instant>; 7],
    /// frozen or asleep until then; asleep, button presses shave it
    held_until: Option<Instant>,
    asleep: bool,
    pressed: bool,
    last_puff: Option<Instant>,
    /// the effects on the Tarnished last frame, and the ones logged once
    effects: Vec<i32>,
    seen: Vec<i32>,
    /// a burn seen (when), waiting a moment to be sure no ailment set in with it
    burn: Option<Instant>,
    burning_since: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    gauges: None,
    procs: [None; 7],
    held_until: None,
    asleep: false,
    pressed: false,
    last_puff: None,
    effects: Vec::new(),
    seen: Vec::new(),
    burn: None,
    burning_since: None,
});

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    f(&mut STATE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Blood loss set in a moment ago: its wedge costs no knockback.
pub fn just_bled() -> bool {
    with(|s| s.procs[BLEED].is_some_and(|t| t.elapsed().as_secs_f32() < 0.4))
}

/// Game thread, with Mario's inputs: frozen stiff or asleep, he doesn't move (true: drop the
/// inputs). `pressed`: a button down this tick (asleep, presses wake him sooner).
pub fn holds(pressed: bool, id: i32) -> bool {
    with(|s| {
        let Some(until) = s.held_until else {
            s.pressed = pressed;
            return false;
        };
        let mut until = until;
        if s.asleep && pressed && !s.pressed {
            until -= std::time::Duration::from_secs_f32(WAKE_PER_PRESS);
        }
        s.pressed = pressed;
        if Instant::now() >= until {
            s.held_until = None;
            if std::mem::take(&mut s.asleep) {
                log("status: woke up");
                worker::call("wake", move |_| unsafe { sm64::sm64_er_status(id, 4, 0) });
            }
            return false;
        }
        s.held_until = Some(until);
        true
    })
}

/// An ailment's gauge going from `prev` to `now` (of `max`) in a frame: it set in (run out, or
/// filled back up by most of it at once; wearing off, it creeps back up).
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
/// `feet`: Elden Ring; `calm`: Mario may be made to cough, sleep... (not dead, riding, held, big).
pub fn tick(player: &ChrIns, id: i32, feet: Vec3, calm: bool) {
    let now_g = gauges(player);
    let mut set_in = Vec::new();
    with(|s| {
        if let Some((now, max)) = now_g {
            if let Some(prev) = s.gauges {
                for i in 0..7 {
                    let fresh = s.procs[i].is_none_or(|t| t.elapsed().as_secs_f32() > 1.5);
                    if sets_in(prev[i], now[i], max[i]) && fresh && i != BLIGHT {
                        s.procs[i] = Some(Instant::now());
                        set_in.push(i);
                    }
                }
            }
            s.gauges = Some(now);
        }
    });
    for &i in &set_in {
        log(format!("status: {} set in", NAMES[i]));
        react(i, id, feet, calm);
    }
    burns(player, id, feet, calm);
    // the poison's and rot's puffs while they last
    with(|s| {
        let tint = [(POISON, [0.45, 0.9, 0.3]), (ROT, [1.0, 0.45, 0.2])]
            .into_iter()
            .find(|(i, _)| s.procs[*i].is_some_and(|t| t.elapsed().as_secs_f32() < PUFF_FOR))
            .map(|(_, c)| c);
        if let Some(c) = tint {
            if s.last_puff.is_none_or(|t| t.elapsed().as_secs_f32() > PUFF_EVERY) {
                s.last_puff = Some(Instant::now());
                crate::impact::status_puff(feet + Vec3::Y * 0.9, c);
            }
        }
        // burning: flames off him
        if s.burning_since.is_some_and(|t| t.elapsed().as_secs_f32() < BURN_TICKS as f32 / 30.0) {
            if s.last_puff.is_none_or(|t| t.elapsed().as_secs_f32() > 0.12) {
                s.last_puff = Some(Instant::now());
                crate::impact::flame_lick(feet + Vec3::Y * (0.4 + crate::rand01() * 0.6));
            }
        }
    });
}

fn react(i: usize, id: i32, feet: Vec3, calm: bool) {
    match i {
        POISON | ROT => {
            if calm {
                worker::call("cough", move |_| unsafe { sm64::sm64_er_status(id, 1, 0) });
            }
            crate::impact::status_puff(feet + Vec3::Y * 0.9, if i == POISON { [0.45, 0.9, 0.3] } else { [1.0, 0.45, 0.2] });
        }
        BLEED => {
            crate::impact::bleed(feet + Vec3::Y * 0.8);
            worker::call("bleed", |_| unsafe { sm64::sm64_play_sound_global(SOUND_MAMA_MIA) });
        }
        FROST => {
            crate::impact::frost(feet);
            if calm {
                with(|s| s.held_until = Some(Instant::now() + std::time::Duration::from_secs_f32(FROZEN)));
                worker::call("shiver", move |_| unsafe { sm64::sm64_er_status(id, 2, 0) });
            }
        }
        SLEEP => {
            if calm {
                with(|s| {
                    s.held_until = Some(Instant::now() + std::time::Duration::from_secs_f32(ASLEEP));
                    s.asleep = true;
                });
                worker::call("sleep", move |_| unsafe {
                    sm64::sm64_er_status(id, 3, 0);
                    sm64::sm64_play_sound_global(SOUND_SNORE);
                });
            }
        }
        MADNESS => {
            crate::impact::flash([1.0, 0.85, 0.2], 0.35, 0.4);
            if calm {
                worker::call("shocked", move |_| unsafe { sm64::sm64_er_status(id, 5, 0) });
            }
        }
        _ => {}
    }
}

/// New effects on the Tarnished (each id logged once, with what it does); one that hurts over time
/// and isn't an ailment's is a burn: Mario catches fire.
fn burns(player: &ChrIns, id: i32, feet: Vec3, calm: bool) {
    let mut fresh_dot = false;
    let mut now_ids = Vec::new();
    for e in player.special_effect.entries() {
        now_ids.push(e.param_id);
        let new = with(|s| !s.effects.contains(&e.param_id));
        if !new {
            continue;
        }
        let Some(p) = e.param_data.map(|p| unsafe { p.as_ref() }) else { continue };
        let hurts = p.change_hp_point() > 0 || p.change_hp_rate() > 0.0;
        let life = p.effect_endurance();
        let first = with(|s| {
            let first = !s.seen.contains(&e.param_id);
            if first && s.seen.len() < 512 {
                s.seen.push(e.param_id);
            }
            first
        });
        if first && (hurts || p.state_info() != 0) {
            log(format!(
                "status: effect {} (state {}, hp {} / {:.1}%, {:.1} s, vfx {})",
                e.param_id,
                p.state_info(),
                p.change_hp_point(),
                p.change_hp_rate(),
                life,
                p.vfx_id()
            ));
        }
        // (the lava's burn is SM64's lava boost already)
        if hurts && life > 0.0 && life <= BURN_MAX_LIFE && e.param_id != 4101 {
            fresh_dot = true;
        }
    }
    let ignite = with(|s| {
        s.effects = now_ids;
        if fresh_dot && s.burn.is_none() {
            s.burn = Some(Instant::now());
        }
        // (a moment's wait: an ailment setting in with it makes it the ailment's, not a fire)
        let Some(t) = s.burn else { return false };
        if t.elapsed().as_secs_f32() < 0.25 {
            return false;
        }
        s.burn = None;
        let ailment = [POISON, ROT, BLEED, FROST, MADNESS].iter().any(|&i| s.procs[i].is_some_and(|p| p.elapsed().as_secs_f32() < NOT_AN_AILMENT));
        if ailment {
            return false;
        }
        s.burning_since = Some(Instant::now());
        true
    });
    if ignite {
        log("status: burning");
        crate::impact::fire_pop(feet + Vec3::Y * 0.6);
        if calm {
            worker::call("burn", move |_| unsafe { sm64::sm64_er_status(id, 6, BURN_TICKS) });
        }
    }
}

pub fn clear() {
    with(|s| {
        s.gauges = None;
        s.held_until = None;
        s.asleep = false;
        s.burn = None;
        s.burning_since = None;
    });
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
