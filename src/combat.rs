//! Mario's attacks hurt Elden Ring characters, and their hits knock Mario back.
//!
//! Each SM64 tick, living characters near Mario are tested with libsm64's own interaction check
//! (`sm64_mario_attack`: punch/kick cones, sweep kick, dive, slide kick, ground pound, stomping from
//! above, with SM64's bounce and hit sound). A hit takes a fixed share of the target's max HP (much
//! less on bosses), the same at every level, so levelling doesn't make Mario stronger; then a small
//! game bullet on the target brings the stagger, blood, hit sounds and the final blow (so deaths,
//! rune drops and kill credit stay the game's own). Bullets 65-70 (unused dev bullets with attack
//! params 994-999) are repurposed at runtime as Mario's attacks.

use std::collections::HashMap;

use eldenring::cs::{
    AtkParam_Pc, Bullet, CSBulletManager, ChrIns, ChrType, FieldInsHandle, SoloParamRepository, WorldChrMan,
};
use eldenring::position::HavokPosition;
use fromsoftware_shared::{F32Vector4, FromStatic};

use crate::{log, sm64};

/// (bullet row, attack row, damage (flat, and % of the weapon's attack rating), poise damage,
/// hit reaction level, hit radius m)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Attack {
    Punch,
    Kick,
    Sweep,
    GroundPound,
    Stomp,
    Dash,
    /// Mario's fireball (fireball.rs), by charge level 0-3
    Fire(u8),
    /// the giant fireball (fireball.rs) sweeping enemies along: a little at a time, then it blows
    FireSweep,
    /// the 20 s fireball's lava embers on the ground (fireball.rs): whoever stands in them burns
    Ember,
    /// Mega Mario's steps: whoever is under his feet
    Trample,
    /// Y in the air: the twirl, hitting all around him (moves.rs)
    Spin,
    /// a ground pound's landing, on everyone around the one under him
    Shockwave,
    /// the pause combo's rush (moves.rs): its two jabs, and its flying kick
    Rush,
    RushKick,
    /// the combos (moves.rs, er_moves.c), bare-handed: the super jump punch on the way up and at
    /// its top, the meteor dive and its landing, the tornado and its last turn
    Uppercut,
    UppercutTop,
    Meteor,
    MeteorLand,
    Tornado,
    TornadoEnd,
    /// with the hammer: its side swings, its overhead smash, the slam (and the plunge's landing),
    /// the swing in the air (and the plunge), the flurry's blows and its giant one, the Bowser spin
    /// and its release
    Hammer,
    HammerSmash,
    Quake,
    HammerAir,
    Flurry,
    FlurryEnd,
    Giro,
    GiroEnd,
    /// the bare-handed string past the kick (and its whirlwind kick), the Koopa shell, the air string
    Chain,
    ChainEnd,
    Shell,
    AirHit,
    /// the hammer's launcher (X, Y): whoever it catches flies high up (juggle.rs)
    Launch,
    /// the boomerang (boomerang.rs): a pass of one (out or back), the big one's blows as it hovers,
    /// the launcher's (it lifts them)
    Rang,
    RangBig,
    RangUpper,
    /// the Chain Chomp (chomp.rs, er_moves.c): a lash or sweep, the cyclone's blows, a crash's
    /// shockwave, a swing in the air, the bite that tosses them up
    Chomp,
    ChompSpin,
    ChompEnd,
    ChompAir,
    ChompLaunch,
    /// the hook (hook.rs): a tug, barely a scratch
    Hook,
    /// on Yoshi (ride.rs): his tongue, an egg (the stomp is a Shockwave)
    Tongue,
    Egg,
    YoshiStomp,
    /// the Mikiri stomp on an attacker's head (timing.rs), the dash strike (X at the dash's end)
    Mikiri,
    Bonk,
    /// the trap kit (traps.rs): an enemy down on the banana, a shell's hit, the Bob-omb's blast,
    /// the spring sending one up
    Slip,
    ShellHit,
    Bomb,
    Boing,
    /// the Rock Mushroom (rock.rs): the boulder rolling into them, its slam after a drop
    Rock,
    RockSlam,
    /// the hook's moves (hook.rs, bat.rs): the home run's swing, an enemy bowled over by the one
    /// batted away, the tether's crash, the kick off the pole swing
    HomeRun,
    Bowled,
    Tether,
    SwingKick,
    /// MAX finishers (maxfin.rs): the fists' phantom blows and their last, a step of the hammer's
    /// cross, the Chomp's ring of fire
    MaxFlurry,
    MaxUpper,
    MaxQuake,
    MaxCyclone,
    /// the boss ride's crash into a wall (bossride.rs)
    RideCrash,
    /// the captured enemy's own blows on the others (bossride.rs): light, heavy
    RideBlow,
    RideSmash,
    /// co-op: the Bros. Attack's ground pound on the enemy both brothers marked (brosattack.rs)
    BrosAttack,
    /// on a colossus (colossus.rs): the star strike on a weak point, a blow anywhere else on it
    Sigil,
    SigilTap,
    /// the hammer's (er_moves.c): King K. Rool's backhand, the charged quakes (a level up, the
    /// titan's full one), Donkey Kong's drum's pounds
    HammerWide,
    QuakeBig,
    Titan,
    Pound,
    /// X + A: the rising blow (the fists' spin kick, the hammer's upswing)
    Rise,
    HammerRise,
    /// RB with the other weapons (fireball.rs, by charge level 0-3, element.rs their elements):
    /// the hammer's ice block, the Chain Chomp's dark orb, the boomerang's poison glob, and the
    /// cloud of poison the glob leaves where it bursts
    Ice(u8),
    Dark(u8),
    Venom(u8),
    VenomCloud,
    /// a status going off on an enemy (element.rs): frostbite's burst, blood loss, a tick of poison
    Frostbite,
    Hemorrhage,
    PoisonTick,
}

/// Every attack but the fireball's levels, in a fixed order: their codes on the wire (coop.rs).
/// Only ever add at the end (both players need the same build anyway).
const CODES: [Attack; 88] = [
    Attack::Punch, Attack::Kick, Attack::Sweep, Attack::GroundPound, Attack::Stomp, Attack::Dash,
    Attack::FireSweep, Attack::Ember, Attack::Trample, Attack::Spin, Attack::Shockwave, Attack::Rush,
    Attack::RushKick, Attack::Uppercut, Attack::UppercutTop, Attack::Meteor, Attack::MeteorLand,
    Attack::Tornado, Attack::TornadoEnd, Attack::Hammer, Attack::HammerSmash, Attack::Quake,
    Attack::HammerAir, Attack::Flurry, Attack::FlurryEnd, Attack::Giro, Attack::GiroEnd, Attack::Chain,
    Attack::ChainEnd, Attack::Shell, Attack::AirHit, Attack::Launch, Attack::Rang, Attack::RangBig,
    Attack::RangUpper, Attack::Chomp, Attack::ChompSpin, Attack::ChompEnd, Attack::ChompAir,
    Attack::ChompLaunch, Attack::Hook, Attack::Tongue, Attack::Egg, Attack::YoshiStomp, Attack::Mikiri, Attack::Bonk,
    Attack::Slip, Attack::ShellHit, Attack::Bomb, Attack::Boing, Attack::Rock, Attack::RockSlam,
    Attack::HomeRun, Attack::Bowled, Attack::Tether, Attack::SwingKick, Attack::MaxFlurry, Attack::MaxUpper,
    Attack::MaxQuake, Attack::MaxCyclone, Attack::RideCrash, Attack::BrosAttack, Attack::Sigil, Attack::SigilTap,
    Attack::RideBlow, Attack::RideSmash, Attack::HammerWide, Attack::QuakeBig, Attack::Titan, Attack::Pound, Attack::Rise,
    Attack::HammerRise, Attack::Ice(0), Attack::Ice(1), Attack::Ice(2), Attack::Ice(3), Attack::Dark(0), Attack::Dark(1),
    Attack::Dark(2), Attack::Dark(3), Attack::Venom(0), Attack::Venom(1), Attack::Venom(2), Attack::Venom(3), Attack::VenomCloud,
    Attack::Frostbite, Attack::Hemorrhage, Attack::PoisonTick,
];
/// the fireball's levels: this plus the level
const FIRE_CODE: u8 = 200;
const FIRE_MAX: u8 = FIRE_CODE + 5;
/// not an attack: an impact's flat share (`impact`), the share as the hit's scale
pub const IMPACT_CODE: u8 = 199;

impl Attack {
    pub fn code(self) -> u8 {
        match self {
            Attack::Fire(level) => FIRE_CODE.saturating_add(level),
            a => CODES.iter().position(|&c| c == a).map_or(u8::MAX, |i| i as u8),
        }
    }

    pub fn from_code(code: u8) -> Option<Attack> {
        match code {
            FIRE_CODE..=FIRE_MAX => Some(Attack::Fire(code - FIRE_CODE)),
            _ => CODES.get(code as usize).copied(),
        }
    }

    fn spec(self) -> (i32, u32, u16, f32, u8, f32) {
        match self {
            // damage: % of the (fist) weapon's attack rating, flat value too
            // poise: a boss with up to 100 poise takes 5 ground pounds
            Attack::Punch => (65, 994, 150, 9.0, 1, 0.7),
            Attack::Kick => (66, 995, 200, 11.0, 1, 0.7),
            Attack::Sweep => (67, 996, 150, 14.0, 2, 0.9),
            Attack::GroundPound => (68, 997, 350, 20.0, 3, 1.5),
            Attack::Stomp => (69, 998, 250, 14.0, 2, 0.8),
            Attack::Dash => (70, 999, 175, 14.0, 2, 0.8),
            // (no bullets of its own: the blow of the move that staggers about as hard)
            Attack::Fire(0 | 1) => Attack::Kick.spec(),
            Attack::Fire(2) => Attack::Stomp.spec(),
            Attack::Fire(_) => Attack::GroundPound.spec(),
            Attack::FireSweep | Attack::Ember | Attack::Trample => Attack::Punch.spec(),
            Attack::Spin => Attack::Sweep.spec(),
            Attack::Shockwave => Attack::Stomp.spec(),
            Attack::Rush => Attack::Punch.spec(),
            Attack::RushKick => Attack::GroundPound.spec(),
            Attack::Uppercut | Attack::Tornado | Attack::Flurry | Attack::AirHit => Attack::Punch.spec(),
            Attack::Chain | Attack::Shell => Attack::Kick.spec(),
            Attack::ChainEnd | Attack::Launch => Attack::Stomp.spec(),
            Attack::Meteor => Attack::Stomp.spec(),
            Attack::Giro | Attack::TornadoEnd => Attack::Sweep.spec(),
            // (the hammer's every blow staggers like a stomp at least, its heavy ones like a ground
            // pound)
            Attack::Hammer | Attack::HammerWide | Attack::HammerAir | Attack::Pound | Attack::HammerRise | Attack::Rise => Attack::Stomp.spec(),
            Attack::UppercutTop | Attack::MeteorLand | Attack::HammerSmash | Attack::Quake | Attack::QuakeBig | Attack::Titan | Attack::FlurryEnd | Attack::GiroEnd => {
                Attack::GroundPound.spec()
            }
            Attack::Rang | Attack::RangBig | Attack::ChompSpin | Attack::Hook | Attack::Tongue => Attack::Punch.spec(),
            Attack::Egg => Attack::Kick.spec(),
            Attack::YoshiStomp => Attack::Shockwave.spec(),
            Attack::Chomp | Attack::ChompAir => Attack::Kick.spec(),
            Attack::RangUpper | Attack::ChompLaunch => Attack::Stomp.spec(),
            Attack::ChompEnd => Attack::GroundPound.spec(),
            // (the Mikiri staggers like a ground pound: it's what it's for)
            Attack::Mikiri => Attack::GroundPound.spec(),
            Attack::Bonk => Attack::Stomp.spec(),
            // (the banana and the shells on the sweep's bullet: the one knockdown() turns into a
            // knockdown for a moment)
            Attack::Slip | Attack::ShellHit => Attack::Sweep.spec(),
            Attack::Bomb => Attack::GroundPound.spec(),
            // (not the punch's row: the finisher turns that one up for a moment)
            Attack::Boing => Attack::Kick.spec(),
            Attack::Rock => Attack::Sweep.spec(),
            Attack::RockSlam => Attack::GroundPound.spec(),
            Attack::HomeRun | Attack::SwingKick => Attack::Stomp.spec(),
            Attack::Bowled | Attack::RideBlow => Attack::Sweep.spec(),
            Attack::RideSmash => Attack::GroundPound.spec(),
            Attack::Tether => Attack::GroundPound.spec(),
            Attack::MaxFlurry => Attack::Punch.spec(),
            Attack::MaxUpper | Attack::MaxQuake | Attack::MaxCyclone | Attack::RideCrash | Attack::BrosAttack | Attack::Sigil => Attack::GroundPound.spec(),
            Attack::SigilTap => Attack::Punch.spec(),
            Attack::Ice(level) | Attack::Dark(level) | Attack::Venom(level) => Attack::Fire(level.min(3)).spec(),
            Attack::VenomCloud | Attack::PoisonTick => Attack::Punch.spec(),
            Attack::Frostbite | Attack::Hemorrhage => Attack::Stomp.spec(),
        }
    }

