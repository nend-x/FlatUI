// Setup — launches the embedded helper executables as child processes.
//
// Helpers are EMBEDDED in the main binary (embedded.rs) and extracted to
// %LOCALAPPDATA%\FlatUI\bin — no external files required. Fallback: a copy
// sitting next to the main exe (or in resources/) is used if extraction
// ever fails (e.g. dev layouts, unwritable dirs).
//
//   - HideTaskbar.exe — hides the native Windows taskbar.
//
// Win-key handling no longer lives here: it is provided in-process by the
// `prevent-alt-win-menu` crate, which is wired up in lib.rs. That replaces
// the old `flatwin.exe` AutoHotkey v2 helper (which used to POST to a local
// HTTP server — also removed).
//
// lib.rs used to call run_setup() twice (once before the Tauri builder and
// once inside the setup thread), double-spawning the helper. SETUP_ONCE now
// guarantees it is only ever spawned a single time.

use std::path::PathBuf;
use std::process::Command;

use once_cell::sync::OnceCell;

use crate::embedded;

static SETUP_ONCE: OnceCell<()> = OnceCell::new();

/// Resolve a helper exe: embedded extraction first, then fallback locations.
fn find_helper(name: &str, extracted: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = extracted {
        if p.exists() {
            return Some(p);
        }
        log::warn!("embedded {name} missing on disk after extraction");
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    [exe_dir.join(name), exe_dir.join("resources").join(name)]
        .into_iter()
        .find(|p| p.exists())
}

fn spawn_helper(name: &str, path: &std::path::Path) -> bool {
    match Command::new(path).spawn() {
        Ok(child) => {
            log::info!("{name} launched as child (PID: {})", child.id());
            true
        }
        Err(e) => {
            log::error!("Failed to launch {name}: {}", e);
            false
        }
    }
}

/// Launches the helper child processes. Returns (step_name, success) pairs.
/// Safe to call multiple times — only the first call does anything.
pub fn run_setup() -> Vec<(String, bool)> {
    if SETUP_ONCE.get().is_some() {
        return Vec::new();
    }
    SETUP_ONCE.set(()).ok();

    let mut steps = Vec::new();

    let extracted = embedded::ensure_helpers();
    let find = |name: &str| {
        let hit = extracted.iter().find(|p| p.file_name().map(|f| f == name).unwrap_or(false));
        find_helper(name, hit.cloned())
    };

    // Step 1: HideTaskbar.exe
    let step_ok = find("HideTaskbar.exe")
        .map(|path| spawn_helper("HideTaskbar.exe", &path))
        .unwrap_or_else(|| {
            log::warn!("HideTaskbar.exe not found (extraction failed and no fallback present)");
            false
        });
    steps.push(("Hiding taskbar...".to_string(), step_ok));

    steps
}
