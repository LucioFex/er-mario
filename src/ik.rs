//! Two-bone arm IK on Mario's SM64 parts (upper arm, forearm, hand): the hand reaches a goal (a
//! thrust's target, a weapon's grip along its path), the elbow bending toward a pole, the bones
//! keeping their lengths. Blended in by a weight (0: SM64's pose as it is, 1: all the way), turned
//! by angle so a half-weighted arm doesn't shrink.

use glam::{Quat, Vec3};

use crate::engine_mario::{PartPose, PARTS};

/// SM64's arm parts: upper arm, forearm, hand (left, right), and the peace-sign copy of the right
/// hand (engine_mario.rs)
const LEFT: [usize; 3] = [4, 5, 6];
const RIGHT: [usize; 3] = [7, 8, 9];
const PEACE: usize = 20;

/// Turns the arm (`right` or left) so its hand reaches `goal` (part space: relative to Mario), the
/// elbow toward `pole` (a direction), by `weight`.
pub fn arm(parts: &mut [PartPose; PARTS], right: bool, goal: Vec3, pole: Vec3, weight: f32) {
    let w = weight.clamp(0.0, 1.0);
    if w <= 0.001 || !goal.is_finite() {
        return;
    }
    let [s, e, h] = if right { RIGHT } else { LEFT };
    let (a, b, c) = (parts[s].pos, parts[e].pos, parts[h].pos);
    let (l1, l2) = ((b - a).length(), (c - b).length());
    if l1 < 1e-5 || l2 < 1e-5 {
        return;
    }
    let to = goal - a;
    let dir = to.try_normalize().unwrap_or_else(|| (c - a).normalize_or(Vec3::X));
    // (a hair short of the arm's full and least reach: a dead straight arm has no bend plane)
    let d = to.length().clamp((l1 - l2).abs() + 1e-4, l1 + l2 - 1e-4);
    // the elbow bends in the plane of the reach and the pole (else the way it bends now)
    let flat = |v: Vec3| v - dir * v.dot(dir);
    let bend = flat(pole).try_normalize().or_else(|| flat(b - a).try_normalize()).unwrap_or_else(|| dir.any_orthonormal_vector());
    // the shoulder's angle off the reach (law of cosines)
    let cos = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let elbow = a + (dir * cos + bend * (1.0 - cos * cos).sqrt()) * l1;
    let hand = a + dir * d;
    // the upper arm turns onto the new elbow, then the forearm (as turned with it) onto the hand
    let q1 = Quat::from_rotation_arc((b - a) / l1, (elbow - a) / l1);
    let q2 = Quat::from_rotation_arc((q1 * (c - b)) / l2, (hand - elbow) / l2);
    // (weighted by angle: the bones keep their lengths at any weight)
    let (q1, q2) = (Quat::IDENTITY.slerp(q1, w), Quat::IDENTITY.slerp(q2, w));
    let elbow = a + q1 * (b - a);
    let hand = elbow + q2 * (q1 * (c - b));
    parts[s].rot = (q1 * parts[s].rot).normalize();
    parts[e].pos = elbow;
    parts[e].rot = (q2 * q1 * parts[e].rot).normalize();
    parts[h].pos = hand;
    parts[h].rot = (q2 * q1 * parts[h].rot).normalize();
    if right {
        parts[PEACE] = PartPose { scale: parts[PEACE].scale, ..parts[h] };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arm straight along +X from the origin: 0.1 m upper arm, 0.1 m forearm.
    fn straight() -> [PartPose; PARTS] {
        let mut p = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
        p[7].pos = Vec3::ZERO;
        p[8].pos = Vec3::new(0.1, 0.0, 0.0);
        p[9].pos = Vec3::new(0.2, 0.0, 0.0);
        p
    }

    fn lengths(p: &[PartPose; PARTS]) -> (f32, f32) {
        ((p[8].pos - p[7].pos).length(), (p[9].pos - p[8].pos).length())
    }

    #[test]
    fn a_reachable_goal_is_reached_with_the_elbow_toward_the_pole() {
        let mut p = straight();
        let goal = Vec3::new(0.12, 0.05, 0.03);
        arm(&mut p, true, goal, Vec3::new(0.0, -1.0, 0.0), 1.0);
        assert!((p[9].pos - goal).length() < 0.001, "hand at {:?}", p[9].pos);
        let (a, b) = lengths(&p);
        assert!((a - 0.1).abs() < 0.001 && (b - 0.1).abs() < 0.001, "lengths {a} {b}");
        // (the elbow below the shoulder-to-hand line)
        let line = goal.normalize();
        let off = p[8].pos - line * p[8].pos.dot(line);
        assert!(off.y < 0.0, "elbow off the line {off:?}");
    }

    #[test]
    fn an_unreachable_goal_straightens_the_arm_toward_it() {
        let mut p = straight();
        arm(&mut p, true, Vec3::new(0.0, 0.0, 0.5), Vec3::new(0.0, -1.0, 0.0), 1.0);
        assert!((p[9].pos - Vec3::new(0.0, 0.0, 0.2)).length() < 0.002, "hand at {:?}", p[9].pos);
        let (a, b) = lengths(&p);
        assert!((a - 0.1).abs() < 0.001 && (b - 0.1).abs() < 0.001);
    }

    #[test]
    fn weight_blends_by_angle() {
        let goal = Vec3::new(0.05, 0.12, 0.0);
        let mut none = straight();
        arm(&mut none, true, goal, Vec3::Y, 0.0);
        assert_eq!(none[9].pos, straight()[9].pos);
        let mut half = straight();
        arm(&mut half, true, goal, Vec3::Y, 0.5);
        let (from, to) = ((straight()[9].pos - goal).length(), (half[9].pos - goal).length());
        assert!(to > 0.005 && to < from, "half-way hand {to} from the goal, was {from}");
        let (a, b) = lengths(&half);
        assert!((a - 0.1).abs() < 0.001 && (b - 0.1).abs() < 0.001, "lengths {a} {b}");
    }

    #[test]
    fn the_hand_turns_with_the_forearm_and_the_peace_sign_follows() {
        let mut p = straight();
        arm(&mut p, true, Vec3::new(0.1, 0.1, 0.0), Vec3::new(0.0, -1.0, 0.0), 1.0);
        let along = (p[9].pos - p[8].pos).normalize();
        assert!((p[9].rot * Vec3::X).dot(along) > 0.999);
        assert!((p[8].rot * Vec3::X).dot(along) > 0.999);
        assert_eq!(p[20].pos, p[9].pos);
    }

    #[test]
    fn the_left_arm_is_its_own() {
        let mut p = straight();
        p[4].pos = Vec3::ZERO;
        p[5].pos = Vec3::new(-0.1, 0.0, 0.0);
        p[6].pos = Vec3::new(-0.2, 0.0, 0.0);
        arm(&mut p, false, Vec3::new(-0.1, 0.1, 0.0), Vec3::Y, 1.0);
        assert!((p[6].pos - Vec3::new(-0.1, 0.1, 0.0)).length() < 0.001);
        assert_eq!(p[9].pos, Vec3::new(0.2, 0.0, 0.0));
    }
}
