//! Cross-platform abstraction layer.
//!
//! Each OS-specific submodule implements the same set of operations
//! (elevation, shortcuts, services, env vars, file associations, config storage,
//! reboot management, architecture detection).  The rest of the codebase can
//! either call the platform module directly or keep using the existing
//! higher-level modules (arch_detect, elevation, shortcuts, …) which delegate
//! here on Windows and provide equivalent behaviour on other platforms.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::path::PathBuf;

// Re-export the active platform's implementations under a uniform namespace.
#[cfg(target_os = "linux")]
use linux as ops;
#[cfg(target_os = "macos")]
use macos as ops;
#[cfg(target_os = "windows")]
use windows as ops;

// ---------------------------------------------------------------------------
// Public cross-platform API
// ---------------------------------------------------------------------------

/// Detect the system architecture as a human-readable string (e.g. "x64", "arm64").
pub fn detect_arch() -> String {
    ops::detect_arch()
}

/// Check whether the current process is running with elevated privileges.
pub fn is_elevated() -> bool {
    ops::is_elevated()
}

/// Return the default installation directory for the given application name.
pub fn default_install_dir(app_name: &str) -> PathBuf {
    ops::default_install_dir(app_name)
}

/// Return the default configuration / data directory.
pub fn default_config_dir() -> PathBuf {
    ops::default_config_dir()
}

/// Fill `buf` with cryptographically secure random bytes.
///
/// Uses the OS CSPRNG on every platform (BCryptGenRandom on Windows,
/// getrandom syscall on Linux, getentropy on macOS).
pub fn fill_random(buf: &mut [u8]) {
    // getrandom works identically on all three platforms — no need to dispatch.
    if getrandom::getrandom(buf).is_err() {
        // Extremely unlikely fallback — should never happen on a functioning OS.
        tracing::error!("getrandom failed — falling back to time-based randomness");
        use sha2::Digest;
        use std::time::{SystemTime, UNIX_EPOCH};
        let time_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut h = sha2::Sha256::new();
        h.update(time_nanos.to_le_bytes());
        h.update(std::process::id().to_le_bytes());
        let hash = h.finalize();
        let len = buf.len().min(hash.len());
        buf[..len].copy_from_slice(&hash[..len]);
    }
}

/// Request a system reboot.
pub fn request_system_reboot() -> crate::error::Result<bool> {
    ops::request_system_reboot()
}

/// Check whether a system reboot is pending.
pub fn is_reboot_pending() -> bool {
    ops::is_reboot_pending()
}

/// Return the path to the user's desktop directory.
pub fn desktop_dir() -> PathBuf {
    ops::desktop_dir()
}

/// Return the path to the user's start-menu / applications directory.
pub fn start_menu_dir() -> PathBuf {
    ops::start_menu_dir()
}
