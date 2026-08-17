//! Uninstaller generation — creates a self-contained uninstall executable.

use crate::error::{CoreError, Result};
use crate::registry;
use crate::shortcuts;
use std::path::Path;
use tracing::info;
use velocity_config::VelocityManifest;

/// Generate an uninstaller executable.
///
/// The uninstaller is a copy of the runtime with a special flag
/// embedded that tells it to run in uninstall mode.
pub fn generate_uninstaller(
    runtime_exe: &[u8],
    manifest: &VelocityManifest,
    install_dir: &Path,
    output_path: &Path,
) -> Result<()> {
    info!("Generating uninstaller at: {}", output_path.display());

    // Create uninstall metadata
    let uninstall_info = UninstallInfo {
        app_name: manifest.app.name.clone(),
        install_dir: install_dir.to_string_lossy().to_string(),
        files_to_remove: Vec::new(), // Will be populated at runtime
        registry_entries: manifest.registry.clone(),
        shortcut_config: manifest.shortcuts.clone(),
        start_menu_folder: manifest.install.start_menu.clone(),
        env_vars: manifest.env_vars.clone(),
        services: manifest.services.clone(),
        pre_uninstall: manifest.scripts.pre_uninstall.clone(),
        post_uninstall: manifest.scripts.post_uninstall.clone(),
    };

    let uninstall_json = serde_json::to_vec(&uninstall_info)
        .map_err(|e| CoreError::Other(format!("Failed to serialize uninstall info: {}", e)))?;

    // The uninstaller is the runtime exe + uninstall marker + uninstall data
    let mut output = Vec::new();
    output.extend_from_slice(runtime_exe);

    // Uninstall marker
    let marker = b"VELOCITY_UNINST_V1";
    output.extend_from_slice(marker);

    // Uninstall data length + data
    let data_len = uninstall_json.len() as u64;
    output.extend_from_slice(&data_len.to_le_bytes());
    output.extend_from_slice(&uninstall_json);

    std::fs::write(output_path, &output)?;

    info!("Uninstaller generated: {} bytes", output.len());
    Ok(())
}

/// Perform the uninstallation process.
pub fn perform_uninstall(info: &UninstallInfo) -> Result<()> {
    info!("Uninstalling: {} from {}", info.app_name, info.install_dir);

    // Run pre-uninstall scripts
    for cmd in &info.pre_uninstall {
        let _ = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .output();
    }

    // Stop and remove services
    if !info.services.is_empty() {
        let _ = crate::services::remove_services(&info.services);
    }

    // Remove environment variables
    if !info.env_vars.is_empty() {
        let _ = crate::env_vars::remove_env_vars(&info.env_vars);
    }

    // Remove registry entries
    if !info.registry_entries.is_empty() {
        let _ = registry::remove_registry_entries(&info.registry_entries);
    }

    // Remove shortcuts
    let _ = shortcuts::remove_shortcuts(
        &info.shortcut_config,
        &info.app_name,
        info.start_menu_folder.as_deref(),
    );

    // Remove the uninstall registry entry
    let _ = registry::remove_uninstall_entry(&info.app_name);

    // Remove installed files
    let install_dir = Path::new(&info.install_dir);
    if install_dir.exists() {
        // Walk and remove files first, then directories
        let mut dirs_to_remove = Vec::new();

        for entry in walkdir::WalkDir::new(install_dir).contents_first(true) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_file() {
                // Don't delete the uninstaller itself until the end
                if entry.path().file_name().map(|n| n == "uninstall.exe").unwrap_or(false) {
                    continue;
                }
                std::fs::remove_file(entry.path()).ok();
            } else if entry.file_type().is_dir() {
                dirs_to_remove.push(entry.path().to_path_buf());
            }
        }

        // Remove directories (deepest first)
        dirs_to_remove.sort_by(|a, b| b.cmp(a));
        for dir in dirs_to_remove {
            std::fs::remove_dir(&dir).ok();
        }

        // Try to remove the install directory itself
        std::fs::remove_dir(install_dir).ok();
    }

    // Run post-uninstall scripts
    for cmd in &info.post_uninstall {
        let _ = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .output();
    }

    info!("Uninstallation complete for: {}", info.app_name);
    Ok(())
}

/// Uninstall metadata embedded in the uninstaller executable.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UninstallInfo {
    pub app_name: String,
    pub install_dir: String,
    pub files_to_remove: Vec<String>,
    pub registry_entries: Vec<velocity_config::RegistryEntry>,
    pub shortcut_config: velocity_config::ShortcutConfig,
    pub start_menu_folder: Option<String>,
    pub env_vars: Vec<velocity_config::EnvVarEntry>,
    pub services: Vec<velocity_config::ServiceEntry>,
    pub pre_uninstall: Vec<String>,
    pub post_uninstall: Vec<String>,
}

/// Check if the current executable is running in uninstall mode.
pub fn is_uninstall_mode() -> bool {
    std::env::args().any(|arg| arg == "/uninstall" || arg == "--uninstall")
}

/// Read uninstall info from the current executable.
pub fn read_uninstall_info(exe_path: &Path) -> Result<UninstallInfo> {
    let data = std::fs::read(exe_path)?;

    // Find the uninstall marker
    let marker = b"VELOCITY_UNINST_V1";
    let marker_pos = data
        .windows(marker.len())
        .position(|w| w == marker)
        .ok_or_else(|| CoreError::InvalidPayload("Not an uninstaller".to_string()))?;

    let data_start = marker_pos + marker.len();
    if data_start + 8 > data.len() {
        return Err(CoreError::InvalidPayload("Truncated uninstall data".to_string()));
    }

    let len_bytes: [u8; 8] = data[data_start..data_start + 8]
        .try_into()
        .map_err(|_| CoreError::InvalidPayload("Invalid length".to_string()))?;
    let data_len = u64::from_le_bytes(len_bytes) as usize;

    let json_start = data_start + 8;
    let json_end = json_start + data_len;
    if json_end > data.len() {
        return Err(CoreError::InvalidPayload("Truncated uninstall JSON".to_string()));
    }

    let info: UninstallInfo = serde_json::from_slice(&data[json_start..json_end])
        .map_err(|e| CoreError::InvalidPayload(format!("Invalid uninstall JSON: {}", e)))?;

    Ok(info)
}
