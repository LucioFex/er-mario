//! Experimental: SM64's Lakitu camera instead of Elden Ring's. Not a port of SM64's camera.c (it's
//! built around SM64's levels), but its free-roam behaviour:
//! - the camera stays where it is and only follows to keep its distance (a leash), so Mario runs
//!   around it the way he does in SM64
//! - C-left / C-right (right stick left / right, or the arrow keys) swing it 45 degrees around Mario
//! - C-down (right stick down, or down arrow) zooms out in three steps, C-up back in, and from the
//!   nearest into the C-up view (Mario stands, his head turning where you look)
//! - it doesn't bob with Mario's jumps (the height follows slowly in the air)
//! - walls in the way pull it in
//! It writes the game's render camera each frame; Elden Ring's own camera takes over for cutscenes,
//! doors and deaths.

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use eldenring::cs::CSCamera;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

/// Mario's head height (m), what the camera looks at
const FOCUS_HEIGHT: f32 = 1.0;
/// distance (m) and height above the focus for the zoom steps (C-down steps out, C-up in)
const ZOOM: [(f32, f32); 3] = [(5.0, 1.6), (7.0, 2.2), (9.5, 3.0)];
const NEAR: (f32, f32) = ZOOM[0];
/// one C-button press
const STEP: f32 = std::f32::consts::FRAC_PI_4;

/// On by default; `camera = elden` in er_mario.ini starts with Elden Ring's camera (F9 switches).
pub static ON: AtomicBool = AtomicBool::new(true);

pub fn load_setting() {
    if crate::paths::config("camera").is_some_and(|v| v.eq_ignore_ascii_case("elden")) {
        ON.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The last camera matrix written (rewritten just before drawing, in case the game copied its own
/// over it in between).
static LAST: Mutex<Option<([f32; 12], std::time::Instant)>> = Mutex::new(None);

/// The SM64 camera's forward direction right now (Mario steers relative to it), if it's active.
pub fn forward() -> Option<Vec3> {
    let (v, at) = (*LAST.lock().unwrap_or_else(|e| e.into_inner()))?;
    (at.elapsed().as_secs_f32() < 0.2).then(|| Vec3::new(v[6], v[7], v[8]))
}

/// Keeps the camera where it is (a popup pausing the world): the last matrix stays current, so it
/// goes on being written instead of the game's camera taking over.
pub fn hold() {
    if let Some((_, at)) = LAST.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        *at = std::time::Instant::now();
    }
}

pub fn reapply() {
    let _span = crate::perf::span(crate::perf::LAKITU);
    // only this frame's (not a stale one when the Mario frame stopped, e.g. loading)
    let Some((v, at)) = *LAST.lock().unwrap_or_else(|e| e.into_inner()) else { return };
    static GAP: AtomicBool = AtomicBool::new(false);
    if at.elapsed().as_secs_f32() <= 0.1 {
        GAP.store(false, std::sync::atomic::Ordering::Relaxed);
    } else {
        if !GAP.swap(true, std::sync::atomic::Ordering::Relaxed) {
            crate::log(format!("lakitu: not updated for {:.2} s (the game's camera shows)", at.elapsed().as_secs_f32()));
        }
        return;
    }
    let Ok(camera) = (unsafe { CSCamera::instance_mut() }) else { return };
    // every camera of the set, at every step from the game's camera update to drawing: the game
    // copies its own camera back in between, and what it sets up there (culling, the sun shadow
    // area) should follow ours
    for cam in [&mut camera.pers_cam_1, &mut camera.pers_cam_2, &mut camera.pers_cam_3, &mut camera.pers_cam_4] {
        let mm = &mut cam.matrix;
        (mm.0.0, mm.0.1, mm.0.2, mm.1.0, mm.1.1, mm.1.2) = (v[0], v[1], v[2], v[3], v[4], v[5]);
        (mm.2.0, mm.2.1, mm.2.2, mm.3.0, mm.3.1, mm.3.2) = (v[6], v[7], v[8], v[9], v[10], v[11]);
    }
}

/// SM64's first-person view (C-up while Mario stands still): look yaw and pitch
#[derive(Clone, Copy)]
struct FirstPerson {
    yaw: f32,
    pitch: f32,
}

/// SM64's camera sounds (SOUND_MENU_CAMERA_*), queued for the libsm64 thread
const SOUND_TURN: i32 = 0x700F_0081;
const SOUND_ZOOM_IN: i32 = 0x7006_0081;
const SOUND_ZOOM_OUT: i32 = 0x7007_0081;
const SOUND_BUZZ: i32 = 0x700E_0081;
static SOUNDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

fn sound(id: i32) {
    SOUNDS.lock().unwrap_or_else(|e| e.into_inner()).push(id);
}

/// The camera sounds to play now.
/// Where Lakitu's camera was last put (Elden Ring), if lately.
pub fn position() -> Option<Vec3> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).filter(|(_, t)| t.elapsed().as_secs_f32() < 0.5).map(|(m, _)| Vec3::new(m[9], m[10], m[11]))
}

