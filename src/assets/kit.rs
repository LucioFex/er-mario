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
pub(crate) const PER_METRE: f32 = 400.0;

/// The kit albedo's swatches: KIT_GRID x KIT_GRID cells (tex::kit_albedo, flver::kit_uv).
pub const KIT_GRID: usize = 8;

/// The kit's colours: one swatch each in its albedo (tex::kit_albedo), in this order.
pub const KIT_PALETTE: [[f32; 3]; 27] = [
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
    // the kart (kartmesh.rs, in the legs piece: the same albedo)
    [0.84, 0.07, 0.06],  // 19 the kart's red
    [0.42, 0.03, 0.03],  // 20 its dark red
    [0.05, 0.05, 0.06],  // 21 tyre black
    [0.2, 0.2, 0.22],    // 22 the tread's grey
    [0.12, 0.22, 0.6],   // 23 the seat's blue
    [0.3, 0.3, 0.33],    // 24 the engine's grey
    [0.86, 0.87, 0.9],   // 25 chrome
    [0.1, 0.1, 0.12],    // 26 near black (the column, the bumpers' rubber)
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

pub(crate) struct Builder<'a> {
    pub out: &'a mut Vec<Tri>,
    pub part: i32,
}

impl Builder<'_> {
    pub(crate) fn tri(&mut self, p: [Vec3; 3], n: [Vec3; 3], color: usize) {
        // (SM64 is mirrored on X: flver.rs flips it back, as for the gear)
        let local = p.map(|v| [-v.x * PER_METRE, v.y * PER_METRE, v.z * PER_METRE]);
        let normal = n.map(|v| [-v.x, v.y, v.z]);
        self.out.push(Tri { part: self.part, local, normal, uv: [[0.0; 2]; 3], color: [KIT_PALETTE[color]; 3], world: local });
    }

    /// A grid of quads: `point(i, j)` a point and its normal, `color(i, j)` each quad's colour.
    /// Collapsed edges (a pole) are skipped. `wrap`: columns that close on themselves.
    pub(crate) fn grid(&mut self, cols: usize, rows: usize, wrap: bool, point: &dyn Fn(usize, usize) -> (Vec3, Vec3), color: &dyn Fn(usize, usize) -> usize) {
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
    pub(crate) fn disc(&mut self, c: Vec3, n: Vec3, r: f32, sides: usize, color: usize) {
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
    pub(crate) fn cylinder(&mut self, a: Vec3, b: Vec3, r: f32, sides: usize, color: usize, caps: bool) {
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

/// the Star Sword: where the blade starts and its tip (m up its part's Y), its width and its
/// thickness. A greatsword (the user: "a toothpick" at 0.9 m by 7 cm): for a 1.25 m Mario, a blade
/// longer than he is tall to his shoulders (Rebellion's silhouette), broad, a two-handed grip.
pub const SWORD_BASE: f32 = 0.12;
pub const SWORD_TIP: f32 = 1.80;
const SWORD_W: f32 = 0.22;
const SWORD_T: f32 = 0.024;

impl Builder<'_> {
    /// A triangle whichever way its points go: turned so it faces `out` (flat shaded).
    pub(crate) fn face(&mut self, p: [Vec3; 3], out: Vec3, color: usize) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        if n.length_squared() < 1e-12 {
            return;
        }
        let (p, n) = if n.dot(out) < 0.0 { ([p[0], p[2], p[1]], -n.normalize()) } else { (p, n.normalize()) };
        self.tri(p, [n; 3], color);
    }

    /// A flat convex outline (points in order) pushed `half` either way along `axis`: both faces
    /// and its rim.
    pub(crate) fn slab(&mut self, outline: &[Vec3], axis: Vec3, half: f32, color: usize) {
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
}

impl Builder<'_> {
    /// A flat convex outline (points in order) as one face toward `out`: a fan, no thickness.
    fn plate(&mut self, pts: &[Vec3], out: Vec3, color: usize) {
        for k in 1..pts.len() - 1 {
            self.face([pts[0], pts[k], pts[k + 1]], out, color);
        }
    }

    /// A four-sided tube along `path` (point, radius) in the YZ plane (rings across X, so they
    /// never twist); `color(k)` its segment k's; a radius of 0 closes it to a point.
    fn tube(&mut self, path: &[(Vec3, f32)], color: &dyn Fn(usize) -> usize) {
        let ring = |k: usize| -> [Vec3; 4] {
            let (c, r) = path[k];
            let d = (path[(k + 1).min(path.len() - 1)].0 - path[k.saturating_sub(1)].0).normalize();
            let v = d.cross(Vec3::X).normalize();
            [c + Vec3::X * r, c + v * r, c - Vec3::X * r, c - v * r]
        };
        for k in 0..path.len() - 1 {
            let (ra, rb) = (ring(k), ring(k + 1));
            let mid = (path[k].0 + path[k + 1].0) * 0.5;
            for i in 0..4 {
                let j = (i + 1) % 4;
                let out = (ra[i] + ra[j] + rb[i] + rb[j]) * 0.25 - mid;
                self.face([ra[i], ra[j], rb[j]], out, color(k));
                self.face([ra[i], rb[j], rb[i]], out, color(k));
            }
        }
    }

    /// A little cone from `base` (radius `r`, `sides` round) to `tip`.
    fn spike(&mut self, base: Vec3, tip: Vec3, r: f32, sides: usize, color: usize) {
        let d = (tip - base).normalize();
        let u = d.any_orthonormal_vector();
        let v = d.cross(u);
        let at = |k: usize| {
            let a = k as f32 / sides as f32 * std::f32::consts::TAU;
            base + (u * a.cos() + v * a.sin()) * r
        };
        for k in 0..sides {
            let (p, q) = (at(k), at(k + 1));
            self.face([p, q, tip], (p + q) * 0.5 - base, color);
        }
    }
}

/// The Star Sword (sword.rs), "Dry Bowser" in Rebellion's shape: a straight steel blade with
/// light-blue edges, a dark-red fuller with five gold diamonds, two notches in each edge at its
/// base; Dry Bowser's skull for the guard (horns with yellow tips, black sockets with a red glint,
/// teeth over a black mouth) and two gold ribs sweeping down from it; a red two-handed grip with
/// three yellow rings; Bowser's spiked bracelet for the pommel (one spike down as its point).
fn sword(b: &mut Builder) {
    // the blade: parallel edges, an angular point over its last 12%. Slices (up, width share):
    // the notches are the dips to 0.82; the last one is the tip
    let slices: [(f32, f32); 9] = [(SWORD_BASE, 1.0), (0.28, 1.0), (0.32, 0.82), (0.36, 1.0), (0.44, 1.0), (0.48, 0.82), (0.52, 1.0), (SWORD_TIP - 0.2, 1.0), (SWORD_TIP, 0.0)];
    let section = |w: f32, t: f32| -> [Vec3; 6] {
        let (w, t) = (SWORD_W * 0.5 * w, SWORD_T * 0.5 * t);
        [Vec3::new(0.0, 0.0, w), Vec3::new(t, 0.0, w * 0.7), Vec3::new(t, 0.0, -w * 0.7), Vec3::new(0.0, 0.0, -w), Vec3::new(-t, 0.0, -w * 0.7), Vec3::new(-t, 0.0, w * 0.7)]
    };
    // (the faces' colours round the section: edge bevels light blue, the flats steel)
    const FACES: [usize; 6] = [17, 16, 17, 17, 16, 17];
    for j in 0..slices.len() - 1 {
        let ((ya, wa), (yb, wb)) = (slices[j], slices[j + 1]);
        let (sa, sb) = (section(wa, 1.0), section(wb, if wb > 0.0 { 1.0 } else { 0.0 }));
        for i in 0..6 {
            let k = (i + 1) % 6;
            let (a0, a1) = (sa[i] + Vec3::Y * ya, sa[k] + Vec3::Y * ya);
            let (b0, b1) = (sb[i] + Vec3::Y * yb, sb[k] + Vec3::Y * yb);
            let out = ((sa[i] + sa[k]) * 0.5).with_y(0.0).normalize_or(Vec3::X);
            b.face([a0, a1, b1], out, FACES[i]);
            b.face([a0, b1, b0], out, FACES[i]);
        }
    }
    // the fuller: a dark-red lozenge up each flat from 8% to 75% (a hair proud), five gold diamonds
    // inlaid in a row on it
    let len = SWORD_TIP - SWORD_BASE;
    let (lo, hi) = (SWORD_BASE + len * 0.08, SWORD_BASE + len * 0.75);
    for s in [1.0f32, -1.0] {
        let x = (SWORD_T * 0.5 + 0.0015) * s;
        let p = |z: f32, y: f32| Vec3::new(x, y, z);
        b.plate(&[p(0.0, lo), p(0.02, lo + 0.04), p(0.02, hi - 0.04), p(0.0, hi), p(-0.02, hi - 0.04), p(-0.02, lo + 0.04)], Vec3::X * s, 7);
        let xd = x + 0.0012 * s;
        for k in 0..5 {
            let (y, h, w) = (lo + 0.26 + k as f32 * 0.17, 0.045, 0.016);
            let q = |z: f32, y: f32| Vec3::new(xd, y, z);
            b.plate(&[q(0.0, y - h), q(w, y), q(0.0, y + h), q(-w, y)], Vec3::X * s, 9);
        }
    }
    // the skull: a dome tapering to the jaw, a brow ridge, a snout, both faces alike
    let sk = |pts: &[(f32, f32)]| -> Vec<Vec3> { pts.iter().map(|&(z, y)| Vec3::new(0.0, y, z)).collect() };
    b.slab(&sk(&[(-0.07, 0.04), (0.07, 0.04), (0.10, 0.08), (0.12, 0.15), (0.09, 0.22), (0.04, 0.24), (-0.04, 0.24), (-0.09, 0.22), (-0.12, 0.15), (-0.10, 0.08)]), Vec3::X, 0.05, 4);
    b.slab(&sk(&[(-0.105, 0.175), (0.105, 0.175), (0.095, 0.20), (-0.095, 0.20)]), Vec3::X, 0.057, 4);
    b.slab(&sk(&[(-0.032, 0.085), (0.032, 0.085), (0.032, 0.125), (-0.032, 0.125)]), Vec3::X, 0.062, 4);
    for s in [1.0f32, -1.0] {
        let p = |z: f32, y: f32, dx: f32| Vec3::new((0.0515 + dx) * s, y, z);
        // black sockets, scowling (their inner tops lower), each with a red glint
        for ez in [0.055f32, -0.055] {
            let o = ez.signum();
            b.plate(&[p(ez + o * 0.024, 0.172, 0.0), p(ez - o * 0.024, 0.162, 0.0), p(ez - o * 0.024, 0.13, 0.0), p(ez + o * 0.024, 0.13, 0.0)], Vec3::X * s, 8);
            b.plate(&[p(ez - 0.007, 0.141, 0.001), p(ez + 0.007, 0.141, 0.001), p(ez + 0.007, 0.155, 0.001), p(ez - 0.007, 0.155, 0.001)], Vec3::X * s, 6);
        }
        // the mouth: black, a row of cream teeth hanging over it
        b.plate(&[p(-0.07, 0.045, 0.0), p(0.07, 0.045, 0.0), p(0.07, 0.075, 0.0), p(-0.07, 0.075, 0.0)], Vec3::X * s, 8);
        for k in 0..6 {
            let z0 = -0.066 + k as f32 * 0.022;
            b.face([p(z0, 0.075, 0.001), p(z0 + 0.018, 0.075, 0.001), p(z0 + 0.009, 0.052, 0.001)], Vec3::X * s, 4);
        }
        // a horn sweeping up and out from the skull's top side (bone, yellow over its upper two thirds)
        let v = |z: f32, y: f32, r: f32| (Vec3::new(0.0, y, z * s), r);
        b.tube(&[v(0.085, 0.20, 0.035), v(0.14, 0.26, 0.026), v(0.175, 0.33, 0.016), v(0.2, 0.41, 0.0)], &|k| if k == 0 { 4 } else { 0 });
        // a gold rib sweeping down and out from the skull's side (Rebellion's guard arm)
        b.tube(&[v(0.105, 0.13, 0.016), v(0.16, 0.09, 0.013), v(0.20, 0.02, 0.01), v(0.215, -0.05, 0.006)], &|_| 9);
    }
    // the grip (both hands), Mario red, three yellow rings
    b.cylinder(Vec3::new(0.0, -0.21, 0.0), Vec3::new(0.0, 0.06, 0.0), 0.02, 8, 6, false);
    for y in [-0.17, -0.07, 0.03] {
        b.cylinder(Vec3::new(0.0, y - 0.008, 0.0), Vec3::new(0.0, y + 0.008, 0.0), 0.026, 8, 0, true);
    }
    // the pommel: Bowser's bracelet, a black band, six cream spikes round it, one pointing down
    b.cylinder(Vec3::new(0.0, -0.26, 0.0), Vec3::new(0.0, -0.21, 0.0), 0.03, 8, 8, true);
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        b.spike(d * 0.03 + Vec3::Y * -0.235, d * 0.062 + Vec3::Y * -0.235, 0.011, 3, 4);
    }
    b.spike(Vec3::new(0.0, -0.26, 0.0), Vec3::new(0.0, -0.30, 0.0), 0.02, 4, 4);
}

