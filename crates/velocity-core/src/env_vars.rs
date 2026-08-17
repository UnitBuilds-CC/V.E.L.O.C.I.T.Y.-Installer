//! Cross-platform environment variable management.
//!
//! - Windows: Registry persistent env vars + WM_SETTINGCHANGE broadcast
//! - Linux: /etc/environment (system) or ~/.profile (user)
//! - macOS: /etc/launchd.conf (system) or ~/.zprofile (user)

use velocity_config::EnvVarEntry;

// ===========================================================================
// Windows implementation (Registry)
// ===========================================================================
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use tracing::{debug, info, warn};
    use winreg::enums::*;
    use winreg::RegKey;

    pub fn apply_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
        for entry in entries {
            apply_env_var(entry)?;
        }
        Ok(())
    }

    fn apply_env_var(entry: &EnvVarEntry) -> Result<()> {
        if entry.name.is_empty() {
            return Err(CoreError::other(
                "set env var",
                "Environment variable name cannot be empty",
            ));
        }
        if entry.name.contains('=') {
            return Err(CoreError::other(
                "set env var",
                format!(
                    "Environment variable name '{}' contains invalid character '='",
                    entry.name
                ),
            ));
        }

        let root = match entry.scope.as_str() {
            "system" => {
                info!("Setting system env var: {} = {}", entry.name, entry.value);
                RegKey::predef(HKEY_LOCAL_MACHINE)
                    .open_subkey_with_flags(
                        "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                        winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                    )
                    .map_err(|e| {
                        CoreError::Registry(format!("Failed to open system env key: {}", e))
                    })?
            }
            _ => {
                info!("Setting user env var: {} = {}", entry.name, entry.value);
                RegKey::predef(HKEY_CURRENT_USER)
                    .open_subkey_with_flags(
                        "Environment",
                        winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                    )
                    .map_err(|e| {
                        CoreError::Registry(format!("Failed to open user env key: {}", e))
                    })?
            }
        };

        if entry.append {
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

        broadcast_env_change();
        debug!("Environment variable set: {}", entry.name);
        Ok(())
    }

    pub fn remove_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
        for entry in entries {
            if entry.delete_on_uninstall {
                remove_env_var(entry)?;
            }
        }
        Ok(())
    }

    fn remove_env_var(entry: &EnvVarEntry) -> Result<()> {
        let root = match entry.scope.as_str() {
            "system" => RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(
                    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                    winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                )
                .map_err(|e| {
                    CoreError::Registry(format!("Failed to open system env key: {}", e))
                })?,
            _ => RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(
                    "Environment",
                    winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                )
                .map_err(|e| CoreError::Registry(format!("Failed to open user env key: {}", e)))?,
        };

        if entry.append {
            if let Ok(existing) = root.get_value::<String, _>(&entry.name) {
                let new_value = existing
                    .split(';')
                    .filter(|part| *part != entry.value)
                    .collect::<Vec<_>>()
                    .join(";");
                if new_value.is_empty() {
                    if let Err(e) = root.delete_value(&entry.name) {
                        warn!("Failed to delete env var {}: {}", entry.name, e);
                    }
                } else if let Err(e) = root.set_value(&entry.name, &new_value) {
                    warn!("Failed to update env var {}: {}", entry.name, e);
                }
            }
        } else if let Err(e) = root.delete_value(&entry.name) {
            warn!("Failed to delete env var {}: {}", entry.name, e);
        }

        broadcast_env_change();
        debug!("Environment variable removed: {}", entry.name);
        Ok(())
    }

    fn broadcast_env_change() {
        use windows::Win32::Foundation::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        // SAFETY: SendMessageTimeoutW with HWND_BROADCAST and static null-terminated string.
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
}

// ===========================================================================
// Linux implementation (/etc/environment + ~/.profile)
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use tracing::{debug, info};

    pub fn apply_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
        for entry in entries {
            apply_env_var(entry)?;
        }
        Ok(())
    }

    fn apply_env_var(entry: &EnvVarEntry) -> Result<()> {
        if entry.name.is_empty() {
            return Err(CoreError::other(
                "set env var",
                "Environment variable name cannot be empty",
            ));
        }
        if entry.name.contains('=') {
            return Err(CoreError::other(
                "set env var",
                format!("Invalid character '=' in env var name '{}'", entry.name),
            ));
        }

        match entry.scope.as_str() {
            "system" => {
                info!("Setting system env var: {} = {}", entry.name, entry.value);
                apply_to_file(
                    "/etc/environment",
                    &entry.name,
                    &entry.value,
                    entry.append,
                    "=",
                )?;
            }
            _ => {
                info!("Setting user env var: {} = {}", entry.name, entry.value);
                let profile = home_profile_path();
                apply_to_file(&profile, &entry.name, &entry.value, entry.append, "=")?;
            }
        }

        debug!("Environment variable set: {}", entry.name);
        Ok(())
    }

    fn apply_to_file(
        path: &str,
        name: &str,
        value: &str,
        append: bool,
        separator: &str,
    ) -> Result<()> {
        let content = if std::path::Path::new(path).exists() {
            std::fs::read_to_string(path).unwrap_or_default()
        } else {
            String::new()
        };

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let prefix = format!("{}=", name);

        // Remove existing line for this variable
        lines.retain(|line| !line.starts_with(&prefix));

        // Build the new value
        let new_line = if append {
            // Check if there was a previous value to append to
            let existing = content
                .lines()
                .find(|l| l.starts_with(&prefix))
                .and_then(|l| l.split_once('='))
                .map(|(_, v)| v.to_string());

            match existing {
                Some(old) if !old.is_empty() => format!(
                    "{}={}{}{}",
                    prefix.trim_end_matches('='),
                    separator,
                    old,
                    value
                ),
                _ => format!("{}={}", name, value),
            }
        } else {
            format!("{}={}", name, value)
        };

        lines.push(new_line);
        lines.push(String::new()); // trailing newline

        std::fs::write(path, lines.join("\n")).map_err(|e| {
            CoreError::other("write env file", format!("Failed to write {}: {}", path, e))
        })?;
        Ok(())
    }

    fn remove_from_file(path: &str, name: &str, value: Option<&str>, append: bool) -> Result<()> {
        if !std::path::Path::new(path).exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let prefix = format!("{}=", name);

        let lines: Vec<String> = content
            .lines()
            .filter(|line| {
                if !line.starts_with(&prefix) {
                    return true;
                }
                if append {
                    if let Some(val) = value {
                        if let Some((_, existing)) = line.split_once('=') {
                            let new_val = existing
                                .split(':')
                                .filter(|p| *p != val)
                                .collect::<Vec<_>>()
                                .join(":");
                            return !new_val.is_empty(); // keep line if there's still content
                        }
                    }
                }
                false // remove the line entirely
            })
            .map(|line| {
                if append && line.starts_with(&prefix) {
                    if let Some(val) = value {
                        if let Some((key, existing)) = line.split_once('=') {
                            let new_val = existing
                                .split(':')
                                .filter(|p| *p != val)
                                .collect::<Vec<_>>()
                                .join(":");
                            return format!("{}={}", key, new_val);
                        }
                    }
                }
                line.to_string()
            })
            .collect();

        let mut result = lines.join("\n");
        if !result.ends_with('\n') {
            result.push('\n');
        }
        std::fs::write(path, result).map_err(|e| {
            CoreError::other("write env file", format!("Failed to write {}: {}", path, e))
        })?;
        Ok(())
    }

    pub fn remove_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
        for entry in entries {
            if entry.delete_on_uninstall {
                remove_env_var(entry)?;
            }
        }
        Ok(())
    }

    fn remove_env_var(entry: &EnvVarEntry) -> Result<()> {
        match entry.scope.as_str() {
            "system" => {
                info!("Removing system env var: {}", entry.name);
                remove_from_file(
                    "/etc/environment",
                    &entry.name,
                    Some(&entry.value),
                    entry.append,
                )?;
            }
            _ => {
                info!("Removing user env var: {}", entry.name);
                let profile = home_profile_path();
                remove_from_file(&profile, &entry.name, Some(&entry.value), entry.append)?;
            }
        }
        debug!("Environment variable removed: {}", entry.name);
        Ok(())
    }

    fn home_profile_path() -> String {
        std::env::var("HOME")
            .map(|h| format!("{}/.profile", h))
            .unwrap_or_else(|_| "/etc/profile".to_string())
    }
}

