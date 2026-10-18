//! MAX finishers (style.rs counts the streak): twelve blows in a row without being hurt and the
//! weapon glows; its next finisher is a MAX one, a bigger version of what it does:
//! - the fists: a burst of phantom blows on what's in front, then the ones it can lift sent up
//! - the hammer: a quake in a cross, its four arms rolling out from where it struck
//! - the Chain Chomp: the cyclone's MAX, a ring of fire that flings the small ones up and staggers
//!   the rest
//! - the boomerang: its big throw is three big ones at once (boomerang.rs)
//! Still blows, not kills: each is a share like the rest, bosses take their own small one.

use std::sync::Mutex;

use eldenring::cs::ChrIns;
use eldenring::position::HavokPosition;
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

const SOUND_PUNCH: [i32; 3] = [0x0444_C081, 0x0444_B081, 0x043F_8081];
const SOUND_QUAKE: i32 = 0x312F_0081;
const SOUND_STAR: i32 = 0x3016_0091;

enum Show {
    Flurry { at: Vec3, dir: Vec3 },
    Cross { feet: Vec3, dir: Vec3 },
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
                    for t in near.iter().filter(|t| t.shootable().is_some_and(combat::liftable)) {
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
        Show::Cross { feet, dir } => {
            if t > 0 && t % CROSS_EVERY == 0 {
                let step = t / CROSS_EVERY;
                let side = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                for arm in [dir, side, -dir, -side] {
                    let p = feet + arm * CROSS_STEP * step as f32;
                    combat.blast(player, p, CROSS_BLAST, Attack::MaxQuake, origin, tick);
                    crate::impact::max_quake(p);
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
        if !combat::liftable(&h) || (Vec3::new(t.er.0, t.er.1, t.er.2) - feet).with_y(0.0).length() > CYCLONE_REACH {
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
