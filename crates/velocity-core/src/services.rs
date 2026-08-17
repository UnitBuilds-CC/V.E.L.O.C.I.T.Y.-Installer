//! Windows service management.

use crate::error::{CoreError, Result};
use tracing::{debug, info};
use velocity_config::ServiceEntry;

/// Validate a Windows service name.
/// Service names must contain only alphanumeric characters, underscores, hyphens, or dots.
fn validate_service_name(name: &str) -> Result<()> {
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err(CoreError::other("service name", format!(
            "Invalid service name: '{}'. Service names must contain only alphanumeric characters, underscores, hyphens, or dots.",
            name
        )));
    }
    Ok(())
}

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

    validate_service_name(&svc.name)?;

    info!("Installing service: {} ({})", svc.display_name, svc.name);

    // Use `sc create` command as a reliable way to install services
    // Note: sc.exe requires "key= value" as a SINGLE argument (space after = is the separator)
    let start_type_flag = match svc.start_type.as_str() {
        "auto" => "auto",
        "manual" => "demand",
        "disabled" => "disabled",
        "delayed_auto" => "delayed-auto",
        _ => "auto",
    };

    let bin_path_arg = format!("binPath= {}", binary_path.to_string_lossy());
    let start_arg = format!("start= {}", start_type_flag);
    let display_arg = format!("DisplayName= {}", svc.display_name);

    let mut cmd = std::process::Command::new("sc");
    cmd.args([
        "create",
        &svc.name,
        &bin_path_arg,
        &start_arg,
        &display_arg,
    ]);

    // Add service account if specified
    if let Some(ref account) = svc.account {
        let obj_arg = format!("obj= {}", account);
        cmd.arg(&obj_arg);
    }

    // Add dependencies if specified
    if !svc.dependencies.is_empty() {
        let deps = svc.dependencies.join("/");
        let depend_arg = format!("depend= {}", deps);
        cmd.arg(&depend_arg);
    }

    let output = cmd.output()
        .map_err(|e| CoreError::other("sc create", format!("{}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::other("install service", format!(
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
        .map_err(|e| CoreError::other("start service", format!("{}", e)))?;

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
    validate_service_name(name)?;

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
        .map_err(|e| CoreError::other("delete service", format!("{}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::other("remove service", format!(
            "Failed to remove service '{}': {}",
            name, stderr
        )));
    }

    debug!("Service removed: {}", name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_service_name_valid() {
        assert!(validate_service_name("MyService").is_ok());
        assert!(validate_service_name("my_service").is_ok());
        assert!(validate_service_name("my-service").is_ok());
        assert!(validate_service_name("my.service").is_ok());
        assert!(validate_service_name("Service123").is_ok());
    }

    #[test]
    fn test_validate_service_name_invalid() {
        assert!(validate_service_name("").is_err());
        assert!(validate_service_name("my service").is_err());
        assert!(validate_service_name("my/service").is_err());
        assert!(validate_service_name("my\\service").is_err());
        assert!(validate_service_name("my&service").is_err());
        assert!(validate_service_name("my|service").is_err());
    }
}
