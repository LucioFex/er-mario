//! Mario (and the co-op partner's Luigi, on their own game) is tougher than SM64's: 40% more health
//! and 30% less damage, worked as one thing: each loss of SM64 health is cut to `loss_share` of it.
//! The meter keeps its eight wedges, each holding that much more. SM64 cuts its own losses
//! (libsm64's update_mario_health: its knockback ticking down, lava, water, gas, burns; the share
//! set when Mario is made), before it clamps a dying Mario to 0xFF, so a lethal blow still kills
//! and its lethal falls, which put him straight on 0xFF, stand. The health the mod takes itself
//! (blows while he's sent flying or let go of, the boulder's, the saddle's, drains) is cut here.
//! Fractions carry over to the next loss.

/// 40% more health, 30% less damage
const HEALTH_MORE: f32 = 1.4;
const DAMAGE_LESS: f32 = 0.3;

/// The share of a loss of health that's taken.
pub fn loss_share() -> f32 {
    (1.0 - DAMAGE_LESS) / HEALTH_MORE
}

#[derive(Default)]
pub struct Tough {
    carry: f32,
}

impl Tough {
    /// Mario's `health` after a loss of `lost` the mod takes itself, cut (0 at the least).
    pub fn cut(&mut self, health: i16, lost: i32) -> i16 {
        let want = lost.max(0) as f32 * loss_share() + self.carry;
        let cut = want.floor();
        self.carry = want - cut;
        (health as i32 - cut as i32).max(0) as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loss_costs_half() {
        // (40% more, 30% less: 0.7 / 1.4)
        assert!((loss_share() - 0.5).abs() < 1e-6);
        let mut t = Tough::default();
        assert_eq!(t.cut(0x880, 0x100), 0x880 - 0x80);
        // eight wedges last sixteen one-wedge losses now
        let mut h = 0x880;
        let mut n = 0;
        while h >= 0x100 {
            h = t.cut(h, 0x100);
            n += 1;
        }
        assert_eq!(n, 16);
    }

    #[test]
    fn small_losses_carry_over() {
        let mut t = Tough::default();
        let mut h = 0x500;
        for _ in 0..10 {
            h = t.cut(h, 1);
        }
        assert_eq!(h, 0x500 - 5);
    }
}
