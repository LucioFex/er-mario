//! The Star Sword's RB (v4 Part 2): lightning, moves of its own. On the ground the sword raised
//! (Zelda's Skyward Strike): a bolt shoots up off it and falls on the enemy aimed at; charged, a
//! thicker one with a small blast round him, then three, jumping from one enemy to the next. In
//! the air, Zeus's throw: a bolt javelin hurled at the enemy, bigger charged, splitting in three
//! where it lands. Ten seconds' charge: a storm cloud (Mario Kart's Thunder Cloud) flies over the
//! enemy and rains bolts on everyone under it for four seconds, a giant one to end.
//! All of it 3D in the HUD's overlay (hud.rs: zigzag bolts with branches, the cloud's mesh).

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// a cast goes for the nearest enemy this near (m) within this cone ahead (cos)
const RANGE: f32 = 32.0;
const CONE: f32 = 0.55;
/// the bolt shoots up off the sword this high (m), and falls on him from this high, this many
/// ticks after it went up
const UP: f32 = 12.0;
const SKY: f32 = 22.0;
const FALL_AFTER: u32 = 6;
/// the charged one's blast round him (m); the third level's bolts jump this far (m), a beat apart
const BLAST: f32 = 2.5;
const JUMP_REACH: f32 = 8.0;
const JUMP_AFTER: u32 = 5;
/// the javelin: its speed (m a tick), its turn toward him a tick, how long it flies (ticks), how
/// near it has to come (m, beyond his body), how far what it splits into reaches (m)
const JAVELIN_SPEED: f32 = 1.5;
const JAVELIN_TURN: f32 = 0.12;
const JAVELIN_LIFE: u32 = 60;
const JAVELIN_NEAR: f32 = 0.6;
const SPLIT_REACH: f32 = 6.0;
/// the storm cloud: its flight over him (ticks), its rain (ticks), a bolt this often (ticks), the
/// same enemy no more often than this (ticks), how far round it its bolts fall (m), how high over
/// him it hangs (m), its giant bolt's blast (m)
const STORM_FLY: u32 = 30;
const STORM_RAIN: u32 = 120;
const STORM_EVERY: u32 = 8;
const STORM_SAME: u32 = 16;
const STORM_REACH: f32 = 5.0;
const STORM_HIGH: f32 = 9.0;
const STORM_END_BLAST: f32 = 4.5;
/// how long a bolt shows (ticks), its thickness by level (m)
const LOOK_LIFE: u32 = 7;
const WIDTH: [f32; 5] = [0.12, 0.18, 0.26, 0.2, 0.5];
/// (not verified in game: a soft boom for the thunder, Mario's own shock's zap)
const SOUND_THUNDER: i32 = 0x300C_8081;
const SOUND_ZAP: i32 = 0x1416_0001;
/// the charge level that calls the storm cloud (10 s)
pub const STORM_LEVEL: u8 = 4;
/// a sword move's crack of lightning comes down from this high (m)
const ZAP_UP: f32 = 4.5;

/// A bolt's zigzag from `from` to `to`: `n` segments, each point off the straight line by up to
/// `wiggle` of its length (none at the ends), the same for the same `seed`.
pub fn jag(from: Vec3, to: Vec3, seed: u32, n: usize, wiggle: f32) -> Vec<Vec3> {
    let d = to - from;
    let len = d.length();
    let dir = d.normalize_or(Vec3::NEG_Y);
    let u = dir.any_orthonormal_vector();
    let v = dir.cross(u);
    // (a small hash: the same seed, the same zigzag)
    let rand = |k: u32| {
        let mut x = seed.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0
    };
    (0..=n)
        .map(|i| {
            let t = i as f32 / n.max(1) as f32;
            if i == 0 || i == n {
                return from + d * t;
            }
            // (round the line, at most `wiggle` of its length off it)
            let (a, b) = (rand(i as u32 * 2), rand(i as u32 * 2 + 1));
            let off = (u * a + v * b).clamp_length_max(1.0) * wiggle * len;
            from + d * t + off
        })
        .collect()
}

