//! The trap kit's things as real meshes (traps.rs throws and places them; the D-pad's power-up,
//! rock.rs, rolls one): two bananas, the green and the red Koopa shell, a Bob-omb, a spring, and the
//! Rock Mushroom's boulder. They go into the gauntlets' piece (its own buffers: the chest piece is
//! full with Mario and his gear), each on a finger bone of its own (leaf bones: nothing hangs off
//! them), in their own palette (KIT_PALETTE: one flat swatch per colour in the piece's albedo).
//! Metres, part-local; each lies on its "floor" (y = 0 its bottom) unless said otherwise:
//! - a banana: lying on its side, curved along X
//! - a shell: its dome up, its rim at y = RIM
//! - the Bob-omb: its centre at y = BOMB_R, facing +Z (its eyes), the key on its back (-Z)
//! - the spring: its base on the floor, its top at SPRING_H (squash it along Y to press it)
//! - the boulder: its centre at the origin (it rolls about it)

use glam::Vec3;

use super::model::Tri;
use crate::engine_mario::{KIT_BANANA, KIT_BOMB, KIT_GREEN_SHELL, KIT_RED_SHELL, KIT_ROCK, KIT_SPRING};

/// SM64 units per metre in the mesh (as the gear's)
const PER_METRE: f32 = 400.0;

/// The kit's colours: one swatch each in its albedo (tex::kit_albedo), in this order.
pub const KIT_PALETTE: [[f32; 3]; 16] = [
    [1.0, 0.86, 0.18],   // 0 banana yellow
    [0.36, 0.23, 0.09],  // 1 banana tips, brown
    [0.12, 0.66, 0.16],  // 2 shell green
    [0.05, 0.38, 0.08],  // 3 dark green
    [0.97, 0.92, 0.72],  // 4 cream (rims, bellies)
    [1.0, 1.0, 1.0],     // 5 white
    [0.86, 0.1, 0.08],   // 6 shell red
    [0.5, 0.04, 0.03],   // 7 dark red
    [0.07, 0.07, 0.09],  // 8 Bob-omb black
    [0.96, 0.76, 0.2],   // 9 gold (the key)
    [1.0, 0.62, 0.1],    // 10 orange (feet)
    [0.72, 0.74, 0.78],  // 11 steel (the coil)
    [0.47, 0.44, 0.41],  // 12 rock
    [0.31, 0.29, 0.28],  // 13 dark rock
    [0.63, 0.6, 0.55],   // 14 light rock
    [0.82, 0.62, 0.38],  // 15 the fuse, tan
];

pub const BANANA_LEN: f32 = 0.42;
pub const SHELL_R: f32 = 0.3;
pub const SHELL_H: f32 = 0.22;
pub const RIM: f32 = 0.05;
pub const BOMB_R: f32 = 0.24;
pub const SPRING_R: f32 = 0.22;
pub const SPRING_H: f32 = 0.34;
pub const ROCK_R: f32 = 0.62;

struct Builder<'a> {
    out: &'a mut Vec<Tri>,
    part: i32,
}

