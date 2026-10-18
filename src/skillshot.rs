//! Skillshots (Bulletstorm's): a creative kill gets a name, a callout over it and style points.
//! What took the enemy out, and how it was then (up in the air, slipped), says which; two or three
//! close together are a DOUBLE / TRIPLE / MULTI KILL instead. The trap kit's and the hook's own
//! (SLIP!, CAROM!, STRIKE!, HOME RUN!, TETHER!, BOING!) are called where they happen
//! (traps.rs, bat.rs, rock.rs); these are the kills.

use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;

use crate::combat::Attack;
use crate::log;

/// kills this close together (s) are one multi kill
const MULTI_WITHIN: f32 = 1.2;

struct State {
    kills: Vec<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { kills: Vec::new() });

/// How an enemy was when its last blow came: up in the air (juggled, batted, thrown), down after
/// a slip.
#[derive(Clone, Copy, Default)]
pub struct How {
    pub airborne: bool,
}

/// The name, colour and points of a kill by `attack` (None: an ordinary one).
fn name(attack: Attack, how: How) -> Option<(&'static str, [f32; 3], f32)> {
    let gold = [1.0, 0.85, 0.3];
    Some(match attack {
        Attack::MaxFlurry | Attack::MaxUpper | Attack::MaxQuake | Attack::MaxCyclone => ("MAXED OUT!", [1.0, 0.7, 0.2], 5.0),
        Attack::Bowled => ("FRIENDLY FIRE!", [1.0, 0.55, 0.3], 5.0),
        Attack::HomeRun => ("OUT OF THE PARK!", gold, 5.0),
        Attack::Tether => ("HEAD TO HEAD!", [0.6, 0.85, 1.0], 4.0),
        Attack::Rock | Attack::RockSlam => ("ROCK BOTTOM!", [0.8, 0.75, 0.65], 4.0),
        Attack::Bomb => ("KABOOM!", [1.0, 0.5, 0.2], 4.0),
        Attack::Slip => ("BANANA SPLIT!", [1.0, 0.9, 0.25], 4.0),
        Attack::ShellHit => ("SHELL SHOCK!", [0.4, 1.0, 0.45], 4.0),
        Attack::Mikiri => ("PARRY THIS!", [0.7, 0.9, 1.0], 4.0),
        Attack::Fire(4..) => ("WELL DONE!", [1.0, 0.45, 0.15], 3.0),
        _ if how.airborne => ("AIR RAID!", [0.55, 0.85, 1.0], 3.0),
        Attack::Stomp | Attack::GroundPound | Attack::YoshiStomp | Attack::MeteorLand => ("STOMPED!", gold, 2.0),
        _ => return None,
    })
}

/// Game thread (combat.rs): an enemy Mario hit last with `attack` died at `at` (its feet).
pub fn kill(attack: Attack, how: How, at: Vec3) {
    let n = {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.kills.retain(|t| t.elapsed().as_secs_f32() < MULTI_WITHIN);
        s.kills.push(Instant::now());
        s.kills.len()
    };
    let over = at + Vec3::Y * 2.3;
    let (text, tint, points) = match n {
        2 => ("DOUBLE!", [1.0, 0.8, 0.3], 4.0),
        3 => ("TRIPLE!", [1.0, 0.6, 0.25], 6.0),
        4.. => ("MULTI KILL!", [1.0, 0.4, 0.3], 8.0),
        _ => match name(attack, how) {
            Some(n) => n,
            None => return,
        },
    };
    log(format!("skillshot: {text} ({attack:?})"));
    crate::style::callout(text, Some(over), tint, n >= 2 || points >= 4.0, points);
}

pub fn clear() {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).kills.clear();
}