pub fn take_sounds() -> Vec<i32> {
    std::mem::take(&mut *SOUNDS.lock().unwrap_or_else(|e| e.into_inner()))
}

static FIRST_PERSON: AtomicBool = AtomicBool::new(false);
static FAR_VIEW: AtomicBool = AtomicBool::new(false);

/// The camera status for the HUD's icons.
pub enum HudState {
    Near,
    Far,
    FirstPerson,
}

/// None when the SM64 camera isn't running.
pub fn hud_state() -> Option<HudState> {
    use std::sync::atomic::Ordering::Relaxed;
    let active = LAST.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|(_, at)| at.elapsed().as_secs_f32() < 0.2);
    if !ON.load(Relaxed) || !active {
        return None;
    }
    Some(if FIRST_PERSON.load(Relaxed) {
        HudState::FirstPerson
    } else if FAR_VIEW.load(Relaxed) {
        HudState::Far
    } else {
        HudState::Near
    })
}
/// Mario's head look in the C-up view (pitch, yaw relative to his body; radians)
static HEAD: Mutex<Option<(f32, f32)>> = Mutex::new(None);
/// how far Mario's head turns (SM64 doesn't let him break his neck)
const HEAD_YAW_MAX: f32 = 1.2;
const HEAD_PITCH_MAX: f32 = 0.7;

/// Where Mario's head should look (C-up view), or None.
pub fn head() -> Option<(f32, f32)> {
    *HEAD.lock().unwrap_or_else(|e| e.into_inner())
}

/// In the first-person view (Mario can't move, his model is hidden).
pub fn first_person() -> bool {
    FIRST_PERSON.load(std::sync::atomic::Ordering::Relaxed)
}

struct Cam {
    fp: Option<FirstPerson>,
    /// running up a hill: how far the look target rises (the camera looks up the slope)
    look_up: f32,
    /// extra height to see over a hill behind Mario (rises fast, settles slowly)
    lift: f32,
    /// how far the camera is from the focus right now (walls pull it in fast, it eases back out)
    dist: f32,
    /// Mario's feet last frame (to tell the game's coordinate shifts apart from his movement)
    mario: Vec3,
    pos: Vec3,
    /// where the camera would be without walls: its direction from Mario is the leash (walls
    /// pulling the camera in close to him must never flip it to his other side)
    leash: Vec3,
    focus: Vec3,
    /// current and wanted yaw (radians, direction from Mario to the camera)
    yaw: f32,
    target_yaw: Option<f32>,
    /// zoom step (0 nearest)
    zoom: usize,
    /// C-button edges (left, right, up, down)
    held: [bool; 4],
    /// how far it backs off for a big body nearby (m, eases in and out)
    big: f32,
    /// ...and for the bounces in a row up a big body (m), and how long it stays out after them (s)
    climb: f32,
    climb_hold: f32,
    /// the eased push out of a big body (snapped there and back, it popped as enemies walked by)
    push: Vec3,
    /// where the view looks, off Mario's head: up and over to a big body close by (eased)
    frame: Vec3,
    /// how much farther and higher it all is for Mega Mario (eased)
    size: f32,
}

