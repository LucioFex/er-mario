//! Mario lights the way in dark places, like the Lantern: the Lantern's own SpEffect on the
//! Tarnished (its engine light follows the body), and a little Fire Flower flame by his left hand.
//! Only in the dark kinds of map (caves, catacombs, tunnels, the underground; ini `light_areas`
//! names others, `light = on/off` forces it), and never twice: a real Lantern in use is left be.

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

/// The map areas (the block id's top byte) that are dark: the underground rivers (12), catacombs
/// and hero's graves (30), caves (31), tunnels and mines (32), the shunning-grounds (35), and the
/// Shadow of the Erdtree's catacombs, gaols, caves and the rest (40-43). To check by the log.
const DARK: [u8; 9] = [12, 30, 31, 32, 35, 40, 41, 42, 43];
/// the Lantern (goods row): its SpEffect is the light
const LANTERN: u32 = 2070;

/// The ini's say: `light = on/off` (None: by the area), `light_areas = 31, 32` (None: DARK).
#[derive(Clone, Debug, Default, PartialEq)]
struct Overrides {
    force: Option<bool>,
    areas: Option<Vec<u8>>,
}

fn dark(area: u8, o: &Overrides) -> bool {
    o.force.unwrap_or_else(|| o.areas.as_deref().unwrap_or(&DARK).contains(&area))
}

fn parse_areas(s: &str) -> Vec<u8> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn overrides() -> Overrides {
    let force = crate::paths::config("light").and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" => Some(true),
        "off" | "0" | "false" => Some(false),
        _ => None,
    });
    let areas = crate::paths::config("light_areas").map(|v| parse_areas(&v));
    Overrides { force, areas }
}

struct State {
    /// the map area last seen, and the Lantern's SpEffect (once the params are there)
    area: Option<u8>,
    effect: Option<i32>,
    /// the light on the Tarnished is ours (not a real Lantern's)
    ours: bool,
    told_real: bool,
    /// times the game dropped ours and it was lit again (logged the first few)
    relit: u32,
    checked: Option<Instant>,
    /// the hand flame: how lit (0-1, eased), where his left hand is (Elden Ring world), when it
    /// last moved on
    glow: f32,
    want: bool,
    hand: Option<Vec3>,
    last: Option<Instant>,
}

static STATE: Mutex<State> =
    Mutex::new(State { area: None, effect: None, ours: false, told_real: false, relit: 0, checked: None, glow: 0.0, want: false, hand: None, last: None });

/// the Lantern's SpEffect, from its goods row
fn lantern_effect() -> Option<i32> {
    use eldenring::cs::{EquipParamGoods, SoloParamRepository};
    use fromsoftware_shared::FromStatic;
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let g = repo.get::<EquipParamGoods>(LANTERN)?;
    // (what kind of thing the reference is: to check by the log that it's a SpEffect)
    crate::log(format!("light: the Lantern's goods row refers to {} (category {})", g.ref_id_default(), g.ref_category()));
    Some(g.ref_id_default())
}

/// Every frame in the world: the light on or off for the Tarnished `chr` (Mario on or not).
pub fn tick(chr: &mut eldenring::cs::ChrIns, mario: bool) {
    use eldenring::cs::ChrInsExt;
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let block = eldenring::cs::BlockId::from(i32::from(chr.block_id));
    let area = block.area();
    let o = overrides();
    let want = mario && dark(area, &o);
    if s.area != Some(area) {
        s.area = Some(area);
        crate::log(format!("light: area m{area} (block {block}), dark {}", if dark(area, &o) { "yes" } else { "no" }));
    }
    s.want = want;
    // (once a second, or at once when it changes: a death or a load drops every SpEffect)
    let due = s.checked.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) || (want != s.ours && !s.told_real);
    if !due {
        return;
    }
    s.checked = Some(Instant::now());
    if s.effect.is_none() {
        s.effect = lantern_effect();
        match s.effect {
            Some(id) => crate::log(format!("light: the Lantern's SpEffect is {id}")),
            None => return,
        }
    }
    let Some(id) = s.effect.filter(|&id| id > 0) else { return };
    let lit = chr.special_effect.entries().any(|e| e.param_id == id);
    if want {
        if !lit {
            if s.ours {
                s.relit += 1;
                if s.relit <= 3 || s.relit % 60 == 0 {
                    crate::log(format!("light: lit again (the game dropped it, {} times)", s.relit));
                }
            }
            chr.apply_speffect(id, true);
            if !s.ours {
                crate::log(format!("light: on in m{area} (SpEffect {id})"));
            }
            s.ours = true;
            s.told_real = false;
        } else if !s.ours && !s.told_real {
            s.told_real = true;
            crate::log("light: a real Lantern is lit, Mario's isn't needed");
        }
    } else if s.ours {
        if lit {
            chr.remove_speffect(id);
        }
        s.ours = false;
        crate::log(format!("light: off (m{area})"));
    } else {
        s.told_real = false;
    }
}

/// Where his left hand is now (Elden Ring world position), for the flame.
pub fn hand(pos: Option<Vec3>) {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).hand = pos;
}

/// The hand flame's motes (fx.rs, element.rs's look: a fire mote is an ember): a few small ones
/// flickering round his left hand, fading in and out over a second.
pub fn motes() -> Vec<crate::fx::Mote> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    let dt = s.last.replace(now).map_or(0.0, |t| now.duration_since(t).as_secs_f32().min(0.1));
    let to = if s.want && s.hand.is_some() { 1.0 } else { 0.0 };
    s.glow += (to - s.glow).clamp(-dt, dt);
    let Some(hand) = s.hand.filter(|_| s.glow > 0.01) else { return Vec::new() };
    let clock = crate::flame::clock();
    (0..3)
        .map(|k| {
            let t = (clock * 1.6 + k as f32 / 3.0).fract();
            let a = clock * 2.3 + k as f32 * 2.1;
            crate::fx::Mote { pos: hand + Vec3::new(a.cos() * 0.03, 0.06 + t * 0.12, a.sin() * 0.03), element: 0, t, size: (0.07 - k as f32 * 0.012) * s.glow }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_kinds_of_map_by_default() {
        let o = Overrides::default();
        assert!(dark(31, &o));
        assert!(dark(30, &o));
        assert!(dark(12, &o));
        assert!(!dark(60, &o));
        assert!(!dark(11, &o));
    }

    #[test]
    fn the_ini_can_name_the_areas_or_force_it() {
        assert_eq!(parse_areas("10, 31 ,x, 300, 32"), vec![10, 31, 32]);
        let named = Overrides { force: None, areas: Some(vec![10, 31]) };
        assert!(dark(10, &named));
        assert!(!dark(30, &named));
        let off = Overrides { force: Some(false), areas: None };
        assert!(!dark(31, &off));
        let on = Overrides { force: Some(true), areas: None };
        assert!(dark(60, &on));
    }
}
