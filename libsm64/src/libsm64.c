#ifndef SM64_LIB_EXPORT
    #define SM64_LIB_EXPORT
#endif

#include "libsm64.h"

#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <math.h>

#include "decomp/audio/external.h"
#include "play_sound.h"
#include "decomp/include/PR/os_cont.h"
#include "decomp/engine/math_util.h"
#include "decomp/include/sm64.h"
#include "decomp/include/seq_ids.h"
#include "decomp/include/types.h"
#include "decomp/shim.h"
#include "decomp/memory.h"
#include "decomp/global_state.h"
#include "decomp/game/mario.h"
#include "decomp/game/object_stuff.h"
#include "decomp/engine/surface_collision.h"
#include "decomp/engine/graph_node.h"
#include "decomp/engine/geo_layout.h"
#include "decomp/game/rendering_graph_node.h"
#include "decomp/mario/geo.inc.h"
#include "decomp/game/platform_displacement.h"
#include "decomp/game/sound_init.h"

#include "debug_print.h"
#include "load_surfaces.h"
#include "gfx_adapter.h"
#include "load_anim_data.h"
#include "load_audio_data.h"
#include "load_tex_data.h"
#include "obj_pool.h"
#include "fake_interaction.h"
#include "decomp/game/interaction.h"
#include "decomp/include/mario_geo_switch_case_ids.h"
#include "decomp/include/object_fields.h"

static struct AllocOnlyPool *s_mario_geo_pool = NULL;
static struct GraphNode *s_mario_graph_node = NULL;

static bool s_init_global = false;
static bool s_init_one_mario = false;

struct MarioInstance
{
    struct GlobalState *globalState;
};
struct ObjPool s_mario_instance_pool = { 0, 0 };

static void update_button( bool on, u16 button )
{
    gController.buttonPressed &= ~button;

    if( on )
    {
        if(( gController.buttonDown & button ) == 0 )
            gController.buttonPressed |= button;

        gController.buttonDown |= button;
    }
    else
    {
        gController.buttonDown &= ~button;
    }
}

static struct Area *allocate_area( void )
{
    struct Area *result = malloc( sizeof( struct Area ));
    memset( result, 0, sizeof( struct Area ));

    result->flags = 1;
    result->camera = malloc( sizeof( struct Camera ));
    memset( result->camera, 0, sizeof( struct Camera ));

    return result;
}

static void free_area( struct Area *area )
{
    free( area->camera );
    free( area );
}

typedef void (*SM64DebugPrintFunctionPtr)( const char * );
SM64_LIB_FN void sm64_register_debug_print_function( SM64DebugPrintFunctionPtr debugPrintFunction )
{
    g_debug_print_func = debugPrintFunction;
}

typedef void (*SM64PlaySoundFunctionPtr)( uint32_t soundBits, float *pos );
SM64_LIB_FN void sm64_register_play_sound_function( SM64PlaySoundFunctionPtr playSoundFunction )
{
    g_play_sound_func = playSoundFunction;
}


SM64_LIB_FN void sm64_global_init( const uint8_t *rom, uint8_t *outTexture )
{
    if( s_init_global )
        sm64_global_terminate();

    s_init_global = true;

    load_mario_textures_from_rom( rom, outTexture );
    load_mario_anims_from_rom( rom );

    memory_init();
}

SM64_LIB_FN void sm64_global_terminate( void )
{
    if( !s_init_global ) return;

    global_state_bind( NULL );

    if( s_init_one_mario )
    {
        for( int i = 0; i < s_mario_instance_pool.size; ++i )
            if( s_mario_instance_pool.objects[i] != NULL )
                sm64_mario_delete( i );

        obj_pool_free_all( &s_mario_instance_pool );
    }

    s_init_global = false;
    s_init_one_mario = false;

    if( s_mario_geo_pool )
    {
        alloc_only_pool_free( s_mario_geo_pool );
        s_mario_geo_pool = NULL;
    }

    surfaces_unload_all();
    unload_mario_anims();
    memory_terminate();
}

SM64_LIB_FN void sm64_audio_init( const uint8_t *rom ) {
    load_audio_banks( rom );
}

#define SAMPLES_HIGH 544
#define SAMPLES_LOW 528

SM64_LIB_FN uint32_t sm64_audio_tick( uint32_t numQueuedSamples, uint32_t numDesiredSamples, int16_t *audio_buffer ) {
    if ( !g_is_audio_initialized ) {
        DEBUG_PRINT("Attempted to tick audio, but sm64_audio_init() has not been called yet.");
        return 0;
    }

    update_game_sound();

    u32 num_audio_samples = numQueuedSamples < numDesiredSamples ? SAMPLES_HIGH : SAMPLES_LOW;
    for (int i = 0; i < 2; i++)
    {
        create_next_audio_buffer( audio_buffer + i * ( 2 * num_audio_samples ), num_audio_samples );
    }

    return num_audio_samples;
}

SM64_LIB_FN void sm64_static_surfaces_load( const struct SM64Surface *surfaceArray, uint32_t numSurfaces )
{
    surfaces_load_static( surfaceArray, numSurfaces );
}