static CAM: Mutex<Option<Cam>> = Mutex::new(None);

/// Big bodies near Mario (feet position, radius, height in m), set every frame by the combat
/// code: the camera backs off to fit them in and never goes inside one.
static BIG: Mutex<Vec<(Vec3, f32, f32)>> = Mutex::new(Vec::new());

pub fn set_big(bodies: Vec<(Vec3, f32, f32)>) {
    *BIG.lock().unwrap_or_else(|e| e.into_inner()) = bodies;
}

/// Bounces in a row up a big body (set every frame by the combat code).
static CLIMB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_climb(bounces: u32) {
    CLIMB.store(bounces, std::sync::atomic::Ordering::Relaxed);
}

/// Each bounce in a row up a big body backs the camera off this much more (m; it rises by
/// CLIMB_RISE of that), up to CLIMB_MAX: the beast grows huge on screen. It stays out CLIMB_HOLD
/// seconds after Mario lands, then eases back in.
const CLIMB_STEP: f32 = 2.2;
const CLIMB_RISE: f32 = 0.55;
const CLIMB_MAX: f32 = 11.0;
const CLIMB_HOLD: f32 = 1.5;

pub fn reset() {
    FIRST_PERSON.store(false, std::sync::atomic::Ordering::Relaxed);
    *HEAD.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if CAM.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        crate::log("lakitu: reset (Elden Ring's camera takes over)");
    }
    *CAM.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI { d - std::f32::consts::TAU } else { d }
}

