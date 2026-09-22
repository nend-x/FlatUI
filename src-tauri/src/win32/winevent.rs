#![cfg(windows)]
// WinEvent hook — fires when the foreground window changes.

use windows::core::*;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{
    SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
};

// WinEvent constants (not exported by windows-rs 0.61)
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;

struct SyncHook(HWINEVENTHOOK);
unsafe impl Send for SyncHook {}
unsafe impl Sync for SyncHook {}

static HOOK: std::sync::OnceLock<SyncHook> = std::sync::OnceLock::new();
static ON_FOREGROUND_CHANGE: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

pub fn install_foreground_hook(callback: Box<dyn Fn() + Send + Sync>) -> windows::core::Result<()> {
    ON_FOREGROUND_CHANGE.set(callback).map_err(|_| {
        windows::core::Error::new(HRESULT(-1), "callback already set")
    })?;

    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(event_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        if hook.is_invalid() {
            return Err(windows::core::Error::new(HRESULT(-1), "SetWinEventHook failed"));
        }
        if HOOK.set(SyncHook(hook)).is_err() {
            let _ = UnhookWinEvent(hook);
            return Err(windows::core::Error::new(HRESULT(-1), "HOOK already set"));
        }
    }

    // Message pump thread for WINEVENT_OUTOFCONTEXT
    std::thread::spawn(|| {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetMessageW, TranslateMessage, DispatchMessageW, MSG,
        };
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });

    Ok(())
}

unsafe extern "system" fn event_callback(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if let Some(cb) = ON_FOREGROUND_CHANGE.get() {
        cb();
    }
}
