//! Enemies' grabs (Elden Ring's throws: a troll's fist, a beast's jaws, a boss lifting the
//! Tarnished up): the game holds the Tarnished and moves him through its throw, so for its length
//! Mario follows him (lib.rs's follow mode, as through a door) instead of the Tarnished following
//! Mario. Mario was moving him back every frame, which broke every grab before it began. Held, he
//! dangles in its arms kicking (SM64's grab by Chuckya, libsm64 ACT_ER_GRABBED); one that holds
//! him inside its body has swallowed him (he's hidden, Bubba's); let go, he's thrown off (SM64's
//! thrown backward), spat out with a cry if he was swallowed. The throw's damage is the game's,
//! taken as Mario's wedges when he's free again.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::log;

/// an enemy this close (m) is the one holding him; he's swallowed when he's this deep in its body
/// (share of its radius, off its centre) and up in it (share of its height), for this long (s)
const HOLDER_NEAR: f32 = 6.0;
const INSIDE: f32 = 0.45;
const INSIDE_UP: f32 = 0.3;
const SWALLOW_AFTER: f32 = 0.25;

pub const ACT_ER_GRABBED: u32 = 0x0002_035D;
pub const ACT_THROWN_BACKWARD: u32 = 0x010208BE;
/// the throw node's states (eldenring ThrowNodeState, read raw: the game also keeps values the enum
/// doesn't have, 0 at rest): held, and held for a killing throw
const IN_THROW_TARGET: u32 = 4;
const DEATH_TARGET: u32 = 6;
/// after being let go, the throw's damage comes in as health only for this long (s): SM64's
/// knockback would end his flight off the enemy
const RELEASE_GRACE: f32 = 0.6;
const SOUND_WAAAOOOW: i32 = 0x2410_C081_u32 as i32;
const SOUND_MAMA_MIA: i32 = 0x2420_8081;
const SOUND_BUBBA_CHOMP: i32 = 0x5073_4081;

/// up on a captured enemy, a throw waits this long (s) to be told apart from its own
const RIDING_WAIT: f32 = 0.15;

struct State {
    /// the throw state last seen (logged when it changes)
    last: Option<u32>,
    /// a state that outlasted the follow's time limit: not a grab any more until it changes
    ignore: Option<u32>,
    /// the last release, until lib.rs throws him off; when it was
    release: Option<Release>,
    released_at: Option<Instant>,
    held_since: Option<Instant>,
    /// deep in the holder's body since then; swallowed
    inside_since: Option<Instant>,
    swallowed: bool,
    /// the holder (character id), for the log
    holder: Option<u32>,
    /// up on a captured enemy: since when a throw has been waiting to be told apart (its own?)
    riding_wait: Option<Instant>,
}

static STATE: Mutex<State> =
    Mutex::new(State { last: None, ignore: None, release: None, released_at: None, held_since: None, inside_since: None, swallowed: false, holder: None, riding_wait: None });

/// Being let go: thrown off, and whether he was swallowed (spat out).
#[derive(Clone, Copy)]
pub struct Release {
    pub swallowed: bool,
}

/// The player's throw node state, as the number it is.
fn raw_state(player: &ChrIns) -> u32 {
    let node = &*player.modules.throw.throw_node;
    unsafe { std::ptr::addr_of!(node.throw_state).cast::<u32>().read() }
}

/// The last release, once (lib.rs throws Mario off with it).
pub fn take_release() -> Option<Release> {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).release.take()
}

/// Let go a moment ago: the throw's damage costs health, not a knockback.
pub fn just_released() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).released_at.is_some_and(|t| t.elapsed().as_secs_f32() < RELEASE_GRACE)
}

/// The follow ran out of time with him still "held": that state isn't a grab until it changes
/// (a throw broken off oddly could leave it, and he'd be grabbed again at once, for good).
pub fn ignore_current(player: &ChrIns) {
    let raw = raw_state(player);
    log(format!("grab: still held after the time limit (state {raw}): ignored until it changes"));
    STATE.lock().unwrap_or_else(|e| e.into_inner()).ignore = Some(raw);
}

/// Mario is hidden inside the one that grabbed him.
pub fn swallowed() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).swallowed
}

