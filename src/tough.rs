//! Mario (and the co-op partner's Luigi, on their own game) is tougher than SM64's: 40% more health
//! and 30% less damage, worked as one thing: each loss of SM64 health is cut to LOSS_SHARE of it.
//! The meter keeps its eight wedges, each holding that much more. Every loss goes through here, from
//! whatever took it (Elden Ring's blows as wedges or drains, the boulder's, the saddle's, SM64's own
//! knockback ticking down, lava, water), so nothing is left out; fractions carry over to the next.
//! SM64's own verdict of death stands: a lethal fall (or the boulder's) puts him straight on 0xFF.

/// 40% more health, 30% less damage
const HEALTH_MORE: f32 = 1.4;
const DAMAGE_LESS: f32 = 0.3;

/// the share of a loss of health that's taken
fn loss_share() -> f32 {
    (1.0 - DAMAGE_LESS) / HEALTH_MORE
}

/// SM64 health that's death's: under a wedge; SM64's lethal fall sets exactly this
const DEAD_AT: i16 = 0xFF;
/// the most SM64's own knockback takes in one tick (its hurt counter, 0x40 a frame)
const TICK_LOSS: i16 = 0x40;

#[derive(Default)]
pub struct Tough {
    /// his health as it stands after the last cut (None: not seen yet)
    kept: Option<i16>,
    carry: f32,
}

impl Tough {
    /// SM64's health after a tick: what it should be instead (Some: set it), a loss since the last
    /// one cut; a gain (coins, a grace, a revive) taken as it is.
    pub fn take(&mut self, health: i16) -> Option<i16> {
        let Some(kept) = self.kept else {
            self.kept = Some(health);
            return None;
        };
        if health >= kept {
            self.kept = Some(health);
            return None;
        }
        let lost = kept - health;
        // (SM64 saying he's dead from a fall: more at once than its knockback ever takes)
        if health == DEAD_AT && lost > TICK_LOSS {
            self.kept = Some(health);
            self.carry = 0.0;
            return None;
        }
        let want = lost as f32 * loss_share() + self.carry;
        let cut = want.floor();
        self.carry = want - cut;
        let now = kept - cut as i16;
        self.kept = Some(now);
        (now != health).then_some(now)
    }

    /// A loss of `lost` health the mod takes itself, cut; where his health then stands is kept.
    pub fn cut(&mut self, health: i16, lost: i32) -> i16 {
        let want = lost.max(0) as f32 * loss_share() + self.carry;
        let cut = want.floor();
        self.carry = want - cut;
        let now = (health as i32 - cut as i32).max(0) as i16;
        self.kept = Some(now);
        now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SM64's knockback taking `wedges` at 0x40 a tick, each tick's health cut
    fn hit(t: &mut Tough, mut health: i16, wedges: i16) -> i16 {
        for _ in 0..wedges * 4 {
            let after = health - 0x40;
            health = t.take(after).unwrap_or(after);
        }
        health
    }

    #[test]
    fn hits_cost_half_what_they_did() {
        let mut t = Tough::default();
        assert_eq!(t.take(0x880), None);
        // (40% more, 30% less: 0.7 / 1.4, half)
        assert_eq!(hit(&mut t, 0x880, 2), 0x880 - 0x100);
        assert_eq!(hit(&mut t, 0x780, 1), 0x780 - 0x80);
        // eight wedges last sixteen one-wedge hits now
        let mut t = Tough::default();
        t.take(0x880);
        let mut h = 0x880;
        let mut hits = 0;
        while h >= 0x100 {
            h = hit(&mut t, h, 1);
            hits += 1;
        }
        assert_eq!(hits, 16);
    }

    #[test]
    fn gains_and_a_lethal_fall_stand() {
        let mut t = Tough::default();
        t.take(0x380);
        assert_eq!(t.take(0x880), None);
        assert_eq!(t.take(0xFF), None);
        // (a big loss that isn't SM64's verdict, the mod's drain: cut)
        let mut t = Tough::default();
        t.take(0x880);
        assert_eq!(t.take(0x780), Some(0x800));
    }

    #[test]
    fn small_losses_carry_over() {
        let mut t = Tough::default();
        t.take(0x500);
        let mut h = 0x500;
        for _ in 0..10 {
            let after = h - 1;
            h = t.take(after).unwrap_or(after);
        }
        assert_eq!(h, 0x500 - 5);
        assert_eq!(t.cut(h, 0x100), 0x500 - 5 - 0x80);
    }
}
