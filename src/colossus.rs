//! Shadow of the Colossus on Elden Ring's giants, Mario's way, and the hook's rope.
//!
//! The colossi: enemies taller than TALL or wider than WIDE. Their skeleton's bones (feet, knees,
//! hips, hands, arms, shoulders, back, neck, head, tail: engine_mario::pose_of) are the spots to
//! hold on to, moving with their animation; one whose bones can't be read (or don't sit in its
//! body) gets spots round its body instead.
//! - In the air near one of those spots, A grabs on: Mario hangs from it (on its back or its head,
//!   he stands). His grip runs down while he hangs, faster the harder it thrashes (the spot's
//!   speed), and comes back standing on top; out of grip, he falls. A blow on him shakes him off.
//! - A leaps to the next spot (the stick picks which way, else the highest near), B lets go.
//! - Two weak points glow on each (gold stars: the head, and its back or a limb): X held on one
//!   charges the star strike (he trembles with it), let go it lands, harder the longer it charged,
//!   with stars bursting, a stance crack and a slow moment. X tapped anywhere else is a small blow.
//!   Every star struck out: COLOSSAL!, its stance broken (the tail grab's moment).
//! - The camera backs off by its size and sinks below him as he climbs, looking up at the beast.
//! The hook's rope (hook.rs hands it over): R2 held as the cap catches a spot high above (a branch,
//! a ledge, a ceiling) or a colossus (its highest steady spot in reach: head, neck, back,
//! shoulders), Mario swings from it on a rope, a real pendulum, a tetherball round a colossus: run
//! by one, hook it, and the run carries him round and round its body (never through it), the stick
//! pumping the swing. A held climbs up the rope (shorter, faster, he leaves the ground), B lets
//! more of it out; the ground under him he runs along, the rope taken up. All the way up to a
//! colossus's spot, A grabs on to it. R2 let go: off along the swing, all its speed kept and a bit
//! more, up, in a flying kick: the catapult. In the air near a colossus's spot, A grabs on again.
//! On a colossus, L2 held slides him down the limb he hangs from, spot to spot, faster and faster:
//! A jumps off with all that speed, R2 throws the rope (to the highest spot in reach, or the map
//! where the camera looks) and the slide's speed feeds the swing; off the limb's end he's flung.
//! The hook's reach and the rope's length go by the place: shut in, as ever (20 m); out in the
//! open by a huge colossus, up to HOOK_MAX, a rope up to ROPE_LONGEST for a swing worth seeing.

use std::f32::consts::TAU;
use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::combat::{self, Attack, Combat};
use crate::{log, sm64, worker};

pub const ACT_ER_CLING: u32 = 0x0000_035B;
const ACT_FLAG_AIR: u32 = 0x800;
const ACT_FREEFALL: u32 = 0x0100_088C;
const ACT_ER_SWING_KICK: u32 = 0x0080_0896;
const ACT_ER_ZIP: u32 = 0x0080_0892;

/// ACT_ER_CLING's poses (libsm64's er_cling_mode)
const HANG: i32 = 0;
const TOP: i32 = 1;
const LEAP: i32 = 2;
const STRIKE: i32 = 3;
const CHARGE: i32 = 4;
const SWING: i32 = 6;
const SLIDE: i32 = 7;

/// a colossus: this tall, or this wide (m)
const TALL: f32 = 3.6;
const WIDE: f32 = 1.6;
/// in the air, A grabs a spot this close to his chest (m); the next spot is leapt to up to this
/// far (m); colossi looked for this far from him (m)
const GRAB_REACH: f32 = 2.4;
const LEAP_REACH: f32 = 7.5;
const LOOK_FOR: f32 = 16.0;
/// grip (a share a second): hanging, more for each m/s it moves past SHAKE_FROM; back on top
const GRIP_HANG: f32 = 0.085;
const GRIP_SHAKE: f32 = 0.12;
const SHAKE_FROM: f32 = 2.0;
const GRIP_TOP: f32 = 0.22;
/// fallen off: no grabbing on again for this long (s)
const FALL_WAIT: f32 = 1.2;
/// his feet below his hands, hanging (m)
const HANG_DROP: f32 = 1.3;
/// the star strike: full after this long held (s); half from this; a tap's wait (s); stars and
/// strikes each
const STRIKE_FULL: f32 = 1.1;
const STRIKE_HALF: f32 = 0.3;
const TAP_EVERY: f32 = 0.45;
const SIGIL_HITS: u32 = 3;
/// struck out, the stars are back after this long (s)
const SIGILS_BACK: f32 = 45.0;
/// the rope: its gravity (m/s², a game's), the stick's pump (m/s²), the climb up and down it (m/s),
/// its length (m), how long one lasts (s), his hands below the rope's end (m: his feet HANG_DROP
/// lower), the fastest he's let go at (m/s), the let go's boost (share of his speed, and up: m/s
/// plus a share of his speed), how far off a colossus's body he swings round it (m), how fast the
/// rope's end follows its spot (share a tick: an arm's jolts eased)
const GRAVITY: f32 = 24.0;
const PUMP: f32 = 9.0;
const CLIMB: f32 = 4.5;
const ROPE_MIN: f32 = 2.5;
/// the rope's longest: shut in, and out in the open by a huge colossus (m)
const ROPE_MAX: f32 = 18.0;
const ROPE_LONGEST: f32 = 40.0;
/// the hook's reach: shut in (hook.rs's own), more in the open, more again by a big colossus, at
/// most (m)
const HOOK_BASE: f32 = 20.0;
const HOOK_OPEN: f32 = 8.0;
const HOOK_MAX: f32 = 50.0;
/// the slide down a limb: its pull (m/s², more the steeper), its fastest (m/s), the speed it starts
/// at (m/s)
const SLIDE_PULL: f32 = 16.0;
const SLIDE_FASTEST: f32 = 26.0;
const SLIDE_START: f32 = 2.5;
/// SM64's slide down a tree (a moving sound: played every tick of the slide, soft)
const SOUND_SLIDE: i32 = 0x1412_8001;
const ROPE_TIME: f32 = 20.0;
const LET_GO_MAX: f32 = 42.0;
/// a jump off a slide (m/s)
const JUMP_OFF: f32 = 7.0;
const LET_GO_BOOST: f32 = 1.1;
const LET_GO_UP: f32 = 1.5;
const LET_GO_UP_SHARE: f32 = 0.08;
const BODY_GAP: f32 = 0.7;
const PIVOT_EASE: f32 = 0.35;
/// a spot this far above his feet for the rope (m)
/// (more than his hang and the shortest rope: from his feet, a rope that short has him in the ground)
pub const ROPE_ABOVE: f32 = 4.0;

const SOUND_GRAB: i32 = 0x0436_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;
const SOUND_STAR: i32 = 0x7828_9081;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Part {
    Foot,
    Knee,
    Hip,
    Hand,
    Arm,
    Shoulder,
    Back,
    Neck,
    Head,
    Tail,
    Side,
}

impl Part {
    /// on top of it: he stands (and gets his grip back)
    fn top(self) -> bool {
        matches!(self, Part::Back | Part::Head)
    }
    /// steady enough to hang the rope from (not a limb's end that flails about)
    fn steady(self) -> bool {
        matches!(self, Part::Head | Part::Neck | Part::Back | Part::Shoulder | Part::Hip | Part::Side)
    }
}

