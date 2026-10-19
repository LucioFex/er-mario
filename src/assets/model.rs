//! Mario's SM64 model, taken from libsm64 (so from the player's ROM): a Mario standing on a flat
//! floor gives the mesh (per-triangle body part, part-local positions and normals, UVs, colours),
//! and the same Mario in his star dance gives the peace-sign hand.

use crate::hammer::HEAD_AT;
use crate::sm64;

pub const PEACE_FRAME: usize = 55;
const ACT_STAR_DANCE_NO_EXIT: u32 = 0x1307;
/// the peace-sign hand is SM64 part 9 (right hand)
const RIGHT_HAND: i32 = 9;
/// the head (the wing cap's wings go on it)
const HEAD: usize = 3;
/// SM64's cap flags: normal cap, wing cap, cap on the head
const NORMAL_CAP: u32 = 0x1 | 0x10;
const WING_CAP: u32 = 0x8 | 0x10;

#[derive(Clone, Copy)]
pub struct Tri {
    pub part: i32,
    pub local: [[f32; 3]; 3],
    pub normal: [[f32; 3]; 3],
    pub uv: [[f32; 2]; 3],
    pub color: [[f32; 3]; 3],
    pub world: [[f32; 3]; 3],
}

pub struct MarioModel {
    pub tris: Vec<Tri>,
    /// per-part 4x4 row-vector matrices (world = [local, 1] * M)
    pub matrices: Vec<[f32; 16]>,
    pub peace: Vec<Tri>,
    /// the wing cap's two wings, on the head (part-local to it)
    pub wings: Vec<Tri>,
    /// Mario's hammer (hammer.rs), on a part of its own
    pub hammer: Vec<Tri>,
    /// the boomerangs, the Chain Chomp and its chain (gear.rs), each on a part of its own
    pub gear: Vec<Tri>,
    /// SM64's texture atlas (11 textures of 64x64, RGBA)
    pub atlas: Vec<u8>,
}

struct Capture {
    mats: Vec<f32>,
    part: Vec<i32>,
    local: Vec<f32>,
    normal: Vec<f32>,
}

fn capture() -> (Capture, usize) {
    let mut c = Capture {
        mats: vec![0.0; 64 * 16],
        part: vec![0; sm64::GEO_MAX_TRIANGLES],
        local: vec![0.0; sm64::GEO_MAX_TRIANGLES * 9],
        normal: vec![0.0; sm64::GEO_MAX_TRIANGLES * 9],
    };
    let parts = unsafe { sm64::sm64_er_get_parts(c.mats.as_mut_ptr(), c.part.as_mut_ptr(), c.local.as_mut_ptr(), c.normal.as_mut_ptr()) };
    (c, parts.max(0) as usize)
}

fn tris(c: &Capture, geo: &sm64::Geometry, filter: impl Fn(i32) -> bool) -> Vec<Tri> {
    let v3 = |a: &[f32], t: usize, k: usize| [a[t * 9 + k * 3], a[t * 9 + k * 3 + 1], a[t * 9 + k * 3 + 2]];
    (0..geo.used())
        .filter(|&t| filter(c.part[t]))
        .map(|t| Tri {
            part: c.part[t],
            local: [0, 1, 2].map(|k| v3(&c.local, t, k)),
            normal: [0, 1, 2].map(|k| v3(&c.normal, t, k)),
            uv: [0, 1, 2].map(|k| [geo.uv[t * 6 + k * 2], geo.uv[t * 6 + k * 2 + 1]]),
            color: [0, 1, 2].map(|k| v3(&geo.color, t, k)),
            world: [0, 1, 2].map(|k| v3(&geo.position, t, k)),
        })
        .collect()
}

