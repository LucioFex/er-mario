//! The fighting-game input grammar for the moves (script.rs): the stick's last moments relative to
//! the enemy in front (else to where Mario faces) and the buttons, read as back → forward + a
//! button (DMC's Stinger), holds and releases with their charge time, a beat between presses
//! (Bayonetta's pause strings), mashing, chords (X + A). And the Dodge Offset: X held through a
//! dash keeps the string's step for the next X.

use std::collections::VecDeque;

/// The buttons, as bits.
pub const X: u8 = 1;
pub const Y: u8 = 2;
pub const A: u8 = 4;
pub const B: u8 = 8;

/// Where the stick points, relative to the enemy (or Mario's facing).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dir {
    Neutral,
    Forward,
    Back,
    Left,
    Right,
}

/// past this, the stick points somewhere (of 1)
const TILT: f32 = 0.6;
/// back → forward: the back this many ticks before the forward at most, the button this many after
const BACK_BEFORE: u32 = 8;
const FORWARD_SLACK: u32 = 2;
/// ticks of history kept
const KEEP: u32 = 60;

#[derive(Clone, Copy)]
struct Sample {
    tick: u32,
    dir: Dir,
    buttons: u8,
}

#[derive(Default)]
pub struct Grammar {
    hist: VecDeque<Sample>,
    /// each button (by bit, X Y A B): held since this tick, and how long it was held when it last
    /// came up and when (charges outlast the history kept)
    down_since: [Option<u32>; 4],
    let_go: [Option<(u32, u32)>; 4],
}

/// a button's bit's slot (X 0, Y 1, A 2, B 3)
fn slot(b: u8) -> usize {
    (b.max(1).trailing_zeros() as usize).min(3)
}

fn dir_of(forward: f32, side: f32) -> Dir {
    if forward.abs() >= side.abs() && forward.abs() >= TILT {
        if forward > 0.0 { Dir::Forward } else { Dir::Back }
    } else if side.abs() > forward.abs() && side.abs() >= TILT {
        if side > 0.0 { Dir::Right } else { Dir::Left }
    } else {
        Dir::Neutral
    }
}

impl Grammar {
    /// Every tick: the stick (`forward` toward the enemy, `side` to his right, each -1..1) and the
    /// buttons held now.
    pub fn push(&mut self, tick: u32, forward: f32, side: f32, buttons: u8) {
        for (k, b) in [X, Y, A, B].into_iter().enumerate() {
            match (buttons & b != 0, self.down_since[k]) {
                (true, None) => self.down_since[k] = Some(tick),
                (false, Some(since)) => {
                    self.down_since[k] = None;
                    self.let_go[k] = Some((tick.wrapping_sub(since), tick));
                }
                _ => {}
            }
        }
        self.hist.push_back(Sample { tick, dir: dir_of(forward, side), buttons });
        while self.hist.front().is_some_and(|s| tick.wrapping_sub(s.tick) > KEEP) {
            self.hist.pop_front();
        }
    }

    fn now(&self) -> Option<&Sample> {
        self.hist.back()
    }

    #[allow(dead_code)]
    pub fn dir(&self) -> Dir {
        self.now().map_or(Dir::Neutral, |s| s.dir)
    }

    /// `b` went down at the sample `i` (an index into the history; the oldest can't tell: it may
    /// have been held from before).
    fn down_at(&self, i: usize, b: u8) -> bool {
        i > 0 && self.hist[i].buttons & b != 0 && self.hist[i - 1].buttons & b == 0
    }

    /// `b` went down this tick.
    pub fn pressed(&self, b: u8) -> bool {
        !self.hist.is_empty() && self.down_at(self.hist.len() - 1, b)
    }

    /// `b` went down this tick with the stick forward now (or a tick or two ago), and back just
    /// before that forward.
    pub fn back_forward(&self, b: u8) -> bool {
        let Some(now) = self.now().map(|s| s.tick) else { return false };
        if !self.pressed(b) {
            return false;
        }
        let Some(fwd) = self.hist.iter().rev().find(|s| now.wrapping_sub(s.tick) <= FORWARD_SLACK && s.dir == Dir::Forward).map(|s| s.tick) else {
            return false;
        };
        self.hist.iter().any(|s| s.dir == Dir::Back && s.tick < fwd && fwd - s.tick <= BACK_BEFORE)
    }

    // (the reads below are the moves' of Parts 2-3: charges, releases, mashing, beats)
    /// How many ticks `b` has been held (0: up).
    #[allow(dead_code)]
    pub fn held(&self, b: u8) -> u32 {
        let Some(now) = self.now().map(|s| s.tick) else { return 0 };
        self.down_since[slot(b)].map_or(0, |since| now.wrapping_sub(since) + 1)
    }

