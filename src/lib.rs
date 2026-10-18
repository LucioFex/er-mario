//! ER Mario: play Elden Ring with Super Mario 64's movement.
//!
//! Milestone 1: libsm64 runs Mario on a flat invisible floor under the player.
//! The Tarnished follows Mario, and Mario's real model is debug-drawn as a wireframe.

mod assets;
mod autopad;
mod audio;
mod blown;
mod boomerang;
mod hook;
mod parry;
mod ride;
mod caps;
mod carry;
mod chomp;
mod collision;
mod coins;
mod combat;
mod coop;
mod dodge;
mod engine_mario;
mod fireball;
mod grab;
mod equip;
mod kbd;
mod knock;
mod lakitu;
mod mega;
mod menu_mario;
mod havok_col;
mod explore;
mod gameover;
mod hammer;
mod hud;
mod impact;
mod juggle;
mod moving;
mod noise;
mod moves;
mod collision_geometry;
mod throw_collision;
mod trample;
mod names;
mod net;
mod notes;
mod pads;
mod save_backup;
mod shove;
mod paths;
mod perf;
mod sm64;
mod squish;
mod status;
mod swing;
mod stats;
mod timing;
mod traps;
mod rock;
mod bat;
mod maxfin;
mod skillshot;
mod stopwatch;
mod bossride;
mod style;
mod update;
mod version;
mod voice;
mod weapons;
mod worker;
mod yoshi;

use std::f32::consts::PI;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use eldenring::{
    cs::{CSCamExt, CSCamera, CSTaskGroupIndex, CSTaskImp, PlayerIns, RendMan, WorldChrMan},
    fd4::FD4TaskData,
    position::HavokPosition,
    rotation::Quaternion,
};
use fromsoftware_shared::{F32Vector4, FromStatic, SharedTaskImpExt};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::Input::XboxController::{
    XINPUT_GAMEPAD_A, XINPUT_GAMEPAD_B, XINPUT_GAMEPAD_LEFT_SHOULDER, XINPUT_GAMEPAD_LEFT_THUMB, XINPUT_GAMEPAD_RIGHT_SHOULDER, XINPUT_GAMEPAD_RIGHT_THUMB,
    XINPUT_GAMEPAD_BACK, XINPUT_GAMEPAD_START, XINPUT_GAMEPAD_X, XINPUT_GAMEPAD_Y,
    XINPUT_GAMEPAD_DPAD_DOWN, XINPUT_GAMEPAD_DPAD_LEFT, XINPUT_GAMEPAD_DPAD_RIGHT, XINPUT_GAMEPAD_DPAD_UP,
    XINPUT_STATE,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{PCSTR, w};

/// Metres per SM64 unit (Mario is ~160 units tall, so ~1.6 m).
/// SM64 units to metres: 0.01, more for Mega Mario (mega.rs: his whole world is scaled, so he's
/// bigger in it, collisions and reach and all)
static SCALE_BITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3C23_D70A);
pub(crate) const BASE_SCALE: f32 = 0.01;

pub(crate) fn scale() -> f32 {
    f32::from_bits(SCALE_BITS.load(Ordering::Relaxed))
}

/// A random share 0..1 (cues' volumes and picks, so the same sound again and again doesn't drone).
pub(crate) fn rand01() -> f32 {
    static SEED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x9E37_79B9);
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    SEED.store(x, Ordering::Relaxed);
    (x >> 8) as f32 / (1u32 << 24) as f32
}

pub(crate) fn set_scale(s: f32) {
    SCALE_BITS.store(s.to_bits(), Ordering::Relaxed);
}

/// How many times his normal size Mario is (Mega Mario).
pub(crate) fn size_factor() -> f32 {
    scale() / BASE_SCALE
}
/// The game's floor materials that are lava (ChrPhysicsMaterialInfo::hit_material): the
/// HitMtrlParam rows that put the lava burn (SpEffect 4101) on whoever stands on them. 7 is the
/// lava of Mt. Gelmir, the others are other places' (Rykard's arena among them).
const LAVA_MATERIALS: [i32; 6] = [7, 24, 27, 39, 47, 62];
/// Havok collision layers Mario collides with (terrain, buildings, props, ...).
const COLLISION_LAYERS: [u32; 11] = [0x1e, 0x2e, 0x37, 0x38, 0x39, 0x3a, 0x46, 0x47, 0x48, 0x49, 0x51];
/// Raycast filter for the ground probes.
const RAY_FILTER: u32 = 0x08;
/// Lifts Mario 1 m (out of places he's stuck in).
const VK_F1: i32 = 0x70;
const VK_F7: i32 = 0x76;

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Mario mode is wanted (always, unless something switched it off): until Mario is actually
/// posed (spawning, loading, respawning) the Tarnished stays invisible.
static WANTED: AtomicBool = AtomicBool::new(true);
/// Mario mode was switched on at launch (it waits for ground under the player first).
static AUTO_STARTED: AtomicBool = AtomicBool::new(false);
static DEBUG_DRAW: AtomicBool = AtomicBool::new(false);
/// A menu or popup is open: the game gets the whole pad, Mario gets none.
static MENU_OPEN: AtomicBool = AtomicBool::new(false);
static PAD: Mutex<Option<(XINPUT_STATE, std::time::Instant)>> = Mutex::new(None);

/// A diagnostic line: only in the log with `debug = 1` (players' logs stay short).
pub(crate) fn dlog(msg: impl AsRef<str>) {
    if debug() {
        log(msg);
    }
}

/// logs\er_mario.log in the mod folder (its own folder, so players find it to send it), started
/// fresh every launch; the previous session's stays as logs\er_mario.prev.log.
pub(crate) fn log(msg: impl AsRef<str>) {
    // Written by a thread of its own, through a file that stays open. Opening and closing the
    // log for every line on the game's thread could take milliseconds a time (antivirus looks
    // at every open), and some moments write several lines: a hitch each.
    static LINES: std::sync::OnceLock<Mutex<std::sync::mpsc::Sender<String>>> = std::sync::OnceLock::new();
    let lines = LINES.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let path = paths::file("logs/er_mario.log");
            let _ = std::fs::create_dir_all(paths::file("logs"));
            // (versions before 0.3.2 kept the log next to the DLL: it becomes the previous one)
            let (old, old_prev) = (paths::file("er_mario.log"), paths::file("er_mario.prev.log"));
            let previous = if old.is_file() { old } else { path.clone() };
            let _ = std::fs::rename(previous, paths::file("logs/er_mario.prev.log"));
            let _ = std::fs::remove_file(old_prev);
            let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(&path).ok();
            while let Ok(line) = rx.recv() {
                if file.is_none() {
                    file = OpenOptions::new().create(true).append(true).open(&path).ok();
                }
                if let Some(f) = file.as_mut() {
                    if writeln!(f, "{line}").is_err() {
                        file = None;
                    }
                }
            }
        });
        Mutex::new(tx)
    });
    let _ = lines.lock().unwrap_or_else(|e| e.into_inner()).send(msg.as_ref().to_string());
}

/// `debug = 1` in er_mario.ini: developer keys (F3-F6, F8-F12) and detailed logging.
pub(crate) fn debug() -> bool {
    static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DEBUG.get_or_init(|| paths::config("debug").is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")))
}

/// A developer key is held (only with debug on).
fn debug_key(vk: i32) -> bool {
    debug() && unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0
}

// ---- controller interception ---------------------------------------------------------------

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XINPUT_STATE) -> u32;

/// The real XInputGetState (or whatever was in the game's import table: Steam's overlay hook).
static XINPUT_ORIGINAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The game's XInputGetState, through its import table (see install_xinput_hooks).
unsafe extern "system" fn xinput_get_state(index: u32, state: *mut XINPUT_STATE) -> u32 {
    let original: XInputGetStateFn = unsafe { std::mem::transmute(XINPUT_ORIGINAL.load(Ordering::Relaxed)) };
    let rc = unsafe { original(index, state) };
    xinput_filter(index, state, rc)
}

/// The inline-hook fallback (only if the import table entry isn't found).
fn xinput_hook(reg: *mut ilhook::x64::Registers, original: usize) -> usize {
    let (index, state) = unsafe { ((*reg).rcx as u32, (*reg).rdx as *mut XINPUT_STATE) };
    let original: XInputGetStateFn = unsafe { std::mem::transmute(original) };
    let rc = unsafe { original(index, state) };
    xinput_filter(index, state, rc) as usize
}

/// Which sticks are Mario's right now, so the game must not see them: (left, right). Every way
/// the game reads a pad hides them (XInput here, DirectInput and libScePad in pads.rs): it
/// merges all its pads, and one it still saw walked the Tarnished under Mario.
pub(crate) fn hidden_sticks() -> (bool, bool) {
    if !ENABLED.load(Ordering::Relaxed) || !IN_WORLD.load(Ordering::Relaxed) {
        return (false, false);
    }
    if MENU_OPEN.load(Ordering::Relaxed) {
        // a menu screen the character can walk in (input_task): the left stick walks Mario only
        return (MENU_WALK.load(Ordering::Relaxed), false);
    }
    // buttons reach the game (menus need them; the Tarnished's actions are stripped in
    // input_task); the left stick is Mario's alone outside menus (on a ladder the game climbs,
    // on Torrent the game rides); with the SM64 camera the right stick is Lakitu's C-buttons,
    // not Elden Ring's camera
    (!ON_LADDER.load(Ordering::Relaxed) && !RIDING.load(Ordering::Relaxed), lakitu::ON.load(Ordering::Relaxed))
}

/// Real pad goes to Mario; the game gets an idle pad (right stick kept for the camera).
fn xinput_filter(index: u32, state: *mut XINPUT_STATE, rc: u32) -> u32 {
    let rc = autopad::apply(index, state, rc);
    if rc != 0 || state.is_null() || index > 3 {
        return rc;
    }
    let s = unsafe { &mut *state };
    pads::xinput_seen(index, s);
    // Mario plays with the pad in use: the last slot anything was pressed or pushed on (a pad
    // plugged back in, or with Steam's virtual one in slot 0, came up in slot 1 and Mario didn't
    // move). The game reads all four and merges them, so every one's sticks are hidden (Proton can
    // show one controller twice, e.g. Steam's virtual pad and the real one: the second one,
    // untouched, walked the Tarnished under Mario)
    static ACTIVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let g = &s.Gamepad;
    let used = g.wButtons.0 != 0
        || g.bLeftTrigger > 30
        || g.bRightTrigger > 30
        || [g.sThumbLX, g.sThumbLY, g.sThumbRX, g.sThumbRY].iter().any(|v| (*v as i32).abs() > 12000);
    if used && ACTIVE.swap(index, Ordering::Relaxed) != index {
        log(format!("pads: Mario follows controller {index}"));
    }
    if index == ACTIVE.load(Ordering::Relaxed) {
        *PAD.lock().unwrap_or_else(|e| e.into_inner()) = Some((*s, std::time::Instant::now()));
    }
    let (left, right) = hidden_sticks();
    let g = &mut s.Gamepad;
    if left {
        g.sThumbLX = 0;
        g.sThumbLY = 0;
    }
    if right {
        g.sThumbRX = 0;
        g.sThumbRY = 0;
    }
    if ENABLED.load(Ordering::Relaxed) && IN_WORLD.load(Ordering::Relaxed) && !MENU_OPEN.load(Ordering::Relaxed) {
        // RB is the fireball, RT the hook and L3 whistles for Torrent (input_task); as the game's
        // attack buttons, pressed in the same frame, they kept the whistle from being used (and L3
        // is the game's crouch)
        // (R3 is the parry and the grab, not the game's lock-on)
        g.wButtons &= !(XINPUT_GAMEPAD_RIGHT_SHOULDER | XINPUT_GAMEPAD_LEFT_THUMB | XINPUT_GAMEPAD_RIGHT_THUMB);
        g.bRightTrigger = 0;
        // L1 brings out Mario's hammer (moves.rs), not the game's guard
        g.wButtons &= !XINPUT_GAMEPAD_LEFT_SHOULDER;
        // Y is Mario's special moves; the game's interact (Y: doors, items, graces, NPCs) is on
        // D-pad down instead, which only takes his cap off (Y's moves waited for the game to start
        // an interaction, slow, and hit anyway)
        let interact = g.wButtons.contains(XINPUT_GAMEPAD_DPAD_DOWN);
        g.wButtons &= !XINPUT_GAMEPAD_Y;
        if interact {
            g.wButtons |= XINPUT_GAMEPAD_Y;
        }
        // the D-pad changes Mario's caps (caps.rs), not the game's items and spells
        g.wButtons &= !(XINPUT_GAMEPAD_DPAD_UP | XINPUT_GAMEPAD_DPAD_DOWN | XINPUT_GAMEPAD_DPAD_LEFT | XINPUT_GAMEPAD_DPAD_RIGHT);
        // X is Mario's punch and the game's "use item": the game only gets it for the whistle
        // (punching while getting off Yoshi whistled him back, over and over in a fight)
        if WHISTLING.load(Ordering::Relaxed) {
            g.wButtons |= XINPUT_GAMEPAD_X;
        } else {
            g.wButtons &= !XINPUT_GAMEPAD_X;
        }
        // Torrent goes where the game's own camera looks, which nobody sees with Lakitu's on:
        // the stick is turned by the angle between the two
        let turn = f32::from_bits(RIDE_TURN.load(Ordering::Relaxed));
        if RIDING.load(Ordering::Relaxed) && turn.is_finite() {
            let (x, y) = (g.sThumbLX as f32, g.sThumbLY as f32);
            let (sin, cos) = turn.sin_cos();
            g.sThumbLX = (x * cos + y * sin).clamp(-32767.0, 32767.0) as i16;
            g.sThumbLY = (y * cos - x * sin).clamp(-32767.0, 32767.0) as i16;
        }
    }
    rc
}

unsafe fn install_xinput_hooks() {
    // Swap the game's own import table entry for XInputGetState. Patching xinput's code instead
    // (an inline hook) clashed with Steam's overlay on Windows, which hooks the same function's
    // first bytes: the game crashed inside XINPUT1_4.dll. Through the import table, Steam's hook
    // (if any) stays in the chain as the "original" we call.
    match unsafe { patch_import(c"XInputGetState", 2, xinput_get_state as *const () as usize) } {
        Some((dll, previous)) => {
            XINPUT_ORIGINAL.store(previous, Ordering::Relaxed);
            log(format!("hooked XInputGetState through the game's import table ({dll})"));
            return;
        }
        None => log("XInputGetState not in the game's import table: falling back to an inline hook"),
    }
    use ilhook::x64::{CallbackOption, HookFlags, hook_closure_retn};
    let mut hooks = Vec::new();
    for dll in [w!("xinput1_4.dll"), w!("xinput1_3.dll")] {
        let Ok(module) = (unsafe { GetModuleHandleW(dll) }) else { continue };
        let Some(proc) = (unsafe { GetProcAddress(module, PCSTR(c"XInputGetState".as_ptr().cast())) }) else {
            continue;
        };
        match unsafe { hook_closure_retn(proc as usize, xinput_hook, CallbackOption::None, HookFlags::empty()) } {
            Ok(h) => {
                log(format!("hooked XInputGetState in {}", unsafe { dll.display() }));
                hooks.push(h);
            }
            Err(e) => log(format!("hook failed {}: {e:?}", unsafe { dll.display() })),
        }
    }
    std::mem::forget(hooks);
}

/// Replaces the game executable's import of `name` (or `ordinal`) from any xinput DLL with
/// `replacement`. Returns (dll name, the pointer that was there).
unsafe fn patch_import(name: &std::ffi::CStr, ordinal: u16, replacement: usize) -> Option<(String, usize)> {
    use windows::Win32::System::Memory::{PAGE_PROTECTION_FLAGS, PAGE_READWRITE, VirtualProtect};
    let base = unsafe { GetModuleHandleW(None) }.ok()?.0 as usize;
    let u32_at = |a: usize| unsafe { (a as *const u32).read_unaligned() };
    let nt = base + u32_at(base + 0x3C) as usize;
    if u32_at(nt) != 0x4550 {
        return None; // "PE\0\0"
    }
    // PE32+: optional header at +0x18, data directories at +0x70 into it; [1] = imports
    let imports = u32_at(nt + 0x18 + 0x70 + 8) as usize;
    if imports == 0 {
        return None;
    }
    let mut desc = base + imports;
    loop {
        let (lookup, dll_name, iat) = (u32_at(desc) as usize, u32_at(desc + 12) as usize, u32_at(desc + 16) as usize);
        if dll_name == 0 {
            return None;
        }
        let dll = unsafe { std::ffi::CStr::from_ptr((base + dll_name) as *const std::ffi::c_char) }.to_string_lossy().to_string();
        if dll.to_ascii_lowercase().starts_with("xinput") {
            let names = if lookup != 0 { lookup } else { iat };
            for k in 0.. {
                let entry = unsafe { ((base + names + k * 8) as *const u64).read_unaligned() };
                if entry == 0 {
                    break;
                }
                let hit = if entry & (1 << 63) != 0 {
                    (entry & 0xFFFF) as u16 == ordinal
                } else {
                    // IMAGE_IMPORT_BY_NAME: u16 hint, then the name
                    let n = unsafe { std::ffi::CStr::from_ptr((base + entry as usize + 2) as *const std::ffi::c_char) };
                    n == name
                };
                if hit {
                    let slot = (base + iat + k * 8) as *mut usize;
                    let mut old = PAGE_PROTECTION_FLAGS(0);
                    unsafe { VirtualProtect(slot as *const _, 8, PAGE_READWRITE, &mut old) }.ok()?;
                    let previous = unsafe { slot.read() };
                    unsafe { slot.write(replacement) };
                    let mut back = PAGE_PROTECTION_FLAGS(0);
                    let _ = unsafe { VirtualProtect(slot as *const _, 8, old, &mut back) };
                    return Some((dll, previous));
                }
            }
        }
        desc += 20;
    }
}

// ---- Mario --------------------------------------------------------------------------------

struct MarioState {
    id: i32,
    filter: u32,
    ticks: u32,
    no_ground: u32,
    surfaces: Vec<sm64::SM64Surface>,
    havok: havok_col::HavokCollision,
    wall_memory: std::collections::VecDeque<Vec<collision::WorldTri>>,
    /// where Mario mode was switched on (safe spot to return to)
    home: [f32; 3],
    /// ER position of SM64's (0, 0, 0); shifted as Mario travels so the floor never ends
    origin: [f32; 3],
    acc: f32,
    state: sm64::SM64MarioState,
    /// Mario's skinned mesh (9 floats per triangle), copied back from the libsm64 thread
    mesh: Vec<f32>,
    mesh_color: Vec<f32>,
    mesh_normal: Vec<f32>,
    /// previous tick, for smoothing SM64's 30 Hz up to the game's frame rate
    prev_mesh: Vec<f32>,
    prev_pos: [f32; 3],
    last_set: Option<[f32; 3]>,
    last_query: Option<(glam::Vec3, u32)>,
    last_query_havok: bool,
    /// Mario's body parts relative to him (this tick and the previous one), for the engine model
    parts: Option<[engine_mario::PartPose; engine_mario::PARTS]>,
    combat: combat::Combat,
    /// Mario's fireball (RB)
    fire: fireball::Fire,
    parry: parry::Parry,
    rang: boomerang::Boomerang,
    hook: hook::Hook,
    /// the dash (B) and the special moves (Y)
    moves: moves::Moves,
    /// the trap kit's things out there (LT + X / Y / B / RB)
    traps: traps::Traps,
    /// what the combos' moves did on the last tick
    combo: sm64::SM64ErMove,
    /// the Tarnished died: Mario plays SM64's death until the game respawns the player
    dead: bool,
    /// lifts, doors and other moving collision as SM64 surface objects
    moving: moving::Moving,
    /// ticks the player pushed the stick while Mario didn't move (stuck inside geometry)
    stuck_ticks: u32,
    prev_parts: Option<[engine_mario::PartPose; engine_mario::PARTS]>,
}

static MARIO: Mutex<Option<MarioState>> = Mutex::new(None);
/// This DLL's module handle.
static MODULE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Mario's head-turn direction in SM64's model (its space is mirrored against the game's)
const HEAD_YAW_SIGN: f32 = -1.0;
const HEAD_PITCH_SIGN: f32 = -1.0;
/// SOUND_GENERAL_COIN, SOUND_GENERAL_HEART_SPIN
const SOUND_COIN: i32 = 0x3811_8081;
const SOUND_HEART: i32 = 0x3064_C081;
const SOUND_METAL_BONK: i32 = 0x0442_8081;
/// Elden Ring's HUD in Mario mode: its HP / FP / stamina bars mean nothing (the power meter is
/// Mario's health), and nothing at all shows over the game-over screen; menus keep their own HUD
/// states. Runs after the menu manager, before the HUD is drawn.
fn hud_task() {
    let _span = perf::span(perf::HUD);
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    names::class_name();
    // character creation: the Vagabond's preview is Mario too
    // (in the world too: the save's picture in the pause menu is taken of a menu model)
    let in_world = IN_WORLD.load(Ordering::Relaxed);
    // (not tied to Mario mode being on: immediately after a load it isn't yet, and the menu can
    // already be opened)
    let active = WANTED.load(Ordering::Relaxed) && assets::ready() && SM64_READY.load(Ordering::Relaxed) && (in_world || equip::menu_mario());
    engine_mario::ACTIVE.store(active, Ordering::Relaxed);
    let menu = active && engine_mario::menu_models();
    menu_mario::tick(menu && !in_world);
    if active && in_world {
        engine_mario::stand_for_picture();
    }
    let Ok(fe) = (unsafe { eldenring::cs::CSFeManImp::instance_mut() }) else { return };
    use eldenring::cs::CSFeManHudState as Hud;
    let mario = ENABLED.load(Ordering::Relaxed) && IN_WORLD.load(Ordering::Relaxed);
    // HideAll: nothing of Elden Ring's HUD. It also hides the subtitle display, and a dialogue line
    // can only be skipped while it's shown, so during a conversation (the game stops taking the
    // character's actions, but no menu is up and the world runs) it's PopupMenu: subtitles on.
    // the subtitle box is up, or was a moment ago (the gap between two lines)
    let line = {
        static SEEN: Mutex<Option<std::time::Instant>> = Mutex::new(None);
        let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        // the box only comes up once the HUD lets it, so a new talk event opens the HUD first
        static EVENTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let events = talk_events();
        if subtitle_up() || EVENTS.swap(events, Ordering::Relaxed) != events {
            *seen = Some(std::time::Instant::now());
        }
        seen.is_some_and(|t| t.elapsed().as_secs_f32() < SUBTITLE_GAP)
    };
    let talking = line || (MENU_OPEN.load(Ordering::Relaxed) && !game_menu_open() && !WORLD_PAUSED.load(Ordering::Relaxed));
    {
        static WAS: AtomicBool = AtomicBool::new(false);
        if WAS.swap(talking, Ordering::Relaxed) != talking {
            log(format!("hud: conversation {}", if talking { "started: subtitles on" } else { "over" }));
        }
    }
    let hidden = if talking { Hud::PopupMenu } else { Hud::HideAll };
    if mario {
        // Elden Ring's own bars are hidden (the enemies' health bars are the overlay's, hud.rs)
        if matches!(fe.hud_state, Hud::Default) || (HIDDEN.load(Ordering::Relaxed) && fe.hud_state != hidden && !matches!(fe.hud_state, Hud::Default) && !gameover::showing() && !game_menu_open()) {
            fe.hud_state = hidden;
            HIDDEN.store(true, Ordering::Relaxed);
        }
        if gameover::showing() {
            fe.hud_state = Hud::HideAll;
            fe.frontend_values.enable_equip_hud = false;
            HIDDEN.store(true, Ordering::Relaxed);
        }
    } else if HIDDEN.swap(false, Ordering::Relaxed) && fe.hud_state == hidden {
        fe.hud_state = Hud::Default;
    }
}

/// Whether the game has a menu or prompt up (the pause menu, the "revive at the Stake of Marika?"
/// question...): its popup menu has a current top menu job then, and none in normal play.
pub(crate) fn game_menu_open() -> bool {
    game_menu_job() != 0
}

/// The popup menu's current top menu job (which menu screen is up), 0 if none.
fn game_menu_job() -> usize {
    unsafe { eldenring::cs::CSMenuManImp::instance() }
        .ok()
        .and_then(|m| m.popup_menu)
        .map(|p| unsafe { *(((p.as_ptr() as usize) + 0xB0) as *const usize) })
        .unwrap_or(0)
}

/// UI element 2 in the menu manager's table is the subtitle box.
const UI_SUBTITLE: usize = 2;
const SUBTITLE_GAP: f32 = 3.0;

fn subtitle_up() -> bool {
    unsafe { eldenring::cs::CSMenuManImp::instance() }.is_ok_and(|m| m.ui_states[UI_SUBTITLE].visible())
}

/// Counts up with every talk or menu the popup menu starts.
fn talk_events() -> u32 {
    unsafe { eldenring::cs::CSMenuManImp::instance() }.ok().and_then(|m| m.popup_menu).map(|p| unsafe { *((p.as_ptr() as usize + 0x168) as *const u32) }).unwrap_or(0)
}

static WORLD_PAUSED: AtomicBool = AtomicBool::new(false);
/// The game's menu is open and the game is walking the character (input_task).
static MENU_WALK: AtomicBool = AtomicBool::new(false);

fn handle_key_of(h: &eldenring::cs::FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<eldenring::cs::FieldInsHandle, u64>(h) }
}

/// Whether a cutscene is playing: the world paused for 0.5 s with no menu or popup up for the last
/// 1.5 s (popups pause too, but their menu shows up a few frames after the pause and goes a moment
/// before it ends: without the margins Mario vanished around every popup), and not a loading
/// screen (anim -1).
fn cutscene_now(dead: bool, player: &PlayerIns) -> bool {
    static PAUSED_SINCE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    static MENU_SEEN: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let now = std::time::Instant::now();
    let mut menu_seen = MENU_SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if game_menu_open() || MENU_OPEN.load(Ordering::Relaxed) {
        *menu_seen = Some(now);
    }
    let mut since = PAUSED_SINCE.lock().unwrap_or_else(|e| e.into_inner());
    if !WORLD_PAUSED.load(Ordering::Relaxed) {
        *since = None;
        return false;
    }
    let paused_for = now.duration_since(*since.get_or_insert(now)).as_secs_f32();
    let menu_recently = menu_seen.is_some_and(|t| t.elapsed().as_secs_f32() < 1.5);
    !dead && paused_for >= 0.5 && !menu_recently && current_anim(&player.chr_ins) != -1
}

/// Whether another character within 40 m has advanced its animation in the last 0.3 s (false if
/// nobody is near: the Tarnished's own clock decides then).
fn others_animating(player: &PlayerIns) -> bool {
    static CLOCKS: Mutex<Vec<(u64, i32, f32)>> = Mutex::new(Vec::new());
    static LAST_MOVE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let me = player.chr_ins.modules.physics.position;
    let main = handle_key_of(&player.chr_ins.field_ins_handle);
    let mut clocks = CLOCKS.lock().unwrap_or_else(|e| e.into_inner());
    let mut seen = Vec::with_capacity(clocks.len());
    let mut changed = false;
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &eldenring::cs::ChrIns = chr;
            let key = handle_key_of(&chr.field_ins_handle);
            if key == main {
                continue;
            }
            let p = chr.modules.physics.position;
            let (dx, dy, dz) = (p.0 - me.0, p.1 - me.1, p.2 - me.2);
            if dx * dx + dy * dy + dz * dz > 40.0 * 40.0 {
                continue;
            }
            let t = &chr.modules.time_act;
            let a = &t.anim_queue[(t.read_idx % 10) as usize];
            match clocks.iter().find(|c| c.0 == key) {
                Some(&(_, id, time)) if id != a.anim_id || time != a.play_time => changed = true,
                _ => {}
            }
            seen.push((key, a.anim_id, a.play_time));
        }
    }
    *clocks = seen;
    let mut last = LAST_MOVE.lock().unwrap_or_else(|e| e.into_inner());
    if changed {
        *last = Some(std::time::Instant::now());
    }
    last.is_some_and(|t| t.elapsed().as_secs_f32() < 0.3)
}