/// Runs on the libsm64 thread, after sm64_global_init and before the audio starts (the star
/// dance queues its jingle). Leaves no Mario and no surfaces behind that matter: the game's own
/// surfaces replace the test floor when Mario mode starts.
pub fn export(geo: &mut sm64::Geometry, atlas: Vec<u8>) -> Option<MarioModel> {
    let e = 8000;
    let floor = [
        sm64::SM64Surface::grass([[-e, 0, -e], [e, 0, e], [e, 0, -e]]),
        sm64::SM64Surface::grass([[-e, 0, -e], [-e, 0, e], [e, 0, e]]),
    ];
    unsafe { sm64::sm64_static_surfaces_load(floor.as_ptr(), floor.len() as u32) };
    let id = unsafe { sm64::sm64_mario_create(0.0, 0.0, 0.0) };
    if id < 0 {
        return None;
    }
    let inputs = sm64::SM64MarioInputs::default();
    let mut state = sm64::SM64MarioState::default();
    let mut tick = |geo: &mut sm64::Geometry| {
        let mut b = geo.buffers();
        unsafe { sm64::sm64_mario_tick(id, &inputs, &mut state, &mut *b) };
    };
    for _ in 0..3 {
        tick(geo);
    }
    let (c, parts) = capture();
    let model_tris = tris(&c, geo, |_| true);
    let matrices = (0..parts).map(|p| c.mats[p * 16..p * 16 + 16].try_into().unwrap()).collect();
    // the wing cap: its wings are drawn last (SM64's see-through layer), each with its own matrix,
    // so the body parts keep their numbers; they're moved onto the head
    unsafe { sm64::sm64_er_set_cap_flags(id, WING_CAP) };
    tick(geo);
    let (c, wing_parts) = capture();
    let inv = invert(&c.mats[HEAD * 16..HEAD * 16 + 16]);
    let wings = tris(&c, geo, |p| p as usize >= parts && (p as usize) < wing_parts)
        .into_iter()
        .map(|mut t| {
            let own: [f32; 16] = c.mats[t.part as usize * 16..t.part as usize * 16 + 16].try_into().unwrap();
            t.part = HEAD as i32;
            for k in 0..3 {
                t.local[k] = transform(&inv, t.world[k], 1.0);
                let n = transform(&inv, transform(&own, t.normal[k], 0.0), 0.0);
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                t.normal[k] = n.map(|v| v / l);
            }
            t
        })
        .collect();
    unsafe { sm64::sm64_er_set_cap_flags(id, NORMAL_CAP) };
    // the star dance (the same Mario two ticks later, like a fresh one after five)
    tick(geo);
    unsafe { sm64::sm64_set_mario_action(id, ACT_STAR_DANCE_NO_EXIT) };
    for _ in 0..=PEACE_FRAME {
        tick(geo);
    }
    let (c, _) = capture();
    let peace = tris(&c, geo, |p| p == RIGHT_HAND);
    unsafe { sm64::sm64_mario_delete(id) };
    let hammer = hammer(&model_tris, &atlas);
    let gear = super::gear::build(&model_tris, &atlas);
    Some(MarioModel { tris: model_tris, matrices, peace, wings, hammer, gear, atlas })
}

/// The hammer's part (engine_mario::HAMMER)
pub const HAMMER_PART: i32 = crate::engine_mario::HAMMER as i32;
/// SM64 units per metre in the mesh (flver.rs: 0.01 m times Mario's 0.25 model scale)
const PER_METRE: f32 = 400.0;

