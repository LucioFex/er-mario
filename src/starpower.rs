//! Star Power (v4 Part 6), Mario's Devil Trigger: reaching the S rank (style.rs) earns the
//! encounter's Star (one banked at most; the encounter ends after CALM s with no blow dealt or
//! taken). LB + RB together lets it out for TIME s: rainbow glints round him and SM64's power-up
//! music, his blows worth more (and more on a stance), small hits shrugged off (their health still
//! taken), and a trait for the weapon in hand: the sword's landed finishers throw its beam, the
//! hammer's quakes reach farther, the Chomp's chain is longer, a boomerang throw is two, the fists
//! leave a trail of fire. It drains with time and with the damage he deals; LB + RB again is the
//! STAR BLAST, one big blow on everyone round him, the stronger the more time was left. After it
//! the style rank is B. Bosses take less of all of it.

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

use crate::{log, sm64, worker};

/// how long it lasts (s), the calm that ends an encounter (s)
pub const TIME: f32 = 12.0;
const CALM: f32 = 20.0;
/// what it costs a % of a regular enemy's health dealt (s)
const DRAIN_PER_PCT: f32 = 0.06;
/// the blows' worth and on a stance, on a boss
const DAMAGE: f32 = 1.2;
const DAMAGE_BOSS: f32 = 1.1;
const STANCE: f32 = 1.5;
const STANCE_BOSS: f32 = 1.25;
/// the blast's worth: at the least, and up to 1 with all the time left; not before this long on (s:
/// let out and blasted at once, the blast was the whole of it)
const BLAST_LEAST: f32 = 0.35;
const BLAST_AFTER: f32 = 3.0;
/// LB and RB this close together (ticks): the chord
const CHORD_TICKS: u32 = 4;

/// The Star and the power: a Star banked, the power's time left (s), it's on (for this long, s),
/// the encounter's Star earned already, the encounter's last blow (s).
#[derive(Clone, Copy, Debug, Default)]
pub struct Power {
    pub banked: bool,
    pub left: f32,
    pub active: bool,
    on_for: f32,
    earned: bool,
    last: f32,
}

impl Power {
    /// The S rank reached at `now` (s): a Star banked, true if it's new.
    pub fn earn(&mut self, now: f32) -> bool {
        let _ = now;
        if self.earned || self.banked {
            return false;
        }
        self.earned = true;
        self.banked = true;
        true
    }

    /// A blow dealt or taken at `now`: the encounter goes on (one calm for CALM ends it).
    pub fn touch(&mut self, now: f32) {
        // (a calm since the last blow: a new encounter, its own Star to earn; the game touches
        // with the blow before its S comes)
        if now - self.last > CALM {
            self.earned = false;
        }
        self.last = now;
    }

    /// LB + RB: on, if a Star's banked (spent).
    pub fn activate(&mut self) -> bool {
        if !self.banked || self.active {
            return false;
        }
        self.banked = false;
        self.active = true;
        self.left = TIME;
        self.on_for = 0.0;
        true
    }

    /// `dt` s on: true when it ran out now.
    pub fn step(&mut self, dt: f32) -> bool {
        if !self.active {
            return false;
        }
        self.on_for += dt;
        self.left -= dt;
        if self.left > 0.0 {
            return false;
        }
        self.left = 0.0;
        self.active = false;
        true
    }

    /// `pct` of a regular enemy's health dealt: its share of the time.
    pub fn dealt(&mut self, pct: f32) {
        if self.active {
            self.left = (self.left - pct.max(0.0) * DRAIN_PER_PCT).max(0.0);
        }
    }

    /// LB + RB on: the blast's worth (BLAST_LEAST .. 1 by the time left); it's over.
    pub fn blast(&mut self) -> Option<f32> {
        if !self.active || self.on_for < BLAST_AFTER {
            return None;
        }
        let worth = BLAST_LEAST + (1.0 - BLAST_LEAST) * (self.left / TIME).clamp(0.0, 1.0);
        self.active = false;
        self.left = 0.0;
        Some(worth)
    }
}

/// What a blow is worth with it on (a boss's less).
pub fn damage(active: bool, boss: bool) -> f32 {
    match (active, boss) {
        (false, _) => 1.0,
        (true, true) => DAMAGE_BOSS,
        (true, false) => DAMAGE,
    }
}

