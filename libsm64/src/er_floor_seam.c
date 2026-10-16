#include "er_floor_seam.h"
#include "decomp/include/sm64.h"
#include "decomp/game/mario.h"
#include "decomp/engine/surface_collision.h"
#include <math.h>

int er_floor_seam_has_flat_support(struct MarioState *m) {
    if (!m->floor || m->curTerrain == TERRAIN_SLIDE
        || mario_get_floor_class(m) != SURFACE_CLASS_DEFAULT
        || m->floor->normal.y > 0.7880108f
        || (m->action & ACT_FLAG_AIR)
        || fabsf(m->pos[1] - m->floorHeight) > 5) return 0;
    float nx = m->floor->normal.x, nz = m->floor->normal.z;
    float length = sqrtf(nx * nx + nz * nz);
    if (!isfinite(length) || length < 0.001f) return 0;
    /* Probe across the steep triangle, not along its seam. find_floor allows
       floors up to 78 units above the query, so validate each returned height
       explicitly: a bridge/step above or a floor below cannot count as support. */
    for (int sign = -1; sign <= 1; sign += 2) {
        struct SM64SurfaceCollisionData *floor = NULL;
        float h = find_floor(m->pos[0] + sign * 20 * nx / length,
                             m->floorHeight, m->pos[2] + sign * 20 * nz / length, &floor);
        if (!floor || floor->transform != m->floor->transform
            || !isfinite(h) || fabsf(h - m->floorHeight) > 8
            || floor->normal.y < 0.9659258f) return 0;
        struct MarioState sample = *m;
        sample.floor = floor;
        sample.curTerrain = floor->terrain;
        if (mario_get_floor_class(&sample) != SURFACE_CLASS_DEFAULT) return 0;
    }
    return 1;
}

/* Elden Ring's ground is full of small steep triangles, and stepping on one set off a slide in
   the middle of a walk or a fight. On default floors a slide starts a little steeper than
   SM64's 38° and only after a few ticks on it; once Mario slides, SM64's own rule. */
#define ER_SLIDE_START_NORMAL_Y 0.7660444f /* ~cos(40 deg) */
#define ER_SLIDE_START_TICKS 3
static int sSteepTicks;

int er_slide_start(struct MarioState *m, int slippery) {
    if (!slippery) {
        sSteepTicks = 0;
        return 0;
    }
    if ((m->action & ACT_FLAG_BUTT_OR_STOMACH_SLIDE) || m->action == ACT_BEGIN_SLIDING
        || m->action == ACT_HOLD_BEGIN_SLIDING || m->curTerrain == TERRAIN_SLIDE
        || mario_get_floor_class(m) != SURFACE_CLASS_DEFAULT) {
        return 1;
    }
    if (m->floor->normal.y > ER_SLIDE_START_NORMAL_Y) {
        sSteepTicks = 0;
        return 0;
    }
    if (sSteepTicks < ER_SLIDE_START_TICKS) {
        sSteepTicks++;
    }
    return sSteepTicks >= ER_SLIDE_START_TICKS;
}
