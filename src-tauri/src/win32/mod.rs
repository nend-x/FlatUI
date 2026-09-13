#![cfg(windows)]
// Win32 module — only compiled on Windows target

pub mod window;
pub mod apps;
pub mod shell;
pub mod icon;
pub mod peek;
pub mod fullscreen;
pub mod subclass;
pub mod winevent;
// Note: win32/hotkey.rs (native LL keyboard hook) is intentionally NOT
// registered anymore — the Win key is handled by the embedded flatwin.exe
// AHK helper -> HTTP /toggle (http_server.rs). Keeping both would toggle
// the launcher twice per Win press.
