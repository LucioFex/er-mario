//! Textures: Mario's albedo (SM64's textures and colours laid out for the FLVER's UVs), the chest
//! piece's TPF with Mario's textures, a minimal BC7 encoder, and the menu icon atlas patch.

use super::flver::{CELLS, INNER, PAD, TILE, mario_colors};
use super::model::MarioModel;

const SIZE: usize = 2048;
const ATLAS_W: usize = 64 * CELLS;

/// RGBA image, f32 channels 0..1
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Image {
    pub fn new(w: usize, h: usize, fill: [f32; 4]) -> Self {
        Self { w, h, px: vec![fill; w * h] }
    }

    /// Area average by an integer factor.
    pub fn shrink(&self, f: usize) -> Image {
        let (w, h) = ((self.w / f).max(1), (self.h / f).max(1));
        let mut out = Image::new(w, h, [0.0; 4]);
        let (fx, fy) = (self.w / w, self.h / h);
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for dy in 0..fy {
                    for dx in 0..fx {
                        let p = self.px[(y * fy + dy) * self.w + x * fx + dx];
                        for c in 0..4 {
                            acc[c] += p[c];
                        }
                    }
                }
                out.px[y * w + x] = acc.map(|v| v / (fx * fy) as f32);
            }
        }
        out
    }

    pub fn rgba8(&self) -> Vec<u8> {
        self.px.iter().flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)).collect()
    }
}

/// What's visible of `src` (premultiplied), scaled to fill most of a w x h card and centred, as
/// straight-alpha RGBA.
pub fn fit(src: &Image, w: usize, h: usize) -> Vec<u8> {
    let seen = |x: usize, y: usize| src.px[y * src.w + x][3] > 0.03;
    let xs: Vec<usize> = (0..src.w).filter(|&x| (0..src.h).any(|y| seen(x, y))).collect();
    let ys: Vec<usize> = (0..src.h).filter(|&y| (0..src.w).any(|x| seen(x, y))).collect();
    let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) = (xs.first(), xs.last(), ys.first(), ys.last()) else {
        return vec![0; w * h * 4];
    };
    let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let scale = (w as f32 * 0.94 / bw).min(h as f32 * 0.94 / bh);
    // shrinking: average the source area under each pixel (steps of the bilinear tap)
    let taps = (1.0 / scale).ceil().max(1.0) as usize;
    let at = |x: f32, y: f32| -> [f32; 4] {
        if x < -0.5 || y < -0.5 || x > src.w as f32 - 0.5 || y > src.h as f32 - 0.5 {
            return [0.0; 4];
        }
        let (x, y) = (x.clamp(0.0, (src.w - 1) as f32), y.clamp(0.0, (src.h - 1) as f32));
        let (xa, ya) = (x.floor() as usize, y.floor() as usize);
        let (xb, yb) = ((xa + 1).min(src.w - 1), (ya + 1).min(src.h - 1));
        let (fx, fy) = (x - xa as f32, y - ya as f32);
        let p = |x: usize, y: usize| src.px[y * src.w + x];
        std::array::from_fn(|c| {
            p(xa, ya)[c] * (1.0 - fx) * (1.0 - fy) + p(xb, ya)[c] * fx * (1.0 - fy) + p(xa, yb)[c] * (1.0 - fx) * fy + p(xb, yb)[c] * fx * fy
        })
    };
    let (cx, cy) = (x0 as f32 + bw / 2.0, y0 as f32 + bh / 2.0);
    let mut out = Image::new(w, h, [0.0; 4]);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for j in 0..taps {
                for i in 0..taps {
                    let sx = cx + (x as f32 + (i as f32 + 0.5) / taps as f32 - w as f32 / 2.0) / scale - 0.5;
                    let sy = cy + (y as f32 + (j as f32 + 0.5) / taps as f32 - h as f32 / 2.0) / scale - 0.5;
                    let p = at(sx, sy);
                    for c in 0..4 {
                        acc[c] += p[c];
                    }
                }
            }
            let mut p = acc.map(|v| v / (taps * taps) as f32);
            if p[3] > 0.0 {
                for c in 0..3 {
                    p[c] /= p[3];
                }
            }
            out.px[y * w + x] = p;
        }
    }
    out.rgba8()
}

