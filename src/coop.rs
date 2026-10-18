//! Co-op (Seamless Co-op, docs/coop): each player runs their own Mario (or Luigi) and the game
//! only syncs the Tarnished under him, so the mod sends the rest itself over net.rs:
//! - the pose: the 31 parts of the Mario model, ~30 times a second; the partner's character is
//!   posed with it (engine_mario::apply_remote), so each sees the other as Mario or Luigi;
//! - the guest's blows: the host owns the enemies (a health the guest writes is overwritten by
//!   the host's), so the guest's hits go to the host, who takes them off (Combat::remote_hits);
//! - shoves: a blow on the partner pushes them a little (no damage).
//! Outside a session nothing here runs: single player is untouched.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eldenring::cs::{CSSessionManager, FieldInsHandle, LobbyState, PlayerIns, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Quat, Vec3};

use crate::engine_mario::{PARTS, PartPose};
use crate::{explore, log, net};

const MAGIC: [u8; 3] = *b"ERM";
/// bumped when a packet changes: the other side's older ones are ignored (and logged)
const VERSION: u8 = 1;
const KIND_POSE: u8 = 1;
const KIND_HIT: u8 = 2;
const KIND_SHOVE: u8 = 3;
/// nothing but "here I am": Steam takes a player's messages only once we've sent them something,
/// so this goes out every second even while we send nothing else (e.g. out of Mario mode)
const KIND_HELLO: u8 = 4;
const HEADER: usize = 5;
/// one part: rotation i16 x4, position mm i16 x3, scale and stretch u16 /1000 x4
const PART_BYTES: usize = 22;
const POSE_BYTES: usize = HEADER + 4 + PARTS * PART_BYTES;
const HIT_BYTES: usize = HEADER + 8 + 4 + 12 + 1 + 4;
const SHOVE_BYTES: usize = HEADER + 8;

/// how often the pose goes out
const POSE_EVERY: Duration = Duration::from_millis(33);
/// how often the session is looked at
const SESSION_EVERY: Duration = Duration::from_millis(500);
/// a blow pushes the partner this close (m, times Mario's size), at most this often
const SHOVE_REACH: f32 = 1.4;
const SHOVE_EVERY: Duration = Duration::from_millis(1000);
const HELLO_EVERY: Duration = Duration::from_millis(1000);
/// a partner's pose with nothing new for this long is let go (they left Mario mode or the area)
const POSE_STALE: f32 = 1.0;
/// a guest's blow older than this when the host gets to it is dropped (not all at once later)
const HIT_STALE: f32 = 0.5;

pub struct Hit {
    pub handle: FieldInsHandle,
    pub chr_id: u32,
    pub pos: [f32; 3],
    /// Attack::code (combat::IMPACT_CODE for an impact)
    pub attack: u8,
    /// the share of max HP the guest worked out (their caps, Mega and counters in it)
    pub pct: f32,
}

pub enum Packet {
    Pose { seq: u32, parts: Box<[PartPose; PARTS]> },
    Hit(Hit),
    Shove { dir: [f32; 2] },
    Hello,
}

fn header(kind: u8, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(kind);
    out
}

fn q16(v: f32, k: f32) -> [u8; 2] {
    ((v * k).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16).to_le_bytes()
}

fn u16k(v: f32) -> [u8; 2] {
    ((v * 1000.0).round().clamp(0.0, u16::MAX as f32) as u16).to_le_bytes()
}

