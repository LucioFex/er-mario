//! MAX finishers (style.rs counts the streak): twelve blows in a row without being hurt and the
//! weapon glows; its next finisher is a MAX one, a bigger version of what it does:
//! - the fists: a burst of phantom blows on what's in front, then the ones it can lift sent up
//! - the hammer: a quake in a cross, its four arms rolling out from where it struck
//! - the Chain Chomp: the cyclone's MAX, a ring of fire that flings the small ones up and staggers
//!   the rest
//! - the boomerang: its big throw is three big ones at once (boomerang.rs)
//! - the Star Sword: Star Cut End (Vergil's Judgement Cut End), a flash, the world slowed, then
//!   cuts of lightning across every enemy within 8 m, three times
//! Still blows, not kills: each is a share like the rest, bosses take their own small one.

use std::sync::Mutex;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, weapons, worker};

/// the fists' phantom blows: how many, how far apart (ticks), how far in front they reach (m)
const FLURRY_BLOWS: u32 = 7;
const FLURRY_EVERY: u32 = 2;
const FLURRY_REACH: f32 = 2.6;
/// the hammer's cross: its arms' steps (m apart), the blast at each (m), ticks between steps
const CROSS_STEP: f32 = 1.9;
const CROSS_STEPS: u32 = 3;
const CROSS_BLAST: f32 = 1.5;
const CROSS_EVERY: u32 = 3;
/// the Chomp's ring of fire (m)
const CYCLONE_REACH: f32 = 6.5;
/// the sword's Star Cut End: who it takes (m round Mario, at most this many), its cuts (how many,
/// ticks apart), each cut's length across them (m)
const STAR_CUT_REACH: f32 = 8.0;
const STAR_CUT_MOST: usize = 8;
const STAR_CUTS: u32 = 3;
const STAR_CUT_EVERY: u32 = 5;
const STAR_CUT_LEN: f32 = 1.4;

const SOUND_PUNCH: [i32; 3] = [0x0444_C081, 0x0444_B081, 0x043F_8081];
const SOUND_QUAKE: i32 = 0x312F_0081;
const SOUND_STAR: i32 = 0x3016_0091;
const SOUND_SWISH: i32 = 0x305C_0081;
/// (bolt.rs's, not verified in game)
const SOUND_THUNDER: i32 = 0x300C_8081;

enum Show {
    Flurry { at: Vec3, dir: Vec3 },
    Cross { feet: Vec3, dir: Vec3 },
    StarCut { dir: Vec3, who: Vec<eldenring::cs::FieldInsHandle> },
}

struct Running {
    show: Show,
    start: u32,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// Game thread: a MAX finisher went off (`attack`, at the enemy's chest `at`), Mario at `feet`
/// facing `dir` (Elden Ring). The weapon in hand picks what it does.
#[allow(clippy::too_many_arguments)]
pub fn fire(attack: Attack, at: Vec3, feet: Vec3, dir: Vec3, combat: &mut Combat, player: &ChrIns, origin: [f32; 3], tick: u32) {
    let dir = dir.with_y(0.0).normalize_or(Vec3::Z);
    let weapon = weapons::current();
    log(format!("max: MAX finisher ({attack:?}, weapon {weapon})"));
    crate::style::callout("MAX!", Some(at + Vec3::Y * 0.6), [1.0, 0.75, 0.2], true, 4.0);
    crate::impact::max_burst(at);
    crate::style::slow_down(0.4);
    worker::call("max", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_STAR, 0.35) });
    match weapon {
        weapons::HAMMER => *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running { show: Show::Cross { feet: at.with_y(feet.y), dir }, start: tick }),
        weapons::CHOMP => cyclone(feet, combat, player, origin, tick),
        weapons::SWORD => {
            // (everyone near it takes it, the nearest first)
            let mut near: Vec<(f32, eldenring::cs::FieldInsHandle)> = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), STAR_CUT_REACH + 2.0, origin)
                .iter()
                .filter(|t| !t.friendly())
                .filter_map(|t| Some(((t.feet() - feet).length(), *t.shootable()?)))
                .filter(|(d, _)| *d <= STAR_CUT_REACH)
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            near.truncate(STAR_CUT_MOST);
            log(format!("max: Star Cut End on {}", near.len()));
            crate::impact::flash([0.8, 0.9, 1.0], 0.45, 0.35);
            crate::style::slow_down(0.7);
            *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running { show: Show::StarCut { dir, who: near.into_iter().map(|n| n.1).collect() }, start: tick });
        }
        _ => *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running { show: Show::Flurry { at, dir }, start: tick }),
    }
}

