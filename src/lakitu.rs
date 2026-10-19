//! Experimental: SM64's Lakitu camera instead of Elden Ring's. Not a port of SM64's camera.c (it's
//! built around SM64's levels), but its free-roam behaviour:
//! - the camera stays where it is and only follows to keep its distance (a leash), so Mario runs
//!   around it the way he does in SM64
//! - the right stick left / right (or the arrow keys) turns it round Mario, smoothly, faster the
//!   further it's pushed
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
    /// the air fight's share (0 on the ground .. 1 in an air combo), and the view's lean towards
    /// the enemy Mario fights (eased)
    air: f32,
    fight_frame: Vec3,
    /// the room around Mario: how far the walls are in eight directions and the ceiling above his
    /// head (m), measured a couple of rays a frame (`probe` the next), and how shut in he is
    /// (0 the open field .. 1 a narrow corridor, eased)
    probe: usize,
    walls: [f32; 8],
    /// the ceiling's height (Elden Ring Y, absolute: Mario's head moves between two looks at it)
    ceiling_y: f32,
    enclosed: f32,
    /// a wall behind the camera: the way round it Lakitu drifts to (yaw), frames to the next look,
    /// and how long since the last slide ended (s: none again right away, or it orbits)
    slide: Option<f32>,
    slide_wait: u32,
    slid_ago: f32,
    /// how fast Mario runs, eased (0..1)
    speed: f32,
    /// the view's lead ahead of where Mario runs, and the eased push off walls close to the lens
    lead: Vec3,
    pad: Vec3,
    /// how long he's kept running the same way (s), and that way
    run_time: f32,
    run_dir: Vec3,
    /// how far it backs off for a group of enemies (m, eased), and to fit the fight in
    crowd: f32,
    fit: f32,
    /// co-op: how far it backs off to fit the partner in (m, eased), and the view's lean to them
    mate: f32,
    mate_frame: Vec3,
    /// on a colossus (colossus.rs): backed off by its size, sunk below Mario as he climbs
    coloss: f32,
    coloss_sink: f32,
}

static CAM: Mutex<Option<Cam>> = Mutex::new(None);

/// Big bodies near Mario (feet position, radius, height in m), set every frame by the combat
/// code: the camera backs off to fit them in and never goes inside one.
static BIG: Mutex<Vec<(Vec3, f32, f32)>> = Mutex::new(Vec::new());

pub fn set_big(bodies: Vec<(Vec3, f32, f32)>) {
    *BIG.lock().unwrap_or_else(|e| e.into_inner()) = bodies;
}

/// The enemy Mario fights (its chest, and whether it's up in the air for him), set every tick by
/// the combat code: God of War's and Devil May Cry's fight camera frames him with it, and in an
/// air combo rises with him, a little back and level, looking up at the fight.
static FIGHT: Mutex<Option<(Vec3, bool)>> = Mutex::new(None);

pub fn set_fight(target: Option<(Vec3, bool)>) {
    *FIGHT.lock().unwrap_or_else(|e| e.into_inner()) = target;
}

/// The enemies around Mario he could be fighting (their chests), set every tick by the combat
/// code: with a few of them about, the view frames the group (God of War), not just the one.
static CROWD: Mutex<Vec<Vec3>> = Mutex::new(Vec::new());

pub fn set_crowd(chests: Vec<Vec3>) {
    *CROWD.lock().unwrap_or_else(|e| e.into_inner()) = chests;
}

/// The co-op partner nearest Mario (their chest), set every frame (lib.rs): close by, the view
/// leans their way and backs off a little, so the two share the screen (it never turns for them:
/// the player keeps the camera).
static MATE: Mutex<Option<Vec3>> = Mutex::new(None);

pub fn set_mate(chest: Option<Vec3>) {
    *MATE.lock().unwrap_or_else(|e| e.into_inner()) = chest;
}

/// the partner's framing: from MATE_NEAR m it comes in (over MATE_EASE m), it's gone by MATE_FAR
/// (fading over MATE_EASE m before); the view leans this share of the way to them (at most
/// MATE_FRAME_MAX m) and backs off this much per metre past MATE_NEAR (at most MATE_BACK_MAX); in
/// a fight it's this share of that (the fight's framing comes first)
const MATE_NEAR: f32 = 2.5;
const MATE_FAR: f32 = 18.0;
const MATE_EASE: f32 = 3.0;
const MATE_LEAN: f32 = 0.3;
const MATE_FRAME_MAX: f32 = 3.5;
const MATE_BACK_PER_M: f32 = 0.3;
const MATE_BACK_MAX: f32 = 3.5;
const MATE_IN_FIGHT: f32 = 0.4;