/// Mario's hammer, part-local to its own bone, in metres: the grip at the origin, the handle up
/// +Y, the head across its top along Z (hammer::HEAD_AT). Big and heavy, its head solid and dark:
/// the darkest opaque patch of the ROM's own textures (the moustache), its bands and handle in the
/// darkest of Mario's colours (the albedo has a band for each, and no room for another).
fn hammer(model: &[Tri], atlas: &[u8]) -> Vec<Tri> {
    let nearest = |c: [f32; 3]| {
        let d = |t: &Tri| (0..3).map(|k| (t.color[0][k] - c[k]).powi(2)).sum::<f32>();
        model.iter().min_by(|a, b| d(a).total_cmp(&d(b))).map_or(c, |t| t.color[0])
    };
    let darkest = model.iter().map(|t| t.color[0]).min_by(|a, b| a.iter().sum::<f32>().total_cmp(&b.iter().sum::<f32>())).unwrap_or([0.2; 3]);
    let (band, handle) = (darkest, nearest([0.3, 0.08, 0.03]));
    // the dark patch: its texel (atlas UV) and a texel's width, or none (then the darkest colour)
    let patch = dark_patch(atlas);
    let mut out = Vec::new();
    // (SM64 is mirrored on X: flver.rs flips it back)
    let mut tri = |p: [[f32; 3]; 3], n: [[f32; 3]; 3], uv: [[f32; 2]; 3], color: [f32; 3]| {
        let local = p.map(|v| [-v[0] * PER_METRE, v[1] * PER_METRE, v[2] * PER_METRE]);
        let normal = n.map(|v| [-v[0], v[1], v[2]]);
        out.push(Tri { part: HAMMER_PART, local, normal, uv, color: [color; 3], world: local });
    };
    // a cylinder from `a` to `b` along an axis (point(angle, radius, along) gives a point and its
    // outward normal), with end caps where asked; `dark`: textured with the dark patch
    let mut cylinder = |point: &dyn Fn(f32, f32, f32) -> ([f32; 3], [f32; 3]), axis: [f32; 3], r: f32, a: f32, b: f32, sides: usize, caps: (bool, bool), color: [f32; 3], dark: bool| {
        let ang = |i: usize| i as f32 / sides as f32 * std::f32::consts::TAU;
        // a texture needs its UVs to differ across a triangle: a texel apart by side and by end,
        // the same at a shared corner (so corners stay shared)
        let uv = |i: usize, end: usize| match patch.filter(|_| dark) {
            Some(([u, v], (du, dv))) => [u + (i % 2) as f32 * du, v + end as f32 * dv],
            None => [0.0, 0.0],
        };
        for i in 0..sides {
            let (p0, n0) = point(ang(i), r, a);
            let (p1, n1) = point(ang(i + 1), r, a);
            let (q0, _) = point(ang(i), r, b);
            let (q1, _) = point(ang(i + 1), r, b);
            tri([p0, p1, q1], [n0, n1, n1], [uv(i, 0), uv(i + 1, 0), uv(i + 1, 1)], color);
            tri([p0, q1, q0], [n0, n1, n0], [uv(i, 0), uv(i + 1, 1), uv(i, 1)], color);
            // (the caps' rims sit just inside the sides': their vertices aren't shared, so the
            // edges stay sharp)
            for (on, at, sign, end) in [(caps.0, a, -1.0f32, 0), (caps.1, b, 1.0, 1)] {
                if on {
                    let (c, _) = point(0.0, 0.0, at);
                    let (e0, _) = point(ang(i), r * 0.995, at);
                    let (e1, _) = point(ang(i + 1), r * 0.995, at);
                    let n = axis.map(|v| v * sign);
                    tri([c, e0, e1], [n; 3], [uv(0, 1 - end), uv(i, end), uv(i + 1, end)], color);
                }
            }
        }
    };
    // the handle, along Y
    let along_y = |t: f32, r: f32, y: f32| ([r * t.cos(), y, r * t.sin()], [t.cos(), 0.0, t.sin()]);
    cylinder(&along_y, [0.0, 1.0, 0.0], 0.04, -0.16, HEAD_AT - 0.1, 8, (true, false), handle, false);
    // the head, along Z around its centre: solid and dark, a band at each end, dark capped ends
    let y = HEAD_AT;
    let along_z = |t: f32, r: f32, z: f32| ([r * t.cos(), y + r * t.sin(), z], [t.cos(), t.sin(), 0.0]);
    cylinder(&along_z, [0.0, 0.0, 1.0], 0.2, -0.24, 0.24, 12, (false, false), band, true);
    cylinder(&along_z, [0.0, 0.0, 1.0], 0.215, -0.30, -0.24, 12, (false, false), band, false);
    cylinder(&along_z, [0.0, 0.0, 1.0], 0.215, 0.24, 0.30, 12, (false, false), band, false);
    cylinder(&along_z, [0.0, 0.0, 1.0], 0.2, -0.34, -0.30, 12, (true, false), band, true);
    cylinder(&along_z, [0.0, 0.0, 1.0], 0.2, 0.30, 0.34, 12, (false, true), band, true);
    out
}