/// Game thread, every SM64 tick: a MAX finisher still going on (the flurry's blows, the cross's
/// steps) lands its next part.
pub fn tick(combat: &mut Combat, player: &ChrIns, origin: [f32; 3], tick: u32) {
    let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    let Some(r) = running.as_ref() else { return };
    let t = tick.wrapping_sub(r.start);
    match r.show {
        Show::Flurry { at, dir } => {
            if t > 0 && t % FLURRY_EVERY == 0 && t / FLURRY_EVERY <= FLURRY_BLOWS {
                let k = t / FLURRY_EVERY;
                let last = k == FLURRY_BLOWS;
                let near = combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), FLURRY_REACH + 2.0, origin);
                let hits: Vec<(usize, Attack, bool)> = near
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.shootable().is_some() && (Vec3::new(t.er.0, t.er.1 + 1.0, t.er.2) - at).length() < FLURRY_REACH + t.radius * crate::scale())
                    .map(|(i, _)| (i, if last { Attack::MaxUpper } else { Attack::MaxFlurry }, false))
                    .collect();
                if !hits.is_empty() {
                    combat.deal(player, &near, &hits, tick);
                }
                // (a phantom blow: a burst round the target, a punch's sound)
                let side = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                let wobble = side * (((k * 7) % 5) as f32 * 0.18 - 0.36) + Vec3::Y * (((k * 3) % 4) as f32 * 0.15 - 0.2);
                crate::impact::max_blow(at + wobble, last);
                let s = SOUND_PUNCH[(k % 3) as usize];
                worker::call("max blow", move |_| unsafe { sm64::sm64_play_sound_volume(s, if last { 0.9 } else { 0.5 }) });
                // the last: whatever it can lift goes up
                if last {
                    for t in near.iter().filter(|t| t.shootable().is_some_and(|h| combat::liftable(h) && !crate::stopwatch::frozen(h))) {
                        let h = t.shootable().copied();
                        if let Some(h) = h {
                            if (Vec3::new(t.er.0, t.er.1 + 1.0, t.er.2) - at).length() < FLURRY_REACH + 0.5 {
                                crate::juggle::try_launch(&h, at - dir, t.radius * crate::scale(), t.height * crate::scale(), true, false, true);
                            }
                        }
                    }
                    *running = None;
                }
            }
        }
        Show::StarCut { dir, ref who } => {
            if t > 0 && t % STAR_CUT_EVERY == 0 {
                let k = t / STAR_CUT_EVERY;
                let last = k >= STAR_CUTS;
                let side = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                // (the cuts' slants: one way, the other, level)
                let across = match k % 3 {
                    1 => (side + Vec3::Y * 0.8).normalize(),
                    2 => (side - Vec3::Y * 0.8).normalize(),
                    _ => side,
                };
                let wcm = unsafe { eldenring::cs::WorldChrMan::instance() }.ok();
                let mut cut = 0;
                for h in who {
                    let Some(chr) = wcm.as_ref().and_then(|w| w.chr_ins_by_handle(h)) else { continue };
                    if chr.modules.data.hp <= 0 {
                        continue;
                    }
                    let p = chr.modules.physics.position;
                    let chest = Vec3::new(p.0, p.1 + 0.9, p.2);
                    let near = combat::nearby(&HavokPosition(chest.x, chest.y, chest.z, 0.0), 3.0, origin);
                    if let Some(i) = near.iter().position(|t| t.shootable() == Some(h)) {
                        combat.deal(player, &near, &[(i, Attack::MaxStarCut, false)], tick);
                        cut += 1;
                    }
                    crate::bolt::slash(chest - across * STAR_CUT_LEN, chest + across * STAR_CUT_LEN, if last { 0.2 } else { 0.12 });
                    crate::impact::max_blow(chest, last);
                }
                let (s, v) = if last { (SOUND_THUNDER, 0.4) } else { (SOUND_SWISH, 0.3) };
                worker::call("max cut", move |_| unsafe { sm64::sm64_play_sound_volume(s, v) });
                if last {
                    log(format!("max: Star Cut End's last cut on {cut}"));
                    crate::impact::flash([0.9, 0.95, 1.0], 0.3, 0.3);
                    crate::lakitu::shake(0.8);
                    *running = None;
                }
            }
        }
        Show::Cross { feet, dir } => {
            if t > 0 && t % CROSS_EVERY == 0 {
                let step = t / CROSS_EVERY;
                let side = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                let points: Vec<Vec3> = [dir, side, -dir, -side].iter().map(|arm| feet + *arm * CROSS_STEP * step as f32).collect();
                // (each enemy once a step, however many of its arms reach it)
                let near = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), CROSS_STEP * step as f32 + CROSS_BLAST + 4.0, origin);
                let hits: Vec<(usize, Attack, bool)> = near
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.shootable().is_some())
                    .filter(|(_, t)| {
                        let foot = Vec3::new(t.er.0, t.er.1, t.er.2);
                        let r = t.radius * crate::scale();
                        points.iter().any(|p| (foot - *p).with_y(0.0).length() <= CROSS_BLAST + r && (foot.y - p.y).abs() < 2.0)
                    })
                    .map(|(i, _)| (i, Attack::MaxQuake, false))
                    .collect();
                if !hits.is_empty() {
                    combat.deal(player, &near, &hits, tick);
                }
                for p in &points {
                    crate::impact::max_quake(*p);
                }
                crate::lakitu::shake(0.7);
                worker::call("max quake", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_QUAKE, 0.6) });
                if step >= CROSS_STEPS {
                    *running = None;
                }
            }
        }
    }
}

/// The Chomp's MAX: a ring of fire round Mario, the small ones flung up (they come down and get
/// up), the rest staggered.
fn cyclone(feet: Vec3, combat: &mut Combat, player: &ChrIns, origin: [f32; 3], tick: u32) {
    let n = combat.blast(player, feet, CYCLONE_REACH, Attack::MaxCyclone, origin, tick);
    let near = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), CYCLONE_REACH, origin);
    let mut up = 0;
    for t in &near {
        let Some(h) = t.shootable().copied() else { continue };
        if !combat::liftable(&h) || crate::stopwatch::frozen(&h) || (Vec3::new(t.er.0, t.er.1, t.er.2) - feet).with_y(0.0).length() > CYCLONE_REACH {
            continue;
        }
        if crate::juggle::try_launch(&h, feet, t.radius * crate::scale(), t.height * crate::scale(), true, false, true) {
            up += 1;
        }
    }
    log(format!("max: Chomp cyclone MAX, {n} hit, {up} flung"));
    crate::impact::max_cyclone(feet);
    crate::lakitu::shake(1.1);
}

pub fn clear() {
    *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