/// Of `candidates` (where each is, from Mario at `me` facing `facing`), the one a cast goes for: the
/// nearest within `range` m inside the cone (cos `cone`) ahead, else None.
pub fn pick(me: Vec3, facing: Vec3, candidates: &[Vec3], range: f32, cone: f32) -> Option<usize> {
    let f = facing.with_y(0.0).normalize_or(Vec3::Z);
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let d = (*p - me).with_y(0.0);
            let l = d.length();
            (l <= range && l > 0.01 && d.dot(f) / l >= cone).then_some((l, i))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
}

/// A bolt falling on someone (or a spot): when it lands, on whom, with what, and its blast.
#[derive(Clone)]
struct Strike {
    land: u32,
    target: Option<FieldInsHandle>,
    at: Vec3,
    attack: Attack,
    blast: Option<(f32, Attack)>,
    /// bolts left to jump on from him (the third level's), and whom it already struck
    jumps: u8,
    struck: Vec<u64>,
    width: f32,
}

struct Javelin {
    pos: Vec3,
    vel: Vec3,
    target: Option<FieldInsHandle>,
    level: u8,
    born: u32,
}

struct Storm {
    pos: Vec3,
    from: Vec3,
    target: Option<FieldInsHandle>,
    born: u32,
    next: u32,
    hit: HashMap<u64, u32>,
    flash_at: u32,
}

struct Look {
    from: Vec3,
    to: Vec3,
    seed: u16,
    born: u32,
    width: f32,
}

/// The sword's bolts out there.
#[derive(Default)]
pub struct Bolts {
    strikes: Vec<Strike>,
    javelins: Vec<Javelin>,
    storm: Option<Storm>,
    looks: Vec<Look>,
    seed: u16,
}

/// What `looks` and `storm_looks` hand the HUD (the game thread publishes it every tick).
static LOOKS: Mutex<(Vec<crate::fx::Bolt>, Vec<crate::fx::Storm>)> = Mutex::new((Vec::new(), Vec::new()));

/// Cuts of lightning asked for from elsewhere (the sword's MAX, maxfin.rs): drawn as bolts on the
/// next tick (from, to, how thick).
static SLASHES: Mutex<Vec<(Vec3, Vec3, f32)>> = Mutex::new(Vec::new());

/// A cut of lightning from `from` to `to` (a look only).
pub fn slash(from: Vec3, to: Vec3, width: f32) {
    SLASHES.lock().unwrap_or_else(|e| e.into_inner()).push((from, to, width));
}

/// The bolts to draw now (fx.rs).
pub fn looks() -> Vec<crate::fx::Bolt> {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).0.clone()
}

/// The storm cloud to draw now (fx.rs).
pub fn storm_looks() -> Vec<crate::fx::Storm> {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).1.clone()
}

pub fn clear() {
    *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = (Vec::new(), Vec::new());
}

fn sound(id: i32, volume: f32) {
    if id == SOUND_THUNDER {
        return thunder(volume);
    }
    worker::call("bolt sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

/// A thunderclap (sfx.rs: synthesized, a crack and its rumble; louder ones roll on longer), a
/// flash of the screen, the camera jolted.
fn thunder(volume: f32) {
    crate::sfx::play(crate::sfx::Sfx::Thunder(volume >= 0.35), (volume * 1.6).min(1.0));
    crate::impact::flash([0.85, 0.9, 1.0], 0.08 + 0.25 * volume, 0.16);
    crate::lakitu::shake(0.15 + 0.5 * volume);
}

/// Where a character's chest is now (Elden Ring world), if it's still there and alive.
fn chest_of(h: &FieldInsHandle) -> Option<Vec3> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let p = chr.modules.physics.position;
    Some(Vec3::new(p.0, p.1 + 0.9, p.2))
}

/// The enemies near `at` (m): handle, chest, feet.
fn enemies(at: Vec3, reach: f32, origin: [f32; 3]) -> Vec<(FieldInsHandle, Vec3, Vec3)> {
    combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), reach, origin)
        .iter()
        .filter(|t| !t.friendly())
        .filter_map(|t| {
            let h = *t.shootable()?;
            let feet = t.feet();
            Some((h, feet + Vec3::Y * (t.height * crate::scale() * 0.55).min(1.6), feet))
        })
        .collect()
}

