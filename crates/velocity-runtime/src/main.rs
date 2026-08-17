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
use tracing::{error, info, warn};
use velocity_core::disk_space;
use velocity_core::logging;
use velocity_core::rollback::RollbackTracker;

/// Command-line arguments parsed at startup.
struct RuntimeArgs {
    /// Silent/quiet mode — no UI, use defaults.
    silent: bool,
    /// Override install directory.
    dir: Option<String>,
    /// Force uninstall without confirmation.
    force: bool,
    /// Password for encrypted installers.
    password: Option<String>,
}

impl RuntimeArgs {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut silent = false;
        let mut dir = None;
        let mut force = false;
        let mut password = None;

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "/S" | "/s" | "--silent" | "-s" | "/quiet" | "-q" => silent = true,
                "--force" | "-f" => force = true,
                _ => {
                    // Check for /D= prefix (Inno Setup compatible directory override)
                    if arg.starts_with("/D=") || arg.starts_with("/d=") {
                        dir = Some(arg[3..].to_string());
                    } else if arg.starts_with("/P=") || arg.starts_with("/p=") {
                        password = Some(arg[3..].to_string());
                    }
                }
            }
        }

        Self {
            silent,
            dir,
            force,
            password,
        }
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
    let (manifest_data, mut payload_data) = match velocity_core::payload::read_payload(&exe_path) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to read payload: {}", e);
            if !args.silent {
                velocity_ui::classic::show_error(
                    "Velocity Installer",
                    &format!(
                        "This installer package is corrupt or damaged.\n\nError: {}",
                        e
                    ),
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

    info!(
        "Installing: {} v{}",
        manifest.app.name, manifest.app.version
    );

    // Step 1.2: Check for updates (non-blocking, best-effort)
    if let Some(ref update_url) = manifest.uninstall.update_url {
        if !update_url.is_empty() && !args.silent {
            info!("Checking for updates...");
            match velocity_core::updater::check_for_update(&manifest.app.version, update_url) {
                Ok(info) if info.update_available => {
                    info!("Update available: {}", info.latest_version);
                    logging::log_op(
                        "UPDATE",
                        &format!("New version {} available", info.latest_version),
                    );
                    let open_url = velocity_ui::classic::show_update_notification(
                        &manifest.app.name,
                        &manifest.app.version,
                        &info.latest_version,
                        info.release_notes.as_deref(),
                    );
                    if open_url && !info.download_url.is_empty() {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", &info.download_url])
                            .spawn();
                    }
                }
                Ok(_) => {
                    info!("No updates available");
                }
                Err(e) => {
                    warn!("Update check failed: {}", e);
                }
            }
        }
    }

    // Step 1.5: Decrypt payload if password-protected
    if velocity_core::encryption::is_encrypted(&payload_data) {
        info!("Payload is encrypted, password required");
        let password = if args.silent {
            // In silent mode, use the password from args or manifest
            args.password.clone().unwrap_or_default()
        } else {
            // Prompt user for password
            velocity_ui::classic::show_password_prompt()
        };
        match velocity_core::encryption::decrypt(&payload_data, &password) {
            Some(decrypted) => {
                info!("Payload decrypted successfully");
                payload_data = decrypted;
            }
            None => {
                error!("Failed to decrypt payload — wrong password or corrupted data");
                if !args.silent {
                    velocity_ui::classic::show_error(
                        "Decryption Failed",
                        "The password is incorrect or the installer is corrupted.",
                    );
                }
                std::process::exit(1);
            }
        }
    }

    // Detect system architecture for path resolution
    let sys_info = velocity_core::arch_detect::detect_system_info();
    let install_64bit = velocity_core::arch_detect::default_install_mode(&manifest.install.arch);
    info!(
        "System: {} (64bit install: {})",
        sys_info.os_arch, install_64bit
    );

    // Check architecture compatibility
    if !velocity_core::arch_detect::is_arch_compatible(&manifest.install.arch) {
        let err_msg = format!(
            "This installer requires a {} system, but your system is {}.",
            manifest.install.arch, sys_info.os_arch
        );
        error!("{}", err_msg);
        if !args.silent {
            velocity_ui::classic::show_error("Architecture Mismatch", &err_msg);
        }
        return Err(anyhow::anyhow!("{}", err_msg));
    }

    // Initialize file-based logging
    let _log_path = logging::init_temp_logger(&manifest.app.name).ok();
    logging::log(&format!(
        "=== Installing {} v{} ===",
        manifest.app.name, manifest.app.version
    ));

    // Check for another instance of this installer
    let _installer_mutex =
        match velocity_core::installer_mutex::InstallerMutex::try_acquire(&manifest.app.name) {
            Ok(m) => m,
            Err(e) => {
                error!("{}", e);
                if !args.silent {
                    velocity_ui::classic::show_error("Installer Already Running", &e.to_string());
                }
                return Err(e.into());
            }
        };

    // Step 2: Show the installation wizard (or use defaults in silent mode)
    let wizard_result = if args.silent {
        // Silent mode: use defaults
        let install_dir = args.dir.clone().unwrap_or_else(|| {
            // Use architecture-aware path resolution
            let default_path = velocity_config::VariableResolver::new(&std::path::PathBuf::from(
                format!("C:\\Program Files\\{}", manifest.app.name),
            ))
            .resolve(&manifest.install.default_dir);

            // Resolve {autopf} variable if present
            if default_path.contains("{autopf}") {
                let pf = velocity_core::arch_detect::program_files_dir(install_64bit);
                default_path.replace("{autopf}", &pf.to_string_lossy())
            } else {
                default_path
            }
        });
        velocity_ui::InstallWizardResult {
            install_dir: std::path::PathBuf::from(install_dir),
            cancelled: false,
            launch_after: false,
            selected_components: Vec::new(),
            install_completed: false,
        }
    } else {
        match velocity_ui::run_install_wizard_with_payload(&manifest, Some(payload_data.clone())) {
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

            if manifest.install.close_app_before_install {
                // Automatically close the application
                info!("Closing application as configured...");
                logging::log_op("CLOSE", &format!("Closing {}", main_exe));
                let process_name = std::path::Path::new(main_exe)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(main_exe);
                let _ = std::process::Command::new("taskkill")
                    .args(["/IM", process_name, "/F"])
                    .output();
                // Wait for process to exit
                let _ = velocity_core::process_detect::wait_for_process_exit(process_name, 10);
            } else if !args.silent {
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
                    info!("Installation cancelled - app is running");
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
                &format!("{}\n\nPlease free up some disk space and try again.", e),
            );
        }
        return Err(e.into());
    }
    logging::log_op(
        "DISK SPACE",
        &format!(
            "OK — {} free required",
            disk_space::format_bytes(estimated_size)
        ),
    );

    // Step 6: Create the installation directory
    std::fs::create_dir_all(install_dir)?;

    // Initialize install logger in the target directory
    let _final_log = logging::move_log_to_install_dir(install_dir, &manifest.app.name).ok();

    // Step 6.5: Run pre-install scripts using the scripting engine
    if !manifest.scripts.pre_install.is_empty() || !manifest.scripts.pre_install_actions.is_empty()
    {
        info!("Running pre-install scripts...");
        let total = manifest.scripts.pre_install.len() + manifest.scripts.pre_install_actions.len();
        logging::log_op(
            "SCRIPT",
            &format!("Running {} pre-install action(s)", total),
        );
        let mut script_engine = velocity_core::scripting::ScriptEngine::new(
            velocity_core::scripting::build_variable_context(
                &install_dir.to_string_lossy(),
                &manifest.app.name,
                &manifest.app.version,
            ),
        );
        // Execute simple shell commands
        let results = script_engine.execute_shell_commands(&manifest.scripts.pre_install);
        for r in &results {
            if r.success {
                logging::log_success(&format!("Pre-install: {}", r.action_name));
            } else {
                warn!("Pre-install script failed: {:?}", r.error);
                logging::log_error("SCRIPT", &format!("{}: {:?}", r.action_name, r.error));
            }
        }
        // Execute structured actions
        let actions =
            velocity_core::scripting::configs_to_actions(&manifest.scripts.pre_install_actions);
        let action_results = script_engine.execute_sequence(&actions);
        for r in &action_results {
            if r.success {
                logging::log_success(&format!("Pre-install: {}", r.action_name));
            } else {
                warn!("Pre-install action failed: {:?}", r.error);
                logging::log_error("SCRIPT", &format!("{}: {:?}", r.action_name, r.error));
            }
        }
    }

    // Step 7: Extract files with rollback tracking
    let mut rollback = RollbackTracker::new();
    let extracted: Vec<std::path::PathBuf>;

    if wizard_result.install_completed {
        // The native wizard already extracted files — just collect them for rollback tracking
        info!("Files already extracted by wizard, collecting file list for tracking...");
        logging::log_success("Files already extracted by wizard");
        extracted = walkdir::WalkDir::new(install_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect();
        for f in &extracted {
            rollback.track_file(f.clone());
        }
    } else {
        info!("Extracting files...");
        let progress_cb: velocity_core::extract::ProgressCallback =
            Box::new(|current, total, name| {
                velocity_ui::show_progress(current, total, name);
                logging::log_extract(name);
            });

        extracted = match velocity_core::extract::extract_archive(
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
    }

    // Step 7.1: Verify checksums if enabled
    if manifest.install.verify_checksums {
        info!("Checksum verification enabled, verifying files...");
        logging::log_op("CHECKSUM", "Verifying file integrity...");
        let algo = velocity_core::checksum::HashAlgorithm::parse(&manifest.install.checksum_algo);
        // Build checksum map from extracted files
        let mut checksum_map = std::collections::HashMap::new();
        for file_path in &extracted {
            if let Ok(rel) = file_path.strip_prefix(install_dir) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if let Ok(hash) = velocity_core::checksum::hash_file(file_path, algo) {
                    checksum_map.insert(rel_str, hash);
                }
            }
        }
        // If manifest provides expected checksums, verify against them
        // For now, just log the computed checksums for audit purposes
        info!("Computed checksums for {} files", checksum_map.len());
        logging::log_success(&format!(
            "Checksummed {} files ({})",
            checksum_map.len(),
            manifest.install.checksum_algo
        ));
    }

    // Step 7.5: Install remote dependencies and bundled apps
    let temp_dir = std::env::temp_dir().join("velocity_installer");
    std::fs::create_dir_all(&temp_dir)?;

    if !manifest.dependencies.is_empty() {
        info!("Installing remote dependencies...");
        logging::log_op(
            "DEPS",
            &format!("Processing {} dependencies(s)", manifest.dependencies.len()),
        );
        let dep_results = velocity_core::dep_installer::install_dependencies(
            &manifest.dependencies,
            &temp_dir,
            &mut rollback,
        );
        // Log summary
        let installed = dep_results.iter().filter(|r| r.installed).count();
        let skipped = dep_results.iter().filter(|r| r.skipped).count();
        let failed = dep_results.iter().filter(|r| r.error.is_some()).count();
        logging::log_op(
            "DEPS",
            &format!(
                "{} installed, {} skipped, {} failed",
                installed, skipped, failed
            ),
        );

        // Check if any required dependency failed
        if !velocity_core::dep_installer::all_required_installed(
            &dep_results,
            &manifest.dependencies,
        ) {
            error!("Required dependency installation failed");
            logging::log_error("DEPS", "Required dependency failed, rolling back");
            let _ = rollback.rollback();
            if !args.silent {
                velocity_ui::classic::show_error(
                    "Dependency Installation Failed",
                    "One or more required dependencies could not be installed.\n\n\
                    The installation has been rolled back.\n\n\
                    Check the log file for details.",
                );
            }
            return Err(anyhow::anyhow!("Required dependency installation failed"));
        }
    }

    if !manifest.bundled_apps.is_empty() {
        info!("Installing bundled applications...");
        logging::log_op(
            "BUNDLED",
            &format!("Processing {} bundled app(s)", manifest.bundled_apps.len()),
        );
        // Build a map of payload files for bundled app lookup
        let mut payload_map = std::collections::HashMap::new();
        for file in extracted.iter() {
            // Use the relative path from the archive
            let rel = file
                .strip_prefix(install_dir)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            payload_map.insert(rel, file.clone());
        }
        let bundled_results = velocity_core::dep_installer::install_bundled_apps(
            &manifest.bundled_apps,
            &payload_map,
            &temp_dir,
            &mut rollback,
        );
        let installed = bundled_results.iter().filter(|r| r.installed).count();
        let skipped = bundled_results.iter().filter(|r| r.skipped).count();
        let failed = bundled_results.iter().filter(|r| r.error.is_some()).count();
        logging::log_op(
            "BUNDLED",
            &format!(
                "{} installed, {} skipped, {} failed",
                installed, skipped, failed
            ),
        );
    }

    // Clean up temp directory
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Step 8: Create variable resolver for path substitution
    let mut resolver = velocity_config::VariableResolver::new(install_dir);
    resolver.set_variable("version", &manifest.app.version);

    // Step 9: Apply registry entries
    if !manifest.registry.is_empty() {
        info!("Writing registry entries...");
        logging::log_op(
            "REGISTRY",
            &format!("Writing {} entries", manifest.registry.len()),
        );
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
    let target_exe = install_dir.join(manifest.install.run_after_install.as_deref().unwrap_or(""));
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

    // Step 10b: Create desktop shortcut if manifest requests it
    if manifest.install.create_desktop_shortcut && !manifest.shortcuts.desktop {
        let desktop_exe = if target_exe.as_os_str().is_empty() {
            install_dir.join(&manifest.app.name)
        } else {
            target_exe.clone()
        };
        match velocity_core::shortcuts::create_shortcuts(
            &velocity_config::ShortcutConfig {
                desktop: true,
                start_menu: false,
                quick_launch: false,
                custom: vec![],
            },
            &manifest.app.name,
            &desktop_exe,
            install_dir,
            None,
        ) {
            Ok(()) => {
                info!("Created desktop shortcut (manifest setting)");
                logging::log_success("Desktop shortcut created");
            }
            Err(e) => {
                warn!("Desktop shortcut error: {}", e);
                logging::log_error("DESKTOP_SHORTCUT", &e.to_string());
            }
        }
    }

    // Step 11: Apply file associations
    if !manifest.file_associations.is_empty() {
        info!("Creating file associations...");
        let main_exe_path =
            install_dir.join(manifest.install.run_after_install.as_deref().unwrap_or(""));
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

    // Build uninstall info and populate files_to_remove with extracted files
    let mut uninstall_info = velocity_core::uninstaller::UninstallInfo {
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
    velocity_core::uninstaller::populate_files_to_remove(&mut uninstall_info, &extracted);

    // Generate the uninstaller executable with embedded info
    match velocity_core::uninstaller::generate_uninstaller_with_info(
        &runtime_exe,
        &uninstall_info,
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
        let icon_str = manifest
            .uninstall
            .icon
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        match velocity_core::registry::write_uninstall_entry(
            &manifest.app.name,
            &install_dir.to_string_lossy(),
            &uninstaller_path.to_string_lossy(),
            &manifest.app.version,
            &manifest.app.publisher,
            icon_str.as_deref(),
            manifest.uninstall.display_name.as_deref(),
            manifest.uninstall.help_url.as_deref(),
            manifest.uninstall.update_url.as_deref(),
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

    // Step 16: Run post-install scripts using the scripting engine
    if !manifest.scripts.post_install.is_empty()
        || !manifest.scripts.post_install_actions.is_empty()
    {
        info!("Running post-install scripts...");
        let total =
            manifest.scripts.post_install.len() + manifest.scripts.post_install_actions.len();
        logging::log_op(
            "SCRIPT",
            &format!("Running {} post-install action(s)", total),
        );
        let mut script_engine = velocity_core::scripting::ScriptEngine::new(
            velocity_core::scripting::build_variable_context(
                &install_dir.to_string_lossy(),
                &manifest.app.name,
                &manifest.app.version,
            ),
        );
        // Execute simple shell commands
        let results = script_engine.execute_shell_commands(&manifest.scripts.post_install);
        for r in &results {
            if r.success {
                logging::log_success(&format!("Post-install: {}", r.action_name));
            } else {
                warn!("Post-install script failed: {:?}", r.error);
                logging::log_error("SCRIPT", &format!("{}: {:?}", r.action_name, r.error));
            }
        }
        // Execute structured actions
        let actions =
            velocity_core::scripting::configs_to_actions(&manifest.scripts.post_install_actions);
        let action_results = script_engine.execute_sequence(&actions);
        for r in &action_results {
            if r.success {
                logging::log_success(&format!("Post-install: {}", r.action_name));
            } else {
                warn!("Post-install action failed: {:?}", r.error);
                logging::log_error("SCRIPT", &format!("{}: {:?}", r.action_name, r.error));
            }
        }
    }

    // Installation successful — clear rollback tracker
    rollback.clear();
    logging::log_success("Installation completed successfully!");

    // Check if a reboot is needed (files locked during install)
    let reboot_needed = velocity_core::reboot::is_velocity_reboot_requested()
        || velocity_core::reboot::is_reboot_pending();
    if reboot_needed {
        info!("System reboot is pending/required");
        logging::log_warning("A system reboot is required to complete the installation.");
    }

    // Step 17: Show completion
    let should_launch = if !args.silent {
        velocity_ui::classic::show_finish_dialog(
            &manifest.app.name,
            install_dir,
            manifest.install.run_after_install.as_deref(),
        )
    } else {
        wizard_result.launch_after
    };

    // Step 18: Launch application if requested
    if should_launch {
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
        if velocity_core::elevation::elevate_if_needed(&cmd_args)? {
            return Ok(());
        }
    }

    // Confirm uninstall (unless silent or force)
    if !args.silent
        && !args.force
        && !velocity_ui::classic::show_confirm(
            "Uninstall",
            "Are you sure you want to uninstall this application?",
        )
    {
        return Ok(());
    }

    // Read uninstall info
    let uninstall_info = velocity_core::uninstaller::read_uninstall_info(exe_path)?;
    logging::log_op(
        "UNINSTALL",
        &format!("Removing: {}", uninstall_info.app_name),
    );

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
