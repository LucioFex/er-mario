//! The Star Sword's crescents (v4 Part 2): the star wave (X held and let go: a crescent of light
//! skimming the ground ahead, 10 m; let go on the sparkle, JUST: three in a quick fan of slants),
//! and at full health the sword beam (Zelda's: a string's finisher sends a small one 12 m). Each
//! goes through everyone in its way once and stops at walls. The cross cut leaves its X hanging in
//! the air, bursting 0.4 s later (Vergil's Judgement Cut). Drawn in 3D in the HUD's overlay (hud.rs,
//! fx.rs `Crescent`).

use std::sync::Mutex;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

/// Which crescent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Wave,
    Just,
    Beam,
    Cross,
}

impl Kind {
    fn code(self) -> u8 {
        match self {
            Kind::Wave => 0,
            Kind::Just => 1,
            Kind::Beam => 2,
            Kind::Cross => 3,
        }
    }
}

/// One crescent of a cast: when it leaves (ticks after the cast), its slant (degrees: 0 flat, 90
/// upright), how wide (m), how fast (m a tick), how far it goes (m), its blow (none: only a look);
/// or one that hangs where it was cut `linger` ticks, then bursts (its blow on everyone within its
/// width).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub delay: u32,
    pub roll: f32,
    pub width: f32,
    pub speed: f32,
    pub range: f32,
    pub attack: Option<Attack>,
    pub linger: u32,
}

const WAVE: [Spec; 1] = [Spec { delay: 0, roll: 70.0, width: 2.6, speed: 0.55, range: 10.0, attack: Some(Attack::SwordWave), linger: 0 }];
/// (one slanted each way, then an upright one: Vergil's Judgement Cut, three cuts)
const JUST: [Spec; 3] = [
    Spec { delay: 0, roll: 35.0, width: 2.8, speed: 0.65, range: 11.0, attack: Some(Attack::SwordWaveJust), linger: 0 },
    Spec { delay: 4, roll: -35.0, width: 2.8, speed: 0.65, range: 11.0, attack: Some(Attack::SwordWaveJust), linger: 0 },
    Spec { delay: 8, roll: 90.0, width: 3.0, speed: 0.7, range: 11.0, attack: Some(Attack::SwordWaveJust), linger: 0 },
];
const BEAM: [Spec; 1] = [Spec { delay: 0, roll: 0.0, width: 1.3, speed: 0.8, range: 12.0, attack: Some(Attack::SwordBeam), linger: 0 }];
/// (the two cuts' X: one burst between them)
const CROSS: [Spec; 2] = [
    Spec { delay: 0, roll: 45.0, width: 2.2, speed: 0.0, range: 0.0, attack: Some(Attack::SwordCrossBurst), linger: 12 },
    Spec { delay: 0, roll: -45.0, width: 2.2, speed: 0.0, range: 0.0, attack: None, linger: 12 },
];

/// The crescents a cast of `kind` sends.
pub fn specs(kind: Kind) -> &'static [Spec] {
    match kind {
        Kind::Wave => &WAVE,
        Kind::Just => &JUST,
        Kind::Beam => &BEAM,
        Kind::Cross => &CROSS,
    }
}

/// A crescent's reach across its way and up (m) by its slant: an upright one is tall and narrow
/// (never thinner than a body's worth), a flat one wide and low.
fn extent(s: &Spec) -> (f32, f32) {
    let r = s.roll.to_radians();
    ((s.width * r.cos().abs()).max(0.9), (s.width * r.sin().abs()).max(0.9))
}

/// Whether a crescent `width` wide (across its way) and `tall` high, going from `from` to `to`
/// over a tick, passes through an upright body (feet `foot`, `radius`, `height`; m).
pub fn sweeps(from: Vec3, to: Vec3, width: f32, tall: f32, foot: Vec3, radius: f32, height: f32) -> bool {
    let way = (to - from).with_y(0.0);
    let len = way.length();
    let dir = way.normalize_or(Vec3::Z);
    let side = Vec3::Y.cross(dir);
    let rel = (foot - from).with_y(0.0);
    let (along, across) = (rel.dot(dir), rel.dot(side));
    let y = (from.y + to.y) * 0.5;
    (-radius..=len + radius).contains(&along) && across.abs() <= width * 0.5 + radius && foot.y <= y + tall * 0.5 && foot.y + height >= y - tall * 0.5
}

