#include "play_sound.h"

#include "decomp/audio/external.h"
#include "debug_print.h"
#include "load_audio_data.h"

SM64PlaySoundFunctionPtr g_play_sound_func = NULL;

// ER Mario: a Mario that ticks without the audio running (the one in the menus) must not queue
// sounds; they'd all start once it runs, the snoring for good, its source being gone by then.
int g_er_mute = 0;

// ER Mario (co-op): the sounds of the ticking Mario's own body (his voice, steps, blows: played
// from his object's position, which sm64_mario_tick points g_er_body_pos at), logged for the
// partner's game to play too (sm64_er_take_sounds)
f32 *g_er_body_pos = NULL;
uint32_t g_er_sound_log[ER_SOUND_LOG];
int g_er_sound_count = 0;

uint32_t g_er_world_log[ER_WORLD_LOG];
float g_er_world_volume[ER_WORLD_LOG];
int g_er_world_count = 0;
int g_er_local_only = 0;

void er_log_world_sound( uint32_t soundBits, float volume ) {
    // (the menu bank, 7: buzzes and chimes are the player's own cues)
    if ( g_er_mute || g_er_local_only || ((soundBits >> 28) & 0xF) == 7 || g_er_world_count >= ER_WORLD_LOG ) return;
    g_er_world_log[g_er_world_count] = soundBits;
    g_er_world_volume[g_er_world_count] = volume;
    g_er_world_count++;
}

int g_er_luigi = 0;
f32 g_er_luigi_spots[ER_LUIGI_SPOTS][3];

int er_is_luigi_voice( const f32 *pos ) {
    if ( pos == NULL ) return 0;
    if ( g_er_luigi && pos == g_er_body_pos ) return 1;
    uintptr_t p = (uintptr_t)pos, first = (uintptr_t)g_er_luigi_spots;
    return p >= first && p < first + sizeof(g_er_luigi_spots);
}

extern void play_sound( uint32_t soundBits, f32 *pos ) {
    if ( g_er_mute ) return;
    if ( pos != NULL && pos == g_er_body_pos && g_er_sound_count < ER_SOUND_LOG ) {
        g_er_sound_log[g_er_sound_count++] = soundBits;
    }
    if ( g_is_audio_initialized ) {
        DEBUG_PRINT("$ play_sound(%d) request %d; pos %f %f %f\n", soundBits,sSoundRequestCount,pos[0],pos[1],pos[2]);
        sSoundRequests[sSoundRequestCount].soundBits = soundBits;
        sSoundRequests[sSoundRequestCount].position = pos;
        sSoundRequestCount++;
    }

    if ( g_play_sound_func ) {
        g_play_sound_func(soundBits, pos);
    }
}