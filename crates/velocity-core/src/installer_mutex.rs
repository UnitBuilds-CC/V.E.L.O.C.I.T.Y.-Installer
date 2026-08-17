//! Installer mutex — prevents multiple instances of the same installer running simultaneously.
//!
//! Cross-platform: uses Win32 named mutexes on Windows, pid file locking on Unix.
//! This prevents conflicts from double-clicking the installer or running it
//! from multiple terminals.

use crate::error::CoreError;
use tracing::{debug, info, warn};

// ===========================================================================
// Windows implementation (Win32 named mutex)
// ===========================================================================

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::error::Result; // disambiguate from windows::core::Result
    use windows::core::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Threading::*;

    /// A guard that holds the installer mutex. When dropped, the mutex is released.
    #[derive(Debug)]
    pub struct InstallerMutex {
        handle: HANDLE,
        name: String,
    }

    impl InstallerMutex {
        /// Try to acquire the installer mutex.
        pub fn try_acquire(app_name: &str) -> Result<Self> {
            let safe_name = sanitize_name(app_name);
            let mutex_name = format!("Global\\VelocityInstaller_{}", safe_name);

            info!("Attempting to acquire installer mutex: {}", mutex_name);

            // SAFETY: Win32 mutex API — CreateMutexW returns a handle closed by CloseHandle.
            unsafe {
                let name_w: Vec<u16> = mutex_name
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                let handle = CreateMutexW(None, true, PCWSTR(name_w.as_ptr()));

                match handle {
                    Ok(h) => {
                        let last_err = GetLastError();
                        if last_err == ERROR_ALREADY_EXISTS {
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
            let safe_name = sanitize_name(app_name);
            let mutex_name = format!("Global\\VelocityInstaller_{}", safe_name);

            // SAFETY: OpenMutexW returns a handle that is always closed with CloseHandle.
            unsafe {
                let name_w: Vec<u16> = mutex_name
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                let result = OpenMutexW(
                    windows::Win32::System::Threading::SYNCHRONIZATION_ACCESS_RIGHTS(0x00100000u32),
                    false,
                    PCWSTR(name_w.as_ptr()),
                );
                match result {
                    Ok(handle) => {
                        let _ = CloseHandle(handle);
                        debug!("Found existing installer mutex: {}", mutex_name);
                        true
                    }
                    Err(_) => false,
                }
            }
        }
    }

    impl Drop for InstallerMutex {
        fn drop(&mut self) {
            if !self.handle.0.is_null() {
                // SAFETY: handle was created by CreateMutexW and is valid.
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
            assert!(InstallerMutex::is_another_running("TestApp_Mutex_1"));
            drop(mutex);
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
            assert!(m2.is_ok());
        }

        #[test]
        fn test_is_another_running_no_instance() {
            assert!(!InstallerMutex::is_another_running(
                "NonExistent_App_XYZ_999"
            ));
        }

        #[test]
        fn test_safe_name_sanitization() {
            let mutex = InstallerMutex::try_acquire("My App! @#$ v2.0").unwrap();
            assert!(mutex.name.contains("VelocityInstaller"));
            assert!(!mutex.name.contains(' '));
            assert!(!mutex.name.contains('!'));
        }
    }
}

// ===========================================================================
// Unix implementation (pid file locking)
// ===========================================================================

#[cfg(not(target_os = "windows"))]
mod unix_impl {
    use super::*;
    use crate::error::Result;
    use std::path::PathBuf;

    /// A guard that holds the installer lock. When dropped, the lock file is removed.
    #[derive(Debug)]
    pub struct InstallerMutex {
        lock_path: PathBuf,
        app_name: String,
    }

    impl InstallerMutex {
        /// Try to acquire the installer lock via a pid file.
        ///
        /// Creates a lock file at `<temp_dir>/.velocity_installer_<sanitized_name>.lock`
        /// containing the current PID. If the lock file exists and the PID is still running,
        /// returns an error.
        pub fn try_acquire(app_name: &str) -> Result<Self> {
            let lock_path = lock_file_path(app_name);

            info!(
                "Attempting to acquire installer lock: {}",
                lock_path.display()
            );

            // Check if a lock file already exists
            if lock_path.exists() {
                if let Ok(pid_str) = std::fs::read_to_string(&lock_path) {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        if is_process_alive(pid) {
                            warn!(
                                "Another instance of the installer is already running (PID {})",
                                pid
                            );
                            return Err(CoreError::Other(format!(
                                "Another instance of the {} installer is already running (PID {}). \
                                 Please close the other instance and try again.",
                                app_name, pid
                            )));
                        }
                    }
                }
                // Lock file exists but process is dead — stale lock, remove it
                debug!("Removing stale lock file: {}", lock_path.display());
                let _ = std::fs::remove_file(&lock_path);
            }

            // Write our PID to the lock file
            let pid = std::process::id();
            if let Some(parent) = lock_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&lock_path, format!("{}\n", pid)).map_err(|e| {
                CoreError::Other(format!(
                    "Failed to create lock file {}: {}",
                    lock_path.display(),
                    e
                ))
            })?;

            info!("Installer lock acquired (PID {})", pid);
            Ok(InstallerMutex {
                lock_path,
                app_name: app_name.to_string(),
            })
        }

        /// Check if another instance is running without acquiring the lock.
        pub fn is_another_running(app_name: &str) -> bool {
            let lock_path = lock_file_path(app_name);
            if !lock_path.exists() {
                return false;
            }
            if let Ok(pid_str) = std::fs::read_to_string(&lock_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    return is_process_alive(pid);
                }
            }
            false
        }
    }

    impl Drop for InstallerMutex {
        fn drop(&mut self) {
            if self.lock_path.exists() {
                if let Err(e) = std::fs::remove_file(&self.lock_path) {
                    warn!(
                        "Failed to remove lock file {}: {}",
                        self.lock_path.display(),
                        e
                    );
                }
            }
            debug!("Released installer lock: {}", self.app_name);
        }
    }

    /// Get the path to the lock file for a given app name.
    fn lock_file_path(app_name: &str) -> PathBuf {
        let safe_name = sanitize_name(app_name);
        let mut path = std::env::temp_dir();
        path.push(format!(".velocity_installer_{}.lock", safe_name));
        path
    }

    /// Check if a process with the given PID is alive.
    ///
    /// Uses `kill(pid, 0)` which doesn't send a signal but checks if the process exists.
    fn is_process_alive(pid: u32) -> bool {
        unsafe {
            extern "C" {
                fn kill(pid: u32, sig: i32) -> i32;
            }
            // kill(pid, 0) returns 0 if process exists, -1 if not
            kill(pid, 0) == 0
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_lock_acquire_and_release() {
            let lock = InstallerMutex::try_acquire("TestApp_Unix_Lock_1").unwrap();
            assert!(lock.lock_path.exists());
            assert!(InstallerMutex::is_another_running("TestApp_Unix_Lock_1"));
            drop(lock);
            // After drop, lock file should be removed
        }

        #[test]
        fn test_lock_prevents_double_instance() {
            let _l1 = InstallerMutex::try_acquire("TestApp_Unix_Double").unwrap();
            let result = InstallerMutex::try_acquire("TestApp_Unix_Double");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("already running"));
        }

        #[test]
        fn test_different_apps_independent() {
            let _l1 = InstallerMutex::try_acquire("TestApp_Unix_Ind_A").unwrap();
            let l2 = InstallerMutex::try_acquire("TestApp_Unix_Ind_B");
            assert!(l2.is_ok());
        }

        #[test]
        fn test_is_another_running_no_instance() {
            assert!(!InstallerMutex::is_another_running(
                "NonExistent_Unix_App_XYZ_999"
            ));
        }

        #[test]
        fn test_stale_lock_recovery() {
            // Write a stale lock file with a dead PID
            let lock_path = lock_file_path("TestApp_Stale_Lock");
            let _ = std::fs::write(&lock_path, "999999999\n");
            // Should succeed because PID 999999999 is almost certainly dead
            let lock = InstallerMutex::try_acquire("TestApp_Stale_Lock");
            assert!(lock.is_ok());
        }
    }
}

// ===========================================================================
// Cross-platform helpers and re-exports
// ===========================================================================

/// Sanitize an app name for use in a mutex/lock name.
fn sanitize_name(app_name: &str) -> String {
    app_name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

// Re-export the platform-specific InstallerMutex
#[cfg(target_os = "windows")]
pub use windows_impl::InstallerMutex;

#[cfg(not(target_os = "windows"))]
pub use unix_impl::InstallerMutex;