#[cfg(test)]
mod tests {
    /// The kit's own vertex count (as the FLVER writer dedups them) and triangles: the gauntlets'
    /// buffers are checked at build time, this only keeps it from growing by accident.
    /// The Dry Bowser Rebellion sword along its part's +Y (the grip at the origin, the tip 1.80 m up:
    /// a greatsword for a 1.25 m Mario, its grip long enough for both hands), its blade's flat
    /// facing X, its guard out along Z; every piece in its colour, every normal a unit one.
    #[test]
    fn the_sword() {
        let tris: Vec<_> = super::build().into_iter().filter(|t| t.part == crate::engine_mario::KIT_SWORD as i32).collect();
        assert!(!tris.is_empty());
        let pts: Vec<[f32; 3]> = tris.iter().flat_map(|t| t.local).map(|p| [-p[0] / super::PER_METRE, p[1] / super::PER_METRE, p[2] / super::PER_METRE]).collect();
        let lo_hi = |k: usize| (pts.iter().map(|p| p[k]).fold(f32::MAX, f32::min), pts.iter().map(|p| p[k]).fold(f32::MIN, f32::max));
        let (y0, y1) = lo_hi(1);
        assert!(y0 > -0.32 && y0 < -0.22 && y1 > 1.75 && y1 < 1.85, "from {y0} to {y1}");
        let (x0, x1) = lo_hi(0);
        assert!(x1 - x0 < 0.14, "{x0}..{x1} thick");
        // (a broad blade: 22 cm or so across along its length, above the guard)
        let mid: Vec<f32> = pts.iter().filter(|p| p[1] > 0.5 && p[1] < 1.7).map(|p| p[2]).collect();
        let width = mid.iter().fold(f32::MIN, |a, &b| a.max(b)) - mid.iter().fold(f32::MAX, |a, &b| a.min(b));
        assert!(width > 0.19 && width < 0.25, "{width} across");
        // the colours by piece: (palette index, a test of where its triangles' middles are)
        let at = |c: usize, ok: &dyn Fn([f32; 3]) -> bool| {
            tris.iter().any(|t| {
                let m = t.local.iter().fold([0.0f32; 3], |a, p| [a[0] - p[0] / super::PER_METRE / 3.0, a[1] + p[1] / super::PER_METRE / 3.0, a[2] + p[2] / super::PER_METRE / 3.0]);
                t.color[0] == super::KIT_PALETTE[c] && ok(m)
            })
        };
        assert!(at(16, &|m| m[1] > 0.5 && m[1] < 1.6), "steel flats");
        assert!(at(17, &|m| m[1] > 0.5 && m[1] < 1.6), "blue edges");
        assert!(at(7, &|m| m[1] > 0.4 && m[1] < 1.4 && m[0].abs() > 0.011), "red fuller proud of the flat");
        assert!(at(9, &|m| m[1] > 0.5 && m[1] < 1.3 && m[0].abs() > 0.011 && m[2].abs() < 0.04), "gold diamonds on the fuller");
        assert!(at(4, &|m| m[1] > 0.0 && m[1] < 0.3), "bone skull");
        assert!(at(8, &|m| m[1] > 0.05 && m[1] < 0.3), "black sockets and mouth");
        assert!(at(6, &|m| m[1] < 0.0 && m[1] > -0.2), "red grip");
        assert!(at(0, &|m| m[1] < 0.0 && m[1] > -0.2), "yellow rings");
        assert!(at(0, &|m| m[1] > 0.1 && m[1] < 0.5), "yellow horn tips");
        assert!(at(8, &|m| m[1] < -0.2), "the bracelet's black band");
        // five diamonds in a row up the fuller (gold triangles there, by their height)
        let mut rows: Vec<i32> = tris.iter().filter(|t| t.color[0] == super::KIT_PALETTE[9]).filter_map(|t| {
            let m = t.local.iter().fold([0.0f32; 3], |a, p| [a[0] - p[0] / super::PER_METRE / 3.0, a[1] + p[1] / super::PER_METRE / 3.0, a[2] + p[2] / super::PER_METRE / 3.0]);
            (m[0] > 0.011 && m[2].abs() < 0.04 && m[1] > 0.4).then(|| (m[1] * 20.0).round() as i32)
        }).collect();
        rows.sort();
        rows.dedup();
        assert!(rows.len() >= 5, "diamonds at {rows:?}");
        // the guard: the skull's horns and the gold ribs span wider than the blade, and not absurdly
        let g: Vec<f32> = pts.iter().filter(|p| p[1] > -0.02 && p[1] < 0.45).map(|p| p[2]).collect();
        let span = g.iter().fold(f32::MIN, |a, &b| a.max(b)) - g.iter().fold(f32::MAX, |a, &b| a.min(b));
        assert!(span > 0.3 && span < 0.6, "guard {span} wide");
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
