//! Cross-platform file association management.
//!
//! - Windows: Registry (HKCR) for file type associations
//! - Linux: xdg-mime + mimeapps.list (freedesktop)
//! - macOS: Launch Services via `defaults` and `duti`/`lsregister`

use crate::logging;
use std::path::Path;
use velocity_config::FileAssociationEntry;

// ===========================================================================
// Windows implementation (Registry)
// ===========================================================================
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use winreg::enums::*;
    use winreg::RegKey;

    pub fn apply_file_associations(
        associations: &[FileAssociationEntry],
        exe_path: &Path,
    ) -> Result<()> {
        for assoc in associations {
            apply_single_association(assoc, exe_path)?;
        }
        Ok(())
    }

    fn apply_single_association(assoc: &FileAssociationEntry, exe_path: &Path) -> Result<()> {
        let ext = if assoc.extension.starts_with('.') {
            assoc.extension.clone()
        } else {
            format!(".{}", assoc.extension)
        };
        let prog_id = &assoc.handler;
        let exe_str = exe_path.to_string_lossy();

        logging::log_op(
            "FILE_ASSOC",
            &format!("Associating {} with {}", ext, prog_id),
        );

        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        let (ext_key, _) = hkcr
            .create_subkey(&ext)
            .map_err(|e| CoreError::Registry(format!("Failed to create key for {}: {}", ext, e)))?;
        ext_key.set_value("", prog_id).map_err(|e| {
            CoreError::Registry(format!("Failed to set default value for {}: {}", ext, e))
        })?;

        let (prog_key, _) = hkcr
            .create_subkey(prog_id)
            .map_err(|e| CoreError::Registry(format!("Failed to create ProgID key: {}", e)))?;
        prog_key
            .set_value("", &assoc.description)
            .map_err(|e| CoreError::Registry(format!("Failed to set ProgID description: {}", e)))?;

        let icon_path = assoc.icon.clone().unwrap_or_else(|| exe_str.to_string());
        let (icon_key, _) = prog_key
            .create_subkey("DefaultIcon")
            .map_err(|e| CoreError::Registry(format!("Failed to create DefaultIcon key: {}", e)))?;
        icon_key
            .set_value("", &icon_path)
            .map_err(|e| CoreError::Registry(format!("Failed to set icon: {}", e)))?;

        let command = if assoc.open_command.contains("%1") {
            let handler_path = exe_path
                .parent()
                .map(|p| p.join(&assoc.handler))
                .unwrap_or_else(|| std::path::PathBuf::from(&assoc.handler));
            assoc
                .open_command
                .replace("%1", &format!("\"{}\"", handler_path.to_string_lossy()))
        } else {
            format!("\"{}\" {}", exe_str, assoc.open_command)
        };
        let (shell_key, _) = prog_key
            .create_subkey("shell")
            .map_err(|e| CoreError::Registry(format!("Failed to create shell key: {}", e)))?;
        let (open_key, _) = shell_key
            .create_subkey("open")
            .map_err(|e| CoreError::Registry(format!("Failed to create open key: {}", e)))?;
        let (cmd_key, _) = open_key
            .create_subkey("command")
            .map_err(|e| CoreError::Registry(format!("Failed to create command key: {}", e)))?;
        cmd_key
            .set_value("", &command)
            .map_err(|e| CoreError::Registry(format!("Failed to set command: {}", e)))?;

        logging::log_success(&format!("File association {} -> {} created", ext, prog_id));
        Ok(())
    }

    pub fn remove_file_associations(associations: &[FileAssociationEntry]) -> Result<()> {
        for assoc in associations {
            remove_single_association(assoc)?;
        }
        Ok(())
    }

    fn remove_single_association(assoc: &FileAssociationEntry) -> Result<()> {
        let ext = if assoc.extension.starts_with('.') {
            assoc.extension.clone()
        } else {
            format!(".{}", assoc.extension)
        };
        logging::log_op("FILE_ASSOC", &format!("Removing association for {}", ext));

        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);

        if let Ok(ext_key) = hkcr.open_subkey(&ext) {
            if let Ok(current_prog_id) = ext_key.get_value::<String, _>("") {
                if current_prog_id == assoc.handler {
                    let _ = hkcr.delete_subkey_all(&ext);
                    logging::log_op("FILE_ASSOC", &format!("Removed extension key for {}", ext));
                } else {
                    logging::log_op(
                        "FILE_ASSOC",
                        &format!(
                            "Skipping extension key {} — now owned by {}",
                            ext, current_prog_id
                        ),
                    );
                }
            } else {
                let _ = hkcr.delete_subkey_all(&ext);
            }
        }
        let _ = hkcr.delete_subkey_all(&assoc.handler);
        logging::log_success(&format!("File association {} removed", ext));
        Ok(())
    }
}