    /// Share of a normal enemy's max HP per hit (%), and its er_mario.ini key.
    fn percent(self) -> (f32, &'static str) {
        // (all of them down to 60% once Mario had his combos, his dash and his speed: the giant
        // fireball, 10 s of charging, only a little; then the fists, kicks and hammer, X and Y, down to half
        // again: the stomps, the ground pound, the dash and the fireballs kept theirs)
        match self {
            // (a regular enemy in eight blows)
            Attack::Punch => (13.5, "damage_punch"),
            Attack::Kick => (13.5, "damage_kick"),
            Attack::Sweep => (13.5, "damage_sweep"),
            Attack::Dash => (15.0, "damage_dive"),
            Attack::Stomp => (27.0, "damage_stomp"),
            // slow and leaves Mario open: it has to be worth it (a regular enemy in three)
            Attack::GroundPound => (42.0, "damage_ground_pound"),
            // a regular enemy in nine plain shots, seven, five, four fully charged ones
            Attack::Fire(0) => (12.0, "damage_fire"),
            Attack::Fire(1) => (16.0, "damage_fire_charged"),
            Attack::Fire(2) => (22.0, "damage_fire_charged_2"),
            Attack::Fire(3) => (33.0, "damage_fire_max"),
            // the giant one (10 s): kills a regular enemy outright
            Attack::Fire(4) => (85.0, "damage_fire_giant"),
            // the sun (20 s): a little more, the most is in its burn, its blast and its embers
            Attack::Fire(_) => (100.0, "damage_fire_sun"),
            // (the giant one's sweep: a tick every few tenths while it carries them or sticks to a
            // boss, the explosion is the finisher)
            Attack::FireSweep => (8.0, "damage_fire_sweep"),
            Attack::Ember => (5.0, "damage_fire_ember"),
            Attack::Trample => (4.0, "damage_mega_step"),
            Attack::Spin => (7.5, "damage_spin"),
            Attack::Shockwave => (21.0, "damage_shockwave"),
            // the reward for the beat: harder blows than the plain combo's
            Attack::Rush => (13.5, "damage_rush"),
            Attack::RushKick => (22.5, "damage_rush_kick"),
            // the combos: their finishers hit hardest, worth the longer string of buttons (the
            // super jump punch's many blows on the way up are mostly for show: it set up an air
            // combo and took a regular enemy's whole bar on its own)
            Attack::Uppercut => (1.5, "damage_uppercut"),
            Attack::UppercutTop => (8.0, "damage_uppercut_top"),
            Attack::Meteor => (10.5, "damage_meteor"),
            Attack::MeteorLand => (25.5, "damage_meteor_land"),
            Attack::Tornado => (5.5, "damage_tornado"),
            Attack::TornadoEnd => (18.0, "damage_tornado_end"),
            // the hammer: the heavy one, few blows and slow, each worth two of the fists' or more
            // (the fists' finishers' worth at the least); the Chain Chomp's least of all, many at
            // once
            Attack::Hammer => (30.0, "damage_hammer"),
            Attack::HammerWide => (33.0, "damage_hammer_backhand"),
            Attack::HammerSmash => (47.0, "damage_hammer_smash"),
            Attack::Quake => (37.0, "damage_hammer_slam"),
            Attack::QuakeBig => (53.0, "damage_hammer_slam_charged"),
            Attack::Titan => (75.0, "damage_hammer_titan"),
            Attack::HammerAir => (23.0, "damage_hammer_air"),
            Attack::HammerRise => (28.0, "damage_hammer_rise"),
            Attack::Pound => (17.0, "damage_hammer_pound"),
            Attack::Flurry => (6.0, "damage_hammer_flurry"),
            Attack::FlurryEnd => (58.0, "damage_hammer_giant"),
            Attack::Giro => (13.0, "damage_hammer_spin"),
            Attack::GiroEnd => (33.0, "damage_hammer_spin_end"),
            // the fists' rising spin kick: a punch and a bit, it's the air string's start
            Attack::Rise => (16.0, "damage_rise"),
            Attack::Chain => (12.0, "damage_string"),
            Attack::ChainEnd => (24.0, "damage_string_end"),
            Attack::Shell => (10.5, "damage_shell"),
            Attack::AirHit => (9.0, "damage_air"),
            // (it sets up the air string: the blows up there are the damage)
            Attack::Launch => (20.0, "damage_hammer_launch"),
            // the boomerang: from afar and safe, a little each pass (it hits going and coming back)
            Attack::Rang => (3.0, "damage_boomerang"),
            Attack::RangBig => (3.0, "damage_boomerang_big"),
            Attack::RangUpper => (5.0, "damage_boomerang_launcher"),
            // the Chain Chomp: wide, many at once, a little each (the cyclone least of all)
            Attack::Chomp => (7.0, "damage_chomp"),
            Attack::ChompSpin => (3.0, "damage_chomp_cyclone"),
            Attack::ChompEnd => (14.0, "damage_chomp_crash"),
            Attack::ChompAir => (6.0, "damage_chomp_air"),
            Attack::ChompLaunch => (6.0, "damage_chomp_bite"),
            // the hook: it brings them in (or Mario to them), it doesn't hurt
            Attack::Hook => (0.5, "damage_hook"),
            // Yoshi: a lick (it makes him an egg), an egg
            Attack::Tongue => (5.0, "damage_yoshi_tongue"),
            Attack::Egg => (8.0, "damage_yoshi_egg"),
            Attack::YoshiStomp => (15.0, "damage_yoshi_stomp"),
            // timing: the Mikiri's worth is its stagger, the stance it cracks and the air string it
            // opens, not its own damage; the dash strike a little over the dash's own
            Attack::Mikiri => (12.0, "damage_mikiri"),
            Attack::Bonk => (16.0, "damage_dash_bonk"),
            // the trap kit: the worth of a banana or a spring is what they set up (an enemy on the
            // ground, up in the air), the shell's in its bounces, the Bob-omb's in its blast
            Attack::Slip => (8.0, "damage_banana"),
            Attack::ShellHit => (10.0, "damage_shell_hit"),
            Attack::Bomb => (28.0, "damage_bobomb"),
            Attack::Boing => (3.0, "damage_spring"),
            // the boulder: bowling them over is the point (a regular enemy in about ten), its slam
            Attack::Rock => (9.0, "damage_rock"),
            Attack::RockSlam => (16.0, "damage_rock_slam"),
            // the hook's moves: setting things up more than hurting (the flight and the crash are
            // the show), the swing kick a strong blow for the risk of the swing
            Attack::HomeRun => (14.0, "damage_home_run"),
            Attack::Bowled => (8.0, "damage_bowled"),
            Attack::Tether => (12.0, "damage_tether"),
            Attack::SwingKick => (18.0, "damage_swing_kick"),
            // MAX: a finisher's worth again, spread over its show
            Attack::MaxFlurry => (3.0, "damage_max_flurry"),
            Attack::MaxUpper => (10.0, "damage_max_upper"),
            Attack::MaxQuake => (9.0, "damage_max_quake"),
            Attack::MaxCyclone => (14.0, "damage_max_cyclone"),
            // the boss ride's crash: its worth is the open stance it leaves
            Attack::RideCrash => (15.0, "damage_ride_crash"),
            // its own blows on the others: a strong enemy's, more than Mario's fists
            Attack::RideBlow => (7.0, "damage_ride_blow"),
            Attack::RideSmash => (12.0, "damage_ride_smash"),
            // the Bros. Attack: a team finisher's worth, once in a while (a streak of BROS! and a
            // wait after it)
            Attack::BrosAttack => (30.0, "damage_bros_attack"),
            // the colossus: a weak point's star strike is worth the climb and the charge (a few a
            // climb: its grip runs out); a blow anywhere else on it is a scratch
            Attack::Sigil => (12.0, "damage_star_strike"),
            Attack::SigilTap => (2.0, "damage_colossus_blow"),
            // the other weapons' shots: the ice block slow and heavy (a lob), the dark orb quick and
            // light (its worth is the blood loss it builds), the poison glob least (its cloud and the
            // poison are the rest)
            Attack::Ice(0) => (14.0, "damage_ice"),
            Attack::Ice(1) => (19.0, "damage_ice_charged"),
            Attack::Ice(2) => (26.0, "damage_ice_charged_2"),
            Attack::Ice(_) => (36.0, "damage_ice_max"),
            Attack::Dark(0) => (10.0, "damage_dark"),
            Attack::Dark(1) => (14.0, "damage_dark_charged"),
            Attack::Dark(2) => (19.0, "damage_dark_charged_2"),
            Attack::Dark(_) => (27.0, "damage_dark_max"),
            Attack::Venom(0) => (8.0, "damage_venom"),
            Attack::Venom(1) => (11.0, "damage_venom_charged"),
            Attack::Venom(2) => (15.0, "damage_venom_charged_2"),
            Attack::Venom(_) => (21.0, "damage_venom_max"),
            Attack::VenomCloud => (1.5, "damage_venom_cloud"),
            // the statuses (element.rs): a burst worth a heavy blow, poison a little a second for
            // its 20 s (a fifth of a regular enemy's health in all)
            Attack::Frostbite => (15.0, "damage_frostbite"),
            Attack::Hemorrhage => (20.0, "damage_blood_loss"),
            Attack::PoisonTick => (1.6, "damage_poison"),
        }
    }

    /// A status going off on him (element.rs): no blow of Mario's, none of a blow's bonuses.
    pub fn status(self) -> bool {
        matches!(self, Attack::Frostbite | Attack::Hemorrhage | Attack::PoisonTick)
    }

    /// A shot from afar (a fireball, a boomerang, a Yoshi egg), not a blow up close.
    pub fn ranged(self) -> bool {
        matches!(
            self,
            Attack::Fire(_)
                | Attack::Ice(_)
                | Attack::Dark(_)
                | Attack::Venom(_)
                | Attack::VenomCloud
                | Attack::FireSweep
                | Attack::Ember
                | Attack::Rang
                | Attack::RangBig
                | Attack::RangUpper
                | Attack::Egg
                | Attack::Hook
                | Attack::Tongue
                | Attack::Slip
                | Attack::ShellHit
                | Attack::Bomb
                | Attack::Boing
                | Attack::Bowled
                | Attack::Tether
        )
    }

    /// On bosses the close hits count for more: Mario has to stand right in front of them to land
    /// one. The ground pound most of all, it leaves him open the longest.
    fn boss_bonus(self) -> f32 {
        match self {
            Attack::GroundPound => 2.0,
            Attack::Punch | Attack::Kick | Attack::Sweep => 1.5,
            Attack::Stomp => 1.4,
            Attack::Dash => 1.1,
            // from afar and safe: on bosses only the charged ones count for more
            Attack::Fire(level) => 1.0 + 0.15 * level as f32,
            Attack::FireSweep => 1.6,
            Attack::Ember => 1.2,
            Attack::Trample => 1.2,
            Attack::Spin => 1.2,
            Attack::Shockwave => 1.4,
            Attack::Rush => 1.6,
            Attack::RushKick => 1.9,
            Attack::Uppercut | Attack::Meteor | Attack::Flurry | Attack::HammerAir => 1.3,
            Attack::Tornado | Attack::Giro => 1.2,
            Attack::UppercutTop => 1.7,
            Attack::MeteorLand => 1.8,
            Attack::TornadoEnd => 1.5,
            // (the hammer's shares are big already: a boss loses a few % to its heaviest)
            Attack::Hammer | Attack::HammerWide => 1.4,
            Attack::HammerSmash | Attack::Quake => 1.3,
            Attack::QuakeBig | Attack::FlurryEnd => 1.1,
            Attack::Titan => 1.0,
            Attack::Pound | Attack::HammerRise | Attack::GiroEnd => 1.2,
            Attack::Rise => 1.4,
            Attack::Chain => 1.5,
            Attack::ChainEnd => 1.8,
            Attack::Shell | Attack::AirHit => 1.3,
            Attack::Launch => 1.2,
            Attack::Rang | Attack::RangBig | Attack::RangUpper | Attack::ChompSpin | Attack::Hook | Attack::Tongue | Attack::Egg => 1.0,
            Attack::YoshiStomp => 1.2,
            Attack::Chomp | Attack::ChompAir | Attack::ChompLaunch => 1.2,
            Attack::ChompEnd => 1.5,
            Attack::Mikiri => 1.6,
            Attack::Bonk => 1.3,
            Attack::Slip | Attack::ShellHit | Attack::Boing => 1.0,
            Attack::Bomb => 1.2,
            Attack::Rock => 1.0,
            Attack::RockSlam => 1.2,
            Attack::HomeRun | Attack::Bowled | Attack::Tether => 1.0,
            Attack::SwingKick => 1.3,
            Attack::MaxFlurry | Attack::MaxUpper | Attack::MaxQuake | Attack::MaxCyclone => 1.3,
            Attack::RideCrash => 1.5,
            Attack::RideBlow | Attack::RideSmash => 1.0,
            Attack::BrosAttack => 1.5,
            Attack::Sigil => 1.5,
            Attack::SigilTap => 1.0,
            Attack::Ice(level) | Attack::Dark(level) | Attack::Venom(level) => 1.0 + 0.15 * level as f32,
            Attack::VenomCloud => 1.0,
            // (rare on a boss: his resistance grows with each, element.rs)
            Attack::Frostbite => 1.3,
            Attack::Hemorrhage => 1.4,
            Attack::PoisonTick => 1.2,
        }
    }

    /// Ticks before the same enemy can take another of these (the rush's blows come 4 ticks apart).
    fn cooldown(self) -> u32 {
        match self {
            Attack::Rush => 3,
            // one hit per blow: the flying kick and the string's blows land over up to 5 ticks (at
            // 3 the backfist, the whirlwind kick and the flying kick each hit twice, a flurry of
            // stagger the enemy couldn't get out of)
            Attack::RushKick | Attack::Chain | Attack::Launch => 6,
            // the flurry's blows come 3 ticks apart, the spins' every few
            Attack::Uppercut | Attack::Flurry | Attack::AirHit => 3,
            Attack::Shell => 5,
            Attack::Tornado | Attack::Giro => 5,
            // a swing hits over 3 ticks, once; the next one comes 8 ticks later at the soonest
            Attack::Hammer | Attack::HammerWide | Attack::HammerAir | Attack::HammerRise | Attack::Rise | Attack::Pound | Attack::Meteor => 6,
            Attack::HammerSmash => 4,
            // the finishers always land, right after the blows before them
            Attack::UppercutTop | Attack::MeteorLand | Attack::TornadoEnd | Attack::Quake | Attack::QuakeBig | Attack::Titan | Attack::FlurryEnd | Attack::GiroEnd | Attack::ChainEnd | Attack::ChompEnd => 1,
            // (each boomerang hits an enemy once a pass: boomerang.rs)
            Attack::Rang | Attack::Egg => 2,
            Attack::Tongue => 6,
            // (the MAX shows' blows come on their own pace: maxfin.rs)
            Attack::MaxFlurry | Attack::MaxUpper | Attack::MaxQuake | Attack::MaxCyclone => 1,
            // (one bowling a second: it rolls on over them)
            Attack::Rock => 30,
            Attack::FireSweep | Attack::Ember | Attack::Trample | Attack::VenomCloud | Attack::Frostbite | Attack::Hemorrhage | Attack::PoisonTick => 1,
            Attack::RangBig => 5,
            Attack::RangUpper | Attack::Chomp | Attack::ChompSpin | Attack::ChompAir | Attack::ChompLaunch => 6,
            _ => 12,
        }
    }

    /// A combo's finishing blow (or the hammer's smash): stars fly off it (impact.rs).
    pub fn finisher(self) -> bool {
        matches!(
            self,
            Attack::UppercutTop
                | Attack::Launch
                | Attack::MeteorLand
                | Attack::TornadoEnd
                | Attack::HammerSmash
                | Attack::Quake
                | Attack::QuakeBig
                | Attack::Titan
                | Attack::FlurryEnd
                | Attack::GiroEnd
                | Attack::ChainEnd
                | Attack::ChompEnd
                | Attack::ChompLaunch
                | Attack::RangUpper
        )
    }

    /// A heavy blow's weight on a strong enemy's stance (swing.rs, its stagger meter): the hammer
    /// breaks it, more the heavier the blow.
    fn posture(self) -> f32 {
        match self {
            Attack::Hammer | Attack::HammerWide => 6.0,
            Attack::HammerAir | Attack::HammerRise | Attack::Launch => 5.0,
            Attack::Pound => 4.0,
            Attack::Quake => 9.0,
            Attack::HammerSmash | Attack::GiroEnd => 12.0,
            Attack::QuakeBig => 15.0,
            Attack::FlurryEnd => 20.0,
            Attack::Titan => 25.0,
            _ => 0.0,
        }
    }

    /// A blow's weight against a punch's (its share over a punch's, within 0.2-2.5): how much of
    /// its weapon's status it builds up (element.rs; the Chomp's cyclone, many light blows a second,
    /// builds up little each)
    pub fn weight(self) -> f32 {
        (self.percent().0 / STRING_UNIT).clamp(0.2, 2.5)
    }

    const ALL: [Attack; 6] = [Attack::Punch, Attack::Kick, Attack::Sweep, Attack::GroundPound, Attack::Stomp, Attack::Dash];
}

// SM64 action and flag bits
const ACT_FLAG_AIR: u32 = 0x0000_0800;
const MARIO_PUNCHING: u32 = 0x0010_0000;
const MARIO_KICKING: u32 = 0x0020_0000;
const MARIO_TRIPPING: u32 = 0x0040_0000;
const ACT_GROUND_POUND: u32 = 0x008008A9;
const ACT_GROUND_POUND_LAND: u32 = 0x0080023C;
const ACT_SLIDE_KICK: u32 = 0x018008AA;
const ACT_SLIDE_KICK_SLIDE: u32 = 0x0080045A;
const ACT_TWIRLING: u32 = 0x108008A4;
const ACT_ER_RUSH: u32 = 0x0080038A;

/// What kind of hit Mario's current state makes (after libsm64 said it is one).
fn classify(state: &sm64::SM64MarioState) -> Attack {
    let a = state.action;
    if a == ACT_ER_RUSH {
        if state.flags & MARIO_KICKING != 0 { Attack::RushKick } else { Attack::Rush }
    } else if state.flags & MARIO_PUNCHING != 0 {
        Attack::Punch
    } else if state.flags & MARIO_KICKING != 0 {
        Attack::Kick
    } else if state.flags & MARIO_TRIPPING != 0 {
        Attack::Sweep
    } else if a == ACT_GROUND_POUND || a == ACT_GROUND_POUND_LAND {
        Attack::GroundPound
    } else if a == ACT_SLIDE_KICK || a == ACT_SLIDE_KICK_SLIDE {
        // (Y while running, moves.rs: in the air for most of it, it's no stomp)
        Attack::Kick
    } else if a & ACT_FLAG_AIR != 0 && state.velocity[1] < 0.0 {
        Attack::Stomp
    } else {
        Attack::Dash
    }
}

/// Mirror of the game's bullet spawn request (fields are private in the bindings).
#[repr(C)]
struct SpawnRequest {
    owner: FieldInsHandle,
    behavior_id: i32,
    magic_id: i32,
    unk10: u32,
    bullet_id: i32,
    goods_id: i32,
    dummy_poly_id: i32,
    target: [u8; 8],
    unk28: u32,
    unk2c: u32,
    unk30: F32Vector4,
    unk40: u32,
    unk44: u32,
    pad48: [u8; 8],
    acceleration_angle: F32Vector4,
    unk60: F32Vector4,
    angle: F32Vector4,
    position: F32Vector4,
    rest: [u8; 0x80],
}
const _: () = assert!(size_of::<SpawnRequest>() == 0x110);

/// The bullets' own (flat) damage: just enough to always land the final blow.
const BULLET_DAMAGE: u16 = 10;
/// ...and the share of the bare fist's attack they hit with (%): a touch, so the bullet is the
/// game's own damaging hit (the reaction, the aggro, a script waiting for a hit) without adding
/// to the blow's share. At 100% it was the whole bare fist on top of every rationed blow (some
/// 230 HP in Reforged at a high level): a regular enemy lost a third of its health to a punch.
/// (the last point is the finisher's, turned up for its few ticks)
const FIST_SHARE: u16 = 5;
/// Share of the normal damage bosses (anything with a boss health bar) take.
/// (raised with the lower shares above, so bosses lose only a little; doubled when the bullets'
/// whole bare fist, most of what a boss lost, went: FIST_SHARE)
const BOSS_FACTOR: f32 = 0.13;
/// On top of every share above: Mario still took too much (after the halving of X and Y)
const DAMAGE_FACTOR: f32 = 0.6;
/// a light blow's bullet (the game's own hit) on the same enemy at most this often (ticks)
const BULLET_EVERY: u32 = 30;
/// the per-enemy cooldowns are kept this long (ticks: the longest of Attack::cooldown)
const COOLDOWN_KEEP: u32 = 32;
/// the same enemy knocked down at most this often (ticks): in between it only staggers (no knocking
/// one down again and again with the trap kit, the boulder, the hook's crashes)
const KNOCK_EVERY: u32 = 105;
/// Mega Mario's steps hurt the same enemy at most this often (ticks)
const TRAMPLE_EVERY: u32 = 45;
const INVADER_FACTOR: f32 = 0.25;
/// Mega Mario's own slam against the hammer's (see take_share)
const MEGA_SLAM: f32 = 0.5;

/// Team types on the player's side (the player, co-op phantoms, summons and spirit ashes): Mario
/// doesn't hurt those. Everyone else can be hit, friendly NPCs included, like with a weapon.
pub fn own_side(team: u8) -> bool {
    matches!(team, 1 | 2 | 5 | 12)
}

/// Regular enemies (team 6).
const TEAM_ENEMY: u8 = 6;
/// Other hostiles, some bosses among them (team 9).
const TEAM_HOSTILE: u8 = 9;
/// Enemies in groups of their own (60, and a boss on 65, seen in the field): every team from here
/// up.
const TEAM_ENEMY_GROUPS: u8 = 50;
/// Bosses and other strong enemies (team 7), also before their health bar shows.
const TEAM_STRONG_ENEMY: u8 = 7;

