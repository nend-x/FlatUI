// Persistence — load/save blacklist to file
//
// Path: %PROGRAMDATA%\FlatUI\blacklist.json
// (ProgramData is world-readable but requires admin to write)
//
// If we can't write there (no admin), fallback to %LOCALAPPDATA%\FlatUI\blacklist.json

use std::path::PathBuf;
use std::fs;
use crate::app_state::BlacklistEntry;

fn data_dir() -> PathBuf {
    // Always use LOCALAPPDATA — no admin required, user-specific
    if let Some(lad) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(&lad).join("FlatUI");
        let _ = fs::create_dir_all(&dir);
        return dir;
    }
    if let Some(pd) = std::env::var_os("PROGRAMDATA") {
        let dir = PathBuf::from(&pd).join("FlatUI");
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

// ===== Background blur (blur vs grain) =====
pub fn load_background_blur() -> bool {
    let path = data_dir().join("background_blur.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<bool>(&s).unwrap_or(false),
        Err(_) => false,
    }
}

pub fn save_background_blur(enabled: bool) {
    let path = data_dir().join("background_blur.json");
    if let Ok(s) = serde_json::to_string_pretty(&enabled) {
        let _ = fs::write(&path, s);
    }
}

// ===== Settings =====
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct Settings {
    pub theme: String,
    pub auto_fullscreen: bool,
    pub refresh_interval: u64,
    pub cube_animation: bool,
}

pub fn load_settings() -> Settings {
    let path = data_dir().join("settings.json");
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| Settings {
            theme: "sand-cream".to_string(),
            auto_fullscreen: true,
            refresh_interval: 2,
            cube_animation: true,
        }),
        Err(_) => Settings {
            theme: "sand-cream".to_string(),
            auto_fullscreen: true,
            refresh_interval: 2,
            cube_animation: true,
        },
    }
}

pub fn save_settings(settings: &Settings) {
    let path = data_dir().join("settings.json");
    if let Ok(s) = serde_json::to_string_pretty(settings) {
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
                    bg_espresso: "#1C4430".to_string(),
                    bg_espresso_rgb: "28, 68, 48".to_string(),
                    bg_espresso_deep: "#183928".to_string(),
                    bg_espresso_deep_rgb: "24, 57, 40".to_string(),
                    bg_espresso_raised: "#2D523F".to_string(),
                    bg_espresso_raised_rgb: "45, 82, 63".to_string(),
                    bg_espresso_frosted: "rgba(24,57,40,0.55)".to_string(),
                    bg_espresso_glass: "rgba(24,57,40,0.65)".to_string(),
                    sand: "#84BCA0".to_string(),
                    sand_rgb: "132, 188, 160".to_string(),
                    sand_bright: "#98D7B7".to_string(),
                    sand_bright_rgb: "152, 215, 183".to_string(),
                    sand_dim: "#5A8A72".to_string(),
                    sand_dim_rgb: "90, 138, 114".to_string(),
                    sand_cream: "#D1EADD".to_string(),
                    sand_cream_rgb: "209, 234, 221".to_string(),
                    accent_terracotta: "#58BA89".to_string(),
                    accent_terracotta_rgb: "88, 186, 137".to_string(),
                    accent_caramel: "#78D3A5".to_string(),
                    accent_caramel_rgb: "120, 211, 165".to_string(),
                    accent_soft: "rgba(88,186,137,0.18)".to_string(),
                    border_subtle: "rgba(209,234,221,0.08)".to_string(),
                    border_strong: "rgba(209,234,221,0.18)".to_string(),
                    status_running: "#78D3A5".to_string(),
                    status_pinned: "#5A8A72".to_string(),
                    // Text — earthly-green theme: light green text on dark green bg
                    text_primary: "#D1EADD".to_string(),
                    text_secondary: "#84BCA0".to_string(),
                    text_muted: "#5A8A72".to_string(),
                    // Shadows — green-tinted dark shadows
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
        ],
    }
}

pub fn load_themes() -> ThemesConfig {
    let path = data_dir().join("themes.json");
    match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<ThemesConfig>(&s) {
            Ok(config) => config,
            Err(_) => {
                // File exists but is old format or corrupt — overwrite with
                // the current defaults so future loads succeed.
                log::warn!("themes.json was old format or corrupt — rewriting with defaults");
                let defaults = default_themes();
                if let Ok(s) = serde_json::to_string_pretty(&defaults) {
                    let _ = fs::write(&path, s);
                }
                defaults
            }
        },
        Err(_) => {
            // First run — create default themes file
            let defaults = default_themes();
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

