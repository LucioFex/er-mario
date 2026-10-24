//! How the weapons' elements look (element.rs, fireball.rs; ours and a co-op partner's alike, from
//! fx.rs's few numbers each), in 3D and low-poly like flame.rs's fireballs, with soft glows round
//! them:
//! - the hammer's ice block: a tumbling cube of clear ice with a frosted heart, glinting, a trail of
//!   snow behind; it shatters into shards that fall, in a puff of frost
//! - the Chain Chomp's dark orb: a black ball with crimson cracks glowing over it, a red rim, two
//!   eyes gleaming in front, wisps of darkness trailing; it bursts in a spray of blood and smoke
//! - the boomerang's poison glob: a wobbling ball of green and purple goo, a shine on it, bubbles,
//!   drops falling off it; it splashes, and its cloud lingers on the ground, bubbling
//! - the statuses on enemies: frostbite's ice crystals clinging to him, blood loss's spray, poison's
//!   bubbles rising off him; each with its burst as it goes off
//! - the motes round a weapon: embers, snowflakes of ice, wisps of darkness, little bubbles

use std::f32::consts::TAU;

use glam::{Quat, Vec3};

use crate::element::Element;
use crate::flame::{across, cone, fbm, hash, noise, rgba, sphere, spread, Eye, Look, Sprite, Tri, FRAME_GLOW};
use crate::fx::{Aura, Blast, Cloud, Mote, Orb, AURA_LONG, ORB_CHARGE};

/// light from above, a little to the side
const LIGHT: Vec3 = Vec3::new(0.3, 0.9, 0.25);
/// the ice's colours: its clear body, its frosted edges, the white of frost
const ICE: [f32; 3] = [0.55, 0.8, 0.98];
const ICE_EDGE: [f32; 3] = [0.9, 0.97, 1.0];
const FROST: [f32; 3] = [0.85, 0.94, 1.0];
/// the dark orb's: its black, its cracks, its rim
const DARK: [f32; 3] = [0.05, 0.0, 0.02];
const CRACK: [f32; 3] = [1.0, 0.16, 0.22];
/// lightning's glow
const THUNDER: [f32; 3] = [1.0, 0.92, 0.45];
const CRIMSON: [f32; 3] = [0.7, 0.04, 0.1];
const BLOOD: [f32; 3] = [0.55, 0.02, 0.05];
/// the poison's: its green, its purple, the haze
const VENOM: [f32; 3] = [0.38, 0.8, 0.18];
const VENOM_DARK: [f32; 3] = [0.5, 0.2, 0.62];
const HAZE_GREEN: [f32; 3] = [0.42, 0.72, 0.22];
const HAZE_PURPLE: [f32; 3] = [0.46, 0.22, 0.58];
/// gravity on the shards, drops and splashes (m/s²)
const FALL: f32 = 9.0;

