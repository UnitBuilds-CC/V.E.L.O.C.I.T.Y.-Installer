//! Cross-platform service management.
//!
//! - Windows: `sc.exe` for Windows Services
//! - Linux: systemd unit files
//! - macOS: launchd plist files

use velocity_config::ServiceEntry;

// ===========================================================================
// Windows implementation (sc.exe)
// ===========================================================================
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use tracing::{debug, info};

    fn validate_service_name(name: &str) -> Result<()> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(CoreError::other("service name", format!(
                "Invalid service name: '{}'. Must contain only alphanumeric, underscores, hyphens, or dots.", name
            )));
        }
        Ok(())
    }

    pub fn install_services(
        services: &[ServiceEntry],
        install_dir: &std::path::Path,
    ) -> Result<()> {
        for svc in services {
            install_service(svc, install_dir)?;
        }
        Ok(())
    }

    fn install_service(svc: &ServiceEntry, install_dir: &std::path::Path) -> Result<()> {
        let binary_path = install_dir.join(&svc.binary_path);
        validate_service_name(&svc.name)?;
        info!("Installing service: {} ({})", svc.display_name, svc.name);
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
        cmd.args(["create", &svc.name, &bin_path_arg, &start_arg, &display_arg]);
        if let Some(ref account) = svc.account {
            cmd.arg(format!("obj= {}", account));
        }
        if !svc.dependencies.is_empty() {
            cmd.arg(format!("depend= {}", svc.dependencies.join("/")));
        }
        let output = cmd
            .output()
            .map_err(|e| CoreError::other("sc create", format!("{}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::other(
                "install service",
                format!("Failed to install service '{}': {}", svc.name, stderr),
            ));
        }
        if let Some(desc) = &svc.description {
            let _ = std::process::Command::new("sc")
                .args(["description", &svc.name, desc])
                .output();
        }
        if svc.start_on_install {
            start_service(&svc.name)?;
        }
        debug!("Service installed: {}", svc.name);
        Ok(())
    }

    pub fn start_service(name: &str) -> Result<()> {
        info!("Starting service: {}", name);
        let output = std::process::Command::new("sc")
            .args(["start", name])
            .output()
            .map_err(|e| CoreError::other("start service", format!("{}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!(
                "Service start returned non-zero (may already be running): {}",
                stderr
            );
        }
        Ok(())
    }

    pub fn remove_services(services: &[ServiceEntry]) -> Result<()> {
        for svc in services {
            if svc.remove_on_uninstall {
                remove_service(&svc.name)?;
            }
        }
        Ok(())
    }

    fn remove_service(name: &str) -> Result<()> {
        validate_service_name(name)?;
        info!("Removing service: {}", name);
        let _ = std::process::Command::new("sc")
            .args(["stop", name])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(2));
        let output = std::process::Command::new("sc")
            .args(["delete", name])
            .output()
            .map_err(|e| CoreError::other("delete service", format!("{}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::other(
                "remove service",
                format!("Failed to remove service '{}': {}", name, stderr),
            ));
        }
        debug!("Service removed: {}", name);
        Ok(())
    }
}

// ===========================================================================
// Linux implementation (systemd)
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use std::path::Path;
    use tracing::{debug, info};

    fn validate_service_name(name: &str) -> Result<()> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(CoreError::other("service name", format!(
                "Invalid service name: '{}'. Must contain only alphanumeric, underscores, or hyphens.", name
            )));
        }
        Ok(())
    }

    fn unit_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from("/etc/systemd/system").join(format!("{}.service", name))
    }

    pub fn install_services(services: &[ServiceEntry], install_dir: &Path) -> Result<()> {
        for svc in services {
            install_service(svc, install_dir)?;
        }
        Ok(())
    }

    fn install_service(svc: &ServiceEntry, install_dir: &Path) -> Result<()> {
        let binary_path = install_dir.join(&svc.binary_path);
        validate_service_name(&svc.name)?;
        info!(
            "Installing systemd service: {} ({})",
            svc.display_name, svc.name
        );

        let after = if svc.dependencies.is_empty() {
            String::new()
        } else {
            format!("After={}", svc.dependencies.join(" "))
        };
        let wants = if svc.dependencies.is_empty() {
            String::new()
        } else {
            format!("Wants={}", svc.dependencies.join(" "))
        };
        let description = svc.description.as_deref().unwrap_or(&svc.display_name);

        let unit_content = format!(
            "[Unit]\n\
             Description={}\n\
             {}\n\
             {}\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={}\n\
             WorkingDirectory={}\n\
             Restart=on-failure\n\
             \n\
             [Install]\n\
             WantedBy=multi-user.target\n",
            description,
            after,
            wants,
            binary_path.display(),
            install_dir.display(),
        );

        let path = unit_path(&svc.name);
        std::fs::write(&path, &unit_content).map_err(|e| {
            CoreError::other(
                "write unit",
                format!("Failed to write {}: {}", path.display(), e),
            )
        })?;

        let _ = std::process::Command::new("systemctl")
            .args(["daemon-reload"])
            .output();
        if svc.start_type == "auto" || svc.start_type == "delayed_auto" {
            let _ = std::process::Command::new("systemctl")
                .args(["enable", &svc.name])
                .output();
        }
        if svc.start_on_install {
            start_service(&svc.name)?;
        }

        debug!("Service installed: {}", svc.name);
        Ok(())
    }

    pub fn start_service(name: &str) -> Result<()> {
        info!("Starting service: {}", name);
        let output = std::process::Command::new("systemctl")
            .args(["start", name])
            .output()
            .map_err(|e| CoreError::other("start service", format!("{}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("Service start returned non-zero: {}", stderr);
        }
        Ok(())
    }

    pub fn remove_services(services: &[ServiceEntry]) -> Result<()> {
        for svc in services {
            if svc.remove_on_uninstall {
                remove_service(&svc.name)?;
            }
        }
        Ok(())
    }

    fn remove_service(name: &str) -> Result<()> {
        validate_service_name(name)?;
        info!("Removing service: {}", name);
        let _ = std::process::Command::new("systemctl")
            .args(["stop", name])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["disable", name])
            .output();
        let path = unit_path(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                CoreError::other(
                    "remove unit",
                    format!("Failed to remove {}: {}", path.display(), e),
                )
            })?;
        }
        let _ = std::process::Command::new("systemctl")
            .args(["daemon-reload"])
            .output();
        debug!("Service removed: {}", name);
        Ok(())
    }
}

