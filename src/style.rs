//! The style meter (Devil May Cry's): Mario's blows in a row, ranked (D, C, B, A, S, SS, SSS) by
//! the string's style points, shown by hud.rs with a word for each rank (DANDY ... WAHOO). Every
//! blow is a point, a finisher more, and a blow of another kind than the last a bonus: a varied
//! string climbs faster than the same punch again and again. It's gone after a while without a
//! blow, or when he's hurt ("MAMMA MIA" from the C rank up).
//! A new rank chimes softly (SM64's red coins, a note higher each time) and from S up Mario cheers.
//! From the S rank a combo's finisher is a star finish: stars and coins burst out, a golden flash,
//! the world slows down. A finisher that takes an enemy out, or any blow that does from the B rank
//! up, slows the world down for a moment too (Mario keeps his pace: SM64's sound runs on his ticks).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::CSFlipper;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::Attack;
use crate::{sm64, worker};

/// a string ends this long after its last blow (s), and its count fades out over this long
pub const KEEP: f32 = 2.5;
const FADE: f32 = 0.6;
/// style points for each rank, its letters and its word
pub const RANKS: [(f32, &str, &str); 7] = [
    (3.0, "D", "DANDY"),
    (9.0, "C", "COOL"),
    (17.0, "B", "BONKERS"),
    (28.0, "A", "AWESOME"),
    (44.0, "S", "SUPER"),
    (64.0, "SS", "SUPER STAR"),
    (92.0, "SSS", "WAHOO"),
];
/// a blow's points: one, a finisher's more, and a bonus for a blow of a new kind
const POINTS: f32 = 1.0;
const FINISHER_POINTS: f32 = 1.5;
const VARIETY_POINTS: f32 = 0.6;
/// a weapon switched in the middle of a string
const SWAP_POINTS: f32 = 1.2;
const STAR_POINTS: f32 = 3.0;
/// the rank from which every kill slows the world (B), and from which finishers are star ones (S)
const SLOW_RANK: usize = 2;
const STAR_RANK: usize = 4;
/// at least this long between two star finishes (s)
const STAR_GAP: f32 = 1.2;
/// the slow motion: how long (real s), how slow, and the share of it spent easing back
const SLOW_TIME: f32 = 0.35;
const STAR_SLOW_TIME: f32 = 0.55;
const SLOW_SPEED: f32 = 0.3;
const SLOW_EASE: f32 = 0.4;
/// how long the "MAMMA MIA" and the star finish's banner show (s)
pub const LOST_TIME: f32 = 1.1;
pub const STAR_TIME: f32 = 1.2;
/// the rank-up chime's volume (it was loud over a fight; 0.15 still a bit loud, 2026-10-18)
const CHIME_VOLUME: f32 = 0.09;

/// SM64 sounds: the red coin chime (its ID goes up a note per coin), the cheers, a kill's poof
const SOUND_RED_COIN: i32 = 0x7828_9081;
const SOUND_YAHOO_WAHA_YIPPEE: i32 = 0x242B_8081;
const SOUND_HERE_WE_GO: i32 = 0x240C_8081;
const SOUND_YAHOO: i32 = 0x2404_8081;
const SOUND_STAR: i32 = 0x701E_FF81;
const SOUND_DEFEAT: i32 = 0x502C_8081;
const SOUND_STAR_APPEARS: i32 = 0x3057_FF91;

struct Meter {
    count: u32,
    points: f32,
    last_kind: Option<std::mem::Discriminant<Attack>>,
    last: Instant,
    rank: Option<usize>,
    /// when the rank last went up
    ranked: Instant,
}

static METER: Mutex<Option<Meter>> = Mutex::new(None);
static SLOW: Mutex<Option<(Instant, f32)>> = Mutex::new(None);
/// the "MAMMA MIA" (when, and the rank lost), the last star finish
static LOST: Mutex<Option<(Instant, usize)>> = Mutex::new(None);
static STAR: Mutex<Option<Instant>> = Mutex::new(None);

fn rank_of(points: f32) -> Option<usize> {
    RANKS.iter().rposition(|&(n, ..)| points >= n)
}

fn sounds(list: Vec<(i32, f32)>) {
    worker::call("style", move |_| {
        for (s, v) in list {
            unsafe {
                if v < 1.0 {
                    sm64::sm64_play_sound_volume(s, v);
                } else {
                    sm64::sm64_play_sound_global(s);
                }
            }
        }
    });
}

fn slow(time: f32) {
    let mut s = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    // (a longer one isn't cut short by a shorter one)
    if s.is_none_or(|(t, len)| t.elapsed().as_secs_f32() > len - time) {
        *s = Some((Instant::now(), time));
    }
}

/// The perfect dodge's witch time (dodge.rs): the world slowed this long (s), Mario not.
pub fn witch_time(time: f32) {
    slow(time);
}

/// The world slowed this long (s) for a big moment (a MAX finisher, maxfin.rs).
pub fn slow_down(time: f32) {
    slow(time);
}

