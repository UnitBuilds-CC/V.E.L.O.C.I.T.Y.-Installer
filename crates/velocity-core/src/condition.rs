//! Condition expression evaluator for dependency and component gating.
//!
//! Supports the following condition formats:
//! - `"always"` — always true
//! - `"never"` — always false
//! - `"registry_missing:HKLM\\Software\\..."` — true if registry key is absent
//! - `"registry_exists:HKLM\\Software\\..."` — true if registry key exists
//! - `"registry_value_missing:HKLM\\Software\\...\\ValueName"` — true if value is absent
//! - `"registry_value_exists:HKLM\\Software\\...\\ValueName"` — true if value exists
//! - `"file_missing:C:\\path\\to\\file.dll"` — true if file doesn't exist
//! - `"file_exists:C:\\path\\to\\file.dll"` — true if file exists
//! - `"dir_exists:C:\\path\\to\\dir"` — true if directory exists
//! - `"not_installed:Product Name"` — true if not in Add/Remove Programs
//! - `"installed:Product Name"` — true if found in Add/Remove Programs
//! - `"arch:x64"` — true if OS is x64
//! - `"arch:x86"` — true if OS is x86
//! - `"arch:arm64"` — true if OS is ARM64
//! - `"is64bitos"` — true if 64-bit OS (Inno Setup compat)
//! - `"is32bitos"` — true if 32-bit OS (Inno Setup compat)
//! - `"winver_at_least:10.0"` — true if Windows version >= specified
//! - `"service_exists:ServiceName"` — true if Windows service exists
//! - `"service_running:ServiceName"` — true if service is running
//! - `"env:VAR_NAME"` — true if environment variable is set and non-empty
//! - `"env_equals:VAR_NAME=value"` — true if env var equals value

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
            return Err(CoreError::other("condition", format!(
                "Invalid condition format: '{}'. Expected 'type:argument' or 'always'/'never'.",
                condition
            )));
        }
    };

    if argument.is_empty() {
        return Err(CoreError::other("condition", format!(
            "Empty argument in condition: '{}'", condition
        )));
    }

    match cond_type {
        "registry_missing" => eval_registry_exists(argument).map(|exists| !exists),
        "registry_exists" => eval_registry_exists(argument),
        "registry_value_missing" => eval_registry_value_exists(argument).map(|exists| !exists),
        "registry_value_exists" => eval_registry_value_exists(argument),
        "file_missing" => Ok(!std::path::Path::new(argument).exists()),
        "file_exists" => Ok(std::path::Path::new(argument).exists()),
        "dir_exists" => Ok(std::path::Path::new(argument).is_dir()),
        "not_installed" => eval_add_remove_installed(argument).map(|found| !found),
        "installed" => eval_add_remove_installed(argument),
        "arch" => eval_arch_condition(argument),
        "winver_at_least" => eval_winver_at_least(argument),
        "service_exists" => eval_service_exists(argument),
        "service_running" => eval_service_running(argument),
        "env" => Ok(std::env::var(argument).map(|v| !v.is_empty()).unwrap_or(false)),
        "env_equals" => eval_env_equals(argument),
        _ => Err(CoreError::other("condition", format!(
            "Unknown condition type: '{}'", cond_type
        ))),
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

/// Check if a registry key exists.
///
/// Argument format: `ROOT\\Sub\\Key` (e.g., `HKLM\\Software\\Microsoft\\Windows\\CurrentVersion`)
fn eval_registry_exists(key_path: &str) -> Result<bool> {
    use winreg::enums::*;

    let (root, sub_key) = parse_registry_path(key_path)?;

    let hive = match root.to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
        "HKU" | "HKEY_USERS" => HKEY_USERS,
        _ => {
            return Err(CoreError::other("condition", format!(
                "Unknown registry root: '{}'", root
            )));
        }
    };

    match winreg::RegKey::predef(hive).open_subkey(sub_key) {
        Ok(_) => Ok(true),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(CoreError::other("condition", format!(
            "Registry check failed for '{}': {}", key_path, e
        ))),
    }
}

/// Check if a registry value exists.
///
/// Argument format: `ROOT\\Sub\\Key\\ValueName`
fn eval_registry_value_exists(path_with_value: &str) -> Result<bool> {
    use winreg::enums::*;

    // Split into key path and value name
    let last_sep = path_with_value.rfind('\\')
        .ok_or_else(|| CoreError::other("condition", format!(
            "Invalid registry value path: '{}'. Expected 'ROOT\\Key\\ValueName'.",
            path_with_value
        )))?;

    let key_path = &path_with_value[..last_sep];
    let value_name = &path_with_value[last_sep + 1..];

    let (root, sub_key) = parse_registry_path(key_path)?;

    let hive = match root.to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
        "HKU" | "HKEY_USERS" => HKEY_USERS,
        _ => {
            return Err(CoreError::other("condition", format!(
                "Unknown registry root: '{}'", root
            )));
        }
    };

    let key = match winreg::RegKey::predef(hive).open_subkey(sub_key) {
        Ok(key) => key,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(CoreError::other("condition", format!(
            "Registry check failed: {}", e
        ))),
    };

    match key.get_raw_value(value_name) {
        Ok(_) => Ok(true),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(CoreError::other("condition", format!(
            "Registry value check failed: {}", e
        ))),
    }
}

/// Check if a product is installed via Add/Remove Programs.
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

