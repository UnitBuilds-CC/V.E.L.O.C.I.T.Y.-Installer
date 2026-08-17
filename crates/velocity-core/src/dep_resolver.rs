//! Dependency condition resolver.
//!
//! Evaluates condition expressions to determine whether a dependency needs installation.
//!
//! Supported conditions:
//! - `"always"` — always returns true (needs install)
//! - `"never"` — always returns false (skip install)
//! - `"registry_missing:<path>"` — true if the registry key does NOT exist
//! - `"registry_exists:<path>"` — true if the registry key exists
//! - `"file_missing:<path>"` — true if the file does NOT exist
//! - `"file_exists:<path>"` — true if the file exists
//! - `"not_installed:<product_name>"` — true if product is not in Add/Remove Programs
//! - `"installed:<product_name>"` — true if product IS in Add/Remove Programs
//! - `"arch:<x64|x86|arm64>"` — true if current process arch matches
//! - `"os_version:>=<major>.<minor>"` — true if OS version matches

use tracing::debug;

/// Evaluate a dependency condition string.
/// Returns `true` if the dependency SHOULD be installed.
pub fn evaluate_condition(condition: &str) -> bool {
    let cond = condition.trim();

    if cond == "always" {
        return true;
    }
    if cond == "never" {
        return false;
    }

    if let Some(path) = cond.strip_prefix("registry_missing:") {
        return !registry_key_exists(path);
    }
    if let Some(path) = cond.strip_prefix("registry_exists:") {
        return registry_key_exists(path);
    }
    if let Some(path) = cond.strip_prefix("file_missing:") {
        return !std::path::Path::new(path).exists();
    }
    if let Some(path) = cond.strip_prefix("file_exists:") {
        return std::path::Path::new(path).exists();
    }
    if let Some(product) = cond.strip_prefix("not_installed:") {
        return !is_product_installed(product);
    }
    if let Some(product) = cond.strip_prefix("installed:") {
        return is_product_installed(product);
    }
    if let Some(arch) = cond.strip_prefix("arch:") {
        return check_arch(arch);
    }
    if let Some(ver) = cond.strip_prefix("os_version:") {
        return check_os_version(ver);
    }

    // Unknown condition — default to install (safe fallback)
    debug!("Unknown condition '{}', defaulting to install", cond);
    true
}

/// Check if a registry key exists.
/// Path format: `HKLM\Software\Foo` or `HKCU\Software\Bar`
fn registry_key_exists(path: &str) -> bool {
    use winreg::RegKey;

    let parts: Vec<&str> = path.splitn(2, '\\').collect();
    if parts.len() != 2 {
        return false;
    }

    let root = match parts[0].to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE),
        "HKCU" | "HKEY_CURRENT_USER" => RegKey::predef(winreg::enums::HKEY_CURRENT_USER),
        "HKCR" | "HKEY_CLASSES_ROOT" => RegKey::predef(winreg::enums::HKEY_CLASSES_ROOT),
        "HKU" | "HKEY_USERS" => RegKey::predef(winreg::enums::HKEY_USERS),
        _ => return false,
    };

    root.open_subkey(parts[1]).is_ok()
}

