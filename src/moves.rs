//! Mario's extra moves on the pad: B dashes (a quick lunge where the stick points, out of harm's
//! way, libsm64's ACT_ER_DASH and ACT_ER_AIR_DASH) and Y does a special move that depends on
//! what he's doing:
//! - standing or walking: SM64's breakdance sweep, hitting all around him
//! - running: a slide kick, closing in
//! - right after a punch (X, the combo): a ground pound slam, its shockwave hits everyone around
//! - in the air: a twirl that lifts him a little and hits all around (once per jump)
//! And X's combo has a second form (Devil May Cry's pause combo): punch, punch, let the second
//! punch end, then X again within a beat: a rush of two quick jabs and a flying kick that hit
//! harder (libsm64's ACT_ER_RUSH).
//!
//! The combos (libsm64's er_moves.c), the reward for a longer string of buttons:
//! - X, X, Y: the super jump punch (Super Smash Bros.), and X in it: the meteor dive
//! - X, X, Y also lifts small enemies (juggle.rs): X in the air goes on with an air string, Y ends
//!   it with the meteor dive (Devil May Cry's launcher, air rave and helm breaker)
//! - Y, X (X in the sweep): the Mario Tornado (Melee)
//! - B, Y (Y in the dash): the Koopa shell, rolling into everyone
//! - X past the combo's kick: the string goes on, a spinning backfist, two kicks, a whirlwind kick
//! L1 takes the next weapon (weapons.rs; held, back to the fists), and changes it all:
//! - the hammer (hammer.rs): X swings it (two side swings and an overhead smash), Y slams it (held:
//!   charged, in the air: the plunge), X, Y the launcher, X, X, Y the Donkey Kong flurry and Y, X
//!   (X after the slam) the Bowser spin. Its X string is five swings long.
//! - the boomerang (boomerang.rs): X throws one (three in the air at once at most), Y three in a
//!   fan, held the big one, X, X, Y the launcher; in the air X and Y throw too, each holding him up
//! - the Chain Chomp (chomp.rs): X lashes, sweeps, crashes it down; Y the cyclone, X, Y the bite
//!   that tosses them up; in the air X swings it round him, Y crashes down with it

use crate::sm64;

pub const ACT_ER_DASH: u32 = 0x0002_047B;
pub const ACT_ER_AIR_DASH: u32 = 0x0002_08B9;
const ACT_ER_RUSH: u32 = 0x0080_038A;
const ACT_ER_UPPERCUT: u32 = 0x0080_08BA;
const ACT_ER_METEOR: u32 = 0x0080_08BB;
const ACT_ER_HAMMER_AIR: u32 = 0x0080_08BC;
const ACT_ER_HAMMER: u32 = 0x0080_038B;
const ACT_ER_HAMMER_SLAM: u32 = 0x0080_038C;
const ACT_ER_HAMMER_FLURRY: u32 = 0x0080_038D;
const ACT_ER_HAMMER_GIRO: u32 = 0x0080_038E;
const ACT_ER_TORNADO: u32 = 0x0080_038F;
const ACT_ER_METEOR_LAND: u32 = 0x0080_0393;
const ACT_ER_CHAIN: u32 = 0x0080_0394;
const ACT_ER_SHELL: u32 = 0x0080_0395;
const ACT_ER_HAMMER_LAUNCH: u32 = 0x0080_0396;
const ACT_ER_AIR_COMBO: u32 = 0x0080_08BF;
pub const ACT_ER_THROW: u32 = 0x0080_0397;
const ACT_ER_CHOMP: u32 = 0x0080_0398;
const ACT_ER_CHOMP_SPIN: u32 = 0x0080_0399;
pub const ACT_ER_AIR_THROW: u32 = 0x0080_0890;
const ACT_ER_CHOMP_AIR: u32 = 0x0080_0891;
/// the throws (er_moves.c ER_THROW_*): the boomerang's, then the fireball's by charge level
pub const THROW_STORM: u32 = 1;
pub const THROW_UPPER: u32 = 2;
pub const THROW_BIG: u32 = 3;
pub const THROW_FIRE: u32 = 4;
/// the hook's (hook.rs), and the pull along its line to what it caught
pub const THROW_HOOK: u32 = 8;
pub const ACT_ER_ZIP: u32 = 0x0080_0892;
/// the combos' moves
const COMBO_ACTIONS: [u32; 18] = [
    ACT_ER_UPPERCUT, ACT_ER_METEOR, ACT_ER_HAMMER_AIR, ACT_ER_HAMMER, ACT_ER_HAMMER_SLAM, ACT_ER_HAMMER_FLURRY, ACT_ER_HAMMER_GIRO,
    ACT_ER_TORNADO, ACT_ER_METEOR_LAND, ACT_ER_CHAIN, ACT_ER_SHELL, ACT_ER_AIR_COMBO, ACT_ER_HAMMER_LAUNCH, ACT_ER_THROW, ACT_ER_CHOMP,
    ACT_ER_CHOMP_SPIN, ACT_ER_AIR_THROW, ACT_ER_CHOMP_AIR,
];
/// the weapons' moves on the ground: L1 waits for them to end
const HAMMER_ACTIONS: [u32; 8] =
    [ACT_ER_HAMMER, ACT_ER_HAMMER_SLAM, ACT_ER_HAMMER_FLURRY, ACT_ER_HAMMER_GIRO, ACT_ER_HAMMER_LAUNCH, ACT_ER_THROW, ACT_ER_CHOMP, ACT_ER_CHOMP_SPIN];
