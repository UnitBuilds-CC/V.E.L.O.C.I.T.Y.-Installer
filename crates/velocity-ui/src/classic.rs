//! Classic Win32 wizard UI for the installer.
//!
//! Implements a full multi-page wizard:
//! - Welcome page
//! - License agreement page
//! - Installation directory page
//! - Installation progress page
//! - Completion page

use crate::error::{UiError, Result};
use std::path::{Path, PathBuf};
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

    // Check if we have a license file to display
    let has_license = manifest.app.license.is_some();

    // Run the wizard using a simple sequential dialog approach
    let result = run_wizard_dialogs(manifest, &default_dir, has_license)?;

    Ok(result)
}

/// Run wizard as a sequence of native dialogs.
fn run_wizard_dialogs(
    manifest: &VelocityManifest,
    default_dir: &str,
    has_license: bool,
) -> Result<WizardResult> {
    // Page 1: Welcome
    let welcome_result = show_welcome_dialog(&manifest.app.name, &manifest.app.publisher);
    match welcome_result {
        DialogAction::Next => {}
        DialogAction::Cancel => {
            return Ok(WizardResult {
                install_dir: PathBuf::from(default_dir),
                license_accepted: false,
                launch_after: false,
                cancelled: true,
            });
        }
    }

    // Page 2: License (if applicable)
    if has_license {
        if let Some(license_path) = &manifest.app.license {
            let license_text = std::fs::read_to_string(license_path).unwrap_or_default();
            let license_result = show_license_dialog(&manifest.app.name, &license_text);
            match license_result {
                DialogAction::Next => {} // License accepted
                _ => {
                    return Ok(WizardResult {
                        install_dir: PathBuf::from(default_dir),
                        license_accepted: false,
                        launch_after: false,
                        cancelled: true,
                    });
                }
            }
        }
    }

    // Page 3: Directory selection
    let selected_dir = show_directory_dialog(&manifest.app.name, default_dir)?;

    // Return the result
    Ok(WizardResult {
        install_dir: PathBuf::from(&selected_dir),
        license_accepted: true,
        launch_after: true,
        cancelled: false,
    })
}

/// Dialog action result.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DialogAction {
    Next,
    Cancel,
}

/// Show the Welcome dialog.
fn show_welcome_dialog(app_name: &str, publisher: &str) -> DialogAction {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let message = format!(
        "Welcome to {} Setup!\r\n\r\n\
         This wizard will guide you through the installation of {}.\r\n\r\n\
         {}\r\n\r\n\
         It is recommended that you close all other applications before continuing.\r\n\r\n\
         Click Next to continue, or Cancel to exit Setup.",
        app_name, app_name,
        if publisher.is_empty() { String::new() } else { format!("Publisher: {}", publisher) }
    );

    unsafe {
        let title_w: Vec<u16> = format!("{} Setup", app_name).encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let result = MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OKCANCEL | MB_ICONINFORMATION,
        );
        if result == IDOK {
            DialogAction::Next
        } else {
            DialogAction::Cancel
        }
    }
}

/// Show the License Agreement dialog.
fn show_license_dialog(app_name: &str, license_text: &str) -> DialogAction {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let message = format!(
        "{} License Agreement\r\n\r\n\
         Please review the license terms before installing {}.\r\n\r\n\
         {}\r\n\r\n\
         Press Page Down to see the rest of the agreement.\r\n\r\n\
         Do you accept all the terms of the preceding license agreement?",
        app_name, app_name,
        if license_text.len() > 2000 {
            format!("{}...", &license_text[..2000])
        } else {
            license_text.to_string()
        }
    );

    unsafe {
        let title_w: Vec<u16> = format!("{} License Agreement", app_name).encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let result = MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        );
        if result == IDYES {
            DialogAction::Next
        } else {
            DialogAction::Cancel
        }
    }
}

/// Show a directory selection dialog using the Windows folder browser.
fn show_directory_dialog(app_name: &str, default_dir: &str) -> Result<String> {
    use windows::core::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;

    unsafe {
        let _ = CoInitialize(None).ok();

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
                debug!("Folder dialog cancelled or failed, using default: {}", default_dir);
                Ok(default_dir.to_string())
            }
        }
    }
}

/// Show installation progress dialog.
pub fn show_install_progress(app_name: &str) -> ProgressHandle {
    // For the MVP, progress is shown via console output
    // Phase 2 will add a proper Win32 progress dialog
    ProgressHandle {
        app_name: app_name.to_string(),
    }
}

/// Handle for updating installation progress.
pub struct ProgressHandle {
    app_name: String,
}

impl ProgressHandle {
    /// Update the progress (0-100).
    pub fn set_progress(&self, percent: u32, file_name: &str) {
        print!("\r[{:3}%] {} - {}", percent, self.app_name, file_name);
        if percent >= 100 {
            println!();
        }
    }

    /// Mark installation as complete.
    pub fn complete(&self) {
        println!("{} installation complete!", self.app_name);
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

/// Show an error message box.
pub fn show_error(title: &str, message: &str) {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR,
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

/// Show the Finish dialog with option to launch the app.
pub fn show_finish_dialog(app_name: &str, install_dir: &Path, _run_after: Option<&str>) -> bool {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let message = format!(
        "{} has been successfully installed!\r\n\r\n\
         Installation directory:\r\n{}\r\n\r\n\
         Click OK to finish the setup.",
        app_name,
        install_dir.display()
    );

    unsafe {
        let title_w: Vec<u16> = format!("{} Setup Complete", app_name).encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }

    true // User clicked OK
}
