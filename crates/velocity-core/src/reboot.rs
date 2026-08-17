//! Reboot management for the installer.
//!
//! Provides:
//! - Schedule files for deletion/rename on next reboot
//! - Detect if a reboot is pending from a previous operation
//! - Request system reboot
//! - Set/clear reboot-required flag in registry
//! - PendingFileRenameOperations management

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

/// Check if a system reboot is pending.
///
/// Checks the Session Manager registry key for PendingFileRenameOperations.
pub fn is_reboot_pending() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager") {
        // Check for PendingFileRenameOperations
        match key.get_raw_value("PendingFileRenameOperations") {
            Ok(_) => return true,
            Err(_) => {}
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

/// Check if this installer session has requested a reboot.
pub fn is_velocity_reboot_requested() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"SOFTWARE\Velocity\Installer") {
        if let Ok(flag) = key.get_value::<u32, _>("RebootRequested") {
            return flag != 0;
        }
    }

    false
}

/// Mark that a reboot is required by this installer.
pub fn request_reboot(app_name: &str) -> Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    info!("Reboot requested by installer for: {}", app_name);

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(r"SOFTWARE\Velocity\Installer")
        .map_err(|e| CoreError::other("reboot", format!("Failed to create registry key: {}", e)))?;

    key.set_value("RebootRequested", &1u32)
        .map_err(|e| CoreError::other("reboot", format!("Failed to set reboot flag: {}", e)))?;

    key.set_value("RebootApp", &app_name.to_string())
        .map_err(|e| CoreError::other("reboot", format!("Failed to set reboot app: {}", e)))?;

    Ok(())
}

/// Clear the reboot request flag.
pub fn clear_reboot_request() -> Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(
        r"SOFTWARE\Velocity\Installer",
        winreg::enums::KEY_SET_VALUE,
    ) {
        let _ = key.delete_value("RebootRequested");
        let _ = key.delete_value("RebootApp");
    }

    debug!("Cleared reboot request flag");
    Ok(())
}

/// Schedule a file for deletion on the next system reboot.
///
/// Uses the PendingFileRenameOperations registry value to schedule
/// the file for deletion. The file will be deleted by the Session
/// Manager during the next boot sequence.
pub fn schedule_file_delete_on_reboot(file_path: &Path) -> Result<()> {
    let path_str = file_path.to_string_lossy().to_string();
    info!("Scheduling file for deletion on reboot: {}", path_str);

    append_pending_rename(&path_str, "")
}

/// Schedule a file to be renamed/moved on the next system reboot.
///
/// Uses the PendingFileRenameOperations registry value.
/// `source` will be renamed to `destination` on next boot.
/// If `destination` is empty, the source file will be deleted.
pub fn schedule_file_rename_on_reboot(source: &Path, destination: &Path) -> Result<()> {
    let src = source.to_string_lossy().to_string();
    let dst = destination.to_string_lossy().to_string();
    info!("Scheduling file rename on reboot: {} -> {}", src, dst);

    append_pending_rename(&src, &dst)
}

/// Append an entry to PendingFileRenameOperations.
///
/// The registry value is a REG_MULTI_SZ where entries are pairs of
/// null-terminated strings: [source\0, destination\0, ...].
/// If destination is empty, the source is deleted on reboot.
fn append_pending_rename(source: &str, destination: &str) -> Result<()> {
    use winreg::RegKey;
    use winreg::enums::*;
    use winreg::RegValue;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Session Manager",
        KEY_READ | KEY_WRITE,
    ).map_err(|e| CoreError::other("reboot", format!(
        "Failed to open Session Manager key: {}. Admin privileges may be required.", e
    )))?;

    // Read existing entries
    let mut existing: Vec<String> = match key.get_raw_value("PendingFileRenameOperations") {
        Ok(reg_value) => {
            if reg_value.vtype == REG_MULTI_SZ {
                // Decode REG_MULTI_SZ: bytes are null-separated strings, terminated by double null
                let bytes: Vec<u16> = reg_value.bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                let full_string = String::from_utf16_lossy(&bytes);
                full_string.split('\0')
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

    let bytes: Vec<u8> = encoded.iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();

    let reg_value = RegValue {
        vtype: REG_MULTI_SZ,
        bytes,
    };

    key.set_raw_value("PendingFileRenameOperations", &reg_value)
        .map_err(|e| CoreError::other("reboot", format!(
            "Failed to update PendingFileRenameOperations: {}", e
        )))?;

    debug!("Appended to PendingFileRenameOperations: {} -> {}", source, destination);
    Ok(())
}

/// Request a system reboot.
///
/// Returns Ok(true) if the reboot was initiated, Ok(false) if it was
/// declined or could not be initiated.
pub fn request_system_reboot() -> Result<bool> {
    info!("Requesting system reboot...");

    // Use shutdown.exe for reliable reboot — avoids needing to manually
    // adjust SeShutdownPrivilege via complex Win32 API calls.
    let output = std::process::Command::new("shutdown")
        .args(["/r", "/t", "0", "/d", "p:4:1"])
        .output()
        .map_err(|e| CoreError::other("reboot", format!("Failed to run shutdown command: {}", e)))?;

    if output.status.success() {
        info!("System reboot initiated");
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to initiate reboot: {}", stderr);
        Err(CoreError::other("reboot", format!(
            "Failed to initiate reboot: {}. Administrator privileges may be required.", stderr
        )))
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
pub fn is_file_locked(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    // Try to open the file with exclusive access
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => false,
        Err(_) => true,
    }
}

/// Check if any files in a list are locked.
///
/// Returns a list of locked file paths.
pub fn find_locked_files(paths: &[&Path]) -> Vec<std::path::PathBuf> {
    paths.iter()
        .filter(|p| is_file_locked(p))
        .map(|p| (*p).to_path_buf())
        .collect()
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
        assert!(!is_file_locked(Path::new("C:\\nonexistent_file_xyz_123.dll")));
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
}
