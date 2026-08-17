//! Windows registry operations.

use crate::error::{CoreError, Result};
use tracing::{debug, info};
use velocity_config::RegistryEntry;
use winreg::enums::*;
use winreg::RegKey;

/// Apply all registry entries from the manifest.
pub fn apply_registry_entries(entries: &[RegistryEntry]) -> Result<()> {
    for entry in entries {
        apply_registry_entry(entry)?;
    }
    Ok(())
}

/// Apply a single registry entry.
pub fn apply_registry_entry(entry: &RegistryEntry) -> Result<()> {
    let root_key = get_root_key(&entry.root)?;
    info!("Writing registry: {}\\{}", entry.root, entry.key);

    let (key, _) = root_key
        .create_subkey(&entry.key)
        .map_err(|e| CoreError::Registry(format!("Failed to create key '{}': {}", entry.key, e)))?;

    let value_name = entry.name.as_deref().unwrap_or("");

    match entry.value_type.as_str() {
        "string" | "REG_SZ" => {
            key.set_value(value_name, &entry.value)
                .map_err(|e| CoreError::Registry(format!("Failed to set string value: {}", e)))?;
        }
        "dword" | "REG_DWORD" => {
            let val: u32 = entry.value.parse().map_err(|_| {
                CoreError::Registry(format!("Invalid DWORD value: '{}'", entry.value))
            })?;
            key.set_value(value_name, &val)
                .map_err(|e| CoreError::Registry(format!("Failed to set DWORD value: {}", e)))?;
        }
        "expand_string" | "REG_EXPAND_SZ" => {
            use winreg::RegValue;
            let wide: Vec<u16> = entry.value.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes: Vec<u8> = wide.iter().flat_map(|w| w.to_le_bytes()).collect();
            let reg_value = RegValue {
                vtype: winreg::enums::RegType::REG_EXPAND_SZ,
                bytes,
            };
            key.set_raw_value(value_name, &reg_value)
                .map_err(|e| CoreError::Registry(format!("Failed to set expand string value: {}", e)))?;
        }
        "multi_string" | "REG_MULTI_SZ" => {
            let values: Vec<&str> = entry.value.split('\n').collect();
            key.set_value(value_name, &values)
                .map_err(|e| CoreError::Registry(format!("Failed to set multi-string value: {}", e)))?;
        }
        _ => {
            // Default to string
            key.set_value(value_name, &entry.value)
                .map_err(|e| CoreError::Registry(format!("Failed to set value: {}", e)))?;
        }
    }

    debug!("Registry entry applied: {}\\{} = {}", entry.root, entry.key, entry.value);
    Ok(())
}

/// Remove registry entries (used during uninstallation).
pub fn remove_registry_entries(entries: &[RegistryEntry]) -> Result<()> {
    for entry in entries {
        if entry.delete_on_uninstall {
            remove_registry_entry(entry)?;
        }
    }
    Ok(())
}

/// Remove a single registry entry.
fn remove_registry_entry(entry: &RegistryEntry) -> Result<()> {
    let root_key = get_root_key(&entry.root)?;

    if let Some(name) = &entry.name {
        // Remove a specific value
        let key = root_key
            .open_subkey(&entry.key)
            .map_err(|e| CoreError::Registry(format!("Failed to open key '{}': {}", entry.key, e)))?;
        key.delete_value(name.as_str())
            .map_err(|e| CoreError::Registry(format!("Failed to delete value '{}': {}", name, e)))?;
        debug!("Deleted registry value: {}\\{}\\{}", entry.root, entry.key, name);
    } else {
        // Remove the entire key
        root_key
            .delete_subkey_all(&entry.key)
            .map_err(|e| CoreError::Registry(format!("Failed to delete key '{}': {}", entry.key, e)))?;
        debug!("Deleted registry key: {}\\{}", entry.root, entry.key);
    }

    Ok(())
}

