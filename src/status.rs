//! Elden Ring's ailments and burns on Mario. They build up as they do on the Tarnished (his resist
//! gauges, which lib.rs used to keep full), and the game does their damage (the HP he loses turns
//! into Mario's wedges as always); Mario shows them the SM64 way, and each costs him something of
//! its own while it lasts (v4 Part 11: before, most only showed):
//! - poison: he coughs (SM64's toxic gas), green puffs while it drains him, heavy legs (slower)
//! - scarlet rot: orange puffs while it drains him, coins heal him half as much
//! - blood loss: a burst of red, a wedge and a half at once (no knockback)
//! - frostbite: frozen stiff a moment (no control), then shivering, icy rings (no knockback);
//!   frostbitten a while after, every blow costs him more
//! - sleep: he drops off to sleep (SM64's), buttons wake him sooner
//! - madness: shocked (SM64's electrocution), a yellow flash; his style and a charge in his hand
//!   are lost
//! - a fire on him (a burn the game puts on the Tarnished): SM64's burning run, flames off him
//! The stuns (frozen, asleep, shocked) take even in the middle of a blow on the ground, and in the
//! air when he lands (before, only from a calm moment: in a fight it seldom came and they were
//! lost). Deathblight is the game's own (the user, 2026-10-25: instant death, as in Elden Ring).
//! Poison and rot swamps (floor_kind) aren't lava: he wades in them, bubbles round his feet, and
//! the swamp's own build-up does the rest.
//! An ailment sets in when its gauge runs out: the game fills it back up, so a jump of most of it in
//! one frame is the sign; but a cure or a rest at a grace fills it up too, so it's only taken once
//! the ailment's own damage comes with it (an HP loss for the bursts, an effect that drains for the
//! poisons). Mario's reaction waits for a calm moment (not mid-air, mid-move or knocked back).

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::ChrIns;
use glam::Vec3;

use crate::{log, sm64, worker};

/// the gauges in the order of the resistances (the resist module)
pub const POISON: usize = 0;
pub const ROT: usize = 1;
pub const BLEED: usize = 2;
pub const BLIGHT: usize = 3;
pub const FROST: usize = 4;
pub const SLEEP: usize = 5;
pub const MADNESS: usize = 6;
const NAMES: [&str; 7] = ["poison", "scarlet rot", "blood loss", "deathblight", "frostbite", "sleep", "madness"];

/// frozen stiff (s), asleep at most (s) and how much each button press wakes him (s), the poison's
/// puffs (every s, for s after it sets in), the burning run (SM64 ticks), coughing or shivering at
/// most (s: SM64 loops them until the stick moves)
const FROZEN: f32 = 1.1;
const ASLEEP: f32 = 3.0;
const WAKE_PER_PRESS: f32 = 0.35;
const PUFF_EVERY: f32 = 0.5;
const PUFF_FOR: f32 = 8.0;
const BURN_TICKS: i32 = 45;
const IDLE_ACT_FOR: f32 = 2.5;
/// a gauge jump and its damage this close together count as the ailment setting in (s); a
/// reaction is tried this long for a calm moment (s); a burn: a new effect on the Tarnished that
/// hurts over time, lasting this long at most (s), no ailment's this close to it (s)
const CONFIRM: f32 = 0.6;
const REACT_FOR: f32 = 4.0;
const BURN_MAX_LIFE: f32 = 20.0;
const NOT_AN_AILMENT: f32 = 1.0;
/// lava's burns (SM64's lava boost already answers them)
pub const LAVA_BURNS: [i32; 2] = [4100, 4101];
/// what each costs while it lasts: poisoned, his stick's reach (his speed); rotting, the share of
/// a coin's healing; frostbitten (this long, s), each blow this much dearer; blood loss, this much
/// health at once (a wedge and a half, SM64's units)
const POISON_LEGS: f32 = 0.85;
const ROT_HEAL: f32 = 0.5;
const FROSTBITE_FOR: f32 = 30.0;
const FROSTBITE_FRAIL: f32 = 1.2;
pub const BLEED_COST: i32 = 0x180;
/// bubbles round his feet wading in a swamp (every s); an icy glint off him frostbitten (every s)
const WADE_EVERY: f32 = 0.35;
const GLINT_EVERY: f32 = 0.8;
/// a gap this long between ticks (s: following a door, a grace, a cutscene) and the gauges are read
/// afresh (a rest refilled them meanwhile)
const STALE: f32 = 0.5;

