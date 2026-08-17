//! Velocity Runtime — the lightweight binary embedded in each installer.
//!
//! When a user runs a Velocity-built installer, this runtime:
//! 1. Reads the embedded manifest and payload
//! 2. Checks for silent mode (/S flag)
//! 3. Shows the installation wizard (or uses defaults in silent mode)
//! 4. Checks disk space
//! 5. Checks if the app is already running
//! 6. Extracts files to the chosen directory (with rollback tracking)
//! 7. Creates registry entries, shortcuts, file associations, etc.
//! 8. Generates the uninstaller
//! 9. Shows the completion dialog
//! 10. On failure, rolls back all changes

use anyhow::Result;
use tracing::{info, error, warn};
use velocity_core::rollback::RollbackTracker;
use velocity_core::logging;
use velocity_core::disk_space;

/// Command-line arguments parsed at startup.
struct RuntimeArgs {
    /// Silent/quiet mode — no UI, use defaults.
    silent: bool,
    /// Override install directory.
    dir: Option<String>,
    /// Force uninstall without confirmation.
    force: bool,
}

impl RuntimeArgs {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut silent = false;
        let mut dir = None;
        let mut force = false;

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "/S" | "/s" | "--silent" | "-s" | "/quiet" | "-q" => silent = true,
                "/D=path" | "/d=path" => {
                    dir = Some(arg[5..].to_string());
                }
                "--force" | "-f" => force = true,
                _ => {
                    // Check for /D= prefix (Inno Setup compatible)
                    if arg.starts_with("/D=") || arg.starts_with("/d=") {
                        dir = Some(arg[3..].to_string());
                    }
                }
            }
        }

        Self { silent, dir, force }
    }
}

