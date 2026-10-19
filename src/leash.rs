//! Co-op: the two keep together. Past WARN_AT metres the HUD says so (TOO FAR, the edge arrow);
//! from PULL_FROM to PULL_TO going farther gets harder and harder until it stops (coming back is
//! always free); left more than WARP_AT apart for WARP_AFTER (a fall, a zip, Torrent, a warp), the
//! one who isn't fighting (both or neither: the guest) pops over to the other in a puff of smoke.
//! Never in a boss fight (theirs or ours: fog walls), while either is down, busy (the game moving
//! them, a menu) or not in Mario mode. Alone none of this runs. L3 held (`call`) warps there at
//! once, whoever's fighting, however close (stuck behind a wall, lost): under the same rules else.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use glam::Vec3;

use crate::coop;
use crate::log;

/// metres between the two: the warning, the pull's start and its wall (horizontal), the warp (3D)
pub const WARN_AT: f32 = 30.0;
const PULL_FROM: f32 = 35.0;
const PULL_TO: f32 = 40.0;
const WARP_AT: f32 = 48.0;
/// how long they must stay that far apart before the warp (s), the next warp at the soonest (s),
/// and no warp past this far (m: their place this far off is more likely wrong than right)
const WARP_AFTER: f32 = 2.0;
const WARP_AGAIN: f32 = 4.0;
const WARP_MAX: f32 = 150.0;
/// the warp puts him where the partner stands (solid ground, not a ledge's edge beside them), this
/// little above
pub const WARP_UP: f32 = 0.15;
/// SM64's warp pipe, in and out
pub const SOUND_PIPE_IN: i32 = 0x7016_A081;
pub const SOUND_PIPE_OUT: i32 = 0x7017_A081;

struct Leash {
    /// the partner held to this frame (feet, Elden Ring), and how far (m)
    partner: Option<(Vec3, f32)>,
    far_since: Option<Instant>,
    warped: Option<Instant>,
}

static LEASH: Mutex<Leash> = Mutex::new(Leash { partner: None, far_since: None, warped: None });

/// the player asked for the warp (L3 held), for the next frame
static CALLED: AtomicBool = AtomicBool::new(false);
/// an asked-for warp only this far off at least (m: right beside them there's nothing to fix)
const CALL_FROM: f32 = 1.5;
/// Lakitu's buzz: no warp now
const SOUND_BUZZ: i32 = 0x700E_0081;

/// Input: the player held L3: Mario warps to the partner on the next frame, if he may.
pub fn call() {
    CALLED.store(true, Ordering::Relaxed);
}

/// Game thread, every frame: our feet (Elden Ring); whether the leash holds us now (Mario mode, on
/// his feet, not down, the game not moving him); fighting, in a boss fight, busy. Some: warp there
/// (feet, Elden Ring) now.
pub fn frame(me: Vec3, free: bool, fighting: bool, boss: bool, busy: bool) -> Option<Vec3> {
    let called = CALLED.swap(false, Ordering::Relaxed);
    let to = hold(me, free, fighting, boss, busy, called);
    if called && to.is_none() {
        crate::worker::call("warp buzz", |_| unsafe { crate::sm64::sm64_play_sound_global(SOUND_BUZZ) });
    }
    to
}

fn hold(me: Vec3, free: bool, fighting: bool, boss: bool, busy: bool, called: bool) -> Option<Vec3> {
    let mut l = LEASH.lock().unwrap_or_else(|e| e.into_inner());
    // (the nearest partner on their feet; one down is gone to, not held to)
    let partner = coop::partners()
        .into_iter()
        .filter(|p| p.flags & (coop::DOWN | coop::REVIVABLE) == 0)
        .min_by(|a, b| (a.feet - me).length().total_cmp(&(b.feet - me).length()));
    // (only where they really are: the game's copy of them can lag far behind, at a join it stood
    // on the host)
    let held = partner.filter(|p| free && !boss && !busy && p.own_place && p.flags & (coop::BOSS | coop::BUSY) == 0 && p.feet.is_finite());
    let Some(p) = held else {
        if called {
            log(format!(
                "leash: no warp asked for now ({})",
                match partner {
                    None => "no partner on their feet",
                    Some(_) if !free => "Mario down or not in Mario mode",
                    Some(_) if boss || busy => "a boss fight, or the game moving him",
                    Some(p) if !p.own_place => "their place not known yet",
                    Some(_) => "the partner in a boss fight or busy",
                }
            ));
        }
        l.partner = None;
        l.far_since = None;
        return None;
    };
    let d = (p.feet - me).length();
    l.partner = Some((p.feet, d));
    if called {
        if d < CALL_FROM {
            log(format!("leash: no warp asked for now (the partner {d:.1} m off)"));
            return None;
        }
        l.far_since = None;
        l.warped = Some(Instant::now());
        log(format!("leash: L3 held, {d:.0} m apart: Mario warps to the partner"));
        return Some(p.feet + Vec3::Y * WARP_UP);
    }
    if !(WARP_AT..WARP_MAX).contains(&d) {
        l.far_since = None;
        return None;
    }
    let since = *l.far_since.get_or_insert_with(Instant::now);
    if since.elapsed().as_secs_f32() < WARP_AFTER || l.warped.is_some_and(|t| t.elapsed().as_secs_f32() < WARP_AGAIN) {
        return None;
    }
    // (the one not fighting goes; both or neither, the guest: the other side decides the same)
    let they_fight = p.flags & coop::FIGHTING != 0;
    let we_go = if fighting != they_fight { !fighting } else { coop::guest() };
    if !we_go {
        return None;
    }
    l.far_since = None;
    l.warped = Some(Instant::now());
    log(format!("leash: {d:.0} m apart for {WARP_AFTER:.0} s: Mario warps to the partner"));
    Some(p.feet + Vec3::Y * WARP_UP)
}