/// Bilinear resample of an atlas cell (premultiplied, like Pillow does for RGBA).
fn cell_resized(atlas: &[u8], cell: usize, size: usize) -> Vec<[f32; 4]> {
    let at = |x: usize, y: usize| {
        let o = (y * ATLAS_W + cell * 64 + x) * 4;
        let a = atlas[o + 3] as f32 / 255.0;
        [atlas[o] as f32 / 255.0 * a, atlas[o + 1] as f32 / 255.0 * a, atlas[o + 2] as f32 / 255.0 * a, a]
    };
    let scale = 64.0 / size as f32;
    let axis = |i: usize| {
        let s = (i as f32 + 0.5) * scale - 0.5;
        let i0 = s.floor();
        let f = s - i0;
        let c = |v: f32| v.clamp(0.0, 63.0) as usize;
        (c(i0), c(i0 + 1.0), f)
    };
    let mut out = Vec::with_capacity(size * size);
    for y in 0..size {
        let (y0, y1, fy) = axis(y);
        for x in 0..size {
            let (x0, x1, fx) = axis(x);
            let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
            out.push(std::array::from_fn(|k| {
                (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy
            }));
        }
    }
    out
}

/// 2048x2048: per SM64 colour a band of padded tiles (each atlas texture composited over the
/// colour), plus solid swatches for untextured triangles.
/// Metal Mario (the metal cap): silver all over, his features barely showing, like SM64's.
pub fn metal_albedo(model: &MarioModel) -> Image {
    let mut img = mario_albedo(model);
    for p in &mut img.px {
        let lum = 0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2];
        let v = 0.6 + 0.35 * lum;
        *p = [v, v, (v * 1.04).min(1.0), 1.0];
    }
    img
}

/// The wing cap's albedo: the wing textures (cells 9 and 10) keep their see-through feathers.
pub fn wing_albedo(model: &MarioModel) -> Image {
    let mut img = mario_albedo(model);
    let cells: Vec<Vec<[f32; 4]>> = (0..CELLS).map(|i| cell_resized(&model.atlas, i, INNER)).collect();
    for k in 0..mario_colors(model).len() {
        for i in [9, 10] {
            for ty in 0..TILE {
                for tx in 0..TILE {
                    let inside = (PAD..PAD + INNER).contains(&tx) && (PAD..PAD + INNER).contains(&ty);
                    let a = if inside { cells[i][(ty - PAD) * INNER + tx - PAD][3] } else { 0.0 };
                    img.px[(k * 256 + ty) * SIZE + i * TILE + tx][3] = a;
                }
            }
        }
    }
    img
}

/// `character = luigi` in er_mario.ini: Mario's brother, in green and navy (for co-op, each player
/// picks his own).
pub fn luigi() -> bool {
    crate::paths::config("character").is_some_and(|v| v.eq_ignore_ascii_case("luigi"))
}

/// Luigi's colours for Mario's: his red (cap, shirt, the cap's letter) green, his blue overalls
/// navy; the rest (skin, hair, gloves, shoes, buttons) as they are.
fn luigi_color(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c;
    if r > 0.4 && g < 0.35 * r && b < 0.35 * r {
        [r * 0.12, r * 0.64, r * 0.17]
    } else if b > 0.35 && r < 0.4 * b && g < 0.5 * b {
        [b * 0.1, b * 0.13, b * 0.5]
    } else {
        c
    }
}

pub fn mario_albedo(model: &MarioModel) -> Image {
    mario_albedo_as(model, luigi())
}

/// SM64's cap emblem: the atlas's third texture (32 x 32 in its cell's corner), Mario's red M on a
/// white disc
const LOGO_CELL: usize = 2;
const LOGO_SIZE: usize = 32;

