//! Disk space validation before installation.
//!
//! Checks that the target drive has enough free space for the installation.
//!
//! Note: `get_free_disk_space` uses Win32 GetDiskFreeSpaceExW and is Windows-only.
//! The entire module is cfg-gated; cross-platform disk space checking will be
//! provided via the platform module.

#![cfg(target_os = "windows")]

use crate::error::{CoreError, Result};
use std::path::Path;

/// Check that the target drive has enough free space.
///
/// `required_bytes` is the minimum number of bytes needed.
/// Returns Ok(()) if sufficient space, or Err with details.
pub fn check_disk_space(target_dir: &Path, required_bytes: u64) -> Result<()> {
    let free_bytes = get_free_disk_space(target_dir)?;

    if free_bytes < required_bytes {
        return Err(CoreError::Other(format!(
            "Insufficient disk space. Required: {} MB, Available: {} MB",
            required_bytes / (1024 * 1024),
            free_bytes / (1024 * 1024)
        )));
    }

    Ok(())
}

/// Get the free disk space on the drive containing the given path.
pub fn get_free_disk_space(path: &Path) -> Result<u64> {
    use windows::core::*;
    use windows::Win32::Storage::FileSystem::*;

    // Get the root of the drive
    let root = get_drive_root(path);
    let root_wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut free_bytes_available: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free_bytes: u64 = 0;

        let result = GetDiskFreeSpaceExW(
            PCWSTR(root_wide.as_ptr()),
            Some(&mut free_bytes_available),
            Some(&mut total_bytes),
            Some(&mut total_free_bytes),
        );

        if result.is_ok() {
            Ok(total_free_bytes)
        } else {
            Err(CoreError::Other(format!(
                "Failed to query disk space for: {}",
                root
            )))
        }
    }
}

/// Get the drive root from a path (e.g., "C:\").
fn get_drive_root(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if path_str.len() >= 2 && path_str.as_bytes()[1] == b':' {
        format!("{}\\", &path_str[..2])
    } else {
        "C:\\".to_string()
    }
}

/// Calculate the total size of files in a directory.
pub fn calculate_dir_size(dir: &Path) -> Result<u64> {
    let mut total = 0u64;
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                total += entry.metadata()?.len();
            } else if path.is_dir() {
                total += calculate_dir_size(&path)?;
            }
        }
    }
    Ok(total)
}

/// Format bytes into a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_get_drive_root() {
        assert_eq!(get_drive_root(Path::new("C:\\Users\\test")), "C:\\");
        assert_eq!(get_drive_root(Path::new("D:\\Program Files\\app")), "D:\\");
    }
}
