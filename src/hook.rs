//! The hook (R2, the middle mouse button), Devil May Cry's Snatch with a Mario touch: Mario flings it out
//! of his hand (er_moves.c's ER_THROW_HOOK), it flies to the enemy he looks at (the nearest one in
//! front of the camera) or, with none there, to the spot of the map the camera looks at:
//! - a small enemy: pulled in to him (juggle.rs), hung in front of him if he's in the air, ready for
//!   the next blow
//! - a big one (a boss, a strong enemy, anything too big to drag): Mario flies to it instead,
//!   popping up in front of it for an air string (er_moves.c act_er_zip)
//! - a wall: he flies to it and clings to it a moment (A: a wall kick off it); a floor or a ledge:
//!   he lands there
//! It barely hurts (combat.rs Attack::Hook): it's for keeping a combo going, not for damage. Works on
//! the ground and in the air. Drawn by hud.rs: Mario's spinning cap on a sparkling line.

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, WorldChrMan};
use eldenring::position::HavokPosition;
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::collision::{er_to_sm, sm_to_er};
use crate::combat::{self, Attack, Combat};
use crate::{log, moves, sm64, worker};

/// how far it reaches (m), how fast it flies out and back (m/s)
const RANGE: f32 = 20.0;
const SPEED: f32 = 75.0;
const BACK_SPEED: f32 = 60.0;
/// enemies within this cone (cos 60°) around where the camera looks
const AIM_CONE_COS: f32 = 0.5;
/// ticks after it's back before the next one; ticks the throw may take to let go
const COOLDOWN: u32 = 10;
const FALLBACK: u32 = 6;
/// ticks it's out at most (a pull or a zip that never ends)
const LIFE: u32 = 90;
/// Mario's body (m): how far from a wall he hangs, how high his chest is
const BODY: f32 = 0.45;
const CHEST: f32 = 0.8;

const SOUND_CLINK: i32 = 0x0444_A081;
const SOUND_LATCH: i32 = 0x0436_8081;
const SOUND_BUZZ: i32 = 0x700E_0081;

#[derive(Clone, Copy)]
enum Aim {
    /// an enemy: its handle, body (m: radius, height), and whether it's small enough to drag in
    Enemy { mob: FieldInsHandle, radius: f32, height: f32, drag: bool },
    /// a spot on the map (Elden Ring), and whether it's a floor (aimed down at it)
    Map { at: Vec3, floor: bool },
    /// nothing there: out to its reach, then back
    Miss { at: Vec3 },
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// thrown: waiting for his arm to let go (since this tick)
    Winding(u32),
    Out,
    /// latched on an enemy coming in
    Pulling,
    /// pulling him (since this tick)
    Zipping(u32),
    Back,
}

pub struct Hook {
    phase: Option<Phase>,
    aim: Aim,
    pos: Vec3,
    born: u32,
    ready_at: u32,
    pressed: bool,
}

/// What hud.rs draws: his hand, the cap's spot, its spin (radians).
#[derive(Clone, Copy)]
pub struct Sprite {
    pub hand: Vec3,
    pub head: Vec3,
    pub spin: f32,
}

static SPRITE: Mutex<Option<(Sprite, Instant)>> = Mutex::new(None);

pub fn sprite() -> Option<Sprite> {
    let s = *SPRITE.lock().unwrap_or_else(|e| e.into_inner());
    s.filter(|(_, at)| at.elapsed().as_secs_f32() < 0.25).map(|(s, _)| s)
}

