//! Where the mod's files live: everything sits next to er_mario.dll (the mod folder), so the mod
//! works wherever the player unpacks it. er_mario.ini there can point at the SM64 ROM; without it
//! the first .z64/.n64/.v64 in the mod folder is used.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW,
    GetModuleHandleExW,
};
use windows::core::PCWSTR;

pub const CONFIG: &str = "er_mario.ini";

/// The folder er_mario.dll was loaded from.
pub fn mod_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let mut module = HMODULE::default();
        let mut buf = [0u16; 1024];
        let len = unsafe {
            let anchor = mod_dir as *const () as *const u16;
            let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
            if GetModuleHandleExW(flags, PCWSTR(anchor), &mut module).is_err() {
                return PathBuf::from(".");
            }
            GetModuleFileNameW(Some(module), &mut buf) as usize
        };
        let dll = PathBuf::from(String::from_utf16_lossy(&buf[..len]));
        dll.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
    })
}

/// A file of the mod's: next to the DLL, the package's ("package/...") in package_dir.
pub fn file(name: &str) -> PathBuf {
    match name.strip_prefix("package") {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => package_dir().join(rest.trim_start_matches('/')),
        _ => mod_dir().join(name),
    }
}

/// Where the built model files go, for me3 to serve: `package_dir = ...` from er_mario.ini
/// (absolute, or relative to the mod folder); with the DLL in Elden Ring Reforged's dll\offline,
/// Reforged's addons\er-mario (its launcher loads the asset mods there); else `package` next to
/// the DLL.
pub fn package_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Some(p) = config("package_dir").filter(|p| !p.is_empty()) {
            let p = PathBuf::from(p);
            return if p.is_absolute() { p } else { mod_dir().join(p) };
        }
        // (an install that already built its package next to the DLL keeps it there: its own
        // profile serves it)
        let own = mod_dir().join("package");
        reforged_addons(mod_dir()).filter(|_| !own.join(".built").is_file()).unwrap_or(own)
    })
}

/// `<Reforged>/addons/er-mario` when `dir` is `<Reforged>/dll/offline` and the addons folder is there.
fn reforged_addons(dir: &Path) -> Option<PathBuf> {
    let named = |p: &Path, n: &str| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.eq_ignore_ascii_case(n));
    let dll = dir.parent().filter(|_| named(dir, "offline"))?;
    let root = dll.parent().filter(|_| named(dll, "dll"))?;
    let addons = root.join("addons");
    addons.is_dir().then(|| addons.join("er-mario"))
}

/// `base_assets = ...` from er_mario.ini: a folder of another mod's game files (e.g. Reforged's)
/// that the asset build starts from instead of the game's own where it has them (the menu
/// textures, the material list, Torrent), so the mod's copies keep that mod's changes.
pub fn base_assets() -> Option<PathBuf> {
    let p = PathBuf::from(config("base_assets").filter(|p| !p.is_empty())?);
    Some(if p.is_absolute() { p } else { mod_dir().join(p) })
}

/// `key = value` from er_mario.ini (lines starting with # or ; are comments).
pub fn config(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(file(CONFIG)).ok()?;
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') {
            return None;
        }
        let (k, v) = line.split_once('=')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// The ROM file: `rom = ...` from the config (absolute, or relative to the mod folder), else the
/// first N64 ROM in the mod folder.
pub fn rom_path() -> Option<PathBuf> {
    if let Some(p) = config("rom").filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        return Some(if p.is_absolute() { p } else { mod_dir().join(p) });
    }
    let mut roms: Vec<PathBuf> = std::fs::read_dir(mod_dir())
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ["z64", "n64", "v64"].contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    roms.sort();
    roms.into_iter().next()
}

/// Reads the ROM and brings it into .z64 (big-endian) byte order. Only the US version works (the
/// data offsets libsm64 and the mod use are the US ROM's).
pub fn read_rom() -> Result<Vec<u8>, String> {
    let path = rom_path().ok_or_else(|| format!("no SM64 ROM in {} (and no rom = ... in {CONFIG})", mod_dir().display()))?;
    let mut rom = std::fs::read(&path).map_err(|e| format!("could not read ROM {}: {e}", path.display()))?;
    match rom.get(..4) {
        Some([0x80, 0x37, 0x12, 0x40]) => {}
        // .v64: byte-swapped 16-bit words
        Some([0x37, 0x80, 0x40, 0x12]) => rom.chunks_exact_mut(2).for_each(|c| c.swap(0, 1)),
        // .n64: little-endian 32-bit words
        Some([0x40, 0x12, 0x37, 0x80]) => rom.chunks_exact_mut(4).for_each(|c| c.reverse()),
        _ => return Err(format!("{} is not an N64 ROM", path.display())),
    }
    let name = String::from_utf8_lossy(&rom[0x20..0x34]).trim().to_string();
    if name != "SUPER MARIO 64" || rom.get(0x3E) != Some(&b'E') || rom.len() != 8 * 1024 * 1024 {
        return Err(format!("{} is not the US Super Mario 64 ROM ({name:?})", path.display()));
    }
    Ok(rom)
}

#[cfg(test)]
mod tests {
    use super::reforged_addons;

    #[test]
    fn finds_reforged_addons() {
        let root = std::env::temp_dir().join(format!("er_mario_paths_{}", std::process::id()));
        let offline = root.join("dll").join("offline");
        std::fs::create_dir_all(&offline).unwrap();
        // no addons folder: not Reforged
        assert_eq!(reforged_addons(&offline), None);
        std::fs::create_dir_all(root.join("addons")).unwrap();
        assert_eq!(reforged_addons(&offline), Some(root.join("addons").join("er-mario")));
        // the DLL elsewhere
        assert_eq!(reforged_addons(&root.join("dll")), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
