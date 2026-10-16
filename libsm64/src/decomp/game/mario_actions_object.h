#ifndef MARIO_ACTIONS_OBJECT_H
#define MARIO_ACTIONS_OBJECT_H

#include "../include/PR/ultratypes.h"

#include "../include/types.h"

s32 mario_update_punch_sequence(struct MarioState *m);
f32 er_punch_step(struct MarioState *m);
void er_aim_turn(struct MarioState *m);
s32 mario_execute_object_action(struct MarioState *m);

#endif // MARIO_ACTIONS_OBJECT_H
