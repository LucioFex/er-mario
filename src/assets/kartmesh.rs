//! The kart's model (v4 Part 4), built here for the legs piece (assets/mod.rs: the Mario set's
//! legs are otherwise empty), each part on a free twist bone of its own posed by engine_mario:
//! Mario's red kart, the M on its nose, a blue seat, a grey engine with two chrome exhausts, fat
//! black tyres with grey tread and gold hubs that spin and steer, a steering wheel with a gold boss.
//! Its own axes (kartplay.rs): x across (+x its left), y up, z ahead, from the chassis' middle; the
//! ground is about 0.42 m below it. The wheels round their own middle, their axle along x; the
//! steering wheel round its middle, its rim across x and y (facing −z, the driver).

use glam::Vec3;

use super::kit::Builder;
use super::model::Tri;
use crate::kartplay::{BODY, STEERING, WHEELS};

/// the palette's swatches the kart uses (kit.rs KIT_PALETTE)
const RED: usize = 19;
const DARK_RED: usize = 20;
const TYRE: usize = 21;
const TREAD: usize = 22;
const SEAT_BLUE: usize = 23;
const ENGINE: usize = 24;
const CHROME: usize = 25;
const BLACK: usize = 26;
const WHITE: usize = 5;
const GOLD: usize = 9;

/// The whole kart: every part.
pub fn build() -> Vec<Tri> {
    let mut out = Vec::new();
    body(&mut Builder { out: &mut out, part: BODY as i32 });
    for w in 0..4 {
        let side = if crate::kart::MOUNTS[w].x > 0.0 { 1.0 } else { -1.0 };
        wheel(&mut Builder { out: &mut out, part: (WHEELS + w) as i32 }, side);
    }
    steering(&mut Builder { out: &mut out, part: STEERING as i32 });
    out
}

/// The tub's cross-sections from its tail to its nose: (z, half width, bottom, top): low and long,
/// the cowl rising in front of the driver
const TUB: [(f32, f32, f32, f32); 8] = [
    (-1.18, 0.3, -0.18, 0.08),
    (-1.0, 0.44, -0.26, 0.14),
    (-0.6, 0.5, -0.28, 0.12),
    (0.0, 0.5, -0.28, 0.11),
    (0.45, 0.46, -0.27, 0.2),
    (0.85, 0.38, -0.24, 0.24),
    (1.15, 0.28, -0.18, 0.16),
    (1.32, 0.16, -0.1, 0.06),
];
/// points round each cross-section
const AROUND: usize = 12;
/// the front wheels' middle at rest (kart.rs: their mount less the spring pressed in by its load)
const FRONT_WHEEL: Vec3 = Vec3::new(0.65, -0.2, 1.0);

