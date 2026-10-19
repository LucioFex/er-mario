#pragma once

#include "decomp/include/types.h"
#include "libsm64.h"
#include <stdio.h>

extern SM64PlaySoundFunctionPtr g_play_sound_func;

#define ER_SOUND_LOG 32
extern f32 *g_er_body_pos;
extern uint32_t g_er_sound_log[ER_SOUND_LOG];
extern int g_er_sound_count;

// er-mario (co-op): the mod's own sounds (its weapons, blows, power-ups: played through
// sm64_play_sound_global / er_play_sound_volume, not by Mario's body), with their volume, for the
// partner's game to play too; the menu bank's (buzzes, chimes) and sm64_play_sound_local's stay ours
#define ER_WORLD_LOG 32
extern uint32_t g_er_world_log[ER_WORLD_LOG];
extern float g_er_world_volume[ER_WORLD_LOG];
extern int g_er_world_count;
extern int g_er_local_only;
void er_log_world_sound( uint32_t soundBits, float volume );

// er-mario: the player is Luigi (sm64_er_set_luigi): his voice a little higher
extern int g_er_luigi;
// spots Luigi's voice is played from (our own and each co-op partner's that is Luigi): the audio
// tells his from Mario's by them
#define ER_LUIGI_OWN 8
#define ER_LUIGI_GLOBAL 8
#define ER_LUIGI_PARTNER 9
#define ER_LUIGI_SPOTS 13
extern f32 g_er_luigi_spots[ER_LUIGI_SPOTS][3];
int er_is_luigi_voice( const f32 *pos );

extern void play_sound( uint32_t soundBits, f32 *pos );