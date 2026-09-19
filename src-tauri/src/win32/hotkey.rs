// Win-key tap-vs-combo interception.
//
// Behavior (Lightshot/Spotlight-style):
//   - Win pressed and released ALONE (a "tap")  -> toggle the FlatUI launcher
//   - Win held with ANY other key (Win+D, Win+E, Win+Tab, ...) -> the combo
//     works exactly like on stock Windows.
//
// How it works:
//   A low-level keyboard hook (WH_KEYBOARD_LL) SWALLOWS the native Win-down
//   and remembers it is "pending". The moment any other key is pressed while
//   Win is pending, the tap is cancelled and the suppressed Win-down is
//   re-injected (marked injected so this hook ignores it) — from then on the
//   rest of the combo flows through Windows untouched. If Win is RELEASED
//   while still pending, it was a tap and we fire the launcher toggle.
//
// Swallowing Win-down (vs the `prevent-alt-win-menu` crate's approach of
// injecting a dummy key-up AFTER Win-up) is what makes the Start menu
// literally unable to open — the OS shell never sees Win-down, so it can't
// start its "Win chord" detection. The dummy-key-up approach is racy under
// focus changes (when the launcher webview has focus, the shell processed
// Win-up before the synthetic dummy-up landed, so the Start menu opened and
// the close tap appeared to do nothing). This hook fixes both symptoms.
//
// We install this hook LAST so it is called FIRST in the LIFO hook chain —
// it swallows Win events before `prevent-alt-win-menu`'s hook (which is
// installed first) ever sees them. `prevent-alt-win-menu` is configured to
// return None for Win in its on_released callback (we handle Win here), and
// only handles Alt (menu bar suppression on Alt release).
//
// The hook runs on its own thread with a standard message pump (required for
// low-level hooks). The toggle callback is invoked from a short-lived worker
// thread so the hook never blocks input processing.

use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::OnceCell;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_LMENU, VK_LWIN, VK_MENU, VK_RMENU, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    UnhookWindowsHookEx,
};

type ToggleFn = Box<dyn Fn() + Send + Sync>;

static TOGGLER: OnceCell<ToggleFn> = OnceCell::new();
static WIN_PENDING: AtomicBool = AtomicBool::new(false);

// Alt menu suppression state — replaces the prevent-alt-win-menu crate.
// ALT_PENDING is set on Alt-down and cleared when any OTHER key is pressed
// (meaning it was an Alt+X combo, not a standalone Alt tap). On Alt-up, if
// ALT_PENDING is still true, we inject a dummy VK__none_ key-up which
// prevents the focused window's menu bar from activating.
static ALT_PENDING: AtomicBool = AtomicBool::new(false);

/// Tracks whether the hook thread is alive. The health-check timer
/// monitors this — if the thread dies (e.g. Windows removed the hook
/// after a LowLevelHooksTimeout, or the thread panicked), the timer
/// re-installs the hook on a fresh thread.
static HOOK_ALIVE: AtomicBool = AtomicBool::new(false);

/// Install the hook. `toggle` is called whenever the user taps Win alone.
/// Safe to call once at app startup. Also starts a background health-check
/// timer that re-installs the hook if it ever dies (Windows can silently
/// remove low-level hooks if the callback takes too long, if the hook
/// thread's message pump stalls, or if an AV interferes).
pub fn install(toggle: ToggleFn) {
    if TOGGLER.set(toggle).is_err() {
        log::warn!("win hotkey hook already installed");
        return;
    }
    spawn_hook_thread();

    // Health-check timer: every 5s, check if the hook thread is alive.
    // If not, re-install the hook on a fresh thread. This makes the hook
    // self-healing — if Windows removes it for any reason (timeout, AV
    // interference, thread panic), it comes back within 5 seconds.
    std::thread::Builder::new()
        .name("win-key-hook-health".into())
        .spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                if !HOOK_ALIVE.load(Ordering::SeqCst) {
                    log::warn!("Win key hook thread died — re-installing");
                    spawn_hook_thread();
                }
            }
        })
        .ok();
}

fn spawn_hook_thread() {
    std::thread::Builder::new()
        .name("win-key-hook".into())
        .spawn(|| unsafe {
            HOOK_ALIVE.store(true, Ordering::SeqCst);

            // Per MSDN, WH_KEYBOARD_LL's hMod can be NULL because the hook is
            // not injected into another process — but using the EXE's HMODULE
            // (via GetModuleHandleW(NULL)) is the more robust form that
            // matches what `prevent-alt-win-menu` does and survives more
            // edge cases (e.g. some AV software that validates hMod).
            let hmod = match GetModuleHandleW(None) {
                Ok(h) => Some(HINSTANCE::from(h)),
                Err(_) => None,
            };
            let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), hmod, 0) {
                Ok(h) => h,
                Err(e) => {
                    log::error!("SetWindowsHookExW(WH_KEYBOARD_LL) failed: {e}");
                    HOOK_ALIVE.store(false, Ordering::SeqCst);
                    return;
                }
            };
            log::info!("Win key hook installed (swallow-down + tap -> launcher, combos -> native)");
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
            let _ = UnhookWindowsHookEx(hook);

            // If we reach here, GetMessageW returned 0 (WM_QUIT) or -1 (error).
            // Mark the hook as dead so the health-check timer re-installs it.
            HOOK_ALIVE.store(false, Ordering::SeqCst);
            log::warn!("Win key hook thread exiting — will be re-installed by health check");
        })
        .ok();
}

