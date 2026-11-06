//! Single-instance guard using a named mutex.

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

use crate::error::InstanceError;

/// Holds the mutex for the life of the process; dropping releases it.
pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the handle was created by CreateMutexW and is closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn acquire() -> Result<InstanceGuard, InstanceError> {
    // SAFETY: creating a named mutex; GetLastError is read immediately after the call.
    unsafe {
        let handle = CreateMutexW(None, true, w!("Local\\wdys-single-instance"))
            .map_err(|e| InstanceError::Os(e.to_string()))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Err(InstanceError::AlreadyRunning);
        }
        Ok(InstanceGuard(handle))
    }
}
