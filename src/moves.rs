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
//! Mixing them (Devil May Cry's weapon switching, God of War's): L1 in the middle of a string, or
//! right after a blow, switches on the spot and opens with the new weapon's swap strike, the string
//! going on: the hammer's overhead smash, the Chomp's wide sweep, the boomerangs' fan, the fists'
//! rush; in the air its air blow (a couple a jump, more with an enemy up there).

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
/// the trap kit's (traps.rs): a banana tossed, a shell kicked, a Bob-omb lobbed, a spring set down
pub const THROW_BANANA: u32 = 9;
pub const THROW_SHELL: u32 = 10;
pub const THROW_BOMB: u32 = 11;
pub const THROW_SPRING: u32 = 12;
pub const ACT_ER_ZIP: u32 = 0x0080_0892;
/// timing (timing.rs): the Mikiri stomp, the dash strike
pub const ACT_ER_MIKIRI: u32 = 0x0082_0893;
pub const ACT_ER_BONK: u32 = 0x0080_039A;
/// the hook's (hook.rs): the home run, the pole swing round a big enemy, the kick off it
pub const ACT_ER_HOME_RUN: u32 = 0x0080_039B;
pub const ACT_ER_POLE_SWING: u32 = 0x0080_0895;
pub const ACT_ER_SWING_KICK: u32 = 0x0080_0896;
/// forward + Y: the leap at an enemy into the weapon's dive (er_moves.c act_er_leap)
const ACT_ER_LEAP: u32 = 0x0080_0897;
/// X + A together: the weapon's rising blow, up into the air with whoever it catches (er_moves.c
/// act_er_rise)
pub const ACT_ER_RISE: u32 = 0x0080_089D;
/// the combos' moves
const COMBO_ACTIONS: [u32; 26] = [
    ACT_ER_UPPERCUT, ACT_ER_METEOR, ACT_ER_HAMMER_AIR, ACT_ER_HAMMER, ACT_ER_HAMMER_SLAM, ACT_ER_HAMMER_FLURRY, ACT_ER_HAMMER_GIRO,
    ACT_ER_TORNADO, ACT_ER_METEOR_LAND, ACT_ER_CHAIN, ACT_ER_SHELL, ACT_ER_AIR_COMBO, ACT_ER_HAMMER_LAUNCH, ACT_ER_THROW, ACT_ER_CHOMP,
    ACT_ER_CHOMP_SPIN, ACT_ER_AIR_THROW, ACT_ER_CHOMP_AIR, ACT_ER_MIKIRI, ACT_ER_BONK, crate::rock::ACT_ER_ROCK, ACT_ER_HOME_RUN, ACT_ER_POLE_SWING,
    ACT_ER_SWING_KICK, ACT_ER_LEAP, ACT_ER_RISE,
];
/// L1 held this long (ticks): back to the fists
const L1_HOLD: u32 = 12;
/// a switch this soon (ticks) after a blow is a swap strike; at most one this often; in the air at
/// most this many a jump (more with an enemy up there)
const SWAP_AFTER: u32 = 14;
const SWAP_COOLDOWN: u32 = 15;
const AIR_SWAPS: u32 = 2;
const AIR_SWAPS_JUGGLE: u32 = 4;
/// swap strikes in one string at most (each staggers: back to back they'd stunlock); the string is
/// over after this many ticks without a blow
const STRING_SWAPS: u32 = 2;
const STRING_OVER: u32 = 30;
/// the weapons' own moves: in the middle of one, L1 switches with a swap strike, or waits for it to
/// end (the move would go on with the wrong weapon in hand); the air string isn't one: it goes on
/// with whatever is in hand (er_moves.c er_air_attack)
const WEAPON_ACTIONS: [u32; 11] = [
    ACT_ER_HAMMER, ACT_ER_HAMMER_SLAM, ACT_ER_HAMMER_FLURRY, ACT_ER_HAMMER_GIRO, ACT_ER_HAMMER_LAUNCH, ACT_ER_HAMMER_AIR, ACT_ER_THROW, ACT_ER_AIR_THROW,
    ACT_ER_CHOMP, ACT_ER_CHOMP_SPIN, ACT_ER_CHOMP_AIR,
];
/// a switch that waited for a weapon's move to end is dropped after this many ticks
const PENDING_TICKS: u32 = 45;
const ACT_IDLE: u32 = 0x0C40_0201;
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
const SLAM_IMPACT: i32 = 13;
/// X and A this close together (ticks): the rising blow, not SM64's jump kick
const CHORD: u32 = 2;
/// a Y kept through a hammer string's locked swings this long at most (ticks: a queued swing
/// begins before the last one frees him, so the lock can last two swings)
const Y_BUFFER: u32 = 60;
const ACT_JUMP: u32 = 0x0300_0880;
const ACT_PUNCHING: u32 = 0x0080_0380;
const ACT_MOVE_PUNCHING: u32 = 0x0080_0457;
const ACT_SLIDE_KICK: u32 = 0x0180_08AA;
const ACT_GROUND_POUND: u32 = 0x0080_08A9;
const ACT_TWIRLING: u32 = 0x1080_08A4;
const ACT_FREEFALL: u32 = 0x0100_088C;
const ACT_DOUBLE_JUMP: u32 = 0x0300_0881;
const ACT_TRIPLE_JUMP: u32 = 0x0100_0882;
const ACT_SIDE_FLIP: u32 = 0x0100_0887;
/// SM64's punch sequence step for the breakdance sweep (crouch sliding + B)
const BREAKDANCE: u32 = 9;
/// SM64's animation: the combo's kick
const ANIM_GROUND_KICK: i32 = 0x66;
/// A press too soon in a scripted move is kept this long (ticks) for when it can cut it.
const SCRIPT_BUFFER: u32 = 8;
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
/// the stick this far toward or away from where he faces (cos) is "forward" / "back"; the back +
/// X launcher only from about standing (a run turning round shouldn't launch)
const LEAN: f32 = 0.6;
const LEAN_SLOW: f32 = 14.0;
/// the freeflow: a blow at most this long ago (ticks) and B is a leap to the next enemy
const FLOW_AFTER: u32 = 24;
/// X this soon (ticks) after the dash ended: the dash strike, at most this often (as er_moves.c's
/// ER_BONK_EVERY)
const BONK_AFTER: u32 = 4;
const BONK_EVERY: u32 = 36;
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
    Mikiri,
    Bonk,
    Rock,
    RockSlam,
    HomeRun,
    SwingKick,
    HammerWide,
    QuakeBig,
    Titan,
    Pound,
    Rise,
    HammerRise,
}

