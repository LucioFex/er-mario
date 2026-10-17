/**
 * er-mario: the combos' moves (moves.rs starts them, combat.rs and hammer.rs read what they do).
 *
 * Bare-handed: the super jump punch (X, X, Y), its meteor dive (X in it) and the tornado (Y, X).
 * With the hammer (L1, er_hammer_mode): X's three swings, Y's slam (in the air: the plunge), the
 * Donkey Kong flurry (X, X, Y), the Bowser spin (Y, X) and a spinning swing in the air.
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
    ER_HIT_GIRO,
    ER_HIT_GIRO_END,
    ER_HIT_HAMMER_AIR,
    ER_HIT_UPPERCUT,
    ER_HIT_UPPERCUT_TOP,
    ER_HIT_METEOR,
    ER_HIT_METEOR_LAND,
    ER_HIT_TORNADO,
    ER_HIT_TORNADO_END,
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
/** throws in the air since he last stood on the ground (the first few hold him up, DMC's guns) */
static s32 sErAirThrows = 0;
/** enemy steps since he last stood on the ground (A off an enemy up there, Devil May Cry's) */
static s32 sErSteps = 0;
#define ER_ENEMY_STEPS 3
#define ER_AIR_THROW_HANG 6
#define ER_AIR_THROWS 10

static u32 er_air_attack(void);

/** the spin of the spinning moves (only one Mario) */
static s16 sErSpin = 0;

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

/** Turns his drawn body by `deg` from where he faces (after the step, which sets it back). */
static void er_twist(struct MarioState *m, f32 deg) {
    m->marioObj->header.gfx.angle[1] = m->faceAngle[1] + (s16)(deg * 65536.0f / 360.0f);
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

/** A grounded step at `speed`; FALSE once he's off the floor (falling). */
static s32 er_ground(struct MarioState *m, f32 speed) {
    mario_set_forward_vel(m, speed);
    if (perform_ground_step(m) == GROUND_STEP_LEFT_GROUND) {
        set_mario_action(m, ACT_FREEFALL, 0);
        return FALSE;
    }
    return TRUE;
}

/* ---- the hammer ---------------------------------------------------------------------------- */

/* the hammer's string of swings: wind-up, blow and recovery ticks */
#define ER_SWINGS 5
static const s8 sSwingWind[ER_SWINGS] = { 3, 3, 6, 2, 7 };
static const s8 sSwingHit[ER_SWINGS] = { 3, 3, 3, 5, 3 };
static const s8 sSwingRest[ER_SWINGS] = { 8, 8, 10, 7, 16 };
#define ER_FINAL_SCALE 1.9f

/**
 * X with the hammer, a string of five (actionArg): from his right to his left, back, an overhead
 * smash into the ground, a full turn with it held out, and a last blow with it grown, that shakes
 * the ground. X during a swing queues the next one (actionState).
 */
s32 act_er_hammer(struct MarioState *m) {
    u32 swing = m->actionArg < ER_SWINGS ? m->actionArg : ER_SWINGS - 1;
    s32 t = m->actionTimer;
    s32 wind = sSwingWind[swing];
    s32 hit = sSwingHit[swing];
    s32 rest = sSwingRest[swing];
    f32 side = swing == 0 ? 1.0f : -1.0f;
    f32 speed = 0.0f;
    f32 twist = 0.0f;
    f32 p;

    if (t == 0) {
        er_aim_turn(m);
        er_sound(m, swing == 4 ? SOUND_MARIO_YAHOO : swing == 2 ? SOUND_MARIO_PUNCH_HOO
                    : swing == 1 ? SOUND_MARIO_PUNCH_WAH : SOUND_MARIO_PUNCH_YAH);
        if (swing == 4) {
            er_sound(m, SOUND_GENERAL_SHORT_STAR);
            er_move_fx |= ER_FX_GROW;
        }
    }
    if (t == wind) {
        er_sound(m, SOUND_ACTION_SPIN);
    }
    if (t >= 2 && swing < ER_SWINGS - 1 && (m->input & INPUT_B_PRESSED)) {
        m->actionState = 1;
    }
    if (t >= wind + hit + 2 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }

    if (swing == 2 || swing == 4) {
        // overhead: raised behind his head, brought down into the ground in front
        f32 size = swing == 4 ? ER_FINAL_SCALE : 1.0f;
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, swing == 4 ? 0xC000 : 0x14000);
        if (t < wind) {
            p = (f32) t / wind;
            er_hammer(0.0f, er_lerp(40.0f, 165.0f, er_ease_out(p)), er_lerp(1.0f, size, p));
        } else if (t < wind + hit) {
            p = (f32)(t - wind + 1) / hit;
            er_hammer(0.0f, er_lerp(165.0f, -40.0f, p * p), size);
            speed = swing == 4 ? 18.0f : 26.0f;
            if (t == wind + hit - 1) {
                er_move_hit = swing == 4 ? ER_HIT_QUAKE : ER_HIT_SMASH_WAVE;
                er_move_fx |= swing == 4 ? ER_FX_QUAKE : ER_FX_GROUND;
                er_sound(m, SOUND_GENERAL_POUND_WOOD_POST);
                if (swing == 4) {
                    er_sound(m, SOUND_OBJ_THWOMP);
                }
                play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
            } else {
                er_move_hit = ER_HIT_HAMMER_SMASH;
            }
        } else {
            p = (f32)(t - wind - hit) / rest;
            er_hammer(0.0f, p < 0.5f ? -40.0f : er_lerp(-40.0f, 10.0f, (p - 0.5f) * 2.0f),
                      p < 0.5f ? size : er_lerp(size, 1.0f, (p - 0.5f) * 2.0f));
        }
    } else if (swing == 3) {
        // a full turn, the hammer held out level, his body turning with it
        set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, 0xC000);
        if (t < wind) {
            er_hammer(er_lerp(-60.0f, -180.0f, (f32)(t + 1) / wind), 5.0f, 1.1f);
        } else if (t < wind + hit) {
            p = (f32)(t - wind + 1) / hit;
            er_hammer(er_lerp(-180.0f, 180.0f, p), 5.0f, 1.1f);
            er_move_hit = ER_HIT_HAMMER;
            if (t % 2 == 0) {
                er_move_fx |= ER_FX_SPIN;
            }
            speed = 8.0f;
        } else {
            p = (f32)(t - wind - hit) / rest;
            er_hammer(er_lerp(180.0f, 200.0f, p), er_lerp(5.0f, -25.0f, p), 1.0f);
        }
        twist = er_hammer_yaw * 0.9f;
    } else {
        set_mario_anim_with_accel(m, swing == 0 ? MARIO_ANIM_FIRST_PUNCH : MARIO_ANIM_SECOND_PUNCH, 0xC000);
        if (t < wind) {
            p = (f32) t / wind;
            er_hammer(side * er_lerp(-70.0f, -120.0f, p), 15.0f, 1.0f);
        } else if (t < wind + hit) {
            p = (f32)(t - wind + 1) / hit;
            er_hammer(side * er_lerp(-120.0f, 80.0f, er_ease_out(p)), er_lerp(10.0f, -5.0f, p), 1.0f);
            er_move_hit = ER_HIT_HAMMER;
            speed = t == wind ? 20.0f : 10.0f;
        } else {
            p = (f32)(t - wind - hit) / rest;
            er_hammer(side * er_lerp(80.0f, 40.0f, p), er_lerp(-5.0f, -30.0f, p), 1.0f);
        }
        twist = er_hammer_yaw * 0.45f;
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, twist);

    m->actionTimer++;
    // the next swing comes as soon as this one has landed (or at its end)
    if (m->actionState == 1 && m->actionTimer >= wind + hit + 2) {
        // (after the full turn he faces where it ended)
        if (swing == 3) {
            m->faceAngle[1] += (s16)(twist * 65536.0f / 360.0f);
        }
        return set_mario_action(m, ACT_ER_HAMMER, swing + 1);
    }
    if (m->actionTimer >= wind + hit + rest) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_SLAM_IMPACT 11
