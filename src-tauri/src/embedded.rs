// Embedded helper executables.
//
// Helpers are compiled INTO the main binary via include_bytes! and extracted
// on demand to %LOCALAPPDATA%\FlatUI\bin:
//
//   - HideTaskbar.exe — small MSVC tool that hides the native Windows taskbar.
//
// The Win-key handling used to live in a sibling `flatwin.exe` AutoHotkey v2
// helper, but that has been replaced by the `prevent-alt-win-menu` crate,
// which installs its own low-level keyboard hook in-process. No external
// Win-key helper is needed anymore.
//
// This keeps the installed app self-contained: the helper no longer needs to
// sit next to the main executable (tauri.conf.json no longer bundles it as a
// resource). The extraction dir is the same FlatUI data dir persist.rs uses.
//
// Notes:
//   - Extraction overwrites on every start (self-heals across app updates).
//   - If the helper is currently running, Windows locks the file; the write
//     fails harmlessly and the existing copy is reused.

use std::path::PathBuf;

/// Small MSVC C++ tool that hides the native Windows taskbar.
pub static HIDE_TASKBAR_EXE: &[u8] = include_bytes!("../resources/HideTaskbar.exe");

/// Directory the helpers are extracted to: %LOCALAPPDATA%\FlatUI\bin
pub fn bin_dir() -> PathBuf {
    if let Some(lad) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(lad).join("FlatUI").join("bin");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }
    // Fallback (non-Windows dev environment): keep out of the project dir.
    let dir = std::env::temp_dir().join("FlatUI").join("bin");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Write one embedded helper to disk.
fn extract(name: &str, bytes: &[u8]) -> Option<PathBuf> {
    let path = bin_dir().join(name);
    match std::fs::write(&path, bytes) {
        Ok(()) => Some(path),
        Err(e) => {
            if path.exists() {
                // Almost certainly locked because the helper is still running.
                log::warn!("extract {name}: file in use ({e}) — reusing existing copy");
                Some(path)
            } else {
                log::error!("extract {name}: failed — {e}");
                None
            }
        }
    }
}

/// Ensure all helpers are on disk. Returns the successfully extracted paths.
pub fn ensure_helpers() -> Vec<PathBuf> {
    [extract("HideTaskbar.exe", HIDE_TASKBAR_EXE)]
        .into_iter()
        .flatten()
        .collect()
}