/// What a blow does to a stance with it on (a boss's less).
pub fn stance(active: bool, boss: bool) -> f32 {
    match (active, boss) {
        (false, _) => 1.0,
        (true, true) => STANCE_BOSS,
        (true, false) => STANCE,
    }
}

/// LB + RB pressed together: when each went down, and whether the two are the chord's until both
/// are let go (nobody else gets them: the weapon switch, the fireball).
#[derive(Clone, Copy, Debug, Default)]
pub struct Chord {
    lb_at: Option<u32>,
    rb_at: Option<u32>,
    eaten: bool,
}

impl Chord {
    /// LB and RB held now, at `tick`, `armed` (a Star or the power to use, the buttons free): (the
    /// chord fired now, both are the chord's now). It fires only as the second goes down: two held
    /// as it got armed (a menu closing, LT let go) are no chord.
    pub fn step(&mut self, lb: bool, rb: bool, armed: bool, tick: u32) -> (bool, bool) {
        let down = (lb && self.lb_at.is_none()) || (rb && self.rb_at.is_none());
        // (when each went down, while held)
        if !lb {
            self.lb_at = None;
        } else if self.lb_at.is_none() {
            self.lb_at = Some(tick);
        }
        if !rb {
            self.rb_at = None;
        } else if self.rb_at.is_none() {
            self.rb_at = Some(tick);
        }
        if self.eaten {
            self.eaten = lb || rb;
            return (false, self.eaten);
        }
        match (self.lb_at, self.rb_at) {
            (Some(a), Some(b)) if armed && down && a.abs_diff(b) <= CHORD_TICKS => {
                self.eaten = true;
                (true, true)
            }
            _ => (false, false),
        }
    }
}

// ---- in the game ------------------------------------------------------------------------------

/// the blast reaches this far round him (m); the weapons' traits: the hammer's quakes, the Chomp's
/// cyclone, its chain past its place by him (C's 1.25 otherwise); the fists' trail: an ember every
/// this many ticks; the sword's beam at most every this many
pub const BLAST_RADIUS: f32 = 7.0;
const QUAKE: f32 = 1.5;
const CYCLONE: f32 = 1.3;
const CHAIN: f32 = 1.6;
const CHAIN_PLAIN: f32 = 1.25;
const TRAIL_EVERY: u32 = 8;
const BEAM_EVERY: u32 = 10;
/// a shrugged-off hit guards him this long (ticks), as SM64's knockback does after a hit
const SHRUG_GUARD: u32 = 30;
/// its glints round him: every this many ticks
const GLINT_EVERY: u32 = 2;

const SOUND_STAR_APPEARS: i32 = 0x3057_FF91;
const SOUND_POWER_UP: i32 = 0x3058_FF81;
const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_BLAST: i32 = 0x312F_0081;

struct State {
    power: Power,
    chord: Chord,
    /// a sword finisher landed (its beam to throw), the last beam, the last ember, the last glint
    beam_due: bool,
    beam_at: u32,
    trail_at: u32,
    glint_at: u32,
    /// the Chomp's chain as libsm64 has it, the power-up music on, a cap's or Mega's own on (theirs
    /// stops it as they end), the shrugged hits' guard until this tick
    chain: f32,
    music: bool,
    under: bool,
    guard: u32,
}