fn config_f32(key: &str, default: f32) -> f32 {
    crate::paths::config(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// (autopad's bullet test: the rows as Mario's blows use them)
pub fn patch_for_test() -> bool {
    patch_params()
}

/// Patches the six bullets/attacks into Mario's moves (once the params are loaded).
fn patch_params() -> bool {
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else { return false };
    for attack in Attack::ALL {
        let (bullet, atk, damage, poise, level, radius) = attack.spec();
        let Some(b) = repo.get_mut::<Bullet>(bullet as u32) else { return false };
        log(format!(
            "combat: bullet {bullet} as found: sfx {} / hit {} / flick {}, se {} {} hit {}, hit bullet {}, interval bullet {}, life {}",
            b.sfx_id_bullet(),
            b.sfx_id_hit(),
            b.sfx_id_flick(),
            b.se_id_bullet1(),
            b.se_id_bullet2(),
            b.se_id_hit(),
            b.hit_bullet_id(),
            b.interval_create_bullet_id(),
            b.life()
        ));
        b.set_atk_id_bullet(atk as i32);
        b.set_life(0.1);
        b.set_dist(0.0);
        b.set_init_vellocity(0.0);
        b.set_max_vellocity(0.0);
        b.set_min_vellocity(0.0);
        b.set_accel_in_range(0.0);
        b.set_accel_out_range(0.0);
        b.set_gravity_in_range(0.0);
        b.set_gravity_out_range(0.0);
        b.set_hit_radius(radius);
        b.set_hit_radius_max(radius);
        b.set_spread_time(0.0);
        b.set_num_shoot(1);
        b.set_homing_angle(0);
        b.set_is_penetrate_chr(true);
        b.set_is_penetrate_obj(true);
        b.set_is_penetrate_map(true);
        b.set_is_hit_both_team(false);
        let Some(a) = repo.get_mut::<AtkParam_Pc>(atk) else { return false };
        // (another mod's regulation, e.g. Reforged, may use these rows: in the game's own they're
        // unused, so a damage or sound here means the row is someone's and gets broken)
        log(format!(
            "combat: attack {atk} as found: phys {} / correction {}, AI sound {} / {}{}",
            a.atk_phys(),
            a.atk_phys_correction(),
            a.appear_ai_sound_id(),
            a.hit_ai_sound_id(),
            if a.atk_phys() > 0 && a.atk_phys() != BULLET_DAMAGE as u16 { " (LOOKS USED by the regulation)" } else { "" }
        ));
        // The damage that counts is a share of the target's max HP (Combat::deal). The bullet
        // is a real hit with a touch of the Tarnished's bare fist (FIST_SHARE). As a flat number
        // alone it took nothing off a boss, whatever the number, so no blow was ever the game's
        // own: a boss at his last point had to be ended by hand, and one whose script waits for
        // a damaging hit there (Malenia's second phase) never got it.
        let _ = damage;
        a.set_atk_phys_correction(FIST_SHARE);
        a.set_atk_mag_correction(0);
        a.set_atk_fire_correction(0);
        a.set_atk_thun_correction(0);
        a.set_atk_stam_correction(100);
        a.set_guard_atk_rate_correction(100);
        a.set_guard_break_correction(100);
        a.set_atk_phys(BULLET_DAMAGE);
        a.set_atk_obj(300); // breaks barrels, crates and other destructible objects
        a.set_atk_mag(0);
        a.set_atk_fire(0);
        a.set_atk_thun(0);
        a.set_atk_stam(30);
        // a bit under the meter's share, so an 80 poise boss doesn't break a hit early
        a.set_atk_super_armor(poise * 0.8);
        a.set_dmg_level(level);
        a.set_atk_attribute(1); // strike
        a.set_is_add_base_atk(true);
        a.set_oppose_target(true);
        a.set_friendly_target(false);
        a.set_self_target(false);
    }
    log("combat: Mario's attacks patched into bullets 65-70");
    patch_knockdown(repo);
    true
}

pub struct Combat {
    patched: bool,
    /// per target: SM64 tick of the last hit (one hit per swing)
    cooldown: HashMap<u64, u32>,
    last_hp: Option<i32>,
    /// characters Mario brought to 1 HP: SM64 tick, for the fallback kill if the bullet misses
    finishing: HashMap<u64, (FieldInsHandle, u32)>,
    /// characters Mario hit recently (their deaths heal him)
    victims: HashMap<u64, (FieldInsHandle, u32)>,
    /// small HP losses (poison and co.) add up here, a wedge per 10% of max HP
    drain: f32,
    /// stomps per enemy since Mario last stood on the ground
    stomps: HashMap<u64, u32>,
    /// Mega Mario's steps: the tick each enemy was last stepped on
    trampled: HashMap<u64, u32>,
    /// the tick of the last bullet on each enemy (they're rationed: BULLET_EVERY)
    bullet_at: HashMap<u64, u32>,
    /// stomps per enemy in a row since Mario last stood on the ground (their damage fades)
    stomp_run: HashMap<u64, u32>,
    /// blows on each enemy in the current string, and the tick of the last one (their damage fades
    /// in a long string: a showy combo isn't over in a second)
    string: HashMap<u64, (f32, u32)>,
    /// a MAX finisher that just landed: the blow and where (maxfin.rs takes it from here)
    max: Option<(Attack, glam::Vec3)>,
    /// the last blow on each enemy and how it was then (its kill's skillshot, skillshot.rs)
    last_blow: HashMap<u64, (Attack, crate::skillshot::How)>,
    /// when each enemy was last knocked down (KNOCK_EVERY)
    knocked: HashMap<u64, u32>,
}

/// What an Elden Ring hit does to Mario.
pub enum Hurt {
    /// half wedges, with SM64's knockback
    Hit(u32),
    /// one wedge, no knockback
    Drain,
}

impl Combat {
    pub fn new() -> Self {
        Combat { patched: false, cooldown: HashMap::new(), last_hp: None, finishing: HashMap::new(), victims: HashMap::new(), drain: 0.0, stomps: HashMap::new(), trampled: HashMap::new(), bullet_at: HashMap::new(), stomp_run: HashMap::new(), string: HashMap::new(), max: None, last_blow: HashMap::new(), knocked: HashMap::new() }
    }
}

/// A living character near Mario, in SM64 coordinates.
pub struct Target {
    key: u64,
    /// characters: their handle (props have none)
    handle: Option<FieldInsHandle>,
    /// body size in SM64 units (horizontal radius, height)
    pub radius: f32,
    pub height: f32,
    pub er: HavokPosition,
    pub sm: [f32; 3],
    /// an enemy Mario's punches turn to (not a friendly NPC, an ally or a prop)
    hostile: bool,
}

impl Target {
    /// A breakable prop (crate, barrel, clutter), not a character.
    pub fn is_prop(&self) -> bool {
        self.handle.is_none()
    }

    /// A character that isn't an enemy (an NPC, an ally): Mario's blows leave it alone.
    pub fn friendly(&self) -> bool {
        !self.hostile && !self.is_prop()
    }

    pub fn key(&self) -> u64 {
        self.key
    }

    /// An enemy a fireball hits: hostile, up, not in Mario's hands or thrown. Its handle.
    pub fn shootable(&self) -> Option<&FieldInsHandle> {
        self.handle.as_ref().filter(|h| self.hostile && !crate::swing::is_down(h) && !crate::carry::is_carried(h))
    }
}

/// Whether a thrown thing (fireball, boomerang) may home in on a point `d` away (m, Elden Ring
/// axes): at most ~30° up or down, and never far up or down. Without it shots aimed at birds on the
/// roofs and enemies up on ledges (the aim only looked at the flat angle) and flew off into the sky.
pub fn aim_pitch_ok(d: glam::Vec3) -> bool {
    let flat = d.with_y(0.0).length();
    let ok = d.y.abs() <= AIM_PITCH_SLACK + flat * AIM_PITCH && d.y.abs() <= AIM_MAX_RISE;
    if !ok {
        static LOGGED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 20 {
            log(format!("aim: skipped a target {:.1} m up, {flat:.1} m away", d.y));
        }
    }
    ok
}
/// (m of height allowed whatever the distance, tan of the slope beyond it, the most up or down)
const AIM_PITCH_SLACK: f32 = 1.5;
const AIM_PITCH: f32 = 0.6;
const AIM_MAX_RISE: f32 = 8.0;

pub fn handle_key(h: &FieldInsHandle) -> u64 {
    unsafe { std::mem::transmute_copy::<FieldInsHandle, u64>(h) }
}

/// An enemy, one Mario's blows land on, as a weapon's would: anyone not on the player's side, but
/// the friends' teams (`FRIENDLY_TEAMS`) and the human NPCs (c0000: merchants, quest NPCs) of the
/// teams not known to be enemies. (Teams 10-49 used to be taken for neutral, and left out: they
/// turned out to be the field's warring factions, Caelid's soldiers on 30 and 48, the giants on
/// 49.) The ini's `hostile_teams` and `friendly_teams` add to the lists.
pub fn hostile(chr: &ChrIns) -> bool {
    let team = chr.team_type;
    if team == 0 || own_side(team) || team == CHARMED {
        return false;
    }
    if listed("hostile_teams", team) || learned(chr) || matches!(chr.chr_type, ChrType::BloodyFingerNpc | ChrType::RecusantNpc) {
        return true;
    }
    if FRIENDLY_TEAMS.contains(&team) || listed("friendly_teams", team) || summoned(chr) {
        return false;
    }
    if matches!(team, TEAM_ENEMY | TEAM_STRONG_ENEMY | TEAM_HOSTILE) || team >= TEAM_ENEMY_GROUPS {
        return true;
    }
    // (a team not known to be an enemy: a warring faction, unless it's a human NPC or one with a
    // name of its own, Blaidd, Alexander, Boc...)
    chr.character_id != 0 && !named(chr)
}

/// A spirit ash or a Torrent (the co-op partner's too): the game's set of summoned buddies.
fn summoned(chr: &ChrIns) -> bool {
    // (the very same character: the set may look at the handle's index alone)
    unsafe { WorldChrMan::instance() }.ok().is_some_and(|w| w.summon_buddy_chr_set.chr_ins_by_handle(&chr.field_ins_handle).is_some_and(|c| std::ptr::eq(c, chr)))
}

/// Whether its NpcParam row gives it a name (the NPCs: Blaidd, Alexander...; none of the field
/// enemies' rows has one, Reforged's regulation looked at). Each row looked up once.
fn named(chr: &ChrIns) -> bool {
    static SEEN: std::sync::Mutex<Vec<(i32, bool)>> = std::sync::Mutex::new(Vec::new());
    let id = chr.npc_param_id;
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(_, named)) = seen.iter().find(|(i, _)| *i == id) {
        return named;
    }
    // (params not up yet: asked again later)
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return false };
    let name = repo.get::<eldenring::cs::NpcParam>(id as u32).map_or(-1, |p| p.name_id());
    // (900000000 and up: a boss's name on its health bar, not an NPC's)
    let named = name > 0 && name < 900_000_000;
    if named {
        log(format!("combat: c{:04} (NpcParam {id}, team {}) has a name ({name}): an NPC, left alone", chr.character_id, chr.team_type));
    }
    seen.push((id, named));
    named
}

/// Teams that are never Mario's enemies: friends (8), a captured enemy (15, bossride.rs), the
/// helpers by graces (26).
const FRIENDLY_TEAMS: [u8; 3] = [8, CHARMED, 26];

/// Whether the ini's list `key` (e.g. `hostile_teams = 30,48`) has `team` (read once).
fn listed(key: &'static str, team: u8) -> bool {
    static LISTS: std::sync::Mutex<Vec<(&'static str, Vec<u8>)>> = std::sync::Mutex::new(Vec::new());
    let mut lists = LISTS.lock().unwrap_or_else(|e| e.into_inner());
    if !lists.iter().any(|(k, _)| *k == key) {
        let teams = crate::paths::config(key).map(|v| v.split(',').filter_map(|t| t.trim().parse().ok()).collect()).unwrap_or_default();
        lists.push((key, teams));
    }
    lists.iter().find(|(k, _)| *k == key).is_some_and(|(_, t)| t.contains(&team))
}

/// The NPCs of teams not known to be enemies who hit Mario: each of them is hostile from then on
/// (`learn_attacker`; only that one: a merchant of the same team is left alone).
static LEARNED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

fn learned(chr: &ChrIns) -> bool {
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).contains(&handle_key(&chr.field_ins_handle))
}

/// Mario was hurt (lib.rs): an NPC his blows don't land on (a human or named one of a team not
/// known to be an enemy) who hit him is an enemy now. The game's "last hit by" often names nobody:
/// then the nearest such NPC close by and in the middle of an attack is taken to be it.
pub fn learn_attacker(player: &ChrIns) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let p = player.modules.physics.position;
    let flat = |c: &ChrIns| {
        let q = c.modules.physics.position;
        ((p.0 - q.0).powi(2) + (p.2 - q.2).powi(2)).sqrt()
    };
    // (not a friend's team: one that attacks the player is put on another by the game itself)
    let spared = |c: &ChrIns| !hostile(c) && c.team_type != 0 && !own_side(c.team_type) && c.team_type != CHARMED && !FRIENDLY_TEAMS.contains(&c.team_type);
    let alive = |c: &ChrIns| c.modules.data.hp > 0 && hittable(c.chr_type) && !helper(c) && !is_torrent(c);
    let attacking = |c: &ChrIns| {
        let t = &c.modules.time_act;
        let anim = t.anim_queue[(t.read_idx % 10) as usize].anim_id % 1_000_000;
        // (enemies' attacks are 3000-3999, a human's 30000-39999, the player's own animations)
        (3000..4000).contains(&anim) || (30000..40000).contains(&anim)
    };
    let chr = match wcm.chr_ins_by_handle(&player.last_hit_by) {
        // ("last hit by" can be stale: only one up close and alive)
        Some(c) => Some(c).filter(|c| spared(c) && alive(c) && flat(c) <= 8.0),
        None => wcm
            .chr_sets
            .iter()
            .flatten()
            .flat_map(|set| set.characters())
            .map(|c| -> &ChrIns { c })
            .filter(|c| alive(c) && spared(c) && flat(c) <= 5.0 && attacking(c))
            .min_by(|a, b| flat(a).total_cmp(&flat(b))),
    };
    let Some(chr) = chr else { return };
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).push(handle_key(&chr.field_ins_handle));
    log(format!("combat: c{:04} of team {} hit Mario: it's hostile from now on", chr.character_id, chr.team_type));
}

/// the team a captured enemy is put on (bossride.rs): never learned as hostile
const CHARMED: u8 = 15;

/// Enemies and NPCs, the red NPC invaders included; some bosses (Margit) are type 7. Never the
/// player, phantoms or the ghost kinds (bloodstains, messages, graces).
pub fn hittable(t: ChrType) -> bool {
    matches!(t, ChrType::Npc | ChrType::Unk6 | ChrType::Unk7 | ChrType::Unk9 | ChrType::Unk12 | ChrType::BloodyFingerNpc | ChrType::RecusantNpc)
}

/// One of the game's invisible helpers: a body far smaller than any creature (0.1 m wide, 0.3 m
/// tall), e.g. one at every grace with ~1900 HP. Vanilla puts them on team 0 (left alone by the
/// team check), Reforged on the enemies' team 6: Mario punched "nothing" by graces, bumped into it
/// and the fight camera framed it.
/// Also anything the game doesn't draw (rendering off, or a MapStudio draw group that isn't shown)
/// and c1000, the invisible stand-in events use: by Reforged's graces Mario still punched and
/// bumped into things of a normal size there.
pub fn helper(chr: &ChrIns) -> bool {
    helper_why(chr).is_some()
}

fn helper_why(chr: &ChrIns) -> Option<&'static str> {
    let ph = &chr.modules.physics;
    if ph.hit_radius.max(ph.chr_hit_radius) <= 0.11 && ph.hit_height.max(ph.chr_hit_height) <= 0.31 {
        Some("tiny body")
    } else if chr.character_id == 1000 {
        Some("c1000")
    } else if !chr.chr_flags1c5.enable_render() {
        Some("not drawn")
    } else if !chr.chr_flags1c4.is_render_group_enabled() {
        // (Reforged leaves enemies loaded in draw groups that aren't shown, c4373 by a grace:
        // Mario punched one 43 times and nobody saw it)
        Some("outside the shown draw groups")
    } else {
        None
    }
}

/// Torrent: the mount the game has down for the player (his model id is no help, c8002 here
/// where c8000 was expected).
pub fn is_torrent(chr: &ChrIns) -> bool {
    let Ok(gdm) = (unsafe { eldenring::cs::GameDataMan::instance() }) else { return false };
    handle_key(&gdm.main_player_game_data.mount_handle) == handle_key(&chr.field_ins_handle)
}

/// A body wider or taller than this (m) is a big enemy: a man is about 0.4 by 1.8
const BIG_WIDTH: f32 = 0.9;
const BIG_HEIGHT: f32 = 2.8;

/// Enemies within `reach` of a point, their own width added: handle, key, and whether it's a
/// boss or one of the strong ones.
pub fn in_the_way(at: glam::Vec3, reach: f32) -> Vec<(FieldInsHandle, u64, bool)> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
    let mut out = Vec::new();
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &ChrIns = chr;
            if !hittable(chr.chr_type) || chr.modules.data.hp <= 0 || chr.team_type == 0 || own_side(chr.team_type) || helper(chr) {
                continue;
            }
            let (p, ph) = (chr.modules.physics.position, &chr.modules.physics);
            let width = ph.hit_radius.max(ph.chr_hit_radius).clamp(0.3, 4.0);
            let (dx, dy, dz) = (p.0 - at.x, p.1 - at.y, p.2 - at.z);
            if dx * dx + dz * dz > (reach + width).powi(2) || !(-3.0..2.0).contains(&dy) || is_torrent(chr) {
                continue;
            }
            // (trolls, dragons and the like count with the bosses: by the size of their body)
            let height = ph.hit_height.max(ph.chr_hit_height);
            let big = width > BIG_WIDTH || height > BIG_HEIGHT;
            let handle = chr.field_ins_handle;
            out.push((handle, handle_key(&handle), big || is_boss(&handle) || chr.team_type == TEAM_STRONG_ENEMY));
        }
    }
    out
}