impl Bolts {
    /// A cast lets go (the sword raised on the ground, thrown in the air): `level` 0-4, from his
    /// hand `hand` facing `facing` (Elden Ring world).
    pub fn cast(&mut self, level: u8, air: bool, hand: Vec3, facing: Vec3, origin: [f32; 3], tick: u32) {
        let near = enemies(hand, RANGE + 4.0, origin);
        let chests: Vec<Vec3> = near.iter().map(|n| n.1).collect();
        let target = pick(hand, facing, &chests, RANGE, CONE).map(|i| near[i]);
        let level = level.min(STORM_LEVEL);
        log(format!(
            "bolt: level {level} {} ({})",
            if level >= STORM_LEVEL { "storm cloud" } else if air { "javelin" } else { "skyward strike" },
            if target.is_some() { "an enemy aimed at" } else { "no one ahead" }
        ));
        if level >= STORM_LEVEL {
            let from = hand + Vec3::Y * 4.0;
            self.storm = Some(Storm { pos: from, from, target: target.map(|t| t.0), born: tick, next: tick + STORM_FLY, hit: HashMap::new(), flash_at: 0 });
            self.look(hand, from, 0.3, tick);
            sound(SOUND_THUNDER, 0.4);
            crate::lakitu::shake(0.4);
            return;
        }
        let ahead = facing.with_y(0.0).normalize_or(Vec3::Z);
        if air {
            let to = target.map_or(hand + ahead * 10.0 - Vec3::Y * 5.0, |t| t.1);
            let vel = (to - hand).normalize_or(Vec3::NEG_Y) * JAVELIN_SPEED;
            self.javelins.push(Javelin { pos: hand, vel, target: target.map(|t| t.0), level, born: tick });
            sound(SOUND_ZAP, 0.25);
            return;
        }
        // the sword raised: the bolt goes up off it, then falls on him (or ahead of Mario)
        self.look(hand, hand + Vec3::Y * UP, WIDTH[level as usize] * 0.7, tick);
        sound(SOUND_ZAP, 0.22);
        let at = target.map_or(hand + ahead * 8.0, |t| t.1);
        let (attack, blast, jumps) = match level {
            0 => (Attack::Bolt(0), None, 0),
            1 => (Attack::Bolt(1), None, 0),
            2 => (Attack::Bolt(2), Some((BLAST, Attack::BoltBlast)), 0),
            _ => (Attack::Bolt(3), None, 2),
        };
        self.strikes.push(Strike { land: tick + FALL_AFTER, target: target.map(|t| t.0), at, attack, blast, jumps, struck: Vec::new(), width: WIDTH[level as usize] });
    }

    /// The charge building (fireball.rs): a little bolt out of the sky onto `at` near him, its crack
    /// (louder by `level`), a flicker of the screen; only a look.
    pub fn crackle(&mut self, at: Vec3, level: u8, tick: u32) {
        let s = self.seed.wrapping_add(17) as f32;
        let from = at + Vec3::new((s * 0.53).sin() * 2.0, SKY * 0.7, (s * 0.29).cos() * 2.0);
        self.look(from, at, WIDTH[0] * (0.5 + 0.12 * level.min(4) as f32), tick);
        crate::sfx::play(crate::sfx::Sfx::Thunder(false), 0.3 + 0.08 * level.min(4) as f32);
        crate::impact::flash([0.85, 0.9, 1.0], 0.04 + 0.02 * level.min(4) as f32, 0.1);
        crate::impact::element_spark(at + Vec3::Y * 0.2, [1.0, 0.95, 0.6], 1.0);
    }

    /// A crack of lightning where the sword came down (its moves' FX_ZAP): a short bolt onto `at`,
    /// only a look (the move's own blow is the hit).
    pub fn zap(&mut self, at: Vec3, tick: u32) {
        let s = self.seed.wrapping_add(31) as f32;
        let from = at + Vec3::new((s * 0.37).sin() * 0.8, ZAP_UP, (s * 0.61).cos() * 0.8);
        self.look(from, at, WIDTH[0], tick);
    }

