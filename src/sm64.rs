//! Minimal FFI to libsm64 (compiled into this DLL by build.rs).
#![allow(dead_code)]

pub const TEXTURE_W: usize = 64 * 11;
pub const TEXTURE_H: usize = 64;
pub const GEO_MAX_TRIANGLES: usize = 1024;

const SURFACE_DEFAULT: i16 = 0x0000;
const TERRAIN_GRASS: u16 = 0x0000;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SM64Surface {
    pub kind: i16,
    pub force: i16,
    pub terrain: u16,
    pub vertices: [[i32; 3]; 3],
}

impl SM64Surface {
    pub fn grass(vertices: [[i32; 3]; 3]) -> Self {
        Self { kind: SURFACE_DEFAULT, force: 0, terrain: TERRAIN_GRASS, vertices }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SM64ObjectTransform {
    pub position: [f32; 3],
    /// degrees (pitch, yaw, roll)
    pub euler_rotation: [f32; 3],
}

#[repr(C)]
pub struct SM64SurfaceObject {
    pub transform: SM64ObjectTransform,
    pub surface_count: u32,
    pub surfaces: *const SM64Surface,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct SM64MarioInputs {
    pub cam_look_x: f32,
    pub cam_look_z: f32,
    pub stick_x: f32,
    pub stick_y: f32,
    pub button_a: u8,
    pub button_b: u8,
    pub button_z: u8,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct SM64MarioState {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub face_angle: f32,
    pub forward_velocity: f32,
    pub health: i16,
    pub action: u32,
    pub anim_id: i32,
    pub anim_frame: i16,
    pub flags: u32,
    pub particle_flags: u32,
    pub invinc_timer: i16,
}

#[repr(C)]
pub struct SM64MarioGeometryBuffers {
    pub position: *mut f32,
    pub normal: *mut f32,
    pub color: *mut f32,
    pub uv: *mut f32,
    pub num_triangles_used: u16,
}

/// Owned vertex buffers for Mario's skinned mesh.
pub struct Geometry {
    pub position: Vec<f32>,
    pub normal: Vec<f32>,
    pub color: Vec<f32>,
    pub uv: Vec<f32>,
    used: u16,
}

impl Geometry {
    pub fn new() -> Self {
        let n = GEO_MAX_TRIANGLES * 3;
        Self { position: vec![0.0; n * 3], normal: vec![0.0; n * 3], color: vec![0.0; n * 3], uv: vec![0.0; n * 2], used: 0 }
    }

    pub fn buffers(&mut self) -> GeometryGuard<'_> {
        GeometryGuard {
            raw: SM64MarioGeometryBuffers {
                position: self.position.as_mut_ptr(),
                normal: self.normal.as_mut_ptr(),
                color: self.color.as_mut_ptr(),
                uv: self.uv.as_mut_ptr(),
                num_triangles_used: 0,
            },
            used: &mut self.used,
        }
    }

    pub fn used(&self) -> usize {
        self.used as usize
    }
}

/// Writes the triangle count back into `Geometry` after a tick.
pub struct GeometryGuard<'a> {
    raw: SM64MarioGeometryBuffers,
    used: &'a mut u16,
}

impl std::ops::Deref for GeometryGuard<'_> {
    type Target = SM64MarioGeometryBuffers;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl std::ops::DerefMut for GeometryGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

impl Drop for GeometryGuard<'_> {
    fn drop(&mut self) {
        *self.used = self.raw.num_triangles_used;
    }
}

unsafe extern "C" {
    pub fn sm64_global_init(rom: *const u8, out_texture: *mut u8);
    pub fn sm64_static_surfaces_load(surfaces: *const SM64Surface, count: u32);
    pub fn sm64_mario_create(x: f32, y: f32, z: f32) -> i32;
    pub fn sm64_mario_tick(
        id: i32,
        inputs: *const SM64MarioInputs,
        out_state: *mut SM64MarioState,
        out_buffers: *mut SM64MarioGeometryBuffers,
    );
    pub fn sm64_mario_delete(id: i32);
    pub fn sm64_audio_init(rom: *const u8);
    pub fn sm64_audio_tick(queued: u32, desired: u32, buffer: *mut i16) -> u32;
    pub fn sm64_set_mario_position(id: i32, x: f32, y: f32, z: f32);
    pub fn sm64_set_mario_faceangle(id: i32, y: f32);
    /// SM64's interaction check against an object at (x, y, z): true if Mario's current move hits it
    /// (also bounces him off / plays the hit sound like SM64).
    pub fn sm64_mario_attack(id: i32, x: f32, y: f32, z: f32, hitbox_height: f32) -> bool;
    pub fn sm64_set_mario_health(id: i32, health: u16);
    pub fn sm64_mario_kill(id: i32);
    /// heals 1/4 wedge per count over the next ticks (SM64 coins give 4)
    pub fn sm64_mario_heal(id: i32, heal_counter: u8);
    pub fn sm64_play_sound_global(sound_bits: i32);
    /// a sound at a share of its volume (0-1)
    pub fn sm64_play_sound_volume(sound_bits: i32, volume: f32);
    pub fn sm64_set_mario_action(id: i32, action: u32);
    pub fn sm64_set_mario_action_arg(id: i32, action: u32, action_arg: u32);
    pub fn sm64_set_mario_velocity(id: i32, x: f32, y: f32, z: f32);
    pub fn sm64_set_mario_forward_velocity(id: i32, vel: f32);
    pub fn sm64_surface_object_create(object: *const SM64SurfaceObject) -> u32;
    pub fn sm64_surface_object_move(id: u32, transform: *const SM64ObjectTransform);
    pub fn sm64_surface_object_delete(id: u32);
    pub fn sm64_surface_find_floor_height(x: f32, y: f32, z: f32) -> f32;
    pub fn sm64_surface_find_ceil(x: f32, y: f32, z: f32, ceil: *mut *mut std::ffi::c_void) -> f32;
    pub fn sm64_surface_find_wall_collision(x: *mut f32, y: *mut f32, z: *mut f32, offset_y: f32, radius: f32) -> i32;
    pub fn sm64_mario_take_damage(id: i32, damage: u32, subtype: u32, x: f32, y: f32, z: f32);
    /// er-mario patch: the last tick's per-part matrices (4x4, row-vector, SM64 units); returns the part count.
    /// er-mario patch: SM64's C-up head look (radians, relative to his body; active 0 = off)
    pub fn sm64_er_set_head(active: i32, pitch: f32, yaw: f32);
    /// er-mario patch: the enemy (SM64 x, z) each hit of the punch combo turns Mario to (0 = none)
    pub fn sm64_er_set_aim(active: i32, x: f32, z: f32);
    /// er-mario patch: how far that enemy is beyond a blow's reach (units): an aimed blow lunges
    pub fn sm64_er_set_aim_gap(gap: f32);
    pub fn sm64_er_set_home(active: i32, x: f32, z: f32);
    pub fn sm64_er_set_stomp(vel: f32, vel_a: f32);
    pub fn sm64_er_end_cap(id: i32, now: i32);
    pub fn sm64_er_set_music_volume(volume: f32);
    /// a blow landed: Mario (and his animation) hold still for this many ticks
    pub fn sm64_er_hitstop(frames: i32);
    pub fn sm64_er_set_cap_flags(id: i32, flags: u32);
    pub fn sm64_mario_interact_cap(id: i32, cap_flag: u32, cap_time: u16, play_music: u8);
    pub fn sm64_er_set_ladder(rate: f32);
    pub fn sm64_er_lava(id: i32);
    pub fn sm64_er_status(id: i32, kind: i32, ticks: i32) -> i32;
    pub fn sm64_er_mute(mute: i32);
    pub fn sm64_er_pick_up(id: i32);
    pub fn sm64_er_held(id: i32, pos: *mut f32) -> i32;
    pub fn sm64_er_drop(id: i32);
    pub fn sm64_er_get_parts(matrices: *mut f32, tri_part: *mut i32, local_pos: *mut f32, local_normal: *mut f32) -> i32;
    /// er-mario patch: the hammer is out (L1), B's punches are its swings
    pub fn sm64_er_set_hammer(on: i32);
    /// er-mario patch: an enemy is up in the air with Mario (juggle.rs): his air blows keep him up
    pub fn sm64_er_set_juggle(on: i32);
    /// Y held (the hammer's slam charges while it is)
    pub fn sm64_er_set_hold_y(held: i32);
    /// the weapon in hand (weapons.rs) and the boomerangs ready to throw
    pub fn sm64_er_set_weapon(weapon: i32, rangs_ready: i32);
    /// Mega Mario: slow for his size, the power-up music
    pub fn sm64_er_set_mega(on: i32);
    /// Mario's fireball thrust from Yoshi's saddle (ride.rs)
    pub fn sm64_er_ride_throw();
    /// the hook (hook.rs): pulls Mario to (x, y, z) (SM64 units), an enemy (0) or the map (1);
    /// `start` 0 only moves the pull's end
    pub fn sm64_er_zip(id: i32, x: f32, y: f32, z: f32, kind: i32, start: i32);
    /// er-mario patch: what the combos' moves did on the last tick
    pub fn sm64_er_get_move(id: i32, out: *mut SM64ErMove);
}

/// What the combos' moves (libsm64's er_moves.c) did on a tick: what they hit (moves::Hit), what
/// to show (moves::FX_*), where the hammer points (degrees around Mario's facing, positive to his
/// left, and up from level) and how big it is.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SM64ErMove {
    pub hit: i32,
    pub fx: u32,
    pub hammer_scripted: i32,
    pub hammer_yaw: f32,
    pub hammer_pitch: f32,
    pub hammer_scale: f32,
    pub action_arg: u32,
    pub action_timer: i32,
    /// a throw let go this tick (its kind + 1: moves::THROW_*), and how long it was charged (ticks)
    pub throw_kind: i32,
    pub throw_charge: i32,
    /// the Chain Chomp's place (chomp.rs): degrees around Mario's facing, SM64 units out and up,
    /// its mouth open (0-1)
    pub chomp_scripted: i32,
    pub chomp_yaw: f32,
    pub chomp_reach: f32,
    pub chomp_height: f32,
    pub chomp_bite: f32,
}
