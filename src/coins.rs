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

/// Where the player is (set every frame): his map block, and the game's world origin shift (its
/// "chunk position": a physics position is the place in the block plus it; a load or a warp moves
/// it, the place in the block stays, so the bag stays where he fell).
#[derive(Clone, Copy)]
struct Here {
    block: BlockId,
    shift: Vec3,
}

static HERE: Mutex<Option<Here>> = Mutex::new(None);

/// A spot in the world: its place in its block (physics position less the world origin shift:
/// the same on every game, PlayerIns::block_position) and the block.
#[derive(Clone, Copy)]
struct Spot {
    pos: Vec3,
    block: BlockId,
}

impl Spot {
    fn of(chr: &ChrIns) -> Spot {
        let (p, c) = (chr.modules.physics.position, chr.chunk_position);
        Spot { pos: Vec3::new(p.0, p.1, p.2) - origin_shift(c.0, c.1, c.2), block: chr.block_id }
    }

    /// Where it is now in physics coordinates: in the player's block, or in another small tile of
    /// the overworld once the tiles' axes are known (tiles.rs).
    fn now(&self, here: &Here) -> Option<Vec3> {
        if self.block == here.block {
            return Some(self.pos + here.shift);
        }
        let axes = axes();
        crate::tiles::between(i32::from(self.block), self.pos, i32::from(here.block), &axes).map(|p| p + here.shift)
    }
}

/// The overworld tiles' axes as learnt so far (kept in the mod folder).
const AXES_FILE: &str = "er_mario_tiles.txt";
static AXES: Mutex<Option<crate::tiles::Axes>> = Mutex::new(None);

fn axes() -> crate::tiles::Axes {
    *AXES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(|| crate::tiles::parse(&std::fs::read_to_string(crate::paths::file(AXES_FILE)).unwrap_or_default()))
}

/// The player's block and place in it last frame, and when: a walk from one tile into the next
/// teaches the tiles' axes.
static LAST_PLACE: Mutex<Option<(BlockId, Vec3, Instant)>> = Mutex::new(None);

fn learn_tiles(spot: &Spot) {
    let prev = LAST_PLACE.lock().unwrap_or_else(|e| e.into_inner()).replace((spot.block, spot.pos, Instant::now()));
    let Some((block, pos, _)) = prev.filter(|(b, _, at)| *b != spot.block && at.elapsed().as_secs_f32() < 0.5) else { return };
    let Some((along_block, step)) = crate::tiles::learn(i32::from(block), pos, i32::from(spot.block), spot.pos) else { return };
    let mut a = axes();
    let slot = if along_block { &mut a.block } else { &mut a.region };
    if *slot == Some(step) {
        return;
    }
    if slot.is_some() {
        crate::log(format!("coins: a tile crossing disagrees with the tile axes kept ({step:?} now, {:?} before): the new one kept", *slot));
    }
    *slot = Some(step);
    *AXES.lock().unwrap_or_else(|e| e.into_inner()) = Some(a);
    let _ = std::fs::write(crate::paths::file(AXES_FILE), crate::tiles::format(&a));
    crate::log(format!("coins: tile axes calibrated (block step {:?}, region step {:?}), from {block} to {}", a.block, a.region, spot.block));
}

/// The game's world origin shift (a character's chunk position: none while a map loads).
fn origin_shift(x: f32, y: f32, z: f32) -> Vec3 {
    let s = Vec3::new(x, y, z);
    if s.is_finite() { s } else { Vec3::ZERO }
}

/// the last spot he stood on, alive: the bag goes there (a fall into the abyss leaves it at the edge)
static GROUND: Mutex<Option<Spot>> = Mutex::new(None);

/// Every frame: where the player is, and whether Mario stands on something.
pub fn here(chr: &ChrIns, grounded: bool) {
    // (the character's bag, once a second: a load can bring another character)
    {
        static CHECKED: Mutex<Option<Instant>> = Mutex::new(None);
        let mut checked = CHECKED.lock().unwrap_or_else(|e| e.into_inner());
        if checked.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
            *checked = Some(Instant::now());
            drop(checked);
            load_bag();
        }
    }
    let spot = Spot::of(chr);
    let c = chr.chunk_position;
    *HERE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Here { block: spot.block, shift: origin_shift(c.0, c.1, c.2) });
    // (a map still loading has no chunk position: no place to learn from)
    if c.0.is_finite() && c.1.is_finite() && c.2.is_finite() {
        learn_tiles(&spot);
    }
    if grounded {
        *GROUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(spot);
    }
}