/// When Mario last threw a blow (set by the combat code): the fight's camera turns to the enemy
/// only while he fights, not as he runs away from it.
static BLOW: Mutex<Option<std::time::Instant>> = Mutex::new(None);

pub fn blow() {
    *BLOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
}

/// Bounces in a row up a big body (set every frame by the combat code).
static CLIMB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Mario on a colossus or swinging from one (colossus.rs): its feet, height, radius and head; the
/// camera backs off to take in the whole beast, sinks below him as he climbs to look up at it, and
/// keeps round on his side of it.
static COLOSSUS: Mutex<Option<(Vec3, f32, f32, Vec3)>> = Mutex::new(None);

/// How open it is around Mario (0 shut in a room, 1 in the open: Lakitu's sense of the walls).
pub fn openness() -> f32 {
    CAM.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(0.5, |c| (1.0 - c.enclosed).clamp(0.0, 1.0))
}

pub fn set_colossus(c: Option<(Vec3, f32, f32, Vec3)>) {
    *COLOSSUS.lock().unwrap_or_else(|e| e.into_inner()) = c;
}

/// the colossus view: backed off at most this far (m), sunk this share of his climb, leaning to its
/// head at most this much (m)
const COLOSSUS_MAX: f32 = 16.0;
const COLOSSUS_SINK: f32 = 0.55;
const COLOSSUS_LEAN: f32 = 4.0;

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
    *FIGHT.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
    orbit: f32,
    look: (f32, f32),
    body_fwd: Vec3,
    idle: bool,
    exit: bool,
    hit: impl Fn(Vec3, Vec3) -> Option<Vec3>,
) {
    // (in real time: in a slow-motion moment, style.rs, the world slows but Mario doesn't, and the
    // camera has to keep up with him)
    let dt = {
        static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
        let now = std::time::Instant::now();
        let real = LAST.lock().unwrap_or_else(|e| e.into_inner()).replace(now).map(|t| now.duration_since(t).as_secs_f32());
        real.filter(|r| *r > 0.0).map_or(dt, |r| r.min(0.1))
    };
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
            air: 0.0,
            fight_frame: Vec3::ZERO,
            probe: 0,
            walls: [ROOM_PROBE; 8],
            ceiling_y: mario.y + FOCUS_HEIGHT + CEILING_PROBE,
            enclosed: 0.0,
            slide: None,
            slide_wait: 0,
            slid_ago: SLIDE_DWELL,
            speed: 0.0,
            lead: Vec3::ZERO,
            pad: Vec3::ZERO,
            run_time: 0.0,
            run_dir: Vec3::ZERO,
            crowd: 0.0,
            fit: 0.0,
            mate: 0.0,
            mate_frame: Vec3::ZERO,
            coloss: 0.0,
            coloss_sink: 0.0,
        }
    });

    // C-buttons (edges)
    let pressed: Vec<bool> = (0..4).map(|i| c[i] && !cam.held[i]).collect();
    cam.held = c;
    // (a turn of his own: the camera leaves its framing and chasing to him a while)
    static MANUAL: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    if orbit.abs() > ORBIT_DEAD && cam.fp.is_none() {
        *MANUAL.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
        // the stick turns it smoothly round him, faster the further it's pushed (eased in: a
        // light touch nudges it)
        let k = (orbit.abs() - ORBIT_DEAD) / (1.0 - ORBIT_DEAD);
        cam.target_yaw = None;
        cam.yaw += orbit.signum() * k * k * ORBIT_SPEED * dt;
        let flat = (cam.leash - cam.focus).with_y(0.0).length().max(1.0);
        cam.leash = cam.focus + Vec3::new(cam.yaw.cos() * flat, cam.leash.y - cam.focus.y, cam.yaw.sin() * flat);
    }
    let manual = MANUAL.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < ASSIST_HOLD);
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
    // an air fight: an enemy up there with him, or one near and above (God of War / Devil May
    // Cry): the camera keeps up with his height, backs off a little and levels out to look up
    let fight = *FIGHT.lock().unwrap_or_else(|e| e.into_inner());
    let fight = fight.filter(|(c, _)| (*c - mario).with_y(0.0).length() < FIGHT_FAR);
    // a group around him: where its middle is (and how many), for framing them all
    let group: Vec<Vec3> = CROWD.lock().unwrap_or_else(|e| e.into_inner()).iter().copied().filter(|c| (*c - mario).with_y(0.0).length() < CROWD_FAR).collect();
    let crowd = (group.len() >= 2).then(|| group.iter().copied().sum::<Vec3>() / group.len() as f32);
    let air_fight = airborne && fight.is_some_and(|(c, up)| up || (c.y > mario.y + 0.3 && (c - mario).length() < AIR_NEAR));
    cam.air += (if air_fight { 1.0 } else { 0.0 } - cam.air) * rate(if air_fight { 3.0 } else { 1.2 });
    let vy = if airborne && want.y > cam.focus.y - 0.5 { 1.2 + (AIR_FOLLOW - 1.2) * cam.air } else { 5.0 };
    cam.focus.y += (want.y - cam.focus.y) * rate(vy);
    // the lag must never put the focus into the ground (running uphill): at least half a metre
    // above the ground under Mario (not his feet: in a jump the focus should stay low)
    let head = mario + Vec3::Y * FOCUS_HEIGHT * ks;
    let ground = hit(head, mario - Vec3::Y * 30.0).map(|g| g.y).unwrap_or(mario.y);
    cam.focus.y = cam.focus.y.max(ground + 0.5);
    // the room: two rays a frame round the eight directions and up, from his head
    for _ in 0..2 {
        let i = cam.probe % 9;
        cam.probe = cam.probe.wrapping_add(1);
        if i < 8 {
            // (from his head and from higher up: a wall stops both, a hillside rising ahead only
            // the lower one, and it's the farther that counts)
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            let d = Vec3::new(a.cos(), 0.0, a.sin()) * ROOM_PROBE * ks;
            let far = |from: Vec3| hit(from, from + d).map_or(ROOM_PROBE * ks, |h| (h - from).with_y(0.0).length());
            let low = far(head);
            // (under a low ceiling the higher look would start in it: the lower one alone then)
            let up = (WALL_HIGH * ks).min(cam.ceiling_y - head.y - 0.25);
            cam.walls[i] = if low < ROOM_PROBE * ks && up > 0.4 { low.max(far(head + Vec3::Y * up)) } else { low };
        } else {
            cam.ceiling_y = hit(head, head + Vec3::Y * CEILING_PROBE * ks).map_or(head.y + CEILING_PROBE * ks, |h| h.y);
        }
    }
    let enclosed_was = cam.enclosed;
    {
        let mut w = cam.walls;
        w.sort_by(f32::total_cmp);
        // (the nearest half: a corridor is shut in even though its two ends are open)
        let walls = (w[0] + w[1] + w[2] + w[3]) / 4.0 / ks;
        let wall_k = 1.0 - ((walls - ROOM_TIGHT) / (ROOM_OPEN - ROOM_TIGHT)).clamp(0.0, 1.0);
        // (a ceiling found below his head is stale: he rose through an opening or was moved)
        if cam.ceiling_y <= head.y {
            cam.ceiling_y = head.y + CEILING_PROBE * ks;
        }
        let ceil_k = 1.0 - (((cam.ceiling_y - head.y) / ks - CEILING_LOW) / (CEILING_HIGH - CEILING_LOW)).clamp(0.0, 1.0);
        let want = (wall_k * 0.7).max(ceil_k * 0.7).max((wall_k + ceil_k) * 0.6).min(1.0);
        // (more than half shut in only under a roof: a cliff or a canyon's walls alone aren't a room)
        let want = if ceil_k < 0.2 { want.min(0.6) } else { want };
        cam.enclosed += (want - cam.enclosed) * rate(if want > cam.enclosed { 2.0 } else { 1.0 });
    }
    if (cam.enclosed - 0.5).signum() != (enclosed_was - 0.5).signum() {
        crate::dlog(format!("lakitu: {} (walls {:.1?} m, ceiling {:.1} m)", if cam.enclosed > 0.5 { "indoors" } else { "outdoors" }, cam.walls, cam.ceiling_y - head.y));
    }
    let enc = cam.enclosed;
    // where he's running: the view leads a little ahead (not in a fight: it frames the enemy)
    let vel = if dt > 1e-4 && jump.length() < 4.0 { jump.with_y(0.0) / dt } else { Vec3::ZERO };
    let fighting_now = BLOW.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < FIGHT_HOT);
    let lead_want = if fighting_now { Vec3::ZERO } else { (vel * LEAD_TIME).clamp_length_max(LEAD_MAX * ks) * (1.0 - 0.6 * enc) };
    cam.lead += (lead_want - cam.lead) * rate(1.8);
    // (eased: the feet move on SM64's 30 ticks a second, the camera on every frame)
    cam.speed += ((vel.length() / (8.0 * ks)).clamp(0.0, 1.0) - cam.speed) * rate(2.0);
    let speed_k = cam.speed;
    // running the same way a while (the chase camera below follows only a steady run)
    let run = vel.normalize_or_zero();
    if vel.length() > 2.5 * ks && run.dot(cam.run_dir) > 0.85 {
        cam.run_time += dt;
    } else {
        cam.run_time = 0.0;
    }
    if vel.length() > 2.5 * ks {
        cam.run_dir = (cam.run_dir * 0.9 + run * 0.1).normalize_or(run);
    }

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
    // a steady run (not towards the camera, not in a fight, not right after a swing of his own):
    // Lakitu drifts round behind Mario, like God of War's camera, faster the faster he runs; the
    // stick never has to steer it
    if cam.target_yaw.is_none() && !manual && cam.slide.is_none() && !fighting_now && cam.run_time > CHASE_AFTER && !airborne {
        let view = -Vec3::new(cam.yaw.cos(), 0.0, cam.yaw.sin());
        let along = cam.run_dir.dot(view);
        if along > CHASE_COS {
            let want = (-cam.run_dir.z).atan2(-cam.run_dir.x);
            let k = ((along - CHASE_COS) / (1.0 - CHASE_COS)).clamp(0.0, 1.0) * speed_k * (1.0 - 0.5 * enc);
            cam.yaw += angle_diff(want, cam.yaw) * rate(CHASE_RATE * k);
        }
    }
    // a big body close by but out of the view (behind or beside the camera): Lakitu drifts round,
    // slowly, until it and Mario are both in sight (Mario in front of it), unless he swung the
    // camera himself just now
    if cam.target_yaw.is_none() && !manual && cam.fp.is_none() {
        let bodies = BIG.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let view = -Vec3::new(cam.yaw.cos(), 0.0, cam.yaw.sin());
        let close: Vec<(f32, Vec3)> =
            bodies.iter().map(|(p, r, _)| (((*p - mario).with_y(0.0).length() - r).max(0.0), *p)).filter(|(gap, _)| *gap < ASSIST_FAR).collect();
        // (one already in view: leave it; two on either side of him can't both be)
        let seen = close.iter().any(|(_, p)| (*p - mario).with_y(0.0).normalize_or(view).dot(view) >= ASSIST_COS);
        let nearest = close.iter().copied().filter(|_| !seen).min_by(|a, b| a.0.total_cmp(&b.0));
        // (no big body: the enemy he fights, gentler)
        let fought = FIGHT.lock().unwrap_or_else(|e| e.into_inner()).filter(|(c, _)| (*c - mario).with_y(0.0).length() < FIGHT_FAR);
        let fighting = BLOW.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < FIGHT_HOT);
        let fought = crowd.map(|c| (c, false)).or(fought);
        let nearest = nearest.or_else(|| {
            let (c, _) = fought.filter(|_| close.is_empty() && fighting)?;
            let d = (c - mario).with_y(0.0).length();
            Some((d.max(0.0) * ASSIST_FAR / FIGHT_FAR + ASSIST_FAR * (1.0 - FIGHT_ASSIST), c))
        });
        if let Some((gap, p)) = nearest {
            let to = (p - mario).with_y(0.0);
            let off = to.normalize_or(view).dot(view);
            if to.length() > 0.5 && off < ASSIST_COS {
                let want = (-to.z).atan2(-to.x);
                let pull = (1.0 - gap / ASSIST_FAR).clamp(0.0, 1.0) * ((ASSIST_COS - off) / (ASSIST_COS + 1.0)).clamp(0.0, 1.0);
                cam.yaw += angle_diff(want, cam.yaw) * (rate(ASSIST_RATE) * pull);
            }
        }
    }
    // a wall behind the camera (found below, last frame): Lakitu slides round it, to where it sees
    cam.slid_ago += dt;
    if let Some(t) = cam.slide.filter(|_| cam.target_yaw.is_none() && !manual) {
        let d = angle_diff(t, cam.yaw);
        cam.yaw += d * rate(SLIDE_RATE);
        if d.abs() < 0.05 {
            cam.slide = None;
            cam.slid_ago = 0.0;
        }
    } else if cam.slide.take().is_some() {
        cam.slid_ago = 0.0;
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
    // in a fight: back far enough for Mario and the enemy both, more the farther he's zoomed out
    // (zoomed out, he wants to see more of the fight)
    let fit_want = match fight {
        Some((c, _)) if fighting_now || (c - mario).with_y(0.0).length() < FIT_NEAR => {
            ((((c - mario).with_y(0.0).length() - FIT_FROM) * FIT_PER_M).clamp(0.0, FIT_MAX) + FIT_BASE) * FIT_ZOOM[cam.zoom] * ks
        }
        _ => 0.0,
    };
    cam.fit += (fit_want - cam.fit) * if fit_want > cam.fit { rate(1.5) } else { rate(0.4) };
    let (dist, height) = (dist + cam.fit * (1.0 - 0.5 * enc), height + cam.fit * 0.3 * (1.0 - 0.5 * enc));
    // shut in: closer and lower (a room's walls would pull it in anyway, jerkily); running in the
    // open, a little farther back
    let (dist, height) = (dist * (1.0 - INDOOR_CLOSER * enc) + RUN_BACK * ks * speed_k * (1.0 - enc), height * (1.0 - INDOOR_LOWER * enc));
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
    // (indoors a beast fills the room: backing off far only finds the walls)
    let want_big = want_big * (1.0 - 0.5 * enc);
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
    // on a colossus: back by its size, sunk below him by how high he's up it (looking up at the
    // beast, Shadow of the Colossus's view), round on his side of it
    let colossus = *COLOSSUS.lock().unwrap_or_else(|e| e.into_inner());
    let want_coloss = colossus.map_or(0.0, |(_, h, r, _)| ((h * 0.55 + r * 0.6) * (1.0 - 0.5 * enc)).clamp(0.0, COLOSSUS_MAX));
    cam.coloss += (want_coloss - cam.coloss) * if want_coloss > cam.coloss { rate(1.2) } else { rate(0.5) };
    let want_sink = colossus.map_or(0.0, |(f, h, _, _)| ((mario.y - f.y).max(0.0) * COLOSSUS_SINK).min(h * 0.6));
    cam.coloss_sink += (want_sink - cam.coloss_sink) * rate(1.0);
    if let Some((f, _, _, top)) = colossus {
        let out = (mario - f).with_y(0.0);
        if out.length() > 0.3 && cam.target_yaw.is_none() && !manual {
            let want = out.z.atan2(out.x);
            cam.yaw += angle_diff(want, cam.yaw) * rate(0.8);
        }
        let lean = ((top - head) * 0.35).clamp_length_max(COLOSSUS_LEAN);
        cam.frame += (lean - cam.frame) * rate(1.5);
    }
    let height = height + cam.coloss * 0.15 - cam.coloss_sink;
    // (sunk, never below a man's height over the ground under him)
    let height = if cam.coloss_sink > 0.01 { height.max(ground + 1.2 - cam.focus.y) } else { height };
    let dist = dist + cam.coloss;
    // (the air fight: back a little, and lower relative to him, looking up at the fight)
    let (dist, height) = (dist + AIR_BACK * cam.air, height * (1.0 - AIR_LEVEL * cam.air));
    // the fight's framing: the view leans towards the enemy, so Mario and it share the screen
    // (more in the air: the one he juggles is up there)
    // (a group: back off a little for each one past the first, and lean to its middle)
    let want_crowd = if fighting_now || group.len() >= 3 { (group.len().saturating_sub(1) as f32 * CROWD_BACK).min(CROWD_MAX) * FIT_ZOOM[cam.zoom] * ks } else { 0.0 };
    cam.crowd += (want_crowd - cam.crowd) * if want_crowd > cam.crowd { rate(1.5) } else { rate(0.4) };
    let (dist, height) = (dist + cam.crowd * (1.0 - 0.5 * enc), height + cam.crowd * 0.35 * (1.0 - 0.5 * enc));
    let fight = match (crowd, fight) {
        (Some(c), Some((_, up))) if cam.air < 0.5 => Some((c, up)),
        _ => fight,
    };
    let want_fight = fight.map_or(Vec3::ZERO, |(c, _)| {
        let to = c - head;
        let lean = FIGHT_LEAN + (AIR_LEAN - FIGHT_LEAN) * cam.air;
        let v = Vec3::new(to.x * lean, to.y * (lean + 0.15 * cam.air), to.z * lean);
        v.clamp_length_max(FIGHT_FRAME_MAX)
    });
    cam.fight_frame += (want_fight - cam.fight_frame) * rate(2.0);
    // co-op: the partner close by shares the screen (a lean their way, a little farther back)
    let (want_mate, want_mate_frame) = MATE.lock().unwrap_or_else(|e| e.into_inner()).map_or((0.0, Vec3::ZERO), |c| {
        let d = (c - mario).with_y(0.0).length();
        let w = ((d - MATE_NEAR) / MATE_EASE).clamp(0.0, 1.0) * ((MATE_FAR - d) / MATE_EASE).clamp(0.0, 1.0);
        let w = w * if fighting_now { MATE_IN_FIGHT } else { 1.0 };
        let back = ((d - MATE_NEAR) * MATE_BACK_PER_M).clamp(0.0, MATE_BACK_MAX) * FIT_ZOOM[cam.zoom] * ks * w;
        let to = c - head;
        (back, Vec3::new(to.x, to.y * 0.5, to.z) * MATE_LEAN * w)
    });
    let want_mate_frame = want_mate_frame.clamp_length_max(MATE_FRAME_MAX);
    cam.mate += (want_mate - cam.mate) * if want_mate > cam.mate { rate(1.2) } else { rate(0.5) };
    cam.mate_frame += (want_mate_frame - cam.mate_frame) * rate(1.5);
    let (dist, height) = (dist + cam.mate * (1.0 - 0.5 * enc), height + cam.mate * 0.3 * (1.0 - 0.5 * enc));
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
    // under a low ceiling: below it, never pressed up against it
    let height = height.min((cam.ceiling_y - CEILING_GAP - cam.focus.y).max(0.25 * ks));
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
        hit(head, ideal).map(|h| ((h - cam.focus).length() - WALL_GAP).max(MIN_DIST * ks)).unwrap_or(full)
    };
    // something in the way (a column, a corner, a doorframe): a small turn either way that sees all
    // the way is better than diving in close; it eases round to the nearest one (in a fight too)
    if want < full * WHISKER_BLOCKED && cam.target_yaw.is_none() && !manual {
        let flat_len = offset.with_y(0.0).length();
        let free = WHISKERS.iter().copied().find(|&turn| {
            let y = cam.yaw + turn;
            let o = Vec3::new(y.cos() * flat_len, offset.y, y.sin() * flat_len);
            hit(head, cam.focus + o).is_none()
        });
        if let Some(turn) = free {
            cam.yaw += turn * rate(WHISKER_RATE);
        }
    }
    // pulled in close: up and looking down a little, not into the back of his head
    let close_lift = ((CLOSE_LIFT_FROM * ks - want) * 0.6).max(0.0).min((cam.ceiling_y - CEILING_GAP - cam.focus.y - height - cam.lift).max(0.0));
    // a wall close behind (the view pulled in to under SLIDE_BLOCKED of its length): look a few
    // turns either way for one that sees farther, and slide round to it
    // (not in a fight: the fight's framing turns it; not right after a slide)
    if cam.target_yaw.is_none() && !manual && cam.slide.is_none() && !fighting_now && cam.slid_ago >= SLIDE_DWELL {
        if cam.slide_wait > 0 {
            cam.slide_wait -= 1;
        } else if want < full * SLIDE_BLOCKED {
            cam.slide_wait = 4;
            let flat_len = offset.with_y(0.0).length();
            let best = SLIDE_TURNS
                .iter()
                .map(|&turn| {
                    let y = cam.yaw + turn;
                    let o = Vec3::new(y.cos() * flat_len, offset.y, y.sin() * flat_len);
                    let free = hit(head, cam.focus + o).map_or(full, |h| (h - cam.focus).length());
                    (free - turn.abs() * 0.8, y, free)
                })
                .max_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, y, free)) = best.filter(|b| b.2 > want + SLIDE_GAIN) {
                crate::dlog(format!("lakitu: wall behind ({want:.1} of {full:.1} m), sliding {:.0} deg ({free:.1} m free)", angle_diff(y, cam.yaw).to_degrees()));
                cam.slide = Some(y);
            }
        }
    }
    cam.dist += (want - cam.dist) * if want < cam.dist { rate(15.0) } else { rate(2.0) };
    let base = cam.focus + dir * cam.dist + Vec3::Y * close_lift;
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
    // walls and ceilings right by the lens (beside, behind or above it): nudged off them, eased
    {
        let right = Vec3::new(-dir.z, 0.0, dir.x).normalize_or(Vec3::X);
        let back = dir.with_y(0.0).normalize_or_zero();
        let mut push = Vec3::ZERO;
        for d in [right, -right, back, Vec3::Y] {
            if d == Vec3::ZERO {
                continue;
            }
            if let Some(h) = hit(pos, pos + d * LENS_PAD) {
                push -= d * (LENS_PAD - (h - pos).length()).max(0.0);
            }
        }
        cam.pad += (push - cam.pad) * rate(10.0);
        pos += cam.pad;
    }
    // never under the ground (the ray starts at head or camera height, inside the room: from
    // higher up it would find the roof in interiors and put the camera on top of it)
    let top = Vec3::new(pos.x, pos.y.max(head.y) + 0.1, pos.z);
    if let Some(g) = hit(top, pos - Vec3::Y * 1.0) {
        pos.y = pos.y.max(g.y + 0.4);
    }
    cam.pos = pos;

    // (the big body's framing wins over the fight's: both lean the view, the bigger counts)
    let (a, b, c) = (cam.frame.length(), cam.fight_frame.length(), cam.mate_frame.length());
    let frame = if a + b + c > 1e-4 { (cam.frame * a + cam.fight_frame * b + cam.mate_frame * c) / (a + b + c) } else { Vec3::ZERO };
    let fwd = (cam.focus + frame + cam.lead + Vec3::Y * cam.look_up - cam.pos).normalize_or(Vec3::Z);
    write(render, fwd, cam.pos);
}

