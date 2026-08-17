//! Linux platform implementations.

use std::path::PathBuf;

/// Detect system architecture via `uname -m` and compile-time checks.
pub fn detect_arch() -> String {
    if let Ok(output) = std::process::Command::new("uname").arg("-m").output() {
        let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return match arch.as_str() {
            "x86_64" => "x64".to_string(),
            "aarch64" => "arm64".to_string(),
            "i686" | "i386" => "x86".to_string(),
            "armv7l" => "arm".to_string(),
            other => other.to_string(),
        };
    }
    if cfg!(target_arch = "x86_64") {
        "x64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "arm64".to_string()
    } else if cfg!(target_arch = "x86") {
        "x86".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Check if running as root (uid 0).
pub fn is_elevated() -> bool {
    unsafe { libc_getuid() == 0 }
}

/// Minimal FFI to getuid() without pulling in the libc crate.
extern "C" {
    fn getuid() -> u32;
}
unsafe fn libc_getuid() -> u32 {
    unsafe { getuid() }
}

/// Default installation directory (e.g. `/opt/app-name`).
pub fn default_install_dir(app_name: &str) -> PathBuf {
    PathBuf::from("/opt").join(app_name.to_lowercase().replace(' ', "-"))
}

/// Default configuration directory (XDG config home).
pub fn default_config_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".config"))
                .unwrap_or_else(|| PathBuf::from("/etc"))
        })
}

/// Request reboot on Linux (just a notification — Linux rarely needs reboots).
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

/// Linux doesn't typically have reboot-pending state like Windows.
pub fn is_reboot_pending() -> bool {
    // Check for /var/run/reboot-required (Debian/Ubuntu convention)
    std::path::Path::new("/var/run/reboot-required").exists()
}

/// User's desktop directory.
pub fn desktop_dir() -> PathBuf {
    std::env::var("XDG_DESKTOP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join("Desktop"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        })
}

/// Applications directory (freedesktop .desktop files).
pub fn start_menu_dir() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(|p| PathBuf::from(p).join("applications"))
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".local").join("share").join("applications"))
                .unwrap_or_else(|| PathBuf::from("/usr/share/applications"))
        })
}