/// A spot to hold on to: its bone (or a point round the body, 1000 on), its part, where it is.
#[derive(Clone, Copy, Debug)]
struct Spot {
    id: u32,
    part: Part,
    at: Vec3,
}

/// A colossus as it is now.
struct Titan {
    mob: FieldInsHandle,
    key: u64,
    feet: Vec3,
    radius: f32,
    height: f32,
    fwd: Vec3,
    spots: Vec<Spot>,
}

impl Titan {
    fn spot(&self, id: u32) -> Option<Spot> {
        self.spots.iter().copied().find(|s| s.id == id)
    }
    /// out from its middle at `p` (level), the way his back is to
    fn out(&self, p: Vec3, or: Vec3) -> Vec3 {
        let o = (p - self.feet).with_y(0.0);
        if o.length() > 0.2 { o.normalize() } else { (or - p).with_y(0.0).normalize_or(-self.fwd) }
    }
    /// a spot within its body (a bone the reading puts far off it, or one flung out on a limb's end,
    /// is no place to hang the rope from: it hung in the sky beside the beast)
    fn in_body(&self, s: &Spot) -> bool {
        let flat = (s.at - self.feet).with_y(0.0).length();
        flat <= self.radius * 2.0 + 1.2 && s.at.y >= self.feet.y - 0.6 && s.at.y <= self.feet.y + self.height * 1.25 + 0.8
    }
    fn head(&self) -> Vec3 {
        self.spots.iter().find(|s| s.part == Part::Head).map_or(self.feet + Vec3::Y * self.height, |s| s.at)
    }
}

#[derive(Clone, Copy)]
enum Pivot {
    Map(Vec3),
    Spot(FieldInsHandle, u32),
}

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    Cling { mob: FieldInsHandle, spot: u32, since: Instant, charge: Option<Instant>, strike_until: Option<Instant>, last_tap: Option<Instant> },
    Leap { mob: FieldInsHandle, spot: u32, from: Vec3, since: Instant, dur: f32, arc: f32 },
    /// `at`: where it hangs from now (eased after its spot), `most`: the rope's longest here,
    /// `climb`: A came up since it started (a jump's A still held doesn't climb, nor grab on)
    Rope { pivot: Pivot, len: f32, hand: Vec3, vel: Vec3, since: Instant, at: Vec3, most: f32, climb: bool },
    /// down a limb from spot `from` to spot `to`, `along` the way (0-1), at `speed` (m/s)
    Slide { mob: FieldInsHandle, from: u32, to: u32, along: f32, speed: f32, since: Instant },
}

struct Sigils {
    key: u64,
    /// the spots' ids and the strikes each has left
    stars: Vec<(u32, u32)>,
    done_at: Option<Instant>,
}

struct State {
    phase: Phase,
    grip: f32,
    fall_wait: Option<Instant>,
    chain: u32,
    sigils: Vec<Sigils>,
    /// buttons last tick: A, B, X, R2
    prev: [bool; 4],
    /// the spots held last, for their speed: whose (its key), when, (id, where)
    last_spots: (u64, Option<Instant>, Vec<(u32, Vec3)>),
    hud: Option<Hud>,
    /// Mario's libsm64 id (to let go of him from outside the tick)
    id: i32,
}

static STATE: Mutex<State> = Mutex::new(State {
    phase: Phase::Idle,
    grip: 1.0,
    fall_wait: None,
    chain: 0,
    sigils: Vec::new(),
    prev: [false; 4],
    last_spots: (0, None, Vec::new()),
    hud: None,
    id: -1,
});

/// What hud.rs draws: his grip (0-1, None off a colossus), whether it thrashes, the star strike's
/// charge (0-1), the stars (where, the way out of the body there, strikes left of SIGIL_HITS) and
/// the spot a leap would go to.
#[derive(Clone, Default)]
pub struct Hud {
    pub grip: Option<f32>,
    pub shaking: bool,
    pub charge: f32,
    pub stars: Vec<(Vec3, Vec3, f32)>,
    pub next: Option<Vec3>,
    pub mario: Vec3,
}

pub fn hud() -> Option<Hud> {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).hud.clone()
}

/// On a colossus or on the rope.
pub fn active() -> bool {
    !matches!(STATE.lock().unwrap_or_else(|e| e.into_inner()).phase, Phase::Idle)
}

/// The hook's reach from `feet` (m): shut in, as ever; out in the open, more, and more again by a
/// big colossus near (a swing worth seeing).
pub fn hook_reach(feet: Vec3, origin: [f32; 3]) -> f32 {
    let open = crate::lakitu::openness();
    if open < 0.05 {
        return HOOK_BASE;
    }
    let tallest = combat::nearby(&HavokPosition(feet.x, feet.y, feet.z, 0.0), 45.0, origin)
        .iter()
        .filter(|t| !t.friendly() && !t.is_prop())
        .map(|t| t.height * crate::scale())
        .fold(0.0f32, f32::max);
    let big = ((tallest - TALL) * 2.0).clamp(0.0, 22.0);
    (HOOK_BASE + open * (HOOK_OPEN + big)).min(HOOK_MAX)
}

/// The rope's longest from here: as the hook's reach goes.
fn rope_max(reach: f32) -> f32 {
    (ROPE_MAX + (reach - HOOK_BASE) * 0.75).clamp(ROPE_MAX, ROPE_LONGEST)
}

/// The rope while he swings on it (his hand, where it hangs from), the hook's line drawn along it
/// when colossus.rs threw it (hook.rs draws its own).
pub fn rope_line() -> Option<(Vec3, Vec3)> {
    match STATE.lock().unwrap_or_else(|e| e.into_inner()).phase {
        Phase::Rope { hand, at, .. } => Some((hand, at)),
        _ => None,
    }
}

/// Where the rope hangs from (the hook's cap there), while he swings.
pub fn rope_pivot() -> Option<Vec3> {
    match STATE.lock().unwrap_or_else(|e| e.into_inner()).phase {
        Phase::Rope { at, .. } => Some(at),
        _ => None,
    }
}

/// Out of Mario mode, dead: off whatever he holds.
pub fn clear() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.phase = Phase::Idle;
    s.hud = None;
    s.grip = 1.0;
    crate::lakitu::set_colossus(None);
}

/// The hook dropped mid-swing (hook.rs): the rope's gone, he falls on (with half its speed).
pub fn cancel_rope() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Phase::Rope { vel, .. } = s.phase {
        s.phase = Phase::Idle;
        let id = s.id;
        drop(s);
        let_go(id, vel * 0.5, false);
    }
}

/// He's put elsewhere (a warp, the world re-scaled: lib.rs): off whatever he holds, falling.
pub fn release() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(s.phase, Phase::Idle) {
        return;
    }
    s.phase = Phase::Idle;
    s.hud = None;
    let id = s.id;
    drop(s);
    crate::lakitu::set_colossus(None);
    let_go(id, Vec3::ZERO, false);
}

/// The world's origin shifted by `d` (lib.rs): the places kept in its terms with it.
pub fn shift(d: Vec3) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.last_spots.2.iter_mut().for_each(|(_, p)| *p += d);
    match &mut s.phase {
        Phase::Rope { pivot, hand, at, .. } => {
            *hand += d;
            *at += d;
            if let Pivot::Map(p) = pivot {
                *p += d;
            }
        }
        Phase::Leap { from, .. } => *from += d,
        _ => {}
    }
}

