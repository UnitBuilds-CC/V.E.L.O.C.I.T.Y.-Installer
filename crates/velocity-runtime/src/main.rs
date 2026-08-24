//! Velocity Runtime — the lightweight binary embedded in each installer.
//!
//! When a user runs a Velocity-built installer, this runtime:
//! 1. Reads the embedded manifest and payload
//! 2. Checks for silent mode (/S flag)
//! 3. Shows the installation wizard (or uses defaults in silent mode)
//! 4. Checks disk space
//! 5. Checks if the app is already running
//! 6. Extracts files to the chosen directory (with rollback tracking)
//! 7. Creates registry entries, shortcuts, file associations, etc.
//! 8. Generates the uninstaller
//! 9. Shows the completion dialog
//! 10. On failure, rolls back all changes

// Release builds are GUI apps (no console window).
// Debug builds keep the console for development output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use tracing::{error, info, warn};
use velocity_core::logging;
use velocity_core::rollback::RollbackTracker;

// Windows-specific module (contains the full installer logic)
#[cfg(target_os = "windows")]
mod windows;

// Cross-platform module for Linux and macOS
#[cfg(not(target_os = "windows"))]
mod unix;

// Cloud-fetch installer for Ninite-style installations
mod fetch_installer;

/// Command-line arguments parsed at startup.
struct RuntimeArgs {
    /// Silent/quiet mode — no UI, use defaults.
    silent: bool,
    /// Override install directory.
    dir: Option<String>,
    /// Force uninstall without confirmation.
    force: bool,
    /// Password for encrypted installers.
    password: Option<String>,
    /// Force modern WebView2 wizard regardless of manifest theme.
    modern_ui: bool,
    /// Elevated process — skip wizard, use dir from args.
    elevated: bool,
}

impl RuntimeArgs {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut silent = false;
        let mut dir = None;
        let mut force = false;
        let mut password = None;
        let mut modern_ui = false;
        let mut elevated = false;

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "/S" | "/s" | "--silent" | "-s" | "/quiet" | "-q" => silent = true,
                "--force" | "-f" => force = true,
                "--modern" | "--webview" => modern_ui = true,
                "--elevated" => elevated = true,
                _ => {
                    // Check for /D= prefix (Inno Setup compatible directory override)
                    if arg.starts_with("/D=") || arg.starts_with("/d=") {
                        dir = Some(arg[3..].to_string());
                    } else if arg.starts_with("/P=") || arg.starts_with("/p=") {
                        password = Some(arg[3..].to_string());
                    }
                }
            }
        }

        Self {
            silent,
            dir,
            force,
            password,
            modern_ui,
            elevated,
        }
    }
}

fn main() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::run()
    }

    #[cfg(not(target_os = "windows"))]
    {
        unix::run()
    }
}

// ---------------------------------------------------------------------------
// Runtime input validation
// ---------------------------------------------------------------------------

/// Maximum allowed password length (prevents PBKDF2 DoS with extremely long passwords).
const MAX_PASSWORD_LENGTH: usize = 1024;

/// Validate the install directory to prevent installation to dangerous system paths.
///
/// Rejects:
/// - Null bytes (path traversal via embedded null)
/// - Windows system directories (C:\Windows, C:\Windows\System32, etc.)
/// - Root of a drive (C:\, D:\) — user should pick a specific directory
/// - Paths longer than 240 characters (MAX_PATH safety margin)
fn validate_install_dir(path: &std::path::Path) -> std::result::Result<(), String> {
    let path_str = path.to_string_lossy();

    // Reject null bytes
    if path_str.contains('\0') {
        return Err("Install path contains invalid null byte".to_string());
    }

    // Reject empty paths
    if path_str.is_empty() {
        return Err("Install path is empty".to_string());
    }

    // Reject paths longer than MAX_PATH safety margin
    if path_str.len() > 240 {
        return Err(format!(
            "Install path is too long ({} chars, max 240)",
            path_str.len()
        ));
    }

    // Normalize to lowercase for comparison
    let normalized = path_str.to_lowercase();
    let normalized_trimmed = normalized.trim_end_matches('\\').trim_end_matches('/');

    // Reject drive roots (e.g., "C:\", "D:\")
    if normalized_trimmed.len() <= 3 && normalized_trimmed.ends_with(':') {
        return Err(
            "Cannot install to the root of a drive. Please choose a specific directory."
                .to_string(),
        );
    }

    // Reject dangerous Windows system directories.
    // Note: We reject the roots of these directories but allow subdirectories
    // (e.g., C:\Users\user\Desktop\app is fine, but C:\Users itself is not).
    let dangerous_exact = [
        "c:\\windows",
        "c:\\windows\\system32",
        "c:\\windows\\syswow64",
        "c:\\program files",
        "c:\\program files (x86)",
        "c:\\programdata",
        "c:\\users",
    ];

    // These are dangerous even as subdirectories (core OS files)
    let dangerous_prefix = ["c:\\windows\\system32", "c:\\windows\\syswow64"];

    for dangerous in &dangerous_exact {
        if normalized_trimmed == *dangerous {
            return Err(format!(
                "Cannot install to system directory '{}'. Please choose a different location.",
                dangerous
            ));
        }
    }

    for dangerous in &dangerous_prefix {
        if normalized_trimmed.starts_with(&format!("{}\\", dangerous)) {
            return Err(format!(
                "Cannot install to system directory '{}'. Please choose a different location.",
                dangerous
            ));
        }
    }

    Ok(())
}

