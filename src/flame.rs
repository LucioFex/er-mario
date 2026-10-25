//! How Mario's fireballs look (fireball.rs's shots, the charge in his hand, their bursts and the
//! sun's lava pools; ours and a co-op partner's alike, from fx.rs's few numbers a ball): a ball of
//! fire in 3D, low-poly like SM64's own, in layers, each charge level grander than the last:
//! - its heart: faces of fire churning (the giant one's magma, dark crust plates over glowing
//!   cracks; the sun's white-hot plasma, orange at its edge), lit by how square they face the eye
//! - a shell of flame round it that glows at its rim (a ball of fire, not a disc, from any side)
//! - tongues of flame licking off it, streaming back the way it came, and a tail behind it
//! - from the full charge on, rays of light round its rim; the big ones' heat rippling off them in
//!   rings, their light on the ground under them; the sun's arcs of fire, and the screen warming
//!   as it fills it (inside it, all fire)
//! - SM64's flame sprites still round it all: its glow, the licks, the tail, embers, smoke
//! The bursts: a flash of fire swelling, the big ones' dome of fire and shock wave over the ground
//! and smoke billowing up (the sun's a mushroom), on top of their flames.
//! Nothing here allocates once its buffers have grown (hud.rs keeps a `Look` and clears it).

use std::f32::consts::{PI, TAU};

use glam::{Quat, Vec3};

use crate::fireball::{big, blast_life, burst_ticks, EXPLODE_REACH, GIANT, SUN};
use crate::fx::{FireFx, Orb, BLAST_LAUNCH, ORB_CHARGE, ORB_REFLECTED, ORB_STUCK};

/// a soft round glow (halos, embers, smoke), and the giant shot's ball of fire (4 frames of its
/// surface churning: only for one too far off for its faces)
pub const FRAME_GLOW: usize = 8;
pub const FRAME_ORB: usize = 9;

/// A flame to draw: world position, size (m), frame (0-7 SM64's flame, then FRAME_GLOW and the
/// FRAME_ORB ones), tint.
#[derive(Clone, Copy)]
pub struct Sprite {
    pub pos: Vec3,
    pub size: f32,
    pub frame: usize,
    pub tint: [f32; 4],
}

/// A face of fire (world points), its colour and see-through
#[derive(Clone, Copy)]
pub struct Tri {
    pub p: [Vec3; 3],
    pub col: [f32; 4],
}

/// What to draw: the faces (only those turned to the eye, or both sides of the thin ones), the
/// sprites, how much the screen warms (0-1: a big one filling it) and how deep in one the camera is
#[derive(Default)]
pub struct Look {
    pub tris: Vec<Tri>,
    /// the light on the ground under the big ones: drawn first, under everything
    pub under: Vec<Tri>,
    pub sprites: Vec<Sprite>,
    pub heat: f32,
    pub inside: f32,
}

impl Look {
    pub fn clear(&mut self) {
        self.tris.clear();
        self.under.clear();
        self.sprites.clear();
        self.heat = 0.0;
        self.inside = 0.0;
    }
}

/// The camera: where, pixels a metre spans 1 m off, the screen's height
pub struct Eye {
    pub pos: Vec3,
    pub px: f32,
    pub screen: f32,
}

/// Seconds on the clock the flames flicker by (the render thread's own).
pub fn clock() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

/// A level's looks: the heart's rings and slices, its shell's glow, tongues of flame, rays round
/// its rim, the tail's rings, the heat's ripples
struct Style {
    rings: usize,
    slices: usize,
    shell: f32,
    tongues: usize,
    rays: usize,
    tail: usize,
    ripples: usize,
}

const STYLE: [Style; 6] = [
    Style { rings: 4, slices: 6, shell: 0.0, tongues: 4, rays: 0, tail: 0, ripples: 0 },
    Style { rings: 5, slices: 8, shell: 0.3, tongues: 6, rays: 0, tail: 0, ripples: 0 },
    Style { rings: 6, slices: 9, shell: 0.36, tongues: 8, rays: 0, tail: 3, ripples: 0 },
    Style { rings: 7, slices: 11, shell: 0.42, tongues: 10, rays: 6, tail: 4, ripples: 0 },
    Style { rings: 9, slices: 14, shell: 0.5, tongues: 14, rays: 10, tail: 6, ripples: 1 },
    Style { rings: 11, slices: 18, shell: 0.55, tongues: 20, rays: 18, tail: 7, ripples: 3 },
];

/// flame colours (sprites): deep orange, hot orange, red-orange with a white core, and the full
/// charge's flashing (Mega Man's buster cycles its colours)
const TINT: [[f32; 4]; 3] = [[1.0, 0.62, 0.25, 1.0], [1.0, 0.5, 0.18, 1.0], [1.0, 0.35, 0.12, 1.0]];
const FLASH: [[f32; 4]; 3] = [[1.0, 1.0, 1.0, 1.0], [1.0, 0.85, 0.3, 1.0], [1.0, 0.3, 0.1, 1.0]];

fn tint(level: u8, tick: u32) -> [f32; 4] {
    match level {
        GIANT.. => FLASH[(tick % 3) as usize],
        3 => FLASH[(tick / 2 % 3) as usize],
        _ => TINT[level.min(2) as usize],
    }
}

/// Fire's colours by heat (0 cold soot red, 1 white-hot)
fn heat(t: f32) -> [f32; 3] {
    const STOPS: [(f32, [f32; 3]); 6] = [
        (0.0, [0.16, 0.03, 0.02]),
        (0.3, [0.72, 0.12, 0.03]),
        (0.5, [1.0, 0.4, 0.06]),
        (0.7, [1.0, 0.7, 0.2]),
        (0.86, [1.0, 0.92, 0.55]),
        (1.0, [1.0, 1.0, 0.93]),
    ];
    let t = t.clamp(0.0, 1.0);
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if t <= b {
            let k = (t - a) / (b - a);
            return [ca[0] + (cb[0] - ca[0]) * k, ca[1] + (cb[1] - ca[1]) * k, ca[2] + (cb[2] - ca[2]) * k];
        }
    }
    STOPS[5].1
}