/// MAX finishers (maxfin.rs): this many blows in a row without being hurt and the next finisher
/// is a MAX one.
pub const MAX_HITS: u32 = 12;
struct Max {
    streak: u32,
    ready: bool,
}
static MAX: Mutex<Max> = Mutex::new(Max { streak: 0, ready: false });

/// A MAX finisher is ready (the weapon glows).
pub fn max_ready() -> bool {
    MAX.lock().unwrap_or_else(|e| e.into_inner()).ready
}

/// Uses the MAX finisher up: true if it was ready (then the streak starts over).
pub fn take_max() -> bool {
    let mut m = MAX.lock().unwrap_or_else(|e| e.into_inner());
    let ready = std::mem::replace(&mut m.ready, false);
    if ready {
        m.streak = 0;
    }
    ready
}

fn max_hit() {
    let mut m = MAX.lock().unwrap_or_else(|e| e.into_inner());
    m.streak += 1;
    if m.streak >= MAX_HITS && !m.ready {
        m.ready = true;
        drop(m);
        crate::log(format!("style: MAX ready ({MAX_HITS} blows without a hit)"));
        callout("MAX READY", None, [1.0, 0.75, 0.2], false, 0.0);
        sounds(vec![(SOUND_RED_COIN + (7 << 16), CHIME_VOLUME)]);
    }
}

/// The current rank, if the string is still going.
pub fn rank() -> Option<usize> {
    METER.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|m| m.last.elapsed().as_secs_f32() <= KEEP).and_then(|m| m.rank)
}

fn add(m: &mut Meter, points: f32) {
    m.points += points;
    let rank = rank_of(m.points);
    if rank > m.rank {
        m.rank = rank;
        m.ranked = Instant::now();
        let r = rank.unwrap_or(0);
        let mut list = vec![(SOUND_RED_COIN + ((r as i32) << 16), CHIME_VOLUME)];
        match r {
            4 => list.push((SOUND_YAHOO_WAHA_YIPPEE, 1.0)),
            5 => list.push((SOUND_YAHOO_WAHA_YIPPEE + (2 << 16), 1.0)),
            6 => list.extend([(SOUND_HERE_WE_GO, 1.0), (SOUND_STAR, 0.3)]),
            _ => {}
        }
        crate::log(format!("style: {} {} ({} hits, {:.1} points)", RANKS[r].1, RANKS[r].2, m.count, m.points));
        sounds(list);
    }
}

/// Game thread: one of Mario's blows landed on an enemy.
pub fn hit(attack: Attack) {
    let mut m = METER.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    let m = match m.as_mut() {
        Some(m) if m.last.elapsed().as_secs_f32() <= KEEP => m,
        _ => m.insert(Meter { count: 0, points: 0.0, last_kind: None, last: now, rank: None, ranked: now }),
    };
    m.count += 1;
    m.last = now;
    max_hit();
    let kind = std::mem::discriminant(&attack);
    let mut points = POINTS;
    if attack.finisher() {
        points += FINISHER_POINTS;
    }
    if m.last_kind.is_some_and(|k| k != kind) {
        points += VARIETY_POINTS;
    }
    m.last_kind = Some(kind);
    add(m, points);
}

/// Game thread: Mario switched weapons in the middle of a string (moves.rs's swap strike): a
/// bonus, if the string is still going.
pub fn swap() {
    let mut m = METER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = m.as_mut().filter(|m| m.last.elapsed().as_secs_f32() <= KEEP) {
        m.last = Instant::now();
        add(m, SWAP_POINTS);
    }
}

/// Game thread: a combo's finisher landed at `at` (Elden Ring, the target's chest). From the S
/// rank it's a star finish: true then (impact.rs shows it).
pub fn finisher(at: Vec3) -> bool {
    let rank = {
        let mut m = METER.lock().unwrap_or_else(|e| e.into_inner());
        let Some(m) = m.as_mut().filter(|m| m.last.elapsed().as_secs_f32() <= KEEP) else { return false };
        let Some(rank) = m.rank.filter(|&r| r >= STAR_RANK) else { return false };
        let mut star = STAR.lock().unwrap_or_else(|e| e.into_inner());
        if star.is_some_and(|t| t.elapsed().as_secs_f32() < STAR_GAP) {
            return false;
        }
        *star = Some(Instant::now());
        add(m, STAR_POINTS);
        rank
    };
    crate::log(format!("style: star finish at {}", RANKS[rank].1));
    crate::impact::star_finish(at, rank);
    let mut list = vec![(SOUND_STAR_APPEARS, 0.7)];
    match rank {
        5 => list.push((SOUND_HERE_WE_GO, 1.0)),
        6 => list.push((SOUND_YAHOO, 1.0)),
        _ => {}
    }
    sounds(list);
    slow(STAR_SLOW_TIME);
    true
}

/// A word that pops out of a blow, over the enemy it landed on ("NICE!", "MIKIRI!", the
/// skillshots): its text, where (Elden Ring; none: the middle of the screen), its colour, big or not.
#[derive(Clone)]
pub struct Callout {
    pub text: &'static str,
    pub at: Option<Vec3>,
    pub tint: [f32; 3],
    pub big: bool,
    pub age: f32,
}