/// Bigger than a man by far: a colossus to climb (radius and height in m).
pub fn is_colossus(radius: f32, height: f32) -> bool {
    !crate::mega::active() && (height > TALL || radius > WIDE)
}

/// The bones' parts by their names (FromSoftware's: L_Hand, R_Calf, Spine2, Head...).
fn part_of(name: &str) -> Option<Part> {
    let n = name.to_ascii_lowercase();
    const SKIP: [&str; 14] = ["twist", "armor", "nub", "_end", "finger", "toe", "dmy", "eye", "jaw", "sfx", "_ik", "pole", "master", "root"];
    if SKIP.iter().any(|s| n.contains(s)) {
        return None;
    }
    Some(if n.contains("head") {
        Part::Head
    } else if n.contains("neck") {
        Part::Neck
    } else if n.contains("spine") {
        Part::Back
    } else if n.contains("pelvis") || n.contains("hips") || n.contains("thigh") {
        Part::Hip
    } else if n.contains("clavicle") || n.contains("shoulder") || n.contains("upperarm") {
        Part::Shoulder
    } else if n.contains("forearm") {
        Part::Arm
    } else if n.contains("hand") || n.contains("palm") {
        Part::Hand
    } else if n.contains("calf") || n.contains("knee") {
        Part::Knee
    } else if n.contains("foot") {
        Part::Foot
    } else if n.contains("tail") {
        Part::Tail
    } else {
        return None;
    })
}

/// Per skeleton: its bones to hold on to (bone, part); empty: none fit.
static PARTS: Mutex<Vec<(usize, Vec<(usize, Part)>)>> = Mutex::new(Vec::new());
/// kinds whose bones were found out of their body (character id): spots round the body for them;
/// the times each was seen so, taken for good after OFF_BODY_AFTER (one odd frame isn't enough)
static OFF_BODY: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static OFF_BODY_SEEN: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
const OFF_BODY_AFTER: u32 = 10;

fn parts_of(skeleton: usize) -> Vec<(usize, Part)> {
    let mut cache = PARTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, p)) = cache.iter().find(|(k, _)| *k == skeleton) {
        return p.clone();
    }
    let names = crate::engine_mario::bone_names(skeleton).unwrap_or_default();
    let mut parts: Vec<(usize, Part)> = names.iter().enumerate().filter_map(|(i, n)| part_of(n).map(|p| (i, p))).collect();
    // (at most a few of each: the last spine bones are its back's top, a tail's every other)
    let mut kept: Vec<(usize, Part)> = Vec::new();
    for (i, p) in parts.drain(..).rev() {
        let n = kept.iter().filter(|(_, q)| *q == p).count();
        let most = match p {
            Part::Back | Part::Tail => 3,
            Part::Head | Part::Neck => 1,
            _ => 4,
        };
        if n < most {
            kept.push((i, p));
        }
    }
    if cache.len() >= 32 {
        cache.remove(0);
    }
    log(format!("colossus: skeleton {skeleton:#x}, {} bones, {} spots to hold ({:?})", names.len(), kept.len(), kept.iter().map(|(_, p)| *p).collect::<Vec<_>>()));
    cache.push((skeleton, kept.clone()));
    kept
}

/// The colossus `h` now: its body and the spots on it.
fn titan(h: &FieldInsHandle) -> Option<Titan> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr: &ChrIns = wcm.chr_ins_by_handle(h)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let ph = &chr.modules.physics;
    let feet = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let height = ph.hit_height.max(ph.chr_hit_height).clamp(0.5, 40.0);
    let radius = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.3, 15.0);
    let o = ph.orientation;
    let fwd = Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(Vec3::new(0.0, 0.0, -1.0)).with_y(0.0).normalize_or(Vec3::Z);
    let mut spots = Vec::new();
    let off_body = OFF_BODY.lock().unwrap_or_else(|e| e.into_inner()).contains(&chr.character_id);
    if !off_body {
        if let Some((skeleton, model, count)) = crate::engine_mario::pose_of(chr as *const ChrIns as usize) {
            let m = &chr.chr_ctrl.model_matrix;
            let (x, y, z, t) = (
                Vec3::new(m.0 .0, m.0 .1, m.0 .2),
                Vec3::new(m.1 .0, m.1 .1, m.1 .2),
                Vec3::new(m.2 .0, m.2 .1, m.2 .2),
                Vec3::new(m.3 .0, m.3 .1, m.3 .2),
            );
            // (every bone kept, the same from frame to frame: a spot held doesn't vanish as the
            // pose moves; one out of its body only counts against the skeleton's reading, and
            // isn't hung from by the rope: Titan::in_body)
            let mut out = 0;
            for (b, part) in parts_of(skeleton) {
                if b >= count {
                    continue;
                }
                let l = unsafe { *((model + b * 0x30) as *const [f32; 3]) };
                let at = t + x * l[0] + y * l[1] + z * l[2];
                if !at.is_finite() {
                    continue;
                }
                let flat = (at - feet).with_y(0.0).length();
                if flat > radius * 3.0 + 2.5 || at.y < feet.y - 1.0 || at.y > feet.y + height * 1.6 + 1.5 {
                    out += 1;
                }
                spots.push(Spot { id: b as u32, part, at });
            }
            // (most of them out of its body, time after time: the matrix turns its skeleton wrong;
            // spots round its body for its kind from then on)
            if out > 3 && out * 2 > spots.len() {
                let mut bad = OFF_BODY_SEEN.lock().unwrap_or_else(|e| e.into_inner());
                let n = match bad.iter_mut().find(|(k, _)| *k == chr.character_id) {
                    Some((_, n)) => {
                        *n += 1;
                        *n
                    }
                    None => {
                        bad.push((chr.character_id, 1));
                        1
                    }
                };
                if n == OFF_BODY_AFTER {
                    log(format!("colossus: c{:04}'s bones are out of its body ({out} of {}): spots round its body instead", chr.character_id, spots.len()));
                    OFF_BODY.lock().unwrap_or_else(|e| e.into_inner()).push(chr.character_id);
                }
                spots.clear();
            }
        }
    }
    // (no bones to go by: spots round its body, at two heights, and its top)
    if spots.len() < 3 {
        spots.clear();
        let side = fwd.cross(Vec3::Y);
        for k in 0..8u32 {
            let a = k as f32 * TAU / 8.0;
            let dir = fwd * a.cos() + side * a.sin();
            let up = if k % 2 == 0 { 0.4 } else { 0.7 };
            spots.push(Spot { id: 1000 + k, part: Part::Side, at: feet + dir * radius * 0.9 + Vec3::Y * height * up });
        }
        spots.push(Spot { id: 1100, part: Part::Back, at: feet + Vec3::Y * height });
    }
    Some(Titan { mob: *h, key: combat::handle_key(h), feet, radius, height, fwd, spots })
}

/// The colossi near `at` (Mario's feet).
fn titans_near(at: Vec3, origin: [f32; 3]) -> Vec<Titan> {
    combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), LOOK_FOR, origin)
        .iter()
        .filter(|t| !t.friendly() && !t.is_prop() && is_colossus(t.radius * crate::scale(), t.height * crate::scale()))
        .filter_map(|t| t.shootable().and_then(titan))
        .take(4)
        .collect()
}

fn pivot_at(p: Pivot) -> Option<Vec3> {
    match p {
        Pivot::Map(at) => Some(at),
        Pivot::Spot(h, id) => titan(&h).and_then(|t| t.spot(id)).map(|s| s.at),
    }
}