pub(crate) fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a.clamp(0.0, 1.0)]
}

/// (a turned-back shot: blue-white)
fn blue(c: [f32; 4], on: bool) -> [f32; 4] {
    if on {
        [c[2], c[1] * 0.95, c[0], c[3]]
    } else {
        c
    }
}

pub(crate) fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth noise in space (0-1)
pub(crate) fn noise(p: Vec3) -> f32 {
    let f = p.floor();
    let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - t * 2.0);
    let lerp = |a: f32, b: f32, k: f32| a + (b - a) * k;
    let c = |dx, dy, dz| hash(x + dx, y + dy, z + dz);
    lerp(
        lerp(lerp(c(0, 0, 0), c(1, 0, 0), s.x), lerp(c(0, 1, 0), c(1, 1, 0), s.x), s.y),
        lerp(lerp(c(0, 0, 1), c(1, 0, 1), s.x), lerp(c(0, 1, 1), c(1, 1, 1), s.x), s.y),
        s.z,
    )
}

pub(crate) fn fbm(p: Vec3) -> f32 {
    0.6 * noise(p) + 0.3 * noise(p * 2.1 + Vec3::splat(5.2)) + 0.1 * noise(p * 4.3 + Vec3::splat(1.7))
}

/// A wobble for flame `i` at time `t` (0-1)
fn flick(i: usize, t: f32) -> f32 {
    noise(Vec3::new(i as f32 * 3.7, t, i as f32 * 1.3))
}

/// The i-th of n ways out spread evenly round a ball
pub(crate) fn spread(i: usize, n: usize) -> Vec3 {
    // (past n it goes round again: y went past -1 and the way grew long and straight down)
    let n = n.max(1);
    let y = 1.0 - 2.0 * ((i % n) as f32 + 0.5) / n as f32;
    let r = (1.0 - y * y).max(0.0).sqrt();
    let a = i as f32 * 2.399_963;
    Vec3::new(a.cos() * r, y, a.sin() * r)
}

/// Two ways across the view at `c` (seen from `eye`)
pub(crate) fn across(eye: Vec3, c: Vec3) -> (Vec3, Vec3) {
    let d = (c - eye).normalize_or(Vec3::Z);
    let u = d.cross(Vec3::Y).normalize_or(Vec3::X);
    (u, u.cross(d))
}

/// A ball's faces (a hemisphere, its top half, with `half`), only those turned to the eye. `col`
/// colours each by its way out (`spin` turning the pattern over it) and how square it faces the
/// eye (1 straight on, 0 edge on), its normal for light from above.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sphere(look: &mut Look, c: Vec3, r: f32, rings: usize, slices: usize, half: bool, eye: Vec3, spin: Quat, col: &dyn Fn(Vec3, f32, Vec3) -> [f32; 4]) {
    let span = if half { PI * 0.5 } else { PI };
    let point = |i: usize, j: usize| {
        let (th, ph) = (span * i as f32 / rings as f32, TAU * j as f32 / slices as f32);
        Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin())
    };
    for i in 0..rings {
        for j in 0..slices {
            let (a, b, cc, d) = (point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1));
            let n = (a + b + cc + d).normalize_or(Vec3::Y);
            let mid = c + n * r;
            let to_eye = eye - mid;
            let facing = n.dot(to_eye.normalize_or(Vec3::Y));
            if facing <= 0.0 {
                continue;
            }
            let colour = col(spin * n, facing, n);
            if colour[3] < 0.01 {
                continue;
            }
            let (pa, pb, pc, pd) = (c + a * r, c + b * r, c + cc * r, c + d * r);
            if i + 1 < rings || half {
                look.tris.push(Tri { p: [pa, pb, pc], col: colour });
            }
            if i > 0 {
                look.tris.push(Tri { p: [pa, pc, pd], col: colour });
            }
        }
    }
}

/// A tongue of flame: a three-sided cone from `base` along `axis` (`len` long, `w` wide at its foot)
pub(crate) fn cone(look: &mut Look, base: Vec3, axis: Vec3, len: f32, w: f32, col: [f32; 4]) {
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    let tip = base + axis * len;
    let foot = |k: usize| {
        let a = k as f32 * TAU / 3.0;
        base + (u * a.cos() + v * a.sin()) * w
    };
    for k in 0..3 {
        let shade = [1.0, 0.88, 0.76][k];
        look.tris.push(Tri { p: [foot(k), foot((k + 1) % 3), tip], col: [col[0], col[1] * shade, col[2] * shade, col[3]] });
    }
}

/// A tube along `pts` (radius `radius(k)` at each), only its faces turned to the eye, coloured by
/// how far along (`col(k)`).
pub(crate) fn tube(look: &mut Look, pts: &[Vec3], sides: usize, eye: Vec3, radius: &dyn Fn(usize) -> f32, col: &dyn Fn(usize) -> [f32; 4]) {
    // (each ring's axes carried on from the one before: picked anew at each point they flipped
    // where its way crossed an axis, and the tube pinched there)
    let mut frames: Vec<(Vec3, Vec3)> = Vec::with_capacity(pts.len());
    for k in 0..pts.len() {
        let d = (pts[(k + 1).min(pts.len() - 1)] - pts[k.saturating_sub(1)]).normalize_or(Vec3::Y);
        let u = frames.last().map_or_else(|| d.any_orthonormal_vector(), |&(u, _)| (u - d * u.dot(d)).try_normalize().unwrap_or_else(|| d.any_orthonormal_vector()));
        frames.push((u, d.cross(u)));
    }
    let frame = |k: usize| frames[k];
    for k in 0..pts.len().saturating_sub(1) {
        let ((u0, v0), (u1, v1)) = (frame(k), frame(k + 1));
        let colour = col(k);
        for j in 0..sides {
            let (a0, a1) = (TAU * j as f32 / sides as f32, TAU * (j + 1) as f32 / sides as f32);
            let ring = |p: Vec3, u: Vec3, v: Vec3, r: f32, a: f32| p + (u * a.cos() + v * a.sin()) * r;
            let (r0, r1) = (radius(k), radius(k + 1));
            let (p00, p01) = (ring(pts[k], u0, v0, r0, a0), ring(pts[k], u0, v0, r0, a1));
            let (p10, p11) = (ring(pts[k + 1], u1, v1, r1, a0), ring(pts[k + 1], u1, v1, r1, a1));
            let mid = (p00 + p01 + p10 + p11) * 0.25;
            let out = mid - (pts[k] + pts[k + 1]) * 0.5;
            if out.dot(eye - mid) <= 0.0 {
                continue;
            }
            look.tris.push(Tri { p: [p00, p10, p11], col: colour });
            look.tris.push(Tri { p: [p00, p11, p01], col: colour });
        }
    }
}

