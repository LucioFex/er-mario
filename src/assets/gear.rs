//! Mario's thrown and swung toys as real meshes, each on a bone of its own like the hammer
//! (model.rs): the Boomerang Bros.' boomerang (three, as many as can be out at once), the Chain
//! Chomp (its ball and its lower jaw, so it bites) and the links of its chain. Built from Mario's
//! own colours and the ROM's darkest texel (the albedo has no room for more), in metres, part-local:
//! - a boomerang lies flat (its face +Y), its spin centre at the origin, the bend towards +Z
//! - the Chomp looks along +Z, its centre at the origin; the jaw hinges about HINGE
//! - a part of the chain is two links hooked into each other along +Y (the first flat in XY, the
//!   second turned a quarter), centred at the origin

use glam::Vec3;

use super::model::Tri;
use crate::engine_mario::{CHAIN, CHAIN_LINKS, CHOMP, JAW, RANG, RANGS};

/// SM64 units per metre in the mesh (as the hammer's: flver.rs scales by 0.01 * 0.25)
const PER_METRE: f32 = 400.0;
/// the Chomp's radius (m) and where its jaw hinges (a little behind its centre, at the mouth's
/// top: the mouth is a wedge out of the ball from there, Pac-Man's, so the jaw turns out of it
/// whole; this far forward its floor stays under the jaw opened all the way)
pub const CHOMP_R: f32 = 0.4;
pub const HINGE: Vec3 = Vec3::new(0.0, 0.0, -0.2 * CHOMP_R);
/// a link's length from end to end (m), and a pair's (the chain's pairs are spread along it)
pub const LINK_LEN: f32 = 0.19;
pub const PAIR_LEN: f32 = LINK_LEN * 1.55;
/// a boomerang's span (m), tip to tip
pub const RANG_SPAN: f32 = 0.55;

#[derive(Clone, Copy, PartialEq)]
enum Paint {
    /// the ROM's darkest texel (the hammer's head)
    Dark,
    Color([f32; 3]),
}

struct Builder<'a> {
    out: &'a mut Vec<Tri>,
    part: i32,
    /// the dark texel's atlas UV and a texel's size, or none (then the darkest colour)
    patch: Option<([f32; 2], (f32, f32))>,
    darkest: [f32; 3],
}

impl Builder<'_> {
    /// One triangle; `uv_at` picks each corner's step on the dark texel (corners that differ must
    /// differ there too, or the triangle isn't textured; shared corners keep theirs).
    fn tri(&mut self, p: [Vec3; 3], n: [Vec3; 3], paint: Paint, steps: [(usize, usize); 3]) {
        let (color, uv) = match (paint, self.patch) {
            (Paint::Dark, Some(([u, v], (du, dv)))) => (self.darkest, steps.map(|(a, b)| [u + (a % 2) as f32 * du, v + (b % 2) as f32 * dv])),
            (Paint::Dark, None) => (self.darkest, [[0.0; 2]; 3]),
            (Paint::Color(c), _) => (c, [[0.0; 2]; 3]),
        };
        // (SM64 is mirrored on X: flver.rs flips it back)
        let local = p.map(|v| [-v.x * PER_METRE, v.y * PER_METRE, v.z * PER_METRE]);
        let normal = n.map(|v| [-v.x, v.y, v.z]);
        self.out.push(Tri { part: self.part, local, normal, uv, color: [color; 3], world: local });
    }

    /// A flat triangle facing away from `inside`.
    fn face(&mut self, p: [Vec3; 3], inside: Vec3, paint: Paint) {
        let mut n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(Vec3::Y);
        if n.dot((p[0] + p[1] + p[2]) / 3.0 - inside) < 0.0 {
            n = -n;
        }
        self.tri(p, [n; 3], paint, [(0, 0), (1, 0), (1, 1)]);
    }

    /// A grid of quads: `point(i, j)` gives a point and its normal for column i (0..=cols) and row
    /// j (0..=rows), `paint(i, j)` each quad's paint (None: no quad). Collapsed edges (a pole) are
    /// skipped. `wrap`: columns and rows that close on themselves (a ring's last is its first).
    fn grid(&mut self, cols: usize, rows: usize, wrap: (bool, bool), point: &dyn Fn(usize, usize) -> (Vec3, Vec3), paint: &dyn Fn(usize, usize) -> Option<Paint>) {
        let step = |i: usize, j: usize| (if wrap.0 { i % cols } else { i }, if wrap.1 { j % rows } else { j });
        for j in 0..rows {
            for i in 0..cols {
                let Some(paint) = paint(i, j) else { continue };
                let (a, na) = point(i, j);
                let (b, nb) = point(i + 1, j);
                let (c, nc) = point(i + 1, j + 1);
                let (d, nd) = point(i, j + 1);
                let (sa, sb, sc, sd) = (step(i, j), step(i + 1, j), step(i + 1, j + 1), step(i, j + 1));
                if a.distance(b) > 1e-5 {
                    self.tri([a, b, c], [na, nb, nc], paint, [sa, sb, sc]);
                }
                if c.distance(d) > 1e-5 {
                    self.tri([a, c, d], [na, nc, nd], paint, [sa, sc, sd]);
                }
            }
        }
    }
}

