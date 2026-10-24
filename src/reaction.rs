//! Pair finishers (v4 Part 3d): a string's finisher with one weapon, and right after it L1 brings
//! in another one's swap strike: the two elements react on the enemy it finished on (a burst of
//! their own, "MIX!", once a string; lib.rs sees the swap and asks combat.rs for the finisher).
//! - fire + frost: steam, a burst round him, small enemies staggered
//! - fire + lightning: plasma, a blast of fire and lightning
//! - fire + blood: his blood boils, a heavy tick on him
//! - fire + poison: ignition, the poison carpets near him go up in flames (carpet.rs)
//! - frost + lightning: shatter, a ring of ice shards, much more stance
//! - frost + blood: crimson spikes rising under him
//! - frost + poison: toxic hail, pellets of poisoned ice falling round him
//! - lightning + blood: blood bolts jumping from him to the three nearest
//! - lightning + poison: a corrosive arc to every poisoned enemy near
//! - blood + poison: rot, a slow tick on him for four seconds

use std::sync::Mutex;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::element::Element;
use crate::{log, sm64, worker};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    Steam,
    Plasma,
    Boiling,
    Ignition,
    Shatter,
    CrimsonSpikes,
    ToxicHail,
    BloodBolts,
    CorrosiveArc,
    Rot,
}

/// What two elements make together (either way round); none of one with itself.
pub fn of(a: Element, b: Element) -> Option<Reaction> {
    use Element::*;
    let pair = |x: Element, y: Element| (a == x && b == y) || (a == y && b == x);
    Some(if pair(Fire, Frost) {
        Reaction::Steam
    } else if pair(Fire, Lightning) {
        Reaction::Plasma
    } else if pair(Fire, Blood) {
        Reaction::Boiling
    } else if pair(Fire, Poison) {
        Reaction::Ignition
    } else if pair(Frost, Lightning) {
        Reaction::Shatter
    } else if pair(Frost, Blood) {
        Reaction::CrimsonSpikes
    } else if pair(Frost, Poison) {
        Reaction::ToxicHail
    } else if pair(Lightning, Blood) {
        Reaction::BloodBolts
    } else if pair(Lightning, Poison) {
        Reaction::CorrosiveArc
    } else if pair(Blood, Poison) {
        Reaction::Rot
    } else {
        return None;
    })
}

impl Reaction {
    pub fn name(self) -> &'static str {
        match self {
            Reaction::Steam => "steam",
            Reaction::Plasma => "plasma",
            Reaction::Boiling => "boiling",
            Reaction::Ignition => "ignition",
            Reaction::Shatter => "shatter",
            Reaction::CrimsonSpikes => "crimson spikes",
            Reaction::ToxicHail => "toxic hail",
            Reaction::BloodBolts => "blood bolts",
            Reaction::CorrosiveArc => "corrosive arc",
            Reaction::Rot => "rot",
        }
    }
}

/// a string's finisher this recent (ticks) before the swap strike makes a pair
pub const WINDOW: u32 = 20;
/// how far each reaches (m)
const STEAM_REACH: f32 = 3.0;
const PLASMA_REACH: f32 = 4.0;
const SHATTER_REACH: f32 = 3.5;
const IGNITE_NEAR: f32 = 8.0;
const IGNITE_FALLBACK: f32 = 2.0;
const BOLTS_REACH: f32 = 8.0;
const BOLTS: usize = 3;
const CORRODE_REACH: f32 = 10.0;
const HAIL: u32 = 5;
const HAIL_EVERY: u32 = 4;
const HAIL_REACH: f32 = 1.3;
const ROT_TICKS: u32 = 8;
const ROT_EVERY: u32 = 15;
/// the boiling's burn after it: ticks of it, how often
const BURNS: u32 = 3;
const BURN_EVERY: u32 = 10;
const SHATTER_POISE: f32 = 25.0;
/// (SM64's, soft)
const SOUND_FLAME: i32 = 0x5055_8081;
const SOUND_SNOW: i32 = 0x5065_D081;
const SOUND_ZAP: i32 = 0x1416_0001;
const SOUND_BITE: i32 = 0x5073_4081;
const SOUND_BUBBLE: i32 = 0x300B_0081;
const SOUND_STAR: i32 = 0x3016_0091;

/// What comes later: the hail's pellets, the rot's ticks (who, where, what, when, how many more).
static PENDING: Mutex<Vec<(FieldInsHandle, Vec3, Reaction, u32, u32)>> = Mutex::new(Vec::new());

