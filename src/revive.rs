//! Co-op: down but not out. Out of health with a partner who can still help, Mario doesn't die (nor
//! does the Tarnished): he lies there in SM64's death pose, untouchable, for BLEED seconds. The
//! partner comes over and holds D-pad down next to him: he's back on his feet with three wedges,
//! SM64's 1-UP and a spin up into the air, stars and coins. Nobody coming in time, both down, or a
//! death the game deals itself (a fall into the void): the real death, Seamless Co-op's as before.
//! Alone none of this runs.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use glam::Vec3;

use crate::log;

/// how long a downed player waits to be revived (s)
pub const BLEED: f32 = 30.0;
/// with nobody left who could revive him (the partner down too, or gone) this long (s): the end
const NOBODY: f32 = 2.0;
/// the reviver must be this close to the partner's feet (m, times Mario's size), and hold D-pad
/// down this many SM64 ticks (1 s)
const REACH: f32 = 1.8;
const HOLD_TICKS: u32 = 30;
/// the same partner isn't sent a second revive for this long (s: their status catches up)
const RESEND: f32 = 2.0;
/// health he's revived with (three wedges)
pub const HEALTH: u16 = 0x380;
/// SM64's spawn spin (the one into a level), and how fast he's sent up into it (SM64 units/tick)
pub const ACT_SPAWN_SPIN_AIRBORNE: u32 = 0x0000_1924;
pub const SPIN_UP: f32 = 52.0;
/// SM64's 1-UP, and the star sound's "let's-a-go" (on the revived side), softly
pub const SOUND_1UP: i32 = 0x3058_FF81;
pub const SOUND_LETS_GO: i32 = 0x7024_FF81;
const VOLUME: f32 = 0.7;

/// Since when we're down, and since when nobody's left who could revive us.
struct Down {
    since: Instant,
    nobody: Option<Instant>,
}

static DOWN: Mutex<Option<Down>> = Mutex::new(None);
static REVIVED_BY: AtomicU64 = AtomicU64::new(0);
/// a downed partner within reach (D-pad down revives them, it's not the game's interact then)
static NEAR: AtomicBool = AtomicBool::new(false);
/// D-pad down pressed next to a downed partner and not let go yet (after the revive too: still
/// held, it mustn't take the cap off or reach the game)
static SWALLOW: AtomicBool = AtomicBool::new(false);

/// The reviver's hold: on whom, how many ticks, the last revive sent (whom, when).
struct Hold {
    steam: u64,
    ticks: u32,
    sent: Option<(u64, Instant)>,
}

static HOLD: Mutex<Hold> = Mutex::new(Hold { steam: 0, ticks: 0, sent: None });

/// Mario is out of health (lib.rs, where the Tarnished would die with him): down instead, if a
/// partner could revive him. False: the real death.
pub fn go_down() -> bool {
    let mut down = DOWN.lock().unwrap_or_else(|e| e.into_inner());
    if down.is_some() {
        return true;
    }
    if !crate::coop::partner_can_revive() {
        return false;
    }
    REVIVED_BY.store(0, Ordering::Relaxed);
    *down = Some(Down { since: Instant::now(), nobody: None });
    log(format!("revive: down, the partner has {BLEED:.0} s to revive Mario (D-pad down next to him)"));
    true
}

