//! The ground's sound under Mario's feet: SM64 took every Elden Ring floor for the same ground
//! (one terrain for every triangle), so his steps, landings and jumps sounded alike on stone,
//! wood, sand and in water. Now by the floor's material (ChrPhysicsMaterialInfo::hit_material, a
//! HitMtrlParam row; names by Paramdex: 2 Rock, 3 Sand, 4 Wood, 21 Water...), the sound only
//! (libsm64 sm64_er_set_terrain_sound): SM64's snow and sand would bury him after a long fall.

use std::sync::atomic::{AtomicI32, Ordering};

// SM64's terrain sounds (audio_defines.h)
const STONE: i32 = 3;
const WATER: i32 = 2;
const WOOD: i32 = 4; // SM64's "spooky": the creaking boards
const SAND: i32 = 7;
/// SM64's own (by its surface): every other floor, dirt and grass among them
const OWN: i32 = -1;

/// The floor material's sound: water and the swamps (they're wet: HitMtrlParam's water depth or
/// water effects), stone (rock, ore, iron grates, bone, the hard floors that ring), sand, wood.
pub fn sound_of(material: i32) -> i32 {
    // (the hundreds are the slide's: 1xx never slides, 4xx / 5xx always; the row is the rest)
    match material.rem_euclid(100) {
        20..=27 | 36..=39 | 46 | 47 | 49 | 60 | 61 => WATER,
        1 | 2 | 6 | 11 | 14 | 15 | 16 | 19 | 29 | 34 | 45 => STONE,
        3 => SAND,
        4 | 8 => WOOD,
        _ => OWN,
    }
}

static LAST: AtomicI32 = AtomicI32::new(i32::MIN);

/// Game thread, every frame: the floor under the Tarnished (where Mario stands).
pub fn tick(material: i32, on_ground: bool) {
    // (in the air the last floor's kept: he lands where he took off, most often)
    if !on_ground {
        return;
    }
    let sound = sound_of(material);
    if LAST.load(Ordering::Relaxed) != sound && crate::worker::call("ground sound", move |_| unsafe { crate::sm64::sm64_er_set_terrain_sound(sound) }).is_some() {
        // (only once it's set: a call the worker didn't answer is tried again)
        LAST.store(sound, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_floor_sounds_like_itself() {
        assert_eq!(sound_of(2), STONE);
        assert_eq!(sound_of(3), SAND);
        assert_eq!(sound_of(4), WOOD);
        assert_eq!(sound_of(21), WATER);
        // the swamps splash too, lava and dirt keep SM64's own
        assert_eq!(sound_of(49), WATER);
        assert_eq!(sound_of(7), OWN);
        assert_eq!(sound_of(5), OWN);
        assert_eq!(sound_of(102), STONE);
    }
}
