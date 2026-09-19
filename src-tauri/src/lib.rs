// FlatUI — Tauri backend entry point
//
// Windows:
//   - taskbar: bottom, topmost, 44px, frameless, transparent, no-activate, registered as AppBar
//   - launcher: fullscreen overlay, hidden by default, opened when the user
//     taps the Win key alone — the `prevent-alt-win-menu` crate installs a
//     low-level keyboard hook in-process that both suppresses the native
//     Start menu and fires our `on_released` callback so we can toggle the
//     launcher (the old `flatwin.exe` AutoHotkey v2 helper + HTTP :2290
//     /toggle bridge has been removed); opening the launcher also minimizes
//     every visible window (show-desktop effect, win32::window::minimize_all_windows)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
pub mod crash_handler;
mod elevation;
mod embedded;
mod hide_taskbar;
mod persist;
mod setup;
#[cfg(windows)]
mod start_menu_killer;
#[cfg(windows)]
mod win32;

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};

use app_state::AppState;

// ===== Launcher visibility state machine =====
//
// Logical open/close state for the launcher, decoupled from raw window
// visibility: during the close animation the window is STILL visible (the
// frontend plays the collapse before we hide the window), so toggling off
// `is_visible()` would mis-route a Win-key press that lands mid-animation.
//
// - LAUNCHER_OPEN — the launcher is logically open (or opening).
// - CLOSE_SEQ     — monotonic sequence for pending delayed hides. A show
//                   request bumps it, which cancels the pending hide.
//
// The frontend mirrors this with its own session tokens (see
// src/launcher/main.ts) — together they make rapid toggling (Win-key spam)
// glitch-free: every request cleanly cancels whatever is in flight.
static LAUNCHER_OPEN: AtomicBool = AtomicBool::new(false);
static CLOSE_SEQ: AtomicU64 = AtomicU64::new(0);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install the crash handler BEFORE anything else — before logger init,
    // before Tauri builder, before any code that might panic. The handler
    // spawns a separate `flatui.exe --crash-report <file>` subprocess so the
    // crash dialog survives the parent's death. See `crash_handler.rs`.
    crash_handler::install();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("FlatUI starting up…");

    // The app requires administrative privileges to function (the Win-key
    // hook needs to intercept input for elevated apps, and the start-menu
    // killer needs to terminate StartMenuExperienceHost.exe). If not
    // elevated, automatically request UAC elevation via ShellExecuteW
    // "runas" — the standard Windows UAC prompt appears. If the user
    // accepts, this (non-elevated) process exits and the elevated process
    // takes over. If the user declines, this process exits.
    #[cfg(windows)]
    if !elevation::is_elevated() {
        match elevation::request_elevation() {
            elevation::ElevationOutcome::Accepted => {
                // The elevated relaunch is in flight. Exit this process
                // immediately — the elevated process will show the setup
                // window and run normally.
                log::info!("Exiting non-elevated instance — elevated relaunch is in flight");
                std::process::exit(0);
            }
            elevation::ElevationOutcome::Declined => {
                // User declined the UAC prompt. Exit — the app can't
                // function without admin rights.
                log::info!("User declined UAC — exiting");
                std::process::exit(0);
            }
            elevation::ElevationOutcome::AlreadyElevated => {
                // Shouldn't happen (we checked is_elevated above), but
                // continue if it does.
            }
        }
    }

    // Check for -rs flag (reset/clean start)
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-rs") {
        log::info!("Clean start requested (-rs flag) — deleting all config files");
        reset_config();
    }

    // Start the in-process HideTaskbar monitor (replaces the old
    // HideTaskbar.exe child process — no more standalone exe needed).
    #[cfg(windows)]
    hide_taskbar::start();

    let state = Arc::new(Mutex::new(AppState::new()));

    // Load persisted blacklist (by title + exe_path — survives restarts)
    {
        let mut s = state.lock();
        s.blacklisted = persist::load_blacklist();
        log::info!("Loaded {} blacklisted window entries from disk", s.blacklisted.len());
    }

    // The app is always elevated at this point (we checked above and exited
    // if not). The setup window is visible by default (tauri.conf.json).

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state.clone())
        .setup(move |app| {
            // Run setup in background — emits to setup window
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                // Wait for setup window to load
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Step 1: Welcome (already shown), wait 1s
                std::thread::sleep(std::time::Duration::from_secs(1));

                // Step 2: Hiding the native taskbar (in-process, no child exe)
                let _ = handle.emit("setup://step", "Hiding taskbar...");
                std::thread::sleep(std::time::Duration::from_secs(1));

                // Step 3: Installing Win key handler
                let _ = handle.emit("setup://step", "Installing Win key handler...");
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = handle.emit("setup://done", ());
            });

            if let Some(taskbar) = app.get_webview_window("taskbar") {
                position_taskbar(&taskbar);
                #[cfg(windows)]
                {
                    let _ = win32::window::apply_no_activate(&taskbar);
                    // Note: do NOT call extend_frameless on taskbar — it leaves a
                    // visible caption strip on Win11. The transparent window + WS_POPUP
                    // style alone is enough.
                    let _ = win32::window::register_appbar(&taskbar, 40);
                }
            }

            if let Some(launcher) = app.get_webview_window("launcher") {
                let _ = launcher.hide();
            }

            // Install the Win-key + Alt-key hooks. Two LL keyboard hooks
            // cooperate:
            //
            //   1. `prevent-alt-win-menu` (installed FIRST → called LAST in
            //      the LIFO chain) handles ONLY the Alt case: it suppresses
            //      the focused window's menu bar on a standalone Alt release.
            //      Its `on_released` callback returns `None` for Win — we
            //      handle Win in our own hook below.
            //
            //   2. `win32::hotkey::install` (installed LAST → called FIRST in
            //      the chain) SWALLOWS Win-down so the OS shell literally
            //      can't start its "Win chord" detection (which is what
            //      opens the Start menu). On a Win tap (Win down + Win up
            //      with no other key in between) it fires our toggle and
            //      swallows Win-up too. On a combo (Win+D, Win+E, …) it
            //      re-injects the Win-down so the shortcut still resolves
            //      natively.
            //
            // The previous integration relied on `prevent-alt-win-menu` for
            // BOTH Win and Alt. That crate's suppression strategy is to
            // inject a dummy key-up AFTER Win-up — which is racy under focus
            // changes (when the launcher webview has focus, the OS shell
            // processed Win-up before the synthetic dummy-up landed, so the
            // Start menu opened AND the close tap appeared to do nothing).
            // Swallowing Win-down is the only race-free fix.
            //
            // Replaces the old `flatwin.exe` AutoHotkey v2 helper + HTTP
            // :2290 /toggle bridge (both removed).
            #[cfg(windows)]
            {
                // (1) prevent-alt-win-menu is DISABLED.
                //
                // It was interfering with the Win-key close-tap: when the
                // launcher was open, tapping Win opened the Start menu
                // instead of closing the launcher. The crate installs its
                // own WH_KEYBOARD_LL hook on a separate thread, and the
                // two hooks' message pumps can interfere under focus
                // changes (when the launcher webview takes focus, the
                // crate's hook thread can stall the hook chain).
                //
                // Our own win32::hotkey hook (installed below) handles
                // BOTH Win (swallow + tap → toggle launcher) and Alt
                // (we add Alt menu suppression directly in the hook
                // callback — see hotkey.rs). No second hook needed.
                //
                // The crate is still in Cargo.toml for now (removing it
                // would require a Cargo.lock update); we just don't call
                // start().

                // (2) Install our own Win-key hook. It handles:
                //   - Win tap (press + release alone) → toggle launcher
                //   - Win combo (Win+D, Win+E, ...) → re-inject Win, let
                //     the combo resolve natively
                //   - Alt release alone → inject VK__none_ to suppress
                //     the focused window's menu bar (same as the crate
                //     used to do, but in our own hook — no second thread,
                //     no second message pump, no interference).
                let app_handle = app.handle().clone();
                win32::hotkey::install(Box::new(move || {
                    let app = app_handle.clone();
                    // Run on a worker thread so the hook never blocks
                    // input processing — same pattern the old HTTP server
                    // used.
                    std::thread::spawn(move || {
                        log::info!("Win key tap (hotkey.rs) — toggling launcher");
                        toggle_launcher_impl(&app);
                    });
                }));
            }

            // Start the Start menu killer monitor. This polls every 100ms
            // for StartMenuExperienceHost.exe (and SearchHost.exe) and kills
            // them on sight. This is the race-free way to prevent the Start
            // menu from appearing when the user taps Win — even if the
            // keyboard hook misses the Win-down event, the Start menu
            // process is terminated before it can render.
            #[cfg(windows)]
            {
                start_menu_killer::start();
            }

            // Initial icon scan
            #[cfg(windows)]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    refresh_taskbar_apps(&handle);
                    refresh_desktop_items(&handle);
                });
            }

            // Periodic refresh — fast polling for snappy UX
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || loop {
                    // Fullscreen check every 500ms
                    std::thread::sleep(std::time::Duration::from_millis(500));

                    #[cfg(windows)]
                    {
                        let fs = win32::fullscreen::is_foreground_fullscreen();
                        let state = handle.state::<Arc<Mutex<AppState>>>();
                        let mut s = state.lock();
                        let was_hidden = s.taskbar_hidden;
                        if fs != was_hidden {
                            s.taskbar_hidden = fs;
                            drop(s);
                            if let Some(taskbar) = handle.get_webview_window("taskbar") {
                                if fs {
                                    let _ = taskbar.hide();
                                } else {
                                    let _ = taskbar.show();
                                }
                            }
                            let _ = handle.emit("taskbar://fullscreen-changed", fs);
                        } else {
                            drop(s);
                        }

                        // Refresh taskbar apps every 2s as fallback
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        refresh_taskbar_apps(&handle);
                    }
                });
            }

            // Install WinEvent hook for instant foreground window change detection
            #[cfg(windows)]
            {
                let handle = app.handle().clone();
                let callback = Box::new(move || {
                    let h = handle.clone();
                    std::thread::spawn(move || {
                        // Small delay to let the new window settle
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        refresh_taskbar_apps(&h);
                    });
                });
                if let Err(e) = win32::winevent::install_foreground_hook(callback) {
                    log::error!("Failed to install foreground hook: {e}");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_setup,
            get_taskbar_apps,
            get_tray_icons,
            get_desktop_items,
            activate_app,
            context_menu_app,
            tray_click,
            toggle_launcher,
            close_launcher,
            launcher_close_finished,
            launch_desktop_item,
            toggle_calendar_flyout,
            get_app_windows,
            get_all_windows,
            activate_window,
            create_desktop_item,
            delete_desktop_item,
            rename_desktop_item,
            refresh_desktop,
            minimize_all_windows,
            get_blacklist,
            set_blacklisted,
            execute_run,
            execute_run_admin,
            search_programs,
            end_task,
            save_clipboard,
            load_clipboard,
            save_notes,
            load_notes,
            get_system_stats,
            get_volume,
            set_volume,
            get_app_volumes,
            set_app_volume,
            save_widget_positions,
            load_widget_positions,
            save_widget_visibility,
            load_widget_visibility,
            save_settings,
            load_settings,
            get_language,
            take_screenshot,
            save_clipboard_image,
            set_clipboard_image,
            get_clipboard_text,
            set_clipboard_text,
            show_launcher_for_screenshot,
            reboot_system,
            shutdown_system,
            add_to_startup,
            remove_from_startup,
            close_setup_window,
            exit_flatui,
            get_active_theme,
            get_all_themes,
            set_active_theme,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "taskbar" {
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running FlatUI");
}

// ===== Setup command (setup window calls this; SETUP_ONCE makes it a safe no-op
// after the backend already ran it before the builder) =====
#[tauri::command]
fn run_setup() -> Vec<(String, bool)> {
    setup::run_setup()
}

// ===== Command handlers =====

#[tauri::command]
fn get_taskbar_apps(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Vec<app_state::TaskbarApp> {
    state.lock().taskbar_apps.clone()
}

#[tauri::command]
fn get_tray_icons() -> Vec<app_state::TrayIcon> {
    Vec::new()
}

#[tauri::command]
fn get_desktop_items(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Vec<app_state::DesktopItem> {
    state.lock().desktop_items.clone()
}

#[tauri::command]
fn activate_app(app_id: String, app: tauri::AppHandle) {
    log::info!("activate_app: {app_id}");
    #[cfg(windows)]
    {
        if let Err(e) = win32::apps::activate_or_launch(&app_id) {
            log::error!("activate_app failed: {e}");
        }
    }
    let _ = app.emit("taskbar://icon-activated", app_id);
}

#[tauri::command]
fn context_menu_app(app_id: String, x: f64, y: f64) {
    log::info!("context_menu_app: {app_id} @ {x},{y}");
    #[cfg(windows)]
    {
        if let Err(e) = win32::apps::show_context_menu(&app_id, x as i32, y as i32) {
            log::error!("context_menu failed: {e}");
        }
    }
}

#[tauri::command]
fn tray_click(_tray_id: String, _right_click: Option<bool>) {
    // Tray functionality removed
}

#[tauri::command]
fn toggle_launcher(app: tauri::AppHandle) {
    log::info!("toggle_launcher called (command)");
    toggle_launcher_impl(&app);
}

pub(crate) fn toggle_launcher_impl(app: &tauri::AppHandle) {
    // Logical state, not window visibility: during the close animation the
    // window is still visible, so is_visible() would mis-route a toggle.
    if LAUNCHER_OPEN.load(Ordering::SeqCst) {
        hide_launcher_animated(app);
    } else {
        show_launcher(app);
    }
}

/// Show the launcher: window FIRST, event AFTER.
///
/// The frontend gates its open animation on real visibility + painted
/// frames, so the cube always starts together with the first presented
/// frame. (The old code emitted `force-shown` before showing the window —
/// the animation clock then ran ahead of presentation under GPU load and
/// the cube appeared speeded-up or fully skipped, e.g. while gaming.)
fn show_launcher(app: &tauri::AppHandle) {
    let Some(launcher) = app.get_webview_window("launcher") else {
        log::error!("show_launcher: launcher window not found");
        return;
    };

    // Cancel any pending delayed hide from an interrupted close.
    CLOSE_SEQ.fetch_add(1, Ordering::SeqCst);
    LAUNCHER_OPEN.store(true, Ordering::SeqCst);

    fit_launcher_to_screen(app);
    // Show-desktop effect: minimize every open window so the launcher
    // sits on a clean desktop instead of on top of other windows.
    #[cfg(windows)]
    {
        win32::window::minimize_all_windows();
    }

    let _ = launcher.set_always_on_top(true);
    let _ = launcher.show();
    let _ = launcher.set_focus();
    // Emit AFTER the window is on screen — the frontend's animation gate
    // then passes immediately instead of polling.
    let _ = app.emit("launcher://force-shown", ());

    let handle = app.clone();
    std::thread::spawn(move || refresh_desktop_items(&handle));
}

/// Hide the launcher with the close animation.
///
/// Emits `force-hidden` (the frontend FIRST fades the elements out, THEN
/// collapses the cube, then reports back via `launcher_close_finished`, at
/// which point we hide the window). A fallback thread hides the window
/// after 2.4 s in case that report is ever lost — the full two-phase close
/// runs ~1.4 s (element cascade ~0.6 s + cube collapse 0.8 s), so the
/// fallback must sit comfortably above it — superseded by any new show via
/// CLOSE_SEQ.
fn hide_launcher_animated(app: &tauri::AppHandle) {
    if !LAUNCHER_OPEN.swap(false, Ordering::SeqCst) {
        return; // already closing or closed — nothing to animate
    }
    let seq = CLOSE_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = app.emit("launcher://force-hidden", ());

    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2400));
        if CLOSE_SEQ.load(Ordering::SeqCst) == seq {
            if let Some(launcher) = handle.get_webview_window("launcher") {
                let _ = launcher.hide();
            }
            log::info!("launcher hidden via fallback timer");
        }
    });
}

