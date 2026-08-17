//! Cross-platform runtime for Linux and macOS.
//!
//! Implements the full installation flow with the same safety checks
//! as the Windows runtime: process detection, disk space validation,
//! and install directory validation.

use super::*;
use anyhow::Result;
use std::path::PathBuf;
use tracing::{error, info, warn};
use velocity_config::VelocityManifest;
use velocity_core::logging;
use velocity_core::payload;
use velocity_core::rollback::RollbackTracker;
use velocity_core::{
    disk_space, env_vars, extract, file_assoc, process_detect, services, shortcuts, uninstaller,
};

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
        match velocity_ui::run_install_wizard_with_payload(
            &manifest,
            Some(payload_data.clone()),
            Some(
                run_install_with_progress
                    as fn(
                        &VelocityManifest,
                        &std::path::Path,
                        &[u8],
                        fn(u32, String),
                    ) -> Result<()>,
            ),
        ) {
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

    // Safety check: validate install directory
    if let Err(e) = validate_install_dir_unix(&install_dir) {
        error!("Invalid install directory: {}", e);
        velocity_ui::show_error("Invalid Directory", &e);
        std::process::exit(1);
    }

    // Safety check: check if app is already running
    if let Some(ref main_exe) = manifest.install.run_after_install {
        if process_detect::is_app_running(main_exe).unwrap_or(false) {
            warn!("Application {} is currently running", main_exe);
            if manifest.install.close_app_before_install {
                info!("Closing application...");
                let process_name = std::path::Path::new(main_exe)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(main_exe);
                let _ = process_detect::kill_process_by_name(process_name);
                let _ = process_detect::wait_for_process_exit(process_name, 10);
            } else if !args.silent {
                // In GUI mode, the wizard already showed; just warn and proceed
                warn!("Application is running, user chose to proceed");
            }
        }
    }

    // Safety check: verify disk space
    let payload_size = payload_data.len() as u64;
    let estimated_size = payload_size.saturating_mul(3).max(10 * 1024 * 1024);
    if let Err(e) = disk_space::check_disk_space(&install_dir, estimated_size) {
        error!("Disk space check failed: {}", e);
        velocity_ui::show_error(
            "Insufficient Disk Space",
            &format!("{}\n\nPlease free up some disk space and try again.", e),
        );
        return Err(e.into());
    }
    info!(
        "Disk space OK — {} free required",
        disk_space::format_bytes(estimated_size)
    );

    // Run the installation (or skip if already done inside the wizard)
    if !wizard_result.install_completed {
        let install_dir_clone = install_dir.clone();
        let manifest_clone = manifest.clone();
        let payload_clone = payload_data.clone();
        run_install_with_progress(
            &manifest_clone,
            &install_dir_clone,
            &payload_clone,
            |pct, msg| {
                info!("[{}%] {}", pct, msg);
            },
        )?;
    }

    // Show completion
    velocity_ui::show_complete(&manifest.app.name, &install_dir);
    info!(
        "Installation complete: {} v{}",
        manifest.app.name, manifest.app.version
    );

    Ok(())
}

