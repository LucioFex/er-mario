//! The weapons' elements: each weapon (weapons.rs) imbues Mario's blows and his RB shot
//! (fireball.rs) with one of Elden Ring's:
//! - the fists: fire. No status of its own: an enemy weak to fire takes more of every blow
//! - the hammer: frost. It builds up frostbite: a burst of damage, and for a while the enemy takes
//!   more of every blow
//! - the Chain Chomp: blood. It builds up blood loss: a burst of damage
//! - the boomerang: poison. It builds up poison: damage a little each second for a while
//! Each enemy takes them as Elden Ring has him: his resistance to each status (NpcParam) is how much
//! it takes to set it off (immune ones never), and a blow of an element he's weak to, or resists,
//! is worth a little more or less (WEAK or RESIST over him, the first time). A status set off makes
//! him tougher to it for a while (a boss much more), and what's built up fades if left alone. So
//! the weapon to use depends on the enemy, and switching pays.
//! The statuses' damage goes through combat.rs's shares like Mario's blows (a boss's few %); in
//! co-op each player builds their own, their bursts dealt by the host like their blows.
//! Taking a weapon shows its element: a burst of it at his hand and its soft sound, then a mote of
//! it now and then round the weapon (embers, snowflakes of ice, wisps of darkness, bubbles); the
//! HUD shows it by the weapon (hud.rs).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ChrIns, FieldInsHandle, NpcParam, SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::combat::{handle_key, Attack};
use crate::{log, sm64, worker};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Element {
    Fire = 0,
    Frost = 1,
    Blood = 2,
    Poison = 3,
}

impl Element {
    pub fn from_u8(v: u8) -> Element {
        match v {
            1 => Element::Frost,
            2 => Element::Blood,
            3 => Element::Poison,
            _ => Element::Fire,
        }
    }

    /// The element weapon `w` (weapons.rs) imbues.
    pub fn of_weapon(w: u8) -> Element {
        match w {
            crate::weapons::HAMMER => Element::Frost,
            crate::weapons::CHOMP => Element::Blood,
            crate::weapons::RANG => Element::Poison,
            _ => Element::Fire,
        }
    }

    pub fn name(self) -> &'static str {
        ["FIRE", "FROST", "BLOOD", "POISON"][self as usize]
    }

    /// Its colour (the HUD's, the callouts', the sparks')
    pub fn tint(self) -> [f32; 3] {
        [[1.0, 0.55, 0.2], [0.6, 0.88, 1.0], [0.9, 0.15, 0.2], [0.6, 0.9, 0.3]][self as usize]
    }
}

/// The element of the weapon in Mario's hand.
pub fn current() -> Element {
    Element::of_weapon(crate::weapons::current())
}

/// The element `attack` carries: its shot's own, else the weapon in hand's; None for what's no
/// blow of his weapon (the trap kit, Yoshi, the hook's flights, Mega's steps, a captured enemy's
/// blows, the colossus, a status itself).
pub fn of_attack(attack: Attack) -> Option<Element> {
    match attack {
        Attack::Fire(_) | Attack::FireSweep | Attack::Ember => Some(Element::Fire),
        Attack::Ice(_) => Some(Element::Frost),
        Attack::Dark(_) => Some(Element::Blood),
        Attack::Venom(_) | Attack::VenomCloud => Some(Element::Poison),
        Attack::Slip
        | Attack::ShellHit
        | Attack::Bomb
        | Attack::Boing
        | Attack::Rock
        | Attack::RockSlam
        | Attack::Hook
        | Attack::Tongue
        | Attack::Egg
        | Attack::YoshiStomp
        | Attack::Trample
        | Attack::RideBlow
        | Attack::RideSmash
        | Attack::RideCrash
        | Attack::HomeRun
        | Attack::Bowled
        | Attack::Tether
        | Attack::Sigil
        | Attack::SigilTap
        | Attack::BrosAttack
        | Attack::Frostbite
        | Attack::Hemorrhage
        | Attack::PoisonTick => None,
        _ => Some(current()),
    }
}