/// Called by the frontend the moment its close animation finished — hide
/// the window at exactly the right time instead of a wall-clock guess.
/// Guarded so a late report can never hide a freshly re-opened launcher.
#[tauri::command]
fn launcher_close_finished(app: tauri::AppHandle) {
    if LAUNCHER_OPEN.load(Ordering::SeqCst) {
        return; // a new show superseded the close while the report was in flight
    }
    CLOSE_SEQ.fetch_add(1, Ordering::SeqCst);
    if let Some(launcher) = app.get_webview_window("launcher") {
        let _ = launcher.hide();
    }
}

// ===== Fit launcher window to the physical screen =====
// The config declares a 1920x1080 (logical) window; under DPI scaling that no
// longer matches the monitor, which made the screenshot overlay "zoomed" and
// left screen edges uncovered. Size/position it to the primary monitor in
// physical pixels so the overlay covers the entire screen 1:1.
fn fit_launcher_to_screen(app: &tauri::AppHandle) {
    if let Some(launcher) = app.get_webview_window("launcher") {
        if let Ok(Some(monitor)) = launcher.primary_monitor() {
            let pos = monitor.position();
            let size = monitor.size();
            let _ = launcher.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
            let _ = launcher.set_size(tauri::PhysicalSize::new(size.width, size.height));
            log::info!("launcher fitted to screen: {}x{} @ ({}, {})",
                size.width, size.height, pos.x, pos.y);
        }
    }
}