/// Run the full installation with progress reporting.
///
/// Performs directory creation, file extraction, shortcut creation,
/// service installation, environment variable setup, file associations,
/// uninstaller generation, and post-install scripts. Reports progress
/// via the `progress` callback as `fn(percentage, status_message)`.
///
/// Extraction reports per-file progress (5%–50%), and post-install steps
/// are tracked in the rollback tracker so they can be undone on failure.
pub fn run_install_with_progress(
    manifest: &VelocityManifest,
    install_dir: &std::path::Path,
    payload_data: &[u8],
    progress: fn(u32, String),
) -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Create install directory
    progress(0, "Creating install directory...".into());
    if let Err(e) = std::fs::create_dir_all(install_dir) {
        return Err(anyhow::anyhow!("Failed to create install directory: {}", e));
    }

    // Set up rollback tracker
    let mut rollback = RollbackTracker::new();
    rollback.track_dir(install_dir.to_path_buf());

    // Extract files with per-file progress reporting
    progress(5, "Extracting files...".into());
    let extracted_count = AtomicUsize::new(0);
    let progress_cb: extract::ProgressCallback = Box::new({
        move |_current, _total, name| {
            let n = extracted_count.fetch_add(1, Ordering::Relaxed);
            // Map extraction to 5%–50% range
            let pct = 5 + (n as u32 + 1).min(45);
            let display_name = if name.len() > 40 {
                format!("...{}", &name[name.len() - 37..])
            } else {
                name.to_string()
            };
            progress(pct, display_name);
        }
    });

    let extracted_files =
        match extract::extract_archive(payload_data, install_dir, Some(&progress_cb)) {
            Ok(files) => {
                // Track each extracted file for rollback
                for f in &files {
                    rollback.track_file(f.clone());
                }
                info!(
                    "Extracted {} files to {}",
                    files.len(),
                    install_dir.display()
                );
                files
            }
            Err(e) => {
                error!("Extraction failed: {}", e);
                let _ = rollback.rollback();
                return Err(anyhow::anyhow!("Extraction failed: {}", e));
            }
        };
    progress(50, format!("Extracted {} files", extracted_files.len()));

    // Create shortcuts (tracked for rollback)
    if manifest.shortcuts.desktop || manifest.shortcuts.start_menu {
        progress(55, "Creating shortcuts...".into());
        let exe_name = manifest.app.name.replace(' ', "-").to_lowercase();
        let target_exe = install_dir.join(&exe_name);
        match shortcuts::create_shortcuts(
            &manifest.shortcuts,
            &manifest.app.name,
            &target_exe,
            install_dir,
            manifest.install.start_menu.as_deref(),
        ) {
            Ok(_) => {
                // Track shortcut files for rollback
                if manifest.shortcuts.desktop {
                    let desktop_name = format!(
                        "{}.desktop",
                        manifest.app.name.replace(' ', "-").to_lowercase()
                    );
                    rollback.track_shortcut(install_dir.join(&desktop_name));
                }
                info!("Shortcuts created");
            }
            Err(e) => {
                warn!("Failed to create shortcuts: {}", e);
            }
        }
    }

    // Install services (tracked for rollback)
    if !manifest.services.is_empty() {
        progress(65, "Installing services...".into());
        match services::install_services(&manifest.services, install_dir) {
            Ok(_) => {
                for svc in &manifest.services {
                    rollback.track_service(&svc.name);
                }
                info!("Services installed: {}", manifest.services.len());
            }
            Err(e) => {
                warn!("Failed to install services: {}", e);
            }
        }
    }

    // Apply environment variables (tracked for rollback)
    if !manifest.env_vars.is_empty() {
        progress(75, "Setting environment variables...".into());
        match env_vars::apply_env_vars(&manifest.env_vars) {
            Ok(_) => {
                for ev in &manifest.env_vars {
                    rollback.track_env_var(&ev.name, &ev.scope);
                }
                info!("Environment variables set: {}", manifest.env_vars.len());
            }
            Err(e) => {
                warn!("Failed to set environment variables: {}", e);
            }
        }
    }

    // Apply file associations
    if !manifest.file_associations.is_empty() {
        progress(80, "Setting file associations...".into());
        let exe_name = manifest.app.name.replace(' ', "-").to_lowercase();
        let target_exe = install_dir.join(&exe_name);
        if let Err(e) =
            file_assoc::apply_file_associations(&manifest.file_associations, &target_exe)
        {
            warn!("Failed to set file associations: {}", e);
        }
    }

    // Generate uninstaller
    progress(85, "Generating uninstaller...".into());
    let current_exe = std::env::current_exe()?;
    let runtime_bytes = std::fs::read(&current_exe).unwrap_or_default();
    if !runtime_bytes.is_empty() {
        let uninstaller_path = install_dir.join("uninstall");
        match uninstaller::generate_uninstaller(
            &runtime_bytes,
            manifest,
            install_dir,
            &uninstaller_path,
        ) {
            Ok(_) => {
                rollback.track_file(uninstaller_path.clone());
                info!("Uninstaller generated: {}", uninstaller_path.display());
            }
            Err(e) => {
                warn!("Failed to generate uninstaller: {}", e);
            }
        }
    }

    // Run post-install scripts
    if !manifest.scripts.post_install.is_empty() {
        progress(90, "Running post-install scripts...".into());
        for (i, cmd) in manifest.scripts.post_install.iter().enumerate() {
            info!("Running post-install script: {}", cmd);
            let _ = std::process::Command::new("sh").args(["-c", cmd]).output();
            let pct = 90 + ((i + 1) * 8 / manifest.scripts.post_install.len().max(1)) as u32;
            progress(
                pct.min(98),
                format!("Script {}/{}", i + 1, manifest.scripts.post_install.len()),
            );
        }
    }

    // Clear rollback (installation succeeded)
    rollback.clear();
    progress(100, "Installation complete".into());

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

/// Validate the install directory on Unix systems.
///
/// Rejects dangerous system paths that should never be used as install targets.
fn validate_install_dir_unix(path: &std::path::Path) -> std::result::Result<(), String> {
    super::validate_unix_install_path(&path.to_string_lossy())
}