// ===========================================================================
// Linux implementation (xdg-mime + mimeapps.list)
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use crate::error::{CoreError, Result};

    pub fn apply_file_associations(
        associations: &[FileAssociationEntry],
        exe_path: &Path,
    ) -> Result<()> {
        for assoc in associations {
            apply_single_association(assoc, exe_path)?;
        }
        Ok(())
    }

    fn apply_single_association(assoc: &FileAssociationEntry, exe_path: &Path) -> Result<()> {
        let ext = if assoc.extension.starts_with('.') {
            &assoc.extension[1..]
        } else {
            &assoc.extension
        };
        let mime_type = format!("application/x-velocity-{}", ext);
        let desktop_name = format!(
            "velocity-{}.desktop",
            assoc.handler.to_lowercase().replace(' ', "-")
        );

        logging::log_op(
            "FILE_ASSOC",
            &format!("Associating .{} with MIME {}", ext, mime_type),
        );

        // 1. Create/update the .desktop file with the MIME type
        let desktop_content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={}\n\
             Exec={} %f\n\
             MimeType={};\n\
             Terminal=false\n\
             NoDisplay=true\n",
            assoc.description,
            exe_path.display(),
            mime_type,
        );

        let apps_dir = crate::platform::start_menu_dir();
        std::fs::create_dir_all(&apps_dir)?;
        let desktop_path = apps_dir.join(&desktop_name);
        std::fs::write(&desktop_path, &desktop_content).map_err(|e| {
            CoreError::other(
                "write desktop",
                format!("Failed to write {}: {}", desktop_path.display(), e),
            )
        })?;

        // 2. Register the MIME type with xdg-mime
        let _ = std::process::Command::new("xdg-mime")
            .args(["default", &desktop_name, &mime_type])
            .output();

        // 3. Also add to mimeapps.list for the user
        let mimeapps_path = mimeapps_list_path();
        add_to_mimeapps(&mimeapps_path, ext, &mime_type, &desktop_name)?;

        logging::log_success(&format!(
            "File association .{} -> {} created",
            ext, mime_type
        ));
        Ok(())
    }

    fn add_to_mimeapps(path: &str, ext: &str, mime_type: &str, desktop_name: &str) -> Result<()> {
        let mut content = std::fs::read_to_string(path).unwrap_or_default();

        // Add to [Added Associations] section
        let entry = format!("{}={};", mime_type, desktop_name);
        if !content.contains(&entry) {
            if content.contains("[Added Associations]") {
                content = content.replace(
                    "[Added Associations]",
                    &format!("[Added Associations]\n{}", entry),
                );
            } else {
                content.push_str(&format!("\n[Added Associations]\n{}\n", entry));
            }
            std::fs::write(path, &content).map_err(|e| {
                CoreError::other("write mimeapps", format!("Failed to write {}: {}", path, e))
            })?;
        }
        Ok(())
    }

    pub fn remove_file_associations(associations: &[FileAssociationEntry]) -> Result<()> {
        for assoc in associations {
            remove_single_association(assoc)?;
        }
        Ok(())
    }

    fn remove_single_association(assoc: &FileAssociationEntry) -> Result<()> {
        let ext = if assoc.extension.starts_with('.') {
            &assoc.extension[1..]
        } else {
            &assoc.extension
        };
        logging::log_op("FILE_ASSOC", &format!("Removing association for .{}", ext));

        // Remove the .desktop file
        let desktop_name = format!(
            "velocity-{}.desktop",
            assoc.handler.to_lowercase().replace(' ', "-")
        );
        let apps_dir = crate::platform::start_menu_dir();
        let desktop_path = apps_dir.join(&desktop_name);
        if desktop_path.exists() {
            std::fs::remove_file(&desktop_path)?;
        }

        // Remove from mimeapps.list
        let mimeapps_path = mimeapps_list_path();
        if std::path::Path::new(&mimeapps_path).exists() {
            let content = std::fs::read_to_string(&mimeapps_path).unwrap_or_default();
            let prefix = format!("application/x-velocity-{}=", ext);
            let new_content = content
                .lines()
                .filter(|l| !l.starts_with(&prefix))
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(&mimeapps_path, new_content).ok();
        }

        logging::log_success(&format!("File association .{} removed", ext));
        Ok(())
    }

    fn mimeapps_list_path() -> String {
        let data_home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| format!("{}/.local/share", h))
                .unwrap_or_else(|_| "/tmp".to_string())
        });
        format!("{}/applications/mimeapps.list", data_home)
    }
}