/// Check if a product is installed by searching Add/Remove Programs.
fn is_product_installed(product_name: &str) -> bool {
    use winreg::RegKey;

    let search_lower = product_name.to_lowercase();

    // Check both 32-bit and 64-bit uninstall paths
    let paths = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    for reg_path in &paths {
        if let Ok(key) = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(reg_path) {
            for subkey_name in key.enum_keys().filter_map(|r| r.ok()) {
                if let Ok(subkey) = key.open_subkey(&subkey_name) {
                    if let Ok(display_name) = subkey.get_value::<String, _>("DisplayName") {
                        if display_name.to_lowercase().contains(&search_lower) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Check if current architecture matches.
fn check_arch(arch: &str) -> bool {
    let current = std::env::consts::ARCH;
    match arch.to_lowercase().as_str() {
        "x64" | "x86_64" | "amd64" => current == "x86_64",
        "x86" | "i686" | "i386" => current == "x86",
        "aarch64" | "arm64" => current == "aarch64",
        _ => true,
    }
}

/// Check OS version against a constraint like `>=10.0` or `>=6.1`.
fn check_os_version(constraint: &str) -> bool {
    let constraint = constraint.trim();

    // Parse operator and version
    let (op, ver_str) = if constraint.starts_with(">=") {
        (">=", &constraint[2..])
    } else if constraint.starts_with("<=") {
        ("<=", &constraint[2..])
    } else if constraint.starts_with('>') {
        (">", &constraint[1..])
    } else if constraint.starts_with('<') {
        ("<", &constraint[1..])
    } else if constraint.starts_with('=') {
        ("=", &constraint[1..])
    } else {
        // Default to >= if no operator
        (">=", constraint)
    };

    // Parse version string
    let parts: Vec<u32> = ver_str
        .split('.')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if parts.is_empty() {
        return true;
    }

    let required_major = parts[0];
    let required_minor = parts.get(1).copied().unwrap_or(0);

    // Get actual OS version
    let info = windows_version();
    let actual_major = info.0;
    let actual_minor = info.1;

    match op {
        ">=" => actual_major > required_major || (actual_major == required_major && actual_minor >= required_minor),
        "<=" => actual_major < required_major || (actual_major == required_major && actual_minor <= required_minor),
        ">" => actual_major > required_major || (actual_major == required_major && actual_minor > required_minor),
        "<" => actual_major < required_major || (actual_major == required_major && actual_minor < required_minor),
        "=" => actual_major == required_major && actual_minor == required_minor,
        _ => true,
    }
}

/// Get Windows version as (major, minor, build).
fn windows_version() -> (u32, u32, u32) {
    // Use RtlGetVersion to get the real version (not manifested)
    #[repr(C)]
    struct OsVersionInfoEx {
        dw_os_version_info_size: u32,
        dw_major_version: u32,
        dw_minor_version: u32,
        dw_build_number: u32,
        dw_platform_id: u32,
        sz_csd_version: [u16; 128],
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(info: &mut OsVersionInfoEx) -> i32;
    }

    let mut info = OsVersionInfoEx {
        dw_os_version_info_size: std::mem::size_of::<OsVersionInfoEx>() as u32,
        dw_major_version: 0,
        dw_minor_version: 0,
        dw_build_number: 0,
        dw_platform_id: 0,
        sz_csd_version: [0; 128],
    };

    unsafe {
        if RtlGetVersion(&mut info) == 0 {
            (info.dw_major_version, info.dw_minor_version, info.dw_build_number)
        } else {
            // Fallback
            (10, 0, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_condition() {
        assert!(evaluate_condition("always"));
    }

    #[test]
    fn test_never_condition() {
        assert!(!evaluate_condition("never"));
    }

    #[test]
    fn test_file_missing_condition() {
        assert!(evaluate_condition("file_missing:C:\\nonexistent_file_xyz.dll"));
    }

    #[test]
    fn test_file_exists_condition() {
        // This file should exist on Windows
        assert!(evaluate_condition("file_exists:C:\\Windows\\System32\\kernel32.dll"));
    }

    #[test]
    fn test_unknown_condition_defaults_to_true() {
        assert!(evaluate_condition("some_unknown_condition"));
    }

    #[test]
    fn test_check_arch_x64() {
        // On a 64-bit system, this should return true
        if std::env::consts::ARCH == "x86_64" {
            assert!(check_arch("x64"));
            assert!(check_arch("x86_64"));
            assert!(check_arch("amd64"));
            assert!(!check_arch("x86"));
        }
    }

    #[test]
    fn test_os_version_check() {
        // Windows 10+ should satisfy >=10.0
        assert!(check_os_version(">=10.0"));
        // Should not satisfy <5.0
        assert!(!check_os_version("<5.0"));
    }

    #[test]
    fn test_registry_key_exists_hklm() {
        // This key should exist on any Windows system
        assert!(registry_key_exists(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion"));
    }

    #[test]
    fn test_registry_key_missing() {
        // This key should NOT exist
        assert!(!registry_key_exists(r"HKLM\SOFTWARE\VelocityNonExistentKey12345"));
    }
}
