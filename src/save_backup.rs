//! A copy of Mario's save on every launch, before the game reads it: the last few are kept in
//! %APPDATA%\EldenRing\<steam id>\ER Mario backups\<date time>\, so a save that breaks can be
//! put back by hand. Only Mario's save (the me3 profile's savefile); the normal one isn't touched.

use std::path::{Path, PathBuf};

use crate::log;

const SAVE: &str = "ER0000_mario.sl2";
const FOLDER: &str = "ER Mario backups";
const KEEP: usize = 5;

pub fn run() {
    let Some(appdata) = std::env::var_os("APPDATA") else { return };
    let Ok(ids) = std::fs::read_dir(PathBuf::from(appdata).join("EldenRing")) else { return };
    for dir in ids.filter_map(|e| e.ok().map(|e| e.path())) {
        if dir.join(SAVE).is_file() {
            match backup(&dir) {
                Ok(Some(to)) => log(format!("save backup: {}", to.display())),
                Ok(None) => {}
                Err(e) => log(format!("save backup failed in {}: {e}", dir.display())),
            }
        }
    }
}

/// Copies the save (and the game's own .bak) into a new dated folder, unless it's the same as the
/// newest backup, then drops the oldest past KEEP.
fn backup(dir: &Path) -> std::io::Result<Option<PathBuf>> {
    let root = dir.join(FOLDER);
    std::fs::create_dir_all(&root)?;
    let mut old: Vec<PathBuf> = std::fs::read_dir(&root)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    old.sort();
    let save = std::fs::read(dir.join(SAVE))?;
    if old.last().is_some_and(|last| std::fs::read(last.join(SAVE)).is_ok_and(|prev| prev == save)) {
        return Ok(None);
    }
    let to = root.join(stamp());
    std::fs::create_dir_all(&to)?;
    std::fs::write(to.join(SAVE), &save)?;
    let bak = format!("{SAVE}.bak");
    if dir.join(&bak).is_file() {
        std::fs::copy(dir.join(&bak), to.join(&bak))?;
    }
    old.push(to.clone());
    for gone in &old[..old.len().saturating_sub(KEEP)] {
        let _ = std::fs::remove_dir_all(gone);
    }
    Ok(Some(to))
}

/// Local date and time, sortable: 2026-10-16_1914-05.
fn stamp() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}_{:02}{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}