/// The centre texel of the darkest opaque 5x5 patch in SM64's texture atlas (11 cells of 64x64,
/// RGBA), as an atlas UV, and a texel's size in UV; None if nothing there is dark enough.
pub(super) fn dark_patch(atlas: &[u8]) -> Option<([f32; 2], (f32, f32))> {
    const CELL: usize = 64;
    const CELLS: usize = 11;
    let w = CELL * CELLS;
    if atlas.len() < w * CELL * 4 {
        return None;
    }
    let px = |x: usize, y: usize| {
        let o = (y * w + x) * 4;
        (atlas[o] as f32 + atlas[o + 1] as f32 + atlas[o + 2] as f32, atlas[o + 3])
    };
    let mut best: Option<(f32, usize, usize)> = None;
    for cell in 0..CELLS {
        for y in 2..CELL - 2 {
            for x in 2..CELL - 2 {
                let mut worst = 0.0f32;
                let mut solid = true;
                for dy in 0..5 {
                    for dx in 0..5 {
                        let (lum, a) = px(cell * CELL + x + dx - 2, y + dy - 2);
                        solid &= a >= 250;
                        worst = worst.max(lum);
                    }
                }
                if solid && best.is_none_or(|(b, ..)| worst < b) {
                    best = Some((worst, cell * CELL + x, y));
                }
            }
        }
    }
    // (dark: under a fifth of white at its brightest)
    let (worst, x, y) = best.filter(|(worst, ..)| *worst < 765.0 * 0.2)?;
    crate::log(format!("assets: the hammer's head is texel ({x}, {y}) of the atlas (brightness {:.2})", worst / 765.0));
    Some(([(x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / CELL as f32], (0.6 / w as f32, 0.6 / CELL as f32)))
}

/// Luigi's build (his model only, co-op's character = luigi): a touch slimmer than his brother, the
/// body and limbs narrower across their bone (the way to the next joint), the head, hands and feet
/// as they are. Each part's (SM64 part, its child, how much across it stays).
/// (the pelvis and forearms least: the hips and hands they meet stay as they are)
const SLIM: [(i32, i32, f32); 10] = [(1, 2, 0.95), (2, 3, 0.9), (4, 5, 0.86), (5, 6, 0.89), (7, 8, 0.86), (8, 9, 0.89), (10, 11, 0.88), (11, 12, 0.88), (13, 14, 0.88), (14, 15, 0.88)];

/// `model` with Luigi's build (SLIM): each part's vertices drawn in towards its bone, the normals
/// the other way round, their world positions from the bind pose again.
pub fn slim(model: &MarioModel) -> MarioModel {
    // each part's bone, in its own space: towards the child's joint
    let bones: Vec<(i32, [f32; 3], f32)> = SLIM
        .iter()
        .filter_map(|&(part, child, keep)| {
            let m: &[f32; 16] = model.matrices.get(part as usize)?;
            let c = model.matrices.get(child as usize)?;
            let v = transform(&invert(m), [c[12], c[13], c[14]], 1.0);
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            (l > 1e-3).then(|| (part, v.map(|x| x / l), keep))
        })
        .collect();
    let tris = model
        .tris
        .iter()
        .map(|t| {
            let Some(&(_, axis, keep)) = bones.iter().find(|b| b.0 == t.part) else { return *t };
            let mut t = *t;
            let dot = |v: [f32; 3]| v[0] * axis[0] + v[1] * axis[1] + v[2] * axis[2];
            for k in 0..3 {
                let along = dot(t.local[k]);
                t.local[k] = std::array::from_fn(|c| axis[c] * along + (t.local[k][c] - axis[c] * along) * keep);
                let n = t.normal[k];
                let along = dot(n);
                let n: [f32; 3] = std::array::from_fn(|c| axis[c] * along + (n[c] - axis[c] * along) / keep);
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                t.normal[k] = n.map(|v| v / l);
                if let Some(m) = model.matrices.get(t.part as usize) {
                    t.world[k] = transform(m, t.local[k], 1.0);
                }
            }
            t
        })
        .collect();
    MarioModel {
        tris,
        matrices: model.matrices.clone(),
        peace: model.peace.clone(),
        wings: model.wings.clone(),
        hammer: model.hammer.clone(),
        gear: model.gear.clone(),
        atlas: model.atlas.clone(),
    }
}

/// Inverse of a 4x4 row-vector matrix (Gauss-Jordan).
fn invert(m: &[f32]) -> [f32; 16] {
    let mut a: [[f64; 8]; 4] = std::array::from_fn(|r| std::array::from_fn(|c| if c < 4 { m[r * 4 + c] as f64 } else if c - 4 == r { 1.0 } else { 0.0 }));
    for col in 0..4 {
        let pivot = (col..4).max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs())).unwrap();
        a.swap(col, pivot);
        let p = a[col][col];
        if p.abs() < 1e-12 {
            continue;
        }
        for c in 0..8 {
            a[col][c] /= p;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                for c in 0..8 {
                    a[r][c] -= f * a[col][c];
                }
            }
        }
    }
    std::array::from_fn(|i| a[i / 4][4 + i % 4] as f32)
}

/// [v, w] * m (row vector)
fn transform(m: &[f32; 16], v: [f32; 3], w: f32) -> [f32; 3] {
    std::array::from_fn(|c| v[0] * m[c] + v[1] * m[4 + c] + v[2] * m[8 + c] + w * m[12 + c])
}
