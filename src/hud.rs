//! SM64's HUD, from the player's ROM: the power meter (the "POWER" frame with the 8-wedge health
//! pie), the counters in SM64's own font (Mario's head x deaths, coins, stars) and the spinning
//! coins enemies drop (coins.rs). Drawn as a 2D overlay on the game's final image (hudhook: a
//! DirectX 12 overlay), a few textured quads a frame: cheap, pinned to the screen and untouched by
//! the game's anti-aliasing.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::time::Instant;

use eldenring::cs::{CSCamExt, CSCamera};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use hudhook::imgui::{self, TextureId};
use hudhook::{ImguiRenderLoop, RenderContext};

use crate::log;

/// MIO0 block with the power meter and the coins (actors/power_meter, actors/coin in the decomp)
const BLOCK: usize = 0x201410;
const LEFT: usize = 0x233E0;
const RIGHT: usize = 0x243E0;
/// pies for 1..=8 wedges
const PIES: [usize; 8] = [0x28BE0, 0x283E0, 0x27BE0, 0x273E0, 0x26BE0, 0x263E0, 0x25BE0, 0x253E0];
/// the yellow coin's spin: front, tilted right, side, tilted left (IA16, tinted yellow)
const COIN_FRAMES: [usize; 4] = [0x5780, 0x5F80, 0x6780, 0x6F80];
/// SM64's flame (actors/flame): 8 frames, IA16, Mario's fireball (fireball.rs)
const FLAME_FRAMES: usize = 0x17320;
/// MIO0 block with SM64's "!" boxes (actors/exclamation_box): the faces of the green (Metal), red
/// (Wing) and blue (Vanish) cap boxes, by caps.rs order
const COMMON0: usize = 0x1F2200;
const CAP_BOXES: [usize; 3] = [0x14620, 0x15E20, 0x12E20];
/// MIO0 block with the HUD font (segment 2): digits 0-9 at 0x200 each, then these
const SEGMENT2: usize = 0x108A40;
const GLYPH_MULTIPLY: usize = 0x4200;
const GLYPH_COIN: usize = 0x4400;
const GLYPH_HEAD: usize = 0x4600;
const GLYPH_STAR: usize = 0x4800;

/// HUD size relative to SM64's, and the meter centre's height on SM64's 240-line screen
const SIZE: f32 = 0.65;
const HEIGHT: f32 = 192.0;
/// ...and where it rests while Mario is hurt (just under the top edge), after sliding up
/// METER_RISE_AFTER seconds after the last health change, over METER_RISE_TIME seconds
const HEIGHT_TOP: f32 = 216.0;
/// above the top edge: where it slides to when Mario is back to full health
const HEIGHT_GONE: f32 = 270.0;
const METER_RISE_AFTER: f32 = 2.0;
const METER_RISE_TIME: f32 = 0.4;
/// seconds the meter stays up after Mario is back to full health
const LINGER: f32 = 1.5;
/// the coin counter's left edge, as a share of the screen width
const COINS_X: f32 = 0.56;
/// textures are uploaded this many times bigger (nearest neighbour), so the overlay's smooth
/// filtering keeps SM64's crisp pixels
const UPSCALE: usize = 4;

/// An RGBA image (already upscaled) and its original size.
struct Image {
    rgba: Vec<u8>,
    w: usize,
    h: usize,
}

impl Image {
    fn new(w: usize, h: usize, pixel: impl Fn(usize, usize) -> [u8; 4]) -> Self {
        let (bw, bh) = (w * UPSCALE, h * UPSCALE);
        let mut rgba = Vec::with_capacity(bw * bh * 4);
        for y in 0..bh {
            for x in 0..bw {
                rgba.extend_from_slice(&pixel(x / UPSCALE, y / UPSCALE));
            }
        }
        Image { rgba, w, h }
    }
}

/// RGBA16 (5-5-5-1) pixel
fn rgba16_px(t: &[u8], i: usize) -> [u8; 4] {
    let v = u16::from_be_bytes([t[i * 2], t[i * 2 + 1]]);
    let c = |shift: u16| (((v >> shift) & 31) as u32 * 255 / 31) as u8;
    [c(11), c(6), c(1), if v & 1 != 0 { 255 } else { 0 }]
}

fn rgba16(tex: &[u8], w: usize, h: usize) -> Image {
    Image::new(w, h, |x, y| rgba16_px(tex, y * w + x))
}

/// IA16 (intensity, alpha) times a tint
fn ia16(tex: &[u8], w: usize, h: usize, tint: [u8; 3]) -> Image {
    Image::new(w, h, |x, y| {
        let (i, a) = (tex[(y * w + x) * 2], tex[(y * w + x) * 2 + 1]);
        let c = tint.map(|t| (t as u32 * i as u32 / 255) as u8);
        [c[0], c[1], c[2], a]
    })
}

/// Every HUD image, in a fixed order (see the indices below).
static IMAGES: Mutex<Option<Vec<Image>>> = Mutex::new(None);
const IMG_BASE: usize = 0;
const IMG_PIES: usize = 1; // 8
const IMG_DIGITS: usize = 9; // 10
const IMG_MULTIPLY: usize = 19;
const IMG_COIN: usize = 20;
const IMG_HEAD: usize = 21;
const IMG_STAR: usize = 22;
const IMG_COIN_FRAMES: usize = 23; // 4
const IMG_BOWSER: usize = 27;
/// SM64's HUD letters (it has no J, Q, V, X, Z), then the apostrophe
const LETTERS: [(char, usize); 21] = [
    ('A', 0x1400), ('B', 0x1600), ('C', 0x1800), ('D', 0x1A00), ('E', 0x1C00), ('F', 0x1E00), ('G', 0x2000),
    ('H', 0x2200), ('I', 0x2400), ('K', 0x2600), ('L', 0x2800), ('M', 0x2A00), ('N', 0x2C00), ('O', 0x2E00),
    ('P', 0x3000), ('R', 0x3200), ('S', 0x3400), ('T', 0x3600), ('U', 0x3800), ('W', 0x3A00), ('Y', 0x3C00),
];
const IMG_LETTERS: usize = 28; // 21
const IMG_APOSTROPHE: usize = 49;
/// the camera status (bottom right): camera, Lakitu, no-camera X, and the 8x8 zoom arrows
const IMG_CAMERA: usize = 50;
const IMG_LAKITU: usize = 51;
const IMG_ARROW_UP: usize = 52;
const IMG_ARROW_DOWN: usize = 53;
const IMG_FLAME: usize = 54; // 8
const IMG_CAP_BOX: usize = 62; // 3
const IMG_SPARK: usize = 65; // 3
/// the fireball's glow, then the giant shot's ball (4 frames), fireball.rs FRAME_GLOW / FRAME_ORB
const IMG_GLOW: usize = 68;
const IMG_ORB: usize = 69; // 4
/// the weapons (weapons.rs): the boomerang, the Chain Chomp (mouth shut, open), a link of its chain,
/// the hammer's icon; the Mega Mushroom (mega.rs)
const IMG_RANG: usize = 73;
const IMG_CHOMP: usize = 74; // 2
#[allow(dead_code)] // (its slot stays: the images after it keep their numbers)
const IMG_LINK: usize = 76;
const IMG_HAMMER_ICON: usize = 77;
const IMG_MUSHROOM: usize = 78;
/// the hook's head (hook.rs): Mario's cap, with eyes
const IMG_CAPPY: usize = 79;
/// a Yoshi egg (ride.rs)
const IMG_EGG: usize = 80;
/// the HUD's Mario head in Luigi's green (the co-op partner's panel)
const IMG_LUIGI_HEAD: usize = 81;
/// the trap kit (traps.rs): banana, shell, Bob-omb, spring
const IMG_KIT: usize = 82; // 4
/// the Rock Mushroom (rock.rs): a mushroom with a stone cap
const IMG_ROCK: usize = 86;
/// an X in SM64's HUD font's colours (it has none: "MAX" read "MAK")
const IMG_X: usize = 87;

/// The glyph for a character of a name in SM64's HUD font (look-alikes for the letters it lacks).
fn glyph_for(c: char) -> Option<usize> {
    let c = match c.to_ascii_uppercase() {
        'J' => 'I',
        'Q' => 'O',
        'V' => 'U',
        'X' => return Some(IMG_X),
        'Z' => 'S',
        c => c,
    };
    if let Some(d) = c.to_digit(10) {
        return Some(IMG_DIGITS + d as usize);
    }
    if c == '\'' || c == ',' {
        return Some(IMG_APOSTROPHE);
    }
    LETTERS.iter().position(|&(l, _)| l == c).map(|i| IMG_LETTERS + i)
}

/// the style ranks' colours (style.rs RANKS), D to SSS
const TINTS: [[f32; 3]; 7] = [
    [0.75, 0.75, 0.8],
    [0.45, 0.7, 1.0],
    [0.4, 0.95, 0.5],
    [1.0, 0.9, 0.3],
    [1.0, 0.6, 0.2],
    [1.0, 0.3, 0.25],
    [1.0, 0.85, 0.35],
];

