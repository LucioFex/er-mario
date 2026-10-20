//! Game version check: fromsoftware-rs's game addresses are for one game version, on any other the
//! mod stays out of the way. The mod's own (the hooks') are found by byte pattern, `find`: they
//! follow the game from one patch to the next, and one not found only switches its hook off.

use fromsoftware_shared::game_version::{GameVersion, LANG_ID_EN, LANG_ID_JP};
use pelite::pe64::PeView;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

/// exe version 2.7.2.0 (worldwide) / 2.7.2.1 (Japan): what fromsoftware-rs supports
pub struct Supported;

impl GameVersion for Supported {
    const NAME: &'static str = "elden ring";

    fn from_lang_version(lang_id: u16, version: &str) -> Option<Self> {
        matches!((lang_id, version), (LANG_ID_EN, "2.7.2.0") | (LANG_ID_JP, "2.7.2.1")).then_some(Supported)
    }
}

/// Ok, or why the mod can't run on this game.
pub fn check() -> Result<(), String> {
    let module = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
    let view = unsafe { PeView::module(module.0 as *const u8) };
    std::panic::catch_unwind(|| Supported::detect(&view).map(|_| ()).map_err(|e| e.to_string()))
        .unwrap_or_else(|_| Err("could not read the game's version".into()))
}

/// The game's code `what` by its bytes (`pattern` as in equip.rs, `??` for what moves from one
/// build to the next: calls' targets, globals), `offset` bytes from it: only if it's there just
/// once. (A function's own start is left out of its pattern, offset back to it: another mod's hook
/// on it, a jump over its first bytes, mustn't hide it.)
pub fn find(what: &str, pattern: &str, offset: isize) -> Option<usize> {
    let hits = crate::equip::scan(pattern);
    match hits.as_slice() {
        [at] => Some(at.wrapping_add_signed(offset)),
        _ => {
            let n = if hits.len() > 4 { "5 or more".to_string() } else { hits.len().to_string() };
            crate::log(format!("version: {what} not found ({n} matches): its hook stays off"));
            None
        }
    }
}