const SOUND_MAMA_MIA: i32 = 0x2420_8081;

/// sm64_er_status kinds
const COUGH: i32 = 1;
const SHIVER: i32 = 2;
const DOZE: i32 = 3;
const WAKE: i32 = 4;
const SHOCK: i32 = 5;
const BURN: i32 = 6;
const STOP_IDLE: i32 = 7;

struct State {
    gauges: Option<[u32; 7]>,
    last_tick: Option<Instant>,
    /// a gauge's jump waiting for its damage, and the ailments that did set in
    jumps: [Option<Instant>; 7],
    procs: [Option<Instant>; 7],
    /// the last HP loss, the last new draining effect
    hp_loss: Option<Instant>,
    new_dot: Option<Instant>,
    /// frozen or asleep until then; asleep, button presses shave it
    held_until: Option<Instant>,
    asleep: bool,
    pressed: bool,
    /// reactions waiting for a calm moment (sm64_er_status kind, since); a cough or shiver going on
    pending: Vec<(i32, Instant)>,
    idle_act: Option<Instant>,
    last_puff: Option<Instant>,
    /// the effects on the Tarnished last tick, and the ones logged once
    effects: Vec<i32>,
    seen: std::collections::HashSet<i32>,
    /// a burn seen (when), waiting a moment to be sure no ailment set in with it
    burn: Option<Instant>,
    burning_since: Option<Instant>,
    /// an effect draining him on him now; poison and rot still on (till the draining's gone)
    draining: bool,
    poisoned: [bool; 2],
    /// the blows' price last set (tough.rs; none yet: a new state, after a death, sets it afresh,
    /// or a frostbite's price would outlive it); madness set in, for lib.rs to take his style;
    /// blood loss's burst not yet paid
    frail: Option<f32>,
    bled_unpaid: bool,
    madness: bool,
    /// wading in a swamp: its kind (POISON or ROT) and the last bubbles
    wading: Option<usize>,
    last_bubble: Option<Instant>,
    last_glint: Option<Instant>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| State {
        gauges: None,
        last_tick: None,
        jumps: [None; 7],
        procs: [None; 7],
        hp_loss: None,
        new_dot: None,
        held_until: None,
        asleep: false,
        pressed: false,
        pending: Vec::new(),
        idle_act: None,
        last_puff: None,
        effects: Vec::new(),
        seen: std::collections::HashSet::new(),
        burn: None,
        burning_since: None,
        draining: false,
        poisoned: [false; 2],
        frail: None,
        bled_unpaid: false,
        madness: false,
        wading: None,
        last_bubble: None,
        last_glint: None,
    }))
}

fn within(t: Option<Instant>, s: f32) -> bool {
    t.is_some_and(|t| t.elapsed().as_secs_f32() < s)
}

/// Blood loss, frostbite or madness set in a moment ago: their wedge costs no knockback (it would
/// knock the reaction away).
pub fn just_set_in() -> bool {
    with(|s| [BLEED, FROST, MADNESS].iter().any(|&i| within(s.procs[i], 0.4)))
}

/// An ailment set in or its damage over time started a moment ago: the HP he just lost was not a
/// blow's (the hammer's guard asks).
pub fn procced_lately() -> bool {
    with(|s| s.procs.iter().any(|&p| within(p, 0.5)) || within(s.new_dot, 0.5))
}

/// Blood loss set in a moment ago and its burst not paid yet: true once (its blow costs at least
/// BLEED_COST; the next ones their own).
pub fn take_bled() -> bool {
    with(|s| within(s.procs[BLEED], 0.4) && std::mem::take(&mut s.bled_unpaid))
}

