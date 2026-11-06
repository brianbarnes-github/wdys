//! "Start with Windows" via the per-user Run registry key.

use windows::core::w;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};

use crate::error::PlatformError;

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("wdys");

pub fn set(enabled: bool) -> Result<(), PlatformError> {
    let mut key = HKEY::default();
    // SAFETY: standard registry calls on a key opened here and closed before returning.
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, KEY_SET_VALUE, &mut key)
            .ok()
            .map_err(|e| PlatformError::Registry(e.to_string()))?;
        let result = if enabled {
            let exe = std::env::current_exe()?;
            let command = format!("\"{}\"", exe.display());
            let bytes: Vec<u8> = command
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            RegSetValueExW(key, VALUE_NAME, None, REG_SZ, Some(&bytes)).ok()
        } else {
            // Deleting a value that does not exist is fine.
            let _ = RegDeleteValueW(key, VALUE_NAME);
            Ok(())
        };
        let _ = RegCloseKey(key);
        result.map_err(|e| PlatformError::Registry(e.to_string()))
    }
}
