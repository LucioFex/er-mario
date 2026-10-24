//! A few buttons' rules, kept pure (lib.rs's input task reads the pad): the game's use item let
//! through after a menu, and L3's tap and hold.

/// how long after a menu closes the game may still use an item asked for in it (s)
pub const MENU_USE_AFTER: f32 = 1.0;

/// L3 held this long (s): the whistle (Yoshi); let go sooner, a tap (the kart)
pub const YOSHI_HOLD: f32 = 1.5;

/// The game's use-item request let through for the inventory's Use: while a menu is up, and a
/// moment after it closes (it closes as the item comes out), but not once X or R has been pressed
/// since it closed: that's Mario's punch, and the first one after a talk used the quick slot's
/// item, the whistle. `closed_ago`: how long since the menu closed (s), if one did.
pub fn item_from_menu(open: bool, closed_ago: Option<f32>, pressed_since: bool) -> bool {
    open || (closed_ago.is_some_and(|t| t < MENU_USE_AFTER) && !pressed_since)
}

/// What a button held a while came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// let go before the hold's time (on the release)
    Tap,
    /// held the hold's time (once, as it gets there; its release then is nothing)
    Held,
}

/// A button told apart by how long it's held.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hold {
    since: Option<f32>,
    fired: bool,
}

impl Hold {
    /// Not held (for a static's start).
    pub const NEW: Hold = Hold { since: None, fired: false };

    /// The button `down` at `now` (s), held `hold` s for a hold.
    pub fn step(&mut self, down: bool, now: f32, hold: f32) -> Option<Press> {
        match (down, self.since) {
            (true, None) => {
                self.since = Some(now);
                self.fired = false;
                None
            }
            (true, Some(t)) if !self.fired && now - t >= hold => {
                self.fired = true;
                Some(Press::Held)
            }
            (true, Some(_)) => None,
            (false, Some(_)) => {
                let tap = !self.fired;
                self.since = None;
                self.fired = false;
                tap.then_some(Press::Tap)
            }
            (false, None) => None,
        }
    }

    /// How far into the hold it is (0-1; 0 when it isn't held, or the hold went off).
    pub fn fill(&self, now: f32, hold: f32) -> f32 {
        match self.since {
            Some(t) if !self.fired => ((now - t) / hold.max(1e-3)).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Taken as it's pressed `now` (for something else: Mega Mario's buzz): held or let go, nothing
    /// comes of this press.
    pub fn swallow(&mut self, now: f32) {
        self.since = Some(now);
        self.fired = true;
    }

    /// Dropped (a menu came up, he got busy): its release is nothing.
    pub fn cancel(&mut self) {
        if self.since.is_some() {
            self.fired = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_after_a_menu_only_without_a_press() {
        assert!(item_from_menu(true, None, false));
        assert!(item_from_menu(true, Some(0.2), true));
        // (the inventory's Use: the item comes out as the menu closes)
        assert!(item_from_menu(false, Some(0.4), false));
        // (a talk over, then a punch: no whistle)
        assert!(!item_from_menu(false, Some(0.4), true));
        assert!(!item_from_menu(false, Some(MENU_USE_AFTER + 0.1), false));
        assert!(!item_from_menu(false, None, false));
    }

    #[test]
    fn a_tap_on_the_release_a_hold_once_as_it_gets_there() {
        let mut h = Hold::default();
        assert_eq!(h.step(true, 0.0, YOSHI_HOLD), None);
        assert_eq!(h.step(true, 0.5, YOSHI_HOLD), None);
        assert!((h.fill(0.75, YOSHI_HOLD) - 0.5).abs() < 1e-4);
        assert_eq!(h.step(false, 0.6, YOSHI_HOLD), Some(Press::Tap));
        assert_eq!(h.step(false, 0.7, YOSHI_HOLD), None);
        assert_eq!(h.fill(0.7, YOSHI_HOLD), 0.0);
        // held: once, as it gets there; the release after it is nothing
        assert_eq!(h.step(true, 1.0, YOSHI_HOLD), None);
        assert_eq!(h.step(true, 2.49, YOSHI_HOLD), None);
        assert_eq!(h.step(true, 2.5, YOSHI_HOLD), Some(Press::Held));
        assert_eq!(h.fill(2.6, YOSHI_HOLD), 0.0);
        assert_eq!(h.step(true, 3.5, YOSHI_HOLD), None);
        assert_eq!(h.step(false, 3.6, YOSHI_HOLD), None);
        // dropped half way: no tap when it's let go
        assert_eq!(h.step(true, 4.0, YOSHI_HOLD), None);
        h.cancel();
        assert_eq!(h.fill(4.5, YOSHI_HOLD), 0.0);
        assert_eq!(h.step(false, 4.6, YOSHI_HOLD), None);
        // swallowed as it's pressed (Mega Mario's buzz): nothing held on or let go comes of it
        h.swallow(5.0);
        assert_eq!(h.step(true, 5.0, YOSHI_HOLD), None);
        assert_eq!(h.step(true, 7.0, YOSHI_HOLD), None);
        assert_eq!(h.fill(7.0, YOSHI_HOLD), 0.0);
        assert_eq!(h.step(false, 7.1, YOSHI_HOLD), None);
        assert_eq!(h.step(true, 8.0, YOSHI_HOLD), None);
        assert_eq!(h.step(false, 8.1, YOSHI_HOLD), Some(Press::Tap));
    }
}