/// L1 held this long (ticks): back to the fists
const L1_HOLD: u32 = 12;
/// SM64's breakdance (the sweep)
const ANIM_BREAKDANCE: i32 = 0x71;
/// X this soon (ticks) after the sweep or the hammer's slam ended: the tornado, the Bowser spin;
/// Y this soon after a string of punches or swings (plus Y's own wait): the super jump punch, the
/// flurry
const COMBO_WINDOW: u32 = 12;
/// an X pressed early for Y, X (during Y's wait, the sweep, the slam's wind-up) still counts this
/// long (ticks)
const X_BUFFER: u32 = 24;
/// the slam's blow, on its timer (er_moves.c ER_SLAM_IMPACT)
const SLAM_IMPACT: i32 = 11;
const ACT_PUNCHING: u32 = 0x0080_0380;
const ACT_MOVE_PUNCHING: u32 = 0x0080_0457;
const ACT_SLIDE_KICK: u32 = 0x0180_08AA;
const ACT_GROUND_POUND: u32 = 0x0080_08A9;
const ACT_TWIRLING: u32 = 0x1080_08A4;
const ACT_FREEFALL: u32 = 0x0100_088C;
/// SM64's punch sequence step for the breakdance sweep (crouch sliding + B)
const BREAKDANCE: u32 = 9;
/// SM64's animation: the combo's kick
const ANIM_GROUND_KICK: i32 = 0x66;
/// the pause combo: the third X this long (ticks) after the second, a beat instead of a quick
/// press (it used to count from the second punch's end, a timing hard to find)
const PAUSE_MIN: u32 = 10;
const PAUSE_MAX: u32 = 36;
/// a string's X presses keep counting this long after it ended (beyond the combo window)
const CHAIN_GRACE: u32 = 4;

const ACT_FLAG_STATIONARY: u32 = 0x200;
const ACT_FLAG_MOVING: u32 = 0x400;
const ACT_FLAG_AIR: u32 = 0x800;
const ACT_FLAG_INVULNERABLE: u32 = 0x0002_0000;
/// swimming, on a pole, hanging, intangible: no moves there
const ACT_NO_MOVES: u32 = 0x2000 | 0x0010_0000 | 0x0020_0000 | 0x1000;
const ACT_GROUP_MASK: u32 = 0x1C0;
const ACT_GROUP_CUTSCENE: u32 = 0x100;