impl Hit {
    pub fn from(v: i32) -> Option<Hit> {
        use Hit::*;
        [
            Hammer, HammerSmash, SmashWave, Quake, Flurry, FlurryEnd, Giro, GiroEnd, HammerAir, Uppercut, UppercutTop, Meteor, MeteorLand, Tornado, TornadoEnd, Chain,
            ChainEnd, Shell, Air, Launch, Chomp, ChompSpin, ChompEnd, ChompAir, ChompLaunch, Mikiri, Bonk, Rock, RockSlam, HomeRun, SwingKick,
            HammerWide, QuakeBig, Titan, Pound, Rise, HammerRise,
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
/// a heavy hammer blow into the ground: a ring, dust, rubble
pub const FX_SLAM: u32 = 0x400;

/// The fists' string at step `step` (1 the first punch): SM64's punches and kick (its punch
/// sequence's actionArg: 0 the first punch, 3 the second, 6 the kick), then the long string.
fn fists_step(step: u32) -> Option<Request> {
    Some(match step {
        0 | 1 => Request::Action(ACT_PUNCHING, 0),
        2 => Request::Action(ACT_PUNCHING, 3),
        3 => Request::Action(ACT_PUNCHING, 6),
        _ => Request::Action(ACT_ER_CHAIN, 0),
    })
}

/// One of the combos' moves (combat.rs leaves their hits to `Hit`).
pub fn combo(action: u32) -> bool {
    COMBO_ACTIONS.contains(&action)
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
    /// the Mikiri (timing.rs): onto this head (SM64 units); whether it starts now or only moves
    Mikiri([f32; 3], bool),
    /// a bounce off something (the Bros. Jump off the partner's head): a double jump at this
    /// velocity (SM64 units per tick)
    Bounce([f32; 3]),
    /// off the spring (traps.rs): a triple jump's flip at this velocity
    Spring([f32; 3]),
    /// the Bros. Attack's leap (brosattack.rs): a triple jump's flip (a side flip with the wing cap
    /// on: no flying off; neither cuts its rise short when A isn't held) this fast up and forward
    /// (SM64 units per tick), facing `yaw` (SM64's radians)
    Leap { up: f32, forward: f32, yaw: f32, flip: bool },
    /// a new velocity, the action kept (the boulder off a big body, rock.rs)
    Velocity([f32; 3]),
    /// the freeflow: a leap to (x, y, z) (SM64 units), just off another enemy's body, popping up in
    /// front of it (er_moves.c's zip, as the hook's)
    Flow([f32; 3]),
    /// a scripted move (script.rs MOVES), on the ground or in the air, turned to `yaw` (SM64's
    /// radians: the enemy the stick was read against) as it starts
    Script { id: u16, air: bool, yaw: Option<f32> },
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
                Request::Mikiri([x, y, z], start) => sm64::sm64_er_mikiri(id, x, y, z, start as i32),
                Request::Bounce(v) => {
                    sm64::sm64_set_mario_action(id, ACT_DOUBLE_JUMP);
                    sm64::sm64_set_mario_velocity(id, v[0], v[1], v[2]);
                }
                Request::Velocity(v) => sm64::sm64_set_mario_velocity(id, v[0], v[1], v[2]),
                Request::Flow([x, y, z]) => sm64::sm64_er_zip(id, x, y, z, 0, 1),
                Request::Script { id: move_id, air, yaw } => {
                    sm64::sm64_set_mario_action_arg(id, if air { crate::script::ACT_ER_SCRIPT_AIR } else { crate::script::ACT_ER_SCRIPT }, move_id as u32);
                    if let Some(yaw) = yaw {
                        sm64::sm64_set_mario_faceangle(id, yaw);
                    }
                }
                Request::Spring(v) => {
                    sm64::sm64_set_mario_action(id, ACT_TRIPLE_JUMP);
                    sm64::sm64_set_mario_velocity(id, v[0], v[1], v[2]);
                }
                Request::Leap { up, forward, yaw, flip } => {
                    sm64::sm64_set_mario_action(id, if flip { ACT_TRIPLE_JUMP } else { ACT_SIDE_FLIP });
                    sm64::sm64_set_mario_faceangle(id, yaw);
                    sm64::sm64_set_mario_forward_velocity(id, forward);
                    sm64::sm64_set_mario_velocity(id, forward * yaw.sin(), up, forward * yaw.cos());
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
    /// the last tick he was throwing a blow; a swap strike to start (the weapon, when), the next
    /// one allowed, and those in this jump
    attack_seen: Option<u32>,
    swap: Option<(u8, u32)>,
    swap_ready: u32,
    air_swaps: u32,
    string_swaps: u32,
    /// a switch waiting for a weapon's move to end (the weapon, since when)
    pending: Option<(u8, u32)>,
    /// in the dash strike (for its log line), and when the next one may come
    bonk_seen: bool,
    bonk_ready: u32,
    /// a Y pressed in a hammer swing's wind-up, kept until it lands
    y_buffer: Option<u32>,
    /// when A and X last went down (X + A: the rising blow)
    a_at: Option<u32>,
    x_at: Option<u32>,
    /// the stick and buttons of the last moments, read as fighting-game inputs (combo.rs)
    grammar: crate::combo::Grammar,
    /// the Dodge Offset: X held through a dash keeps the string's step (combo.rs)
    offset: crate::combo::Offset,
    /// the scripted move playing (script.rs): lib.rs works out its frames
    pub script: crate::script::Player,
    /// X, Y or B pressed while the scripted move couldn't be cut yet: taken when it can (no press
    /// lost to the last ticks of a move)
    script_buffer: Option<(u8, u32)>,
    /// a move asked for from elsewhere (the sword's cast, fireball.rs), and when
    queued: Option<(Request, u32)>,
    /// the Star Sword (swordplay.rs): its move playing last tick, the last one that ended and when
    /// (the pause strings' beat counts from there), its pogo bounces left in this jump, L2 held
    sword_was: Option<u16>,
    sword_ended: Option<(u16, u32)>,
    pogos: u8,
    z_held: bool,
    /// a sword move asked for with no data for it (said once in the log)
    sword_missing: bool,
    /// Aerial Raves begun in this jump (each holds him up a while)
    raves: u8,
    /// the scripted move playing as moves were last read (by its start's count), and since when
    play_was: Option<u32>,
    play_since: u32,
}

/// the sword's pogo bounces a jump (Link's down-thrust)
const POGOS: u8 = 3;
/// Aerial Raves a jump: one, a few with an enemy up there with him (they hold him up: no flying
/// about the field on X alone)
const RAVES: u8 = 1;
const RAVES_JUGGLE: u8 = 3;

/// The dash's invulnerability: enemy blows go through him (lib.rs).
pub fn dodging(action: u32) -> bool {
    action == ACT_ER_DASH || action == ACT_ER_AIR_DASH
}

/// Enemy blows go through him: the dash, the Mikiri's somersault over the blow.
pub fn untouchable(action: u32) -> bool {
    dodging(action) || action == ACT_ER_MIKIRI
}

/// On his feet and free to counter (the Mikiri, timing.rs): not in the air, not hurt or in a
/// cutscene, not swimming or hanging.
pub fn can_counter(action: u32) -> bool {
    action & (ACT_FLAG_INVULNERABLE | ACT_NO_MOVES | ACT_FLAG_AIR) == 0
        && action & ACT_GROUP_MASK != ACT_GROUP_CUTSCENE
        && action & (ACT_FLAG_STATIONARY | ACT_FLAG_MOVING) != 0
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
            attack_seen: None,
            swap: None,
            swap_ready: 0,
            air_swaps: 0,
            string_swaps: 0,
            pending: None,
            bonk_seen: false,
            bonk_ready: 0,
            y_buffer: None,
            a_at: None,
            x_at: None,
            grammar: Default::default(),
            offset: Default::default(),
            script: Default::default(),
            script_buffer: None,
            queued: None,
            sword_was: None,
            sword_ended: None,
            pogos: POGOS,
            z_held: false,
            sword_missing: false,
            raves: 0,
            play_was: None,
            play_since: 0,
        }
    }

    /// A move asked for from elsewhere (the sword's cast): it goes in on the next tick.
    pub fn queue(&mut self, r: Request, tick: u32) {
        self.queued = Some((r, tick));
    }

    /// A swap strike is coming (the switch came in the middle of a fight).
    pub fn swapping(&self) -> bool {
        self.swap.is_some()
    }

    /// The hammer is out.
    pub fn hammer(&self) -> bool {
        self.weapon == crate::weapons::HAMMER
    }

    /// Game thread, every SM64 tick: L1 held now, whether Mario may change hands (no menu, not
    /// riding or holding anyone). A tap takes the next weapon, held a moment the fists: the new one
    /// when it changes.
    pub fn switch_weapon(&mut self, lb: bool, allowed: bool, action: u32, tick: u32) -> Option<u8> {
        use crate::weapons::FISTS;
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
                (!self.lb_long).then_some(crate::weapons::next(self.weapon))
            }
            _ => None,
        };
        // (a scripted move: only in its swap window)
        let script_busy = crate::script::is_script(action) && self.script.cancel() & crate::script::SWAP == 0;
        let busy = WEAPON_ACTIONS.contains(&action) || script_busy;
        // (one that waited: now that the move is over)
        let next = match next {
            Some(w) => Some(w),
            None => match self.pending {
                Some((_, at)) if tick.wrapping_sub(at) > PENDING_TICKS => {
                    self.pending = None;
                    None
                }
                Some((w, _)) if !busy => {
                    self.pending = None;
                    Some(w)
                }
                _ => None,
            },
        };
        let next = next.filter(|_| allowed)?;
        // in the middle of a fight: the new weapon comes in swinging; in the middle of one of the
        // weapons' moves, only that way, or it waits for the move to end
        let fighting = combo(action) || crate::script::is_script(action) || self.attack_seen.is_some_and(|t| tick.wrapping_sub(t) <= SWAP_AFTER);
        let swap = fighting && !script_busy && self.can_swap(next, action, tick);
        if busy && !swap {
            self.pending = Some((next, tick));
            return None;
        }
        self.pending = None;
        self.weapon = next;
        crate::weapons::set(next);
        if swap {
            self.swap = Some((next, tick));
        }
        Some(next)
    }

