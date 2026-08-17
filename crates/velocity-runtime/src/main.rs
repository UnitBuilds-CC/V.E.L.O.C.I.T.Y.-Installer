//! Velocity Runtime — the lightweight binary embedded in each installer.
//!
//! When a user runs a Velocity-built installer, this runtime:
//! 1. Reads the embedded manifest and payload
//! 2. Shows the installation wizard
//! 3. Extracts files to the chosen directory
//! 4. Creates registry entries, shortcuts, etc.
//! 5. Generates the uninstaller
//! 6. Shows the completion dialog

use anyhow::Result;
use tracing::{info, error};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("velocity=info".parse().unwrap()),
        )
        .init();

    let exe_path = std::env::current_exe()?;

    // Check if running in uninstall mode
    if velocity_core::uninstaller::is_uninstall_mode() {
        return run_uninstall(&exe_path);
    }

    // Check if we need admin elevation
    // (This is checked again after the wizard, but we check early for better UX)

    info!("Velocity Installer Runtime starting...");

    // Step 1: Read the embedded manifest and payload
    let (manifest_data, payload_data) = match velocity_core::payload::read_payload(&exe_path) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to read payload: {}", e);
            velocity_ui::classic::show_message(
                "Velocity Installer",
                &format!("This installer package is corrupt or damaged.\n\nError: {}", e),
            );
            std::process::exit(1);
        }
    };

    let manifest: velocity_config::VelocityManifest = match serde_json::from_slice(&manifest_data) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to parse manifest: {}", e);
            velocity_ui::classic::show_message(
                "Velocity Installer",
                &format!("Invalid installer configuration.\n\nError: {}", e),
            );
            std::process::exit(1);
        }
    };

    info!("Installing: {} v{}", manifest.app.name, manifest.app.version);

    // Step 2: Show the installation wizard
    let wizard_result = match velocity_ui::run_install_wizard(&manifest) {
        Ok(result) => result,
        Err(velocity_ui::UiError::Cancelled) => {
            info!("Installation cancelled by user");
            return Ok(());
        }
        Err(e) => {
            error!("Wizard error: {}", e);
            return Err(e.into());
        }
    };

    let install_dir = &wizard_result.install_dir;
    info!("Installing to: {}", install_dir.display());

    // Step 3: Check if elevation is needed
    if manifest.install.require_admin && !velocity_core::elevation::is_admin() {
        let args = std::env::args().collect::<Vec<_>>();
        match velocity_core::elevation::elevate_if_needed(&args)? {
            true => {
                // Elevated process started, exit this one
                return Ok(());
            }
            false => {
                // Already elevated (shouldn't reach here)
            }
        }
    }

    // Step 4: Create the installation directory
    std::fs::create_dir_all(install_dir)?;

    // Step 5: Extract files
    info!("Extracting files...");
    let progress_cb: velocity_core::extract::ProgressCallback = Box::new(
        |current, total, name| {
            velocity_ui::show_progress(current, total, name);
        },
    );
    let extracted = velocity_core::extract::extract_archive(
        &payload_data,
        install_dir,
        Some(&progress_cb),
    )?;
    info!("Extracted {} files", extracted.len());

    // Step 6: Create variable resolver for path substitution
    let mut resolver = velocity_config::VariableResolver::new(install_dir);
    resolver.set_variable("version", &manifest.app.version);

    // Step 7: Apply registry entries
    if !manifest.registry.is_empty() {
        info!("Writing registry entries...");
        velocity_core::registry::apply_registry_entries(&manifest.registry)?;
    }

    // Step 8: Create shortcuts
    info!("Creating shortcuts...");
    let target_exe = install_dir.join(
        manifest.install.run_after_install.as_deref().unwrap_or("")
    );
    velocity_core::shortcuts::create_shortcuts(
        &manifest.shortcuts,
        &manifest.app.name,
        &target_exe,
        install_dir,
        manifest.install.start_menu.as_deref(),
    )?;

    // Step 9: Set environment variables
    if !manifest.env_vars.is_empty() {
        info!("Setting environment variables...");
        velocity_core::env_vars::apply_env_vars(&manifest.env_vars)?;
    }

    // Step 10: Install services
    if !manifest.services.is_empty() {
        info!("Installing services...");
        velocity_core::services::install_services(&manifest.services, install_dir)?;
    }

    // Step 11: Generate uninstaller
    info!("Generating uninstaller...");
    let runtime_exe = std::fs::read(&exe_path)?;
    let uninstaller_path = install_dir.join("uninstall.exe");
    velocity_core::uninstaller::generate_uninstaller(
        &runtime_exe,
        &manifest,
        install_dir,
        &uninstaller_path,
    )?;

    // Step 12: Register in Add/Remove Programs
    if manifest.uninstall.add_remove {
        let icon_str = manifest.uninstall.icon.as_ref().map(|p| p.to_string_lossy().into_owned());
        velocity_core::registry::write_uninstall_entry(
            &manifest.app.name,
            &install_dir.to_string_lossy(),
            &uninstaller_path.to_string_lossy(),
            &manifest.app.version,
            &manifest.app.publisher,
            icon_str.as_deref(),
            manifest.uninstall.display_name.as_deref(),
        )?;
    }

    // Step 13: Run post-install scripts
    for cmd in &manifest.scripts.post_install {
        info!("Running post-install: {}", cmd);
        let _ = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(install_dir)
            .output();
    }

    // Step 14: Show completion
    velocity_ui::show_complete(&manifest.app.name, install_dir);

    // Step 15: Launch application if requested
    if wizard_result.launch_after {
        if let Some(exe) = &manifest.install.run_after_install {
            let exe_path = install_dir.join(exe);
            if exe_path.exists() {
                info!("Launching: {}", exe_path.display());
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
fn run_uninstall(exe_path: &std::path::Path) -> Result<()> {
    info!("Running uninstaller...");

    // Check for admin
    if !velocity_core::elevation::is_admin() {
        let args = vec!["--uninstall".to_string()];
        match velocity_core::elevation::elevate_if_needed(&args)? {
            true => return Ok(()),
            false => {}
        }
    }

    // Confirm uninstall
    if !velocity_ui::classic::show_confirm(
        "Uninstall",
        "Are you sure you want to uninstall this application?",
    ) {
        return Ok(());
    }

    // Read uninstall info
    let uninstall_info = velocity_core::uninstaller::read_uninstall_info(exe_path)?;

    // Perform uninstallation
    velocity_core::uninstaller::perform_uninstall(&uninstall_info)?;

    velocity_ui::classic::show_message(
        "Uninstall Complete",
        &format!("{} has been successfully removed.", uninstall_info.app_name),
    );

    info!("Uninstallation complete!");
    Ok(())
}