SM64_LIB_FN int32_t sm64_mario_create( float x, float y, float z )
{
    int32_t marioIndex = obj_pool_alloc_index( &s_mario_instance_pool, sizeof( struct MarioInstance ));
    struct MarioInstance *newInstance = s_mario_instance_pool.objects[marioIndex];

    newInstance->globalState = global_state_create();
    global_state_bind( newInstance->globalState );
    {
        extern void er_moves_reset(void);
        er_moves_reset();
    }

    if( !s_init_one_mario )
    {
        s_init_one_mario = true;
        s_mario_geo_pool = alloc_only_pool_init();
        s_mario_graph_node = process_geo_layout( s_mario_geo_pool, mario_geo_ptr );
    }

    gCurrSaveFileNum = 1;
    gMarioObject = hack_allocate_mario();
    gCurrentArea = allocate_area();
    gCurrentObject = gMarioObject;

    gMarioSpawnInfoVal.startPos[0] = x;
    gMarioSpawnInfoVal.startPos[1] = y;
    gMarioSpawnInfoVal.startPos[2] = z;

    gMarioSpawnInfoVal.startAngle[0] = 0;
    gMarioSpawnInfoVal.startAngle[1] = 0;
    gMarioSpawnInfoVal.startAngle[2] = 0;

    gMarioSpawnInfoVal.areaIndex = 0;
    gMarioSpawnInfoVal.activeAreaIndex = 0;
    gMarioSpawnInfoVal.behaviorArg = 0;
    gMarioSpawnInfoVal.behaviorScript = NULL;
    gMarioSpawnInfoVal.unk18 = NULL;
    gMarioSpawnInfoVal.next = NULL;

    init_mario_from_save_file();

    if( init_mario() < 0 )
    {
        sm64_mario_delete( marioIndex );
        return -1;
    }

    set_mario_action( gMarioState, ACT_SPAWN_SPIN_AIRBORNE, 0);
    find_floor( x, y, z, &gMarioState->floor );

    return marioIndex;
}

/** er-mario: the combos' moves (decomp/game/er_moves.c) */
extern int er_hammer_mode, er_juggle, er_move_hit, er_hammer_scripted, er_hold_y;
extern int er_weapon, er_rang_ready, er_move_throw, er_move_charge, er_chomp_scripted;
extern f32 er_chomp_yaw, er_chomp_reach, er_chomp_height, er_chomp_bite;
extern u32 er_move_fx;
extern f32 er_hammer_yaw, er_hammer_pitch, er_hammer_scale;
void er_moves_begin_tick(int frozen);

/** er-mario: hit-stop frames left, and the button presses made during them */
static int er_hitstop = 0;
static u16 er_held_presses = 0;

/** er-mario: a blow landed: Mario holds still for this many ticks */
SM64_LIB_FN void sm64_er_hitstop(int frames)
{
    if( frames > er_hitstop )
        er_hitstop = frames;
}

SM64_LIB_FN void sm64_mario_tick( int32_t marioId, const struct SM64MarioInputs *inputs, struct SM64MarioState *outState, struct SM64MarioGeometryBuffers *outBuffers )
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to tick non-existant Mario with ID: %u", marioId);
        return;
    }

    global_state_bind( ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState );

    // er-mario: a hit-stop holds Mario (and his animation) still; presses made meanwhile are kept
    // for when he moves again (a combo's next punch wasn't lost in it)
    int frozen = er_hitstop > 0;
    if( frozen )
        er_hitstop--;
    er_moves_begin_tick( frozen );
    update_button( inputs->buttonA, A_BUTTON );
    update_button( inputs->buttonB, B_BUTTON );
    update_button( inputs->buttonZ, Z_TRIG );
    er_held_presses |= gController.buttonPressed;
    if( !frozen )
    {
        gController.buttonPressed |= er_held_presses;
        er_held_presses = 0;
    }

    gMarioState->marioObj->header.gfx.cameraToObject[0] = 0;
    gMarioState->marioObj->header.gfx.cameraToObject[1] = 0;
    gMarioState->marioObj->header.gfx.cameraToObject[2] = 0;
    // (er-mario: his body's sounds are logged for the co-op partner)
    g_er_body_pos = gMarioState->marioObj->header.gfx.cameraToObject;

    gMarioState->area->camera->yaw = atan2s( inputs->camLookZ, inputs->camLookX );

    gController.stickX = -64.0f * inputs->stickX;
    gController.stickY = 64.0f * inputs->stickY;
    gController.stickMag = sqrtf( gController.stickX*gController.stickX + gController.stickY*gController.stickY );

    if( !frozen )
    {
        apply_mario_platform_displacement();
        bhv_mario_update();
        update_mario_platform();
    }

    gfx_adapter_bind_output_buffers( outBuffers );

    geo_process_root_hack_single_node( s_mario_graph_node );

    // (the animations only move on when this does)
    if( !frozen )
        gAreaUpdateCounter++;

    outState->health = gMarioState->health;
    vec3f_copy( outState->position, gMarioState->pos );
    vec3f_copy( outState->velocity, gMarioState->vel );
    outState->faceAngle = (float)gMarioState->faceAngle[1] / 32768.0f * 3.14159f;
    outState->forwardVelocity = gMarioState->forwardVel;
    outState->action = gMarioState->action;

    struct AnimInfo animInfo = gMarioState->marioObj->header.gfx.animInfo;
    outState->animID = animInfo.animID;
    outState->animFrame = animInfo.animFrame;

    outState->flags = gMarioState->flags;
    outState->particleFlags = gMarioState->particleFlags;
    outState->invincTimer = gMarioState->invincTimer;
}

