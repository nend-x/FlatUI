// Persistence — load/save config files
//
// Config path (FlatUI Hush): %LOCALAPPDATA%\FlatUIHush\
//   blacklist.json, clipboard.json, notes.txt, widgets.json,
//   widget_visibility.json, icon_recolor.json, settings.json,
//   themes.json, tables.json
//
// (FlatUI pre-0.2 used %LOCALAPPDATA%\FlatUI — the rename to FlatUI Hush
// ships a fresh config directory; no migration for the pre-release.)

use std::path::PathBuf;
use std::fs;
use crate::app_state::BlacklistEntry;

/// Root config directory. Pub because lib.rs's reset_config (-rs flag) walks
/// the same directory.
pub fn data_dir() -> PathBuf {
    // Always use LOCALAPPDATA — no admin required, user-specific
    if let Some(lad) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(&lad).join("FlatUIHush");
        let _ = fs::create_dir_all(&dir);
        return dir;
    }
    if let Some(pd) = std::env::var_os("PROGRAMDATA") {
        let dir = PathBuf::from(&pd).join("FlatUIHush");
        let _ = fs::create_dir_all(&dir);
        return dir;
    }
    PathBuf::from(".")
}

fn blacklist_path() -> PathBuf {
    data_dir().join("blacklist.json")
}

pub fn load_blacklist() -> Vec<BlacklistEntry> {
    let path = blacklist_path();
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_blacklist(entries: &[BlacklistEntry]) {
    let path = blacklist_path();
    if let Ok(s) = serde_json::to_string_pretty(entries) {
        let _ = fs::write(&path, s);
    }
}

// ===== Clipboard history =====
pub fn load_clipboard() -> Vec<String> {
    let path = data_dir().join("clipboard.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_clipboard(items: &[String]) {
    let path = data_dir().join("clipboard.json");
    if let Ok(s) = serde_json::to_string_pretty(items) {
        let _ = fs::write(&path, s);
    }
}

// ===== Notes =====
pub fn load_notes() -> String {
    let path = data_dir().join("notes.txt");
    fs::read_to_string(&path).unwrap_or_default()
}

pub fn save_notes(text: &str) {
    let path = data_dir().join("notes.txt");
    let _ = fs::write(&path, text);
}

// ===== Widget positions =====
pub fn load_widget_positions() -> std::collections::HashMap<String, (f64, f64)> {
    let path = data_dir().join("widgets.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => std::collections::HashMap::new(),
    }
}

pub fn save_widget_positions(positions: &std::collections::HashMap<String, (f64, f64)>) {
    let path = data_dir().join("widgets.json");
    if let Ok(s) = serde_json::to_string_pretty(positions) {
        let _ = fs::write(&path, s);
    }
}

// ===== Widget visibility =====
pub fn load_widget_visibility() -> std::collections::HashMap<String, bool> {
    let path = data_dir().join("widget_visibility.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => std::collections::HashMap::new(),
    }
}

pub fn save_widget_visibility(visibility: &std::collections::HashMap<String, bool>) {
    let path = data_dir().join("widget_visibility.json");
    if let Ok(s) = serde_json::to_string_pretty(visibility) {
        let _ = fs::write(&path, s);
    }
}

// ===== Icon recolor =====
pub fn load_icon_recolor() -> bool {
    let path = data_dir().join("icon_recolor.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<bool>(&s).unwrap_or(false),
        Err(_) => false,
    }
}

pub fn save_icon_recolor(enabled: bool) {
    let path = data_dir().join("icon_recolor.json");
    if let Ok(s) = serde_json::to_string_pretty(&enabled) {
        let _ = fs::write(&path, s);
    }
}

// ===== Settings =====
// Extended for the FlatUI Hush 0.2 tables update. New fields all use
// #[serde(default)] so a settings.json written by an older build (or a
// hand-edited one) still deserializes.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct Settings {
    pub theme: String,
    pub auto_fullscreen: bool,
    pub refresh_interval: u64,
    pub cube_animation: bool,
    // --- New in 0.2 (tables update) ---
    /// How long Win must be held before the radial table picker appears.
    /// A shorter tap still toggles the launcher. 80–1000 ms.
    #[serde(default = "default_tables_hold_ms")]
    pub tables_hold_ms: u64,
    /// macOS-style hover magnification strength for the taskbar table.
    /// 1.0 = off, 1.6 = strong. Applied as the max scale of the hovered icon.
    #[serde(default = "default_table_icon_magnify")]
    pub table_icon_magnify: f64,
    /// Taskbar clock format — true = 24h, false = 12h (segmented control).
    #[serde(default = "default_true")]
    pub clock_24h: bool,
    /// Minimize every open window when the launcher (flatlight) opens.
    #[serde(default = "default_true")]
    pub minimize_on_launcher: bool,
    /// Display name shown in the widgets-table header (text input).
    #[serde(default)]
    pub user_name: String,
    /// Hide the desktop icons inside the launcher grid (toggle).
    #[serde(default = "default_true")]
    pub show_desktop_grid: bool,
}

fn default_tables_hold_ms() -> u64 { 220 }
fn default_table_icon_magnify() -> f64 { 1.45 }
fn default_true() -> bool { true }

pub fn load_settings() -> Settings {
    let path = data_dir().join("settings.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| default_settings()),
        Err(_) => default_settings(),
    }
}

fn default_settings() -> Settings {
    Settings {
        theme: "material3-dark".to_string(),
        auto_fullscreen: true,
        refresh_interval: 2,
        cube_animation: true,
        tables_hold_ms: 220,
        table_icon_magnify: 1.45,
        clock_24h: true,
        minimize_on_launcher: true,
        user_name: String::new(),
        show_desktop_grid: true,
    }
}

pub fn save_settings(settings: &Settings) {
    let path = data_dir().join("settings.json");
    if let Ok(s) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(&path, s);
    }
}

