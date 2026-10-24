//! X held with any weapon but the boomerang (v4 Part 10): a charge while he walks (slowed), the
//! weapon gathering power in his hands; let go, a quick blow that closes in, each weapon's own:
//! the fists' bullet straight (a flaming one full), the hammer's leap into a huge hammer's slam,
//! the Chain Chomp's ram dragging him through a line of them, the Star Sword's Drive (its wave of
//! lightning along the ground; three let go right as it fills: JUST). Before, each charge waited
//! 0.6 s held and rooted him in a scripted pose: slow and clunky. Pure: moves.rs feeds it the
//! button and starts what it asks for; lib.rs slows his walk and poses the weapon meanwhile.

use crate::script::{CHOMP_RAM, CHOMP_RAM_FULL, FIST_STRAIGHT, FIST_STRAIGHT_FULL, HAMMER_LEAP, HAMMER_LEAP_FULL, SW_DRIVE, SW_DRIVE_FULL, SW_DRIVE_JUST};
use crate::weapons::{CHOMP, FISTS, HAMMER, SWORD};

/// X held this long (ticks) and the charge begins (the first blow cut short); its first level
/// here, full here; let go in JUST (the sword's): its three
pub const FROM: u32 = 8;
pub const LEVEL1: u32 = 14;
pub const FULL: u32 = 27;
pub const JUST: (u32, u32) = (27, 32);
/// his walk while charging: this share of the stick
pub const WALK: f32 = 0.55;

/// What the button did with the charge this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// nothing to do with it
    Idle,
    /// it begins now (cut the blow short: he walks)
    Begin,
    /// it's charging (nothing else this tick): its level (0, 1, 2 full), ticks held, a level reached now
    Charging { level: u8, held: u32, up: bool },
    /// let go now at this level (JUST: right as it filled)
    Release { level: u8, just: bool },
    /// called off (a jump, the dash, Y, a blow on him, off his feet)
    Cancel,
}

