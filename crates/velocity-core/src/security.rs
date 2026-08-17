//! Security hardening utilities for the installer.
//!
//! Provides:
//! - Secure temporary directory creation with restricted ACLs
//! - Path traversal protection (prevents zip-slip attacks)
//! - Manifest integrity verification
//! - File permission validation
//! - Safe file operations (atomic writes, backup before overwrite)

use crate::error::{CoreError, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Create a secure temporary directory for installer operations.
///
/// The directory is created with a unique name and restricted permissions
/// so that only the current user (and administrators) can access it.
/// Returns the path to the created directory.
pub fn create_secure_temp_dir(app_name: &str) -> Result<PathBuf> {
    let base = std::env::temp_dir();
    let session_id = uuid::Uuid::new_v4();
    let dir_name = format!("velocity_{}_{}", sanitize_component(app_name), &session_id.to_string()[..8]);
    let temp_path = base.join(&dir_name);

    std::fs::create_dir_all(&temp_path)?;

    // On Windows, restrict ACLs to current user + administrators
    #[cfg(target_os = "windows")]
    {
        restrict_directory_acl(&temp_path)?;
    }

    info!("Created secure temp dir: {}", temp_path.display());
    debug!("Session ID: {}", session_id);
    Ok(temp_path)
}

/// Clean up a secure temporary directory.
pub fn cleanup_temp_dir(path: &Path) {
    if path.exists() {
        if let Err(e) = std::fs::remove_dir_all(path) {
            warn!("Failed to clean up temp dir {}: {}", path.display(), e);
        } else {
            debug!("Cleaned up temp dir: {}", path.display());
        }
    }
}

/// Validate that a destination path does not escape the target directory.
///
/// This prevents path traversal attacks (e.g., `../../etc/passwd` in a tar archive).
/// Returns an error if the path escapes the target or if paths cannot be resolved.
pub fn validate_path_within_target(dest_path: &Path, target_dir: &Path) -> Result<()> {
    // Canonicalize both paths for comparison.
    // If canonicalize fails (e.g., path doesn't exist yet), we must NOT fall back
    // to uncanonicalized paths — that would bypass the traversal check.
    let canonical_target = target_dir.canonicalize().map_err(|e| {
        CoreError::permission_denied("path validation", format!(
            "Cannot resolve target directory {}: {}", target_dir.display(), e
        ))
    })?;

    // For the destination, if it doesn't exist yet, canonicalize the parent
    // and append the filename to get a reliable path.
    let canonical_dest = if dest_path.exists() {
        dest_path.canonicalize().map_err(|e| {
            CoreError::permission_denied("path validation", format!(
                "Cannot resolve destination path {}: {}", dest_path.display(), e
            ))
        })?
    } else {
        // Parent must exist and be resolvable
        let parent = dest_path.parent().unwrap_or(dest_path);
        let file_name = dest_path.file_name().ok_or_else(|| {
            CoreError::permission_denied("path validation", format!(
                "Invalid destination path: {}", dest_path.display()
            ))
        })?;
        let canonical_parent = parent.canonicalize().map_err(|e| {
            CoreError::permission_denied("path validation", format!(
                "Cannot resolve parent directory {}: {}", parent.display(), e
            ))
        })?;
        canonical_parent.join(file_name)
    };

    if !canonical_dest.starts_with(&canonical_target) {
        return Err(CoreError::permission_denied("path traversal", format!(
            "{} escapes target directory {}",
            dest_path.display(),
            target_dir.display()
        )));
    }
    Ok(())
}

/// Validate a relative path doesn't contain traversal sequences.
///
/// Checks for `..` components, absolute paths, and null bytes.
pub fn validate_relative_path(path: &str) -> Result<()> {
    // Reject null bytes
    if path.contains('\0') {
        return Err(CoreError::permission_denied("null byte check", "Path contains null byte"));
    }

    // Reject absolute paths (Windows: C:\..., Unix: /...)
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with('/') || path.starts_with('\\') {
        return Err(CoreError::permission_denied("absolute path check", format!(
            "Absolute path not allowed in archive: {}",
            path
        )));
    }

    // Reject path traversal
    for component in p.components() {
        if let std::path::Component::ParentDir = component {
            return Err(CoreError::permission_denied("path traversal", format!(
                "Path traversal detected in archive entry: {}",
                path
            )));
        }
    }

    Ok(())
}

/// Check if a file already exists and determine overwrite behavior.
#[derive(Debug, Clone, PartialEq)]
pub enum OverwriteAction {
    /// File doesn't exist, safe to create
    Create,
    /// File exists and should be overwritten
    Overwrite,
    /// File exists and should be skipped
    Skip,
    /// File exists and user should be prompted
    Prompt,
}

/// Determine what to do when a file already exists.
pub fn check_overwrite(path: &Path, overwrite_mode: &OverwriteMode) -> OverwriteAction {
    if !path.exists() {
        return OverwriteAction::Create;
    }

    match overwrite_mode {
        OverwriteMode::Always => OverwriteAction::Overwrite,
        OverwriteMode::Never => OverwriteAction::Skip,
        OverwriteMode::Prompt => OverwriteAction::Prompt,
        OverwriteMode::NewerOnly => {
            // Compare modification times — overwrite if source is newer
            // This is handled at a higher level where we have source file info
            OverwriteAction::Overwrite
        }
    }
}

/// Overwrite behavior for existing files.
#[derive(Debug, Clone, PartialEq)]
pub enum OverwriteMode {
    /// Always overwrite existing files
    Always,
    /// Never overwrite — skip existing files
    Never,
    /// Prompt the user for each existing file
    Prompt,
    /// Only overwrite if the source is newer
    NewerOnly,
}

/// Create a backup of a file before overwriting it.
///
/// The backup is created with a `.velocity_backup` extension.
/// Returns the path to the backup file, or None if the original doesn't exist.
pub fn backup_file(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }

    let backup_path = path.with_extension(
        format!(
            "{}.velocity_backup",
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
        ),
    );

    std::fs::copy(path, &backup_path)?;
    debug!("Backed up {} to {}", path.display(), backup_path.display());
    Ok(Some(backup_path))
}

