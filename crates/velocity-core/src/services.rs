//! Windows service management.

use crate::error::{CoreError, Result};
use tracing::{debug, info};
use velocity_config::ServiceEntry;

/// Install and configure Windows services.
pub fn install_services(services: &[ServiceEntry], install_dir: &std::path::Path) -> Result<()> {
    for svc in services {
        install_service(svc, install_dir)?;
    }
    Ok(())
}

/// Install a single Windows service.
fn install_service(svc: &ServiceEntry, install_dir: &std::path::Path) -> Result<()> {
    let binary_path = install_dir.join(&svc.binary_path);

    info!("Installing service: {} ({})", svc.display_name, svc.name);

    // Use `sc create` command as a reliable way to install services
    let start_type_flag = match svc.start_type.as_str() {
        "auto" => "auto",
        "manual" => "demand",
        "disabled" => "disabled",
        "delayed_auto" => "delayed-auto",
        _ => "auto",
    };

    let mut cmd = std::process::Command::new("sc");
    cmd.args([
        "create",
        &svc.name,
        "binPath=",
        &binary_path.to_string_lossy(),
        "start=",
        start_type_flag,
        "DisplayName=",
        &svc.display_name,
    ]);

    // Add service account if specified
    if let Some(ref account) = svc.account {
        cmd.args(["obj=", account]);
    }

    // Add dependencies if specified
    if !svc.dependencies.is_empty() {
        let deps = svc.dependencies.join("/");
        cmd.args(["depend=", &deps]);
    }

    let output = cmd.output()
        .map_err(|e| CoreError::Other(format!("Failed to run sc create: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Other(format!(
            "Failed to install service '{}': {}",
            svc.name, stderr
        )));
    }

    // Set description if provided
    if let Some(desc) = &svc.description {
        let _ = std::process::Command::new("sc")
            .args(["description", &svc.name, desc])
            .output();
    }

    // Start the service if configured
    if svc.start_on_install {
        start_service(&svc.name)?;
    }

    debug!("Service installed: {}", svc.name);
    Ok(())
}

/// Start a Windows service.
pub fn start_service(name: &str) -> Result<()> {
    info!("Starting service: {}", name);
    let output = std::process::Command::new("sc")
        .args(["start", name])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to start service: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!("Service start returned non-zero (may already be running): {}", stderr);
    }
    Ok(())
}

/// Stop and remove Windows services.
pub fn remove_services(services: &[ServiceEntry]) -> Result<()> {
    for svc in services {
        if svc.remove_on_uninstall {
            remove_service(&svc.name)?;
        }
    }
    Ok(())
}

/// Stop and remove a single service.
fn remove_service(name: &str) -> Result<()> {
    info!("Removing service: {}", name);

    // Stop the service first
    let _ = std::process::Command::new("sc")
        .args(["stop", name])
        .output();

    // Wait briefly for the service to stop
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Delete the service
    let output = std::process::Command::new("sc")
        .args(["delete", name])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to delete service: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Other(format!(
            "Failed to remove service '{}': {}",
            name, stderr
        )));
    }

    debug!("Service removed: {}", name);
    Ok(())
}