fn main() -> Result<()> {
    // Initialize console logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("velocity=info".parse().unwrap()),
        )
        .init();

    let args = RuntimeArgs::parse();
    let exe_path = std::env::current_exe()?;

    // Check if running in uninstall mode
    if velocity_core::uninstaller::is_uninstall_mode() {
        return run_uninstall(&exe_path, &args);
    }

    info!("Velocity Installer Runtime starting...");

    // Step 1: Read the embedded manifest and payload
    let (manifest_data, payload_data) = match velocity_core::payload::read_payload(&exe_path) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to read payload: {}", e);
            if !args.silent {
                velocity_ui::classic::show_error(
                    "Velocity Installer",
                    &format!("This installer package is corrupt or damaged.\n\nError: {}", e),
                );
            }
            std::process::exit(1);
        }
    };

    let manifest: velocity_config::VelocityManifest = match serde_json::from_slice(&manifest_data) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to parse manifest: {}", e);
            if !args.silent {
                velocity_ui::classic::show_error(
                    "Velocity Installer",
                    &format!("Invalid installer configuration.\n\nError: {}", e),
                );
            }
            std::process::exit(1);
        }
    };

    info!("Installing: {} v{}", manifest.app.name, manifest.app.version);

    // Initialize file-based logging
    let _log_path = logging::init_temp_logger(&manifest.app.name).ok();
    logging::log(&format!("=== Installing {} v{} ===", manifest.app.name, manifest.app.version));

    // Step 2: Show the installation wizard (or use defaults in silent mode)
    let wizard_result = if args.silent {
        // Silent mode: use defaults
        let install_dir = args.dir.clone().unwrap_or_else(|| {
            velocity_config::VariableResolver::new(
                &std::path::PathBuf::from(format!("C:\\Program Files\\{}", manifest.app.name)),
            ).resolve(&manifest.install.default_dir)
        });
        velocity_ui::InstallWizardResult {
            install_dir: std::path::PathBuf::from(install_dir),
            cancelled: false,
            launch_after: false,
        }
    } else {
        match velocity_ui::run_install_wizard(&manifest) {
            Ok(result) => result,
            Err(velocity_ui::UiError::Cancelled) => {
                info!("Installation cancelled by user");
                logging::log("Installation cancelled by user");
                return Ok(());
            }
            Err(e) => {
                error!("Wizard error: {}", e);
                logging::log_error("Wizard", &e.to_string());
                return Err(e.into());
            }
        }
    };

    let install_dir = &wizard_result.install_dir;
    info!("Installing to: {}", install_dir.display());
    logging::log_op("INSTALL", &format!("Target: {}", install_dir.display()));

    // Step 3: Check if elevation is needed
    if manifest.install.require_admin && !velocity_core::elevation::is_admin() {
        let cmd_args = std::env::args().collect::<Vec<_>>();
        match velocity_core::elevation::elevate_if_needed(&cmd_args)? {
            true => {
                // Elevated process started, exit this one
                return Ok(());
            }
            false => {
                // Already elevated
            }
        }
    }

    // Step 4: Check if app is already running
    if let Some(ref main_exe) = manifest.install.run_after_install {
        if velocity_core::process_detect::is_app_running(main_exe).unwrap_or(false) {
            warn!("Application {} is currently running", main_exe);
            logging::log_warning(&format!("Application {} is currently running", main_exe));
            
            if !args.silent {
                let proceed = velocity_ui::classic::show_confirm(
                    "Application Running",
                    &format!(
                        "{} is currently running.\n\n\
                        It is recommended to close it before installing.\n\n\
                        Do you want to continue anyway?",
                        manifest.app.name
                    ),
                );
                if !proceed {
                    info!("Installation cancelled — app is running");
                    return Ok(());
                }
            }
        }
    }

    // Step 5: Check disk space
    let payload_size = payload_data.len() as u64;
    // Estimate uncompressed size as ~3x the compressed size (conservative)
    let estimated_size = payload_size.saturating_mul(3).max(10 * 1024 * 1024); // At least 10MB
    if let Err(e) = disk_space::check_disk_space(install_dir, estimated_size) {
        error!("Disk space check failed: {}", e);
        logging::log_error("DISK SPACE", &e.to_string());
        if !args.silent {
            velocity_ui::classic::show_error(
                "Insufficient Disk Space",
                &format!(
                    "{}\n\nPlease free up some disk space and try again.",
                    e
                ),
            );
        }
        return Err(e.into());
    }
    logging::log_op("DISK SPACE", &format!("OK — {} free required", disk_space::format_bytes(estimated_size)));

    // Step 6: Create the installation directory
    std::fs::create_dir_all(install_dir)?;

    // Initialize install logger in the target directory
    let _final_log = logging::move_log_to_install_dir(install_dir, &manifest.app.name).ok();

    // Step 7: Extract files with rollback tracking
    info!("Extracting files...");
    let mut rollback = RollbackTracker::new();
    
    let progress_cb: velocity_core::extract::ProgressCallback = Box::new(
        |current, total, name| {
            velocity_ui::show_progress(current, total, name);
            logging::log_extract(name);
        },
    );
    
    let extracted = match velocity_core::extract::extract_archive(
        &payload_data,
        install_dir,
        Some(&progress_cb),
    ) {
        Ok(files) => {
            for f in &files {
                rollback.track_file(f.clone());
            }
            files
        }
        Err(e) => {
            error!("Extraction failed: {}", e);
            logging::log_error("EXTRACT", &e.to_string());
            logging::log("Rolling back...");
            let _ = rollback.rollback();
            if !args.silent {
                velocity_ui::classic::show_error(
                    "Installation Failed",
                    &format!("File extraction failed.\n\nError: {}\n\nThe installation has been rolled back.", e),
                );
            }
            return Err(e.into());
        }
    };
    info!("Extracted {} files", extracted.len());
    logging::log_success(&format!("Extracted {} files", extracted.len()));

    // Step 8: Create variable resolver for path substitution
    let mut resolver = velocity_config::VariableResolver::new(install_dir);
    resolver.set_variable("version", &manifest.app.version);

    // Step 9: Apply registry entries
    if !manifest.registry.is_empty() {
        info!("Writing registry entries...");
        logging::log_op("REGISTRY", &format!("Writing {} entries", manifest.registry.len()));
        match velocity_core::registry::apply_registry_entries(&manifest.registry) {
            Ok(()) => {
                for entry in &manifest.registry {
                    rollback.track_registry(&entry.root, &entry.key);
                }
                logging::log_success("Registry entries applied");
            }
            Err(e) => {
                error!("Registry error: {}", e);
                logging::log_error("REGISTRY", &e.to_string());
                // Registry errors are non-fatal — continue installation
            }
        }
    }

    // Step 10: Create shortcuts
    info!("Creating shortcuts...");
    let target_exe = install_dir.join(
        manifest.install.run_after_install.as_deref().unwrap_or("")
    );
    match velocity_core::shortcuts::create_shortcuts(
        &manifest.shortcuts,
        &manifest.app.name,
        &target_exe,
        install_dir,
        manifest.install.start_menu.as_deref(),
    ) {
        Ok(()) => {
            logging::log_success("Shortcuts created");
        }
        Err(e) => {
            warn!("Shortcut creation error: {}", e);
            logging::log_error("SHORTCUT", &e.to_string());
            // Non-fatal
        }
    }

    // Step 11: Apply file associations
    if !manifest.file_associations.is_empty() {
        info!("Creating file associations...");
        let main_exe_path = install_dir.join(
            manifest.install.run_after_install.as_deref().unwrap_or("")
        );
        match velocity_core::file_assoc::apply_file_associations(
            &manifest.file_associations,
            &main_exe_path,
        ) {
            Ok(()) => {
                logging::log_success("File associations created");
            }
            Err(e) => {
                warn!("File association error: {}", e);
                logging::log_error("FILE_ASSOC", &e.to_string());
                // Non-fatal
            }
        }
    }

    // Step 12: Set environment variables
    if !manifest.env_vars.is_empty() {
        info!("Setting environment variables...");
        match velocity_core::env_vars::apply_env_vars(&manifest.env_vars) {
            Ok(()) => {
                for var in &manifest.env_vars {
                    rollback.track_env_var(&var.name, &var.scope);
                }
                logging::log_success("Environment variables set");
            }
            Err(e) => {
                warn!("Environment variable error: {}", e);
                logging::log_error("ENV_VARS", &e.to_string());
                // Non-fatal
            }
        }
    }

    // Step 13: Install services
    if !manifest.services.is_empty() {
        info!("Installing services...");
        match velocity_core::services::install_services(&manifest.services, install_dir) {
            Ok(()) => {
                for svc in &manifest.services {
                    rollback.track_service(&svc.name);
                }
                logging::log_success("Services installed");
            }
            Err(e) => {
                warn!("Service installation error: {}", e);
                logging::log_error("SERVICES", &e.to_string());
                // Non-fatal
            }
        }
    }

    // Step 14: Generate uninstaller
    info!("Generating uninstaller...");
    let runtime_exe = std::fs::read(&exe_path)?;
    let uninstaller_path = install_dir.join("uninstall.exe");
    match velocity_core::uninstaller::generate_uninstaller(
        &runtime_exe,
        &manifest,
        install_dir,
        &uninstaller_path,
    ) {
        Ok(()) => {
            rollback.track_file(uninstaller_path.clone());
            logging::log_success("Uninstaller generated");
        }
        Err(e) => {
            warn!("Uninstaller generation error: {}", e);
            logging::log_error("UNINSTALLER", &e.to_string());
            // Non-fatal
        }
    }

    // Step 15: Register in Add/Remove Programs
    if manifest.uninstall.add_remove {
        let icon_str = manifest.uninstall.icon.as_ref().map(|p| p.to_string_lossy().into_owned());
        match velocity_core::registry::write_uninstall_entry(
            &manifest.app.name,
            &install_dir.to_string_lossy(),
            &uninstaller_path.to_string_lossy(),
            &manifest.app.version,
            &manifest.app.publisher,
            icon_str.as_deref(),
            manifest.uninstall.display_name.as_deref(),
        ) {
            Ok(()) => {
                logging::log_success("Registered in Add/Remove Programs");
            }
            Err(e) => {
                warn!("Add/Remove registration error: {}", e);
                logging::log_error("ADD_REMOVE", &e.to_string());
            }
        }
    }

    // Step 16: Run post-install scripts
    for cmd in &manifest.scripts.post_install {
        info!("Running post-install: {}", cmd);
        logging::log_op("SCRIPT", cmd);
        let _ = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(install_dir)
            .output();
    }

    // Installation successful — clear rollback tracker
    rollback.clear();
    logging::log_success("Installation completed successfully!");

    // Step 17: Show completion
    if !args.silent {
        velocity_ui::classic::show_finish_dialog(
            &manifest.app.name,
            install_dir,
            manifest.install.run_after_install.as_deref(),
        );
    }

    // Step 18: Launch application if requested
    if wizard_result.launch_after {
        if let Some(exe) = &manifest.install.run_after_install {
            let exe_path = install_dir.join(exe);
            if exe_path.exists() {
                info!("Launching: {}", exe_path.display());
                logging::log_op("LAUNCH", &exe_path.display().to_string());
                let _ = std::process::Command::new(&exe_path)
                    .current_dir(install_dir)
                    .spawn();
            }
        }
    }

    info!("Installation complete!");
    Ok(())
}