impl Builder<'_> {
    fn tri(&mut self, p: [Vec3; 3], n: [Vec3; 3], color: usize) {
        // (SM64 is mirrored on X: flver.rs flips it back, as for the gear)
        let local = p.map(|v| [-v.x * PER_METRE, v.y * PER_METRE, v.z * PER_METRE]);
        let normal = n.map(|v| [-v.x, v.y, v.z]);
        self.out.push(Tri { part: self.part, local, normal, uv: [[0.0; 2]; 3], color: [KIT_PALETTE[color]; 3], world: local });
    }

    /// A grid of quads: `point(i, j)` a point and its normal, `color(i, j)` each quad's colour.
    /// Collapsed edges (a pole) are skipped. `wrap`: columns that close on themselves.
    fn grid(&mut self, cols: usize, rows: usize, wrap: bool, point: &dyn Fn(usize, usize) -> (Vec3, Vec3), color: &dyn Fn(usize, usize) -> usize) {
        let col = |i: usize| if wrap { i % cols } else { i };
        for j in 0..rows {
            for i in 0..cols {
                let c = color(i, j);
                let (a, na) = point(col(i), j);
                let (b, nb) = point(col(i + 1), j);
                let (cc, nc) = point(col(i + 1), j + 1);
                let (d, nd) = point(col(i), j + 1);
                if a.distance(b) > 1e-5 {
                    self.tri([a, b, cc], [na, nb, nc], c);
                }
                if cc.distance(d) > 1e-5 {
                    self.tri([a, cc, d], [na, nc, nd], c);
                }
            }
        }
    }

    /// A flat disc at `c` facing `n`.
    fn disc(&mut self, c: Vec3, n: Vec3, r: f32, sides: usize, color: usize) {
        let u = n.any_orthonormal_vector();
        let v = n.cross(u);
        for s in 0..sides {
            let a0 = s as f32 / sides as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / sides as f32 * std::f32::consts::TAU;
            let p0 = c + (u * a0.cos() + v * a0.sin()) * r;
            let p1 = c + (u * a1.cos() + v * a1.sin()) * r;
            self.tri([c, p0, p1], [n; 3], color);
        }
    }

    /// A closed cylinder from `a` to `b`, radius `r`.
    fn cylinder(&mut self, a: Vec3, b: Vec3, r: f32, sides: usize, color: usize, caps: bool) {
        let axis = (b - a).normalize_or(Vec3::Y);
        let u = axis.any_orthonormal_vector();
        let v = axis.cross(u);
        let ring = |i: usize, end: Vec3| {
            let t = i as f32 / sides as f32 * std::f32::consts::TAU;
            let n = u * t.cos() + v * t.sin();
            (end + n * r, n)
        };
        self.grid(sides, 1, true, &|i, j| ring(i, if j == 0 { a } else { b }), &|_, _| color);
        if caps {
            self.disc(b, axis, r, sides, color);
            self.disc(a, -axis, r, sides, color);
        }
    }
}

/// Every piece of the kit.
pub fn build() -> Vec<Tri> {
    let mut out = Vec::new();
    for k in 0..2 {
        banana(&mut Builder { out: &mut out, part: (KIT_BANANA + k) as i32 });
    }
    shell(&mut Builder { out: &mut out, part: KIT_GREEN_SHELL as i32 }, 2, 3);
    shell(&mut Builder { out: &mut out, part: KIT_RED_SHELL as i32 }, 6, 7);
    bomb(&mut Builder { out: &mut out, part: KIT_BOMB as i32 });
    spring(&mut Builder { out: &mut out, part: KIT_SPRING as i32 });
    rock(&mut Builder { out: &mut out, part: KIT_ROCK as i32 });
    out
}

/// A banana on its side: a tube bent along an arc, fat in the middle, brown at both tips.
fn banana(b: &mut Builder) {
    const SEGS: usize = 9;
    const SIDES: usize = 6;
    let span = 1.9f32;
    let bend = BANANA_LEN / span;
    // the arc's centre above it: the banana's middle touches the floor, the tips curl up
    let at = |s: f32| {
        let a = (s - 0.5) * span;
        Vec3::new(a.sin() * bend, bend * (1.0 - a.cos()), 0.0)
    };
    let fat = |s: f32| 0.012 + 0.034 * (s * std::f32::consts::PI).sin().powf(0.7);
    let point = |i: usize, j: usize| {
        let s = j as f32 / SEGS as f32;
        let c = at(s);
        let along = (at((s + 0.01).min(1.0)) - at((s - 0.01).max(0.0))).normalize_or(Vec3::X);
        let side = Vec3::Z;
        let up = side.cross(along).normalize();
        let t = i as f32 / SIDES as f32 * std::f32::consts::TAU;
        let n = up * t.cos() + side * t.sin();
        let r = fat(s);
        (c + Vec3::Y * (fat(0.5)) + n * r, n)
    };
    b.grid(SIDES, SEGS, true, &point, &|_, j| if j == 0 || j == SEGS - 1 { 1 } else { 0 });
}

