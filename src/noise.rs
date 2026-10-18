//! Mario makes noise. Elden Ring's enemies hear the Tarnished's footsteps, jumps, landings and
//! blows (each makes an "AI sound": the game's own noise that sends them to look, then to fight),
//! but in Mario mode the game never walks him (the stick is Mario's: it stands still, carried along),
//! so most enemies only noticed Mario once he hit them. So Mario makes those noises himself, like a
//! player would: walking softly, running, jumping, landing and fighting louder, the heavy blows
//! loudest; standing still or creeping (crouched) he's silent, so sneaking up still works.
//!
//! The noise comes from the game: an unused dev bullet with an unused attack (the same kind
//! combat.rs takes for Mario's blows), harmless (no damage, no target), whose attack "appears" with
//! one of the game's own AI sounds, picked by radius from the ones its attacks already use.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{AiSoundParam, AtkParam_Pc, Bullet, ChrIns, SoloParamRepository};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;

use crate::{log, sm64};

/// dev bullets and attacks to take for the noise (the first unused one of each, from these
/// ranges, in order), never combat.rs's 65-70 and 994-999 (in the first game tried, none of 71-79
/// and 988-993 would do: the rows are looked for wider now, and the ones there logged)
const BULLETS: [std::ops::RangeInclusive<u32>; 3] = [71..=99, 0..=64, 100..=199];
const ATTACKS: [std::ops::RangeInclusive<u32>; 3] = [980..=993, 900..=979, 0..=899];
const TAKEN_BULLETS: std::ops::RangeInclusive<u32> = 65..=70;
const TAKEN_ATTACKS: std::ops::RangeInclusive<u32> = 994..=999;
/// how far each level of noise carries (m): a walk, a run / jump / blow, a heavy blow
const RADII: [f32; 3] = [5.0, 10.0, 18.0];
/// only with an enemy this close (m)
const HEAR_FROM: f32 = 20.0;
/// at most one noise this often (s), sooner for a louder one
const EVERY: f32 = 2.0;
const LOUDER_EVERY: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Level {
    Walk = 0,
    Loud = 1,
    Heavy = 2,
}

struct Setup {
    bullet: u32,
    atk: u32,
    /// the AI sound for each level
    sounds: [i32; 3],
}

struct State {
    setup: Option<Option<Setup>>,
    last: Option<(Instant, Level)>,
}

static STATE: Mutex<State> = Mutex::new(State { setup: None, last: None });

/// The AI sounds the game's attacks make as they appear (a swing, a shot: not a "got hit" one),
/// of the kind most of them are, with their radius (m); and that kind.
fn known_sounds(repo: &SoloParamRepository) -> (Vec<(i32, f32)>, u8) {
    let mut ids: Vec<i32> = Vec::new();
    for i in 0..200_000 {
        let Some(a) = repo.get_row_by_index::<AtkParam_Pc>(i) else { break };
        let id = a.appear_ai_sound_id();
        if id > 0 && !ids.contains(&id) {
            ids.push(id);
        }
        if ids.len() >= 64 {
            break;
        }
    }
    let rows: Vec<(i32, f32, u8, bool)> =
        ids.into_iter().filter_map(|id| repo.get::<AiSoundParam>(id as u32).map(|s| (id, s.radius(), s.typ(), s.b_sp_effect_enable() != 0))).filter(|r| r.1 > 0.5).collect();
    let mut kinds: Vec<(u8, usize)> = Vec::new();
    for r in &rows {
        match kinds.iter_mut().find(|k| k.0 == r.2) {
            Some(k) => k.1 += 1,
            None => kinds.push((r.2, 1)),
        }
    }
    let kind = kinds.iter().max_by_key(|k| k.1).map_or(0, |k| k.0);
    (rows.iter().filter(|r| r.2 == kind && !r.3).map(|r| (r.0, r.1)).collect(), kind)
}

