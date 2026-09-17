#![cfg(windows)]
// App scanner — enumerate running windowed apps + pinned taskbar items.

use crate::app_state::TaskbarApp;
use crate::win32::icon::extract_icon_for_path;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::{self, Interface, PCWSTR, HRESULT};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::core::BOOL;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW, WS_EX_APPWINDOW,
    GetForegroundWindow,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::Win32::System::Com::IPersistFile;
use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;

pub fn scan_taskbar(blacklist: &[usize]) -> core::Result<Vec<TaskbarApp>> {
    let mut apps: Vec<TaskbarApp> = Vec::new();

    // Get the current foreground window's PID to mark the active app
    let foreground_pid = unsafe {
        let fw = GetForegroundWindow();
        if fw.is_invalid() {
            0
        } else {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(fw, Some(&mut pid));
            pid
        }
    };

    // 1. Pinned shortcuts
    if let Some(base) = std::env::var_os("APPDATA") {
        let pinned_dir = std::path::PathBuf::from(&base)
            .join("Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar");

        if let Ok(entries) = std::fs::read_dir(&pinned_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("lnk") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                        .to_string();

                    let icon = resolve_lnk_icon(&path)
                        .or_else(|| extract_icon_for_path(&path.to_string_lossy()));

                    apps.push(TaskbarApp {
                        id: format!("pin:{}", name),
                        name,
                        icon_data_url: icon,
                        running: false,
                        pinned: true,
                        is_foreground: false,
                        blacklisted_windows: None,
                    });
                }
            }
        }
    }

    // 2. Running windowed apps
    let running = scan_running_windows(blacklist)?;
    for r in running {
        if !apps.iter().any(|a| a.name.eq_ignore_ascii_case(&r.name)) {
            apps.push(r);
        }
    }

    // Mark foreground: any running app whose PID matches the foreground window's PID
    if foreground_pid != 0 {
        for app in apps.iter_mut() {
            if app.running {
                if let Some(hwnd_val) = app.id.strip_prefix("run:").and_then(|s| s.parse::<isize>().ok()) {
                    let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                    let mut pid: u32 = 0;
                    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); }
                    if pid == foreground_pid {
                        app.is_foreground = true;
                    }
                }
            }
        }
    }

    Ok(apps)
}

fn resolve_lnk_icon(lnk_path: &Path) -> Option<String> {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let result = unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = shell_link.cast().ok()?;

        let wide: Vec<u16> = OsStr::new(lnk_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        persist.Load(PCWSTR(wide.as_ptr()), STGM::default()).ok()?;

        // GetPath on IShellLinkW: (pszfile buffer, *mut WIN32_FIND_DATAW, fflags)
        let mut target_buf = [0u16; 260];
        let mut find_data = WIN32_FIND_DATAW::default();
        shell_link
            .GetPath(&mut target_buf, &mut find_data as *mut _ as *mut _, 0)
            .ok()?;

        let target_len = target_buf.iter().position(|&c| c == 0).unwrap_or(0);
        let target = String::from_utf16_lossy(&target_buf[..target_len]);
        extract_icon_for_path(&target)
    };

    unsafe { CoUninitialize() };
    result.or_else(|| extract_icon_for_path(&lnk_path.to_string_lossy()))
}

struct RunningWindow {
    name: String,
    icon: Option<String>,
    hwnd: isize,
}

fn scan_running_windows(blacklist: &[usize]) -> core::Result<Vec<TaskbarApp>> {
    let state = ScanState {
        windows: Vec::new(),
        blacklist: blacklist.to_vec(),
    };
    let state_ptr: *mut ScanState = Box::into_raw(Box::new(state));
    let lparam = LPARAM(state_ptr as isize);

    unsafe {
        let _ = EnumWindows(Some(enum_proc), lparam);
    }

    let state = unsafe { Box::from_raw(state_ptr) };

    let mut apps: Vec<TaskbarApp> = Vec::new();
    for w in state.windows.iter() {
        apps.push(TaskbarApp {
            id: format!("run:{}", w.hwnd),
            name: w.name.clone(),
            icon_data_url: w.icon.clone(),
            running: true,
            pinned: false,
            is_foreground: false,
            blacklisted_windows: None,
        });
    }

    Ok(apps)
}

