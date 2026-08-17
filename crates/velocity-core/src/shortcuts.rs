//! Windows shortcut (.lnk) creation via COM IShellLink.

use crate::error::{CoreError, Result};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use velocity_config::{ShortcutConfig, CustomShortcut};

/// Create shortcuts based on the manifest configuration.
pub fn create_shortcuts(
    config: &ShortcutConfig,
    app_name: &str,
    target_exe: &Path,
    install_dir: &Path,
    start_menu_folder: Option<&str>,
) -> Result<()> {
    if config.desktop {
        let desktop = get_known_folder_path(KnownFolder::Desktop)?;
        let lnk_path = desktop.join(format!("{}.lnk", app_name));
        create_lnk(
            &lnk_path,
            target_exe,
            install_dir,
            Some(app_name),
            None,
            None,
        )?;
        info!("Created desktop shortcut: {}", lnk_path.display());
    }

    if config.start_menu {
        if let Some(folder) = start_menu_folder {
            let programs = get_known_folder_path(KnownFolder::Programs)?;
            let menu_dir = programs.join(folder);
            std::fs::create_dir_all(&menu_dir)?;

            let lnk_path = menu_dir.join(format!("{}.lnk", app_name));
            create_lnk(
                &lnk_path,
                target_exe,
                install_dir,
                Some(app_name),
                None,
                None,
            )?;
            info!("Created Start Menu shortcut: {}", lnk_path.display());
        }
    }

    // Custom shortcuts
    for custom in &config.custom {
        create_custom_shortcut(custom, install_dir)?;
    }

    Ok(())
}

/// Create a custom shortcut.
fn create_custom_shortcut(custom: &CustomShortcut, install_dir: &Path) -> Result<()> {
    let target = install_dir.join(&custom.target);
    let working_dir = custom
        .working_dir
        .as_ref()
        .map(|d| install_dir.join(d))
        .unwrap_or_else(|| install_dir.to_path_buf());

    let location_dir = match custom.location.as_str() {
        "desktop" => get_known_folder_path(KnownFolder::Desktop)?,
        "start_menu" => get_known_folder_path(KnownFolder::Programs)?,
        _ => Path::new(&custom.location).to_path_buf(),
    };

    std::fs::create_dir_all(&location_dir)?;
    let lnk_path = location_dir.join(format!("{}.lnk", custom.name));

    create_lnk(
        &lnk_path,
        &target,
        &working_dir,
        Some(&custom.name),
        custom.arguments.as_deref(),
        custom.icon.as_deref().map(Path::new),
    )?;

    info!("Created custom shortcut: {}", lnk_path.display());
    Ok(())
}

/// Create a .lnk shortcut file using COM IShellLink.
pub fn create_lnk(
    lnk_path: &Path,
    target: &Path,
    working_dir: &Path,
    description: Option<&str>,
    arguments: Option<&str>,
    icon_path: Option<&Path>,
) -> Result<()> {
    use windows::core::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;

    unsafe {
        let _ = CoInitialize(None).ok();

        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| CoreError::com("create IShellLink", format!("{}", e)))?;

        // Set the target path
        let target_wide: Vec<u16> = target
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        shell_link
            .SetPath(PCWSTR(target_wide.as_ptr()))
            .map_err(|e| CoreError::com("set shortcut path", format!("{}", e)))?;

        // Set working directory
        let dir_wide: Vec<u16> = working_dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        shell_link
            .SetWorkingDirectory(PCWSTR(dir_wide.as_ptr()))
            .map_err(|e| CoreError::com("set working directory", format!("{}", e)))?;

        // Set description
        if let Some(desc) = description {
            let desc_wide: Vec<u16> = desc.encode_utf16().chain(std::iter::once(0)).collect();
            shell_link
                .SetDescription(PCWSTR(desc_wide.as_ptr()))
                .map_err(|e| CoreError::com("set description", format!("{}", e)))?;
        }

        // Set arguments
        if let Some(args) = arguments {
            let args_wide: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
            shell_link
                .SetArguments(PCWSTR(args_wide.as_ptr()))
                .map_err(|e| CoreError::com("set arguments", format!("{}", e)))?;
        }

        // Set icon
        if let Some(icon) = icon_path {
            let icon_wide: Vec<u16> = icon
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            shell_link
                .SetIconLocation(PCWSTR(icon_wide.as_ptr()), 0)
                .map_err(|e| CoreError::com("set icon", format!("{}", e)))?;
        }

        // Save the shortcut via IPersistFile
        let persist: IPersistFile = shell_link
            .cast()
            .map_err(|e| CoreError::com("get IPersistFile", format!("{}", e)))?;

        let lnk_wide: Vec<u16> = lnk_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        persist
            .Save(PCWSTR(lnk_wide.as_ptr()), true)
            .map_err(|e| CoreError::com("save shortcut", format!("{}", e)))?;

        CoUninitialize();
    }

    debug!("Shortcut created: {} -> {}", lnk_path.display(), target.display());
    Ok(())
}

/// Remove shortcuts created during installation.
pub fn remove_shortcuts(
    config: &ShortcutConfig,
    app_name: &str,
    start_menu_folder: Option<&str>,
) -> Result<()> {
    if config.desktop {
        let desktop = get_known_folder_path(KnownFolder::Desktop)?;
        let lnk_path = desktop.join(format!("{}.lnk", app_name));
        if lnk_path.exists() {
            std::fs::remove_file(&lnk_path)?;
            debug!("Removed desktop shortcut: {}", lnk_path.display());
        }
    }

    if config.start_menu {
        if let Some(folder) = start_menu_folder {
            let programs = get_known_folder_path(KnownFolder::Programs)?;
            let menu_dir = programs.join(folder);
            let lnk_path = menu_dir.join(format!("{}.lnk", app_name));
            if lnk_path.exists() {
                std::fs::remove_file(&lnk_path)?;
                debug!("Removed Start Menu shortcut: {}", lnk_path.display());
            }

            // Try to remove the folder if empty
            std::fs::remove_dir(&menu_dir).ok();
        }
    }

    Ok(())
}

/// Known folder identifiers.
enum KnownFolder {
    Desktop,
    Programs,
}

/// Get a known folder path using SHGetKnownFolderPath.
fn get_known_folder_path(folder: KnownFolder) -> Result<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::*;

    let folder_id = match folder {
        KnownFolder::Desktop => &FOLDERID_Desktop,
        KnownFolder::Programs => &FOLDERID_Programs,
    };

    unsafe {
        let path_ptr = SHGetKnownFolderPath(folder_id, KNOWN_FOLDER_FLAG(0), None)
            .map_err(|e| CoreError::com("SHGetKnownFolderPath", format!("{}", e)))?;

        // Convert PWSTR to String
        let mut len = 0usize;
        let mut ptr = path_ptr.0;
        while *ptr != 0 {
            len += 1;
            ptr = ptr.add(1);
        }
        let slice = std::slice::from_raw_parts(path_ptr.0, len);
        let path_str = String::from_utf16(slice)
            .map_err(|e| CoreError::com("UTF-16 conversion", format!("Invalid UTF-16 path: {}", e)))?;
        let path = PathBuf::from(&path_str);
        CoTaskMemFree(Some(path_ptr.0 as *mut _));
        Ok(path)
    }
}