#define ER_SLAM_END 26
/* Y held through the slam's wind-up charges it (ticks): the hammer grows, a chime per level; full,
 * it comes down as the giant hammer's blow */
#define ER_SLAM_CHARGE_AT 7
#define ER_SLAM_CHARGE_LEVEL 12
#define ER_SLAM_CHARGE_MAX 36
#define ER_SLAM_CHARGE_SCALE 0.8f

/* Y is held (lib.rs, sm64_er_set_hold_y) */
int er_hold_y = 0;

/**
 * Y with the hammer: raised high over his head and brought down, the ground shakes all around.
 * actionArg 1: the plunge landing, straight from the blow.
 */
s32 act_er_hammer_slam(struct MarioState *m) {
    s32 t;
    f32 p;
    s32 charge;
    s32 charging;
    s32 full;
    f32 size;

    if (m->actionTimer == 0) {
        if (m->actionArg == 1) {
            m->actionTimer = ER_SLAM_IMPACT;
        } else {
            er_aim_turn(m);
            er_sound(m, SOUND_MARIO_HOOHOO);
        }
    }
    t = m->actionTimer;
    if (t >= ER_SLAM_IMPACT + 5 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }
    // the charge (actionState: ticks of it): held up high while Y is
    charge = m->actionState;
    charging = m->actionArg == 0 && t == ER_SLAM_CHARGE_AT && er_hold_y && charge < ER_SLAM_CHARGE_MAX;
    if (charging) {
        m->actionState = ++charge;
        if (charge % ER_SLAM_CHARGE_LEVEL == 0) {
            er_sound_soft(SOUND_MENU_COLLECT_RED_COIN + ((charge / ER_SLAM_CHARGE_LEVEL - 1) << 16), 0.25f);
            er_move_fx |= ER_FX_GROW;
        }
    }
    size = 1.1f + ER_SLAM_CHARGE_SCALE * (f32) charge / ER_SLAM_CHARGE_MAX;
    full = charge >= ER_SLAM_CHARGE_MAX;

    set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, charging ? 0 : t < ER_SLAM_IMPACT ? 0x9000 : 0x10000);
    if (t < 8) {
        p = (f32) t / 8;
        er_hammer(0.0f, er_lerp(30.0f, 170.0f, er_ease_out(p)), size);
    } else if (t < ER_SLAM_IMPACT) {
        p = (f32)(t - 7) / 4;
        er_hammer(0.0f, er_lerp(170.0f, -45.0f, p * p), size);
    } else {
        p = (f32)(t - ER_SLAM_IMPACT) / (ER_SLAM_END - ER_SLAM_IMPACT);
        er_hammer(0.0f, p < 0.6f ? -45.0f : er_lerp(-45.0f, 10.0f, (p - 0.6f) / 0.4f), p < 0.6f ? size : er_lerp(size, 1.1f, (p - 0.6f) / 0.4f));
    }
    if (t == ER_SLAM_IMPACT && full) {
        // fully charged: the giant hammer's blow
        er_move_hit = ER_HIT_FLURRY_END;
        er_move_fx |= ER_FX_GIANT;
        er_cheer(m);
        er_sound(m, SOUND_OBJ_THWOMP);
        er_sound(m, SOUND_GENERAL_POUND_WOOD_POST);
        play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
    } else if (t == ER_SLAM_IMPACT) {
        er_move_hit = ER_HIT_QUAKE;
        er_move_fx |= ER_FX_QUAKE;
        er_sound(m, SOUND_OBJ_THWOMP);
        er_sound(m, SOUND_GENERAL_POUND_WOOD_POST);
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
        er_hammer(wob * (2.0f + 5.0f * k), 170.0f + sinf((f32) charge * 3.1f) * (2.0f + 4.0f * k), size);
        if (charge % 4 == 0) {
            er_move_fx |= ER_FX_CHARGE;
        }
    }
    if (!er_ground(m, t >= 8 && t < ER_SLAM_IMPACT ? 12.0f : 0.0f)) {
        return TRUE;
    }
    if (charging) {
        f32 k = (f32) charge / ER_SLAM_CHARGE_MAX;
        er_twist(m, sinf((f32) charge * 2.9f) * (1.5f + 4.0f * k));
        // a squat that deepens, bouncing a little with the effort
        m->marioObj->header.gfx.pos[1] = m->pos[1] - (4.0f + 8.0f * k) * (0.7f + 0.3f * sinf((f32) charge * 1.7f));
        return FALSE;
    }
    if (++m->actionTimer >= ER_SLAM_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_LAUNCH_WIND 6
#define ER_LAUNCH_SCRAPE 3
#define ER_LAUNCH_LIFT 3
#define ER_LAUNCH_END 23

/**
 * X, Y with the hammer: the launcher, a golf swing. Down low behind him on his right, scraped
 * along the ground in front, then straight up: whoever it catches flies high into the air
 * (juggle.rs), for Mario to jump after and go on with the air string. A or the stick soon after
 * the blow: off after them.
 */
s32 act_er_hammer_launch(struct MarioState *m) {
    s32 t = m->actionTimer;
    s32 lift = ER_LAUNCH_WIND + ER_LAUNCH_SCRAPE;
    f32 speed = 0.0f;
    f32 p;

    if (t == 0) {
        er_aim_turn(m);
        er_sound(m, SOUND_MARIO_HOOHOO);
    }
    if (t >= lift + 2 && (m->input & INPUT_A_PRESSED)) {
        return set_jumping_action(m, ACT_JUMP, 0);
    }
    if (t >= lift + 5 && (m->input & INPUT_NONZERO_ANALOG)) {
        return set_mario_action(m, ACT_WALKING, 0);
    }

    set_mario_anim_with_accel(m, MARIO_ANIM_SECOND_PUNCH, t < ER_LAUNCH_WIND ? 0x6000 : 0x10000);
    if (t < ER_LAUNCH_WIND) {
        // (taken back slowly: the wind-up tells it's coming)
        p = (f32)(t + 1) / ER_LAUNCH_WIND;
        er_hammer(er_lerp(-60.0f, -150.0f, er_ease_out(p)), er_lerp(10.0f, -35.0f, p), 1.1f);
    } else if (t < lift) {
        p = (f32)(t - ER_LAUNCH_WIND + 1) / ER_LAUNCH_SCRAPE;
        er_hammer(er_lerp(-150.0f, 0.0f, p * p), -35.0f, 1.1f);
        speed = 14.0f;
        if (t == lift - 1) {
            er_move_fx |= ER_FX_GROUND;
            er_sound(m, SOUND_GENERAL_POUND_WOOD_POST);
            play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
        }
    } else if (t < lift + ER_LAUNCH_LIFT) {
        p = (f32)(t - lift + 1) / ER_LAUNCH_LIFT;
        er_hammer(0.0f, er_lerp(-35.0f, 85.0f, er_ease_out(p)), 1.2f);
        er_move_hit = ER_HIT_LAUNCH;
        if (t == lift) {
            er_move_fx |= ER_FX_LAUNCH | ER_FX_GROUND;
            er_cheer(m);
            er_sound(m, SOUND_ACTION_SPIN);
        }
    } else {
        p = (f32)(t - lift - ER_LAUNCH_LIFT) / (ER_LAUNCH_END - lift - ER_LAUNCH_LIFT);
        er_hammer(0.0f, er_lerp(85.0f, 40.0f, p), er_lerp(1.2f, 1.0f, p));
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    er_twist(m, er_hammer_yaw * 0.45f);
    if (++m->actionTimer >= ER_LAUNCH_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_BANGS 8
#define ER_BANG_START 2
#define ER_GIANT_START (ER_BANG_START + ER_BANGS * 3)
#define ER_GIANT_IMPACT (ER_GIANT_START + 9)
#define ER_FLURRY_END (ER_GIANT_IMPACT + 18)
#define ER_GIANT_SCALE 3.6f

/**
 * X, X, Y with the hammer: Donkey Kong's hammer, eight quick blows stepping forward, then it grows
 * giant for one last blow that shakes the ground.
 */
s32 act_er_hammer_flurry(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 speed = 0.0f;
    f32 p;

    if (t == 0) {
        er_sound(m, SOUND_MARIO_YAH_WAH_HOO);
    }
    if (t < ER_BANG_START) {
        er_hammer(0.0f, er_lerp(40.0f, 110.0f, (f32)(t + 1) / ER_BANG_START), 1.0f);
        set_mario_anim_with_accel(m, MARIO_ANIM_FIRST_PUNCH_FAST, 0x10000);
    } else if (t < ER_GIANT_START) {
        s32 bang = (t - ER_BANG_START) / 3;
        s32 k = (t - ER_BANG_START) % 3;
        static const f32 pitches[3] = { 110.0f, 40.0f, -25.0f };
        if (k == 0) {
            er_aim_turn(m);
            set_mario_animation(m, bang % 2 == 0 ? MARIO_ANIM_FIRST_PUNCH_FAST : MARIO_ANIM_SECOND_PUNCH_FAST);
            set_anim_to_frame(m, 0);
        }
        er_hammer((bang % 2 == 0 ? 12.0f : -12.0f), pitches[k], 1.0f);
        speed = 9.0f;
        if (k == 2) {
            er_move_hit = ER_HIT_FLURRY;
            er_move_fx |= ER_FX_BANG;
            er_sound(m, SOUND_GENERAL_POUND_WOOD_POST);
        }
    } else if (t < ER_GIANT_IMPACT - 2) {
        // the giant hammer: it grows raised high
        p = (f32)(t - ER_GIANT_START) / (ER_GIANT_IMPACT - 2 - ER_GIANT_START);
        if (t == ER_GIANT_START) {
            er_aim_turn(m);
            er_cheer(m);
            er_sound(m, SOUND_GENERAL_SHORT_STAR);
            er_move_fx |= ER_FX_GROW;
        }
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0x8000);
        er_hammer(0.0f, er_lerp(110.0f, 165.0f, p), er_lerp(1.0f, ER_GIANT_SCALE, er_ease_out(p * 1.3f)));
    } else if (t <= ER_GIANT_IMPACT) {
        p = (f32)(t - (ER_GIANT_IMPACT - 2)) / 2;
        set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0x18000);
        er_hammer(0.0f, er_lerp(165.0f, -40.0f, p * p), ER_GIANT_SCALE);
        speed = 14.0f;
        if (t == ER_GIANT_IMPACT) {
            er_move_hit = ER_HIT_FLURRY_END;
            er_move_fx |= ER_FX_GIANT;
            er_sound(m, SOUND_GENERAL_BOWSER_BOMB_EXPLOSION);
            er_sound(m, SOUND_OBJ_THWOMP);
            play_mario_heavy_landing_sound(m, SOUND_ACTION_TERRAIN_HEAVY_LANDING);
        } else {
            er_move_hit = ER_HIT_HAMMER_SMASH;
        }
    } else {
        p = (f32)(t - ER_GIANT_IMPACT) / (ER_FLURRY_END - ER_GIANT_IMPACT);
        er_hammer(0.0f, p < 0.5f ? -40.0f : er_lerp(-40.0f, 10.0f, (p - 0.5f) * 2.0f),
                  p < 0.5f ? ER_GIANT_SCALE : er_lerp(ER_GIANT_SCALE, 1.0f, (p - 0.5f) * 2.0f));
        if (t >= ER_GIANT_IMPACT + 6 && (m->input & INPUT_A_PRESSED)) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    if (++m->actionTimer >= ER_FLURRY_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

#define ER_GIRO_RELEASE 32
#define ER_GIRO_END 46

/**
 * Y, X with the hammer: Mario spins with it held out like Bowser by the tail, faster and faster,
 * hitting all around; the release flings the ones it hits.
 */
s32 act_er_hammer_giro(struct MarioState *m) {
    s32 t = m->actionTimer;
    s16 before = sErSpin;
    f32 p;

    if (t == 0) {
        sErSpin = 0;
        before = 0;
        er_sound(m, SOUND_MARIO_HOOHOO);
        er_sound(m, SOUND_OBJ_BOWSER_TAIL_PICKUP);
    }
    if (t < ER_GIRO_RELEASE) {
        p = (f32) t / ER_GIRO_RELEASE;
        set_mario_animation(m, t < 3 ? MARIO_ANIM_GRAB_BOWSER : MARIO_ANIM_SWINGING_BOWSER);
        sErSpin += (s16) er_lerp(0x1200, 0x3400, p);
        // (a full turn: the swing's whoosh)
        if ((u16) sErSpin < (u16) before) {
            er_sound(m, SOUND_ACTION_SPIN);
        }
        if (t % 6 == 3) {
            er_move_fx |= ER_FX_SPIN;
        }
        if (t >= 3) {
            er_move_hit = ER_HIT_GIRO;
        }
        er_hammer(sErSpin * 360.0f / 65536.0f, 8.0f, 1.3f);
        if (!er_ground(m, 0.0f)) {
            return TRUE;
        }
        er_twist(m, sErSpin * 360.0f / 65536.0f);
    } else {
        if (t == ER_GIRO_RELEASE) {
            // he faces where he let go
            m->faceAngle[1] += sErSpin;
            sErSpin = 0;
            set_mario_animation(m, MARIO_ANIM_RELEASE_BOWSER);
            er_move_hit = ER_HIT_GIRO_END;
            er_move_fx |= ER_FX_QUAKE;
            er_cheer(m);
            er_sound(m, SOUND_OBJ_THWOMP);
        }
        p = (f32)(t - ER_GIRO_RELEASE) / (ER_GIRO_END - ER_GIRO_RELEASE);
        er_hammer(er_lerp(0.0f, -60.0f, p), er_lerp(0.0f, -30.0f, p), er_lerp(1.3f, 1.0f, p));
        if (!er_ground(m, 0.0f)) {
            return TRUE;
        }
        if (t >= ER_GIRO_RELEASE + 6 && (m->input & INPUT_A_PRESSED)) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
    }
    if (++m->actionTimer >= ER_GIRO_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/**
 * X (actionArg 0) in the air with the hammer: a full turn with it held out. Y (actionArg 1): the
 * plunge, a moment held up high, then straight down onto it: lands as the slam's blow.
 */
s32 act_er_hammer_air(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 p;

    if (m->actionArg == 0) {
        if (t == 0) {
            er_sound(m, SOUND_MARIO_PUNCH_HOO);
            er_sound(m, SOUND_ACTION_SPIN);
            set_mario_animation(m, MARIO_ANIM_AIR_KICK);
            // juggling: each swing lifts him a little, to stay up with the enemy
            if (er_juggle) {
                m->vel[1] = 16.0f;
                mario_set_forward_vel(m, 4.0f);
            }
        }
        if (t >= 4 && (m->input & INPUT_B_PRESSED)) {
            m->actionState = 1;
        }
        if (t >= 4 && er_enemy_step(m)) {
            return TRUE;
        }
        p = (f32)(t + 1) / 8;
        er_hammer(er_lerp(-160.0f, 200.0f, p), 0.0f, 1.1f);
        if (t < 8) {
            er_move_hit = ER_HIT_HAMMER_AIR;
        }
        // (a moment of hang time)
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
            er_hammer(0.0f, er_lerp(40.0f, 170.0f, er_ease_out((f32)(t + 1) / 6)), 1.1f);
        } else {
            if (t == 6) {
                er_sound(m, SOUND_MARIO_GROUND_POUND_WAH);
                set_mario_animation(m, MARIO_ANIM_GROUND_POUND);
            }
            m->vel[1] = -75.0f;
            er_hammer(0.0f, -80.0f, 1.1f);
            er_move_hit = ER_HIT_HAMMER_AIR;
        }
    }

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            if (m->actionArg == 1) {
                return set_mario_action(m, ACT_ER_HAMMER_SLAM, 1);
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
        er_twist(m, er_hammer_yaw * 0.8f);
    }

    if (++m->actionTimer >= (m->actionArg == 0 ? 10 : 90)) {
        // X during it: the next swing (or with the hammer put away, the fists' air string)
        if (m->actionArg == 0 && m->actionState == 1) {
            m->faceAngle[1] += (s16)(er_hammer_yaw * 0.8f * 65536.0f / 360.0f);
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
        mario_set_forward_vel(m, 6.0f);
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
    m->marioObj->header.gfx.angle[1] = m->faceAngle[1] + sErSpin;

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
    // (diving: pitched down along the fall)
    if (t >= ER_METEOR_HANG) {
        m->marioObj->header.gfx.angle[0] = 0x1800;
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

#define ER_TORNADO_LAST 34
#define ER_TORNADO_END 42

/**
 * Y, X: Melee's Mario Tornado, a spin with his arms out that hits all around, steered with the
 * stick, ending with a harder last turn.
 */
s32 act_er_tornado(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 speed = 0.0f;

    if (t == 0) {
        sErSpin = 0;
        er_sound(m, SOUND_MARIO_HOOHOO);
    }
    if (t < ER_TORNADO_LAST) {
        set_mario_animation(m, MARIO_ANIM_TWIRL);
        sErSpin += 0x3000;
        if (t % 8 == 0) {
            er_sound(m, SOUND_ACTION_TWIRL);
        }
        if (t % 6 == 0) {
            er_move_fx |= ER_FX_SPIN;
        }
        if (t >= 2) {
            er_move_hit = ER_HIT_TORNADO;
        }
        if (m->input & INPUT_NONZERO_ANALOG) {
            m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x800, 0x800);
            speed = 16.0f;
        }
    } else if (t == ER_TORNADO_LAST) {
        sErSpin += 0x3000;
        er_move_hit = ER_HIT_TORNADO_END;
        er_move_fx |= ER_FX_SPIN | ER_FX_LAUNCH;
        er_cheer(m);
        er_sound(m, SOUND_ACTION_SPIN);
    } else {
        set_mario_animation(m, MARIO_ANIM_GENERAL_LAND);
        sErSpin = 0;
        if (t >= ER_TORNADO_LAST + 4 && (m->input & INPUT_A_PRESSED)) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
    }

    if (!er_ground(m, speed)) {
        return TRUE;
    }
    m->marioObj->header.gfx.angle[1] = m->faceAngle[1] + sErSpin;
    if (++m->actionTimer >= ER_TORNADO_END) {
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* the bare-handed string past SM64's punch, punch, kick (actionArg): a spinning backfist, two
 * kicks, and a whirlwind kick */
#define ER_CHAIN_STEPS 3
static const s8 sChainLength[ER_CHAIN_STEPS] = { 10, 13, 20 };

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
        twist = t < 8 ? er_lerp(0.0f, 540.0f, er_ease_out((f32)(t + 1) / 8)) : 180.0f;
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

/**
 * B, Y: the Koopa shell, Mario curled up and rolling fast, steered with the stick, bouncing off
 * walls, hitting whoever he rolls into.
 */
s32 act_er_shell(struct MarioState *m) {
    s32 t = m->actionTimer;
    f32 speed;

    if (t == 0) {
        sErSpin = 0;
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
            // like a shell: off the wall and back the other way
            m->faceAngle[1] += 0x8000;
            er_sound(m, SOUND_ACTION_HIT);
            break;
    }
    // rolling: the whole body spins forward
    sErSpin += 0x2000;
    m->marioObj->header.gfx.angle[0] = sErSpin;

    if (++m->actionTimer >= ER_SHELL_TICKS) {
        m->marioObj->header.gfx.angle[0] = 0;
        return set_mario_action(m, ACT_IDLE, 0);
    }
    return FALSE;
}

/* ---- the boomerang and the fireball: throws ------------------------------------------------- */

#define ER_BIG_CHARGE_MIN 6
#define ER_BIG_CHARGE_MAX 40

static void er_chomp(f32 yaw, f32 reach, f32 height, f32 bite) {
    er_chomp_scripted = 1;
    er_chomp_yaw = yaw;
    er_chomp_reach = reach;
    er_chomp_height = height;
    er_chomp_bite = bite;
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
        default: release = 2; end = 9; break;
    }

    if (t == 0) {
        er_aim_turn(m);
        sHand ^= 1;
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
            set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, 0);
            if (!er_ground(m, 0.0f)) {
                return TRUE;
            }
            er_twist(m, sinf((f32) charge * 2.9f) * (1.5f + 3.0f * k));
            m->marioObj->header.gfx.pos[1] = m->pos[1] - (3.0f + 6.0f * k);
            return FALSE;
        }
        if (charge < ER_BIG_CHARGE_MIN) {
            // (the charge counter isn't the next-throw flag)
            m->actionState = 0;
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

/* ---- the hook: along its line to what it caught ---------------------------------------------- */

/** where the hook pulls him (SM64 units, hook.rs keeps it on a moving enemy), and to what: 0 an
 * enemy (he pops up in front of it, ready for an air string), 1 the map (a wall: he clings to it a
 * moment, A wall kicks off it; a floor: he lands) */
f32 er_zip_target[3] = { 0.0f, 0.0f, 0.0f };
s32 er_zip_kind = 0;
#define ER_ZIP_MAX 40
#define ER_ZIP_CLING 24

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
        vec3s_set(m->marioObj->header.gfx.angle, 0, m->faceAngle[1], 0);
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
        if (er_zip_kind == 0) {
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
        m->faceAngle[1] = atan2s(dz, dx);
    }
    m->vel[0] = dx / dist * speed;
    m->vel[1] = dy / dist * speed;
    m->vel[2] = dz / dist * speed;
    m->forwardVel = sqrtf(m->vel[0] * m->vel[0] + m->vel[2] * m->vel[2]);
    m->slideVelX = m->vel[0];
    m->slideVelZ = m->vel[2];

    switch (perform_air_step(m, 0)) {
        case AIR_STEP_LANDED:
            // (a floor on the way down, or near the end: he's there)
            if (dist < 300.0f || dy < 0.0f) {
                return set_mario_action(m, ACT_FREEFALL_LAND, 0);
            }
            break;
        case AIR_STEP_HIT_WALL:
            if (m->wall != NULL) {
                m->faceAngle[1] = atan2s(m->wall->normal.z, m->wall->normal.x) + 0x8000;
            }
            if (er_zip_kind == 1 || dist < 250.0f) {
                er_sound(m, SOUND_ACTION_HIT_2);
                m->particleFlags |= PARTICLE_VERTICAL_STAR;
                m->actionState = 1;
                m->actionTimer = 0;
                return FALSE;
            }
            return set_mario_action(m, ACT_FREEFALL, 0);
        case AIR_STEP_HIT_LAVA_WALL:
            lava_boost_on_wall(m);
            return TRUE;
    }
    // (lying along the line, like a dive)
    {
        s16 pitch = atan2s(sqrtf(dx * dx + dz * dz), dy);
        if (pitch > 0x3000) {
            pitch = 0x3000;
        }
        if (pitch < -0x3000) {
            pitch = -0x3000;
        }
        m->marioObj->header.gfx.angle[0] = -pitch;
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

static void er_chomp_bark(struct MarioState *m) {
    er_sound(m, (gAudioRandom & 1) ? SOUND_GENERAL_CHAIN_CHOMP1 : SOUND_GENERAL_CHAIN_CHOMP2);
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
        er_aim_turn(m);
        switch (step) {
            case ER_CHOMP_LASH: er_sound(m, SOUND_MARIO_PUNCH_YAH); break;
            case ER_CHOMP_SWEEP: er_sound(m, SOUND_MARIO_PUNCH_WAH); break;
            case ER_CHOMP_SLAM: er_sound(m, SOUND_MARIO_HOOHOO); break;
            default: er_sound(m, SOUND_MARIO_PUNCH_HOO); break;
        }
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
                    er_sound(m, SOUND_GENERAL_SWISH_AIR);
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
                    er_sound(m, SOUND_ACTION_SPIN);
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
            twist = er_chomp_yaw * 0.35f;
            break;

        case ER_CHOMP_SLAM:
            // raised high over him, then crashed down far ahead
            set_mario_anim_with_accel(m, MARIO_ANIM_HEAVY_THROW, t < 7 ? 0xA000 : 0x14000);
            if (t < 7) {
                p = (f32)(t + 1) / 7;
                er_chomp(0.0f, er_lerp(ER_CHOMP_HOME, 140.0f, p), er_lerp(80.0f, 460.0f, er_ease_out(p)), 0.6f);
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
            // the bite: a lunge low, jaws shut on them, a toss straight up
            set_mario_anim_with_accel(m, t < 7 ? MARIO_ANIM_FIRST_PUNCH : MARIO_ANIM_GROUND_THROW, 0xC000);
            if (t < 3) {
                er_chomp(0.0f, er_lerp(ER_CHOMP_HOME, 60.0f, (f32) t / 3), 50.0f, 0.5f);
            } else if (t < 7) {
                p = (f32)(t - 2) / 4;
                er_chomp(0.0f, er_lerp(60.0f, 330.0f, er_ease_out(p)), 40.0f, t == 6 ? 0.0f : 1.0f);
                speed = 10.0f;
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

#define ER_CYCLONE_RELEASE 48
#define ER_CYCLONE_END 60

/**
 * Y with the Chain Chomp: the cyclone, Mario swings it round and round him on its full chain (SM64's
 * Bowser swing), steered with the stick, hitting everyone around a little; at the end he lets it fly
 * ahead, a shockwave where it lands.
 */
s32 act_er_chomp_spin(struct MarioState *m) {
    s32 t = m->actionTimer;
    s16 before = sErSpin;
    f32 speed = 0.0f;
    f32 p;

    if (t == 0) {
        sErSpin = 0;
        before = 0;
        er_sound(m, SOUND_MARIO_HOOHOO);
        er_chomp_bark(m);
    }
    if (t < ER_CYCLONE_RELEASE) {
        p = (f32) t / ER_CYCLONE_RELEASE;
        set_mario_animation(m, t < 3 ? MARIO_ANIM_GRAB_BOWSER : MARIO_ANIM_SWINGING_BOWSER);
        sErSpin += (s16) er_lerp(0x1000, 0x2600, p * 2.0f);
        if ((u16) sErSpin < (u16) before) {
            er_sound(m, SOUND_ACTION_SPIN);
            if (t % 3 == 0) {
                er_chomp_bark(m);
            }
        }
        if (t % 8 == 4) {
            er_move_fx |= ER_FX_SPIN;
        }
        er_chomp(sErSpin * 360.0f / 65536.0f, er_lerp(ER_CHOMP_HOME, 440.0f, (f32) t / 8), 70.0f + sinf((f32) t * 0.6f) * 25.0f, (f32)((t / 3) % 2));
        if (t >= 4) {
            er_move_hit = ER_HIT_CHOMP_SPIN;
        }
        if (m->input & INPUT_NONZERO_ANALOG) {
            m->faceAngle[1] = m->intendedYaw - approach_s32((s16)(m->intendedYaw - m->faceAngle[1]), 0, 0x400, 0x400);
            speed = 10.0f;
        }
        if (!er_ground(m, speed)) {
            return TRUE;
        }
        er_twist(m, sErSpin * 360.0f / 65536.0f);
    } else {
        if (t == ER_CYCLONE_RELEASE) {
            // let go ahead (where the stick points)
            if (m->input & INPUT_NONZERO_ANALOG) {
                m->faceAngle[1] = m->intendedYaw;
            }
            sErSpin = 0;
            set_mario_animation(m, MARIO_ANIM_RELEASE_BOWSER);
            er_cheer(m);
            er_sound(m, SOUND_GENERAL_SWISH_AIR);
        }
        p = (f32)(t - ER_CYCLONE_RELEASE) / (ER_CYCLONE_END - ER_CYCLONE_RELEASE);
        if (t < ER_CYCLONE_RELEASE + 4) {
            f32 q = (f32)(t - ER_CYCLONE_RELEASE + 1) / 4;
            er_chomp(0.0f, er_lerp(440.0f, ER_CHOMP_FAR, q), er_lerp(120.0f, 0.0f, q), 1.0f);
            er_move_hit = ER_HIT_CHOMP;
            if (t == ER_CYCLONE_RELEASE + 3) {
                er_move_hit = ER_HIT_CHOMP_END;
                er_move_fx |= ER_FX_QUAKE;
                er_sound(m, SOUND_OBJ_THWOMP);
                er_chomp_bark(m);
            }
        } else {
            er_chomp(0.0f, er_lerp(ER_CHOMP_FAR, ER_CHOMP_HOME, er_ease_out(p)), er_lerp(0.0f, 80.0f, p), 0.0f);
        }
        if (!er_ground(m, 0.0f)) {
            return TRUE;
        }
        if (t >= ER_CYCLONE_RELEASE + 6 && (m->input & INPUT_A_PRESSED)) {
            return set_jumping_action(m, ACT_JUMP, 0);
        }
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
            return TRUE;
        }
        er_chomp(er_lerp(-170.0f, 190.0f, (f32)(t + 1) / 10), 330.0f, 30.0f, 1.0f);
        if (t < 10) {
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
        er_twist(m, er_chomp_yaw * 0.7f);
    }
    if (++m->actionTimer >= (m->actionArg == 0 ? 12 : 90)) {
        if (m->actionArg == 0 && m->actionState == 1) {
            m->faceAngle[1] += (s16)(er_chomp_yaw * 0.7f * 65536.0f / 360.0f);
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
        er_hammer_scripted = 0;
        er_chomp_scripted = 0;
    }
}
