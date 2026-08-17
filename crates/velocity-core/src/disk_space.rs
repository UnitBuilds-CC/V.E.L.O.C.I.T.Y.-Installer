//! Disk space validation before installation.
//!
//! Checks that the target drive has enough free space for the installation.
//! Cross-platform: uses GetDiskFreeSpaceExW on Windows, `df` on Unix.

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
#[cfg(target_os = "windows")]
pub fn get_free_disk_space(path: &Path) -> Result<u64> {
    use windows::core::*;
    use windows::Win32::Storage::FileSystem::*;

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

/// Get the free disk space on the filesystem containing the given path.
///
/// Uses `df -P` (POSIX format) for portability across Linux and macOS.
#[cfg(not(target_os = "windows"))]
pub fn get_free_disk_space(path: &Path) -> Result<u64> {
    // Use df -P for POSIX-portable output format
    // Columns: Filesystem 1024-blocks Used Available Capacity Mounted-on
    let target = if path.exists() {
        path.to_path_buf()
    } else {
        // If path doesn't exist yet, check the nearest existing parent
        path.ancestors()
            .find(|a| a.exists())
            .unwrap_or(Path::new("/"))
    };

    let output = std::process::Command::new("df")
        .args(["-P"])
        .arg(&target)
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run df: {}", e)))?;

    if !output.status.success() {
        return Err(CoreError::Other(format!(
            "df failed for path: {}",
            target.display()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse the second line (first line is headers)
    // Available column is the 4th field (index 3), in 1024-byte blocks
    for line in stdout.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 5 {
            // Available is field index 3 in POSIX df output
            if let Ok(available_kb) = fields[3].parse::<u64>() {
                return Ok(available_kb * 1024); // Convert KB to bytes
            }
        }
    }

    Err(CoreError::Other(format!(
        "Could not parse df output for: {}",
        target.display()
    )))
}

/// Get the drive root from a path (e.g., "C:\").
#[cfg(target_os = "windows")]
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

    #[cfg(target_os = "windows")]
    #[test]
    fn test_get_drive_root() {
        assert_eq!(get_drive_root(Path::new("C:\\Users\\test")), "C:\\");
        assert_eq!(get_drive_root(Path::new("D:\\Program Files\\app")), "D:\\");
    }

    #[test]
    fn test_check_disk_space_current_dir() {
        // Current directory should always have some space
        let result = check_disk_space(Path::new("."), 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_disk_space_impossible_amount() {
        // Requesting 1 EB should fail on any real system
        let result = check_disk_space(Path::new("."), u64::MAX);
        assert!(result.is_err());
    }
}
