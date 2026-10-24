//! v4 Part 4's prototype (plan task 4.0): can the mod drive Torrent itself, the kart's body to be?
//! With `kart_test = on` in er_mario.ini, riding him: the horse is given no stick (hidden_sticks,
//! lib.rs: he stands), and the mod moves him itself every frame along the stick (from the camera)
//! at a run, on the ground under him, turned his way, as bossride.rs drives a captured enemy. The
//! next frame it reads him back: how far the game moved him on top of what was written, whether
//! his turn stayed. Every 2 s a log line, and once the verdict.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::WorldChrMan;
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::log;

/// how fast it drives him (m/s), how often it reports (s), after how many frames it says what it found
const SPEED: f32 = 8.0;
const REPORT: f32 = 2.0;
const VERDICT_AFTER: u32 = 150;
/// the game adding less than this (m a frame, on average and at most) on top: he can be driven
const DRIFT_OK: f32 = 0.01;
const DRIFT_MOST: f32 = 0.05;
/// the first frames aren't counted (he was still going on the game's stick)
const WARMUP: u32 = 5;

#[derive(Default)]
struct Test {
    /// what was written last frame (position, turn)
    written: Option<(Vec3, Quat)>,
    warm: u32,
    frames: u32,
    added: f32,
    most: f32,
    vertical: f32,
    turned: f32,
    grounded: u32,
    since: Option<Instant>,
    said: bool,
}

static TEST: Mutex<Option<Test>> = Mutex::new(None);

/// The test is on (er_mario.ini `kart_test = on`).
pub fn enabled() -> bool {
    crate::paths::config("kart_test").is_some_and(|v| v.trim().eq_ignore_ascii_case("on"))
}

/// The horse gets no stick from the game's input while the test drives him.
pub fn holds_stick() -> bool {
    enabled() && TEST.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Every frame: `mounted` on Torrent, the stick's way in the world (`steer`: level, its length the
/// push 0-1), the frame's `dt`; `ray(from, to)`: the map's first hit.
pub fn frame(mounted: bool, steer: Vec3, dt: f32, ray: impl Fn(Vec3, Vec3) -> Option<Vec3>) {
    let mut guard = TEST.lock().unwrap_or_else(|e| e.into_inner());
    if !enabled() || !mounted {
        if guard.take().is_some_and(|t| t.frames > 0) {
            log("kart test: off");
        }
        return;
    }
    let t = guard.get_or_insert_with(|| {
        log("kart test: on, riding: the mod drives Torrent");
        Test::default()
    });
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let Some(c) = wcm.summon_buddy_chr_set.characters().chain(wcm.chr_sets.iter().flatten().flat_map(|set| set.characters())).find(|c| crate::combat::is_torrent(c)) else { return };
    let ph = &mut c.modules.physics;
    let p = Vec3::new(ph.position.0, ph.position.1, ph.position.2);
    let o = ph.orientation;
    let q_now = Quat::from_xyzw(o.0, o.1, o.2, o.3);
    // what the game did on top of last frame's
    if let Some((wp, wq)) = t.written.filter(|_| t.warm >= WARMUP) {
        let d = p - wp;
        let flat = d.with_y(0.0).length();
        t.frames += 1;
        t.added += flat;
        t.most = t.most.max(flat);
        t.vertical += d.y.abs();
        t.turned += wq.angle_between(q_now).to_degrees();
        if ph.touching_solid_ground {
            t.grounded += 1;
        }
    }
    // this frame's: along the stick on the ground under him, turned his way
    let dt = dt.clamp(0.0, 0.05);
    let push = steer.with_y(0.0);
    let mut next = p + push.clamp_length_max(1.0) * SPEED * dt;
    if let Some(g) = ray(next + Vec3::Y * 1.5, next - Vec3::Y * 4.0) {
        next.y = g.y;
    }
    let q = if push.length() > 0.2 {
        let d = push.normalize();
        Quat::from_rotation_y(std::f32::consts::PI + d.x.atan2(d.z))
    } else {
        q_now
    };
    ph.position = HavokPosition(next.x, next.y, next.z, 0.0);
    ph.orientation = eldenring::rotation::Quaternion(q.x, q.y, q.z, q.w);
    ph.chr_proxy_pos_update_requested = true;
    t.written = Some((next, q));
    t.warm = t.warm.saturating_add(1);
    let since = *t.since.get_or_insert_with(Instant::now);
    if since.elapsed().as_secs_f32() >= REPORT && t.frames > 0 {
        let n = t.frames as f32;
        log(format!(
            "kart test: {} frames written, the game added {:.1} cm/frame (most {:.1}), up/down {:.1} cm, turned back {:.1} deg, on the ground {:.0}%",
            t.frames,
            t.added / n * 100.0,
            t.most * 100.0,
            t.vertical / n * 100.0,
            t.turned / n,
            t.grounded as f32 / n * 100.0
        ));
        if t.frames >= VERDICT_AFTER && !t.said {
            t.said = true;
            let ok = t.added / n < DRIFT_OK && t.most < DRIFT_MOST;
            log(if ok { "kart test: Torrent can be driven (the game adds nothing on top)" } else { "kart test: Torrent fights back (the game moves him on top of the mod)" });
        }
        t.since = Some(Instant::now());
    }
}