fn mix(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    let k = k.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

fn scaled(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn glow(look: &mut Look, pos: Vec3, size: f32, c: [f32; 3], a: f32) {
    if a > 0.01 && size > 0.005 {
        look.sprites.push(Sprite { pos, size, frame: FRAME_GLOW, tint: rgba(c, a) });
    }
}

/// A seed from where something is (to 2 m): the same pattern frame to frame, a partner's too.
fn seed_of(p: Vec3) -> i32 {
    (p.x * 0.5).floor() as i32 * 7 + (p.z * 0.5).floor() as i32 * 13
}

/// A closed body of faces round `c` (`corners` from its middle, `faces` of three of them), only
/// those turned to the eye, coloured by their way out and how square they face the eye.
fn body(look: &mut Look, c: Vec3, corners: &[Vec3], faces: &[[usize; 3]], eye: Vec3, col: &dyn Fn(Vec3, f32) -> [f32; 4]) {
    for f in faces {
        let (a, b, d) = (c + corners[f[0]], c + corners[f[1]], c + corners[f[2]]);
        let n = (b - a).cross(d - a).normalize_or_zero();
        // (out from its middle, whichever way round the corners go)
        let n = if n.dot(a + b + d - c * 3.0) < 0.0 { -n } else { n };
        let mid = (a + b + d) / 3.0;
        let facing = n.dot((eye - mid).normalize_or(Vec3::Y));
        if facing <= 0.0 {
            continue;
        }
        let colour = col(n, facing);
        if colour[3] >= 0.01 {
            look.tris.push(Tri { p: [a, b, d], col: colour });
        }
    }
}

const CUBE_FACES: [[usize; 3]; 12] = [
    [0, 1, 2], [0, 2, 3], [4, 6, 5], [4, 7, 6], [0, 4, 5], [0, 5, 1],
    [1, 5, 6], [1, 6, 2], [2, 6, 7], [2, 7, 3], [3, 7, 4], [3, 4, 0],
];

/// A cube (`half` its half side) turned by `rot`.
fn cube(look: &mut Look, c: Vec3, half: f32, rot: Quat, eye: Vec3, col: &dyn Fn(Vec3, f32) -> [f32; 4]) {
    let k = [(-1.0, -1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, -1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0), (-1.0, 1.0, 1.0)];
    let corners: [Vec3; 8] = k.map(|(x, y, z)| rot * (Vec3::new(x, y, z) * half));
    body(look, c, &corners, &CUBE_FACES, eye, col);
}

const DIAMOND_FACES: [[usize; 3]; 8] = [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];

/// A crystal of ice: a long diamond (`size` its length) along `up`, turned about it by `spin`.
fn crystal(look: &mut Look, c: Vec3, size: f32, up: Vec3, spin: f32, eye: Vec3, alpha: f32) {
    let up = up.normalize_or(Vec3::Y);
    let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(spin);
    let w = size * 0.28;
    let corners = [Vec3::X * w, -Vec3::X * w, Vec3::Z * w, -Vec3::Z * w, Vec3::Y * size * 0.65, -Vec3::Y * size * 0.35].map(|p| rot * p);
    let light = LIGHT.normalize();
    body(look, c, &corners, &DIAMOND_FACES, eye, &|n, facing| {
        let lit = 0.6 + 0.4 * n.dot(light).max(0.0);
        rgba(scaled(mix(ICE, ICE_EDGE, (1.0 - facing) * 1.2), lit), alpha * (0.6 + 0.35 * (1.0 - facing)))
    });
}

/// A shard of ice: a small wedge of four faces.
fn shard(look: &mut Look, c: Vec3, size: f32, rot: Quat, eye: Vec3, alpha: f32) {
    let corners = [Vec3::new(0.0, size, 0.0), Vec3::new(size * 0.5, -size * 0.3, size * 0.2), Vec3::new(-size * 0.45, -size * 0.3, size * 0.3), Vec3::new(0.0, -size * 0.25, -size * 0.5)].map(|p| rot * p);
    let light = LIGHT.normalize();
    body(look, c, &corners, &[[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]], eye, &|n, facing| {
        let lit = 0.55 + 0.45 * n.dot(light).max(0.0);
        rgba(scaled(mix(ICE, ICE_EDGE, 1.0 - facing), lit), alpha * 0.85)
    });
}

/// A ball whose radius wobbles (`wob` by the way out, 1 round), only its faces turned to the eye.
#[allow(clippy::too_many_arguments)]
fn blob(look: &mut Look, c: Vec3, r: f32, rings: usize, slices: usize, eye: Vec3, wob: &dyn Fn(Vec3) -> f32, col: &dyn Fn(Vec3, f32) -> [f32; 4]) {
    let point = |i: usize, j: usize| {
        let (th, ph) = (std::f32::consts::PI * i as f32 / rings as f32, TAU * j as f32 / slices as f32);
        let n = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
        c + n * r * wob(n)
    };
    for i in 0..rings {
        for j in 0..slices {
            let (a, b, cc, d) = (point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1));
            let mid = (a + b + cc + d) * 0.25;
            let n = (mid - c).normalize_or(Vec3::Y);
            let facing = n.dot((eye - mid).normalize_or(Vec3::Y));
            if facing <= 0.0 {
                continue;
            }
            let colour = col(n, facing);
            if i + 1 < rings {
                look.tris.push(Tri { p: [a, b, cc], col: colour });
            }
            if i > 0 {
                look.tris.push(Tri { p: [a, cc, d], col: colour });
            }
        }
    }
}

/// A little bubble: a clear ball with a shine.
fn bubble(look: &mut Look, c: Vec3, r: f32, eye: Vec3, tint: [f32; 3], alpha: f32) {
    if alpha < 0.02 || r < 0.004 {
        return;
    }
    sphere(look, c, r, 3, 6, false, eye, Quat::IDENTITY, &|_, f, n| {
        let shine = (n.y * 0.7 + 0.3).max(0.0).powi(6) * f;
        rgba(mix(tint, [1.0, 1.0, 1.0], shine + (1.0 - f) * 0.3), alpha * (0.35 + 0.5 * (1.0 - f) + shine))
    });
}

/// A shot of another weapon's element, or its charge in Mario's hand.
pub fn shot(look: &mut Look, o: &Orb, eye: &Eye, clock: f32) {
    let el = Element::from_u8(o.element);
    let vis = o.vis.clamp(0.0, 1.0);
    let dist = (o.pos - eye.pos).length().max(0.01);
    let px = o.size * eye.px / dist;
    let charge = o.flags & ORB_CHARGE != 0;
    let level = o.level.min(3);
    let speed = o.vel.length();
    let back = if speed > 0.5 && !charge { -o.vel / speed } else { Vec3::ZERO };
    // (far off: a glow of its colour)
    if px < 4.0 {
        let c = match el {
            Element::Frost => ICE_EDGE,
            Element::Blood => CRIMSON,
            _ => VENOM,
        };
        glow(look, o.pos, o.size * 1.6, c, 0.8 * vis);
        return;
    }
    let lod = if px < 40.0 { 2 } else { 1 };
    let age = o.age as f32;
    match el {
        Element::Frost => ice_block(look, o, eye.pos, clock, age, level, charge, back, vis),
        Element::Blood => dark_orb(look, o, eye.pos, clock, age, level, charge, back, lod, vis),
        _ => venom_glob(look, o, eye.pos, clock, age, level, charge, back, lod, vis),
    }
}

#[allow(clippy::too_many_arguments)]
fn ice_block(look: &mut Look, o: &Orb, eye: Vec3, clock: f32, age: f32, level: u8, charge: bool, back: Vec3, vis: f32) {
    let half = o.size * 0.42;
    // tumbling as it flies; in his hand, turning slowly as it forms
    let axis = Vec3::new(0.6, 1.0, 0.3).normalize();
    let rot = Quat::from_axis_angle(axis, age * if charge { 0.07 } else { 0.24 });
    let light = LIGHT.normalize();
    // the frosted heart first (the clear faces over it)
    cube(look, o.pos, half * 0.55, Quat::from_axis_angle(Vec3::new(-0.4, 1.0, 0.7).normalize(), -age * 0.1), eye, &|_, f| rgba(mix(FROST, [1.0, 1.0, 1.0], 1.0 - f), (0.5 + 0.3 * (1.0 - f)) * vis));
    cube(look, o.pos, half, rot, eye, &|n, f| {
        let lit = 0.55 + 0.45 * n.dot(light).max(0.0);
        let rim = (1.0 - f).powf(1.5);
        rgba(scaled(mix(ICE, ICE_EDGE, 0.2 + rim * 0.8), lit + 0.15 * rim), (0.5 + 0.35 * rim) * vis)
    });
    // its glints, at its corners in turn
    for i in 0..4 {
        let k = [(1.0, 1.0, 1.0), (-1.0, 1.0, -1.0), (1.0, -1.0, -1.0), (-1.0, -1.0, 1.0)][i];
        let at = o.pos + rot * (Vec3::new(k.0, k.1, k.2) * half);
        let tw = ((clock * 7.0 + i as f32 * 1.7).sin() * 0.5 + 0.5).powi(4);
        glow(look, at, o.size * 0.35 * tw, [1.0, 1.0, 1.0], 0.9 * tw * vis);
    }
    // a cold glow round it, brighter charged
    glow(look, o.pos, o.size * (1.5 + 0.2 * level as f32), [0.6, 0.85, 1.0], (0.18 + 0.07 * level as f32) * vis);
    if charge {
        // crystals of ice gathering round his hand
        let n = 4 + level as usize * 2;
        for i in 0..n {
            let a = clock * 2.6 + i as f32 * TAU / n as f32;
            let r = o.size * (0.95 + 0.25 * (clock * 3.0 + i as f32).sin());
            let off = Vec3::new(a.cos() * r, 0.25 * r * (a * 2.0).sin(), a.sin() * r);
            crystal(look, o.pos + off, o.size * 0.22, off.normalize_or(Vec3::Y), clock * 4.0 + i as f32, eye, 0.8 * vis);
        }
        return;
    }
    // snow trailing behind it
    if back != Vec3::ZERO {
        for i in 1..=5 {
            let k = i as f32;
            let wob = Vec3::new((clock * 5.0 + k).sin(), (clock * 4.0 + k * 2.0).cos(), (clock * 6.0 + k * 1.3).sin()) * o.size * 0.15;
            glow(look, o.pos + back * o.size * 0.55 * k + wob, o.size * (0.45 - 0.06 * k), FROST, (0.5 - 0.08 * k) * vis);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dark_orb(look: &mut Look, o: &Orb, eye: Vec3, clock: f32, age: f32, level: u8, charge: bool, back: Vec3, lod: usize, vis: f32) {
    let r = o.size * 0.5;
    let spin = Quat::from_rotation_y(age * 0.09) * Quat::from_rotation_x(age * 0.05);
    let (rings, slices) = ((5 + level as usize) / lod.max(1), (8 + 2 * level as usize) / lod.max(1));
    let (rings, slices) = (rings.max(4), slices.max(6));
    // its heart: black, with crimson cracks glowing over it, a red rim
    sphere(look, o.pos, r, rings, slices, false, eye, spin, &|n, f, _| {
        let p = fbm(n * 2.6 + Vec3::splat(clock * 0.6));
        let crack = (1.0 - ((p - 0.5).abs() / 0.07).min(1.0)).powi(2);
        let rim = (1.0 - f).powi(2);
        let c = mix(mix(DARK, CRIMSON, rim * 0.9), CRACK, crack * (0.75 + 0.25 * (clock * 6.0).sin()));
        rgba(c, vis)
    });
    // a shell of red haze at its rim
    sphere(look, o.pos, r * 1.28, rings, slices, false, eye, spin, &|_, f, _| rgba(CRIMSON, 0.45 * (1.0 - f).powf(1.6) * vis));
    // two eyes gleaming in front (the way it goes; in his hand, at the camera)
    let fwd = if back != Vec3::ZERO { -back } else { (eye - o.pos).normalize_or(Vec3::Z) };
    let (side, _) = across(o.pos + fwd, o.pos);
    let side = side - fwd * side.dot(fwd);
    let side = side.normalize_or(Vec3::X);
    let up = fwd.cross(side).normalize_or(Vec3::Y);
    let up = if up.y < 0.0 { -up } else { up };
    for s in [-1.0, 1.0] {
        let at = o.pos + fwd * r * 0.9 + side * s * r * 0.32 + up * r * 0.18;
        glow(look, at, r * 0.32, [1.0, 0.85, 0.85], 0.95 * vis);
        glow(look, at, r * 0.6, CRACK, 0.4 * vis);
    }
    // wisps of darkness curling off it, streaming back
    let n = 4 + 2 * level as usize;
    let away = if back != Vec3::ZERO { back } else { Vec3::Y };
    for i in 0..n {
        let flick = noise(Vec3::new(i as f32 * 3.7, clock * 2.0, 1.3));
        let out = (spin * spread(i, n) + away * 0.9).normalize_or(away);
        let base = o.pos + spread(i, n) * r * 0.85;
        cone(look, base, out, r * (0.7 + 0.8 * flick), r * 0.2, rgba([0.12, 0.0, 0.03], 0.8 * vis));
    }
    glow(look, o.pos, o.size * (1.6 + 0.2 * level as f32), CRIMSON, (0.2 + 0.06 * level as f32) * vis);
    if charge {
        // darkness drawn in to his hand
        for i in 0..6 {
            let t = (clock * 1.4 + i as f32 / 6.0).fract();
            let d = spread(i * 3 + 1, 18);
            glow(look, o.pos + d * o.size * (1.6 - 1.3 * t), o.size * 0.25, [0.15, 0.0, 0.04], 0.7 * t * vis);
        }
        return;
    }
    if back != Vec3::ZERO {
        for i in 1..=5 {
            let k = i as f32;
            glow(look, o.pos + back * o.size * 0.5 * k, o.size * (0.5 + 0.1 * k), [0.12, 0.0, 0.03], (0.55 - 0.09 * k) * vis);
            if i % 2 == 1 {
                let wob = Vec3::new((clock * 9.0 + k).sin(), (clock * 7.0 + k).cos(), (clock * 8.0 + k * 2.0).sin()) * o.size * 0.3;
                glow(look, o.pos + back * o.size * 0.45 * k + wob, 0.06, CRACK, 0.8 * vis);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn venom_glob(look: &mut Look, o: &Orb, eye: Vec3, clock: f32, age: f32, level: u8, charge: bool, back: Vec3, lod: usize, vis: f32) {
    let r = o.size * 0.5;
    let (rings, slices) = (((5 + level as usize) / lod.max(1)).max(4), ((8 + 2 * level as usize) / lod.max(1)).max(6));
    let t = clock;
    let light = LIGHT.normalize();
    blob(
        look,
        o.pos,
        r,
        rings,
        slices,
        eye,
        &|n| 1.0 + 0.12 * (n.y * 3.0 + t * 6.0).sin() * (n.x * 2.0 + n.z * 2.0 + t * 4.0).cos(),
        &|n, f| {
            let swirl = fbm(n * 2.2 + Vec3::new(t * 0.4, -t * 0.3, age * 0.01));
            let c = mix(VENOM, VENOM_DARK, ((swirl - 0.45) * 3.0).clamp(0.0, 1.0));
            let lit = 0.6 + 0.4 * n.dot(light).max(0.0);
            let shine = n.dot(light).max(0.0).powi(12) * f;
            rgba(mix(scaled(c, lit), [1.0, 1.0, 1.0], shine * 0.9), (0.78 + 0.2 * (1.0 - f)) * vis)
        },
    );
    // bubbles on it
    for i in 0..(2 + level as usize) {
        let ph = (t * 0.9 + i as f32 * 0.37).fract();
        let d = spread(i * 5 + 2, 15);
        bubble(look, o.pos + d * r * (0.95 + 0.1 * ph), r * (0.1 + 0.12 * ph), eye, VENOM, (1.0 - ph) * vis);
    }
    glow(look, o.pos, o.size * (1.5 + 0.2 * level as f32), HAZE_GREEN, (0.16 + 0.06 * level as f32) * vis);
    if charge {
        // the goo gathering: drops pulled in to his hand
        for i in 0..5 {
            let k = (t * 1.2 + i as f32 / 5.0).fract();
            let d = spread(i * 4 + 1, 20);
            glow(look, o.pos + d * o.size * (1.4 - 1.1 * k), o.size * 0.18, if i % 2 == 0 { VENOM } else { VENOM_DARK }, 0.8 * k * vis);
        }
        return;
    }
    // drops falling off behind it
    if back != Vec3::ZERO {
        for i in 0..4 {
            let k = (t * 2.0 + i as f32 * 0.25).fract();
            let at = o.pos + back * o.size * (0.4 + 1.2 * k) - Vec3::Y * k * k * 0.6;
            glow(look, at, o.size * 0.22 * (1.0 - 0.5 * k), if i % 2 == 0 { VENOM } else { VENOM_DARK }, 0.8 * (1.0 - k) * vis);
        }
    }
}

/// Another element's burst: the ice shattering, the dark orb's spray, the glob's splash (and a
/// puff of it at his hand as one leaves it: level 0, small).
pub fn burst(look: &mut Look, b: &Blast, eye: &Eye) {
    let el = Element::from_u8(b.element);
    let life = crate::fireball::element_burst_ticks(el, b.level) as f32;
    let t = (b.age as f32 / life).clamp(0.0, 1.0);
    if t >= 1.0 {
        return;
    }
    let vis = b.vis.clamp(0.0, 1.0);
    let s = b.age as f32 / 30.0;
    let size = b.size;
    let seed = seed_of(b.pos) + b.level as i32 * 31;
    let level = b.level.min(3) as f32;
    match el {
        Element::Frost => {
            // a flash of cold, a puff of frost
            if t < 0.3 {
                let e = t / 0.3;
                sphere(look, b.pos, size * (0.4 + 0.8 * e), 4, 7, false, eye.pos, Quat::IDENTITY, &|_, f, _| rgba(FROST, (1.0 - e).powi(2) * 0.7 * (0.4 + 0.6 * f) * vis));
            }
            for i in 0..4 {
                let d = spread(i * 2 + 1, 8);
                glow(look, b.pos + d * size * (0.3 + 1.2 * t), size * (0.8 + 1.4 * t), FROST, 0.45 * (1.0 - t) * vis);
            }
            // the shards: flung out, tumbling, falling
            let n = 6 + 3 * b.level.min(3) as usize;
            for i in 0..n {
                let h = hash(seed, i as i32, 3);
                let dir = (spread(i, n) + Vec3::Y * 0.6).normalize();
                let v = (2.2 + 2.5 * h) * (0.7 + 0.2 * level);
                let p = b.pos + dir * v * s - Vec3::Y * 0.5 * FALL * s * s;
                let rot = Quat::from_axis_angle(spread(i + 7, n + 7), s * (6.0 + 6.0 * h));
                shard(look, p, size * (0.16 + 0.12 * h) * (1.0 - t.powi(3)), rot, eye.pos, (1.0 - t * t) * vis);
            }
            // snow drifting down
            for i in 0..5 {
                let h = hash(seed, i, 9);
                let d = spread(i as usize * 3, 15);
                glow(look, b.pos + d * size * (0.5 + h) - Vec3::Y * t * 0.6, 0.05 + 0.04 * h, [1.0, 1.0, 1.0], 0.8 * (1.0 - t) * vis);
            }
        }
        Element::Blood => {
            // a crimson flash, swelling and fading
            if t < 0.4 {
                let e = t / 0.4;
                sphere(look, b.pos, size * (0.35 + 0.9 * e), 4, 7, false, eye.pos, Quat::IDENTITY, &|_, f, _| rgba(mix(CRACK, CRIMSON, e), (1.0 - e).powi(2) * 0.75 * (0.3 + 0.7 * f) * vis));
            }
            // blood sprayed out, falling
            let n = 8 + 3 * b.level.min(3) as usize;
            for i in 0..n {
                let h = hash(seed, i as i32, 5);
                let dir = (spread(i, n) + Vec3::Y * 0.4).normalize();
                let v = (2.0 + 3.0 * h) * (0.8 + 0.15 * level);
                let p = b.pos + dir * v * s - Vec3::Y * 0.5 * FALL * s * s;
                glow(look, p, 0.07 + 0.07 * h + 0.03 * level, BLOOD, 0.95 * (1.0 - t) * vis);
            }
            // darkness billowing
            for i in 0..4 {
                let d = spread(i * 3 + 2, 12);
                glow(look, b.pos + d * size * (0.2 + 0.9 * t) + Vec3::Y * t * 0.5, size * (0.7 + 1.2 * t), [0.1, 0.0, 0.03], 0.5 * (1.0 - t) * vis);
            }
        }
        _ => {
            // a ring of goo splashing out, drops flung up and falling
            let e = t.min(0.6) / 0.6;
            if e < 1.0 {
                let (u, v) = (Vec3::X, Vec3::Z);
                crate::flame::band(&mut look.tris, b.pos, u, v, size * (0.3 + 1.3 * e), size * (0.45 + 1.6 * e), 12, &|a| 1.0 + 0.1 * (a * 5.0).sin(), rgba(VENOM, 0.55 * (1.0 - e) * vis));
            }
            let n = 7 + 3 * b.level.min(3) as usize;
            for i in 0..n {
                let h = hash(seed, i as i32, 7);
                let dir = (spread(i, n) + Vec3::Y * 0.9).normalize();
                let v = (1.6 + 2.0 * h) * (0.8 + 0.15 * level);
                let p = b.pos + dir * v * s - Vec3::Y * 0.5 * FALL * s * s;
                glow(look, p, 0.08 + 0.06 * h + 0.03 * level, if i % 3 == 0 { VENOM_DARK } else { VENOM }, 0.9 * (1.0 - t) * vis);
            }
            for i in 0..3 {
                let ph = (t * 1.5 + i as f32 * 0.3).min(1.0);
                let d = spread(i * 4 + 1, 12);
                bubble(look, b.pos + d * size * 0.6 + Vec3::Y * ph * 0.6, size * 0.12 * (1.0 + ph), eye.pos, VENOM, (1.0 - ph) * vis);
            }
            glow(look, b.pos, size * (1.0 + 1.5 * t), HAZE_GREEN, 0.4 * (1.0 - t) * vis);
        }
    }
}

/// The poison glob's cloud: a haze of green and purple on the ground, puffs swirling slowly in it,
/// bubbles rising and bursting; thickening as it comes, thinning out at its end.
pub fn cloud(look: &mut Look, c: &Cloud, eye: &Eye, clock: f32) {
    let fade = (c.age as f32 / 8.0).min(1.0) * ((c.life.saturating_sub(c.age)) as f32 / 20.0).min(1.0) * c.vis.clamp(0.0, 1.0);
    if fade < 0.01 {
        return;
    }
    let r = c.radius;
    let seed = seed_of(c.pos);
    glow(look, c.pos + Vec3::Y * 0.3, r * 2.3, HAZE_GREEN, 0.2 * fade);
    glow(look, c.pos + Vec3::Y * 0.5, r * 1.6, HAZE_PURPLE, 0.16 * fade);
    let n = 7;
    for i in 0..n {
        let h = hash(seed, i, 11);
        let turn = if i % 2 == 0 { 0.35 } else { -0.28 };
        let a = i as f32 * TAU / n as f32 + clock * turn + h;
        let d = r * (0.35 + 0.45 * h);
        let y = 0.25 + 0.7 * hash(seed, i, 13) + 0.12 * (clock * 0.8 + h * 6.0).sin();
        glow(look, c.pos + Vec3::new(a.cos() * d, y, a.sin() * d), r * (0.8 + 0.3 * h), if i % 2 == 0 { HAZE_GREEN } else { HAZE_PURPLE }, 0.24 * fade);
    }
    for i in 0..4 {
        let h = hash(seed, i, 17);
        let ph = (clock * (0.5 + 0.3 * h) + h).fract();
        let a = h * TAU * 3.0;
        let d = r * 0.7 * hash(seed, i, 19);
        let at = c.pos + Vec3::new(a.cos() * d, 0.1 + ph * 1.1, a.sin() * d);
        bubble(look, at, 0.04 + 0.05 * h + 0.03 * ph, eye.pos, VENOM, (1.0 - ph) * 0.9 * fade);
    }
}

/// A status on an enemy: frostbite's crystals clinging to him, poison's bubbles rising off him, and
/// each one's burst as it goes off (blood loss: its spray).
pub fn aura(look: &mut Look, a: &Aura, eye: &Eye, clock: f32) {
    let el = Element::from_u8(a.element);
    let h = a.height.clamp(0.6, 8.0);
    let rad = a.radius.clamp(0.25, 4.0);
    let chest = a.pos + Vec3::Y * h * 0.55;
    let seed = seed_of(a.pos);
    let burst = (a.age < AURA_LONG).then_some(a.age as f32 / 30.0);
    match el {
        Element::Frost => {
            if a.left > 0.0 {
                // (they grow in, and melt away in the last of it)
                let k = (a.left / 0.15).min(1.0) * burst.map_or(1.0, |s| (s / 0.3).min(1.0));
                for i in 0..7 {
                    let hh = hash(seed, i, 21);
                    let ang = i as f32 * 2.399 + hh;
                    let out = Vec3::new(ang.cos(), 0.0, ang.sin());
                    let at = a.pos + out * rad * 0.9 + Vec3::Y * h * (0.12 + 0.75 * hash(seed, i, 23));
                    crystal(look, at, (0.14 + 0.12 * hh) * k * (h / 2.0).clamp(0.6, 2.0), out + Vec3::Y * 0.8, hh * 6.0, eye.pos, 0.85);
                }
                glow(look, a.pos + Vec3::Y * 0.2, rad * 2.6, FROST, 0.16 * k);
                // a glint now and then
                let tw = ((clock * 3.0 + hash(seed, 1, 1) * 6.0).sin() * 0.5 + 0.5).powi(8);
                glow(look, chest + Vec3::Y * h * 0.2, 0.3 * tw, [1.0, 1.0, 1.0], tw);
            }
            if let Some(s) = burst.filter(|s| *s < 1.0) {
                let t = s;
                sphere(look, chest, rad * (0.8 + 2.2 * t), 4, 8, false, eye.pos, Quat::IDENTITY, &|_, f, _| rgba(FROST, (1.0 - t).powi(2) * 0.55 * (1.0 - f).powf(0.8)));
                crate::flame::band(&mut look.tris, a.pos + Vec3::Y * 0.05, Vec3::X, Vec3::Z, rad * (0.8 + 3.0 * t), rad * (1.1 + 3.4 * t), 16, &|_| 1.0, rgba(ICE_EDGE, 0.5 * (1.0 - t)));
                for i in 0..10 {
                    let hh = hash(seed, i, 25);
                    let dir = (spread(i as usize, 10) + Vec3::Y * 0.5).normalize();
                    let p = chest + dir * (2.5 + 2.5 * hh) * t - Vec3::Y * 0.5 * FALL * t * t;
                    shard(look, p, 0.12 + 0.1 * hh, Quat::from_axis_angle(spread(i as usize + 3, 13), t * 9.0), eye.pos, 1.0 - t);
                }
            }
        }
        Element::Blood => {
            let Some(s) = burst.filter(|s| *s < 1.6) else { return };
            // the spray: up out of him and falling, a crimson flash, darkness; drops off him after
            if s < 0.5 {
                let e = s / 0.5;
                sphere(look, chest, rad * (0.6 + 1.6 * e), 4, 8, false, eye.pos, Quat::IDENTITY, &|_, f, _| rgba(mix(CRACK, CRIMSON, e), (1.0 - e).powi(2) * 0.6 * (0.2 + 0.8 * f)));
            }
            if s < 1.0 {
                for i in 0..18 {
                    let hh = hash(seed, i, 27);
                    let dir = (spread(i as usize, 18) * 0.7 + Vec3::Y * 0.9).normalize();
                    let p = chest + dir * (2.5 + 3.5 * hh) * s - Vec3::Y * 0.5 * FALL * s * s;
                    glow(look, p, 0.08 + 0.1 * hh, BLOOD, (1.0 - s) * 0.95);
                }
                for i in 0..3 {
                    glow(look, chest + spread(i * 4 + 1, 12) * rad * (0.4 + s), rad * (1.2 + 1.5 * s), [0.1, 0.0, 0.03], 0.45 * (1.0 - s));
                }
            }
            for i in 0..4 {
                let ph = ((s * 1.4 + i as f32 * 0.25) % 1.0).min(1.0);
                let ang = i as f32 * 2.1 + hash(seed, i, 29) * 3.0;
                let at = a.pos + Vec3::new(ang.cos(), 0.0, ang.sin()) * rad * 0.7 + Vec3::Y * h * (0.6 - 0.55 * ph);
                glow(look, at, 0.06, BLOOD, (1.0 - ph) * (1.6 - s).min(1.0) * 0.9);
            }
        }
        _ => {
            if a.left > 0.0 {
                // bubbles rising off him, a haze about him
                let k = (a.left / 0.1).min(1.0);
                for i in 0..5 {
                    let hh = hash(seed, i, 31);
                    let ph = (clock * (0.45 + 0.2 * hh) + hh).fract();
                    let ang = hh * TAU * 2.0 + i as f32;
                    let at = a.pos + Vec3::new(ang.cos(), 0.0, ang.sin()) * rad * (0.6 + 0.4 * hh) + Vec3::Y * h * (0.2 + 0.9 * ph);
                    bubble(look, at, 0.05 + 0.05 * hh + 0.04 * ph, eye.pos, if i % 2 == 0 { VENOM } else { VENOM_DARK }, (1.0 - ph) * 0.85 * k);
                }
                glow(look, chest, rad * 2.4, HAZE_PURPLE, 0.12 * k);
                glow(look, a.pos + Vec3::Y * 0.3, rad * 2.0, HAZE_GREEN, 0.12 * k);
            }
            if let Some(s) = burst.filter(|s| *s < 0.8) {
                let t = s / 0.8;
                for i in 0..10 {
                    let d = spread(i, 10);
                    bubble(look, chest + d * rad * (0.5 + 1.2 * t), 0.07 + 0.05 * hash(seed, i as i32, 33), eye.pos, if i % 2 == 0 { VENOM } else { VENOM_DARK }, (1.0 - t) * 0.9);
                }
                glow(look, chest, rad * (1.5 + 2.5 * t), HAZE_GREEN, 0.4 * (1.0 - t));
            }
        }
    }
}

/// A mote of a weapon's element: an ember rising, a snowflake of ice turning, a wisp of darkness,
/// a little bubble bursting at its end.
pub fn mote(look: &mut Look, m: &Mote, eye: &Eye, clock: f32) {
    let t = m.t.clamp(0.0, 1.0);
    let fade = (1.0 - t) * (t * 6.0).min(1.0);
    match Element::from_u8(m.element) {
        Element::Fire => look.sprites.push(Sprite { pos: m.pos, size: m.size * (1.0 - 0.4 * t), frame: ((t * 8.0) as usize + (m.pos.x * 10.0) as usize) % 8, tint: [1.0, 0.75 - 0.3 * t, 0.3, 0.9 * fade] }),
        Element::Frost => {
            crystal(look, m.pos, m.size, Vec3::new((clock * 2.0 + m.pos.x).sin(), 1.0, (clock * 2.0 + m.pos.z).cos()), clock * 3.0 + m.pos.y * 9.0, eye.pos, 0.85 * fade);
            glow(look, m.pos, m.size * 2.2, FROST, 0.35 * fade);
        }
        Element::Blood => {
            glow(look, m.pos, m.size * (2.0 + 2.0 * t), [0.12, 0.0, 0.03], 0.55 * fade);
            glow(look, m.pos, m.size * 0.6, CRACK, 0.8 * fade);
        }
        Element::Poison => bubble(look, m.pos, m.size * (0.6 + 0.4 * t), eye.pos, if (m.pos.x * 7.0) as i32 % 2 == 0 { VENOM } else { VENOM_DARK }, 0.9 * fade),
        Element::Lightning => {
            // a crackle: a white-hot spark that flickers in a yellow glow
            let flick = 0.6 + 0.4 * ((clock * 41.0 + m.pos.x * 13.0).sin() * 0.5 + 0.5);
            glow(look, m.pos, m.size * 2.4, THUNDER, 0.45 * fade * flick);
            glow(look, m.pos, m.size * 0.7, [1.0, 1.0, 0.95], 0.9 * fade * flick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye() -> Eye {
        Eye { pos: Vec3::new(0.0, 2.0, -10.0), px: 900.0, screen: 1080.0 }
    }

    #[test]
    fn every_element_draws_finite_faces() {
        for element in 1..=3u8 {
            for level in 0..=3u8 {
                for flags in [0, ORB_CHARGE] {
                    let mut look = Look::default();
                    let o = Orb { pos: Vec3::new(0.0, 2.0, 0.0), vel: Vec3::Z * 15.0, size: 0.4 + 0.25 * level as f32, level, age: 9, flags, ground: None, vis: 1.0, element };
                    shot(&mut look, &o, &eye(), 1.5);
                    assert!(!look.tris.is_empty(), "element {element} level {level} flags {flags}");
                    assert!(look.tris.iter().all(|t| t.p.iter().all(|p| p.is_finite()) && t.col.iter().all(|c| c.is_finite())));
                    assert!(look.sprites.iter().all(|s| s.pos.is_finite() && s.size.is_finite()));
                }
                for age in [0, 5, 15, 30] {
                    let mut look = Look::default();
                    let b = Blast { pos: Vec3::new(0.0, 1.0, 0.0), level, age, size: 0.8, ground: None, flags: 0, vis: 1.0, element };
                    burst(&mut look, &b, &eye());
                    assert!(look.tris.iter().all(|t| t.p.iter().all(|p| p.is_finite())));
                }
            }
            for age in [0, 10, 40, AURA_LONG] {
                let mut look = Look::default();
                aura(&mut look, &Aura { pos: Vec3::ZERO, height: 2.0, radius: 0.5, element, age, left: 0.6 }, &eye(), 2.0);
                assert!(look.tris.iter().all(|t| t.p.iter().all(|p| p.is_finite())));
                mote(&mut look, &Mote { pos: Vec3::Y, element, t: 0.5, size: 0.1 }, &eye(), 2.0);
            }
        }
        let mut look = Look::default();
        cloud(&mut look, &Cloud { pos: Vec3::ZERO, age: 30, life: 90, radius: 2.0, vis: 1.0 }, &eye(), 3.0);
        assert!(!look.sprites.is_empty());
    }

    #[test]
    fn far_off_a_shot_is_a_glow() {
        let mut look = Look::default();
        let o = Orb { pos: Vec3::new(0.0, 2.0, 0.0), vel: Vec3::Z, size: 0.5, level: 0, age: 3, flags: 0, ground: None, vis: 1.0, element: 2 };
        shot(&mut look, &o, &Eye { pos: Vec3::new(0.0, 2.0, -500.0), ..eye() }, 1.0);
        assert!(look.tris.is_empty() && look.sprites.len() == 1);
    }
}
