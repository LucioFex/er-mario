/**
 * er-mario: the combos' moves (moves.rs starts them, combat.rs and hammer.rs read what they do).
 *
 * Bare-handed: the super jump punch (X, X, Y), its meteor dive (X in it) and the Fire Palm (Y, X).
 * With the hammer (L1, er_hammer_mode): X's three swings, Y's slam (in the air: the plunge), the
 * Donkey Kong flurry (X, X, Y), the Golf Swing (Y, X) and a spinning swing in the air.
 *
 * Every tick each move says what it hits (er_move_hit: the mod measures who is in reach, around
 * Mario or along the hammer's head), what to show (er_move_fx) and, with the hammer, where it
 * points (er_hammer_*: degrees around Mario's facing, positive to his left, and up from level).
 */

#include <math.h>

#include "../include/PR/ultratypes.h"
#include "../shim.h"

#include "../include/sm64.h"
#include "../audio/external.h"
#include "../engine/math_util.h"
#include "interaction.h"
#include "mario.h"
#include "mario_step.h"
#include "mario_actions_object.h"
#include "../include/mario_animation_ids.h"
#include "../include/object_fields.h"

/* (same list as moves.rs Hit) */
enum ErHit {
    ER_HIT_NONE,
    ER_HIT_HAMMER,
    ER_HIT_HAMMER_SMASH,
    ER_HIT_SMASH_WAVE,
    ER_HIT_QUAKE,
    ER_HIT_FLURRY,
    ER_HIT_FLURRY_END,
    ER_HIT_GOLF_DRAG,
    ER_HIT_GOLF,
    ER_HIT_HAMMER_AIR,
    ER_HIT_UPPERCUT,
    ER_HIT_UPPERCUT_TOP,
    ER_HIT_METEOR,
    ER_HIT_METEOR_LAND,
    ER_HIT_PALM,
    ER_HIT_PALM_BURST,
    ER_HIT_CHAIN,
    ER_HIT_CHAIN_END,
    ER_HIT_SHELL,
    ER_HIT_AIR,
    ER_HIT_LAUNCH,
    ER_HIT_CHOMP,
    ER_HIT_CHOMP_SPIN,
    ER_HIT_CHOMP_END,
    ER_HIT_CHOMP_AIR,
    ER_HIT_CHOMP_LAUNCH,
    ER_HIT_MIKIRI,
    ER_HIT_BONK,
    ER_HIT_ROCK,
    ER_HIT_ROCK_SLAM,
    ER_HIT_HOME_RUN,
    ER_HIT_SWING_KICK,
    ER_HIT_HAMMER_WIDE,
    ER_HIT_QUAKE_BIG,
    ER_HIT_TITAN,
    ER_HIT_POUND,
    ER_HIT_RISE,
    ER_HIT_HAMMER_RISE,
};

/* (same bits as moves.rs FX_*) */
#define ER_FX_GROUND  0x01 /* the hammer hits the ground: dust and a small ring */
#define ER_FX_QUAKE   0x02 /* the slam: a wide ring, dust, stars */
#define ER_FX_GIANT   0x04 /* the giant hammer's blow: the widest ring, more stars */
#define ER_FX_METEOR  0x08 /* the meteor dive lands: ring and stars around Mario */
#define ER_FX_SPIN    0x10 /* a spin's gust around Mario */
#define ER_FX_LAUNCH  0x20 /* a jump off the ground: puffs at his feet */
#define ER_FX_BANG    0x40 /* a flurry blow: a puff at the hammer's head */
#define ER_FX_GROW    0x80 /* the hammer grows giant: sparkles on it */
#define ER_FX_CHARGE  0x100 /* charging: a glow at the hammer's head, dust at his feet */
#define ER_FX_THROW   0x200 /* a throw lets go (er_move_throw says what) */
#define ER_FX_SLAM    0x400 /* a heavy hammer blow in the ground: a ring, dust, rubble */

int er_hammer_mode = 0;
/** the weapon in hand (weapons.rs): 0 fists, 1 hammer, 2 boomerang, 3 Chain Chomp */
#define ER_WEAPON_RANG 2
#define ER_WEAPON_CHOMP 3
int er_weapon = 0;
/** Mega Mario (mega.rs): giant, slow, heavy */
int er_mega = 0;
/** boomerangs in hand (not out flying): a throw needs one */
int er_rang_ready = 0;
/** a throw let go this tick: its kind + 1 (ER_THROW_*), and how long it was charged */
int er_move_throw = 0;
int er_move_charge = 0;
/** where the Chain Chomp is (chomp.rs): degrees around Mario's facing (positive to his left), how
 * far out and how high over his feet (SM64 units), its mouth open (0-1) */
int er_chomp_scripted = 0;
f32 er_chomp_yaw = 0.0f;
f32 er_chomp_reach = 0.0f;
f32 er_chomp_height = 0.0f;
f32 er_chomp_bite = 0.0f;
/** an enemy is up in the air with Mario (juggle.rs): his air blows keep him up */
int er_juggle = 0;
int er_move_hit = 0;
u32 er_move_fx = 0;
int er_hammer_scripted = 0;
f32 er_hammer_yaw = 0.0f;
f32 er_hammer_pitch = 0.0f;
f32 er_hammer_scale = 1.0f;

s32 lava_boost_on_wall(struct MarioState *m);

/* the throws (actionArg): the boomerang's (one, three in a fan, the launcher, the big one, charged
 * with Y held) and the fireball's (its charge levels) */
#define ER_THROW_RANG 0
#define ER_THROW_STORM 1
#define ER_THROW_UPPER 2
#define ER_THROW_BIG 3
#define ER_THROW_FIRE 4
#define ER_THROW_FIRE_MID 5
#define ER_THROW_FIRE_BIG 6
#define ER_THROW_FIRE_GIANT 7
/* the hook (hook.rs): a quick fling of the arm, the hook out of his hand */
#define ER_THROW_HOOK 8
/* the trap kit (traps.rs): a banana tossed in front, a shell kicked, a Bob-omb lobbed, a spring
 * set down */
#define ER_THROW_BANANA 9
#define ER_THROW_SHELL 10
#define ER_THROW_BOMB 11
#define ER_THROW_SPRING 12
/** throws in the air since he last stood on the ground (the first few hold him up, DMC's guns) */
static s32 sErAirThrows = 0;
/** enemy steps since he last stood on the ground (A off an enemy up there, Devil May Cry's) */
static s32 sErSteps = 0;
#define ER_ENEMY_STEPS 3
#define ER_AIR_THROW_HANG 6
#define ER_AIR_THROWS 10

static u32 er_air_attack(void);

/** the dash strike's wait before the next one (ticks, act_er_bonk) */
static s32 sErBonkWait = 0;

/** the spin of the spinning moves (only one Mario) */
static s16 sErSpin = 0;

#define ER_PI 3.14159265f

static f32 er_lerp(f32 a, f32 b, f32 t) {
    if (t < 0.0f) {
        t = 0.0f;
    } else if (t > 1.0f) {
        t = 1.0f;
    }
    return a + (b - a) * t;
}

static f32 er_ease_out(f32 t) {
    if (t > 1.0f) {
        t = 1.0f;
    }
    return 1.0f - (1.0f - t) * (1.0f - t);
}

static void er_hammer(f32 yaw, f32 pitch, f32 scale) {
    er_hammer_scripted = 1;
    er_hammer_yaw = yaw;
    er_hammer_pitch = pitch;
    er_hammer_scale = scale;
}

/** Degrees to an SM64 angle, any number of turns (a float past an s16's range can't be cast to
 * it straight). */
static s16 er_angle(f32 deg) {
    return (s16)(s32)(deg * 65536.0f / 360.0f);
}

/** An SM64 angle to degrees (-180..180). */
static f32 er_degrees(s16 angle) {
    return (f32) angle * 360.0f / 65536.0f;
}

/** From `a` to `b` degrees the short way round, a share `t` of it (0..1). */
static f32 er_lerp_angle(f32 a, f32 b, f32 t) {
    f32 d = fmodf(b - a, 360.0f);
    if (d > 180.0f) {
        d -= 360.0f;
    } else if (d < -180.0f) {
        d += 360.0f;
    }
    return er_lerp(a, a + d, t);
}

/* ---- the drawn turn (er_moves_end_tick) ----------------------------------------------------- */

/* SM64 has no blending: a turn of his facing shows at once, and every step draws him facing
 * straight ahead again. What's left of a turn is drawn as a lag that eases out (this much of it
 * stays each tick): the aimed turns (er_aim_turn, his body and the weapon in his hand), and the
 * twist of a move he left with his body turned (his body only) */
#define ER_LAG_KEEP 0.45f
static s16 sErLagAim = 0;
static s16 sErLagCarry = 0;
/** what was drawn last tick (his yaw, and the lag in it), and the actions he has been put in so
 * far (set_mario_action counts them: a string's next blow is the same action again) */
static s16 sErShownYaw = 0;
static s16 sErShownLag = 0;
static u32 sErShownChanges = 0;
extern u32 er_action_changes;
/** a move twisted his body this tick / in the action he's in (its end carries the twist on) */
static s32 sErTwistTick = 0;
static s32 sErActionTwisted = 0;
/** the twist drawn last (degrees from his facing), for a move to face that way as it ends */
static f32 sErTwistDeg = 0.0f;

/** A new Mario (libsm64.c): no lag or twist left over from another one. */
void er_moves_reset(void) {
    sErLagAim = 0;
    sErLagCarry = 0;
    sErShownYaw = 0;
    sErShownLag = 0;
    sErShownChanges = er_action_changes;
    sErTwistTick = 0;
    sErActionTwisted = 0;
    sErTwistDeg = 0.0f;
}

/** An aimed turn of his facing (`from` was where he faced): drawn over a few ticks. */
void er_turn_lag(struct MarioState *m, s16 from) {
    sErLagAim += (s16)(from - m->faceAngle[1]);
}

/** Turns his drawn body by `deg` from where he faces (after the step, which sets it back). */
static void er_twist(struct MarioState *m, f32 deg) {
    m->marioObj->header.gfx.angle[1] = m->faceAngle[1] + er_angle(deg);
    sErTwistTick = 1;
    sErTwistDeg = deg;
}

/** Leaving a move with his body turned `deg`: he faces that way now, so the next action draws
 * him on from where he was (not snapped back to his old facing). */
static void er_fold(struct MarioState *m, f32 deg) {
    m->faceAngle[1] += er_angle(deg);
    sErTwistDeg = 0.0f;
    // (the weapon stays where it is: its numbers are from his facing)
    er_hammer_yaw -= deg;
    er_chomp_yaw -= deg;
}

/**
 * After Mario's action ran (mario.c): the lag of a turn is drawn on top of his yaw, and eases out.
 * A new action after one that twisted his body starts drawn where that left him.
 */
void er_moves_end_tick(struct MarioState *m) {
    s16 *yaw = &m->marioObj->header.gfx.angle[1];
    s16 lag;

    // (an action that didn't draw him again this tick still has last tick's lag in it)
    if (*yaw == sErShownYaw) {
        *yaw -= sErShownLag;
    }
    if (er_action_changes != sErShownChanges) {
        sErShownChanges = er_action_changes;
        if (sErActionTwisted) {
            sErLagCarry = (s16)(sErShownYaw - (s16)(*yaw + sErLagAim));
        }
        sErActionTwisted = sErTwistTick;
    } else {
        sErActionTwisted |= sErTwistTick;
    }
    sErTwistTick = 0;
    lag = sErLagAim + sErLagCarry;
    *yaw += lag;
    // (the weapon in his hand turns with him)
    if (er_hammer_scripted) {
        er_hammer_yaw += er_degrees(sErLagAim);
    }
    if (er_chomp_scripted) {
        er_chomp_yaw += er_degrees(sErLagAim);
    }
    sErShownYaw = *yaw;
    sErShownLag = lag;
    sErLagAim = (s16)((f32) sErLagAim * ER_LAG_KEEP);
    sErLagCarry = (s16)((f32) sErLagCarry * ER_LAG_KEEP);
}

static void er_sound(struct MarioState *m, u32 sound) {
    play_sound(sound, m->marioObj->header.gfx.cameraToObject);
}

void er_play_sound_volume(int32_t soundBits, float volume);

/** A soft sound (the slam's charge chime was too loud at full volume). */
static void er_sound_soft(u32 sound, f32 volume) {
    er_play_sound_volume((int32_t) sound, volume);
}

/**
 * A in an air blow with an enemy up there: Mario kicks off it, a jump in mid-air (Devil May Cry's
 * enemy step), a few to an air combo. TRUE if he jumped.
 */
static s32 er_enemy_step(struct MarioState *m) {
    if (!(m->input & INPUT_A_PRESSED) || !er_juggle || sErSteps >= ER_ENEMY_STEPS) {
        return FALSE;
    }
    sErSteps++;
    er_move_fx |= ER_FX_LAUNCH;
    play_sound(SOUND_ACTION_HIT, m->marioObj->header.gfx.cameraToObject);
    set_mario_action(m, ACT_DOUBLE_JUMP, 0);
    return TRUE;
}

/** A finisher's cheer: yahoo, waha, yippee... a different one each time (SM64's triple jump's). */
static void er_cheer(struct MarioState *m) {
    er_sound(m, SOUND_MARIO_YAHOO_WAHA_YIPPEE + ((gAudioRandom % 5) << 16));
}

/** A blow's shout: yah, wah or hoo, at random (SM64's jumps'). */
static void er_shout(struct MarioState *m) {
    er_sound(m, SOUND_MARIO_YAH_WAH_HOO + ((gAudioRandom % 3) << 16));
}

f32 er_take_lunge(struct MarioState *m);
void er_aim_lunge(struct MarioState *m);

/** A grounded step at `speed` (and an aimed blow's lunge); FALSE once he's off the floor (falling). */
static s32 er_ground(struct MarioState *m, f32 speed) {
    mario_set_forward_vel(m, speed + er_take_lunge(m));
    if (perform_ground_step(m) == GROUND_STEP_LEFT_GROUND) {
        set_mario_action(m, ACT_FREEFALL, 0);
        return FALSE;
    }
    return TRUE;
}

/* ---- the hammer ---------------------------------------------------------------------------- */

/* The hammer is the heavy one: few blows, each a titan's. Every one has a wind-up that tells it's
 * coming and can't be left (no dash, no jump: er_move_locked), a blow that stops the world a moment
 * (the hit-stop, impact.rs), and a long recovery: missed, it's punished. His body takes the weight:
 * squashed down into the wind-up, stretched into the blow, sinking under the follow-through. */

/** a blow of the hammer can't be left yet (moves.rs: no dash) */
int er_move_locked = 0;
/** his body's squash this tick (1: none; under 1 squashed down, over 1 stretched up), applied by
 * mario.c after SM64's own squish (er_apply_squash) */
f32 er_body_squash = 1.0f;

/** His body squashed `k` (1: as he is) and drawn `dip` units lower (after the step). */
static void er_squash(struct MarioState *m, f32 k, f32 dip) {
    er_body_squash = k;
    m->marioObj->header.gfx.pos[1] = m->pos[1] - dip;
}

/** mario.c, after SM64's squish of the tick: the move's squash on his model (not while crushed). */
void er_apply_squash(struct MarioState *m) {
    if (m->squishTimer == 0 && er_body_squash != 1.0f) {
        f32 across = 1.0f / sqrtf(er_body_squash);
        vec3f_set(m->marioObj->header.gfx.scale, across, er_body_squash, across);
    }
}

/** Smooth in and out (0..1). */
static f32 er_ease_in_out(f32 t) {
    if (t < 0.0f) {
        t = 0.0f;
    } else if (t > 1.0f) {
        t = 1.0f;
    }
    return t * t * (3.0f - 2.0f * t);
}

/* the hammer's string of three: the Hammer Bro's chop, King K. Rool's backhand, Donkey Kong's
 * smash; wind-up, blow and recovery ticks */
#define ER_SWINGS 3
static const s8 sSwingWind[ER_SWINGS] = { 9, 10, 14 };
static const s8 sSwingHit[ER_SWINGS] = { 3, 4, 3 };
static const s8 sSwingRest[ER_SWINGS] = { 18, 18, 27 };
/* after a blow: the next swing comes this many ticks on at the soonest (its follow-through
 * first), a jump or the dash this many (the smash's longer: it's buried in the ground) */
#define ER_SWING_NEXT 7
#define ER_SWING_FREE 8
#define ER_SMASH_FREE 12
/* the smash's hammer grows with the swing, and how high the little hop before it goes (units) */
#define ER_SMASH_SCALE 1.3f
#define ER_SMASH_HOP 55.0f
/* the next swing of a string starts from where the last one left the hammer, eased onto its own
 * wind-up over this many ticks (it jumped across in one) */
#define ER_SWING_EASE 3
/* SM64's heavy throw lets go at this frame: the overhead blows play it so it's there at impact */
#define ER_HEAVY_THROW_RELEASE 13
static f32 sErSwingFromYaw = 0.0f;
static f32 sErSwingFromPitch = 0.0f;

/**
 * X with the hammer, a string of three (actionArg): the Hammer Bro's chop, down across from over
 * his right shoulder; King K. Rool's backhand, a wide sweep from behind his left round to his
 * right, through everyone in front; Donkey Kong's smash, a little hop and the hammer, grown with
 * the swing, buried in the ground, the ground shaking around it (a regular enemy goes down). X
 * during a swing queues the next one (actionState).
 */
