// Brightness dimmer — a pure Win32 software dimming overlay.
//
// "Systemless" by design: nothing on the system is modified — no WMI
// monitor brightness, no DDC/CI, no registry, no power plan. The dim is
// literally a black, topmost, fully click-through layered window sitting
// ON TOP of the display; its layered alpha is the dim strength. Closing
// Hush_UI (or sliding the widget back to 100%) removes it instantly, and
// the system is exactly as it was.
//
// Layer composition:
//   WS_EX_LAYERED            — per-window alpha via SetLayeredWindowAttributes
//   WS_EX_TRANSPARENT        — hit-testing passes through (fully click-through)
//   WS_EX_NOACTIVATE         — can never steal focus
//   WS_EX_TOOLWINDOW         — never appears in alt-tab / task lists
//   WS_EX_TOPMOST            — floats above normal windows (re-asserted by a
//                              timer so it stays above other topmost windows)
//
// The dim strength is clamped so the overlay can never go fully opaque —
// the brightness widget itself always stays reachable.
//
// NOTE: without elevation (UAC declined at launch), Windows' UIPI can keep
// this (medium-IL) overlay BELOW windows of elevated processes — the
// brightness widget surfaces a warning about that when the process is not
// elevated.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[cfg(windows)]
const MAX_DIM_ALPHA: u8 = 220; // ~86% dim ceiling — never fully opaque
#[cfg(windows)]
const REASSERT_TIMER_MS: usize = 1500;

/// Dim level, stored ×1000 (0 = no dim, 1000 = max dim). Read by the
/// overlay thread on creation and by `set_level`.
#[cfg(windows)]
static LEVEL_MILLI: AtomicU64 = AtomicU64::new(0);

/// HWND of the overlay window once created (0 until then).
#[cfg(windows)]
static OVERLAY_HWND: AtomicUsize = AtomicUsize::new(0);

/// Spawn the overlay thread. Safe to call once at startup.
#[cfg(windows)]
pub fn start() {
    std::thread::Builder::new()
        .name("brightness-dimmer".into())
        .spawn(|| {
            if let Err(e) = run_overlay() {
                log::error!("dimmer: overlay thread failed: {e}");
            }
        })
        .ok();
}

/// Apply a dim level (0.0 = off … 1.0 = max dim). Called by the
/// `set_dimmer_level` command and at startup from persisted settings.
#[cfg(windows)]
pub fn set_level(level: f64) {
    let level = level.clamp(0.0, 1.0);
    LEVEL_MILLI.store((level * 1000.0).round() as u64, Ordering::SeqCst);
    let hwnd_val = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd_val == 0 {
        // Overlay not up yet — the overlay thread applies LEVEL_MILLI on
        // creation, nothing else to do here.
        return;
    }
    apply(hwnd_val);
}

#[cfg(windows)]
fn apply(hwnd_val: usize) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetLayeredWindowAttributes, ShowWindow, LWA_ALPHA, SW_HIDE, SW_SHOWNOACTIVATE,
    };

    let level = LEVEL_MILLI.load(Ordering::SeqCst) as f64 / 1000.0;
    let hwnd = windows::Win32::Foundation::HWND(hwnd_val as *mut core::ffi::c_void);
    unsafe {
        if level <= 0.001 {
            let _ = ShowWindow(hwnd, SW_HIDE);
            return;
        }
        let alpha = ((level * MAX_DIM_ALPHA as f64) as u32).clamp(1, MAX_DIM_ALPHA as u32) as u8;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        reassert(hwnd);
    }
}

/// Re-assert topmost + virtual-screen coverage (resolution changes, other
/// topmost windows appearing above us).
#[cfg(windows)]
fn reassert(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, HWND_TOPMOST, SWP_NOACTIVATE,
    };
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x, y, w, h,
            SWP_NOACTIVATE,
        );
    }
}

#[cfg(windows)]
fn run_overlay() -> windows::core::Result<()> {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH, HBRUSH};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW, DefWindowProcW,
        SetTimer, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG,
        WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_EX_LAYERED, WS_POPUP, CW_USEDEFAULT,
    };
    use windows::core::w;

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_TIMER {
            reassert(hwnd);
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class_name = w!("Hush_UIDimmer");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            ..Default::default()
        };
        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(windows::core::Error::from_win32());
        }

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name,
            w!(""),
            WS_POPUP,
            CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;

        OVERLAY_HWND.store(hwnd.0 as usize, Ordering::SeqCst);
        SetTimer(Some(hwnd), 1, REASSERT_TIMER_MS as u32, None);

        // Apply whatever dim level was requested before/while we were coming up.
        apply(hwnd.0 as usize);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Ok(())
    }
}