/// The blows' price to set now (tough.rs), if it isn't what was set last (none set yet: always).
fn frail_change(set: Option<f32>, want: f32) -> Option<f32> {
    (set != Some(want)).then_some(want)
}

/// What the ailments on him cost him now (penalty).
fn now_penalty(s: &State) -> Penalty {
    penalty(s.poisoned[0], s.poisoned[1], within(s.procs[FROST], FROSTBITE_FOR))
}

/// Poisoned: how far his stick reaches (his top speed's share).
pub fn stick_scale() -> f32 {
    with(|s| now_penalty(s).legs)
}

/// Rotting: the share of a coin's healing he gets.
pub fn heal_scale() -> f32 {
    with(|s| now_penalty(s).heal)
}

/// Madness set in since last asked: his style and a charge in his hand go (lib.rs).
pub fn take_madness() -> bool {
    with(|s| std::mem::take(&mut s.madness))
}

/// What an ailment costs Mario besides the game's own damage, while it lasts: his stick's reach
/// (his top speed), a coin's healing, a blow's price (tough.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Penalty {
    pub legs: f32,
    pub heal: f32,
    pub frail: f32,
}

/// Poisoned: heavy legs. Rotting: the rot eats half of a coin's healing. Frostbitten: every blow
/// costs more. (Each its own, all at once if he has them all.)
pub fn penalty(poisoned: bool, rotting: bool, frostbitten: bool) -> Penalty {
    Penalty {
        legs: if poisoned { POISON_LEGS } else { 1.0 },
        heal: if rotting { ROT_HEAL } else { 1.0 },
        frail: if frostbitten { FROSTBITE_FRAIL } else { 1.0 },
    }
}

/// A floor of the map (its HitMtrlParam row: the effects it puts on whoever stands on it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Floor {
    Plain,
    /// a burn (SM64's lava boost answers it)
    Lava,
    /// a swamp that builds up poison (POISON) or rot (ROT)
    Swamp(usize),
}

/// One of a floor's effects, as far as it matters here.
#[derive(Clone, Copy, Debug, Default)]
pub struct FloorEffect {
    pub id: i32,
    /// it takes HP itself
    pub hurts: bool,
    /// its poison's and its rot's build-up
    pub poison: i32,
    pub rot: i32,
}

/// What a floor is, by its effects: a build-up of poison or rot makes a swamp (whatever it also
/// takes: the rot's swamps hurt too), lava's own burn (LAVA_BURNS) a lava floor. A floor that only
/// hurts some other way is no lava. Caelid's rot ground lists lava's burns in its row too, yet only
/// gives rot: the lava boost also needs the burn on him (lava_now).
pub fn floor_kind(effects: &[FloorEffect]) -> Floor {
    let rot: i32 = effects.iter().map(|e| e.rot.max(0)).sum();
    let poison: i32 = effects.iter().map(|e| e.poison.max(0)).sum();
    if rot > 0 || poison > 0 {
        Floor::Swamp(if rot >= poison { ROT } else { POISON })
    } else if effects.iter().any(|e| LAVA_BURNS.contains(&e.id)) {
        Floor::Lava
    } else {
        Floor::Plain
    }
}

/// How long after lava's burn was last on him its floor still answers with SM64's lava boost (s):
/// he's bounced up off it meanwhile.
pub const LAVA_KEEP: f32 = 0.5;

/// Lava's own burn among the effects on him (their param ids).
pub fn burning(effects: impl IntoIterator<Item = i32>) -> bool {
    effects.into_iter().any(|id| LAVA_BURNS.contains(&id))
}

/// Lava under him now: a lava floor (floor_kind) and its burn on him, or on him `since_burn`
/// seconds ago. The floor's row alone isn't enough: Caelid's rot ground lists lava's burns too but
/// only gives rot, and bounced him in SM64's lava boost non-stop (2026-10-26).
pub fn lava_now(lava_floor: bool, since_burn: Option<f32>) -> bool {
    lava_floor && since_burn.is_some_and(|s| s < LAVA_KEEP)
}