// ===========================================================================
// macOS implementation (~/.zprofile + launchctl)
// ===========================================================================
#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use tracing::{debug, info, warn};

    pub fn apply_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
        for entry in entries {
            apply_env_var(entry)?;
        }
        Ok(())
    }

    fn apply_env_var(entry: &EnvVarEntry) -> Result<()> {
        if entry.name.is_empty() {
            return Err(CoreError::other(
                "set env var",
                "Environment variable name cannot be empty",
            ));
        }
        if entry.name.contains('=') {
            return Err(CoreError::other(
                "set env var",
                format!("Invalid character '=' in env var name '{}'", entry.name),
            ));
        }

        match entry.scope.as_str() {
            "system" => {
                info!("Setting system env var: {} = {}", entry.name, entry.value);
                // Use launchctl config for system-wide env on macOS
                let conf = "/etc/launchd.conf";
                let line = format!("setenv {} {}", entry.name, entry.value);
                let mut content = std::fs::read_to_string(conf).unwrap_or_default();
                // Remove existing line for this var
                let prefix = format!("setenv {} ", entry.name);
                content = content
                    .lines()
                    .filter(|l| !l.starts_with(&prefix))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&line);
                content.push('\n');
                std::fs::write(conf, content).map_err(|e| {
                    CoreError::other(
                        "write launchd.conf",
                        format!("Failed to write {}: {}", conf, e),
                    )
                })?;
                // Apply immediately
                match std::process::Command::new("launchctl")
                    .args(["setenv", &entry.name, &entry.value])
                    .output()
                {
                    Ok(output) if !output.status.success() => {
                        warn!(
                            "launchctl setenv failed: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        );
                    }
                    Err(e) => {
                        warn!("launchctl not available: {}", e);
                    }
                    _ => {}
                }
            }
            _ => {
                info!("Setting user env var: {} = {}", entry.name, entry.value);
                let profile = home_zprofile_path();
                let export_line = format!("export {}=\"{}\"", entry.name, entry.value);
                let mut content = std::fs::read_to_string(&profile).unwrap_or_default();
                let prefix = format!("export {}=", entry.name);
                content = content
                    .lines()
                    .filter(|l| !l.starts_with(&prefix))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&export_line);
                content.push('\n');
                std::fs::write(&profile, content).map_err(|e| {
                    CoreError::other(
                        "write zprofile",
                        format!("Failed to write {}: {}", profile, e),
                    )
                })?;
            }
        }

        debug!("Environment variable set: {}", entry.name);
        Ok(())
    }

    pub fn remove_env_vars(entries: &[EnvVarEntry]) -> Result<()> {
        for entry in entries {
            if entry.delete_on_uninstall {
                remove_env_var(entry)?;
            }
        }
        Ok(())
    }

    fn remove_env_var(entry: &EnvVarEntry) -> Result<()> {
        match entry.scope.as_str() {
            "system" => {
                info!("Removing system env var: {}", entry.name);
                let conf = "/etc/launchd.conf";
                if std::path::Path::new(conf).exists() {
                    let content = std::fs::read_to_string(conf).unwrap_or_default();
                    let prefix = format!("setenv {} ", entry.name);
                    let new_content = content
                        .lines()
                        .filter(|l| !l.starts_with(&prefix))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Err(e) = std::fs::write(conf, new_content) {
                        warn!("Failed to write {}: {}", conf, e);
                    }
                }
                match std::process::Command::new("launchctl")
                    .args(["unsetenv", &entry.name])
                    .output()
                {
                    Ok(output) if !output.status.success() => {
                        warn!(
                            "launchctl unsetenv failed: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        );
                    }
                    Err(e) => {
                        warn!("launchctl not available: {}", e);
                    }
                    _ => {}
                }
            }
            _ => {
                info!("Removing user env var: {}", entry.name);
                let profile = home_zprofile_path();
                if std::path::Path::new(&profile).exists() {
                    let content = std::fs::read_to_string(&profile).unwrap_or_default();
                    let prefix = format!("export {}=", entry.name);
                    let new_content = content
                        .lines()
                        .filter(|l| !l.starts_with(&prefix))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Err(e) = std::fs::write(&profile, new_content) {
                        warn!("Failed to write {}: {}", profile, e);
                    }
                }
            }
        }
        debug!("Environment variable removed: {}", entry.name);
        Ok(())
    }

    fn home_zprofile_path() -> String {
        std::env::var("HOME")
            .map(|h| format!("{}/.zprofile", h))
            .unwrap_or_else(|_| "/etc/zprofile".to_string())
    }
}

