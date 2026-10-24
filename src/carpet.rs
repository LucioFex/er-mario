//! The boomerang's RB (v4 Part 3c, rbmoves.rs): carpets of poison on the ground. The arm sweep
//! sprays one over its arc in front of him (wider and longer lasting by charge), the spray in the
//! air a pool where it lands, the 10 s toxic swamp a wide one round him that bubbles and slows the
//! enemies in it. Whoever walks in one is poisoned a little every half second (and builds poison
//! up, element.rs). In the game's physics coordinates, as bolt.rs; drawn by elemfx.rs.

use std::sync::Mutex;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// a tick of poison on whoever is in one this often (ticks); the swamp's slow looked at this often
const BURN_EVERY: u32 = 15;
const SWAMP_EVERY: u32 = 5;
/// it spreads out over this many ticks
const GROW: u32 = 8;
/// how far above or below it a body's feet still count as in it (m)
const BELOW: f32 = 1.0;
const ABOVE: f32 = 2.0;
/// at most this many at once (the oldest dries up)
const MOST: usize = 6;
/// the swamp slows whoever is in it to this (their animations' speed); a boss only a little (no
/// cheese)
const SLOW: f32 = 0.6;
const SLOW_BOSS: f32 = 0.85;
/// (SM64's, soft: its bubbling)
const SOUND_BUBBLE: i32 = 0x300B_0081;

/// Whether feet at `feet` stand in a carpet round `center` `radius` m wide, over the arc `half`
/// radians either side of `dir` (level; π or more: all round).
pub fn inside(center: Vec3, radius: f32, dir: Vec3, half: f32, feet: Vec3) -> bool {
    let d = (feet - center).with_y(0.0);
    let dy = feet.y - center.y;
    if d.length() > radius || dy < -BELOW || dy > ABOVE {
        return false;
    }
    if half >= std::f32::consts::PI - 1e-3 || d.length() < 0.3 {
        return true;
    }
    d.normalize().angle_between(dir.with_y(0.0).normalize_or(Vec3::Z)) <= half
}

struct Carpet {
    center: Vec3,
    radius: f32,
    dir: Vec3,
    half: f32,
    born: u32,
    life: u32,
    swamp: bool,
}

impl Carpet {
    /// how far it has spread (m)
    fn spread(&self, tick: u32) -> f32 {
        self.radius * (tick.wrapping_sub(self.born) as f32 / GROW as f32).min(1.0)
    }
}

/// The carpets out there, and the enemies the swamp has slowed.
#[derive(Default)]
pub struct Carpets {
    out: Vec<Carpet>,
    slowed: Vec<FieldInsHandle>,
    last_burn: u32,
}

/// What the HUD draws (fx.rs).
static LOOKS: Mutex<Vec<crate::fx::Carpet>> = Mutex::new(Vec::new());

pub fn looks() -> Vec<crate::fx::Carpet> {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `h` slowed by the swamp, or given its own speed back.
fn set_slow(h: &FieldInsHandle, slow: bool) {
    if let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(h)) {
        let speed = if !slow { 1.0 } else if combat::boss_class(h, chr) { SLOW_BOSS } else { SLOW };
        chr.modules.behavior.animation_speed = speed;
    }
}