/// Game thread, every frame: wading in a swamp of `kind` (none: out of it): bubbles round his feet
/// in its colour (the swamp builds the ailment up on the Tarnished, as on anyone).
pub fn wade(kind: Option<usize>, feet: Vec3, material: i32) {
    let fresh = with(|s| {
        let fresh = kind.is_some() && s.wading != kind;
        s.wading = kind;
        if let Some(k) = kind {
            if !within(s.last_bubble, WADE_EVERY) {
                s.last_bubble = Some(Instant::now());
                let off = Vec3::new(crate::rand01() - 0.5, 0.0, crate::rand01() - 0.5) * 0.7;
                crate::impact::element_spark(feet + off + Vec3::Y * 0.1, tint(k), 0.3);
            }
        }
        fresh
    });
    if let (true, Some(k)) = (fresh, kind) {
        log(format!("status: wading in a {} swamp (floor material {material})", NAMES[k]));
    }
}

fn tint(kind: usize) -> [f32; 3] {
    if kind == POISON { [0.45, 0.9, 0.3] } else { [1.0, 0.45, 0.2] }
}

/// Game thread, with Mario's inputs: frozen stiff or asleep, he doesn't move (true: drop the
/// inputs). `pressed`: a button down this tick (asleep, presses wake him sooner).
pub fn holds(pressed: bool, id: i32) -> bool {
    let woke = with(|s| {
        let Some(mut until) = s.held_until else {
            s.pressed = pressed;
            return None;
        };
        if s.asleep && pressed && !s.pressed {
            until -= std::time::Duration::from_secs_f32(WAKE_PER_PRESS);
        }
        s.pressed = pressed;
        if Instant::now() >= until {
            s.held_until = None;
            return Some(std::mem::take(&mut s.asleep));
        }
        s.held_until = Some(until);
        None
    });
    match woke {
        None => with(|s| s.held_until.is_some()),
        Some(asleep) => {
            if asleep {
                log("status: woke up");
                worker::call("wake", move |_| unsafe { sm64::sm64_er_status(id, WAKE, 0) });
            }
            false
        }
    }
}

/// A hit on Mario: asleep, it wakes him.
pub fn hit() {
    with(|s| {
        if s.asleep {
            s.held_until = Some(Instant::now());
        }
    });
}

/// An ailment's gauge going from `prev` to `now` (of `max`) in a frame: maybe it set in (run out,
/// or filled back up by most of it at once; wearing off, it creeps back up).
fn sets_in(prev: u32, now: u32, max: u32) -> bool {
    (now == 0 && prev > 0) || (now > prev && now - prev > max / 2)
}

/// The ailments' gauges (what's left of each resistance) and their maxima, from the resist module.
fn gauges(player: &ChrIns) -> Option<([u32; 7], [u32; 7])> {
    const GAUGES: usize = 0x10;
    const MAXIMA: usize = 0x2c;
    let module = unsafe { *((&*player.modules as *const _ as usize + 0x20) as *const usize) };
    if !crate::explore::readable(module, 0x50) {
        return None;
    }
    let now = unsafe { *((module + GAUGES) as *const [u32; 7]) };
    let max = unsafe { *((module + MAXIMA) as *const [u32; 7]) };
    (max[0] > 0).then_some((now, max))
}

