//! Mario's weapons, on L1: a tap is the next one (fists, the hammer, the Chain Chomp, the
//! boomerang), held a moment back to the fists. Each changes what X and Y do (moves.rs, er_moves.c):
//! - the hammer (hammer.rs): heavy blows, slams, the giant hammer
//! - the boomerang (boomerang.rs): from afar, several in the air at once, each going and coming back
//! - the Chain Chomp (chomp.rs): wide, on its chain, many enemies at once, a little each
//! A banner names the new one for a moment (hud.rs).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

pub const FISTS: u8 = 0;
pub const HAMMER: u8 = 1;
pub const RANG: u8 = 2;
pub const CHOMP: u8 = 3;
pub const COUNT: u8 = 4;
pub const NAMES: [&str; 4] = ["FISTS", "HAMMER", "BOOMERANG", "CHAIN CHOMP"];
/// the order a tap of L1 goes through them
const ORDER: [u8; 4] = [FISTS, HAMMER, CHOMP, RANG];
/// how long the banner shows (s)
pub const BANNER_TIME: f32 = 1.3;

static CURRENT: AtomicU8 = AtomicU8::new(FISTS);
static SWITCHED: Mutex<Option<Instant>> = Mutex::new(None);

pub fn current() -> u8 {
    CURRENT.load(Ordering::Relaxed)
}

/// The one after `w` (L1 tapped).
pub fn next(w: u8) -> u8 {
    let i = ORDER.iter().position(|o| *o == w).unwrap_or(0);
    ORDER[(i + 1) % ORDER.len()]
}

/// Takes `w` in hand (the banner names it).
pub fn set(w: u8) {
    CURRENT.store(w % COUNT, Ordering::Relaxed);
    *SWITCHED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    crate::log(format!("weapons: {}", NAMES[(w % COUNT) as usize]));
}

/// The banner: the weapon in hand and how long ago it was taken (s).
pub fn banner() -> Option<(u8, f32)> {
    SWITCHED.lock().unwrap_or_else(|e| e.into_inner()).map(|t| (current(), t.elapsed().as_secs_f32())).filter(|(_, t)| *t < BANNER_TIME)
}

pub fn clear() {
    CURRENT.store(FISTS, Ordering::Relaxed);
    *SWITCHED.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