fn setup() -> Option<Setup> {
    let repo = unsafe { SoloParamRepository::instance_mut() }.ok()?;
    // (only unused ones: everything about the row is overwritten below, and a real one would be
    // some attack of the game's broken for good; none found, Mario stays silent and the rows there
    // are logged)
    let bullets: Vec<u32> = BULLETS.iter().cloned().flatten().filter(|b| !TAKEN_BULLETS.contains(b) && repo.get::<Bullet>(*b).is_some()).collect();
    let atks: Vec<u32> = ATTACKS.iter().cloned().flatten().filter(|a| !TAKEN_ATTACKS.contains(a) && repo.get::<AtkParam_Pc>(*a).is_some()).collect();
    log(format!("noise: bullets there {:?}, attacks there {:?}", &bullets[..bullets.len().min(12)], &atks[..atks.len().min(12)]));
    let bullet = bullets.iter().copied().find(|&b| repo.get::<Bullet>(b).is_some_and(|r| r.atk_id_bullet() <= 0 && r.sfx_id_bullet() <= 0));
    let atk = atks.iter().copied().find(|&a| repo.get::<AtkParam_Pc>(a).is_some_and(|r| r.atk_phys() == 0 && r.appear_ai_sound_id() <= 0));
    let (sounds, kind) = known_sounds(repo);
    let (Some(bullet), Some(atk), false) = (bullet, atk, sounds.is_empty()) else {
        log(format!("noise: no free bullet ({bullet:?}) or attack ({atk:?}), or no AI sounds ({}): Mario makes no noise", sounds.len()));
        return None;
    };
    let pick = |want: f32| sounds.iter().min_by(|a, b| (a.1 - want).abs().total_cmp(&(b.1 - want).abs())).map_or(-1, |s| s.0);
    let chosen = RADII.map(pick);
    let radius = |id: i32| sounds.iter().find(|s| s.0 == id).map_or(0.0, |s| s.1);
    log(format!(
        "noise: bullet {bullet}, attack {atk}, AI sounds {:?} (radius {:?} m, kind {kind}, of {} known)",
        chosen,
        chosen.map(radius),
        sounds.len()
    ));
    let b = repo.get_mut::<Bullet>(bullet)?;
    b.set_atk_id_bullet(atk as i32);
    b.set_sfx_id_bullet(-1);
    b.set_sfx_id_hit(-1);
    b.set_sfx_id_flick(-1);
    b.set_life(0.1);
    b.set_dist(0.0);
    b.set_init_vellocity(0.0);
    b.set_max_vellocity(0.0);
    b.set_min_vellocity(0.0);
    b.set_hit_radius(0.01);
    b.set_hit_radius_max(0.01);
    b.set_num_shoot(1);
    b.set_is_penetrate_chr(true);
    b.set_is_penetrate_obj(true);
    b.set_is_penetrate_map(true);
    b.set_is_hit_both_team(false);
    // (nothing else: no child bullets, effects or sounds of its own, and enemies don't dodge it,
    // guard against it or have their shots erased by it)
    b.set_hit_bullet_id(-1);
    b.set_interval_create_bullet_id(-1);
    b.set_sp_effect_id_for_shooter(-1);
    b.set_sp_effect_id0(-1);
    b.set_sp_effect_id1(-1);
    b.set_sp_effect_id2(-1);
    b.set_sp_effect_id3(-1);
    b.set_sp_effect_id4(-1);
    b.set_se_id_bullet1(-1);
    b.set_se_id_bullet2(-1);
    b.set_se_id_hit(-1);
    b.set_se_id_flick(-1);
    b.set_external_force(0.0);
    b.set_is_disable_ai_projectile_dodge(true);
    b.set_is_ai_interrupt_shoot_no_damage_bullet(false);
    b.set_is_hit_other_bullet_force_erase_a(false);
    b.set_is_hit_other_bullet_force_erase_b(false);
    let a = repo.get_mut::<AtkParam_Pc>(atk)?;
    a.set_atk_phys(0);
    a.set_atk_obj(0);
    a.set_atk_stam(0);
    a.set_atk_super_armor(0.0);
    a.set_dmg_level(0);
    a.set_oppose_target(false);
    a.set_friendly_target(false);
    a.set_self_target(false);
    a.set_hit_ai_sound_id(-1);
    Some(Setup { bullet, atk, sounds: chosen })
}

/// Game thread, every SM64 tick: what Mario did this tick (and before), and the noise it makes.
pub fn tick(player: &ChrIns, state: &sm64::SM64MarioState, before: &sm64::SM64MarioState, attacking: bool, heavy: bool) {
    const ACT_FLAG_AIR: u32 = 0x800;
    const ACT_FLAG_STATIONARY: u32 = 0x200;
    const ACT_CROUCHING: u32 = 0x0C008220;
    const ACT_CRAWLING: u32 = 0x04008448;
    let air = state.action & ACT_FLAG_AIR != 0;
    let was_air = before.action & ACT_FLAG_AIR != 0;
    let speed = state.forward_velocity.abs();
    let quiet = matches!(state.action, ACT_CROUCHING | ACT_CRAWLING) || state.action & ACT_FLAG_STATIONARY != 0;
    let level = if heavy {
        Some(Level::Heavy)
    } else if attacking || (air != was_air) || (!air && speed > 20.0) {
        Some(Level::Loud)
    } else if !air && !quiet && speed > 4.0 {
        Some(Level::Walk)
    } else {
        None
    };
    let Some(level) = level else { return };
    // (each noise costs one of the ~256 bullets the game gives the player a session, combat.rs:
    // only with an enemy near enough to hear it, and not once Mario is fighting, his blows tell)
    let p = player.modules.physics.position;
    if crate::combat::in_the_way(glam::Vec3::new(p.0, p.1, p.2), HEAR_FROM).is_empty() || attacking {
        return;
    }
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, last)) = s.last {
        let age = t.elapsed().as_secs_f32();
        if age < LOUDER_EVERY || (age < EVERY && level <= last) {
            return;
        }
    }
    if s.setup.is_none() {
        s.setup = Some(setup());
    }
    let Some(Some(setup)) = s.setup.as_ref() else { return };
    let (bullet, atk, sound) = (setup.bullet, setup.atk, setup.sounds[level as usize]);
    s.last = Some((Instant::now(), level));
    drop(s);
    if let Some(a) = (unsafe { SoloParamRepository::instance_mut() }).ok().and_then(|r| r.get_mut::<AtkParam_Pc>(atk)) {
        a.set_appear_ai_sound_id(sound);
    }
    let p = player.modules.physics.position;
    crate::combat::shoot(player, &HavokPosition(p.0, p.1, p.2, 0.0), bullet as i32);
}