#[tauri::command]
fn close_launcher(app: tauri::AppHandle) {
    hide_launcher_animated(&app);
}

#[tauri::command]
fn launch_desktop_item(item_id: String, state: tauri::State<'_, Arc<Mutex<AppState>>>) {
    let path = {
        let s = state.lock();
        s.desktop_items.iter().find(|i| i.id == item_id).map(|i| i.path.clone())
    };
    // The desktop scan guarantees id == absolute path, so when the cached
    // state is stale (item created/renamed/deleted between scans) fall back
    // to the id itself. This keeps the click targeting the EXACT item that
    // was rendered — it must never degrade into a name-based lookup, which
    // could launch a different item that merely shares the display name.
    let target = match path {
        Some(p) => Some(p),
        None => {
            if std::path::Path::new(&item_id).exists() {
                log::warn!("launch_desktop_item: id not in cached state, using id as path: {item_id}");
                Some(item_id)
            } else {
                log::error!("launch_desktop_item: item not found: {item_id}");
                None
            }
        }
    };
    if let Some(p) = target {
        let kind = if std::path::Path::new(&p).is_dir() { "dir" } else { "file" };
        log::info!("launch_desktop_item: {p} (kind={kind})");
        #[cfg(windows)]
        {
            if let Err(e) = win32::shell::shell_execute(&p) {
                log::error!("launch_desktop_item failed: {e}");
            }
        }
        #[cfg(not(windows))]
        let _ = &p;
    }
}

#[tauri::command]
fn toggle_calendar_flyout() {
    log::info!("toggle_calendar_flyout (not implemented yet)");
}

#[tauri::command]
fn get_app_windows(
    app_id: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Vec<win32::peek::WindowPreview> {
    #[cfg(windows)]
    {
        let s = state.lock();
        if let Some(app) = s.taskbar_apps.iter().find(|a| a.id == app_id) {
            return win32::peek::get_windows_for_app(app);
        }
    }
    let _ = app_id;
    Vec::new()
}

#[tauri::command]
fn activate_window(hwnd: usize) {
    log::info!("activate_window: hwnd={}", hwnd);
    #[cfg(windows)]
    {
        if let Err(e) = win32::peek::activate_window_by_hwnd(hwnd as isize) {
            log::error!("activate_window failed: {e}");
        }
    }
}

#[tauri::command]
fn get_all_windows(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Vec<win32::peek::WindowPreview> {
    #[cfg(windows)]
    {
        let blacklist = state.lock().blacklisted_hwnds.clone();
        let windows_with_exe = win32::peek::get_all_windows_with_exe(&blacklist);
        return windows_with_exe
            .into_iter()
            .map(|w| win32::peek::WindowPreview {
                hwnd: w.hwnd,
                title: w.title,
                icon_data_url: w.icon_data_url,
            })
            .collect();
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[tauri::command]
fn create_desktop_item(name: String, is_folder: bool, app: tauri::AppHandle) {
    log::info!("create_desktop_item: name={} folder={}", name, is_folder);
    #[cfg(windows)]
    {
        match win32::shell::create_item(&name, is_folder) {
            Ok(_) => refresh_desktop_items(&app),
            Err(e) => log::error!("create_desktop_item failed: {e}"),
        }
    }
}

#[tauri::command]
fn delete_desktop_item(item_id: String, app: tauri::AppHandle) {
    log::info!("delete_desktop_item: {}", item_id);
    #[cfg(windows)]
    {
        match win32::shell::delete_item(&item_id) {
            Ok(_) => refresh_desktop_items(&app),
            Err(e) => log::error!("delete_desktop_item failed: {e}"),
        }
    }
}

#[tauri::command]
fn rename_desktop_item(item_id: String, new_name: String, app: tauri::AppHandle) {
    log::info!("rename_desktop_item: {} -> {}", item_id, new_name);
    #[cfg(windows)]
    {
        match win32::shell::rename_item(&item_id, &new_name) {
            Ok(_) => refresh_desktop_items(&app),
            Err(e) => log::error!("rename_desktop_item failed: {e}"),
        }
    }
}

#[tauri::command]
fn refresh_desktop(app: tauri::AppHandle) {
    refresh_desktop_items(&app);
}

// ===== Minimize all windows =====
#[tauri::command]
fn minimize_all_windows(app: tauri::AppHandle) {
    log::info!("minimize_all_windows");
    #[cfg(windows)]
    {
        // First hide the launcher window so it doesn't get minimized or
        // block. This is an INSTANT hide (everything minimizes right now,
        // so there is nothing pretty to animate over) — fix the state
        // machine accordingly: cancel any pending animated close.
        if let Some(launcher) = app.get_webview_window("launcher") {
            CLOSE_SEQ.fetch_add(1, Ordering::SeqCst);
            LAUNCHER_OPEN.store(false, Ordering::SeqCst);
            let _ = launcher.hide();
            let _ = app.emit("launcher://force-hidden", ());
        }

        // Approach: enumerate all top-level windows and call ShowWindow(SW_MINIMIZE)
        // on each that's visible + not our own + not the desktop/explorer.
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::core::BOOL;
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, IsWindowVisible, GetWindowTextW, GetWindowThreadProcessId,
            GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW, ShowWindowAsync, SW_MINIMIZE,
            IsIconic,
        };
        use std::ffi::c_void;

        struct State { skip_pid: u32 }
        let mut state = State { skip_pid: 0 };
        // Find our own PID (flatui) so we skip our windows
        // We can compare against the launcher/taskbar HWNDs which we know.
        if let Some(launcher) = app.get_webview_window("launcher") {
            if let Ok(hwnd) = launcher.hwnd() {
                let mut pid: u32 = 0;
                unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); }
                state.skip_pid = pid;
            }
        }
        let skip_pid = state.skip_pid;

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let skip_pid = *(lparam.0 as *const u32);
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            // Skip tool windows (like our own taskbar)
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if ex & (WS_EX_TOOLWINDOW.0 as isize) != 0 {
                return BOOL(1);
            }
            // Skip already minimized windows
            if IsIconic(hwnd).as_bool() {
                return BOOL(1);
            }
            // Skip our own process
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == skip_pid {
                return BOOL(1);
            }
            // Skip explorer.exe (desktop) — best effort via window title
            let mut title = [0u16; 64];
            let len = GetWindowTextW(hwnd, &mut title);
            if len > 0 {
                let title_str = String::from_utf16_lossy(&title[..len as usize]);
                let lower = title_str.to_lowercase();
                if lower.contains("program manager") || lower.is_empty() {
                    return BOOL(1);
                }
            }
            let _ = ShowWindowAsync(hwnd, SW_MINIMIZE);
            BOOL(1)
        }

        let skip_pid_ptr: *const u32 = &skip_pid;
        let lparam = LPARAM(skip_pid_ptr as isize);
        unsafe {
            let _ = EnumWindows(Some(enum_proc), lparam);
        }
    }
}

