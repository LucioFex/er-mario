//! The hammer's Hammer Bros. volley (v4 Part 3, hammermoves.rs): small hammers thrown overhand,
//! each in an arc onto an enemy in front of Mario (the next throw on the next one, up to three),
//! spinning end over end; each strikes once and drops away. Drawn as small 3D hammers in the HUD's
//! overlay (hud.rs). In the game's physics coordinates, as bolt.rs.

use std::sync::Mutex;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// enemies this near (m) inside this cone ahead (cos) are thrown at; with none, one lands this far
/// ahead (m)
const RANGE: f32 = 12.0;
const CONE: f32 = 0.5;
const AHEAD: f32 = 7.0;
/// an arc's top this high over its higher end (m); its flight this many ticks, and as many more a
/// metre of the way
const APEX: f32 = 1.6;
const FLIGHT: f32 = 9.0;
const FLIGHT_PER_M: f32 = 0.9;
/// it strikes within this of the enemy's body (m); turns end over end this fast (radians a tick)
const NEAR: f32 = 0.7;
const SPIN: f32 = 0.6;
/// after it strikes or lands it drops away this long (ticks)
const FALL: u32 = 10;
/// its size (m)
const SIZE: f32 = 0.42;
/// (SM64's: the throw's whoosh, the bonk on a head, the clatter on the ground; soft)
const SOUND_SWISH: i32 = 0x305C_0081;
const SOUND_BONK: i32 = 0x0444_A081;
const SOUND_LAND: i32 = 0x0460_8081;

/// Where a thrown hammer is `t` (0-1) along its arc from `from` to `to`: its top `apex` m over the
/// higher of the two.
pub fn arc(from: Vec3, to: Vec3, apex: f32, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let h = apex + (to.y - from.y).abs() * 0.5;
    from.lerp(to, t) + Vec3::Y * (4.0 * h * t * (1.0 - t))
}

struct Thrown {
    from: Vec3,
    to: Vec3,
    target: Option<FieldInsHandle>,
    born: u32,
    flight: u32,
    pos: Vec3,
    /// struck or landed at this tick: it drops away
    done: Option<u32>,
    yaw: f32,
}

/// The volley's hammers out there.
#[derive(Default)]
pub struct Volley {
    out: Vec<Thrown>,
    /// throws in the volley going on, and when the last was
    count: u32,
    last: u32,
}

/// What the HUD draws (fx.rs): each hammer's place, its flight's yaw, its turn end over end, its
/// size.
static LOOKS: Mutex<Vec<crate::fx::Thrown>> = Mutex::new(Vec::new());