/// Enemies whose hits count for less against Mario: character id (cXXXX) and how much of the
/// hit is left. The Godskins took three wedges a hit: the Noble (c3550, the fat one with the
/// rapier) and the Apostle (c3560, the tall one), which covers the Godskin Duo too.
const SOFTER: [(u32, f32); 2] = [(3550, 0.6), (3560, 0.6)];

/// Who hit the player last (character id), and what's left of his hits (1 = all of it).
pub fn last_attacker(player: &ChrIns) -> (Option<u32>, f32) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return (None, 1.0) };
    let softer = |id: u32| SOFTER.iter().find(|s| s.0 == id).map(|s| s.1);
    let id = wcm.chr_ins_by_handle(&player.last_hit_by).map(|c| c.character_id);
    // (the game's "last hit by" named nobody in every fight looked at: with one of them on the
    // boss bar, it's taken to be him)
    let scale = id.and_then(softer).or_else(|| boss_handles().iter().filter_map(|h| wcm.chr_ins_by_handle(h)).find_map(|c| softer(c.character_id)));
    (id, scale.unwrap_or(1.0))
}

/// Where the character `key` (handle_key) is now, if alive and up: its chest (Elden Ring
/// position, halfway up its body, at most 1.5 m above its feet).
pub fn chest_by_key(key: u64) -> Option<glam::Vec3> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return None };
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &ChrIns = chr;
            if handle_key(&chr.field_ins_handle) != key {
                continue;
            }
            if chr.modules.data.hp <= 0 || crate::swing::is_down(&chr.field_ins_handle) || crate::carry::is_carried(&chr.field_ins_handle) {
                return None;
            }
            let (p, ph) = (chr.modules.physics.position, &chr.modules.physics);
            let h = ph.hit_height.max(ph.chr_hit_height);
            let h = if h.is_finite() && h > 0.3 { h.min(3.0) } else { 1.6 };
            return Some(glam::Vec3::new(p.0, p.1 + h * 0.5, p.2));
        }
    }
    None
}

/// Characters within `range` metres of `center` (not the player, alive).
pub fn nearby(center: &HavokPosition, range: f32, origin: [f32; 3]) -> Vec<Target> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
    let mut out = Vec::new();
    for set in wcm.chr_sets.iter().flatten() {
        for chr in set.characters() {
            let chr: &ChrIns = chr;
            if !hittable(chr.chr_type) {
                // diagnostics: what else is near Mario (once each)
                let p = chr.modules.physics.position;
                let (dx, dz) = (p.0 - center.0, p.2 - center.2);
                if dx * dx + dz * dz < range * range && !matches!(chr.chr_type, ChrType::Local) {
                    static SKIPPED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
                    let key = handle_key(&chr.field_ins_handle);
                    let mut skipped = SKIPPED.lock().unwrap_or_else(|e| e.into_inner());
                    if !skipped.contains(&key) {
                        skipped.push(key);
                        log(format!("combat: skipped character of type {:?}, team {}, hp {}", chr.chr_type, chr.team_type, chr.modules.data.hp));
                    }
                }
                continue;
            }
            // (team 0 belongs to no side: the game's invisible helpers, e.g. one at every grace with
            // ~1900 HP and a 0.1 m body, which Mario could punch to death)
            if chr.modules.data.hp <= 0 || chr.team_type == 0 || is_torrent(chr) {
                continue;
            }
            // (switched off by an event script: a boss's next phase waiting where the first fights)
            if chr.debug_flags.character_disabled() {
                continue;
            }
            let p = chr.modules.physics.position;
            let (dx, dy, dz) = (p.0 - center.0, p.1 - center.1, p.2 - center.2);
            if dx * dx + dz * dz > range * range || dy.abs() > range {
                continue;
            }
            if let Some(why) = helper_why(chr) {
                static HELPERS: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
                let key = handle_key(&chr.field_ins_handle);
                let mut helpers = HELPERS.lock().unwrap_or_else(|e| e.into_inner());
                if !helpers.contains(&key) {
                    helpers.push(key);
                    if helpers.len() > 512 {
                        helpers.remove(0);
                    }
                    log(format!("combat: left out c{:04}, invisible ({why}; team {}, {} HP)", chr.character_id, chr.team_type, chr.modules.data.max_hp));
                }
                continue;
            }
            // the character's own body size (bosses like Margit are far bigger than a human)
            let ph = &chr.modules.physics;
            let r = ph.hit_radius.max(ph.chr_hit_radius);
            let h = ph.hit_height.max(ph.chr_hit_height);
            // (up to giant size: capped at 3 m the Fire Giant's ankles were outside his own body)
            // (clamped in metres: Mega Mario's units are bigger)
            let radius = if r.is_finite() && r > 0.1 { r.clamp(0.4, 15.0) / crate::scale() } else { TARGET_RADIUS / crate::size_factor() };
            let height = if h.is_finite() && h > 0.3 { h.clamp(0.8, 40.0) / crate::scale() } else { TARGET_HEIGHT / crate::size_factor() };
            {
                static SEEN: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
                let key = handle_key(&chr.field_ins_handle);
                let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
                if !seen.contains(&key) {
                    seen.push(key);
                    if seen.len() > 512 {
                        seen.remove(0);
                    }
                    log(format!(
                        "combat: target size {radius:.0} x {height:.0} units (hit {r:.2}/{h:.2} m), c{:04} team {}, render group {}, opacity {:.2}/{:.2}/{:.2}",
                        chr.character_id,
                        chr.team_type,
                        chr.chr_flags1c4.is_render_group_enabled(),
                        chr.base_transparency,
                        chr.camouflage_transparency,
                        chr.tint_alpha_multiplier
                    ));
                }
            }
            out.push(Target {
                key: handle_key(&chr.field_ins_handle),
                handle: Some(chr.field_ins_handle.clone()),
                radius,
                height,
                // the bullet spawns 1 m above this: 0.4 m above the feet reaches sheep and dogs too
                er: HavokPosition(p.0, p.1 - 0.6, p.2, 0.0),
                sm: crate::collision::er_to_sm(origin, &p),
                hostile: hostile(chr),
            });
        }
    }
    // breakable props (barrels, crates...)
    for (index, p, layer) in crate::havok_col::props_near(glam::Vec3::new(center.0, center.1, center.2), range) {
        let lift = if layer == 0x1e { 0.4 } else { 0.0 };
        let base = HavokPosition(p.x, p.y - lift, p.z, 0.0);
        out.push(Target {
            key: (1 << 63) | index as u64,
            handle: None,
            // props are solid to Mario (his wall radius keeps him ~90 units from a barrel's
            // centre), so punches and dives need the extra reach
            radius: 80.0,
            height: 90.0,
            sm: crate::collision::er_to_sm(origin, &base),
            er: HavokPosition(p.x, p.y - lift - 0.6, p.z, 0.0),
            hostile: false,
        });
    }
    out
}

/// Whether Mario can pick this character up (a regular enemy: not a boss, not on his side).
/// A regular enemy (not a boss, not one of Mario's side, not held or thrown) the giant fireball
/// blows away (knock.rs).
pub fn blowable(handle: &FieldInsHandle) -> bool {
    if is_boss(handle) || crate::carry::is_carried(handle) || crate::swing::busy_with(handle) || crate::knock::is_flying(handle) {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    wcm.chr_ins_by_handle(handle).is_some_and(|c| !own_side(c.team_type) && !boss_class(handle, c))
}

pub(crate) fn liftable(handle: &FieldInsHandle) -> bool {
    if is_boss(handle) {
        return false;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    wcm.chr_ins_by_handle(handle).is_some_and(|c| !own_side(c.team_type) && !boss_class(handle, c))
}

/// Characters that have shown a boss bar at some point (the bar only appears once a fight starts).
static SEEN_BOSSES: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
/// Big or tough characters count as bosses for damage even before (or without) a boss bar.
const BOSS_RADIUS: f32 = 1.5;
const BOSS_MAX_HP: f32 = 2500.0;

/// Reforged gives enemies more health (its NpcParam HP is 1.7 times vanilla's at the median, 2.6
/// at the top decile): its regular soldiers, 2800-3100 HP, passed BOSS_MAX_HP, took a boss's
/// share of the damage and were never lifted by the super jump punch. The line moves up with it:
/// ini `enemy_hp_scale`, else 2 with Reforged's DLL loaded, 1 without.
fn hp_scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        let reforged = reforged();
        let scale = config_f32("enemy_hp_scale", if reforged { 2.0 } else { 1.0 }).clamp(0.5, 10.0);
        log(format!("combat: enemies count as bosses from {:.0} HP (x{scale}{})", BOSS_MAX_HP * scale, if reforged { ", Reforged" } else { "" }));
        scale
    })
}

/// Whether Elden Ring Reforged's DLL is loaded.
fn reforged() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::core::w!("reforged.dll")) }.is_ok())
}

/// Reforged's enemies are meant to hold out longer (1.7 times the health), but Mario's blows take a
/// share of their health, so they fell as fast as in vanilla: his blows are worth this much less.
const REFORGED_DAMAGE: f32 = 0.7;

/// Whether a character takes boss damage: a boss bar now or before, the strong-enemy team, or
/// big / tough enough (a field boss hit before his bar showed took a regular enemy's 50%).
pub fn boss_class(handle: &FieldInsHandle, chr: &ChrIns) -> bool {
    boss_why(handle, chr).is_some()
}

/// What makes a character a boss for Mario (`boss_class`), None if it's a regular enemy.
pub fn boss_why(handle: &FieldInsHandle, chr: &ChrIns) -> Option<String> {
    if is_boss(handle) {
        Some("boss bar".into())
    } else if chr.team_type == TEAM_STRONG_ENEMY {
        Some("strong-enemy team".into())
    } else if SEEN_BOSSES.lock().unwrap_or_else(|e| e.into_inner()).contains(&handle_key(handle)) {
        Some("had a boss bar".into())
    } else if chr.modules.physics.hit_radius >= BOSS_RADIUS {
        Some(format!("body {:.1} m wide", chr.modules.physics.hit_radius))
    } else if chr.modules.data.max_hp as f32 >= BOSS_MAX_HP * hp_scale() {
        Some(format!("{} HP", chr.modules.data.max_hp))
    } else {
        None
    }
}

/// Logs why the super jump punch left `handle` on the ground (once in a while per enemy).
fn log_not_lifted(handle: &FieldInsHandle, radius_m: f32, height_m: f32, stunned: bool) {
    static LOGGED: std::sync::Mutex<Vec<(u64, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
    let key = handle_key(handle);
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    logged.retain(|(_, at)| at.elapsed().as_secs_f32() < 5.0);
    if logged.iter().any(|(k, _)| *k == key) {
        return;
    }
    logged.push((key, std::time::Instant::now()));
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle(handle) else { return };
    let why = match crate::juggle::refusal(radius_m, height_m, liftable(handle), stunned) {
        Some(crate::juggle::NOT_STUNNED) => match boss_why(handle, chr) {
            Some(why) => format!("a boss ({why}), not stunned"),
            None => "on Mario's side".into(),
        },
        Some(why) => why.to_string(),
        None => "too many up already, or down".into(),
    };
    log(format!("juggle: c{:04} not lifted: {why} (team {}, {:.1} x {:.1} m, {} HP)", chr.character_id, chr.team_type, radius_m, height_m, chr.modules.data.max_hp));
}

/// How long a boss at his last point is left to his script before he's finished by hand (SM64
/// ticks, 30 a second).
const BOSS_FINISH_TICKS: u32 = 90;
/// A boss stuck at his last point, and what the real blows on him showed.
#[derive(Default)]
struct Probe {
    /// tick a blow was sent at, while its bullet lives
    fired: Option<u32>,
    misses: u32,
    undying: bool,
}

static PROBES: std::sync::Mutex<Option<HashMap<u64, Probe>>> = std::sync::Mutex::new(None);
/// How long a blow's bullet gets to land (ticks), and how many may miss before he's ended by hand
const PROBE_TICKS: u32 = 10;
const PROBE_MISSES: u32 = 3;
/// What the punch's bullet does for a real blow, and the tick it goes back to its chip at.
/// (not above 32767: at 60000 the blow made her flinch and took nothing off, as if the game
/// read the number as a signed one)
const FINISHER_DAMAGE: u16 = 30000;
static FINISHER_UNTIL: std::sync::Mutex<Option<u32>> = std::sync::Mutex::new(None);

/// A punch that does real damage, where the others only chip: the punch's attack is turned up
/// for the few ticks its bullet lives.
fn finisher(player: &ChrIns, at: &HavokPosition, tick: u32) -> String {
    let (bullet, atk, ..) = Attack::Punch.spec();
    let Some(a) = (unsafe { SoloParamRepository::instance_mut() }).ok().and_then(|r| r.get_mut::<AtkParam_Pc>(atk)) else { return "no attack param".into() };
    // (flat damage alone took nothing off her, at 60000 or 30000: also as a share of the
    // weapon's attack, with the weapon's own added, whichever of them the game goes by)
    a.set_atk_phys(FINISHER_DAMAGE);
    a.set_atk_phys_correction(10000);
    *FINISHER_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) = Some(tick.wrapping_add(8));
    strike(player, at, bullet)
}

/// The knockdown's own bullet (the banana, the shells, the boulder, the hook's crashes on regular
/// enemies): an unused dev bullet and attack of its own (from the far ends of noise.rs's ranges),
/// set up like the blows' with the game's knockdown as its hit reaction (`knockdown_level` in
/// er_mario.ini: AtkParam's dmgLevel, 4 "blown away" by default) and poise damage enough for a
/// regular enemy. Only ever fired at regular ones (a boss is never knocked down by it).
static KNOCK_BULLET: std::sync::Mutex<Option<i32>> = std::sync::Mutex::new(None);
const KNOCK_POISE: f32 = 60.0;

fn patch_knockdown(repo: &mut SoloParamRepository) {
    let level = crate::paths::config("knockdown_level").and_then(|v| v.parse().ok()).unwrap_or(4u8);
    let bullet = (100..=199u32).rev().find(|&b| repo.get::<Bullet>(b).is_some_and(|r| r.atk_id_bullet() <= 0 && r.sfx_id_bullet() <= 0));
    let atk = (900..=979u32).rev().find(|&a| repo.get::<AtkParam_Pc>(a).is_some_and(|r| r.atk_phys() == 0 && r.appear_ai_sound_id() <= 0));
    let (Some(bullet), Some(atk)) = (bullet, atk) else {
        log("combat: no free bullet / attack for the knockdown: the trap kit only staggers");
        return;
    };
    let (sweep, ..) = Attack::Sweep.spec();
    let Some(from) = repo.get::<Bullet>(sweep as u32).map(|b| (b.hit_radius(), b.life())) else { return };
    let Some(b) = repo.get_mut::<Bullet>(bullet) else { return };
    b.set_atk_id_bullet(atk as i32);
    b.set_sfx_id_bullet(-1);
    b.set_sfx_id_flick(-1);
    b.set_life(from.1);
    b.set_dist(0.0);
    b.set_init_vellocity(0.0);
    b.set_max_vellocity(0.0);
    b.set_min_vellocity(0.0);
    b.set_accel_in_range(0.0);
    b.set_accel_out_range(0.0);
    b.set_gravity_in_range(0.0);
    b.set_gravity_out_range(0.0);
    b.set_hit_radius(from.0);
    b.set_hit_radius_max(from.0);
    b.set_spread_time(0.0);
    b.set_num_shoot(1);
    b.set_homing_angle(0);
    b.set_is_penetrate_chr(true);
    b.set_is_penetrate_obj(true);
    b.set_is_penetrate_map(true);
    b.set_is_hit_both_team(false);
    let Some(a) = repo.get_mut::<AtkParam_Pc>(atk) else { return };
    a.set_atk_phys_correction(FIST_SHARE);
    a.set_atk_mag_correction(0);
    a.set_atk_fire_correction(0);
    a.set_atk_thun_correction(0);
    a.set_atk_stam_correction(100);
    a.set_guard_atk_rate_correction(100);
    a.set_guard_break_correction(100);
    a.set_atk_phys(BULLET_DAMAGE);
    a.set_atk_obj(300);
    a.set_atk_mag(0);
    a.set_atk_fire(0);
    a.set_atk_thun(0);
    a.set_atk_stam(30);
    a.set_atk_super_armor(KNOCK_POISE);
    a.set_dmg_level(level);
    a.set_atk_attribute(1);
    a.set_is_add_base_atk(true);
    a.set_oppose_target(true);
    a.set_friendly_target(false);
    a.set_self_target(false);
    *KNOCK_BULLET.lock().unwrap_or_else(|e| e.into_inner()) = Some(bullet as i32);
    log(format!("combat: the knockdown is bullet {bullet} / attack {atk} (hit reaction level {level})"));
}

/// Bosses whose script took over at their last point: Mario's hits do nothing more to them.
static SCRIPTED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

/// The character plays one of the animations event scripts force (20000-39999: intros, phase
/// changes, waiting for a cutscene).
fn scripted(chr: &ChrIns) -> bool {
    let t = &chr.modules.time_act;
    (20000..40000).contains(&(t.anim_queue[(t.read_idx % 10) as usize].anim_id % 1_000_000))
}