// ===== Window blacklist (by title + exe_path, persistent across launches) =====
#[tauri::command]
fn get_blacklist(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Vec<app_state::BlacklistEntry> {
    state.lock().blacklisted.clone()
}

#[tauri::command]
fn set_blacklisted(hwnd: usize, blacklisted: bool, app: tauri::AppHandle) {
    log::info!("set_blacklisted: hwnd={} blacklisted={}", hwnd, blacklisted);
    {
        let state = app.state::<Arc<Mutex<AppState>>>();
        let mut s = state.lock();

        #[cfg(windows)]
        {
            let all_windows = win32::peek::get_all_windows_with_exe(&[]);
            if blacklisted {
                if let Some(w) = all_windows.iter().find(|w| w.hwnd == hwnd) {
                    // Don't add duplicates by exe_path
                    if !s.blacklisted.iter().any(|b| b.exe_path == w.exe_path) {
                        s.blacklisted.push(app_state::BlacklistEntry {
                            exe_path: w.exe_path.clone(),
                            title: Some(w.title.clone()),
                            hwnd,
                        });
                    }
                }
            } else {
                // Remove by matching exe_path
                if let Some(w) = all_windows.iter().find(|w| w.hwnd == hwnd) {
                    s.blacklisted.retain(|b| b.exe_path != w.exe_path);
                }
            }
        }

        persist::save_blacklist(&s.blacklisted);
    }
    refresh_taskbar_apps(&app);
    let _ = app.emit("taskbar://blacklist-updated", ());
}

// ===== Clipboard history =====
#[tauri::command]
fn save_clipboard(items: Vec<String>) {
    persist::save_clipboard(&items);
}

#[tauri::command]
fn load_clipboard() -> Vec<String> {
    persist::load_clipboard()
}

// ===== Notes =====
#[tauri::command]
fn save_notes(text: String) {
    persist::save_notes(&text);
}

#[tauri::command]
fn load_notes() -> String {
    persist::load_notes()
}

// ===== System stats (CPU, RAM, GPU) =====
#[derive(serde::Serialize)]
struct SystemStats {
    cpu_usage: f32,
    ram_usage: f32,
    ram_total_gb: f32,
    ram_used_gb: f32,
}

#[tauri::command]
fn get_system_stats() -> SystemStats {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

        // RAM
        let mut mem_status = MEMORYSTATUSEX::default();
        mem_status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        unsafe { let _ = GlobalMemoryStatusEx(&mut mem_status); }
        let ram_usage = mem_status.dwMemoryLoad as f32;
        let ram_total_gb = (mem_status.ullTotalPhys as f64 / 1073741824.0) as f32;
        let ram_used_gb = ram_total_gb * ram_usage / 100.0;

        // CPU usage — simplified: use GetSystemTimes for idle/kernel/user
        // For a quick approximation, we use the memory load as a proxy.
        // Real CPU usage requires two samples of GetSystemTimes with a delay.
        let cpu_usage = get_cpu_usage();

        return SystemStats { cpu_usage, ram_usage, ram_total_gb, ram_used_gb };
    }
    #[cfg(not(windows))]
    {
        SystemStats { cpu_usage: 0.0, ram_usage: 0.0, ram_total_gb: 0.0, ram_used_gb: 0.0 }
    }
}

#[cfg(windows)]
fn get_cpu_usage() -> f32 {
    use windows::Win32::Foundation::FILETIME;

    // Quick CPU usage: take two samples 100ms apart
    // GetSystemTimes is in Win32_System_ProcessInformation or we use it via link!
    extern "system" {
        fn GetSystemTimes(
            idle_time: *mut FILETIME,
            kernel_time: *mut FILETIME,
            user_time: *mut FILETIME,
        ) -> i32;
    }

    let mut idle1 = FILETIME::default();
    let mut kernel1 = FILETIME::default();
    let mut user1 = FILETIME::default();
    unsafe { let _ = GetSystemTimes(&mut idle1, &mut kernel1, &mut user1); }

    std::thread::sleep(std::time::Duration::from_millis(100));

    let mut idle2 = FILETIME::default();
    let mut kernel2 = FILETIME::default();
    let mut user2 = FILETIME::default();
    unsafe { let _ = GetSystemTimes(&mut idle2, &mut kernel2, &mut user2); }

    let idle = filetime_diff(&idle2, &idle1);
    let kernel = filetime_diff(&kernel2, &kernel1);
    let user = filetime_diff(&user2, &user1);

    let total = kernel + user;
    if total > 0 {
        (1.0 - (idle as f32 / total as f32)) * 100.0
    } else {
        0.0
    }
}

#[cfg(windows)]
fn filetime_diff(a: &windows::Win32::Foundation::FILETIME, b: &windows::Win32::Foundation::FILETIME) -> u64 {
    let av = ((a.dwHighDateTime as u64) << 32) | (a.dwLowDateTime as u64);
    let bv = ((b.dwHighDateTime as u64) << 32) | (b.dwLowDateTime as u64);
    av.saturating_sub(bv)
}

// ===== Volume control (Windows Core Audio API) =====
#[cfg(windows)]
fn get_volume_impl() -> f32 {
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eRender, eConsole,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, COINIT_MULTITHREADED, CLSCTX_ALL};
    use windows::core::Interface;

    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let vol = unsafe {
        let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            Ok(e) => e,
            Err(_) => { CoUninitialize(); return 0.5; }
        };
        let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
            Ok(d) => d,
            Err(_) => { CoUninitialize(); return 0.5; }
        };
        let endpoint_volume: IAudioEndpointVolume = match device.Activate(CLSCTX_ALL, None) {
            Ok(v) => v,
            Err(_) => { CoUninitialize(); return 0.5; }
        };
        endpoint_volume.GetMasterVolumeLevelScalar().unwrap_or(0.5)
    };
    unsafe { CoUninitialize() };
    vol
}