/// A Koopa shell: a squashed dome in `top` with darker plates, a cream rim, a cream belly.
fn shell(b: &mut Builder, top: usize, plates: usize) {
    const LON: usize = 10;
    const LAT: usize = 4;
    let dome = |i: usize, j: usize| {
        let lon = i as f32 / LON as f32 * std::f32::consts::TAU;
        let lat = j as f32 / LAT as f32 * std::f32::consts::FRAC_PI_2;
        let (x, z) = (lon.sin() * lat.sin(), lon.cos() * lat.sin());
        let p = Vec3::new(x * SHELL_R, RIM + lat.cos() * (SHELL_H - RIM), z * SHELL_R);
        let n = Vec3::new(x / SHELL_R, lat.cos() / (SHELL_H - RIM), z / SHELL_R).normalize();
        (p, n)
    };
    // (the plates: a ring of darker ones round a lighter crown, every other one)
    b.grid(LON, LAT, true, &dome, &|i, j| if j == 2 && i % 2 == 0 { plates } else { top });
    // the rim: a cream band round its edge
    let rim = |i: usize, j: usize| {
        let lon = i as f32 / LON as f32 * std::f32::consts::TAU;
        let n = Vec3::new(lon.sin(), 0.0, lon.cos());
        (n * SHELL_R * (1.0 + 0.04 * (1 - j) as f32) + Vec3::Y * (RIM * (1 - j) as f32), n)
    };
    b.grid(LON, 1, true, &rim, &|_, _| 4);
    b.disc(Vec3::Y * 0.004, -Vec3::Y, SHELL_R * 1.04, LON, 4);
}

/// A Bob-omb: a black ball, two tall white eyes, a fuse on top, a gold key on its back, orange feet.
fn bomb(b: &mut Builder) {
    const LON: usize = 10;
    const LAT: usize = 6;
    let c = Vec3::Y * BOMB_R;
    let ball = |i: usize, j: usize| {
        let lon = i as f32 / LON as f32 * std::f32::consts::TAU;
        let lat = j as f32 / LAT as f32 * std::f32::consts::PI;
        let n = Vec3::new(lat.sin() * lon.sin(), lat.cos(), lat.sin() * lon.cos());
        (c + n * BOMB_R, n)
    };
    b.grid(LON, LAT, true, &ball, &|_, _| 8);
    // the eyes: white ovals on the front, black pupils
    for side in [-1.0f32, 1.0] {
        let n = Vec3::new(side * 0.32, 0.25, 1.0).normalize();
        let at = c + n * BOMB_R * 1.01;
        let u = Vec3::Y.cross(n).normalize();
        let v = n.cross(u);
        for s in 0..8 {
            let a0 = s as f32 / 8.0 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / 8.0 * std::f32::consts::TAU;
            let p = |a: f32| at + u * a.cos() * 0.04 + v * a.sin() * 0.07;
            b.tri([at, p(a0), p(a1)], [n; 3], 5);
        }
        b.disc(at + n * 0.004 + v * -0.01, n, 0.022, 6, 8);
    }
    // the fuse
    b.cylinder(c + Vec3::Y * BOMB_R * 0.92, c + Vec3::Y * (BOMB_R + 0.09), 0.022, 5, 15, true);
    // the key: a stem out of its back and a flat ring of two loops
    let back = c - Vec3::Z * BOMB_R;
    b.cylinder(back, back - Vec3::Z * 0.08, 0.018, 5, 9, false);
    for side in [-1.0f32, 1.0] {
        let loop_c = back - Vec3::Z * 0.1 + Vec3::X * side * 0.07;
        b.cylinder(loop_c - Vec3::Z * 0.012, loop_c + Vec3::Z * 0.012, 0.06, 8, 9, true);
    }
    // the feet: two flat orange ovals under it
    for side in [-1.0f32, 1.0] {
        let f = Vec3::new(side * 0.1, 0.02, 0.05);
        b.cylinder(f - Vec3::Y * 0.02, f + Vec3::Y * 0.025, 0.06, 7, 10, true);
    }
}