/// A boss the game is taking through a phase change: no hits, no grab.
pub fn hands_off(handle: &FieldInsHandle) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    let Some(chr) = wcm.chr_ins_by_handle(handle) else { return false };
    let key = handle_key(handle);
    let mut left = SCRIPTED.lock().unwrap_or_else(|e| e.into_inner());
    // (back at more than his last point: the fight started over)
    if chr.modules.data.hp > 1 {
        left.retain(|k| *k != key);
        return false;
    }
    left.contains(&key) || (is_boss(handle) && scripted(chr))
}

/// Whether a character is a boss right now (its health bar is on screen).
pub fn is_boss_now(handle: &FieldInsHandle) -> bool {
    is_boss(handle)
}

fn is_boss(handle: &FieldInsHandle) -> bool {
    let key = handle_key(handle);
    unsafe { eldenring::cs::CSFeManImp::instance() }
        .is_ok_and(|fe| fe.boss_health_displays.iter().any(|e| !e.field_ins_handle.is_empty() && handle_key(&e.field_ins_handle) == key))
}

/// Characters Mario hit recently: their health bar and damage number (the overlay draws them;
/// hud.rs). Boss damage goes on the game's boss bar.
struct Tag {
    handle: FieldInsHandle,
    hit: std::time::Instant,
    /// damage in the current combo, and the HP before it
    dmg: i32,
    before: i32,
}

static TAGS: std::sync::Mutex<Vec<Tag>> = std::sync::Mutex::new(Vec::new());
/// a combo ends (the damage number resets) this long after the last hit; the bar goes 1.5 s later
const TAG_COMBO: f32 = 1.5;

fn show_damage(handle: &FieldInsHandle, before: i32, dmg: i32, _boss_bar: bool) {
    let key = handle_key(handle);
    let mut tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());
    match tags.iter_mut().find(|t| handle_key(&t.handle) == key) {
        Some(t) if t.hit.elapsed().as_secs_f32() < TAG_COMBO => {
            t.dmg += dmg;
            t.hit = std::time::Instant::now();
        }
        Some(t) => *t = Tag { handle: *handle, hit: std::time::Instant::now(), dmg, before },
        None => tags.push(Tag { handle: *handle, hit: std::time::Instant::now(), dmg, before }),
    }
}

/// Game thread, every frame: the bars to draw (head position, HP share now, HP share before the
/// combo, combo damage while it lasts).
pub fn tags() -> Vec<crate::hud::Tag> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
    let mut tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());
    tags.retain(|t| t.hit.elapsed().as_secs_f32() < TAG_COMBO * 2.0 && wcm.chr_ins_by_handle(&t.handle).is_some());
    let bosses = boss_keys();
    tags.iter()
        .filter(|t| !bosses.contains(&handle_key(&t.handle)))
        .filter_map(|t| {
            let chr = wcm.chr_ins_by_handle(&t.handle)?;
            let (hp, max) = (chr.modules.data.hp.max(0) as f32, chr.modules.data.max_hp.max(1) as f32);
            let p = chr.modules.physics.position;
            let combo = t.hit.elapsed().as_secs_f32() < TAG_COMBO;
            Some(crate::hud::Tag {
                pos: glam::Vec3::new(p.0, p.1, p.2),
                hp: hp / max,
                before: if combo { t.before as f32 / max } else { hp / max },
                dmg: if combo { t.dmg } else { 0 },
                status: crate::element::bar(&t.handle),
            })
        })
        .collect()
}

/// A thrown character hit something: `pct` of its max HP, lethal if that's all it had left.
pub fn impact(combat: &mut Combat, handle: &FieldInsHandle, pct: f32, tick: u32) {
    // co-op guest: the host takes it off, here only a preview that never ends him
    let guest = crate::coop::guest();
    crate::coop::send_hit(handle, IMPACT_CODE, pct);
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle_mut(handle) else { return };
    let data = &mut chr.modules.data;
    let (hp, max) = (data.hp, data.max_hp.max(1));
    // (co-op: Seamless's extra health per player counts, coop::share_factor)
    let pct = pct * crate::coop::share_factor(is_boss(handle));
    let dmg = ((max as f32 * pct / 100.0).ceil() as i32).max(1);
    // (throws kill right there when the impact takes the rest of his HP; a boss with a health
    // bar keeps his last point for a moment, in case his script wants him at it)
    data.hp = (hp - dmg).max(if is_boss(handle) || guest { 1 } else { 0 });
    if guest {
        show_damage(handle, hp, hp - data.hp, true);
        return;
    }
    let dealt = hp - data.hp;
    show_damage(handle, hp, dealt, true);
    if data.hp == 1 {
        combat.finishing.entry(handle_key(handle)).or_insert((*handle, tick));
    }
    combat.victims.insert(handle_key(handle), (*handle, tick));
    if !is_boss(handle) {
        pass_to_bar(wcm, dealt);
    }
}

/// A thrown enemy's landing that doesn't kill (the hook's home run, bat.rs): `pct` of its max HP,
/// never its last point (a launch never ends one by itself).
pub fn impact_soft(combat: &mut Combat, handle: &FieldInsHandle, pct: f32, tick: u32) {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle(handle) else { return };
    let (hp, max) = (chr.modules.data.hp, chr.modules.data.max_hp.max(1));
    // (as a share of what's left over 1 HP: whatever it is, he gets up from it)
    let room = ((hp - 1).max(0) as f32 / max as f32 * 100.0).max(0.0);
    let pct = pct.min(room * 0.9);
    if pct > 0.0 {
        impact(combat, handle, pct, tick);
    }
}

/// The God-Devouring Serpent and Rykard, who stand in lava (c4710, c4711).
const LAVA_BOSSES: [u32; 2] = [4710, 4711];

/// One of the bosses who fight from a lava pool has his bar up. Mario can only hit what he
/// stands next to, so for that fight the lava does nothing to him.
pub fn lava_fight() -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return false };
    boss_handles().iter().any(|h| wcm.chr_ins_by_handle(h).is_some_and(|c| LAVA_BOSSES.contains(&c.character_id)))
}

/// The bosses on screen (their boss bars).
pub fn boss_handles() -> Vec<FieldInsHandle> {
    let handles: Vec<FieldInsHandle> = unsafe { eldenring::cs::CSFeManImp::instance() }
        .map(|fe| fe.boss_health_displays.iter().filter(|e| !e.field_ins_handle.is_empty()).map(|e| e.field_ins_handle).collect())
        .unwrap_or_default();
    let mut seen = SEEN_BOSSES.lock().unwrap_or_else(|e| e.into_inner());
    for h in &handles {
        let k = handle_key(h);
        if !seen.contains(&k) {
            seen.push(k);
        }
    }
    handles
}

fn boss_keys() -> Vec<u64> {
    unsafe { eldenring::cs::CSFeManImp::instance() }
        .map(|fe| fe.boss_health_displays.iter().filter(|e| !e.field_ins_handle.is_empty()).map(|e| handle_key(&e.field_ins_handle)).collect())
        .unwrap_or_default()
}

/// Game thread, every frame: the boss bars to draw (the game's boss list: name, HP now and
/// before Mario's combo, combo damage).
pub fn bosses() -> Vec<crate::hud::BossBar> {
    static NAMES: std::sync::Mutex<Vec<(i32, String)>> = std::sync::Mutex::new(Vec::new());
    let (Ok(fe), Ok(wcm)) = (unsafe { eldenring::cs::CSFeManImp::instance() }, unsafe { WorldChrMan::instance() }) else {
        return Vec::new();
    };
    let tags = TAGS.lock().unwrap_or_else(|e| e.into_inner());
    let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    fe.boss_health_displays
        .iter()
        .filter(|e| !e.field_ins_handle.is_empty())
        .filter_map(|e| {
            let chr = wcm.chr_ins_by_handle(&e.field_ins_handle)?;
            let (hp, max) = (chr.modules.data.hp.max(0) as f32, chr.modules.data.max_hp.max(1) as f32);
            let name = match names.iter().find(|(id, _)| *id == e.fmg_id) {
                Some((_, n)) => n.clone(),
                None => {
                    let n = crate::names::text(e.fmg_id).unwrap_or_default();
                    names.push((e.fmg_id, n.clone()));
                    n
                }
            };
            let key = handle_key(&e.field_ins_handle);
            let combo = tags.iter().find(|t| handle_key(&t.handle) == key && t.hit.elapsed().as_secs_f32() < TAG_COMBO);
            Some(crate::hud::BossBar {
                name,
                hp: hp / max,
                before: combo.map(|t| t.before as f32 / max).unwrap_or(hp / max),
                dmg: combo.map(|t| t.dmg).unwrap_or(0),
                status: crate::element::bar(&e.field_ins_handle),
            })
        })
        .collect()
}

/// Damage to a boss body that isn't the one on the boss bar (the Fire Giant's first phase: the
/// bar is his second-phase character, with about twice the HP, and the game passes each hit on
/// to it): the bar's character loses the same HP, or the bar stood still for the whole first phase.
fn pass_to_bar(wcm: &mut WorldChrMan, dealt: i32) {
    let on_bar: Vec<FieldInsHandle> = unsafe { eldenring::cs::CSFeManImp::instance() }
        .map(|fe| fe.boss_health_displays.iter().filter(|e| !e.field_ins_handle.is_empty()).map(|e| e.field_ins_handle.clone()).collect())
        .unwrap_or_default();
    let [only] = on_bar.as_slice() else { return };
    let Some(main) = wcm.chr_ins_by_handle_mut(only).filter(|c| !own_side(c.team_type) && c.modules.data.hp > 1) else { return };
    let d = &mut main.modules.data;
    let hp = d.hp;
    d.hp = (hp - dealt).max(1);
    show_damage(only, hp, hp - d.hp, true);
    log(format!("combat: passed on to the boss bar's character: {dealt} of {} HP, {} left", d.max_hp, d.hp));
}

/// Takes the attack's share of the character's max HP (never the last point: the bullet deals the
/// final blow, so the kill is the game's own). Returns whether he's down to that last point now;
/// None if the blow took nothing (an ally, a boss in the middle of a scripted turn, a thrown one).
/// `scale`: a share of the attack's damage (stomps in a row, Combat::deal).
fn take_share(handle: &FieldInsHandle, attack: Attack, scale: f32) -> Option<bool> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return None };
    let bar = is_boss(handle);
    // a thrown boss in the air or lying limp takes no hits (the throw's impact is the damage)
    if crate::swing::is_down(handle) || hands_off(handle) {
        return None;
    }
    // (nor while a real blow is finding out whether he can die: his health is the answer)
    if PROBES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|p| p.get(&handle_key(handle)).is_some_and(|p| p.fired.is_some())) {
        return None;
    }
    let Some(chr) = wcm.chr_ins_by_handle_mut(handle) else { return None };
    let team = chr.team_type;
    if own_side(team) {
        log(format!("combat: team {team} is on the player's side, no damage"));
        return None;
    }
    let boss = bar || boss_class(handle, chr);
    let (default, key) = attack.percent();
    let factor = DAMAGE_FACTOR * if reforged() { REFORGED_DAMAGE } else { 1.0 };
    let mut pct = config_f32(key, default) * scale * config_f32("damage_factor", factor);
    if boss {
        pct *= config_f32("boss_damage_factor", BOSS_FACTOR) * attack.boss_bonus();
    }
    // a counter, out of a perfect dodge (dodge.rs): blows up close, not shots from afar
    if crate::dodge::countering() && !attack.ranged() && !attack.status() {
        pct *= crate::dodge::COUNTER_DAMAGE;
    }
    // co-op: BROS POWER, the partner just leapt off us into a Bros. Attack (brosattack.rs)
    if crate::brosattack::powered() && !attack.ranged() && !attack.status() {
        pct *= crate::brosattack::POWER;
    }
    // Mega Mario's blows are a giant's
    if crate::mega::active() {
        pct *= crate::mega::DAMAGE;
        // (his own slam, Y without the hammer, is the hammer's slam in his giant hands: at the
        // weight it had before the hammer became the heavy weapon)
        if crate::weapons::current() != crate::weapons::HAMMER && matches!(attack, Attack::Quake | Attack::QuakeBig | Attack::Titan) {
            pct *= MEGA_SLAM;
        }
    }
    // Metal Mario hits harder (his fireballs too)
    if crate::caps::active() == Some(crate::caps::METAL) {
        pct *= crate::caps::METAL_DAMAGE;
        if let Some((t, tmax)) = crate::swing::toughness_of(handle) {
            log(format!("combat: boss poise {t:.0}/{tmax:.0}"));
        }
        // (not what burns or poisons on its own pace: no stance broken by standing in a cloud)
        if !attack.status() && !matches!(attack, Attack::FireSweep | Attack::Ember | Attack::VenomCloud) {
            crate::swing::add_stance(handle, attack.spec().3);
        }
    }
    // the red NPC invaders are fights of their own, not two-hit mobs
    if !boss && matches!(chr.chr_type, ChrType::BloodyFingerNpc | ChrType::RecusantNpc) {
        pct *= config_f32("invader_damage_factor", INVADER_FACTOR);
    }
    // the weapon's element against him (element.rs): weak to it, or resisting it; frostbitten
    pct *= crate::element::damage_factor(handle, chr, attack);
    // co-op guest: the host takes it off (here only a preview of it, see take_pct)
    crate::coop::send_hit(handle, attack.code(), pct);
    Some(take_pct(handle, pct, &format!("{attack:?}")))
}

/// Takes `pct` of the character's max HP (never the last point). Returns true if it's down to
/// that last point. `what`: for the log.
fn take_pct(handle: &FieldInsHandle, pct: f32, what: &str) -> bool {
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return false };
    let bar = is_boss(handle);
    let Some(chr) = wcm.chr_ins_by_handle_mut(handle) else { return false };
    let team = chr.team_type;
    if own_side(team) {
        return false;
    }
    let boss = bar || boss_class(handle, chr);
    let cid = chr.character_id;
    let data = &mut chr.modules.data;
    let (hp, max) = (data.hp, data.max_hp.max(1));
    // (co-op: Seamless's extra health per player counts, coop::share_factor)
    let pct = pct * crate::coop::share_factor(boss || is_boss(handle));
    let dmg = ((max as f32 * pct / 100.0).ceil() as i32).max(1);
    data.hp = (hp - dmg).max(1);
    show_damage(handle, hp, hp - data.hp, bar);
    log(format!("combat: {what} on c{cid:04} team {team}{}: {dmg} of {max} HP ({pct:.1}%), {} left", if boss { " boss" } else { "" }, data.hp));
    let (last, dealt) = (data.hp == 1, hp - data.hp);
    if boss && !bar {
        pass_to_bar(wcm, dealt);
    }
    last
}

/// Stomps in a row on the same body without touching the ground (bouncing up a big one is many):
/// each takes half of the one before (100%, 50%, 25%...), down to STOMP_FLOOR.
const STOMP_FADE: f32 = 0.5;
/// A string of blows on one enemy (no more than STRING_GAP ticks apart): the first STRING_FULL
/// punches' worth count in full, then each blow a share of the one before (by its weight: a
/// hammer's blow weighs two punches or more, a cyclone's a fraction of one), down to STRING_FLOOR
/// (a finisher's never below STRING_FINISHER_FLOOR). (By count, the hammer's few heavy blows
/// hardly ever faded and the cyclone's many light ones faded at once.)
const STRING_GAP: u32 = 75;
const STRING_FULL: f32 = 10.0;
/// a punch's share: a blow's weight in a string is its share against this, within these
const STRING_UNIT: f32 = 13.5;
const STRING_WEIGHT: (f32, f32) = (0.3, 4.0);
const STRING_FADE: f32 = 0.95;
const STRING_FLOOR: f32 = 0.4;
const STRING_FINISHER_FLOOR: f32 = 0.7;
const STOMP_FLOOR: f32 = 0.1;

/// Target body in SM64 units: horizontal radius and height (a human-sized capsule).
const TARGET_RADIUS: f32 = 55.0;
const TARGET_HEIGHT: f32 = 180.0;
/// Mario's own hitbox radius in SM64 (units) plus a little reach.
const MARIO_RADIUS: f32 = 60.0;
/// ...when landing on an enemy from above (a wider one bounced Mario off heads he missed)
const STOMP_RADIUS: f32 = 52.0;
/// The tallest body a stomp goes by (SM64 units): humans are shorter, beasts' capsules aren't.
const STOMP_HEIGHT: f32 = 220.0;
/// A punch's or kick's reach beyond the target's body (SM64 units), within this cone (cos 60°)
/// in front.
const PUNCH_REACH: f32 = 135.0;
const PUNCH_CONE_COS: f32 = 0.5;
/// Each hit of the punch combo turns Mario to the nearest enemy within this reach beyond its
/// body (SM64 units) and this cone (cos 110°) around where he faces: punches thrown a little to
/// the side of an enemy in front missed it.
const AIM_REACH: f32 = 420.0;
const AIM_CONE_COS: f32 = -0.34;
/// ...and keeps to the one it turned him to last while it's about as good (a string doesn't hop
/// between two enemies side by side); enemies up to this high above his feet count (units)
const AIM_KEEP: f32 = 70.0;
const AIM_ABOVE: f32 = 160.0;
/// Falling, Mario drifts onto the nearest enemy below within this reach beyond its body (SM64
/// units): stomps landed just beside heads.
const HOME_REACH: f32 = 150.0;