#[cfg(windows)]
fn set_volume_impl(volume: f32) {
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eRender, eConsole,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, COINIT_MULTITHREADED, CLSCTX_ALL};
    use windows::core::Interface;

    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    unsafe {
        if let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                if let Ok(endpoint_volume) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                    let _ = endpoint_volume.SetMasterVolumeLevelScalar(volume, &windows::core::GUID::zeroed());
                }
            }
        }
    }
    unsafe { CoUninitialize() };
}

#[tauri::command]
fn get_volume() -> f32 {
    #[cfg(windows)]
    { return get_volume_impl(); }
    #[cfg(not(windows))]
    { 0.5 }
}

#[tauri::command]
fn set_volume(volume: f32) {
    #[cfg(windows)]
    { set_volume_impl(volume); }
}

#[derive(serde::Serialize)]
struct AppVolume {
    pid: u32,
    name: String,
    volume: f32,
}

#[tauri::command]
fn get_app_volumes() -> Vec<AppVolume> {
    Vec::new()
}

#[tauri::command]
fn set_app_volume(_pid: u32, _volume: f32) {
    // Placeholder — per-app volume needs ISimpleAudioVolume
}

// ===== Widget positions =====
#[tauri::command]
fn save_widget_positions(positions: std::collections::HashMap<String, (f64, f64)>) {
    persist::save_widget_positions(&positions);
}

#[tauri::command]
fn load_widget_positions() -> std::collections::HashMap<String, (f64, f64)> {
    persist::load_widget_positions()
}

// ===== Widget visibility =====
#[tauri::command]
fn save_widget_visibility(visibility: std::collections::HashMap<String, bool>) {
    persist::save_widget_visibility(&visibility);
}

#[tauri::command]
fn load_widget_visibility() -> std::collections::HashMap<String, bool> {
    persist::load_widget_visibility()
}

// ===== Settings =====
#[tauri::command]
fn save_settings(settings: serde_json::Value) {
    let mut current = persist::load_settings();

    // Update fields from the JSON
    if let Some(theme) = settings.get("theme").and_then(|v| v.as_str()) {
        current.theme = theme.to_string();
    }
    if let Some(auto) = settings.get("auto_fullscreen").and_then(|v| v.as_bool()) {
        current.auto_fullscreen = auto;
    }
    if let Some(interval) = settings.get("refresh_interval").and_then(|v| v.as_u64()) {
        current.refresh_interval = interval;
    }
    if let Some(cube) = settings.get("cube_animation").and_then(|v| v.as_bool()) {
        current.cube_animation = cube;
    }
    persist::save_settings(&current);
    log::info!("Settings saved — theme: {}", current.theme);
}

#[tauri::command]
fn load_settings() -> persist::Settings {
    persist::load_settings()
}

// ===== Language indicator =====
#[tauri::command]
fn get_language() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyState, VK_CAPITAL};

        unsafe {
            let hwnd = GetForegroundWindow();
            let mut pid: u32 = 0;
            let mut thread_id: u32 = 0;

            if !hwnd.is_invalid() {
                thread_id = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }

            // Get the keyboard layout for the foreground window's thread
            let hkl = GetKeyboardLayout(thread_id);
            // The low word of the HKL contains the language identifier
            let lang_id = (hkl.0 as usize & 0xFFFF) as u32;

            let lang_code = match lang_id {
                0x0409 => "en",
                0x0419 => "ru",
                0x040C => "fr",
                0x0407 => "de",
                0x0410 => "it",
                0x040A => "es",
                0x0415 => "pl",
                0x0422 => "uk",
                _ => "??",
            };

            // Check capslock
            let caps_state = GetKeyState(VK_CAPITAL.0 as i32);
            let caps_on = (caps_state & 0x0001) != 0;
            if caps_on {
                return lang_code.to_uppercase();
            }
            return lang_code.to_string();
        }
    }
    #[cfg(not(windows))]
    {
        "en".to_string()
    }
}

// ===== Screenshot =====
#[tauri::command]
fn take_screenshot() -> Option<String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            GetDC, CreateCompatibleDC, CreateCompatibleBitmap,
            SelectObject, BitBlt, GetDIBits, DeleteDC, DeleteObject, ReleaseDC,
            BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, BI_RGB, RGBQUAD, SRCCOPY,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        use image::ImageEncoder;
        use base64::Engine;

        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);

            let dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(dc));
            let bmp = CreateCompatibleBitmap(dc, w, h);
            let old = SelectObject(mem_dc, bmp.into());

            let _ = BitBlt(mem_dc, 0, 0, w, h, Some(dc), 0, 0, SRCCOPY);
            let _ = SelectObject(mem_dc, old);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut pixels: Vec<u8> = vec![0u8; (w * h * 4) as usize];
            let copied = GetDIBits(
                mem_dc, bmp, 0, h as u32,
                Some(pixels.as_mut_ptr() as *mut _),
                &bmi as *const _ as *mut _, DIB_RGB_COLORS,
            );

            let _ = DeleteDC(mem_dc);
            let _ = DeleteObject(bmp.into());
            let _ = ReleaseDC(None, dc);

            if copied == 0 {
                return None;
            }

            // BGRA → RGBA
            for chunk in pixels.chunks_mut(4) {
                let b = chunk[0];
                chunk[0] = chunk[2];
                chunk[2] = b;
            }

            let img = image::RgbaImage::from_raw(w as u32, h as u32, pixels)?;
            let mut buf = std::io::Cursor::new(Vec::new());
            let png = image::codecs::png::PngEncoder::new(&mut buf);
            png.write_image(&img, w as u32, h as u32, image::ExtendedColorType::Rgba8).ok()?;

            let png_bytes = buf.into_inner();

            // Save to %TEMP%\flatshot.png
            if let Ok(temp) = std::env::var("TEMP") {
                let path = std::path::PathBuf::from(temp).join("flatshot.png");
                let _ = std::fs::write(&path, &png_bytes);
            }

            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
            Some(format!("data:image/png;base64,{}", b64))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

// ===== Save clipboard image =====
#[tauri::command]
fn save_clipboard_image(data_url: String) {
    // Decode base64 from data URL and save to clipboard
    // For now, save the data URL in clipboard history
    if let Some(b64) = data_url.strip_prefix("data:image/png;base64,") {
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            // Save to %TEMP%\flatshot.png (overwrite)
            if let Ok(temp) = std::env::var("TEMP") {
                let path = std::path::PathBuf::from(temp).join("flatshot.png");
                let _ = std::fs::write(&path, &bytes);
            }
        }
    }
}