SM64_LIB_FN void sm64_mario_delete( int32_t marioId )
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to delete non-existant Mario with ID: %u", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    if ( g_is_audio_initialized ) {
        stop_sound(SOUND_MARIO_SNORING3, gMarioState->marioObj->header.gfx.cameraToObject);
    }

    free( gMarioObject );
    free_area( gCurrentArea );

    global_state_delete( globalState );
    obj_pool_free_index( &s_mario_instance_pool, marioId );
}

SM64_LIB_FN void sm64_set_mario_action(int32_t marioId, uint32_t action)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    set_mario_action(gMarioState, action, 0);
}

SM64_LIB_FN void sm64_set_mario_action_arg(int32_t marioId, uint32_t action, uint32_t actionArg)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    set_mario_action(gMarioState, action, actionArg);
}

SM64_LIB_FN void sm64_set_mario_animation(int32_t marioId, int32_t animID)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    set_mario_animation(gMarioState, animID);
}

SM64_LIB_FN void sm64_set_mario_anim_frame(int32_t marioId, int16_t animFrame)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->marioObj->header.gfx.animInfo.animFrame = animFrame;
}

SM64_LIB_FN void sm64_set_mario_state(int32_t marioId, uint32_t flags)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->flags = flags;
}

SM64_LIB_FN void sm64_set_mario_position(int32_t marioId, float x, float y, float z)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    // er-mario: a teleport (the mod re-centres SM64's world) moves the jump's peak along, or
    // Mario would scream as if he fell from the old height
    gMarioState->peakHeight += y - gMarioState->pos[1];
    gMarioState->pos[0] = x;
    gMarioState->pos[1] = y;
    gMarioState->pos[2] = z;
    vec3f_copy(gMarioState->marioObj->header.gfx.pos, gMarioState->pos);
}

SM64_LIB_FN void sm64_set_mario_angle(int32_t marioId, float x, float y, float z)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    vec3s_set(gMarioState->faceAngle, (int16_t)(x / 3.14159f * 32768.f), (int16_t)(y / 3.14159f * 32768.f), (int16_t)(z / 3.14159f * 32768.f));
    vec3s_copy(gMarioState->marioObj->header.gfx.angle, gMarioState->faceAngle);
}

SM64_LIB_FN void sm64_set_mario_faceangle(int32_t marioId, float y)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->faceAngle[1] = (int16_t)(y / 3.14159f * 32768.f);
    vec3s_set(gMarioState->marioObj->header.gfx.angle, 0, gMarioState->faceAngle[1], 0);
}

SM64_LIB_FN void sm64_set_mario_velocity(int32_t marioId, float x, float y, float z)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->vel[0] = x;
    gMarioState->vel[1] = y;
    gMarioState->vel[2] = z;
}

SM64_LIB_FN void sm64_set_mario_forward_velocity(int32_t marioId, float vel)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->forwardVel = vel;
}

SM64_LIB_FN void sm64_set_mario_invincibility(int32_t marioId, int16_t timer)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->invincTimer = timer;
}

SM64_LIB_FN void sm64_set_mario_water_level(int32_t marioId, signed int level)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->waterLevel = level;
}

SM64_LIB_FN void sm64_set_mario_gas_level(int32_t marioId, signed int level)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->gasLevel = level;
}

SM64_LIB_FN void sm64_set_mario_health(int32_t marioId, uint16_t health)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->health = health;
    gMarioState->hurtCounter = 0;
    gMarioState->healCounter = 0;
}

SM64_LIB_FN void sm64_mario_take_damage(int32_t marioId, uint32_t damage, uint32_t subtype, float x, float y, float z)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    fake_damage_knock_back(gMarioState, damage, subtype, x, y, z);
}

SM64_LIB_FN void sm64_mario_heal(int32_t marioId, uint8_t healCounter)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->healCounter += healCounter;
}

SM64_LIB_FN void sm64_mario_kill(int32_t marioId)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->health = 0xff;
}