struct ScanState {
    windows: Vec<RunningWindow>,
    blacklist: Vec<usize>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut ScanState);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip blacklisted HWNDs
    let hwnd_usize = hwnd.0 as usize;
    if state.blacklist.contains(&hwnd_usize) {
        return BOOL(1);
    }

    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    let has_tool = (ex_style & (WS_EX_TOOLWINDOW.0 as isize)) != 0;
    let has_app = (ex_style & (WS_EX_APPWINDOW.0 as isize)) != 0;
    if has_tool && !has_app {
        return BOOL(1);
    }

    let mut title = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut title);
    if len == 0 {
        return BOOL(1);
    }
    let title_str = String::from_utf16_lossy(&title[..len as usize]);
    if title_str.trim().is_empty() {
        return BOOL(1);
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let exe_path = get_process_exe(pid);
    let icon = exe_path.as_deref().and_then(extract_icon_for_path);

    state.windows.push(RunningWindow {
        name: title_str,
        icon,
        hwnd: hwnd.0 as isize,
    });

    BOOL(1)
}

fn get_process_exe(pid: u32) -> Option<String> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snap, &mut entry).is_err() {
            let _ = windows::Win32::Foundation::CloseHandle(snap);
            return None;
        }

        loop {
            if entry.th32ProcessID == pid {
                let exe_len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe_name = String::from_utf16_lossy(&entry.szExeFile[..exe_len]);
                let _ = windows::Win32::Foundation::CloseHandle(snap);
                return resolve_process_full_path(pid)
                    .or(Some(format!("C:\\Windows\\System32\\{}", exe_name)));
            }
            if Process32NextW(snap, &mut entry).is_err() {
                break;
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        None
    }
}

fn resolve_process_full_path(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn activate_or_launch(app_id: &str, origin: Option<super::animate::OriginRect>) -> core::Result<()> {
    if let Some(name) = app_id.strip_prefix("pin:") {
        if let Some(base) = std::env::var_os("APPDATA") {
            let p = std::path::PathBuf::from(&base)
                .join("Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar")
                .join(format!("{}.lnk", name));
            if p.exists() {
                return crate::win32::shell::shell_execute(&p.to_string_lossy());
            }
        }
        return Err(core::Error::new(
            HRESULT(-1),
            format!("pinned shortcut not found: {}", name),
        ));
    }

    if let Some(hwnd_str) = app_id.strip_prefix("run:") {
        let hwnd_val: isize = hwnd_str.parse().unwrap_or(0);
        if hwnd_val == 0 {
            return Err(core::Error::new(HRESULT(-1), "bad HWND"));
        }
        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                IsIconic, ShowWindowAsync, SW_RESTORE, SetForegroundWindow,
                AllowSetForegroundWindow,
            };
            let _ = AllowSetForegroundWindow(ASFW_ANY_VALUE);
            if IsIconic(hwnd).as_bool() {
                // Minimized — grow the window back out of its taskbar button
                // (pop-out animation); fall back to a plain restore when the
                // animation is unavailable.
                if let Some(ori) = origin {
                    if super::animate::restore_animated(hwnd_val, ori) {
                        return Ok(());
                    }
                }
                let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
        return Ok(());
    }

    Err(core::Error::new(HRESULT(-1), "unknown app id"))
}

// ASFW_ANY is `u32 = 4294967295`. Wrap it for the AllowSetForegroundWindow signature.
const ASFW_ANY_VALUE: u32 = 0xFFFFFFFF;

pub fn show_context_menu(app_id: &str, x: i32, y: i32) -> core::Result<()> {
    log::info!("context_menu for {} @ ({}, {})", app_id, x, y);
    Ok(())
}