// ===== Clipboard image (Win32 — Lightshot-style: CF_DIB + registered PNG) =====
/// Puts a REAL image on the system clipboard so pasting into any app inserts
/// the picture, not a base64 text string.
#[tauri::command]
fn set_clipboard_image(data_url: String) {
    #[cfg(windows)]
    {
        use base64::Engine;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        };
        use windows::Win32::System::Ole::CF_DIB;
        use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
        use windows::core::w;

        let Some(b64) = data_url.strip_prefix("data:image/png;base64,") else {
            log::warn!("set_clipboard_image: not a PNG data URL");
            return;
        };
        let Ok(png) = base64::engine::general_purpose::STANDARD.decode(b64) else {
            log::warn!("set_clipboard_image: invalid base64 payload");
            return;
        };
        let Ok(img) = image::load_from_memory(&png) else {
            log::warn!("set_clipboard_image: could not decode PNG");
            return;
        };
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as i32, rgba.height() as i32);
        if w == 0 || h == 0 {
            return;
        }

        // Bottom-up 32bpp CF_DIB (BI_RGB) — the format MS Paint / Office expect.
        let mut dib: Vec<u8> = Vec::with_capacity(40 + rgba.len());
        dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
        dib.extend_from_slice(&w.to_le_bytes()); // biWidth
        dib.extend_from_slice(&h.to_le_bytes()); // biHeight (positive = bottom-up)
        dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
        dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        dib.extend_from_slice(&((w * h * 4) as u32).to_le_bytes()); // biSizeImage
        dib.extend_from_slice(&2835u32.to_le_bytes()); // biXPelsPerMeter (~72 DPI)
        dib.extend_from_slice(&2835u32.to_le_bytes()); // biYPelsPerMeter
        dib.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        dib.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
        for y in (0..h).rev() {
            let row_start = (y as usize) * (w as usize) * 4;
            let row = &rgba.as_raw()[row_start..row_start + (w as usize) * 4];
            for px in row.chunks_exact(4) {
                dib.push(px[2]); // B
                dib.push(px[1]); // G
                dib.push(px[0]); // R
                dib.push(255); // A
            }
        }

        unsafe {
            if OpenClipboard(None).is_err() {
                log::warn!("set_clipboard_image: OpenClipboard failed");
                return;
            }
            let _ = EmptyClipboard();
            let dib_ok = put_clipboard_data(CF_DIB.0 as u32, &dib);
            let png_fmt = RegisterClipboardFormatW(w!("PNG"));
            let png_ok = put_clipboard_data(png_fmt, &png);
            let _ = CloseClipboard();
            log::info!("set_clipboard_image: {}x{} -> dib={} png={}", w, h, dib_ok, png_ok);
        }

        unsafe fn put_clipboard_data(format: u32, data: &[u8]) -> bool {
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

            let Ok(hg) = GlobalAlloc(GMEM_MOVEABLE, data.len()) else {
                return false;
            };
            let ptr = GlobalLock(hg);
            if ptr.is_null() {
                return false;
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
            let _ = GlobalUnlock(hg);
            match SetClipboardData(format, Some(HANDLE(hg.0))) {
                Ok(_) => true, // ownership transferred to the clipboard
                Err(e) => {
                    log::warn!("SetClipboardData(fmt {format}) failed: {e}");
                    false
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = data_url;
    }
}

// ===== Show launcher for screenshot (instant, no cube animation) =====
#[tauri::command]
fn show_launcher_for_screenshot(app: tauri::AppHandle) {
    if let Some(launcher) = app.get_webview_window("launcher") {
        // Screenshot mode reuses the launcher window without the launcher
        // UI. Mark it logically open so a later close_launcher (which the
        // screenshot flow invokes) goes through the proper state machine
        // and actually hides the window.
        CLOSE_SEQ.fetch_add(1, Ordering::SeqCst);
        LAUNCHER_OPEN.store(true, Ordering::SeqCst);
        fit_launcher_to_screen(&app);
        let _ = launcher.show();
        let _ = launcher.set_focus();
        let _ = launcher.set_always_on_top(true);
    }
}

// ===== Close setup window =====
#[tauri::command]
fn close_setup_window(app: tauri::AppHandle) {
    if let Some(setup) = app.get_webview_window("setup") {
        let _ = setup.close();
    }
}

// ===== Exit FlatUI — revert everything and quit =====
//
// Called when the user clicks the exit button (top-left of the launcher).
// Reverts all shell modifications:
//   1. Stop the start-menu killer (so StartMenuExperienceHost.exe can run)
//   2. Show the native taskbar (stop hiding it)
//   3. Restart explorer.exe (restores the native shell: taskbar, Start menu,
//      desktop icons, tray)
//   4. Exit the FlatUI process
#[tauri::command]
fn exit_flatui() {
    log::info!("exit_flatui: reverting everything and exiting");

    // 1. Stop the start-menu killer
    #[cfg(windows)]
    start_menu_killer::stop();

    // 2. Show the native taskbar (stop the hide_taskbar monitor and
    //    immediately set alpha to 255)
    #[cfg(windows)]
    hide_taskbar::stop();

    // 3. Restart explorer.exe — this restores the native shell (taskbar,
    //    Start menu, desktop). We kill explorer first, then relaunch it.
    //    The relaunch uses ShellExecuteW with "open" on explorer.exe.
    #[cfg(windows)]
    {
        use std::process::Command;
        // Kill explorer — it will auto-restart, but we also relaunch it
        // explicitly to be sure.
        let _ = Command::new("taskkill")
            .args(["/f", "/im", "explorer.exe"])
            .spawn();
        // Give it a moment to die
        std::thread::sleep(std::time::Duration::from_millis(500));
        // Relaunch explorer
        let _ = Command::new("explorer.exe").spawn();
    }

    // 4. Exit the FlatUI process
    std::process::exit(0);
}

// ===== System commands =====
#[tauri::command]
fn reboot_system() {
    log::info!("Rebooting system...");
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("shutdown").args(["/r", "/t", "0"]).spawn();
    }
}

#[tauri::command]
fn shutdown_system() {
    log::info!("Shutting down system...");
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("shutdown").args(["/s", "/t", "0"]).spawn();
    }
}

#[tauri::command]
fn add_to_startup() -> bool {
    log::info!("Adding FlatUI to startup...");
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let startup_dir = std::path::PathBuf::from(&appdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs\\Startup");
            let _ = std::fs::create_dir_all(&startup_dir);

            if let Ok(exe_path) = std::env::current_exe() {
                // Create a .lnk shortcut using PowerShell (reliable, no COM dependency)
                let lnk_path = startup_dir.join("FlatUI.lnk");
                let exe_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                let ps_script = format!(
                    "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.Save()",
                    lnk_path.display(),
                    exe_path.display(),
                    exe_dir.display()
                );
                match std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &ps_script])
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            log::info!("Created FlatUI.lnk shortcut in startup");
                            return true;
                        } else {
                            log::error!("PowerShell shortcut creation failed: {}", String::from_utf8_lossy(&output.stderr));
                        }
                    }
                    Err(e) => log::error!("Failed to run PowerShell: {}", e),
                }

                // Fallback: .bat file
                let bat_path = startup_dir.join("FlatUI.bat");
                let exe_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                let bat_content = format!(
                    "@echo off\ncd /d \"{}\"\nstart \"\" \"{}\"",
                    exe_dir.display(),
                    exe_path.display()
                );
                if std::fs::write(&bat_path, bat_content).is_ok() {
                    log::info!("Created FlatUI.bat fallback in startup");
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(windows))]
    { false }
}

#[tauri::command]
fn remove_from_startup() -> bool {
    log::info!("Removing FlatUI from startup...");
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let startup_dir = std::path::PathBuf::from(&appdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs\\Startup");
            let bat_path = startup_dir.join("FlatUI.bat");
            let lnk_path = startup_dir.join("FlatUI.lnk");
            let mut removed = false;
            if bat_path.exists() {
                let _ = std::fs::remove_file(&bat_path);
                removed = true;
            }
            if lnk_path.exists() {
                let _ = std::fs::remove_file(&lnk_path);
                removed = true;
            }
            log::info!("Removed from startup: {}", removed);
            return removed;
        }
        false
    }
    #[cfg(not(windows))]
    { false }
}