/// A flat ring (from radius `r0` to `r1`) about `c` in the plane of `u` and `v`, `n` pieces, each
/// radius wobbled by `wob`
#[allow(clippy::too_many_arguments)]
pub(crate) fn band(tris: &mut Vec<Tri>, c: Vec3, u: Vec3, v: Vec3, r0: f32, r1: f32, n: usize, wob: &dyn Fn(f32) -> f32, col: [f32; 4]) {
    let at = |k: usize, r: f32| {
        let a = TAU * k as f32 / n as f32;
        c + (u * a.cos() + v * a.sin()) * r * wob(a)
    };
    for k in 0..n {
        let (a, b, cc, d) = (at(k, r0), at(k + 1, r0), at(k + 1, r1), at(k, r1));
        if r0 > 0.0 {
            tris.push(Tri { p: [a, b, cc], col });
        }
        tris.push(Tri { p: [a, cc, d], col });
    }
}

/// Everything in `fire` as seen from `eye`, into `look` (cleared first by the caller).
pub fn build(look: &mut Look, fire: &FireFx, eye: &Eye, clock: f32) {
    let tick = (clock * 30.0) as u32;
    // (the other weapons' shots and bursts, the clouds, the statuses and the motes: elemfx.rs)
    for orb in &fire.orbs {
        if crate::element::Element::from_u8(orb.element) == crate::element::Element::Fire {
            ball(look, orb, eye, clock, tick);
        } else {
            crate::elemfx::shot(look, orb, eye, clock);
        }
    }
    for b in &fire.blasts {
        if crate::element::Element::from_u8(b.element) == crate::element::Element::Fire {
            blast(look, b, eye, tick);
        } else {
            crate::elemfx::burst(look, b, eye);
        }
    }
    for p in &fire.pools {
        pool(look, p, tick);
    }
    for c in &fire.clouds {
        crate::elemfx::cloud(look, c, eye, clock);
    }
    for a in &fire.auras {
        crate::elemfx::aura(look, a, eye, clock);
    }
    for b in &fire.beams {
        crate::elemfx::beam(look, b, eye, clock);
    }
    for c in &fire.carpets {
        crate::elemfx::carpet(look, c, eye, clock);
    }
    for m in &fire.motes {
        crate::elemfx::mote(look, m, eye, clock);
    }
}

