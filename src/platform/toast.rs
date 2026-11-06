//! Windows toast notifications for an unpackaged desktop app.
//!
//! Toasts need an AppUserModelID registered under HKCU (name and icon), set on the process.

use std::path::{Path, PathBuf};

use windows::core::{w, HSTRING, PCWSTR};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier};

use crate::error::PlatformError;
use crate::toast::toast_xml;

const APP_ID: &str = "wdys.dictation";
const APP_NAME: &str = "wdys";
const ICON_PNG: &[u8] = include_bytes!("../../resources/wdys-256.png");

fn err(what: &str, e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Toast(format!("{what}: {e}"))
}

fn set_string(key: HKEY, name: PCWSTR, value: &str) -> Result<(), PlatformError> {
    let bytes: Vec<u8> = value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    // SAFETY: `key` is an open registry key and `bytes` is a valid NUL-terminated UTF-16 buffer.
    unsafe { RegSetValueExW(key, name, None, REG_SZ, Some(&bytes)) }
        .ok()
        .map_err(|e| err("registry write failed", e))
}

/// Registers the app's toast identity (display name and icon) for the current user.
fn register_app_id(icon_path: &Path) -> Result<(), PlatformError> {
    let subkey = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"));
    let mut key = HKEY::default();
    // SAFETY: creating/opening a key under HKCU and closing it before returning.
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &subkey,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|e| err("registry open failed", e))?;
        let result = set_string(key, w!("DisplayName"), APP_NAME)
            .and_then(|()| set_string(key, w!("IconUri"), &icon_path.display().to_string()));
        let _ = RegCloseKey(key);
        result
    }
}

pub struct Toaster {
    notifier: ToastNotifier,
}

impl Toaster {
    /// Call once on the main thread. `data_dir` is where the toast icon image is kept.
    pub fn new(data_dir: &Path) -> Result<Toaster, PlatformError> {
        let icon_path: PathBuf = data_dir.join("icon.png");
        std::fs::create_dir_all(data_dir)?;
        if std::fs::read(&icon_path).map_or(true, |existing| existing != ICON_PNG) {
            std::fs::write(&icon_path, ICON_PNG)?;
        }
        register_app_id(&icon_path)?;
        // SAFETY: COM/WinRT initialisation for this thread; an "already initialised" result is fine.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
            SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID))
                .map_err(|e| err("could not set app id", e))?;
        }
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
            .map_err(|e| err("could not create notifier", e))?;
        Ok(Toaster { notifier })
    }

    pub fn show(&self, title: &str, body: &str) -> Result<(), PlatformError> {
        let doc = XmlDocument::new().map_err(|e| err("xml", e))?;
        doc.LoadXml(&HSTRING::from(toast_xml(title, body)))
            .map_err(|e| err("xml", e))?;
        let toast =
            ToastNotification::CreateToastNotification(&doc).map_err(|e| err("toast", e))?;
        self.notifier.Show(&toast).map_err(|e| err("show", e))
    }
}
