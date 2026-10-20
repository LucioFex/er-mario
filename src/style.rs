//! The style meter (Devil May Cry's): Mario's blows in a row, ranked (D, C, B, A, S, SS, SSS) by
//! the string's style points, shown by hud.rs with a word for each rank (DANDY ... WAHOO). Every
//! blow is a point, a finisher more, and a blow of a kind not used in the last few a bonus. The
//! same kind again and again is worth less each time (and so is hammering one enemy), the points
//! drain away faster the higher the rank, and being hurt costs the string and shakes the next
//! seconds: S and up are earned with variety, finishers, timing and keeping at it. It's gone after
//! a while without a blow, or when he's hurt ("MAMMA MIA" from the C rank up).
//! A new rank chimes softly (SM64's red coins, a note higher each time) and from S up Mario cheers.
//! From the S rank a combo's finisher is a star finish: stars and coins burst out, a golden flash,
//! the world slows down. A finisher that takes an enemy out, or any blow that does from the B rank
//! up, slows the world down for a moment too (Mario keeps his pace: SM64's sound runs on his ticks).

use std::collections::HashMap;
use std::mem::Discriminant;
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
    (30.0, "A", "AWESOME"),
    (56.0, "S", "SUPER"),
    (88.0, "SS", "SUPER STAR"),
    (130.0, "SSS", "WAHOO"),
];
/// a blow's points: one, a finisher's more, and a bonus for a blow of a kind not among the last few
const POINTS: f32 = 1.0;
const FINISHER_POINTS: f32 = 1.5;
const VARIETY_POINTS: f32 = 0.6;
/// the last blows that count for "a new kind"
const VARIETY_RECENT: usize = 4;
/// a blow's kind used again within KIND_WINDOW (s) is worth KIND_FADE of its points per use
/// before (down to KIND_FLOOR): the same punch again and again climbs slowly
const KIND_WINDOW: f32 = 6.0;
const KIND_FADE: f32 = 0.8;
const KIND_FLOOR: f32 = 0.3;
/// the blows on one enemy within TARGET_WINDOW (s) past the first TARGET_FREE are each worth
/// TARGET_FADE of the one before (down to TARGET_FLOOR)
const TARGET_WINDOW: f32 = 8.0;
const TARGET_FREE: u32 = 6;
const TARGET_FADE: f32 = 0.92;
const TARGET_FLOOR: f32 = 0.5;
/// style points lost a second at each rank (D .. SSS) while the string goes on
const DRAIN: [f32; 7] = [0.0, 0.0, 0.4, 1.0, 2.2, 3.6, 5.5];
/// a rank is only lost with the points this far under its mark (so it doesn't flicker)
const RANK_HOLD: f32 = 0.94;
/// every point is worth this much for SHAKEN_TIME (s) after being hurt; the knock-on blows of the
/// MAX shows and the like are worth WEAK_SCALE
const SHAKEN_SCALE: f32 = 0.6;
const SHAKEN_TIME: f32 = 4.0;
const WEAK_SCALE: f32 = 0.25;
/// a weapon switched in the middle of a string
const SWAP_POINTS: f32 = 1.2;
const STAR_POINTS: f32 = 2.0;
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

/// Seconds on the style meter's clock (the tests set their own).
fn now() -> f32 {
    #[cfg(test)]
    {
        TEST_NOW.with(|t| t.get())
    }
    #[cfg(not(test))]
    {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f32()
    }
}
#[cfg(test)]
thread_local!(static TEST_NOW: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) });

struct Meter {
    count: u32,
    points: f32,
    last_kind: Option<Discriminant<Attack>>,
    /// the kinds of the last blows and when, the blows on each enemy (when)
    recent: Vec<(Discriminant<Attack>, f32)>,
    targets: HashMap<u64, Vec<f32>>,
    last: f32,
    rank: Option<usize>,
    /// the best rank of the string (the chime only sounds for a new one)
    peak: Option<usize>,
    /// when the rank last went up, and the points drained up to
    ranked: f32,
    drained: f32,
}

impl Meter {
    fn new(now: f32) -> Self {
        Meter { count: 0, points: 0.0, last_kind: None, recent: Vec::new(), targets: HashMap::new(), last: now, rank: None, peak: None, ranked: now, drained: now }
    }

    fn alive(&self, now: f32) -> bool {
        now - self.last <= KEEP
    }

    /// The points drained since the last time (the higher the rank, the faster), the rank
    /// following the points down.
    fn settle(&mut self, now: f32) {
        let dt = (now - self.drained).max(0.0);
        self.drained = now;
        if let Some(r) = self.rank.filter(|_| self.alive(now)) {
            self.points = (self.points - DRAIN[r] * dt).max(0.0);
            self.rerank(now);
        }
    }