    /// `b` came up this tick: how long it had been held.
    #[allow(dead_code)]
    pub fn released(&self, b: u8) -> Option<u32> {
        let now = self.now()?.tick;
        self.let_go[slot(b)].filter(|&(_, at)| at == now).map(|(held, _)| held)
    }

    /// Presses of `b` within the last `window` ticks (this one included).
    #[allow(dead_code)]
    pub fn mash(&self, b: u8, window: u32) -> u32 {
        let Some(now) = self.now().map(|s| s.tick) else { return 0 };
        (0..self.hist.len()).filter(|&i| now.wrapping_sub(self.hist[i].tick) < window && self.down_at(i, b)).count() as u32
    }

    /// When `b` last went down.
    #[allow(dead_code)]
    fn last_press(&self, b: u8) -> Option<u32> {
        (0..self.hist.len()).rev().find(|&i| self.down_at(i, b)).map(|i| self.hist[i].tick)
    }

    /// `a` and `b` both went down within `within` ticks of each other, the later one this tick.
    #[allow(dead_code)]
    pub fn chord(&self, a: u8, b: u8, within: u32) -> bool {
        let Some(now) = self.now().map(|s| s.tick) else { return false };
        let (Some(pa), Some(pb)) = (self.last_press(a), self.last_press(b)) else { return false };
        pa.max(pb) == now && pa.abs_diff(pb) <= within
    }

    /// `b` went down this tick, between `min` and `max` ticks after `since` (the beat of a pause
    /// string).
    #[allow(dead_code)]
    pub fn beat(&self, b: u8, since: u32, min: u32, max: u32) -> bool {
        let Some(now) = self.now().map(|s| s.tick) else { return false };
        self.pressed(b) && (min..=max).contains(&now.wrapping_sub(since))
    }
}

/// The Dodge Offset: X down as a dash starts in the middle of a string, kept down through it,
/// lets the next X go on with the string's next step (within `OFFSET_AFTER` ticks of the dash).
#[derive(Default)]
pub struct Offset {
    step: Option<u32>,
    dashed: bool,
    ended: Option<u32>,
}

/// after the dash, the next X carries the step this long (ticks)
const OFFSET_AFTER: u32 = 30;

impl Offset {
    /// A dash starts: `x_held` now, at string step `step` (0: no string).
    pub fn dash_started(&mut self, x_held: bool, step: u32, tick: u32) {
        let _ = tick;
        *self = if x_held && step > 0 { Offset { step: Some(step), dashed: true, ended: None } } else { Offset::default() };
    }

    /// Every tick: X held, still dashing.
    pub fn tick(&mut self, x_held: bool, dashing: bool, tick: u32) {
        if self.step.is_none() {
            return;
        }
        if self.dashed {
            if !dashing {
                self.dashed = false;
                self.ended = Some(tick);
            } else if !x_held {
                // (let go in the dash: the string is over)
                *self = Offset::default();
            }
        } else if self.ended.is_some_and(|e| tick.wrapping_sub(e) > OFFSET_AFTER) {
            *self = Offset::default();
        }
    }

