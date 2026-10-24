//! Worst cases of the mod's own parts. The averages in the perf log hide a hitch: one slow frame
//! in a few hundred. Each part times itself with `span`, and every 2 s the log names the parts
//! whose slowest run was long enough to feel.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub const FRAME: usize = 0;
pub const HUD: usize = 1;
pub const INPUT: usize = 2;
pub const POSE: usize = 3;
pub const POSE_LATE: usize = 4;
pub const MENU_MODEL: usize = 5;
pub const OVERLAY: usize = 6;
pub const LAKITU: usize = 7;
// inside the frame task
pub const MOVING: usize = 8;
pub const COLLISION: usize = 9;
pub const SURFACES: usize = 10;
pub const TARGETS: usize = 11;
pub const TICK: usize = 12;
// inside the HUD task
pub const MENU_WALK: usize = 13;
pub const CLASS_NAME: usize = 14;
// inside the collision query
pub const HAVOK_QUERY: usize = 15;
pub const TRIANGLES: usize = 16;
pub const FLOOR_PATCHES: usize = 17;
// inside reading Havok: the triangles of the bodies near Mario
pub const HAVOK_PICK: usize = 18;
// more of the frame task's parts (v4: where its 300-600 ms every 2 s go)
pub const LIGHT: usize = 19;
pub const CAMERA_UPDATE: usize = 20;
pub const STANCES: usize = 21;
pub const CARRY: usize = 22;
pub const STOPWATCH: usize = 23;
pub const BAT: usize = 24;
pub const BOSSRIDE: usize = 25;
pub const STATUS: usize = 26;
pub const COLOSSUS: usize = 27;
pub const MOVES: usize = 28;
pub const DODGE: usize = 29;
pub const FIREBALL: usize = 30;
pub const WEAPONS: usize = 31;
pub const GRAB: usize = 32;
pub const YOSHI: usize = 33;
// inside the input task
pub const INPUT_BOSSRIDE: usize = 34;
pub const INPUT_MENU: usize = 35;
// v4 Part 4: the kart's tick (karting.rs)
pub const KART: usize = 36;
// v4 Part 8: the walk of every character, once a tick (combat.rs's roster)
pub const ROSTER: usize = 37;
const N: usize = 38;
const NAMES: [&str; N] = [
    "frame task",
    "HUD task",
    "input task",
    "pose task",
    "late pose tasks",
    "menu model hook",
    "overlay",
    "camera",
    "moving objects",
    "collision query",
    "loading surfaces",
    "finding targets",
    "SM64 tick",
    "menu model walk",
    "class name search",
    "reading Havok",
    "turning triangles",
    "floor patches",
    "picking triangles",
    "light",
    "camera update",
    "stance watch",
    "carrying",
    "stopwatch",
    "flights (bat)",
    "boss ride",
    "statuses",
    "colossus",
    "moves",
    "dodge",
    "fireball",
    "weapons (parry, boomerang, kit, hook)",
    "grabs",
    "Yoshi and trample",
    "input: boss ride",
    "input: menus",
    "kart",
    "character roster",
];

/// per part: slowest run and all runs together since the last report (ns)
static WORST: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static TOTAL: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
/// A run this long is worth a line (ms): a quarter of a frame at 60 fps.
const SLOW_MS: f32 = 4.0;
/// So are many short runs that add up to this over the 2 s (ms): 5% of the time.
const BUSY_MS: f32 = 100.0;

pub struct Span(usize, Instant);

pub fn span(part: usize) -> Span {
    Span(part, Instant::now())
}

impl Drop for Span {
    fn drop(&mut self) {
        let ns = self.1.elapsed().as_nanos() as u64;
        WORST[self.0].fetch_max(ns, Ordering::Relaxed);
        TOTAL[self.0].fetch_add(ns, Ordering::Relaxed);
        NOW[self.0].fetch_add(ns, Ordering::Relaxed);
    }
}

/// What each part took since the frame task last started (ns).
static NOW: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
/// The parts that run by themselves, not inside another one: together they're all the mod does
/// on the game's threads in a frame.
const OWN: [usize; 8] = [FRAME, HUD, INPUT, POSE, POSE_LATE, MENU_MODEL, OVERLAY, LAKITU];
/// Debug: when the frame task last started, and since the last report every frame's length (ms)
/// with what the mod took in it (ms) and its biggest part.
static PACING: std::sync::Mutex<(Option<Instant>, Vec<(f32, f32, usize)>)> = std::sync::Mutex::new((None, Vec::new()));
/// A frame this much longer than the usual one is felt.
const LONG_FRAME: f32 = 1.4;
/// ...and the mod had a hand in it if it took this long (ms) of that frame.
const BUSY_FRAME_MS: f32 = 2.0;