/// How far the partner is, past the warning (for the HUD's TOO FAR).
pub fn warning() -> Option<f32> {
    let l = LEASH.lock().unwrap_or_else(|e| e.into_inner());
    l.partner.map(|(_, d)| d).filter(|d| *d > WARN_AT)
}

/// Each SM64 tick: Mario went from `before` to `after` (feet, Elden Ring). Past PULL_FROM the way
/// out from the partner gets stiffer, at PULL_TO it stops: where he should be instead, if anywhere.
/// Only his step out is taken back, he's never moved towards them (through a wall, off a ledge:
/// their place jumping away is the warp's to see to). Horizontal only: a fall is a fall.
pub fn restrain(before: Vec3, after: Vec3) -> Option<Vec3> {
    let partner = LEASH.lock().unwrap_or_else(|e| e.into_inner()).partner.map(|(p, _)| p)?;
    pull(partner, before, after)
}

fn pull(partner: Vec3, before: Vec3, after: Vec3) -> Option<Vec3> {
    let off = (after - partner).with_y(0.0);
    let d = off.length();
    if d <= PULL_FROM || !d.is_finite() {
        return None;
    }
    let away = off / d;
    let out = (after - before).with_y(0.0).dot(away);
    // (coming back, along, or standing: free)
    if out <= 1e-4 {
        return None;
    }
    let k = ((d - PULL_FROM) / (PULL_TO - PULL_FROM)).clamp(0.0, 1.0);
    Some(after - away * out * k)
}

/// Out of a session, out of Mario mode: nothing held.
pub fn clear() {
    CALLED.store(false, Ordering::Relaxed);
    let mut l = LEASH.lock().unwrap_or_else(|e| e.into_inner());
    l.partner = None;
    l.far_since = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pull_stiffens_and_stops() {
        let p = Vec3::ZERO;
        let step = |x: f32| pull(p, Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.3, 0.0, 0.0));
        // close: free
        assert_eq!(step(20.0), None);
        // in the pull: some of the step taken back, more the farther
        let a = step(36.0).expect("pulled").x - 36.0;
        let b = step(39.0).expect("pulled").x - 39.0;
        assert!(a > 0.0 && b < a && b >= 0.0, "{a} {b}");
        // past the wall: the step out taken back whole, never pulled in
        let at = pull(p, Vec3::new(41.0, 2.0, 0.0), Vec3::new(41.5, 1.0, 0.0)).expect("held");
        assert!((at.x - 41.0).abs() < 1e-3 && at.y == 1.0);
        assert_eq!(pull(p, Vec3::new(47.0, 0.0, 0.0), Vec3::new(47.0, 0.0, 0.0)), None);
        // coming back: free, even in the pull
        assert_eq!(pull(p, Vec3::new(38.0, 0.0, 0.0), Vec3::new(37.7, 0.0, 0.0)), None);
        // falling straight down far below: not held (the warp is for that)
        assert_eq!(pull(p, Vec3::new(10.0, -5.0, 0.0), Vec3::new(10.0, -40.0, 0.0)), None);
    }

    #[test]
    fn alone_nothing_holds() {
        assert_eq!(frame(Vec3::ZERO, true, false, false, false), None);
        assert_eq!(warning(), None);
        assert_eq!(restrain(Vec3::ZERO, Vec3::X * 100.0), None);
    }
}
