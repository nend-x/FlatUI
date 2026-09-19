// HideTaskbar — in-process native Windows taskbar hider.
//
// Ported from https://github.com/sinjs/HideTaskbar (HideTaskbar.cpp).
// Replaces the old embedded HideTaskbar.exe helper — no more standalone
// exe, no more extraction to %LOCALAPPDATA%, no more child process.
//
// How it works (same as the original C++):
//   1. Enumerate all top-level windows.
//   2. Find windows whose class name is "Shell_TrayWnd" (primary taskbar)
//      or "Shell_SecondaryTrayWnd" (secondary taskbars on multi-monitor).
//   3. Add the WS_EX_LAYERED extended style to each.
//   4. Set the layered window alpha to 0 (hide) or 255 (show).
//   5. Redraw the window so the change takes effect immediately.
//
// A background thread calls set_taskbars_hidden(true) every second to keep
// the taskbar hidden — Windows occasionally re-shows it (e.g. when the
// shell restarts, when a fullscreen app exits, when the user presses the
// Win key and the Start menu tries to show the taskbar). The 1s poll is
// the same interval the original C++ tool used.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use windows::core::BOOL;
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    RedrawWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW,
    GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
};

#[cfg(windows)]
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Start the background thread that keeps the taskbar hidden.
/// Safe to call once at app startup. Calling again is a no-op.
#[cfg(windows)]
pub fn start() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return; // already running
    }
    std::thread::Builder::new()
        .name("hide-taskbar".into())
        .spawn(|| {
            log::info!("HideTaskbar monitor started");
            loop {
                if !RUNNING.load(Ordering::SeqCst) {
                    break;
                }
                // Check RUNNING again right before hiding — stop() might
                // have been called between the check above and this line.
                if RUNNING.load(Ordering::SeqCst) {
                    set_taskbars_hidden(true);
                }
                // Sleep in short increments so stop() is responsive
                for _ in 0..10 {
                    if !RUNNING.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
            log::info!("HideTaskbar monitor stopped");
        })
        .ok();
}

/// Stop the background thread and restore the taskbar.
/// Waits briefly for the thread to exit, then sets alpha to 255.
#[cfg(windows)]
pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
    // Wait a moment for the thread to notice RUNNING=false and exit.
    // The thread checks every 100ms, so 200ms is enough.
    std::thread::sleep(std::time::Duration::from_millis(200));
    // Now safely show the taskbar — no race with the background thread.
    set_taskbars_hidden(false);
    log::info!("HideTaskbar stopped and taskbar restored");
}

#[cfg(windows)]
fn set_taskbars_hidden(hidden: bool) {
    let alpha: u8 = if hidden { 0 } else { 255 };
    for hwnd in find_taskbar_windows() {
        set_window_alpha(hwnd, alpha);
    }
}

#[cfg(windows)]
fn find_taskbar_windows() -> Vec<HWND> {
    let mut results = Vec::new();
    results.extend(find_windows_by_class("Shell_TrayWnd"));
    results.extend(find_windows_by_class("Shell_SecondaryTrayWnd"));
    results
}

// Thread-local storage for the current enumeration results.
#[cfg(windows)]
thread_local! {
    static CURRENT_RESULTS: std::cell::RefCell<Vec<HWND>> = std::cell::RefCell::new(Vec::new());
    static CURRENT_CLASS: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
}

#[cfg(windows)]
unsafe extern "system" fn enum_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut class_buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    if len > 0 {
        let found_class = String::from_utf16_lossy(&class_buf[..len as usize]);
        CURRENT_CLASS.with(|cc| {
            if found_class == *cc.borrow() {
                CURRENT_RESULTS.with(|cr| {
                    cr.borrow_mut().push(hwnd);
                });
            }
        });
    }
    BOOL(1)
}

#[cfg(windows)]
fn find_windows_by_class(class_name: &str) -> Vec<HWND> {
    CURRENT_CLASS.with(|cc| {
        *cc.borrow_mut() = class_name.to_string();
    });
    CURRENT_RESULTS.with(|cr| {
        cr.borrow_mut().clear();
    });

    // EnumWindows takes Option<unsafe extern "system" fn(HWND, LPARAM) -> BOOL>
    let _ = unsafe { EnumWindows(Some(enum_proc), LPARAM(0)) };

    CURRENT_RESULTS.with(|cr| cr.borrow().clone())
}

#[cfg(windows)]
fn set_window_alpha(hwnd: HWND, alpha: u8) {
    unsafe {
        // Add WS_EX_LAYERED if not already present
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_LAYERED.0 == 0 {
            SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_LAYERED.0) as i32);
        }
        // Set alpha
        let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), alpha, LWA_ALPHA);
        // Force a redraw
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN,
        );
    }
}

#[cfg(not(windows))]
pub fn start() {}
#[cfg(not(windows))]
pub fn stop() {}