fn sound(id: i32, volume: f32) {
    worker::call("reaction sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

/// Where a character's feet and chest are now (Elden Ring world), if it's alive.
fn place(h: &FieldInsHandle) -> Option<(Vec3, Vec3)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let p = chr.modules.physics.position;
    let feet = Vec3::new(p.0, p.1, p.2);
    Some((feet, feet + Vec3::Y * 0.9))
}

/// `attack` on `h` alone.
fn strike(h: &FieldInsHandle, attack: Attack, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
    let Some((feet, _)) = place(h) else { return };
    let near = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), 4.0, origin);
    if let Some(i) = near.iter().position(|t| t.shootable() == Some(h)) {
        combat.deal(player, &near, &[(i, attack, false)], tick);
    }
}

/// A ring of sparks of `tint` round `at`, `r` wide.
fn ring(at: Vec3, r: f32, tint: [f32; 3], n: usize, k: f32) {
    for i in 0..n {
        let a = i as f32 * std::f32::consts::TAU / n as f32;
        crate::impact::element_spark(at + Vec3::new(a.cos() * r, 0.3, a.sin() * r), tint, k);
    }
}

/// The enemies near `at` (m): handle, chest.
fn enemies(at: Vec3, reach: f32, origin: [f32; 3]) -> Vec<(FieldInsHandle, Vec3)> {
    combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), reach + 2.0, origin)
        .iter()
        .filter(|t| !t.friendly())
        .filter_map(|t| Some((*t.shootable()?, t.feet() + Vec3::Y * 0.9)))
        .filter(|(_, c)| (*c - at).length() <= reach + 1.0)
        .collect()
}

/// The pair `r` goes off on `target` (its burst, its blows), "MIX!". False if it couldn't (he's
/// gone).
pub fn fire(r: Reaction, target: FieldInsHandle, carpets: &mut crate::carpet::Carpets, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) -> bool {
    let Some((feet, chest)) = place(&target) else { return false };
    log(format!("reaction: {} (a pair finisher)", r.name()));
    crate::style::callout("MIX!", Some(chest + Vec3::Y * 1.2), [1.0, 0.6, 1.0], true, 6.0);
    sound(SOUND_STAR, 0.22);
    crate::lakitu::shake(0.5);
    let key = combat::handle_key(&target);
    match r {
        Reaction::Steam => {
            combat.blast(player, feet, STEAM_REACH, Attack::Steam, origin, tick);
            ring(feet, STEAM_REACH * 0.6, [0.95, 0.95, 1.0], 8, 1.6);
            sound(SOUND_FLAME, 0.3);
        }
        Reaction::Plasma => {
            combat.blast(player, feet, PLASMA_REACH, Attack::Plasma, origin, tick);
            crate::impact::fire_blast(feet, PLASMA_REACH);
            for k in 0..3 {
                let a = k as f32 * 2.1;
                crate::bolt::slash(chest + Vec3::new(a.cos() * 2.5, 5.0, a.sin() * 2.5), chest, 0.2);
            }
            sound(SOUND_ZAP, 0.3);
        }
        Reaction::Boiling => {
            strike(&target, Attack::Boiling, player, combat, origin, tick);
            ring(chest, 0.6, [1.0, 0.35, 0.1], 6, 1.2);
            // (and it burns on a moment)
            PENDING.lock().unwrap_or_else(|e| e.into_inner()).push((target, feet, r, tick + BURN_EVERY, BURNS));
            sound(SOUND_BITE, 0.35);
        }
        Reaction::Ignition => {
            // (the carpets near him go up: whoever stands in any of them takes it once)
            let mut lit = carpets.ignite(feet, IGNITE_NEAR);
            if lit.is_empty() {
                lit.push((feet, IGNITE_FALLBACK));
            }
            let reach = lit.iter().map(|(c, r)| (*c - feet).length() + r).fold(0.0f32, f32::max);
            let near = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), reach + 2.0, origin);
            let hits: Vec<(usize, Attack, bool)> = near
                .iter()
                .enumerate()
                .filter(|(_, t)| !t.friendly() && t.shootable().is_some())
                .filter(|(_, t)| {
                    let f = t.feet();
                    lit.iter().any(|(c, r)| (f - *c).with_y(0.0).length() <= r + t.radius * crate::scale() && (f.y - c.y).abs() < 2.0)
                })
                .map(|(i, _)| (i, Attack::Ignition, false))
                .collect();
            if !hits.is_empty() {
                combat.deal(player, &near, &hits, tick);
            }
            for (c, radius) in &lit {
                crate::impact::fire_blast(*c, *radius);
            }
            log(format!("reaction: ignition, {} carpets of poison go up ({} caught)", lit.len(), hits.len()));
            sound(SOUND_FLAME, 0.4);
        }
        Reaction::Shatter => {
            combat.blast(player, feet, SHATTER_REACH, Attack::Shatter, origin, tick);
            if !crate::coop::guest() {
                crate::swing::add_stance(&target, SHATTER_POISE);
            }
            ring(feet, SHATTER_REACH * 0.7, [0.7, 0.9, 1.0], 10, 1.3);
            sound(SOUND_SNOW, 0.45);
        }
        Reaction::CrimsonSpikes => {
            strike(&target, Attack::CrimsonSpikes, player, combat, origin, tick);
            for k in 0..5 {
                let a = k as f32 * 1.26;
                crate::impact::element_spark(feet + Vec3::new(a.cos() * 0.5, 0.2 + 0.3 * k as f32, a.sin() * 0.5), [0.8, 0.05, 0.15], 1.3);
            }
            sound(SOUND_SNOW, 0.35);
            sound(SOUND_BITE, 0.3);
        }
        Reaction::ToxicHail => {
            PENDING.lock().unwrap_or_else(|e| e.into_inner()).push((target, feet, r, tick + HAIL_EVERY, HAIL));
            sound(SOUND_SNOW, 0.3);
        }
        Reaction::BloodBolts => {
            let mut near: Vec<(f32, FieldInsHandle, Vec3)> =
                enemies(chest, BOLTS_REACH, origin).into_iter().filter(|(h, _)| combat::handle_key(h) != key).map(|(h, c)| ((c - chest).length(), h, c)).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, h, c) in near.into_iter().take(BOLTS) {
                crate::bolt::slash(chest, c, 0.16);
                strike(&h, Attack::BloodBolt, player, combat, origin, tick);
            }
            crate::impact::element_spark(chest, [1.0, 0.3, 0.4], 1.3);
            sound(SOUND_ZAP, 0.3);
        }
        Reaction::CorrosiveArc => {
            for (h, c) in enemies(chest, CORRODE_REACH, origin) {
                if combat::handle_key(&h) == key || crate::element::poisoned(&h) {
                    if combat::handle_key(&h) != key {
                        crate::bolt::slash(chest, c, 0.14);
                    }
                    strike(&h, Attack::CorrosiveArc, player, combat, origin, tick);
                }
            }
            sound(SOUND_ZAP, 0.3);
            sound(SOUND_BUBBLE, 0.3);
        }
        Reaction::Rot => {
            PENDING.lock().unwrap_or_else(|e| e.into_inner()).push((target, feet, r, tick + ROT_EVERY, ROT_TICKS));
            ring(chest, 0.5, [0.45, 0.6, 0.2], 6, 1.0);
            sound(SOUND_BUBBLE, 0.35);
        }
    }
    true
}