/// A shot or the charge.
fn ball(look: &mut Look, o: &Orb, eye: &Eye, clock: f32, tick: u32) {
    let level = o.level.min(SUN);
    let style = &STYLE[level as usize];
    let giant = level >= GIANT;
    let charge = o.flags & ORB_CHARGE != 0;
    let moving = !charge && o.flags & ORB_STUCK == 0 && o.vel.length() > 0.5;
    let reflected = o.flags & ORB_REFLECTED != 0;
    let vis = o.vis.clamp(0.0, 1.0);
    let c = o.pos;
    let r = o.size * if giant { 0.5 } else { 0.4 };
    // (the way back: its flames stream that way; held still, they rise)
    let tail = if moving { (-o.vel).normalize_or(Vec3::Y) } else { Vec3::Y };
    let dist = (c - eye.pos).length().max(0.01);
    let px = r * eye.px / dist;
    // (the render's clock, smooth between ticks; each ball a little out of step)
    let t = clock + level as f32 * 0.37;

    // the camera inside it: all fire; the big ones warm the screen as they fill it
    if dist < r * 1.15 {
        look.inside = look.inside.max((1.0 - dist / (r * 1.15)) * vis);
    }
    if giant {
        let share = px / eye.screen.max(1.0);
        let k = if level >= SUN { 1.0 } else { 0.45 };
        look.heat = look.heat.max(((share - 0.12) * 1.6).clamp(0.0, 1.0) * k * vis);
    }

    // SM64's flames round it (and, too small on screen for faces, the ball itself)
    let faces = px >= 4.0;
    let first = look.sprites.len();
    if giant {
        giant_sprites(look, c, tail, o.size, o.age, tick, faces);
    } else {
        ball_sprites(look, c, tail, o.vel, o.size, level, o.age, tick, faces);
    }
    if charge {
        charge_sparks(look, c, o.size, level, tick);
    }
    for s in &mut look.sprites[first..] {
        s.tint[3] *= vis;
        s.tint = blue(s.tint, reflected);
    }
    if !faces {
        return;
    }
    // (far off: fewer faces)
    let lod = if px < 30.0 { 2 } else { 1 };
    let (rings, slices) = ((style.rings / lod).max(3), (style.slices / lod).max(5));
    let spin = Quat::from_axis_angle(tail.any_orthonormal_vector(), t * if giant { 0.7 } else { 2.2 }) * Quat::from_rotation_y(t * 0.9);
    let flow = Vec3::new(0.0, -t * 0.8, t * 0.3);

    // its light on the ground under it
    if let Some(g) = o.ground.filter(|_| level >= 3) {
        let over = (c.y - r - g).max(0.0);
        let near = 1.0 - over / (r * 4.0);
        if near > 0.0 {
            let at = Vec3::new(c.x, g + 0.06, c.z);
            let reach = r * (1.4 + 1.8 * near);
            let col = blue(rgba(heat(0.62), 0.26 * near * vis), reflected);
            band(&mut look.under, at, Vec3::X, Vec3::Z, 0.0, reach * 0.55, 16, &|_| 1.0, col);
            band(&mut look.under, at, Vec3::X, Vec3::Z, reach * 0.55, reach, 16, &|_| 1.0, [col[0], col[1], col[2], col[3] * 0.45]);
        }
    }
    // rays of light round its rim (across the view, behind its heart)
    if style.rays > 0 {
        let (u, v) = across(eye.pos, c);
        let n = (style.rays / lod).max(3);
        let reach = if level >= SUN { 1.7 } else if giant { 1.15 } else { 0.8 };
        for i in 0..n {
            let a = i as f32 * TAU / n as f32 + t * 0.25 * if i % 2 == 0 { 1.0 } else { -0.7 };
            let len = r * (0.25 + 0.5 * flick(i, t * 1.7)) * reach;
            let w = 0.1 + 0.04 * (i % 3) as f32;
            let at = |a: f32, d: f32| c + (u * a.cos() + v * a.sin()) * d;
            let col = blue(rgba([1.0, 0.86, 0.45], 0.3 * vis), reflected);
            look.tris.push(Tri { p: [at(a - w, r * 0.92), at(a + w, r * 0.92), at(a, r + len)], col });
        }
    }
    // the heat rippling off it
    if style.ripples > 0 {
        let (u, v) = across(eye.pos, c);
        for i in 0..style.ripples {
            let phase = (t * 0.55 + i as f32 / style.ripples as f32).fract();
            let rad = r * (1.12 + 0.95 * phase);
            let wob = move |a: f32| 1.0 + 0.045 * (a * 5.0 + t * 7.0 + i as f32).sin();
            let col = blue(rgba([1.0, 0.92, 0.75], 0.2 * (1.0 - phase) * vis), reflected);
            band(&mut look.tris, c, u, v, rad, rad + r * (0.035 + 0.05 * phase), 28 / lod, &wob, col);
        }
    }
    // the tail: a tube of fire tapering off behind it, cooling
    if moving && style.tail > 0 {
        let n = style.tail;
        let step = r * 0.6 * (1.0 + level as f32 * 0.1);
        let side = tail.any_orthonormal_vector();
        let up = tail.cross(side);
        let mut pts = [Vec3::ZERO; 8];
        for (k, p) in pts.iter_mut().enumerate().take(n + 1) {
            let sway = (side * (t * 9.0 + k as f32).sin() + up * (t * 7.0 + k as f32 * 1.3).cos()) * r * 0.08 * k as f32 / n as f32;
            *p = c + tail * (r * 0.3 + step * k as f32) + Vec3::Y * (k as f32 / n as f32).powi(2) * r * 0.4 + sway;
        }
        tube(look, &pts[..=n], if lod == 1 { 6 } else { 4 }, eye.pos, &|k| r * 0.85 * (1.0 - k as f32 / (n + 1) as f32), &|k| {
            let f = k as f32 / n as f32;
            blue(rgba(heat(0.78 - 0.45 * f), 0.5 * (1.0 - f) * vis), reflected)
        });
    }
    // tongues of flame licking off it
    let tongues = (style.tongues / lod).max(3);
    for i in 0..tongues {
        let dir = spin * spread(i, tongues);
        let axis = (dir + tail * if moving { 1.1 } else { 0.5 } + Vec3::Y * 0.3).normalize_or(dir);
        let len = r * (0.4 + 0.5 * flick(i, t * 2.6)) * (1.0 + 0.12 * level as f32);
        let hot = flick(i + 40, t * 4.0);
        let col = blue(rgba([1.0, 0.68 + 0.22 * hot, 0.22 + 0.2 * hot], 0.82 * vis), reflected);
        cone(look, c + dir * r * 0.8, axis, len, r * 0.3, col);
    }
    // its heart
    let sun = level >= SUN;
    sphere(look, c, r, rings, slices, false, eye.pos, spin, &|d, f, _| {
        let n = fbm(d * if sun { 3.6 } else { 2.3 } + flow);
        let h = if sun {
            // (plasma: white-hot where it faces you, orange at its edge, a grain churning)
            0.5 + 0.45 * f.powf(0.6) + 0.14 * (n - 0.5)
        } else if giant {
            // (magma: crust plates, glowing cracks between, lava welling up)
            match n {
                n if n < 0.46 => 0.1 + 0.14 * f + 0.25 * (n / 0.46),
                n if n < 0.51 => 0.93,
                n => 0.6 + 0.22 * f + (n - 0.51) * 0.7,
            }
        } else {
            0.6 + 0.04 * level as f32 + 0.28 * f + 0.32 * (n - 0.5)
        };
        blue(rgba(heat(h), vis), reflected)
    });
    // the shell of flame round it, glowing at its rim (the sun's corona round that)
    if style.shell > 0.0 {
        let a = style.shell;
        let wob = 1.16 + 0.04 * (t * 5.0).sin();
        sphere(look, c, r * wob, rings, slices, false, eye.pos, spin, &|d, f, _| {
            let n = fbm(d * 1.7 - flow);
            blue(rgba(heat(0.6 + 0.25 * n), a * (1.0 - f).powf(1.4) * (0.6 + 0.8 * n) * vis), reflected)
        });
        if sun && lod == 1 {
            sphere(look, c, r * 1.4, rings - 2, slices - 2, false, eye.pos, spin, &|_, f, _| rgba([1.0, 0.8, 0.4], 0.32 * (1.0 - f).powi(2) * vis));
        }
    }
    // the sun's arcs of fire, looping off its face
    if sun && lod == 1 {
        let slow = Quat::from_rotation_y(t * 0.2);
        for a in 0..3 {
            let d1 = slow * spread(a * 3 + 1, 9);
            let turn = Quat::from_axis_angle(d1.any_orthonormal_vector(), 0.55 + 0.1 * a as f32);
            let d2 = turn * d1;
            let lift = 0.3 * (0.8 + 0.4 * (t * 1.3 + a as f32).sin());
            let mut pts = [Vec3::ZERO; 8];
            for (k, p) in pts.iter_mut().enumerate() {
                let s = k as f32 / 7.0;
                let d = d1.lerp(d2, s).normalize_or(d1);
                *p = c + d * r * (0.97 + lift * (PI * s).sin());
            }
            tube(look, &pts, 4, eye.pos, &|k| r * 0.07 * (1.0 - (k as f32 / 7.0 - 0.5).abs()), &|_| rgba(heat(0.84), 0.9 * vis));
        }
    }
}