    /// The weapon's string, from its first blow (out of a scripted move's NEXT window).
    fn string_opener(&self, air: bool) -> Option<Request> {
        use crate::weapons::{CHOMP, FISTS, HAMMER, RANG};
        let rangs = crate::boomerang::ready() > 0;
        let (action, arg) = match (self.weapon, air) {
            (FISTS, false) => return fists_step(self.x_chain.max(1) + 1),
            (FISTS, true) => (ACT_ER_AIR_COMBO, 0),
            (HAMMER, false) => (ACT_ER_HAMMER, 0),
            (HAMMER, true) => (ACT_ER_HAMMER_AIR, 0),
            (CHOMP, false) => (ACT_ER_CHOMP, 0),
            (CHOMP, true) => (ACT_ER_CHOMP_AIR, 0),
            (RANG, false) if rangs => (ACT_ER_THROW, 0),
            (RANG, true) if rangs => (ACT_ER_AIR_THROW, 0),
            _ => return None,
        };
        Some(Request::Action(action, arg))
    }

    /// A swap strike with weapon `w` can start now, out of `action`.
    fn can_swap(&self, w: u8, action: u32, tick: u32) -> bool {
        use crate::weapons::{FISTS, RANG};
        let air = action & ACT_FLAG_AIR != 0;
        let air_left = self.air_swaps < if crate::juggle::active() { AIR_SWAPS_JUGGLE } else { AIR_SWAPS };
        // (juggling up there, the air's allowance is the string's too)
        let string_cap = if air && crate::juggle::active() { AIR_SWAPS_JUGGLE } else { STRING_SWAPS };
        tick >= self.swap_ready
            && self.string_swaps < string_cap
            && action & (ACT_FLAG_INVULNERABLE | ACT_NO_MOVES) == 0
            && (!air || air_left)
            && (w != RANG || crate::boomerang::ready() > 0)
            && (w != FISTS || !crate::mega::active())
    }