    /// X went down (`x_pressed`): the step the string goes on from, if the offset holds one.
    pub fn take(&mut self, x_pressed: bool, tick: u32) -> Option<u32> {
        if !x_pressed || self.dashed {
            return None;
        }
        let fresh = self.ended.is_some_and(|e| tick.wrapping_sub(e) <= OFFSET_AFTER);
        let step = self.step.filter(|_| fresh);
        *self = Offset::default();
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays ticks from 0: each (forward, side, buttons).
    fn play(steps: &[(f32, f32, u8)]) -> Grammar {
        let mut g = Grammar::default();
        for (t, &(f, s, b)) in steps.iter().enumerate() {
            g.push(t as u32, f, s, b);
        }
        g
    }

    const N: (f32, f32, u8) = (0.0, 0.0, 0);
    const BK: (f32, f32, u8) = (-1.0, 0.0, 0);
    const FW: (f32, f32, u8) = (1.0, 0.0, 0);

    #[test]
    fn the_stick_reads_as_a_direction() {
        assert_eq!(dir_of(0.9, 0.1), Dir::Forward);
        assert_eq!(dir_of(-0.8, -0.3), Dir::Back);
        assert_eq!(dir_of(0.2, 0.9), Dir::Right);
        assert_eq!(dir_of(0.1, -0.7), Dir::Left);
        assert_eq!(dir_of(0.3, 0.3), Dir::Neutral);
        assert_eq!(play(&[N, BK]).dir(), Dir::Back);
    }

    #[test]
    fn back_then_forward_and_a_press_is_the_motion() {
        let g = play(&[N, BK, BK, BK, FW, FW, (1.0, 0.0, X)]);
        assert!(g.back_forward(X));
        assert!(!g.back_forward(Y));
        // (the press a tick or two after the stick came off forward still counts)
        let g = play(&[BK, BK, FW, N, (0.0, 0.0, X)]);
        assert!(g.back_forward(X));
    }

    #[test]
    fn no_back_or_a_stale_back_is_no_motion() {
        assert!(!play(&[N, FW, FW, (1.0, 0.0, X)]).back_forward(X));
        let mut steps = vec![BK, BK];
        steps.extend([N; 10]);
        steps.extend([FW, (1.0, 0.0, X)]);
        assert!(!play(&steps).back_forward(X));
        // (back alone, still held back, is the launcher's input, not this)
        assert!(!play(&[N, BK, (-1.0, 0.0, X)]).back_forward(X));
        // (a press held on from before isn't a new one)
        assert!(!play(&[(0.0, 0.0, X), (-1.0, 0.0, X), (1.0, 0.0, X)]).back_forward(X));
    }

    #[test]
    fn holds_and_releases_count_their_ticks() {
        let g = play(&[N, (0.0, 0.0, Y), (0.0, 0.0, Y), (0.0, 0.0, Y)]);
        assert_eq!(g.held(Y), 3);
        assert_eq!(g.released(Y), None);
        assert!(!g.pressed(Y));
        let g = play(&[N, (0.0, 0.0, Y), (0.0, 0.0, Y), (0.0, 0.0, Y), N]);
        assert_eq!(g.held(Y), 0);
        assert_eq!(g.released(Y), Some(3));
        assert!(play(&[N, (0.0, 0.0, Y)]).pressed(Y));
    }

    #[test]
    fn long_holds_count_past_the_history() {
        // (the Great Spin's top charge is 75 ticks: longer than the history kept)
        let mut steps = vec![N];
        steps.extend([(0.0, 0.0, Y); 90]);
        assert_eq!(play(&steps).held(Y), 90);
        steps.push(N);
        assert_eq!(play(&steps).released(Y), Some(90));
    }

    #[test]
    fn mashing_counts_presses_in_a_window() {
        let x = (0.0, 0.0, X);
        let g = play(&[N, x, N, x, N, N, x, N, x]);
        assert_eq!(g.mash(X, 20), 4);
        assert_eq!(g.mash(X, 3), 2);
        assert_eq!(g.mash(Y, 20), 0);
    }

    #[test]
    fn a_button_already_down_when_the_history_starts_is_no_press() {
        let x = (0.0, 0.0, X);
        let g = play(&[x, x, N, x]);
        assert_eq!(g.mash(X, 20), 1);
        assert!(!play(&[x]).pressed(X));
    }

    #[test]
    fn chords_and_beats() {
        let g = play(&[N, (0.0, 0.0, X), (0.0, 0.0, X | A)]);
        assert!(g.chord(X, A, 2));
        assert!(!g.chord(X, A, 0));
        let g = play(&[N, (0.0, 0.0, A), N, N, N, (0.0, 0.0, X)]);
        assert!(!g.chord(X, A, 2));
        // a press 12 ticks after tick 0: inside 10..36, not inside 14..20
        let mut steps = vec![N; 12];
        steps.push((0.0, 0.0, X));
        let g = play(&steps);
        assert!(g.beat(X, 0, 10, 36));
        assert!(!g.beat(X, 0, 14, 20));
    }

    #[test]
    fn dodge_offset_keeps_the_step_while_x_stays_down() {
        let mut o = Offset::default();
        o.dash_started(true, 2, 100);
        for t in 101..110 {
            o.tick(true, true, t);
        }
        o.tick(false, false, 110);
        assert_eq!(o.take(false, 111), None);
        assert_eq!(o.take(true, 112), Some(2));
        // (taken once)
        assert_eq!(o.take(true, 113), None);
    }

    #[test]
    fn dodge_offset_is_lost_by_letting_go_or_waiting() {
        let mut o = Offset::default();
        o.dash_started(true, 1, 0);
        o.tick(false, true, 1);
        o.tick(false, false, 5);
        assert_eq!(o.take(true, 6), None);
        let mut o = Offset::default();
        o.dash_started(true, 1, 0);
        o.tick(true, false, 5);
        assert_eq!(o.take(true, 5 + OFFSET_AFTER + 1), None);
        // (no string, no X: nothing to keep)
        let mut o = Offset::default();
        o.dash_started(false, 2, 0);
        o.tick(false, false, 4);
        assert_eq!(o.take(true, 5), None);
        let mut o = Offset::default();
        o.dash_started(true, 0, 0);
        o.tick(true, false, 4);
        assert_eq!(o.take(true, 5), None);
    }
}
