//! The parry (R3; Sekiro's deflect and the Hollow Knight's, with a Mario touch): a palm thrust
//! (SM64's fireball throw) opens a short window, and any projectile an enemy has fired that comes
//! within reach in it is turned back: the game's bullet is taken out of play and a blue-white
//! fireball (a charged one's strength, fireball.rs) goes back for whoever fired it. A miss costs a
//! wait, a deflect a short one. L2 held is the grab's instead (carry.rs).

use eldenring::cs::{CSBulletManager, ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::{DirectionalVector, HavokPosition};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat;
use crate::fireball::Fire;
use crate::{log, moves, sm64, worker};

/// ticks from the press the window is open (the palm thrust's arm is out)
const WINDOW: std::ops::Range<u32> = 2..16;
/// the wait after a press that deflected nothing, and after a deflect (ticks)
const MISS_WAIT: u32 = 48;
const HIT_WAIT: u32 = 30;
/// how long a turned bullet is looked for again (ticks)
const PARK_WATCH: u32 = 300;
/// how near a projectile must come (m, from his chest), and how many a press turns back
const REACH: f32 = 3.8;
const MAX_PER_PRESS: usize = 3;
/// the deflect's points of style
const POINTS: f32 = 3.0;

const SOUND_CLINK: i32 = 0x0444_A081;
const SOUND_DEFLECT: i32 = 0x3016_0091;
const SOUND_BUZZ: i32 = 0x700E_0081;

pub struct Parry {
    pressed: bool,
    /// the tick of the press whose window is open
    since: Option<u32>,
    ready_at: u32,
    deflected: usize,
    /// the bullets sent under the map: which (handle key), how high, when
    parked: Vec<(u64, f32, u32)>,
}

impl Parry {
    pub fn new() -> Self {
        Parry { pressed: false, since: None, ready_at: 0, deflected: 0, parked: Vec::new() }
    }

    /// Game thread, every SM64 tick: R3 held now, whether Mario may parry (free, on his feet or in
    /// the air, not carrying), his state, feet (Elden Ring).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, id: i32, r3: bool, allowed: bool, state: &sm64::SM64MarioState, feet: Vec3, origin: [f32; 3], tick: u32, player: &ChrIns, fire: &mut Fire) {
        self.watch(tick);
        let pressed = r3 && !self.pressed;
        self.pressed = r3;
        if pressed && allowed {
            if tick < self.ready_at {
                worker::call("parry buzz", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_BUZZ, 0.35) });
            } else {
                self.since = Some(tick);
                self.deflected = 0;
                self.ready_at = tick + MISS_WAIT;
                // (the palm thrust: the fireball's throw, with no fireball behind it)
                let action = if state.action & 0x800 != 0 { moves::ACT_ER_AIR_THROW } else { moves::ACT_ER_THROW };
                worker::call("parry", move |_| unsafe { sm64::sm64_set_mario_action_arg(id, action, moves::THROW_FIRE) });
            }
        }
        let Some(since) = self.since else { return };
        let age = tick.wrapping_sub(since);
        if !allowed || age >= WINDOW.end {
            self.since = None;
            return;
        }
        if !WINDOW.contains(&age) || self.deflected >= MAX_PER_PRESS {
            return;
        }
        let me = combat::handle_key(&player.field_ins_handle);
        let face = Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
        let chest = feet + Vec3::Y * 1.0;
        // the projectiles in reach: where they were, how fast, who fired them
        let mut found: Vec<(Vec3, f32, FieldInsHandle)> = Vec::new();
        if let Ok(mgr) = unsafe { CSBulletManager::instance_mut() } {
            for b in mgr.bullets_mut() {
                let owner = b.targeting_owner.owner_chr_handle;
                if combat::handle_key(&owner) == me || !enemy(&owner) {
                    continue;
                }
                let p = b.physics.position;
                let v = b.physics.velocity;
                let pos = Vec3::new(p.0, p.1, p.2);
                let vel = Vec3::new(v.0, v.1, v.2);
                let speed = vel.length();
                // (one that doesn't move is a spot of ground fire or a cloud, not a shot; one going
                // away from Mario isn't coming for him)
                if (pos - chest).length() > REACH || speed < 0.05 || vel.dot(chest - pos) <= 0.0 {
                    continue;
                }
                // taken out of play: far under the map, standing still
                b.physics.position = HavokPosition(pos.x, pos.y - 500.0, pos.z, p.3);
                b.physics.velocity = DirectionalVector(0.0, 0.0, 0.0, v.3);
                self.parked.push((combat::handle_key(&b.field_ins_handle), pos.y - 500.0, tick));
                found.push((pos, speed, owner));
                if self.deflected + found.len() >= MAX_PER_PRESS {
                    break;
                }
            }
        }
        for (pos, speed, owner) in found {
            self.deflected += 1;
            self.ready_at = tick + HIT_WAIT;
            // (one shot goes back per press: a volley of arrows is stopped, not tripled)
            if self.deflected == 1 {
                let key = combat::handle_key(&owner);
                // back at whoever fired it (else straight ahead, the shot homes on by itself)
                let target = combat::chest_by_key(key);
                let from = feet + Vec3::Y * 1.2 + face * 0.8;
                let aim = target.map_or(face, |c| (c - from).normalize_or(face));
                fire.reflect(from, aim, target.map(|_| key), origin, tick);
                log(format!("parry: a projectile turned back at {:.1} m (speed {speed:.1}){}", (pos - chest).length(), if target.is_some() { ", at its shooter" } else { "" }));
            }
            crate::impact::parry(pos);
            crate::lakitu::shake(0.8);
            crate::style::callout("PARRY!", Some(pos + Vec3::Y * 0.6), [0.5, 0.85, 1.0], true, POINTS);
            worker::call("parry hit", |_| unsafe {
                sm64::sm64_play_sound_volume(SOUND_CLINK, 0.7);
                sm64::sm64_play_sound_volume(SOUND_DEFLECT, 0.3);
                sm64::sm64_er_hitstop(3);
            });
        }
    }

    /// Whether the game really drops a bullet sent under the map (untested): each one parked is
    /// looked for again for a while, and the log says what became of it.
    fn watch(&mut self, tick: u32) {
        if self.parked.is_empty() {
            return;
        }
        let Ok(mgr) = (unsafe { CSBulletManager::instance_mut() }) else {
            self.parked.clear();
            return;
        };
        let live: Vec<(u64, f32)> = mgr.bullets_mut().map(|b| (combat::handle_key(&b.field_ins_handle), b.physics.position.1)).collect();
        self.parked.retain(|&(at, y, since)| {
            let age = tick.wrapping_sub(since);
            match live.iter().find(|l| l.0 == at) {
                None => {
                    log(format!("parry: the turned bullet is gone after {age} ticks"));
                    false
                }
                Some(l) if (l.1 - y).abs() > 2.0 => {
                    log(format!("parry: the turned bullet still moves ({:.1} m from where it was parked)", l.1 - y));
                    false
                }
                Some(_) if age > PARK_WATCH => {
                    log(format!("parry: the turned bullet is still there after {age} ticks"));
                    false
                }
                Some(_) => true,
            }
        });
    }
}

/// Whether the character with this handle is an enemy (not on Mario's side, there at all).
/// One no longer there (dead, or a trap's) still counts; a player (the co-op partner) never does.
fn enemy(h: &FieldInsHandle) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    match wcm.chr_ins_by_handle(h) {
        Some(c) => combat::hostile(c) && !combat::own_side(c.team_type) && combat::hittable(c.chr_type),
        // (in co-op one the game can't name here may be the partner's)
        None => !h.is_empty() && !crate::coop::active(),
    }
}
