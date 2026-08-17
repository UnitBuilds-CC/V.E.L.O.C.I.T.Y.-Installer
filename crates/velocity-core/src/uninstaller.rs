//! Uninstaller generation — creates a self-contained uninstall executable.

use crate::error::{CoreError, Result};
use crate::registry;
use crate::shortcuts;
use crate::file_assoc;
use crate::logging;
use std::path::Path;
use tracing::info;
use velocity_config::VelocityManifest;

/// Generate an uninstaller executable.
///
/// The uninstaller is a copy of the runtime with uninstall metadata
/// embedded that tells it what to remove.
pub fn generate_uninstaller(
    runtime_exe: &[u8],
    manifest: &VelocityManifest,
    install_dir: &Path,
    output_path: &Path,
) -> Result<()> {
    // Build uninstall info from manifest (files_to_remove should be populated
    // separately via populate_files_to_remove before calling this)
    let uninstall_info = UninstallInfo {
        app_name: manifest.app.name.clone(),
        install_dir: install_dir.to_string_lossy().to_string(),
        files_to_remove: Vec::new(),
        registry_entries: manifest.registry.clone(),
        shortcut_config: manifest.shortcuts.clone(),
        start_menu_folder: manifest.install.start_menu.clone(),
        env_vars: manifest.env_vars.clone(),
        services: manifest.services.clone(),
        file_associations: manifest.file_associations.clone(),
        pre_uninstall: manifest.scripts.pre_uninstall.clone(),
        post_uninstall: manifest.scripts.post_uninstall.clone(),
    };
    generate_uninstaller_with_info(runtime_exe, &uninstall_info, output_path)
}

/// Generate an uninstaller with pre-built uninstall info.
pub fn generate_uninstaller_with_info(
    runtime_exe: &[u8],
    uninstall_info: &UninstallInfo,
    output_path: &Path,
) -> Result<()> {
    info!("Generating uninstaller at: {}", output_path.display());

    let uninstall_json = serde_json::to_vec(uninstall_info)
        .map_err(|e| CoreError::other("serialize uninstall", format!("{}", e)))?;

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
    logging::log_op("UNINSTALL", &format!("Removing: {}", info.app_name));

    // Run pre-uninstall scripts
    for cmd in &info.pre_uninstall {
        logging::log_op("SCRIPT", cmd);
        let _ = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .output();
    }

    // Stop and remove services
    if !info.services.is_empty() {
        logging::log_op("UNINSTALL", "Removing services...");
        let _ = crate::services::remove_services(&info.services);
    }

    // Remove environment variables
    if !info.env_vars.is_empty() {
        logging::log_op("UNINSTALL", "Removing environment variables...");
        let _ = crate::env_vars::remove_env_vars(&info.env_vars);
    }

    // Remove file associations
    if !info.file_associations.is_empty() {
        logging::log_op("UNINSTALL", "Removing file associations...");
        let _ = file_assoc::remove_file_associations(&info.file_associations);
    }

    // Remove registry entries
    if !info.registry_entries.is_empty() {
        logging::log_op("UNINSTALL", "Removing registry entries...");
        let _ = registry::remove_registry_entries(&info.registry_entries);
    }

    // Remove shortcuts
    logging::log_op("UNINSTALL", "Removing shortcuts...");
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
        logging::log_op("UNINSTALL", "Removing installed files...");
        let mut dirs_to_remove = Vec::new();
        let mut file_count = 0u32;

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
                // Don't delete the log file until the end
                if entry.path().extension().map(|e| e == "log").unwrap_or(false) {
                    continue;
                }
                if std::fs::remove_file(entry.path()).is_ok() {
                    file_count += 1;
                }
            } else if entry.file_type().is_dir() {
                dirs_to_remove.push(entry.path().to_path_buf());
            }
        }

        // Remove directories (deepest first)
        dirs_to_remove.sort_by(|a, b| b.cmp(a));
        for dir in &dirs_to_remove {
            std::fs::remove_dir(dir).ok();
        }

        // Try to remove the install directory itself
        std::fs::remove_dir(install_dir).ok();
        
        logging::log_success(&format!("Removed {} files", file_count));
    }

    // Run post-uninstall scripts
    for cmd in &info.post_uninstall {
        logging::log_op("SCRIPT", cmd);
        let _ = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .output();
    }

    logging::log_success(&format!("Uninstallation complete: {}", info.app_name));
    info!("Uninstallation complete for: {}", info.app_name);
    Ok(())
}

/// Populate the files_to_remove list by scanning the install directory.
///
/// This is called at runtime before generating the uninstaller, so the
/// uninstaller knows exactly which files were extracted.
pub fn populate_files_to_remove(info: &mut UninstallInfo, extracted_files: &[std::path::PathBuf]) {
    info.files_to_remove = extracted_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
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
    pub file_associations: Vec<velocity_config::FileAssociationEntry>,
    pub pre_uninstall: Vec<String>,
    pub post_uninstall: Vec<String>,
}

/// Check if the current executable is running in uninstall mode.
///
/// Recognized flags:
///   `/uninstall`, `--uninstall`, `-u`  — explicit uninstall
///   `/uninstallexe`                    — launched from Add/Remove Programs
pub fn is_uninstall_mode() -> bool {
    std::env::args().any(|arg| {
        let lower = arg.to_lowercase();
        lower == "/uninstall"
            || lower == "--uninstall"
            || lower == "-u"
            || lower == "/uninstallexe"
            || lower.starts_with("/uninstall=")
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_populate_files_to_remove() {
        let mut info = UninstallInfo {
            app_name: "Test".to_string(),
            install_dir: "C:\\Test".to_string(),
            files_to_remove: Vec::new(),
            registry_entries: Vec::new(),
            shortcut_config: velocity_config::ShortcutConfig::default(),
            start_menu_folder: None,
            env_vars: Vec::new(),
            services: Vec::new(),
            file_associations: Vec::new(),
            pre_uninstall: Vec::new(),
            post_uninstall: Vec::new(),
        };

        let files = vec![
            std::path::PathBuf::from("C:\\Test\\app.exe"),
            std::path::PathBuf::from("C:\\Test\\readme.txt"),
            std::path::PathBuf::from("C:\\Test\\lib\\core.dll"),
        ];

        populate_files_to_remove(&mut info, &files);
        assert_eq!(info.files_to_remove.len(), 3);
        assert_eq!(info.files_to_remove[0], "C:\\Test\\app.exe");
        assert_eq!(info.files_to_remove[2], "C:\\Test\\lib\\core.dll");
    }

    #[test]
    fn test_uninstall_info_serialization() {
        let info = UninstallInfo {
            app_name: "MyApp".to_string(),
            install_dir: "C:\\Program Files\\MyApp".to_string(),
            files_to_remove: vec!["C:\\Program Files\\MyApp\\app.exe".to_string()],
            registry_entries: Vec::new(),
            shortcut_config: velocity_config::ShortcutConfig::default(),
            start_menu_folder: Some("MyApp".to_string()),
            env_vars: Vec::new(),
            services: Vec::new(),
            file_associations: Vec::new(),
            pre_uninstall: Vec::new(),
            post_uninstall: Vec::new(),
        };

        let json = serde_json::to_vec(&info).unwrap();
        let deserialized: UninstallInfo = serde_json::from_slice(&json).unwrap();
        assert_eq!(deserialized.app_name, "MyApp");
        assert_eq!(deserialized.files_to_remove.len(), 1);
    }
}