/// A shot's (or the charge's) SM64 flames: its glow, the white-hot middle (the ball itself when
/// too small for faces), flames licking round it, a tail of flames shed behind it (`tail`: the way
/// back), cooling as they go (yellow, orange, red, a wisp of smoke), and embers flying off.
#[allow(clippy::too_many_arguments)]
fn ball_sprites(look: &mut Look, pos: Vec3, tail: Vec3, vel: Vec3, size: f32, level: u8, age: u32, tick: u32, faces: bool) {
    let out = &mut look.sprites;
    let l = level.min(3) as usize;
    let flicker = 1.0 + 0.07 * (tick as f32 * 2.3).sin();
    let hot = tint(level, tick);
    out.push(Sprite { pos, size: size * 2.3 * flicker, frame: FRAME_GLOW, tint: [hot[0], hot[1] * 0.75, hot[2] * 0.6, 0.28] });
    out.push(Sprite { pos, size: size * 1.45 * flicker, frame: FRAME_GLOW, tint: [1.0, 0.68, 0.28, 0.5] });
    if !faces {
        out.push(Sprite { pos, size: size * 0.8, frame: FRAME_ORB + (age / 2) as usize % 4, tint: [1.0, 1.0, 1.0, 1.0] });
        out.push(Sprite { pos, size: size * 0.5, frame: (age / 2 + 4) as usize % 8, tint: [1.0, 0.97, 0.82, 0.9] });
    }
    // (a frame about the way back: the licks turn round it; with faces, fewer: its tongues lick too)
    let axis = tail.normalize_or(Vec3::Y);
    let u = axis.any_orthonormal_vector();
    let w = axis.cross(u);
    let licks = [6, 7, 8, 10][l] / if faces { 2 } else { 1 };
    for i in 0..licks {
        let a = i as f32 * TAU / licks as f32 + age as f32 * 0.5;
        let lat = (i as f32 * 2.399).sin() * 0.7;
        let dir = (u * a.cos() + w * a.sin()) * lat.cos() + axis * (lat.sin() * 0.6);
        let reach = size * (0.36 + 0.06 * ((age + i) as f32 * 1.7).sin());
        out.push(Sprite {
            pos: pos + dir * reach + axis * size * 0.18,
            size: size * (0.38 + 0.08 * (i % 3) as f32),
            frame: (i as usize + age as usize / 2) % 8,
            tint: [1.0, 0.72 + 0.1 * (i % 2) as f32, 0.32, 0.88],
        });
    }
    // the tail: older flames farther back, smaller, cooler, rising a little and swaying
    let step = (vel.length() / 30.0).max(size * 0.3);
    let tail_n = [5, 6, 7, 8][l];
    for k in 1..=tail_n {
        let t = k as f32 / tail_n as f32;
        let sway = (u * ((age + k) as f32 * 0.9).sin() + w * ((age + k) as f32 * 1.3).cos()) * size * 0.12 * t;
        let p = pos + axis * step * k as f32 * 0.8 + Vec3::Y * t * t * size * 0.5 + sway;
        let c = [1.0, 0.85 - 0.55 * t, 0.45 - 0.37 * t, 0.85 * (1.0 - t) + 0.1];
        out.push(Sprite { pos: p, size: size * (0.85 - 0.5 * t), frame: (age as usize + k as usize * 3) % 8, tint: c });
    }
    if l >= 2 {
        let p = pos + axis * step * (tail_n as f32 + 1.5) * 0.8 + Vec3::Y * size * 0.5;
        out.push(Sprite { pos: p, size: size * 0.9, frame: FRAME_GLOW, tint: [0.12, 0.08, 0.06, 0.25] });
    }
    // embers
    for i in 0..(3 + 2 * l as u32) {
        let t = ((age + i * 7) % 18) as f32 / 18.0;
        let a = i as f32 * 2.399;
        let side = (u * a.cos() + w * a.sin()) * size * 0.6 * (0.4 + t);
        out.push(Sprite {
            pos: pos + axis * step * t * 4.0 + side + Vec3::Y * t * size * 0.7,
            size: 0.05 + 0.04 * (1.0 - t) * (1.0 + l as f32 * 0.5),
            frame: FRAME_GLOW,
            tint: [1.0, 0.85 - 0.35 * t, 0.4 - 0.25 * t, 1.0 - t],
        });
    }
}

