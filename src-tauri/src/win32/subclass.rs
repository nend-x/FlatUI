#![cfg(windows)]
// Window subclass — intercept WM_NCACTIVATE and WM_NCPAINT to prevent
// Windows from drawing the caption bar even when the window receives focus.

use windows::core::*;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{SetWindowSubclass, DefSubclassProc};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_NCACTIVATE, WM_NCPAINT, WM_NCHITTEST, HTCLIENT,
};

/// ID for our subclass procedure (must be unique per process)
const SUBCLASS_ID: usize = 0xC5EA;

/// Install the subclass on a window. After this, WM_NCACTIVATE returns TRUE
/// (preventing caption redraw) and WM_NCPAINT returns 0 (preventing caption paint).
pub fn install_caption_suppression(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        // SetWindowSubclass returns BOOL (not Result) — convert
        let ok = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
        if !ok.as_bool() {
            return Err(windows::core::Error::new(
                HRESULT(-1),
                "SetWindowSubclass returned FALSE",
            ));
        }
    }
    Ok(())
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id_subclass: usize,
    _ref_data: usize,
) -> LRESULT {
    match msg {
        // WM_NCACTIVATE: when Windows wants to redraw the caption to show
        // active/inactive state. Return TRUE to prevent the redraw.
        WM_NCACTIVATE => {
            return LRESULT(1);
        }
        // WM_NCPAINT: when Windows wants to paint the non-client area (caption bar).
        // Return 0 to skip painting entirely.
        WM_NCPAINT => {
            return LRESULT(0);
        }
        // WM_NCHITTEST: tell Windows that all hits are in the client area,
        // so it doesn't try to activate the caption on hover.
        WM_NCHITTEST => {
            return LRESULT(HTCLIENT as isize);
        }
        _ => {}
    }
    // Pass through to default handler for all other messages
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}