pub fn encode(p: &Packet) -> Vec<u8> {
    match p {
        Packet::Pose { seq, parts } => {
            let mut out = header(KIND_POSE, POSE_BYTES);
            out.extend_from_slice(&seq.to_le_bytes());
            for part in parts.iter() {
                let r = part.rot.normalize();
                for v in [r.x, r.y, r.z, r.w] {
                    out.extend_from_slice(&q16(v, 32767.0));
                }
                for v in [part.pos.x, part.pos.y, part.pos.z] {
                    out.extend_from_slice(&q16(v, 1000.0));
                }
                for v in [part.scale, part.stretch.x, part.stretch.y, part.stretch.z] {
                    out.extend_from_slice(&u16k(v));
                }
            }
            out
        }
        Packet::Hit(h) => {
            let mut out = header(KIND_HIT, HIT_BYTES);
            out.extend_from_slice(&crate::combat::handle_key(&h.handle).to_le_bytes());
            out.extend_from_slice(&h.chr_id.to_le_bytes());
            for v in h.pos {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.push(h.attack);
            out.extend_from_slice(&h.pct.to_le_bytes());
            out
        }
        Packet::Shove { dir } => {
            let mut out = header(KIND_SHOVE, SHOVE_BYTES);
            out.extend_from_slice(&dir[0].to_le_bytes());
            out.extend_from_slice(&dir[1].to_le_bytes());
            out
        }
        Packet::Hello => header(KIND_HELLO, HEADER),
    }
}

pub fn decode(b: &[u8]) -> Option<Packet> {
    if b.len() < HEADER || b[..3] != MAGIC || b[3] != VERSION {
        return None;
    }
    let i16at = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]) as f32;
    let u16at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as f32 / 1000.0;
    let f32at = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    match b[4] {
        KIND_POSE if b.len() == POSE_BYTES => {
            let seq = u32::from_le_bytes([b[5], b[6], b[7], b[8]]);
            let mut parts = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
            for (k, p) in parts.iter_mut().enumerate() {
                let o = HEADER + 4 + k * PART_BYTES;
                let q = Quat::from_xyzw(i16at(o), i16at(o + 2), i16at(o + 4), i16at(o + 6));
                p.rot = if q.length_squared() > 0.0 { q.normalize() } else { Quat::IDENTITY };
                p.pos = Vec3::new(i16at(o + 8), i16at(o + 10), i16at(o + 12)) / 1000.0;
                p.scale = u16at(o + 14);
                p.stretch = Vec3::new(u16at(o + 16), u16at(o + 18), u16at(o + 20));
            }
            Some(Packet::Pose { seq, parts: Box::new(parts) })
        }
        KIND_HIT if b.len() == HIT_BYTES => {
            let key = u64::from_le_bytes(b[5..13].try_into().ok()?);
            let (pos, pct) = ([f32at(17), f32at(21), f32at(25)], f32at(30));
            if !pos.iter().all(|v| v.is_finite()) || !pct.is_finite() {
                return None;
            }
            Some(Packet::Hit(Hit {
                handle: unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) },
                chr_id: u32::from_le_bytes(b[13..17].try_into().ok()?),
                pos,
                attack: b[29],
                pct,
            }))
        }
        KIND_SHOVE if b.len() == SHOVE_BYTES => {
            let dir = [f32at(5), f32at(9)];
            dir.iter().all(|v| v.is_finite()).then_some(Packet::Shove { dir })
        }
        KIND_HELLO if b.len() == HEADER => Some(Packet::Hello),
        _ => None,
    }
}

/// A partner's pose as it comes in: the last two, blended over the time between them.
struct Remote {
    steam: u64,
    prev: Option<[PartPose; PARTS]>,
    last: [PartPose; PARTS],
    seq: u32,
    at: Instant,
    gap: f32,
}

impl Remote {
    fn now(&self) -> [PartPose; PARTS] {
        match &self.prev {
            Some(prev) => crate::engine_mario::blend(prev, &self.last, (self.at.elapsed().as_secs_f32() / self.gap).min(1.0)),
            None => self.last,
        }
    }
}

