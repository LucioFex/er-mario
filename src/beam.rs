//! The Chain Chomp's RB (v4 Part 3c, rbmoves.rs): the dark beam, a small and evil kamehameha
//! pouring from his hands for as long as its charge allows (0.6 to 1.8 s), 18 m to the first wall,
//! through everyone in its way, pushing regular enemies back a little; from its second level on
//! the stick swings it round (his facing: the move steers), and it swings itself a little onto the
//! enemy nearest its way. The 10 s Abyss beam: bigger, three seconds, drawing regular enemies
//! toward its core, bursting where it ends as it goes out. Each hit seen and heard. In the game's
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
pub(crate) const HIT_EVERY: u32 = 3;
/// the Abyss beam draws regular enemies this far a tick toward its core (m)
const PULL: f32 = 0.06;
/// aimed up or down at the enemy in front at most this far (degrees), the one in front this near
/// (m) inside this cone (cos)
const PITCH_MAX: f32 = 25.0;
const AIM_RANGE: f32 = 20.0;
const AIM_CONE: f32 = 0.6;
/// it swings itself at most this far off his facing onto an enemy (degrees), this fast (a tick)
const AIM_YAW: f32 = 25.0;
const AIM_RATE: f32 = 3.0;
/// the dark beam pushes regular enemies in it back this far a tick (m)
const PUSH: f32 = 0.05;
/// the Abyss beam's burst as it goes out: this far round its end (m), worth this many of its hits
const BURST: f32 = 4.0;
const BURST_WORTH: f32 = 6.0;
/// its head shoots out this far a tick (m: 18 m in a quarter second), the Abyss beam's
const GROW: f32 = 2.4;
const ABYSS_GROW: f32 = 1.6;
/// a hit's burst and sound on each enemy no more often than this (ticks)
const HIT_HEARD: u32 = 6;
/// the Abyss beam's charge level
pub const ABYSS: u8 = 4;
/// (SM64's, soft: a rumble as it pours, the boom as it starts)
const SOUND_HUM: i32 = 0x5073_4081;
const SOUND_BOOM: i32 = 0x300C_8081;

/// How far its head has got (m) `age` ticks after it began.
fn grown(age: u32, abyss: bool) -> f32 {
    0.5 + age as f32 * if abyss { ABYSS_GROW } else { GROW }
}

/// One tick of the beam's own swing (degrees off his facing) toward `want` (the enemy nearest its
/// way, off his facing; none: his facing), AIM_RATE a tick at most, AIM_YAW at most.
fn aim_step(off: f32, want: Option<f32>) -> f32 {
    let to = want.map_or(0.0, |w| w.clamp(-AIM_YAW, AIM_YAW));
    off + (to - off).clamp(-AIM_RATE, AIM_RATE)
}

