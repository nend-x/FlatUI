#![cfg(windows)]
// Icon extraction utilities — convert HICON to PNG base64 data URL.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::*;
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON, GetIconInfo, ICONINFO};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, GetObjectW, GetDC, ReleaseDC,
    DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BITMAP,
    DIB_RGB_COLORS, BI_RGB, RGBQUAD,
};
use image::ImageEncoder;
use base64::Engine;

/// Extract icon for a file path. Returns PNG data URL (base64) if found.
pub fn extract_icon_for_path(path: &str) -> Option<String> {
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut shfi = SHFILEINFOW::default();
        let flags = SHGFI_ICON | SHGFI_LARGEICON;
        let res = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );

        if res == 0 || shfi.hIcon.is_invalid() {
            return None;
        }

        let result = hicon_to_png(shfi.hIcon);
        let _ = DestroyIcon(shfi.hIcon);
        result
    }
}

/// Convert an HICON to PNG data URL (base64).
pub fn hicon_to_png(hicon: HICON) -> Option<String> {
    unsafe {
        let mut icon_info = ICONINFO::default();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            return None;
        }

        let mut bmp = BITMAP::default();
        let bmp_handle = if !icon_info.hbmColor.is_invalid() {
            icon_info.hbmColor
        } else {
            icon_info.hbmMask
        };

        if GetObjectW(
            bmp_handle.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut _ as *mut _),
        ) == 0
        {
            let _ = DeleteObject(icon_info.hbmColor.into());
            let _ = DeleteObject(icon_info.hbmMask.into());
            return None;
        }

        let width = bmp.bmWidth;
        let height = if icon_info.hbmColor.is_invalid() {
            bmp.bmHeight / 2
        } else {
            bmp.bmHeight
        };

        if width == 0 || height == 0 {
            let _ = DeleteObject(icon_info.hbmColor.into());
            let _ = DeleteObject(icon_info.hbmMask.into());
            return None;
        }

        let dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(dc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
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

        let mut pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];
        let copied = GetDIBits(
            mem_dc,
            bmp_handle.into(),
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &bmi as *const _ as *mut _,
            DIB_RGB_COLORS,
        );

        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, dc);
        let _ = DeleteObject(icon_info.hbmColor.into());
        let _ = DeleteObject(icon_info.hbmMask.into());

        if copied == 0 {
            return None;
        }

        // Convert BGRA → RGBA
        for chunk in pixels.chunks_mut(4) {
            let b = chunk[0];
            chunk[0] = chunk[2];
            chunk[2] = b;
        }

        let img = image::RgbaImage::from_raw(width as u32, height as u32, pixels)?;
        let mut buf = std::io::Cursor::new(Vec::new());
        let png_enc = image::codecs::png::PngEncoder::new(&mut buf);
        png_enc
            .write_image(&img, width as u32, height as u32, image::ExtendedColorType::Rgba8)
            .ok()?;

        let b64 = base64::engine::general_purpose::STANDARD
            .encode(buf.into_inner());
        Some(format!("data:image/png;base64,{}", b64))
    }
}
