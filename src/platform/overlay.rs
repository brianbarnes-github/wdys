//! The hands-free indicator: a small always-on-top pill at the top centre of the primary screen.
//!
//! It is click-through and never takes focus, so it cannot disturb the app being dictated into.
//! Its dot pulses on a thread timer that the main message loop forwards to [`Overlay::pulse`].

use std::ffi::c_void;
use std::mem::size_of;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, KillTimer, RegisterClassW, SetTimer,
    SetWindowPos, ShowWindow, UpdateLayeredWindow, HWND_TOPMOST, SM_CXSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::app::{TrayState, UiEvent};
use crate::error::PlatformError;
use crate::overlay::{overlay_frame, OVERLAY_HEIGHT, OVERLAY_WIDTH};

const PULSE_MS: u32 = 600;
const TOP_MARGIN: i32 = 12;
const ICON_SIZE: u32 = 32;
const ICON_RGBA: &[u8] = include_bytes!("../../resources/tray-32.rgba");

unsafe extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: plain default handling of every message.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn err(what: &str, e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Tray(format!("overlay {what}: {e}"))
}

pub struct Overlay {
    hwnd: HWND,
    bright: bool,
    timer: Option<usize>,
}

impl Overlay {
    pub fn new() -> Result<Overlay, PlatformError> {
        // SAFETY: registers a window class and creates one hidden window owned by this thread.
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| err("module", e))?;
            let class = w!("wdys_handsfree_overlay");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(overlay_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc); // already-registered is fine
            let x = (GetSystemMetrics(SM_CXSCREEN) - OVERLAY_WIDTH as i32) / 2;
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                w!("wdys hands-free"),
                WS_POPUP,
                x,
                TOP_MARGIN,
                OVERLAY_WIDTH as i32,
                OVERLAY_HEIGHT as i32,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| err("window", e))?;
            Ok(Overlay { hwnd, bright: true, timer: None })
        }
    }

    /// Shows the overlay while hands-free is on and hides it for every other state.
    pub fn apply(&mut self, event: &UiEvent) {
        if let UiEvent::Tray(state) = event {
            if *state == TrayState::HandsFree {
                self.show();
            } else {
                self.hide();
            }
        }
    }

    pub fn owns_timer(&self, id: usize) -> bool {
        self.timer == Some(id)
    }

    pub fn pulse(&mut self) {
        self.bright = !self.bright;
        self.render();
    }

    fn show(&mut self) {
        if self.timer.is_some() {
            return;
        }
        self.bright = true;
        self.render();
        // SAFETY: showing our own window without activating it, then starting a thread timer.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            let id = SetTimer(None, 0, PULSE_MS, None);
            self.timer = (id != 0).then_some(id);
        }
    }

    fn hide(&mut self) {
        // SAFETY: stops our thread timer and hides our own window.
        unsafe {
            if let Some(id) = self.timer.take() {
                let _ = KillTimer(None, id);
            }
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Paints the current frame into the layered window.
    fn render(&self) {
        let frame = overlay_frame(ICON_RGBA, ICON_SIZE, self.bright);
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: frame.width as i32,
            biHeight: -(frame.height as i32), // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
        // SAFETY: standard layered-window painting; every GDI object created here is released below,
        // and `bits` points to a buffer of exactly width*height*4 bytes owned by the DIB section.
        unsafe {
            let screen = GetDC(None);
            let memory = CreateCompatibleDC(Some(screen));
            let mut bits: *mut c_void = std::ptr::null_mut();
            if let Ok(bitmap) = CreateDIBSection(Some(memory), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                if !bits.is_null() {
                    std::ptr::copy_nonoverlapping(frame.bgra.as_ptr(), bits as *mut u8, frame.bgra.len());
                    let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
                    let blend = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 255,
                        AlphaFormat: AC_SRC_ALPHA as u8,
                    };
                    let size = SIZE { cx: frame.width as i32, cy: frame.height as i32 };
                    let _ = UpdateLayeredWindow(
                        self.hwnd,
                        Some(screen),
                        None,
                        Some(&size),
                        Some(memory),
                        Some(&POINT { x: 0, y: 0 }),
                        COLORREF(0),
                        Some(&blend),
                        ULW_ALPHA,
                    );
                    SelectObject(memory, previous);
                }
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.hide();
        // SAFETY: destroying the window this struct created.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