/// Every frame in Mario mode (when on): moves the camera and writes it into the game's render
/// camera. `mario` feet position, `airborne`, C-buttons [left, right, up, down], `blocked` gives
/// the first map hit between two points.
pub fn update(
    dt: f32,
    mario: Vec3,
    airborne: bool,
    c: [bool; 4],
    look: (f32, f32),
    body_fwd: Vec3,
    idle: bool,
    exit: bool,
    hit: impl Fn(Vec3, Vec3) -> Option<Vec3>,
) {
    let Ok(camera) = (unsafe { CSCamera::instance_mut() }) else { return };
    let render = &mut camera.pers_cam_1;
    let m = &render.matrix;
    let game_pos = Vec3::new(m.3.0, m.3.1, m.3.2);
    let mut guard = CAM.lock().unwrap_or_else(|e| e.into_inner());
    // start where Elden Ring's camera is
    let cam = guard.get_or_insert_with(|| {
        let d = game_pos - mario;
        crate::log(format!("lakitu: start at {game_pos:.2?}, mario {mario:.2?}"));
        Cam {
            fp: None,
            look_up: 0.0,
            lift: 0.0,
            dist: NEAR.0,
            mario,
            pos: game_pos,
            leash: game_pos,
            focus: mario + Vec3::Y * FOCUS_HEIGHT,
            yaw: d.z.atan2(d.x),
            target_yaw: None,
            zoom: 0,
            held: c,
            big: 0.0,
            climb: 0.0,
            climb_hold: 0.0,
            push: Vec3::ZERO,
            frame: Vec3::ZERO,
            size: 1.0,
        }
    });

    // C-buttons (edges)
    let pressed: Vec<bool> = (0..4).map(|i| c[i] && !cam.held[i]).collect();
    cam.held = c;
    let base = cam.target_yaw.unwrap_or(cam.yaw);
    if cam.fp.is_none() && pressed[0] {
        cam.target_yaw = Some(base - STEP);
        sound(SOUND_TURN);
    }
    if cam.fp.is_none() && pressed[1] {
        cam.target_yaw = Some(base + STEP);
        sound(SOUND_TURN);
    }
    // first person: C-up while standing still in the near view; C-up / C-down / A / B leave it
    if let Some(fp) = cam.fp {
        if pressed[2] || pressed[3] || exit {
            // Lakitu comes back behind Mario, facing where he looked
            cam.fp = None;
            sound(SOUND_ZOOM_OUT);
            cam.yaw = fp.yaw + std::f32::consts::PI;
            cam.target_yaw = None;
            cam.leash = cam.focus + Vec3::new(cam.yaw.cos(), 0.0, cam.yaw.sin()) * NEAR.0;
            cam.pos = cam.leash + Vec3::Y * NEAR.1;
        }
    } else if pressed[2] {
        if cam.zoom > 0 {
            cam.zoom -= 1;
            sound(SOUND_ZOOM_IN);
        } else if idle && !airborne && !crate::mega::active() && !crate::mega::changing() {
            // looking where Mario faces, like SM64 (not as a giant: the eye would be in his legs)
            cam.fp = Some(FirstPerson { yaw: body_fwd.z.atan2(body_fwd.x), pitch: 0.0 });
            sound(SOUND_ZOOM_IN);
        } else {
            // first person only while standing still
            sound(SOUND_BUZZ);
        }
    } else if pressed[3] {
        if cam.zoom + 1 < ZOOM.len() {
            cam.zoom += 1;
            sound(SOUND_ZOOM_OUT);
        } else {
            // as far out as Lakitu goes
            sound(SOUND_BUZZ);
        }
    }
    FIRST_PERSON.store(cam.fp.is_some(), std::sync::atomic::Ordering::Relaxed);
    FAR_VIEW.store(cam.zoom > 0, std::sync::atomic::Ordering::Relaxed);
    if let Some(fp) = cam.fp.as_mut() {
        // the stick looks around; "right" is whichever way the view turns towards the screen's
        // right (the game's coordinates are mirrored against SM64's)
        let m = &render.matrix;
        let right_now = Vec3::new(m.0.0, m.0.1, m.0.2);
        let turn = Vec3::new(-fp.yaw.sin(), 0.0, fp.yaw.cos()).dot(right_now).signum();
        fp.yaw += look.0 * 2.2 * dt * if turn == 0.0 { 1.0 } else { turn };
        // the view goes as far as his head turns, no further
        let body_yaw = body_fwd.z.atan2(body_fwd.x);
        fp.yaw = body_yaw + angle_diff(fp.yaw, body_yaw).clamp(-HEAD_YAW_MAX, HEAD_YAW_MAX);
        fp.pitch = (fp.pitch + look.1 * 1.6 * dt).clamp(-HEAD_PITCH_MAX, HEAD_PITCH_MAX);
        let view = Vec3::new(fp.yaw.cos() * fp.pitch.cos(), fp.pitch.sin(), fp.yaw.sin() * fp.pitch.cos());
        // SM64's C-up view: just behind and above Mario's head, his head turning to look where
        // the view looks (within what a neck allows)
        let head = mario + Vec3::Y * 1.45;
        let eye = head - view * 1.8 + Vec3::Y * 0.1;
        let flat_view = Vec3::new(view.x, 0.0, view.z).normalize_or_zero();
        let body = Vec3::new(body_fwd.x, 0.0, body_fwd.z).normalize_or_zero();
        let rel_yaw = body.cross(flat_view).y.atan2(body.dot(flat_view));
        *HEAD.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((fp.pitch.clamp(-HEAD_PITCH_MAX, HEAD_PITCH_MAX), rel_yaw.clamp(-HEAD_YAW_MAX, HEAD_YAW_MAX)));
        cam.pos = eye;
        cam.focus = head;
        write(render, (head + view * 3.0 - eye).normalize_or(view), eye);
        return;
    }
    *HEAD.lock().unwrap_or_else(|e| e.into_inner()) = None;

    // the focus: Mario's head; in the air the height follows slowly (no bobbing on jumps), unless
    // he falls below it
    // Mega Mario: farther back and higher, his head the focus (eased as he grows and shrinks)
    let size_want = 1.0 + (crate::size_factor() * crate::mega::visual() - 1.0) * 0.8;
    cam.size += (size_want - cam.size) * (1.0 - (-2.5 * dt).exp());
    let ks = cam.size;
    let want = mario + Vec3::Y * FOCUS_HEIGHT * ks;
    // Mario leapt metres in one frame: the game re-based its coordinates (floating origin) or
    // teleported him; the camera moves along instead of swinging round
    let jump = mario - cam.mario;
    cam.mario = mario;
    if Vec3::new(jump.x, 0.0, jump.z).length() > 4.0 || jump.y.abs() > 4.0 {
        crate::log(format!("lakitu: Mario moved {jump:.2?} in one frame, camera moved along"));
        cam.pos += jump;
        cam.leash += jump;
        cam.focus += jump;
    }
    let rate = |r: f32| 1.0 - (-r * dt).exp();
    cam.focus.x = want.x;
    cam.focus.z = want.z;
    let vy = if airborne && want.y > cam.focus.y - 0.5 { 1.2 } else { 5.0 };
    cam.focus.y += (want.y - cam.focus.y) * rate(vy);
    // the lag must never put the focus into the ground (running uphill): at least half a metre
    // above the ground under Mario (not his feet: in a jump the focus should stay low)
    let head = mario + Vec3::Y * FOCUS_HEIGHT * ks;
    let ground = hit(head, mario - Vec3::Y * 30.0).map(|g| g.y).unwrap_or(mario.y);
    cam.focus.y = cam.focus.y.max(ground + 0.5);

    let yaw_before = cam.yaw;
    // yaw: a C-button swing, or the leash (the camera stays put, Mario runs around it)
    match cam.target_yaw {
        Some(t) => {
            let d = angle_diff(t, cam.yaw);
            cam.yaw += d * rate(10.0);
            if d.abs() < 0.01 {
                cam.yaw = t;
                cam.target_yaw = None;
            }
        }
        None => {
            let d = cam.leash - cam.focus;
            if d.x.abs() + d.z.abs() > 0.01 {
                cam.yaw = d.z.atan2(d.x);
            }
        }
    }
    if angle_diff(cam.yaw, yaw_before).abs() > 0.5 {
        crate::log(format!(
            "lakitu: yaw jumped {:.0} deg ({}), focus {:.2?}, leash {:.2?}, pos {:.2?}, dt {dt:.3}",
            angle_diff(cam.yaw, yaw_before).to_degrees(),
            if cam.target_yaw.is_some() { "c-button" } else { "leash" },
            cam.focus,
            cam.leash,
            cam.pos
        ));
    }
    let (dist, height) = ZOOM[cam.zoom];
    let (dist, height) = (dist * ks, height * ks);
    // a big body close to Mario (a giant beast, a dragon): the camera backs off and rises to fit
    // it in (close up it ended up inside the body)
    let bodies = BIG.lock().unwrap_or_else(|e| e.into_inner()).clone();
    // (by its size and how close it is: from BIG_FAR metres off its side it starts, from BIG_NEAR
    // it's all the way; the view also looks up and over to its chest, so its head and its blows
    // stay on screen, not only once Mario is up on it)
    let (want_big, want_frame) = bodies
        .iter()
        .map(|(p, r, h)| {
            let gap = ((*p - mario).with_y(0.0).length() - r).max(0.0);
            let near = 1.0 - ((gap - BIG_NEAR) / (BIG_FAR - BIG_NEAR)).clamp(0.0, 1.0);
            let near = near * near * (3.0 - 2.0 * near);
            let back = ((h - 1.5) * 0.9 + r * 1.0).clamp(0.0, BIG_MAX) * near;
            let chest = *p + Vec3::Y * (h * 0.5);
            let to = chest - head;
            let side = to.with_y(0.0) * 0.3;
            let side = side.clamp_length_max(BIG_FRAME_SIDE);
            let up = (to.y * 0.35).clamp(0.0, BIG_FRAME_UP);
            (back, (side + Vec3::Y * up) * near)
        })
        .fold((0.0f32, Vec3::ZERO), |a, b| if b.0 > a.0 { b } else { a });
    // (out quickly enough to see the beast coming, back in slowly: no pumping as it steps about)
    cam.big += (want_big - cam.big) * if want_big > cam.big { rate(1.6) } else { rate(0.5) };
    cam.frame += (want_frame - cam.frame) * rate(1.2);
    // bounces in a row up a big body: farther out with each (fast, a swoop), slowly back in
    let bounces = CLIMB.load(std::sync::atomic::Ordering::Relaxed);
    let want_climb = (bounces as f32 * CLIMB_STEP).min(CLIMB_MAX);
    if want_climb >= cam.climb {
        cam.climb += (want_climb - cam.climb) * rate(2.5);
        cam.climb_hold = CLIMB_HOLD;
    } else if cam.climb_hold > 0.0 {
        cam.climb_hold -= dt;
    } else {
        cam.climb += (want_climb - cam.climb) * rate(0.6);
    }
    let (dist, height) = (dist + cam.big + cam.climb, height + cam.big * 0.4 + cam.climb * CLIMB_RISE);
    let flat = Vec3::new(cam.yaw.cos() * dist, 0.0, cam.yaw.sin() * dist);
    // uphill ahead (in the view direction): look up the slope, the camera a bit lower
    // (the ground ahead, measured from a point in the open near head height: from high up it
    // would find ceilings and roofs indoors)
    let ahead = mario - flat.normalize_or_zero() * 5.0;
    let probe = Vec3::new(ahead.x, mario.y + FOCUS_HEIGHT * ks + 1.5, ahead.z);
    let open = hit(mario + Vec3::Y * FOCUS_HEIGHT * ks, probe).is_none();
    let rise = if open { hit(probe, ahead - Vec3::Y * 8.0).map(|g| g.y - ground).unwrap_or(0.0) } else { 0.0 };
    let want_up = (rise * 0.6).clamp(0.0, 3.0);
    cam.look_up += (want_up - cam.look_up) * rate(3.0);
    let height = height - (cam.look_up * 0.5).min(height * 0.7);
    cam.leash = cam.focus + flat + Vec3::Y * height;
    // a hill or slope behind Mario: Lakitu flies up over it (the lowest height that sees him);
    // only when that doesn't help (a wall, a low ceiling) does the camera pull in
    // (rays from Mario's head: always in the open, unlike the lagging focus)
    let clear = [0.0, 1.0, 2.0, 3.5, 5.0].into_iter().find(|&extra| hit(head, cam.focus + flat + Vec3::Y * (height + extra)).is_none());
    let want_lift = clear.unwrap_or(0.0);
    if clear != Some(0.0) {
        static LOGGED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 20 == 0 {
            let h0 = hit(head, cam.focus + flat + Vec3::Y * height);
            crate::dlog(format!(
                "lakitu: view blocked (clear at {clear:?}), mario {mario:.2?}, focus {:.2?}, cam {:.2?}, hit {h0:.2?} ({:.2} m from focus)",
                cam.focus,
                cam.focus + flat + Vec3::Y * height,
                h0.map(|h| (h - cam.focus).length()).unwrap_or(-1.0)
            ));
        }
    }
    cam.lift += (want_lift - cam.lift) * if want_lift > cam.lift { rate(10.0) } else { rate(1.5) };
    let offset = flat + Vec3::Y * (height + cam.lift);
    let ideal = cam.focus + offset;
    let full = offset.length();
    let dir = offset / full;
    let want = if clear.is_some() {
        full
    } else {
        hit(head, ideal).map(|h| ((h - cam.focus).length() - 0.3).max(0.6)).unwrap_or(full)
    };
    cam.dist += (want - cam.dist) * if want < cam.dist { rate(15.0) } else { rate(2.0) };
    let base = cam.focus + dir * cam.dist;
    // never inside a big body: out to its side (or the top, close above it), eased
    let mut pos = base;
    for (p, r, h) in &bodies {
        let side = (pos - *p).with_y(0.0);
        let r = r + 0.5;
        if side.length() < r && pos.y > p.y - 0.5 && pos.y < p.y + h + 0.5 {
            let out = side.try_normalize().or((mario - *p).with_y(0.0).try_normalize()).unwrap_or(Vec3::X);
            pos = Vec3::new(p.x + out.x * r, pos.y, p.z + out.z * r);
        }
    }
    cam.push += (pos - base - cam.push) * rate(6.0);
    let mut pos = base + cam.push;
    // never under the ground (the ray starts at head or camera height, inside the room: from
    // higher up it would find the roof in interiors and put the camera on top of it)
    let top = Vec3::new(pos.x, pos.y.max(head.y) + 0.1, pos.z);
    if let Some(g) = hit(top, pos - Vec3::Y * 1.0) {
        pos.y = pos.y.max(g.y + 0.4);
    }
    cam.pos = pos;

    let fwd = (cam.focus + cam.frame + Vec3::Y * cam.look_up - cam.pos).normalize_or(Vec3::Z);
    write(render, fwd, cam.pos);
}