/// A spring: a steel coil between a red top and a dark base.
fn spring(b: &mut Builder) {
    const TURNS: f32 = 2.5;
    const SEGS: usize = 24;
    const SIDES: usize = 4;
    let (base, top) = (0.05, SPRING_H - 0.05);
    let coil_r = SPRING_R * 0.7;
    let tube = 0.018;
    let along = |s: f32| {
        let a = s * TURNS * std::f32::consts::TAU;
        Vec3::new(a.cos() * coil_r, base + (top - base) * s, a.sin() * coil_r)
    };
    let point = |i: usize, j: usize| {
        let s = j as f32 / SEGS as f32;
        let c = along(s);
        let t = (along((s + 0.01).min(1.0)) - along((s - 0.01).max(0.0))).normalize();
        let out = Vec3::new(c.x, 0.0, c.z).normalize_or(Vec3::X);
        let up = t.cross(out).normalize();
        let a = i as f32 / SIDES as f32 * std::f32::consts::TAU;
        let n = out * a.cos() + up * a.sin();
        (c + n * tube, n)
    };
    b.grid(SIDES, SEGS, true, &point, &|_, _| 11);
    b.cylinder(Vec3::ZERO, Vec3::Y * base, SPRING_R, 10, 13, true);
    b.cylinder(Vec3::Y * top, Vec3::Y * SPRING_H, SPRING_R, 10, 6, true);
    // (a white ring on the top plate: something to see it bounce by)
    b.disc(Vec3::Y * (SPRING_H + 0.003), Vec3::Y, SPRING_R * 0.5, 8, 5);
}

/// The Rock Mushroom's boulder: a lumpy ball in three shades of stone.
fn rock(b: &mut Builder) {
    const LON: usize = 12;
    const LAT: usize = 7;
    // (lumps: a fixed bumpiness per point, the poles kept round)
    let lump = |i: usize, j: usize| {
        if j == 0 || j == LAT {
            return 1.0;
        }
        let h = ((i * 7919 + j * 104729) % 97) as f32 / 97.0;
        0.9 + 0.16 * h
    };
    let point = |i: usize, j: usize| {
        let lon = (i % LON) as f32 / LON as f32 * std::f32::consts::TAU;
        let lat = j as f32 / LAT as f32 * std::f32::consts::PI;
        let n = Vec3::new(lat.sin() * lon.sin(), lat.cos(), lat.sin() * lon.cos());
        (n * ROCK_R * lump(i % LON, j), n)
    };
    b.grid(LON, LAT, true, &point, &|i, j| match (i * 3 + j * 5) % 7 {
        0 | 3 => 13,
        5 => 14,
        _ => 12,
    });
}

#[cfg(test)]
mod tests {
    /// The kit's own vertex count (as the FLVER writer dedups them) and triangles: the gauntlets'
    /// buffers are checked at build time, this only keeps it from growing by accident.
    #[test]
    fn size() {
        let tris = super::build();
        let mut keys: Vec<(i32, [i64; 3], [i64; 3])> = Vec::new();
        for t in &tris {
            for k in 0..3 {
                let key = (t.part, t.local[k].map(|v| (v * 100.0).round() as i64), t.color[k].map(|v| (v * 255.0).round() as i64));
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
        println!("kit: {} vertices, {} triangles", keys.len(), tris.len());
        assert!(keys.len() <= 1100 && tris.len() <= 1600);
    }
}