/// Game thread, every frame: ailments setting in (and burns) on the Tarnished, and Mario's answer.
/// `feet`: Elden Ring; `calm`: Mario may be made to cough, sleep... (not dead, riding, held, big);
/// `hurt`: the Tarnished lost HP this frame.
pub fn tick(player: &ChrIns, id: i32, feet: Vec3, calm: bool, hurt: bool) {
    let now_g = gauges(player);
    burns(player);
    let set_in: Vec<usize> = with(|s| {
        let stale = !within(s.last_tick, STALE);
        s.last_tick = Some(Instant::now());
        if hurt {
            s.hp_loss = Some(Instant::now());
        }
        if let Some((now, max)) = now_g {
            if let (Some(prev), false) = (s.gauges, stale) {
                for i in 0..7 {
                    if reacts_to(i) && sets_in(prev[i], now[i], max[i]) && !within(s.procs[i], 1.5) {
                        s.jumps[i] = Some(Instant::now());
                    }
                }
            }
            s.gauges = Some(now);
        }
        // a jump confirmed by its damage (the poisons by their draining effect; sleep has none)
        let mut out = Vec::new();
        for i in 0..7 {
            let Some(j) = s.jumps[i] else { continue };
            let sign = match i {
                POISON | ROT => s.new_dot,
                SLEEP => Some(j),
                _ => s.hp_loss,
            };
            let close = sign.is_some_and(|t| (t.elapsed().as_secs_f32() - j.elapsed().as_secs_f32()).abs() < CONFIRM);
            if close {
                s.jumps[i] = None;
                s.procs[i] = Some(Instant::now());
                if i == POISON || i == ROT {
                    s.poisoned[usize::from(i == ROT)] = true;
                }
                if i == BLEED {
                    s.bled_unpaid = true;
                }
                out.push(i);
            } else if j.elapsed().as_secs_f32() > CONFIRM {
                s.jumps[i] = None;
                log(format!("status: {} gauge refilled with no damage (a cure or a rest)", NAMES[i]));
            }
        }
        out
    });
    for &i in &set_in {
        log(format!("status: {} set in", NAMES[i]));
        react(i, feet);
    }
    // (poison and rot last while something drains him; frostbite its while)
    let frail = with(|s| {
        if !s.draining && s.poisoned != [false; 2] {
            s.poisoned = [false; 2];
            log("status: the poison wore off");
        }
        let f = frail_change(s.frail, now_penalty(s).frail)?;
        s.frail = Some(f);
        Some(f)
    });
    if let Some(f) = frail {
        if f > 1.0 {
            log(format!("status: frostbitten, blows cost x{f:.2} for {FROSTBITE_FOR:.0} s"));
        }
        crate::tough::set_frail(f);
        worker::call("frail", |_| unsafe { sm64::sm64_er_set_loss_share(crate::tough::loss_share()) });
    }
    ignite(feet);
    if calm {
        attempt(id);
    }
    flair(feet, calm);
}

/// Mario shows this ailment (all but deathblight: the game's own instant death takes it).
fn reacts_to(i: usize) -> bool {
    i != BLIGHT
}

/// What Mario does about an ailment: its flash now, his reaction when he's calm.
fn react(i: usize, feet: Vec3) {
    let queue = |kind: i32| with(|s| s.pending.push((kind, Instant::now())));
    match i {
        POISON | ROT => {
            crate::impact::status_puff(feet + Vec3::Y * 0.9, if i == POISON { [0.45, 0.9, 0.3] } else { [1.0, 0.45, 0.2] });
            queue(COUGH);
        }
        BLEED => {
            crate::impact::bleed(feet + Vec3::Y * 0.8);
            worker::call("bleed", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_MAMA_MIA, 0.6) });
        }
        FROST => {
            crate::impact::frost(feet);
            queue(SHIVER);
        }
        SLEEP => queue(DOZE),
        MADNESS => {
            crate::impact::flash([1.0, 0.85, 0.2], 0.35, 0.4);
            with(|s| s.madness = true);
            queue(SHOCK);
        }
        _ => {}
    }
}

