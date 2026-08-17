//! Classic Win32 wizard UI for the installer.

use crate::error::{UiError, Result};
use std::path::PathBuf;
use tracing::{debug, info};
use velocity_config::VelocityManifest;

/// Pages in the classic wizard flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardPage {
    Welcome,
    License,
    Directory,
    Installing,
    Finished,
}

/// Result from the wizard dialog.
#[derive(Debug, Clone)]
pub struct WizardResult {
    /// The chosen installation directory
    pub install_dir: PathBuf,
    /// Whether the user accepted the license
    pub license_accepted: bool,
    /// Whether to launch the app after install
    pub launch_after: bool,
    /// Whether the user cancelled
    pub cancelled: bool,
}

/// Run the classic Win32 wizard and return the user's choices.
pub fn run_classic_wizard(manifest: &VelocityManifest) -> Result<WizardResult> {
    info!("Starting classic wizard for: {}", manifest.app.name);

    let default_dir = velocity_config::VariableResolver::new(
        &PathBuf::from(format!("C:\\Program Files\\{}", manifest.app.name)),
    ).resolve(&manifest.install.default_dir);

    // Show a simple directory selection dialog
    let selected_dir = show_directory_dialog(&manifest.app.name, &default_dir)?;

    Ok(WizardResult {
        install_dir: PathBuf::from(selected_dir),
        license_accepted: true,
        launch_after: true,
        cancelled: false,
    })
}

/// Show a directory selection dialog using the Windows folder browser.
fn show_directory_dialog(app_name: &str, default_dir: &str) -> Result<String> {
    use windows::core::*;
    use windows::Win32::UI::Shell::*;
    use windows::Win32::System::Com::*;

    unsafe {
        let _ = CoInitialize(None).ok();

        // Use the modern file dialog (IFileOpenDialog) in pick-folders mode
        let dialog: IFileOpenDialog = CoCreateInstance(
            &FileOpenDialog,
            None,
            CLSCTX_INPROC_SERVER,
        ).map_err(|e| UiError::Win32(format!("Failed to create file dialog: {}", e)))?;

        dialog.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
            .map_err(|e| UiError::Win32(format!("Failed to set dialog options: {}", e)))?;

        let title: Vec<u16> = format!("Select installation directory for {}\0", app_name)
            .encode_utf16()
            .collect();
        dialog.SetTitle(PCWSTR(title.as_ptr()))
            .map_err(|e| UiError::Win32(format!("Failed to set title: {}", e)))?;

        // Show the dialog
        match dialog.Show(None) {
            Ok(()) => {
                let result = dialog.GetResult()
                    .map_err(|e| UiError::Win32(format!("Failed to get result: {}", e)))?;
                let path = result.GetDisplayName(SIGDN_FILESYSPATH)
                    .map_err(|e| UiError::Win32(format!("Failed to get path: {}", e)))?;
                let path_str = path.to_string()
                    .map_err(|e| UiError::Win32(format!("Invalid path: {}", e)))?;
                CoTaskMemFree(Some(path.0 as *mut _));
                Ok(path_str)
            }
            Err(_e) => {
                // User cancelled or error
                debug!("Folder dialog cancelled or failed, using default: {}", default_dir);
                Ok(default_dir.to_string())
            }
        }
    }
}

/// Show a simple message box.
pub fn show_message(title: &str, message: &str) {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Show a confirmation dialog. Returns true if the user clicks Yes.
pub fn show_confirm(title: &str, message: &str) -> bool {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let result = MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        );
        result == IDYES
    }
}