/// The enemy (SM64 x, z) the next hit of Mario's punch combo turns him to, if any, and how far it
/// is beyond a punch's reach (units: the blow lunges at it).
pub fn aim(state: &sm64::SM64MarioState, targets: &[Target]) -> Option<(f32, f32, f32)> {
    static LAST: std::sync::Mutex<Option<u64>> = std::sync::Mutex::new(None);
    let m = state.position;
    let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
    let last = *LAST.lock().unwrap_or_else(|e| e.into_inner());
    let best = targets
        .iter()
        .filter(|t| t.hostile && t.handle.as_ref().is_some_and(|h| !crate::swing::is_down(h) && !crate::carry::is_carried(h)))
        .filter_map(|t| {
            let (dx, dz) = (t.sm[0] - m[0], t.sm[2] - m[2]);
            let horizontal = (dx * dx + dz * dz).sqrt();
            let radius = t.radius;
            if horizontal < 1.0 || horizontal > AIM_REACH + radius || m[1] > t.sm[1] + t.height || m[1] + AIM_ABOVE < t.sm[1] {
                return None;
            }
            let cos = (dx * fx + dz * fz) / horizontal;
            // the nearest, and of two as near the one more in front (the last one a little ahead)
            let gap = (horizontal - radius).max(0.0);
            let keep = if last == Some(t.key()) { AIM_KEEP } else { 0.0 };
            (cos >= AIM_CONE_COS).then(|| (gap + 110.0 * (1.0 - cos) - keep, t.sm[0], t.sm[2], gap - PUNCH_REACH * 0.8, t.key()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = best.map(|b| b.4);
    best.map(|(_, x, z, gap, _)| (x, z, gap.max(0.0)))
}

/// The enemy (SM64 x, z) a falling Mario drifts onto, if any: the nearest whose top is below him.
pub fn home(state: &sm64::SM64MarioState, targets: &[Target]) -> Option<(f32, f32)> {
    let m = state.position;
    targets
        .iter()
        .filter(|t| t.hostile && t.handle.as_ref().is_some_and(|h| !crate::swing::is_down(h) && !crate::carry::is_carried(h)))
        .filter_map(|t| {
            let (dx, dz) = (t.sm[0] - m[0], t.sm[2] - m[2]);
            let edge = (dx * dx + dz * dz).sqrt() - t.radius;
            // a big body is climbed anywhere on it (hits), a small one from above its head
            let low = if t.height > STOMP_HEIGHT { t.sm[1] + CLIMB_MIN } else { t.sm[1] + t.height.min(STOMP_HEIGHT) * 0.5 };
            (edge <= HOME_REACH && m[1] > low && m[1] < t.sm[1] + t.height + 600.0).then_some((edge, t.sm[0], t.sm[2]))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, x, z)| (x, z))
}

/// The enemies around Mario the fight's camera frames as a group (lakitu.rs): their chests (Elden
/// Ring), not the big bodies (the camera has its own way with those).
pub fn crowd(targets: &[Target]) -> Vec<glam::Vec3> {
    targets
        .iter()
        .filter(|t| t.shootable().is_some() && t.height * crate::scale() < 4.0)
        .map(|t| glam::Vec3::new(t.er.0, t.er.1 + (t.height * crate::scale()).min(3.0) * 0.5, t.er.2))
        .collect()
}

/// The enemy the fight's camera frames with Mario (lakitu.rs): one he's juggling first, else the
/// nearest hostile around him (those in front of him first); its chest (Elden Ring) and whether
/// it's up in the air for him.
pub fn fight_focus(state: &sm64::SM64MarioState, targets: &[Target]) -> Option<(glam::Vec3, bool)> {
    const REACH: f32 = 900.0;
    let m = state.position;
    let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
    targets
        .iter()
        .filter(|t| t.hostile && t.height <= STOMP_HEIGHT * 2.0)
        .filter_map(|t| {
            let h = t.handle.as_ref()?;
            let (dx, dz) = (t.sm[0] - m[0], t.sm[2] - m[2]);
            let d = (dx * dx + dz * dz).sqrt();
            if d > REACH + t.radius || (t.sm[1] - m[1]).abs() > 800.0 || crate::carry::is_carried(h) {
                return None;
            }
            let up = crate::juggle::is_juggled(h);
            let cos = if d > 1.0 { (dx * fx + dz * fz) / d } else { 1.0 };
            let score = (d - t.radius).max(0.0) + 150.0 * (1.0 - cos) - if up { 2000.0 } else { 0.0 };
            let chest = glam::Vec3::new(t.er.0, t.er.1 + t.height * crate::scale() * 0.6, t.er.2);
            Some((score, chest, up))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c, up)| (c, up))
}

/// Big bodies near Mario (Elden Ring feet position, radius and height in m): the camera keeps
/// out of them.
pub fn big_bodies(targets: &[Target]) -> Vec<(glam::Vec3, f32, f32)> {
    targets
        .iter()
        .filter(|t| !t.is_prop() && (t.height > STOMP_HEIGHT || t.radius > 120.0))
        .map(|t| (glam::Vec3::new(t.er.0, t.er.1 + 0.6, t.er.2), t.radius * crate::scale(), t.height * crate::scale()))
        .collect()
}
/// the enemies faded by `fade_occluders`, how see-through each is now, and its last frame
static FADED_STATE: std::sync::Mutex<Option<(HashMap<u64, (FieldInsHandle, f32)>, std::time::Instant)>> = std::sync::Mutex::new(None);

/// Every faded enemy back to full (Mario gone, Lakitu's camera off).
pub fn restore_faded() {
    let Some((faded, _)) = FADED_STATE.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    for (h, _) in faded.values() {
        if let Some(chr) = wcm.chr_ins_by_handle_mut(h) {
            chr.opacity_keyframes_multiplier = 1.0;
            chr.opacity_keyframes_multiplier_previous = 1.0;
        }
    }
}

/// Big enemies the camera is inside of, or that stand between it and Mario (`chest`, Elden Ring),
/// fade out until they're out of the way (a beast sunk into the ground swallowed the view whole when
/// Mario walked into it). Every frame; each faded one comes back on its own.
pub fn fade_occluders(targets: &[Target], cam: glam::Vec3, chest: glam::Vec3) {
    use glam::Vec3;
    /// how see-through, and how fast it gets there and back (per second)
    const FADED: f32 = 0.22;
    const RATE: f32 = 7.0;
    let mut guard = FADED_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let (faded, last) = guard.get_or_insert_with(|| (HashMap::new(), std::time::Instant::now()));
    let dt = last.elapsed().as_secs_f32().min(0.1);
    *last = std::time::Instant::now();
    let mut want: HashMap<u64, FieldInsHandle> = HashMap::new();
    for t in targets.iter().filter(|t| t.hostile && (t.height > 220.0 || t.radius > 90.0)) {
        let Some(h) = &t.handle else { continue };
        let (foot, r, top) = (Vec3::new(t.er.0, t.er.1, t.er.2), t.radius * crate::scale() + 0.35, t.er.1 + t.height * crate::scale() + 0.3);
        let inside = |p: Vec3, r: f32| p.y > foot.y - 0.5 && p.y < top && (p - foot).with_y(0.0).length() < r;
        // the camera in it, or the line to Mario through it (not its last bit: Mario next to it)
        let blocks = inside(cam, r) || (1..9).any(|k| inside(cam.lerp(chest, k as f32 / 10.0), r * 0.85));
        if blocks {
            want.insert(t.key, *h);
        }
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    for (k, h) in &want {
        faded.entry(*k).or_insert((*h, 1.0));
    }
    faded.retain(|k, (h, alpha)| {
        let goal = if want.contains_key(k) { FADED } else { 1.0 };
        *alpha += (goal - *alpha).clamp(-RATE * dt, RATE * dt);
        let Some(chr) = wcm.chr_ins_by_handle_mut(h) else { return false };
        chr.opacity_keyframes_multiplier = *alpha;
        chr.opacity_keyframes_multiplier_previous = *alpha;
        !(goal >= 1.0 && *alpha >= 1.0)
    });
}

/// Called on the libsm64 thread right after a tick: which targets Mario hits now.
/// Stomps in a row on one enemy before Mario has to land (SM64's head bounce would juggle forever).
const MAX_STOMPS: u32 = 1;
/// ...on a big body, climbed a leg, a wing, the back, the head
const MAX_CLIMB: u32 = 5;
/// A big body is a foothold from this high above its feet (SM64 units)...
const CLIMB_MIN: f32 = 40.0;
/// ...once Mario falls this fast (SM64 units per tick: past the top of the last bounce)
const CLIMB_FALL: f32 = 12.0;
/// Each bounce off a big body lifts Mario this high (SM64 units), A held, up to its top and this
/// much over (so the last one lands him on the head).
const CLIMB_STEP: f32 = 180.0;
const CLIMB_STEP_A: f32 = 340.0;
const CLIMB_OVER: f32 = 120.0;
/// SM64's gravity in the air (units per tick per tick)
const GRAVITY: f32 = 4.0;
/// the twirl's reach around Mario, and a ground pound's shockwave's (SM64 units)
const SPIN_RADIUS: f32 = 150.0;
const SHOCK_RADIUS: f32 = 260.0;

/// The combos' reach (SM64 units): the hammer's head beyond its own size, the giant one's and the
/// slams' blows around where it struck, the spins around Mario
const HAMMER_HEAD_REACH: f32 = 150.0;
/// the Chain Chomp's body's (it shares the swept reach)
const CHOMP_REACH: f32 = 125.0;
const SMASH_RADIUS: f32 = 280.0;
const QUAKE_RADIUS: f32 = 380.0;
const QUAKE_BIG_RADIUS: f32 = 460.0;
const TITAN_RADIUS: f32 = 600.0;
const GIANT_RADIUS: f32 = 540.0;
/// a pound of Donkey Kong's drum around where it struck, by the hammer's size; the rising spin
/// kick a little in front of Mario
const POUND_RADIUS: f32 = 150.0;
const RISE_AHEAD: f32 = 60.0;
const RISE_RADIUS: f32 = 150.0;
const GIRO_RADIUS: f32 = 300.0;
const UPPERCUT_RADIUS: f32 = 150.0;
const METEOR_RADIUS: f32 = 130.0;
const METEOR_LAND_RADIUS: f32 = 340.0;
const TORNADO_RADIUS: f32 = 190.0;
/// the string past the kick and the air string reach in front of him like the punches, a bit
/// further; the shell all around him as he rolls
const STRING_REACH: f32 = 170.0;
const SHELL_RADIUS: f32 = 140.0;
/// the launcher catches whoever is around a point this far in front of Mario
const LAUNCH_AHEAD: f32 = 110.0;
const LAUNCH_RADIUS: f32 = 150.0;
/// the Chain Chomp's cyclone reaches all around Mario out to its chain, and its crash's shockwave
// (x1.25 past its place by him, as its chain in er_moves.c ER_CHOMP_LONGER)
const CHOMP_SPIN_RADIUS: f32 = 505.0;
const CHOMP_CRASH_RADIUS: f32 = 347.0;
/// the Mikiri's stomp around Mario, the dash strike's reach in front (SM64 units)
const MIKIRI_RADIUS: f32 = 170.0;
const BONK_REACH: f32 = 200.0;
/// the boulder's reach around Mario (its radius and a bit), its slam's
const ROCK_RADIUS: f32 = 85.0;
const ROCK_SLAM_RADIUS: f32 = 280.0;
/// the home run's swing in front (a bat's reach), the swing kick's all round him
const HOME_RUN_REACH: f32 = 230.0;
const SWING_KICK_RADIUS: f32 = 140.0;

/// The swept weapon's reach beyond its head (SM64 units): the hammer's, or the Chomp's.
fn head_reach() -> f32 {
    if crate::weapons::current() == crate::weapons::CHOMP { CHOMP_REACH } else { HAMMER_HEAD_REACH }
}

/// Who a combo's move hits this tick (moves::Hit): along the hammer's head's sweep (`swing`), or
/// all around a point.
fn combo_hits(state: &sm64::SM64MarioState, targets: &[([f32; 3], f32, f32, usize)], hit: crate::moves::Hit, swing: Option<crate::hammer::Swing>) -> Vec<(usize, Attack, bool)> {
    use crate::moves::Hit;
    use glam::Vec3;
    let me = Vec3::from(state.position);
    let head = swing.map_or(me, |s| s.head);
    let size = swing.map_or(1.0, |s| s.scale);
    // along the hammer head's sweep, or all around a point within a radius
    enum Reach {
        Sweep,
        Around(Vec3, f32),
        /// around a point, out to the radius and the swept body's reach beyond it
        Ring(Vec3, f32),
        Front(f32),
    }
    let (attack, reach) = match hit {
        Hit::Hammer => (Attack::Hammer, Reach::Sweep),
        Hit::HammerSmash => (Attack::HammerSmash, Reach::Sweep),
        Hit::SmashWave => (Attack::HammerSmash, Reach::Around(head, SMASH_RADIUS)),
        Hit::Quake => (Attack::Quake, Reach::Around(head, QUAKE_RADIUS)),
        Hit::QuakeBig => (Attack::QuakeBig, Reach::Around(head, QUAKE_BIG_RADIUS)),
        Hit::Titan => (Attack::Titan, Reach::Around(head, TITAN_RADIUS)),
        Hit::HammerWide => (Attack::HammerWide, Reach::Sweep),
        Hit::HammerRise => (Attack::HammerRise, Reach::Sweep),
        Hit::Pound => (Attack::Pound, Reach::Around(head, POUND_RADIUS * size)),
        Hit::Rise => (Attack::Rise, Reach::Around(me + Vec3::new(state.face_angle.sin(), 0.0, state.face_angle.cos()) * RISE_AHEAD, RISE_RADIUS)),
        Hit::HammerAir => (Attack::HammerAir, Reach::Sweep),
        Hit::Flurry => (Attack::Flurry, Reach::Sweep),
        Hit::FlurryEnd => (Attack::FlurryEnd, Reach::Around(head, GIANT_RADIUS)),
        Hit::Giro => (Attack::Giro, Reach::Around(me, GIRO_RADIUS * size.max(1.0))),
        Hit::GiroEnd => (Attack::GiroEnd, Reach::Around(me, GIRO_RADIUS + 30.0)),
        Hit::Uppercut => (Attack::Uppercut, Reach::Around(me, UPPERCUT_RADIUS)),
        Hit::UppercutTop => (Attack::UppercutTop, Reach::Around(me, UPPERCUT_RADIUS + 20.0)),
        Hit::Meteor => (Attack::Meteor, Reach::Around(me, METEOR_RADIUS)),
        Hit::MeteorLand => (Attack::MeteorLand, Reach::Around(me, METEOR_LAND_RADIUS)),
        Hit::Tornado => (Attack::Tornado, Reach::Around(me, TORNADO_RADIUS)),
        Hit::TornadoEnd => (Attack::TornadoEnd, Reach::Around(me, TORNADO_RADIUS + 40.0)),
        Hit::Chain => (Attack::Chain, Reach::Front(STRING_REACH)),
        Hit::ChainEnd => (Attack::ChainEnd, Reach::Around(me, TORNADO_RADIUS + 30.0)),
        Hit::Shell => (Attack::Shell, Reach::Around(me, SHELL_RADIUS)),
        Hit::Air => (Attack::AirHit, Reach::Front(STRING_REACH)),
        Hit::Launch => (Attack::Launch, Reach::Around(me + Vec3::new(state.face_angle.sin(), 0.0, state.face_angle.cos()) * LAUNCH_AHEAD, LAUNCH_RADIUS)),
        // the Chain Chomp: along its path (swing = its body), all around Mario in the cyclone, a
        // shockwave where it crashes
        Hit::Chomp | Hit::ChompAir => (if hit == Hit::Chomp { Attack::Chomp } else { Attack::ChompAir }, Reach::Sweep),
        Hit::ChompLaunch => (Attack::ChompLaunch, Reach::Sweep),
        Hit::ChompSpin => (Attack::ChompSpin, Reach::Ring(me, CHOMP_SPIN_RADIUS)),
        Hit::ChompEnd => (Attack::ChompEnd, Reach::Around(head, CHOMP_CRASH_RADIUS)),
        // the Mikiri lands on whoever is under him; the dash strike in front, a bit past a punch
        Hit::Mikiri => (Attack::Mikiri, Reach::Around(me, MIKIRI_RADIUS)),
        Hit::Bonk => (Attack::Bonk, Reach::Front(BONK_REACH)),
        Hit::Rock => (Attack::Rock, Reach::Around(me, ROCK_RADIUS)),
        Hit::RockSlam => (Attack::RockSlam, Reach::Around(me, ROCK_SLAM_RADIUS)),
        Hit::HomeRun => (Attack::HomeRun, Reach::Front(HOME_RUN_REACH)),
        Hit::SwingKick => (Attack::SwingKick, Reach::Around(me, SWING_KICK_RADIUS)),
    };
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let mut out = Vec::new();
    for &(t, radius, height, index) in targets {
        let foot = Vec3::from(t);
        let top = foot.y + height;
        let hit = match reach {
            Reach::Around(c, r) => {
                // (the super jump punch rises above them: anyone up to 1.5 m under his feet)
                flat(foot - c).length() <= r + radius && c.y - 150.0 <= top && c.y + 200.0 >= foot.y
            }
            Reach::Ring(c, r) => {
                let reach = swing.map_or(0.0, |s| head_reach() * s.scale);
                flat(foot - c).length() <= r + reach + radius && c.y - 150.0 <= top && c.y + 250.0 >= foot.y
            }
            Reach::Front(r) => {
                let d = flat(foot - me);
                let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
                let ahead = d.length() < 1.0 || (d.x * fx + d.z * fz) / d.length() >= PUNCH_CONE_COS;
                ahead && d.length() <= r + radius && me.y - 150.0 <= top && me.y + 200.0 >= foot.y
            }
            Reach::Sweep => {
                let Some(s) = swing else { continue };
                // the head's path this tick (flat), against the body's axis
                let (a, b) = (flat(s.prev), flat(s.head));
                let ab = b - a;
                let k = if ab.length_squared() > 1.0 { ((flat(foot) - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 1.0 };
                let at = s.prev.lerp(s.head, k);
                let near = flat(foot - at).length() <= head_reach() * s.scale + radius;
                let level = at.y >= foot.y - 80.0 * s.scale && at.y <= top + 80.0 * s.scale;
                // ...and someone hugging Mario, in front of him, inside the head's arc
                let close = {
                    let d = flat(foot - me);
                    let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
                    d.length() <= MARIO_RADIUS + radius + 40.0 && (d.x * fx + d.z * fz) >= 0.0 && me.y <= top && me.y + 150.0 >= foot.y
                };
                (near && level) || close
            }
        };
        if hit {
            out.push((index, attack, false));
        }
    }
    out
}

/// `no_punch`: Mario's punch is only the fireball's throw (fireball.rs), it hits no one.
/// `combo`, `swing`: what a combo's move did this tick (moves.rs), and the hammer's head.
pub fn hits(
    id: i32,
    state: &sm64::SM64MarioState,
    targets: &[([f32; 3], f32, f32, usize)],
    no_stomp: &[usize],
    no_punch: bool,
    combo: &sm64::SM64ErMove,
    swing: Option<crate::hammer::Swing>,
) -> Vec<(usize, Attack, bool)> {
    // the combos' moves hit only as they say (SM64's own checks took the dive for a dash, the
    // super jump punch for a stomp)
    // (the hook's pull hits no one on the way: hook.rs)
    if state.action == crate::moves::ACT_ER_ZIP {
        return Vec::new();
    }
    if crate::moves::combo(state.action) {
        return crate::moves::Hit::from(combo.hit).map_or_else(Vec::new, |hit| combo_hits(state, targets, hit, swing));
    }
    // (the Mikiri's stomp comes on its last tick: whatever he's doing after it)
    if crate::moves::Hit::from(combo.hit) == Some(crate::moves::Hit::Mikiri) {
        return combo_hits(state, targets, crate::moves::Hit::Mikiri, swing);
    }
    let mut out = Vec::new();
    let m = state.position;
    for &(t, radius, height, index) in targets {
        // stomped out: no hit and no bounce, Mario drops past
        if no_stomp.contains(&index) && matches!(classify(state), Attack::Stomp) {
            continue;
        }
        let (dx, dz) = (t[0] - m[0], t[2] - m[2]);
        let horizontal = (dx * dx + dz * dz).sqrt();
        let attack = classify(state);
        if no_punch && matches!(attack, Attack::Punch) {
            continue;
        }
        let level = m[1] <= t[1] + height && m[1] + 150.0 >= t[1];
        // the twirl hits everyone around him (SM64's only hits from above)
        if state.action == ACT_TWIRLING {
            if level && horizontal <= SPIN_RADIUS + radius {
                out.push((index, Attack::Spin, false));
            }
            continue;
        }
        // a ground pound's landing: the shockwave reaches those around the one under him
        if state.action == ACT_GROUND_POUND_LAND && horizontal > MARIO_RADIUS + radius {
            if level && horizontal <= SHOCK_RADIUS + radius {
                out.push((index, Attack::Shockwave, false));
            }
            continue;
        }
        // punches and kicks reach a bit further than SM64's (its hitbox is made for small
        // objects): up to PUNCH_REACH, in front of Mario (picking enemies up from behind was hard
        // to land)
        let in_front = horizontal > 1.0 && {
            let (fx, fz) = (state.face_angle.sin(), state.face_angle.cos());
            (dx * fx + dz * fz) / horizontal >= PUNCH_CONE_COS
        };
        let punch_reach = matches!(attack, Attack::Punch | Attack::Kick | Attack::Rush | Attack::RushKick) && in_front && horizontal <= PUNCH_REACH + radius;
        let body = if matches!(attack, Attack::Stomp) { STOMP_RADIUS } else { MARIO_RADIUS };
        // a big body (a giant beast, a dragon): anywhere on it is a foothold, and each bounce off
        // it lifts Mario higher, a leg up to a wing, up to the head (A held lifts him farther)
        if matches!(attack, Attack::Stomp) && height > STOMP_HEIGHT {
            if horizontal > body + radius || m[1] < t[1] + CLIMB_MIN || m[1] > t[1] + height + 60.0 || state.velocity[1] > -CLIMB_FALL {
                continue;
            }
            let rest = t[1] + height + CLIMB_OVER - m[1];
            let lift = |step: f32| (2.0 * GRAVITY * rest.clamp(100.0, step)).sqrt();
            unsafe { sm64::sm64_er_set_stomp(lift(CLIMB_STEP), lift(CLIMB_STEP_A)) };
            let hit = unsafe { sm64::sm64_mario_attack(id, t[0], m[1] - 1.0, t[2], 1.0) };
            unsafe { sm64::sm64_er_set_stomp(30.0, 30.0) };
            // the climb's reward: landed on the head (its top fifth), the stomp hits like a ground
            // pound (more damage, and its poise damage staggers the beast)
            let head = m[1] > t[1] + height * 0.8;
            if hit {
                out.push((index, if head { Attack::GroundPound } else { attack }, false));
            }
            continue;
        }
        if (horizontal > body + radius && !punch_reach) || m[1] > t[1] + height || m[1] + 100.0 < t[1] {
            continue;
        }
        // aim at mid-body: stomps need Mario above that (and bounce him to the top), on a body no
        // taller than STOMP_HEIGHT: a big beast's 5 m capsule left its back below "mid-body", so
        // Mario walked on it, and a stomp that landed threw him up to 5 m
        let height = if matches!(attack, Attack::Stomp) { height.min(STOMP_HEIGHT) } else { height };
        if unsafe { sm64::sm64_mario_attack(id, t[0], t[1] + height * 0.5, t[2], height * 0.5) } || punch_reach {
            out.push((index, attack, false));
        }
    }
    out
}

impl Combat {
    /// Game thread, co-op host: the guests' blows (coop.rs), taken off like Mario's own; the host
    /// ends the enemy as for his own (`finishing`).
    pub fn remote_hits(&mut self, tick: u32) {
        for h in crate::coop::take_hits() {
            if h.attack == IMPACT_CODE {
                if unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.chr_ins_by_handle(&h.handle)).is_some_and(|c| c.character_id == h.chr_id) {
                    impact(self, &h.handle, h.pct.clamp(0.0, 100.0), tick);
                }
                continue;
            }
            let what = Attack::from_code(h.attack).map_or(format!("code {}", h.attack), |a| format!("partner's {a:?}"));
            let found = unsafe { WorldChrMan::instance() }.ok().and_then(|w| w.chr_ins_by_handle(&h.handle)).map(|c| c.character_id);
            if found != Some(h.chr_id) {
                log(format!("combat: a {what} on c{:04} ({}), not here as {found:?}", h.chr_id, h.handle));
                continue;
            }
            // (thrown or lying limp, or a real blow finding out whether he can die: as for Mario's)
            if crate::swing::is_down(&h.handle) || hands_off(&h.handle) {
                continue;
            }
            let key = handle_key(&h.handle);
            self.victims.insert(key, (h.handle, tick));
            if take_pct(&h.handle, h.pct.clamp(0.0, 100.0), &what) {
                self.finishing.entry(key).or_insert((h.handle, tick));
            }
        }
    }

    /// Game thread: deals the damage for libsm64's hits. `tick` is the SM64 tick counter.
    pub fn deal(&mut self, player: &ChrIns, targets: &[Target], hits: &[(usize, Attack, bool)], tick: u32) {
        if !self.patched {
            self.patched = patch_params();
            if !self.patched {
                return;
            }
        }
        // (kept as long as the longest one, the boulder's)
        self.cooldown.retain(|_, t| tick.wrapping_sub(*t) < COOLDOWN_KEEP);
        {
            let mut until = FINISHER_UNTIL.lock().unwrap_or_else(|e| e.into_inner());
            if until.is_some_and(|u| tick.wrapping_sub(u) < u32::MAX / 2) {
                if let Some(a) = (unsafe { SoloParamRepository::instance_mut() }).ok().and_then(|r| r.get_mut::<AtkParam_Pc>(Attack::Punch.spec().1)) {
                    a.set_atk_phys(BULLET_DAMAGE);
                    a.set_atk_phys_correction(FIST_SHARE);
                }
                *until = None;
            }
        }
        // the final blow is the bullet's; if it can't land (some small or odd characters), finish
        // them after 0.5 s
        self.finishing.retain(|key, (handle, t)| {
            let Some(chr) = unsafe { WorldChrMan::instance_mut() }.ok().and_then(|w| w.chr_ins_by_handle_mut(handle)) else { return false };
            // (stopped in time: it's finished once time runs again)
            if crate::stopwatch::frozen(handle) {
                *t = tick;
                return true;
            }
            // (2 only while a real blow is on its way, see below)
            let probing = PROBES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|p| p.get(key).is_some_and(|p| p.fired.is_some()));
            if chr.modules.data.hp != 1 && !(probing && chr.modules.data.hp == 2) {
                PROBES.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(|p| p.remove(key));
                return false;
            }
            // a boss whose script takes over at his last bit of health (the God-Devouring Serpent
            // turning into Rykard: forced animation, then the cutscene) is never finished by hand,
            // he'd die in the middle of it and the next phase never came
            if is_boss(handle) {
                if scripted(chr) {
                    log(format!("combat: c{:04} is in a scripted animation at his last point, left to the game", chr.character_id));
                    SCRIPTED.lock().unwrap_or_else(|e| e.into_inner()).push(*key);
                    return false;
                }
                // Nor while a cutscene plays or the world stands still, and the wait starts over
                // after it: Malenia's second phase comes as a cutscene, set off by a hit on her
                // at her last point (which the script keeps her at), with no animation to tell.
                if crate::CUTSCENE_HIDE.load(std::sync::atomic::Ordering::Relaxed) || crate::WORLD_PAUSED.load(std::sync::atomic::Ordering::Relaxed) {
                    *t = tick;
                    return true;
                }
                // (the script reacts within a few frames: wait a little longer than for the rest)
                if tick.wrapping_sub(*t) < BOSS_FINISH_TICKS {
                    return true;
                }
                // (not while he's thrown or lying there: no hit reaches him then, and the blows
                // below would count as missed)
                if crate::swing::busy_with(handle) || crate::swing::is_down(handle) {
                    *t = tick.wrapping_sub(BOSS_FINISH_TICKS - 30);
                    return true;
                }
                // Still there: the hit that took him there wasn't one of the game's (a throw's
                // impact, or a blow that didn't connect), or the game
                // won't let him die yet (Malenia is kept at her last point by her script until
                // her second phase is set off; ended by hand she just died). Told apart by a
                // real blow with a point of health to spare: it's gone again if the blow landed,
                // and he's still alive only if he can't die.
                let mut probes = PROBES.lock().unwrap_or_else(|e| e.into_inner());
                let probe = probes.get_or_insert_with(HashMap::new).entry(*key).or_default();
                let hp = chr.modules.data.hp;
                if let Some(fired) = probe.fired {
                    if tick.wrapping_sub(fired) < PROBE_TICKS {
                        return true;
                    }
                    probe.fired = None;
                    if hp >= 2 {
                        probe.misses += 1;
                        chr.modules.data.hp = 1;
                        log(format!("combat: the blow didn't land on c{:04} ({} of {PROBE_MISSES}), {hp} HP", chr.character_id, probe.misses));
                    } else if !probe.undying {
                        probe.undying = true;
                        log(format!("combat: c{:04} took a real blow at his last point and lives: the game is keeping him, never finished by hand", chr.character_id));
                    }
                    // (the next one soon after a miss, every 2 s on one who can't die)
                    *t = tick.wrapping_sub(BOSS_FINISH_TICKS - if probe.undying { 60 } else { 15 });
                    return true;
                }
                if probe.misses < PROBE_MISSES {
                    if crate::debug() {
                        let effects: Vec<i32> = chr.special_effect.entries().map(|e| e.param_id).collect();
                        log(format!("combat: c{:04} at his last point, effects {effects:?}", chr.character_id));
                    }
                    chr.modules.data.hp = 2;
                    let at = chr.modules.physics.position;
                    let result = finisher(player, &HavokPosition(at.0, at.1 - 0.6, at.2, 0.0), tick);
                    log(format!("combat: a real blow on c{:04}: {result}", chr.character_id));
                    probe.fired = Some(tick);
                    return true;
                }
                probes.as_mut().map(|p| p.remove(key));
            }
            // (a boss Mario has grabbed or thrown: the throw's impact deals the final blow)
            if tick.wrapping_sub(*t) < 15 || crate::swing::busy_with(handle) {
                return true;
            }
            chr.modules.data.hp = 0;
            log(format!("combat: final blow missed, finished directly (c{:04})", chr.character_id));
            false
        });
        // (no blow grabs: enemies are taken only with L2 + R3, carry::grab_nearest; a dive or a
        // running punch that lifted one instead of hitting it got in the way of the combos)
        for &(index, attack, _) in hits {
            let Some(target) = targets.get(index) else { continue };
            // (every bounce counts, also within the cooldown: quick climbing bounces did too)
            // (a head stomp at the top of a climb comes as a ground pound: it counts too)
            if matches!(attack, Attack::Stomp | Attack::GroundPound) {
                *self.stomps.entry(target.key).or_insert(0) += 1;
            }
            // (the trap kit and the hook's flights keep their own pace, apart from Mario's blows:
            // traps.rs, bat.rs)
            let own_pace = matches!(attack, Attack::Slip | Attack::ShellHit | Attack::Bomb | Attack::Boing | Attack::Bowled | Attack::Tether);
            if !own_pace && self.cooldown.get(&target.key).is_some_and(|t| tick.wrapping_sub(*t) < attack.cooldown()) {
                continue;
            }
            // (the hook's tug leaves the next blow free: it's what it's for; the giant fireball's
            // burn and Mega's steps come at their own pace and leave Mario's blows free)
            if !own_pace && !matches!(attack, Attack::Hook | Attack::FireSweep | Attack::Ember | Attack::Trample | Attack::VenomCloud) {
                self.cooldown.insert(target.key, tick);
            }
            if let Some(handle) = &target.handle {
                // a thrown boss in the air or lying limp: no hit at all (not even the game's own,
                // which could finish a ragdolled character)
                if crate::swing::is_down(handle) || crate::carry::is_carried(handle) {
                    continue;
                }
                // the super jump punch lifts small enemies, and stunned strong ones not too big, up
                // with Mario (juggle.rs); a blow on one up there keeps it up
                // (the hammer's launcher sends them flying high up instead, for Mario to jump after)
                let me = player.modules.physics.position;
                let me = glam::Vec3::new(me.0, me.1, me.2);
                // stopped in time (stopwatch.rs): the blow's momentum builds up, nothing moves it
                let stopped = crate::stopwatch::frozen(handle);
                if stopped {
                    let heavy = attack.finisher() || matches!(attack, Attack::GroundPound | Attack::Quake | Attack::HammerSmash | Attack::Hammer | Attack::HammerWide | Attack::Bonk | Attack::SwingKick);
                    crate::stopwatch::struck(handle, me, glam::Vec3::new(target.er.0, target.er.1, target.er.2), heavy);
                } else if matches!(attack, Attack::Uppercut | Attack::UppercutTop | Attack::Launch | Attack::RangUpper | Attack::ChompLaunch | Attack::Rise | Attack::HammerRise) && !hands_off(handle) {
                    let regular = liftable(handle);
                    let stunned = !regular && (crate::swing::is_open(handle) || crate::swing::toughness_of(handle).is_some_and(|(now, max)| max > 0.0 && now <= 0.0));
                    let high = matches!(attack, Attack::Launch | Attack::RangUpper | Attack::ChompLaunch);
                    let (radius_m, height_m) = (target.radius * crate::scale(), target.height * crate::scale());
                    if !crate::juggle::try_launch(handle, me, radius_m, height_m, regular, stunned, high) && !crate::coop::guest() {
                        log_not_lifted(handle, radius_m, height_m, stunned);
                    }
                } else if matches!(attack, Attack::Hook) {
                    // (the hook drags it in itself: hook.rs, juggle::pull)
                } else if matches!(attack, Attack::Rang | Attack::RangBig | Attack::Fire(_) | Attack::Ice(_) | Attack::Dark(_) | Attack::Venom(_) | Attack::VenomCloud | Attack::FireSweep | Attack::Ember | Attack::Trample | Attack::Egg | Attack::Tongue | Attack::Slip | Attack::ShellHit | Attack::Bomb | Attack::Boing | Attack::Bowled | Attack::Tether) {
                    // (from afar: an enemy up in the air stays up where it is, Devil May Cry's guns)
                    crate::juggle::shot(handle);
                } else {
                    crate::juggle::hit(handle, me);
                }
                self.victims.insert(target.key, (*handle, tick));
                // (a big body only dents and springs back: a giant flattened under him looked wrong,
                // and the climb's head stomp comes as a ground pound)
                let big = target.height > STOMP_HEIGHT || target.radius > 120.0;
                match attack {
                    Attack::Stomp => crate::squish::start(handle, 0.35, 0.0),
                    Attack::GroundPound | Attack::Quake | Attack::QuakeBig | Attack::Titan | Attack::FlurryEnd | Attack::HammerSmash if big => crate::squish::start(handle, 0.35, 0.0),
                    Attack::GroundPound | Attack::Quake | Attack::QuakeBig | Attack::Titan | Attack::FlurryEnd | Attack::HammerSmash => crate::squish::start(handle, 0.8, 1.0),
                    _ => {}
                }
                // the Bowser spin's release flings regular enemies away, like the giant fireball
                if matches!(attack, Attack::GiroEnd) && blowable(handle) {
                    let me = player.modules.physics.position;
                    crate::knock::launch(handle, glam::Vec3::new(target.er.0 - me.0, 0.0, target.er.2 - me.2));
                }
                // stomps in a row on the same body (the head stomp's ground pound too): each worth
                // less until he's back on the ground (bouncing up a big one was a lot of free damage)
                let scale = if matches!(attack, Attack::Stomp | Attack::GroundPound) {
                    let run = self.stomp_run.entry(target.key).or_insert(0);
                    *run += 1;
                    STOMP_FADE.powi(*run as i32 - 1).max(STOMP_FLOOR)
                } else {
                    1.0
                };
                // a long string on the same enemy: past STRING_FULL blows each is worth a little
                // less (a finisher less so), so long and showy combos don't melt it in a second
                if self.string.len() > 64 {
                    self.string.retain(|_, r| tick.wrapping_sub(r.1) <= STRING_GAP);
                }
                let run = self.string.entry(target.key).or_insert((0.0, tick));
                if tick.wrapping_sub(run.1) > STRING_GAP {
                    run.0 = 0.0;
                }
                // (the giant fireball's burn and Mega's steps aren't blows of a string)
                let before = run.0;
                if !matches!(attack, Attack::FireSweep | Attack::Ember | Attack::Trample | Attack::VenomCloud) {
                    run.0 += (attack.percent().0 / STRING_UNIT).clamp(STRING_WEIGHT.0, STRING_WEIGHT.1);
                    run.1 = tick;
                }
                let fade = STRING_FADE.powf((run.0 - STRING_FULL).max(0.0)).max(STRING_FLOOR);
                let fade = if attack.finisher() { fade.max(STRING_FINISHER_FLOOR) } else { fade };
                // (the Bros. Attack comes out of a string on it, BROS! by BROS!: it lands whole)
                let fade = if matches!(attack, Attack::BrosAttack) { 1.0 } else { fade };
                if fade < 1.0 && (before / 5.0).floor() != (run.0 / 5.0).floor() {
                    log(format!("combat: string of {:.0} punches' worth on the same enemy, blows at {:.0}%", run.0, fade * 100.0));
                }
                // (co-op guest: the health here is only a preview, never the last point: the host
                // takes it off and ends the enemy)
                // (the hammer's heavy blows break a strong enemy's stance, swing.rs)
                if attack.posture() > 0.0 && !liftable(handle) && !crate::coop::guest() {
                    crate::swing::add_stance(handle, attack.posture());
                }
                let took = take_share(handle, attack, scale * fade);
                if took == Some(true) && !crate::coop::guest() {
                    if !self.finishing.contains_key(&target.key) {
                        crate::style::kill(attack.finisher());
                    }
                    self.finishing.entry(target.key).or_insert((*handle, tick));
                }
                let chest = glam::Vec3::new(target.er.0, target.er.1 + (target.height * crate::scale()).min(3.0) * 0.5, target.er.2);
                // the weapon's element (element.rs): its status builds up on him, and goes off (only
                // from a blow that took something)
                if took.is_some() {
                    if let Some(status) = crate::element::struck(handle, attack, chest, tick) {
                        self.extra_share(handle, status, 1.0, tick);
                    }
                }
                // (what hit it last, and how it was: up in the air or not)
                {
                    let airborne = crate::juggle::is_juggled(handle) || crate::bat::is_flying(handle) || crate::knock::is_flying(handle);
                    if self.last_blow.len() > 64 {
                        let live: Vec<u64> = self.victims.keys().copied().collect();
                        self.last_blow.retain(|k, _| live.contains(k));
                    }
                    self.last_blow.insert(target.key, (attack, crate::skillshot::How { airborne }));
                }
                // a finisher with the MAX ready (twelve blows without a hit): its MAX version
                // (maxfin.rs; the boomerang's is its big throw, boomerang.rs)
                if attack.finisher() && !attack.ranged() && self.max.is_none() && crate::weapons::current() != crate::weapons::RANG && crate::style::take_max() {
                    self.max = Some((attack, chest));
                }
                // the action command: X right on the blow (timing.rs)
                if let Some(extra) = crate::timing::landed(handle, attack, chest, tick) {
                    self.extra_share(handle, attack, extra * fade, tick);
                }
                // the Mikiri: the world slows a moment, and a boss's stance cracks (swing.rs: a
                // few of them open him up for the tail grab)
                // the home run: batted away along the camera (bat.rs)
                if attack == Attack::HomeRun {
                    let me = player.modules.physics.position;
                    let dir = crate::lakitu::forward()
                        .map(|f| f.with_y(0.0))
                        .filter(|f| f.length() > 0.3)
                        .unwrap_or(glam::Vec3::new(target.er.0 - me.0, 0.0, target.er.2 - me.2));
                    crate::bat::home_run(handle, dir);
                }
                // the boulder: off a big one it bounces, a regular one counts for the strike
                if attack == Attack::Rock {
                    let me = player.modules.physics.position;
                    let big = !liftable(handle) || target.radius * crate::scale() > 0.9;
                    crate::rock::rolled_into(target.key, glam::Vec3::new(target.er.0, target.er.1 + 0.6, target.er.2), big, glam::Vec3::new(me.0, me.1, me.2));
                }
                if attack == Attack::Mikiri && crate::timing::mikiri_landed() {
                    crate::style::witch_time(crate::timing::MIKIRI_SLOW);
                    crate::style::callout("MIKIRI!", Some(chest), [0.7, 0.9, 1.0], true, crate::style::MIKIRI_POINTS);
                    crate::impact::mikiri(chest + glam::Vec3::Y * 0.4);
                    if !liftable(handle) {
                        crate::swing::add_stance(handle, crate::timing::MIKIRI_POSTURE);
                    }
                }
            }
            let (bullet, ..) = attack.spec();
            if !target.is_prop() && !matches!(attack, Attack::FireSweep | Attack::Ember | Attack::Trample | Attack::VenomCloud) {
                crate::style::hit(attack, target.key);
                // (co-op: both on one enemy is a BROS!)
                crate::coop::own_hit(target.key, glam::Vec3::new(target.er.0, target.er.1 + (target.height * crate::scale()).min(3.0) * 0.6, target.er.2));
            }
            // (breaking a jar or a crate is no blow to stop for)
            if !matches!(attack, Attack::Fire(_) | Attack::Ice(_) | Attack::Dark(_) | Attack::Venom(_) | Attack::VenomCloud | Attack::FireSweep | Attack::Ember) && !target.is_prop() {
                let me = player.modules.physics.position;
                let at = glam::Vec3::new(target.er.0, target.er.1 + (target.height * crate::scale()).min(3.0) * 0.5, target.er.2);
                crate::impact::hit(at, glam::Vec3::new(me.0, me.1 + 0.6, me.2), attack);
                if attack.finisher() && target.handle.is_some() {
                    crate::style::finisher(at);
                }
            }
            // (the game gives the player only ~256 bullets a session: their emitters are never freed,
            // and once they're gone no blow of Mario's is the game's any more, enemies neither react
            // nor wake. So a bullet only where it matters: a heavy blow or a finisher, else at most
            // one per enemy every BULLET_EVERY ticks; none for the burn, the steps, the cyclone,
            // the hook or the tongue)
            let light = matches!(attack, Attack::FireSweep | Attack::Ember | Attack::Trample | Attack::ChompSpin | Attack::Hook | Attack::Tongue | Attack::VenomCloud);
            let heavy = attack.finisher() || matches!(attack, Attack::GroundPound | Attack::Quake | Attack::MeteorLand | Attack::Launch | Attack::UppercutTop | Attack::Fire(3..) | Attack::Ice(3..) | Attack::Dark(3..) | Attack::Venom(3..) | Attack::Mikiri | Attack::Bonk | Attack::Slip | Attack::ShellHit | Attack::Bomb | Attack::RockSlam | Attack::HomeRun | Attack::Bowled | Attack::Tether | Attack::SwingKick | Attack::RideCrash | Attack::BrosAttack | Attack::Sigil);
            let due = self.bullet_at.get(&target.key).is_none_or(|&t| tick.wrapping_sub(t) >= BULLET_EVERY);
            if light || !(heavy || due) {
                continue;
            }
            self.bullet_at.insert(target.key, tick);
            if self.bullet_at.len() > 64 {
                self.bullet_at.retain(|_, t| tick.wrapping_sub(*t) < BULLET_EVERY);
            }
            // the banana, the shells, the boulder, the hook's crashes knock a regular enemy down
            // (the game's own knockdown: a bullet of its own)
            // (and the hammer's heaviest blows: the smash, the charged quakes, the giant one)
            let knock = matches!(attack, Attack::Slip | Attack::ShellHit | Attack::Rock | Attack::Bowled | Attack::Tether | Attack::HammerSmash | Attack::QuakeBig | Attack::Titan | Attack::FlurryEnd)
                && target.handle.as_ref().is_some_and(liftable)
                && self.knocked.get(&target.key).is_none_or(|&t| tick.wrapping_sub(t) >= KNOCK_EVERY);
            if knock {
                if self.knocked.len() > 64 {
                    self.knocked.retain(|_, t| tick.wrapping_sub(*t) < KNOCK_EVERY);
                }
                self.knocked.insert(target.key, tick);
            }
            let bullet = if knock { KNOCK_BULLET.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(bullet) } else { bullet };
            let result = strike(player, &target.er, bullet);
            log(format!("combat: {attack:?} hit -> bullet {bullet}: {result}"));
        }
    }
}

impl Combat {
    /// A MAX finisher that just landed (its blow, where), for maxfin.rs.
    pub fn take_max(&mut self) -> Option<(Attack, glam::Vec3)> {
        self.max.take()
    }

    /// Game thread: a bit more of the blow `attack` on `handle` (`extra` of its share: the action
    /// command's bonus, timing.rs; or a status going off, element.rs), with its own kill if it's the
    /// one that ends him.
    pub fn extra_share(&mut self, handle: &FieldInsHandle, attack: Attack, extra: f32, tick: u32) {
        if crate::swing::is_down(handle) || crate::carry::is_carried(handle) {
            return;
        }
        let key = handle_key(handle);
        // (already at his last point: the blow that ends him is coming, nothing to add)
        if self.finishing.contains_key(&key) {
            return;
        }
        self.victims.insert(key, (*handle, tick));
        if take_share(handle, attack, extra) == Some(true) && !crate::coop::guest() {
            if !self.finishing.contains_key(&key) {
                crate::style::kill(attack.finisher());
            }
            self.finishing.entry(key).or_insert((*handle, tick));
        }
    }
}

/// One of Mario's hits as the game sees it: a bullet of his, 1 m above `p`.
/// A bullet of the player's at `p` (noise.rs's noise; Mario's blows go through strike).
pub fn shoot(player: &ChrIns, p: &HavokPosition, bullet: i32) -> String {
    strike(player, p, bullet)
}

fn strike(player: &ChrIns, p: &HavokPosition, bullet: i32) -> String {
    {
        {
            let from = player.modules.physics.position;
            let dir = glam::Vec3::new(p.0 - from.0, 0.0, p.2 - from.2).normalize_or(glam::Vec3::Z);
            let request = SpawnRequest {
                owner: player.field_ins_handle.clone(),
                behavior_id: -1,
                magic_id: -1,
                unk10: 0,
                bullet_id: bullet,
                goods_id: -1,
                dummy_poly_id: -1,
                target: [0xFF; 8],
                unk28: 0,
                unk2c: 0,
                unk30: F32Vector4(p.0, p.1 + 1.0, p.2, 0.0),
                unk40: 0,
                unk44: 0,
                pad48: [0; 8],
                acceleration_angle: F32Vector4(dir.x, dir.y, dir.z, 0.0),
                unk60: F32Vector4(0.0, 0.0, 0.0, 0.0),
                angle: F32Vector4(dir.x, dir.y, dir.z, 0.0),
                position: F32Vector4(p.0, p.1 + 1.0, p.2, 0.0),
                rest: [0; 0x80],
            };
            let Ok(manager) = (unsafe { CSBulletManager::instance_mut() }) else { return "no bullet manager".into() };
            let result = manager.spawn_bullet(unsafe { &*(&request as *const SpawnRequest as *const _) });
            // (it failed: what's alive in the game's bullet pool, whose, and how old)
            if result.is_err() {
                static FAILS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                if FAILS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 50 == 0 {
                    let (mut n, mut mine, mut oldest) = (0, 0, 0.0f32);
                    for b in manager.bullets() {
                        n += 1;
                        if handle_key(&b.targeting_owner.owner_chr_handle) == handle_key(&player.field_ins_handle) {
                            mine += 1;
                            oldest = oldest.max(b.time_alive);
                        }
                    }
                    log(format!("combat: bullet spawn failed: {n} bullets alive, {mine} the player's (oldest {oldest:.1} s)"));
                }
            }
            format!("{result:?}")
        }
    }
}

impl Combat {

    /// Game thread, every tick: the targets Mario can't stomp again before landing (indexes into
    /// `targets`); landing resets the count.
    /// Bounces in a row on a big body since Mario last stood on the ground (the camera backs off
    /// with each, lakitu.rs).
    pub fn climb(&self, targets: &[Target]) -> u32 {
        targets.iter().filter(|t| t.height > STOMP_HEIGHT).filter_map(|t| self.stomps.get(&t.key)).copied().max().unwrap_or(0)
    }

    pub fn stomp_limits(&mut self, targets: &[Target], grounded: bool) -> Vec<usize> {
        if grounded {
            self.stomps.clear();
            self.stomp_run.clear();
        }
        targets
            .iter()
            .enumerate()
            .filter(|(_, t)| self.stomps.get(&t.key).is_some_and(|&n| n >= if t.height > STOMP_HEIGHT { MAX_CLIMB } else { MAX_STOMPS }))
            .map(|(i, _)| i)
            .collect()
    }

    /// Game thread: a blast on the ground at `feet` (Elden Ring) reaching `reach` m: every enemy
    /// standing in it (feet within 1.5 m up or down) takes `attack`. Mega Mario's steps and landings,
    /// the giant fireball's explosion.
    /// A finishing blast (the explosion, a landing) lands whatever hit came just before; Mega's steps
    /// hurt each enemy at most every TRAMPLE_EVERY ticks (walking over one mustn't stunlock it).
    pub fn blast(&mut self, player: &ChrIns, feet: glam::Vec3, reach: f32, attack: Attack, origin: [f32; 3], tick: u32) -> usize {
        let center = HavokPosition(feet.x, feet.y, feet.z, 0.0);
        let near = nearby(&center, reach + 4.0, origin);
        let hits: Vec<(usize, Attack, bool)> = near
            .iter()
            .enumerate()
            .filter(|(_, t)| t.shootable().is_some())
            .filter(|(_, t)| {
                let p = glam::Vec3::new(t.er.0, t.er.1, t.er.2);
                let r = t.radius * crate::scale();
                let h = t.height * crate::scale();
                (p - feet).with_y(0.0).length() <= reach + r && feet.y + 1.5 >= p.y && feet.y - 1.5 <= p.y + h
            })
            .filter(|(_, t)| attack != Attack::Trample || self.trampled.get(&t.key).is_none_or(|&at| tick.wrapping_sub(at) >= TRAMPLE_EVERY))
            .map(|(i, _)| (i, attack, false))
            .collect();
        for &(i, ..) in &hits {
            if attack == Attack::Trample {
                self.trampled.insert(near[i].key, tick);
            } else {
                self.cooldown.remove(&near[i].key);
            }
        }
        self.trampled.retain(|_, at| tick.wrapping_sub(*at) < TRAMPLE_EVERY);
        if !hits.is_empty() {
            self.deal(player, &near, &hits, tick);
        }
        hits.len()
    }

    /// Game thread: how the Tarnished's HP loss since the last check turns into Mario's health
    /// (his HP is refilled right after, so SM64's power meter is what counts): a real hit costs
    /// half a wedge or a whole one by its size with SM64's knockback (a whole one, or two, on a
    /// boss was too hard), poison / bleed / rot ticks drain a wedge per 10%.
    pub fn took_damage(&mut self, hp: i32, max: i32, scale: f32) -> Option<Hurt> {
        let lost = self.last_hp.map(|old| old - hp).unwrap_or(0);
        self.last_hp = Some(max);
        if lost <= 0 {
            return None;
        }
        // (Mega Mario shrugs off half of it)
        let frac = lost as f32 / max.max(1) as f32 * scale * if crate::mega::active() { crate::mega::HURT } else { 1.0 };
        if frac >= 0.04 {
            return Some(Hurt::Hit(if frac < 0.25 { 1 } else { 2 }));
        }
        self.drain += frac;
        if self.drain >= 0.1 {
            self.drain -= 0.1;
            return Some(Hurt::Drain);
        }
        None
    }

    /// Game thread, every SM64 tick: where the characters Mario hit in the last 10 s died, for
    /// those that died since (each drops a coin).
    pub fn kills(&mut self, tick: u32) -> Vec<glam::Vec3> {
        let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
        let mut out = Vec::new();
        let last_blow = &mut self.last_blow;
        self.victims.retain(|key, (handle, t)| match wcm.chr_ins_by_handle(handle) {
            Some(chr) if chr.modules.data.hp <= 0 => {
                let p = chr.modules.physics.position;
                let at = glam::Vec3::new(p.0, p.1, p.2);
                out.push(at);
                // a creative kill gets its name (skillshot.rs)
                if let Some((attack, how)) = last_blow.remove(key) {
                    crate::skillshot::kill(attack, how, at);
                }
                false
            }
            Some(_) => tick.wrapping_sub(*t) < 300,
            None => false,
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Attack, CODES};

    #[test]
    fn attack_codes_round_trip() {
        for a in CODES.iter().copied().chain((0..=5).map(Attack::Fire)) {
            assert_eq!(Attack::from_code(a.code()), Some(a), "{a:?}");
        }
        assert_eq!(Attack::from_code(u8::MAX), None);
        assert_eq!(Attack::from_code(CODES.len() as u8), None);
    }
}
