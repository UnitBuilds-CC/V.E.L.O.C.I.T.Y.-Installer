//! Condition expression evaluator for dependency and component gating.
//!
//! Supports the following condition formats:
//! - `"always"` — always true
//! - `"never"` — always false
//! - `"file_missing:/path/to/file"` — true if file doesn't exist
//! - `"file_exists:/path/to/file"` — true if file exists
//! - `"dir_exists:/path/to/dir"` — true if directory exists
//! - `"arch:x64"` — true if OS is x64
//! - `"arch:x86"` — true if OS is x86
//! - `"arch:arm64"` — true if OS is ARM64
//! - `"is64bitos"` — true if 64-bit OS (Inno Setup compat)
//! - `"is32bitos"` — true if 32-bit OS (Inno Setup compat)
//! - `"env:VAR_NAME"` — true if environment variable is set and non-empty
//! - `"env_equals:VAR_NAME=value"` — true if env var equals value
//!
//! Windows-only conditions (return error on Unix):
//! - `"registry_missing:HKLM\\Software\\..."` — true if registry key is absent
//! - `"registry_exists:HKLM\\Software\\..."` — true if registry key exists
//! - `"registry_value_missing:HKLM\\...\\ValueName"` — true if value is absent
//! - `"registry_value_exists:HKLM\\...\\ValueName"` — true if value exists
//! - `"not_installed:Product Name"` — true if not in Add/Remove Programs
//! - `"installed:Product Name"` — true if found in Add/Remove Programs
//! - `"winver_at_least:10.0"` — true if Windows version >= specified
//! - `"service_exists:ServiceName"` — true if Windows service exists
//! - `"service_running:ServiceName"` — true if service is running

use crate::error::{CoreError, Result};

/// Evaluate a condition expression string.
///
/// Returns `true` if the condition is met, `false` otherwise.
/// Returns an error if the condition string is malformed.
pub fn evaluate_condition(condition: &str) -> Result<bool> {
    let condition = condition.trim();

    if condition.is_empty() || condition == "always" {
        return Ok(true);
    }

    if condition == "never" {
        return Ok(false);
    }

    // Inno Setup compatibility aliases
    if condition == "is64bitos" || condition == "is64bit" {
        return Ok(crate::arch_detect::is_64bit_os());
    }
    if condition == "is32bitos" || condition == "is32bit" {
        return Ok(!crate::arch_detect::is_64bit_os());
    }

    // Parse "type:argument" format
    let (cond_type, argument) = match condition.find(':') {
        Some(pos) => (&condition[..pos], condition[pos + 1..].trim()),
        None => {
            return Err(CoreError::other(
                "condition",
                format!(
                    "Invalid condition format: '{}'. Expected 'type:argument' or 'always'/'never'.",
                    condition
                ),
            ));
        }
    };

    if argument.is_empty() {
        return Err(CoreError::other(
            "condition",
            format!("Empty argument in condition: '{}'", condition),
        ));
    }

    match cond_type {
        // Cross-platform conditions
        "file_missing" => Ok(!std::path::Path::new(argument).exists()),
        "file_exists" => Ok(std::path::Path::new(argument).exists()),
        "dir_exists" => Ok(std::path::Path::new(argument).is_dir()),
        "arch" => eval_arch_condition(argument),
        "env" => Ok(std::env::var(argument)
            .map(|v| !v.is_empty())
            .unwrap_or(false)),
        "env_equals" => eval_env_equals(argument),

        // Windows-only conditions
        #[cfg(target_os = "windows")]
        "registry_missing" => eval_registry_exists(argument).map(|exists| !exists),
        #[cfg(target_os = "windows")]
        "registry_exists" => eval_registry_exists(argument),
        #[cfg(target_os = "windows")]
        "registry_value_missing" => eval_registry_value_exists(argument).map(|exists| !exists),
        #[cfg(target_os = "windows")]
        "registry_value_exists" => eval_registry_value_exists(argument),
        #[cfg(target_os = "windows")]
        "not_installed" => eval_add_remove_installed(argument).map(|found| !found),
        #[cfg(target_os = "windows")]
        "installed" => eval_add_remove_installed(argument),
        #[cfg(target_os = "windows")]
        "winver_at_least" => eval_winver_at_least(argument),
        #[cfg(target_os = "windows")]
        "service_exists" => eval_service_exists(argument),
        #[cfg(target_os = "windows")]
        "service_running" => eval_service_running(argument),

        // Windows-only conditions called on Unix — return a clear error
        #[cfg(not(target_os = "windows"))]
        "registry_missing"
        | "registry_exists"
        | "registry_value_missing"
        | "registry_value_exists"
        | "not_installed"
        | "installed"
        | "winver_at_least"
        | "service_exists"
        | "service_running" => Err(CoreError::other(
            "condition",
            format!(
                "Condition type '{}' is only supported on Windows",
                cond_type
            ),
        )),

        _ => Err(CoreError::other(
            "condition",
            format!("Unknown condition type: '{}'", cond_type),
        )),
    }
}

