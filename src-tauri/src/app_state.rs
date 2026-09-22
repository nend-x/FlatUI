// AppState — shared mutable state

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct TaskbarApp {
    pub id: String,
    pub name: String,
    #[serde(rename = "icon_data_url")]
    pub icon_data_url: Option<String>,
    pub running: bool,
    pub pinned: bool,
    /// True if this app currently has the foreground (active) window
    pub is_foreground: bool,
    /// HWNDs of windows from this app that are blacklisted
    /// (won't appear in window switcher or taskbar previews)
    #[serde(skip_serializing, skip_deserializing)]
    pub blacklisted_windows: Option<Vec<usize>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DesktopItem {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "icon_data_url")]
    pub icon_data_url: Option<String>,
    pub is_folder: bool,
}

pub struct AppState {
    pub taskbar_apps: Vec<TaskbarApp>,
    pub desktop_items: Vec<DesktopItem>,
    /// Window blacklist — identified by PERMANENT traits (title + exe_path),
    pub blacklisted: Vec<BlacklistEntry>,
    pub blacklisted_hwnds: Vec<usize>,
    pub app_order: Vec<String>,
    /// IDs of apps that the user has just launched and is waiting for
    /// the corresponding window to appear. When a new window matches the
    /// launching app, we emit "app-ready://<id>" and remove from this list.
    pub pending_launches: Vec<(String, std::time::Instant)>,
}

/// A persistent blacklist entry — matches by exe_path (permanent).
/// Title is optional (for display). HWND is the live window handle (transient).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BlacklistEntry {
    /// Process exe path — the permanent identifier
    pub exe_path: String,
    /// Window title (optional, for display only)
    pub title: Option<String>,
    /// Live HWND (set on each scan; not persisted)
    #[serde(skip)]
    pub hwnd: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            taskbar_apps: Vec::new(),
            desktop_items: Vec::new(),
            blacklisted: Vec::new(),
            blacklisted_hwnds: Vec::new(),
            app_order: Vec::new(),
            pending_launches: Vec::new(),
        }
    }
}