/// a crescent skims the ground this high (m), eased onto it this fast (a share a tick); it ends
/// fading this many ticks
const HOVER: f32 = 0.55;
const SETTLE: f32 = 0.5;
const FADE: u32 = 5;
/// (not verified in game: soft) its swish, the JUST's chime
const SOUND_SWISH: i32 = 0x305C_0081;
const SOUND_SHORT_STAR: i32 = 0x3016_0091;
const TINT: [f32; 3] = [0.75, 0.9, 1.0];
const CROSS_TINT: [f32; 3] = [1.0, 0.92, 0.6];
/// the X hangs this far ahead of him (m), where the two cuts crossed
const CROSS_AHEAD: f32 = 1.2;

struct Live {
    spec: Spec,
    kind: Kind,
    pos: Vec3,
    dir: Vec3,
    travelled: f32,
    start: u32,
    struck: Vec<u64>,
    /// stopped (a wall, its range) at this tick: fading
    ended: Option<u32>,
}

/// The sword's crescents out there.
#[derive(Default)]
pub struct Waves {
    live: Vec<Live>,
}

static LOOKS: Mutex<Vec<crate::fx::Crescent>> = Mutex::new(Vec::new());

/// The crescents to draw now (fx.rs).
pub fn looks() -> Vec<crate::fx::Crescent> {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn clear() {
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn sound(id: i32, volume: f32) {
    worker::call("wave sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

impl Waves {
    /// A cast of `kind` off his blade: from `feet` facing `facing` (Elden Ring world).
    pub fn cast(&mut self, kind: Kind, feet: Vec3, facing: Vec3, tick: u32) {
        let dir = facing.with_y(0.0).normalize_or(Vec3::Z);
        // (the beam off his blade at chest height, the X where the cuts crossed; the waves skim the
        // ground)
        let (up, ahead) = match kind {
            Kind::Beam => (0.9, 0.7),
            Kind::Cross => (0.95, CROSS_AHEAD),
            _ => (HOVER, 0.7),
        };
        for s in specs(kind) {
            self.live.push(Live { spec: *s, kind, pos: feet + Vec3::Y * up + dir * ahead, dir, travelled: 0.0, start: tick + s.delay, struck: Vec::new(), ended: None });
        }
        match kind {
            Kind::Wave => {
                log("wave: star wave");
                sound(SOUND_SWISH, 0.35);
            }
            Kind::Just => {
                log("wave: JUST, three star waves");
                sound(SOUND_SWISH, 0.4);
                sound(SOUND_SHORT_STAR, 0.22);
                crate::style::callout("JUST!", Some(feet + Vec3::Y * 2.2), TINT, true, 1.5);
            }
            Kind::Beam => {
                log("wave: sword beam (full health)");
                sound(SOUND_SWISH, 0.25);
            }
            Kind::Cross => {}
        }
    }

    /// Game thread, every SM64 tick: the crescents fly, cut whoever's in their way once, stop at
    /// walls and past their range; their looks published for the HUD. `ray(from, to)`: where the
    /// map is in the way (the game's physics coordinates).
    pub fn tick(&mut self, player: &ChrIns, combat: &mut Combat, origin: [f32; 3], tick: u32, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let mut live = std::mem::take(&mut self.live);
        live.retain_mut(|w| {
            if let Some(at) = w.ended {
                return tick.wrapping_sub(at) < FADE;
            }
            if tick < w.start {
                return true;
            }
            // (one hanging there: its burst when its time's up, on everyone within its width)
            if w.spec.linger > 0 {
                if tick.wrapping_sub(w.start) < w.spec.linger {
                    return true;
                }
                w.ended = Some(tick);
                for k in 0..3 {
                    crate::impact::element_spark(w.pos + w.dir.cross(Vec3::Y) * (k as f32 * 0.3 - 0.3), CROSS_TINT, 1.4);
                }
                let Some(attack) = w.spec.attack else { return true };
                let near = combat::nearby(&HavokPosition(w.pos.x, w.pos.y, w.pos.z, 0.0), w.spec.width + 4.0, origin);
                let hits: Vec<(usize, Attack, bool)> = near
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| !t.friendly() && t.shootable().is_some())
                    .filter(|(_, t)| {
                        let feet = t.feet();
                        let top = feet.y + t.height * crate::scale();
                        (feet - w.pos).with_y(0.0).length() <= w.spec.width * 0.5 + t.radius * crate::scale() && w.pos.y + 1.2 >= feet.y && w.pos.y - 1.2 <= top
                    })
                    .map(|(i, _)| (i, attack, false))
                    .collect();
                if !hits.is_empty() {
                    combat.deal(player, &near, &hits, tick);
                }
                sound(SOUND_SHORT_STAR, 0.22);
                log(format!("wave: the cross bursts ({} cut)", hits.len()));
                return true;
            }
            // (one with no blow of its own still flies its way and ends)
            let attack = w.spec.attack;
            let mut next = w.pos + w.dir * w.spec.speed;
            // (the waves along the ground: up and down its slopes)
            if w.kind != Kind::Beam {
                if let Some(g) = ray(next + Vec3::Y * 1.5, next - Vec3::Y * 3.0) {
                    next.y += (g.y + HOVER - next.y) * SETTLE;
                }
            }
            // a wall: it breaks there
            if let Some(hit) = ray(w.pos, next) {
                crate::impact::element_spark(hit, TINT, 0.8);
                w.pos = hit - w.dir * 0.1;
                w.ended = Some(tick);
                return true;
            }
            let (across, tall) = extent(&w.spec);
            let mid = (w.pos + next) * 0.5;
            let near = combat::nearby(&HavokPosition(mid.x, mid.y, mid.z, 0.0), across + 4.0, origin);
            let cut: Vec<(usize, u64)> = near
                .iter()
                .enumerate()
                .filter(|_| attack.is_some())
                .filter(|(_, t)| !t.friendly())
                .filter_map(|(i, t)| Some((i, combat::handle_key(t.shootable()?))))
                .filter(|(i, key)| {
                    let t = &near[*i];
                    let feet = t.feet();
                    !w.struck.contains(key) && sweeps(w.pos, next, across, tall, feet, t.radius * crate::scale(), t.height * crate::scale())
                })
                .collect();
            let hits: Vec<(usize, Attack, bool)> = cut.iter().filter_map(|&(i, _)| Some((i, attack?, false))).collect();
            for &(i, key) in &cut {
                let t = &near[i];
                w.struck.push(key);
                crate::impact::element_spark(t.feet() + Vec3::Y * 0.9, TINT, 1.0);
            }
            if !hits.is_empty() {
                combat.deal(player, &near, &hits, tick);
            }
            w.pos = next;
            w.travelled += w.spec.speed;
            if w.travelled >= w.spec.range {
                w.ended = Some(tick);
            }
            true
        });
        self.live = live;
        self.publish(tick);
    }

    fn publish(&self, tick: u32) {
        let looks = self
            .live
            .iter()
            .filter(|w| tick >= w.start)
            .map(|w| {
                // (it grows out of the blade, then fades at the end of its way; the X brightens
                // as it waits to burst)
                let age = tick.wrapping_sub(w.start) as f32;
                let grow = (age / 3.0).min(1.0);
                let fade = w.ended.map_or(1.0, |at| 1.0 - tick.wrapping_sub(at) as f32 / FADE as f32);
                let fade = if w.spec.linger > 0 && w.ended.is_none() { fade * (0.45 + 0.55 * age / w.spec.linger as f32) } else { fade };
                crate::fx::Crescent {
                    pos: w.pos,
                    yaw: w.dir.x.atan2(w.dir.z),
                    roll: w.spec.roll,
                    width: w.spec.width * (0.5 + 0.5 * grow),
                    alpha: fade.clamp(0.0, 1.0),
                    kind: w.kind.code(),
                }
            })
            .collect();
        *LOOKS.lock().unwrap_or_else(|e| e.into_inner()) = looks;
    }

    /// The game re-based its physics coordinates by `d`: the crescents move with them.
    pub fn shift(&mut self, d: Vec3) {
        for w in &mut self.live {
            w.pos += d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crescent_cuts_what_is_in_its_way() {
        let (from, to) = (Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 0.5, 0.6));
        // in its way, a body ahead it reaches this tick
        assert!(sweeps(from, to, 2.4, 1.2, Vec3::new(0.0, 0.0, 0.9), 0.4, 1.8));
        // off to the side, within its width (and the body's own)
        assert!(sweeps(from, to, 2.4, 1.2, Vec3::new(1.3, 0.0, 0.3), 0.4, 1.8));
        // too far to the side, beyond this tick's way, behind it
        assert!(!sweeps(from, to, 2.4, 1.2, Vec3::new(2.5, 0.0, 0.3), 0.4, 1.8));
        assert!(!sweeps(from, to, 2.4, 1.2, Vec3::new(0.0, 0.0, 2.0), 0.4, 1.8));
        assert!(!sweeps(from, to, 2.4, 1.2, Vec3::new(0.0, 0.0, -1.5), 0.4, 1.8));
        // over a body flying high, under one hanging above
        assert!(!sweeps(from, to, 2.4, 1.2, Vec3::new(0.0, 3.0, 0.3), 0.4, 1.8));
        assert!(!sweeps(from, to, 2.4, 1.2, Vec3::new(0.0, -4.0, 0.3), 0.4, 1.8));
        // (a big body reaches into its way from farther off)
        assert!(sweeps(from, to, 2.4, 1.2, Vec3::new(3.0, 0.0, 0.3), 2.0, 5.0));
    }

    #[test]
    fn the_casts() {
        let wave = specs(Kind::Wave);
        assert_eq!(wave.len(), 1);
        assert_eq!(wave[0].attack, Some(Attack::SwordWave));
        assert!((wave[0].range - 10.0).abs() < 0.01);
        // JUST: three, a beat apart, each slanted its own way, each its own blow
        let just = specs(Kind::Just);
        assert_eq!(just.len(), 3);
        assert!(just.windows(2).all(|w| w[0].delay < w[1].delay && w[0].roll != w[1].roll));
        assert!(just.iter().all(|s| s.attack == Some(Attack::SwordWaveJust)));
        // the beam: smaller, farther
        let beam = specs(Kind::Beam);
        assert_eq!(beam.len(), 1);
        assert!(beam[0].width < wave[0].width && (beam[0].range - 12.0).abs() < 0.01);
        assert_eq!(beam[0].attack, Some(Attack::SwordBeam));
        // the cross cut's X: two crossing ones hanging there 0.4 s, one blow when it bursts
        let cross = specs(Kind::Cross);
        assert_eq!(cross.len(), 2);
        assert!(cross.iter().all(|s| s.speed == 0.0 && s.linger == 12));
        assert!((cross[0].roll + cross[1].roll).abs() < 0.01 && cross[0].roll != 0.0);
        assert_eq!(cross.iter().filter_map(|s| s.attack).collect::<Vec<_>>(), vec![Attack::SwordCrossBurst]);
        // (the flying ones don't linger)
        assert!([wave, just, beam].iter().all(|k| k.iter().all(|s| s.linger == 0)));
    }

    #[test]
    fn upright_ones_reach_high_flat_ones_wide() {
        let (a, t) = extent(&JUST[2]);
        assert!(t > 2.5 && a < 1.0);
        let (a, t) = extent(&BEAM[0]);
        assert!(a > 1.2 && t < 1.0);
    }
}
