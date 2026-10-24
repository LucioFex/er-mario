//! The overworld's small map tiles (m60_XX_ZZ_00 in the base game, m61 in Shadow of the Erdtree):
//! 256 m squares, the tile's X in the block id's `block` byte and its Z in `region`. A spot is kept
//! as its place in its block (physics position less the world origin shift, coins.rs): seen from
//! another tile, that place is off by the tiles between them, along axes and signs the mod learns
//! the first time the player walks from one tile into the next (`learn`), and keeps.

use glam::Vec3;

/// a small overworld tile's side (m)
pub const TILE: f32 = 256.0;

/// (area, x, z) of an overworld small tile (area 60 or 61, index 0).
pub fn tile(block: i32) -> Option<(u8, i32, i32)> {
    let b = eldenring::cs::BlockId::from(block);
    (block != -1 && matches!(b.area(), 60 | 61) && b.index() == 0).then(|| (b.area(), b.block() as i32, b.region() as i32))
}

/// A jump in the place this close to a tile's side (m), or a walk this short, is read as one.
const SLACK: f32 = 8.0;

/// What one tile step along the block (x) or region (z) number does to the place in the block.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Step {
    /// it jumps by minus the tile's side times `sign` per step up on this axis (0 x, 2 z): the
    /// world is the place plus the tile's number times the side times `sign`
    Jump { axis: usize, sign: f32 },
    /// it goes on as it was: the place is the same frame in every tile
    Continuous,
}

/// The two steps, once learnt.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Axes {
    pub block: Option<Step>,
    pub region: Option<Step>,
}

/// A spot's place `local` in block `from`, in block `to`'s place frame: None if they aren't both
/// small tiles of the same area, or a step needed isn't known yet.
pub fn between(from: i32, local: Vec3, to: i32, axes: &Axes) -> Option<Vec3> {
    if from == to {
        return Some(local);
    }
    let ((area_a, xa, za), (area_b, xb, zb)) = (tile(from)?, tile(to)?);
    if area_a != area_b {
        return None;
    }
    let mut out = local;
    for (steps, step) in [(xa - xb, axes.block), (za - zb, axes.region)] {
        if steps == 0 {
            continue;
        }
        if let Step::Jump { axis, sign } = step? {
            out[axis] += steps as f32 * TILE * sign;
        }
    }
    Some(out)
}

/// One frame's crossing from `prev_block` to `block` (places in each): what it says about a step
/// (true: the block step, false: the region step). None when it can't tell (not two neighbouring
/// small tiles, or a jump that isn't a tile's: a warp, a load).
pub fn learn(prev_block: i32, prev_local: Vec3, block: i32, local: Vec3) -> Option<(bool, Step)> {
    let ((area_a, xa, za), (area_b, xb, zb)) = (tile(prev_block)?, tile(block)?);
    let (dx, dz) = (xb - xa, zb - za);
    if area_a != area_b || dx.abs() + dz.abs() != 1 {
        return None;
    }
    let (along_block, step) = if dx != 0 { (true, dx) } else { (false, dz) };
    let d = local - prev_local;
    if d.length() < SLACK {
        // (places inside a tile's square are tile-local: no jump seen means the block id and the
        // place came on different frames, not one frame for every tile)
        let inside = |p: Vec3| p.x.abs() <= TILE * 0.5 + SLACK && p.z.abs() <= TILE * 0.5 + SLACK;
        if inside(prev_local) && inside(local) {
            return None;
        }
        return Some((along_block, Step::Continuous));
    }
    // one axis jumped by a tile's side, the rest moved no more than a walk
    let axis = if d.x.abs() > d.z.abs() { 0 } else { 2 };
    let rest = Vec3::new(if axis == 0 { 0.0 } else { d.x }, d.y, if axis == 2 { 0.0 } else { d.z });
    if (d[axis].abs() - TILE).abs() > SLACK || rest.length() > SLACK {
        return None;
    }
    let sign = -d[axis].signum() * step as f32;
    Some((along_block, Step::Jump { axis, sign }))
}