/// Where the last spot he stood on, alive, is now (physics coordinates), if the game can say.
pub fn last_ground() -> Option<Vec3> {
    let here = (*HERE.lock().unwrap_or_else(|e| e.into_inner()))?;
    GROUND.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|g| g.now(&here))
}

/// The coins lost in the last death, in a bag where he fell, and how many.
struct Bag {
    spot: Spot,
    amount: u32,
}

static BAG: Mutex<Option<Bag>> = Mutex::new(None);
/// the character whose bag BAG holds (it's kept per character in the bag file)
static BAG_OWNER: Mutex<Option<String>> = Mutex::new(None);
const BAG_FILE: &str = "er_mario_bag.txt";

/// The current character's bag, loaded from the bag file when the character changes, and the
/// character, if there is one.
fn bag_lock() -> (std::sync::MutexGuard<'static, Option<Bag>>, Option<String>) {
    let bag = BAG.lock().unwrap_or_else(|e| e.into_inner());
    let owner = BAG_OWNER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    (bag, owner)
}

/// Game thread: the current character's bag in BAG (read from the bag file when the character
/// changes; the HUD's thread only looks at BAG).
fn load_bag() {
    let name = crate::stats::character();
    let mut bag = BAG.lock().unwrap_or_else(|e| e.into_inner());
    let mut owner = BAG_OWNER.lock().unwrap_or_else(|e| e.into_inner());
    if name.is_some() && *owner != name {
        let text = std::fs::read_to_string(crate::paths::file(BAG_FILE)).unwrap_or_default();
        *bag = name.as_deref().and_then(|n| bag_of(&text, n)).map(|(block, pos, amount)| Bag { spot: Spot { pos, block: BlockId::from(block) }, amount });
        if let Some(b) = bag.as_ref() {
            crate::log(format!("coins: {}'s bag of {} coins is still out there (block {})", name.as_deref().unwrap_or(""), b.amount, b.spot.block));
        }
        owner.clone_from(&name);
    }
}