#[derive(Default)]
struct State {
    checked: Option<Instant>,
    /// the session's other players (Steam ids), the host's, whether that's us
    peers: Vec<u64>,
    host_id: u64,
    host: bool,
    /// each partner's character this frame (Steam id, PlayerIns address)
    chrs: Vec<(u64, usize)>,
    remotes: Vec<Remote>,
    sent: Option<Instant>,
    hello: Option<Instant>,
    seq: u32,
    hits: Vec<(Instant, Hit)>,
    shove: Option<Vec3>,
    shoved: Option<Instant>,
    old_version: bool,
    unpaired: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static GUEST: AtomicBool = AtomicBool::new(false);

/// `coop_test = ...` in er_mario.ini (developer tests with one PC): `loopback` sends every kind of
/// packet to ourselves over Steam and logs what comes back; `guest` plays as a guest would (no
/// grabs or moving enemies, their health never below 1).
fn test_mode(what: &str) -> bool {
    static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    MODE.get_or_init(|| {
        let mode = crate::paths::config("coop_test").unwrap_or_default().to_ascii_lowercase();
        if !mode.is_empty() {
            log(format!("coop: test mode '{mode}'"));
        }
        mode
    })
    .contains(what)
}

/// The loopback test: sent and received counts, when last sent, kinds seen.
struct Loopback {
    sent: u32,
    got: u32,
    at: Option<Instant>,
    summary: Option<Instant>,
    seen: Vec<u8>,
    no_id: bool,
}
static LOOPBACK: Mutex<Loopback> = Mutex::new(Loopback { sent: 0, got: 0, at: None, summary: None, seen: Vec::new(), no_id: false });

fn loopback() {
    let mut t = LOOPBACK.lock().unwrap_or_else(|e| e.into_inner());
    net::receive(|from, data| {
        t.got += 1;
        let kind = data.get(4).copied().unwrap_or(0);
        if !t.seen.contains(&kind) {
            t.seen.push(kind);
            let what = match decode(data) {
                Some(Packet::Pose { seq, parts }) => format!("pose seq {seq}, head at {:.2} m", parts[3].pos.length()),
                Some(Packet::Hit(h)) => format!("hit c{:04} code {} {:.1}%", h.chr_id, h.attack, h.pct),
                Some(Packet::Shove { dir }) => format!("shove {dir:?}"),
                Some(Packet::Hello) => "hello".into(),
                None => "UNREADABLE".into(),
            };
            log(format!("coop test: loopback got {what} from {from:x} ({} B)", data.len()));
        }
    });
    if t.at.is_some_and(|a| a.elapsed() < Duration::from_secs(1)) {
        return;
    }
    t.at = Some(Instant::now());
    let Some(me) = net::my_id() else {
        if !std::mem::replace(&mut t.no_id, true) {
            log("coop test: no Steam id of our own");
        }
        return;
    };
    let pose = (*crate::engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner()))
        .unwrap_or([PartPose { rot: Quat::IDENTITY, pos: Vec3::Y, scale: 1.0, stretch: Vec3::ONE }; PARTS]);
    let key = 0x0001_0002_0000_0003u64;
    let packets = [
        (encode(&Packet::Hello), false),
        (encode(&Packet::Pose { seq: t.sent + 1, parts: Box::new(pose) }), false),
        (encode(&Packet::Hit(Hit { handle: unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) }, chr_id: 4750, pos: [1.0, 2.0, 3.0], attack: 0, pct: 4.5 })), true),
        (encode(&Packet::Shove { dir: [0.6, 0.8] }), true),
    ];
    for (bytes, reliable) in packets {
        if net::send(me, &bytes, reliable) {
            t.sent += 1;
        }
    }
    if t.summary.is_none_or(|s| s.elapsed() >= Duration::from_secs(10)) {
        t.summary = Some(Instant::now());
        log(format!("coop test: loopback to {me:x}: {} sent, {} received", t.sent, t.got));
    }
}

/// In a session with someone.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// In someone else's world: the host owns the enemies (no grabs, no moving them).
pub fn guest() -> bool {
    GUEST.load(Ordering::Relaxed)
}

/// The session's players: (us, other players, the host's id, whether we're the host if the
/// lobby says so).
fn session() -> Option<(u64, Vec<u64>, u64, Option<bool>)> {
    let sm = unsafe { CSSessionManager::instance() }.ok()?;
    let lobby = match sm.lobby_state {
        LobbyState::Host => Some(true),
        LobbyState::Client => Some(false),
        _ => None,
    };
    let mut me = 0;
    let mut host = sm.host_player.as_option().map_or(0, |h| h.steam_id);
    let mut others = Vec::new();
    for p in sm.players.iter() {
        let id = p.base.steam_id;
        if id == 0 {
            continue;
        }
        if p.is_local_player {
            me = id;
        } else if !others.contains(&id) {
            others.push(id);
        }
        if p.is_host {
            host = id;
        }
    }
    (!others.is_empty()).then_some((me, others, host, lobby))
}

/// The Steam id a partner's character is played from.
fn remote_identity(p: &PlayerIns) -> Option<u64> {
    // PlayerSessionHolder +0x18 PlayerNetworkSession, its +0x8 the Steam id
    let holder = &p.player_session_holder as *const _ as usize;
    let session = explore::read_u64(holder + 0x18)? as usize;
    explore::read_u64(session + 8).filter(|&id| id != 0)
}