/// Reads the HUD's textures from the ROM.
pub fn load(rom: &[u8]) {
    let (Some(d), Some(seg2)) = (crate::gameover::mio0(rom, BLOCK), crate::gameover::mio0(rom, SEGMENT2)) else {
        log("hud: HUD textures not found in the ROM");
        return;
    };
    let tex = |d: &[u8], off: usize, len: usize| d.get(off..off + len).map(<[u8]>::to_vec);
    let load = || -> Option<Vec<Image>> {
        let (left, right) = (tex(&d, LEFT, 4096)?, tex(&d, RIGHT, 4096)?);
        // the frame: two 32x64 halves side by side
        let base = Image::new(64, 64, |x, y| if x < 32 { rgba16_px(&left, y * 32 + x) } else { rgba16_px(&right, y * 32 + x - 32) });
        let mut out = vec![base];
        for &o in &PIES {
            out.push(rgba16(&tex(&d, o, 2048)?, 32, 32));
        }
        let glyph = |off: usize| tex(&seg2, off, 512).map(|t| rgba16(&t, 16, 16));
        for i in 0..10 {
            out.push(glyph(i * 0x200)?);
        }
        for off in [GLYPH_MULTIPLY, GLYPH_COIN, GLYPH_HEAD, GLYPH_STAR] {
            out.push(glyph(off)?);
        }
        for &o in &COIN_FRAMES {
            out.push(ia16(&tex(&d, o, 2048)?, 32, 32, [255, 255, 0]));
        }
        // the game-over mask: black, soft edges (smooth filtering: no upscale needed, but keep
        // the same layout)
        let alpha = crate::gameover::mask_alpha(rom)?;
        out.push(Image::new(64, 64, |x, y| [0, 0, 0, alpha[y * 64 + x]]));
        for &(_, off) in &LETTERS {
            out.push(glyph(off)?);
        }
        out.push(glyph(0x3E00)?); // apostrophe
        out.push(glyph(0x7000)?); // camera
        out.push(glyph(0x7200)?); // Lakitu
        out.push(tex(&seg2, 0x7600, 128).map(|t| rgba16(&t, 8, 8))?); // arrow up
        out.push(tex(&seg2, 0x7680, 128).map(|t| rgba16(&t, 8, 8))?); // arrow down
        // the flame: white-hot inside, orange at the edges (tinted per charge level when drawn)
        for k in 0..8 {
            let t = tex(&d, FLAME_FRAMES + k * 0x800, 2048)?;
            out.push(Image::new(32, 32, |x, y| {
                let (v, a) = (t[(y * 32 + x) * 2] as f32 / 255.0, t[(y * 32 + x) * 2 + 1]);
                let mix = |lo: f32, hi: f32| (lo + (hi - lo) * v) as u8;
                [255, mix(110.0, 250.0), mix(20.0, 210.0), a]
            }));
        }
        let boxes = crate::gameover::mio0(rom, COMMON0)?;
        for &o in &CAP_BOXES {
            out.push(rgba16(&tex(&boxes, o, 2048)?, 32, 32));
        }
        // a blow's star burst (impact.rs), drawn in SM64's chunky pixels: a small star, the full
        // burst, then only its rays flying out
        for (core, axis, diagonal, hollow) in [(2.5f32, 5.0f32, 0.0f32, 0.0f32), (3.0, 8.0, 6.0, 0.0), (0.0, 8.0, 7.0, 4.5)] {
            out.push(Image::new(16, 16, |x, y| {
                let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
                let r = (dx * dx + dy * dy).sqrt();
                let on_axis = (dx.abs() < 1.0 || dy.abs() < 1.0) && r <= axis;
                let on_diagonal = (dx.abs() - dy.abs()).abs() < 1.0 && r <= diagonal;
                if r < hollow || !(r <= core || on_axis || on_diagonal) {
                    return [0; 4];
                }
                if r <= core + 1.0 { [255, 255, 255, 255] } else { [255, 236, 96, 255] }
            }));
        }
        // a soft round glow (tinted when drawn)
        out.push(Image::new(32, 32, |x, y| {
            let r = ((x as f32 - 15.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt() / 16.0;
            [255, 255, 255, ((1.0 - r).max(0.0).powi(2) * 255.0) as u8]
        }));
        // the giant fireball: a lit ball of fire, its surface churning over 4 frames
        for frame in 0..4 {
            out.push(Image::new(48, 48, |x, y| orb_pixel(x, y, frame)));
        }
        out.push(Image::new(32, 32, rang_pixel));
        for open in [false, true] {
            out.push(Image::new(48, 48, |x, y| chomp_pixel(x, y, open)));
        }
        out.push(Image::new(16, 16, link_pixel));
        out.push(Image::new(32, 32, hammer_icon_pixel));
        out.push(Image::new(32, 32, mushroom_pixel));
        out.push(Image::new(32, 32, cappy_pixel));
        out.push(Image::new(24, 32, egg_pixel));
        // (Mario's red cap and shirt turned green: his reds have far more red than green)
        let head = tex(&seg2, GLYPH_HEAD, 512)?;
        out.push(Image::new(16, 16, |x, y| {
            let [r, g, b, a] = rgba16_px(&head, y * 16 + x);
            if r as u32 > g as u32 * 3 / 2 + 20 && r as u32 > b as u32 * 3 / 2 + 20 { [g / 2, r, b / 2, a] } else { [r, g, b, a] }
        }));
        for k in 0..4 {
            out.push(Image::new(32, 32, |x, y| kit_pixel(k, x, y)));
        }
        out.push(Image::new(32, 32, rock_pixel));
        // the X: two strokes crossing, filled and outlined with the K's own colours
        {
            let k = tex(&seg2, LETTERS.iter().find(|l| l.0 == 'K').map_or(0x2600, |l| l.1), 512)?;
            let px = |i: usize| rgba16_px(&k, i);
            let opaque: Vec<[u8; 4]> = (0..256).map(px).filter(|p| p[3] > 0).collect();
            let lum = |p: &[u8; 4]| p[0] as u32 + p[1] as u32 + p[2] as u32;
            let fill = opaque.iter().max_by_key(|p| lum(p)).copied().unwrap_or([255, 255, 255, 255]);
            let edge = opaque.iter().min_by_key(|p| lum(p)).copied().unwrap_or([0, 0, 0, 255]);
            out.push(Image::new(16, 16, |x, y| {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let d = ((fx - fy).abs()).min((fx + fy - 16.0).abs()) / std::f32::consts::SQRT_2;
                let inside = (2.0..=14.0).contains(&fx) && (2.0..=14.0).contains(&fy);
                if inside && d < 1.6 { fill } else if inside && d < 2.6 { edge } else { [0; 4] }
            }));
        }
        Some(out)
    };
    match load() {
        Some(images) => {
            log(format!("hud: SM64 HUD loaded ({} textures)", images.len()));
            *IMAGES.lock().unwrap_or_else(|e| e.into_inner()) = Some(images);
        }
        None => log("hud: HUD textures incomplete in the ROM"),
    }
}

// ---- what to show (set by the Mario frame) ------------------------------------------------------

/// wedges 0..=8, or 0xFF: no HUD
/// The first-launch setup box: what's happening, progress (None: no bar), a line of detail.
pub struct Setup {
    pub title: String,
    pub progress: Option<f32>,
    pub text: String,
}

static SETUP: Mutex<Option<Setup>> = Mutex::new(None);

/// Shows (Some) or hides (None) the first-launch setup box.
pub fn set_setup(setup: Option<Setup>) {
    *SETUP.lock().unwrap_or_else(|e| e.into_inner()) = setup;
}

/// Shows the setup box with a progress bar.
pub fn setup_progress(title: &str, progress: f32, text: &str) {
    set_setup(Some(Setup { title: title.into(), progress: Some(progress), text: text.into() }));
}

static WEDGES: AtomicU8 = AtomicU8::new(0xFF);
static DEATHS: AtomicU32 = AtomicU32::new(0);
static COINS: AtomicU32 = AtomicU32::new(0);
static STARS: AtomicU32 = AtomicU32::new(0);
/// when the Mario frame last reported (the HUD goes when it stops, e.g. on a loading screen)
static LAST_SET: Mutex<Option<Instant>> = Mutex::new(None);

/// `hidden`: why the HUD should go (None: it stays). A reason must last HIDE_AFTER before the HUD
/// goes (a one-frame "menu" guess or pause blip made it flicker); death and loading hide at once.
pub fn set(wedges: u8, hidden: Option<&'static str>, active: bool) {
    const HIDE_AFTER: f32 = 0.15;
    static SINCE: Mutex<Option<(Instant, &'static str)>> = Mutex::new(None);
    static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    let mut since = SINCE.lock().unwrap_or_else(|e| e.into_inner());
    let hide = match hidden {
        None => {
            *since = None;
            false
        }
        Some(why) => {
            let (t, _) = *since.get_or_insert((Instant::now(), why));
            why == "dead" || why == "loading" || t.elapsed().as_secs_f32() >= HIDE_AFTER
        }
    };
    let show = active && !hide;
    if SHOWN.swap(show, Ordering::Relaxed) != show {
        crate::log(format!("hud: {}", if show { "shown".to_string() } else { format!("hidden ({})", hidden.unwrap_or("inactive")) }));
    }
    WEDGES.store(if show { wedges } else { 0xFF }, Ordering::Relaxed);
    *LAST_SET.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

pub fn set_counters(deaths: u32, coins: u32, stars: u32) {
    DEATHS.store(deaths, Ordering::Relaxed);
    COINS.store(coins, Ordering::Relaxed);
    STARS.store(stars, Ordering::Relaxed);
}

/// An enemy health bar (combat.rs): feet position, HP now and before the combo (0..1), combo damage,
/// the status built up on him or on him now (element.rs)
pub struct Tag {
    pub pos: Vec3,
    pub hp: f32,
    pub before: f32,
    pub dmg: i32,
    pub status: Option<crate::element::Bar>,
}

static TAGS: Mutex<Vec<Tag>> = Mutex::new(Vec::new());

/// A boss health bar (the bottom of the screen): name, HP now / before the combo, combo damage.
pub struct BossBar {
    pub name: String,
    pub hp: f32,
    pub before: f32,
    pub dmg: i32,
    pub status: Option<crate::element::Bar>,
}

static BOSSES: Mutex<Vec<BossBar>> = Mutex::new(Vec::new());

pub fn set_bosses(bosses: Vec<BossBar>) {
    *BOSSES.lock().unwrap_or_else(|e| e.into_inner()) = bosses;
}

pub fn set_tags(tags: Vec<Tag>) {
    *TAGS.lock().unwrap_or_else(|e| e.into_inner()) = tags;
}

/// A co-op partner's card (coop.rs): the top of their head (Elden Ring; None: not near), their
/// wedges, Luigi or Mario, their power (coop::POWER_), coins and style rank, how shown (0-1), in
/// danger (low or just hurt), down, and their coin bag (where, Elden Ring, and its coins).
#[derive(Clone, Copy)]
pub struct PartnerHud {
    pub head: Option<Vec3>,
    pub wedges: u8,
    pub luigi: bool,
    pub power: u8,
    pub coins: u16,
    pub rank: Option<u8>,
    pub alpha: f32,
    pub danger: bool,
    pub down: bool,
    pub bag: Option<(Vec3, u16)>,
    /// their wing cap on: their head part's centre, which way they face, their size there
    pub wings: Option<(Vec3, glam::Quat, f32)>,
    /// down for us to revive (revive.rs), the seconds they have left, and our revive on them (0-1:
    /// within reach, how long D-pad down has been held)
    pub revivable: bool,
    pub down_left: Option<u8>,
    pub revive: Option<f32>,
    /// the weapon in their hand (weapons.rs): its element's badge on their card
    pub weapon: u8,
}

/// (Seamless Co-op takes up to four players)
const MAX_PARTNERS: usize = 3;
static PARTNERS: Mutex<[Option<PartnerHud>; MAX_PARTNERS]> = Mutex::new([None; MAX_PARTNERS]);

/// The trap kit while it's up (LT held, crouching): how ready each of its buttons' things is
/// (X, Y, B, RB; 1 ready), and since when it's up.
static KIT: Mutex<Option<([f32; 4], Instant)>> = Mutex::new(None);

pub fn set_kit(ready: Option<[f32; 4]>) {
    let mut k = KIT.lock().unwrap_or_else(|e| e.into_inner());
    *k = match (ready, k.take()) {
        (Some(r), Some((_, since))) => Some((r, since)),
        (Some(r), None) => Some((r, Instant::now())),
        (None, _) => None,
    };
}

/// Co-op, ours: down for the partner to revive (over our head, the seconds left); the partner too
/// far off (m, leash.rs).
static COOP: Mutex<(Option<(Vec3, u8)>, Option<f32>)> = Mutex::new((None, None));

pub fn set_coop(down: Option<(Vec3, u8)>, far: Option<f32>) {
    *COOP.lock().unwrap_or_else(|e| e.into_inner()) = (down, far);
}

pub fn set_partners(partners: Vec<PartnerHud>) {
    let mut p = PARTNERS.lock().unwrap_or_else(|e| e.into_inner());
    *p = [None; MAX_PARTNERS];
    for (slot, partner) in p.iter_mut().zip(partners) {
        *slot = Some(partner);
    }
}

/// `n` in decimal into `buf`, as text (no allocation: the HUD draws every frame).
/// An element's badge (element.rs): a dark disc ringed in its colour with its sign in it: one of
/// SM64's flames flickering, a snowflake, a drop of blood, bubbles. `c` its centre, `r` its radius.
fn element_badge(dl: &imgui::draw_list::DrawListMut<'_>, textures: &[TextureId], c: [f32; 2], r: f32, el: crate::element::Element, alpha: f32, clock: f32) {
    use crate::element::Element;
    let t = el.tint();
    let col = |rgb: [f32; 3], a: f32| [rgb[0], rgb[1], rgb[2], a * alpha];
    dl.add_circle(c, r, [0.0, 0.0, 0.0, 0.55 * alpha]).filled(true).build();
    dl.add_circle(c, r, col(t, 0.9)).thickness((r * 0.13).max(1.0)).build();
    let at = |x: f32, y: f32| [c[0] + x * r, c[1] + y * r];
    match el {
        Element::Fire => {
            let k = r * 0.78;
            let frame = (clock * 12.0) as usize % 8;
            dl.add_image(textures[IMG_FLAME + frame], [c[0] - k, c[1] - k * 1.05], [c[0] + k, c[1] + k * 0.95]).col([1.0, 1.0, 1.0, alpha]).build();
        }
        Element::Frost => {
            let w = (r * 0.14).max(1.0);
            let ice = col([0.85, 0.96, 1.0], 1.0);
            for k in 0..3 {
                let a = std::f32::consts::PI * (0.5 + k as f32 / 3.0);
                let (dx, dy) = (a.cos(), a.sin());
                dl.add_line(at(dx * 0.62, dy * 0.62), at(-dx * 0.62, -dy * 0.62), ice).thickness(w).build();
                for s in [1.0f32, -1.0] {
                    let (bx, by) = (dx * 0.36 * s, dy * 0.36 * s);
                    for turn in [0.7f32, -0.7] {
                        let (ca, sa) = (turn.cos(), turn.sin());
                        let (ex, ey) = (dx * s * ca - dy * s * sa, dx * s * sa + dy * s * ca);
                        dl.add_line(at(bx, by), at(bx + ex * 0.22, by + ey * 0.22), ice).thickness(w * 0.8).build();
                    }
                }
            }
        }
        Element::Blood => {
            let red = col([0.78, 0.06, 0.1], 1.0);
            dl.add_circle(at(0.0, 0.2), r * 0.38, red).filled(true).build();
            dl.add_triangle(at(0.0, -0.62), at(-0.33, 0.08), at(0.33, 0.08), red).filled(true).build();
            dl.add_circle(at(-0.14, 0.12), r * 0.1, col([1.0, 0.75, 0.75], 0.7)).filled(true).build();
        }
        Element::Poison => {
            for (x, y, b) in [(-0.27f32, 0.2f32, 0.3f32), (0.26, 0.12, 0.24), (0.0, -0.32, 0.2)] {
                dl.add_circle(at(x, y), r * b, col([0.5, 0.85, 0.25], 1.0)).filled(true).build();
                dl.add_circle(at(x, y), r * b, col([0.55, 0.25, 0.7], 1.0)).thickness((r * 0.08).max(1.0)).build();
                dl.add_circle(at(x - b * 0.35, y - b * 0.35), r * b * 0.28, col([1.0, 1.0, 1.0], 0.75)).filled(true).build();
            }
        }
    }
}

/// A status bar under a health bar (element.rs): `x0`, `y0` its top left, `w` wide, `h` tall; built
/// up it fills in the status's colour, on him it shows its time left, glowing.
fn status_bar(dl: &imgui::draw_list::DrawListMut<'_>, x0: f32, y0: f32, w: f32, h: f32, bar: &crate::element::Bar, clock: f32) {
    let t = bar.tint;
    dl.add_rect([x0 - 1.0, y0 - 1.0], [x0 + w + 1.0, y0 + h + 1.0], [0.0, 0.0, 0.0, 0.6]).filled(true).build();
    let a = if bar.on { 0.75 + 0.25 * (clock * 7.0).sin() } else { 0.85 };
    dl.add_rect([x0, y0], [x0 + w * bar.fill.clamp(0.0, 1.0), y0 + h], [t[0], t[1], t[2], a]).filled(true).build();
}

fn digits_of(n: u16, buf: &mut [u8; 5]) -> &str {
    let mut k = buf.len();
    let mut n = n;
    loop {
        k -= 1;
        buf[k] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[k..]).unwrap_or("0")
}

/// `n` metres as "123M" into `buf` (no allocation).
fn metres_of(n: u16, buf: &mut [u8; 6]) -> &str {
    let mut digits = [0u8; 5];
    let len = digits_of(n, &mut digits).len();
    buf[..len].copy_from_slice(&digits[5 - len..]);
    buf[len] = b'M';
    std::str::from_utf8(&buf[..len + 1]).unwrap_or("M")
}

// ---- the overlay ---------------------------------------------------------------------------------

pub struct Overlay {
    textures: Vec<TextureId>,
    meter: Option<Meter>,
}

/// The power meter's motion (SM64's: damage brings it down, healing fills it where it is).
struct Meter {
    /// wedges last frame
    wedges: u8,
    /// when Mario last lost health (the meter came down then)
    hit: Instant,
    /// when he got back to full health (it slides out then), and its height at that moment
    full: Option<(Instant, f32)>,
}

/// The steps the overlay library takes to find the renderer's functions (a throwaway device,
/// command queue and swap chain), one by one with what each returns. Only run when the overlay
/// didn't start: the log then says which step it is.
pub fn probe_dx12() {
    use hudhook::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
    use hudhook::windows::Win32::Graphics::Direct3D12::{
        D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12_COMMAND_QUEUE_FLAG_NONE, D3D12CreateDevice, ID3D12CommandQueue, ID3D12Device,
    };
    use hudhook::windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED, DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
    };
    use hudhook::windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH, DXGI_SWAP_EFFECT_FLIP_DISCARD,
        DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIFactory2, IDXGISwapChain, IDXGISwapChain1, IDXGISwapChain2, IDXGISwapChain3,
    };
    use hudhook::windows::core::{BOOL, Interface};
    let step = |name: &str, result: String| log(format!("overlay probe: {name}: {result}"));
    let hwnd = hudhook::hooks::DummyHwnd::new();
    step("window", "ok".into());
    let factory: IDXGIFactory2 = match unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) } {
        Ok(f) => f,
        Err(e) => return step("DXGI factory", format!("{e:?}")),
    };
    step("DXGI factory", "ok".into());
    let adapter = match unsafe { factory.EnumAdapters(0) } {
        Ok(a) => a,
        Err(e) => return step("adapter 0", format!("{e:?}")),
    };
    step("adapter 0", "ok".into());
    let mut device: Option<ID3D12Device> = None;
    if let Err(e) = unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) } {
        return step("D3D12 device", format!("{e:?}"));
    }
    let Some(device) = device else { return step("D3D12 device", "no device returned".into()) };
    step("D3D12 device", "ok".into());
    let queue: ID3D12CommandQueue = match unsafe {
        device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, Priority: 0, Flags: D3D12_COMMAND_QUEUE_FLAG_NONE, NodeMask: 0 })
    } {
        Ok(q) => q,
        Err(e) => return step("command queue", format!("{e:?}")),
    };
    step("command queue", "ok".into());
    let desc = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
            Width: 640,
            Height: 480,
            RefreshRate: DXGI_RATIONAL { Numerator: 60, Denominator: 1 },
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        OutputWindow: hwnd.hwnd(),
        Windowed: BOOL(1),
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH.0 as _,
    };
    let mut chain: Option<IDXGISwapChain> = None;
    let hr = unsafe { factory.CreateSwapChain(&queue, &desc, &mut chain) };
    let Some(chain) = chain.filter(|_| hr.is_ok()) else { return step("swap chain", format!("{hr:?}")) };
    step("swap chain", "ok".into());
    step("IDXGISwapChain1", format!("{:?}", chain.cast::<IDXGISwapChain1>().map(|_| "ok")));
    step("IDXGISwapChain2", format!("{:?}", chain.cast::<IDXGISwapChain2>().map(|_| "ok")));
    step("IDXGISwapChain3", format!("{:?}", chain.cast::<IDXGISwapChain3>().map(|_| "ok")));
}

/// Starts the overlay (hooks the game's DirectX 12 presentation).
/// True if it's hooked.
pub fn install(module: usize) -> bool {
    use hudhook::hooks::dx12::ImguiDx12Hooks;
    let hmodule = hudhook::windows::Win32::Foundation::HINSTANCE(module as _);
    let overlay = Overlay { textures: Vec::new(), meter: None };
    let _ = hudhook::NOTE.set(|msg| log(msg));
    // test switch: start the overlay the way it has to on CrossOver (see vendor/hudhook)
    if crate::paths::config("overlay_skip_ecl").is_some_and(|v| v.trim() == "1") {
        unsafe { std::env::set_var("HUDHOOK_SKIP_ECL", "1") };
    }
    match hudhook::Hudhook::builder().with::<ImguiDx12Hooks>(overlay).with_hmodule(hmodule).build().apply() {
        Ok(()) => {
            log("hud: overlay hooked");
            true
        }
        Err(e) => {
            log(format!("hud: overlay hook failed: {e:?}"));
            false
        }
    }
}