// ===== Tables (the radial-picker windows) =====
// tables.json stores the restored position of the movable tables — the
// settings table and the widgets table. Keys are table names, values are
// [x, y] in PHYSICAL screen pixels (matches Tauri's WindowEvent::Moved
// payload, so saving needs no conversion and multi-monitor coords — which
// can be negative — round-trip exactly).
//
// {
//   "settings": [1420, 260],
//   "widgets":  [320, 180]
// }

pub fn load_table_positions() -> std::collections::HashMap<String, (i32, i32)> {
    let path = data_dir().join("tables.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => std::collections::HashMap::new(),
    }
}

pub fn save_table_positions(positions: &std::collections::HashMap<String, (i32, i32)>) {
    let path = data_dir().join("tables.json");
    if let Ok(s) = serde_json::to_string_pretty(positions) {
        let _ = fs::write(&path, s);
    }
}

// ===== Themes =====
// themes.json format:
// {
//   "themes": [
//     {
//       "name": "sand-cream",
//       "active": true,
//       "colors": {
//         "bg-espresso": "#4B3621",
//         "bg-espresso-deep": "#3A2A1A",
//         "bg-espresso-raised": "#54402D",
//         "bg-espresso-frosted": "rgba(58,42,26,0.55)",
//         "bg-espresso-glass": "rgba(58,42,26,0.65)",
//         "sand": "#C2B280",
//         "sand-bright": "#D4C19C",
//         "sand-dim": "#8A7B5C",
//         "sand-cream": "#EDE4D3",
//         "accent-terracotta": "#B8835A",
//         "accent-caramel": "#D4A574",
//         "accent-soft": "rgba(184,131,90,0.18)",
//         "border-subtle": "rgba(194,178,128,0.08)",
//         "border-strong": "rgba(194,178,128,0.18)",
//         "status-running": "#D4A574",
//         "status-pinned": "#8A7B5C"
//       }
//     },
//     ... other themes
//   ]
// }

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ThemeColors {
    #[serde(rename = "bg-espresso")]
    pub bg_espresso: String,
    #[serde(rename = "bg-espresso-rgb")]
    pub bg_espresso_rgb: String,
    #[serde(rename = "bg-espresso-deep")]
    pub bg_espresso_deep: String,
    #[serde(rename = "bg-espresso-deep-rgb")]
    pub bg_espresso_deep_rgb: String,
    #[serde(rename = "bg-espresso-raised")]
    pub bg_espresso_raised: String,
    #[serde(rename = "bg-espresso-raised-rgb")]
    pub bg_espresso_raised_rgb: String,
    #[serde(rename = "bg-espresso-frosted")]
    pub bg_espresso_frosted: String,
    #[serde(rename = "bg-espresso-glass")]
    pub bg_espresso_glass: String,
    pub sand: String,
    #[serde(rename = "sand-rgb")]
    pub sand_rgb: String,
    #[serde(rename = "sand-bright")]
    pub sand_bright: String,
    #[serde(rename = "sand-bright-rgb")]
    pub sand_bright_rgb: String,
    #[serde(rename = "sand-dim")]
    pub sand_dim: String,
    #[serde(rename = "sand-dim-rgb")]
    pub sand_dim_rgb: String,
    #[serde(rename = "sand-cream")]
    pub sand_cream: String,
    #[serde(rename = "sand-cream-rgb")]
    pub sand_cream_rgb: String,
    #[serde(rename = "accent-terracotta")]
    pub accent_terracotta: String,
    #[serde(rename = "accent-terracotta-rgb")]
    pub accent_terracotta_rgb: String,
    #[serde(rename = "accent-caramel")]
    pub accent_caramel: String,
    #[serde(rename = "accent-caramel-rgb")]
    pub accent_caramel_rgb: String,
    #[serde(rename = "accent-soft")]
    pub accent_soft: String,
    #[serde(rename = "border-subtle")]
    pub border_subtle: String,
    #[serde(rename = "border-strong")]
    pub border_strong: String,
    #[serde(rename = "status-running")]
    pub status_running: String,
    #[serde(rename = "status-pinned")]
    pub status_pinned: String,
    // Danger accent (destructive actions: End task, Delete, Exit) — themed
    // per palette so destructive UI never falls back to a hardcoded red.
    #[serde(rename = "danger")]
    pub danger: String,
    #[serde(rename = "danger-rgb")]
    pub danger_rgb: String,
    // Text colors (calibrated per-theme for readability)
    #[serde(rename = "text-primary")]
    pub text_primary: String,
    #[serde(rename = "text-secondary")]
    pub text_secondary: String,
    #[serde(rename = "text-muted")]
    pub text_muted: String,
    // Shadows (calibrated per-theme — light themes use softer shadows)
    #[serde(rename = "shadow-window")]
    pub shadow_window: String,
    #[serde(rename = "shadow-popup")]
    pub shadow_popup: String,
    #[serde(rename = "shadow-icon-hover")]
    pub shadow_icon_hover: String,
    #[serde(rename = "shadow-card")]
    pub shadow_card: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Theme {
    pub name: String,
    pub active: bool,
    pub colors: ThemeColors,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct ThemesConfig {
    pub themes: Vec<Theme>,
}

pub fn default_themes() -> ThemesConfig {
    ThemesConfig {
        themes: vec![
            // ===== material3-dark (active) — pure neutral gray =====
            // Flat gray surfaces (no hue tint), gray text ramp, a neutral
            // gray accent — NOT the M3 baseline purple. Fully opaque
            // surfaces — no blur, no frost, no grain.
            Theme {
                name: "material3-dark".to_string(),
                active: true,
                colors: ThemeColors {
                    // Surfaces — neutral gray: surface / lowest / high
                    bg_espresso: "#1C1C1C".to_string(),
                    bg_espresso_rgb: "28, 28, 28".to_string(),
                    bg_espresso_deep: "#131313".to_string(),
                    bg_espresso_deep_rgb: "19, 19, 19".to_string(),
                    bg_espresso_raised: "#2A2A2A".to_string(),
                    bg_espresso_raised_rgb: "42, 42, 42".to_string(),
                    // Opaque — this theme has NO blur/frosting
                    bg_espresso_frosted: "#1C1C1C".to_string(),
                    bg_espresso_glass: "#1C1C1C".to_string(),
                    // Gray ramp — outline / variant / on-surface
                    sand: "#8A8A8A".to_string(),
                    sand_rgb: "138, 138, 138".to_string(),
                    sand_bright: "#C6C6C6".to_string(),
                    sand_bright_rgb: "198, 198, 198".to_string(),
                    sand_dim: "#4A4A4A".to_string(),
                    sand_dim_rgb: "74, 74, 74".to_string(),
                    sand_cream: "#E8E8E8".to_string(),
                    sand_cream_rgb: "232, 232, 232".to_string(),
                    // Accents — neutral gray (hover/selection states)
                    accent_terracotta: "#ABABAB".to_string(),
                    accent_terracotta_rgb: "171, 171, 171".to_string(),
                    accent_caramel: "#C4C4C4".to_string(),
                    accent_caramel_rgb: "196, 196, 196".to_string(),
                    accent_soft: "rgba(171,171,171,0.14)".to_string(),
                    border_subtle: "rgba(232,232,232,0.06)".to_string(),
                    border_strong: "rgba(232,232,232,0.16)".to_string(),
                    status_running: "#C4C4C4".to_string(),
                    status_pinned: "#6E6E6E".to_string(),
                    danger: "#C97A74".to_string(),
                    danger_rgb: "201, 122, 116".to_string(),
                    // Text — primary / secondary / muted grays
                    text_primary: "#E8E8E8".to_string(),
                    text_secondary: "#C0C0C0".to_string(),
                    text_muted: "#8A8A8A".to_string(),
                    // Shadows — soft, tight elevation shadows
                    shadow_window: "0 1px 3px rgba(0, 0, 0, 0.24)".to_string(),
                    shadow_popup: "0 1px 3px rgba(0, 0, 0, 0.30), 0 4px 10px rgba(0, 0, 0, 0.25)".to_string(),
                    shadow_icon_hover: "0 1px 2px rgba(0, 0, 0, 0.20)".to_string(),
                    shadow_card: "0 8px 24px rgba(0, 0, 0, 0.45)".to_string(),
                },
            },
            /* ===== Old themes — disabled, kept for reference =====
            Theme {
                name: "sand-cream".to_string(),
                active: true,
                colors: ThemeColors {
                    bg_espresso: "#4B3621".to_string(),
                    bg_espresso_rgb: "75, 54, 33".to_string(),
                    bg_espresso_deep: "#3A2A1A".to_string(),
                    bg_espresso_deep_rgb: "58, 42, 26".to_string(),
                    bg_espresso_raised: "#54402D".to_string(),
                    bg_espresso_raised_rgb: "84, 64, 40".to_string(),
                    bg_espresso_frosted: "rgba(58,42,26,0.55)".to_string(),
                    bg_espresso_glass: "rgba(58,42,26,0.65)".to_string(),
                    sand: "#C2B280".to_string(),
                    sand_rgb: "194, 178, 128".to_string(),
                    sand_bright: "#D4C19C".to_string(),
                    sand_bright_rgb: "212, 193, 156".to_string(),
                    sand_dim: "#8A7B5C".to_string(),
                    sand_dim_rgb: "138, 123, 92".to_string(),
                    sand_cream: "#EDE4D3".to_string(),
                    sand_cream_rgb: "237, 228, 211".to_string(),
                    accent_terracotta: "#B8835A".to_string(),
                    accent_terracotta_rgb: "184, 131, 90".to_string(),
                    accent_caramel: "#D4A574".to_string(),
                    accent_caramel_rgb: "212, 165, 116".to_string(),
                    accent_soft: "rgba(184,131,90,0.18)".to_string(),
                    border_subtle: "rgba(194,178,128,0.08)".to_string(),
                    border_strong: "rgba(194,178,128,0.18)".to_string(),
                    status_running: "#D4A574".to_string(),
                    status_pinned: "#8A7B5C".to_string(),
                    danger: "#C4664F".to_string(),
                    danger_rgb: "196, 102, 79".to_string(),
                    // Text — sand-cream theme: light text on dark espresso bg
                    text_primary: "#EDE4D3".to_string(),
                    text_secondary: "#C2B280".to_string(),
                    text_muted: "#8A7B5C".to_string(),
                    // Shadows — warm dark shadows for the espresso theme
                    shadow_window: "0 1px 4px rgba(0, 0, 0, 0.18)".to_string(),
                    shadow_popup: "0 2px 12px rgba(0, 0, 0, 0.32), 0 1px 2px rgba(0, 0, 0, 0.22)".to_string(),
                    shadow_icon_hover: "0 1px 3px rgba(0, 0, 0, 0.14)".to_string(),
                    shadow_card: "0 14px 44px rgba(0, 0, 0, 0.5)".to_string(),
                },
            },
            Theme {
                name: "earthly-green".to_string(),
                active: false,
                colors: ThemeColors {
                    bg_espresso: "#113B1F".to_string(),
                    bg_espresso_rgb: "17, 59, 31".to_string(),
                    bg_espresso_deep: "#0F2D19".to_string(),
                    bg_espresso_deep_rgb: "15, 45, 25".to_string(),
                    bg_espresso_raised: "#1E472C".to_string(),
                    bg_espresso_raised_rgb: "30, 71, 44".to_string(),
                    bg_espresso_frosted: "rgba(15,45,25,0.55)".to_string(),
                    bg_espresso_glass: "rgba(15,45,25,0.65)".to_string(),
                    sand: "#64B47E".to_string(),
                    sand_rgb: "100, 180, 126".to_string(),
                    sand_bright: "#7DCD98".to_string(),
                    sand_bright_rgb: "125, 205, 152".to_string(),
                    sand_dim: "#4C7F5D".to_string(),
                    sand_dim_rgb: "76, 127, 93".to_string(),
                    sand_cream: "#CDE4D4".to_string(),
                    sand_cream_rgb: "205, 228, 212".to_string(),
                    accent_terracotta: "#39AC5F".to_string(),
                    accent_terracotta_rgb: "57, 172, 95".to_string(),
                    accent_caramel: "#69BE85".to_string(),
                    accent_caramel_rgb: "105, 190, 133".to_string(),
                    accent_soft: "rgba(57,172,95,0.18)".to_string(),
                    border_subtle: "rgba(205,228,212,0.08)".to_string(),
                    border_strong: "rgba(205,228,212,0.18)".to_string(),
                    status_running: "#69BE85".to_string(),
                    status_pinned: "#4C7F5D".to_string(),
                    danger: "#C96F5F".to_string(),
                    danger_rgb: "201, 111, 95".to_string(),
                    // Text — dark forest green theme: light mint text on dark green bg
                    text_primary: "#CDE4D4".to_string(),
                    text_secondary: "#64B47E".to_string(),
                    text_muted: "#4C7F5D".to_string(),
                    // Shadows — dark forest shadows
                    shadow_window: "0 1px 4px rgba(0, 0, 0, 0.18)".to_string(),
                    shadow_popup: "0 2px 12px rgba(0, 0, 0, 0.32), 0 1px 2px rgba(0, 0, 0, 0.22)".to_string(),
                    shadow_icon_hover: "0 1px 3px rgba(0, 0, 0, 0.14)".to_string(),
                    shadow_card: "0 14px 44px rgba(0, 0, 0, 0.5)".to_string(),
                },
            },
            Theme {
                name: "silver-lining".to_string(),
                active: false,
                colors: ThemeColors {
                    bg_espresso: "#293037".to_string(),
                    bg_espresso_rgb: "41, 48, 55".to_string(),
                    bg_espresso_deep: "#23282E".to_string(),
                    bg_espresso_deep_rgb: "35, 40, 46".to_string(),
                    bg_espresso_raised: "#393F46".to_string(),
                    bg_espresso_raised_rgb: "57, 63, 70".to_string(),
                    bg_espresso_frosted: "rgba(35,40,46,0.55)".to_string(),
                    bg_espresso_glass: "rgba(35,40,46,0.65)".to_string(),
                    sand: "#95A0AB".to_string(),
                    sand_rgb: "149, 160, 171".to_string(),
                    sand_bright: "#AAB7C4".to_string(),
                    sand_bright_rgb: "170, 183, 196".to_string(),
                    sand_dim: "#69727B".to_string(),
                    sand_dim_rgb: "105, 114, 123".to_string(),
                    sand_cream: "#D8DDE2".to_string(),
                    sand_cream_rgb: "216, 221, 226".to_string(),
                    accent_terracotta: "#74899E".to_string(),
                    accent_terracotta_rgb: "116, 137, 158".to_string(),
                    accent_caramel: "#92A5B9".to_string(),
                    accent_caramel_rgb: "146, 165, 185".to_string(),
                    accent_soft: "rgba(116,137,158,0.18)".to_string(),
                    border_subtle: "rgba(216,221,226,0.08)".to_string(),
                    border_strong: "rgba(216,221,226,0.18)".to_string(),
                    status_running: "#92A5B9".to_string(),
                    status_pinned: "#69727B".to_string(),
                    danger: "#A86E78".to_string(),
                    danger_rgb: "168, 110, 120".to_string(),
                    // Text — silver-lining theme: bright cool-gray text on dark
                    // gray bg. Brighter than sand-cream to ensure readability.
                    text_primary: "#F0F4F8".to_string(),
                    text_secondary: "#B8C4D0".to_string(),
                    text_muted: "#8A96A4".to_string(),
                    // Shadows — SOFTER shadows for the light/cool theme.
                    // Heavy black shadows ruin the silver-lining aesthetic,
                    // so we use lower opacity and blur.
                    shadow_window: "0 1px 3px rgba(0, 0, 0, 0.10)".to_string(),
                    shadow_popup: "0 2px 8px rgba(0, 0, 0, 0.18), 0 1px 2px rgba(0, 0, 0, 0.12)".to_string(),
                    shadow_icon_hover: "0 1px 2px rgba(0, 0, 0, 0.10)".to_string(),
                    shadow_card: "0 10px 30px rgba(0, 0, 0, 0.30)".to_string(),
                },
            },
            ===== end old themes ===== */
        ],
    }
}

pub fn load_themes() -> ThemesConfig {
    let path = data_dir().join("themes.json");
    let defaults = default_themes();
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<ThemesConfig>(&s) {
            Ok(config) => {
                // Migration: a pre-material3-dark themes.json still holds the
                // retired themes. Once the file contains the new default set
                // (detected by the presence of material3-dark) user edits are
                // preserved; older files are rewritten with the defaults.
                if !config.themes.iter().any(|t| t.name == "material3-dark") {
                    log::info!(
                        "themes.json predates material3-dark — rewriting with defaults"
                    );
                    if let Ok(s) = serde_json::to_string_pretty(&defaults) {
                        let _ = fs::write(&path, s);
                    }
                    return defaults;
                }
                config
            }
            Err(_) => {
                // File exists but is old format or corrupt — overwrite with
                // the current defaults so future loads succeed.
                log::warn!("themes.json was old format or corrupt — rewriting with defaults");
                if let Ok(s) = serde_json::to_string_pretty(&defaults) {
                    let _ = fs::write(&path, s);
                }
                defaults
            }
        },
        Err(_) => {
            // First run — create default themes file
            if let Ok(s) = serde_json::to_string_pretty(&defaults) {
                let _ = fs::write(&path, s);
            }
            defaults
        }
    }
}

pub fn save_themes(config: &ThemesConfig) {
    let path = data_dir().join("themes.json");
    if let Ok(s) = serde_json::to_string_pretty(config) {
        let _ = fs::write(&path, s);
    }
}

pub fn get_active_theme() -> Option<Theme> {
    let config = load_themes();
    config.themes.into_iter().find(|t| t.active)
}

pub fn set_active_theme(name: &str) {
    let mut config = load_themes();
    for theme in &mut config.themes {
        theme.active = theme.name == name;
    }
    save_themes(&config);
}