/// Whether the game world is paused (tutorial and other popups that stop the game): the
/// Tarnished's animation clock stands still while our frame keeps running. Short hit-stops in
/// combat are well under the threshold.
fn world_paused(player: &PlayerIns) -> bool {
    static CLOCK: Mutex<Option<(i32, f32, std::time::Instant)>> = Mutex::new(None);
    let t = &player.chr_ins.modules.time_act;
    let a = &t.anim_queue[(t.read_idx % 10) as usize];
    let mut clock = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    // a hit that launches the Tarnished into the air stalls his animation until he lands, and
    // he only lands when Mario moves him: in the air a stalled clock is not a pause
    let airborne = !player.chr_ins.modules.physics.is_touching_ground;
    let still = match *clock {
        Some((id, time, since)) if id == a.anim_id && time == a.play_time => since.elapsed().as_secs_f32() > 0.3 && !airborne,
        _ => {
            *clock = Some((a.anim_id, a.play_time, now));
            false
        }
    };
    // ...but his clock also stalls in his plain idle (anim 0) while the world runs on (Elden Ring's
    // menu doesn't pause, and enemies kept attacking a frozen Mario): if anyone nearby is still
    // animating, the world isn't paused
    let still = still && !others_animating(player);
    // back to running only once the clock has kept going for a moment (cutscenes nudge it)
    static MOVING_SINCE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let mut moving = MOVING_SINCE.lock().unwrap_or_else(|e| e.into_inner());
    let paused = if still {
        *moving = None;
        true
    } else if WORLD_PAUSED.load(Ordering::Relaxed) {
        let since = *moving.get_or_insert(now);
        since.elapsed().as_secs_f32() < 0.3
    } else {
        false
    };
    if paused && !WORLD_PAUSED.load(Ordering::Relaxed) {
        if let Ok(wcm) = unsafe { WorldChrMan::instance() } {
            let main = handle_key_of(&player.chr_ins.field_ins_handle);
            let mine = player.chr_ins.character_id;
            let p = player.chr_ins.modules.physics.position;
            for set in wcm.chr_sets.iter().flatten() {
                for chr in set.characters() {
                    let chr: &eldenring::cs::ChrIns = chr;
                    let q = chr.modules.physics.position;
                    let d = ((q.0 - p.0).powi(2) + (q.2 - p.2).powi(2)).sqrt();
                    if chr.character_id == mine && d < 60.0 {
                        crate::dlog(format!(
                            "pause: player-model character {:?} at {d:.1} m, main {}",
                            chr.chr_type,
                            handle_key_of(&chr.field_ins_handle) == main
                        ));
                    }
                }
            }
        }
    }
    if WORLD_PAUSED.swap(paused, Ordering::Relaxed) != paused {
        log(format!("world {} (anim {} clock {:.3})", if paused { "paused" } else { "running again" }, a.anim_id, a.play_time));
    }
    paused
}

/// A boss died: a star (counted once per boss, both the boss's health and the "FELLED" banner
/// report it) that refills Mario's health like in SM64. False if this boss already gave one.
fn boss_star() -> bool {
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed().as_secs_f32() < 30.0) {
        return false;
    }
    *last = Some(std::time::Instant::now());
    stats::update(|s| s.stars += 1);
    REST.store(true, Ordering::Relaxed);
    log("star collected");
    true
}

/// SM64's action/animation per follow tick (diagnostics).
static FOLLOW_TRACE: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// SM64 ticks run (all, and during follow mode), for the debug tick-rate lines.
static SM64_TICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static FOLLOW_TICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// A game-driven animation just started (follow mode loads its floors).
static FOLLOW_STARTED: AtomicBool = AtomicBool::new(false);
/// When a game-driven animation (fog wall, door) last ended.
/// When a follow (door, fog wall, ladder, grace) ended or Mario was created, and where his feet
/// were then (SM64 units): a safety floor stays there for a moment while the area streams in. It
/// stays at that height: put at his current position on every reload, it caught him mid-jump.
static FOLLOW_ENDED: Mutex<Option<(std::time::Instant, [f32; 3])>> = Mutex::new(None);

/// A flat floor under a point (SM64 units), 20 m across.
fn flat_floor(p: [f32; 3]) -> [sm64::SM64Surface; 2] {
    let (x, y, z, e) = (p[0] as i32, p[1] as i32, p[2] as i32, 2000);
    [
        sm64::SM64Surface::grass([[x - e, y, z - e], [x + e, y, z + e], [x + e, y, z - e]]),
        sm64::SM64Surface::grass([[x - e, y, z - e], [x - e, y, z + e], [x + e, y, z + e]]),
    ]
}

/// Sets the Tarnished's HP.
fn set_player_hp(hp: i32) {
    if let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) {
        p.chr_ins.modules.data.hp = hp;
    }
}

/// Deathblight never builds up on Mario (its gauge is kept full): an instant death has no place on
/// a health meter of eight wedges. The other ailments do, and Mario shows them (status.rs).
fn no_ailments() {
    use eldenring::cs::GameDataMan;
    // the gauges in the order of the resistances: poison, rot, blood loss, deathblight, frost,
    // sleep (5, left out), madness
    // (only deathblight now: the rest build up and Mario shows them, status.rs)
    const BUILD_UPS: [usize; 1] = [status::BLIGHT];
    // The character's resist module (module container +0x20): +0x10 the seven gauges (what's
    // left of each resistance: they run down as an ailment builds up, and it sets in at 0),
    // +0x2c their maxima. The game data has the same numbers, but only as a copy for the menus.
    const GAUGES: usize = 0x10;
    const MAXIMA: usize = 0x2c;
    let Some(player) = (unsafe { WorldChrMan::instance() }).ok().and_then(|w| w.main_player.as_ref()) else { return };
    let Ok(gdm) = (unsafe { GameDataMan::instance() }) else { return };
    let max = gdm.main_player_game_data.resistance_gauge_max;
    let module = unsafe { *((&*player.chr_ins.modules as *const _ as usize + 0x20) as *const usize) };
    if max[0] == 0 || !explore::readable(module, 0x50) {
        return;
    }
    // (only where the module is laid out as expected)
    if unsafe { *((module + MAXIMA) as *const [u32; 7]) } != max {
        return;
    }
    for i in BUILD_UPS {
        unsafe { *((module + GAUGES + i * 4) as *mut u32) = max[i] };
    }
}

/// Set when the Tarnished sits down at a site of grace (Mario's health refills).
static REST: AtomicBool = AtomicBool::new(false);
static SM64_READY: AtomicBool = AtomicBool::new(false);

fn to_er(origin: [f32; 3], p: [f32; 3]) -> HavokPosition {
    // SM64 and Havok are mirrored on X
    HavokPosition(origin[0] - p[0] * scale(), origin[1] + p[1] * scale(), origin[2] + p[2] * scale(), 0.0)
}

/// Starts libsm64 with the player's ROM (once). With `export`, also takes Mario's model from it
/// (for the asset builder) before the audio starts.
fn init_sm64(export: bool) -> Option<Option<assets::model::MarioModel>> {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if SM64_READY.load(Ordering::Relaxed) {
        return Some(None);
    }
    let rom = paths::read_rom().map_err(|e| log(e)).ok()?;
    gameover::load_mask(&rom);
    hud::load(&rom);
    let model = worker::call_timeout("init", std::time::Duration::from_secs(10), move |ctx| {
        let mut tex = vec![0u8; 4 * sm64::TEXTURE_W * sm64::TEXTURE_H];
        unsafe { sm64::sm64_global_init(rom.as_ptr(), tex.as_mut_ptr()) };
        let model = if export { assets::model::export(&mut ctx.geo, tex) } else { None };
        menu_mario::still(&mut ctx.geo);
        unsafe { sm64::sm64_audio_init(rom.as_ptr()) };
        model
    })?;
    log("libsm64 initialised");
    audio::start();
    SM64_READY.store(true, Ordering::Relaxed);
    Some(model)
}

/// At launch: start libsm64 and build the package files if they are missing or outdated.
fn startup() {
    let build = assets::check();
    log(format!("mod folder {}", paths::mod_dir().display()));
    log(format!("package folder {}", paths::package_dir().display()));
    update::start();
    // (not on the setup launch: the game is closed right after it)
    if !build {
        notes::start();
    }
    if build {
        hud::setup_progress("Setting up ER Mario", 0.0, "Reading the ROM");
    }
    match init_sm64(build) {
        None => {
            hud::set_setup(None);
            notify("ER Mario needs a Super Mario 64 ROM (US version).\n\nPut your .z64 file in the mod folder, or set rom = ... in er_mario.ini, then restart the game.", "MARIO NEEDS A SUPER MARIO 64 ROM (SEE README)")
        }
        Some(Some(model)) => match assets::build(&model) {
            Ok(()) => built_restart(),
            Err(e) => {
                log(format!("assets: build failed: {e}"));
                hud::set_setup(None);
                notify("ER Mario could not build its files, see logs\\er_mario.log", "MARIO COULD NOT BE BUILT (SEE THE LOGS FOLDER)");
            }
        },
        Some(None) if build => notify("ER Mario could not read Mario's model from the ROM", "MARIO COULD NOT BE BUILT (SEE ER_MARIO.LOG)"),
        Some(None) => {}
    }
}

/// Any controller button, trigger, key or mouse button down right now (game window focused).
fn any_button_down() -> bool {
    let pad = PAD
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .filter(|(_, t)| t.elapsed().as_secs_f32() < 0.25)
        .is_some_and(|(s, _)| s.Gamepad.wButtons.0 != 0 || s.Gamepad.bLeftTrigger > 100 || s.Gamepad.bRightTrigger > 100);
    let keys = kbd::focused() && (1..=0xFE).any(|vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0);
    pad || keys
}

/// Mario's files are built, but me3 only picks up the package folder when the game starts: the
/// player starts the game again themselves. At the title screen the setup box says so and the next
/// button press closes the game (nothing is saved there; a normal exit would run the whole
/// shutdown, so the process just ends). Already in the world: the box and a banner ask for a
/// restart instead.
fn built_restart() {
    log("ER Mario built Mario from your ROM; the game has to be started again once");
    if !IN_WORLD.load(Ordering::Relaxed) {
        hud::set_setup(Some(hud::Setup {
            title: "Setup complete".into(),
            progress: Some(1.0),
            text: "Please start the game again with er-mario.me3.\n\nPress any button to close the game.".into(),
        }));
        // a press that started after the box appeared (not one held from before)
        while any_button_down() {
            std::thread::sleep(Duration::from_millis(50));
        }
        while !IN_WORLD.load(Ordering::Relaxed) {
            if any_button_down() {
                log("closing the game after the setup");
                use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
                let _ = unsafe { TerminateProcess(GetCurrentProcess(), 0) };
                std::process::exit(0);
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    hud::set_setup(Some(hud::Setup {
        title: "Setup complete".into(),
        progress: None,
        text: "Please quit and start the game again with er-mario.me3.".into(),
    }));
    // (the box stays until the player is in the world for a while; the banner repeats it there)
    std::thread::spawn(|| {
        while !IN_WORLD.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(500));
        }
        std::thread::sleep(Duration::from_secs(15));
        hud::set_setup(None);
    });
    notify("ER Mario setup is complete. Restart the game to finish it.", "RESTART THE GAME TO FINISH THE SETUP");
}

/// A message for the player: logged, and shown as a big banner once they are in the world.
fn notify(text: &str, banner: &'static str) {
    log(text.replace('\n', " "));
    *BANNER.lock().unwrap_or_else(|e| e.into_inner()) = Some(banner);
}

static BANNER: Mutex<Option<&'static str>> = Mutex::new(None);

/// Shows a pending banner through the game's "MAP FOUND" banner (its text swapped for a moment).
fn show_banner() {
    static SHOWN: Mutex<Option<(std::time::Instant, Vec<(usize, u64)>)>> = Mutex::new(None);
    let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, patches)) = shown.as_ref() {
        if t.elapsed().as_secs_f32() > 10.0 {
            names::restore(patches);
            *shown = None;
        }
        return;
    }
    let Some(text) = BANNER.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let patches = names::override_text("MAP FOUND", text);
    if let Ok(menu) = unsafe { eldenring::cs::CSMenuManImp::instance_mut() } {
        menu.display_status_message(eldenring::cs::STATUS_MESSAGE_MAP_FOUND);
    }
    *shown = Some((std::time::Instant::now(), patches));
}

/// Game collision (live Havok triangles) around Mario as SM64 surfaces.
const PLAYER_MOVE_FILTER: u32 = 0x1704;

/// Asks the game itself whether a triangle is collision the player really collides with:
/// a 20 cm ray through its centre with the player's movement filter must hit it.
fn game_confirms(player: &PlayerIns, t: &havok_col::Tri) -> bool {
    // Two rays per triangle, hundreds of triangles a query: the same static triangles came back
    // query after query and fast combat moves (a refresh every 3 ticks) made 30-60 ms hitches.
    // Answers are kept by the triangle's exact place; a "no" is asked again after a few seconds
    // (collision streaming in late), a "yes" for two minutes.
    type Answers = std::collections::HashMap<[u32; 9], (bool, std::time::Instant)>;
    static ANSWERS: Mutex<Option<(Answers, std::time::Instant)>> = Mutex::new(None);
    let key = [t[0].x, t[0].y, t[0].z, t[1].x, t[1].y, t[1].z, t[2].x, t[2].y, t[2].z].map(f32::to_bits);
    let mut guard = ANSWERS.lock().unwrap_or_else(|e| e.into_inner());
    let (answers, swept) = guard.get_or_insert_with(|| (Answers::new(), std::time::Instant::now()));
    if answers.len() > 200_000 || swept.elapsed().as_secs_f32() > 30.0 {
        answers.retain(|_, (ok, at)| *ok && at.elapsed().as_secs_f32() < 120.0);
        *swept = std::time::Instant::now();
    }
    if let Some((ok, at)) = answers.get(&key) {
        if *ok || at.elapsed().as_secs_f32() < 3.0 {
            return *ok;
        }
    }
    let ok = game_confirms_with(player, t, PLAYER_MOVE_FILTER);
    answers.insert(key, (ok, std::time::Instant::now()));
    ok
}

fn game_confirms_with(player: &PlayerIns, t: &havok_col::Tri, filter: u32) -> bool {
    let Ok(havok) = (unsafe { eldenring::cs::CSHavokMan::instance() }) else { return true };
    let c = (t[0] + t[1] + t[2]) / 3.0;
    let n = (t[1] - t[0]).cross(t[2] - t[0]);
    if n.length_squared() < 1e-8 {
        return false;
    }
    let n = n.normalize() * 0.1;
    for (from, dir) in [(c + n, -n * 2.0), (c - n, n * 2.0)] {
        let start = HavokPosition(from.x, from.y, from.z, 0.0);
        let delta = eldenring::position::PositionDelta(dir.x, dir.y, dir.z);
        if let Some(hit) = havok.phys_world.cast_ray(filter, &start, delta, player) {
            if glam::Vec3::new(hit.0, hit.1, hit.2).distance(c) < 0.15 {
                return true;
            }
        }
    }
    false
}

fn havok_surfaces(h: &mut havok_col::HavokCollision, origin: [f32; 3], mario: [f32; 3], player: &PlayerIns) -> Option<Vec<sm64::SM64Surface>> {
    let c = collision::sm_to_er(origin, mario);
    let mut tris = {
        let _span = perf::span(perf::HAVOK_QUERY);
        h.query(glam::Vec3::new(c.0, c.1, c.2))?
    };
    let before = tris.len();
    static COMPARED: AtomicBool = AtomicBool::new(false);
    if !COMPARED.swap(true, Ordering::Relaxed) {
        // offset check: long vertical ray through flat triangles, compare real surface height
        if let Ok(havok) = unsafe { eldenring::cs::CSHavokMan::instance() } {
            let mut rows = Vec::new();
            for (t, _, body) in tris.iter() {
                let n = (t[1] - t[0]).cross(t[2] - t[0]);
                if n.length_squared() < 1e-8 || n.normalize().y.abs() < 0.9 {
                    continue;
                }
                let c = (t[0] + t[1] + t[2]) / 3.0;
                let start = HavokPosition(c.x, c.y + 1.0, c.z, 0.0);
                let hit = havok.phys_world.cast_ray(0x08, &start, eldenring::position::PositionDelta(0.0, -2.0, 0.0), player);
                rows.push(match hit {
                    Some(h) => format!("#{body} dy={:+.3}", h.1 - c.y),
                    None => format!("#{body} miss"),
                });
                if rows.len() >= 40 {
                    break;
                }
            }
            crate::dlog(format!("height check (real surface minus decoded, m): {}", rows.join(", ")));
        }
    }
    let mut per_body: std::collections::BTreeMap<u32, (u32, u32, u32)> = Default::default();
    let confirmed: Vec<_> = tris
        .iter()
        .filter(|(t, layer, body)| {
            // layer 0x37 (detailed map collision the character walks on) and 0x1e (physics props) are
            // invisible to the game's rays, so the ray check can't confirm them: trust them
            let ok = *layer == 0x37 || *layer == 0x1e || h.is_boxed(*body) || game_confirms(player, t);
            let e = per_body.entry(*body).or_insert((*layer, 0, 0));
            e.1 += 1;
            e.2 += ok as u32;
            ok
        })
        .cloned()
        .collect();
    if confirmed.is_empty() && before > 0 {
        log("game raycast check confirmed nothing (filter wrong?): keeping all triangles");
    } else {
        tris = confirmed;
    }
    static ORACLE_LOGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    if ORACLE_LOGS.fetch_add(1, Ordering::Relaxed) % 20 == 0 {
        crate::dlog(format!("game raycast check: kept {} of {before} triangles ({} bodies skipped so far: not in the physics world)", tris.len(), h.not_in_world));
        let rows: Vec<String> = per_body.iter().map(|(b, (l, n, k))| format!("#{b}(L{l:x}) {k}/{n}")).collect();
        crate::dlog(format!("  per body confirmed/picked: {}", rows.join(", ")));
    }
    if tris.is_empty() {
        // A lift may be the only nearby geometry. Its surface object supplies the floor;
        // raycast fallback here would recreate a stationary copy of the moving deck.
        return (!h.exclude.is_empty()).then(Vec::new);
    }
    let mut out = Vec::with_capacity(tris.len());
    let m = glam::Vec3::from(mario);
    // diagnostic: which way does the ground right under Mario face with Havok's own winding?
    static WINDING_LOGGED: AtomicBool = AtomicBool::new(false);
    if !WINDING_LOGGED.swap(true, Ordering::Relaxed) {
        let (mut up, mut down) = (0, 0);
        for (t, _, _) in &tris {
            let s = t.map(|p| glam::Vec3::from(collision::er_to_sm(origin, &HavokPosition(p.x, p.y, p.z, 0.0))));
            let cen = (s[0] + s[1] + s[2]) / 3.0;
            if (cen.x - m.x).abs() < 200.0 && (cen.z - m.z).abs() < 200.0 && (cen.y - m.y).abs() < 100.0 {
                // mirrored X + swapped winding = Havok's own facing in SM64 space
                let n = (s[2] - s[0]).cross(s[1] - s[2]);
                if n.length_squared() > 0.0 {
                    let ny = n.normalize().y;
                    if ny > 0.7 { up += 1 } else if ny < -0.7 { down += 1 }
                }
            }
        }
        crate::dlog(format!("winding check: ground under Mario with Havok winding: {up} facing up, {down} facing down"));
        if let Ok(h) = unsafe { eldenring::cs::CSHavokMan::instance() } {
            log(format!("CSHavokMan at {:#x}", h as *const _ as usize));
        }
    }
    // per body, worked out once (it was per triangle: a few lookups each, and a small mesh's
    // closed-shape test built its edge table again for every one of its triangles): whether it's
    // convex or boxed, and its middle (SM64) if it has an inside
    let mut bodies: std::collections::HashMap<u32, (bool, Option<[f32; 3]>)> = std::collections::HashMap::new();
    let triangles = perf::span(perf::TRIANGLES);
    for (t, layer, body) in &tris {
        let (solid, mid) = *bodies.entry(*body).or_insert_with(|| {
            let solid = h.is_convex(*body) || h.is_boxed(*body);
            let mesh = h.mesh_of(*body);
            let mid = (solid || mesh.as_ref().is_some_and(|m| m.small_closed())).then(|| {
                let mesh = mesh.as_ref()?;
                let (p, q) = h.transform(*body)?;
                let local = mesh.tris().iter().flatten().copied().sum::<glam::Vec3>() / (mesh.tris().len() * 3) as f32;
                let world = q * local + p;
                Some(collision::er_to_sm(origin, &HavokPosition(world.x, world.y, world.z, 0.0)))
            }).flatten();
            (solid, mid)
        });
        let v = t.map(|p| {
            collision::er_to_sm(origin, &HavokPosition(p.x, p.y, p.z, 0.0)).map(|x| x.round() as i32)
        });
        let Some(v) = collision_geometry::body_surface_vertices(v, mid, mario, solid) else { continue };
        let mut surf = sm64::SM64Surface::grass(v);
        surf.force = *layer as i16;
        out.push(surf);
    }
    // moving objects (a lift's floor) are surface objects, not in `out`: no patches under them
    let moving: Vec<(glam::Vec3, glam::Vec3)> = h
        .exclude
        .iter()
        .filter_map(|&i| {
            let (mesh, (t, q)) = (h.mesh_of(i)?, h.transform(i)?);
            let v = mesh.tris().iter().flatten().map(|v| q * *v + t);
            let lo = v.clone().fold(glam::Vec3::splat(f32::MAX), |a, b| a.min(b));
            let hi = v.fold(glam::Vec3::splat(f32::MIN), |a, b| a.max(b));
            Some((lo - glam::Vec3::splat(0.5), hi + glam::Vec3::splat(0.5)))
        })
        .collect();
    drop(triangles);
    {
        let _span = perf::span(perf::FLOOR_PATCHES);
        floor_patches(&mut out, origin, mario, player, &moving);
    }
    Some(out)
}

/// Floors the game's map ray finds around Mario that our collision is missing (meshes built from
/// custom pieces, like the Volcano Manor drawbridge, can't be decoded): a small flat patch there.
/// Not inside `moving` (world boxes of the moving objects): a patch under a lift's floor stayed
/// behind when it went down, and Mario stood on it while the cage's roof came through him.
fn floor_patches(out: &mut Vec<sm64::SM64Surface>, origin: [f32; 3], mario: [f32; 3], player: &PlayerIns, moving: &[(glam::Vec3, glam::Vec3)]) {
    // grid around Mario (SM64 units) and how far up/down the ray looks (metres)
    const STEP: f32 = 75.0;
    const HALF: i32 = 4;
    let Ok(havok) = (unsafe { eldenring::cs::CSHavokMan::instance() }) else { return };
    let floors: Vec<[glam::Vec3; 3]> = out
        .iter()
        .filter(|s| !collision::is_wall(s))
        .map(|s| s.vertices.map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)))
        .filter(|t| (t[1] - t[0]).cross(t[2] - t[1]).y > 0.0)
        .collect();
    // floor heights of our own collision at (x, z)
    let ours = |x: f32, z: f32| {
        floors.iter().filter_map(move |t| {
            let p = glam::Vec2::new(x, z);
            let [a, b, c] = t.map(|v| glam::Vec2::new(v.x, v.z));
            let d = (b - a).perp_dot(c - a);
            if d.abs() < 1e-3 {
                return None;
            }
            let u = (b - p).perp_dot(c - p) / d;
            let v = (c - p).perp_dot(a - p) / d;
            let w = 1.0 - u - v;
            (u >= -0.01 && v >= -0.01 && w >= -0.01).then(|| t[0].y * u + t[1].y * v + t[2].y * w)
        })
    };
    let mut added = 0;
    for gx in -HALF..=HALF {
        for gz in -HALF..=HALF {
            let (x, z) = (mario[0] + gx as f32 * STEP, mario[2] + gz as f32 * STEP);
            let top = collision::sm_to_er(origin, [x, mario[1] + 200.0, z]);
            let Some(hit) = havok.phys_world.cast_ray(RAY_FILTER, &top, eldenring::position::PositionDelta(0.0, -8.0, 0.0), player) else { continue };
            let at = glam::Vec3::new(hit.0, hit.1, hit.2);
            if moving.iter().any(|(lo, hi)| at.cmpge(*lo).all() && at.cmple(*hi).all()) {
                continue;
            }
            let y = collision::er_to_sm(origin, &HavokPosition(hit.0, hit.1, hit.2, 0.0))[1];
            if ours(x, z).any(|h| (h - y).abs() < 40.0) {
                continue;
            }
            let r = STEP / 2.0 + 5.0;
            let q = |dx: f32, dz: f32| [(x + dx).round() as i32, y.round() as i32, (z + dz).round() as i32];
            // two triangles facing up (the facing is settled like any other floor)
            for v in [[q(-r, -r), q(r, -r), q(r, r)], [q(-r, -r), q(r, r), q(-r, r)]] {
                if let Some(v) = collision_geometry::surface_vertices(v, None, Some(mario)) {
                    let mut surf = sm64::SM64Surface::grass(v);
                    surf.force = 0xfe;
                    out.push(surf);
                }
            }
            added += 1;
        }
    }
    if added > 0 {
        crate::dlog(format!("floor patches: {added} from the game's map ray"));
    }
}

/// Which eye texture SM64 is drawing (cells 5 open, 6 half, 7 closed, 8 dead): the median
/// texture cell of this frame's eye triangles.
fn eye_cell(uv: &[f32], used: usize) -> u8 {
    let mut eyes: Vec<f32> = (0..used)
        .filter_map(|t| {
            let u = [uv[t * 6], uv[t * 6 + 2], uv[t * 6 + 4]];
            let mean = (u[0] + u[1] + u[2]) / 3.0;
            let span = u.iter().fold(f32::MIN, |a, &b| a.max(b)) - u.iter().fold(f32::MAX, |a, &b| a.min(b));
            (span > 1e-4 && (5.0 / 11.0..9.0 / 11.0).contains(&mean)).then_some(mean)
        })
        .collect();
    eyes.sort_by(f32::total_cmp);
    eyes.get(eyes.len() / 2).map(|m| (m * 11.0) as u8).unwrap_or(5)
}

/// Debug: a loaded wall Mario's centre crossed front to back between two ticks (at his wall check
/// heights), with his distance to it before and after.
fn crossed_wall(surfaces: &[sm64::SM64Surface], a: [f32; 3], b: [f32; 3]) -> Option<(sm64::SM64Surface, f32, f32)> {
    for s in surfaces.iter().filter(|s| collision::is_wall(s)) {
        let [p0, p1, p2] = s.vertices.map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
        let n = (p1 - p0).cross(p2 - p1);
        if n.length_squared() < 1e-6 {
            continue;
        }
        let n = n.normalize();
        for h in [30.0, 60.0] {
            let (pa, pb) = (glam::Vec3::new(a[0], a[1] + h, a[2]), glam::Vec3::new(b[0], b[1] + h, b[2]));
            let (da, db) = (n.dot(pa - p0), n.dot(pb - p0));
            if da <= 0.0 || db >= 0.0 {
                continue;
            }
            // where the move crosses the plane: inside the triangle?
            let x = pa + (pb - pa) * (da / (da - db));
            let inside = [(p0, p1), (p1, p2), (p2, p0)].iter().all(|(u, v)| n.dot((*v - *u).cross(x - *u)) >= -1.0);
            if inside {
                return Some((*s, da, db));
            }
        }
    }
    None
}

/// Mario's world scaled `k` times his own size (mega.rs), around where he stands: re-centred on him,
/// what lives in its old units dropped, its collision read again at the new scale.
fn rescale(m: &mut MarioState, k: f32, player_ref: &PlayerIns) {
    let here = collision::sm_to_er(m.origin, m.state.position);
    set_scale(BASE_SCALE * k);
    log(format!("mega: world scale x{k} at {:?}", (here.0, here.1, here.2)));
    m.moving.clear(&mut m.havok);
    m.origin = [here.0, here.1, here.2];
    set_mario_position(m.id, [0.0, 0.0, 0.0]);
    let id = m.id;
    worker::call("mega still", move |_| unsafe { sm64::sm64_set_mario_velocity(id, 0.0, 0.0, 0.0) });
    if m.state.action == moves::ACT_ER_ZIP {
        worker::call("mega zip off", move |_| unsafe { sm64::sm64_set_mario_action(id, ACT_FREEFALL) });
    }
    m.state.position = [0.0, 0.0, 0.0];
    m.prev_pos = [0.0, 0.0, 0.0];
    m.fire.drop_shots();
    m.hook.drop();
    m.last_query = None;
    m.wall_memory.clear();
    *FOLLOW_ENDED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let caster = collision::Caster { filter: m.filter, origin: m.origin, player: player_ref };
    let surfaces = havok_surfaces(&mut m.havok, m.origin, [0.0, 0.0, 0.0], player_ref).unwrap_or_else(|| collision::build(&caster, [0.0, 0.0, 0.0]));
    if !surfaces.is_empty() {
        load_surfaces(&surfaces);
        m.surfaces = surfaces;
    }
}

/// Whether SM64 lets Mario stand where he is (at the world's centre, just re-centred): its floor
/// and ceiling at least his height apart.
fn giant_fits() -> bool {
    worker::call("mega fits", |_| unsafe {
        let floor = sm64::sm64_surface_find_floor_height(0.0, 100.0, 0.0);
        let mut hit: *mut std::ffi::c_void = std::ptr::null_mut();
        let ceil = sm64::sm64_surface_find_ceil(0.0, floor + 1.0, 0.0, &mut hit);
        ceil - floor >= 170.0
    })
    .unwrap_or(true)
}