/// Where Mario's feet go on a spot, and the way he faces (Elden Ring).
fn hold(t: &Titan, s: &Spot, mario: Vec3) -> (Vec3, Vec3) {
    if s.part.top() {
        (s.at + Vec3::Y * 0.35, t.fwd)
    } else {
        let out = t.out(s.at, mario);
        let pad = (t.radius * 0.3).clamp(0.3, 1.2);
        let feet = s.at + out * pad - Vec3::Y * HANG_DROP;
        // (a low spot: his feet on the ground by it, not in it)
        (feet.with_y(feet.y.max(t.feet.y + 0.05)), -out)
    }
}

/// Mario put at `feet` (Elden Ring), facing `dir`, in pose `mode` (into ACT_ER_CLING with `start`).
fn place(id: i32, origin: [f32; 3], feet: Vec3, dir: Vec3, mode: i32, start: bool) {
    let sm = crate::collision::er_to_sm(origin, &HavokPosition(feet.x, feet.y, feet.z, 0.0));
    let yaw = (dir.x.abs() + dir.z.abs() > 1e-3).then(|| (-dir.x).atan2(dir.z));
    worker::call("colossus place", move |_| unsafe {
        sm64::sm64_er_cling(id, mode, start as i32);
        sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]);
        sm64::sm64_set_mario_velocity(id, 0.0, 0.0, 0.0);
        if let Some(f) = yaw {
            sm64::sm64_set_mario_faceangle(id, f);
        }
    });
}

/// Off it with `vel` (Elden Ring, m/s): a flying kick along it, or a fall (`kick` false).
fn let_go(id: i32, vel: Vec3, kick: bool) {
    let vel = vel.clamp_length_max(LET_GO_MAX);
    let k = crate::scale() * 30.0;
    let sm = Vec3::new(-vel.x, vel.y, vel.z) / k;
    let flat = sm.with_y(0.0);
    let face = (flat.length() > 0.5).then(|| flat.x.atan2(flat.z));
    worker::call("colossus let go", move |_| unsafe {
        sm64::sm64_set_mario_action(id, if kick { ACT_ER_SWING_KICK } else { ACT_FREEFALL });
        if let Some(f) = face {
            sm64::sm64_set_mario_faceangle(id, f);
        }
        sm64::sm64_set_mario_forward_velocity(id, flat.length());
        sm64::sm64_set_mario_velocity(id, sm.x, sm.y, sm.z);
    });
}