/// The other players' characters: (Steam id, PlayerIns address).
fn partner_chrs(peers: &[u64], unpaired: &mut bool) -> Vec<(u64, usize)> {
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return Vec::new() };
    let main = wcm.main_player.as_ref().map(|p| p.as_ptr() as usize);
    let others: Vec<&mut PlayerIns> = wcm.player_chr_set.characters().filter(|p| Some(*p as *const PlayerIns as usize) != main).collect();
    let mut out: Vec<(u64, usize)> = others
        .iter()
        .filter_map(|p| remote_identity(p).filter(|id| peers.contains(id)).map(|id| (id, &**p as *const PlayerIns as usize)))
        .collect();
    // (the Steam id not where it's expected: with one partner and one other character, that's them)
    let lone = out.is_empty() && peers.len() == 1 && others.len() == 1;
    if lone {
        out.push((peers[0], &*others[0] as *const PlayerIns as usize));
    }
    if lone != *unpaired {
        *unpaired = lone;
        if lone {
            log("coop: partner's character paired by elimination (no Steam id on it)");
        }
    }
    out
}

fn refresh(s: &mut State) {
    let (me, peers, host_id, lobby) = session().unwrap_or_default();
    // (the lobby's word first; else by the ids; with no word at all we're the host: as in single
    // player, the enemies are ours)
    let host = peers.is_empty() || lobby.unwrap_or(host_id == 0 || host_id == me);
    if peers != s.peers || host != s.host || host_id != s.host_id {
        log(format!(
            "coop: session {:x?}, host {:x} ({}), me {me:x}",
            peers,
            host_id,
            if peers.is_empty() { "none" } else if host { "us" } else { "them" }
        ));
        s.remotes.retain(|r| peers.contains(&r.steam));
        s.peers = peers;
        s.host = host;
        s.host_id = host_id;
    }
    // (who's who changes rarely: looked up with the session, checked for life where used)
    s.chrs = if s.peers.is_empty() { Vec::new() } else { partner_chrs(&s.peers, &mut s.unpaired) };
    ACTIVE.store(!s.peers.is_empty(), Ordering::Relaxed);
    GUEST.store((!s.peers.is_empty() && !s.host) || test_mode("guest"), Ordering::Relaxed);
}

/// The packets that came in (sender, bytes), into the state.
fn take_in(s: &mut State, batch: Vec<(u64, Vec<u8>)>) {
    for (from, data) in batch {
        if !s.peers.contains(&from) {
            continue;
        }
        let Some(p) = decode(&data) else {
            if data.len() > 3 && data[..3] == MAGIC && !s.old_version {
                s.old_version = true;
                log(format!("coop: packets of version {} from {from:x} (ours {VERSION}): both need the same ER Mario", data[3]));
            }
            continue;
        };
        match p {
            Packet::Pose { seq, parts } => match s.remotes.iter_mut().find(|r| r.steam == from) {
                // (an old one, out of order: dropped)
                Some(r) if seq.wrapping_sub(r.seq) < u32::MAX / 2 && seq != r.seq => {
                    let gap = r.at.elapsed().as_secs_f32().clamp(0.02, 0.2);
                    r.prev = Some(r.now());
                    r.last = *parts;
                    r.seq = seq;
                    r.at = Instant::now();
                    r.gap = gap;
                }
                Some(_) => {}
                None => {
                    log(format!("coop: pose from {from:x}"));
                    s.remotes.push(Remote { steam: from, prev: None, last: *parts, seq, at: Instant::now(), gap: 0.033 });
                }
            },
            Packet::Hit(h) if s.host => {
                if s.hits.len() < 64 {
                    s.hits.push((Instant::now(), h));
                }
            }
            Packet::Hit(_) | Packet::Hello => {}
            Packet::Shove { dir } => s.shove = Some(Vec3::new(dir[0], 0.0, dir[1])),
        }
    }
    // (they stopped sending: out of Mario mode, gone)
    s.remotes.retain(|r| {
        let fresh = r.at.elapsed().as_secs_f32() < POSE_STALE;
        if !fresh {
            log(format!("coop: no pose from {:x} lately", r.steam));
        }
        fresh
    });
}