// ===========================================================================
// macOS implementation (launchd)
// ===========================================================================
#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use std::path::Path;
    use tracing::{debug, info};

    fn validate_service_name(name: &str) -> Result<()> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(CoreError::other("service name", format!(
                "Invalid service name: '{}'. Must contain only alphanumeric, underscores, hyphens, or dots.", name
            )));
        }
        Ok(())
    }

    fn plist_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from("/Library/LaunchDaemons").join(format!("{}.plist", name))
    }

    pub fn install_services(services: &[ServiceEntry], install_dir: &Path) -> Result<()> {
        for svc in services {
            install_service(svc, install_dir)?;
        }
        Ok(())
    }

    fn install_service(svc: &ServiceEntry, install_dir: &Path) -> Result<()> {
        let binary_path = install_dir.join(&svc.binary_path);
        validate_service_name(&svc.name)?;
        info!(
            "Installing launchd service: {} ({})",
            svc.display_name, svc.name
        );

        let run_at_load = if svc.start_type == "auto" || svc.start_type == "delayed_auto" {
            "true"
        } else {
            "false"
        };

        let plist_content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \t<key>Label</key>\n\
             \t<string>{name}</string>\n\
             \t<key>ProgramArguments</key>\n\
             \t<array>\n\
             \t\t<string>{exe}</string>\n\
             \t</array>\n\
             \t<key>WorkingDirectory</key>\n\
             \t<string>{dir}</string>\n\
             \t<key>RunAtLoad</key>\n\
             \t<{run_at_load}/>\n\
             \t<key>KeepAlive</key>\n\
             \t<true/>\n\
             \t<key>StandardOutPath</key>\n\
             \t<string>/var/log/{name}.log</string>\n\
             \t<key>StandardErrorPath</key>\n\
             \t<string>/var/log/{name}.err.log</string>\n\
             </dict>\n\
             </plist>\n",
            name = svc.name,
            exe = binary_path.display(),
            dir = install_dir.display(),
            run_at_load = run_at_load,
        );

        let path = plist_path(&svc.name);
        std::fs::write(&path, &plist_content).map_err(|e| {
            CoreError::other(
                "write plist",
                format!("Failed to write {}: {}", path.display(), e),
            )
        })?;

        let _ = std::process::Command::new("launchctl")
            .args(["load", &path.to_string_lossy()])
            .output();
        if svc.start_on_install {
            start_service(&svc.name)?;
        }

        debug!("Service installed: {}", svc.name);
        Ok(())
    }

    pub fn start_service(name: &str) -> Result<()> {
        info!("Starting service: {}", name);
        let output = std::process::Command::new("launchctl")
            .args(["start", name])
            .output()
            .map_err(|e| CoreError::other("start service", format!("{}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("Service start returned non-zero: {}", stderr);
        }
        Ok(())
    }

    pub fn remove_services(services: &[ServiceEntry]) -> Result<()> {
        for svc in services {
            if svc.remove_on_uninstall {
                remove_service(&svc.name)?;
            }
        }
        Ok(())
    }

    fn remove_service(name: &str) -> Result<()> {
        validate_service_name(name)?;
        info!("Removing service: {}", name);
        let _ = std::process::Command::new("launchctl")
            .args(["stop", name])
            .output();
        let path = plist_path(name);
        if path.exists() {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &path.to_string_lossy()])
                .output();
            std::fs::remove_file(&path).map_err(|e| {
                CoreError::other(
                    "remove plist",
                    format!("Failed to remove {}: {}", path.display(), e),
                )
            })?;
        }
        debug!("Service removed: {}", name);
        Ok(())
    }
}

// ===========================================================================
// Cross-platform public API
// ===========================================================================

/// Install and configure services.
pub fn install_services(
    services: &[ServiceEntry],
    install_dir: &std::path::Path,
) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::install_services(services, install_dir)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::install_services(services, install_dir)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::install_services(services, install_dir)
    }
}

/// Stop and remove services.
pub fn remove_services(services: &[ServiceEntry]) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::remove_services(services)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::remove_services(services)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::remove_services(services)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_service_name_validation_logic() {
        let valid = "my-service_v1";
        assert!(valid
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.'));
        let invalid = "my service";
        assert!(!invalid
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.'));
    }
}