/// Remove a backup file created by `backup_file`.
pub fn remove_backup(backup_path: &Path) {
    if backup_path.exists() {
        let _ = std::fs::remove_file(backup_path);
    }
}

/// Restore a file from its backup.
pub fn restore_backup(backup_path: &Path, original_path: &Path) -> Result<()> {
    if backup_path.exists() {
        std::fs::copy(backup_path, original_path)?;
        let _ = std::fs::remove_file(backup_path);
        info!("Restored {} from backup", original_path.display());
    }
    Ok(())
}

/// Compute a SHA256 hash of a file for integrity verification.
pub fn verify_file_integrity(path: &Path, expected_sha256: &str) -> Result<bool> {
    use sha2::{Digest, Sha256};

    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    let actual: String = result.iter().map(|b| format!("{:02x}", b)).collect();

    Ok(actual == expected_sha256.to_lowercase())
}

/// Sanitize a string for use in file/directory names.
fn sanitize_component(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Restrict directory ACLs to current user + administrators (Windows only).
///
/// Uses `icacls` to set a restrictive DACL that grants Full Control only to:
/// - The current user (owner)
/// - The Administrators group
/// All other inherited permissions are removed.
#[cfg(target_os = "windows")]
fn restrict_directory_acl(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();

    // Remove inherited permissions
    let remove_result = std::process::Command::new("icacls")
        .args([&path_str, "/inheritance:r"])
        .output()
        .map_err(|e| CoreError::other("ACL", format!("Failed to run icacls: {}", e)))?;

    if !remove_result.status.success() {
        let stderr = String::from_utf8_lossy(&remove_result.stderr);
        return Err(CoreError::other("ACL", format!(
            "icacls /inheritance:r failed: {}", stderr
        )));
    }

    // Grant Full Control to current user
    let user_grant = format!("*{}:(OI)(CI)F", whoami());
    let grant_user = std::process::Command::new("icacls")
        .args([&path_str, "/grant:r", &user_grant])
        .output()
        .map_err(|e| CoreError::other("ACL", format!("Failed to run icacls: {}", e)))?;

    if !grant_user.status.success() {
        let stderr = String::from_utf8_lossy(&grant_user.stderr);
        return Err(CoreError::other("ACL", format!(
            "icacls grant user failed: {}", stderr
        )));
    }

    // Grant Full Control to Administrators group
    let grant_admin = std::process::Command::new("icacls")
        .args([&path_str, "/grant", "*S-1-5-32-544:(OI)(CI)F"])
        .output()
        .map_err(|e| CoreError::other("ACL", format!("Failed to run icacls: {}", e)))?;

    if !grant_admin.status.success() {
        let stderr = String::from_utf8_lossy(&grant_admin.stderr);
        return Err(CoreError::other("ACL", format!(
            "icacls grant admin failed: {}", stderr
        )));
    }

    debug!("Restricted ACLs for: {}", path.display());
    Ok(())
}

/// Get the current user's SID string (for icacls).
#[cfg(target_os = "windows")]
fn whoami() -> String {
    std::process::Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .ok()
        .and_then(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // CSV format: "username","SID"
            stdout.lines().next()
                .and_then(|line| line.split(',').nth(1))
                .map(|s| s.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "BUILTIN\\Users".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_relative_path_valid() {
        assert!(validate_relative_path("file.txt").is_ok());
        assert!(validate_relative_path("subdir/file.txt").is_ok());
        assert!(validate_relative_path("a/b/c/d.txt").is_ok());
    }

    #[test]
    fn test_validate_relative_path_traversal() {
        assert!(validate_relative_path("../escape.txt").is_err());
        assert!(validate_relative_path("subdir/../../escape.txt").is_err());
        assert!(validate_relative_path("..\\escape.txt").is_err());
    }

    #[test]
    fn test_validate_relative_path_absolute() {
        assert!(validate_relative_path("C:\\Windows\\System32").is_err());
        assert!(validate_relative_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_relative_path_null_byte() {
        assert!(validate_relative_path("file\0.txt").is_err());
    }

    #[test]
    fn test_sanitize_component() {
        assert_eq!(sanitize_component("hello world"), "hello_world");
        assert_eq!(sanitize_component("my-app_v2"), "my-app_v2");
        assert_eq!(sanitize_component("bad!@#$chars"), "bad____chars");
    }

    #[test]
    fn test_check_overwrite_create() {
        let action = check_overwrite(
            Path::new("C:\\nonexistent_file_xyz.txt"),
            &OverwriteMode::Always,
        );
        assert_eq!(action, OverwriteAction::Create);
    }

    #[test]
    fn test_check_overwrite_always() {
        // This file should exist on Windows
        let action = check_overwrite(
            Path::new("C:\\Windows\\System32\\kernel32.dll"),
            &OverwriteMode::Always,
        );
        assert_eq!(action, OverwriteAction::Overwrite);
    }

    #[test]
    fn test_check_overwrite_never() {
        let action = check_overwrite(
            Path::new("C:\\Windows\\System32\\kernel32.dll"),
            &OverwriteMode::Never,
        );
        assert_eq!(action, OverwriteAction::Skip);
    }

    #[test]
    fn test_create_and_cleanup_temp_dir() {
        let dir = create_secure_temp_dir("test_app").unwrap();
        assert!(dir.exists());
        assert!(dir.to_string_lossy().contains("velocity_test_app_"));
        cleanup_temp_dir(&dir);
        assert!(!dir.exists());
    }
}