unsafe extern "system" fn ll_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    const SUPPRESS: LRESULT = LRESULT(1);

    if n_code == HC_ACTION as i32 {
        let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        let injected = (kb.flags & LLKHF_INJECTED).0 != 0;
        let is_down = w_param.0 as u32 == WM_KEYDOWN || w_param.0 as u32 == WM_SYSKEYDOWN;
        let is_up = w_param.0 as u32 == WM_KEYUP || w_param.0 as u32 == WM_SYSKEYUP;
        let vk = VIRTUAL_KEY(kb.vkCode as u16);

        // Never touch synthetic input (incl. our own re-injected Win down
        // or the VK__none_ dummy key-up for Alt suppression).
        if !injected {
            if vk == VK_LWIN || vk == VK_RWIN {
                if is_down {
                    // Begin a potential tap; swallow the native down for now
                    // so the OS shell can't start its "Win chord" detection
                    // (which is what opens the Start menu).
                    WIN_PENDING.store(true, Ordering::SeqCst);
                    return SUPPRESS;
                }
                if is_up {
                    if WIN_PENDING.swap(false, Ordering::SeqCst) {
                        // Tap: Win went down and up with no other key in
                        // between. Fire the toggle on a worker thread so
                        // the hook never blocks input processing, and
                        // swallow this Win-up so the OS shell definitely
                        // doesn't see a "Win release alone" event (which
                        // would otherwise re-trigger the Start menu).
                        if let Some(toggle) = TOGGLER.get() {
                            std::thread::spawn(toggle);
                        }
                        return SUPPRESS;
                    }
                    // Not pending: the down was re-injected for a combo —
                    // let the matching up through so modifier state stays
                    // sane for the OS shell.
                    return CallNextHookEx(None, n_code, w_param, l_param);
                }
            } else if vk == VK_LMENU || vk == VK_RMENU || vk == VK_MENU {
                // Alt menu suppression (replaces prevent-alt-win-menu crate).
                // Track Alt state: on Alt-down, mark pending. On Alt-up, if
                // still pending (no other key was pressed while Alt was held),
                // inject a dummy VK__none_ key-up to suppress the menu bar.
                if is_down {
                    ALT_PENDING.store(true, Ordering::SeqCst);
                } else if is_up {
                    if ALT_PENDING.swap(false, Ordering::SeqCst) {
                        // Standalone Alt tap — inject dummy key-up to
                        // prevent the focused window's menu bar from
                        // activating. This runs on the hook thread, but
                        // SendInput is fast (<1ms) so it won't trigger
                        // LowLevelHooksTimeout.
                        inject_dummy_keyup();
                    }
                }
                // Let Alt through normally — we don't suppress it, we just
                // add a dummy key-up after the release.
            } else if is_down && WIN_PENDING.load(Ordering::SeqCst) {
                // Combo detected (Win+D, Win+E, ...): cancel the tap and
                // put the Win modifier back so Windows sees the real
                // shortcut.
                WIN_PENDING.store(false, Ordering::SeqCst);
                re_inject_win_down();
            } else if is_down {
                // Any non-Win, non-Alt key pressed — cancel any pending
                // Alt tap (it was an Alt+X combo, not a standalone Alt).
                ALT_PENDING.store(false, Ordering::SeqCst);
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}

/// Re-send the Win-down we swallowed so an in-flight combo resolves natively.
/// Marked INJECTED: our own hook skips it, everything else sees a real Win.
fn re_inject_win_down() {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_LWIN,
                wScan: 0,
                dwFlags: KEYEVENTF_EXTENDEDKEY,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        if sent != 1 {
            log::warn!("re_inject_win_down: SendInput sent {sent}");
        }
    }
}

/// Inject a dummy VK__none_ key-up to suppress the focused window's menu
/// bar activation on a standalone Alt release. This is the same technique
/// the `prevent-alt-win-menu` crate used, but done in our own hook callback
/// — no second hook thread, no second message pump.
fn inject_dummy_keyup() {
    // VK__none_ = 0xFF — a virtual key that no real keyboard produces.
    // Sending its key-up after Alt-up causes Windows to cancel the menu
    // activation that would otherwise fire on a standalone Alt release.
    let vk_none = VIRTUAL_KEY(0xFF);
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk_none,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        if sent != 1 {
            log::warn!("inject_dummy_keyup: SendInput sent {sent}");
        }
    }
}