/// How much a punch's worth of a blow builds up (Elden Ring's points: a regular enemy's resistance
/// is some 70-250, a boss's 250-550), by element: a status takes a string of blows, never one or
/// two (at 45 the hammer's frostbite came every other swing); frost the slowest, Elden Ring's
/// quickest
const BUILD: [f32; 4] = [0.0, 24.0, 28.0, 30.0];
/// what any status takes at the least (a regular enemy with next to no resistance still takes a
/// string: five of the hammer's heaviest, a dozen bites, a dozen boomerang throws or six or seven
/// charged poison globs)
const MIN_NEEDS: f32 = 160.0;
/// each status set off adds this much of his resistance again (a boss's, a regular enemy's)
const TOUGHER_BOSS: f32 = 0.75;
const TOUGHER: f32 = 0.35;
/// ...for this long after the last one (ticks: 45 s)
const TOUGHER_FOR: u32 = 1350;
/// what's built up starts fading after this long without a blow of it (ticks), this much of the
/// resistance a tick (12% a second)
const FADE_AFTER: u32 = 90;
const FADE: f32 = 0.004;
/// frostbite: how long (ticks: 10 s), every blow on him this much more
const FROST_TICKS: u32 = 300;
const FROST_BONUS: f32 = 1.2;
/// poison: how long (ticks: 20 s), a tick of it every this many
const POISON_TICKS: u32 = 600;
const POISON_EVERY: u32 = 30;
/// blood loss's burst shows this long (ticks)
const BLOOD_SHOWS: u32 = 48;
/// an enemy left alone this long is forgotten (ticks)
const FORGET: u32 = 1800;
/// weak to an element from this factor up, resisting it from this one down (WEAK, RESIST)
const WEAK: f32 = 1.1;
const RESISTS: f32 = 0.88;

// SM64 sounds (soft: they come again and again)
const SOUND_SHORT_STAR: i32 = 0x3016_0091;
const SOUND_SNOWMAN_EXPLODE: i32 = 0x5065_D081;
const SOUND_BUBBA: i32 = 0x5073_4081;
const SOUND_CHOMP_2: i32 = 0x303A_0081;
const SOUND_SKEETER_DEATH: i32 = 0x5072_C081;
const SOUND_BUBBLE: i32 = 0x300B_0081;
const SOUND_BUBBLE_2: i32 = 0x300D_0081;
const SOUND_FLAME_BLOWN: i32 = 0x5055_8081;
const SOUND_SNOW: i32 = 0x502A_0081;
const SOUND_CHOMP_1: i32 = 0x3039_0081;

/// An enemy as the elements see him (NpcParam): each element's worth against him, and each status's
/// resistance (0: none; 999 and up: immune)
#[derive(Clone, Copy)]
struct Npc {
    factor: [f32; 4],
    resist: [f32; 4],
}

const NO_NPC: Npc = Npc { factor: [1.0; 4], resist: [0.0; 4] };

/// A resistance's worth as a factor on the element's blows, against the enemy's own resistances
/// to the other statuses (`mean`: theirs on average, the immune left out): the one he resists least
/// a little more, the one he resists most a little less; immune, the least. (Against a fixed scale
/// every regular enemy, all of them low, came out weak to every status: x1.2 on every blow of the
/// hammer, the Chomp and the boomerang alike.)
fn of_resist(r: f32, mean: f32) -> f32 {
    if r >= 999.0 {
        return 0.8;
    }
    if r <= 0.0 || mean <= 0.0 {
        return 1.0;
    }
    (1.0 + (mean / r - 1.0) * 0.5).clamp(0.8, 1.25)
}

/// The resistances' average, the immune and the missing left out (0: none).
fn mean_resist(resist: &[f32]) -> f32 {
    let known: Vec<f32> = resist.iter().copied().filter(|r| *r > 0.0 && *r < 999.0).collect();
    if known.is_empty() {
        0.0
    } else {
        known.iter().sum::<f32>() / known.len() as f32
    }
}

/// Fire's worth against him: his damage cut rate to fire against a blow's (strike), doubled and a
/// half, within 0.8-1.25 (1 if the row says nothing).
fn fire_factor(fire: f32, strike: f32) -> f32 {
    if !(fire.is_finite() && strike.is_finite() && fire > 0.0 && strike > 0.05) {
        return 1.0;
    }
    (1.0 + (fire / strike - 1.0) * 1.5).clamp(0.8, 1.25)
}