    /// The rank the points say, a rank held a little under its mark.
    fn rerank(&mut self, now: f32) {
        let held = self.rank.filter(|&r| self.points >= RANKS[r].0 * RANK_HOLD);
        let rank = rank_of(self.points).max(held);
        if rank != self.rank {
            if rank > self.rank {
                self.ranked = now;
            }
            self.rank = rank;
        }
    }

    /// Points of one blow of `attack` on enemy `target` (before the after-a-hurt and MAX knock-on scaling).
    fn blow(&mut self, now: f32, attack: Attack, target: u64) -> f32 {
        let kind = std::mem::discriminant(&attack);
        self.recent.retain(|&(_, t)| now - t <= KIND_WINDOW);
        let used = self.recent.iter().filter(|&&(k, _)| k == kind).count() as i32;
        let new_kind = !self.recent.iter().rev().take(VARIETY_RECENT).any(|&(k, _)| k == kind);
        self.recent.push((kind, now));
        let mut points = POINTS * KIND_FADE.powi(used).max(KIND_FLOOR);
        if attack.finisher() {
            points += FINISHER_POINTS;
        }
        if new_kind && self.last_kind.is_some() {
            points += VARIETY_POINTS;
        }
        self.last_kind = Some(kind);
        // (the blows on this one within the window, sliding: older ones don't count)
        if self.targets.len() > 16 {
            self.targets.retain(|_, at| at.last().is_some_and(|&t| now - t <= TARGET_WINDOW));
        }
        let on = self.targets.entry(target).or_default();
        on.retain(|&t| now - t <= TARGET_WINDOW);
        on.push(now);
        let n = on.len() as u32;
        points * TARGET_FADE.powi(n.saturating_sub(TARGET_FREE) as i32).max(TARGET_FLOOR)
    }

    /// Points in (already scaled): the rank it takes to, if that's a new best.
    fn gain(&mut self, now: f32, points: f32) -> Option<usize> {
        self.points += points;
        let before = self.rank;
        self.rerank(now);
        let rank = self.rank.filter(|_| self.rank > before && self.rank > self.peak)?;
        self.peak = self.rank;
        Some(rank)
    }
}

/// When Mario was last hurt (the style clock), for the shaken seconds after.
static HURT_AT: Mutex<f32> = Mutex::new(-100.0);