/// The push a regular enemy in a beam going `dir` gets a tick (along the ground); the Abyss beam
/// draws them in instead.
fn push(dir: Vec3, abyss: bool) -> Vec3 {
    if abyss {
        return Vec3::ZERO;
    }
    dir.with_y(0.0).normalize_or_zero() * PUSH
}

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
    /// its own swing onto an enemy (degrees off his facing)
    yaw: f32,
    from: Vec3,
    to: Vec3,
    /// enemies it hit and when (each again after HIT_EVERY), and when each one's hit was last seen
    /// and heard (every HIT_HEARD)
    last: HashMap<u64, u32>,
    heard: HashMap<u64, u32>,
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
        self.live = Some(Live { level, born: tick, ticks, pitch, yaw: 0.0, from: hand, to: hand + way(f, pitch) * reach(level).0, last: HashMap::new(), heard: HashMap::new() });
        sound(SOUND_BOOM, if level >= ABYSS { 0.45 } else { 0.25 });
        if level >= ABYSS {
            crate::lakitu::shake(0.6);
        }
        log(format!("beam: {} for {ticks} ticks{}", if level >= ABYSS { "the Abyss beam".to_string() } else { format!("a dark beam, level {level}") }, if pitch != 0.0 { format!(", aimed {pitch:.0} degrees") } else { String::new() }));
    }

    /// Game thread, every SM64 tick: from his hands `hand` the way he faces `facing` (Elden Ring),
    /// while its move plays (`casting`): to the first wall, hitting whoever is in it every few
    /// ticks, the Abyss beam drawing them in; its look published. The bursts of its hits (where,
    /// and their level: fireball.rs draws them as blood's).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, hand: Vec3, facing: Vec3, casting: bool, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Vec<(Vec3, u8)> {
        let mut bursts = Vec::new();
        let over = self.live.as_ref().is_some_and(|b| !casting || tick.wrapping_sub(b.born) >= b.ticks);
        if over {
            // (the Abyss beam bursts where it ends as it goes out)
            if let Some(b) = self.live.take().filter(|b| b.level >= ABYSS) {
                let near = combat::nearby(&HavokPosition(b.to.x, b.to.y, b.to.z, 0.0), BURST + 3.0, origin);
                let hits: Vec<(usize, Attack, bool)> = near
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| !t.friendly() && t.shootable().is_some() && (t.feet() - b.to).with_y(0.0).length() <= BURST + t.radius * crate::scale())
                    .map(|(i, _)| (i, Attack::AbyssBeam, false))
                    .collect();
                if !hits.is_empty() {
                    combat.deal_scaled(player, &near, &hits, tick, BURST_WORTH);
                }
                bursts.push((b.to, 3));
                crate::lakitu::shake(0.5);
                sound(SOUND_BOOM, 0.45);
                log(format!("beam: the Abyss beam bursts ({} caught)", hits.len()));
            }
        }
        let Some(b) = self.live.as_mut() else {
            LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
            return bursts;
        };
        let (range, radius) = reach(b.level);
        // (it swings itself a little onto the enemy nearest its way, and tips up or down to him)
        let f = facing.with_y(0.0).normalize_or(Vec3::Z);
        let mark = combat::nearby(&HavokPosition(hand.x, hand.y, hand.z, 0.0), AIM_RANGE + 2.0, origin)
            .iter()
            .filter(|t| !t.friendly() && t.shootable().is_some())
            .filter_map(|t| {
                let chest = t.feet() + Vec3::Y * (t.height * crate::scale() * 0.5).min(1.4);
                let d = chest - hand;
                let flat = d.with_y(0.0);
                let l = flat.length();
                (l > 0.5 && l <= AIM_RANGE && flat.dot(f) / l >= AIM_CONE).then(|| (l, f.cross(flat / l).y.atan2(f.dot(flat / l)).to_degrees(), d.y.atan2(l).to_degrees()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        b.yaw = aim_step(b.yaw, mark.map(|m| m.1));
        if let Some((_, _, p)) = mark {
            b.pitch += (p.clamp(-PITCH_MAX, PITCH_MAX) - b.pitch).clamp(-AIM_RATE, AIM_RATE);
        }
        let dir = way(glam::Quat::from_rotation_y(b.yaw.to_radians()) * f, b.pitch);
        // (shot out of his hands: as far as its head has got, up to the first wall)
        let age = tick.wrapping_sub(b.born);
        let len = ray(hand, hand + dir * range).map_or(range, |p| (p - hand).length()).min(grown(age, b.level >= ABYSS));
        b.from = hand;
        b.to = hand + dir * len;
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
                let mut heard = false;
                for &i in &inside {
                    let key = near[i].key();
                    // (each hit seen where it strikes him: sparks, and a burst every few ticks, heard)
                    let at = near[i].feet() + Vec3::Y * (near[i].height * crate::scale() * 0.5).clamp(0.4, 1.4);
                    let strike = b.from + dir * (at - b.from).dot(dir).clamp(0.0, len);
                    crate::impact::element_spark(strike, [0.85, 0.08, 0.18], 1.0);
                    if b.heard.get(&key).is_none_or(|&was| tick.wrapping_sub(was) >= HIT_HEARD) {
                        bursts.push((strike, if b.level >= ABYSS { 2 } else { 1 }));
                        b.heard.insert(key, tick);
                        heard = true;
                    }
                    b.last.insert(key, tick);
                }
                if heard {
                    crate::sfx::play(crate::sfx::Sfx::BeamHit, if b.level >= ABYSS { 0.5 } else { 0.35 });
                }
                // the dark beam pushes regular enemies back a little (the Abyss beam draws them in)
                let shove = push(dir, b.level >= ABYSS) * HIT_EVERY as f32;
                if shove != Vec3::ZERO && !crate::coop::guest() {
                    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
                        for &i in &inside {
                            let Some(h) = near[i].shootable() else { continue };
                            let busy = crate::juggle::is_juggled(h) || crate::carry::is_carried(h) || crate::knock::is_flying(h) || crate::bat::is_flying(h) || crate::swing::is_down(h) || crate::stopwatch::frozen(h);
                            if !combat::liftable(h) || busy {
                                continue;
                            }
                            let Some(chr) = wcm.chr_ins_by_handle_mut(h) else { continue };
                            let ph = &mut chr.modules.physics;
                            let at = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
                            let chest = at + Vec3::Y * 0.8;
                            if ray(chest, chest + shove * 3.0).is_none() {
                                let to = at + shove;
                                ph.position = HavokPosition(to.x, to.y, to.z, 0.0);
                                ph.chr_proxy_pos_update_requested = true;
                            }
                        }
                    }
                }
                // the Abyss beam draws regular enemies toward its core
                if b.level >= ABYSS && !crate::coop::guest() {
                    if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
                        for &i in &inside {
                            let Some(h) = near[i].shootable() else { continue };
                            let busy = crate::juggle::is_juggled(h) || crate::carry::is_carried(h) || crate::knock::is_flying(h) || crate::bat::is_flying(h) || crate::swing::is_down(h) || crate::stopwatch::frozen(h);
                            if !combat::liftable(h) || busy {
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
        bursts
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

    /// The beam swings itself onto the enemy nearest its way (a little: within AIM_YAW of his
    /// facing), eased there, and back to his facing when there's none.
    #[test]
    fn the_beam_aims_itself_a_little() {
        // (an enemy 20 degrees to the left: on him within half a second)
        let mut off = 0.0;
        for _ in 0..15 {
            off = aim_step(off, Some(20.0));
        }
        assert!((off - 20.0).abs() < 2.0, "{off}");
        // (one 60 degrees off: as far as it goes)
        let mut off = 0.0;
        for _ in 0..30 {
            off = aim_step(off, Some(60.0));
        }
        assert!((off - AIM_YAW).abs() < 0.5, "{off}");
        // (none: back to his facing)
        for _ in 0..30 {
            off = aim_step(off, None);
        }
        assert!(off.abs() < 1.0, "{off}");
    }

    /// A kamehameha: it shoots out of his hands and reaches its end in a moment (the user: it was
    /// there whole at once), the Abyss beam a little slower; nothing past its head is hit.
    #[test]
    fn the_beam_shoots_out_of_his_hands() {
        assert!(grown(0, false) < 1.0);
        assert!(grown(4, false) > 6.0 && grown(4, false) < RANGE);
        assert!(grown(8, false) >= RANGE);
        assert!(grown(8, true) < grown(8, false) && grown(16, true) >= ABYSS_RANGE);
        assert!((1..20).all(|t| grown(t, false) > grown(t - 1, false)));
    }

    /// A regular enemy in the beam is pushed back along it a little each tick (not a boss).
    #[test]
    fn the_beam_pushes_them_back_a_little() {
        let d = push(Vec3::Z, false);
        assert!(d.z > 0.02 && d.z < 0.1 && d.y == 0.0, "{d}");
        // (tipped up: still pushed along the ground)
        let up = way(Vec3::Z, 20.0);
        assert!(push(up, false).y == 0.0);
        // (the Abyss beam draws them in instead: no push)
        assert_eq!(push(Vec3::Z, true), Vec3::ZERO);
    }
}
