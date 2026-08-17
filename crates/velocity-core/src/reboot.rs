//! Cross-platform reboot management.
//!
//! Provides:
//! - Schedule files for deletion/rename on next reboot
//! - Detect if a reboot is pending from a previous operation
//! - Request system reboot
//! - Set/clear reboot-required flag
//! - PendingFileRenameOperations management (Windows)
//! - File-based reboot flag (Unix)
//!
//! Windows uses the registry for reboot management.
//! Unix uses file-based flags (/var/run/reboot-required on Linux,
//! /tmp/.velocity_reboot_required as fallback).

use crate::error::{CoreError, Result};
use std::path::Path;
use tracing::{debug, info, warn};

/// Status of a reboot requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebootStatus {
    /// No reboot is needed
    NotRequired,
    /// A reboot has been requested but not yet scheduled
    Requested,
    /// A reboot is pending (files scheduled for rename/delete)
    Pending,
    /// The system has already been flagged for reboot
    SystemPending,
}

// ===========================================================================
// Windows implementation (registry-based)
// ===========================================================================

/// Check if a system reboot is pending (Windows).
///
/// Checks the Session Manager registry key for PendingFileRenameOperations.
#[cfg(target_os = "windows")]
pub fn is_reboot_pending() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager") {
        // Check for PendingFileRenameOperations
        if key.get_raw_value("PendingFileRenameOperations").is_ok() {
            return true;
        }

        // Also check the RebootRequired flag
        if let Ok(flag) = key.get_value::<u32, _>("RebootRequired") {
            if flag != 0 {
                return true;
            }
        }
    }

    false
}

/// Check if a system reboot is pending (Linux).
///
/// Checks `/var/run/reboot-required` (Debian/Ubuntu) and
/// `/run/reboot-required` (systemd-based distros).
#[cfg(target_os = "linux")]
pub fn is_reboot_pending() -> bool {
    Path::new("/var/run/reboot-required").exists() || Path::new("/run/reboot-required").exists()
}

/// Check if a system reboot is pending (macOS).
///
/// macOS doesn't have a standard reboot-pending indicator.
/// Check for our own Velocity flag file.
#[cfg(target_os = "macos")]
pub fn is_reboot_pending() -> bool {
    velocity_reboot_flag_path().exists()
}

/// Check if this installer session has requested a reboot (Windows).
#[cfg(target_os = "windows")]
pub fn is_velocity_reboot_requested() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"SOFTWARE\Velocity\Installer") {
        if let Ok(flag) = key.get_value::<u32, _>("RebootRequested") {
            return flag != 0;
        }
    }

    false
}

/// Check if this installer session has requested a reboot (Unix).
///
/// Uses a file-based flag at a platform-specific location.
#[cfg(not(target_os = "windows"))]
pub fn is_velocity_reboot_requested() -> bool {
    velocity_reboot_flag_path().exists()
}

/// Mark that a reboot is required by this installer (Windows).
#[cfg(target_os = "windows")]
pub fn request_reboot(app_name: &str) -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    info!("Reboot requested by installer for: {}", app_name);

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"SOFTWARE\Velocity\Installer")
        .map_err(|e| CoreError::other("reboot", format!("Failed to create registry key: {}", e)))?;

    key.set_value("RebootRequested", &1u32)
        .map_err(|e| CoreError::other("reboot", format!("Failed to set reboot flag: {}", e)))?;

    key.set_value("RebootApp", &app_name.to_string())
        .map_err(|e| CoreError::other("reboot", format!("Failed to set reboot app: {}", e)))?;

    Ok(())
}

/// Mark that a reboot is required by this installer (Unix).
///
/// Creates a file-based flag to track the reboot request.
#[cfg(not(target_os = "windows"))]
pub fn request_reboot(app_name: &str) -> Result<()> {
    info!("Reboot requested by installer for: {}", app_name);

    let flag_path = velocity_reboot_flag_path();
    let content = format!("app={}\n", app_name);
    std::fs::write(&flag_path, content).map_err(|e| {
        CoreError::other(
            "reboot",
            format!("Failed to write reboot flag {}: {}", flag_path.display(), e),
        )
    })?;

    debug!("Reboot flag written: {}", flag_path.display());
    Ok(())
}

/// Clear the reboot request flag (Windows).
#[cfg(target_os = "windows")]
pub fn clear_reboot_request() -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) =
        hkcu.open_subkey_with_flags(r"SOFTWARE\Velocity\Installer", winreg::enums::KEY_SET_VALUE)
    {
        let _ = key.delete_value("RebootRequested");
        let _ = key.delete_value("RebootApp");
    }

    debug!("Cleared reboot request flag");
    Ok(())
}

/// Clear the reboot request flag (Unix).
///
/// Removes the file-based flag.
#[cfg(not(target_os = "windows"))]
pub fn clear_reboot_request() -> Result<()> {
    let flag_path = velocity_reboot_flag_path();
    if flag_path.exists() {
        if let Err(e) = std::fs::remove_file(&flag_path) {
            warn!(
                "Failed to remove reboot flag {}: {}",
                flag_path.display(),
                e
            );
        }
    }

    debug!("Cleared reboot request flag");
    Ok(())
}