SM64_LIB_FN void sm64_mario_interact_cap(int32_t marioId, uint32_t capFlag, uint16_t capTime, uint8_t playMusic)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    uint16_t capMusic = 0;
    if(gMarioState->action != ACT_GETTING_BLOWN && capFlag != 0)
    {
        gMarioState->flags &= ~MARIO_CAP_ON_HEAD & ~MARIO_CAP_IN_HAND;
        gMarioState->flags |= capFlag;

        switch(capFlag)
        {
            case MARIO_VANISH_CAP:
                if(capTime == 0) capTime = 600;
                capMusic = SEQUENCE_ARGS(4, SEQ_EVENT_POWERUP);
                break;
            case MARIO_METAL_CAP:
                if(capTime == 0) capTime = 600;
                capMusic = SEQUENCE_ARGS(4, SEQ_EVENT_METAL_CAP);
                break;
            case MARIO_WING_CAP:
                if(capTime == 0) capTime = 1800;
                capMusic = SEQUENCE_ARGS(4, SEQ_EVENT_POWERUP);
                break;
        }

        if (capTime > gMarioState->capTimer) {
            gMarioState->capTimer = capTime;
        }

        if ((gMarioState->action & ACT_FLAG_IDLE) || gMarioState->action == ACT_WALKING) {
            gMarioState->flags |= MARIO_CAP_IN_HAND;
            set_mario_action(gMarioState, ACT_PUTTING_ON_CAP, 0);
        } else {
            gMarioState->flags |= MARIO_CAP_ON_HEAD;
        }

        play_sound(SOUND_MENU_STAR_SOUND, gMarioState->marioObj->header.gfx.cameraToObject);
        play_sound(SOUND_MARIO_HERE_WE_GO, gMarioState->marioObj->header.gfx.cameraToObject);

        if (playMusic != 0 && capMusic != 0) {
            play_cap_music(capMusic);
        }
    }
}

SM64_LIB_FN void sm64_mario_extend_cap(int32_t marioId, uint16_t capTime)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    gMarioState->capTimer += capTime;
}

SM64_LIB_FN bool sm64_mario_attack(int32_t marioId, float x, float y, float z, float hitboxHeight)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
    {
        DEBUG_PRINT("Tried to use non-existant Mario with ID: %d", marioId);
        return false;
    }

    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );

    return fake_interact_bounce_top(gMarioState, x, y, z, hitboxHeight);
}

SM64_LIB_FN uint32_t sm64_surface_object_create( const struct SM64SurfaceObject *surfaceObject )
{
    uint32_t id = surfaces_load_object( surfaceObject );
    for (int i = 0; i < s_mario_instance_pool.size; ++i) {
        struct MarioInstance *instance = s_mario_instance_pool.objects[i];
        if (instance != NULL)
            er_attach_platform(instance->globalState, surfaces_object_get_transform_ptr(id));
    }
    return id;
}

SM64_LIB_FN void sm64_surface_object_move( uint32_t objectId, const struct SM64ObjectTransform *transform )
{
    surface_object_update_transform( objectId, transform );
}

SM64_LIB_FN void sm64_surface_object_delete( uint32_t objectId )
{
    // A mario standing on the platform that is being destroyed will have a pointer to freed memory if we don't clear it.
    for( int i = 0; i < s_mario_instance_pool.size; ++i )
    {
        if( s_mario_instance_pool.objects[i] == NULL )
            continue;

        struct GlobalState *state = ((struct MarioInstance *)s_mario_instance_pool.objects[ i ])->globalState;
        er_detach_platform(state, surfaces_object_get_transform_ptr(objectId));
    }

    surfaces_unload_object( objectId );
}


SM64_LIB_FN int32_t sm64_surface_find_wall_collision( float *xPtr, float *yPtr, float *zPtr, float offsetY, float radius )
{
    return f32_find_wall_collision( xPtr, yPtr, zPtr, offsetY, radius );
}

SM64_LIB_FN int32_t sm64_surface_find_wall_collisions( struct SM64WallCollisionData *colData )
{
    return find_wall_collisions( colData );
}

SM64_LIB_FN float sm64_surface_find_ceil( float posX, float posY, float posZ, struct SM64SurfaceCollisionData **pceil )
{
    return find_ceil( posX, posY, posZ, pceil );
}

SM64_LIB_FN float sm64_surface_find_floor_height_and_data( float xPos, float yPos, float zPos, struct SM64FloorCollisionData **floorGeo )
{
    return find_floor_height_and_data( xPos, yPos, zPos, floorGeo );
}

SM64_LIB_FN float sm64_surface_find_floor_height( float x, float y, float z )
{
    return find_floor_height( x, y, z );
}

SM64_LIB_FN float sm64_surface_find_floor( float xPos, float yPos, float zPos, struct SM64SurfaceCollisionData **pfloor )
{
    return find_floor( xPos, yPos, zPos, pfloor );
}

SM64_LIB_FN float sm64_surface_find_water_level( float x, float z )
{
    return find_water_level( x, z );
}

SM64_LIB_FN float sm64_surface_find_poison_gas_level( float x, float z )
{
    return find_poison_gas_level( x, z );
}

SM64_LIB_FN void sm64_seq_player_play_sequence(uint8_t player, uint8_t seqId, uint16_t arg2)
{
    seq_player_play_sequence(player,seqId,arg2);
}

SM64_LIB_FN void sm64_play_music(uint8_t player, uint16_t seqArgs, uint16_t fadeTimer)
{
    play_music(player,seqArgs,fadeTimer);
}

SM64_LIB_FN void sm64_stop_background_music(uint16_t seqId)
{
    stop_background_music(seqId);
}

SM64_LIB_FN void sm64_fadeout_background_music(uint16_t arg0, uint16_t fadeOut)
{
    fadeout_background_music(arg0,fadeOut);
}

SM64_LIB_FN uint16_t sm64_get_current_background_music()
{
    return get_current_background_music();
}

SM64_LIB_FN void sm64_play_sound(int32_t soundBits, float *pos)
{
    play_sound(soundBits,pos);
}