static BAG_START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// A Mario's effects in the world (fx.rs: ours, or a co-op partner's), drawn over the game:
/// fireballs, flying enemies' streaks, the boomerangs' streaks, Yoshi's tongue and eggs, the hook,
/// the blows' star bursts, the dash, the hammer's and the Chain Chomp's ribbons and the chain's
/// sparks, rings on the ground, stars and coins flying off the blows. `bodies`: spheres the 3D
/// tongue and eggs are hidden behind.
fn draw_fx(dl: &imgui::draw_list::DrawListMut<'_>, textures: &[TextureId], size: [f32; 2], fx: &crate::fx::WorldFx, bodies: &[(Vec3, f32)]) {
    // Mario's fireball: the charge at his hand, the shots, their bursts, the lava pools
    draw_fire(dl, textures, size, &fx.fire, &fx.rubble);

    // enemies flying through the air (juggle.rs): a white streak behind each, fading
    for streak in &fx.streaks {
        for pair in streak.windows(2) {
            let ((a, ta), (b, tb)) = (pair[0], pair[1]);
            let (Some((pa, za)), Some((pb, _))) = (project(a, size), project(b, size)) else { continue };
            if za > 50.0 {
                continue;
            }
            let fade = 1.0 - ta.max(tb);
            let width = (size[1] / za.max(1.0) * 0.05).clamp(1.0, 10.0) * fade;
            dl.add_line(pa, pb, imgui::ImColor32::from_rgba_f32s(1.0, 1.0, 1.0, 0.45 * fade)).thickness(width).build();
        }
    }
    // the boomerangs (models of their own, engine_mario): a streak of air behind each, thin
    // and fading
    for (pts, rsize) in &fx.rangs {
        let n = pts.len().saturating_sub(1);
        for (k, pair) in pts.windows(2).enumerate() {
            let (Some((a, za)), Some((b, _))) = (project(pair[0], size), project(pair[1], size)) else { continue };
            if za > 60.0 {
                continue;
            }
            let fade = (k + 1) as f32 / (n + 1) as f32;
            let width = (size[1] / za.max(1.0) * 0.035 * rsize / 0.55).clamp(1.0, 12.0) * fade;
            dl.add_line(a, b, imgui::ImColor32::from_rgba_f32s(1.0, 1.0, 1.0, 0.3 * fade)).thickness(width).build();
            dl.add_line(a, b, imgui::ImColor32::from_rgba_f32s(1.0, 0.6, 0.5, 0.25 * fade)).thickness(width * 0.35).build();
        }
    }
    // on Yoshi: his tongue (pink, a round tip) and the eggs flying, tumbling
    if let Some((mouth, tip)) = fx.tongue {
        // a tube from his mouth that sags a little, a round tip
        let n = 12;
        let len = (tip - mouth).length();
        let path: Vec<Vec3> = (0..=n)
            .map(|k| {
                let t = k as f32 / n as f32;
                mouth + (tip - mouth) * t - Vec3::Y * (t * (1.0 - t) * len * 0.12)
            })
            .collect();
        let mut mesh = Mesh::default();
        mesh.tube(&path, &|k| 0.13 - 0.02 * k as f32 / n as f32, 8, &|k| if k % 3 == 0 { [0.98, 0.4, 0.55] } else { [0.93, 0.33, 0.5] });
        mesh.ellipsoid(*path.last().unwrap_or(&tip), glam::Quat::IDENTITY, Vec3::splat(0.22), 0.0, 6, 10, &|i, _| if i < 2 { [1.0, 0.6, 0.72] } else { [0.98, 0.42, 0.58] });
        mesh.draw(dl, size, 60.0, bodies);
    }
    for &(pos, age) in &fx.yoshi_fires {
        let (Some((c, z)), Some((top, _))) = (project(pos, size), project(pos + Vec3::Y * 0.3, size)) else { continue };
        let h = (c[1] - top[1]) * 2.0;
        if z > 60.0 || h < 1.0 {
            continue;
        }
        let frame = (age * 24.0) as usize % 8;
        dl.add_image(textures[IMG_GLOW], [c[0] - h, c[1] - h], [c[0] + h, c[1] + h]).col([1.0, 0.55, 0.15, 0.6]).build();
        dl.add_image(textures[IMG_FLAME + frame], [c[0] - h * 0.5, c[1] - h * 0.5], [c[0] + h * 0.5, c[1] + h * 0.5]).build();
    }
    // the eggs: white with green spots, tumbling, in 3D
    for &(pos, age) in &fx.eggs {
        let sz = crate::ride::EGG_SIZE;
        let rot = glam::Quat::from_euler(glam::EulerRot::YXZ, age * 9.0, age * 13.0, 0.0);
        let mut mesh = Mesh::default();
        mesh.ellipsoid(pos, rot, Vec3::new(0.38, 0.5, 0.38) * sz * 2.0, 0.22, 8, 12, &|i, j| {
            if (matches!(i, 2 | 3) && j % 4 == 0) || (i == 5 && j % 4 == 2) || (i == 4 && j % 6 == 3) { [0.2, 0.72, 0.25] } else { [0.98, 0.97, 0.9] }
        });
        mesh.draw(dl, size, 60.0, bodies);
    }
    // the hook: a line of light from his hand with twinkles running along it, his cap spinning
    // at its end
    // (the Chomp as the hook: its chain, dark iron links along the line)
    if let Some((hand, head)) = fx.chain {
        let n = ((head - hand).length() / 0.22).clamp(2.0, 120.0) as usize;
        for k in 0..n {
            let p = hand + (head - hand) * ((k as f32 + 0.5) / n as f32);
            let Some((c, z)) = project(p, size) else { continue };
            if z > 60.0 {
                continue;
            }
            let r = (size[1] / z.max(1.0) * 0.045).clamp(1.0, 9.0);
            let (w, h) = if k % 2 == 0 { (r, r * 0.55) } else { (r * 0.45, r) };
            dl.add_rect([c[0] - w, c[1] - h], [c[0] + w, c[1] + h], imgui::ImColor32::from_rgba_f32s(0.16, 0.16, 0.18, 0.95)).rounding(h).thickness((r * 0.35).max(1.0)).build();
        }
    }
    if let Some((hand, head, spin)) = fx.hook {
        if let (Some((a, za)), Some((b, zb))) = (project(hand, size), project(head, size)) {
            if za < 60.0 && zb < 60.0 {
                let width = (size[1] / za.min(zb).max(1.0) * 0.025).clamp(1.0, 5.0);
                dl.add_line(a, b, imgui::ImColor32::from_rgba_f32s(1.0, 0.85, 0.3, 0.85)).thickness(width).build();
                dl.add_line(a, b, imgui::ImColor32::from_rgba_f32s(1.0, 1.0, 0.9, 0.9)).thickness((width * 0.4).max(1.0)).build();
                for k in 0..4 {
                    let t = ((spin * 0.25 + k as f32 * 0.25) % 1.0).abs();
                    let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                    dl.add_circle(p, width * 1.1, imgui::ImColor32::from_rgba_f32s(1.0, 1.0, 0.8, 0.9)).filled(true).build();
                }
            }
        }
        if let (Some((c, z)), Some((top, _))) = (project(head, size), project(head + Vec3::Y * 0.22, size)) {
            let h = (c[1] - top[1]) * 2.0;
            if z < 60.0 && h > 1.0 {
                // (spinning about its crown: narrower and wider, flipped half the turn)
                let w = h * spin.cos();
                let (u0, u1) = if w >= 0.0 { (0.0, 1.0) } else { (1.0, 0.0) };
                let w = w.abs().max(h * 0.25);
                dl.add_image(textures[IMG_CAPPY], [c[0] - w * 0.5, c[1] - h * 0.5], [c[0] + w * 0.5, c[1] + h * 0.5]).uv_min([u0, 0.0]).uv_max([u1, 1.0]).build();
            }
        }
    }
    // the blows that landed: a star burst each, growing a little as it goes
    for spark in &fx.sparks {
        let t = spark.t;
        let s = spark.size * (0.7 + 0.5 * t);
        let (Some((c, z)), Some((top, _))) = (project(spark.pos, size), project(spark.pos + Vec3::Y * s * 0.5, size)) else { continue };
        let h = (c[1] - top[1]) * 2.0;
        if z > 60.0 || h < 1.0 {
            continue;
        }
        let frame = ((t * 3.0) as usize).min(2);
        dl.add_image(textures[IMG_SPARK + frame], [c[0] - h * 0.5, c[1] - h * 0.5], [c[0] + h * 0.5, c[1] + h * 0.5]).col(spark.tint).build();
    }
    // the dash: streaks along Mario's path, fading behind him, and puffs where it started
    for pair in fx.dash.windows(2) {
        let ((a, ta), (b, tb)) = (pair[0], pair[1]);
        let fade = 1.0 - ta.max(tb);
        for (k, up) in [0.25f32, 0.55, 0.85].into_iter().enumerate() {
            let (Some((pa, za)), Some((pb, _))) = (project(a + Vec3::Y * up, size), project(b + Vec3::Y * up, size)) else { continue };
            if za > 40.0 {
                continue;
            }
            let width = (size[1] / za.max(1.0) * 0.025).clamp(1.0, 6.0) * if k == 1 { 1.4 } else { 0.8 };
            let col = imgui::ImColor32::from_rgba_f32s(0.85, 0.95, 1.0, 0.55 * fade);
            dl.add_line(pa, pb, col).thickness(width).build();
        }
    }
    for &(p, t) in &fx.puffs {
        let s = 0.5 + 0.9 * t;
        let (Some((c, z)), Some((top, _))) = (project(p, size), project(p + Vec3::Y * s * 0.5, size)) else { continue };
        let h = (c[1] - top[1]) * 2.0;
        if z > 40.0 || h < 1.0 {
            continue;
        }
        dl.add_image(textures[IMG_GLOW], [c[0] - h * 0.5, c[1] - h * 0.5], [c[0] + h * 0.5, c[1] + h * 0.5])
            .col([0.85, 0.85, 0.8, 0.6 * (1.0 - t)])
            .build();
    }

    // the combos (impact.rs): the hammer's swing as a fading ribbon from its handle to its head
    for pair in fx.swing.windows(2) {
        let ((h0, m0, t0), (h1, m1, t1)) = (pair[0], pair[1]);
        let pts: Option<Vec<[f32; 2]>> = [h0, h1, m1, m0].iter().map(|p| project(*p, size).filter(|(_, z)| *z < 40.0).map(|(c, _)| c)).collect();
        let Some(pts) = pts else { continue };
        let fade = 1.0 - t0.max(t1);
        let edge = (pts[0], pts[1]);
        dl.add_polyline(pts, imgui::ImColor32::from_rgba_f32s(1.0, 0.97, 0.85, 0.35 * fade)).filled(true).build();
        dl.add_line(edge.0, edge.1, imgui::ImColor32::from_rgba_f32s(1.0, 1.0, 1.0, 0.8 * fade)).thickness(2.5).build();
    }
    // the Chain Chomp's ribbon of fire (Blades of Chaos): a hot core over a wider red glow, its
    // older end cooling and thinning, flames licking along it; and the sparks off its chain
    for (k, pair) in fx.chomp.windows(2).enumerate() {
        let ((h0, m0, t0), (h1, m1, t1)) = (pair[0], pair[1]);
        // (two swings: no ribbon across the gap between them; a hit-stop's pause is no gap)
        if (t0 - t1).abs() * crate::impact::CHOMP_TRAIL_TIME > 0.2 {
            continue;
        }
        let pts: Option<Vec<[f32; 2]>> = [h0, h1, m1, m0].iter().map(|p| project(*p, size).filter(|(_, z)| *z < 45.0).map(|(c, _)| c)).collect();
        let Some(pts) = pts else { continue };
        let fade = 1.0 - t0.max(t1);
        dl.add_polyline(pts.clone(), imgui::ImColor32::from_rgba_f32s(0.9, 0.22, 0.05, 0.35 * fade)).filled(true).build();
        let core: Vec<[f32; 2]> = vec![pts[0], pts[1], lerp2(pts[1], pts[2], 0.45), lerp2(pts[0], pts[3], 0.45)];
        dl.add_polyline(core, imgui::ImColor32::from_rgba_f32s(1.0, 0.62 + 0.3 * fade, 0.2 + 0.4 * fade, 0.55 * fade)).filled(true).build();
        dl.add_line(pts[0], pts[1], imgui::ImColor32::from_rgba_f32s(1.0, 0.95, 0.75, 0.85 * fade)).thickness(2.5).build();
        if k % 2 == 0 {
            if let (Some((c, z)), Some((top, _))) = (project(h1, size), project(h1 + Vec3::Y * 0.35, size)) {
                let h = (c[1] - top[1]) * 2.0 * (0.6 + 0.6 * fade);
                if z < 45.0 && h > 1.0 {
                    let frame = (k + (t1 * 24.0) as usize) % 8;
                    dl.add_image(textures[IMG_FLAME + frame], [c[0] - h * 0.5, c[1] - h * 0.6], [c[0] + h * 0.5, c[1] + h * 0.4]).col([1.0, 0.7, 0.35, 0.8 * fade]).build();
                }
            }
        }
    }
    for &(now, before, t) in &fx.flying {
        let (Some((a, za)), Some((b, _))) = (project(now, size), project(before, size)) else { continue };
        if za > 40.0 {
            continue;
        }
        let width = (size[1] / za.max(1.0) * 0.012).clamp(1.0, 3.0);
        dl.add_line(a, b, imgui::ImColor32::from_rgba_f32s(1.0, 0.9 - 0.4 * t, 0.45 - 0.3 * t, 1.0 - t)).thickness(width).build();
    }
    // shockwave rings on the ground, widening and fading
    for r in &fx.rings {
        let t = r.t;
        let radius = r.radius * (0.25 + 0.75 * (1.0 - (1.0 - t) * (1.0 - t)));
        let pts: Vec<[f32; 2]> = (0..=20)
            .filter_map(|k| {
                let a = k as f32 / 20.0 * std::f32::consts::TAU;
                project(r.pos + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius), size).filter(|(_, z)| *z < 50.0).map(|(c, _)| c)
            })
            .collect();
        if pts.len() < 21 {
            continue;
        }
        let Some((_, z)) = project(r.pos, size) else { continue };
        let width = (size[1] / z.max(1.0) * 0.06).clamp(1.5, 10.0) * (1.0 - t * 0.6);
        let [cr, cg, cb, ca] = r.tint;
        dl.add_polyline(pts, imgui::ImColor32::from_rgba_f32s(cr, cg, cb, ca * 0.8 * (1.0 - t))).thickness(width).build();
    }
    // stars and coins flying off the blows
    for b in &fx.bits {
        let (p, s, t, age) = (b.pos, b.size, b.t, b.age);
        let (Some((c, z)), Some((top, _))) = (project(p, size), project(p + Vec3::Y * s * 0.5, size)) else { continue };
        let h = (c[1] - top[1]) * 2.0;
        if z > 50.0 || h < 1.0 {
            continue;
        }
        let fade = if t > 0.75 { (1.0 - t) / 0.25 } else { 1.0 };
        let img = if b.coin { IMG_COIN_FRAMES + (age * 15.0) as usize % 4 } else { IMG_STAR };
        dl.add_image(textures[img], [c[0] - h * 0.5, c[1] - h * 0.5], [c[0] + h * 0.5, c[1] + h * 0.5]).col([1.0, 1.0, 1.0, fade]).build();
    }
}

/// On a colossus (colossus.rs): its weak points, faint gold stars on it (barely there on its far side),
/// the spot a leap would take him to (a faint ring), and his grip, a ring by him that empties,
/// turns red when low and trembles while it thrashes (Shadow of the Colossus's), the star strike's
/// charge filling inside it.
fn draw_colossus(dl: &imgui::draw_list::DrawListMut<'_>, textures: &[TextureId], size: [f32; 2]) {
    let Some(h) = crate::colossus::hud() else { return };
    let t = crate::flame::clock();
    let eye = camera_position();
    for &(p, out, left) in &h.stars {
        let (Some((c, z)), Some((top, _))) = (project(p, size), project(p + Vec3::Y * 0.5, size)) else { continue };
        if z > 80.0 {
            continue;
        }
        let px = ((c[1] - top[1]) * 2.0).clamp(8.0, 48.0);
        // (subtle, a hint on its body more than a sign: faint, a touch clearer as he nears it;
        // on the far side of its body, barely there)
        let behind = eye.is_some_and(|e| out.dot((e - p).normalize_or_zero()) < -0.15);
        let near = 1.0 - ((p - h.mario).length() / 6.0).clamp(0.0, 1.0);
        let a = if behind { 0.1 } else { 0.32 + 0.28 * near } * (0.6 + 0.4 * left);
        let pulse = 1.0 + 0.08 * (t * 3.0).sin();
        let g = px * 1.0 * pulse;
        dl.add_image(textures[IMG_GLOW], [c[0] - g, c[1] - g], [c[0] + g, c[1] + g]).col([1.0, 0.85, 0.4, 0.3 * a]).build();
        let s = px * 0.42 * pulse;
        dl.add_image(textures[IMG_STAR], [c[0] - s, c[1] - s], [c[0] + s, c[1] + s]).col([1.0, 1.0, 1.0, a]).build();
    }
    if let Some((c, _)) = h.next.and_then(|n| project(n, size)) {
        dl.add_circle(c, 9.0 + 2.0 * (t * 4.0).sin(), imgui::ImColor32::from_rgba_f32s(1.0, 1.0, 1.0, 0.45)).thickness(2.0).build();
    }
    let Some(grip) = h.grip else { return };
    let Some((m, _)) = project(h.mario + Vec3::Y * 1.0, size) else { return };
    let r = (size[1] * 0.024).clamp(14.0, 34.0);
    let shake = if h.shaking { 2.5 * (t * 47.0).sin() } else { 0.0 };
    let c = [m[0] + r * 3.0 + shake, m[1] - r * 0.5];
    let arc = |from: f32, to: f32, rad: f32| -> Vec<[f32; 2]> {
        let n = ((to - from).abs() / std::f32::consts::TAU * 40.0).ceil().max(2.0) as usize;
        (0..=n).map(|k| {
            let a = from + (to - from) * k as f32 / n as f32;
            [c[0] + a.cos() * rad, c[1] + a.sin() * rad]
        }).collect()
    };
    let start = -std::f32::consts::FRAC_PI_2;
    dl.add_polyline(arc(0.0, std::f32::consts::TAU, r), imgui::ImColor32::from_rgba_f32s(0.05, 0.05, 0.08, 0.55)).thickness(r * 0.32).build();
    let low = grip < 0.3;
    let flash = if low { 0.6 + 0.4 * (t * 10.0).sin().abs() } else { 1.0 };
    let col = if low { [1.0, 0.25, 0.15] } else { [1.0, 0.82, 0.3] };
    if grip > 0.005 {
        dl.add_polyline(arc(start, start + std::f32::consts::TAU * grip, r), imgui::ImColor32::from_rgba_f32s(col[0], col[1], col[2], 0.9 * flash)).thickness(r * 0.22).build();
    }
    if h.charge > 0.0 {
        let full = h.charge >= 1.0;
        let cc = if full { [1.0, 1.0, 0.85] } else { [1.0, 0.9, 0.5] };
        dl.add_circle(c, r * 0.55 * h.charge, imgui::ImColor32::from_rgba_f32s(cc[0], cc[1], cc[2], if full { 0.9 } else { 0.6 })).filled(true).build();
    }
}

/// The camera as the overlay draws through it: where, its axes, the view's slopes (x, y) and the
/// pixels a metre spans 1 m off.
struct View {
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    fwd: Vec3,
    kx: f32,
    ky: f32,
    px: f32,
    size: [f32; 2],
}

/// nearer than this to the camera a face is cut off (m), farther than this not drawn
const NEAR: f32 = 0.3;
const FIRE_FAR: f32 = 160.0;

impl View {
    fn now(size: [f32; 2]) -> Option<View> {
        let cam = unsafe { CSCamera::instance() }.ok()?;
        let pers = &cam.pers_cam_1;
        let v = |d: eldenring::position::PositionDelta| Vec3::new(d.0, d.1, d.2);
        let c = pers.position();
        let ky = (pers.fov * 0.5).tan();
        if !(ky > 0.0) || size[1] < 1.0 {
            return None;
        }
        Some(View {
            eye: Vec3::new(c.0, c.1, c.2),
            right: v(pers.right()).normalize_or_zero(),
            up: v(pers.up()).normalize_or_zero(),
            fwd: v(pers.forward()).normalize_or_zero(),
            kx: ky * size[0] / size[1],
            ky,
            px: size[1] * 0.5 / ky,
            size,
        })
    }
    /// a world point in the camera's terms (x right, y up, z ahead)
    fn cam(&self, p: Vec3) -> Vec3 {
        let d = p - self.eye;
        Vec3::new(d.dot(self.right), d.dot(self.up), d.dot(self.fwd))
    }
    fn screen(&self, c: Vec3) -> [f32; 2] {
        [self.size[0] * 0.5 * (1.0 + c.x / (c.z * self.kx)), self.size[1] * 0.5 * (1.0 - c.y / (c.z * self.ky))]
    }
}

/// What the fireballs' looks come down to on screen, drawn far to near: a face (its corners, its
/// colour, solid) or a sprite (its image, corners, tint)
enum FireItem {
    Tri([[f32; 2]; 3], imgui::ImColor32, bool),
    Img(usize, [f32; 2], [f32; 2], [f32; 4]),
}

thread_local! {
    /// (kept from frame to frame: no allocating once they've grown)
    static FIRE: std::cell::RefCell<(crate::flame::Look, Vec<(f32, FireItem)>)> = std::cell::RefCell::new((crate::flame::Look::default(), Vec::new()));
}