s32 act_er_hammer(struct MarioState *m) {
    u32 swing = m->actionArg < ER_SWINGS ? m->actionArg : ER_SWINGS - 1;
    s32 t = m->actionTimer;
    s32 wind = sSwingWind[swing];
    s32 hit = sSwingHit[swing];
    s32 rest = sSwingRest[swing];
    s32 blow = wind + hit;
    s32 free = blow + (swing == 2 ? ER_SMASH_FREE : ER_SWING_FREE);
    f32 speed = 0.0f;
    f32 twist = 0.0f;
    f32 squash = 1.0f;
    f32 dip = 0.0f;
    f32 p;
    f32 e;

    if (t == 0) {
        // (the last swing's hammer, still in the report: this one's wind-up starts from there)
        sErSwingFromYaw = er_hammer_yaw;
        sErSwingFromPitch = er_hammer_pitch;
        er_aim_turn(m);
        er_aim_lunge(m);
        er_sound_soft(SOUND_MARIO_UH, 0.5f);
    }
    if (t == wind) {
        // the shout comes with the blow, the whoosh with its swing
        er_sound(m, swing == 2 ? SOUND_MARIO_HOOHOO : swing == 1 ? SOUND_MARIO_PUNCH_WAH : SOUND_MARIO_PUNCH_YAH);
        er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.5f);
    }
    er_move_locked = t < free;
    if (t >= 2 && swing < ER_SWINGS - 1 && (m->input & INPUT_B_PRESSED)) {
        m->actionState = 1;
    }
    if (t >= free && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }

    if (swing == 0) {
        // the chop: both hands, raised over his right shoulder, down across to his left
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, t < wind ? 0xC000 : t < blow ? 0x20000 : 0x8000);
        if (t < wind) {
            e = er_ease_out((f32)(t + 1) / wind);
            er_hammer(er_lerp(-20.0f, -50.0f, e), er_lerp(40.0f, 150.0f, e), 1.0f);
            twist = -30.0f * e;
            squash = 1.0f - 0.08f * e;
            dip = 5.0f * e;
        } else if (t < blow) {
            p = (f32)(t - wind + 1) / hit;
            er_hammer(er_lerp(-50.0f, 30.0f, p * p), er_lerp(150.0f, -35.0f, p * p), 1.0f);
            er_move_hit = ER_HIT_HAMMER;
            speed = t == wind ? 24.0f : 12.0f;
            twist = er_lerp(-30.0f, 25.0f, p);
            squash = 1.06f;
            if (t == blow - 1) {
                er_move_fx |= ER_FX_GROUND;
                er_sound_soft(SOUND_GENERAL_POUND_WOOD_POST, 0.55f);
            }
        } else {
            // (the follow-through: dragged on a little low with its weight, then up again)
            p = (f32)(t - blow) / rest;
            e = er_ease_in_out((p - 0.3f) / 0.7f);
            er_hammer(er_lerp(er_lerp(30.0f, 40.0f, p / 0.3f), 15.0f, e), er_lerp(er_lerp(-35.0f, -42.0f, p / 0.3f), 20.0f, e), 1.0f);
            twist = er_lerp(25.0f, 0.0f, e);
            squash = er_lerp(0.94f, 1.0f, e);
            dip = er_lerp(4.0f, 0.0f, e);
        }
    } else if (swing == 1) {
        // the backhand: wound round behind his left, swept wide round to his right
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, t < wind ? 0x6000 : t < blow ? 0x18000 : 0x8000);
        if (t < wind) {
            e = er_ease_out((f32)(t + 1) / wind);
            er_hammer(er_lerp(30.0f, 140.0f, e), er_lerp(20.0f, 15.0f, e), 1.0f);
            twist = er_lerp(25.0f, 45.0f, e);
            squash = 1.0f - 0.06f * e;
            dip = 4.0f * e;
        } else if (t < blow) {
            p = (f32)(t - wind + 1) / hit;
            er_hammer(er_lerp(140.0f, -110.0f, p), er_lerp(15.0f, 0.0f, p), 1.0f);
            er_move_hit = ER_HIT_HAMMER_WIDE;
            speed = t == wind ? 16.0f : 6.0f;
            twist = er_lerp(45.0f, -40.0f, p);
            squash = 1.05f;
            if (t == wind + 1) {
                er_move_fx |= ER_FX_SPIN;
            }
        } else {
            p = (f32)(t - blow) / rest;
            e = er_ease_in_out((p - 0.3f) / 0.7f);
            er_hammer(er_lerp(er_lerp(-110.0f, -125.0f, p / 0.3f), -40.0f, e), er_lerp(0.0f, 20.0f, e), 1.0f);
            twist = er_lerp(er_lerp(-40.0f, -45.0f, p / 0.3f), 0.0f, e);
            squash = er_lerp(0.95f, 1.0f, e);
            dip = er_lerp(3.0f, 0.0f, e);
        }
    } else {
        // the smash: a crouch, a little hop with the hammer raised high behind his head (growing
        // with the swing), and down into the ground with all of him
        f32 hop = 0.0f;
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, t < wind ? 0x6000 : t < blow ? 0x28000 : 0x6000);
        if (t < 5) {
            e = er_ease_out((f32)(t + 1) / 5);
            er_hammer(0.0f, er_lerp(40.0f, 120.0f, e), 1.0f);
            squash = 1.0f - 0.14f * e;
            dip = 10.0f * e;
        } else if (t < wind) {
            p = (f32)(t - 5 + 1) / (wind - 5);
            e = er_ease_out(p);
            er_hammer(0.0f, er_lerp(120.0f, 175.0f, e), er_lerp(1.0f, ER_SMASH_SCALE, e));
            hop = sinf(ER_PI * 0.5f * e) * ER_SMASH_HOP;
            squash = er_lerp(0.86f, 1.08f, er_ease_out(p * 2.0f));
            if (t == 5) {
                er_sound_soft(SOUND_ACTION_SIDE_FLIP_UNK, 0.4f);
                er_move_fx |= ER_FX_LAUNCH;
            }
        } else if (t < blow) {
            p = (f32)(t - wind + 1) / hit;
            er_hammer(0.0f, er_lerp(175.0f, -45.0f, p * p), ER_SMASH_SCALE);
            hop = (1.0f - p * p) * ER_SMASH_HOP;
            speed = t == wind ? 22.0f : 10.0f;
            squash = 1.08f;
            if (t == blow - 1) {
                er_move_hit = ER_HIT_SMASH_WAVE;
                er_move_fx |= ER_FX_SLAM;
                er_sound_soft(SOUND_OBJ_THWOMP, 0.5f);
                er_sound_soft(SOUND_GENERAL_POUND_WOOD_POST, 0.6f);
                play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
            } else {
                er_move_hit = ER_HIT_HAMMER_SMASH;
            }
        } else {
            // (buried in the ground a moment, wrenched out, shrinking back)
            p = (f32)(t - blow) / rest;
            e = er_ease_in_out((p - 0.33f) / 0.67f);
            er_hammer(0.0f, er_lerp(-45.0f, 25.0f, e), er_lerp(ER_SMASH_SCALE, 1.0f, e));
            squash = p < 0.12f ? er_lerp(0.82f, 0.9f, p / 0.12f) : er_lerp(0.9f, 1.0f, e);
            dip = er_lerp(8.0f, 0.0f, e);
            if (t >= blow + 8 && t < blow + 11) {
                twist = (t % 2 == 0 ? 4.0f : -4.0f);
            }
        }
        dip -= hop;
    }
    // the string's next swing: its first ticks eased from where the last one left the hammer
    if (swing > 0 && m->prevAction == ACT_ER_HAMMER && t < ER_SWING_EASE) {
        f32 k = (f32)(t + 1) / (ER_SWING_EASE + 1);
        er_hammer(er_lerp_angle(sErSwingFromYaw, er_hammer_yaw, k), er_lerp(sErSwingFromPitch, er_hammer_pitch, k), er_hammer_scale);
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);
    er_squash(m, squash, dip);

    m->actionTimer++;
    // the next swing comes once this one's weight has followed through (his body's twist eases
    // out into it: er_moves_end_tick)
    if (m->actionState == 1 && m->actionTimer >= blow + ER_SWING_NEXT) {
        return set_mario_action(m, ACT_ER_HAMMER, swing + 1);
    }
    if (m->actionTimer >= blow + rest) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_SLAM_RAISE 10
#define ER_SLAM_IMPACT 13
#define ER_SLAM_END 38
/* Y held through the slam's wind-up charges it (ticks): the hammer grows, a chime per level; a
 * level up it comes down as the big quake (a regular enemy goes down), full as the titan's blow,
 * the hammer giant */
#define ER_SLAM_CHARGE_AT 9
#define ER_SLAM_CHARGE_LEVEL 12
#define ER_SLAM_CHARGE_MAX 36
#define ER_SLAM_SIZE 1.1f
#define ER_TITAN_SCALE 2.6f
/* the dash or a jump come back this long after the blow (the hammer is buried) */
#define ER_SLAM_FREE 11

/* Y is held (lib.rs, sm64_er_set_hold_y) */
int er_hold_y = 0;
/* Mario burns without SM64's own health loss (set once an Elden Ring fire lights him: status.rs) */
int er_soft_burn = 0;

/**
 * Y with the hammer, Bowser's quake: raised high over his head, slowly, and brought down, the
 * ground shaking all around. Held: charged, the hammer growing to the titan's. actionArg 1: the
 * plunge landing, straight from the blow.
 */
