//! PE icon resource modification.
//!
//! Allows setting a custom icon on the installer .exe file.
//! Uses rcedit.exe (from Electron) or falls back to a basic
//! resource update approach.

use crate::error::{CoreError, Result};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

/// Set the icon of a PE executable.
///
/// This modifies the .exe in-place to use the specified .ico file.
/// Requires either rcedit.exe in PATH or the Windows SDK.
pub fn set_exe_icon(exe_path: &Path, icon_path: &Path) -> Result<()> {
    if !exe_path.exists() {
        return Err(CoreError::Other(format!(
            "Executable not found: {}",
            exe_path.display()
        )));
    }
    if !icon_path.exists() {
        return Err(CoreError::Other(format!(
            "Icon file not found: {}",
            icon_path.display()
        )));
    }

    info!(
        "Setting icon: {} -> {}",
        exe_path.display(),
        icon_path.display()
    );

    // Try rcedit first (most reliable)
    if let Some(rcedit) = find_rcedit() {
        return set_icon_rcedit(&rcedit, exe_path, icon_path);
    }

    // Try ResourceHacker as fallback
    if let Some(reshack) = find_resource_hacker() {
        return set_icon_reshack(&reshack, exe_path, icon_path);
    }

    Err(CoreError::Other(
        "No icon editing tool found. Install rcedit.exe and add it to PATH, \
         or install Resource Hacker. Download rcedit from: \
         https://github.com/electron/rcedit/releases"
            .to_string(),
    ))
}

/// Set icon using rcedit.exe.
fn set_icon_rcedit(rcedit: &str, exe_path: &Path, icon_path: &Path) -> Result<()> {
    debug!("Using rcedit: {}", rcedit);

    let output = Command::new(rcedit)
        .arg(exe_path)
        .arg("--set-icon")
        .arg(icon_path)
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run rcedit: {}", e)))?;

    if output.status.success() {
        info!("Icon set successfully via rcedit");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(CoreError::Other(format!("rcedit failed: {}", stderr)))
    }
}

/// Set icon using Resource Hacker.
fn set_icon_reshack(reshack: &str, exe_path: &Path, icon_path: &Path) -> Result<()> {
    debug!("Using Resource Hacker: {}", reshack);

    // Resource Hacker command line:
    // -open exe -save exe -action addoverwrite -res ico -mask ICONGROUP,1,
    let output = Command::new(reshack)
        .args([
            "-open",
            &exe_path.to_string_lossy(),
            "-save",
            &exe_path.to_string_lossy(),
            "-action",
            "addoverwrite",
            "-res",
            &icon_path.to_string_lossy(),
            "-mask",
            "ICONGROUP,1,",
        ])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run Resource Hacker: {}", e)))?;

    if output.status.success() {
        info!("Icon set successfully via Resource Hacker");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(CoreError::Other(format!(
            "Resource Hacker failed: {}",
            stderr
        )))
    }
}

/// Find rcedit.exe in PATH or common locations.
fn find_rcedit() -> Option<String> {
    // Check PATH
    if let Ok(output) = Command::new("where").arg("rcedit.exe").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            if let Some(p) = path.lines().next() {
                return Some(p.trim().to_string());
            }
        }
    }

    // Check common locations
    let candidates = ["C:\\tools\\rcedit-x64.exe", "C:\\tools\\rcedit.exe"];
    for path in &candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    None
}

/// Find Resource Hacker in common locations.
fn find_resource_hacker() -> Option<String> {
    let candidates = [
        "C:\\Program Files (x86)\\Resource Hacker\\ResourceHacker.exe",
        "C:\\Program Files\\Resource Hacker\\ResourceHacker.exe",
    ];
    for path in &candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

/// Set version information on a PE executable.
pub fn set_exe_version_info(
    exe_path: &Path,
    version: &str,
    company: Option<&str>,
    description: Option<&str>,
    copyright: Option<&str>,
) -> Result<()> {
    let rcedit = match find_rcedit() {
        Some(r) => r,
        None => {
            debug!("rcedit not found, skipping version info");
            return Ok(());
        }
    };

    let mut cmd = Command::new(&rcedit);
    cmd.arg(exe_path);

    cmd.arg("--set-file-version").arg(version);
    cmd.arg("--set-product-version").arg(version);

    if let Some(c) = company {
        cmd.arg("--set-version-string").arg("CompanyName").arg(c);
    }
    if let Some(d) = description {
        cmd.arg("--set-version-string")
            .arg("FileDescription")
            .arg(d);
    }
    if let Some(c) = copyright {
        cmd.arg("--set-version-string").arg("LegalCopyright").arg(c);
    }

    let output = cmd
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run rcedit: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!("rcedit version info warning: {}", stderr);
    }

    Ok(())
}
