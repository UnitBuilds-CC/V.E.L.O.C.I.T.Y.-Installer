//! macOS platform implementations.

use std::path::PathBuf;

/// Detect system architecture via `uname -m` and `sysctl`.
pub fn detect_arch() -> String {
    if let Ok(output) = std::process::Command::new("uname").arg("-m").output() {
        let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return match arch.as_str() {
            "x86_64" => "x64".to_string(),
            "arm64" => "arm64".to_string(),
            "i386" => "x86".to_string(),
            other => other.to_string(),
        };
    }
    if cfg!(target_arch = "x86_64") {
        "x64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "arm64".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Check if running as root (uid 0).
pub fn is_elevated() -> bool {
    extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() == 0 }
}

/// Default installation directory (e.g. `/Applications/AppName`).
pub fn default_install_dir(app_name: &str) -> PathBuf {
    PathBuf::from("/Applications").join(app_name)
}

/// Default configuration directory (`~/Library/Application Support`).
pub fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("Library").join("Application Support"))
        .unwrap_or_else(|| PathBuf::from("/Library/Application Support"))
}

/// Request reboot on macOS.
pub fn request_system_reboot() -> crate::error::Result<bool> {
    let output = std::process::Command::new("shutdown")
        .args(["-r", "now"])
        .output()
        .map_err(|e| {
            crate::error::CoreError::other("reboot", format!("Failed to run shutdown: {}", e))
        })?;
    if output.status.success() {
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(crate::error::CoreError::other(
            "reboot",
            format!("Failed to reboot: {}", stderr),
        ))
    }
}

/// macOS doesn't have a reboot-pending concept like Windows.
pub fn is_reboot_pending() -> bool {
    false
}

/// User's desktop directory.
pub fn desktop_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("Desktop"))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Applications directory.
pub fn start_menu_dir() -> PathBuf {
    PathBuf::from("/Applications")
}