// ===== Clipboard text (global, via Win32 API) =====
#[tauri::command]
fn get_clipboard_text() -> Option<String> {
    #[cfg(windows)]
    {
        use windows::Win32::System::DataExchange::{
            OpenClipboard, CloseClipboard, GetClipboardData,
        };
        use windows::Win32::System::Ole::CF_UNICODETEXT;

        unsafe {
            if OpenClipboard(None).is_err() {
                return None;
            }

            let result = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
                Ok(handle) => {
                    let ptr = handle.0 as *const u16;
                    if ptr.is_null() {
                        None
                    } else {
                        let mut len = 0;
                        while *ptr.add(len) != 0 {
                            len += 1;
                        }
                        let slice = std::slice::from_raw_parts(ptr, len);
                        Some(String::from_utf16_lossy(slice))
                    }
                }
                Err(_) => None,
            };

            let _ = CloseClipboard();
            result
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[tauri::command]
fn set_clipboard_text(text: String) {
    #[cfg(windows)]
    {
        use windows::Win32::System::DataExchange::{
            OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData,
        };
        use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
        use windows::Win32::System::Ole::CF_UNICODETEXT;
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;

        let wide: Vec<u16> = OsStr::new(&text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let byte_len = wide.len() * 2;

        unsafe {
            if OpenClipboard(None).is_err() {
                return;
            }
            let _ = EmptyClipboard();

            if let Ok(h_mem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
                let ptr = GlobalLock(h_mem);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, byte_len);
                    let _ = GlobalUnlock(h_mem);
                    let handle = windows::Win32::Foundation::HANDLE(h_mem.0);
                    let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(handle));
                }
            }

            let _ = CloseClipboard();
        }
    }
}

// ===== Theme commands =====
#[tauri::command]
fn get_active_theme() -> Option<persist::Theme> {
    persist::get_active_theme()
}

#[tauri::command]
fn get_all_themes() -> Vec<persist::Theme> {
    persist::load_themes().themes
}

#[tauri::command]
fn set_active_theme(name: String) {
    persist::set_active_theme(&name);
    log::info!("Active theme set to: {}", name);
}

// ===== End task — kill the process owning the window =====
#[tauri::command]
fn end_task(app_id: String) {
    #[cfg(windows)]
    {
        if let Some(hwnd_val) = app_id.strip_prefix("run:").and_then(|s| s.parse::<isize>().ok()) {
            let hwnd = windows::Win32::Foundation::HWND(hwnd_val as *mut std::ffi::c_void);
            unsafe {
                // Get the process ID, then open + terminate
                use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
                use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
                use windows::Win32::Foundation::CloseHandle;

                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid == 0 {
                    log::error!("end_task: failed to get pid");
                    return;
                }
                if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                    let _ = TerminateProcess(handle, 0);
                    let _ = CloseHandle(handle);
                    log::info!("end_task: terminated pid {}", pid);
                } else {
                    log::error!("end_task: OpenProcess failed for pid {}", pid);
                }
            }
        }
    }
}

// ===== Run dialog (Win+R replacement) =====
#[tauri::command]
fn execute_run(command: String, app: tauri::AppHandle) {
    log::info!("execute_run: {}", command);
    #[cfg(windows)]
    {
        if let Err(e) = win32::shell::shell_execute(&command) {
            log::error!("execute_run failed: {e}");
        }
    }
    let _ = app;
}

