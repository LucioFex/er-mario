//! The hook (R2, the middle mouse button), Devil May Cry's Snatch with a Mario touch: Mario flings it out
//! of his hand (er_moves.c's ER_THROW_HOOK), it flies to the enemy he looks at (the nearest one in
//! front of the camera) or, with none there, to the spot of the map the camera looks at:
//! - a small enemy: Mario flies to it, popping up in front of it for an air string (er_moves.c
//!   act_er_zip); a medium one: the zip ends in a kick off it, up into the air (the zip-kick), or,
//!   R2 held, he vaults over it to land behind it (odm.rs)
//! - a big one: the cap anchors to the bone nearest the middle of the screen, the AoT hook's line
//!   (odm.rs: orbit, reel in, the strike)
//! - with L2 held (any time, on the ground or in the air) it's the other way round: a small enemy
//!   is pulled in to him (juggle.rs), hung in front of him if he's in the air, ready for the next
//!   blow; a medium one too, but the same one only every MEDIUM_EVERY seconds; a big one (a boss, a
//!   strong enemy) only gets the little hit: it doesn't come and Mario doesn't go. L2 never takes
//!   the map
//! - riding one is L2 + R3 by it only (bossride.rs)
//! - a wall: he flies to it and clings to it a moment (A: a wall kick off it); a floor or a ledge:
//!   he lands there; R2 held on a spot high up, the rope (colossus.rs), looked for ahead and above in
//!   a cone when the camera looks at the sky or the ground
//! It finds enemies on its own: the near ones all around him (the nearest first, those in front of
//! him or the camera before those behind), the far ones only where he or the camera looks. The map
//! is fair game once per airtime: after a wall or a ledge it only clinks off them until he's back on
//! his feet, or until it catches an enemy (enemies are always fair game, and give the wall back: jump,
//! wall, enemy, wall again). It barely hurts (combat.rs Attack::Hook): it's for keeping a combo
//! going, not for damage. Works on the ground and in the air. Drawn by hud.rs: Mario's spinning cap on a sparkling line.

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
/// its reach shut in (m: colossus::hook_reach stretches it in the open)
const RANGE: f32 = 20.0;
const SPEED: f32 = 75.0;
const BACK_SPEED: f32 = 60.0;
/// enemies within this cone (cos 60°) around where the camera or Mario looks, and any closer than
/// this all around him (m)
const AIM_CONE_COS: f32 = 0.5;
const AIM_NEAR: f32 = 12.0;
/// one this close (m, to its body) is taken before farther ones in front (half its score)
const AIM_CLOSE: f32 = 5.0;
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
    Enemy { mob: FieldInsHandle, radius: f32, height: f32, drag: bool, medium: bool },
    /// a big enemy's bone (odm.rs): the line's anchor
    Bone { mob: FieldInsHandle, bone: u32, radius: f32, height: f32 },
    /// a spot on the map (Elden Ring), whether it's a floor (aimed down at it), whether the cone
    /// found it (only the rope's: R2 let go, the cap just comes back)
    Map { at: Vec3, floor: bool, cone: bool },
    /// nothing there: out to its reach, then back; or the map when it isn't fair game (a clink there)
    Miss { at: Vec3, clink: bool },
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
    /// on a medium one with R2 still held (since this tick): let go, the zip-kick; held on, the vault
    Deciding(u32),
    /// on a big one's bone: the AoT hook's line (odm.rs)
    Odm,
    /// R2 held as it caught a spot high up: Mario swings on the rope (colossus.rs)
    Roping,
    Back,
}