/// Truncate a password to the maximum allowed length to prevent PBKDF2 DoS.
fn sanitize_password(password: &str) -> &str {
    if password.len() > MAX_PASSWORD_LENGTH {
        warn!(
            "Password truncated from {} to {} characters",
            password.len(),
            MAX_PASSWORD_LENGTH
        );
        &password[..MAX_PASSWORD_LENGTH]
    } else {
        password
    }
}

/// Validate a URL is safe to pass to `cmd /C start` (no shell metacharacters).
///
/// Only allows URLs that start with http:// or https:// and contain no
/// characters that could be used for command injection.
fn is_safe_url_for_shell(url: &str) -> bool {
    // Must start with http:// or https://
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    // Reject shell metacharacters that could enable command injection
    let dangerous_chars = [
        '&', '|', ';', '`', '$', '(', ')', '{', '}', '<', '>', '"', '\'', '\\', '\n', '\r', '\0',
    ];
    !url.chars().any(|c| dangerous_chars.contains(&c))
}

/// Core Unix install path validation (pure function, testable on all platforms).
///
/// Rejects null bytes, empty paths, overly long paths, the filesystem root,
/// and dangerous system directories on Linux and macOS.
#[cfg(any(test, not(target_os = "windows")))]
fn validate_unix_install_path(path_str: &str) -> std::result::Result<(), String> {
    if path_str.contains('\0') {
        return Err("Install path contains invalid null byte".to_string());
    }
    if path_str.is_empty() {
        return Err("Install path is empty".to_string());
    }
    if path_str.len() > 4096 {
        return Err(format!(
            "Install path is too long ({} chars, max 4096)",
            path_str.len()
        ));
    }
    let normalized = path_str.trim_end_matches('/');
    if normalized.is_empty() || normalized == "/" {
        return Err("Cannot install to the filesystem root".to_string());
    }
    let dangerous = [
        "/bin",
        "/sbin",
        "/usr",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/share",
        "/usr/include",
        "/etc",
        "/dev",
        "/proc",
        "/sys",
        "/boot",
        "/lib",
        "/lib64",
        "/var",
        "/tmp",
        "/home",
        "/root",
        "/private",
        "/System",
        "/Library",
    ];
    for d in &dangerous {
        if normalized == *d {
            return Err(format!(
                "Cannot install to system directory '{}'. Please choose a different location.",
                d
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_install_dir_valid() {
        assert!(validate_install_dir(std::path::Path::new("C:\\Program Files\\MyApp")).is_ok());
        assert!(validate_install_dir(std::path::Path::new("D:\\Games\\MyGame")).is_ok());
        assert!(
            validate_install_dir(std::path::Path::new("C:\\Users\\user\\Desktop\\app")).is_ok()
        );
    }

    #[test]
    fn test_validate_install_dir_rejects_system_dirs() {
        assert!(validate_install_dir(std::path::Path::new("C:\\Windows")).is_err());
        assert!(validate_install_dir(std::path::Path::new("C:\\Windows\\System32")).is_err());
        assert!(validate_install_dir(std::path::Path::new("C:\\Windows\\SysWOW64")).is_err());
        assert!(validate_install_dir(std::path::Path::new("C:\\ProgramData")).is_err());
    }

    #[test]
    fn test_validate_install_dir_rejects_drive_root() {
        assert!(validate_install_dir(std::path::Path::new("C:\\")).is_err());
        assert!(validate_install_dir(std::path::Path::new("D:\\")).is_err());
    }

    #[test]
    fn test_validate_install_dir_rejects_null_byte() {
        assert!(validate_install_dir(std::path::Path::new("C:\\My\0App")).is_err());
    }

    #[test]
    fn test_validate_install_dir_rejects_too_long() {
        let long_path = format!("C:\\{}", "A".repeat(250));
        assert!(validate_install_dir(std::path::Path::new(&long_path)).is_err());
    }

    #[test]
    fn test_sanitize_password_short() {
        let pw = "short_password";
        assert_eq!(sanitize_password(pw), pw);
    }

    #[test]
    fn test_sanitize_password_too_long() {
        let pw = "A".repeat(2000);
        let sanitized = sanitize_password(&pw);
        assert_eq!(sanitized.len(), MAX_PASSWORD_LENGTH);
    }

    #[test]
    fn test_is_safe_url_for_shell_valid() {
        assert!(is_safe_url_for_shell("https://example.com/download"));
        assert!(is_safe_url_for_shell(
            "http://releases.example.com/v1.0/installer.exe"
        ));
    }

    #[test]
    fn test_is_safe_url_for_shell_rejects_injection() {
        assert!(!is_safe_url_for_shell("https://example.com/path&calc.exe"));
        assert!(!is_safe_url_for_shell("https://example.com/path|del"));
        assert!(!is_safe_url_for_shell("https://example.com/path;rm"));
        assert!(!is_safe_url_for_shell("ftp://example.com/file")); // not http(s)
        assert!(!is_safe_url_for_shell("javascript:alert(1)"));
    }

    // -- Unix path validation tests (run on all platforms) --

    #[test]
    fn test_unix_valid_paths() {
        assert!(validate_unix_install_path("/opt/my-app").is_ok());
        assert!(validate_unix_install_path("/Applications/MyApp").is_ok());
        assert!(validate_unix_install_path("/home/user/.local/share/myapp").is_ok());
        assert!(validate_unix_install_path("/opt/my-app/lib/plugins/v2").is_ok());
    }

    #[test]
    fn test_unix_trailing_slash_normalized() {
        assert!(validate_unix_install_path("/opt/my-app/").is_ok());
        assert!(validate_unix_install_path("/opt/my-app///").is_ok());
    }

    #[test]
    fn test_unix_rejects_empty_and_null() {
        assert!(validate_unix_install_path("").is_err());
        assert!(validate_unix_install_path("/opt/my\0app").is_err());
    }

    #[test]
    fn test_unix_rejects_too_long() {
        let long = format!("/opt/{}", "a".repeat(4100));
        assert!(validate_unix_install_path(&long)
            .unwrap_err()
            .contains("too long"));
    }

    #[test]
    fn test_unix_accepts_max_length() {
        // "/opt/" is 5 chars + 4091 = 4096 total
        let path = format!("/opt/{}", "a".repeat(4091));
        assert_eq!(path.len(), 4096);
        assert!(validate_unix_install_path(&path).is_ok());
    }

    #[test]
    fn test_unix_rejects_root() {
        assert!(validate_unix_install_path("/").is_err());
        assert!(validate_unix_install_path("//").is_err());
    }

    #[test]
    fn test_unix_rejects_dangerous_dirs() {
        for dir in &[
            "/bin",
            "/sbin",
            "/usr",
            "/usr/bin",
            "/usr/sbin",
            "/usr/lib",
            "/usr/share",
            "/etc",
            "/dev",
            "/proc",
            "/sys",
            "/boot",
            "/lib",
            "/lib64",
            "/var",
            "/tmp",
            "/home",
            "/root",
            "/private",
            "/System",
            "/Library",
        ] {
            assert!(
                validate_unix_install_path(dir).is_err(),
                "Should reject {}",
                dir
            );
        }
    }

    #[test]
    fn test_unix_rejects_dangerous_with_trailing_slash() {
        assert!(validate_unix_install_path("/etc/").is_err());
        assert!(validate_unix_install_path("/bin/").is_err());
    }

    #[test]
    fn test_unix_allows_subdirs_of_dangerous() {
        assert!(validate_unix_install_path("/usr/local/myapp").is_ok());
        assert!(validate_unix_install_path("/opt/custom/app").is_ok());
    }
}