/// The level at `held` ticks.
pub fn level(held: u32) -> u8 {
    if held >= FULL {
        2
    } else if held >= LEVEL1 {
        1
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Charge {
    on: bool,
    level: u8,
    /// called off with X still down: no charge until it's let go
    spent: bool,
}

impl Charge {
    /// One tick: X held `held` ticks (0: up), let go now after `released` ticks; whether a charge
    /// can begin now (`can`: on his feet, a weapon that charges, no move in the way) and whether
    /// something calls it off (`cancel`).
    pub fn step(&mut self, held: u32, released: Option<u32>, can: bool, cancel: bool) -> Step {
        if held == 0 {
            self.spent = false;
        }
        if self.on {
            if cancel {
                *self = Charge { spent: held > 0, ..Charge::default() };
                return Step::Cancel;
            }
            if let Some(t) = released.or((held == 0).then_some(0)) {
                let t = if t == 0 { FROM } else { t };
                *self = Charge::default();
                return Step::Release { level: level(t), just: (JUST.0..=JUST.1).contains(&t) };
            }
            let now = level(held);
            let up = now > self.level;
            self.level = now;
            return Step::Charging { level: now, held, up };
        }
        if held >= FROM && released.is_none() && can && !cancel && !self.spent {
            *self = Charge { on: true, level: 0, spent: false };
            return Step::Begin;
        }
        Step::Idle
    }

    #[cfg(test)]
    pub fn on(&self) -> bool {
        self.on
    }
}

/// The blow a charge let go at `level` (0-2) is with `weapon`, JUST or not (the sword's three).
pub fn release_move(weapon: u8, level: u8, just: bool) -> Option<u16> {
    let full = level >= 2;
    Some(match weapon {
        FISTS => if full { FIST_STRAIGHT_FULL } else { FIST_STRAIGHT },
        HAMMER => if full { HAMMER_LEAP_FULL } else { HAMMER_LEAP },
        CHOMP => if full { CHOMP_RAM_FULL } else { CHOMP_RAM },
        SWORD if just => SW_DRIVE_JUST,
        SWORD => if full { SW_DRIVE_FULL } else { SW_DRIVE },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Held: it begins at FROM, rises a level at LEVEL1 and FULL, and is let go with its level.
    #[test]
    fn held_it_charges_and_let_go_it_strikes() {
        let mut c = Charge::default();
        for held in 1..FROM {
            assert_eq!(c.step(held, None, true, false), Step::Idle);
        }
        assert_eq!(c.step(FROM, None, true, false), Step::Begin);
        assert!(c.on());
        let mut ups = 0;
        for held in FROM + 1..=FULL + 3 {
            match c.step(held, None, true, false) {
                Step::Charging { up, level, .. } => {
                    ups += up as u32;
                    assert_eq!(level, super::level(held));
                }
                s => panic!("{held}: {s:?}"),
            }
        }
        assert_eq!(ups, 2);
        assert_eq!(c.step(0, Some(FULL + 4), true, false), Step::Release { level: 2, just: true });
        assert!(!c.on());
        // (let go early: its first level's blow, no JUST)
        let mut c = Charge::default();
        c.step(FROM, None, true, false);
        assert_eq!(c.step(0, Some(LEVEL1 + 1), true, false), Step::Release { level: 1, just: false });
        // (held long past full: no JUST)
        let mut c = Charge::default();
        c.step(FROM, None, true, false);
        assert_eq!(c.step(0, Some(60), true, false), Step::Release { level: 2, just: false });
    }

    /// It only begins when it can, and a jump, the dash or a blow call it off (no blow then).
    #[test]
    fn it_begins_only_when_it_can_and_is_called_off() {
        let mut c = Charge::default();
        assert_eq!(c.step(FROM, None, false, false), Step::Idle);
        assert_eq!(c.step(FROM + 3, None, true, false), Step::Begin);
        assert_eq!(c.step(FROM + 4, None, true, true), Step::Cancel);
        assert!(!c.on());
        assert_eq!(c.step(0, Some(FROM + 5), true, false), Step::Idle);
        // (a tap is no charge)
        let mut c = Charge::default();
        assert_eq!(c.step(0, Some(3), true, false), Step::Idle);
    }

    /// Called off with X still held (Y, a jump), it doesn't begin again until X is let go and
    /// pressed anew (it cut the Y move short into a walk).
    #[test]
    fn called_off_it_waits_for_a_new_press() {
        let mut c = Charge::default();
        c.step(FROM, None, true, false);
        assert_eq!(c.step(FROM + 2, None, true, true), Step::Cancel);
        for held in FROM + 3..FROM + 40 {
            assert_eq!(c.step(held, None, true, false), Step::Idle, "{held}");
        }
        assert_eq!(c.step(0, Some(FROM + 40), true, false), Step::Idle);
        assert_eq!(c.step(FROM, None, true, false), Step::Begin);
    }

    #[test]
    fn each_weapon_lets_go_its_own() {
        assert_eq!(release_move(FISTS, 1, false), Some(FIST_STRAIGHT));
        assert_eq!(release_move(FISTS, 2, true), Some(FIST_STRAIGHT_FULL));
        assert_eq!(release_move(HAMMER, 0, false), Some(HAMMER_LEAP));
        assert_eq!(release_move(HAMMER, 2, false), Some(HAMMER_LEAP_FULL));
        assert_eq!(release_move(CHOMP, 2, false), Some(CHOMP_RAM_FULL));
        assert_eq!(release_move(SWORD, 1, false), Some(SW_DRIVE));
        assert_eq!(release_move(SWORD, 2, false), Some(SW_DRIVE_FULL));
        assert_eq!(release_move(SWORD, 2, true), Some(SW_DRIVE_JUST));
        assert_eq!(release_move(crate::weapons::RANG, 2, false), None);
    }
}