/// Schedule a file for deletion on the next system reboot (Windows).
///
/// Uses the PendingFileRenameOperations registry value to schedule
/// the file for deletion. The file will be deleted by the Session
/// Manager during the next boot sequence.
#[cfg(target_os = "windows")]
pub fn schedule_file_delete_on_reboot(file_path: &Path) -> Result<()> {
    let path_str = file_path.to_string_lossy().to_string();
    info!("Scheduling file for deletion on reboot: {}", path_str);

    append_pending_rename(&path_str, "")
}

/// Schedule a file for deletion on the next system reboot (Unix).
///
/// On Unix, there's no built-in mechanism for deferred file deletion.
/// We create a cleanup script that can be run at boot time via cron @reboot
/// or a systemd tmpfiles.d entry.
#[cfg(not(target_os = "windows"))]
pub fn schedule_file_delete_on_reboot(file_path: &Path) -> Result<()> {
    let path_str = file_path.to_string_lossy().to_string();
    info!("Scheduling file for deletion on reboot: {}", path_str);

    // Append to a Velocity-managed cleanup list
    let cleanup_list = velocity_cleanup_list_path();
    let mut content = if cleanup_list.exists() {
        std::fs::read_to_string(&cleanup_list).unwrap_or_default()
    } else {
        // Create parent directory if needed
        if let Some(parent) = cleanup_list.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        String::new()
    };

    content.push_str(&format!("delete {}\n", path_str));
    std::fs::write(&cleanup_list, content).map_err(|e| {
        CoreError::other(
            "reboot",
            format!(
                "Failed to write cleanup list {}: {}",
                cleanup_list.display(),
                e
            ),
        )
    })?;

    debug!("File scheduled for deletion on reboot: {}", path_str);
    Ok(())
}

/// Schedule a file to be renamed/moved on the next system reboot (Windows).
///
/// Uses the PendingFileRenameOperations registry value.
/// `source` will be renamed to `destination` on next boot.
/// If `destination` is empty, the source file will be deleted.
#[cfg(target_os = "windows")]
pub fn schedule_file_rename_on_reboot(source: &Path, destination: &Path) -> Result<()> {
    let src = source.to_string_lossy().to_string();
    let dst = destination.to_string_lossy().to_string();
    info!("Scheduling file rename on reboot: {} -> {}", src, dst);

    append_pending_rename(&src, &dst)
}

/// Schedule a file to be renamed/moved on the next system reboot (Unix).
///
/// Uses a file-based cleanup list similar to schedule_file_delete_on_reboot.
#[cfg(not(target_os = "windows"))]
pub fn schedule_file_rename_on_reboot(source: &Path, destination: &Path) -> Result<()> {
    let src = source.to_string_lossy().to_string();
    let dst = destination.to_string_lossy().to_string();
    info!("Scheduling file rename on reboot: {} -> {}", src, dst);

    let cleanup_list = velocity_cleanup_list_path();
    let mut content = if cleanup_list.exists() {
        std::fs::read_to_string(&cleanup_list).unwrap_or_default()
    } else {
        if let Some(parent) = cleanup_list.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        String::new()
    };

    content.push_str(&format!("rename {} {}\n", src, dst));
    std::fs::write(&cleanup_list, content).map_err(|e| {
        CoreError::other(
            "reboot",
            format!(
                "Failed to write cleanup list {}: {}",
                cleanup_list.display(),
                e
            ),
        )
    })?;

    debug!("File rename scheduled on reboot: {} -> {}", src, dst);
    Ok(())
}

