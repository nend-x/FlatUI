#![cfg(windows)]
// Window helpers — topmost + WS_EX_NOACTIVATE + WS_EX_TOOLWINDOW + AppBar

use tauri::WebviewWindow;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_FRAMECHANGED, SWP_SHOWWINDOW, GWL_EXSTYLE, GWL_STYLE,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
    EnumWindows, IsWindowVisible, IsIconic, GetClassNameW, ShowWindowAsync, SW_MINIMIZE,
    GetWindowThreadProcessId, SetLayeredWindowAttributes, LWA_ALPHA,
};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::GWL_HWNDPARENT;

fn hwnd_of(window: &WebviewWindow) -> HWND {
    window.hwnd().expect("hwnd() failed")
}

// ===== Pie picker overlay show/hide via layered-window alpha =====
//
// The picker is a fullscreen transparent WebView2. Hiding/showing it with
// ShowWindow makes WebView2's composition surface tear down/re-attach and
// flash its default background for a frame — the intermittent light-blue
// flicker on open. Instead the window is created visible ONCE and is
// thereafter toggled with:
//   hidden  → WS_EX_LAYERED + WS_EX_TRANSPARENT (click-through), alpha 0
//   shown   → WS_EX_LAYERED only, alpha 255
// The surface is never destroyed or shown mid-paint, so there is nothing
// left to flash.
pub fn set_picker_visible(window: &WebviewWindow, visible: bool) -> windows::core::Result<()> {
    let hwnd = hwnd_of(window);
    unsafe {
        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        ex |= WS_EX_LAYERED.0 as isize;
        if visible {
            ex &= !(WS_EX_TRANSPARENT.0 as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
            let _ = SetWindowPos(
                hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        } else {
            ex |= WS_EX_TRANSPARENT.0 as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
        }
    }
    Ok(())
}

pub fn apply_no_activate(window: &WebviewWindow) -> windows::core::Result<()> {
    let hwnd = hwnd_of(window);
    unsafe {
        // === DWM attributes — fully remove the caption strip ===
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute,
            DWMWA_NCRENDERING_POLICY, DWMNCRENDERINGPOLICY, DWMNCRP_DISABLED,
            DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_BORDER_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
        };

        // 1. Disable DWM non-client rendering
        let policy = DWMNCRENDERINGPOLICY(DWMNCRP_DISABLED.0);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const _ as *const _,
            std::mem::size_of::<DWMNCRENDERINGPOLICY>() as u32,
        );

        // 2. Set caption color to espresso dark (BGR format: 0x00BBGGRR)
        let dark_colorref: u32 = 0x001A2A3A; // B=26, G=42, R=58
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &dark_colorref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // 3. Caption text color
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR,
            &dark_colorref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // 4. Border color
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &dark_colorref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // 5. Immersive dark mode
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        // === Window styles ===
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex = ex
            | (WS_EX_NOACTIVATE.0 as isize)
            | (WS_EX_TOOLWINDOW.0 as isize)
            | (WS_EX_TOPMOST.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);

        // Strip WS_OVERLAPPEDWINDOW (caption + sysmenu + minmax + thickframe) and
        // replace with WS_POPUP. This removes the Windows-drawn caption buttons.
        //
        // WS_VISIBLE is NOT force-set here: this helper runs at startup on
        // windows that are still hidden (the tables overlays) — OR-ing
        // WS_VISIBLE into the style made a hidden fullscreen overlay window
        // VISIBLE-but-transparent, which silently swallowed every mouse
        // click on the desktop until the user opened + closed the quick
        // menu (the only path that called hide() for real).
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let mut new_style = (style & !(WS_OVERLAPPEDWINDOW.0 as isize)) | (WS_POPUP.0 as isize);
        if IsWindowVisible(hwnd).as_bool() {
            new_style |= WS_VISIBLE.0 as isize;
        }
        SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);

        // === Set window region to client area only ===
        // This physically clips away the non-client area (caption bar) so
        // Windows cannot draw it even if it tries.
        use windows::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn};
        use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
        use windows::Win32::Foundation::RECT;
        let mut rc = RECT::default();
        if GetClientRect(hwnd, &mut rc).is_ok() {
            let rgn = CreateRectRgn(rc.left, rc.top, rc.right, rc.bottom);
            let _ = SetWindowRgn(hwnd, Some(rgn), true);
        }

        // === Install subclass to intercept WM_NCACTIVATE / WM_NCPAINT ===
        // This is the nuclear option — prevents Windows from drawing the caption
        // bar even when the window receives focus via click on empty areas.
        let _ = crate::win32::subclass::install_caption_suppression(hwnd);

        // Force topmost refresh with frame change.
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0, 0, 0, 0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        );
    }
    Ok(())
}

// ===== Minimize all windows (show-desktop effect for the launcher) =====

/// Minimize every visible top-level application window — the same effect as
/// Win+D. Called every time the launcher opens so it sits on a clean desktop
/// instead of stacking on top of other windows.
///
/// Skips:
///   - invisible / already-minimized windows
///   - DWM-cloaked windows (invisible UWP ghosts on Win10/11)
///   - tool windows (floating toolbars, tooltips, helper palettes)
///   - owned windows (dialogs minimize with their owner)
///   - FlatUI's own windows (taskbar, launcher, setup)
///   - the shell desktop (Progman / WorkerW / tray windows)
///
/// Uses ShowWindowAsync so a hung application can never stall the launcher.
pub fn minimize_all_windows() {
    let mut minimized: u32 = 0;
    unsafe {
        let lparam = LPARAM(&mut minimized as *mut u32 as isize);
        if let Err(e) = EnumWindows(Some(minimize_proc), lparam) {
            log::warn!("minimize_all_windows: EnumWindows failed — {e}");
            return;
        }
    }
    if minimized > 0 {
        log::info!("minimize_all_windows: minimized {minimized} window(s)");
    }
}

unsafe extern "system" fn minimize_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let counter = unsafe { &mut *(lparam.0 as *mut u32) };

    // Invisible windows.
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    // Already minimized.
    if IsIconic(hwnd).as_bool() {
        return BOOL(1);
    }
    // DWM-cloaked (virtual, hidden) windows — UWP apps suspended in background.
    let mut cloaked: u32 = 0;
    let hr = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as *mut core::ffi::c_void,
        std::mem::size_of::<u32>() as u32,
    );
    if hr.is_ok() && cloaked != 0 {
        return BOOL(1);
    }
    // Tool windows (palettes, tooltips, helper overlays).
    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TOOLWINDOW.0 != 0 {
        return BOOL(1);
    }
    // Owned windows (dialogs) — their owner gets minimized with them.
    if GetWindowLongPtrW(hwnd, GWL_HWNDPARENT) != 0 {
        return BOOL(1);
    }
    // FlatUI's own windows (taskbar / launcher / setup).
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == GetCurrentProcessId() {
        return BOOL(1);
    }
    // Shell desktop windows.
    let mut buf = [0u16; 64];
    let n = GetClassNameW(hwnd, &mut buf);
    if n > 0 {
        let class = String::from_utf16_lossy(&buf[..n as usize]);
        match class.as_str() {
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" => {
                return BOOL(1);
            }
            _ => {}
        }
    }

    let _ = ShowWindowAsync(hwnd, SW_MINIMIZE);
    *counter += 1;
    BOOL(1)
}