/// The giant shot's (or its charge's) SM64 flames: a smudge of smoke behind it, its glow, flames
/// round its rim, embers shed behind it drifting up (the ball itself when too small for faces).
#[allow(clippy::too_many_arguments)]
fn giant_sprites(look: &mut Look, pos: Vec3, back: Vec3, size: f32, age: u32, tick: u32, faces: bool) {
    let out = &mut look.sprites;
    let flicker = 1.0 + 0.06 * (tick as f32 * 2.1).sin();
    for i in 0..5u32 {
        let t = ((age + i * 4) % 20) as f32 / 20.0;
        let off = back * size * (0.4 + t * 1.6) + Vec3::Y * t * size * 0.6;
        out.push(Sprite { pos: pos + off, size: size * (0.6 + t * 0.8), frame: FRAME_GLOW, tint: [0.12, 0.08, 0.06, 0.35 * (1.0 - t)] });
    }
    out.push(Sprite { pos, size: size * 2.0 * flicker, frame: FRAME_GLOW, tint: [1.0, 0.45, 0.1, 0.35] });
    out.push(Sprite { pos, size: size * 1.4 * flicker, frame: FRAME_GLOW, tint: [1.0, 0.65, 0.25, 0.55] });
    if !faces {
        out.push(Sprite { pos, size, frame: FRAME_ORB + (age / 3) as usize % 4, tint: [1.0, 1.0, 1.0, 1.0] });
    }
    for i in 0..8u32 {
        let a = i as f32 * TAU / 8.0 + age as f32 * 0.35;
        let off = Vec3::new(a.cos(), (a * 1.3).sin() * 0.7, a.sin()) * size * 0.46;
        out.push(Sprite { pos: pos + off, size: size * 0.3, frame: (i + age) as usize % 8, tint: [1.0, 0.7, 0.3, 0.85] });
    }
    for i in 0..16u32 {
        let t = ((age + i * 5) % 16) as f32 / 16.0;
        let a = i as f32 * 2.399;
        let side = Vec3::new(a.cos(), (a * 0.7).sin(), a.sin()) * size * 0.5;
        out.push(Sprite {
            pos: pos + back * size * t * 2.0 + side * (1.0 + t) + Vec3::Y * t * size * 0.8,
            size: 0.07 + 0.08 * (1.0 - t),
            frame: FRAME_GLOW,
            tint: [1.0, 0.8 - 0.3 * t, 0.35 - 0.2 * t, 1.0 - t],
        });
    }
}

/// The charge: sparks of flame gathering at his hand (more, faster with each level).
fn charge_sparks(look: &mut Look, hand: Vec3, size: f32, level: u8, tick: u32) {
    let (count, period) = [(4, 10), (6, 8), (8, 7), (10, 6), (18, 5), (28, 4)][level.min(SUN) as usize];
    for i in 0..count {
        let phase = ((tick + i * period / count) % period) as f32 / period as f32;
        let angle = i as f32 * 2.399 + (tick / period) as f32 * 0.9;
        let reach = (1.1 * (1.0 - phase) + 0.1) * size.max(1.0);
        let off = Vec3::new(angle.cos() * reach, (angle * 1.7).sin() * reach * 0.6, angle.sin() * reach);
        look.sprites.push(Sprite {
            pos: hand + off,
            size: 0.12 + 0.1 * level as f32 * phase,
            frame: (i as usize + tick as usize) % 8,
            tint: {
                let mut c = tint(level, tick + i);
                c[3] = 0.35 + 0.65 * phase;
                c
            },
        });
    }
}

/// A burst: its SM64 flames (a ring of them, the big ones' column of fire), a flash of fire
/// swelling; the big ones' dome of fire and shock wave over the ground and smoke billowing up.
fn blast(look: &mut Look, b: &crate::fx::Blast, eye: &Eye, tick: u32) {
    let level = b.level.min(SUN);
    let vis = b.vis.clamp(0.0, 1.0);
    let flames = burst_ticks(level);
    let life = blast_life(level);
    let size = b.size;
    let dist = (b.pos - eye.pos).length().max(0.01);
    if b.age < flames {
        blast_sprites(look, b.pos, level, size, b.age as f32 / flames as f32, tick, vis);
    }
    let reach = if level >= GIANT { EXPLODE_REACH[big(level)] * size / if level >= SUN { 7.0 } else { 4.2 } } else { size };
    if reach * eye.px / dist < 4.0 {
        return;
    }
    let lod = if reach * eye.px / dist < 40.0 { 2 } else { 1 };
    let spin = Quat::from_rotation_y(b.age as f32 * 0.05);
    // the flash: a ball of fire swelling and fading
    let flash = (flames as f32 * 0.45).max(3.0);
    if (b.age as f32) < flash && level > 0 {
        let e = b.age as f32 / flash;
        let r = size * (0.35 + 1.0 * e) * if level >= GIANT { 1.3 } else { 1.0 };
        let (rings, slices) = if level >= GIANT { (7 / lod, 12 / lod) } else { (4, 7) };
        sphere(look, b.pos, r, rings.max(3), slices.max(5), false, eye.pos, spin, &|_, f, _| rgba(heat(1.0 - 0.35 * e), (1.0 - e).powi(2) * 0.85 * (0.4 + 0.6 * f) * vis));
    }
    // (the flames at his hand as a big one leaves: no explosion)
    if level < GIANT || b.flags & BLAST_LAUNCH != 0 {
        return;
    }
    // (over the ground, if it's there: a dome; in the air, a ball)
    let ground = b.ground.filter(|g| b.pos.y - g < 3.0);
    let at = ground.map_or(b.pos, |g| Vec3::new(b.pos.x, g, b.pos.z));
    let half = ground.is_some();
    let e = (b.age as f32 / (life as f32 * 0.55)).min(1.0);
    let ease = 1.0 - (1.0 - e).powi(3);
    if e < 1.0 {
        sphere(look, at, reach * (0.15 + 0.95 * ease), 6 / lod, 16 / lod, half, eye.pos, spin, &|_, f, _| rgba([1.0, 0.82, 0.5], 0.6 * (1.0 - e).powf(1.3) * (1.0 - f).powf(1.2) * vis));
    }
    // (the sun's shock wave: thin, white, racing out ahead)
    if level >= SUN {
        let e2 = (b.age as f32 / (life as f32 * 0.3)).min(1.0);
        if e2 < 1.0 {
            let ease2 = 1.0 - (1.0 - e2).powi(2);
            sphere(look, at, reach * 1.55 * ease2, 6 / lod, 18 / lod, half, eye.pos, spin, &|_, f, _| rgba([1.0, 0.98, 0.92], 0.5 * (1.0 - e2).powi(2) * (1.0 - f).powf(2.5) * vis));
        }
    }
    // smoke billowing up (the sun's: a column under a cap, a mushroom), lit by the fire beneath
    let sun = level >= SUN;
    let n = if sun { 13 } else { 7 };
    let e = b.age as f32 / life as f32;
    let up = reach * if sun { 1.5 } else { 0.9 };
    // (by where it is to 2 m: the same smoke frame to frame, a partner's moving a little too)
    let seed = (b.pos.x * 0.5).floor() as i32 * 7 + (b.pos.z * 0.5).floor() as i32 * 3;
    for i in 0..n {
        let rnd = hash(seed, i as i32, 7);
        let (p, r) = if sun && i < 5 {
            (at + Vec3::Y * up * (0.15 + 0.17 * i as f32) * (0.35 + 0.65 * ease), reach * (0.14 + 0.03 * i as f32) * (0.6 + 0.6 * e))
        } else {
            let k = if sun { i - 5 } else { i };
            let a = k as f32 * TAU / (n - if sun { 5 } else { 0 }) as f32 + rnd;
            let out = reach * if sun { 0.5 } else { 0.55 + 0.25 * rnd } * (0.3 + 0.7 * ease);
            let h = if sun { up * (0.9 + 0.2 * rnd) } else { up * (0.2 + 0.5 * rnd) * (0.4 + 0.6 * e) };
            (at + Vec3::new(a.cos() * out, h * (0.35 + 0.65 * ease), a.sin() * out), reach * (0.16 + 0.06 * rnd) * (0.6 + 0.7 * e))
        };
        let fade = (1.0 - e).powf(1.2) * (e * 8.0).min(1.0) * vis;
        let fire = (1.0 - e).powi(2);
        sphere(look, p, r, 4, 7, false, eye.pos, Quat::IDENTITY, &|_, _, nrm| {
            let lit = 0.55 + 0.45 * nrm.y.max(0.0);
            let under = fire * (-nrm.y).max(0.0) * 0.9;
            [0.3 * lit + under, 0.27 * lit + under * 0.45, 0.25 * lit + under * 0.12, 0.82 * fade]
        });
    }
}

