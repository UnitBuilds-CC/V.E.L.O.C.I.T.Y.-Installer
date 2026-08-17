//! Installer mutex — prevents multiple instances of the same installer running simultaneously.
//!
//! Uses Win32 named mutexes to detect if another instance of this installer
//! is already running. This prevents conflicts from double-clicking the installer
//! or running it from multiple command prompts.
//!
//! Windows-only: uses Win32 CreateMutexW/OpenMutexW API.

#![cfg(target_os = "windows")]

use crate::error::{CoreError, Result};
use tracing::{debug, info, warn};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;

/// A guard that holds the installer mutex. When dropped, the mutex is released.
///
/// If the guard cannot be acquired (another instance is running), `try_acquire`
/// returns an error.
#[derive(Debug)]
pub struct InstallerMutex {
    handle: HANDLE,
    name: String,
}

impl InstallerMutex {
    /// Try to acquire the installer mutex.
    ///
    /// The mutex name is derived from the app name to allow different installers
    /// to run simultaneously while preventing the same installer from running twice.
    ///
    /// Returns `Ok(InstallerMutex)` if acquired, or `Err` if another instance holds it.
    pub fn try_acquire(app_name: &str) -> Result<Self> {
        // Create a unique mutex name from the app name
        let safe_name = app_name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let mutex_name = format!("Global\\VelocityInstaller_{}", safe_name);

        info!("Attempting to acquire installer mutex: {}", mutex_name);

        // SAFETY: Win32 mutex API — CreateMutexW returns a handle closed by CloseHandle.
        // On ERROR_ALREADY_EXISTS we close the handle and return an error.
        // On other errors we store a null handle (Drop checks for null before release).
        unsafe {
            let name_w: Vec<u16> = mutex_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let handle = CreateMutexW(None, true, PCWSTR(name_w.as_ptr()));

            match handle {
                Ok(h) => {
                    // CreateMutexW succeeds even if mutex already exists.
                    // Check GetLastError to detect that case.
                    let last_err = GetLastError();
                    if last_err == ERROR_ALREADY_EXISTS {
                        // Another instance already holds this mutex
                        let _ = CloseHandle(h);
                        warn!("Another instance of the installer is already running");
                        Err(CoreError::Other(format!(
                            "Another instance of the {} installer is already running. \
                             Please close the other instance and try again.",
                            app_name
                        )))
                    } else {
                        info!("Installer mutex acquired successfully");
                        Ok(InstallerMutex {
                            handle: h,
                            name: mutex_name,
                        })
                    }
                }
                Err(e) => {
                    // Other error — log but allow to proceed (non-fatal)
                    warn!("Mutex creation error (proceeding anyway): {}", e);
                    Ok(InstallerMutex {
                        handle: HANDLE(std::ptr::null_mut()),
                        name: mutex_name,
                    })
                }
            }
        }
    }

    /// Check if another instance is running without acquiring the mutex.
    pub fn is_another_running(app_name: &str) -> bool {
        let safe_name = app_name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let mutex_name = format!("Global\\VelocityInstaller_{}", safe_name);

        // SAFETY: OpenMutexW returns a handle that is always closed with CloseHandle.
        // The mutex name is sanitized to alphanumeric + underscore only.
        unsafe {
            let name_w: Vec<u16> = mutex_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Try to open the existing mutex
            let result = OpenMutexW(
                windows::Win32::System::Threading::SYNCHRONIZATION_ACCESS_RIGHTS(0x00100000u32),
                false,
                PCWSTR(name_w.as_ptr()),
            );
            match result {
                Ok(handle) => {
                    // Mutex exists — another instance is running
                    let _ = CloseHandle(handle);
                    debug!("Found existing installer mutex: {}", mutex_name);
                    true
                }
                Err(_) => {
                    // No mutex — no other instance
                    false
                }
            }
        }
    }
}

impl Drop for InstallerMutex {
    fn drop(&mut self) {
        if !self.handle.0.is_null() {
            // SAFETY: handle was created by CreateMutexW and is valid (non-null).
            // ReleaseMutex + CloseHandle are called exactly once via Drop.
            unsafe {
                let _ = ReleaseMutex(self.handle);
                let _ = CloseHandle(self.handle);
            }
            debug!("Released installer mutex: {}", self.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_acquire_and_release() {
        let mutex = InstallerMutex::try_acquire("TestApp_Mutex_1").unwrap();
        assert!(!mutex.handle.0.is_null());
        // Mutex should be held now
        assert!(InstallerMutex::is_another_running("TestApp_Mutex_1"));
        drop(mutex);
        // After drop, another instance check should return false
        // (may have a tiny race window, but generally reliable)
    }

    #[test]
    fn test_mutex_prevents_double_instance() {
        let _m1 = InstallerMutex::try_acquire("TestApp_Double_Test").unwrap();
        let result = InstallerMutex::try_acquire("TestApp_Double_Test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already running"));
    }

    #[test]
    fn test_different_apps_independent() {
        let _m1 = InstallerMutex::try_acquire("TestApp_Independent_A").unwrap();
        let m2 = InstallerMutex::try_acquire("TestApp_Independent_B");
        assert!(m2.is_ok()); // Different app names should not conflict
    }

    #[test]
    fn test_is_another_running_no_instance() {
        // A unique name that shouldn't have a mutex
        assert!(!InstallerMutex::is_another_running(
            "NonExistent_App_XYZ_999"
        ));
    }

    #[test]
    fn test_safe_name_sanitization() {
        // Special characters should be replaced with underscores
        let mutex = InstallerMutex::try_acquire("My App! @#$ v2.0").unwrap();
        assert!(mutex.name.contains("VelocityInstaller"));
        assert!(!mutex.name.contains(' '));
        assert!(!mutex.name.contains('!'));
    }
}