/// Debug, at the start of every frame task: notes how long the frame before took.
pub fn frame_start() {
    let mut p = PACING.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    let took: Vec<f32> = NOW.iter().map(|n| n.swap(0, Ordering::Relaxed) as f32 / 1e6).collect();
    if let Some(last) = p.0.replace(now) {
        let frame = now.duration_since(last).as_secs_f32() * 1000.0;
        let biggest = OWN.into_iter().max_by(|a, b| took[*a].total_cmp(&took[*b])).unwrap_or(FRAME);
        p.1.push((frame, OWN.iter().map(|&i| took[i]).sum(), biggest));
    }
}

/// Debug: the frames since the last call: how many, the usual length, how many were long, and
/// how many of the long ones came after a busy frame task of ours. Is a hitch the mod's or not?
pub fn pacing() -> String {
    let frames = std::mem::take(&mut PACING.lock().unwrap_or_else(|e| e.into_inner()).1);
    if frames.len() < 10 {
        return String::new();
    }
    let mut lengths: Vec<f32> = frames.iter().map(|f| f.0).collect();
    lengths.sort_by(f32::total_cmp);
    let usual = lengths[lengths.len() / 2];
    let long: Vec<&(f32, f32, usize)> = frames.iter().filter(|f| f.0 > usual * LONG_FRAME).collect();
    let ours = long.iter().filter(|f| f.1 >= BUSY_FRAME_MS).count();
    let busy = frames.iter().filter(|f| f.1 >= BUSY_FRAME_MS).count();
    // each long frame: its length, what the mod took of it, and the mod's biggest part
    let each: Vec<String> = long.iter().take(8).map(|f| format!("{:.0} ms ({:.1} ours, {})", f.0, f.1, NAMES[f.2])).collect();
    format!(
        "{} frames, usual {usual:.1} ms, longest {:.1} ms, {} long (over {:.1} ms), {ours} of them after a busy frame task; busy frame tasks {busy}; mod per frame {:.2} ms{}{}",
        frames.len(),
        lengths[lengths.len() - 1],
        long.len(),
        usual * LONG_FRAME,
        frames.iter().map(|f| f.1).sum::<f32>() / frames.len() as f32,
        if each.is_empty() { "" } else { "; long: " },
        each.join(", ")
    )
}

/// Debug: every part's share since the last report, "frame task 71 ms (worst 4.9), ...", parts
/// that took next to nothing left out. Call before `report`, which starts the count over.
pub fn breakdown() -> String {
    let mut parts: Vec<(f32, f32, &str)> =
        (0..NAMES.len()).map(|i| (TOTAL[i].load(Ordering::Relaxed) as f32 / 1e6, WORST[i].load(Ordering::Relaxed) as f32 / 1e6, NAMES[i])).filter(|p| p.0 >= 1.0).collect();
    parts.sort_by(|a, b| b.0.total_cmp(&a.0));
    parts.iter().map(|(all, worst, name)| format!("{name} {all:.0} ms (worst {worst:.1})")).collect::<Vec<_>>().join(", ")
}

/// All the time each part took since the last `top` (ns): the long view, for `top`.
static LONG: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];

/// The slow parts since the last call, if any: "HUD task 12.3 ms (41 ms in all), ...; most time:
/// frame task 412 ms, moves 80 ms, ..." (the five parts that took the most in all).
pub fn report() -> Option<String> {
    let parts: Vec<(f32, f32, &str)> = (0..N)
        .map(|i| {
            let all = TOTAL[i].swap(0, Ordering::Relaxed);
            LONG[i].fetch_add(all, Ordering::Relaxed);
            (WORST[i].swap(0, Ordering::Relaxed) as f32 / 1e6, all as f32 / 1e6, NAMES[i])
        })
        .collect();
    let mut slow: Vec<&(f32, f32, &str)> = parts.iter().filter(|p| p.0 >= SLOW_MS || p.1 >= BUSY_MS).collect();
    slow.sort_by(|a, b| b.0.total_cmp(&a.0));
    if slow.is_empty() {
        return None;
    }
    let mut most: Vec<&(f32, f32, &str)> = parts.iter().filter(|p| p.1 >= 1.0).collect();
    most.sort_by(|a, b| b.1.total_cmp(&a.1));
    let most: Vec<String> = most.iter().take(5).map(|(_, all, name)| format!("{name} {all:.0} ms")).collect();
    Some(format!(
        "{}; most time: {}",
        slow.iter().map(|(worst, all, name)| format!("{name} {worst:.1} ms ({all:.0} ms in all)")).collect::<Vec<_>>().join(", "),
        most.join(", ")
    ))
}

/// The five parts that took the most since the last call (after `report`s), "frame task 3100 ms,
/// moves 640 ms, ...": logged every 30 s, slow or not.
pub fn top() -> String {
    let mut all: Vec<(f32, &str)> = (0..N).map(|i| (LONG[i].swap(0, Ordering::Relaxed) as f32 / 1e6, NAMES[i])).filter(|p| p.0 >= 1.0).collect();
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    all.iter().take(5).map(|(ms, name)| format!("{name} {ms:.0} ms")).collect::<Vec<_>>().join(", ")
}