/// air dashes in a jump at most (one, more in an air combo)
const AIR_DASHES: u32 = 3;
/// ticks between two of Mega Mario's slams
const MEGA_SLAM_COOLDOWN: u32 = 75;
/// ticks between two dashes
const DASH_COOLDOWN: u32 = 12;
/// running speed from which Y slide kicks
const RUN_SPEED: f32 = 24.0;
/// the twirl's upward pop, and how long it lasts before he just falls (ticks)
const TWIRL_POP: f32 = 30.0;
const TWIRL_TICKS: u32 = 24;

/// What a combo's move hits on a tick (libsm64's er_moves.c ErHit), measured by combat.rs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Hammer,
    HammerSmash,
    SmashWave,
    Quake,
    Flurry,
    FlurryEnd,
    Giro,
    GiroEnd,
    HammerAir,
    Uppercut,
    UppercutTop,
    Meteor,
    MeteorLand,
    Tornado,
    TornadoEnd,
    Chain,
    ChainEnd,
    Shell,
    Air,
    Launch,
    Chomp,
    ChompSpin,
    ChompEnd,
    ChompAir,
    ChompLaunch,
}

impl Hit {
    pub fn from(v: i32) -> Option<Hit> {
        use Hit::*;
        [
            Hammer, HammerSmash, SmashWave, Quake, Flurry, FlurryEnd, Giro, GiroEnd, HammerAir, Uppercut, UppercutTop, Meteor, MeteorLand, Tornado, TornadoEnd, Chain,
            ChainEnd, Shell, Air, Launch, Chomp, ChompSpin, ChompEnd, ChompAir, ChompLaunch,
        ]
            .get((v as usize).wrapping_sub(1))
            .copied()
    }
}

/// What a combo's move shows on a tick (er_moves.c ER_FX_*, impact.rs)
pub const FX_GROUND: u32 = 0x01;
pub const FX_QUAKE: u32 = 0x02;
pub const FX_GIANT: u32 = 0x04;
pub const FX_METEOR: u32 = 0x08;
pub const FX_SPIN: u32 = 0x10;
pub const FX_LAUNCH: u32 = 0x20;
pub const FX_BANG: u32 = 0x40;
pub const FX_GROW: u32 = 0x80;
pub const FX_CHARGE: u32 = 0x100;

/// One of the combos' moves (combat.rs leaves their hits to `Hit`).
pub fn combo(action: u32) -> bool {
    COMBO_ACTIONS.contains(&action)
}

/// One of the weapons' moves on the ground (L1 waits for it to end).
pub fn hammer_busy(action: u32) -> bool {
    HAMMER_ACTIONS.contains(&action)
}

/// The meteor dive and the Koopa shell leave a streak like the dash's (impact.rs).
pub fn streaking(action: u32) -> bool {
    action == ACT_ER_METEOR || action == ACT_ER_SHELL
}

/// What to do to Mario before the next SM64 tick (in the worker, lib.rs).
#[derive(Clone, Copy, Debug)]
pub enum Request {
    Action(u32, u32),
    /// the twirl: the action and an upward pop (keeping his horizontal speed)
    Twirl([f32; 3]),
}

impl Request {
    /// Worker thread.
    pub fn apply(self, id: i32) {
        unsafe {
            match self {
                Request::Action(action, arg) => sm64::sm64_set_mario_action_arg(id, action, arg),
                Request::Twirl(v) => {
                    sm64::sm64_set_mario_action(id, ACT_TWIRLING);
                    sm64::sm64_set_mario_velocity(id, v[0], if crate::mega::active() { TWIRL_POP * 0.62 } else { TWIRL_POP }, v[2]);
                }
            }
        }
    }
}

