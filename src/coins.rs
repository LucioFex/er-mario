//! SM64's yellow coins, dropped by the enemies Mario defeats: they spin where the enemy fell, and
//! touching one heals a wedge and goes into his purse (the HUD's counter, kept per character:
//! stats.rs). They blink out after 20 s, like SM64's. The purse is what the big abilities cost
//! (`pay`: the capture, the Mega and Rock Mushrooms, the wing cap), and, like Elden Ring's runes,
//! half of it is lost when Mario dies, left in a bag where he fell: touch it to have it back (dying
//! again first loses it for good). In co-op every player sees and takes coins of their own: each
//! one's game drops them where enemies near them die (`watch`).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{BlockId, ChrIns};
use fromsoftware_shared::FromStatic;
use glam::Vec3;


/// what the abilities cost (coins), on top of their waits
pub const COST_RIDE: u32 = 8;
pub const COST_MEGA: u32 = 8;
pub const COST_ROCK: u32 = 5;
pub const COST_WING: u32 = 3;

const LIFETIME: f32 = 20.0;
const BLINK: f32 = 3.0;
/// Mario picks it up within this distance (m)
const REACH: f32 = 0.7;

struct Coin {
    pos: Vec3,
    born: Instant,
    /// the camera can see it (no map geometry in the way)
    visible: bool,
}

static COINS: Mutex<Vec<Coin>> = Mutex::new(Vec::new());

/// What was paid for didn't come about: the coins back.
pub fn refund(cost: u32, what: &str) {
    crate::stats::update(|s| s.coins += cost);
    crate::log(format!("coins: {cost} back, {what} didn't start"));
}

/// Pays `cost` coins out of the purse if it holds them (true), else a "NEED COINS" and no (false).
pub fn pay(cost: u32, what: &str) -> bool {
    // (looked at and taken in one go: the character can change in between, in a loading screen)
    let (mut paid, mut had) = (false, 0);
    crate::stats::update(|s| {
        had = s.coins;
        if s.coins >= cost {
            s.coins -= cost;
            paid = true;
        }
    });
    if !paid {
        // (a held button asks every frame: once a second is enough)
        static LAST: Mutex<Option<Instant>> = Mutex::new(None);
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_none_or(|t| t.elapsed().as_secs_f32() > 1.0) {
            *last = Some(Instant::now());
            crate::style::callout("NEED COINS", None, [1.0, 0.8, 0.3], false, 0.0);
            crate::log(format!("coins: {what} costs {cost}, the purse has {had}"));
        }
        return false;
    }
    crate::log(format!("coins: {what} cost {cost}"));
    true
}

/// Where the player is (set every frame): his map block, and how his physics position sits in the
/// map's chunk coordinates (those don't move when the game shifts the world's origin, a load or a
/// warp does, so the bag stays where he fell).
#[derive(Clone, Copy)]
struct Here {
    block: BlockId,
    origin: BlockId,
    /// chunk position minus physics position (None: the game gave no chunk position)
    off: Option<Vec3>,
}

static HERE: Mutex<Option<Here>> = Mutex::new(None);

/// A spot in the world: physics position (moved by `shift`, used if the game gave no chunk
/// position), chunk position and its block.
#[derive(Clone, Copy)]
struct Spot {
    pos: Vec3,
    chunk: Option<Vec3>,
    block: BlockId,
    origin: BlockId,
}

impl Spot {
    fn of(chr: &ChrIns) -> Spot {
        let (p, c) = (chr.modules.physics.position, chr.chunk_position);
        let chunk = Vec3::new(c.0, c.1, c.2);
        Spot { pos: Vec3::new(p.0, p.1, p.2), chunk: (chunk.length_squared() > 1e-6).then_some(chunk), block: chr.block_id, origin: chr.block_origin }
    }

    /// Where it is now in physics coordinates, if in the player's part of the map.
    fn now(&self, here: &Here) -> Option<Vec3> {
        match self.chunk {
            Some(c) => (self.origin == here.origin).then_some(here.off.map(|o| c - o)).flatten(),
            None => (self.block == here.block).then_some(self.pos),
        }
    }
}

/// the last spot he stood on, alive: the bag goes there (a fall into the abyss leaves it at the edge)
static GROUND: Mutex<Option<Spot>> = Mutex::new(None);