// ===========================================================================
// Cross-platform public API
// ===========================================================================

/// Apply environment variable entries.
pub fn apply_env_vars(entries: &[EnvVarEntry]) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::apply_env_vars(entries)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::apply_env_vars(entries)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::apply_env_vars(entries)
    }
}

/// Remove environment variables during uninstallation.
pub fn remove_env_vars(entries: &[EnvVarEntry]) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::remove_env_vars(entries)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::remove_env_vars(entries)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::remove_env_vars(entries)
    }
}

/// Validate an environment variable name.
///
/// Rejects empty names, names containing `=` or null bytes,
/// and names starting with a digit.
pub fn validate_env_var_name(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("Environment variable name cannot be empty".into());
    }
    if name.contains('=') {
        return Err(format!(
            "Environment variable name '{}' contains invalid character '='",
            name
        ));
    }
    if name.contains('\0') {
        return Err(format!(
            "Environment variable name '{}' contains invalid null byte",
            name
        ));
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(format!(
            "Environment variable name '{}' cannot start with a digit",
            name
        ));
    }
    Ok(())
}

/// Format an environment variable as a `KEY=VALUE` line.
///
/// Used for writing to `/etc/environment`, `~/.profile`, and `launchd.conf`.
pub fn format_env_line(name: &str, value: &str) -> String {
    format!("{}={}", name, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_env_var_name_valid() {
        assert!(validate_env_var_name("PATH").is_ok());
        assert!(validate_env_var_name("MY_VAR").is_ok());
        assert!(validate_env_var_name("my_var_123").is_ok());
        assert!(validate_env_var_name("_PRIVATE").is_ok());
    }

    #[test]
    fn test_validate_env_var_name_empty() {
        assert!(validate_env_var_name("").is_err());
    }

    #[test]
    fn test_validate_env_var_name_equals() {
        assert!(validate_env_var_name("FOO=BAR").is_err());
    }

    #[test]
    fn test_validate_env_var_name_null_byte() {
        assert!(validate_env_var_name("FOO\0BAR").is_err());
    }

    #[test]
    fn test_validate_env_var_name_starts_with_digit() {
        assert!(validate_env_var_name("1INVALID").is_err());
        assert!(validate_env_var_name("9VAR").is_err());
    }

    #[test]
    fn test_format_env_line() {
        assert_eq!(format_env_line("PATH", "/usr/bin"), "PATH=/usr/bin");
        assert_eq!(
            format_env_line("MY_VAR", "hello world"),
            "MY_VAR=hello world"
        );
    }
}
