//! Windows: a layered (per-pixel alpha), topmost, transparent (click-through)
//! tool window that never takes focus. In edit mode it stops being
//! transparent and captures the mouse while dragged.

use anyhow::{Context, Result};
use tiny_skia::Pixmap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW,
    HWND_TOPMOST, IDC_SIZEALL, LoadCursorW, MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

use super::{Backend, Pointer, corner, render::to_bgra};

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub struct WinOverlay {
    hwnd: HWND,
    /// Where the window was last placed: pointer input is measured from it.
    position: (i32, i32),
    /// A drag holds the mouse capture.
    dragging: bool,
}

impl WinOverlay {
    pub fn new() -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?.into();
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: w!("FluffyFrameOverlay"),
                // Only seen in edit mode: everywhere else clicks pass through.
                hCursor: LoadCursorW(None, IDC_SIZEALL).unwrap_or_default(),
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
            Ok(Self { hwnd, position: (0, 0), dragging: false })
        }
    }
}

impl Backend for WinOverlay {
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap, right: i32, top: i32) -> Result<()> {
        let monitors = crate::monitors::monitors()?;
        let target = crate::pick(&monitors, monitor).context("no monitor")?;
        let (x, y) = corner(target, pixels.width(), right, top);
        self.position = (x, y);
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

    fn set_interactive(&mut self, on: bool) -> Result<()> {
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let transparent = WS_EX_TRANSPARENT.0 as isize;
            let style = if on { style & !transparent } else { style | transparent };
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, style);
            if !on && self.dragging {
                let _ = ReleaseCapture();
                self.dragging = false;
            }
        }
        Ok(())
    }

    fn pump(&mut self) -> Result<Vec<Pointer>> {
        let mut input = Vec::new();
        unsafe {
            // Screen coordinates are the fixed frame; panel ones only at the press.
            let cursor = || {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                (f64::from(p.x), f64::from(p.y))
            };
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.hwnd == self.hwnd {
                    match msg.message {
                        WM_LBUTTONDOWN => {
                            SetCapture(self.hwnd);
                            self.dragging = true;
                            let at = cursor();
                            let local = (at.0 - f64::from(self.position.0), at.1 - f64::from(self.position.1));
                            input.push(Pointer::Press { local, at });
                        }
                        WM_MOUSEMOVE if self.dragging => input.push(Pointer::Motion { at: cursor() }),
                        WM_LBUTTONUP if self.dragging => {
                            let _ = ReleaseCapture();
                            self.dragging = false;
                            input.push(Pointer::Release);
                        }
                        _ => {}
                    }
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(input)
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
