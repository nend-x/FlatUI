#![cfg(windows)]
// Detect if foreground window is in full-screen mode.
//
// Simplified heuristic: if the foreground window's rect covers the entire
// monitor, we treat it as fullscreen (regardless of window styles).
// This catches games, video players, and maximized apps that go edge-to-edge.
//
// We still skip:
//   - explorer.exe (desktop shell)
//   - flatui's own windows
//   - already-minimized windows

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, IsIconic,
    GetWindowThreadProcessId,
};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::System::Threading::{QueryFullProcessImageNameW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT};
use windows::Win32::Foundation::CloseHandle;

/// Returns true if the foreground window covers the entire monitor.
pub fn is_foreground_fullscreen() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }

        // Minimized window is never fullscreen
        if IsIconic(hwnd).as_bool() {
            return false;
        }

        // Skip desktop / shell host processes (explorer.exe, etc.)
        // Also skip our own process so we don't hide the taskbar when
        // the taskbar/launcher is foreground.
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return false;
        }

        // Check own PID
        let own_pid = std::process::id();
        if pid == own_pid {
            return false;
        }

        if let Some(exe_path) = resolve_process_full_path(pid) {
            let lower = exe_path.to_lowercase();
            // Skip shell components
            if lower.ends_with("explorer.exe")
                || lower.ends_with("textinputhost.exe")
                || lower.ends_with("searchhost.exe")
                || lower.ends_with("startmenuexperiencehost.exe")
                || lower.ends_with("shellexperiencehost.exe")
                || lower.ends_with("applicationframehost.exe")
            {
                return false;
            }
        }

        // Check if window covers the entire monitor
        let mut wnd_rect = RECT::default();
        if GetWindowRect(hwnd, &mut wnd_rect).is_err() {
            return false;
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO::default();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return false;
        }

        let mon = mi.rcMonitor;
        // Use a small tolerance (2px) to handle DPI scaling edge cases
        wnd_rect.left <= mon.left + 2
            && wnd_rect.right >= mon.right - 2
            && wnd_rect.top <= mon.top + 2
            && wnd_rect.bottom >= mon.bottom - 2
    }
}

fn resolve_process_full_path(pid: u32) -> Option<String> {
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