/// The reactions waiting for a calm moment: tried each tick until they take, for a while.
fn attempt(id: i32) {
    let (pending, dropped) = with(|s| {
        let dropped: Vec<i32> = s.pending.iter().filter(|(_, t)| t.elapsed().as_secs_f32() >= REACT_FOR).map(|p| p.0).collect();
        s.pending.retain(|(_, t)| t.elapsed().as_secs_f32() < REACT_FOR);
        (s.pending.clone(), dropped)
    });
    for kind in dropped {
        log(format!("status: reaction {kind} never took in {REACT_FOR:.0} s (Mario never on the ground free)"));
    }
    for (kind, since) in pending {
        let took = worker::call("status", move |_| unsafe { sm64::sm64_er_status(id, kind, BURN_TICKS) }).unwrap_or(0) != 0;
        if !took {
            continue;
        }
        with(|s| {
            s.pending.retain(|p| p.1 != since || p.0 != kind);
            let now = Instant::now();
            match kind {
                SHIVER => {
                    s.held_until = Some(now + std::time::Duration::from_secs_f32(FROZEN));
                    s.idle_act = Some(now);
                }
                DOZE => {
                    s.held_until = Some(now + std::time::Duration::from_secs_f32(ASLEEP));
                    s.asleep = true;
                }
                COUGH => s.idle_act = Some(now),
                BURN => s.burning_since = Some(now),
                _ => {}
            }
        });
    }
    // (SM64 coughs and shivers until the stick moves: enough after a while)
    let stop = with(|s| s.idle_act.is_some_and(|t| t.elapsed().as_secs_f32() > IDLE_ACT_FOR) && s.idle_act.take().is_some());
    if stop {
        worker::call("status idle", move |_| unsafe { sm64::sm64_er_status(id, STOP_IDLE, 0) });
    }
}

/// The poison's puffs, the flames off him while he burns.
fn flair(feet: Vec3, calm: bool) {
    with(|s| {
        let tint = [POISON, ROT].into_iter().find(|&i| s.poisoned[usize::from(i == ROT)] || within(s.procs[i], PUFF_FOR)).map(tint);
        if let Some(c) = tint {
            if !within(s.last_puff, PUFF_EVERY) {
                s.last_puff = Some(Instant::now());
                crate::impact::status_puff(feet + Vec3::Y * 0.9, c);
            }
        }
        if within(s.procs[FROST], FROSTBITE_FOR) && !within(s.last_glint, GLINT_EVERY) {
            s.last_glint = Some(Instant::now());
            let off = Vec3::new(crate::rand01() - 0.5, crate::rand01() * 0.8, crate::rand01() - 0.5) * 0.6;
            crate::impact::element_spark(feet + Vec3::Y * 0.5 + off, [0.6, 0.85, 1.0], 0.5);
        }
        if calm && within(s.burning_since, BURN_TICKS as f32 / 30.0) && !within(s.last_puff, 0.12) {
            s.last_puff = Some(Instant::now());
            crate::impact::flame_lick(feet + Vec3::Y * (0.4 + crate::rand01() * 0.6));
        }
    });
}

/// New effects on the Tarnished (each id logged once, with what it does): one that hurts over time
/// marks a poison setting in, or (with none) a burn.
fn burns(player: &ChrIns) {
    let mut log_lines = Vec::new();
    with(|s| {
        let (mut fresh_dot, mut fire, mut draining) = (false, false, false);
        let mut now_ids = std::mem::take(&mut s.effects);
        let before = now_ids.clone();
        now_ids.clear();
        for e in player.special_effect.entries() {
            now_ids.push(e.param_id);
            let Some(p) = e.param_data.map(|p| unsafe { p.as_ref() }) else { continue };
            let hurts = p.change_hp_point() > 0 || p.change_hp_rate() > 0.0;
            let life = p.effect_endurance();
            let (dot, burn) = drain_kind(e.param_id, hurts, life, p.vfx_id(), p.state_info());
            draining |= dot;
            if before.contains(&e.param_id) {
                continue;
            }
            if (hurts || p.state_info() != 0) && s.seen.len() < 4096 && s.seen.insert(e.param_id) {
                log_lines.push(format!(
                    "status: effect {} (state {}, hp {} / {:.1}%, {:.1} s, vfx {})",
                    e.param_id,
                    p.state_info(),
                    p.change_hp_point(),
                    p.change_hp_rate(),
                    life,
                    p.vfx_id()
                ));
            }
            fresh_dot |= dot;
            fire |= burn;
        }
        s.effects = now_ids;
        s.draining = draining;
        if fresh_dot {
            s.new_dot = Some(Instant::now());
        }
        if fire && s.burn.is_none() {
            s.burn = Some(Instant::now());
        }
    });
    for l in log_lines {
        log(l);
    }
}