pub struct Moves {
    b_held: bool,
    y_held: bool,
    x_held: bool,
    /// in the combo's kick, and when it last ended (X then: the string goes on)
    in_kick: bool,
    kick_end: Option<u32>,
    /// the second X of a string (the pause combo counts from it), the dash, the air string
    x2_at: Option<u32>,
    dash_seen: Option<u32>,
    air_seen: Option<u32>,
    dash_ready: u32,
    /// used up in this jump (back once he's on the ground; with an enemy up there an air blow
    /// gives the air dash back, a few times a jump: dash in between blows)
    air_dashed: bool,
    air_dashes: u32,
    air_spun: bool,
    twirl_since: Option<u32>,
    /// L1 went down at this tick (None: up), and it already went back to the fists
    lb_since: Option<u32>,
    lb_long: bool,
    /// the weapon in hand (weapons.rs)
    pub weapon: u8,
    /// in the sweep, and when it (and the hammer's slam) last ended
    in_sweep: bool,
    sweep_end: Option<u32>,
    slam_end: Option<u32>,
    /// X presses in the current string of punches or swings, and when it was last going
    x_chain: u32,
    chain_seen: Option<u32>,
    /// an X pressed after Y, kept for Y, X
    x_buffer: Option<u32>,
    /// Mega Mario's next slam
    mega_slam_ready: u32,
}

/// The dash's invulnerability: enemy blows go through him (lib.rs).
pub fn dodging(action: u32) -> bool {
    action == ACT_ER_DASH || action == ACT_ER_AIR_DASH
}

impl Moves {
    pub fn new() -> Self {
        Moves {
            b_held: false,
            y_held: false,
            x_held: false,
            in_kick: false,
            kick_end: None,
            x2_at: None,
            dash_seen: None,
            air_seen: None,
            dash_ready: 0,
            air_dashed: false,
            air_dashes: 0,
            air_spun: false,
            twirl_since: None,
            lb_since: None,
            lb_long: false,
            weapon: crate::weapons::FISTS,
            in_sweep: false,
            sweep_end: None,
            slam_end: None,
            x_chain: 0,
            chain_seen: None,
            x_buffer: None,
            mega_slam_ready: 0,
        }
    }

    /// The hammer is out.
    pub fn hammer(&self) -> bool {
        self.weapon == crate::weapons::HAMMER
    }

    /// Game thread, every SM64 tick: L1 held now, whether Mario may change hands (no menu, not
    /// riding or holding anyone). A tap takes the next weapon, held a moment the fists: the new one
    /// when it changes.
    pub fn switch_weapon(&mut self, lb: bool, allowed: bool, tick: u32) -> Option<u8> {
        use crate::weapons::{COUNT, FISTS};
        let next = match (lb, self.lb_since) {
            (true, None) => {
                self.lb_since = Some(tick);
                self.lb_long = false;
                None
            }
            (true, Some(t)) if !self.lb_long && tick.wrapping_sub(t) >= L1_HOLD => {
                self.lb_long = true;
                (self.weapon != FISTS).then_some(FISTS)
            }
            (false, Some(_)) => {
                self.lb_since = None;
                (!self.lb_long).then_some((self.weapon + 1) % COUNT)
            }
            _ => None,
        };
        let next = next.filter(|_| allowed)?;
        self.weapon = next;
        crate::weapons::set(next);
        Some(next)
    }

