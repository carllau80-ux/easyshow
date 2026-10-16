//! Windows 资源管理器**原生图标**。
//!
//! 用 `SHGetFileInfoW` 从 shell 提取「此电脑 / 每个盘符 / 普通文件夹」的系统图标
//! (与资源管理器显示的一致),转换成 egui 纹理供目录树绘制。
//! 非 Windows 平台为空实现,目录树回退到图标字体的字形图标。

use eframe::egui;

use crate::tree::NodeIcon;

#[derive(Default)]
pub struct ShellIcons {
    this_pc: Option<egui::TextureHandle>,
    folder: Option<egui::TextureHandle>,
    drives: std::collections::HashMap<char, egui::TextureHandle>,
}

impl ShellIcons {
    pub fn texture_for(&self, icon: NodeIcon) -> Option<&egui::TextureHandle> {
        match icon {
            NodeIcon::ThisPc => self.this_pc.as_ref(),
            NodeIcon::Folder => self.folder.as_ref(),
            NodeIcon::Drive(letter) => self.drives.get(&letter),
            NodeIcon::Glyph(_) => None,
        }
    }
}

/// 载入全部系统图标(仅 Windows;失败/非 Windows 返回 None → 字形回退)。
pub fn load(ctx: &egui::Context) -> Option<ShellIcons> {
    load_impl(ctx)
}

#[cfg(not(windows))]
fn load_impl(_ctx: &egui::Context) -> Option<ShellIcons> {
    None
}

#[cfg(windows)]
fn load_impl(ctx: &egui::Context) -> Option<ShellIcons> {
    use std::path::PathBuf;

    let mut icons = ShellIcons::default();

    // 「此电脑」(CLSID)
    icons.this_pc = icon_for_path(ctx, "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}");

    // 普通文件夹图标:用用户主目录里的任意普通目录提取
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                icons.folder = icon_for_path(ctx, &path.to_string_lossy());
                break;
            }
        }
    }

    // 各盘符(固定盘/移动盘/网络盘图标各不相同)
    for letter in 'A'..='Z' {
        let root = format!("{letter}:\\");
        if std::path::Path::new(&root).is_dir() {
            if let Some(texture) = icon_for_path(ctx, &root) {
                icons.drives.insert(letter, texture);
            }
        }
    }

    if icons.this_pc.is_none() && icons.folder.is_none() && icons.drives.is_empty() {
        None
    } else {
        Some(icons)
    }
}

#[cfg(windows)]
fn icon_for_path(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if ok == 0 || info.hIcon.is_null() {
        return None;
    }
    let result = unsafe { hicon_to_rgba(info.hIcon) };
    unsafe { DestroyIcon(info.hIcon) };
    let (w, h, rgba) = result?;
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture("shell-icon", color, egui::TextureOptions::LINEAR))
}

/// HICON → (宽, 高, RGBA 像素)。用 32bpp DIB + DrawIconEx 绘制后读回。
#[cfg(windows)]
unsafe fn hicon_to_rgba(hicon: windows_sys::Win32::UI::WindowsAndMessaging::HICON) -> Option<(u32, u32, Vec<u8>)> {
    use std::ffi::c_void;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetObjectW, ReleaseDC,
        SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{DrawIconEx, GetIconInfo, DI_NORMAL, ICONINFO};

    let mut icon_info: ICONINFO = unsafe { std::mem::zeroed() };
    if unsafe { GetIconInfo(hicon, &mut icon_info) } == 0 {
        return None;
    }
    let cleanup_bitmaps = || unsafe {
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask);
        }
    };

    let mut bitmap: BITMAP = unsafe { std::mem::zeroed() };
    let got = unsafe {
        GetObjectW(
            icon_info.hbmColor,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bitmap as *mut BITMAP as *mut c_void,
        )
    };
    if got == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
        cleanup_bitmaps();
        return None;
    }
    let (w, h) = (bitmap.bmWidth as u32, bitmap.bmHeight as u32);

    let screen = unsafe { GetDC(std::ptr::null_mut()) };
    let mem_dc = unsafe { CreateCompatibleDC(screen) };

    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w as i32;
    bmi.bmiHeader.biHeight = -(h as i32); // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = 0; // BI_RGB

    let mut bits: *mut c_void = std::ptr::null_mut();
    let dib = unsafe {
        CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0)
    };
    let old = unsafe { SelectObject(mem_dc, dib) };
    let drawn = unsafe {
        DrawIconEx(mem_dc, 0, 0, hicon, w as i32, h as i32, 0, std::ptr::null_mut(), DI_NORMAL)
    };

    let mut result = None;
    if drawn != 0 && !bits.is_null() {
        let count = (w * h) as usize;
        let src = unsafe { std::slice::from_raw_parts(bits as *const u8, count * 4) };
        let mut rgba = vec![0u8; count * 4];
        for i in 0..count {
            let (b, g, r, mut a) = (src[i * 4], src[i * 4 + 1], src[i * 4 + 2], src[i * 4 + 3]);
            // 无 alpha 的旧式图标:不透明像素补 alpha
            if a == 0 && (r | g | b) != 0 {
                a = 255;
            }
            rgba[i * 4] = r;
            rgba[i * 4 + 1] = g;
            rgba[i * 4 + 2] = b;
            rgba[i * 4 + 3] = a;
        }
        result = Some((w, h, rgba));
    }

    unsafe {
        SelectObject(mem_dc, old);
        DeleteObject(dib);
        DeleteDC(mem_dc);
        ReleaseDC(std::ptr::null_mut(), screen);
    }
    cleanup_bitmaps();
    result
}