pub fn clear() {
    *SPRITE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn sound(id: i32, volume: f32) {
    worker::call("hook sound", move |_| unsafe { sm64::sm64_play_sound_volume(id, volume) });
}

fn key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

/// An enemy's chest now (Elden Ring), and its feet; None once it's gone or dead.
fn chest(mob: &FieldInsHandle, height: f32) -> Option<(Vec3, Vec3)> {
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(mob)?;
    if chr.modules.data.hp <= 0 {
        return None;
    }
    let p = chr.modules.physics.position;
    let feet = Vec3::new(p.0, p.1, p.2);
    Some((feet + Vec3::Y * (height * 0.5).min(1.5), feet))
}

const ACT_FLAG_AIR: u32 = 0x800;
const ACT_FLAG_SWIMMING: u32 = 0x2000;
const ACT_FLAG_INVULNERABLE: u32 = 0x0002_0000;

impl Hook {
    pub fn new() -> Self {
        Hook { phase: None, aim: Aim::Miss { at: Vec3::ZERO }, pos: Vec3::ZERO, born: 0, ready_at: 0, pressed: false }
    }

    /// Mario's world was re-scaled (mega.rs), he died, the game moves him: a hook out goes.
    pub fn drop(&mut self) {
        if self.phase.take().is_some() {
            clear();
        }
    }

    /// Game thread, after every SM64 tick: R2 (`button`), whether he may throw it now, what his moves
    /// did (`mv`), his state, feet and hand (Elden Ring). Throws it, flies it, pulls.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        id: i32,
        button: bool,
        allowed: bool,
        mv: &sm64::SM64ErMove,
        state: &sm64::SM64MarioState,
        feet: Vec3,
        hand: Vec3,
        origin: [f32; 3],
        tick: u32,
        player: &ChrIns,
        combat: &mut Combat,
        ray: impl Fn(Vec3, Vec3) -> Option<Vec3>,
    ) {
        const DT: f32 = 1.0 / 30.0;
        let pressed = button && !self.pressed;
        self.pressed = button;
        let air = state.action & ACT_FLAG_AIR != 0;
        if pressed {
            let busy = state.action & (ACT_FLAG_SWIMMING | ACT_FLAG_INVULNERABLE) != 0 || state.action == moves::ACT_ER_ZIP;
            if self.phase.is_some() || tick < self.ready_at || !allowed || busy {
                if self.phase.is_none() && allowed {
                    sound(SOUND_BUZZ, 0.4);
                }
            } else {
                self.throw(id, state, feet, hand, origin, tick, air, &ray);
            }
        }
        let Some(phase) = self.phase else { return };
        let mut next = Some(phase);
        match phase {
            Phase::Winding(since) => {
                let let_go = mv.throw_kind > 0 && mv.throw_kind as u32 - 1 == moves::THROW_HOOK;
                if let_go || tick.wrapping_sub(since) >= FALLBACK {
                    self.pos = hand;
                    next = Some(Phase::Out);
                }
            }
            // (dead, holding someone, hurt, swimming, riding, a menu: the pull's off)
            Phase::Out if !allowed || state.action & (ACT_FLAG_SWIMMING | ACT_FLAG_INVULNERABLE) != 0 => {
                next = Some(Phase::Back);
            }
            Phase::Out => {
                // flying to it (an enemy on the move: to where it is now)
                let to = match self.aim {
                    Aim::Enemy { mob, height, .. } => match chest(&mob, height) {
                        Some((c, _)) => c,
                        None => {
                            next = Some(Phase::Back);
                            self.pos
                        }
                    },
                    Aim::Map { at, .. } | Aim::Miss { at } => at,
                };
                let d = to - self.pos;
                let step = SPEED * DT;
                if next == Some(Phase::Out) {
                    if d.length() <= step {
                        self.pos = to;
                        next = self.latch(id, feet, origin, tick, air, player, combat);
                    } else {
                        self.pos += d / d.length() * step;
                    }
                }
            }
            Phase::Pulling => {
                // on the enemy as it comes in, until it's in front of him
                match self.aim {
                    Aim::Enemy { mob, height, .. } if crate::juggle::is_pulled(&mob) => match chest(&mob, height) {
                        Some((c, _)) => self.pos = c,
                        None => next = Some(Phase::Back),
                    },
                    _ => next = Some(Phase::Back),
                }
            }
            Phase::Zipping(since) => {
                // the pull's end on an enemy that moves; done once he's off the line
                if tick.wrapping_sub(since) >= 2 && state.action != moves::ACT_ER_ZIP {
                    next = Some(Phase::Back);
                } else if let Aim::Enemy { mob, radius, height, .. } = self.aim {
                    match chest(&mob, height) {
                        Some((c, f)) => {
                            self.pos = c;
                            let to = zip_to_enemy(feet, f, radius, height);
                            let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
                            worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], 0, 0) });
                        }
                        None => next = Some(Phase::Back),
                    }
                }
            }
            Phase::Back => {
                let d = hand - self.pos;
                let step = BACK_SPEED * DT;
                if d.length() <= step.max(0.6) {
                    next = None;
                } else {
                    self.pos += d / d.length() * step;
                }
            }
        }
        if next.is_some() && tick.wrapping_sub(self.born) > LIFE && next != Some(Phase::Back) {
            next = Some(Phase::Back);
        }
        if tick.wrapping_sub(self.born) > LIFE + 30 {
            next = None;
        }
        if next.is_none() {
            self.ready_at = tick + COOLDOWN;
        }
        self.phase = next;
        let shown = matches!(self.phase, Some(p) if !matches!(p, Phase::Winding(_)));
        *SPRITE.lock().unwrap_or_else(|e| e.into_inner()) =
            shown.then(|| (Sprite { hand, head: self.pos, spin: tick as f32 * 0.9 }, Instant::now()));
    }

    /// R2: picks what it goes for and flings it (his arm's throw lets it go: `tick`).
    #[allow(clippy::too_many_arguments)]
    fn throw(&mut self, id: i32, state: &sm64::SM64MarioState, feet: Vec3, hand: Vec3, origin: [f32; 3], tick: u32, air: bool, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) {
        let face = Vec3::new(-state.face_angle.sin(), 0.0, state.face_angle.cos());
        let look = crate::lakitu::forward().filter(|v| v.is_finite() && v.length() > 0.5).map(|v| v.normalize());
        let flat = look.map_or(face, |v| v.with_y(0.0).normalize_or(face));
        // the enemy nearest to where he looks, in sight
        let here = HavokPosition(feet.x, feet.y, feet.z, 0.0);
        let enemy = combat::nearby(&here, RANGE, origin)
            .iter()
            .filter_map(|t| {
                let mob = *t.shootable()?;
                let c = sm_to_er(origin, [t.sm[0], t.sm[1] + t.height.min(300.0) * 0.5, t.sm[2]]);
                let c = Vec3::new(c.0, c.1, c.2);
                let d = (c - hand).with_y(0.0);
                let cos = d.normalize_or(flat).dot(flat);
                if d.length() < 0.3 || cos < AIM_CONE_COS || ray(hand, c).is_some() {
                    return None;
                }
                let (radius, height) = (t.radius * crate::scale(), t.height * crate::scale());
                let drag = crate::juggle::draggable(&mob, radius, height);
                Some((d.length() * (1.6 - cos), Aim::Enemy { mob, radius, height, drag }, c))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let (aim, at) = match enemy {
            Some((_, aim, c)) => (aim, c),
            None => {
                // the map where the camera looks (a little up without Lakitu)
                let dir = look.unwrap_or((face + Vec3::Y * 0.2).normalize());
                let from = feet + Vec3::Y * CHEST;
                match ray(from, from + dir * RANGE) {
                    Some(p) => {
                        // (a floor if there's ground right under a spot just short of it)
                        let near = p - dir * 0.3;
                        let floor = ray(near + Vec3::Y * 0.4, near - Vec3::Y * 0.6).is_some();
                        (Aim::Map { at: p, floor }, p)
                    }
                    None => (Aim::Miss { at: from + dir * RANGE }, from + dir * RANGE),
                }
            }
        };
        log(format!(
            "hook: {}",
            match aim {
                Aim::Enemy { drag: true, .. } => "an enemy, to pull in",
                Aim::Enemy { .. } => "a big enemy, to fly to",
                Aim::Map { floor: true, .. } => "a floor",
                Aim::Map { .. } => "a wall",
                Aim::Miss { .. } => "nothing",
            }
        ));
        self.aim = aim;
        self.phase = Some(Phase::Winding(tick));
        self.born = tick;
        self.pos = hand;
        // he turns to it and flings his arm
        let me = Vec3::from(state.position);
        let to = Vec3::from(er_to_sm(origin, &HavokPosition(at.x, at.y, at.z, 0.0))) - me;
        let yaw = (to.x.abs() + to.z.abs() > 1.0).then(|| to.x.atan2(to.z));
        let action = if air { moves::ACT_ER_AIR_THROW } else { moves::ACT_ER_THROW };
        worker::call("hook throw", move |_| unsafe {
            if let Some(f) = yaw {
                sm64::sm64_set_mario_faceangle(id, f);
            }
            sm64::sm64_set_mario_action_arg(id, action, moves::THROW_HOOK);
        });
    }

    /// It got there: a pull, or the zip. What's next.
    #[allow(clippy::too_many_arguments)]
    fn latch(&mut self, id: i32, feet: Vec3, origin: [f32; 3], tick: u32, air: bool, player: &ChrIns, combat: &mut Combat) -> Option<Phase> {
        match self.aim {
            Aim::Enemy { mob, radius, height, drag } => {
                // its little hit (a flinch, a point of style), then in it comes, or he goes
                let here = HavokPosition(self.pos.x, self.pos.y, self.pos.z, 0.0);
                let near = combat::nearby(&here, 4.0 + radius, origin);
                if let Some(i) = near.iter().position(|t| t.shootable().is_some_and(|h| key(h) == key(&mob))) {
                    combat.deal(player, &near, &[(i, Attack::Hook, false)], tick);
                }
                sound(SOUND_LATCH, 0.6);
                crate::impact::hook_latch(self.pos);
                if drag && crate::juggle::pull(&mob, air) {
                    log("hook: pulling an enemy in");
                    return Some(Phase::Pulling);
                }
                let Some((_, f)) = chest(&mob, height) else { return Some(Phase::Back) };
                let to = zip_to_enemy(feet, f, radius, height);
                let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
                log("hook: Mario flies to it");
                worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], 0, 1) });
                Some(Phase::Zipping(tick))
            }
            Aim::Map { at, floor } => {
                sound(SOUND_CLINK, 0.55);
                crate::impact::hook_latch(at);
                // a floor: his feet on it; a wall: his chest at the spot, his body against it
                let (to, kind) = if floor {
                    (at + Vec3::Y * 0.05, 1)
                } else {
                    let back = (feet - at).with_y(0.0).normalize_or_zero();
                    (at + back * BODY * 0.6 - Vec3::Y * CHEST, 2)
                };
                let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
                worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], kind, 1) });
                Some(Phase::Zipping(tick))
            }
            Aim::Miss { .. } => Some(Phase::Back),
        }
    }
}

/// Where Mario flies to at a big enemy (Elden Ring): just off its body on his side, about its
/// waist high (no higher than he could hit).
fn zip_to_enemy(mario: Vec3, feet: Vec3, radius: f32, height: f32) -> Vec3 {
    let side = (mario - feet).with_y(0.0).normalize_or(Vec3::Z);
    feet + side * (radius + BODY + 0.3) + Vec3::Y * (height * 0.35).min(2.5)
}