/// Every frame: where the player is, and whether Mario stands on something.
pub fn here(chr: &ChrIns, grounded: bool) {
    let spot = Spot::of(chr);
    let off = spot.chunk.map(|c| c - spot.pos);
    *HERE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Here { block: spot.block, origin: spot.origin, off });
    if grounded {
        *GROUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(spot);
    }
}

/// The coins lost in the last death, in a bag where he fell, and how many.
struct Bag {
    spot: Spot,
    amount: u32,
}

static BAG: Mutex<Option<Bag>> = Mutex::new(None);

/// Mario died (the Tarnished `chr`): half the purse stays in a bag where he last stood, the one
/// from an earlier death is gone.
pub fn die(chr: &ChrIns) {
    let mut lost = 0;
    crate::stats::update(|s| {
        lost = s.coins / 2;
        s.coins -= lost;
    });
    let mut bag = BAG.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = bag.take() {
        crate::log(format!("coins: the bag of {} coins from the last death is gone", old.amount));
    }
    let now = Spot::of(chr);
    // (the last ground, if it's in this part of the map and not far: else where he is)
    let ground = GROUND.lock().unwrap_or_else(|e| e.into_inner()).take();
    let near = |g: &Spot| match (g.chunk, now.chunk) {
        (Some(a), Some(b)) => g.origin == now.origin && a.distance(b) < 150.0,
        _ => g.block == now.block && g.pos.distance(now.pos) < 150.0,
    };
    let mut spot = ground.filter(near).unwrap_or(now);
    spot.pos += Vec3::Y * 0.05;
    spot.chunk = spot.chunk.map(|c| c + Vec3::Y * 0.05);
    if lost > 0 {
        *bag = Some(Bag { spot, amount: lost });
        crate::log(format!("coins: died, {lost} coins left in a bag ({})", if spot.chunk.is_some() { "map chunk position" } else { "no chunk position: physics position" }));
    }
}

/// The bag on the ground: where and how many (the HUD draws it, the compass points to it).
pub fn bag() -> Option<(Vec3, u32)> {
    let here = (*HERE.lock().unwrap_or_else(|e| e.into_inner()))?;
    BAG.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|b| Some((b.spot.now(&here)?, b.amount)))
}

/// Mario (feet at `mario`) touches the bag: the coins back in his purse. How many.
pub fn collect_bag(mario: Vec3) -> Option<u32> {
    let (pos, amount) = bag()?;
    let d = pos - mario;
    if Vec3::new(d.x, 0.0, d.z).length() > 1.4 || d.y < -1.0 || d.y > 2.0 {
        return None;
    }
    *BAG.lock().unwrap_or_else(|e| e.into_inner()) = None;
    crate::stats::update(|s| s.coins += amount);
    crate::log(format!("coins: the bag is back, {amount} coins"));
    Some(amount)
}

/// Co-op: the enemies near this player now (key, handle); those that were here before and have
/// died since drop a coin for this player, wherever their blow came from: where they fell.
pub fn watch(now: Vec<(u64, eldenring::cs::FieldInsHandle)>) -> Vec<Vec3> {
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let mut dead = Vec::new();
    if let Ok(wcm) = unsafe { eldenring::cs::WorldChrMan::instance() } {
        for (key, handle) in seen.iter().filter(|(k, _)| !now.iter().any(|(n, _)| n == k)) {
            let _ = key;
            if let Some(chr) = wcm.chr_ins_by_handle(handle).filter(|c| c.modules.data.hp <= 0) {
                let p = chr.modules.physics.position;
                dead.push(Vec3::new(p.0, p.1, p.2));
            }
        }
    }
    *seen = now;
    dead
}

/// A coin where an enemy fell (its feet, in the game's physics coordinates).
pub fn spawn(pos: Vec3) {
    COINS.lock().unwrap_or_else(|e| e.into_inner()).push(Coin { pos: pos + Vec3::Y * 0.05, born: Instant::now(), visible: true });
}

/// Within this distance (m) a coin flies to Mario like to a magnet, faster the closer it gets
pub const MAGNET: f32 = 3.5;