/// Game thread, every frame: whether an enemy holds the Tarnished now (a release is kept for
/// take_release).
pub fn update(player: &ChrIns) -> bool {
    let state = raw_state(player);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.ignore.is_some_and(|i| i != state) {
        s.ignore = None;
    }
    let held = (state == IN_THROW_TARGET || state == DEATH_TARGET) && s.ignore.is_none();
    // (a throw of the enemy he rides: not one, he stays on its back; the game may name the one
    // holding him a frame or two late, so a throw up there waits a moment to be told apart)
    if !held || !crate::bossride::active() {
        s.riding_wait = None;
    } else if s.held_since.is_none() {
        if crate::bossride::own_throw(player) {
            log(format!("grab: a throw by the enemy Mario rides (state {state}), ignored"));
            s.ignore = Some(state);
            s.riding_wait = None;
            return false;
        }
        if s.riding_wait.get_or_insert_with(Instant::now).elapsed().as_secs_f32() < RIDING_WAIT {
            return false;
        }
    }
    if s.last != Some(state) {
        if s.last.is_some() || held {
            log(format!("grab: throw state {:?} -> {state}, the Tarnished's anim {}", s.last, crate::current_anim(player)));
        }
        s.last = Some(state);
    }
    let p = player.modules.physics.position;
    let me = Vec3::new(p.0, p.1 + 0.8, p.2);
    if !held {
        if let Some(t) = s.held_since.take() {
            log(format!(
                "grab: let go after {:.1} s by {}{}",
                t.elapsed().as_secs_f32(),
                s.holder.map_or("someone".to_string(), |id| format!("c{id:04}")),
                if s.swallowed { ", spat out" } else { "" }
            ));
            s.release = Some(Release { swallowed: s.swallowed });
            s.released_at = Some(Instant::now());
        }
        s.inside_since = None;
        s.swallowed = false;
        s.holder = None;
        return false;
    }
    if s.held_since.is_none() {
        s.held_since = Some(Instant::now());
        s.release = None;
    }
    // the holder: the one on the other end of the throw (or the nearest enemy); inside its body
    // (the middle of it, up off its feet), for a moment: swallowed
    let holder = holder_of(player).or_else(|| nearest(me));
    if let Some((id, ..)) = holder {
        if s.holder.is_none() {
            log(format!("grab: held by c{id:04}"));
        }
        s.holder = Some(id);
    }
    let inside = holder.is_some_and(|(_, feet, radius, height)| {
        let flat = (me - feet).with_y(0.0).length();
        flat < radius * INSIDE && me.y > feet.y + height * INSIDE_UP && me.y < feet.y + height * 1.1
    });
    let mut chomp = false;
    if inside {
        let since = *s.inside_since.get_or_insert_with(Instant::now);
        if !s.swallowed && since.elapsed().as_secs_f32() >= SWALLOW_AFTER {
            s.swallowed = true;
            chomp = true;
        }
    } else {
        s.inside_since = None;
    }
    drop(s);
    if chomp {
        log("grab: swallowed");
        crate::worker::call("swallowed", |_| unsafe { crate::sm64::sm64_play_sound_global(SOUND_BUBBA_CHOMP) });
    }
    true
}

/// The one holding him: the other end of the throw's pair animation.
fn holder_of(player: &ChrIns) -> Option<(u32, Vec3, f32, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(&player.modules.throw.throw_node.pair_anim_node.counter_party)?;
    body(chr)
}

fn body(chr: &ChrIns) -> Option<(u32, Vec3, f32, f32)> {
    let ph = &chr.modules.physics;
    let feet = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let r = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.3, 15.0);
    let hgt = ph.hit_height.max(ph.chr_hit_height).clamp(0.8, 40.0);
    Some((chr.character_id, feet, r, hgt))
}

/// The nearest enemy to `at`: character id, feet, body radius and height (m).
fn nearest(at: Vec3) -> Option<(u32, Vec3, f32, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    crate::combat::in_the_way(at, HOLDER_NEAR)
        .into_iter()
        .filter_map(|(h, ..)| body(wcm.chr_ins_by_handle(&h)?))
        .min_by(|a, b| (a.1 - at).length_squared().total_cmp(&(b.1 - at).length_squared()))
}

/// Worker thread: Mario thrown off by the one that let him go (backwards, up a little), with a cry.
pub fn throw_off(id: i32, release: &Release) {
    unsafe {
        crate::sm64::sm64_set_mario_action(id, ACT_THROWN_BACKWARD);
        crate::sm64::sm64_set_mario_forward_velocity(id, -28.0);
        crate::sm64::sm64_set_mario_velocity(id, 0.0, 34.0, 0.0);
        crate::sm64::sm64_play_sound_global(if release.swallowed { SOUND_WAAAOOOW } else { SOUND_MAMA_MIA });
    }
}

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.held_since = None;
    s.inside_since = None;
    s.swallowed = false;
    s.holder = None;
    s.release = None;
    s.ignore = None;
}
