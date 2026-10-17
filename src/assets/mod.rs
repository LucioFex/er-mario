//! Builds the mod's game files on the player's PC: Mario's armour model (his mesh from libsm64,
//! which compiles in the SM64 decompilation's model code), his textures from the player's own SM64
//! ROM, and the menu icons rendered from both, patched into copies of the game's own files, read
//! straight from its archives.
//!
//! me3 picks up the package folder when the game starts, so a fresh build is used from the next
//! launch on; until then Mario mode stays off.

pub mod archive;
pub mod bnd4;
pub mod dcx;
pub mod flver;
pub mod icons;
pub mod matbin;
pub mod model;
pub mod tex;
pub mod yoshi;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{log, paths};

/// Bump when the generated files change, so existing installs rebuild.
const VERSION: &str = "er-mario assets 10";
const STAMP: &str = "package/.built";
const PIECES: [&str; 4] = ["hd", "bd", "am", "lg"];
/// Mario's chest piece models: his own, Metal Mario's and the wing cap's (caps.rs puts them on)
pub const MODEL_MARIO: u16 = 999;
pub const MODEL_METAL: u16 = 998;
pub const MODEL_WING: u16 = 997;
const LOOKS: [u16; 3] = [MODEL_MARIO, MODEL_METAL, MODEL_WING];
const QUALITIES: [&str; 2] = ["hi", "low"];

/// The bone the build put Mario's hammer on (engine_mario reads it), next to the package files.
pub const HAMMER_BONE_FILE: &str = "package/hammer_bone.txt";

/// The package files were complete when the game started (so me3 serves them this session).
static READY: AtomicBool = AtomicBool::new(false);
/// ...and Yoshi's among them: Torrent's model c8002 and its textures. They're only there if he
/// was found in the ROM.
static YOSHI: AtomicBool = AtomicBool::new(false);
const TORRENT: [&str; 3] = ["package/chr/c8002.chrbnd.dcx", "package/chr/c8002_h.texbnd.dcx", "package/chr/c8002_l.texbnd.dcx"];
/// The mesh of Torrent's model Yoshi goes into: his body, with the simple material
const TORRENT_BODY: usize = 7;

/// Torrent is Yoshi this session.
pub fn yoshi_ready() -> bool {
    YOSHI.load(Ordering::Relaxed)
}

pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

fn outputs() -> Vec<String> {
    let mut out: Vec<String> = PIECES
        .iter()
        .flat_map(|p| ["", "_l"].map(|s| format!("package/parts/{p}_m_0999{s}.partsbnd.dcx")))
        .collect();
    out.extend(LOOKS[1..].iter().flat_map(|m| ["", "_l"].map(|s| format!("package/parts/bd_m_{m:04}{s}.partsbnd.dcx"))));
    out.push("package/material/allmaterial.matbinbnd.dcx".into());
    out.extend(QUALITIES.map(|q| format!("package/menu/{q}/01_common.tpf.dcx")));
    out
}

/// A picture of the player's own for the Vagabond's card in character creation, next to the DLL.
/// Without it Mario is rendered from the ROM's model.
const PORTRAIT_FILE: &str = "portrait.png";

/// What the package was built from: a new version or another portrait file builds it again.
fn stamp() -> String {
    match std::fs::metadata(paths::file(PORTRAIT_FILE)) {
        Ok(m) => format!("{VERSION}, portrait {}", m.len()),
        Err(_) => VERSION.to_string(),
    }
}

