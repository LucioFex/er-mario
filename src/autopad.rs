//! Developer test pad: with `autopad.txt` in the mod folder, its script is played on controller 0
//! (through the XInput hook, so the game's menus and Mario both see it), to test without a hand
//! on the pad. Rewriting the file plays it again from the top; deleting it stops it. One step per
//! line: `buttons ms [lx ly rx ry lt rt]`, buttons joined by '+' (A B X Y LB RB LT RT L3 R3
//! START BACK UP DOWN LEFT RIGHT, or `-` for none), sticks and triggers -1..1 / 0..1; `#` comments.

use std::sync::Mutex;
use std::time::{Instant, SystemTime};

use windows::Win32::UI::Input::XboxController::*;

use crate::log;

struct Step {
    /// `GO`: the left stick steers to the nearest hostile (stops 4 m off it)
    seek: bool,
    buttons: u16,
    ms: u32,
    sticks: [f32; 4],
    triggers: [f32; 2],
}

struct State {
    checked: Option<Instant>,
    stamp: Option<SystemTime>,
    steps: Vec<Step>,
    started: Option<Instant>,
    logged: usize,
}

static STATE: Mutex<State> = Mutex::new(State { checked: None, stamp: None, steps: Vec::new(), started: None, logged: usize::MAX });

fn button(name: &str) -> Option<u16> {
    Some(match name.to_ascii_uppercase().as_str() {
        "A" => XINPUT_GAMEPAD_A.0,
        "B" => XINPUT_GAMEPAD_B.0,
        "X" => XINPUT_GAMEPAD_X.0,
        "Y" => XINPUT_GAMEPAD_Y.0,
        "LB" => XINPUT_GAMEPAD_LEFT_SHOULDER.0,
        "RB" => XINPUT_GAMEPAD_RIGHT_SHOULDER.0,
        "L3" => XINPUT_GAMEPAD_LEFT_THUMB.0,
        "R3" => XINPUT_GAMEPAD_RIGHT_THUMB.0,
        "START" => XINPUT_GAMEPAD_START.0,
        "BACK" => XINPUT_GAMEPAD_BACK.0,
        "UP" => XINPUT_GAMEPAD_DPAD_UP.0,
        "DOWN" => XINPUT_GAMEPAD_DPAD_DOWN.0,
        "LEFT" => XINPUT_GAMEPAD_DPAD_LEFT.0,
        "RIGHT" => XINPUT_GAMEPAD_DPAD_RIGHT.0,
        "-" | "LT" | "RT" => 0,
        _ => return None,
    })
}

fn parse(text: &str) -> Vec<Step> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut it = line.split_whitespace();
        let (Some(names), Some(ms)) = (it.next(), it.next().and_then(|m| m.parse::<u32>().ok())) else { continue };
        let mut buttons = 0;
        let mut triggers = [0.0; 2];
        let mut seek = false;
        for n in names.split('+') {
            match n.to_ascii_uppercase().as_str() {
                "GO" => seek = true,
                "LT" => triggers[0] = 1.0,
                "RT" => triggers[1] = 1.0,
                _ => buttons |= button(n).unwrap_or(0),
            }
        }
        let nums: Vec<f32> = it.filter_map(|v| v.parse().ok()).collect();
        let at = |i: usize| nums.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        if nums.len() > 4 {
            triggers = [at(4).max(triggers[0]), at(5).max(triggers[1])];
        }
        out.push(Step { seek, buttons, ms, sticks: [at(0), at(1), at(2), at(3)], triggers });
    }
    out
}

/// XInput hook, before anything else: controller 0 plays the script (connected even if no pad
/// is). Returns the result code to report.
pub fn apply(index: u32, state: *mut XINPUT_STATE, rc: u32) -> u32 {
    if index != 0 || state.is_null() {
        return rc;
    }
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.checked.is_none_or(|t| t.elapsed().as_millis() > 500) {
        s.checked = Some(Instant::now());
        let path = crate::paths::file("autopad.txt");
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if stamp != s.stamp {
            s.stamp = stamp;
            s.steps = stamp.and_then(|_| std::fs::read_to_string(&path).ok()).map(|t| parse(&t)).unwrap_or_default();
            s.started = (!s.steps.is_empty()).then(Instant::now);
            s.logged = usize::MAX;
            log(format!("autopad: {} step(s)", s.steps.len()));
        }
    }
    // (while the file is there, controller 0 stays connected: the game hardly polls an empty slot)
    let connected = |rc: u32| {
        if rc != 0 {
            unsafe { *state = XINPUT_STATE::default() };
        }
        0
    };
    if s.stamp.is_none() {
        return rc;
    }
    let Some(started) = s.started else { return connected(rc) };
    let mut t = started.elapsed().as_millis() as u32;
    let mut current = None;
    for (i, step) in s.steps.iter().enumerate() {
        if t < step.ms {
            current = Some(i);
            break;
        }
        t -= step.ms;
    }
    let Some(i) = current else {
        log("autopad: done");
        s.started = None;
        return connected(rc);
    };
    if s.logged != i {
        s.logged = i;
        log(format!("autopad: step {}", i + 1));
    }
    let step = &s.steps[i];
    let st = unsafe { &mut *state };
    if rc != 0 {
        *st = XINPUT_STATE::default();
    }
    let g = &mut st.Gamepad;
    g.wButtons.0 |= step.buttons;
    let axis = |v: f32| (v * 32767.0) as i16;
    let mut sticks = step.sticks;
    if step.seek {
        let (x, y) = *SEEK.lock().unwrap_or_else(|e| e.into_inner());
        sticks[0] = x;
        sticks[1] = y;
    }
    for (k, v) in sticks.iter().enumerate() {
        if *v != 0.0 {
            match k {
                0 => g.sThumbLX = axis(*v),
                1 => g.sThumbLY = axis(*v),
                2 => g.sThumbRX = axis(*v),
                _ => g.sThumbRY = axis(*v),
            }
        }
    }
    g.bLeftTrigger = g.bLeftTrigger.max((step.triggers[0] * 255.0) as u8);
    g.bRightTrigger = g.bRightTrigger.max((step.triggers[1] * 255.0) as u8);
    st.dwPacketNumber = st.dwPacketNumber.wrapping_add(1);
    0
}