/// Evaluate multiple conditions (all must be true).
pub fn evaluate_all_conditions(conditions: &[String]) -> Result<bool> {
    for condition in conditions {
        if !evaluate_condition(condition)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// ===========================================================================
// Cross-platform condition evaluators
// ===========================================================================

/// Evaluate an architecture condition.
fn eval_arch_condition(arch: &str) -> Result<bool> {
    let info = crate::arch_detect::detect_system_info();
    match arch.to_lowercase().as_str() {
        "x86" | "x86_32" | "win32" | "32bit" => {
            Ok(info.os_arch == crate::arch_detect::SystemArch::X86)
        }
        "x64" | "x86_64" | "amd64" | "64bit" => {
            Ok(info.os_arch == crate::arch_detect::SystemArch::X64)
        }
        "arm64" | "aarch64" => Ok(info.os_arch == crate::arch_detect::SystemArch::Arm64),
        _ => Err(CoreError::other(
            "condition",
            format!("Unknown architecture: '{}'. Use x86, x64, or arm64.", arch),
        )),
    }
}

/// Evaluate an env_equals condition.
///
/// Argument format: `VAR_NAME=value`
fn eval_env_equals(argument: &str) -> Result<bool> {
    let eq_pos = argument.find('=').ok_or_else(|| {
        CoreError::other(
            "condition",
            format!(
                "Invalid env_equals format: '{}'. Expected 'VAR_NAME=value'.",
                argument
            ),
        )
    })?;

    let var_name = &argument[..eq_pos];
    let expected_value = &argument[eq_pos + 1..];

    match std::env::var(var_name) {
        Ok(actual) => Ok(actual == expected_value),
        Err(_) => Ok(false),
    }
}

// ===========================================================================
// Windows-only condition evaluators
// ===========================================================================

/// Check if a registry key exists.
///
/// Argument format: `ROOT\\Sub\\Key` (e.g., `HKLM\\Software\\Microsoft\\Windows\\CurrentVersion`)
#[cfg(target_os = "windows")]
fn eval_registry_exists(key_path: &str) -> Result<bool> {
    use winreg::enums::*;

    let (root, sub_key) = parse_registry_path(key_path)?;

    let hive = match root.to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
        "HKU" | "HKEY_USERS" => HKEY_USERS,
        _ => {
            return Err(CoreError::other(
                "condition",
                format!("Unknown registry root: '{}'", root),
            ));
        }
    };

    match winreg::RegKey::predef(hive).open_subkey(sub_key) {
        Ok(_) => Ok(true),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(CoreError::other(
            "condition",
            format!("Registry check failed for '{}': {}", key_path, e),
        )),
    }
}

/// Check if a registry value exists.
///
/// Argument format: `ROOT\\Sub\\Key\\ValueName`
#[cfg(target_os = "windows")]
fn eval_registry_value_exists(path_with_value: &str) -> Result<bool> {
    use winreg::enums::*;

    // Split into key path and value name
    let last_sep = path_with_value.rfind('\\').ok_or_else(|| {
        CoreError::other(
            "condition",
            format!(
                "Invalid registry value path: '{}'. Expected 'ROOT\\Key\\ValueName'.",
                path_with_value
            ),
        )
    })?;

    let key_path = &path_with_value[..last_sep];
    let value_name = &path_with_value[last_sep + 1..];

    let (root, sub_key) = parse_registry_path(key_path)?;

    let hive = match root.to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
        "HKU" | "HKEY_USERS" => HKEY_USERS,
        _ => {
            return Err(CoreError::other(
                "condition",
                format!("Unknown registry root: '{}'", root),
            ));
        }
    };

    let key = match winreg::RegKey::predef(hive).open_subkey(sub_key) {
        Ok(key) => key,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(CoreError::other(
                "condition",
                format!("Registry check failed: {}", e),
            ))
        }
    };

    match key.get_raw_value(value_name) {
        Ok(_) => Ok(true),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(CoreError::other(
            "condition",
            format!("Registry value check failed: {}", e),
        )),
    }
}