/// All of them: the boomerangs, the Chomp and its jaw, the links. `model` gives Mario's colours.
pub fn build(model: &[Tri], atlas: &[u8]) -> Vec<Tri> {
    let nearest = |c: [f32; 3]| {
        let d = |t: &Tri| (0..3).map(|k| (t.color[0][k] - c[k]).powi(2)).sum::<f32>();
        model.iter().min_by(|a, b| d(a).total_cmp(&d(b))).map_or(c, |t| t.color[0])
    };
    let darkest = model.iter().map(|t| t.color[0]).min_by(|a, b| a.iter().sum::<f32>().total_cmp(&b.iter().sum::<f32>())).unwrap_or([0.2; 3]);
    let (white, red) = (nearest([1.0, 1.0, 1.0]), nearest([1.0, 0.0, 0.0]));
    let patch = super::model::dark_patch(atlas);
    let mut out = Vec::new();
    for k in 0..RANGS {
        boomerang(&mut Builder { out: &mut out, part: (RANG + k) as i32, patch, darkest }, white, red);
    }
    chomp(&mut Builder { out: &mut out, part: CHOMP as i32, patch, darkest }, white, red);
    jaw(&mut Builder { out: &mut out, part: JAW as i32, patch, darkest }, white, red);
    for k in 0..CHAIN_LINKS {
        link(&mut Builder { out: &mut out, part: (CHAIN + k) as i32, patch, darkest });
    }
    out
}

/// A fat chevron: two flattened arms from the bend, white, their tips red, tapering to a point.
fn boomerang(b: &mut Builder, white: [f32; 3], red: [f32; 3]) {
    const SIDES: usize = 5;
    let half = RANG_SPAN * 0.5;
    for side in [-1.0f32, 1.0] {
        // (each arm starts a little inside the other: the open end is buried)
        let from = Vec3::new(-side * 0.06, 0.0, 0.15);
        let tip = Vec3::new(side * half, 0.0, -0.12);
        let along = (tip - from).normalize();
        let across = Vec3::Y.cross(along).normalize();
        // rings: (share of the way, half width, half thickness)
        let rings = [(0.0, 0.05, 0.022), (0.68, 0.042, 0.02)];
        let ring = |r: usize, s: usize| {
            let (t, w, h) = rings[r];
            let a = s as f32 / SIDES as f32 * std::f32::consts::TAU;
            let c = from.lerp(tip, t);
            let p = c + across * (a.cos() * w) + Vec3::Y * (a.sin() * h);
            let n = (across * (a.cos() / w) + Vec3::Y * (a.sin() / h)).normalize();
            (p, n)
        };
        b.grid(SIDES, 1, (true, false), &|i, j| ring(j, i % SIDES), &|_, _| Some(Paint::Color(white)));
        // the red tip: from the second ring to a point
        let end = tip + along * 0.01;
        for s in 0..SIDES {
            let (p0, n0) = ring(1, s);
            let (p1, n1) = ring(1, (s + 1) % SIDES);
            b.tri([p0, p1, end], [n0, n1, along], Paint::Color(red), [(0, 0); 3]);
        }
    }
}

