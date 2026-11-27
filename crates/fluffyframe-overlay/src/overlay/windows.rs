//! Windows: a layered (per-pixel alpha), topmost, transparent (click-through)
//! tool window that never takes focus.

use anyhow::{Context, Result};
use tiny_skia::Pixmap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, HWND_TOPMOST, MSG, PM_REMOVE, PeekMessageW,
    RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow,
    TranslateMessage, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

use super::{Backend, corner, render::to_bgra};

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub struct WinOverlay {
    hwnd: HWND,
}

impl WinOverlay {
    pub fn new() -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?.into();
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: w!("FluffyFrameOverlay"),
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("FluffyFrameOverlay"),
                w!("FluffyFrame overlay"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance),
                None,
            )
            .context("creating the overlay window")?;
            Ok(Self { hwnd })
        }
    }
}

impl Backend for WinOverlay {
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap) -> Result<()> {
        let monitors = crate::monitors::monitors()?;
        let target = crate::pick(&monitors, monitor).context("no monitor")?;
        let (x, y) = corner(target, pixels.width());
        let (w, h) = (pixels.width() as i32, pixels.height() as i32);
        let data = to_bgra(pixels);
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let result = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0).map(|bitmap| {
                std::ptr::copy_nonoverlapping(data.as_ptr(), bits.cast::<u8>(), data.len());
                let old = SelectObject(mem, bitmap.into());
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let shown = update_layered(self.hwnd, screen, mem, (x, y), (w, h), &blend);
                SelectObject(mem, old);
                let _ = DeleteObject(bitmap.into());
                shown
            });
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            result.context("creating the overlay bitmap")??;
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        Ok(())
    }

    fn hide(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn pump(&mut self) -> Result<()> {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }
}

impl Drop for WinOverlay {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// `UpdateLayeredWindow` with the bitmap selected into `mem`.
unsafe fn update_layered(
    hwnd: HWND,
    screen: windows::Win32::Graphics::Gdi::HDC,
    mem: windows::Win32::Graphics::Gdi::HDC,
    (x, y): (i32, i32),
    (w, h): (i32, i32),
    blend: &BLENDFUNCTION,
) -> Result<()> {
    let dst = POINT { x, y };
    let size = SIZE { cx: w, cy: h };
    let src = POINT { x: 0, y: 0 };
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::UpdateLayeredWindow(
            hwnd,
            Some(screen),
            Some(&dst),
            Some(&size),
            Some(mem),
            Some(&src),
            COLORREF(0),
            Some(blend),
            ULW_ALPHA,
        )
    }
    .context("UpdateLayeredWindow")
}
