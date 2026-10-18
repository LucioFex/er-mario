//! SM64's special caps: Metal, Wing and Vanish, on the pad's D-pad (SM64's own pickup animation,
//! music and timer), each with a recharge after it comes off:
//! - D-pad left: Metal (no hit hurts or knocks him back, his blows hit harder; SM64's heavy steps)
//! - D-pad up: Wing (triple jump to fly, like in SM64)
//! - D-pad down: off
//! (D-pad right was the Vanish cap: it's the Mega Mushroom now, mega.rs)

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use crate::assets::{MODEL_MARIO, MODEL_METAL, MODEL_WING};
use crate::{log, sm64, worker};

pub const METAL: u8 = 0;
pub const WING: u8 = 1;
pub const VANISH: u8 = 2;
const NAMES: [&str; 3] = ["metal", "wing", "vanish"];
/// SM64's cap flags
const FLAG: [u32; 3] = [0x4, 0x8, 0x2];
/// how long each lasts (SM64 ticks, 30 a second): SM64's 20 s, the wing cap shorter than its 60
const TICKS: [u16; 3] = [600, 1200, 600];
/// seconds before it can be worn again, from when it comes off
const RECHARGE: [f32; 3] = [60.0, 55.0, 60.0];
/// the coins each cap costs (the Metal Cap is retired: free)
const COST: [u32; 3] = [0, crate::coins::COST_WING, 0];
/// the blows of Metal Mario hit this much harder
pub const METAL_DAMAGE: f32 = 1.6;
/// the cap music's volume, and while a boss's health bar shows (the game's own music matters)
const MUSIC_VOLUME: f32 = 0.15;
const MUSIC_VOLUME_BOSS: f32 = 0.05;
/// Mario's opacity with the Vanish cap (it shimmers a little around this)
pub const VANISH_OPACITY: f32 = 0.15;

const SOUND_BUZZ: i32 = 0x700E_0081;

/// the cap on now (0xFF: none), from SM64's flags every tick
static ACTIVE: AtomicU8 = AtomicU8::new(0xFF);

struct State {
    /// when each cap can be worn again
    ready_at: [Option<Instant>; 3],
    /// D-pad last tick (up, down, left, right)
    held: [bool; 4],
    /// the cap music volume set last
    volume: f32,
}

static STATE: Mutex<State> = Mutex::new(State { ready_at: [None; 3], held: [false; 4], volume: -1.0 });

/// The cap Mario wears now.
pub fn active() -> Option<u8> {
    let a = ACTIVE.load(Ordering::Relaxed);
    (a < 3).then_some(a)
}

/// Elden Ring's hits do nothing to Mario now (Metal, or Vanish).
pub fn shielded() -> bool {
    active().is_some_and(|c| c != WING)
}

pub fn clear() {
    ACTIVE.store(0xFF, Ordering::Relaxed);
}

/// For the HUD, per cap: the share of its recharge done (1: ready).
pub fn hud() -> [f32; 3] {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    std::array::from_fn(|c| {
        state.ready_at[c].map_or(1.0, |t| {
            let left = t.saturating_duration_since(Instant::now()).as_secs_f32();
            1.0 - left / RECHARGE[c]
        })
    })
}

/// Game thread, every SM64 tick: SM64's cap flags, the D-pad (up, down, left, right), whether he
/// can change caps now (not in a menu, alive), whether a boss's health bar shows. The cap he
/// changed to this tick (`Some(None)`: back to his own), for its puff (impact.rs).
pub fn tick(id: i32, flags: u32, dpad: [bool; 4], allowed: bool, boss: bool) -> Option<Option<u8>> {
    // the cap on now; one just gone starts recharging
    let before = active();
    let now = (0..3u8).find(|&c| flags & FLAG[c as usize] != 0);
    ACTIVE.store(now.unwrap_or(0xFF), Ordering::Relaxed);
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = before.filter(|&c| Some(c) != now) {
        state.ready_at[c as usize] = Some(Instant::now() + std::time::Duration::from_secs_f32(RECHARGE[c as usize]));
    }
    // Mario's look: Metal Mario's model, the wing cap's, or his own (Vanish: see-through, lib.rs)
    let changed = (now != before).then_some(now);
    if now != before {
        crate::equip::set_look(match now {
            Some(METAL) => MODEL_METAL,
            Some(WING) => MODEL_WING,
            _ => MODEL_MARIO,
        });
    }
    // the cap music stays low, lower in a boss fight
    let volume = if boss { MUSIC_VOLUME_BOSS } else { MUSIC_VOLUME };
    if volume != state.volume {
        state.volume = volume;
        worker::call("cap music volume", move |_| unsafe { sm64::sm64_er_set_music_volume(volume) });
    }
    let pressed: [bool; 4] = std::array::from_fn(|i| dpad[i] && !state.held[i]);
    state.held = dpad;
    if !allowed {
        return changed;
    }
    if pressed[1] && now.is_some() {
        worker::call("cap off", move |_| unsafe { sm64::sm64_er_end_cap(id, 0) });
        return changed;
    }
    // D-pad: left Metal, up Wing
    let kind = if pressed[2] {
        METAL
    } else if pressed[0] {
        WING
    } else {
        return changed;
    };
    let ready = state.ready_at[kind as usize].is_none_or(|t| Instant::now() >= t);
    drop(state);
    // (no cap on Mega Mario: Lakitu's buzz)
    if now == Some(kind) || !ready || crate::mega::active() || crate::mega::changing() || !crate::coins::pay(COST[kind as usize], NAMES[kind as usize]) {
        worker::call("cap buzz", |_| unsafe { sm64::sm64_play_sound_global(SOUND_BUZZ) });
        return changed;
    }
    let (flag, ticks) = (FLAG[kind as usize], TICKS[kind as usize]);
    worker::call("cap", move |_| unsafe {
        // (another cap comes off first, its music with it)
        sm64::sm64_er_end_cap(id, 1);
        sm64::sm64_mario_interact_cap(id, flag, ticks, 1);
    });
    log(format!("caps: {} cap on", NAMES[kind as usize]));
    changed
}