// ===========================================================================
// macOS implementation (Launch Services via defaults/lsregister)
// ===========================================================================
#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use crate::error::{CoreError, Result};

    pub fn apply_file_associations(
        associations: &[FileAssociationEntry],
        exe_path: &Path,
    ) -> Result<()> {
        for assoc in associations {
            apply_single_association(assoc, exe_path)?;
        }
        Ok(())
    }

    fn apply_single_association(assoc: &FileAssociationEntry, exe_path: &Path) -> Result<()> {
        let ext = if assoc.extension.starts_with('.') {
            &assoc.extension[1..]
        } else {
            &assoc.extension
        };
        let uti = format!("com.velocity.{}", ext);

        logging::log_op(
            "FILE_ASSOC",
            &format!("Associating .{} with {}", ext, exe_path.display()),
        );

        // Use `defaults write` to register the file association via Launch Services
        // This writes to the user's preferences for the handler app
        let bundle_id = format!(
            "com.velocity.{}",
            assoc.handler.to_lowercase().replace(' ', "-")
        );

        // Register via defaults (LSHandler)
        let _ = std::process::Command::new("defaults")
            .args(["write", "com.apple.LaunchServices/com.apple.launchservices.secure",
                   "LSHandlers",
                   "-array-add",
                   &format!("{{LSHandlerContentTag='public.{}';LSHandlerContentTagClass='public.filename-extension';LSHandlerRoleAll='{}';}}", ext, bundle_id)])
            .output();

        // Rebuild Launch Services database to pick up changes
        let _ = std::process::Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
            .args(["-kill", "-r", "-domain", "local", "-domain", "user"])
            .output();

        logging::log_success(&format!(
            "File association .{} -> {} created",
            ext, bundle_id
        ));
        Ok(())
    }

    pub fn remove_file_associations(associations: &[FileAssociationEntry]) -> Result<()> {
        for assoc in associations {
            remove_single_association(assoc)?;
        }
        Ok(())
    }

    fn remove_single_association(assoc: &FileAssociationEntry) -> Result<()> {
        let ext = if assoc.extension.starts_with('.') {
            &assoc.extension[1..]
        } else {
            &assoc.extension
        };
        logging::log_op("FILE_ASSOC", &format!("Removing association for .{}", ext));

        let bundle_id = format!(
            "com.velocity.{}",
            assoc.handler.to_lowercase().replace(' ', "-")
        );

        // Remove the handler registration
        let _ = std::process::Command::new("defaults")
            .args([
                "delete",
                "com.apple.LaunchServices/com.apple.launchservices.secure",
                "LSHandlers",
            ])
            .output();

        // Rebuild database
        let _ = std::process::Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
            .args(["-kill", "-r", "-domain", "local", "-domain", "user"])
            .output();

        logging::log_success(&format!("File association .{} removed", ext));
        Ok(())
    }
}

// ===========================================================================
// Cross-platform public API
// ===========================================================================

/// Apply file associations from the manifest.
pub fn apply_file_associations(
    associations: &[FileAssociationEntry],
    exe_path: &Path,
) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::apply_file_associations(associations, exe_path)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::apply_file_associations(associations, exe_path)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::apply_file_associations(associations, exe_path)
    }
}

/// Remove file associations.
pub fn remove_file_associations(associations: &[FileAssociationEntry]) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::remove_file_associations(associations)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::remove_file_associations(associations)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::remove_file_associations(associations)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_extension_normalization() {
        let ext1 = "txt";
        let ext2 = ".txt";

        let normalize = |e: &str| {
            if e.starts_with('.') {
                e.to_string()
            } else {
                format!(".{}", e)
            }
        };
        assert_eq!(normalize(ext1), ".txt");
        assert_eq!(normalize(ext2), ".txt");
    }
}
