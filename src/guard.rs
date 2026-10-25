//! The hammer's B (Part 12.6b): the guard. B pressed with the stick still holds the hammer's head
//! up in front of him; held, he walks at a third of his pace. Blows from the front 120° lose 75%;
//! the third blocked blow breaks the guard (a second stunned, none for two). B pressed just before
//! a blow lands (the Just Guard) takes nothing: the attacker recoils and the next X is a counter
//! (dodge.rs). With the stick pushed B is the heavy roll. Grabs (the game takes him out of the
//! guard) and colossi (their blows pass) are not guarded. All of it pure down here; lib.rs reads
//! the hurt against it.

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, starstep};

/// the front 120°: the attacker within 60° of where he faces (cos)
pub const FRONT_COS: f32 = 0.5;
/// a blocked blow costs this share
pub const BLOCK_SHARE: f32 = 0.25;
/// the third blocked blow breaks the guard
pub const BREAK_AT: u8 = 3;
/// stunned 1 s, no guard for 2 s (SM64 ticks, 30 a second)
pub const STUN_TICKS: u32 = 30;
pub const LOCK_TICKS: u32 = 60;
/// the hammer's head takes this many ticks to come up in front of him
pub const BLEND: u32 = 3;
/// B this many ticks before a blow (or fewer) is the Just Guard
pub const JUST_TICKS: u32 = 5;
/// B let go and pressed again within this many ticks keeps its blocks counted and Just Guard spent
pub const KEEP_TICKS: u32 = 30;
/// ... and within these there is no new Just Guard at all
pub const JUST_CD_TICKS: u32 = 15;
/// his walk while he guards
pub const WALK: f32 = 0.35;
/// the stick below this (raw, 0..1) is "still"
pub const NEUTRAL: f32 = 0.3;
/// act_er_dash's action arg for the heavy roll (about 0.6 of the dash's speed)
pub const ROLL_ARG: u32 = 2;