/// The fireballs (flame.rs's looks): 3D faces cut at the camera's near plane (a sun at arm's
/// length isn't lost, nor flattened) and SM64's sprites, all drawn far to near together; the
/// screen warming as a big one fills it, all fire inside one.
fn draw_fire(dl: &imgui::draw_list::DrawListMut<'_>, textures: &[TextureId], size: [f32; 2], fire: &crate::fx::FireFx, rubble: &[crate::fx::Rock]) {
    if fire.is_empty() && rubble.is_empty() {
        return;
    }
    let Some(view) = View::now(size) else { return };
    FIRE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (look, items) = &mut *guard;
        look.clear();
        items.clear();
        crate::flame::build(look, fire, &crate::flame::Eye { pos: view.eye, px: view.px, screen: size[1] }, crate::flame::clock());
        crate::flame::rubble(look, rubble, view.eye);
        // (the light on the ground first, under all the rest)
        let under = look.under.len();
        for (i, t) in look.under.iter().chain(look.tris.iter()).enumerate() {
            let c = [view.cam(t.p[0]), view.cam(t.p[1]), view.cam(t.p[2])];
            let depth = if i < under { f32::MAX } else { (c[0].z + c[1].z + c[2].z) / 3.0 };
            if (depth > FIRE_FAR && i >= under) || c.iter().all(|v| v.z < NEAR || v.z > FIRE_FAR) {
                continue;
            }
            let solid = t.col[3] >= 0.98;
            let col = imgui::ImColor32::from_rgba_f32s(t.col[0], t.col[1], t.col[2], t.col[3]);
            // (cut at the near plane: what's left is a triangle or a four-sided piece)
            let mut poly = [Vec3::ZERO; 4];
            let mut n = 0;
            for k in 0..3 {
                let (a, b) = (c[k], c[(k + 1) % 3]);
                if a.z >= NEAR {
                    poly[n] = a;
                    n += 1;
                }
                if (a.z >= NEAR) != (b.z >= NEAR) && n < 4 {
                    poly[n] = a + (b - a) * ((NEAR - a.z) / (b.z - a.z));
                    n += 1;
                }
            }
            for k in 1..n.saturating_sub(1) {
                items.push((depth, FireItem::Tri([view.screen(poly[0]), view.screen(poly[k]), view.screen(poly[k + 1])], col, solid)));
            }
        }
        for s in &look.sprites {
            let c = view.cam(s.pos);
            if c.z < NEAR || c.z > FIRE_FAR {
                continue;
            }
            let h = s.size * view.px / c.z;
            // (one right at the camera would cover the screen: it fades as it grows that big)
            let fade = ((size[1] * 2.2 - h) / (size[1] * 0.9)).clamp(0.0, 1.0);
            if h < 0.75 || fade <= 0.0 {
                continue;
            }
            let img = match s.frame {
                crate::flame::FRAME_GLOW => IMG_GLOW,
                n if n >= crate::flame::FRAME_ORB => IMG_ORB + (n - crate::flame::FRAME_ORB) % 4,
                n => IMG_FLAME + n % 8,
            };
            // (a halo a little behind its fire, so the flames licking round a ball's far side go
            // behind it and the near ones over it)
            let depth = c.z + if s.frame == crate::flame::FRAME_GLOW && s.tint[3] < 0.6 { s.size * 0.5 } else { 0.0 };
            let p = view.screen(c);
            let mut tint = s.tint;
            tint[3] *= fade;
            items.push((depth, FireItem::Img(img, [p[0] - h * 0.5, p[1] - h * 0.5], [p[0] + h * 0.5, p[1] + h * 0.5], tint)));
        }
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, item) in items.iter() {
            match item {
                FireItem::Tri(p, col, solid) => {
                    dl.add_triangle(p[0], p[1], p[2], *col).filled(true).build();
                    // (solid faces' seams closed)
                    if *solid {
                        dl.add_triangle(p[0], p[1], p[2], *col).thickness(1.0).build();
                    }
                }
                FireItem::Img(img, a, b, tint) => {
                    dl.add_image(textures[*img], *a, *b).col(*tint).build();
                }
            }
        }
        // the heat of a big one filling the screen: its edges glow; inside one, all fire
        let [w, h] = size;
        if look.heat > 0.01 {
            let col = |a: f32| imgui::ImColor32::from_rgba_f32s(1.0, 0.5, 0.15, a);
            dl.add_rect([0.0, 0.0], [w, h], col(0.07 * look.heat)).filled(true).build();
            let edge = h * 0.2;
            let (on, off) = (col(0.32 * look.heat), col(0.0));
            dl.add_rect_filled_multicolor([0.0, 0.0], [w, edge], on, on, off, off);
            dl.add_rect_filled_multicolor([0.0, h - edge], [w, h], off, off, on, on);
            dl.add_rect_filled_multicolor([0.0, 0.0], [edge, h], on, off, off, on);
            dl.add_rect_filled_multicolor([w - edge, 0.0], [w, h], off, on, on, off);
        }
        if look.inside > 0.01 {
            dl.add_rect([0.0, 0.0], [w, h], imgui::ImColor32::from_rgba_f32s(1.0, 0.72, 0.35, 0.75 * look.inside.min(1.0))).filled(true).build();
        }
    });
}

fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// A flat-shaded mesh in the world drawn far to near (Yoshi's tongue and eggs): triangles of
/// world points with their outward normal and colour, lit from above.
#[derive(Default)]
struct Mesh {
    tris: Vec<([Vec3; 3], Vec3, [f32; 3])>,
}

impl Mesh {
    /// An ellipsoid (an egg: `taper` narrows its top), turned by `rot`, its colour by (ring, slice).
    #[allow(clippy::too_many_arguments)]
    fn ellipsoid(&mut self, centre: Vec3, rot: glam::Quat, radii: Vec3, taper: f32, rings: usize, slices: usize, colour: &dyn Fn(usize, usize) -> [f32; 3]) {
        let point = |i: usize, j: usize| {
            let (th, ph) = (std::f32::consts::PI * i as f32 / rings as f32, std::f32::consts::TAU * j as f32 / slices as f32);
            let narrow = 1.0 - taper * th.cos().max(0.0);
            let local = Vec3::new(th.sin() * ph.cos() * narrow, th.cos(), th.sin() * ph.sin() * narrow);
            (centre + rot * (local * radii), rot * (local / radii).normalize_or(Vec3::Y))
        };
        for i in 0..rings {
            for j in 0..slices {
                let (a, b, c, d) = (point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1));
                let col = colour(i, j);
                self.tris.push(([a.0, b.0, c.0], (a.1 + b.1 + c.1).normalize_or(Vec3::Y), col));
                self.tris.push(([a.0, c.0, d.0], (a.1 + c.1 + d.1).normalize_or(Vec3::Y), col));
            }
        }
    }

    /// A tube along `path` (points), `radius` at each, round sides.
    fn tube(&mut self, path: &[Vec3], radius: &dyn Fn(usize) -> f32, sides: usize, colour: &dyn Fn(usize) -> [f32; 3]) {
        let ring = |k: usize| {
            let d = (path[(k + 1).min(path.len() - 1)] - path[k.saturating_sub(1)]).normalize_or(Vec3::Z);
            let u = d.cross(Vec3::Y).normalize_or(Vec3::X);
            let v = d.cross(u).normalize_or(Vec3::Y);
            (0..sides)
                .map(|j| {
                    let a = std::f32::consts::TAU * j as f32 / sides as f32;
                    let n = u * a.cos() + v * a.sin();
                    (path[k] + n * radius(k), n)
                })
                .collect::<Vec<_>>()
        };
        let mut prev = ring(0);
        for k in 1..path.len() {
            let next = ring(k);
            for j in 0..sides {
                let j2 = (j + 1) % sides;
                let n = (prev[j].1 + prev[j2].1 + next[j].1 + next[j2].1).normalize_or(Vec3::Y);
                self.tris.push(([prev[j].0, next[j].0, next[j2].0], n, colour(k)));
                self.tris.push(([prev[j].0, next[j2].0, prev[j2].0], n, colour(k)));
            }
            prev = next;
        }
    }

    fn draw(self, dl: &imgui::draw_list::DrawListMut<'_>, size: [f32; 2], far: f32, bodies: &[(Vec3, f32)]) {
        let eye = camera_position();
        let light = Vec3::new(0.35, 0.85, 0.4).normalize();
        let mut out = Vec::with_capacity(self.tris.len());
        for (p, n, c) in self.tris {
            let (Some(a), Some(b), Some(d)) = (project(p[0], size), project(p[1], size), project(p[2], size)) else { continue };
            let z = (a.1 + b.1 + d.1) / 3.0;
            if z > far {
                continue;
            }
            // (behind a body that's in front of it: not drawn)
            if let Some(eye) = eye {
                let c = (p[0] + p[1] + p[2]) / 3.0;
                let d = c - eye;
                let len2 = d.length_squared().max(1e-4);
                if bodies.iter().any(|&(o, r)| {
                    // (not one the camera is inside of)
                    if (o - eye).length() <= r {
                        return false;
                    }
                    // (a sphere is rounder than the body it stands for, and one by the point itself
                    // shouldn't hide it: smaller, and well in front)
                    let t = (o - eye).dot(d) / len2;
                    t > 0.0 && t < 0.85 && (eye + d * t - o).length() < r * 0.75
                }) {
                    continue;
                }
            }
            let lit = (0.55 + 0.5 * n.dot(light)).clamp(0.35, 1.1);
            out.push((z, [a.0, b.0, d.0], imgui::ImColor32::from_rgba_f32s((c[0] * lit).min(1.0), (c[1] * lit).min(1.0), (c[2] * lit).min(1.0), 1.0)));
        }
        out.sort_by(|x, y| y.0.total_cmp(&x.0));
        for (_, p, col) in out {
            dl.add_triangle(p[0], p[1], p[2], col).filled(true).build();
            dl.add_triangle(p[0], p[1], p[2], col).thickness(1.0).build();
        }
    }
}

/// The way the game's camera looks.
fn camera_forward() -> Option<Vec3> {
    let cam = unsafe { CSCamera::instance() }.ok()?;
    let f = cam.pers_cam_1.forward();
    Some(Vec3::new(f.0, f.1, f.2)).filter(|v| v.length() > 0.1)
}

/// Where the game's camera is.
fn camera_position() -> Option<Vec3> {
    let cam = unsafe { CSCamera::instance() }.ok()?;
    let c = cam.pers_cam_1.position();
    Some(Vec3::new(c.0, c.1, c.2))
}

/// World position -> screen pixel (None behind the camera), and its distance.
fn project(p: Vec3, size: [f32; 2]) -> Option<([f32; 2], f32)> {
    let cam = unsafe { CSCamera::instance() }.ok()?;
    let pers = &cam.pers_cam_1;
    let v = |d: eldenring::position::PositionDelta| Vec3::new(d.0, d.1, d.2);
    let c = pers.position();
    let (right, up, fwd) = (v(pers.right()).normalize_or_zero(), v(pers.up()).normalize_or_zero(), v(pers.forward()).normalize_or_zero());
    let d = p - Vec3::new(c.0, c.1, c.2);
    let z = d.dot(fwd);
    if z < 0.1 {
        return None;
    }
    let t = (pers.fov * 0.5).tan();
    let x = d.dot(right) / (z * t * size[0] / size[1]);
    let y = d.dot(up) / (z * t);
    Some(([size[0] * 0.5 * (1.0 + x), size[1] * 0.5 * (1.0 - y)], z))
}

