//! Cross-platform runtime for Linux and macOS.
//!
//! Implements the full installation flow using terminal UI and
//! cross-platform core modules.

use super::*;
use anyhow::Result;
use std::path::PathBuf;
use tracing::{error, info, warn};
use velocity_config::VelocityManifest;
use velocity_core::logging;
use velocity_core::payload;
use velocity_core::rollback::RollbackTracker;
use velocity_core::{env_vars, extract, file_assoc, services, shortcuts, uninstaller};

/// Run the installer on Linux/macOS.
pub fn run() -> Result<()> {
    let args = RuntimeArgs::parse();

    // Initialize logging
    logging::init("velocity-runtime");
    info!("Velocity Runtime starting (non-Windows)");

    // Check for uninstall mode
    if uninstaller::is_uninstall_mode() {
        return run_uninstall();
    }

    // Read the embedded payload
    info!("Reading embedded payload...");
    let exe_path = std::env::current_exe()?;
    let (manifest, payload_data) = match payload::read_payload(&exe_path, args.password.as_deref())
    {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to read payload: {}", e);
            eprintln!("Error: This installer payload could not be read.");
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    };

    info!(
        "Manifest loaded for: {} v{}",
        manifest.app.name, manifest.app.version
    );

    // Show wizard or use defaults
    let wizard_result = if args.silent {
        info!("Silent mode — using defaults");
        let install_dir = args
            .dir
            .map(PathBuf::from)
            .unwrap_or_else(|| velocity_core::platform::default_install_dir(&manifest.app.name));

        let selected_components: Vec<String> = manifest
            .components
            .iter()
            .filter(|c| c.selected_by_default || c.mandatory)
            .map(|c| c.id.clone())
            .collect();

        velocity_ui::InstallWizardResult {
            install_dir,
            cancelled: false,
            launch_after: false,
            selected_components,
            install_completed: false,
        }
    } else {
        match velocity_ui::run_install_wizard(&manifest) {
            Ok(result) => result,
            Err(velocity_ui::UiError::Cancelled) => {
                info!("Installation cancelled by user");
                return Ok(());
            }
            Err(e) => {
                error!("Wizard error: {}", e);
                velocity_ui::show_error("Installation Error", &format!("{}", e));
                std::process::exit(1);
            }
        }
    };

    if wizard_result.cancelled {
        info!("Installation cancelled");
        return Ok(());
    }

    let install_dir = PathBuf::from(&wizard_result.install_dir);
    info!("Installing to: {}", install_dir.display());

    // Create install directory
    if let Err(e) = std::fs::create_dir_all(&install_dir) {
        error!("Failed to create install directory: {}", e);
        velocity_ui::show_error("Error", &format!("Failed to create directory: {}", e));
        std::process::exit(1);
    }

    // Set up rollback tracker
    let mut rollback = RollbackTracker::new();
    rollback.track_dir(install_dir.clone());

    // Extract files
    info!("Extracting files...");
    match extract::extract_from_bytes(&payload_data, &install_dir, Some(&mut rollback)) {
        Ok(stats) => {
            info!(
                "Extracted {} files ({} bytes) to {}",
                stats.files_extracted,
                stats.total_bytes,
                install_dir.display()
            );
        }
        Err(e) => {
            error!("Extraction failed: {}", e);
            velocity_ui::show_error("Extraction Error", &format!("{}", e));
            warn!("Rolling back...");
            let _ = rollback.rollback();
            std::process::exit(1);
        }
    }

    // Create shortcuts
    if manifest.shortcuts.desktop || manifest.shortcuts.start_menu {
        info!("Creating shortcuts...");
        let exe_name = if cfg!(windows) {
            format!("{}.exe", manifest.app.name.replace(' ', "-"))
        } else {
            manifest.app.name.replace(' ', "-").to_lowercase()
        };
        let target_exe = install_dir.join(&exe_name);
        if let Err(e) = shortcuts::create_shortcuts(
            &manifest.shortcuts,
            &manifest.app.name,
            &target_exe,
            &install_dir,
            manifest.install.start_menu.as_deref(),
        ) {
            warn!("Failed to create shortcuts: {}", e);
        }
    }

    // Install services
    if !manifest.services.is_empty() {
        info!("Installing services...");
        if let Err(e) = services::install_services(&manifest.services, &install_dir) {
            warn!("Failed to install services: {}", e);
        }
    }

    // Apply environment variables
    if !manifest.env_vars.is_empty() {
        info!("Setting environment variables...");
        if let Err(e) = env_vars::apply_env_vars(&manifest.env_vars) {
            warn!("Failed to set environment variables: {}", e);
        }
    }

    // Apply file associations
    if !manifest.file_associations.is_empty() {
        info!("Setting file associations...");
        let exe_name = manifest.app.name.replace(' ', "-").to_lowercase();
        let target_exe = install_dir.join(&exe_name);
        if let Err(e) =
            file_assoc::apply_file_associations(&manifest.file_associations, &target_exe)
        {
            warn!("Failed to set file associations: {}", e);
        }
    }

    // Generate uninstaller
    info!("Generating uninstaller...");
    let current_exe = std::env::current_exe()?;
    let runtime_bytes = std::fs::read(&current_exe).unwrap_or_default();
    if !runtime_bytes.is_empty() {
        let uninstaller_name = if cfg!(windows) {
            "uninstall.exe"
        } else {
            "uninstall"
        };
        let uninstaller_path = install_dir.join(uninstaller_name);
        match uninstaller::generate_uninstaller(
            &runtime_bytes,
            &manifest,
            &install_dir,
            &uninstaller_path,
        ) {
            Ok(_) => {
                rollback.track_file(uninstaller_path);
                info!("Uninstaller generated: {}", uninstaller_path.display());
            }
            Err(e) => {
                warn!("Failed to generate uninstaller: {}", e);
            }
        }
    }

    // Run post-install scripts
    for cmd in &manifest.scripts.post_install {
        info!("Running post-install script: {}", cmd);
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/C", cmd]).output();
        #[cfg(not(target_os = "windows"))]
        let _ = std::process::Command::new("sh").args(["-c", cmd]).output();
    }

    // Clear rollback (installation succeeded)
    rollback.clear();

    // Show completion
    velocity_ui::show_complete(&manifest.app.name, &install_dir);
    info!(
        "Installation complete: {} v{}",
        manifest.app.name, manifest.app.version
    );

    Ok(())
}

/// Run the uninstaller on Linux/macOS.
fn run_uninstall() -> Result<()> {
    info!("Running uninstaller (non-Windows)");

    let exe_path = std::env::current_exe()?;
    let uninstall_info = match uninstaller::read_uninstall_info(&exe_path) {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to read uninstall info: {}", e);
            eprintln!("Error: Could not read uninstall information.");
            std::process::exit(1);
        }
    };

    if let Err(e) = uninstaller::perform_uninstall(&uninstall_info) {
        error!("Uninstall failed: {}", e);
        velocity_ui::show_error("Uninstall Error", &format!("{}", e));
        std::process::exit(1);
    }

    println!("Uninstallation complete.");
    Ok(())
}
