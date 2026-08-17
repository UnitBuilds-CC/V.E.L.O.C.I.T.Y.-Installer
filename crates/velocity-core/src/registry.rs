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

    let (key, _) = root_key.create_subkey(&entry.key).map_err(|e| {
        CoreError::registry(
            "create registry key",
            format!("Failed to create key '{}': {}", entry.key, e),
        )
    })?;

    let value_name = entry.name.as_deref().unwrap_or("");

    match entry.value_type.as_str() {
        "string" | "REG_SZ" => {
            key.set_value(value_name, &entry.value)
                .map_err(|e| CoreError::registry("set string value", format!("{}", e)))?;
        }
        "dword" | "REG_DWORD" => {
            let val: u32 = entry.value.parse().map_err(|_| {
                CoreError::registry(
                    "parse DWORD",
                    format!("Invalid DWORD value: '{}'", entry.value),
                )
            })?;
            key.set_value(value_name, &val)
                .map_err(|e| CoreError::registry("set DWORD value", format!("{}", e)))?;
        }
        "expand_string" | "REG_EXPAND_SZ" => {
            use winreg::RegValue;
            let wide: Vec<u16> = entry
                .value
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let bytes: Vec<u8> = wide.iter().flat_map(|w| w.to_le_bytes()).collect();
            let reg_value = RegValue {
                vtype: winreg::enums::RegType::REG_EXPAND_SZ,
                bytes,
            };
            key.set_raw_value(value_name, &reg_value)
                .map_err(|e| CoreError::registry("set expand string", format!("{}", e)))?;
        }
        "multi_string" | "REG_MULTI_SZ" => {
            let values: Vec<&str> = entry.value.split('\n').collect();
            key.set_value(value_name, &values)
                .map_err(|e| CoreError::registry("set multi-string", format!("{}", e)))?;
        }
        _ => {
            // Default to string
            key.set_value(value_name, &entry.value)
                .map_err(|e| CoreError::registry("set value", format!("{}", e)))?;
        }
    }

    debug!(
        "Registry entry applied: {}\\{} = {}",
        entry.root, entry.key, entry.value
    );
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
        let key = root_key.open_subkey(&entry.key).map_err(|e| {
            CoreError::registry(
                "open registry key",
                format!("Failed to open key '{}': {}", entry.key, e),
            )
        })?;
        key.delete_value(name.as_str()).map_err(|e| {
            CoreError::registry(
                "delete value",
                format!("Failed to delete value '{}': {}", name, e),
            )
        })?;
        debug!(
            "Deleted registry value: {}\\{}\\{}",
            entry.root, entry.key, name
        );
    } else {
        // Remove the entire key
        root_key.delete_subkey_all(&entry.key).map_err(|e| {
            CoreError::registry(
                "delete key",
                format!("Failed to delete key '{}': {}", entry.key, e),
            )
        })?;
        debug!("Deleted registry key: {}\\{}", entry.root, entry.key);
    }

    Ok(())
}

/// Write the Add/Remove Programs entry for the uninstaller.
#[allow(clippy::too_many_arguments)]
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
        .map_err(|e| CoreError::registry("create uninstall key", format!("{}", e)))?;

    let name = display_name.unwrap_or(app_name);

    key.set_value("DisplayName", &name)
        .map_err(|e| CoreError::registry("set DisplayName", format!("{}", e)))?;
    key.set_value("DisplayVersion", &version)
        .map_err(|e| CoreError::registry("set DisplayVersion", format!("{}", e)))?;
    key.set_value("Publisher", &publisher)
        .map_err(|e| CoreError::registry("set Publisher", format!("{}", e)))?;
    key.set_value("InstallLocation", &install_dir)
        .map_err(|e| CoreError::registry("set InstallLocation", format!("{}", e)))?;
    key.set_value("UninstallString", &format!("\"{}\"", uninstaller_path))
        .map_err(|e| CoreError::registry("set UninstallString", format!("{}", e)))?;
    key.set_value(
        "QuietUninstallString",
        &format!("\"{}\" /quiet", uninstaller_path),
    )
    .map_err(|e| CoreError::registry("set QuietUninstallString", format!("{}", e)))?;

    // NoModify and NoRepair = 1 (hide modify/repair buttons)
    key.set_value("NoModify", &1u32)
        .map_err(|e| CoreError::registry("set NoModify", format!("{}", e)))?;
    key.set_value("NoRepair", &1u32)
        .map_err(|e| CoreError::registry("set NoRepair", format!("{}", e)))?;

    if let Some(icon) = icon_path {
        key.set_value("DisplayIcon", &icon)
            .map_err(|e| CoreError::registry("set DisplayIcon", format!("{}", e)))?;
    }

    // Optional URLs
    if let Some(url) = help_url {
        key.set_value("HelpLink", &url)
            .map_err(|e| CoreError::registry("set HelpLink", format!("{}", e)))?;
    }
    if let Some(url) = update_url {
        key.set_value("URLUpdateInfo", &url)
            .map_err(|e| CoreError::registry("set URLUpdateInfo", format!("{}", e)))?;
    }

    // Estimated size (in KB) — default 10MB if unknown
    key.set_value("EstimatedSize", &10240u32)
        .map_err(|e| CoreError::registry("set EstimatedSize", format!("{}", e)))?;

    // InstallDate as YYYYMMDD string
    let now = chrono::Local::now();
    let date_str = now.format("%Y%m%d").to_string();
    key.set_value("InstallDate", &date_str)
        .map_err(|e| CoreError::registry("set InstallDate", format!("{}", e)))?;

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
    hklm.delete_subkey_all(&uninstall_key)
        .map_err(|e| CoreError::registry("remove uninstall entry", format!("{}", e)))?;

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
        _ => Err(CoreError::registry(
            "unknown root",
            format!("Unknown registry root: {}", root),
        )),
    }
}