SM64_LIB_FN void sm64_play_sound_global(int32_t soundBits)
{
    play_sound(soundBits,gGlobalSoundSource);
}

/* er-mario: a sound at a share of its volume (0-1): played from far enough in front of the camera
 * that SM64's own distance fall-off brings it down that much (VOLUME_RANGE_UNK1, a level's reach
 * of 20000, a third of it for banks 3 up) */
void er_play_sound_volume(int32_t soundBits, float volume)
{
    static float spots[8][3];
    static int next = 0;
    u8 bank = (soundBits >> 28) & 0xF;
    float reach = 20000.0f / (bank < 3 ? 2.0f : 3.0f);
    float *pos = spots[next];
    if (volume > 1.0f) volume = 1.0f;
    if (volume < 0.05f) volume = 0.05f;
    next = (next + 1) % 8;
    pos[0] = 0.0f;
    pos[1] = 0.0f;
    pos[2] = -(1.0f - volume) / 0.9f * reach;
    play_sound(soundBits, pos);
}

SM64_LIB_FN void sm64_play_sound_volume(int32_t soundBits, float volume)
{
    er_play_sound_volume(soundBits, volume);
}

/* er-mario (co-op): a partner's sound at a share of its volume, from a spot of that partner's own
 * (`slot` 0-3): SM64 tells a sound that goes on (a slide, the wind) by where it comes from, so
 * each refresh of it must come from the same spot, and the others' sounds aren't moved by it */
SM64_LIB_FN void sm64_er_play_partner(int32_t soundBits, uint32_t slot, float volume)
{
    static float spots[4][3];
    u8 bank = (soundBits >> 28) & 0xF;
    float reach = 20000.0f / (bank < 3 ? 2.0f : 3.0f);
    float *pos = spots[slot & 3];
    if (volume > 1.0f) volume = 1.0f;
    if (volume < 0.05f) volume = 0.05f;
    pos[0] = 0.0f;
    pos[1] = 0.0f;
    pos[2] = -(1.0f - volume) / 0.9f * reach;
    play_sound(soundBits, pos);
}

/* er-mario (co-op): the sounds Mario's own body made since the last call (his voice, steps,
 * blows), up to `max` into `out`; how many */
SM64_LIB_FN uint32_t sm64_er_take_sounds(uint32_t *out, uint32_t max)
{
    uint32_t n = (uint32_t)g_er_sound_count < max ? (uint32_t)g_er_sound_count : max;
    for (uint32_t i = 0; i < n; i++)
        out[i] = g_er_sound_log[i];
    g_er_sound_count = 0;
    return n;
}

SM64_LIB_FN void sm64_set_sound_volume(float vol)
{
    gAudioVolume = vol;
}


extern float er_part_matrices[64][16];
extern int er_part_count;
extern int er_tri_part[SM64_GEO_MAX_TRIANGLES];
extern float er_local_pos[SM64_GEO_MAX_TRIANGLES * 9];
extern float er_local_normal[SM64_GEO_MAX_TRIANGLES * 9];

SM64_LIB_FN int sm64_er_get_parts( float *matrices_out, int *tri_part_out, float *local_pos_out, float *local_normal_out )
{
    if( matrices_out ) memcpy( matrices_out, er_part_matrices, sizeof( float ) * 16 * er_part_count );
    if( tri_part_out ) memcpy( tri_part_out, er_tri_part, sizeof( er_tri_part ) );
    if( local_pos_out ) memcpy( local_pos_out, er_local_pos, sizeof( er_local_pos ) );
    if( local_normal_out ) memcpy( local_normal_out, er_local_normal, sizeof( er_local_normal ) );
    return er_part_count;
}

/* er-mario: SM64's C-up head look: Mario's head turns (yaw, pitch in radians, relative to his body) */
int er_head_active = 0;
s16 er_head_yaw = 0, er_head_pitch = 0;

// er-mario: Mario picks up an Elden Ring enemy like a Bob-omb. SM64's carrying actions need a
// held object; this stand-in is it (the mod moves the real enemy to Mario's hands).
static struct Object er_held_stand_in;

static struct MarioState *er_bind(int32_t marioId)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return NULL;
    global_state_bind( ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState );
    return gMarioState;
}

// ER Mario: no sounds from the ticks in between (see play_sound).
SM64_LIB_FN void sm64_er_mute(int32_t mute)
{
    extern int g_er_mute;
    g_er_mute = mute;
}

// The floor under Mario is lava (the mod knows from the game's floor material): SM64's lava boost
// (check_lava_boost), but one wedge of health instead of three: Elden Ring's lava pools are wide.
// Does nothing in the air, so it can be called every tick.
SM64_LIB_FN void sm64_er_lava(int32_t marioId)
{
    struct MarioState *m = er_bind(marioId);
    if (!m || m->health < 0x100 || m->action == ACT_LAVA_BOOST || (m->action & ACT_FLAG_INTANGIBLE)) return;
    if (!(m->action & ACT_FLAG_RIDING_SHELL) && m->pos[1] < m->floorHeight + 10.0f) {
        m->hurtCounter += 4;
        update_mario_sound_and_camera(m);
        drop_and_set_mario_action(m, ACT_LAVA_BOOST, 0);
    }
}