/// Down, waiting for the partner.
pub fn downed() -> bool {
    DOWN.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Down: the seconds left (for the status, the HUD).
pub fn left() -> Option<u8> {
    let down = DOWN.lock().unwrap_or_else(|e| e.into_inner());
    down.as_ref().map(|d| (BLEED - d.since.elapsed().as_secs_f32()).clamp(0.0, 254.0).ceil() as u8)
}

/// A partner revived us (coop.rs, their packet).
pub fn revived(from: u64) {
    if downed() {
        REVIVED_BY.store(from, Ordering::Relaxed);
    }
}

/// What becomes of a downed Mario this frame.
#[derive(PartialEq, Debug)]
pub enum Outcome {
    /// not down, or still waiting
    Wait,
    /// back on his feet (lib.rs: health, the spin up, the 1-UP)
    Up,
    /// nobody came: the real death
    Die,
}

/// Game thread, every frame.
pub fn tick() -> Outcome {
    let mut guard = DOWN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(d) = guard.as_mut() else { return Outcome::Wait };
    let by = REVIVED_BY.swap(0, Ordering::Relaxed);
    if by != 0 {
        log(format!("revive: revived by {by:x} after {:.1} s", d.since.elapsed().as_secs_f32()));
        *guard = None;
        return Outcome::Up;
    }
    if crate::coop::partner_can_revive() {
        d.nobody = None;
    } else if d.nobody.get_or_insert_with(Instant::now).elapsed().as_secs_f32() >= NOBODY {
        log("revive: nobody left to revive Mario: he dies");
        *guard = None;
        return Outcome::Die;
    }
    if d.since.elapsed().as_secs_f32() >= BLEED {
        log("revive: nobody came in time: Mario dies");
        *guard = None;
        return Outcome::Die;
    }
    Outcome::Wait
}

/// Down no more without a revive (a real death, a respawn, out of Mario mode, out of the session).
pub fn clear() {
    *DOWN.lock().unwrap_or_else(|e| e.into_inner()) = None;
    REVIVED_BY.store(0, Ordering::Relaxed);
    NEAR.store(false, Ordering::Relaxed);
    SWALLOW.store(false, Ordering::Relaxed);
    HOLD.lock().unwrap_or_else(|e| e.into_inner()).ticks = 0;
}

/// A downed partner is within reach: D-pad down is the revive (not the game's interact, not the
/// cap off).
pub fn near() -> bool {
    NEAR.load(Ordering::Relaxed)
}

/// D-pad down is the revive's now: within reach of a downed partner, or held on from there.
pub fn owns_dpad_down() -> bool {
    NEAR.load(Ordering::Relaxed) || SWALLOW.load(Ordering::Relaxed)
}

/// The revive going on, for the HUD: whom (Steam id) and how far along (0-1; 0 within reach, not
/// held yet).
pub fn progress() -> Option<(u64, f32)> {
    if !near() {
        return None;
    }
    let h = HOLD.lock().unwrap_or_else(|e| e.into_inner());
    Some((h.steam, h.ticks as f32 / HOLD_TICKS as f32))
}

/// Game thread, each SM64 tick: our Mario at `me` (feet, Elden Ring), D-pad down `held`, free to
/// help (on his feet, not down, nothing else going on). Next to a downed partner, holding it a
/// second revives them.
pub fn hold(held: bool, me: Vec3, free: bool, size: f32) {
    if !held {
        SWALLOW.store(false, Ordering::Relaxed);
    }
    if !crate::coop::active() {
        NEAR.store(false, Ordering::Relaxed);
        return;
    }
    let reach = REACH * size.max(1.0);
    let target = crate::coop::partners()
        .into_iter()
        .filter(|p| p.flags & crate::coop::REVIVABLE != 0 && (p.feet - me).with_y(0.0).length() < reach && (p.feet.y - me.y).abs() < 1.5 * size.max(1.0))
        .min_by(|a, b| (a.feet - me).length().total_cmp(&(b.feet - me).length()));
    let mut h = HOLD.lock().unwrap_or_else(|e| e.into_inner());
    let Some(p) = target.filter(|_| free) else {
        NEAR.store(false, Ordering::Relaxed);
        h.ticks = 0;
        return;
    };
    NEAR.store(true, Ordering::Relaxed);
    if held {
        SWALLOW.store(true, Ordering::Relaxed);
    }
    if h.steam != p.steam || !held {
        h.steam = p.steam;
        h.ticks = 0;
    }
    if !held || h.sent.is_some_and(|(s, t)| s == p.steam && t.elapsed().as_secs_f32() < RESEND) {
        return;
    }
    h.ticks += 1;
    if h.ticks < HOLD_TICKS {
        return;
    }
    h.ticks = 0;
    h.sent = Some((p.steam, Instant::now()));
    // (the hold's lock let go before the state's)
    drop(h);
    log(format!("revive: reviving {:x} ({})", p.steam, if p.luigi { "Luigi" } else { "Mario" }));
    crate::coop::send_revive(p.steam);
    crate::impact::one_up(p.feet);
    crate::style::callout("1UP", Some(p.head), [0.45, 1.0, 0.5], true, 2.0);
    crate::worker::call("revive 1-up", |_| unsafe { crate::sm64::sm64_play_sound_volume(SOUND_1UP, VOLUME) });
}

/// The revived side's sounds (lib.rs, with the spin up).
pub fn sounds() {
    crate::worker::call("revived", |_| unsafe {
        crate::sm64::sm64_play_sound_volume(SOUND_1UP, VOLUME);
        crate::sm64::sm64_play_sound_volume(SOUND_LETS_GO, VOLUME);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alone_nobody_goes_down() {
        // out of a session: the real death, nothing waits
        assert!(!go_down());
        assert!(!downed());
        assert_eq!(left(), None);
        assert_eq!(tick(), Outcome::Wait);
        revived(5);
        assert_eq!(REVIVED_BY.load(Ordering::Relaxed), 0);
    }
}
