//! High-level wizard orchestrator — selects and runs the appropriate UI theme.

use crate::classic;
use crate::error::{UiError, Result};
use std::path::PathBuf;
use velocity_config::VelocityManifest;

/// Result from running the installer wizard.
#[derive(Debug, Clone)]
pub struct InstallWizardResult {
    pub install_dir: PathBuf,
    pub cancelled: bool,
    pub launch_after: bool,
}

/// Run the installer wizard based on the manifest's UI configuration.
pub fn run_install_wizard(manifest: &VelocityManifest) -> Result<InstallWizardResult> {
    match manifest.ui.theme.as_str() {
        "classic" => run_classic(manifest),
        "modern" => {
            // For MVP, fall back to classic. Modern UI comes in Phase 2.
            tracing::info!("Modern UI not yet implemented, falling back to classic wizard");
            run_classic(manifest)
        }
        _ => Err(UiError::Other(format!(
            "Unknown theme: {}",
            manifest.ui.theme
        ))),
    }
}

/// Run the classic wizard and map to InstallWizardResult.
fn run_classic(manifest: &VelocityManifest) -> Result<InstallWizardResult> {
    let result = classic::run_classic_wizard(manifest)?;

    if result.cancelled {
        return Err(UiError::Cancelled);
    }

    Ok(InstallWizardResult {
        install_dir: result.install_dir,
        cancelled: false,
        launch_after: result.launch_after,
    })
}

/// Show installation progress (simple console-based for MVP).
pub fn show_progress(current: usize, total: usize, file_name: &str) {
    let pct = if total > 0 {
        (current * 100) / total
    } else {
        0
    };
    print!("\r[{}/{}] {}% - {}", current, total, pct, file_name);
    if current == total {
        println!();
    }
}

/// Show installation complete message.
pub fn show_complete(app_name: &str, install_dir: &PathBuf) {
    classic::show_message(
        &format!("{} Installed", app_name),
        &format!(
            "{} has been successfully installed to:\n\n{}\n\nClick OK to finish.",
            app_name,
            install_dir.display()
        ),
    );
}

/// Show an error message.
pub fn show_error(title: &str, message: &str) {
    classic::show_error(title, message);
}

/// Show the finish dialog with option to launch the app.
pub fn show_finish(app_name: &str, install_dir: &PathBuf, run_after: Option<&str>) -> bool {
    classic::show_finish_dialog(app_name, install_dir, run_after)
}
