//! Saving a picture of the app.
//!
//! The normal path uses egui's own screenshot support, which reads the pixels
//! back from the graphics device. If that is not available, the window is
//! grabbed with the classic Windows GDI calls instead.

use egui::ColorImage;
use std::sync::atomic::{AtomicIsize, Ordering};

/// Write an egui image to a PNG file.
pub fn save_color_image(image: &ColorImage, path: &str) -> Result<(), String> {
    let width = image.size[0] as u32;
    let height = image.size[1] as u32;
    let mut buffer = image::RgbaImage::new(width, height);
    for (index, pixel) in image.pixels.iter().enumerate() {
        let [r, g, b, a] = pixel.to_array();
        let alpha = a as u32;
        let (r, g, b) = if alpha == 0 {
            (0, 0, 0)
        } else {
            (
                ((r as u32 * 255) / alpha).min(255) as u8,
                ((g as u32 * 255) / alpha).min(255) as u8,
                ((b as u32 * 255) / alpha).min(255) as u8,
            )
        };
        buffer.put_pixel(
            index as u32 % width,
            index as u32 / width,
            image::Rgba([r, g, b, a]),
        );
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    buffer.save(path).map_err(|e| format!("could not save {path}: {e}"))
}

static FOUND_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Grab the app window with GDI and store it as a PNG.
pub fn capture_window_to_png(title: &str, path: &str) -> Result<(), String> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect,
    };

    unsafe {
        let process_id = std::process::id();
        FOUND_WINDOW.store(0, Ordering::SeqCst);
        let _ = EnumWindows(Some(enum_proc), LPARAM(process_id as isize));
        let handle = FOUND_WINDOW.load(Ordering::SeqCst);
        if handle == 0 {
            return Err("could not find the app window".to_string());
        }
        let hwnd = HWND(handle as *mut _);

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return Err("could not measure the window".to_string());
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err("the window has no size".to_string());
        }
        let _ = title;

        let screen_dc = GetDC(None);
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(memory_dc, width, height);
        let old = SelectObject(memory_dc, windows::Win32::Graphics::Gdi::HGDIOBJ(bitmap.0));
        let _ = PrintWindow(hwnd, memory_dc, PRINT_WINDOW_FLAGS(2));

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: (width * height * 4) as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        GetDIBits(
            memory_dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );

        SelectObject(memory_dc, old);
        let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);

        let mut buffer = image::RgbaImage::new(width as u32, height as u32);
        for y in 0..height as u32 {
            for x in 0..width as u32 {
                let offset = ((y * width as u32 + x) * 4) as usize;
                buffer.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        pixels[offset + 2],
                        pixels[offset + 1],
                        pixels[offset],
                        pixels[offset + 3],
                    ]),
                );
            }
        }
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        buffer.save(path).map_err(|e| format!("could not save {path}: {e}"))
    }
}

/// Finds a visible top level window of this process.
unsafe extern "system" fn enum_proc(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::core::BOOL;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};
    let wanted = lparam.0 as u32;
    let mut process = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process));
        if process == wanted && IsWindowVisible(hwnd).as_bool() {
            FOUND_WINDOW.store(hwnd.0 as isize, Ordering::SeqCst);
            return BOOL(0);
        }
    }
    BOOL(1)
}