/// The room around Mario: walls looked for this far (m), the ceiling this high above his head;
/// walls this close on average (the nearest half) are a tight room, this far the open; a ceiling
/// this close above his head is low, this far high; shut in, the camera comes this share closer and
/// this share lower, and stays this far (m) under the ceiling
const ROOM_PROBE: f32 = 9.0;
const CEILING_PROBE: f32 = 8.0;
const ROOM_TIGHT: f32 = 2.0;
const ROOM_OPEN: f32 = 7.0;
const CEILING_LOW: f32 = 1.5;
const CEILING_HIGH: f32 = 5.0;
const INDOOR_CLOSER: f32 = 0.3;
const INDOOR_LOWER: f32 = 0.45;
const CEILING_GAP: f32 = 0.45;
/// a wall pulling the view in to under this share of its length: it looks these turns either way
/// (radians) and slides round, at this rate, to one that sees at least this much farther (m)
const SLIDE_BLOCKED: f32 = 0.7;
const SLIDE_TURNS: [f32; 6] = [-1.2, -0.75, -0.4, 0.4, 0.75, 1.2];
const SLIDE_RATE: f32 = 2.2;
const SLIDE_GAIN: f32 = 1.0;
const SLIDE_DWELL: f32 = 1.5;
/// the second look for walls, this much higher (m)
const WALL_HIGH: f32 = 1.3;
/// nothing within this (m) of the lens
const LENS_PAD: f32 = 0.6;
/// a wall in the way: the camera stops this far (m) in front of it, never closer than this to
/// Mario's head (m); closer than this (m) it rises a little to look down at him
const WALL_GAP: f32 = 0.5;
const MIN_DIST: f32 = 1.0;
const CLOSE_LIFT_FROM: f32 = 2.2;
/// the view blocked to under this share of its length: these small turns (radians, nearest first)
/// are tried for a clear one, eased to at this rate
const WHISKER_BLOCKED: f32 = 0.85;
const WHISKERS: [f32; 6] = [0.22, -0.22, 0.45, -0.45, 0.7, -0.7];
const WHISKER_RATE: f32 = 3.0;
/// the right stick: a push under this share does nothing, a full one turns the camera this fast
/// (radians a second)
const ORBIT_DEAD: f32 = 0.18;
const ORBIT_SPEED: f32 = 2.6;
/// a fight (an enemy hit just now, or this close, m): the camera backs off this much (m) plus this
/// much per metre the enemy is farther than FIT_FROM (m), at most FIT_MAX, times the zoom step's
/// share (zoomed out, more)
const FIT_NEAR: f32 = 4.0;
const FIT_BASE: f32 = 0.6;
const FIT_FROM: f32 = 2.5;
const FIT_PER_M: f32 = 0.5;
const FIT_MAX: f32 = 3.0;
const FIT_ZOOM: [f32; 3] = [0.7, 1.0, 1.35];
/// the view leads where he runs by this long (s), at most this far (m); running flat out in the
/// open, the camera this much farther back (m)
const LEAD_TIME: f32 = 0.18;
const LEAD_MAX: f32 = 1.3;
const RUN_BACK: f32 = 0.7;

