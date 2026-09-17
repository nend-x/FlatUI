#![cfg(windows)]
// Pop-out restore animation — "window-to-taskbar zoom".
//
// When the user clicks a taskbar icon for a MINIMIZED window, the window
// doesn't just blink back into existence — it visibly grows out of the
// taskbar button it was minimized to:
//
//   1. cloak the window (DWMWA_CLOAK) so we can restore it invisibly
//   2. SW_RESTORE it while cloaked  ->  GetWindowRect = exact final bounds
//   3. teleport it (still cloaked) to the taskbar button's screen rect
//   4. uncloak + hand over foreground
//   5. animate 12 frames @ ~16ms, ease-out cubic — fast start, silky
//      deceleration into the final rect, ZERO overshoot (Fluent curve)
//   6. exact landing, re-maximize if the window was zoomed
//
// Runs on a worker thread so the invoke command returns instantly. A
// per-hwnd guard set prevents stacking a second animation onto the same
// window. If DWM cloaking is unavailable we fall back to the legacy
// instant restore (no animation, no visual regression).
//
// Safety: a CloakGuard uncloaks the window on every exit path — if we
// abort mid-sequence the window can never be left permanently invisible.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, GetWindowRect, IsIconic, IsWindow, IsZoomed, SetForegroundWindow,
    SetWindowPos, ShowWindowAsync, HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER, SW_MAXIMIZE,
    SW_RESTORE,
};

/// Physical-screen rect of the taskbar button a window should pop out of.
#[derive(Debug, Clone, Copy)]
pub struct OriginRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

const FRAME_MS: u64 = 16;
const FRAMES: u32 = 12;
/// How long we wait for an async SW_RESTORE to take effect before bailing.
const RESTORE_TIMEOUT_MS: u64 = 250;

static ANIMATING: Lazy<Mutex<HashSet<isize>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Try to restore a minimized window with the pop-out animation.
/// Returns true if an animation was started, false if the caller should
/// perform the legacy instant restore instead.
pub fn restore_animated(hwnd_val: isize, origin: OriginRect) -> bool {
    {
        let mut set = ANIMATING.lock();
        if set.contains(&hwnd_val) {
            return false; // already animating — never stack a second pass
        }
        set.insert(hwnd_val);
    }

    let spawned = thread::Builder::new()
        .name("popout-anim".into())
        .spawn(move || {
            run(hwnd_val, origin);
            ANIMATING.lock().remove(&hwnd_val);
        });

    match spawned {
        Ok(_) => true,
        Err(_) => {
            ANIMATING.lock().remove(&hwnd_val);
            false
        }
    }
}

struct CloakGuard(HWND);
impl Drop for CloakGuard {
    fn drop(&mut self) {
        // Idempotent: uncloaking an already-uncloaked window is a no-op.
        let _ = set_cloak(self.0, false);
    }
}

fn set_cloak(hwnd: HWND, on: bool) -> bool {
    let val: i32 = if on { 1 } else { 0 };
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &val as *const i32 as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        )
        .is_ok()
    }
}

fn run(hwnd_val: isize, origin: OriginRect) {
    let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return; // vanished between click and thread start
        }
        if !IsIconic(hwnd).as_bool() {
            // Restored by someone else meanwhile — just take focus.
            let _ = SetForegroundWindow(hwnd);
            return;
        }

        let _ = AllowSetForegroundWindow(0xFFFFFFFF);

        // 1. Cloak so the restore happens invisibly.
        if !set_cloak(hwnd, true) {
            // Legacy fallback: no DWM cloak available — plain restore.
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
            return;
        }
        let _guard = CloakGuard(hwnd);

        // 2. Restore invisibly; wait for the async show to settle.
        let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        let deadline = Instant::now() + Duration::from_millis(RESTORE_TIMEOUT_MS);
        let mut settled = false;
        while Instant::now() < deadline {
            if !IsIconic(hwnd).as_bool() {
                settled = true;
                break;
            }
            thread::sleep(Duration::from_millis(4));
        }
        if !settled || !IsWindow(Some(hwnd)).as_bool() {
            return; // hung window — guard will uncloak on the way out
        }

        // 3. Capture the exact final bounds + zoomed state while still hidden.
        let was_maximized = IsZoomed(hwnd).as_bool();
        let mut final_rect = windows::Win32::Foundation::RECT::default();
        if GetWindowRect(hwnd, &mut final_rect).is_err() {
            return;
        }
        let fw = (final_rect.right - final_rect.left).max(1);
        let fh = (final_rect.bottom - final_rect.top).max(1);

        // 4. Teleport onto the taskbar button (still cloaked).
        let ow = origin.w.max(1);
        let oh = origin.h.max(1);
        let _ = SetWindowPos(hwnd, Some(HWND_TOP), origin.x, origin.y, ow, oh, SWP_NOACTIVATE | SWP_NOZORDER);

        // 5. Reveal at the button + hand over foreground immediately so the
        //    swap feels instant, then grow the window.
        let _ = set_cloak(hwnd, false);
        let _ = SetForegroundWindow(hwnd);

        // 6. Ease-out cubic: fast launch, smooth deceleration, no bounce.
        for f in 1..=FRAMES {
            if !IsWindow(Some(hwnd)).as_bool() {
                return;
            }
            let t = f as f32 / FRAMES as f32;
            let e = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
            let x = origin.x as f32 + (final_rect.left as f32 - origin.x as f32) * e;
            let y = origin.y as f32 + (final_rect.top as f32 - origin.y as f32) * e;
            let w = ow as f32 + (fw as f32 - ow as f32) * e;
            let h = oh as f32 + (fh as f32 - oh as f32) * e;
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                x.round() as i32,
                y.round() as i32,
                w.round() as i32,
                h.round() as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            thread::sleep(Duration::from_millis(FRAME_MS));
        }

        // 7. Exact landing + restore proper maximized state.
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                final_rect.left,
                final_rect.top,
                fw,
                fh,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            let _ = SetForegroundWindow(hwnd);
            if was_maximized {
                let _ = ShowWindowAsync(hwnd, SW_MAXIMIZE);
            }
        }
    }
}