/// An effect on the Tarnished (`id`, it takes HP, lasting `life` s, its vfx and state): whether it
/// drains him over time (a poison's or rot's, a burn's), and whether it's a burn. The poisons last
/// long (the rot from a Kindred of Rot, 4053: 180 s): before, only effects of 20 s at most counted,
/// so poison and rot never took ("gauge refilled with no damage"). A burn is short.
fn drain_kind(id: i32, hurts: bool, life: f32, vfx: i32, state: u16) -> (bool, bool) {
    let dot = hurts && life > 0.0 && !LAVA_BURNS.contains(&id);
    (dot, dot && life <= BURN_MAX_LIFE && looks_like_fire(vfx, state))
}

/// Whether a new effect that hurts is something burning on him: it shows (an effect on the body)
/// or has a state of its own. Reforged takes HP off in one go with effects that have neither
/// (5002351/5002352: 4.5% and 6% once, no vfx, state 0, on some of its hits): each one set Mario
/// off on SM64's burning run, out of control for 1.5 s in the middle of a fight.
fn looks_like_fire(vfx: i32, state: u16) -> bool {
    vfx > 0 || state != 0
}

/// A draining effect that no ailment came with, a moment later: Mario's on fire.
fn ignite(feet: Vec3) {
    let lit = with(|s| {
        let Some(t) = s.burn else { return false };
        if t.elapsed().as_secs_f32() < CONFIRM {
            return false;
        }
        s.burn = None;
        let ailment = [POISON, ROT, BLEED, FROST, MADNESS].iter().any(|&i| within(s.procs[i], NOT_AN_AILMENT) || s.jumps[i].is_some());
        if !ailment {
            s.pending.push((BURN, Instant::now()));
        }
        !ailment
    });
    if lit {
        log("status: burning");
        crate::impact::fire_pop(feet + Vec3::Y * 0.6);
    }
}