static STATE: Mutex<State> = Mutex::new(State {
    power: Power { banked: false, left: 0.0, active: false, on_for: 0.0, earned: false, last: -1000.0 },
    chord: Chord { lb_at: None, rb_at: None, eaten: false },
    beam_due: false,
    beam_at: 0,
    trail_at: 0,
    glint_at: 0,
    chain: CHAIN_PLAIN,
    music: false,
    under: false,
    guard: 0,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Seconds on its clock.
fn clock() -> f32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

fn sound(id: i32, volume: f32) {
    worker::call("star power sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

/// Reaching S (style.rs): the encounter's Star.
pub fn earn() {
    let new = state().power.earn(clock());
    if new {
        log("starpower: a Star (S rank): LB + RB lets it out");
        crate::style::callout("STAR POWER READY", None, [1.0, 0.9, 0.45], false, 0.0);
        sound(SOUND_STAR_APPEARS, 0.3);
    }
}

/// A blow dealt or taken: the encounter goes on.
pub fn touch() {
    state().power.touch(clock());
}

pub fn active() -> bool {
    state().power.active
}

/// What a blow is worth now (on a boss, `boss`).
pub fn damage_now(boss: bool) -> f32 {
    damage(active(), boss)
}

/// What a blow does to a stance now.
pub fn stance_now(boss: bool) -> f32 {
    stance(active(), boss)
}

/// `pct` of a regular enemy's health dealt (combat.rs): its share of the time.
pub fn dealt(pct: f32) {
    state().power.dealt(pct);
}

fn weapon_on(w: u8) -> bool {
    active() && crate::weapons::current() == w
}

/// The hammer's quakes' reach now (×).
pub fn quake() -> f32 {
    if weapon_on(crate::weapons::HAMMER) { QUAKE } else { 1.0 }
}

/// The Chomp's cyclone's reach now (×).
pub fn cyclone() -> f32 {
    if weapon_on(crate::weapons::CHOMP) { CYCLONE } else { 1.0 }
}

/// A plain boomerang throw is two now.
pub fn double_rang() -> bool {
    weapon_on(crate::weapons::RANG)
}

/// A combo's finisher of the sword's landed (combat.rs): its beam, thrown now (lib.rs).
pub fn finisher_landed() {
    if weapon_on(crate::weapons::SWORD) {
        state().beam_due = true;
    }
}

/// The sword's beam to throw this tick (not twice in a moment: one of its own, at full health, just went).
pub fn take_beam(tick: u32, thrown_now: bool) -> bool {
    let mut s = state();
    if thrown_now {
        s.beam_at = tick;
    }
    let due = std::mem::take(&mut s.beam_due) && tick.wrapping_sub(s.beam_at) >= BEAM_EVERY;
    if due {
        s.beam_at = tick;
    }
    due
}

/// The move's own sword beam may go now (Star Power's went a moment ago: not two).
pub fn native_beam_ok(tick: u32) -> bool {
    tick.wrapping_sub(state().beam_at) >= BEAM_EVERY
}

/// A small hit shrugged off at `tick`: whether it costs health (a moment after one it doesn't, as
/// SM64's knockback leaves him guarded).
pub fn shrug_costs(tick: u32) -> bool {
    let mut s = state();
    if tick < s.guard && s.guard.wrapping_sub(tick) <= SHRUG_GUARD {
        return false;
    }
    s.guard = tick.wrapping_add(SHRUG_GUARD);
    true
}

/// The fists' trail: an ember to lay under him now (running on the ground, `moving`).
pub fn fire_trail(tick: u32, moving: bool) -> bool {
    if !moving || !weapon_on(crate::weapons::FISTS) {
        return false;
    }
    let mut s = state();
    let due = tick.wrapping_sub(s.trail_at) >= TRAIL_EVERY;
    if due {
        s.trail_at = tick;
    }
    due
}

/// What LB + RB did this tick.
pub enum Pressed {
    /// nothing together
    No,
    /// together, but there's nothing to let out (a buzz)
    Nothing,
    /// Star Power on
    On,
    /// the STAR BLAST: its worth
    Blast(f32),
}

/// LB and RB held now (lib.rs, before the weapon switch and the fireball), `free` (no menu, no trap
/// kit, not Mega): what they did, and whether they're the chord's now (eaten: nobody else gets
/// them). Only with a Star or the power on: without, LB and RB are their own.
pub fn chord(lb: bool, rb: bool, free: bool, tick: u32) -> (Pressed, bool) {
    let mut s = state();
    let armed = free && (s.power.banked || s.power.active);
    let (fired, eat) = s.chord.step(lb, rb, armed, tick);
    if !fired {
        return (Pressed::No, eat);
    }
    // (on: the blast, once it's been on a moment; a buzz before)
    let pressed = if s.power.active {
        s.power.blast().map_or(Pressed::Nothing, Pressed::Blast)
    } else if s.power.activate() {
        Pressed::On
    } else {
        Pressed::Nothing
    };
    drop(s);
    match pressed {
        Pressed::On => {
            log(format!("starpower: on ({TIME:.0} s)"));
            crate::style::callout("STAR POWER", None, [1.0, 0.85, 0.35], true, 0.0);
            sound(SOUND_POWER_UP, 0.35);
            music(true);
        }
        Pressed::Nothing => sound(SOUND_BUZZ, 0.3),
        _ => {}
    }
    (pressed, eat)
}

/// SM64's power-up music (the cap's volume), not stopped under a cap's or Mega's own.
fn music(on: bool) {
    let mut s = state();
    if s.music == on {
        return;
    }
    s.music = on;
    drop(s);
    if on || (crate::caps::active().is_none() && !crate::mega::active()) {
        worker::call("star power music", move |_| unsafe { sm64::sm64_er_star_music(on as i32) });
    }
}

/// It's over (run out, or the blast): the style rank to B, the music off.
fn over(why: &str) {
    log(format!("starpower: over ({why})"));
    music(false);
    crate::style::set_rank(2);
}

/// The STAR BLAST let out (lib.rs dealt its blows on `hit` enemies, `worth`).
pub fn blasted(at: Vec3, worth: f32, hit: usize) {
    log(format!("starpower: STAR BLAST x{worth:.2} on {hit}"));
    crate::impact::star_finish(at, 6);
    crate::lakitu::shake(1.6);
    crate::style::slow_down(0.6);
    crate::style::callout("STAR BLAST!", Some(at + Vec3::Y * 2.0), [1.0, 0.9, 0.5], true, 0.0);
    sound(SOUND_BLAST, 0.45);
    over("the blast");
}

/// Game thread, every SM64 tick (`dt` s): its time runs, its glints round him (`at`, his middle),
/// the Chomp's chain as long as it should be.
pub fn tick(dt: f32, at: Vec3, tick: u32) {
    let under = crate::caps::active().is_some() || crate::mega::active();
    let mut s = state();
    // (a cap's or Mega's music ending stops SM64's cap music, the power-up's too: again)
    let replay = s.power.active && s.music && s.under && !under;
    s.under = under;
    let ran_out = s.power.step(dt);
    let on = s.power.active;
    let glint = on && tick.wrapping_sub(s.glint_at) >= GLINT_EVERY;
    if glint {
        s.glint_at = tick;
    }
    let chain = if on && crate::weapons::current() == crate::weapons::CHOMP { CHAIN } else { CHAIN_PLAIN };
    let send = (chain != s.chain).then_some(chain);
    if let Some(c) = send {
        s.chain = c;
    }
    drop(s);
    if ran_out {
        over("time");
    }
    if replay {
        worker::call("star power music", |_| unsafe { sm64::sm64_er_star_music(1) });
    }
    if let Some(c) = send {
        worker::call("star power chain", move |_| unsafe { sm64::sm64_er_set_chomp_longer(c) });
    }
    // (the rainbow: a glint round him, its colour going round)
    if glint {
        let h = (tick as f32 * 0.09).rem_euclid(1.0);
        let a = tick as f32 * 0.7;
        let p = at + Vec3::new(a.cos() * 0.55, (tick as f32 * 0.37).sin() * 0.6, a.sin() * 0.55);
        crate::impact::glint(p, rainbow(h), 0.35);
    }
}

/// A colour round the rainbow (`h` 0-1).
fn rainbow(h: f32) -> [f32; 4] {
    let f = |k: f32| (((h * 6.0 + k) % 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    [f(0.0), f(4.0), f(2.0), 1.0]
}

/// What hud.rs draws: a Star banked, the power's time left (0-1) while it's on.
pub fn hud() -> (bool, Option<f32>) {
    let s = state();
    (s.power.banked, s.power.active.then_some(s.power.left / TIME))
}

/// Out of Mario mode, dead: it's gone (the Star with it).
pub fn clear() {
    let mut s = state();
    let was = s.power.active;
    s.power = Power { last: -1000.0, ..Power::default() };
    s.beam_due = false;
    drop(s);
    if was {
        music(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_star_an_encounter() {
        let mut p = Power::default();
        assert!(p.earn(1.0));
        assert!(p.banked);
        // (S again in the same fight: nothing more; one banked at most)
        p.touch(5.0);
        assert!(!p.earn(6.0));
        // (a calm: the next fight's S earns one, but it's still one banked)
        p.touch(6.0);
        assert!(!p.earn(6.0 + CALM + 1.0) || p.banked);
        let mut p = Power::default();
        assert!(p.earn(1.0));
        assert!(p.activate());
        assert!(!p.banked && p.active);
        p.step(TIME + 1.0);
        p.touch(20.0);
        assert!(!p.earn(25.0));
        // (as the game has it: the blow's touch, then its S; after a calm, a new encounter)
        p.touch(20.0 + CALM + 0.5);
        assert!(p.earn(20.0 + CALM + 0.6));
        // (blows all along, no calm: the same encounter)
        let mut p = Power::default();
        for t in 0..60 {
            p.touch(t as f32);
        }
        p.earn(10.0);
        p.activate();
        p.step(TIME + 1.0);
        p.touch(60.0);
        assert!(!p.earn(60.1));
    }

    #[test]
    fn the_blast_waits_a_moment() {
        let mut p = Power::default();
        p.earn(0.0);
        p.activate();
        p.step(BLAST_AFTER * 0.5);
        assert_eq!(p.blast(), None);
        assert!(p.active);
        p.step(BLAST_AFTER);
        assert!(p.blast().is_some());
    }

    #[test]
    fn it_needs_a_star_and_drains() {
        let mut p = Power::default();
        assert!(!p.activate());
        p.earn(0.0);
        assert!(p.activate());
        assert!((p.left - TIME).abs() < 1e-5);
        // (time and damage dealt)
        assert!(!p.step(2.0));
        p.dealt(50.0);
        assert!((p.left - (TIME - 2.0 - 50.0 * DRAIN_PER_PCT)).abs() < 1e-4, "{}", p.left);
        assert!(p.step(TIME));
        assert!(!p.active && p.left == 0.0);
        // (over: no more steps that run out)
        assert!(!p.step(1.0));
    }

    #[test]
    fn the_blast_by_the_time_left() {
        let mut p = Power::default();
        assert_eq!(p.blast(), None);
        p.earn(0.0);
        p.activate();
        p.step(BLAST_AFTER);
        let soon = p.blast().unwrap();
        let k = (TIME - BLAST_AFTER) / TIME;
        assert!((soon - (BLAST_LEAST + (1.0 - BLAST_LEAST) * k)).abs() < 1e-4 && !p.active, "{soon}");
        p.touch(CALM * 3.0);
        p.earn(CALM * 3.0);
        p.activate();
        p.step(TIME * 0.5);
        let half = p.blast().unwrap();
        assert!((half - (BLAST_LEAST + (1.0 - BLAST_LEAST) * 0.5)).abs() < 1e-4, "{half}");
    }

    #[test]
    fn its_worth_on_regulars_and_bosses() {
        assert_eq!(damage(false, false), 1.0);
        assert_eq!(damage(true, false), DAMAGE);
        assert_eq!(damage(true, true), DAMAGE_BOSS);
        assert_eq!(stance(false, true), 1.0);
        assert_eq!(stance(true, false), STANCE);
        assert_eq!(stance(true, true), STANCE_BOSS);
    }

    #[test]
    fn lb_and_rb_together_are_the_chord() {
        let mut c = Chord::default();
        assert_eq!(c.step(true, false, true, 10), (false, false));
        assert_eq!(c.step(true, true, true, 13), (true, true));
        // (held on: eaten, not fired again)
        assert_eq!(c.step(true, true, true, 14), (false, true));
        // (one up: the other still eaten until it's up too)
        assert_eq!(c.step(false, true, true, 15), (false, true));
        assert_eq!(c.step(false, false, true, 16), (false, false));
        // RB first, then LB
        assert_eq!(c.step(false, true, true, 20), (false, false));
        assert_eq!(c.step(true, true, true, 22), (true, true));
        assert_eq!(c.step(false, false, true, 23), (false, false));
        // (too far apart: no chord, nothing eaten)
        assert_eq!(c.step(true, false, true, 30), (false, false));
        assert_eq!(c.step(true, true, true, 36), (false, false));
        assert_eq!(c.step(false, false, true, 37), (false, false));
        // (not armed: no Star, a menu, the kit: nothing, the buttons their own)
        assert_eq!(c.step(true, false, false, 40), (false, false));
        assert_eq!(c.step(true, true, false, 41), (false, false));
        // (armed while both were held already: no chord out of it)
        assert_eq!(c.step(true, true, true, 42), (false, false));
        assert_eq!(c.step(false, false, true, 43), (false, false));
    }
}