pub struct Hook {
    phase: Option<Phase>,
    aim: Aim,
    pos: Vec3,
    born: u32,
    ready_at: u32,
    pressed: bool,
    /// a wall or a ledge was hooked this airtime (the next only clinks off)
    map_used: bool,
    /// X held last tick (the home run)
    x_held: bool,
    /// L2 was held when it was thrown: it pulls the enemy in instead of taking Mario there
    pull: bool,
    /// the zip-kick's blow landed (once a zip)
    kicked: bool,
    /// a throw asked for without R2 (the AoT hook's rebound A, odm.rs)
    force: bool,
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

/// The hook, only while it's out right now (the Chomp takes its place: not a moment longer).
pub fn live() -> Option<Sprite> {
    let s = *SPRITE.lock().unwrap_or_else(|e| e.into_inner());
    s.filter(|(_, at)| at.elapsed().as_secs_f32() < 0.12).map(|(s, _)| s)
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
        Hook { phase: None, aim: Aim::Miss { at: Vec3::ZERO, clink: false }, pos: Vec3::ZERO, born: 0, ready_at: 0, pressed: false, map_used: false, x_held: false, pull: false, kicked: false, force: false }
    }

    /// Mario's world was re-scaled (mega.rs), he died, the game moves him: a hook out goes.
    pub fn drop(&mut self) {
        // (on the rope: off it, he falls on; the map is fair game again, rope to rope; R2 still held
        // from the rope isn't a new throw)
        if matches!(self.phase, Some(Phase::Roping)) {
            crate::colossus::cancel_rope();
            self.map_used = false;
            self.pressed = true;
        }
        // (on the AoT hook's line: off it, falling on)
        if matches!(self.phase, Some(Phase::Odm)) {
            crate::odm::cancel();
        }
        if self.phase.take().is_some() {
            clear();
        }
    }

    /// The AoT hook's rebound A (odm.rs): a throw on the next tick, as if R2 went down.
    pub fn rethrow(&mut self) {
        self.force = true;
    }