/// how long a callout shows (s), how many at once
pub const CALLOUT_TIME: f32 = 0.9;
const CALLOUTS_MAX: usize = 4;
static CALLOUTS: Mutex<Vec<(Instant, &'static str, Option<Vec3>, [f32; 3], bool)>> = Mutex::new(Vec::new());

/// Game thread: a word pops out at `at` (style points with it).
pub fn callout(text: &'static str, at: Option<Vec3>, tint: [f32; 3], big: bool, points: f32) {
    {
        let mut c = CALLOUTS.lock().unwrap_or_else(|e| e.into_inner());
        c.retain(|(t, ..)| t.elapsed().as_secs_f32() < CALLOUT_TIME);
        if c.len() >= CALLOUTS_MAX {
            c.remove(0);
        }
        c.push((Instant::now(), text, at, tint, big));
    }
    if points > 0.0 {
        let mut m = METER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = m.as_mut().filter(|m| m.last.elapsed().as_secs_f32() <= KEEP) {
            m.last = Instant::now();
            add(m, points);
        }
    }
}

/// The callouts to draw now.
pub fn callouts() -> Vec<Callout> {
    let c = CALLOUTS.lock().unwrap_or_else(|e| e.into_inner());
    c.iter()
        .map(|(t, text, at, tint, big)| Callout { text, at: *at, tint: *tint, big: *big, age: t.elapsed().as_secs_f32() })
        .filter(|c| c.age < CALLOUT_TIME)
        .collect()
}

/// The action command landed (timing.rs): "NICE!" (or "GREAT!") over the enemy.
pub fn nice(at: Vec3, great: bool) {
    if great {
        callout("GREAT!", Some(at), [1.0, 0.55, 0.2], true, NICE_POINTS * 2.0);
    } else {
        callout("NICE!", Some(at), [1.0, 0.9, 0.3], false, NICE_POINTS);
    }
    crate::impact::nice(at, great);
}

/// a "NICE!"'s style points, a Mikiri's
const NICE_POINTS: f32 = 1.0;
pub const MIKIRI_POINTS: f32 = 2.5;

/// Game thread: Mario was hurt, the string is lost.
pub fn hurt() {
    {
        let mut m = MAX.lock().unwrap_or_else(|e| e.into_inner());
        if m.ready {
            crate::log("style: MAX lost (hurt)");
        }
        *m = Max { streak: 0, ready: false };
    }
    let lost = METER.lock().unwrap_or_else(|e| e.into_inner()).take().filter(|m| m.last.elapsed().as_secs_f32() <= KEEP).and_then(|m| m.rank);
    if let Some(r) = lost.filter(|&r| r >= 1) {
        *LOST.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), r));
    }
}

/// Game thread: a blow of Mario's took an enemy down (`finisher`: one of the combos' finishers).
pub fn kill(finisher: bool) {
    sounds(vec![(SOUND_DEFEAT, 1.0)]);
    if finisher || rank().is_some_and(|r| r >= SLOW_RANK) {
        slow(SLOW_TIME);
    }
}

/// What hud.rs draws: the count, its rank, how fresh the last blow and the rank are (1 just now,
/// down to 0 in a moment), how visible it all is (fading out at the end), how much of the string's
/// time is left (1 just after a blow), how old the rank and the star finish are (s), and a lost rank.
pub struct Show {
    pub count: u32,
    pub rank: Option<usize>,
    pub pop: f32,
    pub rank_age: f32,
    pub alpha: f32,
    pub left: f32,
    pub star_age: Option<f32>,
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
    let rank_age = m.ranked.elapsed().as_secs_f32();
    let star_age = STAR.lock().unwrap_or_else(|e| e.into_inner()).map(|t| t.elapsed().as_secs_f32()).filter(|&t| t < STAR_TIME);
    Some(Show {
        count: m.count,
        rank: m.rank,
        pop: fresh(since, 0.15),
        rank_age,
        alpha,
        left: (1.0 - since / KEEP).clamp(0.0, 1.0),
        star_age,
    })
}

/// The "MAMMA MIA" after a lost string: its age (s) and the rank lost.
pub fn lost() -> Option<(f32, usize)> {
    LOST.lock().unwrap_or_else(|e| e.into_inner()).map(|(t, r)| (t.elapsed().as_secs_f32(), r)).filter(|(t, _)| *t < LOST_TIME)
}

/// Game thread, every frame: the slow motion's game speed (back to normal once it's over, and
/// only then: someone else's speed is left alone).
pub fn update() {
    static SET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let speed = match *slow {
        Some((t, len)) => {
            let p = t.elapsed().as_secs_f32() / len;
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
    CALLOUTS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *MAX.lock().unwrap_or_else(|e| e.into_inner()) = Max { streak: 0, ready: false };
    *SLOW.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *LOST.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *STAR.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