fn npc(id: i32) -> Npc {
    static CACHE: Mutex<Option<HashMap<i32, Npc>>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(n) = cache.get(&id) {
        return *n;
    }
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return NO_NPC };
    let Some(p) = repo.get::<NpcParam>(id as u32) else { return NO_NPC };
    let resist = [0.0, p.resist_freeze() as f32, p.resist_blood() as f32, p.resist_poison() as f32];
    let fire = fire_factor(p.fire_damage_cut_rate(), p.blow_damage_cut_rate());
    let mean = mean_resist(&resist[1..]);
    let n = Npc { factor: [fire, of_resist(resist[1], mean), of_resist(resist[2], mean), of_resist(resist[3], mean)], resist };
    log(format!(
        "element: NpcParam {id}: fire cut {:.2} (strike {:.2}) x{:.2}; frost {} x{:.2}, blood {} x{:.2}, poison {} x{:.2}",
        p.fire_damage_cut_rate(),
        p.blow_damage_cut_rate(),
        n.factor[0],
        resist[1],
        n.factor[1],
        resist[2],
        n.factor[2],
        resist[3],
        n.factor[3]
    ));
    if cache.len() > 512 {
        cache.clear();
    }
    cache.insert(id, n);
    n
}

/// What the statuses have built up on an enemy, and what's on him.
struct Meter {
    handle: FieldInsHandle,
    boss: bool,
    /// by element (Fire's unused): built up, the tick it last built, how many went off and the
    /// tick of the last, what it takes now
    build: [f32; 4],
    last: [u32; 4],
    procs: [u8; 4],
    last_proc: [u32; 4],
    needs: [f32; 4],
    /// the element built last (the HUD's bar)
    shown: Element,
    frost_until: u32,
    poison_until: u32,
    next_poison: u32,
    /// the last status set off, and when (its burst)
    burst: Option<(Element, u32)>,
    /// WEAK / RESIST said already, a bit per element
    told: u8,
    /// the tick anything last happened to him
    seen: u32,
}

static METERS: Mutex<Option<HashMap<u64, Meter>>> = Mutex::new(None);
/// the latest SM64 tick (the shares' frostbite check comes without one)
static NOW: AtomicU32 = AtomicU32::new(0);

fn meters() -> std::sync::MutexGuard<'static, Option<HashMap<u64, Meter>>> {
    METERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Game thread (combat.rs take_share): the share of a blow of `attack` on him, as his element
/// makes it: weak to it or resisting it, and frostbitten.
pub fn damage_factor(handle: &FieldInsHandle, chr: &ChrIns, attack: Attack) -> f32 {
    let k = of_attack(attack).map_or(1.0, |e| npc(chr.npc_param_id).factor[e as usize]);
    let now = NOW.load(Ordering::Relaxed);
    let frozen = meters().as_ref().and_then(|m| m.get(&handle_key(handle))).is_some_and(|m| now < m.frost_until);
    if frozen {
        k * FROST_BONUS
    } else {
        k
    }
}

