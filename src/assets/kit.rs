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
//! - the Star Sword: its grip at the origin, the blade up along +Y (its flat facing X, its edges
//!   along Z), the pommel below

use glam::Vec3;

use super::model::Tri;
use crate::engine_mario::{KIT_BANANA, KIT_BOMB, KIT_GREEN_SHELL, KIT_RED_SHELL, KIT_ROCK, KIT_SPRING, KIT_SWORD};

/// SM64 units per metre in the mesh (as the gear's)
const PER_METRE: f32 = 400.0;

/// The kit albedo's swatches: KIT_GRID x KIT_GRID cells (tex::kit_albedo, flver::kit_uv).
pub const KIT_GRID: usize = 8;

/// The kit's colours: one swatch each in its albedo (tex::kit_albedo), in this order.
pub const KIT_PALETTE: [[f32; 3]; 19] = [
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
    [0.9, 0.92, 0.97],   // 16 the sword's blade, silver-white
    [0.62, 0.84, 1.0],   // 17 its edges, light blue
    [0.78, 0.81, 0.88],  // 18 its ridge, a shade darker
];
const _: () = assert!(KIT_PALETTE.len() <= KIT_GRID * KIT_GRID);

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
    sword(&mut Builder { out: &mut out, part: KIT_SWORD as i32 });
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

/// the Star Sword: where the blade starts and its tip (m up its part's Y), its width at the guard
/// and its thickness
pub const SWORD_BASE: f32 = 0.075;
pub const SWORD_TIP: f32 = 0.89;
const SWORD_W: f32 = 0.07;
const SWORD_T: f32 = 0.014;

impl Builder<'_> {
    /// A triangle whichever way its points go: turned so it faces `out` (flat shaded).
    fn face(&mut self, p: [Vec3; 3], out: Vec3, color: usize) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        if n.length_squared() < 1e-12 {
            return;
        }
        let (p, n) = if n.dot(out) < 0.0 { ([p[0], p[2], p[1]], -n.normalize()) } else { (p, n.normalize()) };
        self.tri(p, [n; 3], color);
    }

    /// A flat convex outline (points in order) pushed `half` either way along `axis`: both faces
    /// and its rim.
    fn slab(&mut self, outline: &[Vec3], axis: Vec3, half: f32, color: usize) {
        let c = outline.iter().copied().sum::<Vec3>() / outline.len() as f32;
        let n = outline.len();
        for side in [1.0f32, -1.0] {
            let off = axis * half * side;
            for k in 0..n {
                self.face([c + off, outline[k] + off, outline[(k + 1) % n] + off], axis * side, color);
            }
        }
        for k in 0..n {
            let (a, b) = (outline[k], outline[(k + 1) % n]);
            let out = ((a + b) * 0.5 - c).normalize_or(axis);
            self.face([a + axis * half, b + axis * half, b - axis * half], out, color);
            self.face([a + axis * half, b - axis * half, a - axis * half], out, color);
        }
    }

    /// A five-point star in the plane across `axis` round `c` (radius `r`, its first point toward
    /// `up`), `half` thick.
    fn star(&mut self, c: Vec3, axis: Vec3, up: Vec3, r: f32, half: f32, color: usize) {
        let side = axis.cross(up).normalize_or(Vec3::Z);
        let pts: Vec<Vec3> = (0..10)
            .map(|k| {
                let a = k as f32 / 10.0 * std::f32::consts::TAU;
                let rr = if k % 2 == 0 { r } else { r * 0.45 };
                c + (up * a.cos() + side * a.sin()) * rr
            })
            .collect();
        // (a star isn't convex: a fan from its middle on each face, and its rim)
        for s in [1.0f32, -1.0] {
            let off = axis * half * s;
            for k in 0..10 {
                self.face([c + off, pts[k] + off, pts[(k + 1) % 10] + off], axis * s, color);
            }
        }
        for k in 0..10 {
            let (a, b) = (pts[k], pts[(k + 1) % 10]);
            let out = ((a + b) * 0.5 - c).normalize_or(up);
            self.face([a + axis * half, b + axis * half, b - axis * half], out, color);
            self.face([a + axis * half, b - axis * half, a - axis * half], out, color);
        }
    }
}