/// What every point is worth now: less just after being hurt.
fn scale() -> f32 {
    let mut s = 1.0;
    if now() - *HURT_AT.lock().unwrap_or_else(|e| e.into_inner()) < SHAKEN_TIME {
        s *= SHAKEN_SCALE;
    }
    s
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
/// a MAX finisher ready keeps this long (s) from when it got ready, blows or none: used or lost
/// (Devil Trigger's way; kept by every blow, in a long fight its twinkle at his hand never ended)
pub const MAX_KEEP: f32 = 12.0;
struct Max {
    streak: u32,
    /// when it got ready (None: not ready)
    ready: Option<std::time::Instant>,
}
static MAX: Mutex<Max> = Mutex::new(Max { streak: 0, ready: None });

impl Max {
    /// Ready, and not run out (run out: lost, the streak starts over).
    fn ready(&mut self) -> bool {
        if self.ready.is_some_and(|t| t.elapsed().as_secs_f32() > MAX_KEEP) {
            *self = Max { streak: 0, ready: None };
            crate::log("style: MAX lost (not used in time)");
        }
        self.ready.is_some()
    }
}

/// A MAX finisher is ready (the weapon glows), and how long it's had (s).
pub fn max_ready() -> Option<f32> {
    let mut m = MAX.lock().unwrap_or_else(|e| e.into_inner());
    if m.ready() {
        m.ready.map(|t| t.elapsed().as_secs_f32())
    } else {
        None
    }
}

/// Uses the MAX finisher up: true if it was ready (then the streak starts over).
pub fn take_max() -> bool {
    let mut m = MAX.lock().unwrap_or_else(|e| e.into_inner());
    let ready = m.ready();
    if ready {
        *m = Max { streak: 0, ready: None };
    }
    ready
}

fn max_hit() {
    let mut m = MAX.lock().unwrap_or_else(|e| e.into_inner());
    if m.ready() {
        return;
    }
    m.streak += 1;
    if m.streak >= MAX_HITS {
        m.ready = Some(std::time::Instant::now());
        drop(m);
        crate::log(format!("style: MAX ready ({MAX_HITS} blows without a hit)"));
        callout("MAX READY", None, [1.0, 0.75, 0.2], false, 0.0);
        sounds(vec![(SOUND_RED_COIN + (7 << 16), CHIME_VOLUME)]);
    }
}

/// The string still going (its points drained up to now).
fn live(g: &mut Option<Meter>) -> Option<&mut Meter> {
    let now = now();
    let m = g.as_mut().filter(|m| m.alive(now))?;
    m.settle(now);
    Some(m)
}

/// The current rank, if the string is still going.
pub fn rank() -> Option<usize> {
    live(&mut METER.lock().unwrap_or_else(|e| e.into_inner())).and_then(|m| m.rank)
}

/// Points into the string, and the chime and the cheers for a rank not reached before.
fn add(m: &mut Meter, now: f32, points: f32) {
    let points = points * scale();
    let up = m.gain(now, points);
    if let Some(r) = up {
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

/// Blows that are the knock-on of a MAX show or the like: a fraction of a point each.
fn weak(attack: Attack) -> bool {
    matches!(attack, Attack::MaxFlurry | Attack::MaxUpper | Attack::MaxQuake | Attack::MaxCyclone | Attack::RideCrash | Attack::RideBlow | Attack::RideSmash | Attack::Bowled)
}

/// Game thread: one of Mario's blows landed on the enemy `target`.
pub fn hit(attack: Attack, target: u64) {
    let mut g = METER.lock().unwrap_or_else(|e| e.into_inner());
    let now = now();
    if live(&mut g).is_none() {
        *g = Some(Meter::new(now));
    }
    let Some(m) = g.as_mut() else { return };
    m.count += 1;
    m.last = now;
    // (the MAX shows' own blows and the knock-on hits don't count towards the next one)
    if !weak(attack) {
        max_hit();
    }
    let points = m.blow(now, attack, target) * if weak(attack) { WEAK_SCALE } else { 1.0 };
    add(m, now, points);
}

/// Game thread: Mario switched weapons in the middle of a string (moves.rs's swap strike): a
/// bonus, if the string is still going.
pub fn swap() {
    bonus(SWAP_POINTS);
}

/// Game thread: Mario leapt from one enemy to the next in the middle of a string (moves.rs's
/// freeflow): a bonus, the string kept going.
pub fn flow() {
    bonus(FLOW_POINTS);
}
const FLOW_POINTS: f32 = 1.5;

/// Co-op: points for a blow with the partner (coop.rs's BROS! streak), no word popping out.
pub fn team(points: f32) {
    bonus(points);
}

/// Points that keep the string going (no blow of their own).
fn bonus(points: f32) {
    let mut g = METER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = live(&mut g) {
        let now = now();
        m.last = now;
        add(m, now, points);
    }
}

/// Game thread: a combo's finisher landed at `at` (Elden Ring, the target's chest). From the S
/// rank it's a star finish: true then (impact.rs shows it).
pub fn finisher(at: Vec3) -> bool {
    let rank = {
        let mut m = METER.lock().unwrap_or_else(|e| e.into_inner());
        let Some(m) = live(&mut m) else { return false };
        let Some(rank) = m.rank.filter(|&r| r >= STAR_RANK) else { return false };
        let mut star = STAR.lock().unwrap_or_else(|e| e.into_inner());
        if star.is_some_and(|t| t.elapsed().as_secs_f32() < STAR_GAP) {
            return false;
        }
        *star = Some(Instant::now());
        add(m, now(), STAR_POINTS);
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
        bonus(points);
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
        if m.ready.is_some() {
            crate::log("style: MAX lost (hurt)");
        }
        *m = Max { streak: 0, ready: None };
    }
    *HURT_AT.lock().unwrap_or_else(|e| e.into_inner()) = now();
    let lost = METER.lock().unwrap_or_else(|e| e.into_inner()).take().filter(|m| m.alive(now())).and_then(|m| m.rank);
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
    let mut g = METER.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.as_mut()?;
    if m.alive(now()) {
        m.settle(now());
    }
    let since = now() - m.last;
    let alpha = if since <= KEEP { 1.0 } else { 1.0 - (since - KEEP) / FADE };
    if alpha <= 0.0 || m.count < 2 {
        return None;
    }
    let fresh = |t: f32, len: f32| (1.0 - t / len).max(0.0);
    let rank_age = now() - m.ranked;
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
    *HURT_AT.lock().unwrap_or_else(|e| e.into_inner()) = -100.0;
    *METER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    CALLOUTS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *MAX.lock().unwrap_or_else(|e| e.into_inner()) = Max { streak: 0, ready: None };
    *SLOW.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *LOST.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *STAR.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a string of blows is worth now: the final points and the best rank, for blows at
    /// `gap` s, `bonus` points of callouts on every `every`th blow.
    fn run(seq: &[(Attack, u64)], gap: f32, every: usize, bonus: f32) -> (f32, f32, Option<usize>) {
        let mut m = Meter::new(0.0);
        let (mut t, mut best, mut peak) = (0.0f32, 0.0f32, None);
        for (i, &(a, target)) in seq.iter().enumerate() {
            t += gap;
            m.settle(t);
            m.last = t;
            m.count += 1;
            let p = m.blow(t, a, target);
            m.gain(t, p);
            if every > 0 && i % every == every - 1 {
                m.gain(t, bonus);
            }
            best = best.max(m.points);
            peak = peak.max(m.rank);
        }
        (m.points, best, peak)
    }

    /// The old rule: a point a blow, a finisher's 1.5, 0.6 for a kind other than the last's.
    fn old(seq: &[(Attack, u64)], every: usize, bonus: f32) -> (f32, Option<usize>) {
        let (mut p, mut last) = (0.0, None);
        for (i, &(a, _)) in seq.iter().enumerate() {
            p += 1.0 + if a.finisher() { 1.5 } else { 0.0 } + if last.is_some_and(|k| k != std::mem::discriminant(&a)) { 0.6 } else { 0.0 };
            if every > 0 && i % every == every - 1 {
                p += bonus;
            }
            last = Some(std::mem::discriminant(&a));
        }
        let old_ranks = [3.0, 9.0, 17.0, 28.0, 44.0, 64.0, 92.0];
        (p, old_ranks.iter().rposition(|&n| p >= n))
    }

    fn letter(r: Option<usize>) -> &'static str {
        r.map_or("-", |r| RANKS[r].1)
    }

    #[test]
    fn typical_fights_before_and_after() {
        use Attack::*;
        // button mashing: 60 punches on one enemy, 5 a second
        let mash: Vec<_> = (0..60).map(|_| (Punch, 1)).collect();
        // a plain fight: punch string, kick, a hammer smash now and then, two enemies, 2 blows a second
        let plain: Vec<_> = (0..40).map(|i| ([Punch, Punch, Kick, Punch, Hammer, HammerSmash][i % 6], 1 + (i / 12) as u64)).collect();
        // a varied fight: eight kinds, three enemies, finishers, skill callouts every 8th blow
        let kinds = [Punch, Kick, Uppercut, UppercutTop, Hammer, HammerSmash, Chomp, ChompEnd, Rang, AirHit, Flurry, FlurryEnd];
        let varied: Vec<_> = (0..70).map(|i| (kinds[i % kinds.len()], 1 + (i / 5 % 3) as u64)).collect();
        // an expert: 3 blows a second on four enemies, every kind, a skillshot every 4th blow
        let expert: Vec<_> = (0..120).map(|i| (kinds[(i * 5) % kinds.len()], 1 + (i / 3 % 4) as u64)).collect();
        for (name, seq, gap, every, bonus) in [("mash", &mash, 0.2, 0, 0.0), ("plain", &plain, 0.5, 0, 0.0), ("varied", &varied, 0.45, 8, 3.0), ("expert", &expert, 0.33, 4, 3.0)] {
            let (before, rb) = old(seq, every, bonus);
            let (after, best, rn) = run(seq, gap, every, bonus);
            println!("style test: {name:7} {} blows: before {before:5.1} pts ({}), after {after:5.1} pts (best {best:5.1}, rank {})", seq.len(), letter(rb), letter(rn));
        }
        // the mash must stay low, the varied fight must reach S or more
        assert!(run(&mash, 0.2, 0, 0.0).2 < Some(3));
        assert!(run(&varied, 0.45, 8, 3.0).2 >= Some(4));
        assert!(run(&plain, 0.5, 0, 0.0).2 < Some(4));
        assert!(run(&expert, 0.33, 4, 3.0).2 >= Some(5));
    }

    #[test]
    fn rank_drains_and_holds() {
        let mut m = Meter::new(0.0);
        m.points = 60.0;
        m.rank = Some(4);
        m.peak = Some(4);
        m.last = 0.0;
        m.settle(0.5);
        assert_eq!(m.rank, Some(4));
        m.last = 2.0;
        m.settle(2.4);
        // 60 - 2.2 * 2.4 s, still S ... and a new peak is the only chime
        assert!(m.points < 56.0);
        // (held at S down to 0.94 of its mark)
        assert_eq!(m.rank, Some(4));
        assert!(m.gain(2.4, 0.5).is_none());
        m.points = 52.0;
        m.settle(2.4);
        assert_eq!(m.rank, Some(3));
    }

    #[test]
    fn one_enemy_window_slides() {
        // 7 blows on one enemy, then a pause longer than the window from the first ones but not
        // from the last: only the blows within 8 s count
        let mut m = Meter::new(0.0);
        for i in 0..7 {
            m.blow(i as f32, Attack::Punch, 1);
        }
        m.blow(9.0, Attack::Kick, 1);
        // (the one at 0 s is out of the window by 9 s: 1..6 and 9)
        assert_eq!(m.targets[&1].len(), 7);
    }
}