/// Writes the current character's bag (or its absence) into the bag file.
fn save_bag(name: Option<&str>, bag: &Option<Bag>) {
    let Some(name) = name else { return };
    let path = crate::paths::file(BAG_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::write(path, with_bag(&text, name, bag.as_ref().map(|b| (i32::from(b.spot.block), b.spot.pos, b.amount))));
}

/// Mario died (the Tarnished `chr`): half the purse stays in a bag where he last stood, the one
/// from an earlier death is gone.
pub fn die(chr: &ChrIns) {
    let mut lost = 0;
    crate::stats::update(|s| {
        lost = s.coins / 2;
        s.coins -= lost;
    });
    load_bag();
    let (mut bag, name) = bag_lock();
    if let Some(old) = bag.take() {
        crate::log(format!("coins: the bag of {} coins from the last death is gone", old.amount));
    }
    let now = Spot::of(chr);
    // (the last ground, if it's in this part of the map and not far: else where he is)
    let ground = *GROUND.lock().unwrap_or_else(|e| e.into_inner());
    let axes = axes();
    let near = |g: &Spot| crate::tiles::between(i32::from(g.block), g.pos, i32::from(now.block), &axes).is_some_and(|p| p.distance(now.pos) < 150.0);
    let mut spot = ground.filter(near).unwrap_or(now);
    spot.pos += Vec3::Y * 0.05;
    if lost > 0 {
        *bag = Some(Bag { spot, amount: lost });
        crate::log(format!("coins: died, {lost} coins left in a bag (block {}, at {:.1?} in it)", spot.block, spot.pos));
    }
    save_bag(name.as_deref(), &bag);
}

/// One character's bag as a line of the bag file: name, block, place in it, coins.
fn format_bag(name: &str, block: i32, pos: Vec3, amount: u32) -> String {
    format!("{}\t{block}\t{}\t{}\t{}\t{amount}\n", name.replace(['\t', '\n', '\r'], " "), pos.x, pos.y, pos.z)
}

fn parse_bag(line: &str) -> Option<(String, i32, Vec3, u32)> {
    let f: Vec<&str> = line.trim_end_matches(['\n', '\r']).split('\t').collect();
    if f.len() != 6 {
        return None;
    }
    let n = |i: usize| f[i].trim().parse::<f32>().ok();
    Some((f[0].to_string(), f[1].trim().parse().ok()?, Vec3::new(n(2)?, n(3)?, n(4)?), f[5].trim().parse().ok()?))
}

/// The bag file's text with `name`'s bag set (or gone).
fn with_bag(text: &str, name: &str, bag: Option<(i32, Vec3, u32)>) -> String {
    let mut out: String = text.lines().filter(|l| parse_bag(l).is_some_and(|b| b.0 != name)).map(|l| format!("{l}\n")).collect();
    if let Some((block, pos, amount)) = bag {
        out += &format_bag(name, block, pos, amount);
    }
    out
}

/// `name`'s bag in the bag file's text.
fn bag_of(text: &str, name: &str) -> Option<(i32, Vec3, u32)> {
    text.lines().filter_map(parse_bag).find(|b| b.0 == name).map(|(_, block, pos, amount)| (block, pos, amount))
}

/// The bag on the ground: where and how many (the HUD draws it, the compass points to it).
pub fn bag() -> Option<(Vec3, u32)> {
    match bag_view()? {
        BagView::Here(pos, amount) => Some((pos, amount)),
        BagView::Elsewhere => None,
    }
}

/// The bag as the compass shows it: where it is now (physics coordinates), or only that it's out
/// there in another part of the map (another area, a dungeon, a tile the axes don't reach yet).
#[derive(Clone, Copy, Debug)]
pub enum BagView {
    Here(Vec3, u32),
    Elsewhere,
}

pub fn bag_view() -> Option<BagView> {
    let here = *HERE.lock().unwrap_or_else(|e| e.into_inner());
    let (bag, _) = bag_lock();
    let b = bag.as_ref()?;
    Some(match here.and_then(|h| b.spot.now(&h)) {
        Some(pos) => BagView::Here(pos, b.amount),
        None => BagView::Elsewhere,
    })
}

/// The bag for the co-op partner's compass: its block, its place in it (the same on both players'
/// games, physics positions aren't), its coins.
pub fn bag_chunk() -> Option<(i32, Vec3, u32)> {
    bag_lock().0.as_ref().map(|b| (i32::from(b.spot.block), b.spot.pos, b.amount))
}

/// Where a partner's bag (`bag_chunk` on their game) is in our physics coordinates, if it's in our
/// block.
pub fn place(block: i32, pos: Vec3) -> Option<Vec3> {
    let here = (*HERE.lock().unwrap_or_else(|e| e.into_inner()))?;
    Spot { pos, block: BlockId::from(block) }.now(&here)
}

/// Mario (feet at `mario`) touches the bag: the coins back in his purse. How many.
pub fn collect_bag(mario: Vec3) -> Option<u32> {
    let (pos, amount) = bag()?;
    let d = pos - mario;
    if Vec3::new(d.x, 0.0, d.z).length() > 1.4 || d.y < -1.0 || d.y > 2.0 {
        return None;
    }
    {
        let (mut bag, name) = bag_lock();
        *bag = None;
        save_bag(name.as_deref(), &bag);
    }
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
    // (the bag's and the ground's places in their block don't move)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bag_line_round_trips() {
        let line = format_bag("Randolph Carter", 0x3C2A2400, Vec3::new(12.5, -3.25, 100.0), 17);
        assert_eq!(parse_bag(&line), Some(("Randolph Carter".to_string(), 0x3C2A2400, Vec3::new(12.5, -3.25, 100.0), 17)));
        // (a tab in a name can't split its line)
        assert_eq!(parse_bag(&format_bag("a\tb", -1, Vec3::ZERO, 1)).map(|b| b.0), Some("a b".to_string()));
        assert_eq!(parse_bag("nonsense"), None);
    }

    #[test]
    fn the_file_keeps_one_bag_per_character() {
        let text = format!("{}{}", format_bag("A", 1, Vec3::ONE, 3), format_bag("B", 2, Vec3::ZERO, 4));
        let text = with_bag(&text, "A", Some((5, Vec3::X, 9)));
        assert_eq!(bag_of(&text, "A"), Some((5, Vec3::X, 9)));
        assert_eq!(bag_of(&text, "B"), Some((2, Vec3::ZERO, 4)));
        let text = with_bag(&text, "B", None);
        assert_eq!(bag_of(&text, "B"), None);
        assert_eq!(bag_of(&with_bag(&text, "C", Some((7, Vec3::Y, 1))), "C"), Some((7, Vec3::Y, 1)));
    }
}