/// A burst's SM64 flames (`t` 0-1 through them): a flame, a ring of them; the big ones' glow, a
/// ring of sparks and a column of fire rising (the sun's white-hot core).
fn blast_sprites(look: &mut Look, pos: Vec3, level: u8, size: f32, age: f32, tick: u32, vis: f32) {
    let out = &mut look.sprites;
    let l = level as usize;
    let spread = size * (0.4 + 1.2 * age);
    let mut c = tint(level, tick);
    c[3] = (1.0 - age) * vis;
    out.push(Sprite { pos, size: size * (1.2 - 0.6 * age), frame: (age * 8.0) as usize % 8, tint: c });
    let ring = [6, 16, 24][(level >= GIANT) as usize + big(level)];
    if level >= GIANT {
        out.push(Sprite { pos, size: size * (1.5 + 2.0 * age), frame: FRAME_GLOW, tint: [1.0, 0.55, 0.15, 0.7 * (1.0 - age) * vis] });
        for i in 0..14 {
            let a = i as f32 * 2.399;
            let dir = Vec3::new(a.cos(), 0.4 + 0.6 * ((a * 3.1).sin() * 0.5 + 0.5), a.sin());
            out.push(Sprite { pos: pos + dir * spread * 1.4, size: 0.18, frame: FRAME_GLOW, tint: [1.0, 0.85, 0.4, (1.0 - age) * vis] });
        }
        if level >= SUN {
            out.push(Sprite { pos, size: size * (0.9 - 0.6 * age), frame: FRAME_GLOW, tint: [1.0, 0.97, 0.85, 0.9 * (1.0 - age) * vis] });
        }
        for i in 0..[8, 12][big(level)] {
            let up = Vec3::Y * spread * (0.2 + 0.25 * i as f32) * (0.5 + age);
            out.push(Sprite { pos: pos + up, size: size * 0.5 * (1.0 - age), frame: (i + (age * 8.0) as usize) % 8, tint: c });
        }
    }
    for i in 0..ring {
        let a = i as f32 * TAU / ring as f32 + l as f32;
        let off = Vec3::new(a.cos(), 0.5 + 0.4 * (a * 2.0).sin(), a.sin()) * spread * 0.6;
        out.push(Sprite { pos: pos + off, size: size * 0.45 * (1.0 - age * 0.5), frame: (i + (age * 8.0) as usize) % 8, tint: c });
    }
}