/// The ball on the Chomp's sphere: column i (longitude, 0 = front), row j (0 = top).
const LON: usize = 10;
const LAT: usize = 6;
/// the mouth's quads: rows 3..5 (from the middle down) in the columns around the front, a hole in
/// the ball that the jaw fills
fn mouth(i: usize, j: usize) -> bool {
    (j == 3 || j == 4) && (i >= LON - 2 || i < 2)
}
/// the mouth's rim on its own grid (column 0..=4 from -X round the front, row 0..=2 down from the
/// middle): along the top from -X, down the +X side, back along the bottom, up the -X side
const RIM: [(usize, usize); 12] = [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (4, 1), (4, 2), (3, 2), (2, 2), (1, 2), (0, 2), (0, 1)];
/// the teeth's spacing along the mouth (radians round the front): the lower ones between the upper
const TEETH: f32 = 0.3;

fn sphere(i: usize, j: usize, r: f32) -> (Vec3, Vec3) {
    let lon = i as f32 / LON as f32 * std::f32::consts::TAU;
    let lat = j as f32 / LAT as f32 * std::f32::consts::PI;
    let n = Vec3::new(lat.sin() * lon.sin(), lat.cos(), lat.sin() * lon.cos());
    (n * r, n)
}

/// A point of the mouth's patch of the ball (column i, row j of its own grid) and its normal.
fn lip(i: usize, j: usize) -> (Vec3, Vec3) {
    sphere((i + LON - 2) % LON, j + 3, CHOMP_R)
}

/// Edge k of the mouth's rim fanned to the hinge (the wedge the jaw is): the edge's ends on the
/// mouth's grid, the triangle, and its normal out of the wedge (the ball's side faces the other way).
fn fan(k: usize) -> ([(usize, usize); 2], [Vec3; 3], Vec3) {
    let ends = [RIM[k], RIM[(k + 1) % RIM.len()]];
    let [p, q] = ends.map(|(i, j)| lip(i, j).0);
    (ends, [p, q, HINGE], (p - HINGE).cross(q - HINGE).normalize())
}

/// The Chomp's ball: dark iron, its mouth a red hollow, two big eyes and its upper teeth.
fn chomp(b: &mut Builder, white: [f32; 3], red: [f32; 3]) {
    b.grid(LON, LAT, (true, false), &|i, j| sphere(i % LON, j, CHOMP_R), &|i, j| (!mouth(i, j)).then_some(Paint::Dark));
    // the mouth's hollow, from the hole's rim back to the hinge, seen from in front: its roof flat
    // at the middle (the jaw's top shuts against it), its cheeks and floor sloping in
    for k in 0..RIM.len() {
        let (_, p, n) = fan(k);
        b.tri(p, [-n; 3], Paint::Color(red), [(0, 0); 3]);
    }
    // the eyes: white discs, the pupils a little in front and towards the middle
    let at = |lon: f32, lat: f32, r: f32| {
        let n = Vec3::new(lat.sin() * lon.sin(), lat.cos(), lat.sin() * lon.cos());
        (n * r, n)
    };
    for side in [-1.0f32, 1.0] {
        let (c, n) = at(side * 0.5, 0.95, CHOMP_R * 1.01);
        disc(b, c, n, 0.11, 6, Paint::Color(white));
        let (c, n) = at(side * 0.42, 1.0, CHOMP_R * 1.03);
        disc(b, c, n, 0.05, 5, Paint::Dark);
    }
    // the upper teeth along the top of the mouth, pointing down
    for k in 0..5 {
        let lon = (k as f32 - 2.0) * TEETH;
        tooth(b, lon, std::f32::consts::FRAC_PI_2, 0.3, white);
    }
}

/// The Chomp's lower jaw: the wedge the mouth is, out of the ball (its shell flush with the ball's,
/// shut it fills the hole), its top the red floor of the mouth, its teeth pointing up.
fn jaw(b: &mut Builder, white: [f32; 3], red: [f32; 3]) {
    b.grid(4, 2, (false, false), &lip, &|_, _| Some(Paint::Dark));
    // its top, from the rim's top edge to the hinge, red; its sides and bottom (only seen opened,
    // out of the ball) dark, on the shell's own steps, the hinge on a step of its own
    for k in 0..RIM.len() {
        let (ends, p, n) = fan(k);
        if k < 4 {
            b.tri(p, [n; 3], Paint::Color(red), [(0, 0); 3]);
        } else {
            b.tri(p, [n; 3], Paint::Dark, [ends[0], ends[1], (1, 1)]);
        }
    }
    for k in 0..4 {
        let lon = (k as f32 - 1.5) * TEETH;
        tooth(b, lon, std::f32::consts::FRAC_PI_2, -0.3, white);
    }
}