/// Evaluate an architecture condition.
fn eval_arch_condition(arch: &str) -> Result<bool> {
    let info = crate::arch_detect::detect_system_info();
    match arch.to_lowercase().as_str() {
        "x86" | "x86_32" | "win32" | "32bit" => Ok(info.os_arch == crate::arch_detect::SystemArch::X86),
        "x64" | "x86_64" | "amd64" | "64bit" => Ok(info.os_arch == crate::arch_detect::SystemArch::X64),
        "arm64" | "aarch64" => Ok(info.os_arch == crate::arch_detect::SystemArch::Arm64),
        _ => Err(CoreError::other("condition", format!(
            "Unknown architecture: '{}'. Use x86, x64, or arm64.", arch
        ))),
    }
}

/// Evaluate a Windows version condition.
///
/// Argument format: `major.minor` (e.g., `10.0` for Windows 10/11)
fn eval_winver_at_least(version_str: &str) -> Result<bool> {
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() != 2 {
        return Err(CoreError::other("condition", format!(
            "Invalid version format: '{}'. Expected 'major.minor'.", version_str
        )));
    }

    let required_major: u32 = parts[0].parse().map_err(|_| {
        CoreError::other("condition", format!("Invalid major version: '{}'", parts[0]))
    })?;
    let required_minor: u32 = parts[1].parse().map_err(|_| {
        CoreError::other("condition", format!("Invalid minor version: '{}'", parts[1]))
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
                return Ok(major > required_major || (major == required_major && minor >= required_minor));
            }
        }

        // Fallback: try CurrentVersion (older format)
        if let Ok(current_version) = key.get_value::<String, _>("CurrentVersion") {
            let cv_parts: Vec<&str> = current_version.split('.').collect();
            if cv_parts.len() >= 2 {
                if let (Ok(major), Ok(minor)) = (cv_parts[0].parse::<u32>(), cv_parts[1].parse::<u32>()) {
                    return Ok(major > required_major || (major == required_major && minor >= required_minor));
                }
            }
        }
    }

    // Last resort: assume Windows 10+
    Ok(required_major <= 10)
}

/// Check if a Windows service exists.
fn eval_service_exists(service_name: &str) -> Result<bool> {
    let output = std::process::Command::new("sc")
        .args(["query", service_name])
        .output()
        .map_err(|e| CoreError::other("condition", format!("Failed to query service: {}", e)))?;

    // sc query returns 0 if service exists, 1060 if it doesn't
    Ok(output.status.success())
}

/// Check if a Windows service is running.
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

/// Evaluate an env_equals condition.
///
/// Argument format: `VAR_NAME=value`
fn eval_env_equals(argument: &str) -> Result<bool> {
    let eq_pos = argument.find('=')
        .ok_or_else(|| CoreError::other("condition", format!(
            "Invalid env_equals format: '{}'. Expected 'VAR_NAME=value'.", argument
        )))?;

    let var_name = &argument[..eq_pos];
    let expected_value = &argument[eq_pos + 1..];

    match std::env::var(var_name) {
        Ok(actual) => Ok(actual == expected_value),
        Err(_) => Ok(false),
    }
}

/// Parse a registry path into (root, sub_key).
///
/// Input: `HKLM\\Software\\Microsoft`
/// Output: `("HKLM", "Software\\Microsoft")`
fn parse_registry_path(path: &str) -> Result<(&str, &str)> {
    let sep = path.find('\\')
        .ok_or_else(|| CoreError::other("condition", format!(
            "Invalid registry path: '{}'. Expected 'ROOT\\Sub\\Key'.", path
        )))?;

    Ok((&path[..sep], &path[sep + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_condition() {
        assert_eq!(evaluate_condition("always").unwrap(), true);
        assert_eq!(evaluate_condition("").unwrap(), true);
    }

    #[test]
    fn test_never_condition() {
        assert_eq!(evaluate_condition("never").unwrap(), false);
    }

    #[test]
    fn test_file_exists() {
        // This file should exist on Windows
        assert_eq!(evaluate_condition("file_exists:C:\\Windows\\System32\\kernel32.dll").unwrap(), true);
        assert_eq!(evaluate_condition("file_exists:C:\\nonexistent_file_xyz_123.dll").unwrap(), false);
    }

    #[test]
    fn test_file_missing() {
        assert_eq!(evaluate_condition("file_missing:C:\\nonexistent_file_xyz_123.dll").unwrap(), true);
        assert_eq!(evaluate_condition("file_missing:C:\\Windows\\System32\\kernel32.dll").unwrap(), false);
    }

    #[test]
    fn test_dir_exists() {
        assert_eq!(evaluate_condition("dir_exists:C:\\Windows").unwrap(), true);
        assert_eq!(evaluate_condition("dir_exists:C:\\nonexistent_dir_xyz").unwrap(), false);
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
        assert_eq!(evaluate_condition("env:PATH").unwrap(), true);
        assert_eq!(evaluate_condition("env:VELOCITY_NONEXISTENT_VAR_XYZ").unwrap(), false);
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
        let conditions = vec![
            "always".to_string(),
            "file_exists:C:\\Windows".to_string(),
        ];
        assert_eq!(evaluate_all_conditions(&conditions).unwrap(), true);

        let conditions_with_false = vec![
            "always".to_string(),
            "never".to_string(),
        ];
        assert_eq!(evaluate_all_conditions(&conditions_with_false).unwrap(), false);
    }

    #[test]
    fn test_parse_registry_path() {
        let (root, sub) = parse_registry_path(r"HKLM\Software\Microsoft").unwrap();
        assert_eq!(root, "HKLM");
        assert_eq!(sub, r"Software\Microsoft");
    }

    #[test]
    fn test_parse_registry_path_invalid() {
        assert!(parse_registry_path("NOSLASH").is_err());
    }
}
