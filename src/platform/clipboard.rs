//! Win32 clipboard access for plain text.

use std::thread::sleep;
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

use crate::error::PasteError;
use crate::paste::Clipboard;

const OPEN_RETRIES: u32 = 10;

fn err(what: &str, e: impl std::fmt::Display) -> PasteError {
    PasteError::Clipboard(format!("{what}: {e}"))
}

/// Opens the clipboard (another app may hold it briefly) and closes it on drop.
struct OpenGuard;

impl OpenGuard {
    fn open() -> Result<OpenGuard, PasteError> {
        let mut last = None;
        for _ in 0..OPEN_RETRIES {
            // SAFETY: no window is associated with the clipboard; None is allowed.
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(OpenGuard),
                Err(e) => {
                    last = Some(e);
                    sleep(Duration::from_millis(10));
                }
            }
        }
        Err(err("could not open clipboard", last.map(|e| e.to_string()).unwrap_or_default()))
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by this thread in `open`.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// Copies `bytes` into a movable global allocation, as clipboard data must be.
fn alloc_global(bytes: &[u8]) -> Result<HGLOBAL, PasteError> {
    // SAFETY: allocate, lock, copy exactly `bytes.len()` bytes into it, unlock.
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| err("alloc failed", e))?;
        let ptr = GlobalLock(handle) as *mut u8;
        if ptr.is_null() {
            return Err(PasteError::Clipboard("could not lock memory".into()));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(handle);
        Ok(handle)
    }
}

fn set_dword_format(name: PCWSTR, value: u32) -> Result<(), PasteError> {
    // SAFETY: registering a named format and handing the clipboard an owned allocation.
    unsafe {
        let format = RegisterClipboardFormatW(name);
        let data = alloc_global(&value.to_le_bytes())?;
        SetClipboardData(format, Some(HANDLE(data.0))).map_err(|e| err("set flag failed", e))?;
    }
    Ok(())
}

pub struct WinClipboard;

impl Clipboard for WinClipboard {
    fn get_text(&mut self) -> Result<Option<String>, PasteError> {
        let _guard = OpenGuard::open()?;
        // SAFETY: the clipboard is open; the locked pointer is read only until the matching unlock.
        unsafe {
            if IsClipboardFormatAvailable(u32::from(CF_UNICODETEXT.0)).is_err() {
                return Ok(None);
            }
            let handle = GetClipboardData(u32::from(CF_UNICODETEXT.0)).map_err(|e| err("read failed", e))?;
            let global = HGLOBAL(handle.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                return Ok(None);
            }
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(global);
            Ok(Some(text))
        }
    }

    fn set_text(&mut self, text: &str, exclude_from_history: bool) -> Result<(), PasteError> {
        let _guard = OpenGuard::open()?;
        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes: Vec<u8> = utf16.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        let data = alloc_global(&bytes)?;
        // SAFETY: the clipboard is open; ownership of `data` passes to the system on success.
        unsafe {
            EmptyClipboard().map_err(|e| err("clear failed", e))?;
            SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(data.0)))
                .map_err(|e| err("write failed", e))?;
        }
        if exclude_from_history {
            set_dword_format(w!("ExcludeClipboardContentFromMonitorProcessing"), 0)?;
            set_dword_format(w!("CanIncludeInClipboardHistory"), 0)?;
            set_dword_format(w!("CanUploadToCloudClipboard"), 0)?;
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), PasteError> {
        let _guard = OpenGuard::open()?;
        // SAFETY: the clipboard is open on this thread.
        unsafe { EmptyClipboard() }.map_err(|e| err("clear failed", e))
    }
}