/// Run the uninstallation process.
fn run_uninstall(exe_path: &std::path::Path, args: &RuntimeArgs) -> Result<()> {
    info!("Running uninstaller...");
    logging::log("=== Starting uninstallation ===");

    // Check for admin
    if !velocity_core::elevation::is_admin() {
        let cmd_args = vec!["--uninstall".to_string()];
        match velocity_core::elevation::elevate_if_needed(&cmd_args)? {
            true => return Ok(()),
            false => {}
        }
    }

    // Confirm uninstall (unless silent or force)
    if !args.silent && !args.force {
        if !velocity_ui::classic::show_confirm(
            "Uninstall",
            "Are you sure you want to uninstall this application?",
        ) {
            return Ok(());
        }
    }

    // Read uninstall info
    let uninstall_info = velocity_core::uninstaller::read_uninstall_info(exe_path)?;
    logging::log_op("UNINSTALL", &format!("Removing: {}", uninstall_info.app_name));

    // Perform uninstallation
    match velocity_core::uninstaller::perform_uninstall(&uninstall_info) {
        Ok(()) => {
            logging::log_success("Uninstallation completed successfully");
            if !args.silent {
                velocity_ui::classic::show_message(
                    "Uninstall Complete",
                    &format!("{} has been successfully removed.", uninstall_info.app_name),
                );
            }
        }
        Err(e) => {
            error!("Uninstall error: {}", e);
            logging::log_error("UNINSTALL", &e.to_string());
            if !args.silent {
                velocity_ui::classic::show_error(
                    "Uninstall Error",
                    &format!("Some files could not be removed.\n\nError: {}", e),
                );
            }
        }
    }

    info!("Uninstallation complete!");
    Ok(())
}