pub fn clear() {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::sets_in;

    #[test]
    fn reforged_one_off_drain_is_no_fire() {
        // Reforged's 5002351 / 5002352: no vfx, state 0
        assert!(!super::looks_like_fire(-1, 0));
        // a burn with flames on the body, or a state of its own
        assert!(super::looks_like_fire(6, 6));
        assert!(super::looks_like_fire(-1, 6));
        assert!(super::looks_like_fire(8640, 0));
    }

    #[test]
    fn a_long_poison_drains_and_is_no_fire() {
        use super::drain_kind;
        // the Kindred of Rot's rot (log 2026-10-25): 8 HP and 2% a while, 180 s, vfx 5, state 5
        assert_eq!(drain_kind(4053, true, 180.0, 5, 5), (true, false));
        // a burn: short, with flames on him
        assert_eq!(drain_kind(9999, true, 10.0, 6, 6), (true, true));
        // lava's own burn, an effect for good, one that takes nothing: no
        assert_eq!(drain_kind(4101, true, 5.0, 6, 6), (false, false));
        assert_eq!(drain_kind(100620, true, -1.0, -1, 323), (false, false));
        assert_eq!(drain_kind(505, false, 900.0, -1, 275), (false, false));
    }

    #[test]
    fn each_ailment_costs_its_own() {
        use super::{penalty, Penalty};
        assert_eq!(penalty(false, false, false), Penalty { legs: 1.0, heal: 1.0, frail: 1.0 });
        let p = penalty(true, false, false);
        assert!(p.legs < 0.9 && p.legs >= 0.8 && p.heal == 1.0 && p.frail == 1.0);
        let r = penalty(false, true, false);
        assert!(r.heal == 0.5 && r.legs == 1.0 && r.frail == 1.0);
        let f = penalty(false, false, true);
        assert!(f.frail > 1.1 && f.frail <= 1.25 && f.legs == 1.0 && f.heal == 1.0);
        // blood loss: more than a wedge at once, less than two
        assert!(super::BLEED_COST > 0x100 && super::BLEED_COST < 0x200);
    }

    /// The reviewer: frostbitten, then dead within 30 s: the new state thought the price was 1.0
    /// already and never put back the game-wide x1.2.
    #[test]
    fn a_new_state_sets_the_price_afresh() {
        use super::frail_change;
        assert_eq!(frail_change(None, 1.0), Some(1.0));
        assert_eq!(frail_change(Some(1.2), 1.0), Some(1.0));
        assert_eq!(frail_change(Some(1.0), 1.0), None);
    }

    #[test]
    fn lava_only_while_its_burn_is_on_him() {
        use super::{burning, lava_now};
        // Caelid's rot ground lists lava's burns in its row but puts only rot on him (2026-10-26)
        assert!(!burning([4051, 505, 100620]));
        assert!(burning([505, 4101]));
        assert!(burning([4100]));
        // a lava floor with its burn on him (or a moment ago: bounced up off it): the boost
        assert!(lava_now(true, Some(0.0)));
        assert!(lava_now(true, Some(0.4)));
        // the burn long gone, never had, or not on a lava floor: none
        assert!(!lava_now(true, Some(0.8)));
        assert!(!lava_now(true, None));
        assert!(!lava_now(false, Some(0.0)));
    }

    #[test]
    fn swamps_are_no_lava() {
        use super::{floor_kind, Floor, FloorEffect, POISON, ROT};
        // lava only by its own burns (4100, 4101)
        assert_eq!(floor_kind(&[FloorEffect { id: 4101, ..Default::default() }]), Floor::Lava);
        assert_eq!(floor_kind(&[FloorEffect { id: 4100, hurts: true, ..Default::default() }]), Floor::Lava);
        // a floor that hurts some other way isn't lava: Caelid's rot swamp (its build-up comes from
        // an effect of its effect, read as none here) bounced him in SM64's lava boost (2026-10-25)
        assert_eq!(floor_kind(&[FloorEffect { id: 7777, hurts: true, ..Default::default() }]), Floor::Plain);
        assert_eq!(
            floor_kind(&[FloorEffect { id: 7777, hurts: true, ..Default::default() }, FloorEffect { id: 12, ..Default::default() }]),
            Floor::Plain
        );
        // a build-up: a swamp, even one that hurts too
        assert_eq!(floor_kind(&[FloorEffect { id: 4550, poison: 8, ..Default::default() }]), Floor::Swamp(POISON));
        assert_eq!(floor_kind(&[FloorEffect { id: 4560, hurts: true, rot: 6, ..Default::default() }]), Floor::Swamp(ROT));
        // nothing: plain ground
        assert_eq!(floor_kind(&[]), Floor::Plain);
        assert_eq!(floor_kind(&[FloorEffect { id: 12, ..Default::default() }]), Floor::Plain);
    }

    /// Deathblight is vanilla's (the user, 2026-10-25): nothing of the mod's stands between it and
    /// the game's instant death, and Mario has no reaction to it; the others he shows.
    #[test]
    fn deathblight_is_the_games_own() {
        use super::{reacts_to, BLEED, BLIGHT, FROST, MADNESS, POISON, ROT, SLEEP};
        assert!(!reacts_to(BLIGHT));
        for i in [POISON, ROT, BLEED, FROST, SLEEP, MADNESS] {
            assert!(reacts_to(i));
        }
    }

    #[test]
    fn ailment_sets_in() {
        // building up, wearing off: no
        assert!(!sets_in(300, 250, 300));
        assert!(!sets_in(100, 104, 300));
        // run out, or refilled at once: yes
        assert!(sets_in(20, 0, 300));
        assert!(sets_in(20, 300, 300));
        // at rest: no
        assert!(!sets_in(0, 0, 300));
        assert!(!sets_in(300, 300, 300));
    }
}