#[tauri::command]
fn execute_run_admin(command: String) {
    log::info!("execute_run_admin: {}", command);
    #[cfg(windows)]
    {
        // Use ShellExecuteW with verb "runas" to elevate
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let wide_path: Vec<u16> = OsStr::new(&command)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = OsStr::new("runas")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let _ = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(wide_path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
    }
}

// ===== Search installed programs (Start Menu shortcuts) + system shortcuts =====
#[derive(serde::Serialize)]
struct SearchResult {
    id: String,
    name: String,
    path: String,
    icon_data_url: Option<String>,
    is_folder: bool,
}

#[tauri::command]
fn search_programs(query: String) -> Vec<SearchResult> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    #[cfg(windows)]
    {
        let mut results = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        // 1. Built-in system shortcuts (control panel, add/remove, etc.)
        let system_shortcuts = [
            ("control panel", "control.exe"),
            ("add or remove programs", "appwiz.cpl"),
            ("programs and features", "appwiz.cpl"),
            ("uninstall a program", "appwiz.cpl"),
            ("system properties", "sysdm.cpl"),
            ("power options", "powercfg.cpl"),
            ("mouse properties", "main.cpl"),
            ("sound", "mmsys.cpl"),
            ("audio devices", "mmsys.cpl"),
            ("device manager", "devmgmt.msc"),
            ("task manager", "taskmgr.exe"),
            ("registry editor", "regedit.exe"),
            ("services", "services.msc"),
            ("event viewer", "eventvwr.msc"),
            ("disk management", "diskmgmt.msc"),
            ("computer management", "compmgmt.msc"),
            ("local security policy", "secpol.msc"),
            ("group policy", "gpedit.msc"),
            ("command prompt", "cmd.exe"),
            ("powershell", "powershell.exe"),
            ("windows terminal", "wt.exe"),
            ("settings", "ms-settings:"),
            ("display settings", "ms-settings:display"),
            ("network settings", "ms-settings:network"),
            ("bluetooth", "ms-settings:bluetooth"),
            ("apps", "ms-settings:appsfeatures"),
            ("accounts", "ms-settings:accounts"),
            ("personalization", "ms-settings:personalization"),
            ("windows update", "ms-settings:windowsupdate"),
            ("date and time", "timedate.cpl"),
            ("internet options", "inetcpl.cpl"),
            ("user accounts", "netplwiz"),
            ("system information", "msinfo32.exe"),
            ("file explorer options", "control.exe folders"),
            ("file explorer", "explorer.exe"),
            ("recycle bin", "explorer.exe shell:RecycleBinFolder"),
            ("this pc", "explorer.exe"),
            ("snipping tool", "snippingtool.exe"),
            ("calculator", "calc.exe"),
            ("notepad", "notepad.exe"),
            ("paint", "mspaint.exe"),
            ("magnifier", "magnify.exe"),
            ("narrator", "narrator.exe"),
            ("on-screen keyboard", "osk.exe"),
            // FlatUI commands
            ("reboot", "flatui:reboot"),
            ("shutdown", "flatui:shutdown"),
            ("add flatui to startup", "flatui:addstartup"),
            ("remove flatui from startup", "flatui:removestartup"),
        ];

        for (name, cmd) in system_shortcuts.iter() {
            if name.contains(&q) {
                let key = name.to_lowercase();
                if !seen_names.contains(&key) {
                    seen_names.insert(key);
                    results.push(SearchResult {
                        id: format!("sys:{}", cmd),
                        name: name.to_string(),
                        path: cmd.to_string(),
                        icon_data_url: None,
                        is_folder: false,
                    });
                }
            }
        }

        // 2. Start Menu shortcuts
        let start_menu_dirs: Vec<std::path::PathBuf> = vec![
            std::env::var("PROGRAMDATA")
                .map(std::path::PathBuf::from)
                .map(|p| p.join("Microsoft\\Windows\\Start Menu\\Programs"))
                .unwrap_or_default(),
            std::env::var("APPDATA")
                .map(std::path::PathBuf::from)
                .map(|p| p.join("Microsoft\\Windows\\Start Menu\\Programs"))
                .unwrap_or_default(),
        ];

        for dir in start_menu_dirs {
            walk_programs(&dir, &q, &mut results, &mut seen_names, false);
        }

        // Also include Desktop items. Directory entries are included here so
        // that a desktop FOLDER is never shadowed by a same-named .exe/.lnk —
        // both appear as separate, individually launchable results.
        if let Ok(desktop_dir) = std::env::var("USERPROFILE")
            .map(std::path::PathBuf::from)
            .map(|p| p.join("Desktop"))
        {
            walk_programs(&desktop_dir, &q, &mut results, &mut seen_names, true);
        }

        // Disambiguate colliding display names. The Start Menu walk recurses
        // into Programs\Startup, so an autostart "FlatUI.lnk" (→ flatui.exe)
        // is returned alongside a desktop folder "flatui" and "flatui.exe"
        // — three results that all rendered as "flatui", with the exe
        // shortcut sorting FIRST. Picking "the flatui entry" then launched
        // the exe instead of the folder. Same rule as the desktop grid:
        // when several results share a label, non-folder items show their
        // full file name with extension, so the folder becomes the only
        // entry named exactly "flatui".
        {
            let mut name_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for r in results.iter() {
                *name_counts.entry(r.name.to_lowercase()).or_default() += 1;
            }
            for r in results.iter_mut() {
                let count = name_counts.get(&r.name.to_lowercase()).copied().unwrap_or(0);
                if count > 1 && !r.is_folder {
                    if let Some(fname) =
                        std::path::Path::new(&r.path).file_name().and_then(|s| s.to_str())
                    {
                        r.name = fname.to_string();
                    }
                }
            }
        }

        // Sort by the (now disambiguated) label so the folder — the only
        // entry still named exactly "flatui" — ranks above its same-stem
        // .lnk/.exe siblings.
        results.sort_by(|a, b| {
            let ia = a.name.to_lowercase().find(&q).unwrap_or(usize::MAX);
            let ib = b.name.to_lowercase().find(&q).unwrap_or(usize::MAX);
            ia.cmp(&ib).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        results.truncate(20);
        return results;
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn walk_programs(
    dir: &std::path::Path,
    q: &str,
    results: &mut Vec<SearchResult>,
    seen: &mut std::collections::HashSet<String>,
    include_dirs: bool,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        if is_dir {
            // Always recurse so nested shortcuts are still found.
            walk_programs(&path, q, results, seen, include_dirs);
            // Directory entries themselves are only returned for desktop
            // scans, where a folder must stay launchable even when an exe
            // with the same display name exists next to it.
            if !include_dirs {
                continue;
            }
        } else {
            // Only .lnk shortcuts and .exe files
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "lnk" && ext != "exe" {
                continue;
            }
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let name_lower = name.to_lowercase();
        if !name_lower.contains(q) {
            continue;
        }
        // Deduplicate by full PATH, not by display name. Deduping by name
        // made the first same-named match win, so a desktop folder could be
        // shadowed by (or resolve to) a completely different .exe/.lnk that
        // merely shares the file stem.
        let key = path.to_string_lossy().to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let icon = crate::win32::icon::extract_icon_for_path(&path.to_string_lossy());
        let path_str = path.to_string_lossy().to_string();
        results.push(SearchResult {
            id: path_str.clone(),
            name,
            path: path_str,
            icon_data_url: icon,
            is_folder: is_dir,
        });
    }
}

// ===== Reset config (for -rs flag) =====
fn reset_config() {
    use std::path::PathBuf;
    let data_dir = if let Some(lad) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(&lad).join("FlatUI")
    } else {
        return;
    };

    let files = [
        "blacklist.json",
        "clipboard.json",
        "notes.txt",
        "widgets.json",
        "settings.json",
        "themes.json",
    ];

    for file in &files {
        let path = data_dir.join(file);
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(_) => log::info!("Deleted {}", file),
                Err(e) => log::warn!("Failed to delete {}: {}", file, e),
            }
        }
    }
}

// ===== Helpers =====

fn position_taskbar(window: &tauri::WebviewWindow) {
    if let Some(monitor) = window.current_monitor().ok().flatten() {
        let mon_size = monitor.size();
        let mon_pos = monitor.position();
        let scale = monitor.scale_factor();
        let logical_h = 40.0;
        let physical_h = (logical_h * scale).round() as i32;
        let _ = window.set_size(tauri::PhysicalSize {
            width: mon_size.width,
            height: physical_h as u32,
        });
        let _ = window.set_position(tauri::PhysicalPosition {
            x: mon_pos.x,
            y: mon_pos.y + mon_size.height as i32 - physical_h,
        });
    }
}

#[cfg(windows)]
fn refresh_taskbar_apps(handle: &tauri::AppHandle) {
    // Reconcile persistent blacklist entries with currently open windows.
    // Match by exe_path (permanent). Remove entries whose exe_path is empty
    // or matches no current window AND has no valid exe_path (phantom cleanup).
    {
        let state = handle.state::<Arc<Mutex<AppState>>>();
        let mut s = state.lock();
        let all_windows = win32::peek::get_all_windows_with_exe(&[]);

        let mut new_hwnds: Vec<usize> = Vec::new();
        let mut valid_entries: Vec<app_state::BlacklistEntry> = Vec::new();

        for mut entry in s.blacklisted.drain(..) {
            // Skip entries with empty exe_path (phantom data from old format)
            if entry.exe_path.is_empty() {
                continue;
            }

            // Try to find a current window matching this exe_path
            if let Some(w) = all_windows.iter().find(|w| w.exe_path == entry.exe_path) {
                entry.hwnd = w.hwnd;
                // Update title if we have one
                if entry.title.is_none() || entry.title.as_deref() != Some(&w.title) {
                    entry.title = Some(w.title.clone());
                }
                new_hwnds.push(w.hwnd);
            } else {
                // Window not currently open — keep entry (will match when app reopens)
                entry.hwnd = 0;
            }
            valid_entries.push(entry);
        }

        s.blacklisted = valid_entries;
        s.blacklisted_hwnds = new_hwnds;
    }

    let blacklist_hwnds = handle.state::<Arc<Mutex<AppState>>>().lock().blacklisted_hwnds.clone();
    match win32::apps::scan_taskbar(&blacklist_hwnds) {
        Ok(apps) => {
            let state = handle.state::<Arc<Mutex<AppState>>>();
            let mut s = state.lock();

            let mut new_order: Vec<String> = Vec::new();
            for id in s.app_order.iter() {
                if apps.iter().any(|a| &a.id == id) {
                    new_order.push(id.clone());
                }
            }
            for a in apps.iter() {
                if !new_order.contains(&a.id) {
                    new_order.push(a.id.clone());
                }
            }
            s.app_order = new_order.clone();

            let mut sorted_apps = apps.clone();
            sorted_apps.sort_by_key(|a| {
                new_order.iter().position(|id| id == &a.id).unwrap_or(usize::MAX)
            });

            s.taskbar_apps = sorted_apps.clone();
            drop(s);
            let _ = handle.emit("taskbar://apps-updated", sorted_apps);
        }
        Err(e) => log::error!("scan_taskbar failed: {e}"),
    }
}

#[cfg(windows)]
fn refresh_desktop_items(handle: &tauri::AppHandle) {
    match win32::shell::scan_desktop() {
        Ok(items) => {
            handle.state::<Arc<Mutex<AppState>>>().lock().desktop_items = items.clone();
            let _ = handle.emit("launcher://items-updated", items);
        }
        Err(e) => log::error!("scan_desktop failed: {e}"),
    }
}

#[cfg(not(windows))]
fn refresh_taskbar_apps(_: &tauri::AppHandle) {}
#[cfg(not(windows))]
fn refresh_desktop_items(_: &tauri::AppHandle) {}