/// The axes as kept in the mod folder's file, and back.
pub fn format(a: &Axes) -> String {
    let one = |s: Option<Step>| match s {
        None => "unknown".to_string(),
        Some(Step::Continuous) => "continuous".to_string(),
        Some(Step::Jump { axis, sign }) => format!("{}{}", if sign < 0.0 { "-" } else { "+" }, if axis == 0 { "x" } else { "z" }),
    };
    format!("block {}
region {}
", one(a.block), one(a.region))
}

pub fn parse(s: &str) -> Axes {
    let one = |v: &str| match v.trim() {
        "continuous" => Some(Step::Continuous),
        "+x" => Some(Step::Jump { axis: 0, sign: 1.0 }),
        "-x" => Some(Step::Jump { axis: 0, sign: -1.0 }),
        "+z" => Some(Step::Jump { axis: 2, sign: 1.0 }),
        "-z" => Some(Step::Jump { axis: 2, sign: -1.0 }),
        _ => None,
    };
    let mut a = Axes::default();
    for line in s.lines() {
        match line.split_once(' ') {
            Some(("block", v)) => a.block = one(v),
            Some(("region", v)) => a.region = one(v),
            _ => {}
        }
    }
    a
}

/// A jump of the player's physics place by more than this (m) that the world origin's shift doesn't
/// account for is a warp (a grace, a fast travel), not the floating origin moving.
pub const WARP_FAR: f32 = 5.0;

/// Whether the player's physics position jumping by `jump` while the world origin (chunk_position)
/// moved by `origin_moved` (None: it's not known, a map loading) is a warp: the floating origin
/// moves both alike, a warp moves him through the world.
pub fn warped(jump: Vec3, origin_moved: Option<Vec3>) -> bool {
    origin_moved.is_none_or(|o| (jump - o).length() > WARP_FAR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eldenring::cs::BlockId;

    #[test]
    fn a_warp_is_a_jump_the_origin_doesnt_account_for() {
        // the floating origin's steps: he and the origin move alike
        assert!(!warped(Vec3::new(32.0, -0.003, 0.0), Some(Vec3::new(32.0, 0.0, 0.0))));
        assert!(!warped(Vec3::new(-32.0, 8.0, 24.0), Some(Vec3::new(-32.0, 8.0, 24.0))));
        // a fast travel: the place jumps, the origin with it only in part, or not at all
        assert!(warped(Vec3::new(141.1, 817.8, 87.8), Some(Vec3::ZERO)));
        assert!(warped(Vec3::new(-120.0, -791.9, -72.0), Some(Vec3::new(-96.0, -768.0, -64.0))));
        // (a map loading, its origin not there yet)
        assert!(warped(Vec3::new(3.0, 0.0, 3.0), None));
        assert!(!warped(Vec3::new(3.0, 0.0, 3.0), Some(Vec3::new(3.0, 0.0, 3.0))));
    }

    fn id(area: u8, x: u8, z: u8, index: u8) -> i32 {
        i32::from(BlockId::from_parts(area, x, z, index))
    }

    #[test]
    fn small_overworld_tiles_only() {
        assert_eq!(tile(id(60, 42, 36, 0)), Some((60, 42, 36)));
        assert_eq!(tile(id(61, 45, 40, 0)), Some((61, 45, 40)));
        assert_eq!(tile(id(60, 21, 18, 1)), None);
        assert_eq!(tile(id(11, 0, 0, 0)), None);
        assert_eq!(tile(-1), None);
    }

    #[test]
    fn a_walk_across_a_tile_edge_teaches_the_step() {
        // block 42 -> 43, the place in the block 256 m back on x, plus the walk's 0.4 m
        let got = learn(id(60, 42, 36, 0), Vec3::new(127.8, 10.0, 3.0), id(60, 43, 36, 0), Vec3::new(-127.8, 10.0, 3.1));
        assert_eq!(got, Some((true, Step::Jump { axis: 0, sign: 1.0 })));
        // region 36 -> 37 with the place 256 m up on z: the sign is the other way
        let got = learn(id(60, 42, 36, 0), Vec3::new(5.0, 0.0, -127.9), id(60, 42, 37, 0), Vec3::new(5.0, 0.0, 128.1));
        assert_eq!(got, Some((false, Step::Jump { axis: 2, sign: -1.0 })));
        // stepping down a tile says the same about the step up
        let got = learn(id(60, 43, 36, 0), Vec3::new(-127.8, 0.0, 0.0), id(60, 42, 36, 0), Vec3::new(127.9, 0.0, 0.0));
        assert_eq!(got, Some((true, Step::Jump { axis: 0, sign: 1.0 })));
    }

    #[test]
    fn a_place_that_goes_on_is_continuous() {
        let got = learn(id(60, 42, 36, 0), Vec3::new(10883.0, 0.0, 9300.0), id(60, 43, 36, 0), Vec3::new(10883.4, 0.0, 9300.0));
        assert_eq!(got, Some((true, Step::Continuous)));
    }

    #[test]
    fn a_place_inside_a_tile_never_reads_as_continuous() {
        // (the block id and the place updated on different frames: no jump seen, but the places
        // are tile-local, so it can't be the same frame everywhere)
        assert_eq!(learn(id(60, 42, 36, 0), Vec3::new(127.8, 0.0, 3.0), id(60, 43, 36, 0), Vec3::new(128.1, 0.0, 3.0)), None);
    }

    #[test]
    fn warps_and_far_tiles_teach_nothing() {
        // a 40 m jump isn't a tile's
        assert_eq!(learn(id(60, 42, 36, 0), Vec3::ZERO, id(60, 43, 36, 0), Vec3::new(40.0, 0.0, 0.0)), None);
        // not neighbours, or diagonal, or another area
        assert_eq!(learn(id(60, 42, 36, 0), Vec3::ZERO, id(60, 44, 36, 0), Vec3::new(-512.0, 0.0, 0.0)), None);
        assert_eq!(learn(id(60, 42, 36, 0), Vec3::ZERO, id(60, 43, 37, 0), Vec3::new(-256.0, 0.0, 256.0)), None);
        assert_eq!(learn(id(60, 42, 36, 0), Vec3::ZERO, id(61, 43, 36, 0), Vec3::new(-256.0, 0.0, 0.0)), None);
        assert_eq!(learn(id(60, 42, 36, 0), Vec3::ZERO, id(60, 42, 36, 0), Vec3::new(0.3, 0.0, 0.0)), None);
    }

    #[test]
    fn a_spot_seen_from_another_tile() {
        let axes = Axes { block: Some(Step::Jump { axis: 0, sign: 1.0 }), region: Some(Step::Jump { axis: 2, sign: -1.0 }) };
        let spot = Vec3::new(-118.0, 5.0, 20.0);
        // 10 m into tile 42 from its edge with 43 (x 118) is x -138 in tile 43
        assert_eq!(between(id(60, 42, 36, 0), Vec3::new(118.0, 5.0, 20.0), id(60, 43, 36, 0), &axes), Some(Vec3::new(-138.0, 5.0, 20.0)));
        // two tiles up in region and one down in block
        assert_eq!(between(id(60, 43, 36, 0), spot, id(60, 42, 38, 0), &axes), Some(Vec3::new(-118.0 + 256.0, 5.0, 20.0 + 512.0)));
        // the same tile: as it is
        assert_eq!(between(id(60, 43, 36, 0), spot, id(60, 43, 36, 0), &Axes::default()), Some(spot));
        // continuous: as it is from any tile
        let flat = Axes { block: Some(Step::Continuous), region: Some(Step::Continuous) };
        assert_eq!(between(id(60, 43, 36, 0), spot, id(60, 40, 30, 0), &flat), Some(spot));
        // a step not learnt yet, another area, a dungeon: unknown
        let half = Axes { block: Some(Step::Jump { axis: 0, sign: 1.0 }), region: None };
        assert_eq!(between(id(60, 43, 36, 0), spot, id(60, 43, 37, 0), &half), None);
        assert_eq!(between(id(60, 43, 36, 0), spot, id(60, 44, 36, 0), &half), Some(Vec3::new(-118.0 - 256.0, 5.0, 20.0)));
        assert_eq!(between(id(60, 43, 36, 0), spot, id(61, 43, 36, 0), &axes), None);
        assert_eq!(between(id(60, 43, 36, 0), spot, id(31, 1, 0, 0), &axes), None);
    }

    #[test]
    fn the_axes_file_round_trips() {
        for a in [
            Axes::default(),
            Axes { block: Some(Step::Jump { axis: 0, sign: 1.0 }), region: Some(Step::Jump { axis: 2, sign: -1.0 }) },
            Axes { block: Some(Step::Continuous), region: None },
        ] {
            assert_eq!(parse(&format(&a)), a);
        }
        assert_eq!(parse("rubbish"), Axes::default());
    }
}