fn body(b: &mut Builder) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    // the tub: rounded box sections (a superellipse), its belly a darker red
    let point = |i: usize, j: usize| {
        let (z, w, lo, hi) = TUB[j];
        let a = -FRAC_PI_2 + TAU * (i % AROUND) as f32 / AROUND as f32;
        let f = |v: f32| v.signum() * v.abs().sqrt();
        let (x, y) = (f(a.cos()), f(a.sin()));
        let (cy, h) = ((hi + lo) * 0.5, (hi - lo) * 0.5);
        (Vec3::new(w * x, cy + h * y, z), Vec3::new(x.powi(3) / w, y.powi(3) / h, 0.0).normalize_or(Vec3::Y))
    };
    b.grid(AROUND, TUB.len() - 1, true, &point, &|i, _| if matches!(i, 10 | 11 | 0 | 1) { DARK_RED } else { RED });
    // its ends closed
    for (j, sign) in [(0usize, -1.0f32), (TUB.len() - 1, 1.0)] {
        let (z, _, lo, hi) = TUB[j];
        let c = Vec3::new(0.0, (lo + hi) * 0.5, z);
        for i in 0..AROUND {
            b.face([c, point(i, j).0, point(i + 1, j).0], Vec3::Z * sign, RED);
        }
    }
    // side pods, the axles, the bumpers and their struts, the fenders
    for side in [-1.0f32, 1.0] {
        b.cylinder(Vec3::new(side * 0.56, -0.12, -0.55), Vec3::new(side * 0.56, -0.12, 0.45), 0.13, 10, RED, true);
    }
    for z in [FRONT_WHEEL.z, -FRONT_WHEEL.z] {
        b.cylinder(Vec3::new(-0.55, FRONT_WHEEL.y, z), Vec3::new(0.55, FRONT_WHEEL.y, z), 0.035, 6, BLACK, false);
    }
    b.cylinder(Vec3::new(-0.5, -0.12, 1.3), Vec3::new(0.5, -0.12, 1.3), 0.045, 8, BLACK, true);
    b.cylinder(Vec3::new(-0.45, -0.1, -1.25), Vec3::new(0.45, -0.1, -1.25), 0.045, 8, BLACK, true);
    for side in [-1.0f32, 1.0] {
        b.cylinder(Vec3::new(side * 0.22, -0.12, 1.3), Vec3::new(side * 0.18, -0.12, 1.12), 0.028, 6, BLACK, false);
        fender(b, side * FRONT_WHEEL.x);
    }
    // the seat: a blue cushion on the tub, its back tilted back
    let y = crate::kartplay::SEAT.y;
    b.slab(&[Vec3::new(-0.2, y - 0.03, -0.14), Vec3::new(0.2, y - 0.03, -0.14), Vec3::new(0.2, y - 0.03, -0.5), Vec3::new(-0.2, y - 0.03, -0.5)], Vec3::Y, 0.03, SEAT_BLUE);
    let back = [Vec3::new(-0.2, y, -0.52), Vec3::new(0.2, y, -0.52), Vec3::new(0.18, 0.6, -0.62), Vec3::new(-0.18, 0.6, -0.62)];
    let n = (back[1] - back[0]).cross(back[3] - back[0]).normalize();
    b.slab(&back, n, 0.035, SEAT_BLUE);
    // the engine behind him, its air filter, the two exhausts out the back
    b.slab(&[Vec3::new(-0.26, 0.2, -0.68), Vec3::new(0.26, 0.2, -0.68), Vec3::new(0.26, 0.2, -1.0), Vec3::new(-0.26, 0.2, -1.0)], Vec3::Y, 0.1, ENGINE);
    b.cylinder(Vec3::new(0.0, 0.3, -0.84), Vec3::new(0.0, 0.36, -0.84), 0.1, 10, CHROME, true);
    for mouth in crate::kartplay::EXHAUSTS {
        let root = Vec3::new(mouth.x, mouth.y + 0.02, -0.95);
        b.cylinder(root, mouth - Vec3::Z * 0.03, 0.045, 8, CHROME, false);
        b.cylinder(mouth - Vec3::Z * 0.03, mouth, 0.06, 8, CHROME, false);
        b.disc(mouth - Vec3::Z * 0.01, -Vec3::Z, 0.05, 8, BLACK);
    }
    // the steering column, from the cowl up to the wheel
    let n = glam::Quat::from_rotation_x(crate::kartplay::WHEEL_TILT) * -Vec3::Z;
    let wheel = crate::kartplay::WHEEL_AT;
    b.cylinder(wheel - n * 0.36, wheel - n * 0.02, 0.022, 6, BLACK, false);
    // the M on the nose: a white disc, the letter's four red strokes on it
    let n = Vec3::new(0.0, 0.93, 0.36).normalize();
    let c = Vec3::new(0.0, 0.18, 1.08) + n * 0.012;
    b.disc(c, n, 0.1, 12, WHITE);
    let u = Vec3::X;
    let v = n.cross(u).normalize();
    let v = if v.z > 0.0 { v } else { -v };
    let on = |x: f32, y: f32| c + n * 0.005 + u * x + v * y;
    for (a, z) in [((-0.06, -0.055), (-0.06, 0.055)), ((0.06, -0.055), (0.06, 0.055)), ((-0.06, 0.055), (0.0, -0.01)), ((0.06, 0.055), (0.0, -0.01))] {
        let (p, q) = (on(a.0, a.1), on(z.0, z.1));
        let across = n.cross(q - p).normalize() * 0.011;
        b.face([p - across, q - across, q + across], n, RED);
        b.face([p - across, q + across, p + across], n, RED);
    }
}