impl ImguiRenderLoop for Overlay {
    fn initialize<'a>(&'a mut self, ctx: &mut imgui::Context, _render_context: &'a mut dyn RenderContext) {
        // no cursor, no settings file
        ctx.io_mut().mouse_draw_cursor = false;
        ctx.set_ini_filename(None);
    }

    fn before_render<'a>(&'a mut self, _ctx: &mut imgui::Context, render_context: &'a mut dyn RenderContext) {
        if !self.textures.is_empty() {
            return;
        }
        let images = IMAGES.lock().unwrap_or_else(|e| e.into_inner());
        let Some(images) = images.as_ref() else { return };
        let ids: Option<Vec<TextureId>> = images
            .iter()
            .map(|i| render_context.load_texture(&i.rgba, (i.w * UPSCALE) as u32, (i.h * UPSCALE) as u32).ok())
            .collect();
        match ids {
            Some(ids) => {
                log("hud: overlay textures uploaded");
                self.textures = ids;
            }
            None => log("hud: overlay texture upload failed"),
        }
    }

    fn render(&mut self, ui: &mut imgui::Ui) {
        let _span = crate::perf::span(crate::perf::OVERLAY);
        // first launch: building Mario from the ROM (needs no textures, so it shows from the start)
        if let Some(setup) = SETUP.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let size = ui.io().display_size;
            let scale = (size[1] / 1080.0).max(1.0) * 1.6;
            let w = 560.0 * scale / 1.6;
            ui.window("##er_mario_setup")
                .position([size[0] * 0.5, size[1] * 0.42], imgui::Condition::Always)
                .position_pivot([0.5, 0.5])
                .size([w, 0.0], imgui::Condition::Always)
                .bg_alpha(0.9)
                .no_decoration()
                .no_inputs()
                .build(|| {
                    ui.set_window_font_scale(scale);
                    ui.text("ER MARIO");
                    ui.separator();
                    ui.text_wrapped(&setup.title);
                    if let Some(p) = setup.progress {
                        imgui::ProgressBar::new(p.clamp(0.0, 1.0)).size([-1.0, 0.0]).build(ui);
                    }
                    if !setup.text.is_empty() {
                        ui.text_wrapped(&setup.text);
                    }
                });
        }
        if self.textures.len() <= IMG_X {
            return;
        }
        let size = ui.io().display_size;
        let textures = &self.textures;
        let dl = ui.get_foreground_draw_list();
        let image = |id: usize, x: f32, y: f32, w: f32, h: f32| {
            dl.add_image(textures[id], [x, y], [x + w, y + h]).build();
        };

        // SM64's game over: black everywhere except a Bowser-shaped hole that shrinks to nothing
        if let Some(t) = crate::gameover::iris() {
            let black = |x0: f32, y0: f32, x1: f32, y1: f32| {
                dl.add_rect([x0, y0], [x1, y1], [0.0, 0.0, 0.0, 1.0]).filled(true).build();
            };
            let s = size[0].max(size[1]) * 2.6 * (1.0 - t).powi(2);
            let (cx, cy) = (size[0] * 0.5, size[1] * 0.5);
            if s < 1.0 {
                black(0.0, 0.0, size[0], size[1]);
            } else {
                let (l, r, top, bot) = (cx - s * 0.5, cx + s * 0.5, cy - s * 0.5, cy + s * 0.5);
                black(0.0, 0.0, size[0], top.max(0.0));
                black(0.0, bot.min(size[1]), size[0], size[1]);
                black(0.0, top, l.max(0.0), bot);
                black(r.min(size[0]), top, size[0], bot);
                image(IMG_BOWSER, l, top, s, s);
            }
            return;
        }

        // outside the world (title screen, menus, loading): which mod is loaded, bottom left, in
        // SM64's HUD font (it has no dot: a small square at the baseline); only once Mario's files are
        // built and loaded this session (not during the first-launch setup)
        let g = 10.0 * size[1] / 240.0 * SIZE;
        let text = |text: &str, y: f32, g: f32| {
            let mut x = size[1] * 0.04;
            for c in text.chars() {
                match c {
                    ' ' => x += g * 0.5,
                    '.' => {
                        let d = g * 0.18;
                        dl.add_rect([x, y + g - d * 1.6], [x + d, y + g - d * 0.6], [1.0, 1.0, 1.0, 1.0]).filled(true).build();
                        x += g * 0.35;
                    }
                    c => {
                        if let Some(id) = glyph_for(c) {
                            image(id, x, y, g, g);
                        }
                        x += g * 0.8;
                    }
                }
            }
        };
        if !crate::in_world() && crate::assets::ready() {
            let y = size[1] * 0.96 - g;
            text(concat!("ER MARIO ", env!("CARGO_PKG_VERSION")), y, g);
            // smaller, right above the version
            if let Some(v) = crate::update::newer() {
                text(&format!("UPDATE AVAILABLE {v}"), y - g * 1.1, g * 0.75);
            }
        }

        // nothing outside Mario mode, in menus, or when the Mario frame stopped (loading)
        let wedges = WEDGES.load(Ordering::Relaxed);
        let alive = LAST_SET.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < 0.5);
        if wedges == 0xFF || !alive {
            return;
        }
        // in game: a small note in the bottom left corner once a newer version is out
        if let Some(v) = crate::update::newer() {
            text(&format!("ER MARIO {v} IS OUT"), size[1] * 0.985 - g * 0.5, g * 0.5);
        }

        // the coins in the world (up to 40 m away): SM64's 64 units = 0.64 m
        for (pos, age) in crate::coins::visible() {
            let (Some((foot, z)), Some((head, _))) = (project(pos, size), project(pos + Vec3::Y * 0.64, size)) else { continue };
            let h = foot[1] - head[1];
            if z > 40.0 || h < 1.0 {
                continue;
            }
            image(IMG_COIN_FRAMES + (age * 15.0) as usize % 4, foot[0] - h * 0.5, head[1], h, h);
        }

        // the bag of coins lost in the last death: a big spinning coin in a golden glow
        if let Some((pos, _)) = crate::coins::bag() {
            let lift = pos + Vec3::Y * 0.9;
            if let (Some((c, z)), Some((top, _))) = (project(lift, size), project(lift + Vec3::Y * 0.5, size)) {
                let h = (c[1] - top[1]) * 2.0;
                if z < 80.0 && h > 1.0 {
                    let clock = Instant::now().duration_since(*BAG_START).as_secs_f32();
                    let pulse = 0.8 + 0.2 * (clock * 3.0).sin();
                    dl.add_image(textures[IMG_GLOW], [c[0] - h * 1.4, c[1] - h * 1.4], [c[0] + h * 1.4, c[1] + h * 1.4]).col([1.0, 0.8, 0.2, 0.55 * pulse]).build();
                    image(IMG_COIN_FRAMES + (clock * 12.0) as usize % 4, c[0] - h * 0.5, c[1] - h * 0.5, h, h);
                }
            }
        }

        // Mario's effects in the world (fx.rs), and each co-op partner's around where they're shown
        {
            let fx = crate::fx::WorldFx::local();
            draw_fx(&dl, textures, size, &fx, &crate::ride::sprites().bodies);
            for partner in crate::coop::partner_fx() {
                draw_fx(&dl, textures, size, &partner, &[]);
            }
            draw_colossus(&dl, textures, size);
        }

        // Mario got hurt: the screen's edges flash red, fading
        let flash = crate::impact::hurt_flash();
        if flash > 0.0 {
            let edge = size[1] * 0.18;
            let red = |a: f32| imgui::ImColor32::from_rgba_f32s(0.85, 0.05, 0.02, a);
            let (on, off) = (red(0.55 * flash), red(0.0));
            let [w, h] = size;
            dl.add_rect_filled_multicolor([0.0, 0.0], [w, edge], on, on, off, off);
            dl.add_rect_filled_multicolor([0.0, h - edge], [w, h], off, off, on, on);
            dl.add_rect_filled_multicolor([0.0, 0.0], [edge, h], on, off, off, on);
            dl.add_rect_filled_multicolor([w - edge, 0.0], [w, h], off, on, on, off);
        }

        // a star finish: the screen flashes gold, brightest at the edges
        let (tint, gold) = crate::impact::screen_flash();
        if gold > 0.0 {
            let [w, h] = size;
            let col = |a: f32| imgui::ImColor32::from_rgba_f32s(tint[0], tint[1], tint[2], a);
            dl.add_rect([0.0, 0.0], [w, h], col(gold * 0.35)).filled(true).build();
            let edge = h * 0.22;
            let (on, off) = (col(gold * 0.8), col(0.0));
            dl.add_rect_filled_multicolor([0.0, 0.0], [w, edge], on, on, off, off);
            dl.add_rect_filled_multicolor([0.0, h - edge], [w, h], off, off, on, on);
            dl.add_rect_filled_multicolor([0.0, 0.0], [edge, h], on, off, off, on);
            dl.add_rect_filled_multicolor([w - edge, 0.0], [w, h], off, on, on, off);
        }

        // one of SM64's pixels (its screen is 240 lines high), at HUD size
        let px = size[1] / 240.0 * SIZE;

        // the caps (top left, in D-pad order: left Metal, up Wing, then right the Mega Mushroom):
        // bright when ready, pulsing while worn, dim and filling up while recharging (the mushroom
        // draining while he's big)
        let mut caps = crate::caps::hud();
        let (mega_ready, mega_on) = crate::mega::hud();
        let (rock_ready, rock_left) = crate::rock::hud();
        caps[2] = mega_ready;
        caps[0] = rock_ready;
        {
            let worn = if mega_on { Some(2) } else if rock_left.is_some() { Some(0) } else { crate::caps::active() };
            let s = 20.0 * px;
            // (under the lives, coins and stars counters, which take the top 31 SM64 pixels)
            let (x0, y0) = (22.0 * px, 38.0 * px);
            static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            let t = T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f32();
            for (i, &ready) in caps.iter().enumerate() {
                let x = x0 + i as f32 * s * 1.25;
                let on = worn == Some(i as u8);
                let grow = if on { s * 0.12 * (1.0 + (t * 8.0).sin()) } else { 0.0 };
                let alpha = if on || ready >= 1.0 { 1.0 } else { 0.35 };
                let img = match i {
                    2 => IMG_MUSHROOM,
                    0 => IMG_ROCK,
                    _ => IMG_CAP_BOX + i,
                };
                dl.add_image(textures[img], [x - grow, y0 - grow], [x + s + grow, y0 + s + grow])
                    .col([1.0, 1.0, 1.0, alpha])
                    .build();
                if let (0, Some(left)) = (i, rock_left) {
                    let bar = [x, y0 + s + px * 2.0];
                    dl.add_rect(bar, [x + s, bar[1] + px * 2.0], [0.0, 0.0, 0.0, 0.6]).filled(true).build();
                    dl.add_rect(bar, [x + s * left, bar[1] + px * 2.0], [0.75, 0.7, 0.62, 0.95]).filled(true).build();
                }
                if let (2, Some(left)) = (i, crate::mega::left()) {
                    let bar = [x, y0 + s + px * 2.0];
                    dl.add_rect(bar, [x + s, bar[1] + px * 2.0], [0.0, 0.0, 0.0, 0.6]).filled(true).build();
                    dl.add_rect(bar, [x + s * left, bar[1] + px * 2.0], [1.0, 0.3, 0.25, 0.9]).filled(true).build();
                }
                if !on && ready < 1.0 {
                    let bar = [x, y0 + s + px * 2.0];
                    dl.add_rect(bar, [x + s, bar[1] + px * 2.0], [0.0, 0.0, 0.0, 0.6]).filled(true).build();
                    dl.add_rect(bar, [x + s * ready.clamp(0.0, 1.0), bar[1] + px * 2.0], [1.0, 0.85, 0.2, 0.9]).filled(true).build();
                }
            }
            // on Yoshi: his eggs in stock, a row under the caps
            let ride = crate::ride::sprites();
            if ride.riding {
                for k in 0..ride.stock {
                    let x = x0 + k as f32 * s * 0.55;
                    let y = y0 + s * 1.45;
                    dl.add_image(textures[IMG_EGG], [x, y], [x + s * 0.5, y + s * 0.66]).build();
                }
            }
            // the weapon in hand (L1), after a gap (nothing with the fists); the boomerang with a
            // pip for each one ready to throw
            let weapon = match crate::weapons::current() {
                crate::weapons::HAMMER => Some(IMG_HAMMER_ICON),
                crate::weapons::RANG => Some(IMG_RANG),
                crate::weapons::CHOMP => Some(IMG_CHOMP),
                _ => None,
            };
            // the MAX finisher ready: MAX in gold under the weapon's spot, pulsing
            // (its last 3 s blinking: running out)
            if let Some(age) = crate::style::max_ready() {
                let left = crate::style::MAX_KEEP - age;
                let blink = if left < 3.0 { 0.45 + 0.55 * (t * 14.0).sin().abs() } else { 1.0 };
                let x = x0 + caps.len() as f32 * s * 1.25 + s * 0.35;
                let pulse = 0.75 + 0.25 * (t * 6.0).sin();
                let g = 8.0 * px * (1.0 + 0.08 * pulse);
                for (k, ch) in "MAX".chars().enumerate() {
                    if let Some(id) = glyph_for(ch) {
                        let gx = x + k as f32 * g * 0.8;
                        dl.add_image(textures[id], [gx, y0 + s + px * 6.0], [gx + g, y0 + s + px * 6.0 + g]).col([1.0, 0.75 * pulse + 0.15, 0.25, blink]).build();
                    }
                }
            }
            // the element in hand (element.rs): its badge at the weapon's corner (below), the fists'
            // alone in the weapon's spot
            if weapon.is_none() {
                let x = x0 + caps.len() as f32 * s * 1.25 + s * 0.35;
                element_badge(&dl, textures, [x + s * 0.5, y0 + s * 0.5], s * 0.36, crate::element::current(), 1.0, t);
            }
            if let Some(img) = weapon {
                let x = x0 + caps.len() as f32 * s * 1.25 + s * 0.35;
                dl.add_rect([x - px * 2.0, y0 - px * 2.0], [x + s + px * 2.0, y0 + s + px * 2.0], [0.0, 0.0, 0.0, 0.35]).filled(true).rounding(px * 3.0).build();
                dl.add_image(textures[img], [x, y0], [x + s, y0 + s]).build();
                let el = crate::element::current();
                element_badge(&dl, textures, [x + s * 0.95, y0 + s * 0.88], s * 0.3, el, 1.0, t);
                if crate::weapons::current() == crate::weapons::RANG {
                    for k in 0..crate::boomerang::ready() {
                        let c = [x + s * (0.2 + 0.3 * k as f32), y0 + s + px * 3.0];
                        dl.add_circle(c, px * 1.3, [1.0, 0.85, 0.2, 0.95]).filled(true).build();
                    }
                }
            }
        }

        // boss bars at the bottom (Elden Ring's layout), the name in SM64's font
        for (i, boss) in BOSSES.lock().unwrap_or_else(|e| e.into_inner()).iter().enumerate() {
            let w = (size[1] * 16.0 / 9.0 * 0.55).min(size[0] * 0.9);
            let h = (size[1] * 0.008).max(4.0);
            let x0 = (size[0] - w) * 0.5;
            let y0 = size[1] * 0.86 - i as f32 * size[1] * 0.07;
            let g = 11.0 * px;
            let mut x = x0;
            for c in boss.name.chars() {
                if let Some(id) = glyph_for(c) {
                    image(id, x, y0 - g - 6.0, g, g);
                }
                x += g * 0.8;
            }
            let rect = |a: f32, b: f32, col: [f32; 4]| {
                dl.add_rect([x0 + w * a, y0], [x0 + w * b, y0 + h], col).filled(true).build();
            };
            dl.add_rect([x0 - 2.0, y0 - 2.0], [x0 + w + 2.0, y0 + h + 2.0], [0.0, 0.0, 0.0, 0.7]).filled(true).build();
            rect(boss.hp, boss.before.max(boss.hp), [0.85, 0.7, 0.2, 1.0]);
            rect(0.0, boss.hp, [0.6, 0.05, 0.05, 1.0]);
            if let Some(bar) = &boss.status {
                status_bar(&dl, x0, y0 + h + 4.0, w * 0.3, (h * 0.5).max(2.0), bar, crate::flame::clock());
            }
            if boss.dmg > 0 {
                let digits = boss.dmg.to_string();
                let mut x = x0 + w - digits.len() as f32 * g * 0.8;
                for ch in digits.bytes() {
                    image(IMG_DIGITS + (ch - b'0') as usize, x, y0 - g - 6.0, g, g);
                    x += g * 0.8;
                }
            }
        }

        // enemy health bars (Elden Ring's style: dark frame, red HP, yellow for the combo's damage)
        // over the heads of what Mario hit, with the combo's damage in SM64's digits
        for tag in TAGS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            let Some((top, z)) = project(tag.pos + Vec3::Y * 2.1, size) else { continue };
            if z > 40.0 {
                continue;
            }
            let (w, h) = (size[1] * 0.075, (size[1] * 0.006).max(3.0));
            let (x0, y0) = (top[0] - w * 0.5, top[1]);
            let rect = |a: f32, b: f32, col: [f32; 4]| {
                dl.add_rect([x0 + w * a, y0], [x0 + w * b, y0 + h], col).filled(true).build();
            };
            dl.add_rect([x0 - 2.0, y0 - 2.0], [x0 + w + 2.0, y0 + h + 2.0], [0.0, 0.0, 0.0, 0.7]).filled(true).build();
            rect(tag.hp, tag.before.max(tag.hp), [0.85, 0.7, 0.2, 1.0]);
            rect(0.0, tag.hp, [0.6, 0.05, 0.05, 1.0]);
            if let Some(bar) = &tag.status {
                status_bar(&dl, x0, y0 + h + 3.0, w, (h * 0.55).max(2.0), bar, crate::flame::clock());
            }
            if tag.dmg > 0 {
                let g = 10.0 * px;
                for (k, ch) in tag.dmg.to_string().bytes().enumerate() {
                    image(IMG_DIGITS + (ch - b'0') as usize, x0 + w + 6.0 + k as f32 * g * 0.75, y0 + h * 0.5 - g * 0.5, g, g);
                }
            }
        }
        // the power meter's height (None: out of the screen). Damage brings it down to HEIGHT,
        // a moment later it slides up to HEIGHT_TOP; healing just fills it where it is; back to
        // full health it stays LINGER s, then slides out of the screen.
        let now = Instant::now();
        let ease = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let hurt_height = |hit: Instant| {
            HEIGHT + (HEIGHT_TOP - HEIGHT) * ease((now.duration_since(hit).as_secs_f32() - METER_RISE_AFTER) / METER_RISE_TIME)
        };
        let long_ago = now.checked_sub(std::time::Duration::from_secs(60)).unwrap_or(now);
        let m = self.meter.get_or_insert(Meter { wedges, hit: long_ago, full: (wedges >= 8).then_some((long_ago, HEIGHT_GONE)) });
        if wedges < m.wedges {
            m.hit = now;
        }
        if wedges >= 8 && m.wedges < 8 {
            m.full = Some((now, hurt_height(m.hit)));
        } else if wedges < 8 {
            m.full = None;
        }
        m.wedges = wedges;
        let meter_height = match m.full {
            None => Some(hurt_height(m.hit)),
            Some((t, h0)) => {
                let out = (now.duration_since(t).as_secs_f32() - LINGER) / METER_RISE_TIME;
                (out < 1.0).then(|| h0 + (HEIGHT_GONE - h0) * ease(out))
            }
        };

        // the counters (SM64's layout: icon, x, digits 12 px apart, 15 px from the top), in the
        // top of the screen, always
        {
            let top = 15.0 * px;
            let counter = |icon: usize, x: f32, n: u32| {
                image(icon, x, top, 16.0 * px, 16.0 * px);
                image(IMG_MULTIPLY, x + 16.0 * px, top, 16.0 * px, 16.0 * px);
                for (k, ch) in n.to_string().bytes().enumerate() {
                    image(IMG_DIGITS + (ch - b'0') as usize, x + (32.0 + 12.0 * k as f32) * px, top, 16.0 * px, 16.0 * px);
                }
            };
            counter(IMG_HEAD, 22.0 * px, DEATHS.load(Ordering::Relaxed));
            // the coins right of the screen's centre, where SM64's widescreen ports put them
            counter(IMG_COIN, size[0] * COINS_X, COINS.load(Ordering::Relaxed));
            counter(IMG_STAR, size[0] - 78.0 * px, STARS.load(Ordering::Relaxed));
        }

        // the style meter (style.rs), right of the screen: the rank big, in its colour (SSS
        // flashing), slamming in when it goes up, its word sliding in under it, the count and HITS,
        // and a bar for the time the string has left; a star finish's banner in the middle of the
        // screen; a lost string's "MAMMA MIA" falling off where the meter was
        {
            static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
            let clock = now.duration_since(*START).as_secs_f32();
            let glyph = |id: usize, cx: f32, cy: f32, g: f32, tint: [f32; 4]| {
                dl.add_image(textures[id], [cx - g * 0.5, cy - g * 0.5], [cx + g * 0.5, cy + g * 0.5]).col(tint).build();
            };
            // right-aligned at `right`, letters `gap` of their size apart, each raised by `wave(k)`
            let line_wave = |text: &str, right: f32, cy: f32, g: f32, gap: f32, tint: [f32; 4], wave: &dyn Fn(usize) -> f32| {
                let n = text.chars().count() as f32;
                for (k, c) in text.chars().enumerate() {
                    if let Some(id) = glyph_for(c) {
                        glyph(id, right - (n - k as f32 - 0.5) * g * gap, cy - wave(k), g, tint);
                    }
                }
            };
            let line = |text: &str, right: f32, cy: f32, g: f32, gap: f32, tint: [f32; 4]| line_wave(text, right, cy, g, gap, tint, &|_| 0.0);
            // the compass (Elden Ring's own is hidden with the rest of its HUD): a strip at the top with
            // the way the camera looks (north is +Z, `compass_north` in the ini turns it), and the bag
            // of lost coins on it
            if let (Some(fwd), Some(eye)) = (camera_forward(), camera_position()) {
                let turn: f32 = crate::paths::config("compass_north").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let heading = fwd.x.atan2(fwd.z).to_degrees() + turn;
                // (under the power meter while it shows, following it as it slides)
                let y0 = meter_height.map_or(36.0 * px, |h| (size[1] * (1.0 - h / 240.0) + 34.0 * px).max(36.0 * px));
                let (span, w, cx) = (110.0f32, size[0] * 0.28, size[0] * 0.5);
                let rel = |deg: f32| ((deg - heading + 540.0).rem_euclid(360.0)) - 180.0;
                let fade = |d: f32| (1.0 - (d.abs() / (span * 0.5)).powi(2)).clamp(0.0, 1.0);
                dl.add_line([cx - w * 0.5, y0 + 13.0 * px], [cx + w * 0.5, y0 + 13.0 * px], [1.0, 1.0, 1.0, 0.22]).thickness(px.max(1.0)).build();
                const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
                for step in 0..24 {
                    let deg = step as f32 * 15.0;
                    let d = rel(deg);
                    if d.abs() > span * 0.5 {
                        continue;
                    }
                    let x = cx + d / span * w;
                    let a = fade(d);
                    let major = step % 3 == 0;
                    let h = if major { 8.0 } else { 4.0 } * px;
                    dl.add_line([x, y0 + 13.0 * px - h], [x, y0 + 13.0 * px], [1.0, 1.0, 1.0, 0.7 * a]).thickness(px.max(1.0)).build();
                    if major {
                        let name = NAMES[step / 3];
                        let g = if name.len() == 1 { 12.0 } else { 9.0 } * px;
                        let n = name.chars().count() as f32;
                        line(name, x + n * g * 0.8 * 0.5, y0 + 2.0 * px, g, 0.8, if step == 0 { [1.0, 0.4, 0.35, a] } else { [1.0, 1.0, 1.0, a] });
                    }
                }
                // (where he's looking: a little marker under the strip)
                dl.add_triangle([cx - 4.0 * px, y0 + 20.0 * px], [cx + 4.0 * px, y0 + 20.0 * px], [cx, y0 + 15.0 * px], [1.0, 0.85, 0.3, 0.9]).filled(true).build();
                // the bag of coins lost in the last death
                if let Some((bag, _)) = crate::coins::bag() {
                    let d = rel((bag.x - eye.x).atan2(bag.z - eye.z).to_degrees() + turn);
                    if d.abs() <= span * 0.5 {
                        let x = cx + d / span * w;
                        let pulse = 0.75 + 0.25 * (clock * 4.0).sin();
                        dl.add_circle([x, y0 + 13.0 * px], 5.0 * px, [1.0, 0.85, 0.25, pulse]).filled(true).build();
                        dl.add_circle([x, y0 + 13.0 * px], 5.0 * px, [0.4, 0.25, 0.0, 0.9]).thickness(px.max(1.0)).build();
                        let metres = ((bag.x - eye.x).powi(2) + (bag.z - eye.z).powi(2)).sqrt();
                        let text = format!("{}M", metres as u32);
                        let g = 8.0 * px;
                        line(&text, x + text.chars().count() as f32 * g * 0.8 * 0.5, y0 + 27.0 * px, g, 0.8, [1.0, 0.9, 0.4, 0.9]);
                    }
                }
                // co-op: the partners on it, faint (their head, the distance once they're far off;
                // bright in danger), and their coin bags (a coin ringed in their colour)
                let partners = *PARTNERS.lock().unwrap_or_else(|e| e.into_inner());
                for p in partners.iter().flatten() {
                    let ring = if p.luigi { [0.35, 0.9, 0.4, 0.9] } else { [1.0, 0.35, 0.3, 0.9] };
                    if let Some((bag, _)) = p.bag {
                        let d = rel((bag.x - eye.x).atan2(bag.z - eye.z).to_degrees() + turn);
                        if d.abs() <= span * 0.5 {
                            let x = cx + d / span * w;
                            dl.add_circle([x, y0 + 13.0 * px], 3.5 * px, [1.0, 0.85, 0.25, 0.8]).filled(true).build();
                            dl.add_circle([x, y0 + 13.0 * px], 3.5 * px, ring).thickness((1.5 * px).max(1.0)).build();
                        }
                    }
                    let Some(at) = p.head else { continue };
                    let d = rel((at.x - eye.x).atan2(at.z - eye.z).to_degrees() + turn);
                    if d.abs() > span * 0.5 {
                        continue;
                    }
                    let x = cx + d / span * w;
                    let alpha = if p.danger { 0.75 + 0.25 * (clock * 8.0).sin() } else { 0.55 * fade(d) + 0.15 };
                    let k = 10.0 * px;
                    let face = if p.luigi { IMG_LUIGI_HEAD } else { IMG_HEAD };
                    let grey = if p.down { 0.45 } else { 1.0 };
                    dl.add_image(textures[face], [x - k * 0.5, y0 + 13.0 * px - k * 0.5], [x + k * 0.5, y0 + 13.0 * px + k * 0.5]).col([grey, grey, grey, alpha]).build();
                    let metres = ((at.x - eye.x).powi(2) + (at.z - eye.z).powi(2)).sqrt();
                    if metres > 30.0 {
                        let mut buf = [0u8; 6];
                        let text = metres_of(metres.min(999.0) as u16, &mut buf);
                        let g = 7.0 * px;
                        line(text, x + text.len() as f32 * g * 0.8 * 0.5, y0 + 26.0 * px, g, 0.8, [ring[0], ring[1], ring[2], alpha]);
                    }
                }
            }
            let right = size[0] - 24.0 * px;
            let top = size[1] * 0.30;
            if let Some(st) = crate::style::show() {
                if let Some(r) = st.rank {
                    let mut t = TINTS[r];
                    if r == crate::style::RANKS.len() - 1 {
                        let f = (clock * 9.0).sin() * 0.5 + 0.5;
                        t = t.map(|c| c + (1.0 - c) * f * 0.7);
                    }
                    // the slam: in from twice its size over a fifth of a second, a shake as it lands
                    let a = st.rank_age;
                    let slam = (1.0 - a / 0.2).max(0.0);
                    let shake = if a < 0.45 { (a * 70.0).sin() * 3.0 * px * (1.0 - a / 0.45) } else { 0.0 };
                    let g = 30.0 * px * (1.0 + 1.2 * slam * slam + 0.15 * st.pop);
                    let alpha = st.alpha * (1.0 - slam * 0.6);
                    // (a glow behind it as it lands)
                    if a < 0.5 {
                        let w = 34.0 * px * (1.0 + a * 2.0);
                        let glow = (1.0 - a / 0.5) * 0.8 * st.alpha;
                        dl.add_image(textures[IMG_GLOW], [right - w * 1.3, top - w * 0.6], [right + w * 0.3, top + w * 0.6]).col([t[0], t[1], t[2], glow]).build();
                    }
                    line(crate::style::RANKS[r].1, right + shake, top, g, 0.78, [t[0], t[1], t[2], alpha]);
                    // its word: in from the right, the top rank's letters riding a wave
                    let p = (a / 0.22).min(1.0);
                    let slide = (1.0 - p) * (1.0 - p) * 70.0 * px;
                    let word = crate::style::RANKS[r].2;
                    let wave = |k: usize| if r >= 5 { (clock * 8.0 - k as f32 * 0.7).sin() * 2.0 * px } else { 0.0 };
                    line_wave(word, right + slide, top + 24.0 * px, 11.0 * px, 0.78, [t[0], t[1], t[2], st.alpha * p], &wave);
                }
                let g = 14.0 * px * (1.0 + 0.4 * st.pop);
                let white = [1.0, 1.0, 1.0, st.alpha];
                let y = top + 42.0 * px;
                line("HITS", right, y, 9.0 * px, 0.8, white);
                line(&st.count.to_string(), right - 4.0 * 9.0 * px * 0.8 - 4.0 * px, y - 1.0 * px, g, 0.75, white);
                // the time left: a bar under it, in the rank's colour
                let t = st.rank.map_or([0.85, 0.85, 0.9], |r| TINTS[r]);
                let (w, h) = (70.0 * px, 2.5 * px);
                let (x0, y0) = (right - w, y + 9.0 * px);
                dl.add_rect([x0, y0], [right, y0 + h], imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.45 * st.alpha)).filled(true).build();
                dl.add_rect([right - w * st.left, y0], [right, y0 + h], imgui::ImColor32::from_rgba_f32s(t[0], t[1], t[2], 0.9 * st.alpha)).filled(true).build();
                // the star finish: its banner over the middle of the screen
                if let Some(a) = st.star_age {
                    let pop = (1.0 - a / 0.15).max(0.0);
                    let fade = 1.0 - ((a - crate::style::STAR_TIME * 0.6) / (crate::style::STAR_TIME * 0.4)).clamp(0.0, 1.0);
                    let g = 20.0 * px * (1.0 + 0.8 * pop * pop);
                    let text = "STAR FINISH";
                    let half = text.chars().count() as f32 * g * 0.78 * 0.5;
                    let shine = (clock * 10.0).sin() * 0.5 + 0.5;
                    let gold = [1.0, 0.82 + 0.15 * shine, 0.3 + 0.4 * shine, fade];
                    let wave = |k: usize| (clock * 9.0 - k as f32 * 0.6).sin() * 2.5 * px;
                    line_wave(text, size[0] * 0.5 + half, size[1] * 0.27, g, 0.78, gold, &wave);
                    image(IMG_STAR, size[0] * 0.5 - half - g * 1.1, size[1] * 0.27 - g * 0.5, g, g);
                    image(IMG_STAR, size[0] * 0.5 + half + g * 0.1, size[1] * 0.27 - g * 0.5, g, g);
                }
            } else if let Some((a, _)) = crate::style::lost() {
                // shaken loose, then falling off and fading
                let p = a / crate::style::LOST_TIME;
                let shake = (a * 60.0).sin() * 3.0 * px * (1.0 - p);
                let fall = (p * p) * 40.0 * px;
                line("MAMMA MIA", right + shake, top + 10.0 * px + fall, 14.0 * px, 0.78, [1.0, 0.3, 0.25, 1.0 - p * p]);
            }
        }

        // callouts (style.rs): words popping out of the blows ("NICE!", "MIKIRI!", the skillshots)
        // over the enemy they landed on: in with an overshoot, rising, the letters riding a little
        // wave, a drop shadow under them, fading out at the end
        for c in crate::style::callouts() {
            let p = c.age / crate::style::CALLOUT_TIME;
            let anchor = match c.at {
                Some(at) => project(at + Vec3::Y * 0.9, size).filter(|(_, z)| *z < 45.0).map(|(s, _)| s),
                None => Some([size[0] * 0.5, size[1] * 0.33]),
            };
            let Some([cx, cy]) = anchor else { continue };
            let pop = if c.age < 0.08 {
                0.4 + 0.95 * (c.age / 0.08)
            } else if c.age < 0.2 {
                1.35 - 0.35 * ((c.age - 0.08) / 0.12)
            } else {
                1.0
            };
            let g = if c.big { 15.0 } else { 11.0 } * px * pop;
            let rise = (1.0 - (1.0 - p.min(1.0)) * (1.0 - p.min(1.0))) * 22.0 * px;
            let alpha = if p > 0.65 { 1.0 - (p - 0.65) / 0.35 } else { 1.0 };
            let gap = 0.78;
            let n = c.text.chars().count() as f32;
            let left = cx - n * g * gap * 0.5;
            let [r, gr, b] = c.tint;
            for (k, ch) in c.text.chars().enumerate() {
                let x = left + (k as f32 + 0.5) * g * gap;
                let y = cy - rise - (c.age * 10.0 - k as f32 * 0.8).sin() * 1.5 * px;
                let d = g * 0.07;
                if ch == '!' {
                    for (off, col) in [(d, [0.0, 0.0, 0.0, 0.6 * alpha]), (0.0, [r, gr, b, alpha])] {
                        dl.add_rect([x - g * 0.1 + off, y - g * 0.42 + off], [x + g * 0.1 + off, y + g * 0.14 + off], col).filled(true).rounding(g * 0.08).build();
                        dl.add_circle([x + off, y + g * 0.32 + off], g * 0.1, col).filled(true).build();
                    }
                    continue;
                }
                let Some(id) = glyph_for(ch) else { continue };
                dl.add_image(textures[id], [x - g * 0.5 + d, y - g * 0.5 + d], [x + g * 0.5 + d, y + g * 0.5 + d]).col([0.0, 0.0, 0.0, 0.6 * alpha]).build();
                dl.add_image(textures[id], [x - g * 0.5, y - g * 0.5], [x + g * 0.5, y + g * 0.5]).col([r, gr, b, alpha]).build();
            }
        }

        // the weapon just taken (L1, weapons.rs): its icon and name, low in the middle of the screen,
        // popping in and fading
        if let Some((w, age)) = crate::weapons::banner() {
            let pop = (1.0 - age / 0.15).max(0.0);
            let fade = 1.0 - ((age - crate::weapons::BANNER_TIME * 0.7) / (crate::weapons::BANNER_TIME * 0.3)).clamp(0.0, 1.0);
            let name = crate::weapons::NAMES[w as usize];
            let el = crate::element::Element::of_weapon(w);
            let g = 12.0 * px * (1.0 + 0.5 * pop);
            let n = name.chars().count() as f32;
            // (then its element: its badge and its name in its colour)
            let ge = g * 0.8;
            let el_w = g * 1.3 + el.name().chars().count() as f32 * ge * 0.78;
            let width = n * g * 0.78 + g * 1.6 + el_w;
            let (cx, cy) = (size[0] * 0.5, size[1] * 0.78);
            let icon = match w {
                crate::weapons::HAMMER => IMG_HAMMER_ICON,
                crate::weapons::RANG => IMG_RANG,
                crate::weapons::CHOMP => IMG_CHOMP,
                _ => IMG_HEAD,
            };
            let x0 = cx - width * 0.5;
            dl.add_rect([x0 - g * 0.4, cy - g * 0.9], [x0 + width + g * 0.4, cy + g * 0.9], imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.45 * fade)).filled(true).rounding(g * 0.4).build();
            dl.add_image(textures[icon], [x0, cy - g * 0.7], [x0 + g * 1.4, cy + g * 0.7]).col([1.0, 1.0, 1.0, fade]).build();
            for (k, c) in name.chars().enumerate() {
                if let Some(id) = glyph_for(c) {
                    let x = x0 + g * 1.6 + (k as f32 + 0.5) * g * 0.78;
                    dl.add_image(textures[id], [x - g * 0.5, cy - g * 0.5], [x + g * 0.5, cy + g * 0.5]).col([1.0, 0.95, 0.7, fade]).build();
                }
            }
            let ex = x0 + g * 1.6 + n * g * 0.78 + g * 0.3;
            element_badge(&dl, textures, [ex + g * 0.45, cy], g * 0.45, el, fade, crate::flame::clock());
            let tint = el.tint();
            for (k, c) in el.name().chars().enumerate() {
                if let Some(id) = glyph_for(c) {
                    let x = ex + g * 1.1 + (k as f32 + 0.5) * ge * 0.78;
                    dl.add_image(textures[id], [x - ge * 0.5, cy - ge * 0.5], [x + ge * 0.5, cy + ge * 0.5]).col([tint[0], tint[1], tint[2], fade]).build();
                }
            }
        }

        // the camera status, bottom right like SM64's (x 54 from the right, line 205): camera,
        // Lakitu, and the zoom arrow (down: zoomed out, up: the C-up view)
        if let Some(state) = crate::lakitu::hud_state() {
            let (x, y) = (size[0] - 54.0 * px, size[1] - (240.0 - 205.0) * px);
            image(IMG_CAMERA, x, y, 16.0 * px, 16.0 * px);
            image(IMG_LAKITU, x + 16.0 * px, y, 16.0 * px, 16.0 * px);
            match state {
                crate::lakitu::HudState::Far => image(IMG_ARROW_DOWN, x + 4.0 * px, y + 16.0 * px, 8.0 * px, 8.0 * px),
                crate::lakitu::HudState::FirstPerson => image(IMG_ARROW_UP, x + 4.0 * px, y - 8.0 * px, 8.0 * px, 8.0 * px),
                crate::lakitu::HudState::Near => {}
            }
        }

        // the trap kit (LT held while crouching): its four things in a diamond like the pad's buttons
        // (X left, Y up, B right, RB below), popping in, bright when ready, a filling ring while not
        if let Some((ready, since)) = *KIT.lock().unwrap_or_else(|e| e.into_inner()) {
            let t = since.elapsed().as_secs_f32();
            let pop = (t / 0.15).min(1.0);
            let pop = 1.0 - (1.0 - pop) * (1.0 - pop);
            let (cx, cy) = (size[0] * 0.5, size[1] * 0.72);
            let s = 22.0 * px * (0.6 + 0.4 * pop);
            let gap = 26.0 * px * pop;
            let spots = [(-gap, 0.0), (0.0, -gap), (gap, 0.0), (0.0, gap)];
            let names = ["X", "Y", "B", "RB"];
            // (the stopwatch in its middle: LT + L1)
            {
                let r = crate::stopwatch::ready();
                let on = r >= 1.0;
                let rr = s * 0.42;
                let col = |a: f32| imgui::ImColor32::from_rgba_f32s(0.75, 0.88, 1.0, a * pop);
                dl.add_circle([cx, cy], rr, imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.45 * pop)).filled(true).build();
                dl.add_circle([cx, cy], rr, col(if on { 0.95 } else { 0.35 })).thickness(px * 1.6).build();
                let hand = -std::f32::consts::FRAC_PI_2 + r.clamp(0.0, 1.0) * std::f32::consts::TAU;
                dl.add_line([cx, cy], [cx + hand.cos() * rr * 0.75, cy + hand.sin() * rr * 0.75], col(1.0)).thickness(px * 1.4).build();
                dl.add_line([cx, cy], [cx, cy - rr * 0.5], col(1.0)).thickness(px * 1.4).build();
                // (its button in the free corner up and right of it, clear of Y and B)
                let g = 5.0 * px;
                for (i, ch) in "LB".chars().enumerate() {
                    if let Some(id) = glyph_for(ch) {
                        let lx = cx + rr * 0.9 + i as f32 * g * 0.8;
                        let ly = cy - rr * 1.4 - g;
                        dl.add_image(textures[id], [lx, ly], [lx + g, ly + g]).col([1.0, 1.0, 1.0, 0.8 * pop]).build();
                    }
                }
            }
            for k in 0..4 {
                let (x, y) = (cx + spots[k].0, cy + spots[k].1);
                let on = ready[k] >= 1.0;
                dl.add_circle([x, y], s * 0.72, imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.45 * pop)).filled(true).build();
                if !on {
                    // (the wait: an arc filling round it)
                    let n = (24.0 * ready[k].clamp(0.0, 1.0)) as usize;
                    let pts: Vec<[f32; 2]> = (0..=n).map(|i| {
                        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 24.0 * std::f32::consts::TAU;
                        [x + a.cos() * s * 0.72, y + a.sin() * s * 0.72]
                    }).collect();
                    if pts.len() > 1 {
                        dl.add_polyline(pts, imgui::ImColor32::from_rgba_f32s(1.0, 0.85, 0.3, 0.8 * pop)).thickness(px * 1.5).build();
                    }
                }
                let alpha = if on { pop } else { 0.35 * pop };
                let bob = if on { (t * 4.0 + k as f32).sin() * px * 0.8 } else { 0.0 };
                dl.add_image(textures[IMG_KIT + k], [x - s * 0.5, y - s * 0.5 + bob], [x + s * 0.5, y + s * 0.5 + bob]).col([1.0, 1.0, 1.0, alpha]).build();
                // (its button under it, small)
                let g = 6.0 * px;
                let label = names[k];
                for (i, ch) in label.chars().enumerate() {
                    if let Some(id) = glyph_for(ch) {
                        let lx = x - label.len() as f32 * g * 0.4 + i as f32 * g * 0.8;
                        dl.add_image(textures[id], [lx, y + s * 0.55], [lx + g, y + s * 0.55 + g]).col([1.0, 1.0, 1.0, 0.8 * pop]).build();
                    }
                }
            }
        }

        // a captured enemy (bossride.rs): a ring for the time left round its health, low in the
        // middle, with the buttons it answers to
        if let Some((time, hp)) = crate::bossride::hud() {
            let (cx, cy) = (size[0] * 0.5, size[1] * 0.8);
            let rr = 15.0 * px;
            dl.add_circle([cx, cy], rr + 3.0 * px, imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.45)).filled(true).build();
            let n = (40.0 * time.clamp(0.0, 1.0)) as usize;
            let pts: Vec<[f32; 2]> = (0..=n)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 40.0 * std::f32::consts::TAU;
                    [cx + a.cos() * rr, cy + a.sin() * rr]
                })
                .collect();
            if pts.len() > 1 {
                let blink = time < 0.25 && (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f32() % 60.0) * 6.0).sin() > 0.0;
                dl.add_polyline(pts, imgui::ImColor32::from_rgba_f32s(1.0, 0.82, 0.3, if blink { 0.45 } else { 0.95 })).thickness(px * 2.5).build();
            }
            // (its health: an inner ring, green to red)
            let h = hp.clamp(0.0, 1.0);
            let col = imgui::ImColor32::from_rgba_f32s(1.0 - h * 0.7, 0.35 + h * 0.55, 0.3, 0.9);
            let m = (32.0 * h) as usize;
            let inner: Vec<[f32; 2]> = (0..=m)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 32.0 * std::f32::consts::TAU;
                    [cx + a.cos() * rr * 0.55, cy + a.sin() * rr * 0.55]
                })
                .collect();
            if inner.len() > 1 {
                dl.add_polyline(inner, col).thickness(px * 4.0).build();
            }
            let g = 6.0 * px;
            for (k, label) in ["X", "Y", "B", "A"].iter().enumerate() {
                if let Some(id) = label.chars().next().and_then(glyph_for) {
                    let lx = cx + (k as f32 - 1.5) * g * 1.7 - g * 0.5;
                    dl.add_image(textures[id], [lx, cy + rr + 5.0 * px], [lx + g, cy + rr + 5.0 * px + g]).col([1.0, 1.0, 1.0, 0.8]).build();
                }
            }
        }

        // an enemy worn down enough to ride (bossride.rs): a gold ring closing in over its head,
        // Mario's (or Luigi's) face in it, gone in a moment (God of War's prompt circle)
        for (head, age) in crate::bossride::marks().into_iter().flatten() {
            let Some(([cx, cy], _)) = project(head + Vec3::Y * 0.5, size).filter(|(_, z)| *z < 60.0) else { continue };
            let close = 1.0 - (age / 0.25).min(1.0);
            let r = 11.0 * px * (1.0 + 1.6 * close * close);
            let alpha = (1.0 - ((age - (crate::bossride::MARK_TIME - 0.35)) / 0.35).clamp(0.0, 1.0)) * (1.0 - close * 0.5);
            dl.add_circle([cx, cy], r, [0.0, 0.0, 0.0, 0.45 * alpha]).thickness(4.0 * px).num_segments(40).build();
            dl.add_circle([cx, cy], r, [1.0, 0.82, 0.3, 0.95 * alpha]).thickness(2.0 * px).num_segments(40).build();
            let s = 12.0 * px;
            let face = if crate::equip::luigi() { IMG_LUIGI_HEAD } else { IMG_HEAD };
            dl.add_image(textures[face], [cx - s * 0.5, cy - s * 0.5], [cx + s * 0.5, cy + s * 0.5]).col([1.0, 1.0, 1.0, alpha]).build();
        }

        // co-op: the enemy marked for a Bros. Attack (brosattack.rs), both brothers' faces over its
        // head in a ring that turns red and green, breathing a little; in quickly, out at the end
        if let Some((head, age)) = crate::brosattack::mark() {
            if let Some(([cx, cy], _)) = project(head + Vec3::Y * 0.6, size).filter(|(_, z)| *z < 60.0) {
                let fade_in = (age / 0.2).min(1.0);
                let fade_out = 1.0 - ((age - (crate::brosattack::MARK_TIME - 0.6)) / 0.6).clamp(0.0, 1.0);
                let alpha = fade_in * fade_out;
                let pop = 1.0 + 0.6 * (1.0 - fade_in).powi(2) + 0.06 * (age * 5.0).sin();
                let r = 14.0 * px * pop;
                let mix = 0.5 + 0.5 * (age * 3.0).sin();
                let tint = [1.0 - 0.65 * mix, 0.3 + 0.65 * mix, 0.25 + 0.15 * mix, 0.95 * alpha];
                dl.add_circle([cx, cy], r, [0.0, 0.0, 0.0, 0.45 * alpha]).thickness(4.0 * px).num_segments(40).build();
                dl.add_circle([cx, cy], r, tint).thickness(2.5 * px).num_segments(40).build();
                let s = 11.0 * px * pop;
                for (k, face) in [IMG_HEAD, IMG_LUIGI_HEAD].into_iter().enumerate() {
                    let x = cx + (k as f32 - 0.5) * s * 0.9;
                    dl.add_image(textures[face], [x - s * 0.5, cy - s * 0.5], [x + s * 0.5, cy + s * 0.5]).col([1.0, 1.0, 1.0, alpha]).build();
                }
            }
        }

        // the co-op partner (coop.rs), a small card bottom left that comes up only when something's
        // going on (a fight, either hurt, them low, down or far off) and fades out once it's quiet:
        // their head (greyed and crossed out when down), a little power pie with their wedges (red
        // when low, blinking at one), the power they're using, their coins and style rank; over
        // their head when they're in danger, their pie; off screen an arrow on the screen's edge
        // their way with how far they are (red and quicker in danger, grey when down)
        {
            let partners = *PARTNERS.lock().unwrap_or_else(|e| e.into_inner());
            static T0: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            let clock = T0.get_or_init(Instant::now).elapsed().as_secs_f32();
            let cam = unsafe { CSCamera::instance() }.ok().map(|c| {
                let pers = &c.pers_cam_1;
                let v = |d: eldenring::position::PositionDelta| Vec3::new(d.0, d.1, d.2).normalize_or_zero();
                let p = pers.position();
                (Vec3::new(p.0, p.1, p.2), v(pers.right()), v(pers.up()), v(pers.forward()))
            });
            let img = |id: usize, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]| {
                dl.add_image(textures[id], [x, y], [x + w, y + h]).col(col).build();
            };
            // (SM64's letters left to right from x, centred on y; how wide)
            let print = |text: &str, x: f32, y: f32, g: f32, col: [f32; 4]| -> f32 {
                let mut cx = x;
                for ch in text.chars() {
                    if let Some(id) = glyph_for(ch) {
                        img(id, cx, y - g * 0.5, g, g, col);
                    }
                    cx += g * 0.8;
                }
                cx - x
            };
            // three stars circling over a head (dazed, down: revive.rs) at screen point x, y
            let dizzy = |x: f32, y: f32, k: f32| {
                for i in 0..3 {
                    let a = clock * 3.5 + i as f32 * std::f32::consts::TAU / 3.0;
                    let (sx, sy) = (x + a.cos() * k * 1.4, y - k * 0.8 + a.sin() * k * 0.45);
                    let front = 0.7 + 0.3 * a.sin();
                    img(IMG_STAR, sx - k * 0.5, sy - k * 0.5, k, k, [1.0, 1.0, 1.0, front]);
                }
            };
            // their wing cap's wings (their model is the plain one: the wings are drawn over it), in
            // 3D: three white feathers a side, out of the cap's sides, sweeping back, beating softly
            for (head, face, k) in partners.iter().flatten().filter_map(|p| p.wings) {
                if project(head, size).is_none_or(|(_, z)| z > 60.0) {
                    continue;
                }
                let (fwd, side, up) = (face * Vec3::new(0.0, 0.0, -1.0), face * Vec3::X, Vec3::Y);
                let beat = (clock * 7.0).sin() * 0.18;
                let mut mesh = Mesh::default();
                for out in [-1.0f32, 1.0] {
                    let root = head + up * 0.16 * k + side * out * 0.12 * k - fwd * 0.02 * k;
                    for f in 0..3 {
                        let fl = f as f32;
                        let dir = (-fwd * (0.85 - 0.25 * fl) + up * (0.25 + 0.4 * fl + beat) + side * out * (0.45 + beat * 0.5)).normalize_or(up);
                        let len = (0.11 - 0.015 * fl) * k;
                        let normal = dir.cross(-fwd).normalize_or(side);
                        let across = dir.cross(normal).normalize_or(up);
                        let rot = glam::Quat::from_mat3(&glam::Mat3::from_cols(across, dir, normal)).normalize();
                        let tint = if f == 0 { [0.86, 0.87, 0.93] } else { [0.98, 0.98, 1.0] };
                        mesh.ellipsoid(root + dir * len * 0.9, rot, Vec3::new(0.035 * k, len, 0.008 * k), 0.35, 4, 8, &|_, _| tint);
                    }
                }
                mesh.draw(&dl, size, 60.0, &[]);
            }
            let mut row = 0.0;
            for p in partners.iter().flatten() {
                let s = 16.0 * px;
                let (head, tint) = if p.luigi { (IMG_LUIGI_HEAD, [0.35, 0.9, 0.4]) } else { (IMG_HEAD, [1.0, 0.35, 0.3]) };
                let pulse = 0.75 + 0.25 * (clock * if p.danger { 8.0 } else { 4.0 }).sin();
                let a = p.alpha;
                if a > 0.01 {
                    let (x0, y0) = (size[1] * 0.04, size[1] * 0.83 - row * s * 1.6);
                    row += 1.0;
                    // (the card's width: head, pie, then what's there)
                    let mut digits = [0u8; 5];
                    let coins = digits_of(p.coins, &mut digits);
                    let mut w = s * 2.4 + s * 0.75 + coins.len() as f32 * s * 0.55 * 0.8 + s * 0.85;
                    if p.power != crate::coop::POWER_NONE {
                        w += s * 0.95;
                    }
                    if p.rank.is_some() {
                        w += s * 0.9;
                    }
                    let bg = if p.danger { [0.45, 0.05, 0.05, 0.45 * a * pulse] } else { [0.0, 0.0, 0.0, 0.35 * a] };
                    dl.add_rect([x0 - px * 3.0, y0 - px * 3.0], [x0 + w + px * 3.0, y0 + s + px * 3.0], bg).filled(true).rounding(px * 4.0).build();
                    let grey = if p.down { [0.45, 0.45, 0.45, a] } else { [1.0, 1.0, 1.0, a] };
                    img(head, x0, y0, s, s, grey);
                    if p.down {
                        img(IMG_X, x0 + s * 0.2, y0 + s * 0.2, s * 0.6, s * 0.6, [1.0, 0.3, 0.25, a]);
                    }
                    let low = p.wedges <= 2;
                    let blink = !p.down && p.wedges <= 1 && (clock * 6.0).sin() < 0.0;
                    let (cx, cy) = (x0 + s * 1.75, y0 + s * 0.5);
                    let r = s * 0.62;
                    if !blink {
                        img(IMG_BASE, cx - r, cy - r, r * 2.0, r * 2.0, grey);
                        if p.wedges > 0 {
                            let pie = r * 0.5;
                            img(IMG_PIES + (p.wedges.min(8) - 1) as usize, cx - pie, cy - pie, pie * 2.0, pie * 2.0, if low { [1.0, 0.6, 0.6, a] } else { grey });
                        }
                    }
                    let mut x = x0 + s * 2.55;
                    // the element in their hand (element.rs)
                    element_badge(&dl, textures, [x + s * 0.35, cy], s * 0.33, crate::element::Element::of_weapon(p.weapon), a, clock);
                    x += s * 0.85;
                    let power = match p.power {
                        crate::coop::POWER_WING => Some(IMG_CAP_BOX + 1),
                        crate::coop::POWER_METAL => Some(IMG_CAP_BOX),
                        crate::coop::POWER_VANISH => Some(IMG_CAP_BOX + 2),
                        crate::coop::POWER_ROCK => Some(IMG_ROCK),
                        crate::coop::POWER_MEGA => Some(IMG_MUSHROOM),
                        crate::coop::POWER_YOSHI => Some(IMG_EGG),
                        crate::coop::POWER_CAPTURE => Some(IMG_CAPPY),
                        _ => None,
                    };
                    if let Some(id) = power {
                        let k = s * 0.8 * (1.0 + 0.08 * (clock * 8.0).sin());
                        img(id, x + (s * 0.8 - k) * 0.5, cy - k * 0.5, k, k, grey);
                        x += s * 0.95;
                    }
                    let c = s * 0.6;
                    img(IMG_COIN, x, cy - c * 0.5, c, c, grey);
                    x += s * 0.75;
                    x += print(coins, x, cy, s * 0.55, grey);
                    if let Some(r) = p.rank.map(|r| (r as usize).min(TINTS.len() - 1)) {
                        let t = TINTS[r];
                        print(crate::style::RANKS[r].1, x + s * 0.25, cy, s * 0.7, [t[0], t[1], t[2], a]);
                    }
                }
                let Some(at) = p.head else { continue };
                let on_screen = project(at, size).filter(|(c, _)| c[0] >= 0.0 && c[0] <= size[0] && c[1] >= 0.0 && c[1] <= size[1]);
                // down for us to revive: stars circling over their head and their seconds left;
                // within reach the D-pad down prompt and a ring filling as it's held
                if let Some(([hx, hy], _)) = on_screen.filter(|(_, z)| p.revivable && *z < 60.0) {
                    dizzy(hx, hy, s * 0.55);
                    if let Some(k) = p.revive {
                        let (cx, cy) = (hx, hy - s * 1.7);
                        let r = s * 0.75;
                        dl.add_circle([cx, cy], r, imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.45)).filled(true).build();
                        if k > 0.0 {
                            let steps = (k * 32.0).ceil() as usize;
                            let arc: Vec<[f32; 2]> = (0..=steps)
                                .map(|i| {
                                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k * i as f32 / steps as f32;
                                    [cx + a.cos() * r, cy + a.sin() * r]
                                })
                                .collect();
                            dl.add_polyline(arc, imgui::ImColor32::from_rgba_f32s(0.45, 1.0, 0.5, 1.0)).thickness(px * 3.0).build();
                        }
                        let a = s * 0.8;
                        img(IMG_ARROW_DOWN, cx - a * 0.5, cy - a * 0.5, a, a, [1.0, 1.0, 1.0, pulse.max(0.85)]);
                        let g = 9.0 * px;
                        print("RESCUE", cx - 6.0 * g * 0.4, cy + r + g * 0.9, g, [0.45, 1.0, 0.5, 1.0]);
                    } else if let Some(left) = p.down_left {
                        let mut buf = [0u8; 5];
                        let t = digits_of(left as u16, &mut buf);
                        let g = 10.0 * px;
                        print(t, hx - t.len() as f32 * g * 0.4, hy - s * 1.4, g, [1.0, 1.0, 1.0, 0.9]);
                    }
                }
                // in danger and in sight: their pie over their head
                if let (Some(([hx, hy], z)), true) = (on_screen, p.danger) {
                    if z < 60.0 && p.wedges > 0 {
                        let r = s * 0.42;
                        let hy = hy - r * 1.6;
                        img(IMG_BASE, hx - r, hy - r, r * 2.0, r * 2.0, [1.0, 1.0, 1.0, 0.9 * pulse]);
                        let pie = r * 0.5;
                        img(IMG_PIES + (p.wedges.min(8) - 1) as usize, hx - pie, hy - pie, pie * 2.0, pie * 2.0, [1.0, 0.6, 0.6, 0.9 * pulse]);
                    }
                }
                // off screen (or behind the camera): an arrow at the edge, and the distance in m
                if on_screen.is_none() {
                    let Some((cam, right, up, fwd)) = cam else { continue };
                    let d = at - cam;
                    // (behind the camera: straight to the side they're on)
                    let side = d.dot(right);
                    let dir = if d.dot(fwd) < 0.0 {
                        glam::Vec2::new(if side < 0.0 { -1.0 } else { 1.0 }, 0.0)
                    } else {
                        glam::Vec2::new(side, -d.dot(up)).normalize_or(glam::Vec2::X)
                    };
                    let c = [size[0] * 0.5, size[1] * 0.5];
                    let edge = [c[0] + dir.x * size[0] * 0.44, c[1] + dir.y * size[1] * 0.40];
                    let (ax, ay) = (dir.x, dir.y);
                    let k = if p.danger { s * 1.2 } else { s };
                    let tip = [edge[0] + ax * k * 0.7, edge[1] + ay * k * 0.7];
                    let l = [edge[0] - ay * k * 0.45, edge[1] + ax * k * 0.45];
                    let rr = [edge[0] + ay * k * 0.45, edge[1] - ax * k * 0.45];
                    let (fill, alpha) = if p.revivable {
                        ([0.45, 1.0, 0.5], 0.95 * pulse)
                    } else if p.down {
                        ([0.55, 0.55, 0.55], 0.7)
                    } else if p.danger {
                        ([1.0, 0.25, 0.2], 0.95 * pulse)
                    } else {
                        (tint, (0.55 + 0.3 * a) * pulse)
                    };
                    dl.add_triangle(tip, l, rr, imgui::ImColor32::from_rgba_f32s(fill[0], fill[1], fill[2], alpha)).filled(true).build();
                    dl.add_triangle(tip, l, rr, imgui::ImColor32::from_rgba_f32s(0.0, 0.0, 0.0, 0.6)).thickness(px).build();
                    let mut buf = [0u8; 6];
                    let metres = metres_of(d.length().round().min(999.0) as u16, &mut buf);
                    let g = 8.0 * px;
                    let n = metres.len() as f32;
                    print(metres, edge[0] - ax * k * 0.9 - n * g * 0.4, edge[1] - ay * k * 0.9, g, [1.0, 1.0, 1.0, 1.0]);
                }
            }
            // ours: down for the partner to revive (stars over our head, HELP and the seconds left
            // up top); the partner too far off (TOO FAR and the distance, pulsing)
            let (own_down, far) = *COOP.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((head, left)) = own_down {
                let s = 16.0 * px;
                if let Some(([hx, hy], _)) = project(head, size) {
                    dizzy(hx, hy, s * 0.55);
                }
                let g = 16.0 * px;
                let mut buf = [0u8; 5];
                let t = digits_of(left as u16, &mut buf);
                let w = (5.0 + t.len() as f32) * g * 0.8;
                let (x, y) = (size[0] * 0.5 - w * 0.5, size[1] * 0.2);
                let pulse = 0.75 + 0.25 * (clock * if left <= 5 { 8.0 } else { 3.0 }).sin();
                let x = x + print("HELP", x, y, g, [1.0, 0.85, 0.3, pulse]) + g * 0.8;
                print(t, x, y, g, if left <= 5 { [1.0, 0.35, 0.3, 1.0] } else { [1.0, 1.0, 1.0, 1.0] });
            }
            if let Some(d) = far.filter(|_| own_down.is_none()) {
                let g = 11.0 * px;
                let mut buf = [0u8; 6];
                let metres = metres_of(d.round().min(999.0) as u16, &mut buf);
                let w = (8.0 + metres.len() as f32) * g * 0.8;
                let (x, y) = (size[0] * 0.5 - w * 0.5, size[1] * 0.16);
                let pulse = 0.6 + 0.4 * (clock * 5.0).sin();
                let x = x + print("TOO FAR", x, y, g, [1.0, 0.85, 0.3, pulse]) + g * 0.8;
                print(metres, x, y, g, [1.0, 1.0, 1.0, 0.85]);
            }
            // the BROS! streak (blows by both on one enemy), under the style meter while it lasts
            if let Some((n, age)) = crate::coop::bros_streak() {
                let fade = 1.0 - ((age - (crate::coop::BROS_KEEP - 1.0)) / 1.0).clamp(0.0, 1.0);
                let pop = 1.0 + 0.3 * (1.0 - (age / 0.15).min(1.0));
                let g = 10.0 * px * pop;
                let mut digits = [0u8; 5];
                let count = digits_of(n.min(9999) as u16, &mut digits);
                let right = size[0] - 24.0 * px;
                let y = size[1] * 0.30 + 66.0 * px;
                let w = (5.0 + count.len() as f32) * g * 0.8;
                let col = [0.45, 1.0, 0.5, 0.9 * fade];
                let mut x = right - w;
                x += print("BROS", x, y, g, col);
                img(IMG_MULTIPLY, x, y - g * 0.4, g * 0.8, g * 0.8, col);
                print(count, x + g * 0.8, y, g, col);
            }
        }

        // the power meter, centred at the top (its height worked out above)
        if let Some(height) = meter_height {
            let (cx, cy) = (size[0] * 0.5, size[1] * (1.0 - height / 240.0));
            image(IMG_BASE, cx - 32.0 * px, cy - 32.0 * px, 64.0 * px, 64.0 * px);
            if wedges > 0 {
                image(IMG_PIES + (wedges.min(8) - 1) as usize, cx - 16.0 * px, cy - 16.0 * px, 32.0 * px, 32.0 * px);
            }
        }
    }
}