    /// Game thread, every SM64 tick: B, Y and X held now, A just pressed, whether Mario may use them (not holding
    /// an enemy, not on Yoshi, no menu), whether the game is busy with an interaction (a door, a
    /// grace, a menu: no Y move then), his last state, whether the Star Sword is in his hand, L2
    /// held (without the hook or R3 with it). The move to start, if any.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        b: bool,
        y: bool,
        x: bool,
        a_pressed: bool,
        a_held: bool,
        allowed: bool,
        interacting: bool,
        state: &sm64::SM64MarioState,
        move_timer: i32,
        locked: bool,
        lean: f32,
        stick: (f32, f32),
        flow: Option<[f32; 3]>,
        sword_in_hand: bool,
        z: bool,
        tick: u32,
    ) -> Option<Request> {
        let a = state.action;
        {
            use crate::combo::{A, B, X, Y};
            let buttons = if x { X } else { 0 } | if y { Y } else { 0 } | if b { B } else { 0 } | if a_held || a_pressed { A } else { 0 };
            self.grammar.push(tick, stick.0, stick.1, buttons);
        }
        let z_pressed = !std::mem::replace(&mut self.z_held, z) && z;
        // the sword's move that was playing ended: the pause strings' beat counts from there
        let sword_now = self.script.active().filter(|&id| crate::script::is_sword(id) && crate::script::is_script(a));
        if let Some(was) = self.sword_was.filter(|&w| Some(w) != sword_now) {
            self.sword_ended = Some((was, tick));
        }
        self.sword_was = sword_now;
        // (any scripted move playing, the sword's RB ones too: when it began)
        let playing_now = self.script.active().filter(|_| crate::script::is_script(a));
        let play_now = playing_now.map(|_| self.script.starts());
        if play_now != self.play_was {
            self.play_was = play_now;
            self.play_since = tick;
        }
        if a & ACT_FLAG_AIR == 0 {
            self.pogos = POGOS;
            self.raves = 0;
        }
        let sword = sword_in_hand && !crate::mega::active();
        // (a move asked for from elsewhere: first, a few ticks at most, waiting while moves aren't
        // allowed)
        if let Some((r, at)) = self.queued.take() {
            if tick.wrapping_sub(at) <= 3 {
                if allowed {
                    return Some(r);
                }
                self.queued = Some((r, at));
            }
        }
        // (the held state is kept on every tick: behind `b &&` it was only on presses, so it stayed
        // "held" for good after the first one)
        let b_pressed = !std::mem::replace(&mut self.b_held, b) && b;
        let y_pressed = !std::mem::replace(&mut self.y_held, y) && y;
        let x_pressed = !std::mem::replace(&mut self.x_held, x) && x;
        // (Y is Mario's alone: the game's interact moved to D-pad down, lib.rs; no waiting for it)
        let y_pressed = y_pressed && !interacting;
        // Y in a hammer swing's wind-up waits for it to land (the swing can't be cut short), then
        // goes on as the combo's Y
        let y_pressed = if y_pressed && locked && a == ACT_ER_HAMMER {
            self.y_buffer = Some(tick);
            false
        } else {
            y_pressed || (!locked && a == ACT_ER_HAMMER && self.y_buffer.take().is_some_and(|t| tick.wrapping_sub(t) <= Y_BUFFER))
        };
        if a != ACT_ER_HAMMER {
            self.y_buffer = None;
        }
        if y_pressed {
            self.x_buffer = None;
        }
        // X after Y: kept for Y, X (and away from SM64 while Y still waits)
        if x_pressed && allowed && (self.in_sweep || a == ACT_ER_HAMMER_SLAM) {
            self.x_buffer = Some(tick);
        }
        let punching = a == ACT_PUNCHING || a == ACT_MOVE_PUNCHING;
        let air = a & ACT_FLAG_AIR != 0;
        // on his feet and free to move (not knocked down or back, in a cutscene, in water, on a
        // ledge), or in the dash
        let on_foot = allowed
            && a & ACT_GROUP_MASK != ACT_GROUP_CUTSCENE
            && ((a & (ACT_FLAG_INVULNERABLE | ACT_NO_MOVES) == 0 && a & (ACT_FLAG_STATIONARY | ACT_FLAG_MOVING) != 0) || dodging(a));
        let closer = x_pressed && self.grammar.back_forward(crate::combo::X);
        // the Dodge Offset (Bayonetta's): X held through a dash, the string's step is kept for the
        // next X (not for a press that's another move's: the closer, X + A)
        self.offset.tick(x, dodging(a), tick);
        let offset_x = x_pressed && !closer && !a_pressed && on_foot && !dodging(a) && !air;
        if let Some(step) = self.offset.take(offset_x, tick).filter(|_| self.weapon == crate::weapons::FISTS && !crate::mega::active()) {
            if let Some(r) = fists_step(step + 1) {
                self.x_chain = step + 1;
                self.chain_seen = Some(tick);
                self.attack_seen = Some(tick);
                crate::log(format!("moves: Dodge Offset, the string goes on at step {}", step + 1));
                return Some(r);
            }
        }
        // a scripted move (script.rs) playing: only what its cancel windows open now cuts it short
        // (sooner once one of its blows landed: no dead time after a hit)
        let script_on = crate::script::is_script(a);
        let cancel = if script_on { self.script.cancel() } else { crate::script::ALL };
        let open = |bit: u8| cancel & bit != 0;
        // (X, Y or B too soon in a move: kept a moment for when it can be cut; X as pressed too, for
        // the sword's mashing)
        let x_raw = x_pressed;
        let (x_pressed, b_pressed, y_pressed) = if script_on {
            use crate::script::{DASH, NEXT};
            if x_pressed && !open(NEXT) {
                self.script_buffer = Some((crate::combo::X, tick));
            } else if y_pressed && !open(NEXT) {
                self.script_buffer = Some((crate::combo::Y, tick));
            } else if b_pressed && !open(DASH) {
                self.script_buffer = Some((crate::combo::B, tick));
            }
            let held = self.script_buffer.filter(|(_, at)| tick.wrapping_sub(*at) <= SCRIPT_BUFFER);
            let take = |bit: u8, need: u8| held.is_some_and(|(b, _)| b == bit) && open(need);
            let (bx, by, bb) = (take(crate::combo::X, NEXT), take(crate::combo::Y, NEXT), take(crate::combo::B, DASH));
            if bx || by || bb {
                self.script_buffer = None;
            }
            (x_pressed && open(NEXT) || bx, b_pressed && open(DASH) || bb, y_pressed && open(NEXT) || by)
        } else {
            self.script_buffer = None;
            (x_pressed, b_pressed, y_pressed)
        };
        if script_on {
            if a_pressed && open(crate::script::JUMP) && !air && allowed {
                crate::log("moves: jump out of the move");
                return Some(Request::Action(ACT_JUMP, 0));
            }
            // the dash out of it (its own: the rest of the moves wait for NEXT)
            if b_pressed && tick >= self.dash_ready && !locked && !crate::mega::active() && allowed {
                self.dash_ready = tick + DASH_COOLDOWN;
                if air {
                    if self.air_dashed {
                        return None;
                    }
                    self.air_dashed = true;
                    self.air_dashes += 1;
                    crate::log("moves: air dash out of the move");
                    return Some(Request::Action(ACT_ER_AIR_DASH, 0));
                }
                self.offset.dash_started(x, self.x_chain, tick);
                crate::log("moves: dash out of the move");
                return Some(Request::Action(ACT_ER_DASH, 0));
            }
            if x_pressed && open(crate::script::NEXT) && allowed && !closer && !sword {
                if let Some(r) = self.string_opener(air) {
                    self.x_chain = self.x_chain.max(1) + 1;
                    self.chain_seen = Some(tick);
                    self.attack_seen = Some(tick);
                    crate::log("moves: the string goes on out of the move");
                    return Some(r);
                }
            }
            // (a swap in its window: the swap strike below; the sword reads its own moves' windows:
            // a charge let go, mashing in the thrust)
            if !open(crate::script::NEXT) && !(self.swap.is_some() && open(crate::script::SWAP)) && !sword {
                return None;
            }
        }
        // back, forward + X with the fists: the Bullet Bill punch (Devil May Cry's Stinger), the
        // gap closer (a string's first blow: X then goes on with it); out of the dash too
        let closer_from = !combo(a) || punching || matches!(a, ACT_ER_CHAIN | ACT_ER_RUSH) || script_on;
        if closer && on_foot && !air && !locked && closer_from && self.weapon == crate::weapons::FISTS && !crate::mega::active() {
            self.x_chain = 1;
            self.chain_seen = Some(tick);
            self.attack_seen = Some(tick);
            crate::log("moves: Bullet Bill punch (back, forward + X)");
            let id = crate::script::BULLET_PUNCH;
            return Some(Request::Script { id, air: crate::script::MOVES[id as usize].air, yaw: None });
        }
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
        if !air {
            self.air_swaps = 0;
        }
        // X + A together: the weapon's rising blow (God of War's, Devil May Cry's launcher with a
        // jump), from standing, a string or the jump A just began (SM64 took the A first); not in
        // a hammer blow's wind-up (it can't be left), not for Mega Mario (SM64's jump kick)
        if a_pressed {
            self.a_at = Some(tick);
        }
        if x_pressed {
            self.x_at = Some(tick);
        }
        if let (Some(at_a), Some(at_x)) = (self.a_at, self.x_at) {
            let together = at_a.max(at_x) == tick && at_a.abs_diff(at_x) <= CHORD;
            let jump_begun = in_air && a == ACT_JUMP && tick.wrapping_sub(at_a) <= CHORD;
            // (X first: the swing it began a tick or two ago gives way)
            let just_began = move_timer <= CHORD as i32 + 1 && tick.wrapping_sub(at_x) <= CHORD;
            if together && (ground || jump_begun) && (!locked || just_began) && !crate::mega::active() && !sword {
                self.a_at = None;
                self.x_at = None;
                self.x_chain = 0;
                crate::log("moves: rising blow (X + A)");
                return Some(Request::Action(ACT_ER_RISE, self.weapon as u32));
            }
        }
        if self.attack_seen.is_none_or(|t| tick.wrapping_sub(t) > STRING_OVER) {
            self.string_swaps = 0;
        }
        // a blow going on (the combos' moves, SM64's punches and kicks, the air string)
        if combo(a) || script_on || punching || a == ACT_ER_RUSH || a == ACT_GROUND_POUND || a == ACT_SLIDE_KICK || a == ACT_TWIRLING || a == 0x0180_08A9 || a == 0x0180_08AE {
            self.attack_seen = Some(tick);
        }
        // the swap strike: the weapon just taken opens with its own blow, the string going on
        if let Some((w, at)) = self.swap.take().filter(|(_, at)| tick.wrapping_sub(*at) <= 3) {
            use crate::weapons::{CHOMP, FISTS, HAMMER, RANG, SWORD};
            let rangs = crate::boomerang::ready() > 0;
            let mega = crate::mega::active();
            let air_left = self.air_swaps < if crate::juggle::active() { AIR_SWAPS_JUGGLE } else { AIR_SWAPS };
            // the Star Sword's: the quick draw (Mortal Draw), an iai slash in front, a spin slash in
            // the air
            if w == SWORD && (ground || in_air && air_left) {
                let id = if ground { crate::script::SW_DRAW } else { crate::script::SW_DRAW_AIR };
                if let Some(m) = crate::script::MOVES.get(id as usize) {
                    self.swap_ready = tick + SWAP_COOLDOWN;
                    self.string_swaps += 1;
                    if air {
                        self.air_swaps += 1;
                    }
                    self.x_chain = 1;
                    self.chain_seen = Some(tick);
                    crate::style::swap();
                    crate::log(format!("moves: swap strike, {}", m.name));
                    return Some(Request::Script { id, air: m.air, yaw: None });
                }
            }
            let strike = if ground {
                match w {
                    HAMMER => Some((ACT_ER_HAMMER, 2, "hammer smash")),
                    CHOMP => Some((ACT_ER_CHOMP, 1, "Chomp sweep")),
                    RANG if rangs => Some((ACT_ER_THROW, THROW_STORM, "boomerang fan")),
                    FISTS if !mega => Some((ACT_ER_RUSH, 0, "rush")),
                    _ => None,
                }
            } else if in_air && air_left {
                match w {
                    HAMMER => Some((ACT_ER_HAMMER_AIR, 0, "hammer whirl")),
                    CHOMP => Some((ACT_ER_CHOMP_AIR, 0, "Chomp swing")),
                    RANG if rangs => Some((ACT_ER_AIR_THROW, THROW_STORM, "boomerang fan")),
                    FISTS if !mega => Some((ACT_ER_AIR_COMBO, 0, "air string")),
                    _ => None,
                }
            } else {
                None
            };
            if strike.is_none() && WEAPON_ACTIONS.contains(&a) {
                // (it couldn't start after all: the old weapon's move ends, it isn't in hand)
                let _ = at;
                return Some(Request::Action(if air { ACT_FREEFALL } else { ACT_IDLE }, 0));
            }
            if let Some((action, arg, name)) = strike {
                self.swap_ready = tick + SWAP_COOLDOWN;
                self.string_swaps += 1;
                if air {
                    self.air_swaps += 1;
                }
                // (the string goes on: its next X is the new weapon's next blow)
                self.x_chain = 1;
                self.chain_seen = Some(tick);
                crate::style::swap();
                crate::log(format!("moves: swap strike, {name}"));
                return Some(Request::Action(action, arg));
            }
        }
        // the freeflow (Arkham's): B toward another enemy in the middle of a string, a leap to it, the
        // string going on (its blows keep counting, lib.rs found the one the stick points at)
        let in_string = self.attack_seen.is_some_and(|t| tick.wrapping_sub(t) <= FLOW_AFTER);
        if b_pressed && in_string && !locked && !crate::mega::active() && a & (ACT_FLAG_INVULNERABLE | ACT_NO_MOVES) == 0 {
            if let Some(to) = flow {
                self.dash_ready = tick + DASH_COOLDOWN;
                self.chain_seen = Some(tick);
                crate::style::flow();
                crate::log("moves: freeflow (B to another enemy)");
                return Some(Request::Flow(to));
            }
        }
        // (no dashes for Mega Mario: too fast for a giant's collision, mario_step.c)
        // (not in a hammer blow's wind-up and impact: it's committed)
        if b_pressed && tick >= self.dash_ready && !locked && !crate::mega::active() {
            let step = if in_string { self.x_chain } else { 0 };
            if ground {
                self.dash_ready = tick + DASH_COOLDOWN;
                self.offset.dash_started(x, step, tick);
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
        // the Star Sword in hand: X, Y and LT in the air are its moves alone (swordplay.rs)
        if sword {
            // (not out of er_moves.c's own moves: the hook's home run and pole swing, the Mikiri;
            // the swing kick off the pole takes an air blow as it does with the other weapons)
            let c_move = combo(a) && !(a == ACT_ER_SWING_KICK && move_timer >= 8);
            if !(allowed && (ground || in_air || dodging(a))) || c_move {
                return None;
            }
            use crate::combo::{A, X, Y};
            // (A then X a tick or two apart: the jump A began is the spin jump's, not an aerial)
            let jump_begun = a == ACT_JUMP && self.a_at.is_some_and(|t| tick.wrapping_sub(t) <= CHORD);
            // (the playing move's own presses: the stabs count the mashing since the thrust began)
            let playing_for = tick.wrapping_sub(self.play_since);
            let mash_window = if playing_now.is_some() { crate::swordplay::MASH_WINDOW.min(playing_for) } else { crate::swordplay::MASH_WINDOW };
            let input = crate::swordplay::Input {
                // (as pressed too: mashing in the thrust while it can't be cut yet)
                x: x_pressed || x_raw,
                y: y_pressed,
                lt: z_pressed,
                x_held: self.grammar.held(X),
                y_held: self.grammar.held(Y),
                x_released: self.grammar.released(X),
                y_released: self.grammar.released(Y),
                back_forward: closer,
                chord_xa: self.grammar.chord(X, A, CHORD) && (x_pressed || a_pressed),
                dir: Some(self.grammar.dir()),
                mash: self.grammar.mash(X, mash_window),
            };
            let sword_state = crate::swordplay::State {
                // (only the chord's jump: A then Y or LT is an air move, he's up already)
                air: air && !(jump_begun && input.chord_xa),
                // (the sword's RB moves too: X before the bolt is let go would lose the cast)
                playing: playing_now.map(|id| (id, open(crate::script::NEXT))),
                playing_for,
                landed: self.script.has_landed(),
                ended: self.sword_ended.map(|(id, at)| (id, tick.wrapping_sub(at))),
                dashed: if dodging(a) { Some(0) } else { self.dash_seen.map(|t| tick.wrapping_sub(t)) },
                dashing: dodging(a),
                // (a perfect dodge's counter gives one Flurry Rush)
                countering: crate::dodge::flurry_ready(),
                pogos: self.pogos,
            };
            let id = crate::swordplay::choose(&input, &sword_state)?;
            let Some(m) = crate::script::MOVES.get(id as usize) else {
                if !std::mem::replace(&mut self.sword_missing, true) {
                    crate::log(format!("moves: the sword's move {id} has no data"));
                }
                return None;
            };
            if id == crate::script::SW_POGO {
                self.pogos = self.pogos.saturating_sub(1);
            }
            if id == crate::script::SW_FLURRY {
                crate::dodge::spend_flurry();
            }
            if id == crate::script::SW_RAVE {
                let most = if crate::juggle::active() { RAVES_JUGGLE } else { RAVES };
                if self.raves >= most {
                    return None;
                }
                self.raves += 1;
            }
            self.script_buffer = None;
            self.attack_seen = Some(tick);
            self.chain_seen = Some(tick);
            crate::log(format!("moves: sword, {}", m.name));
            return Some(Request::Script { id, air: m.air, yaw: None });
        }
        // the directional moves (Devil May Cry's): back + X the launcher, whatever's in hand; forward
        // + Y the leap at an enemy into the weapon's dive (running, the fists keep the slide kick)
        {
            use crate::weapons::{CHOMP, FISTS, HAMMER, RANG};
            let slow = state.forward_velocity < LEAN_SLOW;
            let stringy = !combo(a) || matches!(a, ACT_ER_HAMMER | ACT_ER_CHOMP | ACT_ER_THROW | ACT_ER_CHAIN) || punching;
            if x_pressed && ground && lean < -LEAN && slow && stringy && !locked && !crate::mega::active() {
                let launcher = match self.weapon {
                    FISTS => Some((ACT_ER_UPPERCUT, 0, "super jump punch")),
                    HAMMER => Some((ACT_ER_HAMMER_LAUNCH, 0, "hammer launcher")),
                    CHOMP => Some((ACT_ER_CHOMP, 3, "Chomp bite")),
                    RANG if crate::boomerang::ready() > 0 => Some((ACT_ER_THROW, THROW_UPPER, "boomerang launcher")),
                    _ => None,
                };
                if let Some((action, arg, name)) = launcher {
                    self.x_chain = 0;
                    crate::log(format!("moves: back + X, the {name}"));
                    return Some(Request::Action(action, arg));
                }
            }
            if y_pressed && ground && lean > LEAN && state.forward_velocity < RUN_SPEED && !combo(a) && !punching && !crate::mega::active() {
                let arg = match self.weapon {
                    HAMMER => 1,
                    RANG => 2,
                    CHOMP => 3,
                    _ => 0,
                };
                self.x_chain = 0;
                crate::log("moves: forward + Y, the leap");
                return Some(Request::Action(ACT_ER_LEAP, arg));
            }
        }
        // X just after the dash: the dash strike (Hades'; er_moves.c takes an X in its last ticks too)
        if x_pressed && ground && !crate::mega::active() && tick >= self.bonk_ready && self.dash_seen.is_some_and(|t| tick.wrapping_sub(t) <= BONK_AFTER) {
            self.dash_seen = None;
            crate::log("moves: dash bonk (X after the dash)");
            return Some(Request::Action(ACT_ER_BONK, 0));
        }
        if a == ACT_ER_BONK && !std::mem::replace(&mut self.bonk_seen, true) {
            self.bonk_ready = tick + BONK_EVERY;
            crate::log("moves: dash bonk (X at the dash's end)");
        } else if a != ACT_ER_BONK {
            self.bonk_seen = false;
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
