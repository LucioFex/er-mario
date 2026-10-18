//! A side channel to the other players of a session (co-op, coop.rs): Steam's
//! SteamNetworkingMessages from the game's own steam_api64.dll, on a channel of our own (the game
//! and Seamless Co-op never read it). Sending to a player also accepts their messages, so both
//! sides just send.

use std::sync::OnceLock;

use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{PCSTR, w};

use crate::log;

/// ours: "MA" (no other mod is known to use it)
pub const CHANNEL: i32 = 0x4D41;
/// k_nSteamNetworkingSend_*: unreliable, no Nagle delay, auto-restart a broken session
const UNRELIABLE: i32 = 1 | 32;
const RELIABLE: i32 = 8 | 32;
/// SteamNetworkingIdentity (pack 1): type, size, a 128-byte union
const IDENTITY_SIZE: usize = 136;
/// SteamNetworkingMessage_t: data pointer at 0, size at 8, the sender's identity at 16
const MSG_IDENTITY: usize = 16;

type Accessor = unsafe extern "C" fn() -> usize;
type Send = unsafe extern "C" fn(usize, *const u8, *const u8, u32, i32, i32) -> i32;
type Receive = unsafe extern "C" fn(usize, i32, *mut usize, i32) -> i32;
type Release = unsafe extern "C" fn(usize);
type SetId = unsafe extern "C" fn(*mut u8, u64);
type GetId = unsafe extern "C" fn(*const u8) -> u64;

struct Api {
    /// SteamNetworkingMessages(): asked each time (0 while Steam isn't up yet)
    messages: Accessor,
    send: Send,
    receive: Receive,
    release: Release,
    set_id: SetId,
    get_id: GetId,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

/// The API and its interface (None while Steam has none).
fn api() -> Option<(&'static Api, usize)> {
    API.get_or_init(|| {
        let module = unsafe { GetModuleHandleW(w!("steam_api64.dll")) }.ok()?;
        let get = |name: &str| {
            let c = format!("{name}\0");
            unsafe { GetProcAddress(module, PCSTR(c.as_ptr())) }.map(|f| f as usize)
        };
        let names = [
            "SteamAPI_SteamNetworkingMessages_SteamAPI_v002",
            "SteamAPI_ISteamNetworkingMessages_SendMessageToUser",
            "SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel",
            "SteamAPI_SteamNetworkingMessage_t_Release",
            "SteamAPI_SteamNetworkingIdentity_SetSteamID64",
            "SteamAPI_SteamNetworkingIdentity_GetSteamID64",
        ];
        let f: Vec<usize> = names.iter().filter_map(|n| get(n)).collect();
        if f.len() != names.len() {
            log(format!("net: steam_api64 lacks the messages API ({} of {} functions)", f.len(), names.len()));
            return None;
        }
        log(format!("net: Steam messages API found, channel {CHANNEL:#x}"));
        unsafe {
            Some(Api {
                messages: std::mem::transmute::<usize, Accessor>(f[0]),
                send: std::mem::transmute::<usize, Send>(f[1]),
                receive: std::mem::transmute::<usize, Receive>(f[2]),
                release: std::mem::transmute::<usize, Release>(f[3]),
                set_id: std::mem::transmute::<usize, SetId>(f[4]),
                get_id: std::mem::transmute::<usize, GetId>(f[5]),
            })
        }
    })
    .as_ref()
    .and_then(|a| {
        let m = unsafe { (a.messages)() };
        (m != 0).then_some((a, m))
    })
}

/// Our own Steam id (the co-op self test sends to it).
pub fn my_id() -> Option<u64> {
    type User = unsafe extern "C" fn() -> usize;
    type GetSteamId = unsafe extern "C" fn(usize) -> u64;
    let module = unsafe { GetModuleHandleW(w!("steam_api64.dll")) }.ok()?;
    let user = unsafe { GetProcAddress(module, PCSTR(c"SteamAPI_SteamUser_v021".as_ptr() as *const u8)) }?;
    let get = unsafe { GetProcAddress(module, PCSTR(c"SteamAPI_ISteamUser_GetSteamID".as_ptr() as *const u8)) }?;
    let u = unsafe { std::mem::transmute::<_, User>(user)() };
    (u != 0).then(|| unsafe { std::mem::transmute::<_, GetSteamId>(get)(u) }).filter(|&id| id != 0)
}

/// Sends `data` to the player with this Steam id. False if Steam refused it.
pub fn send(peer: u64, data: &[u8], reliable: bool) -> bool {
    let Some((a, messages)) = api() else { return false };
    let mut id = [0u8; IDENTITY_SIZE];
    unsafe { (a.set_id)(id.as_mut_ptr(), peer) };
    let flags = if reliable { RELIABLE } else { UNRELIABLE };
    // EResult 1 = OK
    unsafe { (a.send)(messages, id.as_ptr(), data.as_ptr(), data.len() as u32, flags, CHANNEL) == 1 }
}

/// Every message waiting on our channel: (sender's Steam id, bytes).
pub fn receive(mut f: impl FnMut(u64, &[u8])) {
    let Some((a, messages)) = api() else { return };
    let mut msgs = [0usize; 32];
    // (at most a few batches a frame)
    for _ in 0..8 {
        let n = unsafe { (a.receive)(messages, CHANNEL, msgs.as_mut_ptr(), msgs.len() as i32) }.min(msgs.len() as i32);
        if n <= 0 {
            return;
        }
        for &m in &msgs[..n as usize] {
            let (data, size) = unsafe { (*(m as *const usize), *((m + 8) as *const i32)) };
            let from = unsafe { (a.get_id)((m + MSG_IDENTITY) as *const u8) };
            if data != 0 && size > 0 {
                f(from, unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) });
            }
            unsafe { (a.release)(m) };
        }
        if (n as usize) < msgs.len() {
            return;
        }
    }
}
