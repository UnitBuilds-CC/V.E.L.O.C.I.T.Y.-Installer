//! Environment variable management.

use crate::error::{CoreError, Result};
use tracing::{debug, info};
use velocity_config::EnvVarEntry;
use winreg::enums::*;
use winreg::RegKey;

/// Apply environment variable entries.
pub fn apply_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
    for entry in entries {
        apply_env_var(entry)?;
    }
    Ok(())
}

/// Apply a single environment variable.
fn apply_env_var(entry: &EnvVarEntry) -> Result<()> {
    // Validate env var name
    if entry.name.is_empty() {
        return Err(CoreError::other("set env var", "Environment variable name cannot be empty"));
    }
    if entry.name.contains('=') {
        return Err(CoreError::other("set env var", format!(
            "Environment variable name '{}' contains invalid character '='", entry.name
        )));
    }

    let root = match entry.scope.as_str() {
        "system" => {
            info!("Setting system env var: {} = {}", entry.name, entry.value);
            RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(
                    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                    winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                )
                .map_err(|e| CoreError::Registry(format!("Failed to open system env key: {}", e)))?
        }
        "user" | _ => {
            info!("Setting user env var: {} = {}", entry.name, entry.value);
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(
                    "Environment",
                    winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                )
                .map_err(|e| CoreError::Registry(format!("Failed to open user env key: {}", e)))?
        }
    };

    if entry.append {
        // Append to existing value
        let existing: String = root.get_value(&entry.name).unwrap_or_default();
        let new_value = if existing.is_empty() {
            entry.value.clone()
        } else {
            format!("{};{}", existing, entry.value)
        };
        root.set_value(&entry.name, &new_value)
            .map_err(|e| CoreError::Registry(format!("Failed to set env var: {}", e)))?;
    } else {
        root.set_value(&entry.name, &entry.value)
            .map_err(|e| CoreError::Registry(format!("Failed to set env var: {}", e)))?;
    }

    // Broadcast WM_SETTINGCHANGE to notify other applications
    broadcast_env_change();

    debug!("Environment variable set: {}", entry.name);
    Ok(())
}

/// Remove environment variables during uninstallation.
pub fn remove_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
    for entry in entries {
        if entry.delete_on_uninstall {
            remove_env_var(entry)?;
        }
    }
    Ok(())
}

/// Remove a single environment variable.
fn remove_env_var(entry: &EnvVarEntry) -> Result<()> {
    let root = match entry.scope.as_str() {
        "system" => RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
            )
            .map_err(|e| CoreError::Registry(format!("Failed to open system env key: {}", e)))?,
        "user" | _ => RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(
                "Environment",
                winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
            )
            .map_err(|e| CoreError::Registry(format!("Failed to open user env key: {}", e)))?,
    };

    if entry.append {
        // Remove only our portion from the value
        if let Ok(existing) = root.get_value::<String, _>(&entry.name) {
            let new_value = existing
                .split(';')
                .filter(|part| *part != entry.value)
                .collect::<Vec<_>>()
                .join(";");

            if new_value.is_empty() {
                root.delete_value(&entry.name).ok();
            } else {
                root.set_value(&entry.name, &new_value).ok();
            }
        }
    } else {
        root.delete_value(&entry.name).ok();
    }

    broadcast_env_change();
    debug!("Environment variable removed: {}", entry.name);
    Ok(())
}

/// Broadcast WM_SETTINGCHANGE to notify applications of environment changes.
fn broadcast_env_change() {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let env_str: Vec<u16> = "Environment\0".encode_utf16().collect();
        let result = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(env_str.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
        if result.0 == 0 {
            debug!("WM_SETTINGCHANGE broadcast timed out (non-critical)");
        }
    }
}