/// The atlas with Luigi's L on the cap instead of Mario's M: the M's red painted over in the
/// disc's white, an L in its red in the middle of where the M was (its stem and its foot), left
/// as it is if the emblem isn't where it's expected.
pub fn luigi_logo(atlas: &[u8]) -> Vec<u8> {
    let mut out = atlas.to_vec();
    let at = |x: usize, y: usize| (y * ATLAS_W + LOGO_CELL * 64 + x) * 4;
    let px = |x: usize, y: usize| {
        let o = at(x, y);
        [atlas[o], atlas[o + 1], atlas[o + 2], atlas[o + 3]].map(|v| v as f32 / 255.0)
    };
    let red = |p: [f32; 4]| p[3] > 0.5 && p[0] > 0.4 && p[1] < 0.5 * p[0] && p[2] < 0.5 * p[0];
    let white = |p: [f32; 4]| p[3] > 0.5 && p[0] > 0.7 && p[1] > 0.7 && p[2] > 0.7;
    let (mut x0, mut y0, mut x1, mut y1) = (LOGO_SIZE, LOGO_SIZE, 0, 0);
    let (mut ink, mut paper, mut n_ink, mut n_paper) = ([0.0f32; 3], [0.0f32; 3], 0.0, 0.0);
    for y in 0..LOGO_SIZE {
        for x in 0..LOGO_SIZE {
            let p = px(x, y);
            if red(p) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                (0..3).for_each(|c| ink[c] += p[c]);
                n_ink += 1.0;
            } else if white(p) {
                (0..3).for_each(|c| paper[c] += p[c]);
                n_paper += 1.0;
            }
        }
    }
    if n_ink < 10.0 || n_paper < 10.0 || x1 <= x0 + 3 || y1 <= y0 + 3 {
        return out;
    }
    let (ink, paper) = (ink.map(|v| v / n_ink), paper.map(|v| v / n_paper));
    // the L: as tall as the M, about two thirds as wide, centred; its stem a third of that, its foot
    // a third of its height
    let (w, h) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let lw = w * 0.62;
    let left = x0 as f32 + (w - lw) * 0.5;
    let stem = lw * 0.36;
    let foot = h * 0.32;
    let in_l = |x: f32, y: f32| {
        let (dx, dy) = (x - left, y - y0 as f32);
        (0.0..lw).contains(&dx) && (0.0..h).contains(&dy) && (dx < stem || dy >= h - foot)
    };
    for y in 0..LOGO_SIZE {
        for x in 0..LOGO_SIZE {
            let p = px(x, y);
            // (the M's soft edge a texel round it too)
            let near_m = (x0.saturating_sub(1)..=x1 + 1).contains(&x) && (y0.saturating_sub(1)..=y1 + 1).contains(&y);
            if p[3] <= 0.5 || !(red(p) || white(p) || near_m) {
                continue;
            }
            // (2 x 2 samples a texel: the L's edges a little soft, like the M's)
            let cover = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)].iter().filter(|(sx, sy)| in_l(x as f32 + sx, y as f32 + sy)).count() as f32 / 4.0;
            let rgb: [f32; 3] = std::array::from_fn(|c| paper[c] + (ink[c] - paper[c]) * cover);
            let o = at(x, y);
            for c in 0..3 {
                out[o + c] = (rgb[c] * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Mario's albedo in Mario's colours or Luigi's (the co-op partner's model is the other one).
pub fn mario_albedo_as(model: &MarioModel, luigi: bool) -> Image {
    let mut img = Image::new(SIZE, SIZE, [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0]);
    // (Luigi's L on the cap)
    let atlas = if luigi { luigi_logo(&model.atlas) } else { model.atlas.clone() };
    let mut cells: Vec<Vec<[f32; 4]>> = (0..CELLS).map(|i| cell_resized(&atlas, i, INNER)).collect();
    if luigi {
        for p in cells.iter_mut().flatten() {
            let [r, g, b] = luigi_color([p[0], p[1], p[2]]);
            *p = [r, g, b, p[3]];
        }
    }
    for (k, col) in mario_colors(model).iter().enumerate() {
        let c = col.map(|v| v as f32 / 255.0);
        let c = if luigi { luigi_color(c) } else { c };
        for (i, cell) in cells.iter().enumerate() {
            for ty in 0..TILE {
                for tx in 0..TILE {
                    let inside = (PAD..PAD + INNER).contains(&tx) && (PAD..PAD + INNER).contains(&ty);
                    let rgb = if inside {
                        let p = cell[(ty - PAD) * INNER + tx - PAD];
                        [0, 1, 2].map(|ch| p[ch] + c[ch] * (1.0 - p[3]))
                    } else {
                        c
                    };
                    img.px[(k * 256 + ty) * SIZE + i * TILE + tx] = [rgb[0], rgb[1], rgb[2], 1.0];
                }
            }
        }
        for y in 1600..SIZE {
            for x in k * 340..k * 340 + 336 {
                img.px[y * SIZE + x] = [c[0], c[1], c[2], 1.0];
            }
        }
    }
    img
}

/// The trap kit's albedo (kit.rs): its palette as a 4 x 4 grid of flat swatches.
pub fn kit_albedo() -> Image {
    let mut img = Image::new(SIZE, SIZE, [0.5, 0.5, 0.5, 1.0]);
    let cell = SIZE / 4;
    for (k, c) in super::kit::KIT_PALETTE.iter().enumerate() {
        let (cx, cy) = ((k % 4) * cell, (k / 4) * cell);
        for y in cy..cy + cell {
            for x in cx..cx + cell {
                img.px[y * SIZE + x] = [c[0], c[1], c[2], 1.0];
            }
        }
    }
    img
}

/// Every UTF-16 string in `blob` containing `needle` (for the logs: what a file calls itself).
pub fn utf16_with(blob: &[u8], needle: &str) -> Vec<String> {
    let n = u16s(needle);
    let mut out = Vec::new();
    let mut i = 0;
    while i + n.len() <= blob.len() {
        if blob[i..i + n.len()] == n[..] {
            // back to the string's start (a zero char before it), then to its end
            let mut a = i;
            while a >= 2 && !(blob[a - 2] == 0 && blob[a - 1] == 0) && i - a < 400 {
                a -= 2;
            }
            let mut e = i;
            while e + 1 < blob.len() && !(blob[e] == 0 && blob[e + 1] == 0) && e - a < 600 {
                e += 2;
            }
            let s: Vec<u16> = blob[a..e].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let s = String::from_utf16_lossy(&s);
            if !out.contains(&s) {
                out.push(s);
            }
            i = e;
        } else {
            i += 2;
        }
    }
    out
}

/// A TPF's texture names.
pub fn tpf_names(tpf: &[u8]) -> Vec<String> {
    tpf_textures(tpf).into_iter().map(|t| t.name).collect()
}

/// BC1 with a full mip chain (box-filtered), as stored in a DDS body.
fn bc1_mips(img: &Image, mips: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut level = Image { w: img.w, h: img.h, px: img.px.clone() };
    for m in 0..mips {
        if m > 0 {
            level = if level.w >= 2 && level.h >= 2 { level.shrink(2) } else { level };
        }
        let fmt = texpresso::Format::Bc1;
        let mut buf = vec![0u8; fmt.compressed_size(level.w, level.h)];
        fmt.compress(&level.rgba8(), level.w, level.h, texpresso::Params::default(), &mut buf);
        out.extend_from_slice(&buf);
    }
    out
}

fn u16s(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Replaces every UTF-16 occurrence of `old` with `new` (same length).
pub fn rename(blob: &mut [u8], old: &str, new: &str) {
    let (o, n) = (u16s(old), u16s(new));
    debug_assert_eq!(o.len(), n.len());
    let mut i = 0;
    while i + o.len() <= blob.len() {
        if blob[i..i + o.len()] == o[..] {
            blob[i..i + o.len()].copy_from_slice(&n);
            i += o.len();
        } else {
            i += 1;
        }
    }
}

fn i32_at(d: &[u8], o: usize) -> usize {
    i32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize
}

fn u32_at(d: &[u8], o: usize) -> usize {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize
}

struct TpfTexture {
    name: String,
    off: usize,
    size: usize,
    mips: usize,
}

fn tpf_textures(t: &[u8]) -> Vec<TpfTexture> {
    (0..i32_at(t, 8))
        .map(|i| {
            let a = 0x10 + i * 0x14;
            TpfTexture {
                off: i32_at(t, a),
                size: i32_at(t, a + 4),
                mips: t[a + 10] as usize,
                name: super::bnd4::wstr(t, i32_at(t, a + 12)).unwrap_or_default(),
            }
        })
        .collect()
}

fn dds_header(dds: &[u8]) -> usize {
    if &dds[84..88] == b"DX10" { 148 } else { 128 }
}

/// Mario's albedo, a flat matte normal map and an empty metal mask, in the chest piece's own
/// sizes and formats (full textures "X_a", low-detail ones "X_a_l"). `metal`: a full metal mask and
/// a shiny surface instead (Metal Mario).
pub fn build_tpf(tpf: &[u8], albedo: &Image, metal: bool) -> Result<Vec<u8>, String> {
    let mut t = tpf.to_vec();
    for tex in tpf_textures(tpf) {
        let base = tex.name.strip_suffix("_l").unwrap_or(&tex.name);
        let dds = &tpf[tex.off..tex.off + tex.size];
        let head = dds_header(dds);
        let (h, w) = (u32_at(dds, 12), u32_at(dds, 16));
        let body = if base.ends_with("_a") && !base.ends_with("rope_a") {
            if w > SIZE || SIZE % w != 0 || w != h {
                return Err(format!("unexpected albedo size {w}x{h}"));
            }
            let img = if w == SIZE { albedo.shrink(1) } else { albedo.shrink(SIZE / w) };
            bc1_mips(&img, tex.mips)
        } else if base.ends_with("_n") {
            // BC7 mode 6, constant (128, 128, 50, 254): flat normal, low shine, no detail grime
            // (shine 230 for metal)
            let shine = if metal { 115 } else { 25 };
            let mut bits: u128 = 1 << 6;
            for (k, c7) in [64u128, 64, 64, 64, shine, shine, 127, 127].into_iter().enumerate() {
                bits |= c7 << (7 + 7 * k);
            }
            bits.to_le_bytes().repeat((tex.size - head) / 16)
        } else if base.ends_with("_m") {
            // BC4: all 255 is metal everywhere
            vec![if metal { 0xFF } else { 0 }; tex.size - head]
        } else {
            continue;
        };
        if body.len() != tex.size - head {
            return Err(format!("texture {} size mismatch ({} vs {})", tex.name, body.len(), tex.size - head));
        }
        t[tex.off + head..tex.off + tex.size].copy_from_slice(&body);
    }
    Ok(t)
}

/// A mount's stand-in: its albedo (square, a multiple of the texture's size) over the textures
/// named `*{key}_a`, a flat matte normal map over `*{key}_n` and nothing in the `_1m` mask, in the
/// sizes and formats they have. The other textures stay.
pub fn build_mount_tpf(tpf: &[u8], albedo: &Image, key: &str) -> Result<Vec<u8>, String> {
    let mut t = tpf.to_vec();
    for tex in tpf_textures(tpf) {
        let base = tex.name.strip_suffix("_l").unwrap_or(&tex.name);
        let Some(kind) = base.rsplit_once(key).map(|(_, kind)| kind) else { continue };
        let dds = &tpf[tex.off..tex.off + tex.size];
        let head = dds_header(dds);
        let (h, w) = (u32_at(dds, 12), u32_at(dds, 16));
        let body = match kind {
            "_a" => {
                if w == 0 || w > albedo.w || albedo.w % w != 0 || w != h {
                    return Err(format!("unexpected albedo size {w}x{h}"));
                }
                bc1_mips(&albedo.shrink(albedo.w / w), tex.mips)
            }
            "_n" => {
                // BC7 mode 6, constant (128, 128, 50, 254): flat normal, low shine
                let mut bits: u128 = 1 << 6;
                for (k, c7) in [64u128, 64, 64, 64, 25, 25, 127, 127].into_iter().enumerate() {
                    bits |= c7 << (7 + 7 * k);
                }
                bits.to_le_bytes().repeat((tex.size - head) / 16)
            }
            "_1m" => vec![0; tex.size - head],
            _ => continue,
        };
        if body.len() != tex.size - head {
            return Err(format!("texture {} size mismatch ({} vs {})", tex.name, body.len(), tex.size - head));
        }
        t[tex.off + head..tex.off + tex.size].copy_from_slice(&body);
    }
    Ok(t)
}

// ---- BC7 ---------------------------------------------------------------------------------------

const BC7_WEIGHTS: [f64; 16] = [0., 4., 9., 13., 17., 21., 26., 30., 34., 38., 43., 47., 51., 55., 60., 64.];

/// Minimal BC7 encoder (mode 6 only: RGBA endpoints 7 bits + p-bit, 16 index levels).
pub fn bc7_mode6(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h);
    for by in 0..h / 4 {
        for bx in 0..w / 4 {
            let px: [[f64; 4]; 16] =
                std::array::from_fn(|i| std::array::from_fn(|c| rgba[((by * 4 + i / 4) * w + bx * 4 + i % 4) * 4 + c] as f64));
            let lo: [f64; 4] = std::array::from_fn(|c| px.iter().map(|p| p[c]).fold(f64::MAX, f64::min));
            let hi: [f64; 4] = std::array::from_fn(|c| px.iter().map(|p| p[c]).fold(f64::MIN, f64::max));
            let q = |v: f64| (v / 2.0).round_ties_even().clamp(0.0, 127.0) as u128;
            let (mut e0, mut e1) = (lo.map(q), hi.map(q));
            let (f0, f1) = (e0.map(|v| v as f64 * 2.0), e1.map(|v| v as f64 * 2.0));
            let d: [f64; 4] = std::array::from_fn(|c| f1[c] - f0[c]);
            let dd: f64 = d.iter().map(|v| v * v).sum();
            let mut idx: [u128; 16] = std::array::from_fn(|i| {
                let t = if dd > 0.0 { (0..4).map(|c| (px[i][c] - f0[c]) * d[c]).sum::<f64>() / dd.max(1.0) } else { 0.0 };
                let t = t.clamp(0.0, 1.0);
                let mut best = 0;
                for k in 1..16 {
                    if (t - BC7_WEIGHTS[k] / 64.0).abs() < (t - BC7_WEIGHTS[best] / 64.0).abs() {
                        best = k;
                    }
                }
                best as u128
            });
            // the first pixel's index has only 3 bits: swap the endpoints where it would need the 4th
            if idx[0] >= 8 {
                std::mem::swap(&mut e0, &mut e1);
                idx = idx.map(|i| 15 - i);
            }
            let mut bits: u128 = 1 << 6;
            for c in 0..4 {
                bits |= e0[c] << (7 + 14 * c) | e1[c] << (14 + 14 * c);
            }
            bits |= idx[0] << 65;
            for (k, &i) in idx.iter().enumerate().skip(1) {
                bits |= i << (64 + 4 + 4 * (k - 1));
            }
            out.extend_from_slice(&bits.to_le_bytes());
        }
    }
    out
}

// ---- menu icons --------------------------------------------------------------------------------

const ICON_ATLAS: &str = "SB_Icon_04";
const SPRITE: usize = 160;
/// icon -> (x, y) of its 160x160 sprite in SB_Icon_04 (the unused icons 13580-13583)
pub const ICON_RECTS: [(usize, usize); 4] = [(1312, 820), (1312, 656), (1312, 492), (1312, 328)];

/// Paints the icons (cap, overalls, gloves, shoes; 160x160 RGBA) into the menu atlas texture.
pub fn patch_icon_atlas(tpf: &[u8], icons: &[Vec<u8>; 4]) -> Result<Vec<u8>, String> {
    let mut t = tpf.to_vec();
    for (icon, &at) in icons.iter().zip(ICON_RECTS.iter()) {
        paint(&mut t, ICON_ATLAS, (4096, 2048), at, icon, (SPRITE, SPRITE))?;
    }
    Ok(t)
}

const PRESET_ATLAS: &str = "SB_Preset";
/// The Vagabond's card (MENU_Ch_01 in SB_Preset.layout) in the hi and low atlas: its corner and
/// the atlas size. The low one is half size.
pub const PORTRAIT_AT: [((usize, usize), (usize, usize)); 2] = [((0, 1372), (4096, 4096)), ((664, 800), (4096, 2048))];

/// Paints Mario over the Vagabond's character creation card. `quality` 0 hi, 1 low.
pub fn patch_portrait(tpf: &mut [u8], quality: usize, rgba: &[u8], size: (usize, usize)) -> Result<(), String> {
    let (at, atlas) = PORTRAIT_AT[quality];
    paint(tpf, PRESET_ATLAS, atlas, at, rgba, size)
}

/// Re-encodes the BC7 blocks of one rect (whole blocks: x, y, w, h multiples of 4) in a texture
/// of the menu tpf; the rest stays bit-identical.
fn paint(t: &mut [u8], name: &str, expect: (usize, usize), (x, y): (usize, usize), rgba: &[u8], (pw, ph): (usize, usize)) -> Result<(), String> {
    let tex = tpf_textures(t).into_iter().find(|t| t.name == name).ok_or(format!("{name} not found"))?;
    let dds = &t[tex.off..tex.off + tex.size];
    let (h, w) = (u32_at(dds, 12), u32_at(dds, 16));
    // a wrong header read once made a gigantic image: check everything before touching it
    if (w, h) != expect || &dds[84..88] != b"DX10" || u32_at(dds, 128) != 98 || tex.size - 148 < w * h {
        return Err(format!("{name} is not the expected texture"));
    }
    if x % 4 != 0 || y % 4 != 0 || pw % 4 != 0 || ph % 4 != 0 || x + pw > w || y + ph > h || rgba.len() != pw * ph * 4 {
        return Err(format!("{name}: bad rect"));
    }
    let data = tex.off + 148;
    let row_blocks = w / 4;
    let blocks = bc7_mode6(rgba, pw, ph);
    let row = pw / 4 * 16;
    for j in 0..ph / 4 {
        let start = data + ((y / 4 + j) * row_blocks + x / 4) * 16;
        t[start..start + row].copy_from_slice(&blocks[j * row..(j + 1) * row]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// An M on a white disc in the emblem's cell becomes an L: red only inside the L's stem and
    /// foot, none left where the M's other strokes were.
    #[test]
    fn luigi_has_an_l() {
        let mut atlas = vec![0u8; super::ATLAS_W * 64 * 4];
        let set = |a: &mut Vec<u8>, x: usize, y: usize, c: [u8; 4]| {
            let o = (y * super::ATLAS_W + super::LOGO_CELL * 64 + x) * 4;
            a[o..o + 4].copy_from_slice(&c);
        };
        for y in 0..32 {
            for x in 0..32 {
                let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
                if dx * dx + dy * dy < 13.0 * 13.0 {
                    set(&mut atlas, x, y, [250, 250, 250, 255]);
                }
            }
        }
        // the M: two legs and a V between them, x 8-24, y 10-21
        for y in 10..22 {
            for x in [8, 9, 10, 22, 23, 24] {
                set(&mut atlas, x, y, [220, 20, 20, 255]);
            }
            let v = 10 + (y - 10) / 2;
            set(&mut atlas, v, y, [220, 20, 20, 255]);
            set(&mut atlas, 32 - v, y, [220, 20, 20, 255]);
        }
        let out = super::luigi_logo(&atlas);
        let red = |x: usize, y: usize| {
            let o = (y * super::ATLAS_W + super::LOGO_CELL * 64 + x) * 4;
            out[o] > 150 && out[o + 1] < 100
        };
        // the M's right leg is gone, the L's stem and foot are there
        assert!((10..22).all(|y| !red(23, y)));
        assert!(red(12, 12) && red(12, 20));
        assert!(red(18, 20));
        assert!(!red(18, 12));
        // nothing outside the cell touched
        assert_eq!(out[..super::LOGO_CELL * 64 * 4], atlas[..super::LOGO_CELL * 64 * 4]);
    }

    /// (by hand, with the ROM at C:\ER-Mario: prints the real emblem made into an L)
    #[test]
    #[ignore]
    fn luigi_logo_from_the_rom() {
        let Ok(rom) = std::fs::read("C:/ER-Mario/Super Mario 64 (USA).z64") else { return };
        let seg = crate::gameover::mio0(&rom, 1132368).expect("mario's textures");
        let mut atlas = vec![0u8; super::ATLAS_W * 64 * 4];
        for i in 0..32 * 32 {
            let v = u16::from_be_bytes([seg[6288 + i * 2], seg[6289 + i * 2]]);
            let c = |s: u16| ((v >> s & 31) as u32 * 255 / 31) as u8;
            let (x, y) = (i % 32, i / 32);
            let o = (y * super::ATLAS_W + super::LOGO_CELL * 64 + x) * 4;
            atlas[o..o + 4].copy_from_slice(&[c(11), c(6), c(1), if v & 1 == 1 { 255 } else { 0 }]);
        }
        let out = super::luigi_logo(&atlas);
        for y in 0..32 {
            let row: String = (0..32)
                .map(|x| {
                    let o = (y * super::ATLAS_W + super::LOGO_CELL * 64 + x) * 4;
                    if out[o + 3] == 0 { '.' } else if out[o] > 150 && out[o + 1] < 100 { 'L' } else { 'o' }
                })
                .collect();
            println!("{row}");
        }
    }

    /// Luigi: Mario's red green, his blue navy, his skin and white untouched.
    #[test]
    fn luigi_colors() {
        let g = super::luigi_color([1.0, 0.0, 0.0]);
        assert!(g[1] > 0.5 && g[0] < 0.2 && g[2] < 0.2, "{g:?}");
        let n = super::luigi_color([0.0, 0.0, 1.0]);
        assert!(n[2] > 0.4 && n[2] < 0.6 && n[0] < 0.15, "{n:?}");
        for c in [[1.0, 0.8, 0.6], [1.0, 1.0, 1.0], [0.45, 0.2, 0.1], [1.0, 1.0, 0.0]] {
            assert_eq!(super::luigi_color(c), c);
        }
    }
}