/// Game thread, every frame: the session, the partners' packets, our pose out. (Steam is called
/// with the state unlocked: the pose tasks wait on it.)
pub fn frame() {
    let peers = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let s = guard.get_or_insert_with(State::default);
        if s.checked.is_none_or(|t| t.elapsed() >= SESSION_EVERY) {
            s.checked = Some(Instant::now());
            refresh(s);
        }
        s.peers.clone()
    };
    if peers.is_empty() {
        if test_mode("loopback") {
            loopback();
        }
        return;
    }
    let mut batch = Vec::new();
    net::receive(|from, data| {
        if batch.len() < 256 {
            batch.push((from, data.to_vec()));
        }
    });
    let pose = *crate::engine_mario::POSE.lock().unwrap_or_else(|e| e.into_inner());
    let out = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        take_in(s, batch);
        let mut out = Vec::new();
        if let Some(parts) = pose.filter(|_| s.sent.is_none_or(|t| t.elapsed() >= POSE_EVERY)) {
            s.sent = Some(Instant::now());
            s.seq = s.seq.wrapping_add(1);
            out.push(encode(&Packet::Pose { seq: s.seq, parts: Box::new(parts) }));
        }
        if s.hello.is_none_or(|t| t.elapsed() >= HELLO_EVERY) {
            s.hello = Some(Instant::now());
            out.push(encode(&Packet::Hello));
        }
        out
    };
    for bytes in out {
        for &peer in &peers {
            net::send(peer, &bytes, false);
        }
    }
}

/// Every pose task: each partner's character in their Mario pose.
pub fn apply_remote() {
    if !active() {
        return;
    }
    // (only while the partner row has the other character's model: out of Mario mode their
    // Tarnished is left alone)
    if !crate::equip::partner_row_on() {
        return;
    }
    let poses: Vec<(usize, [PartPose; PARTS])> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_ref() else { return };
        s.remotes.iter().filter_map(|r| s.chrs.iter().find(|c| c.0 == r.steam).map(|c| (c.1, r.now()))).collect()
    };
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    // (only characters still there: one may have left since the session was looked at)
    let live: Vec<usize> = wcm.player_chr_set.characters().map(|p| p as *const PlayerIns as usize).collect();
    for (chr, pose) in poses.iter().filter(|p| live.contains(&p.0)) {
        let ins = unsafe { &(*(*chr as *const PlayerIns)).chr_ins } as *const _ as usize;
        crate::engine_mario::apply_remote(ins, pose);
    }
}

/// Players in the session (1 out of one).
pub fn players() -> usize {
    if !active() {
        return 1;
    }
    STATE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(1, |s| s.peers.len() + 1)
}

