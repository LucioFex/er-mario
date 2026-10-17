//! The perfect dodge (Bayonetta's Witch Time): B's dash started while an enemy close by is in the
//! middle of one of its attacks slows the world down for a moment (Mario keeps his own speed,
//! style.rs), with a blue flash, a ring and a "waha!"; his blows in that moment are counters and hit
//! harder (combat.rs). Once in a while, not on every dash near an enemy.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::WorldChrMan;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::{log, sm64, worker};

/// an enemy this close (m, off its body) in an attack: the dash's first this many ticks dodge it
const NEAR: f32 = 2.8;
const WINDOW: u32 = 6;
/// the world slowed this long (s); the counters last this long (s) and hit this much harder; the
/// next perfect dodge at least this long after (s)
const SLOW_TIME: f32 = 1.0;
const COUNTER_TIME: f32 = 1.6;
pub const COUNTER_DAMAGE: f32 = 1.6;
const COOLDOWN: f32 = 2.5;
/// enemies' attacks: their animations (a000_003000 on, Elden Ring's enemy attacks)
const ATTACKS: std::ops::Range<i32> = 3000..4000;

const SOUND_WAHA: i32 = 0x242B_8081 + (1 << 16);
const SOUND_SHORT_STAR: i32 = 0x3016_0091;

struct State {
    dash_since: Option<u32>,
    last: Option<Instant>,
    counter_until: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { dash_since: None, last: None, counter_until: None });

/// Mario's blows are counters right now (a perfect dodge a moment ago).
pub fn countering() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).counter_until.is_some_and(|t| Instant::now() < t)
}

/// Game thread, every SM64 tick: Mario's action and feet (Elden Ring). True when it was a perfect
/// dodge.
pub fn tick(action: u32, feet: Vec3, tick: u32) -> bool {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !crate::moves::dodging(action) {
        s.dash_since = None;
        return false;
    }
    let since = *s.dash_since.get_or_insert(tick);
    if tick.wrapping_sub(since) > WINDOW || s.last.is_some_and(|t| t.elapsed().as_secs_f32() < COOLDOWN) {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let attacker = crate::combat::in_the_way(feet, NEAR).into_iter().find_map(|(handle, ..)| {
        let chr = wcm.chr_ins_by_handle(&handle)?;
        let t = &chr.modules.time_act;
        let anim = t.anim_queue[(t.read_idx % 10) as usize].anim_id % 1_000_000;
        ATTACKS.contains(&anim).then_some((chr.character_id, anim))
    });
    let Some((id, anim)) = attacker else { return false };
    let now = Instant::now();
    s.last = Some(now);
    s.counter_until = Some(now + std::time::Duration::from_secs_f32(COUNTER_TIME));
    // (once: the rest of this dash can't dodge again)
    s.dash_since = Some(tick.wrapping_sub(WINDOW + 1));
    drop(s);
    log(format!("dodge: perfect dodge (c{id:04}, attack {anim}), witch time"));
    crate::style::witch_time(SLOW_TIME);
    crate::impact::perfect_dodge(feet);
    worker::call("perfect dodge", |_| unsafe {
        sm64::sm64_play_sound_global(SOUND_WAHA);
        sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.35);
    });
    true
}

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.dash_since = None;
    s.counter_until = None;
}