/// Check if a product is installed via Add/Remove Programs.
#[cfg(target_os = "windows")]
fn eval_add_remove_installed(product_name: &str) -> Result<bool> {
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let search_name = product_name.to_lowercase();

    // Check both 64-bit and 32-bit uninstall keys
    let paths = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    for path in &paths {
        if let Ok(key) = winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(path) {
            for subkey_name in key.enum_keys().filter_map(|k| k.ok()) {
                if let Ok(subkey) = key.open_subkey(&subkey_name) {
                    if let Ok(display_name) = subkey.get_value::<String, _>("DisplayName") {
                        if display_name.to_lowercase().contains(&search_name) {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

/// Evaluate a Windows version condition.
///
/// Argument format: `major.minor` (e.g., `10.0` for Windows 10/11)
#[cfg(target_os = "windows")]
fn eval_winver_at_least(version_str: &str) -> Result<bool> {
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() != 2 {
        return Err(CoreError::other(
            "condition",
            format!(
                "Invalid version format: '{}'. Expected 'major.minor'.",
                version_str
            ),
        ));
    }

    let required_major: u32 = parts[0].parse().map_err(|_| {
        CoreError::other(
            "condition",
            format!("Invalid major version: '{}'", parts[0]),
        )
    })?;
    let required_minor: u32 = parts[1].parse().map_err(|_| {
        CoreError::other(
            "condition",
            format!("Invalid minor version: '{}'", parts[1]),
        )
    })?;

    // Get Windows version from registry (reliable method)
    let nt_key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");

    if let Ok(key) = nt_key {
        if let (Ok(major_str), Ok(minor_str)) = (
            key.get_value::<String, _>("CurrentMajorVersionNumber"),
            key.get_value::<String, _>("CurrentMinorVersionNumber"),
        ) {
            if let (Ok(major), Ok(minor)) = (major_str.parse::<u32>(), minor_str.parse::<u32>()) {
                return Ok(
                    major > required_major || (major == required_major && minor >= required_minor)
                );
            }
        }

        // Fallback: try CurrentVersion (older format)
        if let Ok(current_version) = key.get_value::<String, _>("CurrentVersion") {
            let cv_parts: Vec<&str> = current_version.split('.').collect();
            if cv_parts.len() >= 2 {
                if let (Ok(major), Ok(minor)) =
                    (cv_parts[0].parse::<u32>(), cv_parts[1].parse::<u32>())
                {
                    return Ok(major > required_major
                        || (major == required_major && minor >= required_minor));
                }
            }
        }
    }

    // Last resort: assume Windows 10+
    Ok(required_major <= 10)
}

/// Check if a Windows service exists.
#[cfg(target_os = "windows")]
fn eval_service_exists(service_name: &str) -> Result<bool> {
    let output = std::process::Command::new("sc")
        .args(["query", service_name])
        .output()
        .map_err(|e| CoreError::other("condition", format!("Failed to query service: {}", e)))?;

    // sc query returns 0 if service exists, 1060 if it doesn't
    Ok(output.status.success())
}

/// Check if a Windows service is running.
#[cfg(target_os = "windows")]
fn eval_service_running(service_name: &str) -> Result<bool> {
    let output = std::process::Command::new("sc")
        .args(["query", service_name])
        .output()
        .map_err(|e| CoreError::other("condition", format!("Failed to query service: {}", e)))?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("RUNNING"))
}

/// Parse a registry path into (root, sub_key).
///
/// Input: `HKLM\\Software\\Microsoft`
/// Output: `("HKLM", "Software\\Microsoft")`
#[cfg(target_os = "windows")]
fn parse_registry_path(path: &str) -> Result<(&str, &str)> {
    let sep = path.find('\\').ok_or_else(|| {
        CoreError::other(
            "condition",
            format!(
                "Invalid registry path: '{}'. Expected 'ROOT\\Sub\\Key'.",
                path
            ),
        )
    })?;

    Ok((&path[..sep], &path[sep + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_condition() {
        assert!(evaluate_condition("always").unwrap());
        assert!(evaluate_condition("").unwrap());
    }

    #[test]
    fn test_never_condition() {
        assert!(!evaluate_condition("never").unwrap());
    }

    #[test]
    fn test_file_exists() {
        // Use a path that exists on all platforms
        assert!(evaluate_condition("file_exists:.").unwrap());
        assert!(!evaluate_condition("file_exists:/nonexistent_file_xyz_123_absent").unwrap());
    }

    #[test]
    fn test_file_missing() {
        assert!(evaluate_condition("file_missing:/nonexistent_file_xyz_123_absent").unwrap());
        assert!(!evaluate_condition("file_missing:.").unwrap());
    }

    #[test]
    fn test_dir_exists() {
        assert!(evaluate_condition("dir_exists:.").unwrap());
        assert!(!evaluate_condition("dir_exists:/nonexistent_dir_xyz_12345").unwrap());
    }

    #[test]
    fn test_invalid_condition_format() {
        assert!(evaluate_condition("badcondition").is_err());
    }

    #[test]
    fn test_unknown_condition_type() {
        assert!(evaluate_condition("unknown_type:argument").is_err());
    }

    #[test]
    fn test_env_condition() {
        // PATH should be set on any system
        assert!(evaluate_condition("env:PATH").unwrap());
        assert!(!evaluate_condition("env:VELOCITY_NONEXISTENT_VAR_XYZ").unwrap());
    }

    #[test]
    fn test_arch_condition() {
        // Should not error for valid arch values
        let _ = evaluate_condition("arch:x86");
        let _ = evaluate_condition("arch:x64");
        let _ = evaluate_condition("arch:arm64");
        assert!(evaluate_condition("arch:sparc").is_err());
    }

    #[test]
    fn test_is64bitos_compat() {
        // Inno Setup compatibility
        let result = evaluate_condition("is64bitos").unwrap();
        let info = crate::arch_detect::detect_system_info();
        assert_eq!(result, info.supports_64bit);
    }

    #[test]
    fn test_evaluate_all_conditions() {
        let conditions = vec!["always".to_string(), "dir_exists:.".to_string()];
        assert!(evaluate_all_conditions(&conditions).unwrap());

        let conditions_with_false = vec!["always".to_string(), "never".to_string()];
        assert!(!evaluate_all_conditions(&conditions_with_false).unwrap());
    }

    #[test]
    fn test_env_equals_condition() {
        // Set a temp env var and check
        std::env::set_var("VELOCITY_TEST_ENV_EQ", "hello");
        assert!(evaluate_condition("env_equals:VELOCITY_TEST_ENV_EQ=hello").unwrap());
        assert!(!evaluate_condition("env_equals:VELOCITY_TEST_ENV_EQ=world").unwrap());
        assert!(!evaluate_condition("env_equals:VELOCITY_NONEXISTENT_XYZ=val").unwrap());
        std::env::remove_var("VELOCITY_TEST_ENV_EQ");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_parse_registry_path() {
        let (root, sub) = parse_registry_path(r"HKLM\Software\Microsoft").unwrap();
        assert_eq!(root, "HKLM");
        assert_eq!(sub, r"Software\Microsoft");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_parse_registry_path_invalid() {
        assert!(parse_registry_path("NOSLASH").is_err());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_windows_conditions_error_on_unix() {
        assert!(evaluate_condition("registry_exists:HKLM\\Software").is_err());
        assert!(evaluate_condition("registry_missing:HKLM\\Software").is_err());
        assert!(evaluate_condition("installed:SomeProduct").is_err());
        assert!(evaluate_condition("winver_at_least:10.0").is_err());
        assert!(evaluate_condition("service_exists:ssh").is_err());
        assert!(evaluate_condition("service_running:ssh").is_err());
    }
}