/// The stick that heads for the nearest hostile (screen right, forward), from the game thread.
static SEEK: Mutex<(f32, f32)> = Mutex::new((0.0, 0.0));

/// Game thread, every frame: while autopad.txt is there, every second, the hostiles
/// within 60 m: id, distance, where they are from the Tarnished (degrees, 0 = ahead), which way
/// they face (0 = at him), animation; to see whether they notice Mario.
pub fn report(player: &eldenring::cs::ChrIns) {
    use eldenring::cs::WorldChrMan;
    use fromsoftware_shared::FromStatic;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if s.stamp.is_none() {
            return;
        }
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    seek(player, wcm);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) {
        return;
    }
    *last = Some(Instant::now());
    let p = player.modules.physics.position;
    let o = player.modules.physics.orientation;
    let fwd = glam::Quat::from_xyzw(o.0, o.1, o.2, o.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
    let yaw = |v: glam::Vec3| v.x.atan2(-v.z);
    let mut out = Vec::new();
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &eldenring::cs::ChrIns = chr;
            if chr.modules.data.hp <= 0 || chr.team_type == 0 || crate::combat::own_side(chr.team_type) || !crate::combat::hittable(chr.chr_type) {
                continue;
            }
            let q = chr.modules.physics.position;
            let d = glam::Vec3::new(q.0 - p.0, q.1 - p.1, q.2 - p.2);
            if d.length() > 60.0 {
                continue;
            }
            let eo = chr.modules.physics.orientation;
            let efwd = glam::Quat::from_xyzw(eo.0, eo.1, eo.2, eo.3).mul_vec3(glam::vec3(0.0, 0.0, -1.0));
            let deg = |a: f32| ((a.to_degrees() + 540.0) % 360.0 - 180.0).round() as i32;
            out.push((d.length(), format!(
                "c{:04} {:.0} m at {}°, faces {}°, anim {}",
                chr.character_id,
                d.length(),
                deg(yaw(d) - yaw(fwd)),
                deg(yaw(efwd) - yaw(-d)),
                crate::current_anim(chr)
            )));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let list: Vec<String> = out.into_iter().take(6).map(|o| o.1).collect();
    log(format!("autopad: hostiles {}", if list.is_empty() { "none".into() } else { list.join(" | ") }));
}


/// The stick (camera-relative) that walks Mario to the nearest hostile within 60 m.
fn seek(player: &eldenring::cs::ChrIns, wcm: &eldenring::cs::WorldChrMan) {
    use fromsoftware_shared::FromStatic;
    let p = player.modules.physics.position;
    let me = glam::Vec3::new(p.0, p.1, p.2);
    let nearest = wcm
        .chr_sets
        .iter()
        .flatten()
        .flat_map(|set| set.characters())
        .filter(|c| c.modules.data.hp > 0 && c.team_type != 0 && !crate::combat::own_side(c.team_type) && crate::combat::hittable(c.chr_type))
        .map(|c| glam::Vec3::new(c.modules.physics.position.0, c.modules.physics.position.1, c.modules.physics.position.2) - me)
        .filter(|d| d.length() < 60.0)
        .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
    let stick = match (nearest, unsafe { eldenring::cs::CSCamera::instance() }) {
        (Some(d), Ok(cam)) if d.with_y(0.0).length() > 4.0 => {
            let m = &cam.pers_cam_1.matrix;
            let right = glam::Vec3::new(m.0.0, 0.0, m.0.2).normalize_or_zero();
            let fwd = glam::Vec3::new(m.2.0, 0.0, m.2.2).normalize_or_zero();
            let d = d.with_y(0.0).normalize_or_zero();
            (d.dot(right), d.dot(fwd))
        }
        _ => (0.0, 0.0),
    };
    *SEEK.lock().unwrap_or_else(|e| e.into_inner()) = stick;
}