fn load_surfaces(surfaces: &[sm64::SM64Surface]) {
    let s = surfaces.to_vec();
    worker::call("load surfaces", move |_| unsafe { sm64::sm64_static_surfaces_load(s.as_ptr(), s.len() as u32) });
}

fn set_mario_position(id: i32, p: [f32; 3]) {
    worker::call("set position", move |_| unsafe { sm64::sm64_set_mario_position(id, p[0], p[1], p[2]) });
}

/// Draws Mario's mesh as filled, lit, vertex-coloured triangles with the game's debug renderer.
#[allow(dead_code)]
fn draw_mario(m: &MarioState, alpha: f32) {
    use eldenring::cs::EzDrawFillMode;
    use fromsoftware_shared::Triangle;
    let Some(draw) = unsafe { RendMan::instance_mut() }.ok().map(|r| r.debug_ez_draw.as_mut()) else { return };
    let tris = m.mesh.len() / 9;
    if tris == 0 || m.mesh_color.len() < tris * 9 || m.mesh_normal.len() < tris * 9 {
        return;
    }
    let light = glam::Vec3::new(0.3, 1.0, 0.4).normalize();
    // group triangles by (quantised) colour to keep colour changes down
    let mut groups: std::collections::HashMap<[u8; 3], Vec<usize>> = std::collections::HashMap::new();
    for t in 0..tris {
        let i = t * 9;
        let c = glam::Vec3::new(
            (m.mesh_color[i] + m.mesh_color[i + 3] + m.mesh_color[i + 6]) / 3.0,
            (m.mesh_color[i + 1] + m.mesh_color[i + 4] + m.mesh_color[i + 7]) / 3.0,
            (m.mesh_color[i + 2] + m.mesh_color[i + 5] + m.mesh_color[i + 8]) / 3.0,
        );
        // SM64 space is mirrored on X relative to the game
        let n = glam::Vec3::new(
            -(m.mesh_normal[i] + m.mesh_normal[i + 3] + m.mesh_normal[i + 6]),
            m.mesh_normal[i + 1] + m.mesh_normal[i + 4] + m.mesh_normal[i + 7],
            m.mesh_normal[i + 2] + m.mesh_normal[i + 5] + m.mesh_normal[i + 8],
        )
        .normalize_or_zero();
        let shade = 0.45 + 0.55 * n.dot(light).max(0.0);
        let q = |v: f32| ((v * shade).clamp(0.0, 1.0) * 31.0).round() as u8;
        groups.entry([q(c.x), q(c.y), q(c.z)]).or_default().push(t);
    }
    draw.set_fill_mode(EzDrawFillMode::Fill);
    for (col, list) in groups {
        draw.set_color(&F32Vector4(col[0] as f32 / 31.0, col[1] as f32 / 31.0, col[2] as f32 / 31.0, 1.0));
        for t in list {
            let lerp_ok = m.prev_mesh.len() == m.mesh.len();
            let p = |k: usize| {
                let i = t * 9 + k * 3;
                let cur = glam::Vec3::new(m.mesh[i], m.mesh[i + 1], m.mesh[i + 2]);
                let v = if lerp_ok {
                    let prev = glam::Vec3::new(m.prev_mesh[i], m.prev_mesh[i + 1], m.prev_mesh[i + 2]);
                    if prev.distance(cur) < 200.0 { prev.lerp(cur, alpha) } else { cur }
                } else {
                    cur
                };
                let h = to_er(m.origin, v.into());
                glam::Vec3::new(h.0, h.1, h.2)
            };
            let (a, b, c) = (p(0), p(1), p(2));
            let v4 = |v: glam::Vec3| F32Vector4(v.x, v.y, v.z, 0.0);
            draw.draw_triangle(&Triangle { origin: v4(a), edge1: v4(b - a), edge2: v4(c - a) });
        }
    }
}

/// F11: debug Mario (hide the Tarnished, draw Mario's libsm64 mesh with the debug renderer).
static EZ_MARIO: AtomicBool = AtomicBool::new(false);

/// Runs after the game's animation, before rendering: F11 toggle + engine Mario pose.
fn pose_task() {
    let _span = perf::span(perf::POSE);
    static WAS: AtomicBool = AtomicBool::new(false);
    let f11 = debug_key(0x7A);
    if f11 && !WAS.swap(true, Ordering::Relaxed) {
        let on = !EZ_MARIO.load(Ordering::Relaxed);
        EZ_MARIO.store(on, Ordering::Relaxed);
        log(format!("debug mario {}", if on { "ON" } else { "OFF" }));
    } else if !f11 {
        WAS.store(false, Ordering::Relaxed);
    }
    // co-op partners as Mario or Luigi (coop.rs)
    coop::apply_remote();
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    squish::apply();
    let Some(player) = (unsafe { WorldChrMan::instance() }).ok().and_then(|w| w.main_player.as_ref()) else { return };
    engine_mario::apply(&player.chr_ins as *const _ as usize);
    yoshi::apply();
}

fn pose_task_late() {
    let _span = perf::span(perf::POSE_LATE);
    // no Tarnished while Mario is on his way (spawning, loading in, respawning)
    // (only when Mario mode is really on its way: not when it went off by itself, e.g. first
    // launch without the built files, no ROM, or the no-ground safety)
    let coming = ENABLED.load(Ordering::Relaxed)
        || (assets::ready() && (!AUTO_STARTED.load(Ordering::Relaxed) || SM64_READY.load(Ordering::Relaxed)));
    // (and never for long: after 5 s without Mario the Tarnished shows again)
    static WAITING_SINCE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let no_pose = engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()).is_none();
    let too_long = {
        let mut w = WAITING_SINCE.lock().unwrap_or_else(|e| e.into_inner());
        if !no_pose {
            *w = None;
        }
        w.get_or_insert_with(std::time::Instant::now).elapsed().as_secs_f32() > 5.0
    };
    if WANTED.load(Ordering::Relaxed) && coming && !worker::hung() && no_pose && !too_long {
        if let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) {
            p.chr_ins.opacity_keyframes_multiplier = 0.0;
            p.chr_ins.opacity_keyframes_multiplier_previous = 0.0;
        }
    }
    // in a cutscene the player isn't rendered at all (opacity is the cutscene's): written in every
    // task group up to Draw_Pre, so the cutscene can't turn it back on in between; switched back
    // on once when it ends
    {
        static WAS_HIDDEN: AtomicBool = AtomicBool::new(false);
        let hide = CUTSCENE_HIDE.load(Ordering::Relaxed) && ENABLED.load(Ordering::Relaxed);
        let was = WAS_HIDDEN.swap(hide, Ordering::Relaxed);
        if hide || was {
            if let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) {
                p.chr_ins.chr_flags1c5.set_enable_render(!hide);
            }
        }
    }
    // co-op partners as Mario or Luigi (coop.rs)
    coop::apply_remote();
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    squish::apply();
    let Some(player) = (unsafe { WorldChrMan::instance() }).ok().and_then(|w| w.main_player.as_ref()) else { return };
    engine_mario::apply(&player.chr_ins as *const _ as usize);
    yoshi::apply();
}

/// A cutscene is playing: the player isn't rendered (pose_task_late).
static CUTSCENE_HIDE: AtomicBool = AtomicBool::new(false);


/// Set by input_task when the player pressed interact; frame() then watches for an event animation.
static INTERACT_PRESSED: AtomicBool = AtomicBool::new(false);
/// ...and what the game made of it then: interact allowed by the Tarnished's animation, blocked,
/// queued (a door once didn't open on Y: logged when no event animation follows)
static INTERACT_STATE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Elden Ring's ladder animations (getting on, climbing, sliding, getting off).
fn ladder_anim(anim: i32) -> bool {
    (28000..29000).contains(&anim) || (51100..51200).contains(&anim)
}

/// A game-driven animation Mario follows: events (6xxxx: fog walls, doors, levers) and ladders.
fn game_driven(anim: i32) -> bool {
    (60000..70000).contains(&anim) || ladder_anim(anim)
}

/// SM64 action for climbing an Elden Ring ladder (libsm64 patch: the pole climb, moved by the game).
const ACT_ER_LADDER: u32 = 0x0000035F;
/// all of SM64's sound while the wing cap is on
const WING_VOLUME: f32 = 0.65;
const ACT_ER_RIDE: u32 = 0x0000035E;
const ACT_FREEFALL: u32 = 0x0100088C;

/// Mario is following the Tarnished through a game-driven animation (fog wall, door, ladder...).
/// The Tarnished's last animation outside the event range (what he returns to after one).
static LAST_FREE_ANIM: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static HANDS_OFF: AtomicBool = AtomicBool::new(false);
/// F6 experiment mode: 0 normal, 1 gravity/fall left to the game, 2 also no proxy teleport request
static EXPERIMENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// A player character exists (in the world, not on the title screen / loading).
static IN_WORLD: AtomicBool = AtomicBool::new(false);

/// Whether the player is in the world (not the title screen, a menu before loading, or a load).
pub(crate) fn in_world() -> bool {
    IN_WORLD.load(Ordering::Relaxed)
}/// The loadout from before Mario mode (restored when it's switched off; kept across respawns).
static SAVED_LOADOUT: Mutex<Option<equip::Loadout>> = Mutex::new(None);
/// Loadout to put back shortly after Mario mode ended (after the model reload).
static PENDING_RESTORE: Mutex<Option<(std::time::Instant, equip::Loadout)>> = Mutex::new(None);
/// When the last Mario creation failed (no floor yet): retry after a moment.
static CREATE_RETRY: Mutex<Option<std::time::Instant>> = Mutex::new(None);
static RETURN_HOME: AtomicBool = AtomicBool::new(false);
static FOLLOWING: AtomicBool = AtomicBool::new(false);
/// Mario is on Torrent (or getting on): the game rides, like it walks him through a door, and
/// the buttons a rider needs reach it.
static RIDING: AtomicBool = AtomicBool::new(false);
/// Yoshi is being ridden with the keyboard (Elden Ring's own camera shows then).
static KEY_RIDE: AtomicBool = AtomicBool::new(false);
/// Mario is whistling for Torrent: the game gets its "use item" button pressed.
static WHISTLING: AtomicBool = AtomicBool::new(false);
/// Riding with Lakitu's camera: the angle (f32 bits) the left stick is turned by, NaN otherwise.
static RIDE_TURN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x7fc0_0000);
/// The Tarnished is on a ladder: the left stick (and WASD) climb it, so they go to the game.
static ON_LADDER: AtomicBool = AtomicBool::new(false);

/// Runs right after the game turned the pad into character actions (ChrIns_PreBehaviorSafe):
/// detects menus (buttons pressed but nothing reaches the character) and strips every action but
/// interact from the Tarnished, so he never rolls, attacks or jumps on Mario's buttons.
fn input_task() {
    let _span = perf::span(perf::INPUT);
    if !ENABLED.load(Ordering::Relaxed) {
        MENU_OPEN.store(false, Ordering::Relaxed);
        return;
    }
    // (a captured enemy's attack goes in now: after its AI, before its behaviour)
    bossride::pre_behavior();
    let Some(player) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) else { return };
    let pad = PAD.lock().unwrap_or_else(|e| e.into_inner()).filter(|(_, t)| t.elapsed().as_secs_f32() < 0.25).map(|(p, _)| p);
    // getting on or off Torrent, or whistling for him (the whistle's animations)
    let ride_busy = {
        let cur = current_anim(&player.chr_ins);
        mount_anim(cur) || matches!(cur, 50190 | 50191)
    };
    let mounted = player.chr_ins.modules.ride.is_mounted;
    let req: &mut eldenring::cs::CSChrActionRequestModule = &mut player.chr_ins.modules.action_request;
    let bits = |a: &mut eldenring::cs::ChrActions| unsafe { &mut *(a as *mut _ as *mut u64) };
    let routed = *bits(&mut req.action_requests) != 0 || req.movement_request_duration > 0.0;
    // Elden Ring lets the character walk with some menus open (its main menu): the first time the game
    // asks for movement in a menu screen, that screen is noted, and from then on the left stick there
    // walks Mario and no longer reaches the game (the game walking the Tarnished as well fought
    // Mario's position and swung Lakitu's camera around). Other screens (inventory...) keep the stick.
    {
        static WALK_JOB: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let job = game_menu_job();
        let walks = job != 0 && (WALK_JOB.load(Ordering::Relaxed) == job || req.movement_request_duration > 0.0);
        if walks {
            WALK_JOB.store(job, Ordering::Relaxed);
        }
        MENU_WALK.store(walks, Ordering::Relaxed);
    }
    let menu = MENU_OPEN.load(Ordering::Relaxed);
    let (mut opener, mut pressed) = (false, false);
    // alt-tabbed: the game ignores the pad but we still read it, which looked like a menu eating
    // every press (Mario froze until the next press got through)
    if let Some(p) = pad.filter(|_| kbd::focused()) {
        let g = p.Gamepad;
        let b = g.wButtons;
        let stick = (g.sThumbLX as i32).abs() > 12000 || (g.sThumbLY as i32).abs() > 12000;
        pressed |= b.contains(XINPUT_GAMEPAD_A)
            || b.contains(XINPUT_GAMEPAD_B)
            || b.contains(XINPUT_GAMEPAD_X)
            || b.contains(XINPUT_GAMEPAD_DPAD_DOWN)
            || b.contains(XINPUT_GAMEPAD_LEFT_SHOULDER)
            || g.bLeftTrigger > 100
            || g.bRightTrigger > 100
            || (menu && stick);
        opener |= b.contains(XINPUT_GAMEPAD_START) || b.contains(XINPUT_GAMEPAD_BACK);
    }
    // Start or Select that opened no menu (seen once: the pad's Start did nothing until the game
    // was minimised and restored): logged with the focus, to tell what the game was waiting for
    {
        static START_AT: Mutex<Option<(std::time::Instant, bool)>> = Mutex::new(None);
        static START_HELD: AtomicBool = AtomicBool::new(false);
        let start = pad.is_some_and(|p| p.Gamepad.wButtons.contains(XINPUT_GAMEPAD_START) || p.Gamepad.wButtons.contains(XINPUT_GAMEPAD_BACK));
        let mut at = START_AT.lock().unwrap_or_else(|e| e.into_inner());
        if start && !START_HELD.swap(true, Ordering::Relaxed) && !menu {
            *at = Some((std::time::Instant::now(), kbd::focused()));
        }
        START_HELD.store(start, Ordering::Relaxed);
        if menu || game_menu_open() {
            *at = None;
        } else if let Some((t, focused)) = *at {
            if t.elapsed().as_secs_f32() > 1.0 {
                *at = None;
                log(format!(
                    "input: Start/Select pressed, no menu after 1 s (game window in front: {} then, {} now; world paused: {})",
                    focused,
                    kbd::focused(),
                    WORLD_PAUSED.load(Ordering::Relaxed)
                ));
            }
        }
    }
    if kbd::focused() {
        let key = |vk: i32| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
        // Esc (menu), G (map); E interact, Space, F, R, Q, Enter, mouse buttons, and WASD in menus
        opener |= key(0x1B) || key(0x47);
        pressed |= [0x45, 0x20, 0x46, 0x52, 0x51, 0x0D, 0x01, 0x02].into_iter().any(key)
            || (menu && [0x57, 0x41, 0x53, 0x44].into_iter().any(key));
    }
    // the game's own "a menu is up" (pause menu, prompts): the most reliable signal, both ways
    static GAME_MENU: AtomicBool = AtomicBool::new(false);
    let game_menu = game_menu_open();
    if game_menu {
        opener = true;
    } else if GAME_MENU.load(Ordering::Relaxed) && menu {
        log("input: game menu closed");
        MENU_OPEN.store(false, Ordering::Relaxed);
    }
    GAME_MENU.store(game_menu, Ordering::Relaxed);
    // a menu is only guessed from a fresh press: some held buttons (crouch) only reach the
    // character on some frames, and holding one flipped between "menu" and "back in game" every
    // few frames. Back in game takes anything held (the stick, still held after unpausing).
    static HELD: AtomicBool = AtomicBool::new(false);
    let fresh = pressed && !HELD.swap(pressed, Ordering::Relaxed);
    let flip = if menu { pressed && routed } else { fresh && !routed };
    if opener {
        if !menu {
            log("input: menu opened");
        }
        MENU_OPEN.store(true, Ordering::Relaxed);
    } else if flip {
        // in a menu the game stops feeding the character; in gameplay it always does
        log(format!("input: {}", if routed { "back in game" } else { "menu/popup" }));
        MENU_OPEN.store(!routed, Ordering::Relaxed);
    }
    const ACTION: u64 = 1 << 4; // interact (doors, chests, graces, messages...)
    // research (debug): which action bits a press makes, to find what NPC dialogue listens to
    if debug() {
        static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let presses = *bits(&mut req.new_action_presses) | *bits(&mut req.action_requests) | *bits(&mut req.queued_action_inputs);
        if presses != 0 && LAST.swap(presses, Ordering::Relaxed) != presses {
            log(format!(
                "input: actions new {:#x} requests {:#x} queued {:#x} (possible {:#x})",
                *bits(&mut req.new_action_presses),
                *bits(&mut req.action_requests),
                *bits(&mut req.queued_action_inputs),
                *bits(&mut req.possible_action_inputs)
            ));
        } else if presses == 0 {
            LAST.store(0, Ordering::Relaxed);
        }
    }
    if *bits(&mut req.new_action_presses) & ACTION != 0 {
        INTERACT_PRESSED.store(true, Ordering::Relaxed);
        let flag = |a: &mut eldenring::cs::ChrActions, bit: u32| if *bits(a) & ACTION != 0 { bit } else { 0 };
        let state = flag(&mut req.possible_action_inputs, 1) | flag(&mut req.disabled_action_inputs, 2) | flag(&mut req.queued_action_inputs, 4);
        INTERACT_STATE.store(state, Ordering::Relaxed);
    }
    // on Torrent the game needs the rider's buttons: dash (5), jump (6) and getting off (13)
    const USE_ITEM: u64 = 1 << 7;
    const RIDER: u64 = 1 << 5 | 1 << 6 | 1 << 13;
    // ("use item", the whistle, only while Mario whistles, see below: kept on Torrent, a punch
    // while getting off was queued and whistled him back as soon as Mario was down)
    let keep = ACTION | if mounted { RIDER } else { 0 } | if WHISTLING.load(Ordering::Relaxed) { USE_ITEM } else { 0 };
    for a in [
        &mut req.action_requests,
        &mut req.new_action_presses,
        &mut req.queued_action_inputs,
        &mut req.cancel_ready_actions,
    ] {
        *bits(a) &= keep;
    }
    // L3 (or R): Mario uses the item in the quick slot, the Spectral Steed Whistle if it's there
    // (RB is the fireball, RT the hook)
    {
        static HELD: AtomicBool = AtomicBool::new(false);
        let rb = kbd::focused()
            && !MENU_OPEN.load(Ordering::Relaxed)
            && (pad.is_some_and(|p| p.Gamepad.wButtons.contains(XINPUT_GAMEPAD_LEFT_THUMB))
                || unsafe { GetAsyncKeyState(0x52) } as u16 & 0x8000 != 0);
        // The key is held for a few frames. Not while he's getting on or off (a second whistle
        // then breaks it off half way).
        static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let busy = ride_busy;
        let giant = mega::active() || mega::changing();
        if rb && giant && !HELD.swap(rb, Ordering::Relaxed) {
            // (Yoshi can't carry a giant: Lakitu's buzz)
            worker::call("whistle buzz", |_| unsafe { sm64::sm64_play_sound_global(0x700E_0081) });
        } else if rb && !giant && !HELD.swap(rb, Ordering::Relaxed) && !busy && equip::select_whistle() {
            log("input: whistle (use item) requested");
            FRAMES.store(8, Ordering::Relaxed);
            if !mounted {
                yoshi::call();
            }
        } else if !rb {
            HELD.store(false, Ordering::Relaxed);
        }
        if busy && FRAMES.swap(0, Ordering::Relaxed) > 1 {
            kbd::use_item_key(false);
        }
        // Asked for through the game's own "use item" buttons (X on the pad, R), which are let
        // through for these frames: a request only written here comes too late in the frame.
        let left = FRAMES.load(Ordering::Relaxed);
        WHISTLING.store(left > 0, Ordering::Relaxed);
        if left > 0 {
            if left == 8 || left == 1 {
                kbd::use_item_key(left == 8);
            }
            FRAMES.store(left - 1, Ordering::Relaxed);
        }
    }
}

/// On Yoshi Mario sits lower than the saddle the game seats him on.
fn seat(mut parts: [engine_mario::PartPose; engine_mario::PARTS]) -> [engine_mario::PartPose; engine_mario::PARTS] {
    if RIDING.load(Ordering::Relaxed) {
        let drop = yoshi::seat_drop();
        for p in &mut parts {
            p.pos.y -= drop;
        }
    }
    parts
}

/// Lava that does nothing, for the fights where the boss stands in it (`on`): the lava floors
/// lose the effects they put on whoever stands on them (the burn among them, SpEffect 4101),
/// and get them back when the fight is over. Returns `on`.
fn safe_lava(on: bool) -> bool {
    type Effects = [i32; 7];
    static SAVED: Mutex<Option<Vec<(i32, Effects)>>> = Mutex::new(None);
    /// Rykard's arena burns by an effect of its own (11954): every 1.1 s it sets off a fire
    /// attack on whoever has it, 59 HP a time. The attack it sets off, while it's taken out.
    const ARENA_BURN: u32 = 11954;
    static BURN: Mutex<Option<i32>> = Mutex::new(None);
    {
        let mut burn = BURN.lock().unwrap_or_else(|e| e.into_inner());
        if on != burn.is_some() {
            if let Some(row) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }).ok().and_then(|r| r.get_mut::<eldenring::cs::SpEffectParam>(ARENA_BURN)) {
                match burn.take() {
                    Some(own) => row.set_behavior_id(own),
                    None => {
                        *burn = Some(row.behavior_id());
                        row.set_behavior_id(-1);
                    }
                }
            }
        }
    }
    let mut saved = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    if on == saved.is_some() {
        return on;
    }
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance_mut() }) else { return saved.is_some() };
    let write = |row: &mut eldenring::param::HIT_MTRL_PARAM_ST, e: Effects| {
        row.set_sp_effect_id_on_hit0(e[0]);
        row.set_sp_effect_id_on_hit1(e[1]);
        row.set_sp_effect_id_for_wet00(e[2]);
        row.set_sp_effect_id_for_wet01(e[3]);
        row.set_sp_effect_id_for_wet02(e[4]);
        row.set_sp_effect_id_for_wet03(e[5]);
        row.set_sp_effect_id_for_wet04(e[6]);
    };
    if on {
        let mut own = Vec::new();
        for id in LAVA_MATERIALS {
            let Some(row) = repo.get_mut::<eldenring::cs::HitMtrlParam>(id as u32) else { continue };
            own.push((
                id,
                [
                    row.sp_effect_id_on_hit0(),
                    row.sp_effect_id_on_hit1(),
                    row.sp_effect_id_for_wet00(),
                    row.sp_effect_id_for_wet01(),
                    row.sp_effect_id_for_wet02(),
                    row.sp_effect_id_for_wet03(),
                    row.sp_effect_id_for_wet04(),
                ],
            ));
            write(row, [-1; 7]);
        }
        log(format!("lava: harmless for this fight ({} floor materials)", own.len()));
        *saved = Some(own);
    } else {
        for (id, effects) in saved.take().unwrap_or_default() {
            if let Some(row) = repo.get_mut::<eldenring::cs::HitMtrlParam>(id as u32) {
                write(row, effects);
            }
        }
        log("lava: burns again");
    }
    on
}

/// Torrent takes no damage in Mario mode. Looked up twice a second, he comes and goes.
fn torrent_cant_die() {
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed().as_secs_f32() < 0.5) {
        return;
    }
    *last = Some(std::time::Instant::now());
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    // (he's kept with the spirit summons, or among the map's characters)
    let all = wcm.summon_buddy_chr_set.characters().chain(wcm.chr_sets.iter().flatten().flat_map(|set| set.characters()));
    for chr in all.filter(|c| combat::is_torrent(c)) {
        if debug() && !chr.chr_flags1c5.is_invincible() {
            log(format!(
                "ride: Torrent made invincible (c{}, npc {}, type {:?}, team {}, hp {})",
                chr.character_id, chr.npc_id, chr.chr_type, chr.team_type, chr.modules.data.hp
            ));
        }
        chr.chr_flags1c5.set_is_invincible(true);
        if chr.modules.data.hp > 0 {
            chr.modules.data.hp = chr.modules.data.max_hp;
        }
    }
}

/// Getting on or off Torrent (101004 on, 101012 and 101212 off).
fn mount_anim(anim: i32) -> bool {
    (101000..102000).contains(&anim)
}

/// The Tarnished's current animation id.
pub(crate) fn current_anim(chr: &eldenring::cs::ChrIns) -> i32 {
    let t = &chr.modules.time_act;
    t.anim_queue[(t.read_idx % 10) as usize].anim_id
}

/// The last SM64 tick's time in libsm64's thread: all, Mario's tick, the sound's (ms), his action.
static LONG_TICK: Mutex<(f32, f32, f32, u32)> = Mutex::new((0.0, 0.0, 0.0, 0));
/// An SM64 tick this long (ms) is worth a line.
const LONG_TICK_MS: f32 = 20.0;

/// FPS and scan-cost bookkeeping, logged every ~2 s.
struct Perf {
    tick_ms: f32,
    draw_ms: f32,
    frames: u32,
    time: f32,
    scans: u32,
    scan_ms: f32,
    scan_max: f32,
    tris: usize,
}

impl Perf {
    const fn new() -> Self {
        Perf { tick_ms: 0.0, draw_ms: 0.0, frames: 0, time: 0.0, scans: 0, scan_ms: 0.0, scan_max: 0.0, tris: 0 }
    }

    fn scan(&mut self, ms: f32, tris: usize) {
        self.scans += 1;
        self.scan_ms += ms;
        self.scan_max = self.scan_max.max(ms);
        self.tris = tris;
    }
}

static PERF: Mutex<Perf> = Mutex::new(Perf::new());

/// Adds the time spent debug drawing to the perf stats when dropped.
struct DrawTimer(std::time::Instant);
impl Drop for DrawTimer {
    fn drop(&mut self) {
        PERF.lock().unwrap_or_else(|e| e.into_inner()).draw_ms += self.0.elapsed().as_secs_f32() * 1000.0;
    }
}