/// The giant fireball's ball (48 x 48, one of 4 frames): a sphere lit from the upper left, hot
/// Distance from (x, y) to the segment a-b.
fn seg_dist(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * abx + (y - a.1) * aby) / (abx * abx + aby * aby)).clamp(0.0, 1.0);
    let (px, py) = (a.0 + abx * t, a.1 + aby * t);
    ((x - px).powi(2) + (y - py).powi(2)).sqrt()
}

/// The Boomerang Bros.' boomerang: a fat white chevron with red tips and a dark outline.
/// The Rock Mushroom's icon: a mushroom whose cap is a grey boulder with darker spots.
fn rock_pixel(x: usize, y: usize) -> [u8; 4] {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let outline = [24, 20, 16, 255];
    // the cap: the top of an ellipse
    let (dx, dy) = ((fx - 16.0) / 14.0, (fy - 16.0) / 12.0);
    let r = (dx * dx + dy * dy).sqrt();
    if fy <= 17.0 && r < 1.0 {
        if r > 0.9 || fy > 16.0 {
            return outline;
        }
        let spot = |cx: f32, cy: f32, s: f32| ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() < s;
        if spot(10.0, 10.0, 3.0) || spot(20.0, 7.0, 2.5) || spot(23.0, 13.0, 2.0) {
            return [85, 80, 76, 255];
        }
        let shade = (150.0 - (fy - 4.0) * 3.0) as u8;
        return [shade, shade - 6, shade - 14, 255];
    }
    // the stem, with eyes
    if (17.0..29.0).contains(&fy) && (fx - 16.0).abs() < 7.0 {
        if (fx - 16.0).abs() > 6.0 || fy > 28.0 {
            return outline;
        }
        if ((fx - 13.0).abs() < 1.2 || (fx - 19.0).abs() < 1.2) && (19.0..24.0).contains(&fy) {
            return [20, 20, 20, 255];
        }
        return [245, 230, 200, 255];
    }
    [0; 4]
}