/// A lava pool: a dark red pool glowing, small flames licking up from it and sparks rising, dimming
/// in its last half second.
fn pool(look: &mut Look, e: &crate::fx::Pool, tick: u32) {
    let out = &mut look.sprites;
    let age = e.age;
    let fade = ((e.life - age.min(e.life)) as f32 / 15.0).min(1.0) * e.vis.clamp(0.0, 1.0);
    let n = (((e.pos.x * 0.5).floor() as i32 * 13 + (e.pos.z * 0.5).floor() as i32 * 7).unsigned_abs()) % 17;
    let throb = 1.0 + 0.08 * ((tick + n * 5) as f32 * 0.5).sin();
    let r = e.radius;
    out.push(Sprite { pos: e.pos + Vec3::Y * 0.12, size: r * 2.4 * throb, frame: FRAME_GLOW, tint: [0.85, 0.18, 0.04, 0.55 * fade] });
    out.push(Sprite { pos: e.pos + Vec3::Y * 0.18, size: r * 1.3 * throb, frame: FRAME_GLOW, tint: [1.0, 0.5, 0.12, 0.7 * fade] });
    for i in 0..3u32 {
        let t = ((age + i * 7 + n * 3) % 20) as f32 / 20.0;
        let a = (i + n) as f32 * 2.399;
        let off = Vec3::new(a.cos(), 0.0, a.sin()) * r * 0.55 + Vec3::Y * (0.25 + t * 0.8);
        out.push(Sprite { pos: e.pos + off, size: 0.55 * (1.0 - t * 0.6) * fade, frame: (age as usize / 2 + i as usize * 3) % 8, tint: [1.0, 0.75 - 0.3 * t, 0.3, 0.85 * (1.0 - t) * fade] });
    }
    for i in 0..2u32 {
        let t = ((age + i * 11 + n * 7) % 26) as f32 / 26.0;
        let a = (i * 3 + n) as f32 * 1.7;
        let off = Vec3::new(a.cos(), 0.0, a.sin()) * r * 0.7 * (0.3 + t) + Vec3::Y * (0.3 + t * 1.8);
        out.push(Sprite { pos: e.pos + off, size: 0.07, frame: FRAME_GLOW, tint: [1.0, 0.85, 0.4, (1.0 - t) * fade] });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A way out of a ball is a way out of it whatever the index (the lightning charge's arcs
    /// asked for the 120th of 23: a "way" ten times long, straight down: the line under his charge).
    #[test]
    fn every_spread_way_is_a_unit_way() {
        for n in [5usize, 19, 23, 29] {
            for i in 0..400 {
                let v = spread(i, n);
                assert!((v.length() - 1.0).abs() < 1e-3, "spread({i}, {n}) = {v}");
            }
        }
    }
    use crate::fx::{Blast, Orb};

    fn eye() -> Eye {
        Eye { pos: Vec3::new(0.0, 2.0, -20.0), px: 900.0, screen: 1080.0 }
    }

    fn orb(level: u8, size: f32) -> Orb {
        Orb { pos: Vec3::new(0.0, 2.0, 0.0), vel: Vec3::Z * 30.0, size, level, age: 12, flags: 0, ground: Some(0.0), vis: 1.0, element: 0 }
    }

    #[test]
    fn each_level_grander_than_the_last() {
        let mut look = Look::default();
        let mut last = 0;
        for (level, size) in [0.5, 0.8, 1.15, 2.0, 4.2, 7.0].into_iter().enumerate() {
            look.clear();
            build(&mut look, &FireFx { orbs: vec![orb(level as u8, size)], ..Default::default() }, &eye(), 1.0);
            println!("flame test: level {level}, {} faces, {} sprites", look.tris.len(), look.sprites.len());
            assert!(look.tris.len() > last, "level {level}: {} faces after {last}", look.tris.len());
            last = look.tris.len();
            assert!(look.tris.iter().all(|t| t.p.iter().all(|p| p.is_finite()) && t.col.iter().all(|c| c.is_finite())));
        }
        // (the sun near the camera: a few hundred faces, not thousands)
        assert!(last < 2500, "sun {last}");
    }

    #[test]
    fn far_off_its_faces_give_way_to_a_sprite() {
        let mut look = Look::default();
        let far = Eye { pos: Vec3::new(0.0, 2.0, -3000.0), ..eye() };
        build(&mut look, &FireFx { orbs: vec![orb(1, 0.8)], ..Default::default() }, &far, 1.0);
        assert!(look.tris.is_empty());
        assert!(look.sprites.iter().any(|s| s.frame >= FRAME_ORB));
    }

    #[test]
    fn inside_the_sun_it_is_all_fire() {
        let mut look = Look::default();
        let inside = Eye { pos: Vec3::new(0.0, 2.5, 0.5), ..eye() };
        build(&mut look, &FireFx { orbs: vec![orb(SUN, 7.0)], ..Default::default() }, &inside, 1.0);
        assert!(look.inside > 0.5, "inside {}", look.inside);
        look.clear();
        build(&mut look, &FireFx { orbs: vec![orb(SUN, 7.0)], ..Default::default() }, &Eye { pos: Vec3::new(0.0, 2.0, -8.0), ..eye() }, 1.0);
        assert!(look.heat > 0.3, "heat {}", look.heat);
    }

    #[test]
    fn the_suns_blast_has_its_dome_and_smoke_to_the_end() {
        let mut look = Look::default();
        for age in [0, 10, 40, 90] {
            look.clear();
            let b = Blast { pos: Vec3::new(0.0, 1.0, 0.0), level: SUN, age, size: 7.0, ground: Some(0.0), flags: 0, vis: 1.0, element: 0 };
            build(&mut look, &FireFx { blasts: vec![b], ..Default::default() }, &eye(), 1.0);
            assert!(!look.tris.is_empty(), "age {age}");
        }
    }
}

/// The rubble a heavy blow throws up (impact.rs): each chunk a lumpy rock of eight faces (an
/// octahedron, each corner pushed in or out by its seed), lit from above, earth-coloured, only its
/// faces turned to the eye.
pub fn rubble(look: &mut Look, rocks: &[crate::fx::Rock], eye: Vec3) {
    const LIGHT: Vec3 = Vec3::new(0.3, 0.9, 0.25);
    const FACES: [[usize; 3]; 8] = [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];
    let light = LIGHT.normalize();
    for r in rocks {
        if r.size < 0.01 {
            continue;
        }
        // (each corner its own way out: a lump, not a gem)
        let lump = |k: u32| 0.65 + 0.6 * ((r.seed.wrapping_mul(2654435761).wrapping_add(k.wrapping_mul(40503)) >> 7) % 1000) as f32 / 1000.0;
        let corners = [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z, Vec3::Y * 0.8, -Vec3::Y * 0.7];
        let p: Vec<Vec3> = corners.iter().enumerate().map(|(k, c)| r.pos + r.rot * (*c * r.size * lump(k as u32))).collect();
        // (earth: a little darker or lighter, warmer or greyer by its seed)
        let tone = 0.85 + 0.3 * (r.seed % 97) as f32 / 97.0;
        let base = [0.46 * tone, 0.40 * tone, 0.33 * tone];
        for f in FACES {
            let (a, b, c) = (p[f[0]], p[f[1]], p[f[2]]);
            // (out from its middle, whichever way round the corners go)
            let n = (b - a).cross(c - a).normalize_or_zero();
            let n = if n.dot(a + b + c - r.pos * 3.0) < 0.0 { -n } else { n };
            if n.dot(eye - a) <= 0.0 {
                continue;
            }
            let lit = 0.45 + 0.55 * n.dot(light).max(0.0);
            look.tris.push(Tri { p: [a, b, c], col: [base[0] * lit, base[1] * lit, base[2] * lit, 1.0] });
        }
    }
}
