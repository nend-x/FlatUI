// Start menu killer — kill-on-birth monitor for StartMenuExperienceHost.exe.
//
// WHY: the low-level keyboard hook (WH_KEYBOARD_LL) approach for blocking
// the Win key is racy. The hook callback is delivered via SendMessage to
// the hook thread, and under message traffic (especially when a WebView2
// window in the same process has focus), the hook can miss the Win-down
// event. The OS then launches StartMenuExperienceHost.exe (the Start menu
// process), which renders the Start menu.
//
// Instead of trying to prevent Win-down from reaching the OS, we take the
// opposite approach: let the OS do whatever it wants, but kill
// StartMenuExperienceHost.exe the moment it spawns. This is race-free —
// no hook timing, no message pump issues. The Start menu process is
// terminated before it can render a single frame.
//
// The monitor polls every 100ms using CreateToolhelp32Snapshot. This is
// cheap (the snapshot is kernel-side, no per-process handle opens until
// we find the target). A 100ms poll means the Start menu process lives
// at most ~100ms — not enough time for its window to appear on screen.
//
// We also kill SearchHost.exe (the search flyout that can be triggered
// by Win+S or by typing from the Start menu) and ShellExperienceHost.exe
// (the shell flyout host) for good measure, since those can also be
// triggered by Win-key combos that slip past the hook.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE,
};

/// The processes to kill on sight. These are the UWP shell surfaces that
/// the Win key can trigger.
#[cfg(windows)]
const KILL_TARGETS: &[&str] = &[
    "StartMenuExperienceHost.exe",
    "SearchHost.exe",
];

/// Start the kill-on-birth monitor on a background thread. Runs forever
/// (until the process exits). Safe to call once at app startup.
#[cfg(windows)]
pub fn start() {
    std::thread::Builder::new()
        .name("start-menu-killer".into())
        .spawn(monitor_loop)
        .ok();
}

#[cfg(windows)]
fn monitor_loop() {
    log::info!("Start menu killer monitor started (targets: {:?})", KILL_TARGETS);
    loop {
        kill_targets();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(windows)]
fn kill_targets() {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(s) => s,
            Err(e) => {
                log::error!("CreateToolhelp32Snapshot failed: {e}");
                return;
            }
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_err() {
            let _ = windows::Win32::Foundation::CloseHandle(snapshot);
            return;
        }

        loop {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
            );

            if KILL_TARGETS.iter().any(|t| name.eq_ignore_ascii_case(t)) {
                // Found a target — open it and terminate it.
                let pid = entry.th32ProcessID;
                if let Ok(handle) = OpenProcess(
                    PROCESS_QUERY_INFORMATION | PROCESS_TERMINATE,
                    false,
                    pid,
                ) {
                    let _ = TerminateProcess(handle, 1);
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                    log::info!("Killed {} (pid: {})", name, pid);
                }
            }

            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }

        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }
}

#[cfg(not(windows))]
pub fn start() {}