/// The trap kit's icons, 32 x 32: a banana, a green shell, a Bob-omb, a spring.
fn kit_pixel(k: usize, x: usize, y: usize) -> [u8; 4] {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let outline = [24, 20, 16, 255];
    match k {
        0 => {
            // a crescent: inside a circle, outside a smaller one shifted up
            let d1 = ((fx - 16.0).powi(2) + (fy - 12.0).powi(2)).sqrt();
            let d2 = ((fx - 16.0).powi(2) + (fy - 5.0).powi(2)).sqrt();
            if d1 < 13.0 && d2 > 11.5 {
                let tip = !(7.0..=25.0).contains(&fx);
                if d1 > 11.5 || d2 < 13.0 { outline } else if tip { [110, 70, 25, 255] } else { [255, 220, 50, 255] }
            } else {
                [0; 4]
            }
        }
        1 => {
            // a dome, a cream rim, darker plates
            let (dx, dy) = ((fx - 16.0) / 13.0, (fy - 20.0) / 13.0);
            let r = (dx * dx + dy * dy).sqrt();
            if fy > 20.0 && fy < 24.0 && (fx - 16.0).abs() < 14.0 {
                if fy > 23.0 || (fx - 16.0).abs() > 13.0 { outline } else { [245, 235, 185, 255] }
            } else if fy <= 20.0 && r < 1.0 {
                if r > 0.9 { outline } else if ((fx / 5.0).floor() as i32 + (fy / 5.0).floor() as i32) % 2 == 0 { [30, 170, 45, 255] } else { [15, 110, 25, 255] }
            } else {
                [0; 4]
            }
        }
        2 => {
            // a black ball, white eyes, a fuse with a spark
            let d = ((fx - 15.0).powi(2) + (fy - 18.0).powi(2)).sqrt();
            let eye = |ex: f32| ((fx - ex) / 2.0).powi(2) + ((fy - 15.0) / 3.5).powi(2) < 1.0;
            if (fx - 22.0).abs() < 2.5 && (fy - 6.0).abs() < 2.5 {
                [255, 200, 60, 255]
            } else if (fx - 20.0).abs() < 1.5 && (6.0..=9.0).contains(&fy) {
                [210, 160, 100, 255]
            } else if d < 11.0 {
                if eye(11.5) || eye(17.5) { [255, 255, 255, 255] } else if d > 10.0 { outline } else { [30, 30, 38, 255] }
            } else {
                [0; 4]
            }
        }
        _ => {
            // a red top, a zigzag coil, a dark base
            if (6.0..=26.0).contains(&fx) && (4.0..=9.0).contains(&fy) {
                if fy < 5.0 || fy > 8.0 || fx < 7.0 || fx > 25.0 { outline } else { [225, 40, 30, 255] }
            } else if (8.0..=24.0).contains(&fx) && (24.0..=28.0).contains(&fy) {
                [70, 70, 80, 255]
            } else if (9.0..24.0).contains(&fy) {
                let phase = ((fy - 9.0) / 5.0).fract();
                let want = 9.0 + 14.0 * if phase < 0.5 { phase * 2.0 } else { 2.0 - phase * 2.0 };
                if (fx - want).abs() < 1.8 { [190, 195, 205, 255] } else { [0; 4] }
            } else {
                [0; 4]
            }
        }
    }
}