/// portrait.png fitted into the card, as (hi, low) RGBA; None without the file.
fn own_portrait() -> Option<Result<(Vec<u8>, Vec<u8>), String>> {
    let file = std::fs::File::open(paths::file(PORTRAIT_FILE)).ok()?;
    Some((|| {
        let mut decoder = png::Decoder::new(file);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let (w, h) = (info.width as usize, info.height as usize);
        let n = info.color_type.samples();
        if w == 0 || h == 0 || w > 8192 || h > 8192 {
            return Err(format!("{w}x{h} is no size for a portrait"));
        }
        let mut src = tex::Image::new(w, h, [0.0; 4]);
        for (i, p) in buf[..w * h * n].chunks_exact(n).enumerate() {
            let v = |k: usize| p[k] as f32 / 255.0;
            let (rgb, a) = match n {
                1 => ([v(0); 3], 1.0),
                2 => ([v(0); 3], v(1)),
                3 => ([v(0), v(1), v(2)], 1.0),
                _ => ([v(0), v(1), v(2)], v(3)),
            };
            // premultiplied, so transparent pixels don't bleed their colour into the edges
            src.px[i] = [rgb[0] * a, rgb[1] * a, rgb[2] * a, a];
        }
        let (cw, ch) = icons::PORTRAIT;
        Ok((tex::fit(&src, cw, ch), tex::fit(&src, cw / 2, ch / 2)))
    })())
}

/// Checks the package once at startup. Returns true if it has to be built.
pub fn check() -> bool {
    let complete = std::fs::read_to_string(paths::file(STAMP)).is_ok_and(|s| s.trim() == stamp())
        && outputs().iter().all(|f| paths::file(f).is_file());
    READY.store(complete, Ordering::Relaxed);
    YOSHI.store(complete && TORRENT.iter().all(|f| paths::file(f).is_file()), Ordering::Relaxed);
    !complete
}

fn write(rel: &str, data: &[u8]) -> Result<(), String> {
    let path: PathBuf = paths::file(rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| format!("{}: {e}", tmp.display()))?;
    // the game may have the old one open (the menu textures, once past the title screen): the new
    // one waits next to it, put in place at the next launch (apply_pending)
    if let Err(e) = std::fs::rename(&tmp, &path) {
        if !path.is_file() {
            return Err(format!("{}: {e}", path.display()));
        }
        std::fs::rename(&tmp, pending(&path)).map_err(|e| format!("{}: {e}", path.display()))?;
        log(format!("assets: {rel} is in use ({e}), put in place at the next launch"));
    }
    Ok(())
}

fn pending(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".pending");
    p.into()
}

/// At load, before the game reads any of them: the files built while it had the old ones open.
pub fn apply_pending() {
    fn walk(dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if path.is_dir() {
                walk(&path);
            } else if let Some(target) = path.to_str().and_then(|p| p.strip_suffix(".pending")) {
                match std::fs::rename(&path, target) {
                    Ok(()) => log(format!("assets: {target} put in place")),
                    Err(e) => log(format!("assets: {target} could not be put in place: {e}")),
                }
            }
        }
    }
    walk(&paths::file("package"));
}