    /// Game thread, every SM64 tick: B, Y and X held now, whether Mario may use them (not holding
    /// an enemy, not on Yoshi, no menu), whether the game is busy with an interaction (a door, a
    /// grace, a menu: no Y move then), his last state. The move to start, if any.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        b: bool,
        y: bool,
        x: bool,
        allowed: bool,
        interacting: bool,
        state: &sm64::SM64MarioState,
        move_timer: i32,
        tick: u32,
    ) -> Option<Request> {
        let a = state.action;
        // (the held state is kept on every tick: behind `b &&` it was only on presses, so it stayed
        // "held" for good after the first one)
        let b_pressed = !std::mem::replace(&mut self.b_held, b) && b;
        let y_pressed = !std::mem::replace(&mut self.y_held, y) && y;
        let x_pressed = !std::mem::replace(&mut self.x_held, x) && x;
        // (Y is Mario's alone: the game's interact moved to D-pad down, lib.rs; no waiting for it)
        let y_pressed = y_pressed && !interacting;
        if y_pressed {
            self.x_buffer = None;
        }
        // X after Y: kept for Y, X (and away from SM64 while Y still waits)
        if x_pressed && allowed && (self.in_sweep || a == ACT_ER_HAMMER_SLAM) {
            self.x_buffer = Some(tick);
        }
        let punching = a == ACT_PUNCHING || a == ACT_MOVE_PUNCHING;
        // the sweep and the hammer's slam: when they end, the beat for the next button of a combo
        // starts
        let ended = |now: bool, was: &mut bool, end: &mut Option<u32>| {
            if now {
                *end = None;
            } else if *was {
                *end = Some(tick);
            }
            *was = now;
        };
        ended(a == ACT_MOVE_PUNCHING && state.anim_id == ANIM_BREAKDANCE, &mut self.in_sweep, &mut self.sweep_end);
        ended(punching && state.anim_id == ANIM_GROUND_KICK, &mut self.in_kick, &mut self.kick_end);
        if dodging(a) {
            self.dash_seen = Some(tick);
        }
        if a == ACT_ER_UPPERCUT || a == ACT_ER_AIR_COMBO {
            self.air_seen = Some(tick);
        }
        if a == ACT_ER_HAMMER_SLAM {
            self.slam_end = Some(tick);
        }
        let recent = |end: Option<u32>| end.is_some_and(|t| tick.wrapping_sub(t) <= COMBO_WINDOW);
        // the string of punches or swings: its X presses, counted until it has been over a while
        let chain_window = |end: Option<u32>| end.is_some_and(|t| tick.wrapping_sub(t) <= COMBO_WINDOW + CHAIN_GRACE);
        let in_chain = a == ACT_ER_HAMMER
            || a == ACT_ER_CHAIN
            || a == ACT_ER_RUSH
            || a == ACT_ER_THROW
            || a == ACT_ER_CHOMP
            || (punching && state.anim_id != ANIM_BREAKDANCE);
        let fists = self.weapon == crate::weapons::FISTS;
        // the pause combo: punch, punch, a beat, X
        let mut pause = false;
        if x_pressed {
            pause = self.x_chain == 2 && self.x2_at.is_some_and(|t| (PAUSE_MIN..=PAUSE_MAX).contains(&tick.wrapping_sub(t)));
            self.x_chain = if in_chain || chain_window(self.chain_seen) || pause { self.x_chain + 1 } else { 1 };
            if self.x_chain == 2 {
                self.x2_at = Some(tick);
            }
        }
        if in_chain {
            self.chain_seen = Some(tick);
        }
        let x_twice = self.x_chain >= 2 && (in_chain || chain_window(self.chain_seen));
        let x_late = self.x_buffer.is_some_and(|t| tick.wrapping_sub(t) <= X_BUFFER);
        let air = a & ACT_FLAG_AIR != 0;
        if !air {
            self.air_dashed = false;
            self.air_dashes = 0;
            self.air_spun = false;
        }
        let air_blow = matches!(a, ACT_ER_AIR_COMBO | ACT_ER_HAMMER_AIR | ACT_ER_CHOMP_AIR | ACT_ER_AIR_THROW);
        if air_blow && self.air_dashed && self.air_dashes < AIR_DASHES && crate::juggle::active() {
            self.air_dashed = false;
        }
        // the twirl lasts a moment, then he falls
        if a == ACT_TWIRLING {
            let since = *self.twirl_since.get_or_insert(tick);
            if tick.wrapping_sub(since) >= TWIRL_TICKS {
                self.twirl_since = None;
                return Some(Request::Action(ACT_FREEFALL, 0));
            }
        } else {
            self.twirl_since = None;
        }
        let free = allowed && a & (ACT_FLAG_INVULNERABLE | ACT_NO_MOVES) == 0 && a & ACT_GROUP_MASK != ACT_GROUP_CUTSCENE;
        let ground = free && a & (ACT_FLAG_STATIONARY | ACT_FLAG_MOVING) != 0;
        let in_air = free && air && a != ACT_GROUND_POUND;
        // (no dashes for Mega Mario: too fast for a giant's collision, mario_step.c)
        if b_pressed && tick >= self.dash_ready && !crate::mega::active() {
            if ground {
                self.dash_ready = tick + DASH_COOLDOWN;
                crate::log("moves: dash");
                return Some(Request::Action(ACT_ER_DASH, 0));
            }
            if in_air && !self.air_dashed {
                self.air_dashed = true;
                self.air_dashes += 1;
                self.dash_ready = tick + DASH_COOLDOWN;
                crate::log("moves: air dash");
                return Some(Request::Action(ACT_ER_AIR_DASH, 0));
            }
        }
        // Y, X: the tornado out of the sweep; with the hammer, the Bowser spin out of the slam (once
        // its blow has landed)
        if x_late && ground && fists && (self.in_sweep || recent(self.sweep_end)) {
            self.sweep_end = None;
            self.x_buffer = None;
            crate::log("moves: tornado (Y, X)");
            return Some(Request::Action(ACT_ER_TORNADO, 0));
        }
        let after_slam = if a == ACT_ER_HAMMER_SLAM { move_timer > SLAM_IMPACT } else { recent(self.slam_end) };
        if x_late && ground && self.hammer() && after_slam {
            self.slam_end = None;
            self.x_buffer = None;
            crate::log("moves: Bowser spin (hammer Y, X)");
            return Some(Request::Action(ACT_ER_HAMMER_GIRO, 0));
        }
        if pause && ground && fists {
            crate::log("moves: rush (pause combo)");
            return Some(Request::Action(ACT_ER_RUSH, 0));
        }
        // X past the combo's kick: the string goes on (er_moves.c chains the rest)
        if x_pressed && ground && fists && (self.in_kick || recent(self.kick_end)) {
            self.kick_end = None;
            crate::log("moves: long string (X past the kick)");
            return Some(Request::Action(ACT_ER_CHAIN, 0));
        }
        // B, Y: the Koopa shell out of the dash (invulnerable then: not `ground`)
        let dashing = self.dash_seen.is_some_and(|t| tick.wrapping_sub(t) <= 6);
        if y_pressed && allowed && !air && dashing && !crate::mega::active() {
            self.dash_seen = None;
            crate::log("moves: Koopa shell (B, Y)");
            return Some(Request::Action(ACT_ER_SHELL, 0));
        }
        let x_once = self.x_chain == 1 && (in_chain || chain_window(self.chain_seen));
        // the boomerang: X, X, Y the launcher, Y three in a fan (held: the big one, er_moves.c tells
        // them apart); in the air three in a fan
        if y_pressed && self.weapon == crate::weapons::RANG {
            if crate::boomerang::ready() == 0 {
                return None;
            }
            if ground && (!combo(a) || a == ACT_ER_THROW) {
                let (kind, name) = if x_twice { (THROW_UPPER, "boomerang launcher (X, X, Y)") } else { (THROW_BIG, "boomerang Y") };
                self.x_chain = 0;
                crate::log(format!("moves: {name}"));
                return Some(Request::Action(ACT_ER_THROW, kind));
            }
            if in_air && (!self.air_spun || crate::juggle::active()) && a != ACT_ER_AIR_THROW {
                self.air_spun = true;
                crate::log("moves: boomerang fan in the air");
                return Some(Request::Action(ACT_ER_AIR_THROW, THROW_STORM));
            }
            return None;
        }
        // the Chain Chomp: X, Y the bite, Y the cyclone; in the air the crash
        if y_pressed && self.weapon == crate::weapons::CHOMP {
            if ground && (!combo(a) || a == ACT_ER_CHOMP) {
                let (action, arg, name) =
                    if x_once || x_twice { (ACT_ER_CHOMP, 3, "Chomp bite (X, Y)") } else { (ACT_ER_CHOMP_SPIN, 0, "Chomp cyclone") };
                self.x_chain = 0;
                crate::log(format!("moves: {name}"));
                return Some(Request::Action(action, arg));
            }
            if in_air && (!self.air_spun || crate::juggle::active()) && a != ACT_ER_CHOMP_AIR {
                self.air_spun = true;
                crate::log("moves: Chomp crash");
                return Some(Request::Action(ACT_ER_CHOMP_AIR, 1));
            }
            return None;
        }
        if y_pressed && self.hammer() {
            // (not out of the hammer's other moves, but out of its swings)
            if ground && (!combo(a) || a == ACT_ER_HAMMER) {
                // X, X, Y: the flurry; X, Y: the launcher; otherwise the slam
                let (action, name) = if x_twice {
                    (ACT_ER_HAMMER_FLURRY, "Donkey Kong flurry (hammer X, X, Y)")
                } else if x_once {
                    (ACT_ER_HAMMER_LAUNCH, "launcher (hammer X, Y)")
                } else {
                    (ACT_ER_HAMMER_SLAM, "hammer slam")
                };
                self.x_chain = 0;
                crate::log(format!("moves: {name}"));
                return Some(Request::Action(action, 0));
            }
            // (with an enemy up there the plunge brings it down: no once-a-jump limit)
            if in_air && (!self.air_spun || crate::juggle::active()) && a != ACT_ER_HAMMER_AIR {
                self.air_spun = true;
                crate::log("moves: hammer plunge");
                return Some(Request::Action(ACT_ER_HAMMER_AIR, 1));
            }
            return None;
        }
        if y_pressed {
            // X, X, Y: the super jump punch out of the second punch
            if ground && x_twice && !crate::mega::active() {
                self.x_chain = 0;
                crate::log("moves: super jump punch (X, X, Y)");
                return Some(Request::Action(ACT_ER_UPPERCUT, 0));
            }
            // (not out of the combos' other moves)
            if ground && combo(a) && a != ACT_ER_CHAIN {
                return None;
            }
            // Mega Mario: a two-fisted slam that shakes everything around (held: charged)
            if ground && crate::mega::active() {
                // (a moment between two: twenty seconds of them back to back flattened everything)
                if tick < self.mega_slam_ready {
                    return None;
                }
                self.mega_slam_ready = tick + MEGA_SLAM_COOLDOWN;
                crate::log("moves: Mega slam");
                return Some(Request::Action(ACT_ER_HAMMER_SLAM, 0));
            }
            if ground {
                let (request, name) = if punching || a == ACT_ER_RUSH {
                    (Request::Action(ACT_GROUND_POUND, 0), "slam")
                } else if state.forward_velocity >= RUN_SPEED {
                    (Request::Action(ACT_SLIDE_KICK, 0), "slide kick")
                } else {
                    // (the moving punch's: the standing one stops at a touch of the stick)
                    (Request::Action(ACT_MOVE_PUNCHING, BREAKDANCE), "sweep")
                };
                crate::log(format!("moves: {name}"));
                return Some(request);
            }
            // the air string's end (or the super jump punch's): the meteor dive, bringing the
            // enemies up there down with him
            let air_string = a == ACT_ER_UPPERCUT || a == ACT_ER_AIR_COMBO || self.air_seen.is_some_and(|t| tick.wrapping_sub(t) <= COMBO_WINDOW);
            if in_air && (air_string || crate::juggle::active()) && a != ACT_ER_METEOR && !crate::mega::active() {
                self.air_seen = None;
                crate::log("moves: meteor dive (air string, Y)");
                return Some(Request::Action(ACT_ER_METEOR, 0));
            }
            // (also out of a jump kick or the air dash: kick, then spin)
            if in_air && !self.air_spun && a != ACT_TWIRLING {
                self.air_spun = true;
                crate::log("moves: twirl");
                return Some(Request::Twirl(state.velocity));
            }
        }
        None
    }
}