/// Write the Add/Remove Programs entry for the uninstaller.
pub fn write_uninstall_entry(
    app_name: &str,
    install_dir: &str,
    uninstaller_path: &str,
    version: &str,
    publisher: &str,
    icon_path: Option<&str>,
    display_name: Option<&str>,
    help_url: Option<&str>,
    update_url: Option<&str>,
) -> Result<()> {
    let uninstall_key = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        app_name.replace(' ', "_")
    );

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm
        .create_subkey(&uninstall_key)
        .map_err(|e| CoreError::Registry(format!("Failed to create uninstall key: {}", e)))?;

    let name = display_name.unwrap_or(app_name);

    key.set_value("DisplayName", &name)
        .map_err(|e| CoreError::Registry(format!("Failed to set DisplayName: {}", e)))?;
    key.set_value("DisplayVersion", &version)
        .map_err(|e| CoreError::Registry(format!("Failed to set DisplayVersion: {}", e)))?;
    key.set_value("Publisher", &publisher)
        .map_err(|e| CoreError::Registry(format!("Failed to set Publisher: {}", e)))?;
    key.set_value("InstallLocation", &install_dir)
        .map_err(|e| CoreError::Registry(format!("Failed to set InstallLocation: {}", e)))?;
    key.set_value("UninstallString", &format!("\"{}\"", uninstaller_path))
        .map_err(|e| CoreError::Registry(format!("Failed to set UninstallString: {}", e)))?;
    key.set_value("QuietUninstallString", &format!("\"{}\" /quiet", uninstaller_path))
        .map_err(|e| CoreError::Registry(format!("Failed to set QuietUninstallString: {}", e)))?;

    // NoModify and NoRepair = 1 (hide modify/repair buttons)
    key.set_value("NoModify", &1u32)
        .map_err(|e| CoreError::Registry(format!("Failed to set NoModify: {}", e)))?;
    key.set_value("NoRepair", &1u32)
        .map_err(|e| CoreError::Registry(format!("Failed to set NoRepair: {}", e)))?;

    if let Some(icon) = icon_path {
        key.set_value("DisplayIcon", &icon)
            .map_err(|e| CoreError::Registry(format!("Failed to set DisplayIcon: {}", e)))?;
    }

    // Optional URLs
    if let Some(url) = help_url {
        key.set_value("HelpLink", &url)
            .map_err(|e| CoreError::Registry(format!("Failed to set HelpLink: {}", e)))?;
    }
    if let Some(url) = update_url {
        key.set_value("URLUpdateInfo", &url)
            .map_err(|e| CoreError::Registry(format!("Failed to set URLUpdateInfo: {}", e)))?;
    }

    // Estimated size (in KB) — default 10MB if unknown
    key.set_value("EstimatedSize", &10240u32)
        .map_err(|e| CoreError::Registry(format!("Failed to set EstimatedSize: {}", e)))?;

    // InstallDate as YYYYMMDD string
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days = now.as_secs() / 86400;
    let (year, month, day) = days_to_ymd(days as i64);
    let date_str = format!("{:04}{:02}{:02}", year, month, day);
    key.set_value("InstallDate", &date_str)
        .map_err(|e| CoreError::Registry(format!("Failed to set InstallDate: {}", e)))?;

    info!("Uninstall entry registered for: {}", name);
    Ok(())
}

/// Remove the Add/Remove Programs entry.
pub fn remove_uninstall_entry(app_name: &str) -> Result<()> {
    let uninstall_key = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        app_name.replace(' ', "_")
    );

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm
        .delete_subkey_all(&uninstall_key)
        .map_err(|e| CoreError::Registry(format!("Failed to remove uninstall entry: {}", e)))?;

    info!("Uninstall entry removed for: {}", app_name);
    Ok(())
}

/// Convert a root string to a winreg RegKey.
fn get_root_key(root: &str) -> Result<RegKey> {
    match root {
        "HKLM" => Ok(RegKey::predef(HKEY_LOCAL_MACHINE)),
        "HKCU" => Ok(RegKey::predef(HKEY_CURRENT_USER)),
        "HKCR" => Ok(RegKey::predef(HKEY_CLASSES_ROOT)),
        "HKU" => Ok(RegKey::predef(HKEY_USERS)),
        _ => Err(CoreError::Registry(format!("Unknown registry root: {}", root))),
    }
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days_since_epoch: i64) -> (i64, u32, u32) {
    let days = days_since_epoch + 719468;
    let era = floor_div(days, 146097);
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Floor division (rounds towards negative infinity).
fn floor_div(a: i64, b: i64) -> i64 {
    let d = a / b;
    let r = a % b;
    if (r > 0 && b < 0) || (r < 0 && b > 0) { d - 1 } else { d }
}