    /// Game thread, after every SM64 tick: R2 (`button`; `held`: the trigger itself, also while the
    /// line or the colossus has the buttons), whether he may throw it now, what his moves did (`mv`),
    /// his state, feet and hand (Elden Ring). Throws it, flies it, pulls.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        id: i32,
        button: bool,
        held: bool,
        l2: bool,
        x: bool,
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
        // (a press is the trigger going down, not its being handed back: held through the line or a
        // vault, then given back, it's no new throw)
        let pressed = (button && !self.pressed) || std::mem::take(&mut self.force);
        self.pressed = held;
        let x_pressed = x && !self.x_held;
        self.x_held = x;
        // (held up by colossus.rs or odm.rs, on a limb or off a hit: off the ground all the same)
        let air = state.action & ACT_FLAG_AIR != 0 || state.action == crate::colossus::ACT_ER_CLING;
        // back on his feet: the map is fair game again
        if !air && state.action != moves::ACT_ER_ZIP && !matches!(self.phase, Some(Phase::Zipping(_))) {
            self.map_used = false;
        }
        // R2 again with an enemy on the line: the tether, it and the next one yanked into each other
        if pressed && allowed {
            if let (Some(Phase::Pulling | Phase::Zipping(_)), Aim::Enemy { mob, .. }) = (self.phase, self.aim) {
                match self.second(&mob, feet, origin, &ray).filter(|b| crate::bat::tether(&mob, b)) {
                    Some(_) => {
                        sound(SOUND_LATCH, 0.7);
                        if matches!(self.phase, Some(Phase::Zipping(_))) {
                            worker::call("tether", move |_| unsafe { sm64::sm64_set_mario_action(id, 0x0100_088C) });
                        }
                        self.phase = Some(Phase::Back);
                    }
                    None => sound(SOUND_BUZZ, 0.4),
                }
                return;
            }
        }
        if pressed {
            let busy = state.action & (ACT_FLAG_SWIMMING | ACT_FLAG_INVULNERABLE) != 0 || state.action == moves::ACT_ER_ZIP;
            if self.phase.is_some() || tick < self.ready_at || !allowed || busy {
                if self.phase.is_none() && allowed {
                    sound(SOUND_BUZZ, 0.4);
                }
            } else {
                self.pull = l2;
                self.throw(id, state, feet, hand, origin, tick, air, &ray);
            }
        }
        let Some(phase) = self.phase else { return };
        let mut next = Some(phase);
        // (off the AoT hook's line: no wait for the next)
        let mut quick = false;
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
                    Aim::Bone { mob, bone, .. } => match crate::odm::bone_at(&mob, bone) {
                        Some(p) => p,
                        None => {
                            next = Some(Phase::Back);
                            self.pos
                        }
                    },
                    Aim::Map { at, .. } | Aim::Miss { at, .. } => at,
                };
                let d = to - self.pos;
                let step = SPEED * DT;
                if next == Some(Phase::Out) {
                    if d.length() <= step {
                        self.pos = to;
                        let v = state.velocity;
                        let vel = Vec3::new(-v[0], v[1], v[2]) * crate::scale() * 30.0;
                        next = self.latch(id, feet, vel, button, origin, tick, air, player, combat, &ray);
                    } else {
                        self.pos += d / d.length() * step;
                    }
                }
            }
            Phase::Pulling => {
                // X as it comes in, on his feet: the home run (er_moves.c's swing, bat.rs's flight)
                if x_pressed && !air && allowed && state.action & ACT_FLAG_INVULNERABLE == 0 {
                    log("hook: home run swing");
                    worker::call("home run", move |_| unsafe { sm64::sm64_set_mario_action(id, moves::ACT_ER_HOME_RUN) });
                }
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
                } else if let Aim::Enemy { mob, radius, height, medium, .. } = self.aim {
                    match chest(&mob, height) {
                        Some((c, f)) => {
                            self.pos = c;
                            let to = zip_to_enemy(feet, f, radius, height);
                            let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
                            // (a medium one: the zip-kick, its blow once as he gets to it)
                            let kind = if medium { 3 } else { 0 };
                            worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], kind, 0) });
                            if medium && !self.kicked && (c - hand).length() <= radius + KICK_REACH {
                                self.kicked = true;
                                self.zip_kick(&mob, radius, origin, tick, player, combat);
                            }
                        }
                        None => next = Some(Phase::Back),
                    }
                }
            }
            // on a big one's bone (odm.rs reels him in): the cap where the line's anchored; off it,
            // the cap's back at once, ready for the next (the rebound's A or R2: the next bone)
            Phase::Odm => match crate::odm::anchor() {
                Some(p) => self.pos = p,
                None => {
                    quick = true;
                    next = None;
                }
            },
            // a medium one caught with R2 held: let go soon, the zip-kick; held 0.2 s from the throw,
            // the vault over it
            Phase::Deciding(_) => {
                if let Aim::Enemy { mob, radius, height, .. } = self.aim {
                    match chest(&mob, height) {
                        Some((c, _)) => {
                            self.pos = c;
                            if !button || !allowed {
                                next = self.zip_kick_to(id, &mob, radius, height, feet, origin, tick);
                            } else if tick.wrapping_sub(self.born) >= VAULT_HOLD {
                                next = if crate::odm::vault(&mob, feet, tick, &ray) {
                                    log("hook: the vault over it");
                                    Some(Phase::Back)
                                } else {
                                    self.zip_kick_to(id, &mob, radius, height, feet, origin, tick)
                                };
                            }
                        }
                        None => next = Some(Phase::Back),
                    }
                }
            }
            // on the rope (colossus.rs swings him): the cap where it hangs from; off it, it comes back
            // and the map is fair game again (rope to rope, Spider-Man's way)
            Phase::Roping => match crate::colossus::rope_pivot() {
                Some(p) => self.pos = p,
                None => {
                    self.map_used = false;
                    self.pressed = true;
                    next = Some(Phase::Back);
                }
            },
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
        // (the line and the rope last as long as they do: odm.rs, colossus.rs let go)
        if next.is_some() && tick.wrapping_sub(self.born) > LIFE && next != Some(Phase::Back) && !matches!(next, Some(Phase::Odm | Phase::Roping)) {
            next = Some(Phase::Back);
        }
        if tick.wrapping_sub(self.born) > LIFE + 150 && !matches!(next, Some(Phase::Odm | Phase::Roping)) {
            next = None;
        }
        if next.is_none() {
            self.ready_at = tick + if quick { 0 } else { COOLDOWN };
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
        // (near ones all around, the nearest first, those in front before those behind; far ones only
        // where he or the camera looks)
        // (why near ones weren't taken, for the log: out of sight, outside the cone)
        let (mut hidden, mut aside) = (0u32, 0u32);
        let chest = feet + Vec3::Y * (CHEST + 0.4);
        // (its reach by the place: shut in, as ever; in the open by a colossus, far longer, colossus.rs)
        let reach = crate::colossus::hook_reach(feet, origin);
        if reach > RANGE + 1.0 {
            log(format!("hook: reach {reach:.0} m (open {:.1})", crate::lakitu::openness()));
        }
        let enemy = combat::nearby(&here, reach, origin)
            .iter()
            .filter_map(|t| {
                let mob = *t.shootable()?;
                let c = sm_to_er(origin, [t.sm[0], t.sm[1] + t.height.min(300.0) * 0.5, t.sm[2]]);
                let c = Vec3::new(c.0, c.1, c.2);
                let d = (c - hand).with_y(0.0);
                let cos = d.normalize_or(flat).dot(flat).max(d.normalize_or(face).dot(face));
                let dist = d.length().max((c - hand).length() * 0.8);
                let (radius, height) = (t.radius * crate::scale(), t.height * crate::scale());
                // (near is measured to its body: a big one's centre is far off when its leg is right there)
                let edge = (dist - radius).max(0.0);
                if d.length() < 0.3 {
                    return None;
                }
                if cos < AIM_CONE_COS && edge > AIM_NEAR {
                    aside += 1;
                    return None;
                }
                // (in sight from his hand or his chest, to its middle or high on it: a bump in the
                // ground or a low fence between his hand and its middle hid enemies right there)
                let high = sm_to_er(origin, [t.sm[0], t.sm[1] + t.height.min(400.0) * 0.85, t.sm[2]]);
                let high = Vec3::new(high.0, high.1, high.2);
                if [hand, chest].iter().all(|&from| ray(from, c).is_some() && ray(from, high).is_some()) {
                    hidden += 1;
                    return None;
                }
                let size = crate::juggle::pull_size(&mob, radius, height);
                let (drag, medium) = (size != crate::juggle::PullSize::Big, size == crate::juggle::PullSize::Medium);
                // (the edge of a big one's body counts, not its centre; one close by is taken
                // before a farther one in front, it's the one he's fighting)
                let gap = (dist - radius * 0.7).max(0.5);
                let close = if edge <= AIM_CLOSE { 0.5 } else { 1.0 };
                Some((gap * (1.8 - cos) * close, Aim::Enemy { mob, radius, height, drag, medium }, c))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if enemy.is_none() && hidden + aside > 0 {
            log(format!("hook: no enemy taken ({hidden} out of sight, {aside} outside the aim)"));
        }
        let (aim, at) = match enemy {
            // a big one (not L2's): the bone nearest the middle of the screen, the line's anchor
            // (odm.rs); none there, the zip as ever
            Some((_, Aim::Enemy { mob, radius, height, drag: false, medium }, c)) if !self.pull => match crate::odm::aim(&mob, hand, reach) {
                Some((bone, p)) => (Aim::Bone { mob, bone, radius, height }, p),
                None => (Aim::Enemy { mob, radius, height, drag: false, medium }, c),
            },
            Some((_, aim, c)) => (aim, c),
            None => {
                // the map where the camera looks (a little up without Lakitu)
                let dir = look.unwrap_or((face + Vec3::Y * 0.2).normalize());
                let from = feet + Vec3::Y * CHEST;
                let hit = ray(from, from + dir * reach);
                // (nothing there, the sky: a spot for the rope ahead and above, if R2 stays held; a
                // wall ahead is the zip and the cling, a floor or a ledge the zip there, as ever)
                let floor_at = |p: Vec3| {
                    let near = p - dir * 0.3;
                    ray(near + Vec3::Y * 0.4, near - Vec3::Y * 0.6).is_some()
                };
                let cone = || {
                    let far = reach.min(CONE_REACH).min(crate::colossus::rope_reach(feet, origin));
                    let hits: Vec<(Vec3, Option<Vec3>)> = crate::odm::cone_dirs(flat).into_iter().map(|d| (d, ray(from, from + d * far))).collect();
                    let spot = crate::odm::cone_pick(&hits, from, feet, flat)?;
                    let d = spot - from;
                    log(format!("hook: the rope's cone found a spot {:.1} m ahead, {:.0} deg up", d.with_y(0.0).length(), (d.y / d.length()).asin().to_degrees()));
                    Some(spot)
                };
                match hit {
                    // (L2 only takes enemies: it flies out and back)
                    Some(_) if self.pull => (Aim::Miss { at: from + dir * reach, clink: false }, from + dir * reach),
                    // (once an airtime: then it only clinks off)
                    Some(p) if self.map_used => (Aim::Miss { at: p, clink: true }, p),
                    // (a floor if there's ground right under a spot just short of it)
                    Some(p) => (Aim::Map { at: p, floor: floor_at(p), cone: false }, p),
                    None if self.pull || self.map_used => (Aim::Miss { at: from + dir * reach, clink: false }, from + dir * reach),
                    None => match cone() {
                        Some(c) => (Aim::Map { at: c, floor: false, cone: true }, c),
                        None => (Aim::Miss { at: from + dir * reach, clink: false }, from + dir * reach),
                    },
                }
            }
        };
        log(format!(
            "hook: {}",
            match aim {
                Aim::Enemy { drag: true, .. } if self.pull => "an enemy, to pull in",
                Aim::Enemy { .. } if self.pull => "a big enemy (L2: only a tap)",
                Aim::Enemy { medium: true, .. } => "a medium enemy, to fly to (R2 held: the vault)",
                Aim::Enemy { .. } => "an enemy, to fly to",
                Aim::Bone { .. } => "a big enemy's bone, the line",
                Aim::Map { cone: true, .. } => "a spot for the rope",
                Aim::Map { floor: true, .. } => "a floor",
                Aim::Map { .. } => "a wall",
                Aim::Miss { clink: true, .. } => "the map again this airtime (clinks off)",
                Aim::Miss { .. } => "nothing",
            }
        ));
        self.aim = aim;
        self.phase = Some(Phase::Winding(tick));
        self.born = tick;
        self.kicked = false;
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
    fn latch(&mut self, id: i32, feet: Vec3, vel: Vec3, button: bool, origin: [f32; 3], tick: u32, air: bool, player: &ChrIns, combat: &mut Combat, ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Option<Phase> {
        match self.aim {
            Aim::Bone { mob, bone, radius, height } => {
                self.tap(&mob, radius, origin, tick, player, combat);
                if crate::odm::start(&mob, bone, feet, vel, tick) {
                    log("hook: on the line");
                    return Some(Phase::Odm);
                }
                // (its body or bone gone: the zip as ever)
                let Some((_, f)) = chest(&mob, height) else { return Some(Phase::Back) };
                let to = zip_to_enemy(feet, f, radius, height);
                let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
                worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], 0, 1) });
                self.aim = Aim::Enemy { mob, radius, height, drag: false, medium: false };
                Some(Phase::Zipping(tick))
            }
            Aim::Enemy { mob, radius, height, drag, medium } => {
                // its little hit (a flinch, a point of style), then in it comes, or he goes
                self.tap(&mob, radius, origin, tick, player, combat);
                // (L2: the enemy comes, if it's small or medium; a big one, or a medium one pulled
                // not long ago, only gets the tap)
                if self.pull {
                    if drag && !(medium && medium_waits(&mob)) && crate::juggle::pull(&mob, air, radius, medium) {
                        if medium {
                            medium_pulled(&mob);
                        }
                        log(format!("hook: pulling {} enemy in", if medium { "a medium" } else { "a small" }));
                        return Some(Phase::Pulling);
                    }
                    log(if drag { "hook: a medium enemy, not again yet (a tap)" } else { "hook: a big enemy: a tap, nothing moves" });
                    sound(SOUND_BUZZ, 0.35);
                    return Some(Phase::Back);
                }
                // a medium one: R2 let go already, the zip-kick; held, it's decided on (0.2 s from
                // the throw: the vault over it, odm.rs)
                if medium {
                    if button && tick.wrapping_sub(self.born) >= VAULT_HOLD && crate::odm::vault(&mob, feet, tick, ray) {
                        log("hook: the vault over it");
                        return Some(Phase::Back);
                    }
                    if button && tick.wrapping_sub(self.born) < VAULT_HOLD {
                        return Some(Phase::Deciding(tick));
                    }
                    return self.zip_kick_to(id, &mob, radius, height, feet, origin, tick);
                }
                let Some((_, f)) = chest(&mob, height) else { return Some(Phase::Back) };
                let to = zip_to_enemy(feet, f, radius, height);
                let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
                log("hook: Mario flies to it");
                worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], 0, 1) });
                Some(Phase::Zipping(tick))
            }
            Aim::Map { at, floor, cone } => {
                // (the cone's spot is only the rope's: R2 let go, the cap comes back)
                if cone && !button {
                    log("hook: the rope's spot, R2 let go");
                    return Some(Phase::Back);
                }
                self.map_used = true;
                sound(SOUND_CLINK, 0.55);
                crate::impact::hook_latch(at);
                // R2 held on a spot high above him, aimed up at it (a branch, a ledge, a ceiling;
                // a wall ahead is the zip and the cling, as ever): the rope there
                let up = cone || (at - feet).normalize_or_zero().y > 0.45;
                if button && up && at.y > feet.y + crate::colossus::ROPE_ABOVE && crate::colossus::start_rope(Some(at), None, feet, vel, air, origin) {
                    log("hook: the rope, on the map");
                    return Some(Phase::Roping);
                }
                // (the cone's spot is no place to zip to: a ceiling, a branch over him)
                if cone {
                    log("hook: the rope's spot, no rope there");
                    return Some(Phase::Back);
                }
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
            Aim::Miss { at, clink } => {
                if clink {
                    sound(SOUND_CLINK, 0.4);
                    crate::impact::hook_latch(at);
                }
                Some(Phase::Back)
            }
        }
    }
}

