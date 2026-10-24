//! The Chain Chomp's RB (v4 Part 3c, rbmoves.rs): the dark beam, a small and evil kamehameha
//! pouring from his hands for as long as its charge allows (0.4 to 1.2 s), 18 m to the first wall,
//! through everyone in its way; the 10 s Abyss beam, bigger, two and a half seconds, swept round by
//! the stick (his facing: the move steers), drawing regular enemies toward its core. In the game's
//! physics coordinates, as bolt.rs; drawn by elemfx.rs.

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{ChrIns, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// how far it reaches (m), how wide by charge (m); the Abyss beam's
const RANGE: f32 = 18.0;
const ABYSS_RANGE: f32 = 24.0;
const RADIUS: [f32; 4] = [0.35, 0.45, 0.55, 0.65];
const ABYSS_RADIUS: f32 = 1.3;
/// a hit on each enemy in it this often (ticks)
const HIT_EVERY: u32 = 3;
/// the Abyss beam draws regular enemies this far a tick toward its core (m)
const PULL: f32 = 0.06;
/// aimed up or down at the enemy in front at most this far (degrees), the one in front this near
/// (m) inside this cone (cos)
const PITCH_MAX: f32 = 25.0;
const AIM_RANGE: f32 = 20.0;
const AIM_CONE: f32 = 0.6;
/// the Abyss beam's charge level
pub const ABYSS: u8 = 4;
/// (SM64's, soft: a rumble as it pours, the boom as it starts)
const SOUND_HUM: i32 = 0x5073_4081;
const SOUND_BOOM: i32 = 0x300C_8081;

/// Whether a beam from `from` along `dir` (unit), `len` long and `radius` wide, goes through an
/// upright body (feet `feet`, `body_r`, `height`; m).
pub fn in_beam(from: Vec3, dir: Vec3, len: f32, radius: f32, feet: Vec3, body_r: f32, height: f32) -> bool {
    let flat = dir.with_y(0.0);
    let f2 = flat.length_squared();
    // (the point along it nearest the body's axis, seen from above)
    let s = if f2 > 1e-6 { ((feet - from).with_y(0.0).dot(flat) / f2).clamp(0.0, len) } else { 0.0 };
    let p = from + dir * s;
    (feet - p).with_y(0.0).length() <= radius + body_r && p.y >= feet.y - radius && p.y <= feet.y + height + radius
}

struct Live {
    level: u8,
    born: u32,
    ticks: u32,
    pitch: f32,
    from: Vec3,
    to: Vec3,
    /// enemies it hit and when (each again after HIT_EVERY)
    last: HashMap<u64, u32>,
}

/// The beam pouring now, if any.
#[derive(Default)]
pub struct Beam {
    live: Option<Live>,
}

/// What the HUD draws (fx.rs).
static LOOKS: Mutex<Vec<crate::fx::Beam>> = Mutex::new(Vec::new());

pub fn looks() -> Vec<crate::fx::Beam> {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn clear() {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn sound(id: i32, volume: f32) {
    worker::call("beam sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

fn reach(level: u8) -> (f32, f32) {
    if level >= ABYSS { (ABYSS_RANGE, ABYSS_RADIUS) } else { (RANGE, RADIUS[level.min(3) as usize]) }
}

/// The beam's way from facing `facing` (level) tipped `pitch` degrees up.
fn way(facing: Vec3, pitch: f32) -> Vec3 {
    let f = facing.with_y(0.0).normalize_or(Vec3::Z);
    let p = pitch.to_radians();
    (f * p.cos() + Vec3::Y * p.sin()).normalize_or(f)
}

impl Beam {
    /// It starts from his hands `hand` (Elden Ring world), he faces `facing`: `level` 0-3 (4: the
    /// Abyss beam), for `ticks`; aimed up or down at the enemy in front, if one is.
    pub fn start(&mut self, level: u8, ticks: u32, hand: Vec3, facing: Vec3, origin: [f32; 3], tick: u32) {
        let f = facing.with_y(0.0).normalize_or(Vec3::Z);
        let pitch = combat::nearby(&HavokPosition(hand.x, hand.y, hand.z, 0.0), AIM_RANGE + 2.0, origin)
            .iter()
            .filter(|t| !t.friendly() && t.shootable().is_some())
            .filter_map(|t| {
                let chest = t.feet() + Vec3::Y * (t.height * crate::scale() * 0.5).min(1.4);
                let d = chest - hand;
                let l = d.with_y(0.0).length();
                (l > 0.5 && l <= AIM_RANGE && d.with_y(0.0).dot(f) / l >= AIM_CONE).then(|| (l, d.y.atan2(l).to_degrees()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(0.0, |(_, p)| p.clamp(-PITCH_MAX, PITCH_MAX));
        self.live = Some(Live { level, born: tick, ticks, pitch, from: hand, to: hand + way(f, pitch) * reach(level).0, last: HashMap::new() });
        sound(SOUND_BOOM, if level >= ABYSS { 0.45 } else { 0.25 });
        if level >= ABYSS {
            crate::lakitu::shake(0.6);
        }
        log(format!("beam: {} for {ticks} ticks{}", if level >= ABYSS { "the Abyss beam".to_string() } else { format!("a dark beam, level {level}") }, if pitch != 0.0 { format!(", aimed {pitch:.0} degrees") } else { String::new() }));
    }

    /// Game thread, every SM64 tick: from his hands `hand` the way he faces `facing` (Elden Ring),
    /// while its move plays (`casting`): to the first wall, hitting whoever is in it every few
    /// ticks, the Abyss beam drawing them in; its look published.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, hand: Vec3, facing: Vec3, casting: bool, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let over = self.live.as_ref().is_some_and(|b| !casting || tick.wrapping_sub(b.born) >= b.ticks);
        if over {
            self.live = None;
        }
        let Some(b) = self.live.as_mut() else {
            LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
            return;
        };
        let (range, radius) = reach(b.level);
        let dir = way(facing, b.pitch);
        let len = ray(hand, hand + dir * range).map_or(range, |p| (p - hand).length());
        b.from = hand;
        b.to = hand + dir * len;
        let age = tick.wrapping_sub(b.born);
        if age % HIT_EVERY == 0 {
            let mid = (b.from + b.to) * 0.5;
            let near = combat::nearby(&HavokPosition(mid.x, mid.y, mid.z, 0.0), len * 0.5 + radius + 3.0, origin);
            let attack = if b.level >= ABYSS { Attack::AbyssBeam } else { Attack::DarkBeam(b.level.min(3)) };
            let inside: Vec<usize> = near
                .iter()
                .enumerate()
                .filter(|(_, t)| !t.friendly() && t.shootable().is_some())
                .filter(|(_, t)| b.last.get(&t.key()).is_none_or(|&at| tick.wrapping_sub(at) >= HIT_EVERY))
                .filter(|(_, t)| in_beam(b.from, dir, len, radius, t.feet(), t.radius * crate::scale(), t.height * crate::scale()))
                .map(|(i, _)| i)
                .collect();
            if !inside.is_empty() {
                let hits: Vec<(usize, Attack, bool)> = inside.iter().map(|&i| (i, attack, false)).collect();
                combat.deal(player, &near, &hits, tick);
                for &i in &inside {
                    b.last.insert(near[i].key(), tick);
                    crate::impact::element_spark(near[i].feet() + Vec3::Y * 0.9, [0.8, 0.1, 0.2], 0.7);
                }
                // the Abyss beam draws regular enemies toward its core
                if b.level >= ABYSS && !crate::coop::guest() {
                    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
                        for &i in &inside {
                            let Some(h) = near[i].shootable() else { continue };
                            if !combat::liftable(h) || crate::juggle::is_juggled(h) || crate::carry::is_carried(h) {
                                continue;
                            }
                            let Some(chr) = wcm.chr_ins_by_handle_mut(h) else { continue };
                            let ph = &mut chr.modules.physics;
                            let at = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
                            let s = ((at - b.from).dot(dir)).clamp(0.0, len);
                            let core = b.from + dir * s;
                            let pull = (core - at).with_y(0.0).clamp_length_max(PULL);
                            let chest = at + Vec3::Y * 0.8;
                            if pull.length() > 1e-3 && ray(chest, chest + pull * 4.0).is_none() {
                                let to = at + pull;
                                ph.position = HavokPosition(to.x, to.y, to.z, 0.0);
                                ph.chr_proxy_pos_update_requested = true;
                            }
                        }
                    }
                }
            }
            b.last.retain(|_, at| tick.wrapping_sub(*at) < 30);
        }
        if age % 12 == 6 {
            sound(SOUND_HUM, if b.level >= ABYSS { 0.25 } else { 0.15 });
        }
        let t = age as f32 / b.ticks.max(1) as f32;
        *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = vec![crate::fx::Beam { from: b.from, to: b.to, radius, t, kind: (b.level >= ABYSS) as u8 }];
    }

    /// The game re-based its physics coordinates by `d`.
    pub fn shift(&mut self, d: Vec3) {
        if let Some(b) = self.live.as_mut() {
            b.from += d;
            b.to += d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whoever_stands_in_the_beam_is_in_it() {
        let (from, dir, len, r) = (Vec3::new(0.0, 1.0, 0.0), Vec3::Z, 10.0, 0.4);
        // (on its axis, a regular body)
        assert!(in_beam(from, dir, len, r, Vec3::new(0.0, 0.0, 5.0), 0.4, 1.8));
        // (beside it: within its width and his body's)
        assert!(in_beam(from, dir, len, r, Vec3::new(0.7, 0.0, 5.0), 0.4, 1.8));
        assert!(!in_beam(from, dir, len, r, Vec3::new(1.2, 0.0, 5.0), 0.4, 1.8));
        // (behind his hands, past its end)
        assert!(!in_beam(from, dir, len, r, Vec3::new(0.0, 0.0, -2.0), 0.4, 1.8));
        assert!(!in_beam(from, dir, len, r, Vec3::new(0.0, 0.0, 11.5), 0.4, 1.8));
        // (a dog under it: too low; a tall one it crosses at his knees)
        assert!(!in_beam(from, dir, len, r, Vec3::new(0.0, 0.0, 5.0), 0.4, 0.3));
        assert!(in_beam(from, dir, len, r, Vec3::new(0.0, 0.8, 5.0), 0.6, 4.0));
        // (tipped up 20 degrees: it passes over a man far off)
        let up = way(Vec3::Z, 20.0);
        assert!(!in_beam(from, up, len, r, Vec3::new(0.0, 0.0, 9.0), 0.4, 1.8));
        assert!(in_beam(from, up, len, r, Vec3::new(0.0, 0.0, 2.0), 0.4, 1.8));
    }
}