fn sound(id: i32, volume: f32) {
    worker::call("colossus sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

/// The hook caught something with R2 held (hook.rs): the rope, from `at` on the map or from the
/// highest steady spot in reach on colossus `mob` (its head, its back...: a hand flails). `feet`,
/// `vel`: Mario's (Elden Ring). False: no rope (nothing high enough, too far): the hook does what it
/// does.
pub fn start_rope(at: Option<Vec3>, mob: Option<&FieldInsHandle>, feet: Vec3, vel: Vec3, air: bool, origin: [f32; 3]) -> bool {
    let hand = feet + Vec3::Y * HANG_DROP;
    let longest = rope_max(hook_reach(feet, origin));
    let pivot = match (at, mob) {
        (_, Some(h)) => {
            let Some(t) = titan(h) else { return false };
            let fits = |s: &&Spot| t.in_body(s) && s.at.y > feet.y + ROPE_ABOVE && (s.at - hand).length() <= longest;
            let highest = |a: &&Spot, b: &&Spot| a.at.y.total_cmp(&b.at.y);
            let best = t.spots.iter().filter(|s| s.part.steady()).filter(fits).max_by(highest).or_else(|| t.spots.iter().filter(fits).max_by(highest));
            match best {
                Some(s) => Pivot::Spot(*h, s.id),
                None => return false,
            }
        }
        (Some(p), None) if p.y > feet.y + ROPE_ABOVE && (p - hand).length() <= longest => Pivot::Map(p),
        _ => return false,
    };
    let Some(q) = pivot_at(pivot) else { return false };
    let len = (q - hand).length().clamp(ROPE_MIN, longest);
    // (from his feet: his run goes on into the swing, a little hop off the ground; no yank in
    // towards it, that swung him into its body)
    let vel = if air { vel } else { vel + Vec3::Y * 4.0 };
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !matches!(s.phase, Phase::Idle) {
        return false;
    }
    s.phase = Phase::Rope { pivot, len, hand, vel, since: Instant::now(), at: q, most: longest, climb: false };
    log(format!("colossus: on the rope ({}, {len:.1} m)", if mob.is_some() { "a colossus" } else { "the map" }));
    true
}

/// His buttons (held now) and the stick's way (Elden Ring, level, length 0-1).
pub struct Input {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub rt: bool,
    pub lt: bool,
    pub steer: Vec3,
}

/// Mario now (Elden Ring): his feet, SM64 action, whether he's dead, whether the game is paused for
/// him (a menu, the first-person view: he holds on, nothing's read)
pub struct Mario {
    pub feet: Vec3,
    pub action: u32,
    pub dead: bool,
    pub paused: bool,
}

/// What the tick did: his buttons were taken (nobody else gets them), the hook must let go.
#[derive(Default)]
pub struct Out {
    pub consumed: bool,
    pub drop_hook: bool,
}

/// Game thread, every SM64 tick before SM64's: grabs on, hangs, leaps, strikes, swings.
/// `allowed`: he may (not dead, not big, not on Yoshi or a captured enemy, not holding one).
#[allow(clippy::too_many_arguments)]
pub fn update(inp: &Input, m: &Mario, allowed: bool, id: i32, origin: [f32; 3], combat: &mut Combat, player: &ChrIns, tick: u32, ray: impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Out {
    const DT: f32 = 1.0 / 30.0;
    let mut out = Out::default();
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let pressed = [inp.a && !s.prev[0], inp.b && !s.prev[1], inp.x && !s.prev[2], inp.rt && !s.prev[3]];
    let x_up = !inp.x && s.prev[2];
    s.prev = [inp.a, inp.b, inp.x, inp.rt];
    let air = m.action & ACT_FLAG_AIR != 0;
    s.id = id;
    // (a menu: he holds on as he is, nothing's read)
    if m.paused && !m.dead {
        s.prev = [false; 4];
        return Out { consumed: !matches!(s.phase, Phase::Idle), drop_hook: false };
    }
    if !allowed {
        if !matches!(s.phase, Phase::Idle) {
            s.phase = Phase::Idle;
            s.hud = None;
            drop(s);
            log("colossus: let go (not allowed now)");
            crate::lakitu::set_colossus(None);
            if !m.dead {
                let_go(id, Vec3::ZERO, false);
            }
            return out;
        }
        s.hud = None;
        crate::lakitu::set_colossus(None);
        return out;
    }
    let phase = s.phase;
    match phase {
        Phase::Idle => {
            s.grip = (s.grip + GRIP_TOP * DT).min(1.0);
            s.chain = 0;
            let waiting = s.fall_wait.is_some_and(|t| t.elapsed().as_secs_f32() < FALL_WAIT);
            // in the air by a colossus: A grabs on; the hook's zip there (R2 let go) catches on
            let zipping = m.action == ACT_ER_ZIP && !inp.rt;
            if (air && pressed[0] || zipping) && !waiting {
                let chest = m.feet + Vec3::Y * 0.8;
                let reach = if zipping { GRAB_REACH * 1.3 } else { GRAB_REACH };
                let best = titans_near(m.feet, origin)
                    .into_iter()
                    .flat_map(|t| t.spots.iter().filter(|sp| holdable(&t, sp)).map(|sp| ((sp.at - chest).length(), t.mob, *sp)).collect::<Vec<_>>())
                    .filter(|(d, ..)| *d <= reach)
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, mob, spot)) = best {
                    s.last_spots = (0, None, Vec::new());
                    s.phase = Phase::Cling { mob, spot: spot.id, since: Instant::now(), charge: None, strike_until: None, last_tap: None };
                    out.consumed = true;
                    out.drop_hook = zipping;
                    crate::impact::hook_latch(spot.at);
                    drop(s);
                    sound(SOUND_GRAB, 0.5);
                    log(format!("colossus: grabbed on ({:?})", spot.part));
                    return out;
                }
            }
            s.hud = None;
            crate::lakitu::set_colossus(None);
        }
        Phase::Cling { mob, spot, since, charge, strike_until, last_tap } => {
            out.consumed = true;
            let Some(t) = titan(&mob) else {
                s.phase = Phase::Idle;
                drop(s);
                let_go(id, Vec3::ZERO, false);
                log("colossus: it's gone");
                return out;
            };
            // (its spot gone from the reading: the nearest one instead)
            let Some(sp) = t.spot(spot).or_else(|| t.spots.iter().copied().filter(|x| holdable(&t, x)).min_by(|a, b| (a.at - m.feet).length().total_cmp(&(b.at - m.feet).length()))) else {
                s.phase = Phase::Idle;
                drop(s);
                let_go(id, Vec3::ZERO, false);
                return out;
            };
            let spot = sp.id;
            // (a blow on him, or SM64 took him elsewhere: shaken off)
            if since.elapsed().as_secs_f32() > 0.2 && m.action != ACT_ER_CLING {
                s.phase = Phase::Idle;
                s.fall_wait = Some(Instant::now());
                log("colossus: shaken off");
                crate::lakitu::set_colossus(None);
                return Out::default();
            }
            // its speed there: it thrashes, his grip goes
            let speed = match &s.last_spots {
                (k, Some(at), list) if *k == t.key => {
                    let dt = at.elapsed().as_secs_f32().max(DT * 0.5);
                    list.iter().find(|(i, _)| *i == spot).map_or(0.0, |(_, p)| (sp.at - *p).length() / dt)
                }
                _ => 0.0,
            };
            s.last_spots = (t.key, Some(Instant::now()), t.spots.iter().map(|x| (x.id, x.at)).collect());
            let shake = (speed - SHAKE_FROM).max(0.0);
            let top = sp.part.top();
            let drain = if top { if shake > 1.0 { GRIP_SHAKE * 0.4 * shake } else { -GRIP_TOP } } else { GRIP_HANG + GRIP_SHAKE * shake };
            s.grip = (s.grip - drain * DT).clamp(0.0, 1.0);
            // the stars on it (picked the first time)
            let key = t.key;
            if !s.sigils.iter().any(|g| g.key == key) {
                let stars = pick_sigils(&t);
                log(format!("colossus: {} weak points glow on it", stars.len()));
                s.sigils.push(Sigils { key, stars, done_at: None });
            }
            if let Some(g) = s.sigils.iter_mut().find(|g| g.key == key) {
                if g.done_at.is_some_and(|d| d.elapsed().as_secs_f32() > SIGILS_BACK) {
                    g.stars = pick_sigils(&t);
                    g.done_at = None;
                }
            }
            let on_star = s.sigils.iter().find(|g| g.key == key).is_some_and(|g| g.stars.iter().any(|(i, n)| *i == spot && *n > 0));
            // out of grip: he falls
            if s.grip <= 0.0 {
                s.phase = Phase::Idle;
                s.fall_wait = Some(Instant::now());
                drop(s);
                let_go(id, (sp.at - t.feet).with_y(0.0).normalize_or(Vec3::Z) * 2.0, false);
                log("colossus: out of grip, he falls");
                crate::lakitu::set_colossus(None);
                return out;
            }
            // B: lets go
            if pressed[1] {
                s.phase = Phase::Idle;
                s.fall_wait = Some(Instant::now() - std::time::Duration::from_secs_f32(FALL_WAIT * 0.6));
                drop(s);
                let_go(id, t.out(sp.at, m.feet) * 3.0 + Vec3::Y * 3.0, false);
                crate::lakitu::set_colossus(None);
                return out;
            }
            let next = pick_leap(&t, &sp, inp.steer, &s);
            // L2 held: down the limb, sliding (to the nearest spot below, along it)
            if inp.lt && charge.is_none() {
                if let Some(down) = pick_slide(&t, &sp, None) {
                    s.phase = Phase::Slide { mob, from: sp.id, to: down.id, along: 0.0, speed: SLIDE_START, since: Instant::now() };
                    return out;
                }
            }
            // R2: the rope, from up there (its speed: none, hanging still)
            if pressed[3] && charge.is_none() {
                let (feet, _) = hold(&t, &sp, m.feet);
                // (off his hold for the rope; nothing to hang from up there: back on it)
                s.phase = Phase::Idle;
                drop(s);
                if !rope_from(&t, feet, Vec3::ZERO, origin, &ray) {
                    STATE.lock().unwrap_or_else(|e| e.into_inner()).phase = phase;
                    sound(SOUND_BUZZ, 0.3);
                }
                return out;
            }
            // A: the leap to the next spot
            if pressed[0] && charge.is_none() {
                match next {
                    Some(n) => {
                        let (from, _) = hold(&t, &sp, m.feet);
                        let d = (n.at - sp.at).length();
                        s.phase = Phase::Leap { mob, spot: n.id, from, since: Instant::now(), dur: (d / 14.0).clamp(0.28, 0.55), arc: 0.6 + d * 0.18 };
                        crate::impact::dash_point(from, true);
                        return out;
                    }
                    None => {
                        drop(s);
                        sound(SOUND_BUZZ, 0.3);
                        return out;
                    }
                }
            }
            // X: on a star, held, the star strike's charge; let go, it lands; elsewhere a blow
            let mut charge = charge;
            let mut strike_until = strike_until;
            let mut last_tap = last_tap;
            let mut blow: Option<(Attack, u32)> = None;
            if on_star && pressed[2] {
                charge = Some(Instant::now());
            }
            if let Some(c) = charge {
                let held = c.elapsed().as_secs_f32();
                if x_up || !inp.x {
                    charge = None;
                    blow = Some(if held >= STRIKE_FULL { (Attack::Sigil, 2) } else if held >= STRIKE_HALF { (Attack::Sigil, 1) } else { (Attack::SigilTap, 0) });
                }
            } else if pressed[2] && last_tap.is_none_or(|l| l.elapsed().as_secs_f32() >= TAP_EVERY) {
                blow = Some((Attack::SigilTap, 0));
                last_tap = Some(Instant::now());
            }
            if blow.is_some() {
                strike_until = Some(Instant::now() + std::time::Duration::from_secs_f32(0.3));
            }
            let mode = if charge.is_some() {
                CHARGE
            } else if strike_until.is_some_and(|u| Instant::now() < u) {
                STRIKE
            } else if top {
                TOP
            } else {
                HANG
            };
            s.phase = Phase::Cling { mob, spot, since, charge, strike_until, last_tap };
            let (feet, face) = hold(&t, &sp, m.feet);
            let charged = charge.map_or(0.0, |c| (c.elapsed().as_secs_f32() / STRIKE_FULL).min(1.0));
            s.hud = Some(Hud { grip: Some(s.grip), shaking: shake > 1.0, charge: charged, stars: stars_of(&s, &t), next: next.map(|n| n.at), mario: feet });
            crate::lakitu::set_colossus(Some((t.feet, t.height, t.radius, t.head())));
            drop(s);
            place(id, origin, feet, face, mode, true);
            if let Some((attack, hits)) = blow {
                strike(&t, &sp, attack, hits, origin, combat, player, tick);
            }
        }
        Phase::Leap { mob, spot, from, since, dur, arc } => {
            out.consumed = true;
            let Some((t, sp)) = titan(&mob).and_then(|t| t.spot(spot).map(|sp| (t, sp))) else {
                s.phase = Phase::Idle;
                drop(s);
                let_go(id, Vec3::Y * 2.0, false);
                return out;
            };
            // (a blow on him mid-leap: off)
            if since.elapsed().as_secs_f32() > 0.2 && m.action != ACT_ER_CLING {
                s.phase = Phase::Idle;
                s.fall_wait = Some(Instant::now());
                crate::lakitu::set_colossus(None);
                return Out::default();
            }
            let (to, face) = hold(&t, &sp, m.feet);
            let k = (since.elapsed().as_secs_f32() / dur).min(1.0);
            let e = k * k * (3.0 - 2.0 * k);
            let at = from.lerp(to, e) + Vec3::Y * arc * 4.0 * k * (1.0 - k);
            crate::impact::dash_point(at, false);
            crate::lakitu::set_colossus(Some((t.feet, t.height, t.radius, t.head())));
            if k >= 1.0 {
                s.chain += 1;
                s.last_spots = (0, None, Vec::new());
                let chain = s.chain;
                s.phase = Phase::Cling { mob, spot, since: Instant::now(), charge: None, strike_until: None, last_tap: None };
                drop(s);
                crate::impact::hook_latch(sp.at);
                crate::lakitu::punch(0.25);
                sound(SOUND_GRAB, 0.45);
                if sp.part == Part::Head {
                    crate::style::callout("SUMMIT!", Some(sp.at + Vec3::Y * 0.8), [1.0, 0.85, 0.35], true, 4.0);
                } else if chain == 3 {
                    crate::style::callout("CLIMB!", Some(sp.at), [1.0, 0.85, 0.35], false, 2.0);
                }
                place(id, origin, to, face, if sp.part.top() { TOP } else { HANG }, true);
            } else {
                let dir = (to - from).with_y(0.0);
                drop(s);
                place(id, origin, at, if dir.length() > 0.1 { dir } else { face }, LEAP, true);
            }
        }
        Phase::Slide { mob, from, to, along, speed, since } => {
            out.consumed = true;
            let Some(t) = titan(&mob) else {
                s.phase = Phase::Idle;
                drop(s);
                let_go(id, Vec3::ZERO, false);
                return out;
            };
            let (Some(a), Some(b)) = (t.spot(from), t.spot(to)) else {
                s.phase = Phase::Idle;
                drop(s);
                let_go(id, Vec3::ZERO, false);
                return out;
            };
            // (a blow on him: off)
            if since.elapsed().as_secs_f32() > 0.2 && m.action != ACT_ER_CLING {
                s.phase = Phase::Idle;
                s.fall_wait = Some(Instant::now());
                crate::lakitu::set_colossus(None);
                return Out::default();
            }
            let (fa, _) = hold(&t, &a, m.feet);
            let (fb, face_b) = hold(&t, &b, m.feet);
            let seg = fb - fa;
            let len = seg.length().max(0.3);
            let dir = seg / len;
            // (faster the steeper it goes down)
            let steep = (-dir.y).clamp(0.25, 1.0);
            let speed = (speed + SLIDE_PULL * steep * DT).min(SLIDE_FASTEST);
            let along = along + speed * DT / len;
            let at = fa.lerp(fb, along.min(1.0));
            let vel = dir * speed;
            crate::impact::dash_point(at, false);
            crate::lakitu::set_colossus(Some((t.feet, t.height, t.radius, t.head())));
            // A: off with all its speed and a jump; R2: the rope, the slide's speed into the swing
            if pressed[0] {
                s.phase = Phase::Idle;
                drop(s);
                log(format!("colossus: off the slide at {speed:.1} m/s, a jump"));
                crate::impact::dash_point(at, true);
                let_go(id, vel + Vec3::Y * JUMP_OFF, true);
                return out;
            }
            if pressed[3] {
                s.phase = Phase::Idle;
                drop(s);
                if !rope_from(&t, at, vel, origin, &ray) {
                    let_go(id, vel, true);
                }
                return out;
            }
            if along >= 1.0 {
                // on down the limb while L2 stays held, the speed kept; else he holds on there; off
                // its end, he's flung
                match pick_slide(&t, &b, Some(&a)).filter(|_| inp.lt) {
                    Some(c) => s.phase = Phase::Slide { mob, from: b.id, to: c.id, along: 0.0, speed, since },
                    None if inp.lt && speed > 6.0 => {
                        s.phase = Phase::Idle;
                        s.fall_wait = Some(Instant::now() - std::time::Duration::from_secs_f32(FALL_WAIT * 0.5));
                        drop(s);
                        log(format!("colossus: flung off the limb's end at {speed:.1} m/s"));
                        let_go(id, vel + Vec3::Y * 3.0, true);
                        return out;
                    }
                    None => {
                        s.last_spots = (0, None, Vec::new());
                        s.phase = Phase::Cling { mob, spot: b.id, since: Instant::now(), charge: None, strike_until: None, last_tap: None };
                        drop(s);
                        crate::impact::hook_latch(b.at);
                        place(id, origin, fb, face_b, if b.part.top() { TOP } else { HANG }, true);
                        return out;
                    }
                }
            } else {
                s.phase = Phase::Slide { mob, from, to, along, speed, since };
            }
            s.hud = Some(Hud { grip: Some(s.grip), shaking: false, charge: 0.0, stars: stars_of(&s, &t), next: None, mario: at });
            drop(s);
            place(id, origin, at, dir.with_y(0.0), SLIDE, true);
            sound(SOUND_SLIDE, 0.2 + 0.2 * speed / SLIDE_FASTEST);
        }
        Phase::Rope { pivot, len, hand, vel, since, at, most, climb } => {
            out.consumed = true;
            // where it hangs from: a colossus's spot moves with it (eased: its animation's jolts
            // shook him about), one dropped from the reading a moment keeps its last place
            let t_now = if let Pivot::Spot(h, _) = pivot { titan(&h) } else { None };
            let target = match pivot {
                Pivot::Map(p) => Some(p),
                Pivot::Spot(_, sid) => t_now.as_ref().map(|t| t.spot(sid).map_or(at, |x| x.at)),
            };
            let Some(target) = target else {
                s.phase = Phase::Idle;
                drop(s);
                let_go(id, vel, true);
                log("colossus: the rope's colossus is gone");
                return out;
            };
            let q = at.lerp(target, PIVOT_EASE);
            // (a blow on him: off the rope)
            if since.elapsed().as_secs_f32() > 0.2 && m.action != ACT_ER_CLING {
                s.phase = Phase::Idle;
                return Out { consumed: false, drop_hook: true };
            }
            let r = (hand - q).normalize_or(Vec3::NEG_Y);
            let pump = inp.steer - r * inp.steer.dot(r);
            let mut vel = vel + (Vec3::NEG_Y * GRAVITY + pump * PUMP) * DT;
            let mut len = len;
            let climb = climb || !inp.a;
            let climbing = climb && inp.a;
            // A held: up the rope (shorter, the swing faster for it, a skater pulling her arms
            // in); B: down it
            if climbing && len > ROPE_MIN {
                let shorter = (len - CLIMB * DT).max(ROPE_MIN);
                vel *= (len / shorter).min(1.04);
                len = shorter;
            } else if inp.b && len < most {
                len = (len + CLIMB * DT).min(most);
            }
            let mut next = hand + vel * DT;
            let d = next - q;
            if d.length() > len {
                next = q + d.normalize() * len;
            }
            // (pushed out of the body or up off the ground: that push isn't speed, only the part of
            // his speed into it goes; taken from the speed as it was, not the pushed place: a curb
            // or a colossus turning into him flung him off)
            let mut push = Vec3::ZERO;
            // round a colossus, never through it: off its body by a little (the rope let out if
            // that takes him past its length, rather than the two fighting)
            if let Some(t) = &t_now {
                if next.y > t.feet.y - 0.5 && next.y < t.feet.y + t.height {
                    let flat = (next - t.feet).with_y(0.0);
                    let least = t.radius + BODY_GAP;
                    if flat.length() < least {
                        let o = flat.try_normalize().unwrap_or_else(|| (hand - t.feet).with_y(0.0).normalize_or(-t.fwd));
                        let out_to = Vec3::new(t.feet.x + o.x * least, next.y, t.feet.z + o.z * least);
                        push += out_to - next;
                        next = out_to;
                        len = len.max((next - q).length().min(most));
                    }
                }
            }
            // a wall at his middle: stopped against it, a little bounce back
            let mut bumped = false;
            if let Some(w) = ray(hand - Vec3::Y * 0.6, next - Vec3::Y * 0.6) {
                let back = (hand - next).with_y(0.0).normalize_or_zero() * 0.3;
                next = Vec3::new(w.x + back.x, next.y, w.z + back.z);
                vel = Vec3::new(-vel.x * 0.25, vel.y, -vel.z * 0.25);
                bumped = true;
                crate::impact::thud_small(w);
            }
            // the ground under him: he runs along it, round and round, the rope taken up so he
            // never sinks into it (A lifts him off it)
            let from = Vec3::new(next.x, hand.y.max(next.y) + 0.3, next.z);
            let ground = ray(from, next - Vec3::Y * (HANG_DROP + 0.05));
            if let Some(g) = ground {
                let up = g.y + HANG_DROP + 0.02 - next.y;
                push.y += up;
                next.y += up;
                len = len.min((next - q).length()).max(ROPE_MIN);
            }
            let mut vel = if bumped { vel } else { (next - push - hand) / DT * 0.998 };
            if let Some(n) = push.try_normalize() {
                let into = vel.dot(n);
                if into < 0.0 {
                    vel -= n * into;
                }
            }
            if ground.is_some() {
                vel = Vec3::new(vel.x * 0.97, vel.y.max(0.0), vel.z * 0.97);
            }
            let vel = vel.clamp_length_max(LET_GO_MAX);
            // all the way up to a colossus's spot: A grabs on to it
            if let (Some(t), Pivot::Spot(_, sid)) = (&t_now, pivot) {
                if climbing && ground.is_none() && len <= ROPE_MIN + 0.05 {
                    if let Some(sp) = t.spot(sid).filter(|x| holdable(t, x)) {
                        s.last_spots = (0, None, Vec::new());
                        s.phase = Phase::Cling { mob: t.mob, spot: sp.id, since: Instant::now(), charge: None, strike_until: None, last_tap: None };
                        crate::impact::hook_latch(sp.at);
                        drop(s);
                        sound(SOUND_GRAB, 0.5);
                        log(format!("colossus: up the rope, grabbed on ({:?})", sp.part));
                        return Out { consumed: true, drop_hook: true };
                    }
                }
            }
            // R2 let go: off along the swing, its speed kept and a little more, up: the catapult
            let off = !inp.rt || since.elapsed().as_secs_f32() > ROPE_TIME;
            if off {
                s.phase = Phase::Idle;
                s.fall_wait = None;
                drop(s);
                let speed = vel.length();
                let v = vel * LET_GO_BOOST + Vec3::Y * (LET_GO_UP + speed * LET_GO_UP_SHARE);
                log(format!("colossus: off the rope at {:.1} m/s ({len:.1} m of rope{})", v.length(), if ground.is_some() { ", from the ground" } else { "" }));
                if v.length() > 12.0 {
                    crate::impact::dash_point(next - Vec3::Y * HANG_DROP, true);
                }
                let_go(id, v, true);
                return Out { consumed: true, drop_hook: true };
            }
            s.phase = Phase::Rope { pivot, len, hand: next, vel, since, at: q, most, climb };
            if vel.length() > 10.0 {
                crate::impact::dash_point(next - Vec3::Y * HANG_DROP, false);
            }
            let cam = t_now.as_ref().map(|t| (t.feet, t.height, t.radius, t.head()));
            crate::lakitu::set_colossus(cam);
            s.hud = None;
            drop(s);
            place(id, origin, next - Vec3::Y * HANG_DROP, vel.with_y(0.0), SWING, true);
        }
    }
    out
}

/// A spot he can hold on to now: one on top, or high enough off the ground to hang from.
fn holdable(t: &Titan, s: &Spot) -> bool {
    s.part.top() || s.at.y >= t.feet.y + HANG_DROP + 0.2
}

/// The spot a slide from `from` goes down to: the nearest below it, onward from `came` (not back up
/// to it), in reach.
fn pick_slide(t: &Titan, from: &Spot, came: Option<&Spot>) -> Option<Spot> {
    let back = came.map(|c| (from.at - c.at).normalize_or_zero());
    t.spots
        .iter()
        .filter(|sp| sp.id != from.id && came.is_none_or(|c| c.id != sp.id))
        .filter_map(|sp| {
            let d = sp.at - from.at;
            let dist = d.length();
            if d.y > -0.4 || !(0.6..=LEAP_REACH).contains(&dist) {
                return None;
            }
            // (along the way it came, down the same limb)
            let on = back.map_or(0.0, |b| -d.normalize_or_zero().dot(b) * 1.5);
            Some((dist + on, *sp))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, sp)| sp)
}

/// The rope thrown from on a colossus (his feet at `feet`, his speed `vel`: a slide's): to its
/// highest spot in reach above him, else the map where the camera looks. False: nothing to hang
/// from.
fn rope_from(t: &Titan, feet: Vec3, vel: Vec3, origin: [f32; 3], ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> bool {
    if start_rope(None, Some(&t.mob), feet, vel, true, origin) {
        return true;
    }
    let look = crate::lakitu::forward().filter(|v| v.is_finite() && v.length() > 0.5).map(|v| v.normalize()).unwrap_or(Vec3::Y);
    let from = feet + Vec3::Y * 1.0;
    let reach = hook_reach(feet, origin);
    ray(from, from + look * reach).is_some_and(|p| start_rope(Some(p), None, feet, vel, true, origin))
}

/// The weak points on a colossus: its head (or its highest spot), and its back, a shoulder, a hip
/// or its tail.
fn pick_sigils(t: &Titan) -> Vec<(u32, u32)> {
    let mut stars = Vec::new();
    let head = t.spots.iter().filter(|s| s.part == Part::Head).chain(t.spots.iter()).max_by(|a, b| (a.part == Part::Head, a.at.y).partial_cmp(&(b.part == Part::Head, b.at.y)).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(h) = head {
        stars.push((h.id, SIGIL_HITS));
    }
    let others: Vec<&Spot> = t.spots.iter().filter(|s| matches!(s.part, Part::Back | Part::Shoulder | Part::Hip | Part::Tail | Part::Side) && !stars.iter().any(|(i, _)| *i == s.id)).collect();
    if !others.is_empty() {
        let k = (t.key as usize).wrapping_mul(2_654_435_761) % others.len();
        stars.push((others[k].id, SIGIL_HITS));
    }
    stars
}

fn stars_of(s: &State, t: &Titan) -> Vec<(Vec3, Vec3, f32)> {
    s.sigils
        .iter()
        .find(|g| g.key == t.key)
        .map(|g| g.stars.iter().filter(|(_, n)| *n > 0).filter_map(|(i, n)| t.spot(*i).map(|sp| (sp.at, t.out(sp.at, sp.at + t.fwd), *n as f32 / SIGIL_HITS as f32))).collect())
        .unwrap_or_default()
}

/// The spot a leap from `from` goes to: the stick's way (else up, and the stars), in reach.
fn pick_leap(t: &Titan, from: &Spot, steer: Vec3, s: &State) -> Option<Spot> {
    let stars: Vec<u32> = s.sigils.iter().find(|g| g.key == t.key).map(|g| g.stars.iter().filter(|(_, n)| *n > 0).map(|(i, _)| *i).collect()).unwrap_or_default();
    t.spots
        .iter()
        .filter(|sp| sp.id != from.id && holdable(t, sp))
        .filter_map(|sp| {
            let d = sp.at - from.at;
            let dist = d.length();
            if !(0.8..=LEAP_REACH).contains(&dist) {
                return None;
            }
            let way = if steer.length() > 0.2 { -d.with_y(0.0).normalize_or_zero().dot(steer.normalize()) * 2.5 } else { -d.y.max(0.0) * 0.4 };
            let star = if stars.contains(&sp.id) { -0.5 } else { 0.0 };
            Some((dist * 0.15 + way + star, *sp))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, sp)| sp)
}

/// A blow on the spot he holds: the star strike (`hits` struck off the star there) or a tap.
#[allow(clippy::too_many_arguments)]
fn strike(t: &Titan, sp: &Spot, attack: Attack, hits: u32, origin: [f32; 3], combat: &mut Combat, player: &ChrIns, tick: u32) {
    let near = combat::nearby(&HavokPosition(t.feet.x, t.feet.y, t.feet.z, 0.0), t.radius + 6.0, origin);
    if let Some(i) = near.iter().position(|x| x.key() == t.key) {
        combat.deal(player, &near, &[(i, attack, false)], tick);
    }
    if hits == 0 {
        crate::impact::thud_small(sp.at);
        crate::swing::add_stance(&t.mob, 3.0);
        return;
    }
    crate::impact::max_burst(sp.at);
    crate::swing::add_stance(&t.mob, 18.0 * hits as f32);
    crate::lakitu::shake(0.7 + 0.5 * hits as f32);
    if hits >= 2 {
        crate::style::slow_down(0.45);
    }
    sound(SOUND_STAR, 0.35);
    let done = {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(g) = s.sigils.iter_mut().find(|g| g.key == t.key) else { return };
        if let Some(star) = g.stars.iter_mut().find(|(i, n)| *i == sp.id && *n > 0) {
            star.1 = star.1.saturating_sub(hits);
        }
        let done = g.done_at.is_none() && g.stars.iter().all(|(_, n)| *n == 0);
        if done {
            g.done_at = Some(Instant::now());
        }
        done
    };
    log(format!("colossus: star strike ({hits}){}", if done { ", every star struck out" } else { "" }));
    if done {
        crate::swing::add_stance(&t.mob, 100.0);
        crate::impact::max_quake(sp.at);
        crate::lakitu::shake(2.0);
        crate::style::slow_down(0.8);
        crate::style::callout("COLOSSAL!", Some(sp.at + Vec3::Y), [1.0, 0.8, 0.3], true, 10.0);
    } else {
        crate::style::callout("WEAK POINT!", Some(sp.at + Vec3::Y * 0.6), [1.0, 0.9, 0.5], false, 3.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bones_by_their_names() {
        assert_eq!(part_of("Head"), Some(Part::Head));
        assert_eq!(part_of("L_Hand"), Some(Part::Hand));
        assert_eq!(part_of("R_ForeArm"), Some(Part::Arm));
        assert_eq!(part_of("R_ForeArmTwist"), None);
        assert_eq!(part_of("L_UpperArm"), Some(Part::Shoulder));
        assert_eq!(part_of("Spine2"), Some(Part::Back));
        assert_eq!(part_of("L_Calf"), Some(Part::Knee));
        assert_eq!(part_of("R_Foot"), Some(Part::Foot));
        assert_eq!(part_of("Tail3"), Some(Part::Tail));
        assert_eq!(part_of("L_Finger01"), None);
        assert_eq!(part_of("Master"), None);
    }

    fn giant() -> Titan {
        let spots = vec![
            Spot { id: 1, part: Part::Foot, at: Vec3::new(1.0, 0.3, 0.0) },
            Spot { id: 2, part: Part::Knee, at: Vec3::new(1.0, 2.5, 0.0) },
            Spot { id: 3, part: Part::Hand, at: Vec3::new(2.0, 4.0, 0.5) },
            Spot { id: 4, part: Part::Back, at: Vec3::new(0.0, 6.0, -0.5) },
            Spot { id: 5, part: Part::Head, at: Vec3::new(0.0, 8.0, 0.5) },
        ];
        Titan { mob: unsafe { std::mem::zeroed() }, key: 7, feet: Vec3::ZERO, radius: 1.5, height: 8.5, fwd: Vec3::Z, spots }
    }

    #[test]
    fn a_leap_goes_up_unless_the_stick_says_otherwise() {
        let t = giant();
        let s = State { phase: Phase::Idle, grip: 1.0, fall_wait: None, chain: 0, sigils: Vec::new(), prev: [false; 4], last_spots: (0, None, Vec::new()), hud: None, id: -1 };
        let knee = t.spot(2).unwrap();
        let up = pick_leap(&t, &knee, Vec3::ZERO, &s).unwrap();
        assert!(up.at.y > knee.at.y);
        // (the stick towards the hand's side)
        let side = pick_leap(&t, &knee, Vec3::X, &s).unwrap();
        assert_eq!(side.id, 3);
        // the head is a star
        let stars = pick_sigils(&t);
        assert_eq!(stars[0].0, 5);
        assert!(stars.len() == 2 && stars[1].0 == 4);
    }

    #[test]
    fn hanging_he_faces_it_on_top_he_stands() {
        let t = giant();
        let (feet, face) = hold(&t, &t.spot(3).unwrap(), Vec3::new(5.0, 0.0, 0.0));
        assert!(feet.y < 4.0 && face.dot(Vec3::new(-1.0, 0.0, -0.25).normalize()) > 0.8);
        let (feet, face) = hold(&t, &t.spot(5).unwrap(), Vec3::ZERO);
        assert!(feet.y > 8.0 && face == Vec3::Z);
    }
}