/// Game thread, every SM64 tick: the hail's pellets falling, the rot's ticks.
pub fn tick(player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
    let due: Vec<_> = {
        let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        let (due, wait): (Vec<_>, Vec<_>) = p.drain(..).partition(|e| tick >= e.3);
        *p = wait;
        due
    };
    for (h, at, r, _, left) in due {
        match r {
            Reaction::ToxicHail => {
                // (round where he stood, a pellet at a time)
                let seed = left as f32 * 2.399;
                let p = at + Vec3::new(seed.cos() * 1.2, 0.0, seed.sin() * 1.2);
                combat.blast(player, p, HAIL_REACH, Attack::Hail, origin, tick);
                crate::impact::element_spark(p + Vec3::Y * 0.4, [0.5, 0.9, 0.4], 1.2);
                if left > 1 {
                    PENDING.lock().unwrap_or_else(|e| e.into_inner()).push((h, at, r, tick + HAIL_EVERY, left - 1));
                }
            }
            Reaction::Rot | Reaction::Boiling => {
                if place(&h).is_none() {
                    continue;
                }
                let (attack, every) = if r == Reaction::Rot { (Attack::Rot, ROT_EVERY) } else { (Attack::Ember, BURN_EVERY) };
                strike(&h, attack, player, combat, origin, tick);
                if left > 1 {
                    PENDING.lock().unwrap_or_else(|e| e.into_inner()).push((h, at, r, tick + every, left - 1));
                }
            }
            _ => {}
        }
    }
}

pub fn clear() {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// The game re-based its physics coordinates by `d`: the hail's spots move with them.
pub fn shift(d: Vec3) {
    for e in PENDING.lock().unwrap_or_else(|e| e.into_inner()).iter_mut() {
        e.1 += d;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Element::*;

    #[test]
    fn two_elements_make_one_reaction_either_way_round() {
        let all = [Fire, Frost, Blood, Poison, Lightning];
        let mut seen = Vec::new();
        for (i, &a) in all.iter().enumerate() {
            assert_eq!(of(a, a), None);
            for &b in &all[i + 1..] {
                let r = of(a, b).expect("a pair reacts");
                assert_eq!(of(b, a), Some(r));
                assert!(!seen.contains(&r), "{r:?} twice");
                seen.push(r);
            }
        }
        assert_eq!(seen.len(), 10);
        assert_eq!(of(Fire, Frost), Some(Reaction::Steam));
        assert_eq!(of(Poison, Blood), Some(Reaction::Rot));
        assert_eq!(of(Lightning, Poison), Some(Reaction::CorrosiveArc));
    }
}