impl Carpets {
    /// A carpet laid round `center` (Elden Ring world, on the ground), `radius` m, over `half`
    /// radians either side of `dir`, for `life` ticks; `swamp`: it slows whoever is in it.
    #[allow(clippy::too_many_arguments)]
    pub fn lay(&mut self, center: Vec3, radius: f32, dir: Vec3, half: f32, life: u32, swamp: bool, tick: u32) {
        if self.out.len() >= MOST {
            self.out.remove(0);
        }
        self.out.push(Carpet { center, radius, dir: dir.with_y(0.0).normalize_or(Vec3::Z), half, born: tick, life, swamp });
        worker::call("carpet sound", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_BUBBLE, 0.4) });
        log(format!("carpet: poison {radius:.1} m round, {:.0} degrees, {:.1} s{}", (half * 2.0).to_degrees().min(360.0), life as f32 / 30.0, if swamp { ", a swamp" } else { "" }));
    }

    /// Game thread, every SM64 tick: whoever is in one is poisoned every half second (once however
    /// many he stands in), the swamp slows them; the dried up go; the looks published.
    pub fn tick(&mut self, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
        self.out.retain(|c| tick.wrapping_sub(c.born) < c.life);
        // who stands in which (the enemies near each)
        let mut in_swamp: Vec<FieldInsHandle> = Vec::new();
        let burn = tick.wrapping_sub(self.last_burn) >= BURN_EVERY;
        if burn {
            self.last_burn = tick;
        }
        let mut done: Vec<u64> = Vec::new();
        // (who's in a swamp, every few ticks; who's in any, on a burn: no scan in between)
        let swamp_tick = tick % SWAMP_EVERY == 0;
        for c in &self.out {
            if !burn && !(c.swamp && swamp_tick) {
                continue;
            }
            let spread = c.spread(tick);
            let near = combat::nearby(&HavokPosition(c.center.x, c.center.y, c.center.z, 0.0), spread + 3.0, origin);
            let inside_now: Vec<usize> = near
                .iter()
                .enumerate()
                .filter(|(_, t)| !t.friendly() && t.shootable().is_some())
                .filter(|(_, t)| inside(c.center, spread + t.radius * crate::scale() * 0.5, c.dir, c.half, t.feet()))
                .map(|(i, _)| i)
                .collect();
            if c.swamp {
                in_swamp.extend(inside_now.iter().filter_map(|&i| near[i].shootable().copied()));
            }
            if burn {
                let hits: Vec<(usize, Attack, bool)> = inside_now.iter().filter(|&&i| !done.contains(&near[i].key())).map(|&i| (i, Attack::VenomCloud, false)).collect();
                if !hits.is_empty() {
                    done.extend(hits.iter().map(|h| near[h.0].key()));
                    combat.deal(player, &near, &hits, tick);
                }
            }
        }
        if burn && !done.is_empty() {
            // (soft: it comes again and again)
            worker::call("carpet bubble", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_BUBBLE, 0.15) });
        }
        // the swamp's slow: on whoever is in it now, given back to whoever left it
        if !crate::coop::guest() && swamp_tick {
            let key = combat::handle_key;
            in_swamp.dedup_by(|a, b| key(a) == key(b));
            for h in self.slowed.iter().filter(|h| !in_swamp.iter().any(|n| key(n) == key(h))) {
                set_slow(h, false);
            }
            for h in in_swamp.iter().filter(|n| !self.slowed.iter().any(|h| key(h) == key(n))) {
                set_slow(h, true);
            }
            self.slowed = in_swamp;
        }
        let looks = self
            .out
            .iter()
            .map(|c| crate::fx::Carpet { center: c.center, radius: c.spread(tick), yaw: c.dir.x.atan2(c.dir.z), half: c.half, t: tick.wrapping_sub(c.born) as f32 / c.life.max(1) as f32, swamp: c.swamp })
            .collect();
        *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = looks;
    }

    /// The fire's ignition (reaction.rs): the carpets within `near` m of `at` go up in flames, gone
    /// (where each was, how wide).
    pub fn ignite(&mut self, at: Vec3, near: f32) -> Vec<(Vec3, f32)> {
        let mut lit = Vec::new();
        self.out.retain(|c| {
            let close = (c.center - at).with_y(0.0).length() <= near + c.radius * 0.5;
            if close {
                lit.push((c.center, c.radius.min(6.0)));
            }
            !close
        });
        lit
    }

    /// All gone (Mario off, a load): the slowed get their speed back.
    pub fn clear(&mut self) {
        for h in self.slowed.drain(..) {
            set_slow(&h, false);
        }
        self.out.clear();
        LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// The game re-based its physics coordinates by `d`.
    pub fn shift(&mut self, d: Vec3) {
        for c in &mut self.out {
            c.center += d;
        }
    }
}

/// (Mario gone or rebuilt: no enemy is left slowed for good)
impl Drop for Carpets {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn whoever_stands_on_it_is_in_it() {
        let (c, dir) = (Vec3::new(0.0, 0.0, 0.0), Vec3::Z);
        let half = (60.0f32).to_radians();
        // (in front, in its arc, near its edge too)
        assert!(inside(c, 3.0, dir, half, Vec3::new(0.5, 0.1, 2.0)));
        assert!(inside(c, 3.0, dir, half, Vec3::new(1.4, 0.0, 1.4)));
        assert!(!inside(c, 3.0, dir, half, Vec3::new(2.0, 0.0, 0.9)));
        // (beside him, outside the arc; behind him)
        assert!(!inside(c, 3.0, dir, half, Vec3::new(2.5, 0.0, 0.5)));
        assert!(!inside(c, 3.0, dir, half, Vec3::new(0.0, 0.0, -2.0)));
        // (past its edge, up a ledge above it)
        assert!(!inside(c, 3.0, dir, half, Vec3::new(0.0, 0.0, 3.5)));
        assert!(!inside(c, 3.0, dir, half, Vec3::new(0.0, 2.5, 1.0)));
        // (all round: anywhere within it)
        assert!(inside(c, 3.0, dir, PI, Vec3::new(0.0, 0.0, -2.0)));
        // (an arc round behind him: the angle wraps past 180)
        assert!(inside(c, 5.0, Vec3::NEG_Z, (120.0f32).to_radians(), Vec3::new(-1.0, 0.0, -3.0)));
        assert!(!inside(c, 5.0, Vec3::NEG_Z, (60.0f32).to_radians(), Vec3::new(0.0, 0.0, 3.0)));
    }
}