/// a medium enemy pulled in with L2 can't be again for this long (s)
const MEDIUM_EVERY: f32 = 5.0;
static MEDIUM_PULLED: Mutex<Vec<(u64, Instant)>> = Mutex::new(Vec::new());

fn medium_waits(mob: &FieldInsHandle) -> bool {
    MEDIUM_PULLED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(k, t)| *k == key(mob) && t.elapsed().as_secs_f32() < MEDIUM_EVERY)
}

fn medium_pulled(mob: &FieldInsHandle) {
    let mut list = MEDIUM_PULLED.lock().unwrap_or_else(|e| e.into_inner());
    list.retain(|(k, t)| *k != key(mob) && t.elapsed().as_secs_f32() < MEDIUM_EVERY);
    list.push((key(mob), Instant::now()));
}

/// R2 held this many ticks from the throw on a medium one: the vault over it (let go sooner, the
/// zip-kick); the zip-kick's blow from his hand this far off its body (m)
const VAULT_HOLD: u32 = 6;
const KICK_REACH: f32 = 1.3;
/// the rope's cone looks this far (m)
const CONE_REACH: f32 = 25.0;
/// the tether's second enemy within this far of the first (m)
const TETHER_RANGE: f32 = 15.0;

impl Hook {
    /// The hook's little hit on `mob` as it latches (a flinch, a point of style), and its sound.
    fn tap(&mut self, mob: &FieldInsHandle, radius: f32, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat) {
        let here = HavokPosition(self.pos.x, self.pos.y, self.pos.z, 0.0);
        let near = combat::nearby(&here, 4.0 + radius, origin);
        if let Some(i) = near.iter().position(|t| t.shootable().is_some_and(|h| key(h) == key(mob))) {
            combat.deal(player, &near, &[(i, Attack::Hook, false)], tick);
        }
        sound(SOUND_LATCH, 0.6);
        crate::impact::hook_latch(self.pos);
        // (an enemy gives the wall back)
        self.map_used = false;
    }