/// A fender over the front wheel at `x`: a thick arc round it, over its top and front, held to the
/// tub by a strut.
fn fender(b: &mut Builder, x: f32) {
    const STEPS: usize = 7;
    const SPAN: (f32, f32) = (0.25, 2.6);
    const WIDE: f32 = 0.17;
    let c = FRONT_WHEEL.with_x(x);
    let at = |k: usize, r: f32, dx: f32| {
        let a = SPAN.0 + (SPAN.1 - SPAN.0) * k as f32 / STEPS as f32;
        let n = Vec3::new(0.0, a.sin(), a.cos());
        (c + Vec3::X * dx + n * r, n)
    };
    b.grid(STEPS, 1, false, &|i, j| at(i, 0.34, if j == 0 { -WIDE } else { WIDE }), &|_, _| RED);
    b.grid(
        STEPS,
        1,
        false,
        &|i, j| {
            let (p, n) = at(i, 0.31, if j == 0 { -WIDE } else { WIDE });
            (p, -n)
        },
        &|_, _| DARK_RED,
    );
    for dx in [-WIDE, WIDE] {
        for k in 0..STEPS {
            let (a0, a1, b0, b1) = (at(k, 0.31, dx).0, at(k + 1, 0.31, dx).0, at(k, 0.34, dx).0, at(k + 1, 0.34, dx).0);
            let out = Vec3::X * dx.signum();
            b.face([a0, a1, b1], out, RED);
            b.face([a0, b1, b0], out, RED);
        }
    }
    b.cylinder(Vec3::new(x.signum() * 0.3, 0.0, c.z), Vec3::new(x.signum() * (x.abs() - WIDE), 0.06, c.z), 0.03, 6, BLACK, false);
}

/// A wheel round its middle, its axle along x: a fat black tyre with a band of grey tread blocks
/// (they show it turning), its sidewalls, a gold hub on its `out` side with a chrome cap.
fn wheel(b: &mut Builder, out: f32) {
    use std::f32::consts::TAU;
    const AROUND: usize = 12;
    let r = crate::kart::WHEEL_R;
    // the tyre's profile across: (x, radius, its normal's lean along x)
    let profile: [(f32, f32, f32); 6] = [(-0.13, r - 0.05, -0.7), (-0.115, r - 0.015, -0.35), (-0.09, r, 0.0), (0.09, r, 0.0), (0.115, r - 0.015, 0.35), (0.13, r - 0.05, 0.7)];
    let point = |i: usize, j: usize| {
        let a = TAU * (i % AROUND) as f32 / AROUND as f32;
        let radial = Vec3::new(0.0, a.sin(), a.cos());
        let (x, rr, lean) = profile[j];
        (Vec3::X * x + radial * rr, (radial * (1.0 - lean.abs()) + Vec3::X * lean).normalize())
    };
    b.grid(AROUND, profile.len() - 1, true, &point, &|i, j| if j == 2 && i % 2 == 0 { TREAD } else { TYRE });
    for side in [-1.0f32, 1.0] {
        let ring = |i: usize, j: usize| {
            let a = TAU * (i % AROUND) as f32 / AROUND as f32;
            let radial = Vec3::new(0.0, a.sin(), a.cos());
            (Vec3::X * 0.13 * side + radial * if j == 0 { r - 0.05 } else { 0.1 }, Vec3::X * side)
        };
        b.grid(AROUND, 1, true, &ring, &|_, _| TYRE);
        b.disc(Vec3::X * 0.13 * side, Vec3::X * side, 0.1, AROUND, if side == out { GOLD } else { BLACK });
    }
    b.cylinder(Vec3::X * 0.13 * out, Vec3::X * 0.155 * out, 0.04, 8, CHROME, false);
    b.disc(Vec3::X * 0.155 * out, Vec3::X * out, 0.04, 8, CHROME);
}