/// the chase camera: after a steady run this long (s) within this angle of the view (cos, about
/// 110 degrees: not running at the camera), it swings behind Mario at up to this rate
const CHASE_AFTER: f32 = 0.6;
const CHASE_COS: f32 = -0.35;
const CHASE_RATE: f32 = 1.1;
/// a group of enemies within this far (m): the camera backs off this much for each past the first
/// (m), at most this far
const CROWD_FAR: f32 = 9.0;
const CROWD_BACK: f32 = 0.6;
const CROWD_MAX: f32 = 2.4;
/// A big body's pull on the camera: how close (m, off its side) it starts and is full, the most it
/// backs off (m), and how far the view turns to it (m sideways, m up, at most)
const BIG_FAR: f32 = 14.0;
const BIG_NEAR: f32 = 3.0;
const BIG_MAX: f32 = 9.5;
const BIG_FRAME_SIDE: f32 = 2.4;
const BIG_FRAME_UP: f32 = 2.6;
/// a big body this close (m, off its side) and outside this cone (cos 60°) of the view pulls the
/// camera round, at most this fast (rate), not for this long after a C-button swing (s)
const ASSIST_FAR: f32 = 10.0;
const ASSIST_COS: f32 = 0.5;
const ASSIST_RATE: f32 = 1.4;
const ASSIST_HOLD: f32 = 2.5;
/// the fight's camera: an enemy within this (m, flat) is framed with Mario; the view leans this
/// share of the way to it (on the ground, in the air), at most this far (m); the camera turns to
/// it at this share of the big bodies' pull
const FIGHT_FAR: f32 = 10.0;
const FIGHT_LEAN: f32 = 0.22;
const AIR_LEAN: f32 = 0.42;
const FIGHT_FRAME_MAX: f32 = 2.6;
const FIGHT_ASSIST: f32 = 0.45;
/// ...for this long after one of his blows (s)
const FIGHT_HOT: f32 = 1.5;
/// an air fight: with an enemy above him this close (m), the camera follows his height this fast
/// (rate), backs off this much (m) and keeps this share less of its height above him
const AIR_NEAR: f32 = 6.0;
const AIR_FOLLOW: f32 = 6.0;
const AIR_BACK: f32 = 1.6;
const AIR_LEVEL: f32 = 0.55;

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