fn frame(data: &FD4TaskData) {
    if debug() {
        perf::frame_start();
    }
    let _span = perf::span(perf::FRAME);
    // once, a few seconds in (every mod's DLL loaded by then): what a memory check costs here
    {
        static BENCHED: AtomicBool = AtomicBool::new(false);
        static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if FRAMES.fetch_add(1, Ordering::Relaxed) == 600 && !BENCHED.swap(true, Ordering::Relaxed) {
            let (ok, bad) = explore::bench();
            log(format!("memory check: {ok:.2} us when the memory is there, {bad:.2} us when not (a caught fault)"));
        }
    }
    // co-op (Seamless): the session, the partners' poses, blows and shoves (nothing out of one)
    coop::frame();
    // the style meter's slow motion (the game's speed back to normal after it, in or out of Mario mode)
    style::update();
    // outside Mario mode the Tarnished dies like anyone (the flag is set further down, while
    // Mario is alive)
    if !ENABLED.load(Ordering::Relaxed) {
        if let Ok(flags) = unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
            flags.player_no_dead = false;
            flags.all_no_hit = false;
        }
    }
    {
        let mut p = PERF.lock().unwrap_or_else(|e| e.into_inner());
        p.frames += 1;
        p.time += data.delta_time.time;
        // (always: a hitch someone reports has to be in a log made without debug mode)
        if p.time >= 2.0 {
            if debug() {
                log(format!("perf: parts in the last 2 s ({} frames): {}", p.frames, perf::breakdown()));
                log(format!("perf: pacing, mario {}: {}", if ENABLED.load(Ordering::Relaxed) { "on" } else { "off" }, perf::pacing()));
            }
            if let Some(slow) = perf::report() {
                use std::sync::atomic::Ordering::Relaxed;
                let havok = if slow.contains("Havok") {
                    format!(
                        " | havok: {} bodies, at most {} looked at and {} decoded in a query",
                        havok_col::BODIES.load(Relaxed),
                        havok_col::MOST_LOOKED_AT.load(Relaxed),
                        havok_col::MOST_DECODED.load(Relaxed)
                    )
                } else {
                    String::new()
                };
                log(format!("perf: slow in the last 2 s: {slow}{havok}"));
            }
            havok_col::MOST_DECODED.store(0, std::sync::atomic::Ordering::Relaxed);
            havok_col::MOST_LOOKED_AT.store(0, std::sync::atomic::Ordering::Relaxed);
        }
        if p.time >= 2.0 && !debug() {
            *p = Perf::new();
        } else if p.time >= 2.0 {
            let fps = p.frames as f32 / p.time;
            // SM64 ticks per real second (should be 30), and the game's own clock against the real one
            {
                static WALL: Mutex<Option<std::time::Instant>> = Mutex::new(None);
                let mut wall = WALL.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(t0) = *wall {
                    let secs = t0.elapsed().as_secs_f32();
                    log(format!(
                        "perf: SM64 {:.0} ticks/s, game time {:.2} s per real {:.2} s",
                        SM64_TICKS.swap(0, Ordering::Relaxed) as f32 / secs,
                        p.time,
                        secs
                    ));
                }
                *wall = Some(std::time::Instant::now());
            }
            let mario = if ENABLED.load(Ordering::Relaxed) { "ON" } else { "off" };
            if let Some(player) = (unsafe { WorldChrMan::instance() }).ok().and_then(|w| w.main_player.as_ref()) {
                let req = &player.chr_ins.modules.action_request;
                let bits = |a: &eldenring::cs::ChrActions| unsafe { *(a as *const _ as *const u64) };
                let ph = &player.chr_ins.modules.physics;
                log(format!(
                    "tarnished: anim {}, possible inputs {:#x}, disabled {:#x}, falling {} touching {} standing {}",
                    current_anim(&player.chr_ins),
                    bits(&req.possible_action_inputs),
                    bits(&req.disabled_action_inputs),
                    ph.is_falling,
                    ph.is_touching_ground,
                    ph.standing_on_solid_ground
                ));
            }
            {
                use std::sync::atomic::Ordering::Relaxed;
                let (checks, failed) = (explore::CHECKS.swap(0, Relaxed), explore::FAILED.swap(0, Relaxed));
                let failed_ms = explore::FAILED_NS.swap(0, Relaxed) as f64 / 1e6;
                let frames = p.frames.max(1) as f64;
                log(format!("perf: memory checks {:.0}/frame, {failed} said no in the last 2 s ({failed_ms:.1} ms)", checks as f64 / frames));
            }
            if p.scans > 0 {
                let per_frame = |ms: f32| ms / p.frames as f32;
                log(format!(
                    "perf: mario {mario}, {fps:.0} fps | per frame: scan {:.2} ms, tick {:.2} ms, draw {:.2} ms | {} tris",
                    per_frame(p.scan_ms), per_frame(p.tick_ms), per_frame(p.draw_ms), p.tris
                ));
            } else {
                log(format!("perf: mario {mario}, {fps:.0} fps"));
            }
            *p = Perf::new();
        }
    }
    // a pad reading older than 0.25 s is stale (game unfocused / not polling): treat as neutral
    let pad = PAD
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .filter(|(_, t)| t.elapsed().as_secs_f32() < 0.25)
        .map(|(s, _)| s);
    // in a menu Mario gets nothing, unless the game itself walks the character there (then only the
    // left stick: confirming in the menu mustn't make him jump or punch)
    // (on Torrent the right stick still turns Lakitu's camera)
    let cam_pad = pad.filter(|_| RIDING.load(Ordering::Relaxed) && !MENU_OPEN.load(Ordering::Relaxed));
    let pad = if FOLLOWING.load(Ordering::Relaxed) {
        None
    } else if MENU_OPEN.load(Ordering::Relaxed) {
        pad.filter(|_| MENU_WALK.load(Ordering::Relaxed)).map(|mut p| {
            p.Gamepad.wButtons = Default::default();
            p.Gamepad.bLeftTrigger = 0;
            p.Gamepad.bRightTrigger = 0;
            p.Gamepad.sThumbRX = 0;
            p.Gamepad.sThumbRY = 0;
            p
        })
    } else {
        pad
    };
    // On Yoshi with the keyboard: whichever of the two last steered him. The keys go by the
    // game's own camera (a stick can be turned to Lakitu's, keys can't), so that camera shows.
    let riding = RIDING.load(Ordering::Relaxed);
    if !riding {
        KEY_RIDE.store(false, Ordering::Relaxed);
    } else if kbd::read().is_some_and(|k| k.stick_x != 0.0 || k.stick_y != 0.0) {
        KEY_RIDE.store(true, Ordering::Relaxed);
    } else if cam_pad.is_some_and(|p| (p.Gamepad.sThumbLX as i32).abs() > 12000 || (p.Gamepad.sThumbLY as i32).abs() > 12000) {
        KEY_RIDE.store(false, Ordering::Relaxed);
    }
    // Mario's keys (WASD etc.) only reach the game in menus or with Mario off, and on ladders
    // and on Yoshi, where the game moves him (hidden there, Yoshi could only be made to sprint)
    kbd::CAPTURE.store(
        ENABLED.load(Ordering::Relaxed)
            && IN_WORLD.load(Ordering::Relaxed)
            && (!MENU_OPEN.load(Ordering::Relaxed) || MENU_WALK.load(Ordering::Relaxed))
            && !ON_LADDER.load(Ordering::Relaxed)
            && !riding,
        Ordering::Relaxed,
    );

    static F10_WAS_DOWN: AtomicBool = AtomicBool::new(false);
    let f10 = debug_key(0x79);
    let dump_truth = f10 && !F10_WAS_DOWN.swap(true, Ordering::Relaxed);
    if !f10 {
        F10_WAS_DOWN.store(false, Ordering::Relaxed);
    }

    let Some(player) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) else {
        IN_WORLD.store(false, Ordering::Relaxed);
        // (out of the world, a load: a captured enemy is let go while its handle still names it)
        bossride::clear();
        return;
    };
    IN_WORLD.store(true, Ordering::Relaxed);
    // Mario mode is the default: switch it on once the player has been in the world for 2 s
    {
        static IN_WORLD_TIME: Mutex<f32> = Mutex::new(0.0);
        let mut t = IN_WORLD_TIME.lock().unwrap_or_else(|e| e.into_inner());
        *t += data.delta_time.time;
        // wait until the world has collision under the player (not mid-load)
        let grounded = *t > 0.5 && {
            let p = player.chr_ins.modules.physics.position;
            let me: &PlayerIns = unsafe { &*(&**player as *const PlayerIns) };
            unsafe { eldenring::cs::CSHavokMan::instance() }.ok().is_some_and(|h| {
                h.phys_world
                    .cast_ray(0x08, &HavokPosition(p.0, p.1 + 1.0, p.2, 0.0), eldenring::position::PositionDelta(0.0, -4.0, 0.0), me)
                    .is_some()
            })
        };
        if grounded && player.chr_ins.modules.data.hp > 0 {
            show_banner();
        }
        // at launch, and again whenever it went off by itself (the no-ground safety after a fall
        // into the void, for one) while the player still wants Mario: on as soon as there's ground
        let back = AUTO_STARTED.load(Ordering::Relaxed)
            && WANTED.load(Ordering::Relaxed)
            && !ENABLED.load(Ordering::Relaxed)
            && assets::ready()
            && SM64_READY.load(Ordering::Relaxed)
            && !worker::hung();
        // (debug, `mario = off` in er_mario.ini: the mod loaded but idle, to compare frame pacing)
        static IDLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let idle = *IDLE.get_or_init(|| debug() && paths::config("mario").is_some_and(|v| v.eq_ignore_ascii_case("off")));
        if !idle && grounded && player.chr_ins.modules.data.hp > 0 && (!AUTO_STARTED.swap(true, Ordering::Relaxed) || back) {
            ENABLED.store(true, Ordering::Relaxed);
            log(if back { "mario mode ON again" } else { "mario mode ON (default at launch)" });
        }
    }
    // without Mario's model files (first launch builds them) Mario mode stays off
    if ENABLED.load(Ordering::Relaxed) && !assets::ready() {
        ENABLED.store(false, Ordering::Relaxed);
        static TOLD: AtomicBool = AtomicBool::new(false);
        if !TOLD.swap(true, Ordering::Relaxed) {
            log("mario mode needs the built files: restart the game after the first launch");
        }
    }
    let player_ref: &PlayerIns = unsafe { &*(&**player as *const PlayerIns) };
    let set_opacity = |a: f32| unsafe {
        let chr = &mut (*(player_ref as *const PlayerIns as *mut PlayerIns)).chr_ins;
        chr.opacity_keyframes_multiplier = a;
        chr.opacity_keyframes_multiplier_previous = a;
    };
    // F6: experiment, leave the Tarnished's gravity and fall motion to the game
    static F6_WAS: AtomicBool = AtomicBool::new(false);
    let f6 = debug_key(0x75);
    if f6 && !F6_WAS.swap(true, Ordering::Relaxed) {
        let mode = (EXPERIMENT.load(Ordering::Relaxed) + 1) % 3;
        EXPERIMENT.store(mode, Ordering::Relaxed);
        HANDS_OFF.store(mode >= 1, Ordering::Relaxed);
        log(format!("experiment mode {mode}"));
    } else if !f6 {
        F6_WAS.store(false, Ordering::Relaxed);
    }
    {
        static LAST_ANIM: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);
        let a = current_anim(&player.chr_ins);
        let last = LAST_ANIM.swap(a, Ordering::Relaxed);
        // sitting down at a site of grace (the rest animations)
        if last != a && (68000..69000).contains(&a) && !(68000..69000).contains(&last) {
            REST.store(true, Ordering::Relaxed);
            log(format!("resting at a grace (anim {a}): health refilled"));
        }
        if last != a && debug() {
            crate::dlog(format!("tarnished anim -> {a}"));
        }
        // debug: what the Tarnished stands on (to learn material ids, e.g. lava)
        if debug() {
            static LAST_MATERIAL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);
            let material = player.chr_ins.modules.physics.material_info.hit_material;
            if LAST_MATERIAL.swap(material, Ordering::Relaxed) != material {
                log(format!("floor material -> {material} (material param {})", player.chr_ins.modules.material.material_param_id));
            }
        }
    }
    static FALL_OVERRIDDEN: AtomicBool = AtomicBool::new(false);
    if ENABLED.load(Ordering::Relaxed) && !HANDS_OFF.load(Ordering::Relaxed) {
        let fall = &mut player.chr_ins.modules.fall;
        fall.fall_timer = 0.0;
        fall.disable_fall_motion = true;
        FALL_OVERRIDDEN.store(true, Ordering::Relaxed);
    } else if FALL_OVERRIDDEN.swap(false, Ordering::Relaxed) {
        player.chr_ins.modules.fall.disable_fall_motion = false;
    }
    let physics = &mut player.chr_ins.modules.physics;
    if dump_truth {
        // ground truth for decoder testing: real surface heights on a grid around the player
        if let Ok(havok) = unsafe { eldenring::cs::CSHavokMan::instance() } {
            let p = physics.position;
            let mut out = format!("# player {} {} {}\n", p.0, p.1, p.2);
            for i in -20..=20 {
                for j in -20..=20 {
                    let (x, z) = (p.0 + i as f32 * 0.25, p.2 + j as f32 * 0.25);
                    let start = HavokPosition(x, p.1 + 3.0, z, 0.0);
                    if let Some(h) = havok.phys_world.cast_ray(0x08, &start, eldenring::position::PositionDelta(0.0, -8.0, 0.0), player_ref) {
                        out += &format!("{x} {} {z}\n", h.1);
                    }
                }
            }
            let _ = std::fs::write(paths::file("truth.txt"), out);
            log("wrote ground-truth ray grid to truth.txt");
        }
    }

    let mut guard = MARIO.lock().unwrap_or_else(|e| e.into_inner());
    if worker::hung() {
        ENABLED.store(false, Ordering::Relaxed);
    }
    if !ENABLED.load(Ordering::Relaxed) {
        if gameover::ACTIVE.swap(false, Ordering::Relaxed) {
            gameover::reset();
        }
        hud::set_kit(None);
        {
            let mut pending = PENDING_RESTORE.lock().unwrap_or_else(|e| e.into_inner());
            // every 0.25 s: empty the armour (model reload), then put the old loadout back
            if let Some((t, saved)) = pending.as_mut() {
                if t.elapsed().as_secs_f32() > 0.25 {
                    *t = std::time::Instant::now();
                    if equip::restore(player_ref, saved) {
                        pending.take();
                        log("equip: previous loadout restored");
                    }
                }
            }
        }
        if let Some(mut m) = guard.take() {
            m.moving.clear(&mut m.havok);
            if let Some(saved) = SAVED_LOADOUT.lock().unwrap_or_else(|e| e.into_inner()).take() {
                equip::leave(&saved);
                *PENDING_RESTORE.lock().unwrap_or_else(|e| e.into_inner()) = Some((std::time::Instant::now(), saved));
            }
            let id = m.id;
            rock::clear(id);
            // (nothing of his left behind: an enemy stopped in time, flights, a ride, the star music)
            stopwatch::clear();
            bossride::clear();
            bat::clear();
            maxfin::clear();
            worker::call("delete", move |_| unsafe { sm64::sm64_mario_delete(id) });
            mega::clear();
            set_scale(BASE_SCALE);
            combat::restore_faded();
            physics.gravity_disabled = false;
            set_opacity(1.0);
            *engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            // back to normal where Mario is (the no-ground safety net still returns home)
            if RETURN_HOME.swap(false, Ordering::Relaxed) {
                physics.position = HavokPosition(m.home[0], m.home[1], m.home[2], 0.0);
                physics.chr_proxy_pos_update_requested = true;
            }
        }
        return;
    }

    if !SM64_READY.load(Ordering::Relaxed) && init_sm64(false).is_none() {
        ENABLED.store(false, Ordering::Relaxed);
        return;
    }

    if guard.is_none() && CREATE_RETRY.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) {
        return;
    }
    // Mario only once the game has solid ground right under the player: after a respawn the
    // floor there may still be loading while something lower (a cave, the terrain under a
    // building) already is, and Mario would be put on that, under the floor
    if guard.is_none() {
        let p = physics.position;
        let ground = unsafe { eldenring::cs::CSHavokMan::instance() }.ok().and_then(|h| {
            h.phys_world.cast_ray(RAY_FILTER, &HavokPosition(p.0, p.1 + 1.0, p.2, 0.0), eldenring::position::PositionDelta(0.0, -4.0, 0.0), player_ref)
        });
        if ground.is_none() {
            *CREATE_RETRY.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
            log("waiting for the ground under the player before creating Mario");
            return;
        }
    }
    let m = guard.get_or_insert_with(|| {
        // (a new Mario is his own size)
        mega::clear();
        set_scale(BASE_SCALE);
        let p = physics.position;
        let feet = [p.0, p.1, p.2];
        log("probing raycast filters:");
        let filter = RAY_FILTER;
        log(format!("using filter {filter:#010x}"));
        let o = physics.orientation;
        let fw = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
        log("forward probe (chest height, 5 m):");
        collision::probe_forward(player_ref, feet, [fw.x, fw.y, fw.z]);
        let caster = collision::Caster { filter, origin: feet, player: player_ref };
        let mut havok = havok_col::HavokCollision::new(COLLISION_LAYERS.to_vec());
        let t0 = std::time::Instant::now();
        let surfaces = match havok_surfaces(&mut havok, feet, [0.0, 0.0, 0.0], player_ref) {
            Some(s) => {
                log(format!("initial collision from Havok: {} triangles in {:.0} ms", s.len(), t0.elapsed().as_secs_f32() * 1000.0));
                s
            }
            None => {
                log("Havok collision unavailable, falling back to ray scanning");
                collision::build(&caster, [0.0, 0.0, 0.0])
            }
        };
        // plus a floor exactly where the game has his feet (and for a moment after): whatever he
        // stands on, SM64 can create Mario there, at the right height (a floor it doesn't know,
        // like some platforms, made creation fail and left the player invisible)
        // (the game holds a respawning player about a metre under the floor for a moment: the
        // top of the ground within 2 m above his feet is where Mario goes)
        let lift = unsafe { eldenring::cs::CSHavokMan::instance() }
            .ok()
            .and_then(|h| h.phys_world.cast_ray(RAY_FILTER, &HavokPosition(p.0, p.1 + 2.0, p.2, 0.0), eldenring::position::PositionDelta(0.0, -4.0, 0.0), player_ref))
            .map(|g| (g.1 - p.1).clamp(0.0, 2.0))
            .unwrap_or(0.0);
        if lift > 0.05 {
            log(format!("spawn: the ground is {lift:.2} m above the player's feet, Mario goes on top"));
        }
        let start_y = lift / scale();
        let mut surfaces = surfaces;
        surfaces.extend(flat_floor([0.0, start_y, 0.0]));
        load_surfaces(&surfaces);
        *FOLLOW_ENDED.lock().unwrap_or_else(|e| e.into_inner()) = Some((std::time::Instant::now(), [0.0, start_y, 0.0]));
        {
            let mut saved = SAVED_LOADOUT.lock().unwrap_or_else(|e| e.into_inner());
            if saved.is_none() {
                *saved = equip::enter(player_ref);
            } else {
                equip::enforce(player_ref);
            }
        }
        let id = worker::call("create", move |_| unsafe { sm64::sm64_mario_create(0.0, start_y + 1.0, 0.0) }).unwrap_or(-1);
        log(format!("mario created id={id} at {:?}", (p.0, p.1, p.2)));
        // which collision is under his feet (a layer SM64 doesn't get would drop him through)
        for line in havok.probe(glam::Vec3::new(p.0, p.1, p.2)) {
            if line.contains("surface") {
                log(format!("spawn floor:{line}"));
            }
        }
        {
            // which armour pieces is the player wearing? (for swapping in the Mario model)
            use eldenring::cs::{EquipParamProtector, SoloParamRepository};
            let asm = &player_ref.chr_asm;
            let repo = unsafe { SoloParamRepository::instance() }.ok();
            for (slot, name) in [(12usize, "head"), (13, "chest"), (14, "hands"), (15, "legs")] {
                let id = asm.equipment_param_ids[slot];
                let model = repo.and_then(|r| r.get::<EquipParamProtector>(id as u32)).map(|p| p.equip_model_id());
                log(format!("equipped {name}: param {id} model {model:?}"));
            }
            for slot in 0..6usize {
                let id = asm.equipment_param_ids[slot];
                let model = repo.and_then(|r| r.get::<eldenring::cs::EquipParamWeapon>(id as u32)).map(|p| p.equip_model_id());
                log(format!("weapon slot {slot}: param {id} model {model:?}"));
            }
        }
        {
            let pm = &**physics as *const _ as usize;
            let proxy = unsafe { *((pm + 0x98) as *const usize) };
            let proxy2 = unsafe { *((pm + 0xa0) as *const usize) };
            log(format!(
                "player physics module {pm:#x}, chr_proxy {proxy:#x} ({:?}), chr_proxy2 {proxy2:#x} ({:?})",
                explore::class_of(proxy),
                explore::class_of(proxy2)
            ));
        }
        let mut moving = moving::Moving::default();
        moving.watch_query(&havok);
        MarioState {
            id,
            filter,
            ticks: 0,
            no_ground: 0,
            surfaces,
            havok,
            wall_memory: Default::default(),
            home: feet,
            origin: feet,
            acc: 0.0,
            state: Default::default(),
            mesh: Vec::new(),
            mesh_color: Vec::new(),
            mesh_normal: Vec::new(),
            prev_mesh: Vec::new(),
            prev_pos: [0.0; 3],
            last_set: None,
            last_query: None,
            last_query_havok: false,
            parts: None,
            combat: combat::Combat::new(),
            fire: fireball::Fire::new(),
            parry: parry::Parry::new(),
            rang: {
                // (a new Mario starts bare-handed)
                weapons::clear();
                hammer::set_out(false);
                chomp::set_out(false);
                boomerang::clear();
                boomerang::Boomerang::new()
            },
            hook: {
                hook::clear();
                hook::Hook::new()
            },
            moves: moves::Moves::new(),
            traps: traps::Traps::new(),
            combo: sm64::SM64ErMove::default(),
            dead: false,
            moving,
            stuck_ticks: 0,
            prev_parts: None,
        }
    });
    if m.id < 0 {
        // libsm64 only creates Mario on a floor: right after a load the collision may not be there
        // yet. Drop this attempt and retry shortly (instead of staying broken with the last pose).
        log("mario creation failed (no floor yet), retrying in 1 s");
        *engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *guard = None;
        *CREATE_RETRY.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
        return;
    }

    // a cap's look going on (the chest piece back on, with its new model)
    equip::look_tick(player_ref);
    // equipment lock: only the Mario set and fists while Mario is on
    {
        static LOCK_TIMER: Mutex<f32> = Mutex::new(0.0);
        let mut t = LOCK_TIMER.lock().unwrap_or_else(|e| e.into_inner());
        *t += data.delta_time.time;
        if *t > 0.5 {
            *t = 0.0;
            if SAVED_LOADOUT.lock().unwrap_or_else(|e| e.into_inner()).is_some() && equip::enforce(player_ref) {
                log("equip: Mario set / fists re-equipped");
            }
        }
    }

    // SM64 game over instead of "YOU DIED" (Bowser's laugh + the Bowser iris), black until respawn
    gameover::ACTIVE.store(true, Ordering::Relaxed);
    {
        // F3: game-over transition on demand (testing; ends right away since nobody died)
        static F3_WAS: AtomicBool = AtomicBool::new(false);
        let f3 = debug_key(0x72);
        if f3 && !F3_WAS.swap(true, Ordering::Relaxed) {
            gameover::STARTED.store(true, Ordering::Relaxed);
        } else if !f3 {
            F3_WAS.store(false, Ordering::Relaxed);
        }
    }
    // F1 held for 3 s: the Tarnished dies (stuck where nothing else helps, under a lift for one)
    {
        static HELD: Mutex<Option<std::time::Instant>> = Mutex::new(None);
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        let f1 = kbd::focused() && !MENU_OPEN.load(Ordering::Relaxed) && unsafe { GetAsyncKeyState(VK_F1) } as u16 & 0x8000 != 0;
        if !f1 || m.dead {
            *held = None;
        } else if held.get_or_insert_with(std::time::Instant::now).elapsed().as_secs_f32() >= 3.0 {
            *held = None;
            log("F1 held for 3 s: the Tarnished dies");
            set_player_hp(0);
        }
    }
    // SM64's camera (F9 switches to Elden Ring's): Elden Ring's own for cutscenes, doors, deaths
    {
        static F9_WAS: AtomicBool = AtomicBool::new(false);
        let f9 = kbd::focused() && unsafe { GetAsyncKeyState(0x78) } as u16 & 0x8000 != 0;
        if f9 && !F9_WAS.swap(true, Ordering::Relaxed) {
            let on = !lakitu::ON.load(Ordering::Relaxed);
            lakitu::ON.store(on, Ordering::Relaxed);
            log(format!("SM64 camera {}", if on { "on" } else { "off" }));
        } else if !f9 {
            F9_WAS.store(false, Ordering::Relaxed);
        }
        let in_cutscene = cutscene_now(m.dead, player_ref);
        if in_cutscene {
            // a cutscene: the game's own camera shows (no update, so the camera isn't written);
            // Lakitu carries on afterwards where he was
        } else if WORLD_PAUSED.load(Ordering::Relaxed) {
            // a popup pausing the world: Lakitu's camera stays exactly where it was (handing it to
            // the game's camera made it drift off to its own spot)
            if lakitu::ON.load(Ordering::Relaxed) {
                lakitu::hold();
            }
        } else if lakitu::ON.load(Ordering::Relaxed) && !m.dead && (!FOLLOWING.load(Ordering::Relaxed) || (RIDING.load(Ordering::Relaxed) && !KEY_RIDE.load(Ordering::Relaxed))) {
            // (a popup pausing the game freezes the camera's controls too)
            let frozen = WORLD_PAUSED.load(Ordering::Relaxed);
            let key = |vk: i32| !frozen && kbd::focused() && unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
            let stick = pad.or(cam_pad).filter(|_| !frozen).map(|p| (p.Gamepad.sThumbRX as i32, p.Gamepad.sThumbRY as i32)).unwrap_or((0, 0));
            const T: i32 = 20000;
            // C-left, C-right, C-up, C-down
            let c = [
                stick.0 < -T || key(0x25),
                stick.0 > T || key(0x27),
                stick.1 > T || key(0x26),
                stick.1 < -T || key(0x28),
            ];
            let p = physics.position;
            let airborne = m.state.action & 0x800 != 0;
            let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
            // first person: the left stick (or WASD) looks around; A / B leave it
            let (mut look, mut exit) = ((0.0, 0.0), false);
            // (not on Torrent, the stick rides him)
            if let Some(pd) = pad.filter(|_| !frozen && !RIDING.load(Ordering::Relaxed)) {
                let g = pd.Gamepad;
                let axis = |v: i16| {
                    let f = v as f32 / 32767.0;
                    if f.abs() < 0.2 { 0.0 } else { f.clamp(-1.0, 1.0) }
                };
                look = (axis(g.sThumbLX), axis(g.sThumbLY));
                exit = g.wButtons.contains(XINPUT_GAMEPAD_A) || g.wButtons.contains(XINPUT_GAMEPAD_B) || g.wButtons.contains(XINPUT_GAMEPAD_X);
            }
            if let Some(k) = kbd::read().filter(|_| !frozen) {
                if look == (0.0, 0.0) {
                    look = (k.stick_x, -k.stick_y);
                }
                exit |= k.a || k.b;
            }
            const ACT_FLAG_STATIONARY: u32 = 0x200;
            let idle = m.state.action & ACT_FLAG_STATIONARY != 0 && m.state.forward_velocity.abs() < 1.0;
            let o = physics.orientation;
            let body_fwd = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
            // (lifted by an enemy's grab, Mario is where its animation carries the Tarnished's body)
            let lifted = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(engine_mario::root_offset());
            // the right stick (or the arrow keys) turns the camera smoothly
            let orbit = if key(0x25) { -1.0 } else if key(0x27) { 1.0 } else if stick.0.abs() > 4000 { (stick.0 as f32 / 32767.0).clamp(-1.0, 1.0) } else { 0.0 };
            lakitu::update(data.delta_time.time, glam::Vec3::new(p.0, p.1, p.2) + lifted, airborne, c, orbit, look, body_fwd, idle, exit, |from, to| {
                let h = havok?;
                let d = to - from;
                h.phys_world
                    .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                    .map(|h| glam::Vec3::new(h.0, h.1, h.2))
            });
            let sounds = lakitu::take_sounds();
            if !sounds.is_empty() {
                worker::call("camera sound", move |_| {
                    for id in sounds {
                        unsafe { sm64::sm64_play_sound_global(id) };
                    }
                });
            }
        } else {
            lakitu::reset();
        }
        let turn = lakitu::forward().filter(|_| RIDING.load(Ordering::Relaxed)).and_then(|ours| {
            let cam = unsafe { WorldChrMan::instance() }.ok()?.chr_cam?;
            let m = &unsafe { cam.as_ref() }.pers_cam.matrix;
            let (right, theirs) = (glam::vec2(m.0.0, m.0.2), glam::vec2(m.2.0, m.2.2).try_normalize()?);
            let ours = glam::vec2(ours.x, ours.z).try_normalize()?;
            // which side the game's "right" is on, so the turn goes the right way round
            let side = right.dot(glam::vec2(theirs.y, -theirs.x)).signum();
            Some((side * (ours.x * theirs.y - ours.y * theirs.x)).atan2(ours.dot(theirs)))
        });
        RIDE_TURN.store(turn.unwrap_or(f32::NAN).to_bits(), Ordering::Relaxed);
    }
    let wedges = if m.dead { 0 } else { (m.state.health.max(0) >> 8) as u8 };
    // (no HUD on the loading screen: the Tarnished has no animation yet while the world loads)
    let loading = current_anim(&player_ref.chr_ins) == -1;
    let paused = WORLD_PAUSED.load(Ordering::Relaxed);
    let hide_why = if m.dead {
        Some("dead")
    } else if loading {
        Some("loading")
    } else if paused {
        Some("paused")
    } else if MENU_OPEN.load(Ordering::Relaxed) {
        Some("menu")
    } else {
        None
    };
    hud::set(wedges.min(8), hide_why, true);
    // the tail swing: watch the bosses' stance, carry / throw / fly the grabbed one
    swing::watch_stances(&combat::boss_handles());
    squish::tick();
    {
        let me = to_er(m.origin, m.state.position);
        let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
        let impact = swing::update(data.delta_time.time, glam::Vec3::new(me.0, me.1, me.2), m.state.face_angle, m.state.action, |from, to| {
            let h = havok?;
            let d = to - from;
            h.phys_world
                .cast_ray(PLAYER_MOVE_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                .or_else(|| h.phys_world.cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref))
                .map(|h| glam::Vec3::new(h.0, h.1, h.2))
        });
        if let Some((boss, pct)) = impact {
            combat::impact(&mut m.combat, &boss, pct, m.ticks);
            worker::call("impact sound", |_| unsafe { sm64::sm64_play_sound_global(swing::SOUND_IMPACT) });
        }
        // an enemy picked up like a Bob-omb: carried between Mario's hands, thrown with B
        let id = m.id;
        let hands = worker::call("held", move |_| {
            let mut p = [0f32; 3];
            (unsafe { sm64::sm64_er_held(id, p.as_mut_ptr()) } != 0).then_some(p)
        })
        .flatten()
        // (libsm64 doesn't fill SM64's held-object point, that comes from its camera renderer:
        // in front of Mario at chest height, like SM64 holds a Bob-omb)
        .map(|_| {
            let fa = m.state.face_angle;
            glam::Vec3::new(me.0, me.1, me.2) + glam::Vec3::new(-fa.sin(), 0.0, fa.cos()) * 0.45 + glam::Vec3::Y * 0.75
        });
        let impact = carry::update(data.delta_time.time, hands, m.state.face_angle, m.state.action, |from, to| {
            let h = havok?;
            let d = to - from;
            h.phys_world
                .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                .map(|h| glam::Vec3::new(h.0, h.1, h.2))
        });
        for (mob, pct) in &impact {
            combat::impact(&mut m.combat, mob, *pct, m.ticks);
        }
        // enemies up in the air with Mario (the air combos), or falling back down
        {
            let me = to_er(m.origin, m.state.position);
            juggle::update(data.delta_time.time, glam::Vec3::new(me.0, me.1, me.2), m.state.face_angle, |from, to| {
                let h = havok?;
                let d = to - from;
                h.phys_world
                    .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                    .map(|h| glam::Vec3::new(h.0, h.1, h.2))
            });
        }
        // enemies the giant fireball blew away, landing
        let landed = knock::update(data.delta_time.time, |from, to| {
            let h = havok?;
            let d = to - from;
            h.phys_world
                .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                .map(|h| glam::Vec3::new(h.0, h.1, h.2))
        });
        for (mob, pct) in &landed {
            combat::impact(&mut m.combat, mob, *pct, m.ticks);
        }
        // an enemy stopped in time (stopwatch.rs): its clock, and time running again
        stopwatch::update();
        // enemies sent flying by the hook's moves (the home run, the tether)
        bat::update(data.delta_time.time, m.origin, &player_ref.chr_ins, &mut m.combat, m.ticks, |from, to| {
            let h = havok?;
            let d = to - from;
            h.phys_world
                .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                .map(|h| glam::Vec3::new(h.0, h.1, h.2))
        });
    }
    let st = stats::get();
    hud::set_counters(st.deaths, st.coins, st.stars);
    hud::set_tags(combat::tags());
    // co-op: what Mario is using, for the partner's card; their cards
    if coop::active() {
        coop::set_power(if bossride::active() {
            coop::POWER_CAPTURE
        } else if yoshi::active() {
            coop::POWER_YOSHI
        } else if mega::active() {
            coop::POWER_MEGA
        } else if rock::active() {
            coop::POWER_ROCK
        } else {
            match caps::active() {
                Some(caps::WING) => coop::POWER_WING,
                Some(caps::METAL) => coop::POWER_METAL,
                Some(_) => coop::POWER_VANISH,
                None => coop::POWER_NONE,
            }
        });
        let feet = to_er(m.origin, m.state.position);
        hud::set_partners(coop::hud(glam::Vec3::new(feet.0, feet.1, feet.2), data.delta_time.time));
    } else {
        hud::set_partners(Vec::new());
    }
    hud::set_bosses(combat::bosses());
    if gameover::update(!m.dead) {
        log("game over: Bowser laughs");
        worker::call("laugh", |_| unsafe { sm64::sm64_play_sound_global(gameover::BOWSER_LAUGH) });
    }

    // a boss died: SM64's star dance ("Here we go!", peace sign) once Mario is on the ground
    {
        static BOSS_HP: Mutex<Vec<(u64, i32)>> = Mutex::new(Vec::new());
        static DANCE_PENDING: Mutex<Option<std::time::Instant>> = Mutex::new(None);
        let mut known = BOSS_HP.lock().unwrap_or_else(|e| e.into_inner());
        if let (Ok(fe), Ok(wcm)) = (unsafe { eldenring::cs::CSFeManImp::instance() }, unsafe { WorldChrMan::instance() }) {
            for entry in &fe.boss_health_displays {
                if entry.field_ins_handle.is_empty() {
                    continue;
                }
                let key = unsafe { std::mem::transmute_copy::<eldenring::cs::FieldInsHandle, u64>(&entry.field_ins_handle) };
                let Some(boss) = wcm.chr_ins_by_handle(&entry.field_ins_handle) else { continue };
                let hp = boss.modules.data.hp;
                match known.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, last)) => {
                        if *last > 0 && hp <= 0 {
                            log(format!("boss down (fmg {}): star dance", entry.fmg_id));
                            if boss_star() {
                                *DANCE_PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
                            }
                        }
                        *last = hp;
                    }
                    None => known.push((key, hp)),
                }
            }
        }
        let mut pending = DANCE_PENDING.lock().unwrap_or_else(|e| e.into_inner());
        // the game's own "... FELLED" banner also means a boss died
        if gameover::BOSS_FELLED.swap(false, Ordering::Relaxed) && pending.is_none() && boss_star() {
            log("felled banner: star dance");
            *pending = Some(std::time::Instant::now());
        }
        // F4: star dance on demand (testing)
        static F4_WAS: AtomicBool = AtomicBool::new(false);
        let f4 = debug_key(0x73);
        if f4 && !F4_WAS.swap(true, Ordering::Relaxed) {
            *pending = Some(std::time::Instant::now());
        } else if !f4 {
            F4_WAS.store(false, Ordering::Relaxed);
        }
        if let Some(t) = *pending {
            const ACT_FLAG_AIR: u32 = 0x0000_0800;
            if t.elapsed().as_secs_f32() > 10.0 || m.dead {
                *pending = None;
            } else if m.state.action & ACT_FLAG_AIR == 0 {
                *pending = None;
                let id = m.id;
                worker::call("star dance", move |_| unsafe { sm64::sm64_set_mario_action(id, 0x0000_1307) });
            }
        }
    }

    // Mega Mario can't stay big on Torrent, through a door or a fog wall, up a ladder (the game moves
    // him: follow mode), or dead; his world goes back to his size once he isn't big
    if (mega::active() || mega::changing()) && (m.dead || FOLLOWING.load(Ordering::Relaxed) || RIDING.load(Ordering::Relaxed)) {
        log("mega: over (riding, following the game, or dead)");
        mega::clear();
    }
    if scale() != BASE_SCALE && !mega::active() {
        rescale(m, 1.0, player_ref);
    }
    // (the hook's off while the game moves him)
    if FOLLOWING.load(Ordering::Relaxed) || RIDING.load(Ordering::Relaxed) {
        m.hook.drop();
    }

    // death: the Tarnished's HP is the truth. At 0, Mario dies SM64-style; when the game respawns
    // the player (HP back), Mario is recreated fresh where the player now is.
    let hp = player_ref.chr_ins.modules.data.hp;
    if !m.dead && hp <= 0 {
        m.dead = true;
        let id = m.id;
        worker::call("kill", move |_| unsafe { sm64::sm64_mario_kill(id) });
        log("the Tarnished died: Mario dies");
        stats::update(|s| s.deaths += 1);
        coins::clear();
        // (half the purse stays in a bag where he fell)
        coins::die(&player_ref.chr_ins);
        fireball::clear();
        boomerang::clear();
        blown::clear();
        m.hook.drop();
        ride::clear();
        chomp::clear();
        impact::clear();
        knock::clear();
        juggle::clear();
        style::clear();
        timing::clear();
        m.traps.clear();
        rock::clear(m.id);
        bat::clear();
        maxfin::clear();
        skillshot::clear();
        stopwatch::clear();
        bossride::clear();
        dodge::clear();
        grab::clear();
        status::clear();
        caps::clear();
        mega::clear();
        swing::reset();
        carry::reset();
    } else if m.dead && hp > 0 {
        log("respawned: recreating Mario");
        m.moving.clear(&mut m.havok);
        let id = m.id;
        rock::clear(id);
        stopwatch::clear();
        bossride::clear();
        worker::call("delete", move |_| unsafe { sm64::sm64_mario_delete(id) });
        *guard = None;
        return;
    }

    // Elden Ring re-bases its physics coordinates as you travel (floating origin). If the player
    // is suddenly metres away from where we put him, the world shifted: shift our frame with it.
    if let Some(last) = m.last_set {
        let p = physics.position;
        let d = [p.0 - last[0], p.1 - last[1], p.2 - last[2]];
        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > 2.0 {
            log(format!("world origin shifted by {d:?}"));
            // (a load or a warp: Mega Mario wouldn't know whether he fits where he lands; not the
            // floating origin's steps as he walks, tens of metres: his frame shifts with them)
            let far = (d[0] * d[0] + d[2] * d[2]).sqrt() > 150.0;
            if far && (mega::active() || mega::changing()) {
                log("mega: over (the world moved)");
                mega::clear();
            }
            coins::shift(glam::Vec3::from(d));
            m.traps.shift(glam::Vec3::from(d));
            bat::shift(glam::Vec3::from(d));
            m.havok.refresh_bodies();
            m.moving.rewatch(&m.havok);
            for i in 0..3 {
                m.origin[i] += d[i];
                m.home[i] += d[i];
            }
            for scan in m.wall_memory.iter_mut() {
                for w in scan.iter_mut() {
                    for v in w.v.iter_mut() {
                        for i in 0..3 {
                            v[i] += d[i];
                        }
                    }
                }
            }
        }
    }

    // run SM64 at its native 30 Hz (not while the game world is paused by a popup)
    // (a death isn't a pause: the death animation holds still at its end, and Mario's own death,
    // the game over and its sounds must play out)
    let paused = !m.dead && world_paused(player_ref);
    // following the Tarnished through a door, fog wall, grace or ladder: the follow mode steps SM64
    // itself (frame below). Stepping it here as well ran Mario at up to twice the speed there.
    let paused = paused || FOLLOWING.load(Ordering::Relaxed);
    if paused {
        m.acc = 0.0;
        // SM64's sound keeps going while the world is paused (queued sounds would wait otherwise)
        static AUDIO_ACC: Mutex<f32> = Mutex::new(0.0);
        let mut acc = AUDIO_ACC.lock().unwrap_or_else(|e| e.into_inner());
        *acc += data.delta_time.time.min(0.25);
        while *acc >= 1.0 / 30.0 {
            *acc -= 1.0 / 30.0;
            worker::call("audio", |_| {
                let mut buf = [0i16; 544 * 2 * 2];
                let frames = unsafe { sm64::sm64_audio_tick(audio::queued(), 1100, buf.as_mut_ptr()) } as usize;
                audio::push(&buf[..(frames * 2 * 2).min(buf.len())]);
            });
        }
    }
    // (in the style meter's slow motion Mario keeps his pace)
    let dt = style::mario_dt(data.delta_time.time);
    // (Mega Mario is a titan: all of him runs slower, his walk, his blows, his jumps)
    let titan = if mega::active() { mega::TIME } else { 1.0 };
    m.acc += if paused { 0.0 } else { dt.min(0.25) * titan };
    let ticks_before = SM64_TICKS.load(Ordering::Relaxed);
    while m.acc >= 1.0 / 30.0 {
        m.acc -= 1.0 / 30.0;
        SM64_TICKS.fetch_add(1, Ordering::Relaxed);
        let mut inputs = sm64::SM64MarioInputs::default();
        // RB is the fireball (fireball.rs), B the dash and Y the special moves (moves.rs); B is
        // SM64's B like X while Mario holds an enemy (the throw) or rides
        let fire_ok = !carry::holding() && !RIDING.load(Ordering::Relaxed);
        let (mut b_fire, mut b_dash, mut y_move, mut x_move, mut l_hammer) = (false, false, false, false, false);
        let mut dpad = [false; 4];
        // R2 (the middle mouse button): the hook (hook.rs)
        let mut r3 = false;
        let mut b_hook = kbd::focused() && unsafe { GetAsyncKeyState(0x04) } as u16 & 0x8000 != 0;
        if let Some(p) = pad.filter(|_| !m.dead) {
            let g = p.Gamepad;
            let axis = |v: i16| {
                let f = v as f32 / 32767.0;
                if f.abs() < 0.2 { 0.0 } else { f.clamp(-1.0, 1.0) }
            };
            inputs.stick_x = axis(g.sThumbLX);
            inputs.stick_y = -axis(g.sThumbLY);
            inputs.button_a = g.wButtons.contains(XINPUT_GAMEPAD_A) as u8;
            inputs.button_b = (g.wButtons.contains(XINPUT_GAMEPAD_X) || (g.wButtons.contains(XINPUT_GAMEPAD_B) && !fire_ok)) as u8;
            b_fire = g.wButtons.contains(XINPUT_GAMEPAD_RIGHT_SHOULDER);
            b_dash = g.wButtons.contains(XINPUT_GAMEPAD_B);
            y_move = g.wButtons.contains(XINPUT_GAMEPAD_Y);
            x_move = g.wButtons.contains(XINPUT_GAMEPAD_X);
            dpad = [XINPUT_GAMEPAD_DPAD_UP, XINPUT_GAMEPAD_DPAD_DOWN, XINPUT_GAMEPAD_DPAD_LEFT, XINPUT_GAMEPAD_DPAD_RIGHT].map(|b| g.wButtons.contains(b));
            // (L1 is the hammer, L2 SM64's Z)
            l_hammer = g.wButtons.contains(XINPUT_GAMEPAD_LEFT_SHOULDER);
            inputs.button_z = (g.bLeftTrigger > 100) as u8;
            r3 = g.wButtons.contains(XINPUT_GAMEPAD_RIGHT_THUMB);
            b_hook |= g.bRightTrigger > 100;
        }
        // SM64's first-person view: Mario stands still, the stick looks around
        if lakitu::first_person() {
            inputs = sm64::SM64MarioInputs::default();
        }
        // mouse and keyboard (the PC port's keys), on top of the pad
        // (in a menu where the game walks the character: WASD walks Mario, the buttons stay the menu's)
        let menu_walk = MENU_OPEN.load(Ordering::Relaxed) && MENU_WALK.load(Ordering::Relaxed);
        let gameplay = !m.dead && (!MENU_OPEN.load(Ordering::Relaxed) || menu_walk) && !FOLLOWING.load(Ordering::Relaxed);
        let fire_allowed = fire_ok && gameplay && !MENU_OPEN.load(Ordering::Relaxed) && !lakitu::first_person();
        if let Some(k) = kbd::read().filter(|_| gameplay && !lakitu::first_person()) {
            if inputs.stick_x == 0.0 && inputs.stick_y == 0.0 {
                inputs.stick_x = k.stick_x;
                inputs.stick_y = k.stick_y;
            }
            if !menu_walk {
                inputs.button_a |= k.a as u8;
                inputs.button_b |= k.b as u8;
                inputs.button_z |= k.z as u8;
            }
        }
        // frozen stiff or asleep (status.rs): no moving; asleep, presses wake him sooner
        let pressed = inputs.button_a != 0 || inputs.button_b != 0 || inputs.button_z != 0 || b_fire || b_dash || y_move || x_move;
        if status::holds(pressed, m.id) {
            inputs = sm64::SM64MarioInputs::default();
            (b_fire, b_dash, y_move, x_move, l_hammer, b_hook) = (false, false, false, false, false, false);
            dpad = [false; 4];
        }
        if let Ok(cam) = unsafe { CSCamera::instance() } {
            // the SM64 camera's direction when it's on (the game's own camera keeps running
            // underneath and may face elsewhere)
            let f = lakitu::forward().map(|v| (v.x, v.y, v.z)).unwrap_or_else(|| {
                let f = cam.pers_cam_1.forward();
                (f.0, f.1, f.2)
            });
            inputs.cam_look_x = -f.0;
            inputs.cam_look_z = f.2;
            // the star dance turns Mario to the camera's yaw, which libsm64 takes from the look
            // direction: flip it so he faces the camera instead of looking away from it
            if m.state.action == 0x0000_1307 {
                inputs.cam_look_x = -inputs.cam_look_x;
                inputs.cam_look_z = -inputs.cam_look_z;
            }
        }
        m.ticks += 1;
        let moving_changed = {
            let _span = perf::span(perf::MOVING);
            m.moving.update(&mut m.havok, m.origin, m.state.position)
        };
        if moving_changed {
            m.last_query = None;
        }
        let mut continue_tick = false;
        // (Mega Mario every tick: a giant crosses the read area faster)
        if moving_changed || m.ticks % 3 == 0 || mega::active() {
            let caster = collision::Caster { filter: m.filter, origin: m.origin, player: player_ref };
            let t0 = std::time::Instant::now();
            // real collision only needs refreshing when Mario has moved a bit (or every 0.5 s);
            // the same distance in metres for Mega Mario, whose walls reach farther out
            let p = glam::Vec3::from(m.state.position);
            let stale = m.last_query.is_none_or(|(q, tick)| p.distance(q) > 75.0 / size_factor() || m.ticks - tick >= 15);
            // on Torrent the game moves him and SM64's collision isn't used: no reading it at
            // riding speed, where it went stale every few frames (and straight away once he's off)
            let riding = RIDING.load(Ordering::Relaxed);
            if riding {
                m.last_query = None;
            }
            if riding || (!stale && m.last_query_havok) {
                continue_tick = true;
            }
            let (mut surfaces, from_havok) = if continue_tick {
                (Vec::new(), true)
            } else {
                let _span = perf::span(perf::COLLISION);
                match havok_surfaces(&mut m.havok, m.origin, m.state.position, player_ref) {
                    Some(s) => {
                        m.moving.watch_query(&m.havok);
                        m.last_query = Some((p, m.ticks));
                        m.last_query_havok = true;
                        (s, true)
                    }
                    None => {
                        m.last_query_havok = false;
                        (collision::build(&caster, m.state.position), false)
                    }
                }
            };
            let scan_ms = t0.elapsed().as_secs_f32() * 1000.0;
            if continue_tick {
                PERF.lock().unwrap_or_else(|e| e.into_inner()).scan(scan_ms, m.surfaces.len());
            }
            PERF.lock().unwrap_or_else(|e| e.into_inner()).scan(scan_ms, surfaces.len());
            // short memory of walls (last 4 scans, ~0.4 s) so one bad scan can't open a hole
            let walls: Vec<_> = surfaces.iter().filter(|s| collision::is_wall(s)).map(|s| collision::to_world(m.origin, s)).collect();
            m.wall_memory.push_back(if from_havok { Vec::new() } else { walls });
            while m.wall_memory.len() > 4 {
                m.wall_memory.pop_front();
            }
            for old in m.wall_memory.iter().rev().skip(1) {
                surfaces.extend(old.iter().map(|w| collision::from_world(m.origin, w)));
            }
            if continue_tick {
                // nothing to do: keep the loaded surfaces
            } else if surfaces.is_empty() && !from_havok {
                m.no_ground += 1;
            } else {
                m.no_ground = 0;
                // the query only reaches 3 m down around Mario (40 m in a column right under him):
                // past a high ledge SM64 would find no floor at all and treat the drop as out of
                // bounds (an invisible wall). A catch floor 45 m down, rebuilt with every query,
                // lets him step off; the real ground below is picked up as he falls.
                let (x, y, z) = (m.state.position[0] as i32, m.state.position[1] as i32 - 4500, m.state.position[2] as i32);
                let e = 8000;
                surfaces.push(sm64::SM64Surface::grass([[x - e, y, z - e], [x + e, y, z + e], [x + e, y, z - e]]));
                surfaces.push(sm64::SM64Surface::grass([[x - e, y, z - e], [x - e, y, z + e], [x + e, y, z + e]]));
                if let Some((t, feet)) = *FOLLOW_ENDED.lock().unwrap_or_else(|e| e.into_inner()) {
                    if t.elapsed().as_secs_f32() < 1.5 {
                        surfaces.extend(flat_floor(feet));
                    }
                }
                let _span = perf::span(perf::SURFACES);
                load_surfaces(&surfaces);
                m.surfaces = surfaces;
            }
        }
        let id = m.id;
        // characters Mario could hit this tick, and whether the Tarnished just got hurt
        let here = to_er(m.origin, m.state.position);
        let targets = {
            let _span = perf::span(perf::TARGETS);
            combat::nearby(&here, 8.0 * size_factor(), m.origin)
        };
        let no_stomp = m.combat.stomp_limits(&targets, m.state.action & 0x800 == 0);
        // breakable props (crates, jars, clutter, many tiny or invisible) never bounce Mario like a
        // stomped enemy when he drops onto them (he bounced off "nothing"); ground pounds still break them
        let mut no_stomp = no_stomp;
        no_stomp.extend(targets.iter().enumerate().filter(|(_, t)| t.is_prop()).map(|(i, _)| i));
        // (NPCs and allies: never hit, Y's moves near a merchant swept him)
        let target_pos: Vec<([f32; 3], f32, f32, usize)> =
            targets.iter().enumerate().filter(|(_, t)| !t.friendly()).map(|(i, t)| (t.sm, t.radius, t.height, i)).collect();
        // the enemy the next hit of a punch combo turns Mario to
        let aim = combat::aim(&m.state, &targets);
        // the enemy a falling Mario drifts onto, and the big bodies the camera keeps out of
        let home = combat::home(&m.state, &targets);
        lakitu::set_big(combat::big_bodies(&targets));
        let focus = combat::fight_focus(&m.state, &targets);
        coop::set_fighting(focus.is_some());
        lakitu::set_fight(focus);
        lakitu::set_crowd(combat::crowd(&targets));
        match lakitu::position() {
            Some(cam) => {
                let feet = to_er(m.origin, m.state.position);
                combat::fade_occluders(&targets, cam, glam::Vec3::new(feet.0, feet.1 + 0.7, feet.2));
            }
            None => combat::restore_faded(),
        }
        lakitu::set_climb(m.combat.climb(&targets));
        // the enemies worn down enough to ride: a mark over their heads
        {
            let feet = to_er(m.origin, m.state.position);
            bossride::watch(glam::Vec3::new(feet.0, feet.1, feet.2), m.origin);
        }
        // SM64's health is Mario's: Elden Ring hits cost wedges, and the Tarnished's HP is kept full
        // (a hit bigger than his whole HP would still kill him outright, a low-level character
        // against a late boss: the game's own "player can't die" flag stops him at 1 HP, and it
        // comes off when Mario himself is out of health, since his death is the Tarnished's)
        if let Ok(flags) = unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
            flags.player_no_dead = !m.dead;
            // the Vanish cap: enemy hits go through Mario
            // (and while he dashes)
            flags.all_no_hit = !m.dead && (caps::active() == Some(caps::VANISH) || moves::untouchable(m.state.action));
        }
        // Elden Ring's ailments and burns setting in on him (before his hurt: blood loss's wedge
        // costs no knockback)
        {
            let p = player_ref.chr_ins.modules.physics.position;
            let calm = !m.dead && !mega::active() && !RIDING.load(Ordering::Relaxed) && !FOLLOWING.load(Ordering::Relaxed) && !carry::holding() && !swing::holding();
            let data = &player_ref.chr_ins.modules.data;
            status::tick(&player_ref.chr_ins, m.id, glam::Vec3::new(p.0, p.1, p.2), calm, data.hp < data.max_hp);
        }
        let hurt = {
            let data = &player_ref.chr_ins.modules.data;
            // the enemy Mario holds (a boss by the tail, or a picked-up enemy) and one he just threw
            // can't hurt him (its swings broke the grab, its hitbox caught him running after it);
            // everyone else still can (carry.rs / swing.rs tell them apart by who's near)
            let pos = player_ref.chr_ins.modules.physics.position;
            let here = glam::Vec3::new(pos.0, pos.1, pos.2);
            let (attacker, scale) = combat::last_attacker(&player_ref.chr_ins);
            let damaged = m.combat.took_damage(data.hp, data.max_hp, scale);
            let was_damaged = damaged.is_some();
            if was_damaged {
                shove::hurt();
            }
            let from_held = was_damaged && (carry::harmless(here) || swing::harmless(here));
            // the game's own lava damage: Mario already pays for lava with SM64's lava boost
            // (also for a moment after: the burn keeps ticking while he's bounced up)
            static ON_LAVA: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            let mut on_lava = ON_LAVA.lock().unwrap_or_else(|e| e.into_inner());
            if LAVA_MATERIALS.contains(&player_ref.chr_ins.modules.physics.material_info.hit_material) {
                *on_lava = Some(std::time::Instant::now());
            }
            // (not in a boss fight: there's no telling the burn from his hits, and in an arena
            // full of lava every one of them was thrown away with it)
            let from_lava = on_lava.is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) && combat::boss_handles().is_empty();
            drop(on_lava);
            let hurt = damaged.filter(|_| !m.dead && !from_held && !from_lava);
            if debug() && was_damaged {
                log(format!(
                    "hurt: {} by {} (last carried {:#x}), hp {} of {}, mario action {:#x}",
                    if from_held { "ignored, from the enemy Mario holds or threw" } else if from_lava { "ignored, the game's lava damage" } else { "taken" },
                    attacker.map_or("nobody".to_string(), |id| format!("c{id:04}")),
                    carry::last_mob_key(),
                    data.hp,
                    data.max_hp,
                    m.state.action
                ));
                let effects: Vec<i32> = player_ref.chr_ins.special_effect.entries().map(|e| e.param_id).collect();
                log(format!("hurt: effects on him {effects:?}, floor material {}", player_ref.chr_ins.modules.physics.material_info.hit_material));
            }
            if !m.dead && data.hp > 0 {
                set_player_hp(data.max_hp);
                no_ailments();
            }
            hurt
        };
        // (a hit wakes him up)
        if matches!(hurt, Some(combat::Hurt::Hit(_))) {
            status::hit();
            // (whoever hit him from a neutral team is an enemy now: Caelid's soldiers)
            combat::learn_attacker(&player_ref.chr_ins);
        }
        // the Metal and Vanish caps: Elden Ring's hits do nothing (on Metal Mario they clang)
        if matches!(hurt, Some(combat::Hurt::Hit(_))) && caps::active() == Some(caps::METAL) {
            worker::call("metal clang", |_| unsafe { sm64::sm64_play_sound_global(SOUND_METAL_BONK) });
        }
        let hurt = hurt.filter(|_| !caps::shielded() && !moves::untouchable(m.state.action));
        // (up on a captured enemy: its back takes the blow, not him)
        let hurt = match hurt {
            Some(combat::Hurt::Hit(w)) if bossride::active() => {
                bossride::shield(w);
                None
            }
            Some(_) if bossride::active() => None,
            h => h,
        };
        if matches!(hurt, Some(combat::Hurt::Hit(_))) {
            let me = to_er(m.origin, m.state.position);
            impact::hurt(glam::Vec3::new(me.0, me.1 + 0.6, me.2));
            style::hurt();
            timing::hurt();
            // (a blow puts out the fireball he was charging)
            m.fire.douse(&m.state, m.origin, m.ticks);
        }
        // a blow that cost him nothing (a push, a bash): the Tarnished's damage animation says so
        let shoved = {
            let p = player_ref.chr_ins.modules.physics.position;
            let nearest = targets.iter().filter(|t| !t.friendly() && !t.is_prop()).map(|t| glam::Vec3::new(t.er.0 - p.0, t.er.1 - p.1, t.er.2 - p.2).length()).min_by(f32::total_cmp);
            let calm = hurt.is_none() && !bossride::active() && !status::just_set_in() && !m.dead && !caps::shielded() && !moves::untouchable(m.state.action) && !mega::active() && !blown::flying();
            let calm = calm && !RIDING.load(Ordering::Relaxed) && !FOLLOWING.load(Ordering::Relaxed) && !carry::holding() && !swing::holding();
            shove::check(current_anim(&player_ref.chr_ins), nearest, calm)
        };
        if shoved {
            m.fire.douse(&m.state, m.origin, m.ticks);
        }
        // healing: enemies Mario defeated drop coins (a wedge each when he touches one); a grace
        // or a boss's star refills him
        let kills = if m.dead {
            0
        } else {
            // (alone, the enemies Mario defeated drop coins; in co-op each player's game drops them
            // where enemies near them die, so each sees and takes their own)
            let fallen = m.combat.kills(m.ticks);
            if coop::active() {
                let near = targets.iter().filter_map(|t| t.shootable().map(|h| (t.key(), *h))).collect();
                for p in coins::watch(near) {
                    coins::spawn(p);
                }
            } else {
                for p in fallen {
                    coins::spawn(p);
                }
            }
            let me = to_er(m.origin, m.state.position);
            coins::here(&player_ref.chr_ins, m.state.action & 0x0800 == 0);
            let n = coins::collect(glam::Vec3::new(me.0, me.1, me.2));
            if let Some(amount) = coins::collect_bag(glam::Vec3::new(me.0, me.1, me.2)) {
                log(format!("coins: {amount} coins recovered"));
                style::callout("COINS BACK", None, [1.0, 0.85, 0.25], true, 0.0);
                worker::call("coins back", |_| unsafe { sm64::sm64_play_sound_volume(0x7828_9081_u32 as i32, 0.25) });
            }
            // coins behind walls and hills stay hidden: a ray from the camera to each coin
            if let (Ok(cam), Ok(havok)) = (unsafe { CSCamera::instance() }, unsafe { eldenring::cs::CSHavokMan::instance() }) {
                let c = cam.pers_cam_1.position();
                // (and behind Mario or anyone near him)
                let mut bodies = vec![(glam::Vec3::new(me.0, me.1, me.2), 0.4, 1.2)];
                bodies.extend(targets.iter().filter(|t| !t.is_prop()).map(|t| {
                    let p = to_er(m.origin, t.sm);
                    (glam::Vec3::new(p.0, p.1, p.2), t.radius * scale(), t.height * scale())
                }));
                coins::update_visibility(glam::Vec3::new(c.0, c.1, c.2), &bodies, |from, to| {
                    let d = to - from;
                    let len = d.length();
                    let start = HavokPosition(from.x, from.y, from.z, 0.0);
                    havok
                        .phys_world
                        .cast_ray(RAY_FILTER, &start, eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                        .is_some_and(|h| glam::Vec3::new(h.0, h.1, h.2).distance(from) < len - 0.2)
                });
            }
            if n > 0 {
                stats::update(|s| s.coins += n);
            }
            n
        };
        let rested = !m.dead && REST.swap(false, Ordering::Relaxed);
        let health_before = m.state.health;
        let head = lakitu::head();
        let grab = swing::take_start();
        let pick_up = carry::take_start();
        let put_down = carry::take_drop();
        let stagger_cue = swing::take_cue();
        let action_before = m.state.action;
        let alive = !m.dead;
        let hurt_from = targets
            .iter()
            .filter(|t| !shoved || (!t.friendly() && !t.is_prop()))
            .min_by(|a, b| {
                let d = |t: &combat::Target| glam::Vec3::from(t.sm).distance(glam::Vec3::from(m.state.position));
                d(a).total_cmp(&d(b))
            })
            .map(|t| t.sm)
            .unwrap_or(m.state.position);
        // a co-op partner's blow (coop.rs): pushed away from them, no wedge (SM64's X is ER's mirrored)
        let pushed = coop::take_shove().filter(|_| alive).map(|d| {
            let p = m.state.position;
            [p[0] + d.x * 100.0, p[1], p[2] - d.z * 100.0]
        });
        // stuck inside something (e.g. a lift that stopped around him): pushing the stick but not moving
        // (pushing against a wall is fine: SM64 plays its push / sidestep animation then), or hanging
        // in mid-air without moving
        const ANIM_PUSHING: i32 = 0x6C;
        const ANIM_SIDESTEP: [i32; 2] = [0x7F, 0x80];
        const ACT_FLAG_AIR: u32 = 0x800;
        let pushing = (inputs.stick_x * inputs.stick_x + inputs.stick_y * inputs.stick_y) > 0.25
            && m.state.anim_id != ANIM_PUSHING
            && !ANIM_SIDESTEP.contains(&m.state.anim_id);
        let hovering = m.state.action & ACT_FLAG_AIR != 0;
        let still = glam::Vec3::from(m.state.position).distance(glam::Vec3::from(m.prev_pos)) < 1.0;
        // standing still is normal with a boss by the tail
        if (pushing || hovering) && still && !m.dead && !FOLLOWING.load(Ordering::Relaxed) && !swing::holding() {
            m.stuck_ticks += 1;
        } else {
            m.stuck_ticks = 0;
        }
        // after 0.5 s: sunk into a floor? put him on top. After 3 s: lift him 1 m (again every 3 s)
        let unstick = m.stuck_ticks == 15;
        // F7: the player lifts him 1 m themselves (stuck somewhere the check above doesn't see)
        let f7 = {
            static WAS: AtomicBool = AtomicBool::new(false);
            let down = kbd::focused() && !MENU_OPEN.load(Ordering::Relaxed) && unsafe { GetAsyncKeyState(VK_F7) } as u16 & 0x8000 != 0;
            down && !WAS.swap(down, Ordering::Relaxed) || {
                WAS.store(down, Ordering::Relaxed);
                false
            }
        };
        let lift = m.stuck_ticks >= 90 || (f7 && !m.dead);
        if lift {
            m.stuck_ticks = 0;
        }
        let stuck_at = m.state.position;
        // no Y moves while the Tarnished is in an event animation (picking up, a grace, a door...)
        // or a menu is up
        let interacting = FOLLOWING.load(Ordering::Relaxed) || MENU_OPEN.load(Ordering::Relaxed) || game_driven(current_anim(&player_ref.chr_ins));
        // L1: the next weapon (held: the fists; not while holding anyone or riding: his hands are
        // busy, nor in the middle of one of the weapons' moves)
        let hands_free = fire_allowed && !carry::holding();
        let big = mega::active() || mega::changing();
        if big && m.moves.weapon != weapons::FISTS {
            m.moves.weapon = weapons::FISTS;
            weapons::set(weapons::FISTS);
            hammer::set_out(false);
            chomp::set_out(false);
        }
        // (in the middle of one of the weapons' moves too: the new one comes in swinging, moves.rs;
        // not while a fireball is being thrown)
        let kit_held = inputs.button_z != 0 && traps::crouching(m.state.action);
        {
            // LT + L1 while crouching: the stopwatch (stopwatch.rs)
            static LB_HELD: AtomicBool = AtomicBool::new(false);
            static LB_KIT: AtomicBool = AtomicBool::new(false);
            let was = LB_HELD.swap(l_hammer, Ordering::Relaxed);
            if kit_held && l_hammer && !was && fire_allowed && !big {
                let feet = to_er(m.origin, m.state.position);
                let facing = glam::Vec3::new(-m.state.face_angle.sin(), 0.0, m.state.face_angle.cos());
                stopwatch::use_it(glam::Vec3::new(feet.0, feet.1, feet.2), facing, m.origin);
            }
            // (pressed for the stopwatch, L1 is the kit's until it's let go: no weapon switch on
            // its release)
            LB_KIT.store(l_hammer && (LB_KIT.load(Ordering::Relaxed) || kit_held), Ordering::Relaxed);
            if LB_KIT.load(Ordering::Relaxed) {
                l_hammer = false;
            }
        }
        if let Some(w) = m.moves.switch_weapon(l_hammer && !kit_held, hands_free && !big && !rock::active() && !bossride::active() && !m.dead && !m.fire.throwing(m.ticks), m.state.action, m.ticks) {
            hammer::set_out(w == weapons::HAMMER);
            chomp::set_out(w == weapons::CHOMP);
            let hand = to_er(m.origin, m.state.position);
            impact::summon(glam::Vec3::new(hand.0, hand.1 + 0.7, hand.2), w != weapons::FISTS);
            if m.moves.swapping() {
                impact::swap(glam::Vec3::new(hand.0, hand.1, hand.2));
            }
        }
        let weapon_on = if hands_free { m.moves.weapon } else { weapons::FISTS };
        let rangs_ready = boomerang::ready() as i32;
        let juggling = juggle::active();
        // the trap kit: LT held while he crouches, plus X / Y / B / RB (traps.rs); those buttons are
        // the kit's alone then (no breakdance, dash, special or fireball)
        // (the boulder: none of his moves, X is its ram and A its hop, libsm64's)
        // (A on a boss's back jumps off it: its own press, SM64 doesn't get it there)
        let ride_jump = {
            static A_HELD: AtomicBool = AtomicBool::new(false);
            let a = inputs.button_a != 0;
            a && !A_HELD.swap(a, Ordering::Relaxed)
        };
        // (up on a captured enemy, X, Y and B are its attacks and its charge)
        let ride_keys = [x_move, y_move, b_dash];
        if rock::active() || bossride::active() {
            (x_move, y_move, b_dash, b_fire) = (false, false, false, false);
            if rock::active() {
                b_hook = false;
            }
            if bossride::active() {
                inputs.button_a = 0;
                inputs.button_b = 0;
            }
        }
        let kit_on = inputs.button_z != 0 && traps::crouching(m.state.action) && fire_allowed && !interacting && !big && !carry::holding() && !swing::holding() && !RIDING.load(Ordering::Relaxed);
        let kit_request = m.traps.input(kit_on, [x_move, y_move, b_dash, b_fire], m.ticks);
        hud::set_kit(kit_on.then(|| m.traps.ready()));
        // a button that went down for the kit stays the kit's until it's let go (held on, it
        // reached the dash, the fireball or the boomerang once the throw began)
        {
            static KIT_HELD: Mutex<[bool; 4]> = Mutex::new([false; 4]);
            let mut held = KIT_HELD.lock().unwrap_or_else(|e| e.into_inner());
            let now = [x_move, y_move, b_dash, b_fire];
            for k in 0..4 {
                held[k] = now[k] && (held[k] || kit_on);
            }
            if held[0] {
                x_move = false;
                inputs.button_b = 0;
            }
            if held[1] {
                y_move = false;
            }
            if held[2] {
                b_dash = false;
            }
            if held[3] {
                b_fire = false;
            }
        }
        let hold_y = y_move;
        // timing (timing.rs): A and X as pressed; an X just after a blow landed is a "NICE!"
        let a_pressed = timing::buttons(inputs.button_a != 0, inputs.button_b != 0, m.ticks);
        if let Some((h, attack, extra)) = timing::late_nice(m.ticks) {
            m.combat.extra_share(&h, attack, extra, m.ticks);
        }
        coop::set_own_health(m.state.health);
        // (the moves' tick runs every tick, so their own held buttons stay right)
        // where the stick points against where he faces (cos; 0 with the stick at rest): SM64's own
        // intended yaw (mario.c: atan2s(-stickY, stickX) plus the camera's yaw, libsm64.c flips the stick's X) against his facing
        let lean = {
            let (sx, sy) = (inputs.stick_x, inputs.stick_y);
            if sx * sx + sy * sy < 0.36 {
                0.0
            } else {
                ((-sx).atan2(-sy) + inputs.cam_look_x.atan2(inputs.cam_look_z) - m.state.face_angle).cos()
            }
        };
        // up on a big one's back: the stick steers it, R2 let go or A jumps off (bossride.rs)
        if bossride::active() {
            let (sx, sy) = (inputs.stick_x, inputs.stick_y);
            let mag = (sx * sx + sy * sy).sqrt().min(1.0);
            let yaw = (-sx).atan2(-sy) + inputs.cam_look_x.atan2(inputs.cam_look_z);
            let steer = if mag > 0.2 { glam::Vec3::new(-yaw.sin(), 0.0, yaw.cos()) * mag } else { glam::Vec3::ZERO };
            let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
            bossride::update(1.0 / 30.0, steer, b_hook, ride_jump, ride_keys, m.state.action, m.id, m.origin, &mut m.combat, &player_ref.chr_ins, m.ticks, |from, to| {
                let d = to - from;
                havok?
                    .phys_world
                    .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                    .map(|h| glam::Vec3::new(h.0, h.1, h.2))
            });
        }
        // the freeflow's enemy: the one the stick points at (within 35°), 3 to 12 m away, in sight
        let flow = {
            let (sx, sy) = (inputs.stick_x, inputs.stick_y);
            (sx * sx + sy * sy >= 0.36)
                .then(|| {
                    let yaw = (-sx).atan2(-sy) + inputs.cam_look_x.atan2(inputs.cam_look_z);
                    let dir = glam::Vec2::new(yaw.sin(), yaw.cos());
                    let me = m.state.position;
                    let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
                    let eye = to_er(m.origin, [me[0], me[1] + 80.0, me[2]]);
                    let seen = |t: &combat::Target| {
                        let c = to_er(m.origin, [t.sm[0], t.sm[1] + (t.height * 0.5).min(150.0), t.sm[2]]);
                        let d = eldenring::position::PositionDelta(c.0 - eye.0, c.1 - eye.1, c.2 - eye.2);
                        havok.is_none_or(|h| h.phys_world.cast_ray(RAY_FILTER, &eye, d, player_ref).is_none())
                    };
                    targets
                        .iter()
                        .filter(|t| t.shootable().is_some())
                        .filter_map(|t| {
                            let d = glam::Vec2::new(t.sm[0] - me[0], t.sm[2] - me[2]);
                            // (from the edge of its body: the boss he's hitting isn't the "next" one)
                            let metres = (d.length() - t.radius) * scale();
                            ((3.0..=12.0).contains(&metres) && d.normalize_or_zero().dot(dir) >= 0.82 && (t.sm[1] - me[1]).abs() * scale() < 3.0 && seen(t)).then_some((metres, t))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, t)| {
                            // (just off its body on his side, about its waist high: the hook's spot)
                            let d = glam::Vec2::new(me[0] - t.sm[0], me[2] - t.sm[2]).normalize_or(glam::Vec2::Y);
                            let off = t.radius + 70.0;
                            [t.sm[0] + d.x * off, t.sm[1] + (t.height * 0.35).min(250.0), t.sm[2] + d.y * off]
                        })
                })
                .flatten()
        };
        let moved = m.moves.tick(b_dash, y_move, x_move, fire_allowed, interacting, &m.state, m.combo.action_timer, lean, flow, m.ticks);
        let request = kit_request.or(moved);
        // the boulder off something big
        let request = request.or_else(|| rock::take_bounce(m.state.velocity).map(moves::Request::Velocity));
        // off the spring: up, higher than any jump
        let request = request.or_else(|| {
            let up = m.traps.take_bounce()?;
            let v = m.state.velocity;
            Some(moves::Request::Spring([v[0] * 0.5, up, v[2] * 0.5]))
        });
        // A just before an enemy's blow lands: the Mikiri (and while it lasts, its target follows
        // the enemy); A on nothing in particular stays SM64's jump
        let request = request.or_else(|| {
            let feet = to_er(m.origin, m.state.position);
            let ok = fire_allowed
                && !interacting
                && !big
                && !carry::holding()
                && !swing::holding()
                && !RIDING.load(Ordering::Relaxed)
                && hurt.is_none()
                && moves::can_counter(m.state.action);
            timing::mikiri(a_pressed, ok, glam::Vec3::new(feet.0, feet.1, feet.2), m.origin).map(|head| moves::Request::Mikiri(head, true))
        });
        let request = request.or_else(|| {
            (m.state.action == moves::ACT_ER_MIKIRI).then(|| timing::mikiri_target(m.origin)).flatten().map(|head| moves::Request::Mikiri(head, false))
        });
        // the Bros. Jump: falling onto the co-op partner's head, a high bounce off it
        let request = request.or_else(|| {
            static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            // (only out of a jump or a fall: not a ground pound, an air combo, a knockback, holding
            // anyone, on Yoshi)
            const JUMPS: [u32; 8] = [0x0300_0880, 0x0300_0881, 0x0100_0882, 0x0100_0883, 0x0100_0887, 0x0300_0888, 0x0300_0886, 0x0100_088C];
            if m.dead || !JUMPS.contains(&m.state.action) || m.state.velocity[1] >= 0.0 || !coop::active() || big || RIDING.load(Ordering::Relaxed) || carry::holding() || swing::holding() {
                return None;
            }
            let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
            if last.is_some_and(|t| t.elapsed().as_secs_f32() < 0.6) {
                return None;
            }
            let feet = to_er(m.origin, m.state.position);
            let feet = glam::Vec3::new(feet.0, feet.1, feet.2);
            let k = size_factor();
            let on = coop::partners().into_iter().find(|p| (p.feet - feet).with_y(0.0).length() < 0.45 * k && feet.y <= p.head.y + 0.4 * k && feet.y >= p.head.y - 0.25 * k)?;
            *last = Some(std::time::Instant::now());
            log(format!("coop: Bros. Jump off the partner's head ({})", if on.luigi { "Luigi" } else { "Mario" }));
            impact::bros_jump(on.head);
            let v = m.state.velocity;
            Some(moves::Request::Bounce([v[0] * 0.6, 62.0, v[2] * 0.6]))
        });
        {
            let p = player_ref.chr_ins.modules.physics.position;
            dodge::tick(m.state.action, glam::Vec3::new(p.0, p.1, p.2), m.ticks);
        }
        let no_punch = m.fire.throwing(m.ticks);
        let tt = std::time::Instant::now();
        let tick_span = perf::span(perf::TICK);
        // lava under the Tarnished (he stands where Mario does): SM64's lava boost
        let lava = !safe_lava(combat::lava_fight()) && LAVA_MATERIALS.contains(&player_ref.chr_ins.modules.physics.material_info.hit_material);
        let lift_k = size_factor();
        // a heavy blow from a boss or a big one: now and then he's sent flying (blown.rs)
        let still_flying = blown::flying();
        let blown_away = match hurt {
            Some(combat::Hurt::Hit(w)) if !still_flying && !grab::just_released() && !status::just_set_in() => {
                const ACT_FLAG_SWIMMING: u32 = 0x2000;
                const ACT_FLAG_ON_POLE: u32 = 0x0010_0000;
                const ACT_FLAG_HANGING: u32 = 0x0020_0000;
                let a = m.state.action;
                let free = a & (ACT_FLAG_SWIMMING | ACT_FLAG_ON_POLE | ACT_FLAG_HANGING) == 0 && a != moves::ACT_ER_ZIP && a != ACT_ER_LADDER;
                let calm = free && !m.dead && !mega::active() && !rock::active() && caps::active() != Some(caps::METAL) && !RIDING.load(Ordering::Relaxed);
                let calm = calm && !carry::holding() && !swing::holding() && !FOLLOWING.load(Ordering::Relaxed);
                let p = player_ref.chr_ins.modules.physics.position;
                calm && blown::roll(w, &player_ref.chr_ins, glam::Vec3::new(p.0, p.1, p.2))
            }
            _ => false,
        };
        let mario_at = m.state.position;
        let bled = status::just_set_in();
        // (the boulder: hits cost half, no knockback)
        let rocking = rock::active();
        // (L2 held: the grab and the hook's pull)
        let l2_held = inputs.button_z != 0;
        // (in the air L2 is SM64's ground pound: not for a press that the hook or R3 went with,
        // even once they're let go first; a fresh press in the air waits a few ticks for them)
        {
            static Z_EATEN: AtomicBool = AtomicBool::new(false);
            static Z_DOWN_AT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            static Z_WAS: AtomicBool = AtomicBool::new(false);
            // (a press on the ground isn't held back: L2 + A leaves the ground for the backflip at
            // once, and a held-back Z would come in the air as a fresh press, a ground pound)
            static Z_IN_AIR: AtomicBool = AtomicBool::new(false);
            if l2_held && !Z_WAS.swap(l2_held, Ordering::Relaxed) {
                Z_DOWN_AT.store(m.ticks, Ordering::Relaxed);
                Z_IN_AIR.store(m.state.action & 0x800 != 0, Ordering::Relaxed);
            }
            Z_WAS.store(l2_held, Ordering::Relaxed);
            if !l2_held {
                Z_EATEN.store(false, Ordering::Relaxed);
            } else if b_hook || r3 {
                Z_EATEN.store(true, Ordering::Relaxed);
            }
            let fresh = Z_IN_AIR.load(Ordering::Relaxed) && m.ticks.wrapping_sub(Z_DOWN_AT.load(Ordering::Relaxed)) < 3;
            if l2_held && m.state.action & 0x800 != 0 && (Z_EATEN.load(Ordering::Relaxed) || fresh) {
                inputs.button_z = 0;
            }
        }
        let result = worker::call("tick", move |ctx| {
            if lava {
                unsafe { sm64::sm64_er_lava(id) };
            }
            if unstick {
                // a floor just above his feet means he sank into it: put him on top
                let [x, y, z] = stuck_at;
                let top = unsafe { sm64::sm64_surface_find_floor_height(x, y + 150.0, z) };
                let mut hit: *mut std::ffi::c_void = std::ptr::null_mut();
                let ceil = unsafe { sm64::sm64_surface_find_ceil(x, y + 1.0, z, &mut hit) };
                // inside a solid platform there's no ceiling between his feet and its top (under a
                // table there is: leave him alone)
                if top > y + 2.0 && top < y + 150.0 && ceil > top {
                    unsafe { sm64::sm64_set_mario_position(id, x, top + 1.0, z) };
                    log(format!("unstuck: Mario was {:.0} units inside a floor, put on top", top - y));
                }
            }
            if lift {
                let [x, y, z] = stuck_at;
                unsafe { sm64::sm64_set_mario_position(id, x, y + 100.0 / lift_k, z) };
                log(if f7 { "unstuck: F7, lifted 1 m" } else { "unstuck: Mario stuck for 3 s, lifted 1 m" });
            }
            match hurt {
                // (flying off already: the blow costs health, SM64's knockback would end the flight)
                // (let go by an enemy: thrown off it already, the same)
                Some(combat::Hurt::Hit(wedges)) if still_flying || grab::just_released() || bled => unsafe {
                    sm64::sm64_set_mario_health(id, (health_before as i32 - wedges as i32 * 0x100).max(0xFF) as u16)
                },
                // (the boulder: half of it, and it rolls on)
                Some(combat::Hurt::Hit(wedges)) if rocking => unsafe {
                    sm64::sm64_set_mario_health(id, (health_before as i32 - wedges as i32 * 0x80).max(0xFF) as u16)
                },
                Some(combat::Hurt::Hit(wedges)) => unsafe {
                    sm64::sm64_mario_take_damage(id, wedges, 0, hurt_from[0], hurt_from[1], hurt_from[2]);
                    if blown_away {
                        blown::launch(id, mario_at, hurt_from);
                    }
                },
                Some(combat::Hurt::Drain) => unsafe { sm64::sm64_set_mario_health(id, (health_before - 0x100).max(0xFF) as u16) },
                // (a shove: SM64's soft knockback, no wedge)
                None if shoved => unsafe { sm64::sm64_mario_take_damage(id, 0, 0, hurt_from[0], hurt_from[1], hurt_from[2]) },
                None if pushed.is_some() => {
                    let [x, y, z] = pushed.unwrap_or_default();
                    unsafe { sm64::sm64_mario_take_damage(id, 0, 0, x, y, z) }
                }
                None => {}
            }
            if kills > 0 {
                // (a wedge and a half each)
                unsafe { sm64::sm64_mario_heal(id, (6 * kills).min(32) as u8) };
                unsafe { sm64::sm64_play_sound_volume(SOUND_COIN, 0.5) };
            }
            // Bowser's tail swing: Mario grabs the boss (SM64's pickup, swing and throw follow)
            if grab {
                unsafe { sm64::sm64_set_mario_action(id, swing::ACT_PICKING_UP_BOWSER) };
                unsafe { sm64::sm64_play_sound_global(swing::SOUND_GRAB) };
            }
            // an enemy picked up like a Bob-omb (SM64's pickup and carrying, carry.rs)
            if pick_up {
                unsafe { sm64::sm64_er_pick_up(id) };
            }
            if put_down {
                unsafe { sm64::sm64_er_drop(id) };
            }
            if stagger_cue {
                unsafe { sm64::sm64_play_sound_global(swing::SOUND_STAGGER) };
            }
            // SM64's C-up view: Mario's head looks where the camera looks
            // (and stands in SM64's first-person action: breathing, only the head moves)
            match head {
                Some((pitch, yaw)) => unsafe {
                    sm64::sm64_er_set_head(1, pitch * HEAD_PITCH_SIGN, yaw * HEAD_YAW_SIGN);
                    const ACT_FIRST_PERSON: u32 = 0x0C00_0227;
                    const ACT_FLAG_AIR: u32 = 0x800;
                    if action_before != ACT_FIRST_PERSON && action_before & ACT_FLAG_AIR == 0 {
                        sm64::sm64_set_mario_action(id, ACT_FIRST_PERSON);
                    }
                },
                None => unsafe { sm64::sm64_er_set_head(0, 0.0, 0.0) },
            }
            match aim {
                Some((x, z, gap)) => unsafe {
                    sm64::sm64_er_set_aim(1, x, z);
                    sm64::sm64_er_set_aim_gap(gap);
                },
                None => unsafe {
                    sm64::sm64_er_set_aim(0, 0.0, 0.0);
                    sm64::sm64_er_set_aim_gap(0.0);
                },
            }
            match home {
                Some((x, z)) => unsafe { sm64::sm64_er_set_home(1, x, z) },
                None => unsafe { sm64::sm64_er_set_home(0, 0.0, 0.0) },
            }
            if rested {
                unsafe { sm64::sm64_set_mario_health(id, 0x880) };
                unsafe { sm64::sm64_play_sound_global(SOUND_HEART) };
            }
            unsafe { sm64::sm64_er_set_weapon(weapon_on as i32, rangs_ready) };
            unsafe { sm64::sm64_er_set_juggle(juggling as i32) };
            unsafe { sm64::sm64_er_set_hold_y(hold_y as i32) };
            if let Some(r) = request {
                r.apply(id);
            }
            let mut state = sm64::SM64MarioState::default();
            let job_start = std::time::Instant::now();
            {
                let mut buffers = ctx.geo.buffers();
                unsafe { sm64::sm64_mario_tick(id, &inputs, &mut state, &mut *buffers) };
            }
            let mario_ms = job_start.elapsed().as_secs_f32() * 1000.0;
            // SM64's sound engine runs at the same 30 Hz as Mario
            let mut buf = [0i16; 544 * 2 * 2];
            let frames = (unsafe { sm64::sm64_audio_tick(audio::queued(), 1100, buf.as_mut_ptr()) }) as usize;
            audio::push(&buf[..(frames * 2 * 2).min(buf.len())]);
            let sound_ms = job_start.elapsed().as_secs_f32() * 1000.0 - mario_ms;
            let n = ctx.geo.used() * 9;
            let mut mats = vec![0f32; 64 * 16];
            let mut tri_part = vec![0i32; sm64::GEO_MAX_TRIANGLES];
            let count = unsafe {
                sm64::sm64_er_get_parts(mats.as_mut_ptr(), tri_part.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut())
            };
            // peace sign: SM64 swapped the right hand's mesh (more triangles than the fist)
            let right_hand = tri_part[..ctx.geo.used()].iter().filter(|&&p| p == 9).count();
            let peace = right_hand > engine_mario::FIST_TRIANGLES;
            let eye_cell = eye_cell(&ctx.geo.uv, ctx.geo.used());
            let mut parts = engine_mario::relative_parts(&mats, count, state.position, eye_cell, peace);
            let mut combo = sm64::SM64ErMove::default();
            unsafe { sm64::sm64_er_get_move(id, &mut combo) };
            // the hammer in his hand (out, and not swimming or in a cutscene)
            const ACT_GROUP_MASK: u32 = 0x1C0;
            let busy_hands = matches!(state.action & ACT_GROUP_MASK, 0x0C0 | 0x100) || peace;
            let hammer_swing = parts.as_mut().and_then(|p| hammer::pose(p, &state, &combo, hands_free && !busy_hands));
            // (the Chain Chomp's body hits like the hammer's head)
            let chomp_swing = parts.as_mut().and_then(|p| chomp::pose(p, &state, &combo, hands_free && !busy_hands));
            let swing = chomp_swing.or(hammer_swing);
            let hits = if alive { combat::hits(id, &state, &target_pos, &no_stomp, no_punch, &combo, swing) } else { Vec::new() };
            // (a long one: which part of it, for the log below)
            *LONG_TICK.lock().unwrap_or_else(|e| e.into_inner()) = (job_start.elapsed().as_secs_f32() * 1000.0, mario_ms, sound_ms, state.action);
            (state, ctx.geo.position[..n].to_vec(), ctx.geo.color[..n].to_vec(), ctx.geo.normal[..n].to_vec(), parts, hits, combo, swing)
        });
        drop(tick_span);
        // the log showed SM64 ticks of 40-75 ms (a hitch each): where they go
        {
            let took = tt.elapsed().as_secs_f32() * 1000.0;
            static LOGGED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            if took >= LONG_TICK_MS && LOGGED.fetch_add(1, Ordering::Relaxed) < 40 {
                let (job, mario, sound, action) = *LONG_TICK.lock().unwrap_or_else(|e| e.into_inner());
                log(format!(
                    "perf: a long SM64 tick, {took:.1} ms: {job:.1} in libsm64's thread (Mario {mario:.1}, sound {sound:.1}, poses and hits {:.1}), action {action:#x}",
                    job - mario - sound
                ));
            }
        }
        PERF.lock().unwrap_or_else(|e| e.into_inner()).tick_ms += tt.elapsed().as_secs_f32() * 1000.0;
        match result {
            Some((state, mesh, colors, normals, mut parts, hits, combo, swing)) => {
                // debug: Mario's centre crossing a loaded wall front to back in one tick
                let step = glam::Vec3::from(state.position) - glam::Vec3::from(m.state.position);
                if debug() && !FOLLOWING.load(Ordering::Relaxed) && step.length() < 200.0 {
                    if let Some((wall, da, db)) = crossed_wall(&m.surfaces, m.state.position, state.position) {
                        log(format!(
                            "through a wall: action {:#x} fwd vel {:.1} moved {:.1}, distance {da:.1} -> {db:.1}, layer {:#x}",
                            state.action, state.forward_velocity, step.length(), wall.force
                        ));
                    }
                }
                if m.state.action == moves::ACT_ER_MIKIRI && state.action != moves::ACT_ER_MIKIRI {
                    timing::mikiri_done();
                }
                m.combat.deal(&player_ref.chr_ins, &targets, &hits, m.ticks);
                // a MAX finisher: its show (maxfin.rs), and the one going on
                if let Some((attack, at)) = m.combat.take_max() {
                    let p = to_er(m.origin, state.position);
                    let dir = glam::Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
                    maxfin::fire(attack, at, glam::Vec3::new(p.0, p.1, p.2), dir, &mut m.combat, &player_ref.chr_ins, m.origin, m.ticks);
                }
                maxfin::tick(&mut m.combat, &player_ref.chr_ins, m.origin, m.ticks);
                // (the MAX ready: a twinkle at his hand now and then)
                if style::max_ready() && m.ticks % 8 == 0 {
                    if let Some(p) = m.parts.as_ref() {
                        let feet = to_er(m.origin, state.position);
                        impact::max_glow(glam::Vec3::new(feet.0, feet.1, feet.2) + p[9].pos);
                    }
                }
                // co-op host: the guest's blows (coop.rs)
                m.combat.remote_hits(m.ticks);
                {
                    let p = to_er(m.origin, state.position);
                    blown::tick(id, &state, glam::Vec3::new(p.0, p.1, p.2));
                    // the noise he makes, that the enemies hear (the game hears no steps of his)
                    const ACT_GROUND_POUND_LAND: u32 = 0x0080_023C;
                    const LOUD_FX: u32 = moves::FX_QUAKE | moves::FX_GIANT | moves::FX_METEOR | moves::FX_BANG;
                    let heavy = combo.fx & LOUD_FX != 0 || (state.action == ACT_GROUND_POUND_LAND && m.state.action != ACT_GROUND_POUND_LAND);
                    let attacking = !hits.is_empty() || moves::Hit::from(combo.hit).is_some() || state.flags & (0x0010_0000 | 0x0020_0000) != 0;
                    if !m.dead {
                        noise::tick(&player_ref.chr_ins, &state, &m.state, attacking, heavy);
                    }
                    if attacking {
                        lakitu::blow();
                    }
                    // a co-op partner in the way of a blow is pushed (no damage)
                    let me = player_ref.chr_ins.modules.physics.position;
                    coop::shove_partners(glam::Vec3::new(me.0, me.1, me.2), attacking && !m.dead);
                }
                // the dash's streaks (hud.rs), the meteor dive's too
                let streak = |a: u32| moves::dodging(a) || moves::streaking(a);
                if streak(state.action) {
                    let p = to_er(m.origin, state.position);
                    impact::dash_point(glam::Vec3::new(p.0, p.1, p.2), !streak(m.state.action));
                }
                // the combos' rings, stars and dust, and the hammer's swing ribbon
                {
                    let er = |p: glam::Vec3| {
                        let h = to_er(m.origin, p.into());
                        glam::Vec3::new(h.0, h.1, h.2)
                    };
                    let feet = er(state.position.into());
                    let head = swing.map(|s| er(s.head));
                    if combo.fx != 0 {
                        impact::combo(combo.fx, feet, head);
                    }
                    if let (Some(s), true) = (swing, moves::Hit::from(combo.hit).is_some()) {
                        // (the Chomp's: from its chain's far end, a lash of light behind it)
                        match weapon_on {
                            weapons::HAMMER => impact::swing_point(er(s.head), er(s.grip.lerp(s.head, 0.45))),
                            weapons::CHOMP => impact::chomp_point(er(s.head) + glam::Vec3::Y * 0.2, er(s.grip.lerp(s.head, 0.75)) + glam::Vec3::Y * 0.15, er(s.grip)),
                            _ => {}
                        }
                    }
                }
                m.combo = combo;
                {
                    let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
                    let ray = |from: glam::Vec3, to: glam::Vec3| {
                        let d = to - from;
                        havok?
                            .phys_world
                            .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                            .map(|h| glam::Vec3::new(h.0, h.1, h.2))
                    };
                    // another attack while a fireball charges puts the charge out (RB has to come up)
                    // (a button pressed now, not one held from before)
                    static PREV_ATTACKS: Mutex<[bool; 5]> = Mutex::new([false; 5]);
                    let attacks = [x_move, y_move, b_dash, b_hook, l_hammer];
                    let rising = {
                        let mut prev = PREV_ATTACKS.lock().unwrap_or_else(|e| e.into_inner());
                        let rising = attacks.iter().zip(prev.iter()).any(|(n, p)| *n && !*p);
                        *prev = attacks;
                        rising
                    };
                    if b_fire && m.fire.charging() && rising {
                        m.fire.put_out(&state, m.origin, m.ticks);
                    }
                    m.fire.tick(id, b_fire, fire_allowed, &combo, &state, m.origin, m.ticks, &player_ref.chr_ins, &mut m.combat, ray);
                    // R3: the parry (parry.rs); with L2 held, the grab (carry.rs: a small enemy, or a big one
                    // with its stance broken)
                    {
                        static PREV_R3: AtomicBool = AtomicBool::new(false);
                        let r3_press = r3 && !PREV_R3.swap(r3, Ordering::Relaxed);
                        PREV_R3.store(r3, Ordering::Relaxed);
                        let feet = to_er(m.origin, state.position);
                        let feet = glam::Vec3::new(feet.0, feet.1, feet.2);
                        // (not swimming, hanging, on a pole or a shell, zipping, on a ladder, in a
                        // cutscene action, rolling as the Rock: the palm thrust would cut those short)
                        let a = state.action;
                        let busy = a & (0x2000 | 0x0020_0000 | 0x0010_0000 | 0x0001_0000) != 0 || a & 0x1C0 == 0x100 || a == moves::ACT_ER_ZIP || a == moves::ACT_ER_POLE_SWING || a == ACT_ER_LADDER;
                        let free = fire_allowed && !m.dead && a & 0x0002_0000 == 0 && !busy && !mega::active() && !rock::active() && !bossride::active() && !swing::holding() && !carry::holding();
                        // (the grab is on his feet: SM64's pick-up actions are ground ones)
                        if r3_press && l2_held && free && a & 0x800 == 0 {
                            let face = glam::Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
                            if !carry::grab_nearest(feet, face, m.origin, id) {
                                worker::call("grab buzz", |_| unsafe { sm64::sm64_play_sound_volume(0x700E_0081, 0.35) });
                            }
                        }
                        m.parry.tick(id, r3, free && !l2_held, &state, feet, m.origin, m.ticks, &player_ref.chr_ins, &mut m.fire);
                    }
                    // the boomerangs: thrown when a throw lets go, from his right hand
                    let feet = to_er(m.origin, state.position);
                    let feet = glam::Vec3::new(feet.0, feet.1, feet.2);
                    let hand = parts.as_ref().map_or(feet + glam::Vec3::Y * 0.75, |p| feet + p[9].pos);
                    m.rang.tick(&combo, &state, feet, hand, weapon_on == weapons::RANG, m.origin, m.ticks, &player_ref.chr_ins, &mut m.combat, &ray);
                    if let Some(p) = parts.as_mut() {
                        // (none in his hand while he swims or is in a cutscene)
                        let busy = matches!(state.action & 0x1C0, 0x0C0 | 0x100);
                        m.rang.pose(p, feet, state.face_angle, weapon_on == weapons::RANG && hands_free && !busy, &combo, state.action);
                    }
                    // the trap kit's things, out there on their own
                    // (the spring's super jump only out of a jump or a fall, as the Bros. Jump)
                    const JUMPS: [u32; 8] = [0x0300_0880, 0x0300_0881, 0x0100_0882, 0x0100_0883, 0x0100_0887, 0x0300_0888, 0x0300_0886, 0x0100_088C];
                    let can_bounce = JUMPS.contains(&state.action) && !mega::active() && !RIDING.load(Ordering::Relaxed) && !carry::holding() && !swing::holding() && !m.dead;
                    m.traps.tick(&combo, &state, feet, hand, m.origin, m.ticks, &player_ref.chr_ins, &mut m.combat, can_bounce, &ray);
                    if let Some(p) = parts.as_mut() {
                        m.traps.pose(p, feet);
                        rock::pose(p, feet, state.face_angle);
                    }
                    // the hook: out of the same hand
                    m.hook.tick(id, b_hook, l2_held, x_move, a_pressed, fire_allowed && !swing::holding() && !bossride::active(), &combo, &state, feet, hand, m.origin, m.ticks, &player_ref.chr_ins, &mut m.combat, &ray);
                }
                let boss = unsafe { eldenring::cs::CSFeManImp::instance() }
                    .is_ok_and(|fe| fe.boss_health_displays.iter().any(|e| !e.field_ins_handle.is_empty()));
                // D-pad down: whatever power is on comes off (the cap: caps.rs; the Rock, the Mega)
                static DOWN_WAS: AtomicBool = AtomicBool::new(false);
                let power_off = dpad[1] && !DOWN_WAS.swap(dpad[1], Ordering::Relaxed) && !MENU_OPEN.load(Ordering::Relaxed);
                DOWN_WAS.store(dpad[1], Ordering::Relaxed);
                // D-pad left: the Rock Mushroom (the Metal Cap's old place: caps.rs gets no left)
                {
                    let feet = to_er(m.origin, state.position);
                    let free = gameplay && !MENU_OPEN.load(Ordering::Relaxed) && !lakitu::first_person() && !m.dead;
                    let can_stay = !m.dead && !RIDING.load(Ordering::Relaxed) && !FOLLOWING.load(Ordering::Relaxed) && !mega::active() && !carry::holding() && !swing::holding();
                    let can_start = free && can_stay && !mega::changing() && caps::active().is_none() && state.health >= 0x100;
                    rock::tick(dpad[2], power_off, can_start, can_stay, id, state.action, glam::Vec3::new(feet.0, feet.1, feet.2));
                }
                let dpad = [dpad[0], dpad[1], false, dpad[3]];
                if let Some(cap) = caps::tick(id, state.flags, dpad, gameplay && !MENU_OPEN.load(Ordering::Relaxed) && !rock::active(), boss) {
                    let p = to_er(m.origin, state.position);
                    impact::cap_change(glam::Vec3::new(p.0, p.1, p.2), cap);
                }
                // D-pad right: the Mega Mushroom (he grows on the ground, with room for a giant)
                let mega_change = {
                    let feet = to_er(m.origin, state.position);
                    let feet = glam::Vec3::new(feet.0, feet.1, feet.2);
                    let can_stay = !m.dead && !RIDING.load(Ordering::Relaxed);
                    let can_start = can_stay && gameplay && !MENU_OPEN.load(Ordering::Relaxed) && !lakitu::first_person();
                    const GROUNDED: u32 = 0x200 | 0x400;
                    let can_start = can_start && state.action & 0x800 == 0 && state.action & GROUNDED != 0 && caps::active().is_none() && !rock::active() && !carry::holding() && !swing::holding();
                    let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
                    let ray = |from: glam::Vec3, to: glam::Vec3| {
                        let d = to - from;
                        havok?
                            .phys_world
                            .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                            .map(|h| glam::Vec3::new(h.0, h.1, h.2))
                    };
                    mega::tick(dpad[3] && !MENU_OPEN.load(Ordering::Relaxed), power_off, can_start, can_stay, || mega::room(feet, ray), id, state.action, feet, m.ticks)
                };
                m.prev_parts = m.parts.take();
                m.parts = parts;
                static PARTS_LOGGED: AtomicBool = AtomicBool::new(false);
                if !PARTS_LOGGED.swap(true, Ordering::Relaxed) {
                    log(format!("engine mario: part poses {}", if m.parts.is_some() { "ok" } else { "MISSING (part count?)" }));
                }
                m.prev_mesh = std::mem::take(&mut m.mesh);
                m.prev_pos = m.state.position;
                m.mesh_color = colors;
                m.mesh_normal = normals;
                let (a, b) = (m.state.position, state.position);
                let jump = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                if jump > 200.0 {
                    log(format!("MARIO JUMP {jump:.0} units in one tick: {:?} -> {:?}", m.state, state));
                }
                m.state = state;
                m.mesh = mesh;
                // Mega Mario's steps crush whoever is under his feet, his landings shake them all
                if let Some((feet, landing)) = mega::take_quake() {
                    let (reach, attack) = if landing { (mega::LANDING_REACH, combat::Attack::Shockwave) } else { (mega::STEP_REACH, combat::Attack::Trample) };
                    let n = m.combat.blast(&player_ref.chr_ins, feet, reach, attack, m.origin, m.ticks);
                    if n > 0 && landing {
                        log(format!("mega: landing quake hit {n}"));
                    }
                }
                // Mega Mario grew or shrank: his world is scaled around where he stands (re-centred
                // on him, its collision read again at the new scale)
                if let Some(mega::Change::Scale(k)) = mega_change {
                    rescale(m, k, player_ref);
                    // (a ceiling only SM64's collision has, which the room check's rays can't see:
                    // no room for a giant after all)
                    if k > 1.0 && !giant_fits() {
                        log("mega: no room under the ceiling at his new size, back to his own");
                        mega::refuse();
                        rescale(m, 1.0, player_ref);
                    }
                }
                // out of health: the Tarnished dies too (the game's death, Bowser and respawn follow)
                if !m.dead && state.health < 0x100 {
                    log("Mario is out of health");
                    set_player_hp(0);
                }
            }
            None => {
                log(format!("last mario state before hang: {:?}", m.state));
                log(format!("surfaces loaded: {}", m.surfaces.len()));
                return;
            }
        }
    }

    // SM64's sound ticks with Mario: while he runs slower (the titan) it gets the ticks he skips, so
    // the music and the sounds keep their pace
    {
        static AUDIO_DEBT: Mutex<f32> = Mutex::new(0.0);
        let mut debt = AUDIO_DEBT.lock().unwrap_or_else(|e| e.into_inner());
        if titan < 1.0 && !paused {
            let ran = SM64_TICKS.load(Ordering::Relaxed).wrapping_sub(ticks_before);
            *debt = (*debt + dt.min(0.25) - ran as f32 / 30.0).clamp(-0.1, 0.25);
            while *debt >= 1.0 / 30.0 {
                *debt -= 1.0 / 30.0;
                worker::call("audio", |_| {
                    let mut buf = [0i16; 544 * 2 * 2];
                    let frames = unsafe { sm64::sm64_audio_tick(audio::queued(), 1100, buf.as_mut_ptr()) } as usize;
                    audio::push(&buf[..(frames * 2 * 2).min(buf.len())]);
                });
            }
        } else {
            *debt = 0.0;
        }
    }

    // keep SM64 coordinates small: re-centre its world on Mario when he travels far
    let [mx, my, mz] = m.state.position;
    // (not in a pole swing or a zip, whose centre and end are in the old frame, unless far out)
    let anchored = m.state.action == moves::ACT_ER_POLE_SWING || m.state.action == moves::ACT_ER_ZIP;
    let limit = if anchored { 8000.0 } else { 4000.0 };
    if mx.abs() > limit || mz.abs() > limit || my.abs() > limit {
        let here = collision::sm_to_er(m.origin, m.state.position);
        log(format!("re-centring SM64 world at {:?} (mario was at {:?})", (here.0, here.1, here.2), m.state.position));
        // moving objects live in SM64 coordinates: rebuild them in the new frame (moving them there
        // would drag Mario along ~40 m, since SM64 carries whoever stands on a platform)
        m.moving.clear(&mut m.havok);
        m.origin = [here.0, here.1, here.2];
        set_mario_position(m.id, [0.0, 0.0, 0.0]);
        m.state.position = [0.0, 0.0, 0.0];
        let caster = collision::Caster { filter: m.filter, origin: m.origin, player: player_ref };
        // (on Torrent this comes round every couple of seconds, and all SM64 needs under a
        // sitting Mario is some floor: the real one is read again when he's off)
        let surfaces = if RIDING.load(Ordering::Relaxed) {
            flat_floor([0.0, 0.0, 0.0]).into_iter().collect()
        } else {
            havok_surfaces(&mut m.havok, m.origin, [0.0, 0.0, 0.0], player_ref).unwrap_or_else(|| collision::build(&caster, [0.0, 0.0, 0.0]))
        };
        if !surfaces.is_empty() {
            load_surfaces(&surfaces);
            m.surfaces = surfaces;
        }
    }
    // safety net: no ground anywhere near for ~3 s -> back to where Mario mode started
    if m.no_ground > 30 {
        log("no ground for 3 s: Mario off, player returned to where Mario mode started");
        RETURN_HOME.store(true, Ordering::Relaxed);
        ENABLED.store(false, Ordering::Relaxed);
        return;
    }

    // debug Mario (F11): hide the Tarnished and draw Mario with the debug renderer instead
    // In a cutscene the game poses the character itself after Mario's pose is written (Mario
    // crumples and the Tarnished's face shows): the player is hidden then. A cutscene = the world
    // paused with no menu or prompt up, by the game's own signal (our "menu" guess from ignored
    // button presses fires in cutscenes too, they ignore input the same way).
    // (a loading screen pauses too: anim -1, not a cutscene)
    let cutscene = cutscene_now(m.dead, player_ref);
    {
        static IN_CUTSCENE: AtomicBool = AtomicBool::new(false);
        if IN_CUTSCENE.swap(cutscene, Ordering::Relaxed) != cutscene {
            log(format!("cutscene: {}", if cutscene { "player hidden" } else { "over, player shown" }));
        }
    }
    CUTSCENE_HIDE.store(cutscene, Ordering::Relaxed);
    // (see-through with the Vanish cap)
    static SHIMMER: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let vanish = caps::VANISH_OPACITY * (1.0 + 0.3 * (SHIMMER.get_or_init(std::time::Instant::now).elapsed().as_secs_f32() * 6.0).sin());
    // an enemy's grab (grab.rs): the game holds him, Mario follows; swallowed, he's hidden
    autopad::report(&player_ref.chr_ins);
    let grabbed = grab::update(&player_ref.chr_ins);
    // (held, his pose follows what the grab's animation does to the Tarnished's body)
    engine_mario::ROOT_FOLLOW.store(grabbed && !m.dead && !RIDING.load(Ordering::Relaxed), Ordering::Relaxed);
    set_opacity(if cutscene || grab::swallowed() { 0.0 } else if caps::active() == Some(caps::VANISH) { vanish } else { 1.0 });

    // event animations (fog walls, doors, ladders...): after an interact, if the Tarnished starts a
    // new animation, the game drives him and Mario follows until he's back to what he was doing
    {
        static ARMED: Mutex<Option<(std::time::Instant, i32)>> = Mutex::new(None);
        static FOLLOW: Mutex<Option<(std::time::Instant, i32, i32)>> = Mutex::new(None); // start, before, current
        let cur = current_anim(&player_ref.chr_ins);
        // (the game's "mounting" flag is no use here: it stays on when getting on is broken off
        // and drops a frame before "mounted" comes on, so the animation says it)
        // (no asking for the mount either: loading in already riding, the module doesn't have it)
        let riding = player_ref.chr_ins.modules.ride.is_mounted || mount_anim(cur);
        // debug, End: every boss on the bar down to his last few points, so one more hit of
        // Mario's is the one that takes him to his last (to try phase changes without the fight)
        {
            static HELD: AtomicBool = AtomicBool::new(false);
            let end = debug_key(0x23);
            if end && !HELD.swap(true, Ordering::Relaxed) {
                if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
                    for h in combat::boss_handles() {
                        if let Some(chr) = wcm.chr_ins_by_handle_mut(&h) {
                            log(format!("debug: c{:04} from {} HP to 2", chr.character_id, chr.modules.data.hp));
                            chr.modules.data.hp = chr.modules.data.hp.min(2);
                        }
                    }
                }
            } else if !end {
                HELD.store(false, Ordering::Relaxed);
            }
        }
        torrent_cant_die();
        yoshi::tick();
        trample::update(&mut m.combat, m.ticks, data.delta_time.time, yoshi::charge());
        // Yoshi's own moves while Mario rides him (ride.rs): X tongue, Y / RB eggs, L2 stomp, LB burst
        {
            let saddle = player_ref.chr_ins.modules.ride.is_mounted
                && !mount_anim(cur)
                && !m.dead
                && !MENU_OPEN.load(Ordering::Relaxed)
                && !WHISTLING.load(Ordering::Relaxed)
                && yoshi::active();
            let keys = kbd::read().filter(|_| saddle);
            let g = cam_pad.map(|p| p.Gamepad);
            let b = |bit| g.is_some_and(|g| g.wButtons.contains(bit));
            let buttons = ride::Buttons([
                b(XINPUT_GAMEPAD_X) || keys.as_ref().is_some_and(|k| k.b),
                b(XINPUT_GAMEPAD_Y) || (kbd::focused() && saddle && unsafe { GetAsyncKeyState(0x04) } as u16 & 0x8000 != 0),
                b(XINPUT_GAMEPAD_RIGHT_SHOULDER),
                g.is_some_and(|g| g.bLeftTrigger > 100) || keys.as_ref().is_some_and(|k| k.z),
                b(XINPUT_GAMEPAD_LEFT_SHOULDER),
            ]);
            let o = physics.orientation;
            let facing = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
            let havok = unsafe { eldenring::cs::CSHavokMan::instance() }.ok();
            let ray = |from: glam::Vec3, to: glam::Vec3| {
                let d = to - from;
                havok?
                    .phys_world
                    .cast_ray(RAY_FILTER, &HavokPosition(from.x, from.y, from.z, 0.0), eldenring::position::PositionDelta(d.x, d.y, d.z), player_ref)
                    .map(|h| glam::Vec3::new(h.0, h.1, h.2))
            };
            ride::tick(data.delta_time.time, saddle, buttons, yoshi::ridden(), facing, m.origin, &mut m.ticks, &player_ref.chr_ins, &mut m.combat, ray);
        }
        // the whistle itself isn't heard: Yoshi answers in its place (yoshi::call)
        if matches!(cur, 50190 | 50191) && yoshi::active() {
            let chr = &player_ref.chr_ins as *const eldenring::cs::ChrIns as *mut eldenring::cs::ChrIns;
            unsafe { (*chr).chr_flags1ca.set_sounds_active(false) };
        }
        {
            static GIVEN: AtomicBool = AtomicBool::new(false);
            if !GIVEN.swap(true, Ordering::Relaxed) {
                equip::give_whistle();
            }
        }
        if debug() {
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);
            let r = &player_ref.chr_ins.modules.ride;
            let raw = unsafe { *((&**r as *const _ as usize + 0x160) as *const u32) };
            let key = (r.is_mounting as u64) | (r.is_mounted as u64) << 1 | (r.has_ride_param as u64) << 2 | (r.is_ride_character as u64) << 3 | (r.last_mounted.is_some() as u64) << 4 | (raw as u64) << 8;
            if LAST.swap(key, Ordering::Relaxed) != key {
                log(format!(
                    "ride: mounting {} mounted {} ride param {} last mounted {} raw+0x160 {raw:#010x} (anim {cur})",
                    r.is_mounting, r.is_mounted, r.has_ride_param, r.last_mounted.is_some()
                ));
            }
        }
        let was_riding = RIDING.swap(riding, Ordering::Relaxed);
        if was_riding != riding {
            log(format!("ride: {} (anim {cur})", if riding { "on Torrent" } else { "off" }));
        }
        // In the saddle it's the rider enemies hit (he flinches, the mount with him, and a few
        // hits throw him off). The SM64 tick that turns the Tarnished's lost health into Mario's
        // doesn't run while the game drives, so it's done here: wedges off, his HP back to full.
        if riding && !m.dead {
            let (hp, max) = (player_ref.chr_ins.modules.data.hp, player_ref.chr_ins.modules.data.max_hp);
            // (half wedges, like the hits on foot)
            let halves = match m.combat.took_damage(hp, max, combat::last_attacker(&player_ref.chr_ins).1) {
                Some(combat::Hurt::Hit(n)) => n as i32,
                Some(combat::Hurt::Drain) => 2,
                None => 0,
            };
            if halves > 0 {
                let health = (m.state.health as i32 - halves * 0x80).max(0xFF);
                m.state.health = health as i16;
                let id = m.id;
                worker::call("ride hurt", move |_| unsafe { sm64::sm64_set_mario_health(id, health as u16) });
                log(format!("ride: hit in the saddle, {} wedge(s), health {health:#x}", halves as f32 / 2.0));
                if health < 0x100 {
                    log("Mario is out of health");
                    set_player_hp(0);
                }
            }
            if hp > 0 && m.state.health >= 0x100 {
                set_player_hp(max);
            }
        }
        // Yoshi's sprint runs on the Tarnished's stamina, and at twice the pace it's gone twice
        // as fast: out of it he dropped out of the sprint and stood there until asked again.
        if riding {
            if let Some(p) = (unsafe { WorldChrMan::instance_mut() }).ok().and_then(|w| w.main_player.as_mut()) {
                let d = &mut p.chr_ins.modules.data;
                d.stamina = d.max_stamina;
            }
        }
        let mut armed = ARMED.lock().unwrap_or_else(|e| e.into_inner());
        let mut follow = FOLLOW.lock().unwrap_or_else(|e| e.into_inner());
        if INTERACT_PRESSED.swap(false, Ordering::Relaxed) && follow.is_none() {
            *armed = Some((std::time::Instant::now(), cur));
        }
        // an enemy grabbed him: the game holds and moves him, Mario follows at once
        if follow.is_none() && grabbed {
            log(format!("follow: grabbed (anim {cur})"));
            FOLLOW_STARTED.store(true, Ordering::Relaxed);
            m.moving.clear(&mut m.havok);
            *follow = Some((std::time::Instant::now(), LAST_FREE_ANIM.load(Ordering::Relaxed), cur));
            *armed = None;
        }
        // any event animation (6xxxx: fog walls, doors, levers...) is game-driven
        if follow.is_none() && armed.is_none() && (game_driven(cur) || riding) {
            *armed = Some((std::time::Instant::now(), LAST_FREE_ANIM.load(Ordering::Relaxed)));
        }
        if !game_driven(cur) && follow.is_none() {
            LAST_FREE_ANIM.store(cur, Ordering::Relaxed);
        }
        if let Some((t, before)) = *armed {
            if (cur != before && game_driven(cur)) || riding {
                log(format!("follow: interact started anim {cur} (was {before})"));
                FOLLOW_STARTED.store(true, Ordering::Relaxed);
                // no moving / dynamic collision (the fog wall itself) while the game walks him
                m.moving.clear(&mut m.havok);
                *follow = Some((std::time::Instant::now(), before, cur));
                *armed = None;
            } else if t.elapsed().as_secs_f32() > 1.0 {
                *armed = None;
                // (once every few seconds at most: Y is also Mario's special move)
                static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
                let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
                if last.is_none_or(|l| l.elapsed().as_secs_f32() > 3.0) {
                    *last = Some(std::time::Instant::now());
                    let st = INTERACT_STATE.load(Ordering::Relaxed);
                    log(format!(
                        "input: interact (D-pad down) pressed, no event animation after 1 s (allowed {}, blocked {}, queued {}; tarnished anim {before} -> {cur}, Mario action {:#x})",
                        st & 1 != 0,
                        st & 2 != 0,
                        st & 4 != 0,
                        m.state.action
                    ));
                }
            }
        }
        if let Some((t, before, last)) = *follow {
            if cur != last {
                log(format!("follow: anim {last} -> {cur}"));
                if ladder_anim(cur) {
                    let o = physics.orientation;
                    let f = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
                    log(format!(
                        "ladder: Tarnished at {:.2?} facing {f:.2?}, Mario face angle {:.2}",
                        (physics.position.0, physics.position.1, physics.position.2),
                        m.state.face_angle
                    ));
                }
                *follow = Some((t, before, cur));
            }
            let _ = before;
            // (a long ladder takes a while)
            let limit = if ladder_anim(last) { 60.0 } else { 15.0 };
            if !(game_driven(cur) || riding || grabbed) || (!riding && t.elapsed().as_secs_f32() > limit) {
                log("follow: done");
                if grabbed {
                    grab::ignore_current(&player_ref.chr_ins);
                }
                // let go by an enemy: thrown off it
                let release = grab::take_release();
                if m.state.action == grab::ACT_ER_GRABBED {
                    let id = m.id;
                    let release = release.unwrap_or(grab::Release { swallowed: false });
                    worker::call("grab release", move |_| grab::throw_off(id, &release));
                    m.state.action = grab::ACT_THROWN_BACKWARD;
                }
                // off the ladder or off Torrent: SM64 takes over, dropping him onto the floor he's on
                if m.state.action == ACT_ER_LADDER || m.state.action == ACT_ER_RIDE {
                    let id = m.id;
                    worker::call("ladder off", move |_| unsafe {
                        sm64::sm64_er_set_ladder(0.0);
                        sm64::sm64_set_mario_action(id, ACT_FREEFALL);
                    });
                    m.state.action = ACT_FREEFALL;
                }
                let trace = std::mem::take(&mut *FOLLOW_TRACE.lock().unwrap_or_else(|e| e.into_inner()));
                log(format!("follow: SM64 action/anim per tick: {}", trace.join(" ")));
                let secs = t.elapsed().as_secs_f32();
                log(format!(
                    "follow: {} SM64 ticks in {secs:.1} s ({:.0}/s, should be 30)",
                    FOLLOW_TICKS.swap(0, Ordering::Relaxed),
                    trace.len() as f32 / secs.max(0.01)
                ));
                *follow = None;
                // the real collision again, with a floor under his feet for a moment (the area past
                // a fog wall may still be loading in)
                m.last_query = None;
                *FOLLOW_ENDED.lock().unwrap_or_else(|e| e.into_inner()) = Some((std::time::Instant::now(), m.state.position));
            }
        }
        FOLLOWING.store(follow.is_some(), Ordering::Relaxed);
        ON_LADDER.store(follow.is_some() && ladder_anim(cur), Ordering::Relaxed);
        if follow.is_some() {
            let p = physics.position;
            let sm = collision::er_to_sm(m.origin, &p);
            // Mario walks along (SM64's own walk, turned the way the Tarnished goes): a gentle stick
            // push in his direction at his speed; the position stays the game's
            // (his movement measured tick to tick and smoothed: the game moves him in uneven steps)
            struct Walk {
                pos: [f32; 3],
                acc: f32,
                speed: f32,
                still: f32,
                climb: f32,
            }
            static WALK: Mutex<Option<Walk>> = Mutex::new(None);
            let mut walk = WALK.lock().unwrap_or_else(|e| e.into_inner());
            let w = walk.get_or_insert(Walk { pos: [p.0, p.1, p.2], acc: 0.0, speed: 0.0, still: 1.0, climb: 0.0 });
            w.acc += data.delta_time.time.max(1e-3);
            let tick = w.acc >= 1.0 / 30.0;
            // time since the last tick (the leftover after 1/30 s is kept, like the normal loop:
            // resetting it to 0 dropped follow mode to ~20-27 ticks/s at 60 fps)
            let since_tick = w.acc;
            if tick {
                let d = glam::Vec3::new(p.0 - w.pos[0], 0.0, p.2 - w.pos[2]);
                let v = d.length() / since_tick;
                if v < 15.0 {
                    // (teleport-sized jumps don't count)
                    w.speed += (v - w.speed) * 0.4;
                }
                w.still = if w.speed < 0.2 { w.still + since_tick } else { 0.0 };
                let vy = (p.1 - w.pos[1]).abs() / since_tick;
                if vy < 15.0 {
                    w.climb += (vy - w.climb) * 0.4;
                }
                w.pos = [p.0, p.1, p.2];
                // (at most one tick behind: no burst of ticks after a hitch)
                w.acc = (w.acc - 1.0 / 30.0).min(1.0 / 30.0);
            }
            let (speed, walking, climb) = (w.speed, w.still < 0.3, w.climb);
            let on_ladder = ladder_anim(current_anim(&player_ref.chr_ins));
            // how far between two SM64 ticks this frame is (the pose is blended, like in play)
            let alpha = (w.acc * 30.0).clamp(0.0, 1.0);
            drop(walk);
            // the way he faces (steady), not his step-by-step movement (which can jump about)
            let o = physics.orientation;
            let facing = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
            let dir = glam::Vec3::new(facing.x, 0.0, facing.z).normalize_or_zero();
            if tick {
                SM64_TICKS.fetch_add(1, Ordering::Relaxed);
                FOLLOW_TICKS.fetch_add(1, Ordering::Relaxed);
                let mut inputs = sm64::SM64MarioInputs::default();
                // walking pace (SM64's full stick runs at ~9 m/s); teleport-sized jumps don't count
                if walking && dir != glam::Vec3::ZERO && !on_ladder && !riding && !grabbed {
                    inputs.cam_look_x = -dir.x;
                    inputs.cam_look_z = dir.z;
                    // a clear walking pace: at the Tarnished's slow speed SM64 would sit on the edge
                    // between tiptoeing and walking and flip between them (his position is the
                    // game's anyway)
                    let _ = speed;
                    inputs.stick_y = -0.6;
                }
                let id = m.id;
                // the Tarnished's facing as SM64's face angle (forward = (-sin a, 0, cos a))
                let face = (-dir.x).atan2(dir.z);
                // the floors SM64 already has plus a flat one at his feet, but no walls (fog walls
                // and door frames would make him push against them); walls come back afterwards
                // (reloaded only when he's moved away from where they were loaded: every reload
                // makes SM64 look for his floor again, which can flicker his pose)
                static LOADED_AT: Mutex<Option<[f32; 3]>> = Mutex::new(None);
                let mut loaded_at = LOADED_AT.lock().unwrap_or_else(|e| e.into_inner());
                let reload = FOLLOW_STARTED.swap(false, Ordering::Relaxed)
                    || loaded_at.is_none_or(|q| ((q[0] - sm[0]).powi(2) + (q[2] - sm[2]).powi(2)).sqrt() > 500.0 || (q[1] - sm[1]).abs() > 100.0);
                // (on Torrent no floor is needed: he sits, SM64 doesn't move him)
                let floors: Option<Vec<sm64::SM64Surface>> = (reload && !riding).then(|| {
                    *loaded_at = Some(sm);
                    let mut f: Vec<sm64::SM64Surface> = m.surfaces.iter().filter(|s| !collision::is_wall(s)).copied().collect();
                    f.extend(flat_floor(sm));
                    f
                });
                drop(loaded_at);
                let ladder_was = m.state.action == ACT_ER_LADDER;
                let grabbed_was = m.state.action == grab::ACT_ER_GRABBED;
                // (not on a mount, nor once he's dead: then he just goes limp with the game)
                let dangle = grabbed && !riding && !m.dead;
                let sitting = m.state.action == ACT_ER_RIDE;
                let parts = worker::call("follow tick", move |ctx| {
                    if let Some(floors) = &floors {
                        unsafe { sm64::sm64_static_surfaces_load(floors.as_ptr(), floors.len() as u32) };
                    }
                    // exactly the Tarnished's facing (the game steers here; SM64 turning on its own
                    // made the two disagree)
                    if dir != glam::Vec3::ZERO {
                        unsafe { sm64::sm64_set_mario_faceangle(id, face) };
                    }
                    unsafe { sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]) };
                    // held by an enemy: he dangles in its grip
                    if dangle && !grabbed_was {
                        unsafe { sm64::sm64_set_mario_action(id, grab::ACT_ER_GRABBED) };
                    }
                    // on Torrent: he sits (SM64's slide pose)
                    if riding && !sitting {
                        unsafe { sm64::sm64_set_mario_action(id, ACT_ER_RIDE) };
                    }
                    // on a ladder: SM64's pole climb, at a pace from how fast the game moves him
                    // (~1.5 m/s climbing = SM64's quick climb)
                    if on_ladder {
                        unsafe {
                            sm64::sm64_er_set_ladder(if climb > 0.2 { (climb * 1.2).clamp(0.5, 2.5) } else { 0.0 });
                            if !ladder_was {
                                sm64::sm64_set_mario_action(id, ACT_ER_LADDER);
                            }
                        }
                    }
                    let mut state = sm64::SM64MarioState::default();
                    {
                        let mut buffers = ctx.geo.buffers();
                        unsafe { sm64::sm64_mario_tick(id, &inputs, &mut state, &mut *buffers) };
                    }
                    unsafe { sm64::sm64_set_mario_position(id, sm[0], sm[1], sm[2]) };
                    let mut mats = vec![0f32; 64 * 16];
                    let mut tri_part = vec![0i32; sm64::GEO_MAX_TRIANGLES];
                    let count = unsafe {
                        sm64::sm64_er_get_parts(mats.as_mut_ptr(), tri_part.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut())
                    };
                    // (relative to where the step left him, not where it started: his parts would
                    // shift by each step's own movement)
                    let eyes = eye_cell(&ctx.geo.uv, ctx.geo.used());
                    (state, engine_mario::relative_parts(&mats, count, state.position, eyes, false))
                });
                if let Some((state, parts)) = parts {
                    FOLLOW_TRACE.lock().unwrap_or_else(|e| e.into_inner()).push(format!("{:x}/{}", state.action & 0x1FF, state.anim_id));
                    m.state.face_angle = state.face_angle;
                    m.state.action = state.action;
                    if parts.is_some() {
                        m.prev_parts = m.parts.take();
                        m.parts = parts;
                    }
                }
            }
            m.state.position = sm;
            m.prev_pos = sm;
            m.last_set = Some([p.0, p.1, p.2]);
            physics.gravity_disabled = false;
            // (the game steers the Tarnished here; Mario's SM64 facing is set to his every tick,
            // and the pose converted with that same tick's facing, so it stays put on his body
            // between ticks while the model turns with the Tarnished)
            let q = glam::Quat::from_rotation_y(PI - m.state.face_angle);
            *engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()) = match (&m.prev_parts, &m.parts) {
                (Some(a), Some(b)) => Some(seat(engine_mario::to_character(&engine_mario::blend(a, b, alpha), q))),
                (None, Some(b)) => Some(seat(engine_mario::to_character(b, q))),
                _ => None,
            };
            return;
        }
    }

    // F2 (debug): every Site of Grace unlocked (their "lit" event flags from BonfireWarpParam)
    // and the whole map revealed (WorldMapPieceParam), for testing around the world.
    // Offline Mario save only.
    {
        static F2_WAS: AtomicBool = AtomicBool::new(false);
        let f2 = debug_key(0x71);
        if f2 && !F2_WAS.swap(true, Ordering::Relaxed) {
            use eldenring::cs::{BonfireWarpParam, CSEventFlagMan, SoloParamRepository};
            if let (Ok(repo), Ok(flags)) = (unsafe { SoloParamRepository::instance() }, unsafe { CSEventFlagMan::instance_mut() }) {
                let mut n = 0;
                for i in 0..4000 {
                    let Some(row) = repo.get_row_by_index::<BonfireWarpParam>(i) else { break };
                    let flag = row.eventflag_id();
                    if flag != 0 {
                        flags.virtual_memory_flag.set_flag(flag, true);
                        n += 1;
                    }
                }
                log(format!("graces: {n} unlocked"));
                let mut pieces = 0;
                for i in 0..1000 {
                    let Some(row) = repo.get_row_by_index::<eldenring::cs::WorldMapPieceParam>(i) else { break };
                    for flag in [row.open_event_flag_id(), row.acquisition_event_flag_id()] {
                        if flag != 0 {
                            flags.virtual_memory_flag.set_flag(flag, true);
                        }
                    }
                    pieces += 1;
                }
                log(format!("map: {pieces} pieces revealed"));
            }
        } else if !f2 {
            F2_WAS.store(false, Ordering::Relaxed);
        }
    }

    // F5: probe the ground 1.5 m in front of Mario (which body/layer is there, and the game's ray)
    {
        static F5_WAS: AtomicBool = AtomicBool::new(false);
        let f5 = debug_key(0x74);
        if f5 && !F5_WAS.swap(true, Ordering::Relaxed) {
            let fa = m.state.face_angle;
            let here = to_er(m.origin, m.state.position);
            let p = glam::Vec3::new(here.0 - fa.sin() * 0.15, here.1, here.2 + fa.cos() * 0.15);
            // SM64's floor triangles whose x/z box contains that point
            let sp = collision::er_to_sm(m.origin, &HavokPosition(p.x, p.y, p.z, 0.0));
            for surf in &m.surfaces {
                let v = surf.vertices.map(|q| glam::Vec3::new(q[0] as f32, q[1] as f32, q[2] as f32));
                let n = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero();
                let lo = v[0].min(v[1]).min(v[2]);
                let hi = v[0].max(v[1]).max(v[2]);
                if lo.x <= sp[0] && hi.x >= sp[0] && lo.z <= sp[2] && hi.z >= sp[2] && (lo.y - sp[1]).abs() < 150.0 {
                    log(format!("  sm64 surface over crack (n.y {:.2}, layer {:#x}): {:?}", n.y, surf.force, v));
                }
            }
            if let Ok(havok) = unsafe { eldenring::cs::CSHavokMan::instance() } {
                let start = HavokPosition(p.x, p.y + 2.0, p.z, 0.0);
                let hit = havok.phys_world.cast_ray(m.filter, &start, eldenring::position::PositionDelta(0.0, -6.0, 0.0), player_ref);
                log(format!("F5 probe: game ray (filter {:#x}) hits {:?}", m.filter, hit.map(|h| h.1)));
            }
            for line in m.havok.probe(p) {
                log(line);
            }
            log("F5 bodies around Mario (6 m):");
            for line in m.havok.bodies_around(p, 6.0) {
                log(line);
            }
            for line in m.moving.describe(&m.havok, m.origin, m.state.position) {
                log(line);
            }
            for layer in [0x3a, 0x39] {
                for line in havok_col::dump_layer_near(p, 6.0, layer).into_iter().take(12) {
                    log(line);
                }
            }
            // SM64's own view: loaded wall-ish triangles near Mario (within 120 units, Mario's height band)
            let [mx, my, mz] = m.state.position;
            for surf in &m.surfaces {
                let v = surf.vertices.map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
                let n = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero();
                if n.y > 0.01 {
                    continue; // floors
                }
                let lo = v[0].min(v[1]).min(v[2]);
                let hi = v[0].max(v[1]).max(v[2]);
                let near = lo.x < mx + 120.0 && hi.x > mx - 120.0 && lo.z < mz + 120.0 && hi.z > mz - 120.0
                    && hi.y > my - 20.0 && lo.y < my + 260.0;
                if near {
                    log(format!("  sm64 {} layer {:#x}: {:?} n {n:?} (mario at {:?})", if n.y < -0.01 { "CEILING" } else { "wall" }, surf.force, v, m.state.position));
                }
            }
            // SM64's own view along Mario's facing: what his ground step would see
            {
                let (pos, fa) = (m.state.position, m.state.face_angle);
                let rows = worker::call("probe", move |_| {
                    let mut rows = Vec::new();
                    for k in 0..16 {
                        let d = k as f32 * 10.0;
                        let (x, z) = (pos[0] + fa.sin() * d, pos[2] + fa.cos() * d);
                        let floor = unsafe { sm64::sm64_surface_find_floor_height(x, pos[1] + 100.0, z) };
                        let mut hit: *mut std::ffi::c_void = std::ptr::null_mut();
                        let ceil = unsafe { sm64::sm64_surface_find_ceil(x, floor + 80.0, z, &mut hit) };
                        let wall = |off: f32, r: f32| {
                            let (mut wx, mut wy, mut wz) = (x, floor.max(pos[1] - 100.0), z);
                            let n = unsafe { sm64::sm64_surface_find_wall_collision(&mut wx, &mut wy, &mut wz, off, r) };
                            (n, ((wx - x).powi(2) + (wz - z).powi(2)).sqrt())
                        };
                        let (lo, hi) = (wall(30.0, 24.0), wall(60.0, 50.0));
                        rows.push(format!(
                            "  +{d:>3}: floor {floor:>8.1} ceil {ceil:>8.1} gap {:>7.1} | walls lo {} (push {:.0}) hi {} (push {:.0})",
                            ceil - floor, lo.0, lo.1, hi.0, hi.1
                        ));
                    }
                    rows
                });
                log(format!("F5 sm64 path from {:?} facing {:.2}:", m.state.position, m.state.face_angle));
                for r in rows.unwrap_or_default() {
                    log(r);
                }
            }
            m.havok.clear_cache();
            log("F5: mesh cache cleared, probing again");
            for line in m.havok.probe(p) {
                log(line);
            }
            m.last_query = None;
        } else if !f5 {
            F5_WAS.store(false, Ordering::Relaxed);
        }
    }

    // move the Tarnished to Mario
    let alpha = (m.acc * 30.0).clamp(0.0, 1.0);
    let (a, b) = (glam::Vec3::from(m.prev_pos), glam::Vec3::from(m.state.position));
    // big jumps (re-centring, teleports) snap instead of sliding across the map
    let smooth = if a.distance(b) < 200.0 { a.lerp(b, alpha) } else { b };
    let pos = to_er(m.origin, smooth.into());
    m.last_set = Some([pos.0, pos.1, pos.2]);

    // positional voice: pan by Mario's side of the camera, quieter with distance
    if let Ok(cam) = unsafe { CSCamera::instance() } {
        let cp = cam.pers_cam_1.position();
        let right = cam.pers_cam_1.right();
        let to = glam::Vec3::new(pos.0 - cp.0, pos.1 + 0.8 - cp.1, pos.2 - cp.2);
        let dist = to.length().max(0.01);
        let pan = glam::Vec3::new(right.0, right.1, right.2).normalize_or_zero().dot(to / dist);
        let volume = (1.0 / (1.0 + (dist - 3.0).max(0.0) * 0.12)).clamp(0.0, 1.0);
        // (with the wing cap SM64's flight is loud: its whooshes and flaps play all along)
        let volume = volume * if caps::active() == Some(caps::WING) { WING_VOLUME } else { 1.0 };
        audio::set_position(pan, volume);
    }
    physics.position = pos;
    // airborne: SM64 flies him; grounded: gravity on so his capsule really stands (the game only
    // allows interactions like doors when it thinks he's on the ground)
    if !HANDS_OFF.load(Ordering::Relaxed) {
        physics.gravity_disabled = m.state.action & 0x0000_0800 != 0;
    } else {
        physics.gravity_disabled = false;
    }
    if EXPERIMENT.load(Ordering::Relaxed) < 2 {
        physics.chr_proxy_pos_update_requested = true;
    }
    // SM64 owns falling: keep the Tarnished "standing" while Mario is grounded, so the game allows
    // interactions (doors, chests...), and never let Elden Ring's fall timer/damage/motion run
    const ACT_FLAG_AIR: u32 = 0x0000_0800;
    if m.state.action & ACT_FLAG_AIR == 0 && !HANDS_OFF.load(Ordering::Relaxed) {
        physics.is_falling = false;
        physics.is_touching_ground = true;
        physics.standing_on_solid_ground = true;
        physics.touching_solid_ground = true;
    }
    let q = glam::Quat::from_rotation_y(PI - m.state.face_angle);
    physics.orientation = Quaternion(q.x, q.y, q.z, q.w);

    // engine Mario: part poses in character space, applied to the skeleton by engine_mario::apply
    *engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()) = match (&m.prev_parts, &m.parts) {
        (Some(a), Some(b)) => Some(engine_mario::to_character(&engine_mario::blend(a, b, alpha), q)),
        (None, Some(b)) => Some(engine_mario::to_character(b, q)),
        _ => None,
    }