/// Game thread (combat.rs deal): a blow of `attack` landed on `handle` (his chest at `chest`): its
/// element's WEAK or RESIST the first time, its status built up. The status's blow to deal, if it
/// went off now (poison deals its own, `tick`).
pub fn struck(handle: &FieldInsHandle, attack: Attack, chest: Vec3, tick: u32) -> Option<Attack> {
    let e = of_attack(attack)?;
    NOW.store(tick, Ordering::Relaxed);
    let wcm = unsafe { WorldChrMan::instance() }.ok()?;
    let chr = wcm.chr_ins_by_handle(handle)?;
    let info = npc(chr.npc_param_id);
    let boss = crate::combat::is_boss_now(handle) || crate::combat::boss_class(handle, chr);
    let cid = chr.character_id;
    let key = handle_key(handle);
    let mut guard = meters();
    let all = guard.get_or_insert_with(HashMap::new);
    if all.len() > 64 {
        all.retain(|_, m| tick.wrapping_sub(m.seen) < FORGET);
    }
    let m = all.entry(key).or_insert_with(|| Meter {
        handle: *handle,
        boss,
        build: [0.0; 4],
        last: [tick; 4],
        procs: [0; 4],
        last_proc: [tick; 4],
        needs: [0.0; 4],
        shown: e,
        frost_until: 0,
        poison_until: 0,
        next_poison: 0,
        burst: None,
        told: 0,
        seen: tick,
    });
    m.seen = tick;
    m.boss = boss;
    let i = e as usize;
    // what the element is worth against him, said over him the first time
    let k = info.factor[i];
    let weak = k >= WEAK;
    if m.told & (1 << i) == 0 && (weak || k <= RESISTS) {
        m.told |= 1 << i;
        let over = Some(chest + Vec3::Y * 0.7);
        if weak {
            crate::style::callout("WEAK", over, e.tint(), false, 0.0);
            worker::call("element weak", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.2) });
        } else {
            crate::style::callout("RESIST", over, [0.62, 0.62, 0.66], false, 0.0);
        }
        log(format!("element: c{cid:04} {} to {} (x{k:.2})", if weak { "weak" } else { "resists" }, e.name()));
    }
    // (a spark of the element on him: a bigger one where it tells; not for what burns on its own
    // pace, the giant fireball's, the embers, the poison cloud)
    if !matches!(attack, Attack::FireSweep | Attack::Ember | Attack::VenomCloud) {
        crate::impact::element_spark(chest, e.tint(), if weak { 1.0 } else { 0.55 });
    }
    if e == Element::Fire {
        return None;
    }
    // immune: nothing builds up
    let resist = if info.resist[i] > 0.0 { info.resist[i] } else { 300.0 };
    if resist >= 999.0 {
        return None;
    }
    // (frostbite and poison run their course before they can come again)
    if (e == Element::Frost && tick < m.frost_until) || (e == Element::Poison && tick < m.poison_until) {
        return None;
    }
    if tick.wrapping_sub(m.last_proc[i]) >= TOUGHER_FOR {
        m.procs[i] = 0;
    }
    let tougher = if boss { TOUGHER_BOSS } else { TOUGHER };
    let needs = resist.max(MIN_NEEDS) * (1.0 + tougher * m.procs[i] as f32);
    m.needs[i] = needs;
    m.build[i] += BUILD[i] * attack.weight();
    m.last[i] = tick;
    m.shown = e;
    if m.build[i] < needs {
        return None;
    }
    m.build[i] = 0.0;
    m.procs[i] = m.procs[i].saturating_add(1);
    m.last_proc[i] = tick;
    m.burst = Some((e, tick));
    let over = Some(chest + Vec3::Y * 0.9);
    log(format!("element: {} set off on c{cid:04}{} ({} time{}, it took {needs:.0})", e.name(), if boss { " (boss)" } else { "" }, m.procs[i], if m.procs[i] == 1 { "" } else { "s" }));
    match e {
        Element::Frost => {
            m.frost_until = tick + FROST_TICKS;
            crate::style::callout("FROSTBITE", over, e.tint(), true, 0.0);
            crate::impact::flash([0.5, 0.75, 1.0], 0.12, 0.35);
            worker::call("frostbite", |_| unsafe { sm64::sm64_play_sound_volume(SOUND_SNOWMAN_EXPLODE, 0.45) });
            Some(Attack::Frostbite)
        }
        Element::Blood => {
            crate::style::callout("BLOOD LOSS", over, e.tint(), true, 0.0);
            crate::impact::flash([0.8, 0.05, 0.08], 0.12, 0.3);
            crate::lakitu::shake(0.4);
            worker::call("blood loss", |_| unsafe {
                sm64::sm64_play_sound_volume(SOUND_BUBBA, 0.5);
                sm64::sm64_play_sound_volume(SOUND_CHOMP_2, 0.3);
            });
            Some(Attack::Hemorrhage)
        }
        _ => {
            m.poison_until = tick + POISON_TICKS;
            m.next_poison = tick + POISON_EVERY;
            crate::style::callout("POISONED", over, e.tint(), true, 0.0);
            worker::call("poisoned", |_| unsafe {
                sm64::sm64_play_sound_volume(SOUND_SKEETER_DEATH, 0.35);
                sm64::sm64_play_sound_volume(SOUND_BUBBLE_2, 0.4);
            });
            None
        }
    }
}