/* Elden Ring's ailments and burns on Mario (status.rs): kind 1 coughs (poison, rot), 2 shivers
 * (frostbite), 3 falls asleep, 4 wakes up, 5 is shocked (madness), 6 catches fire for `ticks`
 * (SM64's burning run; the burn's damage is Elden Ring's, not SM64's), 7 stops coughing or
 * shivering (they'd loop until the stick moves). Only from a calm action: on the ground, not hurt
 * or in a move, holding nothing (a fire also in the air, not hanging, on a shell or with the Metal
 * cap). Returns whether it took (status.rs tries again a moment later). */
extern int er_soft_burn;
SM64_LIB_FN int32_t sm64_er_status(int32_t marioId, int32_t kind, int32_t ticks)
{
    struct MarioState *m = er_bind(marioId);
    if (!m || m->health < 0x100 || (m->action & ACT_FLAG_INTANGIBLE)) return 0;
    u32 group = m->action & ACT_GROUP_MASK;
    int grounded = group == ACT_GROUP_STATIONARY || group == ACT_GROUP_MOVING;
    int calm = grounded && !(m->action & ACT_FLAG_INVULNERABLE) && m->heldObj == NULL;
    int burning = m->action == ACT_BURNING_GROUND || m->action == ACT_BURNING_JUMP || m->action == ACT_BURNING_FALL;
    switch (kind) {
        case 1:
            if (!calm) return 0;
            set_mario_action(m, ACT_COUGHING, 0);
            return 1;
        case 2:
            if (!calm) return 0;
            set_mario_action(m, ACT_SHIVERING, 0);
            return 1;
        case 3:
            if (!calm) return 0;
            set_mario_action(m, ACT_START_SLEEPING, 0);
            return 1;
        case 4:
            if (m->action == ACT_START_SLEEPING || m->action == ACT_SLEEPING) set_mario_action(m, ACT_WAKING_UP, 0);
            return 1;
        case 5:
            if (!calm) return 0;
            drop_and_set_mario_action(m, ACT_SHOCKED, 0);
            return 1;
        case 6:
            if (burning) return 1;
            if ((m->action & (ACT_FLAG_SWIMMING | ACT_FLAG_METAL_WATER | ACT_FLAG_HANGING | ACT_FLAG_RIDING_SHELL | ACT_FLAG_INVULNERABLE | ACT_FLAG_ON_POLE))
                || (m->flags & MARIO_METAL_CAP) || m->heldObj != NULL) {
                return 0;
            }
            er_soft_burn = 1;
            m->marioObj->oMarioBurnTimer = 160 - 2 * (ticks < 10 ? 10 : ticks > 80 ? 80 : ticks);
            play_sound(SOUND_MARIO_ON_FIRE, m->marioObj->header.gfx.cameraToObject);
            drop_and_set_mario_action(m, (m->action & ACT_FLAG_AIR) ? ACT_BURNING_FALL : ACT_BURNING_GROUND, 1);
            return 1;
        case 7:
            if (m->action == ACT_COUGHING || m->action == ACT_SHIVERING) set_mario_action(m, ACT_IDLE, 0);
            return 1;
    }
    return 0;
}

SM64_LIB_FN void sm64_er_pick_up(int32_t marioId)
{
    struct MarioState *m = er_bind(marioId);
    if (!m) return;
    memset(&er_held_stand_in, 0, sizeof(er_held_stand_in));
    m->heldObj = NULL;
    m->usedObj = &er_held_stand_in;
    // (no stale hand position from an earlier carry: the mod waits for SM64 to place the hands)
    vec3f_set(m->marioBodyState->heldObjLastPosition, 0.0f, 0.0f, 0.0f);
    if (m->action & ACT_FLAG_DIVING) {
        // a dive grab, like SM64's into a Bob-omb: held right away; in the air the dive goes on
        // and lands into the dive pickup, on the ground (belly slide) it's that at once
        mario_grab_used_object(m);
        m->marioBodyState->grabPos = GRAB_POS_LIGHT_OBJ;
        if (!(m->action & ACT_FLAG_AIR)) {
            set_mario_action(m, ACT_DIVE_PICKING_UP, 0);
        }
    } else {
        set_mario_action(m, ACT_PICKING_UP, 0);
    }
}

// Whether Mario still holds it; `pos` gets where it is (between his hands, SM64 coordinates).
SM64_LIB_FN int sm64_er_held(int32_t marioId, float *pos)
{
    struct MarioState *m = er_bind(marioId);
    if (!m) return 0;
    if (pos) {
        pos[0] = m->marioBodyState->heldObjLastPosition[0];
        pos[1] = m->marioBodyState->heldObjLastPosition[1];
        pos[2] = m->marioBodyState->heldObjLastPosition[2];
    }
    return m->heldObj != NULL || (m->action == ACT_PICKING_UP && m->usedObj == &er_held_stand_in);
}

// Lets go (without throwing): Mario's carrying actions end.
SM64_LIB_FN void sm64_er_drop(int32_t marioId)
{
    struct MarioState *m = er_bind(marioId);
    if (!m) return;
    if (m->heldObj == &er_held_stand_in || m->usedObj == &er_held_stand_in) {
        m->heldObj = NULL;
        m->usedObj = NULL;
        if (m->action & ACT_FLAG_AIR) set_mario_action(m, ACT_FREEFALL, 0);
        else set_mario_action(m, ACT_IDLE, 0);
    }
}