/// A big body's pull on the camera: how close (m, off its side) it starts and is full, the most it
/// backs off (m), and how far the view turns to it (m sideways, m up, at most)
const BIG_FAR: f32 = 14.0;
const BIG_NEAR: f32 = 3.0;
const BIG_MAX: f32 = 9.0;
const BIG_FRAME_SIDE: f32 = 2.0;
const BIG_FRAME_UP: f32 = 2.2;

/// A heavy blow shakes Lakitu's camera: when, and how hard (~1 a big hit, 2 the giant hammer).
static SHAKE: Mutex<Option<(std::time::Instant, f32)>> = Mutex::new(None);
const SHAKE_TIME: f32 = 0.22;
/// metres of shake per unit of strength
const SHAKE_SIZE: f32 = 0.05;

/// Shakes the camera (game thread, impact.rs); a weaker shake doesn't cut a stronger one short.
pub fn shake(strength: f32) {
    let mut s = SHAKE.lock().unwrap_or_else(|e| e.into_inner());
    if s.is_none_or(|(t, k)| shake_left(t, k) < strength) {
        *s = Some((std::time::Instant::now(), strength));
    }
}

fn shake_left(since: std::time::Instant, strength: f32) -> f32 {
    let p = 1.0 - since.elapsed().as_secs_f32() / (SHAKE_TIME * (1.0 + strength * 0.5));
    if p > 0.0 { strength * p * p } else { 0.0 }
}