;

    if EZ_MARIO.load(Ordering::Relaxed) {
        // wireframe of libsm64's own mesh over the engine model, to compare
        if let Some(draw) = unsafe { RendMan::instance_mut() }.ok().map(|r| r.debug_ez_draw.as_mut()) {
            draw.set_color(&F32Vector4(1.0, 0.1, 0.1, 1.0));
            let v = &m.mesh;
            for t in 0..v.len() / 9 {
                let p = |k: usize| {
                    let i = (t * 3 + k) * 3;
                    to_er(m.origin, [v[i], v[i + 1], v[i + 2]])
                };
                let (a, b, c) = (p(0), p(1), p(2));
                draw.draw_line(&a, &b);
                draw.draw_line(&b, &c);
                draw.draw_line(&c, &a);
            }
        }
    }

    // debug-draw Mario's real model as a wireframe
    static DRAW_WAS_DOWN: AtomicBool = AtomicBool::new(false);
    let f8 = debug_key(0x77);
    if f8 && !DRAW_WAS_DOWN.swap(true, Ordering::Relaxed) {
        DEBUG_DRAW.fetch_xor(true, Ordering::Relaxed);
    } else if !f8 {
        DRAW_WAS_DOWN.store(false, Ordering::Relaxed);
    }
    if !DEBUG_DRAW.load(Ordering::Relaxed) {
        return;
    }
    let dt = std::time::Instant::now();
    let _draw_timer = DrawTimer(dt);
    if let Some(draw) = unsafe { RendMan::instance_mut() }.ok().map(|r| r.debug_ez_draw.as_mut()) {
        draw.set_color(&F32Vector4(0.1, 0.9, 0.2, 1.0));
        let near = |_s: &sm64::SM64Surface| true;
        // draw only the part of each edge inside a 3 m box around Mario (big triangles otherwise
        // shoot lines across the map)
        let (lo, hi) = (glam::Vec3::new(mx - 300.0, my - 300.0, mz - 300.0), glam::Vec3::new(mx + 300.0, my + 300.0, mz + 300.0));
        let clip = |a: glam::Vec3, b: glam::Vec3| -> Option<(glam::Vec3, glam::Vec3)> {
            let (mut t0, mut t1) = (0.0f32, 1.0f32);
            let d = b - a;
            for k in 0..3 {
                if d[k].abs() < 1e-6 {
                    if a[k] < lo[k] || a[k] > hi[k] {
                        return None;
                    }
                } else {
                    let (mut e, mut f) = ((lo[k] - a[k]) / d[k], (hi[k] - a[k]) / d[k]);
                    if e > f {
                        std::mem::swap(&mut e, &mut f);
                    }
                    t0 = t0.max(e);
                    t1 = t1.min(f);
                    if t0 > t1 {
                        return None;
                    }
                }
            }
            Some((a + d * t0, a + d * t1))
        };
        let er = |p: glam::Vec3| to_er(m.origin, [p.x, p.y, p.z]);
        for surf in m.surfaces.iter().filter(|s| near(s)) {
            let col = match surf.force {
                0x39 => F32Vector4(0.1, 0.9, 0.2, 1.0),
                0x48 => F32Vector4(0.2, 0.4, 1.0, 1.0),
                0x3a => F32Vector4(1.0, 0.9, 0.1, 1.0),
                0x38 => F32Vector4(0.1, 0.9, 0.9, 1.0),
                _ => F32Vector4(1.0, 1.0, 1.0, 1.0),
            };
            draw.set_color(&col);
            let v = surf.vertices.map(|p| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
            for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
                if let Some((a, b)) = clip(a, b) {
                    draw.draw_line(&er(a), &er(b));
                }
            }
        }
        draw.set_color(&F32Vector4(1.0, 0.1, 0.1, 1.0));
        let v = &m.mesh;
        for t in 0..v.len() / 9 {
            let p = |k: usize| {
                let i = (t * 3 + k) * 3;
                to_er(m.origin, [v[i], v[i + 1], v[i + 2]])
            };
            let (a, b, c) = (p(0), p(1), p(2));
            draw.draw_line(&a, &b);
            draw.draw_line(&b, &c);
        }
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// Called by the Windows loader only.
pub unsafe extern "C" fn DllMain(hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }
    MODULE.store(hmodule, Ordering::Relaxed);
    std::thread::spawn(|| {
        log(format!("er-mario {} loaded", env!("CARGO_PKG_VERSION")));
        // (before anything is waited for: on another game version the task system below may
        // never be found, and the mod sat there without a word)
        if let Err(e) = version::check() {
            log(format!("this game version is not supported ({e}); ER Mario stays off"));
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_TOPMOST, MessageBoxW};
                let text = windows::core::HSTRING::from(format!(
                    "ER Mario does not support this version of Elden Ring ({e}).\n\nThe game runs without the mod. Check for an ER Mario update."
                ));
                MessageBoxW(None, &text, windows::core::w!("ER Mario"), MB_ICONWARNING | MB_TOPMOST);
            }
            return;
        }
        std::panic::set_hook(Box::new(|info| log(format!("PANIC: {info}"))));
        save_backup::run();
        assets::apply_pending();
        let started = std::time::Instant::now();
        let cs_task = loop {
            match CSTaskImp::wait_for_instance(Duration::from_secs(30)) {
                Ok(task) => break task,
                Err(_) => log(format!("still waiting for the game to start up ({:.0} s)", started.elapsed().as_secs_f32())),
            }
        };
        unsafe { install_xinput_hooks() };
        unsafe { kbd::install_hooks() };
        // on its own thread: where the overlay can't hook the renderer (CrossOver on a Mac died
        // right here, and the mod never got any further) Mario still works, without his HUD.
        // If it fails, or hasn't come back after a while, the steps it takes are tried one by one
        // and logged, so the log says which one it is.
        {
            static DONE: AtomicBool = AtomicBool::new(false);
            fn probe_once() {
                static PROBED: AtomicBool = AtomicBool::new(false);
                if !PROBED.swap(true, Ordering::Relaxed) && std::panic::catch_unwind(hud::probe_dx12).is_err() {
                    log("overlay probe: stopped by a panic (see above)");
                }
            }
            std::thread::spawn(|| {
                let hooked = std::panic::catch_unwind(|| hud::install(MODULE.load(Ordering::Relaxed))).unwrap_or(false);
                DONE.store(true, Ordering::Relaxed);
                if !hooked {
                    log("hud: the overlay could not be started; Mario runs without his HUD");
                    probe_once();
                }
            });
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(20));
                if !DONE.load(Ordering::Relaxed) {
                    log("hud: the overlay still hasn't started after 20 s");
                    probe_once();
                }
            });
        }
        unsafe { gameover::install_hook() };
        unsafe { engine_mario::install_anim_hook() };
        unsafe { engine_mario::install_menu_hook() };
        equip::init();
        lakitu::load_setting();
        yoshi::init();
        std::thread::spawn(startup);
        cs_task.run_recurring(
            |d: &FD4TaskData| {
                // a bug in the mod must never take the game down: log it and switch Mario off
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| frame(d))).is_err() {
                    ENABLED.store(false, Ordering::Relaxed);
                    if let Ok(mut g) = MARIO.try_lock() {
                        g.take();
                    }
                }
            },
            CSTaskGroupIndex::ChrIns_PostPhysics,
        );
        let guarded = |f: fn()| {
            move |_: &FD4TaskData| {
                let _ = std::panic::catch_unwind(f);
            }
        };
        cs_task.run_recurring(guarded(pose_task), CSTaskGroupIndex::ChrIns_PrePhysics);
        cs_task.run_recurring(guarded(input_task), CSTaskGroupIndex::ChrIns_PreBehaviorSafe);
        cs_task.run_recurring(guarded(hud_task), CSTaskGroupIndex::GameFlowStep_Post);
        cs_task.run_recurring(guarded(pads::task), CSTaskGroupIndex::GameFlowStep_Post);
        // our camera into the game's at every step from its camera update to drawing (it copies its
        // own back in between, and sets up culling and the sun shadow area from it)
        for group in [
            CSTaskGroupIndex::CameraStep,
            CSTaskGroupIndex::DrawParamUpdate,
            CSTaskGroupIndex::ChrIns_PostPhysicsSafe,
            CSTaskGroupIndex::CSDistViewManager_Update,
            CSTaskGroupIndex::WorldChrMan_PostPhysics,
            CSTaskGroupIndex::GameFlowStep_Post,
            CSTaskGroupIndex::Draw_Pre,
        ] {
            cs_task.run_recurring(guarded(lakitu::reapply), group);
        }
        // the game re-animates the skeleton at several points of the frame: re-apply after each
        for group in [
            CSTaskGroupIndex::ChrIns_PrePhysics_End,
            CSTaskGroupIndex::ChrIns_RagdollSafe,
            CSTaskGroupIndex::LocationUpdate_PostCloth,
            CSTaskGroupIndex::ChrIns_PreCloth,
            CSTaskGroupIndex::ChrIns_PreClothSafe,
            CSTaskGroupIndex::HavokClothUpdate_Pre_ClothModelInsSafe,
            CSTaskGroupIndex::ChrIns_PostPhysics,
            CSTaskGroupIndex::GameFlowStep_Post,
            CSTaskGroupIndex::ChrIns_PrePhysicsSafe,
            CSTaskGroupIndex::LocationUpdate_PrePhysics,
            CSTaskGroupIndex::LocationUpdate_PrePhysics_Post,
            CSTaskGroupIndex::LocationUpdate_PostCloth_Post,
            CSTaskGroupIndex::HavokWorldUpdate_Post,
            CSTaskGroupIndex::ChrIns_PostPhysicsSafe,
            CSTaskGroupIndex::WorldChrMan_PostPhysics,
            CSTaskGroupIndex::Draw_Pre,
        ] {
            cs_task.run_recurring(guarded(pose_task_late), group);
        }
        log("frame task registered");
    });
    true
}