/// The steering wheel round its middle, its rim across x and y: a black rim, three spokes, a red
/// middle with a gold boss (facing −z, the driver).
fn steering(b: &mut Builder) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    const AROUND: usize = 12;
    const TUBE: usize = 3;
    const THICK: f32 = 0.018;
    let rim = crate::kartplay::WHEEL_RIM;
    let point = |i: usize, j: usize| {
        let a = TAU * (i % AROUND) as f32 / AROUND as f32;
        let out = Vec3::new(a.cos(), a.sin(), 0.0);
        let s = TAU * (j % TUBE) as f32 / TUBE as f32;
        let n = out * s.cos() + Vec3::Z * s.sin();
        (out * rim + n * THICK, n)
    };
    b.grid(AROUND, TUBE, true, &point, &|_, _| BLACK);
    for k in 0..3 {
        let a = -FRAC_PI_2 + TAU * k as f32 / 3.0;
        b.cylinder(Vec3::ZERO, Vec3::new(a.cos(), a.sin(), 0.0) * (rim - THICK), 0.012, 6, BLACK, false);
    }
    b.disc(-Vec3::Z * 0.012, -Vec3::Z, 0.05, 10, RED);
    b.disc(-Vec3::Z * 0.016, -Vec3::Z, 0.025, 8, GOLD);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::kit::{KIT_PALETTE, PER_METRE};
    use crate::kart;
    use crate::kartplay::PARTS;

    fn points(tris: &[Tri], part: usize) -> Vec<Vec3> {
        tris.iter().filter(|t| t.part == part as i32).flat_map(|t| t.local).map(|p| Vec3::new(-p[0], p[1], p[2]) / PER_METRE).collect()
    }

    #[test]
    fn it_fits_the_legs_piece() {
        let tris = build();
        let (verts, faces) = crate::assets::flver::kit_vertices(&tris, 0..PARTS);
        println!("kart: {} vertices, {} triangles", verts.len(), faces.len());
        for part in 0..PARTS {
            println!("kart: part {part} {} vertices", crate::assets::flver::kit_vertices(&tris, part..part + 1).0.len());
        }
        // (the legs piece's biggest mesh: 1736 vertices, 2492 triangles)
        assert!(verts.len() <= 1700 && faces.len() <= 2400, "{} {}", verts.len(), faces.len());
        for part in 0..PARTS {
            assert!(!points(&tris, part).is_empty(), "part {part} is empty");
        }
        assert!(tris.iter().all(|t| (0..PARTS as i32).contains(&t.part)));
    }

    #[test]
    fn its_wheels_turn_round_their_axles() {
        let tris = build();
        for w in 0..4 {
            let pts = points(&tris, WHEELS + w);
            let r = pts.iter().map(|p| p.y.hypot(p.z)).fold(0.0f32, f32::max);
            assert!((r - kart::WHEEL_R).abs() < 0.01, "wheel {w}: radius {r}");
            assert!(pts.iter().all(|p| p.x.abs() <= 0.16), "wheel {w} too wide");
            // (its gold hub on its outside: +x for the +x wheels)
            let out = if kart::MOUNTS[w].x > 0.0 { 1.0 } else { -1.0 };
            let hub: Vec<f32> = tris.iter().filter(|t| t.part == (WHEELS + w) as i32 && t.color[0] == KIT_PALETTE[GOLD]).flat_map(|t| t.local).map(|p| -p[0] / PER_METRE).collect();
            assert!(!hub.is_empty() && hub.iter().all(|x| x * out > 0.0), "wheel {w}'s hub");
        }
    }

    #[test]
    fn its_body_and_steering_wheel_where_they_go() {
        let tris = build();
        for p in points(&tris, BODY) {
            assert!(p.x.abs() <= 0.86 && p.z.abs() <= 1.45 && p.y > -0.36 && p.y < 0.7, "{p}");
        }
        for p in points(&tris, STEERING) {
            assert!(p.z.abs() <= 0.05 && p.with_z(0.0).length() <= crate::kartplay::WHEEL_RIM + 0.03, "{p}");
        }
        // (the M on its nose: white and red up front)
        let nose = |c: usize| tris.iter().any(|t| t.part == BODY as i32 && t.color[0] == KIT_PALETTE[c] && t.local.iter().all(|p| p[2] / PER_METRE > 1.0));
        assert!(nose(WHITE) && nose(RED));
        for t in &tris {
            for n in t.normal {
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((l - 1.0).abs() < 1e-3, "normal {n:?}");
            }
        }
    }
}