    /// Off to medium enemy `mob`: the zip that ends in the kick (zip kind 3).
    #[allow(clippy::too_many_arguments)]
    fn zip_kick_to(&self, id: i32, mob: &FieldInsHandle, radius: f32, height: f32, feet: Vec3, origin: [f32; 3], tick: u32) -> Option<Phase> {
        let (_, f) = chest(mob, height)?;
        let to = zip_to_enemy(feet, f, radius, height);
        let sm = er_to_sm(origin, &HavokPosition(to.x, to.y, to.z, 0.0));
        log("hook: Mario flies to it, the zip-kick");
        worker::call("hook zip", move |_| unsafe { sm64::sm64_er_zip(id, sm[0], sm[1], sm[2], 3, 1) });
        Some(Phase::Zipping(tick))
    }

    /// The zip-kick's blow on medium enemy `mob` as he gets to it.
    fn zip_kick(&self, mob: &FieldInsHandle, radius: f32, origin: [f32; 3], tick: u32, player: &ChrIns, combat: &mut Combat) {
        let here = HavokPosition(self.pos.x, self.pos.y, self.pos.z, 0.0);
        let near = combat::nearby(&here, 4.0 + radius, origin);
        if let Some(i) = near.iter().position(|t| t.shootable().is_some_and(|h| key(h) == key(mob))) {
            log("hook: zip-kick");
            combat.deal(player, &near, &[(i, Attack::ZipKick, false)], tick);
        }
    }

