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

// Note: Win-key tap handling no longer lives in this module. It used to be
// implemented two ways (a native LL keyboard hook in `hotkey.rs` and an
// external `flatwin.exe` AutoHotkey v2 helper that POSTed to a local HTTP
// server), but both have been replaced by the `prevent-alt-win-menu` crate,
// which is wired up in lib.rs. That crate installs its own low-level keyboard
// hook in-process and invokes our `on_released` callback when the Win key is
// released alone (a "tap") — we both suppress the Start menu and toggle the
// launcher from that callback.