/// A finisher punches the camera in towards Mario a little (s, how far in m), then it eases back:
/// the blow's weight in the picture (Devil May Cry's, God of War's).
static PUNCH: Mutex<Option<(std::time::Instant, f32)>> = Mutex::new(None);
const PUNCH_IN: f32 = 0.06;
const PUNCH_OUT: f32 = 0.3;

/// Punches the camera in by `metres` (game thread, impact.rs); a bigger one isn't cut short.
pub fn punch(metres: f32) {
    let mut p = PUNCH.lock().unwrap_or_else(|e| e.into_inner());
    if p.is_none_or(|(t, m)| punch_now(t, m) < metres * 0.8) {
        *p = Some((std::time::Instant::now(), metres.min(0.6)));
    }
}

/// How far in the punch has the camera now (m): in fast, back out eased.
fn punch_now(since: std::time::Instant, metres: f32) -> f32 {
    let t = since.elapsed().as_secs_f32();
    if t < PUNCH_IN {
        metres * t / PUNCH_IN
    } else if t < PUNCH_IN + PUNCH_OUT {
        let k = 1.0 - (t - PUNCH_IN) / PUNCH_OUT;
        metres * k * k * (3.0 - 2.0 * k)
    } else {
        0.0
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
    let pos = match *PUNCH.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((since, m)) => pos + fwd * punch_now(since, m),
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