    /// The tether's other enemy: the best one near the first, other than it, in sight of it.
    fn second(&self, first: &FieldInsHandle, feet: Vec3, origin: [f32; 3], ray: &impl Fn(Vec3, Vec3) -> Option<Vec3>) -> Option<FieldInsHandle> {
        let at = self.pos;
        let look = crate::lakitu::forward().map(|v| v.with_y(0.0).normalize_or(Vec3::Z)).unwrap_or(Vec3::Z);
        combat::nearby(&HavokPosition(at.x, at.y, at.z, 0.0), TETHER_RANGE, origin)
            .iter()
            .filter_map(|t| {
                let mob = *t.shootable()?;
                if key(&mob) == key(first) {
                    return None;
                }
                let c = Vec3::new(t.er.0, t.er.1 + 0.6 + (t.height * crate::scale()).min(3.0) * 0.5, t.er.2);
                if ray(at, c).is_some() {
                    return None;
                }
                // (the one most in front of the camera, then the nearest)
                let d = (c - feet).with_y(0.0);
                let cos = d.normalize_or(look).dot(look);
                Some(((c - at).length() * (1.8 - cos), mob))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, m)| m)
    }
}

/// Where Mario flies to at a big enemy (Elden Ring): just off its body on his side, about its
/// waist high (no higher than he could hit).
fn zip_to_enemy(mario: Vec3, feet: Vec3, radius: f32, height: f32) -> Vec3 {
    let side = (mario - feet).with_y(0.0).normalize_or(Vec3::Z);
    feet + side * (radius + BODY + 0.3) + Vec3::Y * (height * 0.35).min(2.5)
}