/// The enemies' extra health per extra player (%), regular and boss, that Mario's blows go by:
/// `coop_health_scaling` / `coop_boss_health_scaling` in er_mario.ini, else Seamless Co-op's own
/// defaults (35 / 100). Not read from Seamless's ersc_settings.ini: Reforged sets its scaling to 0
/// there, and with Mario's blows being shares of max HP, two Marios would melt everything twice as
/// fast.
fn health_scaling() -> (f32, f32) {
    static SCALING: std::sync::OnceLock<(f32, f32)> = std::sync::OnceLock::new();
    *SCALING.get_or_init(|| {
        let key = |k: &str, default: f32| crate::paths::config(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(default);
        let s = (key("coop_health_scaling", 35.0), key("coop_boss_health_scaling", 100.0));
        log(format!("coop: with two, enemies take blows as if {}% healthier per extra player (bosses {}%)", s.0, s.1));
        s
    })
}

/// What Mario's blows (shares of max HP) are worth with this many players (1 alone).
pub fn share_factor(boss: bool) -> f32 {
    let extra = players().saturating_sub(1) as f32;
    if extra == 0.0 {
        return 1.0;
    }
    let (regular, bosses) = health_scaling();
    1.0 / (1.0 + extra * if boss { bosses } else { regular } / 100.0).max(1.0)
}

/// Guest: a blow of ours on an enemy (its share of max HP, all bonuses in), for the host to take off.
pub fn send_hit(handle: &FieldInsHandle, attack: u8, pct: f32) {
    if !guest() {
        return;
    }
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(chr) = wcm.chr_ins_by_handle(handle) else { return };
    let p = chr.modules.physics.position;
    let bytes = encode(&Packet::Hit(Hit { handle: *handle, chr_id: chr.character_id, pos: [p.0, p.1, p.2], attack, pct }));
    // (the host's id unknown: everyone, only a host takes it)
    let to: Vec<u64> = {
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_ref() else { return };
        if s.peers.contains(&s.host_id) { vec![s.host_id] } else { s.peers.clone() }
    };
    for peer in to {
        net::send(peer, &bytes, true);
    }
}

/// Host: the guests' blows since last time (not the ones that waited too long).
pub fn take_hits() -> Vec<Hit> {
    let hits = STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(|s| std::mem::take(&mut s.hits)).unwrap_or_default();
    hits.into_iter().filter(|(t, _)| t.elapsed().as_secs_f32() < HIT_STALE).map(|(_, h)| h).collect()
}

/// A partner pushed us: the way to go (ER axes, horizontal).
pub fn take_shove() -> Option<Vec3> {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut().and_then(|s| s.shove.take())
}

/// Game thread, each SM64 tick: Mario is punching or kicking at `me` (ER world): a partner within
/// reach is pushed away.
pub fn shove_partners(me: Vec3, attacking: bool) {
    if !attacking || !active() {
        return;
    }
    let chrs = {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        if s.shoved.is_some_and(|t| t.elapsed() < SHOVE_EVERY) {
            return;
        }
        s.chrs.clone()
    };
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return };
    let live: Vec<usize> = wcm.player_chr_set.characters().map(|p| p as *const PlayerIns as usize).collect();
    let reach = SHOVE_REACH * crate::size_factor();
    let mut shoved = false;
    for &(steam, chr) in chrs.iter().filter(|c| live.contains(&c.1)) {
        let p = unsafe { &*(chr as *const PlayerIns) }.chr_ins.modules.physics.position;
        let off = Vec3::new(p.0 - me.x, 0.0, p.2 - me.z);
        if off.length() > reach || (p.1 - me.y).abs() > 1.5 {
            continue;
        }
        let dir = off.normalize_or(Vec3::Z);
        net::send(steam, &encode(&Packet::Shove { dir: [dir.x, dir.z] }), true);
        shoved = true;
        log(format!("coop: shoved partner {steam:x}"));
    }
    if shoved {
        if let Some(s) = STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            s.shoved = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose() -> [PartPose; PARTS] {
        let mut parts = [PartPose { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: 1.0, stretch: Vec3::ONE }; PARTS];
        for (k, p) in parts.iter_mut().enumerate() {
            let f = k as f32;
            p.rot = Quat::from_euler(glam::EulerRot::YXZ, f * 0.3, -f * 0.1, f * 0.05);
            p.pos = Vec3::new(f * 0.05 - 0.7, f * 0.04, -f * 0.03);
            p.scale = if k % 7 == 0 { 0.01 } else { 1.0 + f * 0.02 };
            p.stretch = Vec3::new(1.0, 1.2, 0.8);
        }
        parts
    }

    #[test]
    fn pose_round_trip() {
        let a = pose();
        let bytes = encode(&Packet::Pose { seq: 7, parts: Box::new(a) });
        assert_eq!(bytes.len(), POSE_BYTES);
        let Some(Packet::Pose { seq, parts }) = decode(&bytes) else { panic!("no pose") };
        assert_eq!(seq, 7);
        for (x, y) in a.iter().zip(parts.iter()) {
            assert!(x.rot.angle_between(y.rot) < 0.001);
            assert!(x.pos.distance(y.pos) < 0.001);
            assert!((x.scale - y.scale).abs() < 0.001);
            assert!(x.stretch.distance(y.stretch) < 0.002);
        }
    }

    #[test]
    fn hit_and_shove_round_trip() {
        let key = 0x0012_3456_0000_00ABu64;
        let handle = unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&key) };
        let bytes = encode(&Packet::Hit(Hit { handle, chr_id: 4750, pos: [1.0, -2.5, 300.25], attack: 203, pct: 0.65 }));
        let Some(Packet::Hit(h)) = decode(&bytes) else { panic!("no hit") };
        assert_eq!(crate::combat::handle_key(&h.handle), key);
        assert_eq!((h.chr_id, h.pos, h.attack, h.pct), (4750, [1.0, -2.5, 300.25], 203, 0.65));
        let Some(Packet::Shove { dir }) = decode(&encode(&Packet::Shove { dir: [0.6, -0.8] })) else { panic!("no shove") };
        assert_eq!(dir, [0.6, -0.8]);
    }

    #[test]
    fn alone_blows_are_worth_the_same() {
        // out of a session: single player, nothing scaled
        assert_eq!(share_factor(false), 1.0);
        assert_eq!(share_factor(true), 1.0);
    }

    #[test]
    fn rejects_foreign_or_broken() {
        let mut bytes = encode(&Packet::Shove { dir: [1.0, 0.0] });
        assert!(decode(&bytes[..bytes.len() - 1]).is_none());
        bytes[3] = VERSION + 1;
        assert!(decode(&bytes).is_none());
        assert!(decode(b"hello world").is_none());
        assert!(decode(&[]).is_none());
    }
}