/// The Star Sword (sword.rs): a silver-white blade with faint light-blue edges and a darker ridge,
/// a gold Power Star inlaid on both faces by the guard, two little white Wing Cap wings for its
/// guard (the Master Sword's), a red grip ringed in gold, a yellow star for its pommel.
fn sword(b: &mut Builder) {
    // the blade: a flat hexagon across, tapering up to a point
    let slices: [(f32, f32); 6] = [(SWORD_BASE, 1.0), (0.25, 0.97), (0.55, 0.89), (0.74, 0.83), (0.8, 0.78), (SWORD_TIP, 0.0)];
    let section = |w: f32, t: f32| -> [Vec3; 6] {
        let (w, t) = (SWORD_W * 0.5 * w, SWORD_T * 0.5 * t);
        [Vec3::new(0.0, 0.0, w), Vec3::new(t, 0.0, w * 0.45), Vec3::new(t, 0.0, -w * 0.45), Vec3::new(0.0, 0.0, -w), Vec3::new(-t, 0.0, -w * 0.45), Vec3::new(-t, 0.0, w * 0.45)]
    };
    // (the faces' colours round the section: edge bevels light blue, the flats silver)
    const FACES: [usize; 6] = [17, 16, 17, 17, 16, 17];
    for j in 0..slices.len() - 1 {
        let ((ya, wa), (yb, wb)) = (slices[j], slices[j + 1]);
        let (sa, sb) = (section(wa, wa.max(0.6)), section(wb, if wb > 0.0 { wb.max(0.6) } else { 0.0 }));
        for i in 0..6 {
            let k = (i + 1) % 6;
            let (a0, a1) = (sa[i] + Vec3::Y * ya, sa[k] + Vec3::Y * ya);
            let (b0, b1) = (sb[i] + Vec3::Y * yb, sb[k] + Vec3::Y * yb);
            let out = ((sa[i] + sa[k]) * 0.5).with_y(0.0).normalize_or(Vec3::X);
            b.face([a0, a1, b1], out, FACES[i]);
            b.face([a0, b1, b0], out, FACES[i]);
        }
    }
    // its ridge: a thin darker strip up the middle of each flat (a hair proud of it)
    for s in [1.0f32, -1.0] {
        let x = SWORD_T * 0.5 * s + 0.0008 * s;
        let pts = [Vec3::new(x, 0.13, 0.004), Vec3::new(x, 0.74, 0.003), Vec3::new(x, 0.74, -0.003), Vec3::new(x, 0.13, -0.004)];
        b.face([pts[0], pts[1], pts[2]], Vec3::X * s, 18);
        b.face([pts[0], pts[2], pts[3]], Vec3::X * s, 18);
    }
    // the Power Star inlaid on both faces, just above the guard
    for s in [1.0f32, -1.0] {
        b.star(Vec3::new((SWORD_T * 0.5 + 0.002) * s, 0.12, 0.0), Vec3::X, Vec3::Y, 0.022, 0.0015, 9);
    }
    // the guard: a gold block, and a Wing Cap wing each side swept up
    b.slab(&[Vec3::new(0.0, 0.04, -0.035), Vec3::new(0.0, 0.04, 0.035), Vec3::new(0.0, 0.08, 0.035), Vec3::new(0.0, 0.08, -0.035)], Vec3::X, 0.02, 9);
    for s in [1.0f32, -1.0] {
        let wing = [(0.03, 0.035), (0.03, 0.09), (0.075, 0.125), (0.12, 0.155), (0.11, 0.11), (0.07, 0.06)];
        let outline: Vec<Vec3> = wing.iter().map(|&(z, y)| Vec3::new(0.0, y, z * s)).collect();
        b.slab(&outline, Vec3::X, 0.008, 5);
    }
    // the grip, red, ringed in gold; the pommel a yellow star
    b.cylinder(Vec3::new(0.0, -0.105, 0.0), Vec3::new(0.0, 0.04, 0.0), 0.016, 8, 6, false);
    for y in [-0.095, 0.022] {
        b.cylinder(Vec3::new(0.0, y - 0.007, 0.0), Vec3::new(0.0, y + 0.007, 0.0), 0.0195, 8, 9, true);
    }
    b.star(Vec3::new(0.0, -0.14, 0.0), Vec3::X, Vec3::NEG_Y, 0.038, 0.01, 0);
}

#[cfg(test)]
mod tests {
    /// The kit's own vertex count (as the FLVER writer dedups them) and triangles: the gauntlets'
    /// buffers are checked at build time, this only keeps it from growing by accident.
    /// The Star Sword along its part's +Y (the grip at the origin, the tip ~0.9 m up), its blade's
    /// flat facing X, its wings out along Z; every normal a unit one.
    #[test]
    fn the_sword() {
        let tris: Vec<_> = super::build().into_iter().filter(|t| t.part == crate::engine_mario::KIT_SWORD as i32).collect();
        assert!(!tris.is_empty());
        let pts: Vec<[f32; 3]> = tris.iter().flat_map(|t| t.local).map(|p| [-p[0] / super::PER_METRE, p[1] / super::PER_METRE, p[2] / super::PER_METRE]).collect();
        let lo_hi = |k: usize| (pts.iter().map(|p| p[k]).fold(f32::MAX, f32::min), pts.iter().map(|p| p[k]).fold(f32::MIN, f32::max));
        let (y0, y1) = lo_hi(1);
        assert!(y0 > -0.2 && y0 < -0.1 && y1 > 0.85 && y1 < 0.95, "from {y0} to {y1}");
        let (z0, z1) = lo_hi(2);
        assert!(z1 - z0 < 0.3 && z1 > 0.1 && z0 < -0.1, "wings from {z0} to {z1}");
        let (x0, x1) = lo_hi(0);
        assert!(x1 - x0 < 0.06, "{x0}..{x1} thick");
        for t in &tris {
            for n in t.normal {
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((l - 1.0).abs() < 1e-3, "normal {n:?}");
            }
        }
    }

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
        assert!(keys.len() <= 1500 && tris.len() <= 2200);
    }
}
