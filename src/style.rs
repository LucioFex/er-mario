//! The style meter (Devil May Cry's): Mario's blows in a row, ranked by the string's length (D, C,
//! B, A, S, SS, SSS), shown by hud.rs. It's gone after a while without a blow, or when he's hurt.
//! A new rank chimes (SM64's red coins, a note higher each time) and from S up Mario cheers. A
//! finisher that takes an enemy out, or any blow that does from the B rank up, slows the world
//! down for a moment (Mario keeps his pace: SM64's sound runs on his ticks).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::CSFlipper;
use fromsoftware_shared::FromStatic;

use crate::{sm64, worker};

/// a string ends this long after its last blow (s), and its count fades out over this long
const KEEP: f32 = 2.5;
const FADE: f32 = 0.6;
/// blows for each rank
pub const RANKS: [(u32, &str); 7] = [(3, "D"), (8, "C"), (15, "B"), (25, "A"), (40, "S"), (60, "SS"), (90, "SSS")];
/// the rank from which every kill slows the world (B)
const SLOW_RANK: usize = 2;
/// the slow motion: how long (real s), how slow, and the share of it spent easing back
const SLOW_TIME: f32 = 0.35;
const SLOW_SPEED: f32 = 0.3;
const SLOW_EASE: f32 = 0.4;

/// SM64 sounds: the red coin chime (its ID goes up a note per coin), the cheers, a kill's poof
const SOUND_RED_COIN: i32 = 0x7828_9081;
const SOUND_YAHOO_WAHA_YIPPEE: i32 = 0x242B_8081;
const SOUND_HERE_WE_GO: i32 = 0x240C_8081;
const SOUND_STAR: i32 = 0x701E_FF81;
const SOUND_DEFEAT: i32 = 0x502C_8081;

struct Meter {
    count: u32,
    last: Instant,
    rank: Option<usize>,
    /// when the rank last went up
    ranked: Instant,
}

static METER: Mutex<Option<Meter>> = Mutex::new(None);
static SLOW: Mutex<Option<Instant>> = Mutex::new(None);

fn rank_of(count: u32) -> Option<usize> {
    RANKS.iter().rposition(|&(n, _)| count >= n)
}

fn sounds(list: Vec<i32>) {
    worker::call("style", move |_| {
        for s in list {
            unsafe { sm64::sm64_play_sound_global(s) };
        }
    });
}

/// Game thread: one of Mario's blows landed on an enemy.
pub fn hit() {
    let mut m = METER.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    let m = match m.as_mut() {
        Some(m) if m.last.elapsed().as_secs_f32() <= KEEP => m,
        _ => m.insert(Meter { count: 0, last: now, rank: None, ranked: now }),
    };
    m.count += 1;
    m.last = now;
    let rank = rank_of(m.count);
    if rank > m.rank {
        m.rank = rank;
        m.ranked = now;
        let r = rank.unwrap_or(0);
        let mut list = vec![SOUND_RED_COIN + ((r as i32) << 16)];
        match r {
            4 => list.push(SOUND_YAHOO_WAHA_YIPPEE),
            5 => list.push(SOUND_YAHOO_WAHA_YIPPEE + (2 << 16)),
            6 => list.extend([SOUND_HERE_WE_GO, SOUND_STAR]),
            _ => {}
        }
        crate::log(format!("style: {} ({} hits)", RANKS[r].1, m.count));
        sounds(list);
    }
}

/// Game thread: Mario was hurt, the string is lost.
pub fn hurt() {
    *METER.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Game thread: a blow of Mario's took an enemy down (`finisher`: one of the combos' finishers).
pub fn kill(finisher: bool) {
    let rank = METER.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.rank);
    sounds(vec![SOUND_DEFEAT]);
    if finisher || rank.is_some_and(|r| r >= SLOW_RANK) {
        *SLOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }
}

/// What hud.rs draws: the count, its rank, how fresh the last blow and the rank are (1 just now,
/// down to 0 in a moment) and how visible it all is (fading out at the end).
pub struct Show {
    pub count: u32,
    pub rank: Option<usize>,
    pub pop: f32,
    pub rank_pop: f32,
    pub alpha: f32,
}

pub fn show() -> Option<Show> {
    let m = METER.lock().unwrap_or_else(|e| e.into_inner());
    let m = m.as_ref()?;
    let since = m.last.elapsed().as_secs_f32();
    let alpha = if since <= KEEP { 1.0 } else { 1.0 - (since - KEEP) / FADE };
    if alpha <= 0.0 || m.count < 2 {
        return None;
    }
    let fresh = |t: f32, len: f32| (1.0 - t / len).max(0.0);
    Some(Show { count: m.count, rank: m.rank, pop: fresh(since, 0.15), rank_pop: fresh(m.ranked.elapsed().as_secs_f32(), 0.35), alpha })
}

/// Game thread, every frame: the slow motion's game speed (back to normal once it's over, and
/// only then: someone else's speed is left alone).
pub fn update() {
    static SET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let speed = match *slow {
        Some(t) => {
            let p = t.elapsed().as_secs_f32() / SLOW_TIME;
            if p >= 1.0 {
                *slow = None;
                None
            } else if p < 1.0 - SLOW_EASE {
                Some(SLOW_SPEED)
            } else {
                Some(SLOW_SPEED + (1.0 - SLOW_SPEED) * (p - (1.0 - SLOW_EASE)) / SLOW_EASE)
            }
        }
        None => None,
    };
    let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else { return };
    match speed {
        Some(s) => {
            flipper.game_speed = s;
            SET.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        None if SET.swap(false, std::sync::atomic::Ordering::Relaxed) => flipper.game_speed = 1.0,
        None => {}
    }
}

/// Mario's step this frame (s): the game's own, or in the slow motion the real time since the last
/// frame (he isn't slowed down with the world).
pub fn mario_dt(game_dt: f32) -> f32 {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let now = Instant::now();
    let real = LAST.lock().unwrap_or_else(|e| e.into_inner()).replace(now).map_or(game_dt, |t| now.duration_since(t).as_secs_f32());
    if SLOW.lock().unwrap_or_else(|e| e.into_inner()).is_some() { real.min(0.25) } else { game_dt }
}

pub fn clear() {
    *METER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *SLOW.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