pub fn looks() -> Vec<crate::fx::Thrown> {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn clear() {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn sound(id: i32, volume: f32) {
    worker::call("volley sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
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

impl Volley {
    /// A small hammer leaves his hand `hand` (Elden Ring world), Mario facing `facing`: onto the
    /// next enemy in front (the volley's throws take them in turn), or ahead onto the ground.
    pub fn throw(&mut self, hand: Vec3, facing: Vec3, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        // (a new volley: from the nearest again)
        if tick.wrapping_sub(self.last) > 30 {
            self.count = 0;
        }
        self.last = tick;
        let f = facing.with_y(0.0).normalize_or(Vec3::Z);
        let mut near: Vec<(f32, FieldInsHandle, Vec3)> = combat::nearby(&HavokPosition(hand.x, hand.y, hand.z, 0.0), RANGE + 2.0, origin)
            .iter()
            .filter(|t| !t.friendly())
            .filter_map(|t| {
                let h = *t.shootable()?;
                let chest = t.feet() + Vec3::Y * (t.height * crate::scale() * 0.55).min(1.6);
                let d = (chest - hand).with_y(0.0);
                let l = d.length();
                (l <= RANGE && l > 0.3 && d.dot(f) / l >= CONE).then_some((l, h, chest))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.truncate(3);
        let (target, to) = match near.get(self.count as usize % near.len().max(1)) {
            Some(&(_, h, chest)) => (Some(h), chest),
            None => {
                // (no one: down on the ground ahead)
                let at = hand + f * AHEAD;
                (None, ray(at + Vec3::Y * 2.0, at - Vec3::Y * 6.0).unwrap_or(at - Vec3::Y * 0.8))
            }
        };
        self.count += 1;
        let flight = (FLIGHT + (to - hand).length() * FLIGHT_PER_M).round() as u32;
        self.out.push(Thrown { from: hand, to, target, born: tick, flight, pos: hand, done: None, yaw: f.x.atan2(f.z) });
        sound(SOUND_SWISH, 0.3);
        log(format!("volley: a small hammer at {}", if target.is_some() { "an enemy" } else { "the ground" }));
    }

    /// Game thread, every SM64 tick: the hammers fly (bending to where their enemy is now), strike
    /// once, land; the looks published.
    pub fn tick(&mut self, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32) {
        self.out.retain_mut(|h| {
            if let Some(at) = h.done {
                h.pos -= Vec3::Y * 0.08;
                return tick.wrapping_sub(at) < FALL;
            }
            // (its enemy moved: the arc's end follows it, easing)
            if let Some(chest) = h.target.and_then(|t| chest_of(&t)) {
                h.to = h.to.lerp(chest, 0.3);
            }
            let t = tick.wrapping_sub(h.born) as f32 / h.flight.max(1) as f32;
            h.pos = arc(h.from, h.to, APEX, t);
            let near = combat::nearby(&HavokPosition(h.pos.x, h.pos.y, h.pos.z, 0.0), 3.0, origin);
            let struck = near.iter().position(|e| {
                e.shootable().is_some() && !e.friendly() && {
                    let feet = e.feet();
                    let (r, ht) = (e.radius * crate::scale(), e.height * crate::scale());
                    (h.pos - feet).with_y(0.0).length() <= r + NEAR && h.pos.y >= feet.y - NEAR && h.pos.y <= feet.y + ht + NEAR
                }
            });
            if let Some(i) = struck {
                combat.deal(player, &near, &[(i, Attack::HammerBro, false)], tick);
                crate::impact::element_spark(h.pos, [0.9, 0.95, 1.0], 1.0);
                sound(SOUND_BONK, 0.35);
                h.done = Some(tick);
            } else if t >= 1.0 {
                crate::impact::thud_small(h.pos);
                sound(SOUND_LAND, 0.2);
                h.done = Some(tick);
            }
            true
        });
        self.publish(tick);
    }

    fn publish(&self, tick: u32) {
        let looks = self
            .out
            .iter()
            .map(|h| crate::fx::Thrown { pos: h.pos, yaw: h.yaw, spin: tick.wrapping_sub(h.born) as f32 * SPIN, size: SIZE })
            .collect();
        *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = looks;
    }

    /// The game re-based its physics coordinates by `d`: what's out there moves with them.
    pub fn shift(&mut self, d: Vec3) {
        for h in &mut self.out {
            h.from += d;
            h.to += d;
            h.pos += d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arc_lands_where_it_was_thrown_at() {
        let (from, to) = (Vec3::new(0.0, 1.5, 0.0), Vec3::new(6.0, 0.9, 2.0));
        assert!(arc(from, to, APEX, 0.0).distance(from) < 1e-5);
        assert!(arc(from, to, APEX, 1.0).distance(to) < 1e-5);
        // (its top above both ends; past the end it stays there)
        assert!(arc(from, to, APEX, 0.5).y > from.y.max(to.y) + 1.0);
        assert!(arc(from, to, APEX, 1.4).distance(to) < 1e-5);
        // (thrown at one well below: still above the higher end)
        let low = Vec3::new(4.0, -3.0, 0.0);
        assert!((0..=10).map(|k| arc(from, low, APEX, k as f32 / 10.0).y).fold(f32::MIN, f32::max) > from.y);
    }
}
