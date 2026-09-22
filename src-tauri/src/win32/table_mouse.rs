// Outside-click detection for the taskbar table strip.
//
// While the vertical taskbar strip is open, a temporary WH_MOUSE_LL hook
// watches every click: a click that lands OUTSIDE the strip window's rect
// (left / right / middle button) fires the close callback WITHOUT
// swallowing the click — the app under the cursor receives it normally and
// the strip hides itself.
//
// Threading: low-level mouse hooks deliver callbacks to the thread that
// installed them, which MUST pump messages. Each start() spawns a dedicated
// hook thread with a GetMessageW pump that stays alive until stop()
// posts WM_QUIT to it (same pattern as hotkey.rs). Clicks inside the strip
// and synthetic input are ignored.

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, GetWindowRect, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, LLMHF_INJECTED,
    MSLLHOOKSTRUCT, MSG, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN,
};

type CloseFn = Arc<dyn Fn() + Send + Sync + 'static>;

static CLOSE_FN: OnceCell<CloseFn> = OnceCell::new();
/// (hook thread id, strip hwnd). The hwnd is what the hook thread reads;
/// the thread id is what stop() posts WM_QUIT to.
static ACTIVE: Mutex<Option<(u32, isize)>> = Mutex::new(None);
/// Scratch copy of the strip hwnd read directly by the hook callback.
static STRIP_HWND: AtomicIsize = AtomicIsize::new(0);

/// Start watching for outside clicks. `hwnd` is the strip window (physical
/// rect via GetWindowRect), `on_outside` fires (on a worker thread) for any
/// real click landing outside it. Starting twice stops the previous watch
/// first — cheap idempotency for rapid open/open sequences.
pub fn start(hwnd: isize, on_outside: CloseFn) {
    stop();
    let _ = CLOSE_FN.set(on_outside);
    STRIP_HWND.store(hwnd, Ordering::SeqCst);

    let hmod_isize = unsafe {
        match GetModuleHandleW(None) {
            Ok(h) => h.0 as isize,
            Err(_) => 0isize,
        }
    };

    std::thread::Builder::new()
        .name("strip-outside-click".into())
        .spawn(move || unsafe {
            let tid = GetCurrentThreadId();
            // Register this thread BEFORE installing the hook so a stop()
            // racing startup can always post WM_QUIT at it.
            *ACTIVE.lock() = Some((tid, hwnd));
            // HINSTANCE isn't Send (raw pointer inside) — reconstruct it
            // in-thread from the plain isize captured above.
            let hmod = HINSTANCE(hmod_isize as *mut core::ffi::c_void);
            let hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse_proc), Some(hmod), 0)
            {
                Ok(h) => h,
                Err(e) => {
                    log::warn!("strip outside-click hook install failed: {e}");
                    *ACTIVE.lock() = None;
                    return;
                }
            };
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(hook);
            *ACTIVE.lock() = None;
            STRIP_HWND.store(0, Ordering::SeqCst);
        })
        .ok();
}

/// Stop watching. Posts WM_QUIT to the hook thread; the thread unhooks
/// itself. Safe to call when not watching.
pub fn stop() {
    if let Some((tid, _)) = ACTIVE.lock().take() {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        STRIP_HWND.store(0, Ordering::SeqCst);
    }
}

unsafe extern "system" fn ll_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code == HC_ACTION as i32 {
        let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
        let injected = (info.flags & LLMHF_INJECTED) != 0;
        let msg = w_param.0 as u32;
        let is_click = msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN;

        // Synthetic input (our own SendInput calls, remote desktop injection)
        // never closes the strip; only real button-downs participate.
        if !injected && is_click {
            let hwnd_val = STRIP_HWND.load(Ordering::SeqCst);
            if hwnd_val != 0 {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_val as *mut core::ffi::c_void);
                let mut rect = RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_ok() {
                    let p = POINT {
                        x: info.pt.x,
                        y: info.pt.y,
                    };
                    let inside = p.x >= rect.left
                        && p.x < rect.right
                        && p.y >= rect.top
                        && p.y < rect.bottom;
                    if !inside {
                        if let Some(f) = CLOSE_FN.get() {
                            let f = f.clone();
                            std::thread::spawn(move || f());
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}