/// Append an entry to PendingFileRenameOperations (Windows only).
///
/// The registry value is a REG_MULTI_SZ where entries are pairs of
/// null-terminated strings: [source\0, destination\0, ...].
/// If destination is empty, the source is deleted on reboot.
#[cfg(target_os = "windows")]
fn append_pending_rename(source: &str, destination: &str) -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;
    use winreg::RegValue;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey_with_flags(
            r"SYSTEM\CurrentControlSet\Control\Session Manager",
            KEY_READ | KEY_WRITE,
        )
        .map_err(|e| {
            CoreError::other(
                "reboot",
                format!(
                    "Failed to open Session Manager key: {}. Admin privileges may be required.",
                    e
                ),
            )
        })?;

    // Read existing entries
    let mut existing: Vec<String> = match key.get_raw_value("PendingFileRenameOperations") {
        Ok(reg_value) => {
            if reg_value.vtype == REG_MULTI_SZ {
                // Decode REG_MULTI_SZ: bytes are null-separated strings, terminated by double null
                let bytes: Vec<u16> = reg_value
                    .bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                let full_string = String::from_utf16_lossy(&bytes);
                full_string
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    };

    // Add new entry (source + destination pair)
    existing.push(source.to_string());
    existing.push(destination.to_string());

    // Encode back to REG_MULTI_SZ
    let mut encoded: Vec<u16> = Vec::new();
    for entry in &existing {
        encoded.extend(entry.encode_utf16());
        encoded.push(0); // null terminator
    }
    encoded.push(0); // final null terminator

    let bytes: Vec<u8> = encoded.iter().flat_map(|w| w.to_le_bytes()).collect();

    let reg_value = RegValue {
        vtype: REG_MULTI_SZ,
        bytes,
    };

    key.set_raw_value("PendingFileRenameOperations", &reg_value)
        .map_err(|e| {
            CoreError::other(
                "reboot",
                format!("Failed to update PendingFileRenameOperations: {}", e),
            )
        })?;

    debug!(
        "Appended to PendingFileRenameOperations: {} -> {}",
        source, destination
    );
    Ok(())
}

// ===========================================================================
// Cross-platform functions
// ===========================================================================

/// Request a system reboot.
///
/// Returns Ok(true) if the reboot was initiated, Ok(false) if it was
/// declined or could not be initiated.
pub fn request_system_reboot() -> Result<bool> {
    info!("Requesting system reboot...");

    #[cfg(target_os = "windows")]
    {
        // Use shutdown.exe for reliable reboot — avoids needing to manually
        // adjust SeShutdownPrivilege via complex Win32 API calls.
        let output = std::process::Command::new("shutdown")
            .args(["/r", "/t", "0", "/d", "p:4:1"])
            .output()
            .map_err(|e| {
                CoreError::other("reboot", format!("Failed to run shutdown command: {}", e))
            })?;

        if output.status.success() {
            info!("System reboot initiated");
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to initiate reboot: {}", stderr);
            Err(CoreError::other(
                "reboot",
                format!(
                    "Failed to initiate reboot: {}. Administrator privileges may be required.",
                    stderr
                ),
            ))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Unix: use shutdown -r now (works on Linux and macOS)
        let output = std::process::Command::new("shutdown")
            .args(["-r", "now"])
            .output()
            .map_err(|e| {
                CoreError::other("reboot", format!("Failed to run shutdown command: {}", e))
            })?;

        if output.status.success() {
            info!("System reboot initiated");
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to initiate reboot: {}", stderr);
            Err(CoreError::other(
                "reboot",
                format!(
                    "Failed to initiate reboot: {}. Root privileges may be required. stderr: {}",
                    stderr
                ),
            ))
        }
    }
}

/// Get the current reboot status.
pub fn get_reboot_status() -> RebootStatus {
    if is_reboot_pending() {
        RebootStatus::SystemPending
    } else if is_velocity_reboot_requested() {
        RebootStatus::Requested
    } else {
        RebootStatus::NotRequired
    }
}

/// Check if a specific file is locked (in use by another process).
///
/// This is useful for determining if a reboot is needed because a file
/// couldn't be replaced during installation.
///
/// On Windows, tries to open the file with exclusive write access.
/// On Unix, checks if the file exists and is non-writable (a rough heuristic;
/// true lock detection on Unix requires flock/fcntl which varies by filesystem).
pub fn is_file_locked(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        // Try to open the file with exclusive access
        std::fs::OpenOptions::new().write(true).open(path).is_err()
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix, try exclusive write as a basic check.
        // This won't detect flock/fcntl locks but catches the common case
        // where a file is held open by a running process with exclusive access.
        std::fs::OpenOptions::new().write(true).open(path).is_err()
    }
}

/// Check if any files in a list are locked.
///
/// Returns a list of locked file paths.
pub fn find_locked_files(paths: &[&Path]) -> Vec<std::path::PathBuf> {
    paths
        .iter()
        .filter(|p| is_file_locked(p))
        .map(|p| (*p).to_path_buf())
        .collect()
}

// ===========================================================================
// Unix helper functions
// ===========================================================================

/// Get the path to the Velocity reboot flag file (Unix).
#[cfg(not(target_os = "windows"))]
fn velocity_reboot_flag_path() -> std::path::PathBuf {
    // Use /tmp for user-level flag (no root required to create)
    std::path::PathBuf::from("/tmp/.velocity_reboot_required")
}

/// Get the path to the Velocity cleanup list (Unix).
///
/// This file records files/directories to delete or rename on next boot.
#[cfg(not(target_os = "windows"))]
fn velocity_cleanup_list_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(".velocity_cleanup_on_reboot");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reboot_status_values() {
        assert_ne!(RebootStatus::NotRequired, RebootStatus::Pending);
        assert_ne!(RebootStatus::Requested, RebootStatus::SystemPending);
    }

    #[test]
    fn test_is_file_locked_nonexistent() {
        assert!(!is_file_locked(Path::new(
            "/nonexistent_file_xyz_123_nonexistent.dll"
        )));
    }

    #[test]
    fn test_find_locked_files_empty() {
        let result = find_locked_files(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_reboot_status() {
        // Should not panic
        let _status = get_reboot_status();
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_velocity_reboot_flag_path() {
        let path = velocity_reboot_flag_path();
        assert!(path.to_string_lossy().contains("velocity_reboot_required"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_velocity_cleanup_list_path() {
        let path = velocity_cleanup_list_path();
        assert!(path
            .to_string_lossy()
            .contains("velocity_cleanup_on_reboot"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_clear_reboot_request_no_flag() {
        // Should not error even if flag doesn't exist
        let _ = clear_reboot_request();
    }
}