/// Builds every package file from the exported Mario model.
pub fn build(model: &model::MarioModel) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let step = |p: f32, text: &str| crate::hud::setup_progress("Setting up ER Mario", p, text);
    step(0.05, "Reading the game archives");
    let _ = std::fs::remove_file(paths::file(STAMP));
    let archives = archive::Archives::open()?;
    log(format!("assets: archives indexed in {:.1} s", t0.elapsed().as_secs_f32()));
    // the wing cap's model: Mario with its wings on his head, in the nearest of Mario's own colours
    // (the albedo has a row per colour, and no room for another)
    let nearest = |c: [f32; 3]| {
        let d = |t: &model::Tri| (0..3).map(|k| (t.color[0][k] - c[k]).powi(2)).sum::<f32>();
        model.tris.iter().min_by(|a, b| d(a).total_cmp(&d(b))).map_or(c, |t| t.color[0])
    };
    // each wing twice, the second facing the other way: Elden Ring draws only the front of a
    // triangle, and the flat wings vanished seen from behind
    let wings: Vec<model::Tri> = model
        .wings
        .iter()
        .flat_map(|t| {
            let front = model::Tri { color: [nearest(t.color[0]); 3], ..*t };
            let mut back = front;
            for k in [1, 2] {
                back.local[k] = front.local[3 - k];
                back.normal[k] = front.normal[3 - k];
                back.uv[k] = front.uv[3 - k];
                back.color[k] = front.color[3 - k];
                back.world[k] = front.world[3 - k];
            }
            for n in back.normal.iter_mut() {
                *n = n.map(|v| -v);
            }
            [front, back]
        })
        .collect();
    let winged = model::MarioModel {
        tris: model.tris.iter().chain(&wings).copied().collect(),
        matrices: model.matrices.clone(),
        peace: model.peace.clone(),
        wings: Vec::new(),
        hammer: model.hammer.clone(),
        atlas: model.atlas.clone(),
    };
    log(format!("assets: wing cap {} triangles", model.wings.len()));
    // (look, its mesh, its albedo, metal)
    let looks = [
        (MODEL_MARIO, model, tex::mario_albedo(model), false),
        (MODEL_METAL, model, tex::metal_albedo(model), true),
        (MODEL_WING, &winged, tex::wing_albedo(&winged), false),
    ];

    // the armour set, as its own model 999: Mario in the chest piece, the rest empty; and the
    // chest piece again for each cap's look
    let mut done = 0.0;
    let total = (PIECES.len() + looks.len() - 1) * 2;
    for piece in PIECES {
        for (look, mesh, albedo, metal) in &looks {
            if piece != "bd" && *look != MODEL_MARIO {
                continue;
            }
            let id = format!("{look:04}");
            for suffix in ["", "_l"] {
                step(0.15 + 0.5 * done / total as f32, "Building the model and textures");
                done += 1.0;
                let src = dcx::decompress(&archives.read(&format!("/parts/{piece}_m_1280{suffix}.partsbnd.dcx"))?)?;
                let mut files = bnd4::read(&src)?;
                for f in &mut files {
                    let lower = f.name.to_lowercase();
                    if lower.ends_with(".flver") {
                        let fl = flver::Flver::new(std::mem::take(&mut f.data))?;
                        f.data = if piece == "bd" {
                            if *look == MODEL_MARIO && suffix.is_empty() {
                                let bone = flver::hammer_bone(&fl).map_or("", |(_, n)| n);
                                log(format!("assets: the hammer goes on {}", if bone.is_empty() { "no bone" } else { bone }));
                                write(HAMMER_BONE_FILE, bone.as_bytes())?;
                            }
                            let mut d = flver::build_mario(fl, mesh, 1)?;
                            tex::rename(&mut d, "P[BD_M_1280]_Fabric.matxml", &format!("P[BD_M_{id}]_Fabric.matxml"));
                            d
                        } else {
                            fl.empty()
                        };
                    } else if lower.ends_with(".tpf") && piece == "bd" {
                        f.data = tex::build_tpf(&f.data, albedo, *metal)?;
                        tex::rename(&mut f.data, "BD_M_1280_", &format!("BD_M_{id}_"));
                    }
                    f.name = f.name.replace("_1280", &format!("_{id}"));
                }
                write(&format!("package/parts/{piece}_m_{id}{suffix}.partsbnd.dcx"), &dcx::compress(&bnd4::build(&src, &files))?)?;
            }
        }
    }
    log(format!("assets: armour built ({:.1} s)", t0.elapsed().as_secs_f32()));
    step(0.7, "Building the material");

    // its material: the vanilla list plus P[BD_M_0999]_Fabric (renamed textures, no detail grime),
    // and one for each cap's look
    let mat = dcx::decompress(&archives.read("/material/allmaterial.matbinbnd.dcx")?)?;
    let mut files = bnd4::read(&mat)?;
    let src = files.iter().find(|f| f.name.ends_with("P[BD_M_1280]_Fabric.matbin")).ok_or("Vagabond material not found")?;
    let (name, data) = (src.name.clone(), src.data.clone());
    for look in LOOKS {
        let id = format!("BD_M_{look:04}");
        let mut data = data.clone();
        tex::rename(&mut data, "BD_M_1280", &id);
        matbin::clear_detail(&mut data);
        let new = bnd4::File { id: files.iter().map(|f| f.id).max().unwrap_or(0) + 1, name: name.replace("BD_M_1280", &id), data };
        files.push(new);
    }
    write("package/material/allmaterial.matbinbnd.dcx", &dcx::compress(&bnd4::build(&mat, &files))?)?;

    log(format!("assets: material built ({:.1} s)", t0.elapsed().as_secs_f32()));

    // menu icons
    step(0.75, "Building the menu icons");
    let icons = icons::render_all(model);
    log(format!("assets: icons rendered ({:.1} s)", t0.elapsed().as_secs_f32()));
    step(0.85, "Saving the menu icons");
    let (hi, low) = match own_portrait() {
        Some(Ok(p)) => p,
        Some(Err(e)) => {
            log(format!("assets: {PORTRAIT_FILE} not used: {e}"));
            icons::portrait(model)
        }
        None => icons::portrait(model),
    };
    log(format!("assets: portrait done ({:.1} s)", t0.elapsed().as_secs_f32()));
    for (i, q) in QUALITIES.iter().enumerate() {
        let tpf = dcx::decompress(&archives.read(&format!("/menu/{q}/01_common.tpf.dcx"))?)?;
        let mut tpf = tex::patch_icon_atlas(&tpf, &icons)?;
        let (w, h) = icons::PORTRAIT;
        tex::patch_portrait(&mut tpf, i, if i == 0 { &hi } else { &low }, (w >> i, h >> i))?;
        write(&format!("package/menu/{q}/01_common.tpf.dcx"), &dcx::compress(&tpf)?)?;
    }
    step(0.95, "Building Yoshi");
    if let Err(e) = build_yoshi(&archives) {
        // (Torrent stays Torrent then: none of his files may be left half done)
        log(format!("assets: no Yoshi ({e})"));
        for f in TORRENT {
            let _ = std::fs::remove_file(paths::file(f));
        }
    }
    write(STAMP, stamp().as_bytes())?;
    log(format!("assets: all built in {:.1} s", t0.elapsed().as_secs_f32()));
    Ok(())
}

