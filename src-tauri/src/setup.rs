// Setup — launches the embedded helper executables as child processes.
//
// Both helpers are EMBEDDED in the main binary (embedded.rs) and extracted
// to %LOCALAPPDATA%\FlatUI\bin — no external files required. Fallback: a
// copy sitting next to the main exe (or in resources/) is used if extraction
// ever fails (e.g. dev layouts, unwritable dirs).
//
//   - HideTaskbar.exe — hides the native Windows taskbar.
//   - flatwin.exe     — AutoHotkey v2 helper: intercepts the Win key and
//                       POSTs to http://127.0.0.1:2290/toggle, which the HTTP
//                       server (http_server.rs) turns into a launcher toggle.
//                       It ALSO gives the "minimize all windows" desktop
//                       effect when the launcher opens (handled app-side in
//                       toggle_launcher_impl).
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

/// Kill any flatwin.exe left over from a previous session so exactly ONE
/// instance exists before we spawn the freshly extracted copy. Two running
/// instances would send two POSTs per Win press and toggle the launcher
/// twice (net no-op).
#[cfg(windows)]
fn kill_stale_flatwin() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    match Command::new("taskkill")
        .args(["/f", "/im", "flatwin.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(out) => {
            log::info!("stale flatwin.exe cleanup: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        Err(e) => log::warn!("stale flatwin.exe cleanup failed: {e}"),
    }
    // taskkill returns immediately; give the old process a moment to die so
    // the file lock on the exe is released before we extract over it.
    std::thread::sleep(std::time::Duration::from_millis(300));
}

#[cfg(not(windows))]
fn kill_stale_flatwin() {}

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

    // Exactly one flatwin instance must run — clean up leftovers first.
    kill_stale_flatwin();

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

    // Step 2: flatwin.exe (AHK Win-key handler -> HTTP /toggle)
    let step_ok = find("flatwin.exe")
        .map(|path| spawn_helper("flatwin.exe", &path))
        .unwrap_or_else(|| {
            log::warn!("flatwin.exe not found (extraction failed and no fallback present)");
            false
        });
    steps.push(("Registering Win key handler...".to_string(), step_ok));

    steps
}