fn rang_pixel(x: usize, y: usize) -> [u8; 4] {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (c, l, r) = ((16.0, 21.0), (4.5, 8.0), (27.5, 8.0));
    let d = seg_dist(x, y, c, l).min(seg_dist(x, y, c, r));
    if d > 4.6 {
        return [0; 4];
    }
    if d > 3.4 {
        return [40, 30, 50, 255];
    }
    // (the tips red, a shine along the top edge)
    let tip = (x - 16.0).abs() > 8.5;
    if tip {
        [230, 40, 40, 255]
    } else if y < 15.0 - (x - 16.0).abs() * 0.95 {
        [255, 255, 255, 255]
    } else {
        [225, 228, 240, 255]
    }
}

/// The Chain Chomp, facing right: a black ball with a shine, its big eye, its teeth (open: a red
/// mouth full of them).
fn chomp_pixel(x: usize, y: usize, open: bool) -> [u8; 4] {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (dx, dy) = (x - 24.0, y - 24.0);
    let r = (dx * dx + dy * dy).sqrt();
    if r > 21.5 {
        return [0; 4];
    }
    // the mouth: a wedge to the right, open wide or a line of teeth
    let ang = dy.atan2(dx);
    let half = if open { 0.55 } else { 0.0 };
    let mouth_c = 0.18;
    if open && r > 6.0 && (ang - mouth_c).abs() < half {
        let edge = half - (ang - mouth_c).abs();
        // (teeth along the lips)
        let k = ((r - 6.0) / 3.2).floor() as i32;
        let tooth = edge < 0.22 && (k % 2 == 0 || edge < 0.1);
        return if tooth { [250, 250, 245, 255] } else { [150, 20, 25, 255] };
    }
    if !open && r > 9.0 && (ang - mouth_c).abs() < 0.13 {
        let k = ((r - 9.0) / 2.6).floor() as i32;
        return if (ang - mouth_c).abs() < 0.04 { [40, 0, 0, 255] } else if k % 2 == 0 { [250, 250, 245, 255] } else { [20, 20, 22, 255] };
    }
    // the eye: white with a black pupil looking ahead
    let (ex, ey) = (x - 28.0, y - 13.5);
    let er = (ex * ex + ey * ey).sqrt();
    if er < 6.5 {
        let (px, py) = (x - 30.0, y - 13.5);
        return if (px * px + py * py).sqrt() < 2.6 { [10, 10, 10, 255] } else if er > 5.6 { [30, 30, 30, 255] } else { [250, 250, 250, 255] };
    }
    // the ball, lit from the top left
    let light = (1.0 - ((x - 15.0).powi(2) + (y - 13.0).powi(2)).sqrt() / 30.0).max(0.0);
    let shine = if ((x - 14.0).powi(2) + (y - 12.0).powi(2)).sqrt() < 3.5 { 120.0 } else { 0.0 };
    let v = (18.0 + 70.0 * light * light + shine).min(255.0) as u8;
    let rim = if r > 20.0 { 0.5 } else { 1.0 };
    [(v as f32 * rim) as u8, (v as f32 * rim) as u8, ((v as f32 + 8.0) * rim) as u8, 255]
}

/// A link of the Chomp's chain: a grey ring.
fn link_pixel(x: usize, y: usize) -> [u8; 4] {
    let (dx, dy) = (x as f32 - 7.5, (y as f32 - 7.5) * 1.3);
    let r = (dx * dx + dy * dy).sqrt();
    if !(3.0..=6.8).contains(&r) {
        return [0; 4];
    }
    let v = if dy < 0.0 { 190 } else { 120 };
    [v, v, (v as u32 + 10).min(255) as u8, 255]
}

/// The hammer's icon: a wooden handle and a dark head with a band.
fn hammer_icon_pixel(x: usize, y: usize) -> [u8; 4] {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let head = (6.0..26.0).contains(&x) && (5.0..15.0).contains(&y);
    let band = head && (13.0..19.0).contains(&x);
    let handle = (14.0..18.0).contains(&x) && (14.0..29.0).contains(&y);
    let edge = |inside: bool, xi: f32, yi: f32| inside && !(xi < 7.0 || xi > 25.0 || yi < 6.0 || yi > 14.0);
    if head {
        if !edge(true, x, y) {
            return [20, 20, 24, 255];
        }
        return if band { [200, 160, 60, 255] } else if y < 8.0 { [110, 110, 120, 255] } else { [70, 70, 80, 255] };
    }
    if handle {
        return if x < 15.0 || x > 17.0 { [70, 40, 20, 255] } else { [170, 110, 55, 255] };
    }
    [0; 4]
}

/// A Yoshi egg: white with green spots, a dark outline.
fn egg_pixel(x: usize, y: usize) -> [u8; 4] {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (cx, cy) = (12.0, 17.0);
    // (narrower at the top)
    let rx = if y < cy { 9.0 + (y - cy) * 0.12 } else { 10.5 };
    let e = ((x - cx) / rx).powi(2) + ((y - cy) / 14.0).powi(2);
    if e > 1.0 {
        return [0; 4];
    }
    if e > 0.8 {
        return [30, 60, 30, 255];
    }
    let spot = |sx: f32, sy: f32, r: f32| ((x - sx).powi(2) + (y - sy).powi(2)).sqrt() < r;
    if spot(8.0, 12.0, 3.0) || spot(16.0, 20.0, 3.4) || spot(9.5, 24.5, 2.2) || spot(15.5, 8.0, 2.0) {
        return [70, 190, 60, 255];
    }
    let shade = if x < 9.0 && y < 14.0 { 255 } else { 235 };
    [shade, shade, (shade as u32 * 96 / 100) as u8, 255]
}

/// Mario's cap thrown as the hook: red, the white badge with its M, a dark brim, two big eyes.
fn cappy_pixel(x: usize, y: usize) -> [u8; 4] {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (cx, cy) = (16.0, 20.0);
    let dome = y <= cy && ((x - cx) / 14.0).powi(2) + ((y - cy) / 15.0).powi(2) <= 1.0;
    let brim = (cy..cy + 6.0).contains(&y) && ((x - cx) / 15.5).powi(2) + ((y - cy - 1.5) / 4.5).powi(2) <= 1.0;
    if dome {
        if ((x - cx) / 12.8).powi(2) + ((y - cy) / 13.8).powi(2) > 1.0 {
            return [50, 10, 10, 255];
        }
        // the eyes, low on the dome
        for ex in [11.0, 21.0] {
            let (dx, dy) = ((x - ex) / 3.0, (y - 14.5) / 4.2);
            if dx * dx + dy * dy <= 1.0 {
                let pupil = ((x - ex - 0.6) / 1.4).powi(2) + ((y - 15.2) / 2.2).powi(2) <= 1.0;
                return if pupil { [20, 20, 30, 255] } else { [255, 255, 255, 255] };
            }
        }
        // the badge
        let (bx, by) = (x - cx, y - 6.5);
        if bx * bx + by * by <= 16.0 {
            let m = (by > -2.2 && by < 2.6) && (bx.abs() > 1.6 && bx.abs() < 2.8 || (bx.abs() <= 1.6 && (by + 1.5 - bx.abs()).abs() < 0.9));
            return if m { [220, 30, 30, 255] } else { [255, 255, 255, 255] };
        }
        return if y < 8.0 && x < 12.0 { [255, 90, 80, 255] } else { [215, 25, 25, 255] };
    }
    if brim {
        return if y < cy + 2.0 { [170, 15, 15, 255] } else { [120, 10, 10, 255] };
    }
    [0; 4]
}

/// SM64's mushroom, a Mega one: a red cap with white spots over a cream stem with eyes.
fn mushroom_pixel(x: usize, y: usize) -> [u8; 4] {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (cx, cy) = (16.0, 17.0);
    let cap = y <= cy && ((x - cx) / 14.5).powi(2) + ((y - cy) / 13.5).powi(2) <= 1.0;
    let stem = y > cy - 1.0 && y < 30.0 && (x - cx).abs() < 8.5 - (y - cy) * 0.15;
    let outline = |on: bool, grow: f32| {
        on && !(y <= cy && ((x - cx) / (14.5 - grow)).powi(2) + ((y - cy) / (13.5 - grow)).powi(2) <= 1.0)
    };
    if cap {
        if outline(true, 1.2) {
            return [40, 10, 10, 255];
        }
        let spot = |sx: f32, sy: f32, r: f32| ((x - sx).powi(2) + (y - sy).powi(2)).sqrt() < r;
        if spot(16.0, 8.5, 4.2) || spot(6.5, 13.5, 3.0) || spot(25.5, 13.5, 3.0) {
            return [255, 255, 255, 255];
        }
        return if y < 9.0 && x < 13.0 { [255, 90, 80, 255] } else { [220, 30, 30, 255] };
    }
    if stem {
        if (x - cx).abs() > 7.3 - (y - cy) * 0.15 || y > 28.8 {
            return [60, 40, 20, 255];
        }
        let eye = (x - 12.5).abs() < 1.3 && (20.0..25.0).contains(&y) || (x - 19.5).abs() < 1.3 && (20.0..25.0).contains(&y);
        return if eye { [20, 20, 20, 255] } else { [250, 225, 180, 255] };
    }
    [0; 4]
}

/// white in the middle through yellow and orange to a deep red rim, its surface churning (value
/// noise, shifted with each frame).
fn orb_pixel(x: usize, y: usize, frame: usize) -> [u8; 4] {
    let (dx, dy) = ((x as f32 - 23.5) / 23.5, (y as f32 - 23.5) / 23.5);
    let r2 = dx * dx + dy * dy;
    if r2 >= 1.0 {
        return [0; 4];
    }
    let nz = (1.0 - r2).sqrt();
    let light = (-0.45 * dx - 0.5 * dy + 0.74 * nz).max(0.0);
    fn hash(x: i32, y: i32) -> f32 {
        let h = (x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263)) as u32;
        let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        (h ^ (h >> 16)) as f32 / u32::MAX as f32
    }
    fn noise(x: f32, y: f32) -> f32 {
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let top = hash(ix, iy) + (hash(ix + 1, iy) - hash(ix, iy)) * sx;
        let bottom = hash(ix, iy + 1) + (hash(ix + 1, iy + 1) - hash(ix, iy + 1)) * sx;
        top + (bottom - top) * sy
    }
    // (the surface, bent round the ball, drifting up and across frame by frame)
    let (u, v) = (dx / (nz + 0.5) * 3.0 + frame as f32 * 0.7, dy / (nz + 0.5) * 3.0 - frame as f32 * 0.9);
    let churn = noise(u, v) * 0.65 + noise(u * 2.3 + 5.0, v * 2.3) * 0.35;
    let heat = (0.4 * nz + 0.75 * (churn - 0.5) + 0.3 * light + 0.15).clamp(0.0, 1.0);
    let stops = [(0.0, [110.0, 15.0, 5.0]), (0.35, [225.0, 80.0, 15.0]), (0.6, [255.0, 165.0, 40.0]), (0.8, [255.0, 225.0, 110.0]), (1.0, [255.0, 252.0, 230.0])];
    let mut c = stops[0].1;
    for w in stops.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if heat >= a {
            let k = ((heat - a) / (b - a)).min(1.0);
            c = [ca[0] + (cb[0] - ca[0]) * k, ca[1] + (cb[1] - ca[1]) * k, ca[2] + (cb[2] - ca[2]) * k];
        }
    }
    // (a soft edge)
    let alpha = ((1.0 - r2.sqrt()) * 12.0).clamp(0.0, 1.0);
    [c[0] as u8, c[1] as u8, c[2] as u8, (alpha * 255.0) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_without_allocating() {
        let mut d = [0u8; 5];
        assert_eq!(digits_of(0, &mut d), "0");
        assert_eq!(digits_of(7, &mut d), "7");
        assert_eq!(digits_of(65535, &mut d), "65535");
        let mut m = [0u8; 6];
        assert_eq!(metres_of(48, &mut m), "48M");
        assert_eq!(metres_of(999, &mut m), "999M");
    }
}
