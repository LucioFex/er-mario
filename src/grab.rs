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

use eldenring::cs::{ChrIns, ThrowNodeState, WorldChrMan};
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
const ACT_THROWN_BACKWARD: u32 = 0x010208BE;
const SOUND_WAAAOOOW: i32 = 0x2410_C081_u32 as i32;
const SOUND_MAMA_MIA: i32 = 0x2420_8081;
const SOUND_BUBBA_CHOMP: i32 = 0x5073_4081;

struct State {
    /// the throw state last seen (logged when it changes)
    last: Option<ThrowNodeState>,
    held_since: Option<Instant>,
    /// deep in the holder's body since then; swallowed
    inside_since: Option<Instant>,
    swallowed: bool,
    /// the holder (character id), for the log
    holder: Option<u32>,
}

static STATE: Mutex<State> = Mutex::new(State { last: None, held_since: None, inside_since: None, swallowed: false, holder: None });

/// Being let go: thrown off, and whether he was swallowed (spat out).
pub struct Release {
    pub swallowed: bool,
}

fn held_state(s: ThrowNodeState) -> bool {
    matches!(s, ThrowNodeState::InThrowTarget | ThrowNodeState::DeathTarget)
}

/// Mario is hidden inside the one that grabbed him.
pub fn swallowed() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).swallowed
}

/// Game thread, every frame: whether an enemy holds the Tarnished now, and the release if it just
/// let him go.
pub fn update(player: &ChrIns) -> (bool, Option<Release>) {
    let state = player.modules.throw.throw_node.throw_state;
    let held = held_state(state);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.last != Some(state) {
        if s.last.is_some() || held {
            log(format!("grab: throw state {:?} -> {state:?}, the Tarnished's anim {}", s.last, current_anim(player)));
        }
        s.last = Some(state);
    }
    let p = player.modules.physics.position;
    let me = Vec3::new(p.0, p.1 + 0.8, p.2);
    if !held {
        let release = s.held_since.take().map(|t| {
            log(format!(
                "grab: let go after {:.1} s by {}{}",
                t.elapsed().as_secs_f32(),
                s.holder.map_or("someone".to_string(), |id| format!("c{id:04}")),
                if s.swallowed { ", spat out" } else { "" }
            ));
            Release { swallowed: s.swallowed }
        });
        s.inside_since = None;
        s.swallowed = false;
        s.holder = None;
        return (false, release);
    }
    if s.held_since.is_none() {
        s.held_since = Some(Instant::now());
    }
    // the holder: the nearest enemy; inside its body (the middle of it, up off its feet), for a
    // moment: swallowed
    let holder = nearest(me);
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
    if inside {
        let since = *s.inside_since.get_or_insert_with(Instant::now);
        if !s.swallowed && since.elapsed().as_secs_f32() >= SWALLOW_AFTER {
            s.swallowed = true;
            log("grab: swallowed");
            crate::worker::call("swallowed", |_| unsafe { crate::sm64::sm64_play_sound_global(SOUND_BUBBA_CHOMP) });
        }
    } else {
        s.inside_since = None;
    }
    (true, None)
}

/// The nearest enemy to `at`: character id, feet, body radius and height (m).
fn nearest(at: Vec3) -> Option<(u32, Vec3, f32, f32)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    crate::combat::in_the_way(at, HOLDER_NEAR)
        .into_iter()
        .filter_map(|(h, ..)| {
            let chr = wcm.chr_ins_by_handle(&h)?;
            let ph = &chr.modules.physics;
            let feet = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
            let r = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.3, 15.0);
            let hgt = ph.hit_height.max(ph.chr_hit_height).clamp(0.8, 40.0);
            Some((chr.character_id, feet, r, hgt))
        })
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

fn current_anim(chr: &ChrIns) -> i32 {
    let t = &chr.modules.time_act;
    t.anim_queue[(t.read_idx % 10) as usize].anim_id
}

pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.held_since = None;
    s.inside_since = None;
    s.swallowed = false;
    s.holder = None;
}