extern float er_ladder_rate;
SM64_LIB_FN void sm64_er_set_ladder(float rate)
{
    er_ladder_rate = rate;
}

SM64_LIB_FN void sm64_er_set_head(int active, float pitch, float yaw)
{
    er_head_active = active;
    er_head_pitch = (s16)(pitch * 32768.0f / 3.14159265f);
    er_head_yaw = (s16)(yaw * 32768.0f / 3.14159265f);
}

// the enemy the punch combo turns Mario to (mario_actions_object.c)
extern int er_aim_active;
extern f32 er_aim_x, er_aim_z;

SM64_LIB_FN void sm64_er_set_aim(int active, float x, float z)
{
    er_aim_active = active;
    er_aim_x = x;
    er_aim_z = z;
}

// how far that enemy is beyond a blow's reach (units): the blow's lunge (mario_actions_object.c)
extern f32 er_aim_gap, er_lunge;

SM64_LIB_FN void sm64_er_set_aim_gap(float gap)
{
    er_aim_gap = gap;
    // (a lunge lasts a few ticks, fading)
    er_lunge *= 0.55f;
    if (er_lunge < 1.0f) {
        er_lunge = 0.0f;
    }
}

extern int er_home_active;
extern f32 er_home_x, er_home_z;

SM64_LIB_FN void sm64_er_set_home(int active, float x, float z)
{
    er_home_active = active;
    er_home_x = x;
    er_home_z = z;
}

/** er-mario: the special cap ends (next tick, with SM64's own music change), or right away
 * (another one goes on next) */
SM64_LIB_FN void sm64_er_end_cap(int32_t marioId, int now)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );
    if (gMarioState->capTimer == 0)
        return;
    if (now) {
        stop_cap_music();
        gMarioState->flags &= ~MARIO_SPECIAL_CAPS;
        gMarioState->flags |= MARIO_NORMAL_CAP | MARIO_CAP_ON_HEAD;
        gMarioState->capTimer = 0;
    } else {
        gMarioState->capTimer = 1;
    }
}

extern f32 er_level_music_volume;

/** er-mario: the cap music's volume (0-1) */
SM64_LIB_FN void sm64_er_set_music_volume(float volume)
{
    er_level_music_volume = volume;
}

/** er-mario: sets Mario's cap flags as they are, no pickup (the model export's wing cap) */
SM64_LIB_FN void sm64_er_set_cap_flags(int32_t marioId, uint32_t flags)
{
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );
    gMarioState->flags = (gMarioState->flags & ~MARIO_CAPS) | flags;
}

extern f32 er_stomp_vel, er_stomp_vel_a;

SM64_LIB_FN void sm64_er_set_stomp(float vel, float velA)
{
    er_stomp_vel = vel;
    er_stomp_vel_a = velA;
}

/** er-mario: Y is held (the hammer's slam charges while it is) */
SM64_LIB_FN void sm64_er_set_hold_y(int held)
{
    er_hold_y = held;
}

/** er-mario: an enemy is up in the air with Mario (juggle.rs) */
SM64_LIB_FN void sm64_er_set_juggle(int on)
{
    er_juggle = on;
}

/** er-mario: the hammer is out (L1): B's punches are its swings */
SM64_LIB_FN void sm64_er_set_hammer(int on)
{
    er_hammer_mode = on;
}

/** er-mario: the hook (hook.rs) pulls Mario to (x, y, z) (SM64 units): `kind` 0 an enemy, 1 the
 * map. `start`: the pull starts now (else only its end moves, an enemy on the move). */
SM64_LIB_FN void sm64_er_zip(int32_t marioId, float x, float y, float z, int kind, int start)
{
    extern f32 er_zip_target[3];
    extern s32 er_zip_kind;
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    global_state_bind( ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState );
    er_zip_target[0] = x;
    er_zip_target[1] = y;
    er_zip_target[2] = z;
    er_zip_kind = kind;
    // (not dead, holding something, hurt, in a cutscene)
    if (start && gMarioState->health >= 0x100 && gMarioState->heldObj == NULL
        && !(gMarioState->action & (ACT_FLAG_INVULNERABLE | ACT_FLAG_INTANGIBLE))
        && (gMarioState->action & ACT_GROUP_MASK) != ACT_GROUP_CUTSCENE)
        set_mario_action(gMarioState, ACT_ER_ZIP, (u32) kind);
}

/** er-mario: where the Mikiri stomp (timing.rs) lands, the attacker's head (SM64 units); `start`:
 * the counter starts now (else only its target moves, the enemy on the move). */
SM64_LIB_FN void sm64_er_mikiri(int32_t marioId, float x, float y, float z, int start)
{
    extern f32 er_mikiri_target[3];
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    global_state_bind( ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState );
    er_mikiri_target[0] = x;
    er_mikiri_target[1] = y;
    er_mikiri_target[2] = z;
    if (start && gMarioState->health >= 0x100 && gMarioState->heldObj == NULL
        && !(gMarioState->action & (ACT_FLAG_INVULNERABLE | ACT_FLAG_INTANGIBLE))
        && (gMarioState->action & ACT_GROUP_MASK) != ACT_GROUP_CUTSCENE)
        set_mario_action(gMarioState, ACT_ER_MIKIRI, 0);
}