/// Yoshi in Torrent's place: his mesh into the body mesh of c8002, his colours and eyes over
/// the body's textures.
fn build_yoshi(archives: &archive::Archives) -> Result<(), String> {
    let rom = paths::read_rom()?;
    let model = yoshi::model(&rom).ok_or("he wasn't found in the ROM")?;
    let src = dcx::decompress(&archives.read("/chr/c8002.chrbnd.dcx")?)?;
    let mut files = bnd4::read(&src)?;
    let f = files.iter_mut().find(|f| f.name.to_lowercase().ends_with(".flver")).ok_or("Torrent's model has no FLVER")?;
    let fl = flver::Flver::new(std::mem::take(&mut f.data))?;
    f.data = flver::build_mount(fl, &crate::yoshi::BONES, &model.verts, &model.tris, TORRENT_BODY)?;
    let mut out = vec![(TORRENT[0], dcx::compress(&bnd4::build(&src, &files))?)];
    for path in &TORRENT[1..] {
        let src = dcx::decompress(&archives.read(&path["package".len()..])?)?;
        let mut files = bnd4::read(&src)?;
        let f = files.iter_mut().find(|f| f.name.to_lowercase().ends_with(".tpf")).ok_or("Torrent's textures have no TPF")?;
        f.data = tex::build_mount_tpf(&f.data, &model.albedo, "Body")?;
        out.push((path, dcx::compress(&bnd4::build(&src, &files))?));
    }
    // all three or none
    for (path, data) in &out {
        write(path, data)?;
    }
    log(format!("assets: Yoshi built ({} vertices)", model.verts.len()));
    Ok(())
}
