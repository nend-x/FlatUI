#![cfg(windows)]
// Shell helpers — read Desktop, launch files, create/rename/delete items.

use crate::app_state::DesktopItem;
use crate::win32::icon::extract_icon_for_path;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::{self, PCWSTR, HRESULT};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn scan_desktop() -> core::Result<Vec<DesktopItem>> {
    let desktop = std::env::var("USERPROFILE")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("Desktop"))
        .map_err(|_| core::Error::new(HRESULT(-1), "USERPROFILE not set"))?;

    if !desktop.exists() {
        return Ok(Vec::new());
    }

    let mut items: Vec<DesktopItem> = Vec::new();
    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![desktop];

    if let Some(p) = std::env::var("PUBLIC").ok().map(std::path::PathBuf::from).map(|p| p.join("Desktop")) {
        if p.exists() {
            dirs_to_scan.push(p);
        }
    }

    for dir in dirs_to_scan {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .or_else(|| path.file_name().and_then(|s| s.to_str()))
                    .unwrap_or("?")
                    .to_string();

                if let Ok(meta) = entry.metadata() {
                    use std::os::windows::fs::MetadataExt;
                    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                    if meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
                        continue;
                    }
                }

                let is_folder = path.is_dir();
                let icon = extract_icon_for_path(&path.to_string_lossy());

                items.push(DesktopItem {
                    id: path.to_string_lossy().to_string(),
                    name,
                    path: path.to_string_lossy().to_string(),
                    icon_data_url: icon,
                    is_folder,
                });
            }
        }
    }

    items.sort_by(|a, b| match (a.is_folder, b.is_folder) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(items)
}

pub fn shell_execute(path: &str) -> core::Result<()> {
    let wide_path: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
    let verb: Vec<u16> = OsStr::new("open").encode_wide().chain(std::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(wide_path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    if result.0 as usize <= 32 {
        return Err(core::Error::new(
            HRESULT(-1),
            format!("ShellExecuteW failed: error code {}", result.0 as usize),
        ));
    }
    Ok(())
}

/// Create a new folder or empty file on the Desktop.
pub fn create_item(name: &str, is_folder: bool) -> core::Result<DesktopItem> {
    use std::io::Write;
    let desktop = std::env::var("USERPROFILE")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("Desktop"))
        .map_err(|_| core::Error::new(HRESULT(-1), "USERPROFILE not set"))?;

    // Find a non-colliding name
    let base_path = desktop.join(name);
    let final_path = if base_path.exists() {
        let stem = std::path::Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name);
        let ext = std::path::Path::new(name).extension().and_then(|s| s.to_str());
        let mut idx = 2;
        loop {
            let candidate_name = match ext {
                Some(e) => format!("{} ({}).{}", stem, idx, e),
                None => format!("{} ({})", stem, idx),
            };
            let candidate = desktop.join(&candidate_name);
            if !candidate.exists() {
                break candidate;
            }
            idx += 1;
        }
    } else {
        base_path
    };

    if is_folder {
        std::fs::create_dir(&final_path)
            .map_err(|e| core::Error::new(HRESULT(-1), format!("create_dir: {}", e)))?;
    } else {
        let _ = std::fs::File::create(&final_path)
            .map_err(|e| core::Error::new(HRESULT(-1), format!("create file: {}", e)))?
            .write_all(b"")?;
    }

    let final_name = final_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_string();

    Ok(DesktopItem {
        id: final_path.to_string_lossy().to_string(),
        name: final_name,
        path: final_path.to_string_lossy().to_string(),
        icon_data_url: extract_icon_for_path(&final_path.to_string_lossy()),
        is_folder,
    })
}

pub fn delete_item(item_id: &str) -> core::Result<()> {
    let path = std::path::Path::new(item_id);
    if !path.exists() {
        return Err(core::Error::new(HRESULT(-1), "not found"));
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path)
            .map_err(|e| core::Error::new(HRESULT(-1), format!("remove_dir_all: {}", e)))?;
    } else {
        std::fs::remove_file(path)
            .map_err(|e| core::Error::new(HRESULT(-1), format!("remove_file: {}", e)))?;
    }
    Ok(())
}

pub fn rename_item(item_id: &str, new_name: &str) -> core::Result<DesktopItem> {
    let old_path = std::path::Path::new(item_id);
    if !old_path.exists() {
        return Err(core::Error::new(HRESULT(-1), "not found"));
    }
    let parent = old_path.parent().ok_or_else(|| core::Error::new(HRESULT(-1), "no parent"))?;
    let new_path = parent.join(new_name);

    std::fs::rename(old_path, &new_path)
        .map_err(|e| core::Error::new(HRESULT(-1), format!("rename: {}", e)))?;

    Ok(DesktopItem {
        id: new_path.to_string_lossy().to_string(),
        name: new_name.to_string(),
        path: new_path.to_string_lossy().to_string(),
        icon_data_url: extract_icon_for_path(&new_path.to_string_lossy()),
        is_folder: new_path.is_dir(),
    })
}
