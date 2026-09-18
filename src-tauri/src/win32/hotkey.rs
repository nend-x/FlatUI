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
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, VIRTUAL_KEY, VK_LWIN,
    VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    UnhookWindowsHookEx,
};

type ToggleFn = Box<dyn Fn() + Send + Sync>;

static TOGGLER: OnceCell<ToggleFn> = OnceCell::new();
static WIN_PENDING: AtomicBool = AtomicBool::new(false);

/// Install the hook. `toggle` is called whenever the user taps Win alone.
/// Safe to call once at app startup. Installing more than once is a no-op
/// (the second call logs a warning and returns without re-installing).
pub fn install(toggle: ToggleFn) {
    if TOGGLER.set(toggle).is_err() {
        log::warn!("win hotkey hook already installed");
        return;
    }
    std::thread::Builder::new()
        .name("win-key-hook".into())
        .spawn(|| unsafe {
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
                    return;
                }
            };
            log::info!("Win key hook installed (swallow-down + tap -> launcher, combos -> native)");
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
            let _ = UnhookWindowsHookEx(hook);
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

        // Never touch synthetic input (incl. our own re-injected Win down).
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
            } else if is_down && WIN_PENDING.load(Ordering::SeqCst) {
                // Combo detected (Win+D, Win+E, ...): cancel the tap and
                // put the Win modifier back so Windows sees the real
                // shortcut.
                WIN_PENDING.store(false, Ordering::SeqCst);
                re_inject_win_down();
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
