#pragma once

#include "decomp/include/types.h"
#include "libsm64.h"
#include <stdio.h>

extern SM64PlaySoundFunctionPtr g_play_sound_func;

#define ER_SOUND_LOG 32
extern f32 *g_er_body_pos;
extern uint32_t g_er_sound_log[ER_SOUND_LOG];
extern int g_er_sound_count;

extern void play_sound( uint32_t soundBits, f32 *pos );