/** er-mario: the pole swing (hook.rs): round (x, y, z) (SM64 units), `mode` 1 starts it, 0 moves
 * its centre (the enemy on the move), 2 lets go */
SM64_LIB_FN void sm64_er_pole(int32_t marioId, float x, float y, float z, float radius, int mode)
{
    extern f32 er_pole_centre[3];
    extern f32 er_pole_radius;
    extern s32 er_pole_release;
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    global_state_bind( ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState );
    er_pole_centre[0] = x;
    er_pole_centre[1] = y;
    er_pole_centre[2] = z;
    if (radius > 0.0f) {
        er_pole_radius = radius;
    }
    if (mode == 2) {
        er_pole_release = 1;
    } else if (mode == 1) {
        er_pole_release = 0;
        if (gMarioState->health >= 0x100 && gMarioState->heldObj == NULL
            && !(gMarioState->action & (ACT_FLAG_INVULNERABLE | ACT_FLAG_INTANGIBLE))
            && (gMarioState->action & ACT_GROUP_MASK) != ACT_GROUP_CUTSCENE)
            set_mario_action(gMarioState, ACT_ER_POLE_SWING, 0);
    }
}

/** er-mario: Star Rush (starrush.rs): SM64's power-up music on or off */
SM64_LIB_FN void sm64_er_star_music(int on)
{
    if (on)
        play_cap_music(SEQUENCE_ARGS(4, SEQ_EVENT_POWERUP));
    else
        stop_cap_music();
}

/** er-mario: the Rock Mushroom (rock.rs): Mario rolls as a boulder while `on` */
SM64_LIB_FN void sm64_er_rock(int32_t marioId, int on)
{
    extern s32 er_rock_on;
    extern void er_rock_peak_from(f32 y);
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    global_state_bind( ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState );
    if (!on) {
        er_rock_on = 0;
    }
    if (on && gMarioState->action != ACT_ER_ROCK && gMarioState->health >= 0x100 && gMarioState->heldObj == NULL
        && !(gMarioState->action & (ACT_FLAG_INVULNERABLE | ACT_FLAG_INTANGIBLE))
        && (gMarioState->action & ACT_GROUP_MASK) != ACT_GROUP_CUTSCENE) {
        s32 air = (gMarioState->action & ACT_FLAG_AIR) != 0;
        er_rock_on = 1;
        set_mario_action(gMarioState, ACT_ER_ROCK, 0);
        gMarioState->actionState = air ? 1 : 0;
        er_rock_peak_from(gMarioState->peakHeight > gMarioState->pos[1] ? gMarioState->peakHeight : gMarioState->pos[1]);
    }
}

/** er-mario: Mario throws a fireball from Yoshi's saddle (ride.rs): his palm thrust */
SM64_LIB_FN void sm64_er_ride_throw(void)
{
    extern s32 er_ride_throw;
    er_ride_throw = 8;
}

/** er-mario: Mega Mario (mega.rs): slow for his size, the power-up music while he lasts */
SM64_LIB_FN void sm64_er_set_mega(int on)
{
    extern int er_mega;
    if (on && !er_mega)
        play_cap_music(SEQUENCE_ARGS(4, SEQ_EVENT_POWERUP));
    if (!on && er_mega)
        stop_cap_music();
    er_mega = on;
}

/** er-mario: the weapon in hand (0 fists, 1 hammer, 2 boomerang, 3 Chain Chomp) and the boomerangs
 * ready to throw */
SM64_LIB_FN void sm64_er_set_weapon(int weapon, int rangs_ready)
{
    er_weapon = weapon;
    er_hammer_mode = weapon == 1;
    er_rang_ready = rangs_ready;
}

/** er-mario: what the combos' moves did on the last tick (sm64.rs SM64ErMove) */
SM64_LIB_FN void sm64_er_get_move(int32_t marioId, struct SM64ErMove *out)
{
    out->hit = er_move_hit;
    out->fx = er_move_fx;
    out->hammerScripted = er_hammer_scripted;
    out->hammerYaw = er_hammer_yaw;
    out->hammerPitch = er_hammer_pitch;
    out->hammerScale = er_hammer_scale;
    out->throwKind = er_move_throw;
    out->throwCharge = er_move_charge;
    out->chompScripted = er_chomp_scripted;
    out->chompYaw = er_chomp_yaw;
    out->chompReach = er_chomp_reach;
    out->chompHeight = er_chomp_height;
    out->chompBite = er_chomp_bite;
    out->actionArg = 0;
    out->actionTimer = 0;
    if( marioId >= s_mario_instance_pool.size || s_mario_instance_pool.objects[marioId] == NULL )
        return;
    struct GlobalState *globalState = ((struct MarioInstance *)s_mario_instance_pool.objects[ marioId ])->globalState;
    global_state_bind( globalState );
    out->actionArg = gMarioState->actionArg;
    out->actionTimer = gMarioState->actionTimer;
}