/// An attacker at `to` (from Mario, flat) is in front of him as he faces `facing`.
pub fn frontal(facing: Vec3, to: Vec3) -> bool {
    let (f, t) = (facing.with_y(0.0).normalize_or_zero(), to.with_y(0.0).normalize_or_zero());
    t != Vec3::ZERO && f.dot(t) >= FRONT_COS
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Press {
    Guard,
    Roll,
    Locked,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Verdict {
    /// not guarded (behind, a colossus, no guard up)
    Through,
    Block,
    /// the third block: the guard breaks
    Broken,
    Just,
}

/// What a blow of `scale` (1: all of it) costs under this verdict.
pub fn scaled(v: Verdict, scale: f32) -> f32 {
    match v {
        Verdict::Through => scale,
        Verdict::Block | Verdict::Broken => scale * BLOCK_SHARE,
        Verdict::Just => 0.0,
    }
}

#[derive(Default)]
pub struct Guard {
    /// B went down at this tick and the guard is up
    since: Option<u32>,
    blocks: u8,
    /// no guard before this tick
    locked_until: Option<u32>,
    /// the Just Guard of this press is spent
    just_spent: bool,
    /// B let go at this tick (a quick re-press keeps the blocks and the spent Just Guard)
    released_at: Option<u32>,
}

/// `t` is still ahead of `tick` (wrapping ticks).
fn before(tick: u32, t: u32) -> bool {
    t != tick && t.wrapping_sub(tick) < u32::MAX / 2
}

impl Guard {
    /// B went down at `tick` with the stick `stick` (raw 0..1).
    pub fn press(&mut self, tick: u32, stick: f32) -> Press {
        if stick >= NEUTRAL {
            return Press::Roll;
        }
        if self.locked_until.is_some_and(|t| before(tick, t)) {
            return Press::Locked;
        }
        // (tapped again soon: the blocks stay counted and no new Just Guard until the cooldown ends)
        let quick = self.released_at.map(|r| tick.wrapping_sub(r));
        let mut g = match quick {
            Some(d) if d < KEEP_TICKS => Guard { blocks: self.blocks, just_spent: self.just_spent || d < JUST_CD_TICKS, ..Guard::default() },
            _ => Guard::default(),
        };
        g.since = Some(tick);
        *self = g;
        Press::Guard
    }

    /// B let go at `tick`, or he is no longer on his feet with the hammer.
    pub fn release(&mut self, tick: u32) {
        if self.since.take().is_some() {
            self.released_at = Some(tick);
        }
    }

    pub fn up(&self) -> bool {
        self.since.is_some()
    }

    /// Ticks the guard has been up.
    pub fn age(&self, tick: u32) -> Option<u32> {
        self.since.map(|s| tick.wrapping_sub(s))
    }

    /// A blow lands at `tick`: `front` it comes from the front, `passes` it can't be guarded (a
    /// colossus's).
    pub fn judge(&mut self, tick: u32, front: bool, passes: bool) -> Verdict {
        let Some(age) = self.age(tick) else { return Verdict::Through };
        if !front || passes {
            return Verdict::Through;
        }
        if age <= JUST_TICKS && !self.just_spent {
            self.just_spent = true;
            return Verdict::Just;
        }
        self.blocks += 1;
        if self.blocks >= BREAK_AT {
            self.release(tick);
            self.blocks = 0;
            self.released_at = None;
            self.locked_until = Some(tick.wrapping_add(LOCK_TICKS));
            return Verdict::Broken;
        }
        Verdict::Block
    }

    /// A blow that cost him HP at `tick`: `procced` an ailment's damage landed (not a blow), `at`
    /// the attacker's (front, passes) if an attack is known to have caused it. Nobody known: the
    /// guard doesn't hold (bleeding, a fall, a trap).
    pub fn judge_source(&mut self, tick: u32, procced: bool, at: Option<(bool, bool)>) -> Verdict {
        match at {
            Some((front, passes)) if !procced => self.judge(tick, front, passes),
            _ => Verdict::Through,
        }
    }

    pub fn clear(&mut self) {
        *self = Guard::default();
    }
}

// ---- the game's side ------------------------------------------------------------------------

/// The enemy that hit him, as the guard needs it.
pub struct Located {
    pub handle: FieldInsHandle,
    pub id: i32,
    /// its feet (Elden Ring) and its body's radius (m)
    pub pos: Vec3,
    pub radius_m: f32,
    /// a colossus: its blows pass the guard
    pub colossus: bool,
}

/// `by` (the game's "last hit by", as a key) is not the one it named the last time asked: a new hit.
pub fn fresh(by: u64) -> bool {
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    LAST.swap(by, std::sync::atomic::Ordering::Relaxed) != by
}

/// Who hit the Tarnished: `by` (the game's "last hit by") if it just changed (`fresh`), else the
/// nearest enemy close by in an attack that faces Mario (the game's "last hit by" names nobody in
/// some fights, and goes stale: a hit with no attack near, bleeding or a fall, is nobody's).
/// `feet`: Mario's (Elden Ring).
pub fn locate(by: &FieldInsHandle, feet: Vec3, targets: &[combat::Target], fresh: bool) -> Option<Located> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let near = || crate::dodge::attacker(feet, 6.0, (0.0, 1.5), None).map(|f| (f.handle, f.pos, f.id));
    let named = || {
        wcm.chr_ins_by_handle(by).map(|c| {
            let p = c.modules.physics.position;
            (*by, Vec3::new(p.0, p.1, p.2), c.character_id as i32)
        })
    };
    let (handle, pos, id) = if fresh { named().or_else(near)? } else { near()? };
    let key = combat::handle_key(&handle);
    let k = crate::scale();
    let (radius_m, height_m) = targets.iter().find(|t| t.key() == key).map_or((0.6, 1.8), |t| (t.radius * k, t.height * k));
    Some(Located { handle, id, pos, radius_m, colossus: crate::colossus::is_colossus(radius_m, height_m) })
}

/// A Just Guard: no damage, the attacker recoils (a stance blow) with a clang, and the next blows
/// are a counter (dodge.rs, x1.6 for 1.6 s). `at`: who hit him, if known.
pub fn just(i: &starstep::In, combat: &mut Combat, player: &ChrIns, at: Option<&Located>) {
    let toward = at.map_or(Vec3::ZERO, |l| (l.pos - i.feet).with_y(0.0).normalize_or_zero());
    log(format!("hammer: just guard{}", at.map_or(String::new(), |l| format!(" c{:04}", l.id))));
    crate::impact::clash(i.feet + Vec3::Y + toward * 0.7);
    if let Some(l) = at {
        starstep::strike(i, combat, player, &l.handle, l.pos, l.radius_m, Attack::HammerClang);
    }
    crate::dodge::open_counter(false);
}

/// A blocked blow: a small spark and clang at the head of the hammer in front of him (the blow
/// itself may be too small to knock him back).
pub fn blocked(feet: Vec3, facing: Vec3) {
    crate::impact::block_spark(feet + Vec3::Y + facing.with_y(0.0).normalize_or_zero() * 0.6);
}

/// The third block: the guard breaks (the stun itself is a scripted move, lib.rs queues it).
pub fn broken(feet: Vec3) {
    log("hammer: guard broken");
    crate::impact::guard_break(feet + Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_front_is_a_hundred_and_twenty_degrees() {
        let facing = Vec3::Z;
        assert!(frontal(facing, Vec3::new(0.0, 0.0, 3.0)));
        // 59 degrees off: still in front; 61: the side
        let at = |deg: f32| Vec3::new(deg.to_radians().sin(), 0.0, deg.to_radians().cos()) * 2.0;
        assert!(frontal(facing, at(59.0)) && frontal(facing, at(-59.0)));
        assert!(!frontal(facing, at(61.0)) && !frontal(facing, at(-61.0)));
        assert!(!frontal(facing, Vec3::new(0.0, 0.0, -3.0)));
        // (height doesn't count; right on top of him is no front)
        assert!(frontal(facing, Vec3::new(0.0, 4.0, 3.0)));
        assert!(!frontal(facing, Vec3::ZERO));
    }

    #[test]
    fn a_block_costs_a_quarter_and_a_just_guard_nothing() {
        assert_eq!(scaled(Verdict::Through, 1.0), 1.0);
        assert!((scaled(Verdict::Block, 1.0) - 0.25).abs() < 1e-6);
        assert!((scaled(Verdict::Broken, 0.8) - 0.2).abs() < 1e-6);
        assert_eq!(scaled(Verdict::Just, 1.0), 0.0);
    }

    #[test]
    fn the_third_blocked_blow_breaks_the_guard_for_two_seconds() {
        let mut g = Guard::default();
        assert_eq!(g.press(100, 0.0), Press::Guard);
        assert!(g.up());
        // (past the Just Guard's few ticks)
        assert_eq!(g.judge(120, true, false), Verdict::Block);
        assert_eq!(g.judge(130, true, false), Verdict::Block);
        assert_eq!(g.judge(140, true, false), Verdict::Broken);
        assert!(!g.up());
        // no guard for LOCK_TICKS from the break; the roll is not a guard
        g.release(141);
        assert_eq!(g.press(141, 0.0), Press::Locked);
        assert_eq!(g.press(140 + LOCK_TICKS - 1, 0.1), Press::Locked);
        assert_eq!(g.press(141, 0.9), Press::Roll);
        assert_eq!(g.press(140 + LOCK_TICKS, 0.0), Press::Guard);
        // (a fresh guard counts from none)
        assert_eq!(g.judge(140 + LOCK_TICKS + 20, true, false), Verdict::Block);
    }

    #[test]
    fn blocks_do_not_carry_over_a_released_guard() {
        let mut g = Guard::default();
        g.press(100, 0.0);
        assert_eq!(g.judge(120, true, false), Verdict::Block);
        assert_eq!(g.judge(125, true, false), Verdict::Block);
        g.release(126);
        g.press(200, 0.0);
        assert_eq!(g.judge(220, true, false), Verdict::Block);
        assert_eq!(g.judge(225, true, false), Verdict::Block);
        assert_eq!(g.judge(230, true, false), Verdict::Broken);
    }

    #[test]
    fn b_just_before_the_blow_is_the_just_guard() {
        let mut g = Guard::default();
        g.press(100, 0.0);
        assert_eq!(g.judge(105, true, false), Verdict::Just);
        // (one a press: the next blow is a block)
        assert_eq!(g.judge(106, true, false), Verdict::Block);
        let mut g = Guard::default();
        g.press(100, 0.0);
        assert_eq!(g.judge(106, true, false), Verdict::Block);
        // (a new press, a new chance)
        g.release(126);
        g.press(200, 0.0);
        assert_eq!(g.judge(203, true, false), Verdict::Just);
    }

    #[test]
    fn behind_him_or_a_colossus_or_no_guard_goes_through() {
        let mut g = Guard::default();
        assert_eq!(g.judge(100, true, false), Verdict::Through);
        g.press(100, 0.0);
        assert_eq!(g.judge(102, false, false), Verdict::Through);
        assert_eq!(g.judge(102, true, true), Verdict::Through);
        // (through ones are no blocks)
        assert_eq!(g.judge(120, true, false), Verdict::Block);
        assert_eq!(g.judge(121, true, false), Verdict::Block);
        assert_eq!(g.judge(122, true, false), Verdict::Broken);
    }

    #[test]
    fn the_stick_pushed_is_the_roll_and_the_guard_stays_down() {
        let mut g = Guard::default();
        assert_eq!(g.press(100, NEUTRAL), Press::Roll);
        assert!(!g.up());
        assert_eq!(g.press(100, NEUTRAL - 0.01), Press::Guard);
        assert_eq!(g.age(103), Some(3));
        g.release(104);
        assert_eq!(g.age(104), None);
    }

    #[test]
    fn mashing_b_gives_one_just_guard_and_the_guard_still_breaks() {
        let mut g = Guard::default();
        let mut out = Vec::new();
        // B tapped every 6 ticks, a blow 2 ticks after each press
        for i in 0..4u32 {
            let t = i * 6;
            g.press(t, 0.0);
            out.push(g.judge(t + 2, true, false));
            g.release(t + 3);
        }
        assert_eq!(out, [Verdict::Just, Verdict::Block, Verdict::Block, Verdict::Broken]);
    }

    #[test]
    fn a_new_just_guard_waits_for_the_cooldown_and_a_long_pause_resets() {
        let mut g = Guard::default();
        g.press(0, 0.0);
        g.release(10);
        // 12 ticks after the release: still no Just Guard
        g.press(22, 0.0);
        assert_eq!(g.judge(23, true, false), Verdict::Block);
        g.release(24);
        // a pause of a second and more: all new
        g.press(24 + KEEP_TICKS, 0.0);
        assert_eq!(g.judge(24 + KEEP_TICKS + 1, true, false), Verdict::Just);
    }

    #[test]
    fn a_blow_from_nobody_known_or_an_ailment_is_not_guarded() {
        let mut g = Guard::default();
        g.press(100, 0.0);
        assert_eq!(g.judge_source(102, false, None), Verdict::Through);
        assert_eq!(g.judge_source(102, true, Some((true, false))), Verdict::Through);
        // (and they use no Just Guard, no block)
        assert_eq!(g.judge_source(103, false, Some((true, false))), Verdict::Just);
        assert_eq!(g.judge_source(120, false, Some((false, false))), Verdict::Through);
        assert_eq!(g.judge_source(121, false, Some((true, false))), Verdict::Block);
    }
}