/// The statuses on enemies as fx.rs sends and elemfx.rs draws them (gathered each tick).
static AURAS: Mutex<Vec<crate::fx::Aura>> = Mutex::new(Vec::new());

/// Game thread, every SM64 tick: what's built up fades, the gone are forgotten, poison's ticks come
/// due (the blows to deal: each a PoisonTick on its enemy), the statuses' looks are gathered.
pub fn tick(tick: u32) -> Vec<(FieldInsHandle, Attack)> {
    NOW.store(tick, Ordering::Relaxed);
    let mut due = Vec::new();
    let mut auras = Vec::new();
    {
        let mut guard = meters();
        let Some(all) = guard.as_mut() else {
            AURAS.lock().unwrap_or_else(|e| e.into_inner()).clear();
            return due;
        };
        let wcm = unsafe { WorldChrMan::instance() }.ok();
        all.retain(|_, m| {
            let Some(chr) = wcm.as_ref().and_then(|w| w.chr_ins_by_handle(&m.handle)) else { return false };
            if chr.modules.data.hp <= 0 || tick.wrapping_sub(m.seen) >= FORGET {
                return false;
            }
            for i in 1..4 {
                if m.build[i] > 0.0 && tick.wrapping_sub(m.last[i]) > FADE_AFTER {
                    m.build[i] = (m.build[i] - m.needs[i].max(100.0) * FADE).max(0.0);
                }
            }
            if tick < m.poison_until && tick >= m.next_poison {
                m.next_poison = tick + POISON_EVERY;
                m.seen = tick;
                due.push((m.handle, Attack::PoisonTick));
            }
            // its looks: frostbite and poison while they last, a burst as one goes off
            let ph = &chr.modules.physics;
            let p = ph.position;
            let (r, h) = (ph.hit_radius.max(ph.chr_hit_radius), ph.hit_height.max(ph.chr_hit_height));
            let radius = if r.is_finite() && r > 0.1 { r.clamp(0.3, 4.0) } else { 0.5 };
            let height = if h.is_finite() && h > 0.3 { h.clamp(0.6, 8.0) } else { 1.8 };
            let pos = Vec3::new(p.0, p.1, p.2);
            let burst = m.burst.filter(|(_, at)| tick.wrapping_sub(*at) < crate::fx::AURA_LONG);
            let mut push = |el: Element, left: f32| {
                let age = burst.filter(|(e, _)| *e == el).map_or(crate::fx::AURA_LONG, |(_, at)| tick.wrapping_sub(at));
                if left > 0.0 || age < crate::fx::AURA_LONG {
                    auras.push(crate::fx::Aura { pos, height, radius, element: el as u8, age, left });
                }
            };
            push(Element::Frost, if tick < m.frost_until { (m.frost_until - tick) as f32 / FROST_TICKS as f32 } else { 0.0 });
            push(Element::Poison, if tick < m.poison_until { (m.poison_until - tick) as f32 / POISON_TICKS as f32 } else { 0.0 });
            // (blood loss has nothing lasting: its spray, its drops, 1.6 s)
            if burst.is_some_and(|(e, at)| e == Element::Blood && tick.wrapping_sub(at) < BLOOD_SHOWS) {
                push(Element::Blood, 0.0);
            }
            true
        });
    }
    *AURAS.lock().unwrap_or_else(|e| e.into_inner()) = auras;
    due
}