s32 act_er_hammer_slam(struct MarioState *m) {
    s32 t;
    f32 p;
    f32 e;
    s32 charge;
    s32 charging;
    s32 full;
    f32 size;
    f32 squash = 1.0f;
    f32 dip = 0.0f;
    s32 landing = FALSE;

    if (m->actionTimer == 0) {
        if (m->actionArg == 1) {
            m->actionTimer = ER_SLAM_IMPACT;
            landing = TRUE;
        } else {
            er_aim_turn(m);
            er_sound_soft(SOUND_MARIO_UH, 0.5f);
        }
    }
    t = m->actionTimer;
    er_move_locked = t < ER_SLAM_IMPACT + ER_SLAM_FREE;
    if (t >= ER_SLAM_IMPACT + ER_SLAM_FREE && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }
    // the charge (actionState: ticks of it): held up high while Y is
    charge = m->actionState;
    charging = m->actionArg == 0 && t == ER_SLAM_CHARGE_AT && er_hold_y && charge < ER_SLAM_CHARGE_MAX;
    if (charging) {
        m->actionState = ++charge;
        if (charge % ER_SLAM_CHARGE_LEVEL == 0) {
            er_sound_soft(SOUND_MENU_COLLECT_RED_COIN + ((charge / ER_SLAM_CHARGE_LEVEL - 1) << 16), 0.22f);
            er_move_fx |= ER_FX_GROW;
        }
    }
    size = ER_SLAM_SIZE + (ER_TITAN_SCALE - ER_SLAM_SIZE) * (f32) charge / ER_SLAM_CHARGE_MAX;
    full = charge >= ER_SLAM_CHARGE_MAX;

    set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, charging ? 0 : t < ER_SLAM_RAISE ? 0x8000 : t < ER_SLAM_IMPACT ? 0x28000 : 0x8000);
    // (the plunge lands on the blow itself: the throw from its release, not its wind-up)
    if (landing) {
        set_anim_to_frame(m, ER_HEAVY_THROW_RELEASE);
    }
    if (t < ER_SLAM_RAISE) {
        e = er_ease_out((f32)(t + 1) / ER_SLAM_RAISE);
        er_hammer(0.0f, er_lerp(30.0f, 172.0f, e), size);
        squash = 1.0f - 0.1f * e;
        dip = 7.0f * e;
    } else if (t < ER_SLAM_IMPACT) {
        p = (f32)(t - ER_SLAM_RAISE + 1) / (ER_SLAM_IMPACT - ER_SLAM_RAISE);
        er_hammer(0.0f, er_lerp(172.0f, -45.0f, p * p), size);
        squash = 1.07f;
    } else {
        // (buried in the ground a moment, wrenched out, shrinking back)
        p = (f32)(t - ER_SLAM_IMPACT) / (ER_SLAM_END - ER_SLAM_IMPACT);
        e = er_ease_in_out((p - 0.3f) / 0.7f);
        er_hammer(0.0f, er_lerp(-45.0f, 15.0f, e), er_lerp(size, 1.0f, e));
        squash = p < 0.1f ? er_lerp(0.8f, 0.9f, p / 0.1f) : er_lerp(0.9f, 1.0f, e);
        dip = er_lerp(8.0f, 0.0f, e);
    }
    if (t == ER_SLAM_IMPACT) {
        if (full) {
            // fully charged: the titan's blow
            er_move_hit = ER_HIT_TITAN;
            er_move_fx |= ER_FX_GIANT;
            er_cheer(m);
            er_sound_soft(SOUND_GENERAL_BOWSER_BOMB_EXPLOSION, 0.45f);
        } else if (charge >= ER_SLAM_CHARGE_LEVEL) {
            er_move_hit = ER_HIT_QUAKE_BIG;
            er_move_fx |= ER_FX_QUAKE;
            er_shout(m);
        } else {
            er_move_hit = ER_HIT_QUAKE;
            er_move_fx |= ER_FX_SLAM;
            er_sound(m, SOUND_MARIO_PUNCH_HOO);
        }
        er_sound_soft(SOUND_OBJ_THWOMP, full ? 0.65f : 0.5f);
        er_sound_soft(SOUND_GENERAL_POUND_WOOD_POST, 0.6f);
        play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
    }

    if (charging) {
        // (not a statue while it charges: he turns where the stick points, trembles harder as it
        // grows, the hammer shakes up there and gathers light, dust rises at his feet)
        f32 k = (f32) charge / ER_SLAM_CHARGE_MAX;
        f32 wob = sinf((f32) charge * 2.3f);
        if (m->input & INPUT_NONZERO_ANALOG) {
            m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x280, 0x280);
        }
        er_hammer(wob * (2.0f + 5.0f * k), 172.0f + sinf((f32) charge * 3.1f) * (2.0f + 4.0f * k), size);
        if (charge % 4 == 0) {
            er_move_fx |= ER_FX_CHARGE;
        }
    }
    if (!er_ground(m, t >= ER_SLAM_RAISE && t < ER_SLAM_IMPACT ? 12.0f : 0.0f)) {
        return TRUE;
    }
    if (charging) {
        f32 k = (f32) charge / ER_SLAM_CHARGE_MAX;
        er_twist(m, sinf((f32) charge * 2.9f) * (1.5f + 4.0f * k));
        // a squat that deepens, bouncing a little with the effort
        er_squash(m, 0.9f - 0.06f * k, (7.0f + 8.0f * k) * (0.7f + 0.3f * sinf((f32) charge * 1.7f)));
        return FALSE;
    }
    // (out of a charge he rises from the squat over a few ticks, not in one)
    if (m->actionArg == 0 && charge > 0 && t >= ER_SLAM_CHARGE_AT && t < ER_SLAM_CHARGE_AT + 3) {
        static const f32 rise[3] = { 0.6f, 0.3f, 0.1f };
        f32 k = (f32) charge / ER_SLAM_CHARGE_MAX;
        f32 held = (7.0f + 8.0f * k) * rise[t - ER_SLAM_CHARGE_AT];
        if (held > dip) {
            dip = held;
        }
    }
    er_squash(m, squash, dip);
    if (++m->actionTimer >= ER_SLAM_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_LAUNCH_WIND 8
#define ER_LAUNCH_SCRAPE 3
#define ER_LAUNCH_LIFT 3
#define ER_LAUNCH_END 26

/**
 * X, Y (or back + X) with the hammer: the launcher, a golf swing. Down low behind him on his
 * right, scraped along the ground in front, then straight up: whoever it catches flies high into
 * the air (juggle.rs), for Mario to jump after and go on with the air string. A or the stick soon
 * after the blow: off after them.
 */
s32 act_er_hammer_launch(struct MarioState *m) {
    s32 t = m->actionTimer;
    s32 lift = ER_LAUNCH_WIND + ER_LAUNCH_SCRAPE;
    f32 speed = 0.0f;
    f32 squash = 1.0f;
    f32 dip = 0.0f;
    f32 p;

    if (t == 0) {
        er_aim_turn(m);
        er_sound_soft(SOUND_MARIO_UH, 0.5f);
    }
    er_move_locked = t < lift + 2;
    if (t >= lift + 2 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }
    if (t >= lift + 6 && (m->input & INPUT_NONZERO_ANALOG)) {
        return set_mario_action(m, ACT_WALKING, 0);
    }

    set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, t < ER_LAUNCH_WIND ? 0x5000 : 0x10000);
    if (t < ER_LAUNCH_WIND) {
        // (taken back slowly: the wind-up tells it's coming)
        p = (f32)(t + 1) / ER_LAUNCH_WIND;
        er_hammer(er_lerp(-60.0f, -150.0f, er_ease_out(p)), er_lerp(10.0f, -35.0f, p), 1.1f);
        squash = 1.0f - 0.08f * er_ease_out(p);
        dip = 6.0f * er_ease_out(p);
    } else if (t < lift) {
        p = (f32)(t - ER_LAUNCH_WIND + 1) / ER_LAUNCH_SCRAPE;
        er_hammer(er_lerp(-150.0f, 0.0f, p * p), -35.0f, 1.1f);
        speed = 14.0f;
        dip = 6.0f;
        if (t == lift - 1) {
            er_move_fx |= ER_FX_GROUND;
            er_sound_soft(SOUND_GENERAL_POUND_WOOD_POST, 0.55f);
            play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
        }
    } else if (t < lift + ER_LAUNCH_LIFT) {
        p = (f32)(t - lift + 1) / ER_LAUNCH_LIFT;
        er_hammer(0.0f, er_lerp(-35.0f, 85.0f, er_ease_out(p)), 1.2f);
        er_move_hit = ER_HIT_LAUNCH;
        squash = 1.1f;
        if (t == lift) {
            er_move_fx |= ER_FX_LAUNCH | ER_FX_GROUND;
            er_cheer(m);
            er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.5f);
        }
    } else {
        p = (f32)(t - lift - ER_LAUNCH_LIFT) / (ER_LAUNCH_END - lift - ER_LAUNCH_LIFT);
        er_hammer(0.0f, er_lerp(85.0f, 40.0f, er_ease_in_out(p)), er_lerp(1.2f, 1.0f, p));
        squash = er_lerp(1.04f, 1.0f, p);
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, er_hammer_yaw * 0.45f);
    er_squash(m, squash, dip);
    if (++m->actionTimer >= ER_LAUNCH_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* Donkey Kong's drum: three pounds into the ground (each raised, brought down, held a moment),
 * the hammer and their rings growing, then it grows giant for the last */
#define ER_POUNDS 3
#define ER_POUND_START 3
#define ER_POUND_TICKS 11
#define ER_POUND_RAISE 6
#define ER_POUND_DOWN 2
#define ER_GIANT_START (ER_POUND_START + ER_POUNDS * ER_POUND_TICKS)
#define ER_GIANT_IMPACT (ER_GIANT_START + 13)
#define ER_FLURRY_END (ER_GIANT_IMPACT + 26)
#define ER_GIANT_SCALE 3.0f

/**
 * X, X, Y with the hammer: Donkey Kong's drum, three pounds into the ground stepping forward,
 * each shaking it wider, then the hammer grows giant for one last blow that shakes it all.
 */
s32 act_er_hammer_flurry(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 speed = 0.0f;
    f32 squash = 1.0f;
    f32 dip = 0.0f;
    f32 p;
    f32 e;

    if (t == 0) {
        er_sound(m, SOUND_MARIO_YAH_WAH_HOO);
    }
    er_move_locked = t < ER_GIANT_IMPACT + ER_SMASH_FREE;
    if (t < ER_POUND_START) {
        er_hammer(0.0f, er_lerp(er_hammer_pitch, 40.0f, (f32)(t + 1) / ER_POUND_START), 1.0f);
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0x8000);
        dip = 3.0f;
    } else if (t < ER_GIANT_START) {
        s32 pound = (t - ER_POUND_START) / ER_POUND_TICKS;
        s32 k = (t - ER_POUND_START) % ER_POUND_TICKS;
        f32 size = 1.0f + 0.15f * pound;
        if (k == 0) {
            er_aim_turn(m);
            set_mario_animation(m, MARIO_ANIM_HEAVY_THROW);
            set_anim_to_frame(m, 2);
        }
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, k < ER_POUND_RAISE ? 0xE000 : 0x20000);
        if (k < ER_POUND_RAISE) {
            e = er_ease_out((f32)(k + 1) / ER_POUND_RAISE);
            er_hammer(0.0f, er_lerp(-40.0f, 160.0f, e), size);
            speed = 6.0f;
            squash = 1.0f - 0.07f * e;
            dip = 4.0f * e;
        } else if (k < ER_POUND_RAISE + ER_POUND_DOWN) {
            p = (f32)(k - ER_POUND_RAISE + 1) / ER_POUND_DOWN;
            er_hammer(0.0f, er_lerp(160.0f, -40.0f, p * p), size);
            squash = 1.06f;
            if (k == ER_POUND_RAISE + ER_POUND_DOWN - 1) {
                er_move_hit = ER_HIT_POUND;
                er_move_fx |= pound == ER_POUNDS - 1 ? ER_FX_SLAM : ER_FX_GROUND;
                er_sound_soft(SOUND_GENERAL_POUND_WOOD_POST, 0.5f + 0.05f * pound);
                if (pound == ER_POUNDS - 1) {
                    er_sound_soft(SOUND_OBJ_THWOMP, 0.4f);
                }
            }
        } else {
            er_hammer(0.0f, -40.0f, size);
            squash = 0.9f;
            dip = 4.0f;
        }
    } else if (t < ER_GIANT_IMPACT - 3) {
        // the giant hammer: it grows raised high
        p = (f32)(t - ER_GIANT_START) / (ER_GIANT_IMPACT - 3 - ER_GIANT_START);
        if (t == ER_GIANT_START) {
            er_aim_turn(m);
            er_cheer(m);
            er_sound_soft(SOUND_GENERAL_SHORT_STAR, 0.4f);
            er_move_fx |= ER_FX_GROW;
        }
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0x6000);
        e = er_ease_out(p);
        er_hammer(0.0f, er_lerp(-40.0f, 168.0f, e), er_lerp(1.3f, ER_GIANT_SCALE, er_ease_out(p * 1.3f)));
        squash = 1.0f - 0.12f * e;
        dip = 9.0f * e;
    } else if (t <= ER_GIANT_IMPACT) {
        // (brought down over all three ticks: it stood still on the first and fell 154 degrees in
        // the last)
        p = (f32)(t - (ER_GIANT_IMPACT - 3)) / 3;
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0x28000);
        er_hammer(0.0f, er_lerp(168.0f, -40.0f, p * p), ER_GIANT_SCALE);
        speed = 14.0f;
        squash = 1.1f;
        if (t == ER_GIANT_IMPACT) {
            er_move_hit = ER_HIT_FLURRY_END;
            er_move_fx |= ER_FX_GIANT;
            er_sound_soft(SOUND_GENERAL_BOWSER_BOMB_EXPLOSION, 0.45f);
            er_sound_soft(SOUND_OBJ_THWOMP, 0.6f);
            play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
        } else if (p > 0.0f) {
            er_move_hit = ER_HIT_HAMMER_SMASH;
        }
    } else {
        p = (f32)(t - ER_GIANT_IMPACT) / (ER_FLURRY_END - ER_GIANT_IMPACT);
        e = er_ease_in_out((p - 0.35f) / 0.65f);
        er_hammer(0.0f, er_lerp(-40.0f, 15.0f, e), er_lerp(ER_GIANT_SCALE, 1.0f, e));
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0x6000);
        squash = p < 0.1f ? er_lerp(0.8f, 0.9f, p / 0.1f) : er_lerp(0.9f, 1.0f, e);
        dip = er_lerp(9.0f, 0.0f, e);
        if (t >= ER_GIANT_IMPACT + ER_SMASH_FREE && (m->input & INPUT_A_PRESSED)) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_squash(m, squash, dip);
    if (++m->actionTimer >= ER_FLURRY_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* the Golf Swing (King K. Rool's, Monster Hunter's Big Bang finish), out of the slam: the hammer
 * dragged back low, ripped out of the ground in one drive and swept up over his left shoulder.
 * The head's yaw (degrees from his facing, + his left), pitch and his body's twist on the ticks of
 * the sweep (5..9; the hit is at 7, most of the arc in three ticks) */
#define ER_GOLF_BACK 5
#define ER_GOLF_DRAG 3
#define ER_GOLF_HIT 7
#define ER_GOLF_SWEEP_END 9
#define ER_GOLF_HOLD 18
#define ER_GOLF_FREE 16
#define ER_GOLF_END 26
#define ER_GOLF_AIM 6
#define ER_GOLF_SCALE 1.2f
#define ER_GOLF_BACK_YAW -140.0f
#define ER_GOLF_BACK_PITCH -22.0f
#define ER_GOLF_BACK_TWIST -40.0f
static const f32 sGolfYaw[5] = { -140.0f, -85.0f, -5.0f, 55.0f, 86.0f };
static const f32 sGolfPitch[5] = { -22.0f, -14.0f, -8.0f, 50.0f, 112.0f };
static const f32 sGolfTwist[5] = { -40.0f, -15.0f, 20.0f, 50.0f, 60.0f };

/**
 * Y, X with the hammer: the head is dragged back low along the ground to his back right, then
 * ripped out of it in a low drive toward where the stick points, up over his left shoulder; one
 * heavy blow (the drag before it is a small one). His body coils back and follows through, never
 * past a quarter turn.
 */
/* where the hammer was as the Golf Swing began (the slam's pose, wherever it had got to) */
static f32 sGolfFrom[3] = { 0.0f, 15.0f, 1.0f };

s32 act_er_hammer_golf(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 speed = 0.0f;
    f32 twist;
    f32 squash = 1.0f;
    f32 dip = 0.0f;
    f32 p;
    f32 e;

    if (t == 0) {
        sGolfFrom[0] = er_hammer_yaw;
        sGolfFrom[1] = er_hammer_pitch;
        sGolfFrom[2] = er_hammer_scale;
        er_sound(m, SOUND_MARIO_YAH_WAH_HOO);
        er_sound_soft(SOUND_OBJ_BOWSER_TAIL_PICKUP, 0.4f);
    }
    // (the stick aims it while he winds up; once the head is on its way it flies true)
    if (t < ER_GOLF_AIM && (m->input & INPUT_NONZERO_ANALOG)) {
        m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x800, 0x800);
    }
    er_move_locked = t < ER_GOLF_FREE;
    if (t >= ER_GOLF_FREE && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }

    if (t <= ER_GOLF_BACK) {
        // the backswing: the head dragged back low (from where the slam left it in front), the body
        // coiled, the knees bent
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x5000);
        // (from the slam's pose: no jump on the first tick)
        e = er_ease_out((f32) t / ER_GOLF_BACK);
        er_hammer(er_lerp(sGolfFrom[0], ER_GOLF_BACK_YAW, e), er_lerp(sGolfFrom[1], ER_GOLF_BACK_PITCH, e), er_lerp(sGolfFrom[2], ER_GOLF_SCALE, e));
        twist = ER_GOLF_BACK_TWIST * e;
        squash = 1.0f - 0.05f * e;
        dip = 3.5f * e;
        if (t == ER_GOLF_DRAG) {
            er_move_hit = ER_HIT_GOLF_DRAG;
            er_move_fx |= ER_FX_GROUND;
            er_sound_soft(SOUND_GENERAL_POUND_ROCK, 0.3f);
        }
        if (t == ER_GOLF_BACK) {
            speed = 4.0f;
        }
    } else if (t <= ER_GOLF_SWEEP_END) {
        // the drive: from behind low, through the front low, up over his left shoulder
        s32 k = t - ER_GOLF_BACK;
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x20000);
        er_hammer(sGolfYaw[k], sGolfPitch[k], ER_GOLF_SCALE);
        twist = sGolfTwist[k];
        speed = 4.0f;
        // (the coil let go over the first ticks of the drive, not in one)
        squash = k == 1 ? 1.0f : 1.05f;
        dip = 3.5f * (1.0f - (f32) k / 4.0f);
        if (t == ER_GOLF_BACK + 1) {
            er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.6f);
        }
        if (t == ER_GOLF_HIT) {
            er_move_hit = ER_HIT_GOLF;
            er_move_fx |= ER_FX_BANG;
            er_cheer(m);
        }
    } else if (t < ER_GOLF_HOLD) {
        // the follow-through held, the hammer high over his left shoulder, creeping on a little
        p = (f32)(t - ER_GOLF_SWEEP_END) / (ER_GOLF_HOLD - ER_GOLF_SWEEP_END);
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x4000);
        er_hammer(er_lerp(sGolfYaw[4], sGolfYaw[4] + 6.0f, p), er_lerp(sGolfPitch[4], sGolfPitch[4] + 6.0f, p), ER_GOLF_SCALE);
        twist = sGolfTwist[4];
        squash = er_lerp(1.05f, 0.95f, er_ease_out(p));
        dip = 2.0f * er_ease_out(p);
    } else {
        // back to rest
        p = (f32)(t - ER_GOLF_HOLD) / (ER_GOLF_END - ER_GOLF_HOLD);
        e = er_ease_in_out(p);
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x6000);
        er_hammer(er_lerp(sGolfYaw[4] + 6.0f, 0.0f, e), er_lerp(sGolfPitch[4] + 6.0f, 15.0f, e), er_lerp(ER_GOLF_SCALE, 1.0f, e));
        twist = er_lerp(sGolfTwist[4], 0.0f, e);
        squash = er_lerp(0.95f, 1.0f, e);
        dip = er_lerp(2.0f, 0.0f, e);
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);
    er_squash(m, squash, dip);
    if (++m->actionTimer >= ER_GOLF_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* the plunge from high up lands as a charged slam: fully charged (the titan's blow) after a drop
 * of this much (SM64 units, 12 m), from a quarter of it on more and more; out of the leap (forward
 * + Y) it lands a level up at least, the Hammer Bro's jump */
#define ER_PLUNGE_FULL_DROP 1200.0f
static f32 sErPlungeFrom = 0.0f;
static s32 sErPlungeLeap = 0;

/* a swing in the air goes a whole turn round him from behind on his right (degrees from his
 * facing). His body turns from where it started (a share of the swing, eased in over its first
 * ticks: it jumped to -92 degrees at once) and comes all the way round over the last two, so he
 * faces where he started, and a next swing starts where this one left it (it jumped 150) */
#define ER_AIR_SWING_FROM -160.0f
#define ER_AIR_SWING_RAMP 2
#define ER_AIR_SWING_SETTLE 2

/** His body's twist on tick `t` of an air swing a whole turn over `ticks`. */
static f32 er_air_swing_twist(s32 t, s32 ticks, f32 share) {
    if (t >= ticks) {
        return er_lerp(360.0f * share, 360.0f, (f32)(t - ticks + 1) / ER_AIR_SWING_SETTLE);
    }
    return 360.0f * share * (f32)(t + 1) / ticks * (t < ER_AIR_SWING_RAMP ? (f32)(t + 1) / (ER_AIR_SWING_RAMP + 1) : 1.0f);
}

/* the air somersault: raised behind him, brought over in a flip, followed through */
#define ER_FLIP_RAISE 4
#define ER_FLIP_BLOW 4
#define ER_FLIP_END 13

/**
 * X (actionArg 0) in the air with the hammer: a somersault, the hammer brought over and down in
 * front with it. Y (actionArg 1): the plunge, a moment held up high, then straight down onto it:
 * lands as the slam's blow, the bigger the higher it came down from.
 */
s32 act_er_hammer_air(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 p;

    if (m->actionArg == 0) {
        if (t == 0) {
            er_sound_soft(SOUND_MARIO_UH, 0.5f);
            set_mario_animation(m, MARIO_ANIM_FORWARD_SPINNING);
            // juggling: each swing lifts him a little, to stay up with the enemy
            if (er_juggle) {
                m->vel[1] = 16.0f;
                mario_set_forward_vel(m, 4.0f);
            }
        }
        if (t >= 4 && (m->input & INPUT_B_PRESSED)) {
            m->actionState = 1;
        }
        if (t >= ER_FLIP_RAISE + ER_FLIP_BLOW && er_enemy_step(m)) {
            return TRUE;
        }
        if (t < ER_FLIP_RAISE) {
            er_hammer(0.0f, er_lerp(60.0f, 170.0f, er_ease_out((f32)(t + 1) / ER_FLIP_RAISE)), 1.1f);
        } else if (t < ER_FLIP_RAISE + ER_FLIP_BLOW) {
            p = (f32)(t - ER_FLIP_RAISE + 1) / ER_FLIP_BLOW;
            er_hammer(0.0f, er_lerp(170.0f, -70.0f, p * p), 1.15f);
            er_move_hit = ER_HIT_HAMMER_AIR;
            if (t == ER_FLIP_RAISE) {
                er_sound(m, SOUND_MARIO_PUNCH_HOO);
                er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.45f);
            }
        } else {
            p = (f32)(t - ER_FLIP_RAISE - ER_FLIP_BLOW) / (ER_FLIP_END - ER_FLIP_RAISE - ER_FLIP_BLOW);
            er_hammer(0.0f, er_lerp(-70.0f, -30.0f, er_ease_in_out(p)), 1.1f);
        }
        // (a moment of hang time)
        if (m->vel[1] < -10.0f) {
            m->vel[1] = -10.0f;
        }
    } else {
        if (t == 0) {
            sErPlungeLeap = m->prevAction == ACT_ER_LEAP;
            er_aim_turn(m);
            er_sound(m, SOUND_MARIO_PUNCH_HOO);
            set_mario_animation(m, MARIO_ANIM_START_GROUND_POUND);
        }
        mario_set_forward_vel(m, 0.0f);
        if (t < 6) {
            m->vel[1] = 0.0f;
            er_hammer(0.0f, er_lerp(40.0f, 170.0f, er_ease_out((f32)(t + 1) / 6)), 1.1f);
        } else {
            if (t == 6) {
                er_sound(m, SOUND_MARIO_GROUND_POUND_WAH);
                set_mario_animation(m, MARIO_ANIM_GROUND_POUND);
                sErPlungeFrom = m->pos[1];
            }
            m->vel[1] = -75.0f;
            er_hammer(0.0f, -80.0f, 1.1f);
            er_move_hit = ER_HIT_HAMMER_AIR;
        }
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (m->actionArg == 1) {
                // (the drop charges the slam: its charge is the slam's actionState)
                f32 drop = t > 6 ? sErPlungeFrom - m->pos[1] : 0.0f;
                f32 k = (drop / ER_PLUNGE_FULL_DROP - 0.25f) / 0.75f;
                s32 charge = k <= 0.0f ? 0 : k >= 1.0f ? ER_SLAM_CHARGE_MAX : (s32)(k * ER_SLAM_CHARGE_MAX);
                if (sErPlungeLeap && charge < ER_SLAM_CHARGE_LEVEL) {
                    charge = ER_SLAM_CHARGE_LEVEL;
                }
                set_mario_action(m, ACT_ER_HAMMER_SLAM, 1);
                m->actionState = charge;
                return TRUE;
            }
            return set_mario_action(m, ACT_FREEFALL_LAND, 0);

        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }

    if (++m->actionTimer >= (m->actionArg == 0 ? ER_FLIP_END : 90)) {
        // X during it: the next swing (or with the hammer put away, the fists' air string)
        if (m->actionArg == 0 && m->actionState == 1) {
            return set_mario_action(m, er_air_attack(), 0);
        }
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/* the air string: punch, punch, kick, again and again while X keeps coming */
#define ER_AIR_HIT_TICKS 7
#define ER_AIR_HITS 15

/**
 * X in the air after the super jump punch (or while an enemy is up there with him): DMC's air
 * combo, punch, punch, kick, each blow lifting him a little to stay up with the enemy. Y ends it
 * with the meteor dive (moves.rs); L1 brings the hammer out and X goes on swinging it.
 */
s32 act_er_air_combo(struct MarioState *m) {
    static const s16 anims[3] = { MARIO_ANIM_FIRST_PUNCH, MARIO_ANIM_SECOND_PUNCH, MARIO_ANIM_AIR_KICK };
    u32 blow = m->actionArg % 3;
    s32 t = m->actionTimer;

    if (t == 0) {
        er_aim_turn(m);
        set_mario_animation(m, anims[blow]);
        set_anim_to_frame(m, 0);
        // (a long string gets louder: shouts at random past the first round, a cheer every two)
        if (m->actionArg > 0 && m->actionArg % 6 == 5) {
            er_cheer(m);
        } else if (m->actionArg >= 3) {
            er_shout(m);
        } else {
            er_sound(m, blow == 2 ? SOUND_MARIO_PUNCH_HOO : blow == 0 ? SOUND_MARIO_PUNCH_YAH : SOUND_MARIO_PUNCH_WAH);
        }
        m->vel[1] = er_juggle ? 14.0f : 6.0f;
        // (with an enemy up there: a smaller step forward, a long string carried him past it)
        mario_set_forward_vel(m, er_juggle ? 3.0f : 6.0f);
    }
    if (t >= 2 && (m->input & INPUT_B_PRESSED)) {
        m->actionState = 1;
    }
    if (t >= 2 && er_enemy_step(m)) {
        return TRUE;
    }
    if (t >= 1 && t <= 3) {
        er_move_hit = ER_HIT_AIR;
        m->flags |= blow == 2 ? MARIO_KICKING : MARIO_PUNCHING;
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            return set_mario_action(m, ACT_FREEFALL_LAND, 0);

        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }

    if (++m->actionTimer >= ER_AIR_HIT_TICKS) {
        if (m->actionState == 1) {
            if (er_weapon != 0) {
                return set_mario_action(m, er_air_attack(), 0);
            }
            if (m->actionArg + 1 < ER_AIR_HITS) {
                return set_mario_action(m, ACT_ER_AIR_COMBO, m->actionArg + 1);
            }
        }
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/* ---- bare-handed ---------------------------------------------------------------------------- */

#define ER_UPPERCUT_TOP 11
#define ER_UPPERCUT_END 20

/**
 * X, X, Y: Super Smash Bros.' super jump punch, up spinning with a fist raised, hitting on the way
 * up (coins fly) and hard at the top, lifting small enemies with him (juggle.rs). X in it: the air
 * string (with the hammer, its swings); Y: the meteor dive (moves.rs).
 */
s32 act_er_uppercut(struct MarioState *m) {
    s32 t = m->actionTimer;

    if (t == 0) {
        er_aim_turn(m);
        sErSpin = 0;
        // (Mega Mario's goes up a giant's way, like his jumps: mario.c)
        m->vel[1] = er_mega ? 31.0f : 50.0f;
        mario_set_forward_vel(m, 10.0f);
        set_mario_animation(m, MARIO_ANIM_SINGLE_JUMP);
        set_anim_to_frame(m, 0);
        er_sound(m, SOUND_MARIO_PUNCH_HOO);
        er_sound(m, SOUND_ACTION_SPIN);
        er_move_fx |= ER_FX_LAUNCH;
    }
    if (t >= 3 && (m->input & INPUT_B_PRESSED)) {
        m->faceAngle[1] += sErSpin;
        sErSpin = 0;
        return set_mario_action(m, er_air_attack(), 0);
    }
    if (t >= 1 && t < ER_UPPERCUT_TOP) {
        er_move_hit = ER_HIT_UPPERCUT;
    } else if (t == ER_UPPERCUT_TOP) {
        er_move_hit = ER_HIT_UPPERCUT_TOP;
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (t > 2) {
                m->faceAngle[1] += sErSpin;
                sErSpin = 0;
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;

        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    // spinning on the way up, slowing at the top
    if (t <= ER_UPPERCUT_TOP) {
        sErSpin += (s16) er_lerp(0x2C00, 0x800, (f32) t / ER_UPPERCUT_TOP);
    }
    er_twist(m, er_degrees(sErSpin));

    if (++m->actionTimer >= ER_UPPERCUT_END) {
        m->faceAngle[1] += sErSpin;
        sErSpin = 0;
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

#define ER_METEOR_HANG 5

/** X in the super jump punch: a flip in mid-air, then a dive fist first onto the enemy. */
s32 act_er_meteor(struct MarioState *m) {
    s32 t = m->actionTimer;

    if (t == 0) {
        m->faceAngle[1] += sErSpin;
        sErSpin = 0;
        er_aim_turn(m);
        set_mario_animation(m, MARIO_ANIM_FORWARD_SPINNING);
        set_anim_to_frame(m, 0);
        er_sound(m, SOUND_ACTION_SPIN);
    }
    if (t < ER_METEOR_HANG) {
        mario_set_forward_vel(m, 0.0f);
        m->vel[1] = 0.0f;
    } else {
        if (t == ER_METEOR_HANG) {
            er_aim_turn(m);
            set_mario_animation(m, MARIO_ANIM_DIVE);
            er_sound(m, SOUND_MARIO_GROUND_POUND_WAH);
            er_sound(m, SOUND_ACTION_FLYING_FAST);
        }
        mario_set_forward_vel(m, 46.0f);
        m->vel[1] = -70.0f;
        er_move_hit = ER_HIT_METEOR;
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            return set_mario_action(m, ACT_ER_METEOR_LAND, 0);

        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    // (diving: pitched down along the fall, tipping over into it over its first ticks)
    if (t >= ER_METEOR_HANG) {
        static const s16 tip[3] = { 0x800, 0x1000, 0x1800 };
        m->marioObj->header.gfx.angle[0] = tip[t - ER_METEOR_HANG < 2 ? t - ER_METEOR_HANG : 2];
    }
    if (++m->actionTimer >= 75) {
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/** The meteor dive's landing: a blast all around. */
s32 act_er_meteor_land(struct MarioState *m) {
    if (m->actionTimer == 0) {
        er_move_hit = ER_HIT_METEOR_LAND;
        er_move_fx |= ER_FX_METEOR;
        er_sound(m, SOUND_GENERAL_BOWSER_BOMB_EXPLOSION);
        play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
        set_mario_animation(m, MARIO_ANIM_GROUND_POUND_LANDING);
    }
    if (m->actionTimer >= 6 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }
    if (!er_ground(m, 0.0f)) {
        return TRUE;
    }
    if (++m->actionTimer >= 16) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_PALM_HIT 5
#define ER_PALM_BURST 6
#define ER_PALM_AIM 5
#define ER_PALM_FREE 12
#define ER_PALM_HOLD 14
#define ER_PALM_END 22
#define ER_PALM_PREP 7

/* the body's turn (degrees) over the wind-up and the push: drawn back, then through to the palms */
static const f32 sPalmTwist[ER_PALM_PREP] = { -8.0f, -16.0f, -22.0f, -25.0f, -12.0f, 10.0f, 10.0f };
/* the lunge of ticks 4 to 6 (units per tick: 0.8 m in all) */
static const f32 sPalmLunge[3] = { 20.0f, 32.0f, 28.0f };

/**
 * Y, X: Smash Bros. Mario's forward smash, the Fire Palm. Out of the low sweep he rises into one
 * step and pushes with both palms, which burst in a cone of fire in front of him (the mod draws it
 * when the burst hit lands: impact.rs, fireball.rs).
 */
s32 act_er_fire_palm(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 speed = 0.0f;
    f32 twist;
    f32 squash = 1.0f;
    f32 dip = 0.0f;
    f32 p;
    f32 e;

    if (t == 0) {
        er_shout(m);
    }
    // (the stick aims it while he draws back; once the palms are on their way it goes true)
    if (t < ER_PALM_AIM && (m->input & INPUT_NONZERO_ANALOG)) {
        m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x800, 0x800);
    }
    er_move_locked = t < ER_PALM_FREE;
    if (t >= ER_PALM_FREE && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }

    if (t < ER_PALM_HIT) {
        // rising out of the crouch, both palms drawn back at the hip
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x3000);
        e = er_ease_out((f32)(t + 1) / ER_PALM_HIT);
        twist = sPalmTwist[t];
        squash = 1.0f - 0.04f * e;
        dip = 3.0f * (1.0f - 0.5f * e);
        if (t == ER_PALM_HIT - 1) {
            speed = sPalmLunge[0];
        }
    } else if (t <= ER_PALM_BURST) {
        // the push: a lunge along his facing, the palms forward; the contact, then the burst
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x1C000);
        twist = sPalmTwist[t];
        speed = sPalmLunge[t - ER_PALM_HIT + 1];
        squash = 1.04f;
        dip = 1.0f;
        if (t == ER_PALM_HIT) {
            er_move_hit = ER_HIT_PALM;
            er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.5f);
        } else {
            er_move_hit = ER_HIT_PALM_BURST;
            er_sound_soft(SOUND_GENERAL_FLAME_OUT, 0.45f);
            er_cheer(m);
        }
    } else if (t < ER_PALM_HOLD) {
        // the follow-through held, arms out
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x3000);
        p = (f32)(t - ER_PALM_BURST) / (ER_PALM_HOLD - ER_PALM_BURST);
        twist = 10.0f;
        squash = er_lerp(1.04f, 0.98f, er_ease_out(p));
        dip = er_lerp(1.0f, 0.0f, p);
    } else {
        // back to rest
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0x5000);
        p = (f32)(t - ER_PALM_HOLD) / (ER_PALM_END - ER_PALM_HOLD);
        e = er_ease_in_out(p);
        twist = er_lerp(10.0f, 0.0f, e);
        squash = er_lerp(0.98f, 1.0f, e);
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);
    er_squash(m, squash, dip);
    if (++m->actionTimer >= ER_PALM_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* the bare-handed string past SM64's punch, punch, kick (actionArg): a spinning backfist, two
 * kicks, and a whirlwind kick */
#define ER_CHAIN_STEPS 3
static const s8 sChainLength[ER_CHAIN_STEPS] = { 10, 13, 20 };
/* the whirlwind kick's last half turn after the blow (ticks) */
#define ER_WHIRL_SETTLE 8

/**
 * X after the combo's kick: the string goes on, X during each blow queues the next (actionState).
 */
s32 act_er_chain(struct MarioState *m) {
    u32 step = m->actionArg < ER_CHAIN_STEPS ? m->actionArg : ER_CHAIN_STEPS - 1;
    s32 t = m->actionTimer;
    s32 end = sChainLength[step];
    f32 speed = 0.0f;
    f32 twist = 0.0f;

    if (t == 0) {
        er_aim_turn(m);
    }
    if (t >= 2 && step < ER_CHAIN_STEPS - 1 && (m->input & INPUT_B_PRESSED)) {
        m->actionState = 1;
    }
    if (t >= end - 4 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }

    if (step == 0) {
        // the spinning backfist: a full turn, the fist out at its end
        if (t == 0) {
            set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH_FAST);
            set_anim_to_frame(m, 0);
            er_sound(m, SOUND_MARIO_PUNCH_WAH);
            er_sound(m, SOUND_ACTION_SPIN);
        }
        twist = t < 5 ? er_lerp(0.0f, 360.0f, er_ease_out((f32)(t + 1) / 5)) : 0.0f;
        if (t >= 2 && t <= 5) {
            er_move_hit = ER_HIT_CHAIN;
            m->flags |= MARIO_PUNCHING;
        }
        speed = t < 3 ? 18.0f : 0.0f;
    } else if (step == 1) {
        // two kicks, stepping in
        if (t == 0 || t == 6) {
            set_mario_animation(m, MARIO_ANIM_GROUND_KICK);
            set_anim_to_frame(m, 0);
            er_sound(m, t == 0 ? SOUND_MARIO_PUNCH_YAH : SOUND_MARIO_PUNCH_HOO);
        }
        if ((t >= 1 && t <= 3) || (t >= 7 && t <= 9)) {
            er_move_hit = ER_HIT_CHAIN;
            m->flags |= MARIO_KICKING;
        }
        speed = (t < 2 || (t >= 6 && t < 8)) ? 16.0f : 0.0f;
    } else {
        // the whirlwind kick: a turn and a half, the leg out, and a gust all around at its end
        if (t == 0) {
            set_mario_animation(m, MARIO_ANIM_AIR_KICK);
            set_anim_to_frame(m, 0);
            er_sound(m, SOUND_MARIO_PUNCH_HOO);
            er_sound(m, SOUND_ACTION_SPIN);
        }
        // (and on round the last half turn, slowing, to face where it started: held at half a
        // turn, he snapped round to it as it ended)
        twist = t < 8 ? er_lerp(0.0f, 540.0f, er_ease_out((f32)(t + 1) / 8))
                      : er_lerp(540.0f, 720.0f, er_ease_out((f32)(t - 7) / ER_WHIRL_SETTLE));
        if (t >= 2 && t < 7) {
            er_move_hit = ER_HIT_CHAIN;
            m->flags |= MARIO_KICKING;
        } else if (t == 7) {
            er_move_hit = ER_HIT_CHAIN_END;
            er_move_fx |= ER_FX_SPIN | ER_FX_LAUNCH;
            er_cheer(m);
        }
        speed = t < 4 ? 14.0f : 0.0f;
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);

    m->actionTimer++;
    if (m->actionState == 1 && m->actionTimer >= end - 3) {
        return set_mario_action(m, ACT_ER_CHAIN, step + 1);
    }
    if (m->actionTimer >= end) {
        // (the whirlwind kick ends facing back where it started)
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_SHELL_TICKS 26
/* its last ticks slowing down, and its wall bounce's turn round drawn over this many */
#define ER_SHELL_SLOW 5
#define ER_SHELL_BOUNCE_TICKS 2

/**
 * B, Y: the Koopa shell, Mario curled up and rolling fast, steered with the stick, bouncing off
 * walls, hitting whoever he rolls into.
 */
s32 act_er_shell(struct MarioState *m) {
    static s32 sBounce = 0;
    s32 t = m->actionTimer;
    f32 speed;

    if (t == 0) {
        sErSpin = 0;
        sBounce = 0;
        if (m->input & INPUT_NONZERO_ANALOG) {
            m->faceAngle[1] = m->intendedYaw;
        }
        er_cheer(m);
    }
    if (t % 6 == 0) {
        er_sound(m, SOUND_ACTION_SPIN);
    }
    if (t % 4 == 0) {
        er_move_fx |= ER_FX_LAUNCH;
    }
    set_mario_animation(m, MARIO_ANIM_FORWARD_SPINNING);
    if (is_anim_at_end(m)) {
        set_anim_to_frame(m, 0);
    }
    if (m->input & INPUT_NONZERO_ANALOG) {
        m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x600, 0x600);
    }
    speed = t < ER_SHELL_TICKS - 5 ? 50.0f : er_lerp(50.0f, 10.0f, (f32)(t - (ER_SHELL_TICKS - 5)) / 5);
    er_move_hit = ER_HIT_SHELL;

    mario_set_forward_vel(m, speed);
    switch (perform_ground_step(m)) {
        case GROUND_STEP_LEFT_GROUND:
            return set_mario_action(m, ACT_FREEFALL, 0);

        case GROUND_STEP_HIT_WALL:
            // like a shell: off the wall and back the other way (drawn turning round over two
            // ticks)
            m->faceAngle[1] += 0x8000;
            sBounce = ER_SHELL_BOUNCE_TICKS;
            er_sound(m, SOUND_ACTION_HIT);
            break;
    }
    // rolling: the whole body spins forward; slowing down at the end, it rolls on just to the
    // next whole turn, upright (it stood up from upside down at once)
    if (t < ER_SHELL_TICKS - ER_SHELL_SLOW) {
        sErSpin += 0x2000;
    } else {
        s32 left = ER_SHELL_TICKS - t;
        sErSpin += (s16)((f32)(u16)(-sErSpin) * 2.0f / (left + 1));
    }
    m->marioObj->header.gfx.angle[0] = sErSpin;
    if (sBounce > 0) {
        // (a share of the half turn still to go)
        m->marioObj->header.gfx.angle[1] = m->faceAngle[1] + (s16)(0x8000 * sBounce / (ER_SHELL_BOUNCE_TICKS + 1));
        sBounce--;
    }

    if (++m->actionTimer >= ER_SHELL_TICKS) {
        m->marioObj->header.gfx.angle[0] = 0;
        // (uncurling into a landing, not straight up standing)
        return set_mario_action(m, ACT_FREEFALL_LAND, 0);
    }
    return FALSE;
}

/* ---- the boomerang and the fireball: throws ------------------------------------------------- */

#define ER_BIG_CHARGE_MIN 6
#define ER_BIG_CHARGE_MAX 40
/* the big boomerang's wind-up: a whirl from this many degrees a tick to that many, fully charged;
 * let go, the throw goes on from this frame of SM64's heavy throw (its release is at 13) */
#define ER_WHIRL_FROM 14.0f
#define ER_WHIRL_TO 46.0f
#define ER_BIG_THROW_FRAME 9
static f32 sRangWhirl = 0.0f;

/* the Chomp's chain is this much longer than the moves' numbers say, past its place by him (more
 * under Star Power: sm64_er_set_chomp_longer, starpower.rs) */
f32 er_chomp_longer = 1.25f;

/** the ticks Mario's moves have run (not the hit-stop's) */
static u32 sErTick = 0;
/** where a move last put the Chomp (its reach as the move gave it), and on which tick */
static f32 sErChompRawReach = 0.0f;
static u32 sErChompTick = 0;
/* where it hops beside him between blows (chomp.rs REST_*): a move that finds it there starts
 * from there */
#define ER_CHOMP_REST_YAW -140.0f
#define ER_CHOMP_REST_REACH 120.0f
#define ER_CHOMP_REST_HEIGHT 30.0f

static void er_chomp(f32 yaw, f32 reach, f32 height, f32 bite) {
    er_chomp_scripted = 1;
    er_chomp_yaw = yaw;
    er_chomp_reach = reach > 110.0f ? 110.0f + (reach - 110.0f) * er_chomp_longer : reach;
    er_chomp_height = height;
    er_chomp_bite = bite;
    sErChompRawReach = reach;
    sErChompTick = sErTick;
}

/** where a move found the Chomp (degrees, reach, height), for its first ticks to start from */
static f32 sErChompFrom[3] = { 0.0f, 0.0f, 0.0f };

/** A Chomp move starts: where the Chomp is now (a move's last place, this tick or the last, or
 * beside him). */
static void er_chomp_from_here(void) {
    s32 moved = sErChompTick == sErTick || sErChompTick + 1 == sErTick;
    sErChompFrom[0] = moved ? er_chomp_yaw : ER_CHOMP_REST_YAW;
    sErChompFrom[1] = moved ? sErChompRawReach : ER_CHOMP_REST_REACH;
    sErChompFrom[2] = moved ? er_chomp_height : ER_CHOMP_REST_HEIGHT;
}

/** This tick's Chomp a share `k` of the way from where the move found it (k 1: as it is). */
static void er_chomp_eased(f32 k) {
    if (k < 1.0f) {
        er_chomp(er_lerp_angle(sErChompFrom[0], er_chomp_yaw, k), er_lerp(sErChompFrom[1], sErChompRawReach, k),
                 er_lerp(sErChompFrom[2], er_chomp_height, k), er_chomp_bite);
    }
}

/** The weapon's blow in the air: the fists' air string, the hammer's swing, a throw, the Chomp. */
static u32 er_air_attack(void) {
    switch (er_weapon) {
        case 1: return ACT_ER_HAMMER_AIR;
        case ER_WEAPON_RANG: return er_rang_ready ? ACT_ER_AIR_THROW : ACT_FREEFALL;
        case ER_WEAPON_CHOMP: return ACT_ER_CHOMP_AIR;
        default: return ACT_ER_AIR_COMBO;
    }
}

/** A throw lets go now: the mod spawns what it throws (boomerang.rs, fireball.rs). */
static void er_release(struct MarioState *m, u32 kind) {
    er_move_throw = (s32) kind + 1;
    er_move_charge = m->actionState;
    er_move_fx |= ER_FX_THROW;
    if (kind == ER_THROW_SPRING) {
        er_sound(m, SOUND_ACTION_TERRAIN_STEP + (SOUND_TERRAIN_STONE << 16));
        return;
    }
    if (kind == ER_THROW_SHELL) {
        er_sound(m, SOUND_ACTION_HIT);
        return;
    }
    er_sound(m, kind == ER_THROW_HOOK ? SOUND_ACTION_SPIN : kind >= ER_THROW_FIRE ? SOUND_ACTION_THROW : SOUND_GENERAL_SWISH_AIR);
}

/**
 * A throw on the ground (actionArg ER_THROW_*): the boomerang from one hand then the other, three at
 * once out of a spin (Y), up in an arc (X, X, Y: it lifts them), the big one raised overhead and
 * charged while Y is held (let go early, it's the spin's three); the fireball as a palm thrust that
 * pushes him back the harder it is, the giant one from over his head. X again after the boomerang
 * leaves his hand: the next one.
 */
s32 act_er_throw(struct MarioState *m) {
    static s32 sHand = 0;
    u32 kind = m->actionArg;
    s32 t = m->actionTimer;
    s32 release;
    s32 end;
    f32 speed = m->forwardVel * 0.7f;
    f32 twist = 0.0f;

    switch (kind) {
        case ER_THROW_STORM: release = 6; end = 15; break;
        case ER_THROW_UPPER: release = 4; end = 15; break;
        case ER_THROW_BIG: release = 5; end = 20; break;
        case ER_THROW_FIRE_MID: release = 2; end = 12; break;
        case ER_THROW_FIRE_BIG: release = 3; end = 15; break;
        case ER_THROW_FIRE_GIANT: release = 7; end = 24; break;
        case ER_THROW_FIRE: release = 2; end = 10; break;
        case ER_THROW_HOOK: release = 1; end = 9; break;
        case ER_THROW_BANANA: release = 5; end = 15; break;
        case ER_THROW_SHELL: release = 4; end = 14; break;
        case ER_THROW_BOMB: release = 6; end = 17; break;
        case ER_THROW_SPRING: release = 8; end = 18; break;
        default: release = 2; end = 9; break;
    }

    if (t == 0) {
        er_aim_turn(m);
        sHand ^= 1;
        sRangWhirl = 0.0f;
        switch (kind) {
            case ER_THROW_STORM:
                set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH_FAST);
                er_shout(m);
                er_sound(m, SOUND_ACTION_SPIN);
                break;
            case ER_THROW_UPPER:
                set_mario_animation(m, MARIO_ANIM_GROUND_THROW);
                er_sound(m, SOUND_MARIO_PUNCH_HOO);
                break;
            case ER_THROW_BIG:
            case ER_THROW_FIRE_GIANT:
                set_mario_animation(m, MARIO_ANIM_HEAVY_THROW);
                er_sound(m, SOUND_MARIO_HOOHOO);
                break;
            case ER_THROW_FIRE:
                set_mario_animation(m, MARIO_ANIM_FIRST_PUNCH_FAST);
                er_sound(m, SOUND_MARIO_PUNCH_YAH);
                break;
            case ER_THROW_HOOK:
                set_mario_animation(m, MARIO_ANIM_GROUND_THROW);
                er_sound(m, SOUND_MARIO_PUNCH_HOO);
                break;
            case ER_THROW_BANANA:
                set_mario_animation(m, MARIO_ANIM_GROUND_THROW);
                break;
            case ER_THROW_SHELL:
                set_mario_animation(m, MARIO_ANIM_GROUND_KICK);
                er_sound(m, SOUND_MARIO_PUNCH_HOO);
                break;
            case ER_THROW_BOMB:
                set_mario_animation(m, MARIO_ANIM_GROUND_THROW);
                er_sound(m, SOUND_MARIO_HOOHOO);
                break;
            case ER_THROW_SPRING:
                set_mario_animation(m, MARIO_ANIM_PLACE_LIGHT_OBJ);
                break;
            case ER_THROW_FIRE_MID:
            case ER_THROW_FIRE_BIG:
                set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH);
                er_sound(m, SOUND_MARIO_PUNCH_WAH);
                break;
            default:
                set_mario_animation(m, sHand ? MARIO_ANIM_FIRST_PUNCH_FAST : MARIO_ANIM_SECOND_PUNCH_FAST);
                er_sound(m, sHand ? SOUND_MARIO_PUNCH_YAH : SOUND_MARIO_PUNCH_WAH);
                break;
        }
        set_anim_to_frame(m, 0);
    }

    // the big boomerang: raised and charged while Y is held; let go before it took, the spin's three
    if (kind == ER_THROW_BIG && t == 2) {
        s32 charge = m->actionState;
        if (er_hold_y && charge < ER_BIG_CHARGE_MAX) {
            f32 k = (f32) charge / ER_BIG_CHARGE_MAX;
            m->actionState = ++charge;
            if (charge % 4 == 0) {
                er_move_fx |= ER_FX_CHARGE;
            }
            if (charge == ER_BIG_CHARGE_MAX) {
                er_sound_soft(SOUND_MENU_COLLECT_RED_COIN + (3 << 16), 0.3f);
            }
            if (m->input & INPUT_NONZERO_ANALOG) {
                m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x300, 0x300);
            }
            // (not a statue: he whirls round on the spot with it held out, like Bowser by the tail,
            // faster and faster as it charges, leaning into it, a whoosh each turn)
            set_mario_anim_with_accel(m, MARIO_ANIM_SWINGING_BOWSER, (s32)((1.0f + 2.0f * k) * 0x10000));
            sRangWhirl += ER_WHIRL_FROM + (ER_WHIRL_TO - ER_WHIRL_FROM) * k;
            if (sRangWhirl >= 360.0f) {
                sRangWhirl -= 360.0f;
                er_sound_soft(SOUND_ACTION_SPIN, 0.2f);
            }
            if (!er_ground(m, 0.0f)) {
                return TRUE;
            }
            er_twist(m, sRangWhirl);
            m->marioObj->header.gfx.pos[1] = m->pos[1] - (2.0f + 5.0f * k) * (0.8f + 0.2f * sinf((f32) charge * 1.3f));
            return FALSE;
        }
        // let go (charged): out of the whirl straight into the throw, facing where it goes (his
        // body turned there over a few ticks from where the whirl left it)
        if (charge >= ER_BIG_CHARGE_MIN && sRangWhirl != 0.0f) {
            er_turn_lag(m, m->faceAngle[1] + er_angle(sRangWhirl));
            sRangWhirl = 0.0f;
            er_aim_turn(m);
            set_mario_animation(m, MARIO_ANIM_HEAVY_THROW);
            set_anim_to_frame(m, ER_BIG_THROW_FRAME);
        }
        if (charge < ER_BIG_CHARGE_MIN) {
            // (the charge counter isn't the next-throw flag)
            m->actionState = 0;
            sRangWhirl = 0.0f;
            kind = m->actionArg = ER_THROW_STORM;
            release = 6;
            end = 15;
            set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH_FAST);
            er_sound(m, SOUND_ACTION_SPIN);
        }
    }

    if (kind == ER_THROW_STORM && t < release) {
        twist = er_lerp(0.0f, 360.0f, er_ease_out((f32)(t + 1) / release));
        if (t == release - 2) {
            set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH_FAST);
            set_anim_to_frame(m, 0);
        }
    }
    if (t == release) {
        er_release(m, kind);
        if (kind == ER_THROW_FIRE_GIANT || kind == ER_THROW_BIG) {
            er_cheer(m);
        }
    }
    // the recoil of a strong shot: pushed back a little
    if (t >= release && t < release + 3) {
        switch (kind) {
            case ER_THROW_FIRE_MID: speed = -8.0f; break;
            case ER_THROW_FIRE_BIG: speed = -14.0f; break;
            case ER_THROW_FIRE_GIANT: speed = -20.0f; break;
            case ER_THROW_BIG: speed = -6.0f; break;
        }
    }
    if (t > release) {
        if ((kind == ER_THROW_RANG || kind == ER_THROW_STORM) && (m->input & INPUT_B_PRESSED) && er_rang_ready) {
            m->actionState = 1;
        }
        // (after a shot of fire, X goes straight on with the weapon's blows)
        if (kind >= ER_THROW_FIRE && kind <= ER_THROW_FIRE_GIANT && t >= release + 2 && (m->input & INPUT_B_PRESSED)) {
            return set_mario_action(m, ACT_PUNCHING, 0);
        }
        if (t >= release + 1 && (m->input & INPUT_A_PRESSED)) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
        if (t >= release + 4 && kind != ER_THROW_FIRE_GIANT && (m->input & INPUT_NONZERO_ANALOG) && m->actionState != 1) {
            return set_mario_action(m, ACT_WALKING, 0);
        }
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);
    m->actionTimer++;
    if (m->actionState == 1 && kind <= ER_THROW_STORM && m->actionTimer >= release + 3) {
        return set_mario_action(m, ACT_ER_THROW, ER_THROW_RANG);
    }
    if (m->actionTimer >= end) {
        // (the trap kit's, with LT still held: back down in the crouch, the next one ready)
        if (kind >= ER_THROW_BANANA && (m->input & INPUT_Z_DOWN)) {
            return set_mario_action(m, ACT_CROUCHING, 0);
        }
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/**
 * A throw in the air (actionArg ER_THROW_*): one boomerang after the other, three out of a spin, the
 * fireball. Each holds him up a moment (Devil May Cry's guns), a few to a jump.
 */
s32 act_er_air_throw(struct MarioState *m) {
    static s32 sHand = 0;
    u32 kind = m->actionArg;
    s32 t = m->actionTimer;
    s32 release = kind == ER_THROW_STORM ? 5 : kind == ER_THROW_FIRE_GIANT ? 6 : kind == ER_THROW_HOOK ? 1 : 2;
    s32 end = kind == ER_THROW_STORM ? 12 : kind == ER_THROW_HOOK ? 10 : kind >= ER_THROW_FIRE_BIG ? 14 : 9;
    f32 twist = 0.0f;

    if (t == 0) {
        er_aim_turn(m);
        sHand ^= 1;
        sRangWhirl = 0.0f;
        // (the hook holds him up there without using up a throw)
        if (kind != ER_THROW_HOOK) {
            sErAirThrows++;
        }
        if (kind == ER_THROW_STORM) {
            set_mario_animation(m, MARIO_ANIM_AIR_KICK);
            er_shout(m);
            er_sound(m, SOUND_ACTION_SPIN);
        } else {
            set_mario_animation(m, sHand ? MARIO_ANIM_FIRST_PUNCH : MARIO_ANIM_SECOND_PUNCH);
            er_sound(m, sHand ? SOUND_MARIO_PUNCH_YAH : SOUND_MARIO_PUNCH_WAH);
        }
        set_anim_to_frame(m, 0);
        if (sErAirThrows <= ER_AIR_THROW_HANG || kind == ER_THROW_HOOK) {
            if (m->vel[1] < (er_juggle ? 10.0f : 6.0f)) {
                m->vel[1] = er_juggle ? 10.0f : 6.0f;
            }
        }
        mario_set_forward_vel(m, m->forwardVel * 0.5f);
    }
    // (hanging there while it throws)
    if ((sErAirThrows <= ER_AIR_THROW_HANG || kind == ER_THROW_HOOK) && m->vel[1] < -4.0f) {
        m->vel[1] = -4.0f;
    }
    if (kind == ER_THROW_STORM && t < release) {
        twist = er_lerp(0.0f, 360.0f, er_ease_out((f32)(t + 1) / release));
    }
    if (t == release) {
        er_release(m, kind);
    }
    if (t > release && (m->input & INPUT_B_PRESSED) && er_rang_ready && kind <= ER_THROW_STORM) {
        m->actionState = 1;
    }
    if (t > release && er_enemy_step(m)) {
        return TRUE;
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (t > release) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;

        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    er_twist(m, twist);
    if (++m->actionTimer >= (m->actionState == 1 ? release + 3 : end)) {
        if (m->actionState == 1 && sErAirThrows < ER_AIR_THROWS) {
            return set_mario_action(m, ACT_ER_AIR_THROW, ER_THROW_RANG);
        }
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/* ---- the boomerang's glide ------------------------------------------------------------------- */

/* The rule of the tick comes from glide.rs (sm64_er_set_glide): vertical and forward speed (units a
 * tick), the turn (degrees) and whether the 3 s are up. Mario holds the boomerang overhead (drawn by
 * boomerang.rs, spinning), he doesn't spin. L2 pounds, A lets go; X is moves.rs' (the air throw). */
f32 er_glide_vy = 0.0f;
f32 er_glide_fwd = 0.0f;
f32 er_glide_turn = 0.0f;
s32 er_glide_done = 0;

s32 act_er_glide(struct MarioState *m) {
    if (m->actionTimer == 0) {
        er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.4f);
    }
    if (m->input & INPUT_Z_PRESSED) {
        return set_mario_action(m, ACT_GROUND_POUND, 0);
    }
    if ((m->input & INPUT_A_PRESSED) || er_glide_done) {
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    set_mario_animation(m, MARIO_ANIM_FALL_WITH_LIGHT_OBJ);
    m->faceAngle[1] += er_angle(er_glide_turn);
    mario_set_forward_vel(m, er_glide_fwd);
    // (never any height out of it: the rule's speed is 0 or down)
    m->vel[1] = er_glide_vy <= 0.0f ? er_glide_vy : 0.0f;
    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            return set_mario_action(m, ACT_FREEFALL_LAND, 0);

        case AIR_STEP_HIT_WALL:
            // (a wall ends it: he'd only hug it, sailing into it, for the rest of the 3 s)
            mario_set_forward_vel(m, 0.0f);
            return set_mario_action(m, ACT_FREEFALL, 0);

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    vec3s_set(m->marioObj->header.gfx.angle, 0, m->faceAngle[1], 0);
    m->actionTimer++;
    return FALSE;
}

/* ---- the hook: along its line to what it caught ---------------------------------------------- */

/** where the hook pulls him (SM64 units, hook.rs keeps it on a moving enemy), and to what: 0 an
 * enemy (he pops up in front of it, ready for an air string), 1 a floor (he lands), 2 a wall (he
 * clings to it a moment, A wall kicks off it), 3 a medium enemy (the zip-kick: a kick off it, up
 * into the air) */
f32 er_zip_target[3] = { 0.0f, 0.0f, 0.0f };
s32 er_zip_kind = 0;
#define ER_ZIP_MAX 40
#define ER_ZIP_CLING 24
/* the zip-kick: its pose this long (ticks), bounced up and back off it (units a tick) */
#define ER_ZIP_KICK 10
#define ER_ZIP_KICK_UP 38.0f
#define ER_ZIP_KICK_BACK -8.0f
/* lying along the line drawn tipping into it, and up from it at the wall, over this many ticks
 * (the pitch drawn last) */
#define ER_ZIP_EASE 3
static s16 sErZipPitch = 0;

/**
 * Pulled along the hook's line, faster and faster, to an enemy or a spot on the map. Into a wall
 * on the way: he clings to it (A: a wall kick, Z: let go). At an enemy: a little pop up in front of
 * it, his air string ready (X).
 */
s32 act_er_zip(struct MarioState *m) {
    s32 t = m->actionTimer++;
    // clinging to the wall: still, facing it; A kicks off it
    if (m->actionState == 1) {
        mario_set_forward_vel(m, 0.0f);
        vec3f_set(m->vel, 0.0f, 0.0f, 0.0f);
        set_mario_animation(m, MARIO_ANIM_START_WALLKICK);
        if (m->input & INPUT_A_PRESSED) {
            m->vel[1] = 52.0f;
            m->faceAngle[1] += 0x8000;
            return set_mario_action(m, ACT_WALL_KICK_AIR, 0);
        }
        if ((m->input & INPUT_Z_PRESSED) || t >= ER_ZIP_CLING) {
            m->wallKickTimer = 5;
            return set_mario_action(m, ACT_FREEFALL, 0);
        }
        vec3f_copy(m->marioObj->header.gfx.pos, m->pos);
        // (up from lying along the line over a few ticks)
        vec3s_set(m->marioObj->header.gfx.angle, t < ER_ZIP_EASE - 1 ? (s16)(sErZipPitch * (ER_ZIP_EASE - 2 - t) / ER_ZIP_EASE) : 0, m->faceAngle[1], 0);
        return FALSE;
    }
    // the zip-kick: kicked off it, bouncing up and back (hook.rs lands its blow), his air string free
    if (m->actionState == 2) {
        set_mario_animation(m, MARIO_ANIM_AIR_KICK);
        switch (perform_air_step(m, 0)) {
            case AIR_STEP_LANDED:
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            case AIR_STEP_HIT_LAVA_WALL:
                lava_boost_on_wall(m);
                return TRUE;
        }
        if (t >= ER_ZIP_KICK) {
            return set_mario_action(m, ACT_FREEFALL, 0);
        }
        return FALSE;
    }

    f32 dx = er_zip_target[0] - m->pos[0];
    f32 dy = er_zip_target[1] - m->pos[1];
    f32 dz = er_zip_target[2] - m->pos[2];
    f32 dist = sqrtf(dx * dx + dy * dy + dz * dz);
    f32 speed = 40.0f + 14.0f * (f32) t;
    if (speed > (er_mega ? 24.0f : 110.0f)) {
        speed = er_mega ? 24.0f : 110.0f;
    }
    if (t == 0) {
        set_mario_animation(m, MARIO_ANIM_DIVE);
        er_sound(m, SOUND_MARIO_HOOHOO);
        m->particleFlags |= PARTICLE_DUST;
    }
    // there: at an enemy, a pop up in front of it; on the map, falls from there (onto a ledge, a floor)
    if (dist <= speed || t >= ER_ZIP_MAX) {
        // (at the wall: he clings to it)
        if (er_zip_kind == 2 && dist <= speed) {
            vec3f_copy(m->pos, er_zip_target);
            er_sound(m, SOUND_ACTION_HIT_2);
            m->particleFlags |= PARTICLE_VERTICAL_STAR;
            m->actionState = 1;
            m->actionTimer = 0;
            m->marioObj->header.gfx.angle[0] = (s16)(sErZipPitch * (ER_ZIP_EASE - 1) / ER_ZIP_EASE);
            return FALSE;
        }
        if (er_zip_kind == 3 && dist <= speed) {
            er_sound(m, SOUND_ACTION_HIT_2);
            er_sound(m, SOUND_MARIO_PUNCH_HOO);
            set_mario_animation(m, MARIO_ANIM_AIR_KICK);
            set_anim_to_frame(m, 0);
            m->vel[1] = ER_ZIP_KICK_UP;
            mario_set_forward_vel(m, ER_ZIP_KICK_BACK);
            m->actionState = 2;
            m->actionTimer = 0;
            m->marioObj->header.gfx.angle[0] = 0;
            return FALSE;
        }
        if (er_zip_kind == 0 || er_zip_kind == 3) {
            m->vel[1] = 26.0f;
            mario_set_forward_vel(m, 4.0f);
            er_sound(m, SOUND_ACTION_TWIRL);
        } else {
            m->vel[1] = 10.0f;
            mario_set_forward_vel(m, 6.0f);
        }
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    if (dx * dx + dz * dz > 1.0f) {
        s16 from = m->faceAngle[1];
        m->faceAngle[1] = atan2s(dz, dx);
        // (turned to the line at once, drawn turning)
        if (t == 0) {
            er_turn_lag(m, from);
        }
    }
    m->vel[0] = dx / dist * speed;
    m->vel[1] = dy / dist * speed;
    m->vel[2] = dz / dist * speed;
    m->forwardVel = sqrtf(m->vel[0] * m->vel[0] + m->vel[2] * m->vel[2]);
    m->slideVelX = m->vel[0];
    m->slideVelZ = m->vel[2];

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            // (a floor near the end: he's there; earlier he slides along it on his way)
            if (dist < 300.0f) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;
        case AIR_STEP_HIT_WALL:
            if (m->wall != NULL) {
                s16 from = m->faceAngle[1];
                m->faceAngle[1] = atan2s(m->wall->normal.z, m->wall->normal.x) + 0x8000;
                er_turn_lag(m, from);
            }
            if (er_zip_kind == 2 || dist < 250.0f) {
                er_sound(m, SOUND_ACTION_HIT_2);
                m->particleFlags |= PARTICLE_VERTICAL_STAR;
                m->actionState = 1;
                m->actionTimer = 0;
                m->marioObj->header.gfx.angle[0] = (s16)(sErZipPitch * (ER_ZIP_EASE - 1) / ER_ZIP_EASE);
                return FALSE;
            }
            return set_mario_action(m, ACT_FREEFALL, 0);
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    // (lying along the line, like a dive: tipping over into it over the first ticks)
    {
        s16 pitch = atan2s(sqrtf(dx * dx + dz * dz), dy);
        if (pitch > 0x3000) {
            pitch = 0x3000;
        }
        if (pitch < -0x3000) {
            pitch = -0x3000;
        }
        sErZipPitch = (s16)(-pitch * (t < ER_ZIP_EASE ? t + 1 : ER_ZIP_EASE) / ER_ZIP_EASE);
        m->marioObj->header.gfx.angle[0] = sErZipPitch;
    }
    return FALSE;
}

/* ---- the Chain Chomp ------------------------------------------------------------------------- */

#define ER_CHOMP_LASH 0
#define ER_CHOMP_SWEEP 1
#define ER_CHOMP_SLAM 2
#define ER_CHOMP_LAUNCH 3
static const s8 sChompEnd[4] = { 15, 17, 25, 20 };
/* how far the Chomp lunges, and where it sits by him between blows (SM64 units) */
#define ER_CHOMP_FAR 540.0f
#define ER_CHOMP_HOME 110.0f

/** A random share 0..1 (the sounds' volumes: the same cue again and again must not drone). */
static f32 er_random(void) {
    static u32 seed = 0x2545F491;
    seed = seed * 1664525u + 1013904223u + gAudioRandom;
    return (f32)((seed >> 8) % 1000) / 1000.0f;
}

/** Its bark (SM64's Chomp lunging), at a varied volume, not too loud: it comes with every blow. */
static void er_chomp_bark(UNUSED struct MarioState *m) {
    er_sound_soft((gAudioRandom & 1) ? SOUND_GENERAL_CHAIN_CHOMP1 : SOUND_GENERAL_CHAIN_CHOMP2, 0.45f + 0.25f * er_random());
}

/** The chain: a rattle of iron links (SM64's metal steps and the Chomp's chain), soft and varied. */
static void er_chain(f32 volume) {
    static const u32 links[] = { SOUND_ACTION_METAL_STEP, SOUND_ACTION_METAL_STEP_TIPTOE, SOUND_ACTION_METAL_LANDING, SOUND_GENERAL_CHAIN_CHOMP1 };
    static u32 last = 0;
    u32 k = (u32)(er_random() * 4.0f) % 4;
    if (k == last) {
        k = (k + 1) % 4;
    }
    last = k;
    er_sound_soft(links[k], volume * (0.75f + 0.4f * er_random()));
}

/** A whoosh through the air, a little louder or softer each time (SWISH_AIR_2 is the same sample
 * always at full volume; Mario's own whooshes would cut his steps and voice short). */
static void er_whoosh(f32 volume) {
    er_sound_soft(SOUND_GENERAL_SWISH_AIR, volume * (0.65f + 0.55f * er_random()));
}

/** Mario's shout on a blow, but not on every one (a voice on each lash got tiring). */
static void er_maybe_shout(struct MarioState *m, u32 sound, f32 chance) {
    if (er_random() < chance) {
        er_sound(m, sound);
    }
}

/**
 * X with the Chain Chomp, a string (actionArg): it lunges out on its chain straight ahead and is
 * pulled back, then swung in a wide arc in front, then raised high and crashed down far ahead (a
 * shockwave there). X, Y (moves.rs): the bite, it lunges, bites and tosses them up (they stay up for
 * the air string). X during a blow queues the next.
 */
s32 act_er_chomp(struct MarioState *m) {
    u32 step = m->actionArg < 4 ? m->actionArg : 0;
    s32 t = m->actionTimer;
    s32 end = sChompEnd[step];
    f32 speed = 0.0f;
    f32 twist = 0.0f;
    f32 p;

    if (t == 0) {
        er_chomp_from_here();
        er_aim_turn(m);
        switch (step) {
            case ER_CHOMP_LASH: er_maybe_shout(m, SOUND_MARIO_PUNCH_YAH, 0.55f); break;
            case ER_CHOMP_SWEEP: er_maybe_shout(m, SOUND_MARIO_PUNCH_WAH, 0.55f); break;
            case ER_CHOMP_SLAM: er_sound(m, SOUND_MARIO_HOOHOO); break;
            default: er_maybe_shout(m, SOUND_MARIO_PUNCH_HOO, 0.7f); break;
        }
        // the chain comes off the floor
        er_chain(0.3f);
    }
    if (t >= 3 && step < ER_CHOMP_SLAM && (m->input & INPUT_B_PRESSED)) {
        m->actionState = 1;
    }
    if (t >= end - 6 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }

    switch (step) {
        case ER_CHOMP_LASH:
            set_mario_anim_with_accel(m, MARIO_ANIM_FIRST_PUNCH, 0xC000);
            if (t < 3) {
                er_chomp(-25.0f, er_lerp(ER_CHOMP_HOME, 60.0f, (f32) t / 3), 80.0f, 0.3f);
            } else if (t < 7) {
                p = (f32)(t - 2) / 4;
                er_chomp(er_lerp(-25.0f, 0.0f, p), er_lerp(60.0f, ER_CHOMP_FAR, er_ease_out(p)), 80.0f, 1.0f);
                er_move_hit = ER_HIT_CHOMP;
                speed = 8.0f;
                if (t == 3) {
                    er_whoosh(0.5f);
                }
                if (t == 5) {
                    er_chain(0.4f);
                }
                if (t == 6) {
                    er_chomp_bark(m);
                }
            } else {
                p = (f32)(t - 7) / (end - 7);
                er_chomp(0.0f, er_lerp(ER_CHOMP_FAR, ER_CHOMP_HOME, er_ease_out(p)), 80.0f, 0.0f);
            }
            break;

        case ER_CHOMP_SWEEP:
            // a wide arc in front, from his right to his left, his body turning with it
            set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0xA000);
            if (t < 3) {
                er_chomp(er_lerp(-40.0f, -125.0f, (f32)(t + 1) / 3), er_lerp(ER_CHOMP_HOME, 300.0f, (f32)(t + 1) / 3), 70.0f, 0.4f);
            } else if (t < 10) {
                p = (f32)(t - 2) / 7;
                er_chomp(er_lerp(-125.0f, 130.0f, er_ease_out(p)), 420.0f, 70.0f, 1.0f);
                er_move_hit = ER_HIT_CHOMP;
                if (t == 3) {
                    er_whoosh(0.55f);
                }
                if (t == 5 || t == 8) {
                    er_chain(0.35f);
                }
                if (t == 6) {
                    er_move_fx |= ER_FX_SPIN;
                    er_chomp_bark(m);
                }
                speed = 6.0f;
            } else {
                p = (f32)(t - 10) / (end - 10);
                er_chomp(er_lerp(130.0f, 30.0f, p), er_lerp(420.0f, ER_CHOMP_HOME, er_ease_out(p)), 70.0f, 0.0f);
            }
            break;

        case ER_CHOMP_SLAM:
            // raised high over him, then crashed down far ahead
            set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, t < 7 ? 0xA000 : 0x14000);
            if (t < 7) {
                p = (f32)(t + 1) / 7;
                er_chomp(0.0f, er_lerp(ER_CHOMP_HOME, 140.0f, p), er_lerp(80.0f, 460.0f, er_ease_out(p)), 0.6f);
                if (t == 3) {
                    er_chain(0.35f);
                }
                if (t == 6) {
                    er_whoosh(0.6f);
                }
            } else if (t <= 9) {
                p = (f32)(t - 6) / 3;
                er_chomp(0.0f, er_lerp(140.0f, 470.0f, p), er_lerp(460.0f, 0.0f, p * p), 1.0f);
                speed = 12.0f;
                if (t == 9) {
                    er_move_hit = ER_HIT_CHOMP_END;
                    er_move_fx |= ER_FX_QUAKE;
                    er_chomp_bark(m);
                    er_sound(m, SOUND_OBJ_THWOMP);
                    play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
                } else {
                    er_move_hit = ER_HIT_CHOMP;
                }
            } else {
                p = t < 16 ? 0.0f : (f32)(t - 16) / (end - 16);
                er_chomp(0.0f, er_lerp(470.0f, ER_CHOMP_HOME, er_ease_out(p)), t < 16 ? 0.0f : er_lerp(0.0f, 80.0f, p), 0.0f);
            }
            break;

        default:
            // the bite: a lunge low, jaws shut on them, a toss straight up (the toss's throw from
            // its swing up, with the bite itself)
            set_mario_anim_with_accel(m, t < 6 ? MARIO_ANIM_FIRST_PUNCH : MARIO_ANIM_GROUND_THROW, 0xC000);
            if (t == 6) {
                set_anim_to_frame(m, 3);
            }
            if (t < 3) {
                er_chomp(0.0f, er_lerp(ER_CHOMP_HOME, 60.0f, (f32) t / 3), 50.0f, 0.5f);
            } else if (t < 7) {
                p = (f32)(t - 2) / 4;
                er_chomp(0.0f, er_lerp(60.0f, 330.0f, er_ease_out(p)), 40.0f, t == 6 ? 0.0f : 1.0f);
                speed = 10.0f;
                if (t == 4) {
                    er_whoosh(0.45f);
                }
                if (t == 6) {
                    er_move_hit = ER_HIT_CHOMP_LAUNCH;
                    er_move_fx |= ER_FX_LAUNCH;
                    er_chomp_bark(m);
                    er_cheer(m);
                }
            } else if (t < 11) {
                p = (f32)(t - 6) / 4;
                er_chomp(0.0f, er_lerp(330.0f, 200.0f, p), er_lerp(40.0f, 380.0f, er_ease_out(p)), 0.0f);
            } else {
                p = (f32)(t - 11) / (end - 11);
                er_chomp(0.0f, er_lerp(200.0f, ER_CHOMP_HOME, p), er_lerp(380.0f, 80.0f, er_ease_out(p)), 0.0f);
            }
            break;
    }
    // (its wind-up from where the move found it: the string's blows jumped it metres across,
    // and it jumped out from beside him)
    er_chomp_eased((f32)(t + 1) / (step == ER_CHOMP_SLAM ? 4 : 3));
    if (step == ER_CHOMP_SWEEP) {
        twist = er_chomp_yaw * 0.35f;
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);
    m->actionTimer++;
    if (m->actionState == 1 && step < ER_CHOMP_SLAM && m->actionTimer >= end - 5) {
        return set_mario_action(m, ACT_ER_CHOMP, step + 1);
    }
    if (m->actionTimer >= end) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* the cyclone, tick by tick (30 a second): the coil (the Chomp dragged round behind him, a squat),
 * the launch (faster and faster, the chain paying out), the swing (round and round, steered), the
 * aim (the last turns brought round to where it goes), the release, its flight, the crash, and the
 * Chomp lying there, then reeled in */
#define ER_CYCLONE_LAUNCH 6
#define ER_CYCLONE_SWING 14
#define ER_CYCLONE_AIM 42
#define ER_CYCLONE_RELEASE 50
#define ER_CYCLONE_IMPACT 53
#define ER_CYCLONE_REEL 59
#define ER_CYCLONE_END 68
#define ER_CYCLONE_HITS_FROM 9
/* the Chomp's reach (as er_chomp takes it) swung round, aimed, and at the crash (ER_CHOMP_FAR) */
#define ER_CYCLONE_REACH 400.0f
#define ER_CYCLONE_AIM_REACH 430.0f
/* where the coil drags it to (degrees from his facing: behind him, a little to his right), and his
 * body's turn back from it */
#define ER_CYCLONE_COIL_YAW -170.0f
#define ER_CYCLONE_COIL_BODY -35.0f
/* his body leads the Chomp round by a lag that grows with its speed (degrees, and degrees per
 * degree a tick), closing this share of the way to it each tick */
#define ER_CYCLONE_LAG 10.0f
#define ER_CYCLONE_LAG_PER_SPEED 0.55f
#define ER_CYCLONE_LAG_RATE 0.3f
/* the swing's speed (degrees a tick) round the front, and how much faster it goes there */
#define ER_CYCLONE_SPEED 40.0f
#define ER_CYCLONE_SPEED_FRONT 5.0f
/* the stick steers him while it swings (an SM64 angle a tick) and walks him this fast */
#define ER_CYCLONE_STEER 0x300
#define ER_CYCLONE_WALK 10.0f
/* passing his front a whoosh, passing behind him (degrees) the chain's clink */
#define ER_CYCLONE_CLINK_AT -150.0f
/* how far round it flies on once let go (degrees: the fling's turns, 18 + 10 + 5 + 2) */
#define ER_CYCLONE_FLING_TURN 35.0f

static f32 er_smoothstep(f32 t) {
    if (t < 0.0f) {
        t = 0.0f;
    } else if (t > 1.0f) {
        t = 1.0f;
    }
    return t * t * (3.0f - 2.0f * t);
}

/**
 * Y with the Chain Chomp: the cyclone (Kratos' Blades of Exile). Mario coils, drags it round behind
 * him and lets it out on its chain faster and faster, swings it round and round him (SM64's Bowser
 * swing), steered with the stick, hitting everyone around; the last turns are brought round to
 * where the stick points (or ahead), he lets go and it flies there, a shockwave where it crashes;
 * it lies there a moment and he reels it in.
 */
s32 act_er_chomp_spin(struct MarioState *m) {
    /* the Chomp's yaw (degrees from his facing, positive to his left, any number of turns), how
     * fast it goes round (degrees a tick), his body's twist and its lead on the Chomp, the
     * Chomp's reach and height now, where the aim brings it and how far round that is, where it
     * crashed, and the swing's one bark */
    static f32 sBall = 0.0f;
    static f32 sSpeed = 0.0f;
    static f32 sBody = 0.0f;
    static f32 sLag = 0.0f;
    static f32 sReach = 0.0f;
    static f32 sHeight = 0.0f;
    static f32 sAimFrom = 0.0f;
    static f32 sAimTurn = 0.0f;
    static f32 sCrashYaw = 0.0f;
    static s32 sBarkAt = 0;
    static const s8 coilHop[ER_CYCLONE_LAUNCH] = { 0, 6, 13, 18, 8, 0 };
    static const s8 coilSquat[ER_CYCLONE_LAUNCH] = { -4, -7, -9, -9, -7, -4 };
    static const s8 launchSpeed[ER_CYCLONE_SWING - ER_CYCLONE_LAUNCH] = { 6, 10, 15, 20, 25, 30, 34, 38 };
    static const f32 aimShare[ER_CYCLONE_RELEASE - ER_CYCLONE_AIM] = { 0.10f, 0.11f, 0.12f, 0.125f, 0.13f, 0.135f, 0.14f, 0.14f };
    static const s8 flingSpeed[ER_CYCLONE_IMPACT + 1 - ER_CYCLONE_RELEASE] = { 18, 10, 5, 2 };
    static const s16 flingHeight[ER_CYCLONE_IMPACT + 1 - ER_CYCLONE_RELEASE] = { 140, 150, 90, 0 };
    static const s8 flingBody[ER_CYCLONE_IMPACT + 1 - ER_CYCLONE_RELEASE] = { 4, 8, 11, 13 };
    static const s8 recoverBody[6] = { 14, 8, 2, -2, -1, 0 };
    static const s8 recoverSquat[4] = { -6, -4, -2, -1 };
    s32 t = m->actionTimer;
    f32 speed = 0.0f;
    f32 before = sBall;
    f32 bite = 0.0f;
    f32 squat = 0.0f;
    // (one sound cue a tick at most: it goes round many times)
    s32 cued = FALSE;
    f32 p;

    if (t == 0) {
        er_chomp_from_here();
        sBall = sErChompFrom[0];
        sBody = 0.0f;
        sSpeed = 0.0f;
        sBarkAt = ER_CYCLONE_SWING + 2 + (s32)(er_random() * (ER_CYCLONE_AIM - ER_CYCLONE_SWING - 6));
    }
    if (t >= ER_CYCLONE_REEL - 3 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }
    if (t >= ER_CYCLONE_REEL + 1 && (m->input & INPUT_NONZERO_ANALOG)) {
        return set_mario_action(m, ACT_WALKING, 0);
    }

    if (t < ER_CYCLONE_LAUNCH) {
        // the coil: dragged round behind him from where it was, a hop, his body turned back and
        // down in a squat
        p = er_smoothstep((f32)(t + 1) / ER_CYCLONE_LAUNCH);
        set_mario_anim_with_accel(m, MARIO_ANIM_GRAB_BOWSER, 0x8000);
        sBall = er_lerp_angle(sErChompFrom[0], ER_CYCLONE_COIL_YAW, p);
        sReach = er_lerp(sErChompFrom[1], 170.0f, p);
        sHeight = er_lerp(sErChompFrom[2], 40.0f, p) + coilHop[t];
        sBody = er_lerp(0.0f, ER_CYCLONE_COIL_BODY, p);
        sLag = sBody - sBall;
        squat = coilSquat[t];
        bite = 0.3f;
        if (t == 1) {
            er_chain(0.25f);
            cued = TRUE;
        }
    } else if (t < ER_CYCLONE_RELEASE) {
        if (t < ER_CYCLONE_SWING) {
            // the launch: faster and faster, the chain paying out
            p = (f32)(t - ER_CYCLONE_LAUNCH + 1) / (ER_CYCLONE_SWING - ER_CYCLONE_LAUNCH);
            sSpeed = launchSpeed[t - ER_CYCLONE_LAUNCH];
            sReach = er_lerp(170.0f, ER_CYCLONE_REACH, er_ease_out(p));
            sHeight = er_lerp(40.0f, 80.0f, p);
            if (t == ER_CYCLONE_LAUNCH) {
                er_sound(m, SOUND_MARIO_HOOHOO);
            } else if (t == ER_CYCLONE_LAUNCH + 2) {
                er_chain(0.35f);
                cued = TRUE;
            } else if (t == ER_CYCLONE_LAUNCH + 4) {
                er_sound_soft((gAudioRandom & 1) ? SOUND_GENERAL_CHAIN_CHOMP1 : SOUND_GENERAL_CHAIN_CHOMP2, 0.5f);
                cued = TRUE;
            }
        } else if (t < ER_CYCLONE_AIM) {
            // the swing: round and round, a little faster past his front, tilted (high on his
            // left, low on his right), the chain breathing in and out; the stick steers
            f32 rad = sBall * ER_PI / 180.0f;
            sSpeed = ER_CYCLONE_SPEED + ER_CYCLONE_SPEED_FRONT * cosf(rad);
            sReach = ER_CYCLONE_REACH + 20.0f * sinf(0.45f * t);
            if (m->input & INPUT_NONZERO_ANALOG) {
                m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, ER_CYCLONE_STEER, ER_CYCLONE_STEER);
                speed = ER_CYCLONE_WALK;
            }
        } else {
            // the aim: where it goes (the stick, else ahead), the last turns shared out to bring
            // it round there, a little faster each tick (short of it by the way it flies on once
            // let go, so it crashes there)
            if (t == ER_CYCLONE_AIM) {
                f32 target = (m->input & INPUT_NONZERO_ANALOG) ? er_degrees((s16)(m->intendedYaw - m->faceAngle[1])) : 0.0f;
                sAimTurn = fmodf(target - ER_CYCLONE_FLING_TURN - sBall, 360.0f);
                if (sAimTurn < 0.0f) {
                    sAimTurn += 360.0f;
                }
                if (sAimTurn < 200.0f) {
                    sAimTurn += 360.0f;
                }
                sAimFrom = sReach;
            }
            sSpeed = sAimTurn * aimShare[t - ER_CYCLONE_AIM];
            sReach = er_lerp(sAimFrom, ER_CYCLONE_AIM_REACH, (f32)(t - ER_CYCLONE_AIM + 1) / (ER_CYCLONE_RELEASE - ER_CYCLONE_AIM));
            if (t == ER_CYCLONE_RELEASE - 3) {
                set_mario_animation(m, MARIO_ANIM_RELEASE_BOWSER);
                set_anim_to_frame(m, 0);
            }
        }
        if (t >= ER_CYCLONE_LAUNCH && t < ER_CYCLONE_RELEASE - 3) {
            set_mario_animation(m, MARIO_ANIM_SWINGING_BOWSER);
        }
        sBall += sSpeed;
        if (t >= ER_CYCLONE_SWING) {
            f32 tilt = 75.0f + 55.0f * sinf((sBall - 40.0f) * ER_PI / 180.0f) + 8.0f * sinf(1.3f * t);
            // (raised for the throw over the aim's last two ticks)
            sHeight = t < ER_CYCLONE_RELEASE - 2 ? tilt : er_lerp(tilt, 120.0f, (f32)(t - (ER_CYCLONE_RELEASE - 3)) / 2);
        }
        // his body leads it round (never turning back while it winds up)
        sLag += (ER_CYCLONE_LAG + ER_CYCLONE_LAG_PER_SPEED * sSpeed - sLag) * ER_CYCLONE_LAG_RATE;
        if (sBall + sLag > sBody) {
            sBody = sBall + sLag;
        }
        bite = 0.4f + 0.2f * sinf(1.9f * t);
        if (t % 8 == 4) {
            er_move_fx |= ER_FX_SPIN;
        }
        if (t >= ER_CYCLONE_HITS_FROM) {
            er_move_hit = ER_HIT_CHOMP_SPIN;
        }
        // passing his front: a whoosh; behind him: the chain's clink; once in the swing, a bark
        if (!cued && floorf(sBall / 360.0f) > floorf(before / 360.0f)) {
            er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.22f + 0.12f * er_random());
            cued = TRUE;
        }
        if (!cued && floorf((sBall - ER_CYCLONE_CLINK_AT) / 360.0f) > floorf((before - ER_CYCLONE_CLINK_AT) / 360.0f)) {
            er_chain(0.225f);
            cued = TRUE;
        }
        if (t >= sBarkAt && sBarkAt > 0 && t < ER_CYCLONE_AIM) {
            if (!cued) {
                er_chomp_bark(m);
                cued = TRUE;
                sBarkAt = 0;
            }
        }
        // (the turns all done: back to one, so the numbers stay small)
        if (sBall > 720.0f) {
            sBall -= 360.0f;
            sBody -= 360.0f;
        }
    } else if (t <= ER_CYCLONE_IMPACT) {
        // let go: he faces where his body was, the Chomp flies on, slowing, out to the full
        // chain, up and down onto the crash
        s32 k = t - ER_CYCLONE_RELEASE;
        if (t == ER_CYCLONE_RELEASE) {
            er_fold(m, sBody);
            sBall -= sBody;
            sBody = 0.0f;
            er_cheer(m);
            er_chain(0.4f);
        }
        sBall += flingSpeed[k];
        sReach = er_lerp(ER_CYCLONE_AIM_REACH, ER_CHOMP_FAR, er_ease_out((f32)(k + 1) / (ER_CYCLONE_IMPACT + 1 - ER_CYCLONE_RELEASE)));
        sHeight = flingHeight[k];
        sBody = flingBody[k];
        bite = 1.0f;
        er_move_hit = ER_HIT_CHOMP;
        if (t == ER_CYCLONE_IMPACT) {
            er_move_hit = ER_HIT_CHOMP_END;
            er_move_fx |= ER_FX_QUAKE;
            er_sound(m, SOUND_OBJ_THWOMP);
            er_chomp_bark(m);
            sCrashYaw = sBall;
        }
    } else {
        // it lies there a moment, then he reels it in to beside him, hopping
        s32 k = t - ER_CYCLONE_IMPACT - 1;
        sBody = k < 6 ? recoverBody[k] : 0.0f;
        squat = k < 4 ? recoverSquat[k] : 0.0f;
        if (t < ER_CYCLONE_REEL) {
            sReach = ER_CHOMP_FAR;
            sHeight = 0.0f;
        } else {
            s32 r = t - ER_CYCLONE_REEL;
            p = er_smoothstep((f32)(r + 1) / (ER_CYCLONE_END - ER_CYCLONE_REEL));
            sBall = er_lerp_angle(sCrashYaw, ER_CHOMP_REST_YAW, p * p);
            sReach = er_lerp(ER_CHOMP_FAR, ER_CHOMP_REST_REACH, p);
            sHeight = r < 5 ? 35.0f * sinf((f32) r * ER_PI / 4.0f) : 15.0f * sinf((f32)(r - 5) * ER_PI / 3.0f);
            if (r == 4 || r == 8) {
                er_chain(0.2f);
            }
        }
    }
    er_chomp(sBall, sReach, sHeight, bite);

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, sBody);
    if (squat != 0.0f) {
        m->marioObj->header.gfx.pos[1] = m->pos[1] + squat;
    }
    if (++m->actionTimer >= ER_CYCLONE_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/**
 * The Chain Chomp in the air (actionArg): 0, X, it swings once all the way round him (with an enemy
 * up there each one lifts him a little); 1, Y, the crash: held up over him a moment, then he drops
 * with it under him, a shockwave where they land.
 */
#define ER_CHOMP_AIR_FROM -170.0f
#define ER_CHOMP_AIR_TICKS 10

s32 act_er_chomp_air(struct MarioState *m) {
    s32 t = m->actionTimer;

    if (m->actionArg == 0) {
        if (t == 0) {
            er_shout(m);
            er_sound(m, SOUND_ACTION_SPIN);
            set_mario_animation(m, MARIO_ANIM_AIR_KICK);
            if (er_juggle) {
                m->vel[1] = 16.0f;
                mario_set_forward_vel(m, 4.0f);
            }
        }
        if (t >= 5 && (m->input & INPUT_B_PRESSED)) {
            m->actionState = 1;
        }
        if (t >= 5 && er_enemy_step(m)) {
            er_fold(m, sErTwistDeg);
            return TRUE;
        }
        // (a whole turn from behind him on his right, his body turning after it: er_air_swing_twist)
        er_chomp(ER_CHOMP_AIR_FROM + er_lerp(0.0f, 360.0f, (f32)(t + 1) / ER_CHOMP_AIR_TICKS), 330.0f, 30.0f, 1.0f);
        if (t < ER_CHOMP_AIR_TICKS) {
            er_move_hit = ER_HIT_CHOMP_AIR;
        }
        if (t == 5) {
            er_chomp_bark(m);
        }
        if (m->vel[1] < -10.0f) {
            m->vel[1] = -10.0f;
        }
    } else {
        if (t == 0) {
            er_aim_turn(m);
            er_sound(m, SOUND_MARIO_PUNCH_HOO);
            set_mario_animation(m, MARIO_ANIM_START_GROUND_POUND);
        }
        mario_set_forward_vel(m, 0.0f);
        if (t < 6) {
            m->vel[1] = 0.0f;
            er_chomp(0.0f, er_lerp(ER_CHOMP_HOME, 40.0f, (f32)(t + 1) / 6), er_lerp(80.0f, 300.0f, er_ease_out((f32)(t + 1) / 6)), 0.5f);
        } else {
            if (t == 6) {
                er_sound(m, SOUND_MARIO_GROUND_POUND_WAH);
                set_mario_animation(m, MARIO_ANIM_GROUND_POUND);
                er_chomp_bark(m);
            }
            m->vel[1] = -75.0f;
            er_chomp(0.0f, 60.0f, -40.0f, 1.0f);
            er_move_hit = ER_HIT_CHOMP_AIR;
        }
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (m->actionArg == 1) {
                er_move_hit = ER_HIT_CHOMP_END;
                er_move_fx |= ER_FX_QUAKE;
                er_sound(m, SOUND_OBJ_THWOMP);
                er_sound(m, SOUND_GENERAL_BOWSER_BOMB_EXPLOSION);
                play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
                er_chomp(0.0f, 90.0f, 0.0f, 0.0f);
            } else if (t > 0) {
                er_fold(m, sErTwistDeg);
            }
            return set_mario_action(m, ACT_FREEFALL_LAND, 0);

        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;

        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    if (m->actionArg == 0) {
        er_twist(m, er_air_swing_twist(t, ER_CHOMP_AIR_TICKS, 0.7f));
    }
    if (++m->actionTimer >= (m->actionArg == 0 ? ER_CHOMP_AIR_TICKS + ER_AIR_SWING_SETTLE : 90)) {
        // (he faces where the swing left him: a whole turn, the next one starts where it ended)
        if (m->actionArg == 0) {
            er_fold(m, sErTwistDeg);
        }
        if (m->actionArg == 0 && m->actionState == 1) {
            return set_mario_action(m, ACT_ER_CHOMP_AIR, 0);
        }
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/**
 * With the hammer out, B's punches and kicks (pad X) become its swings: called by
 * set_mario_action with the action SM64 picked. Only for B pressed this frame, empty-handed.
 */
u32 er_hammer_redirect(struct MarioState *m, u32 action, u32 *actionArg) {
    // the swings in the air, two a jump (each holds his fall a moment: more kept him up for good)
    static s32 sAirSwings = 0;

    if (!(m->action & ACT_FLAG_AIR)) {
        sAirSwings = 0;
        sErAirThrows = 0;
        sErSteps = 0;
    }
    if (!(m->input & INPUT_B_PRESSED) || m->heldObj != NULL) {
        return action;
    }
    // the boomerang: every punch and kick is a throw (none in hand: SM64's own)
    if (er_weapon == ER_WEAPON_RANG && er_rang_ready) {
        if (action == ACT_PUNCHING || (action == ACT_MOVE_PUNCHING && *actionArg == 0)
            || ((action == ACT_JUMP_KICK || action == ACT_DIVE) && !(m->action & ACT_FLAG_AIR))) {
            *actionArg = ER_THROW_RANG;
            return ACT_ER_THROW;
        }
        if ((action == ACT_JUMP_KICK || action == ACT_DIVE) && sErAirThrows < ER_AIR_THROWS) {
            *actionArg = ER_THROW_RANG;
            return ACT_ER_AIR_THROW;
        }
    }
    // the Chain Chomp: its lashes, and its swing in the air (two a jump, more with an enemy up there)
    if (er_weapon == ER_WEAPON_CHOMP) {
        if (action == ACT_PUNCHING || (action == ACT_MOVE_PUNCHING && *actionArg == 0)
            || ((action == ACT_JUMP_KICK || action == ACT_DIVE) && !(m->action & ACT_FLAG_AIR))) {
            *actionArg = 0;
            return ACT_ER_CHOMP;
        }
        if ((action == ACT_JUMP_KICK || action == ACT_DIVE) && (sAirSwings < 2 || er_juggle)) {
            sAirSwings++;
            *actionArg = 0;
            return ACT_ER_CHOMP_AIR;
        }
    }
    // an enemy up in the air with him: X goes on with the air string
    if (er_juggle && er_weapon == 0 && (m->action & ACT_FLAG_AIR) && (action == ACT_JUMP_KICK || action == ACT_DIVE)) {
        *actionArg = 0;
        return ACT_ER_AIR_COMBO;
    }
    // SM64's speed kick (B running flat out: a dive with a hop) is the string's first punch: the
    // fists' combo goes on from a run (the dive left him lying there, the string lost)
    if (!er_hammer_mode && er_weapon == 0 && action == ACT_DIVE && !(m->action & ACT_FLAG_AIR)) {
        // (the dive's hop SM64 set before asking for it: none on a punch)
        m->vel[1] = 0.0f;
        *actionArg = 0;
        return ACT_MOVE_PUNCHING;
    }
    if (!er_hammer_mode) {
        return action;
    }
    if (action == ACT_PUNCHING || (action == ACT_MOVE_PUNCHING && *actionArg == 0)) {
        *actionArg = 0;
        return ACT_ER_HAMMER;
    }
    if (action == ACT_JUMP_KICK || action == ACT_DIVE) {
        if (!(m->action & ACT_FLAG_AIR)) {
            *actionArg = 0;
            return ACT_ER_HAMMER;
        }
        if (sAirSwings < 2 || er_juggle) {
            sAirSwings++;
            *actionArg = 0;
            return ACT_ER_HAMMER_AIR;
        }
    }
    return action;
}

/**
 * The tick's report starts empty (libsm64.c, before Mario's update). In a hit-stop his move
 * doesn't run: no hit and nothing to show again, the hammer stays where it was.
 */
void er_moves_begin_tick(int frozen) {
    er_move_hit = ER_HIT_NONE;
    er_move_fx = 0;
    er_move_throw = 0;
    if (!frozen) {
        // (in a hit-stop his move holds: still locked, still squashed)
        er_move_locked = 0;
        er_body_squash = 1.0f;
    }
    if (!frozen && sErBonkWait > 0) {
        sErBonkWait--;
    }
    if (!frozen) {
        er_hammer_scripted = 0;
        er_chomp_scripted = 0;
        sErTick++;
    }
}

/* ---- timing (timing.rs) ---------------------------------------------------------------------- */

/** where the Mikiri stomp lands: the attacker's head (SM64 units), kept up to date by timing.rs */
f32 er_mikiri_target[3] = { 0.0f, 0.0f, 0.0f };
#define ER_MIKIRI_CROUCH 3
#define ER_MIKIRI_MAX 34

/**
 * A just before an enemy's blow lands (timing.rs, Sekiro's Mikiri counter): a quick crouch, then a
 * somersault up over the blow onto the attacker's head, a stomp there that staggers it, and a
 * bounce off it (his air string ready). Invulnerable the whole way: the blow goes under him.
 */
s32 act_er_mikiri(struct MarioState *m) {
    s32 t = m->actionTimer++;
    f32 dx = er_mikiri_target[0] - m->pos[0];
    f32 dz = er_mikiri_target[2] - m->pos[2];
    f32 flat = sqrtf(dx * dx + dz * dz);

    // the tick after the stomp (which held him on the head: its hit is reported with this action
    // still on, combat.rs): the bounce off it
    if (m->actionState == 1) {
        mario_set_forward_vel(m, 6.0f);
        set_mario_action(m, ACT_DOUBLE_JUMP, 0);
        m->vel[1] = 46.0f;
        return TRUE;
    }

    // the wind-up: down into a crouch (anticipation), turned to the enemy
    if (t < ER_MIKIRI_CROUCH) {
        if (t == 0) {
            if (flat > 1.0f) {
                m->faceAngle[1] = atan2s(dz, dx);
            }
            // (its few frames spread over the wind-up)
            set_mario_anim_with_accel(m, MARIO_ANIM_START_CROUCHING, 0x8000);
            er_sound(m, SOUND_ACTION_SIDE_FLIP_UNK);
        }
        mario_set_forward_vel(m, 0.0f);
        m->vel[1] = 0.0f;
        vec3f_copy(m->marioObj->header.gfx.pos, m->pos);
        vec3s_set(m->marioObj->header.gfx.angle, 0, m->faceAngle[1], 0);
        return FALSE;
    }
    // the leap: up to just over the head, its peak right above it
    if (t == ER_MIKIRI_CROUCH) {
        f32 rise = er_mikiri_target[1] + 70.0f - m->pos[1];
        f32 ticks, speed;
        if (rise < 140.0f) {
            rise = 140.0f;
        }
        if (rise > 900.0f) {
            rise = 900.0f;
        }
        m->vel[1] = sqrtf(2.0f * 4.0f * rise);
        ticks = m->vel[1] / 4.0f;
        speed = flat / (ticks > 1.0f ? ticks : 1.0f);
        mario_set_forward_vel(m, speed > 48.0f ? 48.0f : speed);
        set_mario_animation(m, MARIO_ANIM_FORWARD_SPINNING);
        set_anim_to_frame(m, 0);
        er_sound(m, SOUND_MARIO_HOOHOO);
        er_sound(m, SOUND_ACTION_SPIN);
        er_move_fx |= ER_FX_LAUNCH;
    }
    // (on the way: still turned to the enemy, its speed worked out again each tick, so it gets
    // there with the peak of the leap even when the enemy moves)
    if (flat > 20.0f) {
        s16 to = atan2s(dz, dx);
        m->faceAngle[1] = to - approach_s32((s16)(to - m->faceAngle[1]), 0, 0x1000, 0x1000);
    }
    if (t > ER_MIKIRI_CROUCH) {
        f32 ticks = m->vel[1] > 0.0f ? m->vel[1] / 4.0f + 2.0f : 2.0f;
        f32 speed = flat / ticks;
        mario_set_forward_vel(m, flat < 40.0f ? 0.0f : (speed > 48.0f ? 48.0f : speed));
    }
    // over the head, coming down: the stomp, and the bounce off it
    if (t > ER_MIKIRI_CROUCH && m->vel[1] <= 4.0f && flat < 130.0f && m->pos[1] <= er_mikiri_target[1] + 90.0f
        && m->pos[1] >= er_mikiri_target[1] - 120.0f) {
        er_move_hit = ER_HIT_MIKIRI;
        er_move_fx |= ER_FX_SPIN;
        er_sound(m, SOUND_ACTION_BOUNCE_OFF_OBJECT);
        er_sound(m, SOUND_ACTION_HIT_3);
        er_cheer(m);
        m->particleFlags |= PARTICLE_HORIZONTAL_STAR;
        // (held on the head this tick, squashed down onto it: the bounce comes next tick)
        mario_set_forward_vel(m, 0.0f);
        m->vel[1] = 0.0f;
        m->actionState = 1;
        vec3f_copy(m->marioObj->header.gfx.pos, m->pos);
        m->marioObj->header.gfx.pos[1] -= 12.0f;
        return FALSE;
    }
    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            // (he missed it: down on his feet)
            if (t > ER_MIKIRI_CROUCH + 2) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;
        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    if (t >= ER_MIKIRI_MAX) {
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/** the dash strike at most this often (ticks): dash, strike, dash, strike was a loop with the
 * dash's invulnerability half the time */
#define ER_BONK_EVERY 36

/** The dash may end in its strike now (mario_actions_moving.c). */
s32 er_bonk_ready(void) {
    return sErBonkWait == 0;
}

#define ER_BONK_HIT_FROM 2
#define ER_BONK_HIT_TO 5
#define ER_BONK_END 16
#define ER_BONK_CANCEL 9

/**
 * X at the end of the dash (Hades' dash strike): the dash's speed goes into a lunging straight
 * punch, the body leaning into it and back up as it slows (follow-through), then he's free again.
 */
s32 act_er_bonk(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 lean, speed;

    if (t == 0) {
        sErBonkWait = ER_BONK_EVERY;
        if (m->input & INPUT_NONZERO_ANALOG) {
            m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x2000, 0x2000);
        }
        er_aim_turn(m);
        set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH);
        set_anim_to_frame(m, 0);
        er_sound(m, SOUND_MARIO_PUNCH_HOO);
        er_sound(m, SOUND_ACTION_THROW);
        m->particleFlags |= PARTICLE_DUST;
    }
    if (t >= ER_BONK_HIT_FROM && t <= ER_BONK_HIT_TO) {
        er_move_hit = ER_HIT_BONK;
    }
    if (t >= ER_BONK_CANCEL) {
        if (m->input & INPUT_B_PRESSED) {
            return set_mario_action(m, ACT_PUNCHING, 0);
        }
        if (m->input & INPUT_A_PRESSED) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
    }
    // fast out of the dash, slowing; leaning into the blow, then back up
    speed = t < 4 ? 50.0f - 4.0f * t : 34.0f * (1.0f - (f32)(t - 4) / (ER_BONK_END - 4));
    if (!er_ground(m, speed)) {
        return TRUE;
    }
    // (straight on from the dash's flat dive: leaning fully into it, then up again)
    lean = t < 4 ? 1.0f : 1.0f - er_ease_out((f32)(t - 4) / 8.0f);
    m->marioObj->header.gfx.angle[0] = (s16)(0x0E00 * lean);
    // (the punch held at its blow and let go slowly: a heavier follow-through)
    if (t == 4) {
        struct AnimInfo *anim = &m->marioObj->header.gfx.animInfo;
        anim->animFrameAccelAssist = anim->animFrame << 16;
        anim->animAccel = 0x6000;
    }
    if (++m->actionTimer >= ER_BONK_END) {
        return set_mario_action(m, (m->input & INPUT_NONZERO_ANALOG) ? ACT_WALKING : ACT_IDLE, 0);
    }
    return FALSE;
}

/* ---- the Rock Mushroom (rock.rs) ------------------------------------------------------------- */

/** a boulder now (rock.rs turns it on and off) */
s32 er_rock_on = 0;
/** the ram's wait (ticks), and how fast he was falling just before he landed */
static s32 sErRockRam = 0;
/** the highest he's been since he left the ground (a fall from very high kills, as on foot) */
static f32 sErRockPeak = 0.0f;
#define ER_ROCK_RAM_EVERY 45
#define ER_ROCK_TOP 78.0f
#define ER_ROCK_SLOPE 7.5f
#define ER_ROCK_PUSH 1.6f
#define ER_ROCK_FRICTION 0.988f
#define ER_ROCK_HIT_SPEED 16.0f
#define ER_ROCK_SLAM_FALL 48.0f

/** Turned into a boulder in mid-air: his fall counts from here. */
void er_rock_peak_from(f32 y) {
    sErRockPeak = y;
}

/** Off a wall: the speed into it turned back out (a little lost), a thud. */
static void er_rock_bounce(struct MarioState *m, f32 *vx, f32 *vz) {
    f32 nx, nz, d;
    if (m->wall != NULL) {
        nx = m->wall->normal.x;
        nz = m->wall->normal.z;
    } else {
        f32 l = sqrtf(*vx * *vx + *vz * *vz);
        nx = l > 0.1f ? -*vx / l : 0.0f;
        nz = l > 0.1f ? -*vz / l : 0.0f;
    }
    d = *vx * nx + *vz * nz;
    if (d < 0.0f) {
        *vx = (*vx - 2.0f * d * nx) * 0.72f;
        *vz = (*vz - 2.0f * d * nz) * 0.72f;
    }
    if (-d > 14.0f) {
        er_sound(m, SOUND_GENERAL_POUND_ROCK);
        er_move_fx |= ER_FX_GROUND;
        m->particleFlags |= PARTICLE_VERTICAL_STAR;
    }
}

/**
 * The Rock Mushroom (Super Mario Galaxy 2): Mario as a boulder. The ground under him drives it:
 * down a slope he speeds up, up one he slows down, the stick only nudges it (a heavy ball); walls
 * send him back, a long drop lands as a slam. A hops, X rams forward. What he rolls into he hits
 * (rock.rs, combat.rs); rock.rs draws the boulder and his head peeking out of it.
 */
s32 act_er_rock(struct MarioState *m) {
    f32 vx = m->vel[0];
    f32 vz = m->vel[2];
    f32 speed;
    s32 t = m->actionTimer++;

    if (!er_rock_on) {
        mario_set_forward_vel(m, m->forwardVel > 24.0f ? 24.0f : m->forwardVel);
        if (m->actionState == 1) {
            return set_mario_action(m, ACT_FREEFALL, 0);
        }
        return set_mario_action(m, m->forwardVel > 8.0f ? ACT_WALKING : ACT_IDLE, 0);
    }
    if (t == 0) {
        sErRockRam = 0;
        set_mario_animation(m, MARIO_ANIM_START_CROUCHING);
        set_anim_to_frame(m, 8);
    }
    // the stick nudges it
    if (m->input & INPUT_NONZERO_ANALOG) {
        f32 push = ER_ROCK_PUSH * m->intendedMag / 32.0f;
        vx += sins(m->intendedYaw) * push;
        vz += coss(m->intendedYaw) * push;
    }
    // X: a ram, a burst along the way it rolls
    if (sErRockRam > 0) {
        sErRockRam--;
    }
    if ((m->input & INPUT_B_PRESSED) && sErRockRam == 0) {
        f32 l = sqrtf(vx * vx + vz * vz);
        f32 dx = l > 4.0f ? vx / l : sins(m->faceAngle[1]);
        f32 dz = l > 4.0f ? vz / l : coss(m->faceAngle[1]);
        vx += dx * 30.0f;
        vz += dz * 30.0f;
        sErRockRam = ER_ROCK_RAM_EVERY;
        er_sound(m, SOUND_ACTION_SPIN);
        er_shout(m);
        er_move_fx |= ER_FX_SPIN;
    }
    if (m->actionState == 0) {
        // down the slope it's on, faster; rolling, a little slower all the time
        if (m->floor != NULL) {
            vx += m->floor->normal.x * ER_ROCK_SLOPE;
            vz += m->floor->normal.z * ER_ROCK_SLOPE;
        }
        vx *= ER_ROCK_FRICTION;
        vz *= ER_ROCK_FRICTION;
        if (m->input & INPUT_A_PRESSED) {
            m->vel[1] = 40.0f;
            m->actionState = 1;
            sErRockPeak = m->pos[1];
            er_sound(m, SOUND_ACTION_TERRAIN_JUMP + (SOUND_TERRAIN_STONE << 16));
        }
    }
    speed = sqrtf(vx * vx + vz * vz);
    // (downhill it may go on past its top speed on the flat: it brakes back down to it slowly)
    if (speed > ER_ROCK_TOP * 1.4f) {
        vx *= ER_ROCK_TOP * 1.4f / speed;
        vz *= ER_ROCK_TOP * 1.4f / speed;
    } else if (speed > ER_ROCK_TOP && m->actionState == 0 && (m->floor == NULL || m->floor->normal.y > 0.97f)) {
        vx *= 0.97f;
        vz *= 0.97f;
    }
    speed = sqrtf(vx * vx + vz * vz);
    m->vel[0] = vx;
    m->vel[2] = vz;
    m->slideVelX = vx;
    m->slideVelZ = vz;
    m->forwardVel = speed;
    if (speed > 2.0f) {
        m->faceAngle[1] = atan2s(vz, vx);
    }
    if (m->actionState == 0) {
        switch (perform_ground_step(m)) {
            case GROUND_STEP_LEFT_GROUND:
                m->actionState = 1;
                m->vel[1] = 0.0f;
                sErRockPeak = m->pos[1];
                break;
            case GROUND_STEP_HIT_WALL:
                er_rock_bounce(m, &m->vel[0], &m->vel[2]);
                break;
        }
    } else {
        f32 fall = -m->vel[1];
        if (m->pos[1] > sErRockPeak) {
            sErRockPeak = m->pos[1];
        }
        switch (perform_air_step(m, 0)) {
            case AIR_STEP_LANDED:
                m->actionState = 0;
                // (from very high up the boulder is no help: as a fall on foot, the end)
                if (sErRockPeak - m->pos[1] > 2200.0f) {
                    m->health = 0xFF;
                    m->hurtCounter = 0;
                    play_sound(SOUND_MARIO_ATTACKED, m->marioObj->header.gfx.cameraToObject);
                    return drop_and_set_mario_action(m, ACT_HARD_BACKWARD_GROUND_KB, 4);
                }
                if (fall > ER_ROCK_SLAM_FALL) {
                    er_move_hit = ER_HIT_ROCK_SLAM;
                    er_move_fx |= ER_FX_QUAKE;
                    er_sound(m, SOUND_GENERAL_POUND_ROCK);
                    play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
                } else {
                    er_sound(m, SOUND_ACTION_TERRAIN_LANDING + (SOUND_TERRAIN_STONE << 16));
                }
                m->vel[1] = 0.0f;
                break;
            case AIR_STEP_HIT_WALL:
                er_rock_bounce(m, &m->vel[0], &m->vel[2]);
                break;
            case AIR_STEP_HIT_LAVA_WALL:
                lava_boost_on_wall(m);
                return TRUE;
        }
    }
    // rolling into things hits them (rock.rs, combat.rs)
    if (speed > ER_ROCK_HIT_SPEED && er_move_hit == ER_HIT_NONE) {
        er_move_hit = ER_HIT_ROCK;
    }
    // its rumble: stone steps, faster the faster it rolls, soft
    if (m->actionState == 0 && speed > 6.0f) {
        s32 every = 10 - (s32)(speed / 12.0f);
        if (every < 3) {
            every = 3;
        }
        if (t % every == 0) {
            er_sound_soft(SOUND_ACTION_TERRAIN_STEP + (SOUND_TERRAIN_STONE << 16), 0.25f);
        }
    }
    vec3s_set(m->marioObj->header.gfx.angle, 0, m->faceAngle[1], 0);
    return FALSE;
}

/* ---- the hook's moves (hook.rs, bat.rs) ------------------------------------------------------ */

#define ER_HOMERUN_WIND 5
#define ER_HOMERUN_HIT 4
#define ER_HOMERUN_END 22

/**
 * X while the hook drags a small enemy in (hook.rs): a home-run swing at it as it arrives, the
 * hammer if it's out (held level, a bat), else the fist. A wind-up twisted back, the swing round,
 * a long follow-through; the blow sends it flying (bat.rs).
 */
s32 act_er_home_run(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 twist;
    f32 k;

    if (t == 0) {
        er_aim_turn(m);
        set_mario_animation(m, MARIO_ANIM_SECOND_PUNCH);
        set_anim_to_frame(m, 0);
        er_sound(m, SOUND_MARIO_PUNCH_HOO);
    }
    if (t == ER_HOMERUN_WIND) {
        er_sound(m, SOUND_ACTION_SPIN);
        er_shout(m);
    }
    // the twist: back over the wind-up (anticipation), round fast through the blow, on past it
    if (t < ER_HOMERUN_WIND) {
        k = er_ease_out((f32)(t + 1) / ER_HOMERUN_WIND);
        twist = -100.0f * k;
        // (held back, the punch's own frames waiting)
        set_anim_to_frame(m, 0);
    } else if (t < ER_HOMERUN_WIND + ER_HOMERUN_HIT) {
        k = (f32)(t - ER_HOMERUN_WIND + 1) / ER_HOMERUN_HIT;
        twist = er_lerp(-100.0f, 160.0f, k);
        er_move_hit = ER_HIT_HOME_RUN;
    } else {
        k = er_ease_out((f32)(t - ER_HOMERUN_WIND - ER_HOMERUN_HIT + 1) / 8.0f);
        twist = er_lerp(160.0f, 0.0f, k);
    }
    if (er_hammer_mode) {
        // the hammer held level, round with him, a bat
        er_hammer(twist - 90.0f, -10.0f, 1.25f);
    }
    if (!er_ground(m, t >= ER_HOMERUN_WIND && t < ER_HOMERUN_WIND + ER_HOMERUN_HIT ? 14.0f : 0.0f)) {
        return TRUE;
    }
    er_twist(m, twist);
    if (t >= ER_HOMERUN_WIND + ER_HOMERUN_HIT + 6 && (m->input & INPUT_B_PRESSED)) {
        return set_mario_action(m, ACT_PUNCHING, 0);
    }
    if (++m->actionTimer >= ER_HOMERUN_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_SWING_KICK_HITS 12
#define ER_SWING_KICK_END 40

/** Off the rope (colossus.rs) or the AoT hook's line (odm.rs): a flying kick along the swing,
 * hitting what it meets. */
s32 act_er_swing_kick(struct MarioState *m) {
    s32 t = m->actionTimer++;
    if (t == 0) {
        set_mario_animation(m, MARIO_ANIM_AIR_KICK);
        er_sound(m, SOUND_MARIO_YAHOO);
        er_move_fx |= ER_FX_SPIN;
    }
    if (t < ER_SWING_KICK_HITS) {
        er_move_hit = ER_HIT_SWING_KICK;
    }
    // (slowing a little, a dive's arc)
    mario_set_forward_vel(m, m->forwardVel * 0.985f);
    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            return set_mario_action(m, ACT_FREEFALL_LAND, 0);
        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            return set_mario_action(m, ACT_FREEFALL, 0);
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    if (t >= 8 && (m->input & INPUT_B_PRESSED)) {
        return set_mario_action(m, er_air_attack(), 0);
    }
    if (t >= ER_SWING_KICK_END) {
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/* ---- the directional moves (moves.rs) -------------------------------------------------------- */

#define ER_LEAP_CROUCH 3
#define ER_LEAP_MAX 30

/**
 * Forward + Y (Devil May Cry's Stinger / Helm Breaker, a gap closer): a crouch, a leap at the enemy
 * in front with a flip, and from its top the weapon's dive onto it (actionArg: the weapon): the
 * fists' meteor dive, the hammer's plunge, the Chomp's crash, the boomerangs' fan.
 */
s32 act_er_leap(struct MarioState *m) {
    s32 t = m->actionTimer++;
    if (t < ER_LEAP_CROUCH) {
        if (t == 0) {
            er_aim_turn(m);
            set_mario_anim_with_accel(m, MARIO_ANIM_START_CROUCHING, 0x9000);
            er_sound(m, SOUND_ACTION_SIDE_FLIP_UNK);
        }
        mario_set_forward_vel(m, m->forwardVel * 0.5f);
        m->vel[1] = 0.0f;
        if (perform_ground_step(m) == GROUND_STEP_LEFT_GROUND) {
            m->actionTimer = ER_LEAP_CROUCH;
        }
        vec3s_set(m->marioObj->header.gfx.angle, 0, m->faceAngle[1], 0);
        return FALSE;
    }
    if (t == ER_LEAP_CROUCH) {
        // (farther to an enemy further away: the aimed blow's gap, as the lunges)
        f32 far = er_take_lunge(m);
        er_aim_turn(m);
        m->vel[1] = er_mega ? 22.0f : 36.0f;
        mario_set_forward_vel(m, 30.0f + (far > 30.0f ? 30.0f : far));
        set_mario_animation(m, MARIO_ANIM_TRIPLE_JUMP);
        set_anim_to_frame(m, 0);
        er_sound(m, SOUND_MARIO_HOOHOO);
        er_sound(m, SOUND_ACTION_SPIN);
        er_move_fx |= ER_FX_LAUNCH;
    }
    // at the top of the leap: the dive
    if (t > ER_LEAP_CROUCH + 4 && m->vel[1] < 8.0f) {
        switch (m->actionArg) {
            case 1: return set_mario_action(m, ACT_ER_HAMMER_AIR, 1);
            case 2:
                if (er_rang_ready) {
                    return set_mario_action(m, ACT_ER_AIR_THROW, ER_THROW_STORM);
                }
                return set_mario_action(m, ACT_FREEFALL, 0);
            case 3: return set_mario_action(m, ACT_ER_CHOMP_AIR, 1);
            default: return set_mario_action(m, ACT_ER_METEOR, 0);
        }
    }
    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (t > ER_LEAP_CROUCH + 2) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;
        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    if (t >= ER_LEAP_MAX) {
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/* the rising blow: a crouch (its wind-up), up off the ground, the weapon's blow on the way up, a
 * while to follow it with the air string */
#define ER_RISE_CROUCH 3
#define ER_RISE_BLOW 6
#define ER_RISE_END 24

/**
 * X + A together (moves.rs), with whatever is in hand (actionArg: the weapon): up into the air
 * with a rising blow that takes whoever it catches up with him, the start of an air string. The
 * fists' rising spin kick, the hammer's upswing, the Chain Chomp's bite upwards, the boomerangs'
 * launcher thrown on the way up. X once he's up: the weapon's air blow.
 */
s32 act_er_rise(struct MarioState *m) {
    s32 t = m->actionTimer;
    u32 w = m->actionArg;
    s32 up = t - ER_RISE_CROUCH;
    f32 spin = 0.0f;
    f32 p;

    if (t == 0) {
        er_aim_turn(m);
        er_aim_lunge(m);
        set_mario_anim_with_accel(m, MARIO_ANIM_START_CROUCHING, 0x14000);
        if (w == ER_WEAPON_CHOMP) {
            er_chomp_from_here();
        }
    }
    if (t < ER_RISE_CROUCH) {
        // (the crouch on the ground: squashed down, the weapon drawn low)
        p = (f32)(t + 1) / ER_RISE_CROUCH;
        if (w == 1) {
            er_hammer(-20.0f, er_lerp(20.0f, -40.0f, p), 1.1f);
        } else if (w == ER_WEAPON_CHOMP) {
            er_chomp(0.0f, 80.0f, 10.0f, 0.3f);
            er_chomp_eased(p);
        }
        mario_set_forward_vel(m, 0.0f);
        m->vel[1] = 0.0f;
        perform_ground_step(m);
        vec3s_set(m->marioObj->header.gfx.angle, 0, m->faceAngle[1], 0);
        er_squash(m, 1.0f - 0.14f * p, 7.0f * p);
        m->actionTimer++;
        return FALSE;
    }
    if (up == 0) {
        m->vel[1] = w == 1 ? 40.0f : 46.0f;
        mario_set_forward_vel(m, 10.0f + er_take_lunge(m) * 0.5f);
        m->pos[1] += 1.0f;
        er_move_fx |= ER_FX_LAUNCH;
        er_shout(m);
        er_sound_soft(SOUND_GENERAL_SWISH_AIR, 0.45f);
        set_mario_animation(m, w == 0 ? MARIO_ANIM_AIR_KICK : w == 1 ? MARIO_ANIM_HEAVY_THROW : MARIO_ANIM_SECOND_PUNCH);
    }
    if (up < ER_RISE_BLOW) {
        p = (f32)(up + 1) / ER_RISE_BLOW;
        switch (w) {
            case 1:
                // the hammer swung up from the ground in front, over his head
                er_hammer(er_lerp(-20.0f, 0.0f, p), er_lerp(-40.0f, 125.0f, er_ease_out(p)), 1.15f);
                er_move_hit = ER_HIT_HAMMER_RISE;
                break;
            case ER_WEAPON_RANG:
                if (up == 1 && er_rang_ready) {
                    er_release(m, ER_THROW_UPPER);
                }
                break;
            case ER_WEAPON_CHOMP:
                // the Chomp bites its way up in front of him, jaws snapping shut at the top
                er_chomp(0.0f, er_lerp(160.0f, 130.0f, p), er_lerp(20.0f, 260.0f, er_ease_out(p)), p < 0.7f ? 1.0f : 0.0f);
                er_move_hit = ER_HIT_CHOMP_LAUNCH;
                break;
            default:
                // a spinning kick, one turn on the way up (drawn after the step)
                spin = 360.0f * er_ease_out(p);
                er_move_hit = ER_HIT_RISE;
                if (up == 2) {
                    er_move_fx |= ER_FX_SPIN;
                }
                break;
        }
        er_body_squash = er_lerp(1.12f, 1.0f, p);
    } else {
        // X: the weapon's air blow (with whoever it took up, the air string)
        if (m->input & INPUT_B_PRESSED) {
            return set_mario_action(m, er_air_attack(), 0);
        }
        if (w == 1) {
            er_hammer(0.0f, er_lerp(125.0f, 60.0f, (f32)(up - ER_RISE_BLOW) / (ER_RISE_END - ER_RISE_CROUCH - ER_RISE_BLOW)), 1.1f);
        }
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (up > 2) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;
        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    if (spin != 0.0f) {
        er_twist(m, spin);
    }
    if (++m->actionTimer >= ER_RISE_END) {
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    return FALSE;
}

/* ---- the scripted moves (script.rs) --------------------------------------------------------- */

/* A move as data: script.rs works out every tick of it (the animation and the frame of it to
 * show, the body's turn and lean, his speed, squash and sound) and sets it right before the tick;
 * these two actions only play it, with SM64's own steps (walls, ledges, falling off a ledge), on
 * the ground and in the air, its tick count going on from one to the other. Its blows are measured
 * by the mod from the same data (combat.rs). A tick with no frame set ends the move, as a blow on
 * him does (any other action). */
static struct SM64ErScript sErScript;

void er_set_script(const struct SM64ErScript *frame) {
    sErScript = *frame;
}

/** Shows `frame` of the animation playing (a fraction of a frame too, with an accelerated one). */
static void er_script_anim_frame(struct MarioState *m, f32 frame) {
    struct AnimInfo *a = &m->marioObj->header.gfx.animInfo;
    if (a->curAnim == NULL) {
        return;
    }
    if (a->animAccel) {
        // (as set_anim_to_frame: the update about to come moves it on by one step)
        s32 at = (s32)(frame * 65536.0f);
        a->animFrameAccelAssist = (a->curAnim->flags & ANIM_FLAG_FORWARD) ? at + a->animAccel : at - a->animAccel;
        // (an animation the update doesn't move on shows animFrame as it is)
        a->animFrame = (s16) frame;
    } else {
        set_anim_to_frame(m, (s16) frame);
    }
}

/** What a frame does before the step: the animation, his facing, the lock, the sound. */
static void er_script_begin(struct MarioState *m, const struct SM64ErScript *f) {
    if (f->animId >= 0) {
        set_mario_anim_with_accel(m, f->animId, 0x10000);
        if (f->animFrame >= 0.0f) {
            er_script_anim_frame(m, f->animFrame);
        }
    }
    if (f->aim) {
        er_aim_turn(m);
    }
    if (f->turn != 0.0f) {
        m->faceAngle[1] += er_angle(f->turn);
    }
    er_move_locked = f->locked;
    if (f->invulnerable && m->invincTimer < 2) {
        m->invincTimer = 2;
    }
    if (f->sound != 0) {
        if (f->soundVolume > 0.0f) {
            er_sound_soft(f->sound, f->soundVolume);
        } else {
            er_sound(m, f->sound);
        }
    }
    mario_set_forward_vel(m, f->forwardVel);
    if (f->sideVel != 0.0f) {
        // (a footwork step sideways: along his left, the step's walls and ledges as ever)
        s16 left = m->faceAngle[1] + 0x4000;
        m->vel[0] += sins(left) * f->sideVel;
        m->vel[2] += coss(left) * f->sideVel;
    }
}

/** What a frame does after the step (which draws him straight): the lean, roll, twist, squash. */
static void er_script_draw(struct MarioState *m, const struct SM64ErScript *f) {
    // (SM64's drawn pitch tips the nose down, as the script's leans forward: the dive's, the bonk's)
    vec3s_set(m->marioObj->header.gfx.angle, er_angle(f->pitch), m->faceAngle[1], er_angle(f->roll));
    if (f->twist != 0.0f) {
        er_twist(m, f->twist);
    }
    er_squash(m, f->squash > 0.0f ? f->squash : 1.0f, f->dip);
}

/** Into the move's other action (ground or air) with its tick count going on; the new one plays
 * from the next tick. */
static s32 er_script_to(struct MarioState *m, u32 action) {
    s32 t = m->actionTimer;
    set_mario_action(m, action, m->actionArg);
    m->actionTimer = t;
    return FALSE;
}

s32 act_er_script(struct MarioState *m) {
    struct SM64ErScript *f = &sErScript;
    if (!f->active || f->end) {
        f->active = 0;
        return set_mario_action(m, (m->input & INPUT_NONZERO_ANALOG) ? ACT_WALKING : ACT_IDLE, 0);
    }
    f->active = 0;
    er_script_begin(m, f);
    // up off the ground (a hop, a launcher): this tick's step already in the air (its share of
    // the lunge with it), the air action from the next tick
    if (f->setVelY && f->velY > 0.0f) {
        m->vel[1] = f->velY;
        switch (perform_air_step(m, 0)) {
            case AIR_STEP_HIT_WALL:
                mario_set_forward_vel(m, 0.0f);
                break;
            case AIR_STEP_HIT_LAVA_WALL:
                lava_boost_on_wall(m);
                return TRUE;
        }
        er_script_draw(m, f);
        m->actionTimer++;
        return er_script_to(m, ACT_ER_SCRIPT_AIR);
    }
    m->vel[1] = 0.0f;
    if (perform_ground_step(m) == GROUND_STEP_LEFT_GROUND) {
        // (off a ledge: the move goes on in the air)
        er_script_draw(m, f);
        m->actionTimer++;
        return er_script_to(m, ACT_ER_SCRIPT_AIR);
    }
    er_script_draw(m, f);
    m->actionTimer++;
    return FALSE;
}

s32 act_er_script_air(struct MarioState *m) {
    struct SM64ErScript *f = &sErScript;
    if (!f->active || f->end) {
        f->active = 0;
        return set_mario_action(m, ACT_FREEFALL, 0);
    }
    f->active = 0;
    er_script_begin(m, f);
    if (f->setVelY) {
        m->vel[1] = f->velY;
    } else if (f->hold) {
        m->vel[1] = 0.0f;
    }
    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            m->actionTimer++;
            if (f->landEnds) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            er_script_draw(m, f);
            return er_script_to(m, ACT_ER_SCRIPT);
        case AIR_STEP_HIT_WALL:
            mario_set_forward_vel(m, 0.0f);
            break;
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    // (held up: the step's gravity undone)
    if (f->hold && !f->setVelY) {
        m->vel[1] = 0.0f;
    }
    er_script_draw(m, f);
    m->actionTimer++;
    return FALSE;
}