/// Moves `pos` toward Mario's middle (feet at `mario`) if he's within MAGNET, over `dt` seconds.
pub fn attract(pos: &mut Vec3, mario: Vec3, dt: f32) {
    let to = mario + Vec3::Y * 0.4 - *pos;
    let d = to.length();
    if d < MAGNET && d > 0.01 {
        let speed = 3.0 + (MAGNET - d) * 6.0;
        *pos += to / d * (speed * dt).min(d);
    }
}

/// Coins Mario (feet at `mario`) touches now; they're gone (as are expired ones).
pub fn collect(mario: Vec3) -> u32 {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let dt = LAST.lock().unwrap_or_else(|e| e.into_inner()).replace(Instant::now()).map_or(0.0, |t| t.elapsed().as_secs_f32().min(0.1));
    let mut coins = COINS.lock().unwrap_or_else(|e| e.into_inner());
    for c in coins.iter_mut() {
        attract(&mut c.pos, mario, dt);
    }
    let mut taken = 0;
    coins.retain(|c| {
        let d = c.pos - mario;
        let touched = Vec3::new(d.x, 0.0, d.z).length() < REACH && d.y > -0.8 && d.y < 1.7;
        taken += touched as u32;
        !touched && c.born.elapsed().as_secs_f32() < LIFETIME
    });
    taken
}

/// The game re-based its physics coordinates by `d`.
pub fn shift(d: Vec3) {
    // (the chunk positions don't move)
    if let Some(b) = BAG.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        b.spot.pos += d;
    }
    if let Some(g) = GROUND.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        g.pos += d;
    }
    for c in COINS.lock().unwrap_or_else(|e| e.into_inner()).iter_mut() {
        c.pos += d;
    }
}

static SEEN: Mutex<Vec<(u64, eldenring::cs::FieldInsHandle)>> = Mutex::new(Vec::new());

pub fn clear() {
    // (and the co-op watch starts over: nobody who died while Mario was down drops a coin)
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).clear();
    COINS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// Game thread: which coins the camera at `cam` can see; `blocked(from, to)` says whether map
/// geometry is between two points, `bodies` are Mario and the characters around him (feet,
/// radius, height: the coins are drawn over the game's picture, so anyone standing in front of
/// one has to hide it here). A coin shows if its middle or its top is in view.
pub fn update_visibility(cam: Vec3, bodies: &[(Vec3, f32, f32)], blocked: impl Fn(Vec3, Vec3) -> bool) {
    for c in COINS.lock().unwrap_or_else(|e| e.into_inner()).iter_mut() {
        c.visible = [0.32, 0.6].iter().any(|&h| {
            let p = c.pos + Vec3::Y * h;
            !bodies.iter().any(|&b| behind(cam, p, b)) && !blocked(cam, p)
        });
    }
}

/// Whether the body (an upright cylinder) stands between the camera and `p`.
fn behind(cam: Vec3, p: Vec3, (feet, radius, height): (Vec3, f32, f32)) -> bool {
    // (a coin he's standing in isn't behind him)
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    if flat(p - feet).length() < radius && p.y > feet.y - 0.2 && p.y < feet.y + height {
        return false;
    }
    // closest points of the line of sight and the cylinder's axis
    let (d1, d2, r) = (p - cam, Vec3::Y * height, cam - feet);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    if a < 1e-6 || e < 1e-6 {
        return false;
    }
    let (b, c) = (d1.dot(d2), d1.dot(r));
    let denom = a * e - b * b;
    let mut s = if denom > 1e-6 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let t = ((b * s + f) / e).clamp(0.0, 1.0);
    s = ((b * t - c) / a).clamp(0.0, 1.0);
    // (in front of the coin, not at it)
    s < 0.98 && ((cam + d1 * s) - (feet + d2 * t)).length() < radius
}

/// The coins to draw: position (bottom centre) and age in seconds (blinking ones left out).
pub fn visible() -> Vec<(Vec3, f32)> {
    COINS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|c| c.visible)
        .map(|c| (c.pos, c.born.elapsed().as_secs_f32()))
        .filter(|&(_, age)| age < LIFETIME && !(age > LIFETIME - BLINK && (age * 10.0) as u32 % 2 == 0))
        .collect()
}