/// The statuses on enemies now (fx.rs).
pub fn auras() -> Vec<crate::fx::Aura> {
    AURAS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A status bar under an enemy's health (hud.rs): its colour, how full (built up against what it
/// takes; or the time a status on him has left) and whether it's on him now.
#[derive(Clone, Copy)]
pub struct Bar {
    pub tint: [f32; 3],
    pub fill: f32,
    pub on: bool,
}

/// Game thread: the bar to show under `handle`'s health, if anything's built up or on him.
pub fn bar(handle: &FieldInsHandle) -> Option<Bar> {
    let now = NOW.load(Ordering::Relaxed);
    let guard = meters();
    let m = guard.as_ref()?.get(&handle_key(handle))?;
    if now < m.frost_until {
        return Some(Bar { tint: Element::Frost.tint(), fill: (m.frost_until - now) as f32 / FROST_TICKS as f32, on: true });
    }
    if now < m.poison_until {
        return Some(Bar { tint: Element::Poison.tint(), fill: (m.poison_until - now) as f32 / POISON_TICKS as f32, on: true });
    }
    let i = m.shown as usize;
    (m.build[i] > 0.0 && m.needs[i] > 0.0).then(|| Bar { tint: m.shown.tint(), fill: (m.build[i] / m.needs[i]).clamp(0.0, 1.0), on: false })
}

/// A mote of a weapon's element in flight: where it started, its drift (m/s), how long it lives
/// (s), its size (m)
struct Live {
    pos: Vec3,
    vel: Vec3,
    el: Element,
    born: Instant,
    life: f32,
    size: f32,
}

static MOTES: Mutex<Vec<Live>> = Mutex::new(Vec::new());
const MOTES_MAX: usize = 32;

fn mote(pos: Vec3, vel: Vec3, el: Element, life: f32, size: f32) {
    let mut motes = MOTES.lock().unwrap_or_else(|e| e.into_inner());
    motes.retain(|m| m.born.elapsed().as_secs_f32() < m.life);
    if motes.len() >= MOTES_MAX {
        motes.remove(0);
    }
    motes.push(Live { pos, vel, el, born: Instant::now(), life, size });
}

/// How a mote of `el` drifts: embers and bubbles rise, ice falls slowly, darkness hangs.
fn drift(el: Element) -> Vec3 {
    match el {
        Element::Fire => Vec3::Y * 0.9,
        Element::Frost => -Vec3::Y * 0.25,
        Element::Blood => Vec3::Y * 0.15,
        Element::Poison => Vec3::Y * 0.5,
    }
}

fn mote_size(el: Element) -> f32 {
    match el {
        Element::Fire => 0.16,
        Element::Frost => 0.07,
        Element::Blood => 0.06,
        Element::Poison => 0.05,
    }
}

/// Game thread: weapon `w` just taken (weapons.rs, L1) at Mario's hand `hand`: a burst of its
/// element round it, its soft sound.
pub fn switched(w: u8, hand: Vec3) {
    let el = Element::of_weapon(w);
    for i in 0..10 {
        let a = i as f32 * std::f32::consts::TAU / 10.0;
        let out = Vec3::new(a.cos(), 0.35 + 0.3 * ((i * 7) % 3) as f32, a.sin());
        mote(hand + out * 0.12, out * 1.1 + drift(el), el, 0.5 + 0.05 * (i % 4) as f32, mote_size(el) * 1.2);
    }
    worker::call("element switched", move |_| unsafe {
        match el {
            Element::Fire => sm64::sm64_play_sound_volume(SOUND_FLAME_BLOWN, 0.25),
            Element::Frost => {
                sm64::sm64_play_sound_volume(SOUND_SNOW, 0.4);
                sm64::sm64_play_sound_volume(SOUND_SHORT_STAR, 0.15);
            }
            Element::Blood => sm64::sm64_play_sound_volume(SOUND_CHOMP_1, 0.3),
            Element::Poison => sm64::sm64_play_sound_volume(SOUND_BUBBLE, 0.45),
        }
    });
    log(format!("element: {} in hand", el.name()));
}

/// how often a mote rises off the weapon in hand (ticks)
const AMBIENT_EVERY: u32 = 9;

/// Game thread, every SM64 tick: now and then a mote of the element in hand off one of `points`
/// (Elden Ring: the fists, the hammer's head, the Chomp, the boomerang's hand).
pub fn ambient(points: &[Vec3], tick: u32) {
    if points.is_empty() || tick % AMBIENT_EVERY != 0 {
        return;
    }
    let el = current();
    let p = points[(tick / AMBIENT_EVERY) as usize % points.len()];
    let r = crate::rand01();
    let side = Vec3::new(r - 0.5, 0.0, crate::rand01() - 0.5) * 0.25;
    mote(p + side, side * 0.6 + drift(el), el, 0.7 + 0.3 * r, mote_size(el));
}

/// The motes now (fx.rs).
pub fn motes() -> Vec<crate::fx::Mote> {
    let motes = MOTES.lock().unwrap_or_else(|e| e.into_inner());
    motes
        .iter()
        .filter_map(|m| {
            let age = m.born.elapsed().as_secs_f32();
            (age < m.life).then(|| crate::fx::Mote { pos: m.pos + m.vel * age, element: m.el as u8, t: age / m.life, size: m.size })
        })
        .collect()
}

pub fn clear() {
    *meters() = None;
    AURAS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    MOTES.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resistances_make_it_a_little_more_or_less() {
        assert_eq!(of_resist(0.0, 100.0), 1.0);
        assert_eq!(of_resist(999.0, 100.0), 0.8);
        // (immune to all three: no average, still immune)
        assert_eq!(of_resist(999.0, mean_resist(&[999.0, 999.0, 999.0])), 0.8);
        // (one seen in the log: frost 90, blood 135, poison 68: weak to poison, resisting blood)
        let resist = [90.0, 135.0, 68.0];
        let mean = mean_resist(&resist);
        let [frost, blood, poison] = resist.map(|r| of_resist(r, mean));
        assert!(poison >= 1.1 && blood <= 0.88 && (0.95..1.1).contains(&frost), "{frost} {blood} {poison}");
        // all alike: nothing either way
        assert!((of_resist(150.0, mean_resist(&[150.0, 150.0, 150.0])) - 1.0).abs() < 1e-4);
        // the immune left out of the average
        assert_eq!(mean_resist(&[999.0, 100.0, 300.0]), 200.0);
        // (never past the moderate band, and lower always worth more)
        let mut last = 2.0;
        for r in (20..1100).step_by(25) {
            let k = of_resist(r as f32, 200.0);
            assert!((0.8..=1.25).contains(&k) && k <= last, "{r}: {k}");
            last = k;
        }
        assert_eq!(fire_factor(1.0, 1.0), 1.0);
        assert!(fire_factor(1.2, 1.0) > 1.2 && fire_factor(1.2, 1.0) <= 1.25);
        assert_eq!(fire_factor(0.2, 1.0), 0.8);
        assert_eq!(fire_factor(f32::NAN, 1.0), 1.0);
        assert_eq!(fire_factor(1.0, 0.0), 1.0);
    }

    #[test]
    fn each_weapon_its_element() {
        assert_eq!(Element::of_weapon(crate::weapons::FISTS), Element::Fire);
        assert_eq!(Element::of_weapon(crate::weapons::HAMMER), Element::Frost);
        assert_eq!(Element::of_weapon(crate::weapons::CHOMP), Element::Blood);
        assert_eq!(Element::of_weapon(crate::weapons::RANG), Element::Poison);
        for e in [Element::Fire, Element::Frost, Element::Blood, Element::Poison] {
            assert_eq!(Element::from_u8(e as u8), e);
        }
        // (the shots their own, whatever's in hand; the trap kit and the statuses none)
        assert_eq!(of_attack(Attack::Ice(2)), Some(Element::Frost));
        assert_eq!(of_attack(Attack::VenomCloud), Some(Element::Poison));
        assert_eq!(of_attack(Attack::Bomb), None);
        assert_eq!(of_attack(Attack::Hemorrhage), None);
    }

    #[test]
    fn statuses_take_a_string_of_blows() {
        let blows = |el: Element, attack: Attack, resist: f32| (resist.max(MIN_NEEDS) / (BUILD[el as usize] * attack.weight())).ceil() as u32;
        // the hammer's heaviest on the least resistant regular enemy, and on one seen in the log
        assert!(blows(Element::Frost, Attack::Titan, 68.0) >= 5);
        assert!(blows(Element::Frost, Attack::Hammer, 135.0) >= 5);
        assert!(blows(Element::Blood, Attack::Chomp, 68.0) >= 12);
        // frost the slowest of them
        assert!(BUILD[Element::Frost as usize] < BUILD[Element::Blood as usize]);
    }
}