/// A tooth on the ball's mouth line: its base around (lon, lat), its tip `reach` radians further
/// down (up when negative), a little out of the ball. Narrow enough for the upper and lower teeth
/// to sit side by side, shut, without cutting into each other.
fn tooth(b: &mut Builder, lon: f32, lat: f32, reach: f32, white: [f32; 3]) {
    let at = |lon: f32, lat: f32, r: f32| {
        let n = Vec3::new(lat.sin() * lon.sin(), lat.cos(), lat.sin() * lon.cos());
        n * r
    };
    let w = TEETH * 0.22;
    let (l, r) = (at(lon - w, lat, CHOMP_R * 1.05), at(lon + w, lat, CHOMP_R * 1.05));
    let back = at(lon, lat, CHOMP_R * 0.95);
    let tip = at(lon, lat + reach, CHOMP_R * 1.07);
    let inside = (l + r + back + tip) / 4.0 - (l + r) * 0.02;
    b.face([l, r, tip], back * 0.5, Paint::Color(white));
    b.face([l, back, tip], inside + (r - l) * 0.3, Paint::Color(white));
    b.face([r, back, tip], inside - (r - l) * 0.3, Paint::Color(white));
    // (its base too: with the mouth open it shows from below, or above)
    b.face([l, r, back], tip, Paint::Color(white));
}

/// A flat disc at `c`, facing `n`.
fn disc(b: &mut Builder, c: Vec3, n: Vec3, r: f32, sides: usize, paint: Paint) {
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    for s in 0..sides {
        let a0 = s as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (s + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let p0 = c + (u * a0.cos() + v * a0.sin()) * r;
        let p1 = c + (u * a1.cos() + v * a1.sin()) * r;
        b.tri([c, p0, p1], [n; 3], paint, [(0, 0), (s, 1), ((s + 1) % sides, 1)]);
    }
}

/// Two links of the chain, hooked into each other: oval rings of dark iron, long along Y.
fn link(b: &mut Builder) {
    const MAJOR: usize = 6;
    const MINOR: usize = 3;
    let (half, wide, tube) = (LINK_LEN * 0.5 - 0.022, 0.05, 0.022);
    for (k, turned) in [(-1.0f32, false), (1.0, true)] {
        let shift = Vec3::Y * (k * (PAIR_LEN - LINK_LEN) * 0.5);
        let point = |i: usize, j: usize| {
            let a = (i % MAJOR) as f32 / MAJOR as f32 * std::f32::consts::TAU;
            let m = (j % MINOR) as f32 / MINOR as f32 * std::f32::consts::TAU;
            let c = Vec3::new(a.cos() * wide, a.sin() * half, 0.0);
            let out = Vec3::new(a.cos() / wide, a.sin() / half, 0.0).normalize();
            let n = out * m.cos() + Vec3::Z * m.sin();
            let (p, n) = (c + n * tube, n);
            // (the second a quarter turn round Y)
            if turned { (Vec3::new(-p.z, p.y, p.x) + shift, Vec3::new(-n.z, n.y, n.x)) } else { (p + shift, n) }
        };
        b.grid(MAJOR, MINOR, (true, true), &point, &|_, _| Some(Paint::Dark));
    }
}

#[cfg(test)]
mod tests {
    /// The gear fits next to Mario in the chest piece's buffers (1184 vertices, 5592 indices; Mario
    /// and the hammer took 752 and 3081): at most 400 vertices and 800 triangles of its own.
    #[test]
    fn fits() {
        let tris = super::build(&[], &[]);
        let mut keys: Vec<(i32, [i64; 3], [i64; 2], [i64; 3])> = Vec::new();
        for t in &tris {
            for k in 0..3 {
                let key = (t.part, t.local[k].map(|v| (v * 100.0).round() as i64), t.uv[k].map(|v| (v * 1e6).round() as i64), t.color[k].map(|v| (v * 255.0).round() as i64));
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
        println!("gear: {} vertices, {} triangles", keys.len(), tris.len());
        assert!(keys.len() <= 400 && tris.len() <= 800);
    }
}