/// Writes the render camera: rows right, up, forward, position (keeping the game's handedness).
fn write(render: &mut eldenring::cs::CSPersCam, fwd: Vec3, pos: Vec3) {
    let m = &render.matrix;
    let (gr, gu, gf) = (Vec3::new(m.0.0, m.0.1, m.0.2), Vec3::new(m.1.0, m.1.1, m.1.2), Vec3::new(m.2.0, m.2.1, m.2.2));
    let handed = gr.dot(gu.cross(gf)).signum();
    let mut right = Vec3::Y.cross(fwd).normalize_or(Vec3::X);
    let up = fwd.cross(right).normalize_or(Vec3::Y);
    if right.dot(up.cross(fwd)).signum() != handed {
        right = -right;
    }
    let up = if up.y < 0.0 { -up } else { up };
    // (a shake: two quick wobbles, sideways and up, dying away)
    let pos = match *SHAKE.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((since, k)) => {
            let (t, amp) = (since.elapsed().as_secs_f32(), shake_left(since, k) * SHAKE_SIZE);
            pos + right * (t * 71.0).sin() * amp + up * (t * 53.0 + 1.3).sin() * amp
        }
        None => pos,
    };
    let mm = &mut render.matrix;
    (mm.0.0, mm.0.1, mm.0.2) = (right.x, right.y, right.z);
    (mm.1.0, mm.1.1, mm.1.2) = (up.x, up.y, up.z);
    (mm.2.0, mm.2.1, mm.2.2) = (fwd.x, fwd.y, fwd.z);
    (mm.3.0, mm.3.1, mm.3.2) = (pos.x, pos.y, pos.z);
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(([right.x, right.y, right.z, up.x, up.y, up.z, fwd.x, fwd.y, fwd.z, pos.x, pos.y, pos.z], std::time::Instant::now()));
}

