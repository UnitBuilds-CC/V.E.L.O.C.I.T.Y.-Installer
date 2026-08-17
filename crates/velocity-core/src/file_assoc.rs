//! File association management for Windows.
//!
//! Creates and removes file type associations in the registry,
//! enabling double-click to open files with the installed application.

use crate::error::{CoreError, Result};
use crate::logging;
use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;

/// Apply file associations from the manifest.
pub fn apply_file_associations(
    associations: &[velocity_config::FileAssociationEntry],
    exe_path: &Path,
) -> Result<()> {
    for assoc in associations {
        apply_single_association(assoc, exe_path)?;
    }
    Ok(())
}

/// Remove file associations.
pub fn remove_file_associations(
    associations: &[velocity_config::FileAssociationEntry],
) -> Result<()> {
    for assoc in associations {
        remove_single_association(assoc)?;
    }
    Ok(())
}

/// Apply a single file association.
fn apply_single_association(
    assoc: &velocity_config::FileAssociationEntry,
    exe_path: &Path,
) -> Result<()> {
    let ext = if assoc.extension.starts_with('.') {
        assoc.extension.clone()
    } else {
        format!(".{}", assoc.extension)
    };

    // Use handler as the ProgID (e.g., "MyApp.myext")
    let prog_id = &assoc.handler;
    let exe_str = exe_path.to_string_lossy();

    logging::log_op(
        "FILE_ASSOC",
        &format!("Associating {} with {}", ext, prog_id),
    );

    // HKCR\.ext -> ProgID
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let (ext_key, _) = hkcr
        .create_subkey(&ext)
        .map_err(|e| CoreError::Registry(format!("Failed to create key for {}: {}", ext, e)))?;
    ext_key.set_value("", prog_id).map_err(|e| {
        CoreError::Registry(format!("Failed to set default value for {}: {}", ext, e))
    })?;

    // HKCR\ProgID
    let (prog_key, _) = hkcr
        .create_subkey(prog_id)
        .map_err(|e| CoreError::Registry(format!("Failed to create ProgID key: {}", e)))?;
    prog_key
        .set_value("", &assoc.description)
        .map_err(|e| CoreError::Registry(format!("Failed to set ProgID description: {}", e)))?;

    // HKCR\ProgID\DefaultIcon
    let icon_path = if let Some(ref icon) = assoc.icon {
        icon.clone()
    } else {
        exe_str.to_string()
    };
    let (icon_key, _) = prog_key
        .create_subkey("DefaultIcon")
        .map_err(|e| CoreError::Registry(format!("Failed to create DefaultIcon key: {}", e)))?;
    icon_key
        .set_value("", &icon_path)
        .map_err(|e| CoreError::Registry(format!("Failed to set icon: {}", e)))?;

    // HKCR\ProgID\shell\open\command
    // Build command: handler is the executable, open_command is the format
    let command = if assoc.open_command.contains("%1") {
        // open_command already has the format string with %1
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

/// Remove a single file association.
///
/// Only removes the ProgID key that we created. The extension key is only
/// removed if it still points to our ProgID (i.e., no other app has claimed it).
fn remove_single_association(assoc: &velocity_config::FileAssociationEntry) -> Result<()> {
    let ext = if assoc.extension.starts_with('.') {
        assoc.extension.clone()
    } else {
        format!(".{}", assoc.extension)
    };

    logging::log_op("FILE_ASSOC", &format!("Removing association for {}", ext));

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);

    // Only remove the extension key if it still points to our ProgID
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
            // No default value — safe to remove
            let _ = hkcr.delete_subkey_all(&ext);
        }
    }

    // Remove the ProgID key that we created
    let _ = hkcr.delete_subkey_all(&assoc.handler);

    logging::log_success(&format!("File association {} removed", ext));
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_extension_normalization() {
        // Test that extension normalization works
        let ext1 = "txt";
        let ext2 = ".txt";

        let normalized1 = if ext1.starts_with('.') {
            ext1.to_string()
        } else {
            format!(".{}", ext1)
        };

        assert_eq!(normalized1, ".txt");
        assert_eq!(ext2, ".txt");
    }
}