    fn look(&mut self, from: Vec3, to: Vec3, width: f32, tick: u32) {
        self.seed = self.seed.wrapping_add(7919);
        self.looks.push(Look { from, to, seed: self.seed, born: tick, width });
        if self.looks.len() > 24 {
            self.looks.remove(0);
        }
    }

    /// A bolt out of the sky lands on `target` (or at `at`): its look, its blow. Where it landed.
    #[allow(clippy::too_many_arguments)]
    fn land(&mut self, target: Option<FieldInsHandle>, at: Vec3, attack: Attack, width: f32, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) -> Vec3 {
        let at = target.and_then(|h| chest_of(&h)).unwrap_or(at);
        let s = self.seed as f32;
        let jitter = Vec3::new((s * 0.37).sin(), 0.0, (s * 0.61).cos()) * 1.5;
        self.look(at + Vec3::Y * SKY + jitter, at, width, tick);
        crate::impact::element_spark(at, [1.0, 0.95, 0.6], 1.0);
        match target {
            Some(h) => {
                let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), 4.0, origin);
                if let Some(i) = near.iter().position(|t| t.shootable() == Some(&h)) {
                    combat.deal(player, &near, &[(i, attack, false)], tick);
                }
            }
            None => {
                combat.blast(player, at - Vec3::Y * 0.9, 1.2, attack, origin, tick);
            }
        }
        at
    }

    /// Game thread, every SM64 tick: the bolts falling, the javelins flying, the storm raining; the
    /// looks published for the HUD. `ray(from, to)`: where the map is in the way.
    pub fn tick(&mut self, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        for (from, to, width) in std::mem::take(&mut *SLASHES.lock().unwrap_or_else(|e| e.into_inner())) {
            self.look(from, to, width, tick);
        }
        // the bolts falling
        let (due, wait): (Vec<Strike>, Vec<Strike>) = std::mem::take(&mut self.strikes).into_iter().partition(|s| tick >= s.land);
        self.strikes = wait;
        for mut s in due {
            let at = self.land(s.target, s.at, s.attack, s.width, player, combat, origin, tick);
            sound(SOUND_THUNDER, (0.18 + 0.6 * s.width).min(0.4));
            if let Some((reach, blast)) = s.blast {
                combat.blast(player, at - Vec3::Y * 0.9, reach, blast, origin, tick);
                crate::impact::element_spark(at, [1.0, 0.92, 0.45], 1.6);
            }
            if let Some(h) = s.target {
                s.struck.push(combat::handle_key(&h));
            }
            // (on to the nearest it hasn't struck yet)
            if s.jumps > 0 {
                let next = enemies(at, JUMP_REACH + 2.0, origin)
                    .into_iter()
                    .filter(|(h, c, _)| !s.struck.contains(&combat::handle_key(h)) && c.distance(at) <= JUMP_REACH)
                    .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
                if let Some((h, c, _)) = next {
                    self.look(at, c, s.width * 0.8, tick);
                    self.strikes.push(Strike { land: tick + JUMP_AFTER, target: Some(h), at: c, jumps: s.jumps - 1, ..s });
                }
            }
        }
        // the javelins
        let mut javelins = std::mem::take(&mut self.javelins);
        let mut splits = Vec::new();
        javelins.retain_mut(|j| {
            if tick.wrapping_sub(j.born) > JAVELIN_LIFE {
                return false;
            }
            if let Some(c) = j.target.and_then(|h| chest_of(&h)) {
                let want = (c - j.pos).normalize_or(j.vel.normalize_or(Vec3::Z)) * JAVELIN_SPEED;
                j.vel = j.vel.lerp(want, JAVELIN_TURN).normalize_or(want) * JAVELIN_SPEED;
            }
            let next = j.pos + j.vel;
            // (an enemy where it's going: struck; else the map)
            let near = combat::nearby(&HavokPosition(next.x, next.y, next.z, 0.0), 3.0, origin);
            let hit = near.iter().position(|t| {
                t.shootable().is_some() && !t.friendly() && {
                    let feet = t.feet();
                    let (r, h) = (t.radius * crate::scale(), t.height * crate::scale());
                    (next - feet).with_y(0.0).length() <= r + JAVELIN_NEAR && next.y >= feet.y - JAVELIN_NEAR && next.y <= feet.y + h + JAVELIN_NEAR
                }
            });
            let wall = if hit.is_none() { ray(j.pos, next) } else { None };
            let at = match (hit, wall) {
                (Some(i), _) => {
                    combat.deal(player, &near, &[(i, Attack::Javelin(j.level), false)], tick);
                    next
                }
                (None, Some(w)) => {
                    combat.blast(player, w, 1.6, Attack::BoltBlast, origin, tick);
                    w
                }
                (None, None) => {
                    j.pos = next;
                    return true;
                }
            };
            crate::impact::element_spark(at, [1.0, 0.95, 0.6], 1.3);
            sound(SOUND_THUNDER, 0.25 + 0.05 * j.level as f32);
            if j.level >= 3 {
                splits.push(at);
            }
            false
        });
        self.javelins = javelins;
        // (the full javelin splits in three: bolts out of it on the three nearest round it)
        for at in splits {
            let mut round: Vec<(FieldInsHandle, Vec3, Vec3)> = enemies(at, SPLIT_REACH + 2.0, origin).into_iter().filter(|e| e.1.distance(at) <= SPLIT_REACH).collect();
            round.sort_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
            for (k, (h, c, _)) in round.into_iter().take(3).enumerate() {
                self.look(at, c, WIDTH[1], tick);
                self.strikes.push(Strike { land: tick + 3 * (k as u32 + 1), target: Some(h), at: c, attack: Attack::Bolt(0), blast: None, jumps: 0, struck: Vec::new(), width: WIDTH[1] });
            }
        }
        self.rain(player, combat, origin, tick);
        self.publish(tick);
    }

    /// The storm cloud: over him, its rain, its giant bolt.
    fn rain(&mut self, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
        let Some(mut st) = self.storm.take() else { return };
        let age = tick.wrapping_sub(st.born);
        let over = st.target.and_then(|h| chest_of(&h)).map(|c| c + Vec3::Y * STORM_HIGH);
        if age < STORM_FLY {
            let k = age as f32 / STORM_FLY as f32;
            let to = over.unwrap_or(st.from + Vec3::Y * 4.0);
            st.pos = st.from.lerp(to, k * k * (3.0 - 2.0 * k));
        } else if let Some(o) = over {
            st.pos = st.pos.lerp(o, 0.08);
        }
        if age >= STORM_FLY + STORM_RAIN {
            // the giant one, straight down its middle
            let ground = st.pos - Vec3::Y * (STORM_HIGH + 0.9);
            self.look(st.pos, ground, WIDTH[4], tick);
            combat.blast(player, ground, STORM_END_BLAST, Attack::StormEnd, origin, tick);
            crate::impact::flash([1.0, 0.95, 0.7], 0.2, 0.4);
            crate::lakitu::shake(0.9);
            sound(SOUND_THUNDER, 0.55);
            log("bolt: the storm's giant bolt");
            return;
        }
        if age >= STORM_FLY && tick >= st.next {
            st.next = tick + STORM_EVERY;
            st.flash_at = tick;
            // (on someone under it not struck a moment ago; else a bolt where it may)
            let under = enemies(st.pos - Vec3::Y * STORM_HIGH, STORM_REACH + 3.0, origin)
                .into_iter()
                .find(|(h, _, feet)| (feet.with_y(0.0) - st.pos.with_y(0.0)).length() <= STORM_REACH && st.hit.get(&combat::handle_key(h)).is_none_or(|&at| tick.wrapping_sub(at) >= STORM_SAME));
            match under {
                Some((h, c, _)) => {
                    st.hit.insert(combat::handle_key(&h), tick);
                    let from = st.pos;
                    self.storm = Some(st);
                    let at = self.land(Some(h), c, Attack::Storm, WIDTH[1], player, combat, origin, tick);
                    // (from the cloud, not the sky over him)
                    if let Some(l) = self.looks.last_mut() {
                        l.from = from;
                        l.to = at;
                    }
                    sound(SOUND_THUNDER, 0.15);
                    return;
                }
                None => {
                    let a = (tick as f32 * 2.399).rem_euclid(std::f32::consts::TAU);
                    let spot = st.pos + Vec3::new(a.cos(), 0.0, a.sin()) * STORM_REACH * 0.6 - Vec3::Y * STORM_HIGH;
                    self.look(st.pos, spot, WIDTH[0], tick);
                }
            }
        }
        self.storm = Some(st);
    }

    fn publish(&mut self, tick: u32) {
        self.looks.retain(|l| tick.wrapping_sub(l.born) < LOOK_LIFE);
        let mut bolts: Vec<crate::fx::Bolt> = self.looks.iter().map(|l| crate::fx::Bolt { from: l.from, to: l.to, seed: l.seed, t: tick.wrapping_sub(l.born) as f32 / LOOK_LIFE as f32, width: l.width }).collect();
        // (the javelins as short bolts along their flight, flickering)
        bolts.extend(self.javelins.iter().map(|j| crate::fx::Bolt {
            from: j.pos - j.vel * 1.2,
            to: j.pos + j.vel * 0.3,
            seed: (j.born as u16).wrapping_mul(31).wrapping_add((tick / 2) as u16),
            t: 0.2,
            width: WIDTH[j.level as usize],
        }));
        let storms = self
            .storm
            .as_ref()
            .map(|s| {
                let age = tick.wrapping_sub(s.born);
                let t = (age as f32 / (STORM_FLY + STORM_RAIN) as f32).min(1.0);
                let flash = (1.0 - tick.wrapping_sub(s.flash_at) as f32 / 4.0).max(0.0);
                crate::fx::Storm { pos: s.pos, radius: STORM_REACH, t, flash }
            })
            .into_iter()
            .collect();
        *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = (bolts, storms);
    }

    /// The game re-based its physics coordinates by `d`: what's out there moves with them.
    pub fn shift(&mut self, d: Vec3) {
        for s in &mut self.strikes {
            s.at += d;
        }
        for j in &mut self.javelins {
            j.pos += d;
        }
        if let Some(st) = self.storm.as_mut() {
            st.pos += d;
            st.from += d;
        }
        for l in &mut self.looks {
            l.from += d;
            l.to += d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bolt_zigzags_between_its_ends() {
        let (a, b) = (Vec3::new(0.0, 20.0, 0.0), Vec3::new(1.0, 0.0, 0.5));
        let p = jag(a, b, 7, 9, 0.12);
        assert_eq!(p.len(), 10);
        assert!(p[0].distance(a) < 1e-5 && p[9].distance(b) < 1e-5);
        let len = a.distance(b);
        let dir = (b - a) / len;
        let off: Vec<f32> = p.iter().map(|q| { let d = *q - a; (d - dir * d.dot(dir)).length() }).collect();
        assert!(off.iter().all(|o| *o <= 0.12 * len + 1e-4), "{off:?}");
        // (it does zig: some points well off the line)
        assert!(off.iter().any(|o| *o > 0.02 * len));
        // the same seed, the same bolt; another seed, another
        assert_eq!(jag(a, b, 7, 9, 0.12), p);
        assert_ne!(jag(a, b, 8, 9, 0.12), p);
    }

    #[test]
    fn a_cast_goes_for_the_nearest_ahead() {
        let me = Vec3::ZERO;
        let ahead = Vec3::Z;
        let c = [Vec3::new(0.0, 0.0, 20.0), Vec3::new(1.0, 0.0, 8.0), Vec3::new(0.0, 0.0, -3.0), Vec3::new(9.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 40.0)];
        assert_eq!(pick(me, ahead, &c, 30.0, 0.5), Some(1));
        // (behind and to the side don't count, nor past the range)
        assert_eq!(pick(me, ahead, &c[2..4], 30.0, 0.5), None);
        assert_eq!(pick(me, ahead, &c[4..], 30.0, 0.5), None);
    }
}
