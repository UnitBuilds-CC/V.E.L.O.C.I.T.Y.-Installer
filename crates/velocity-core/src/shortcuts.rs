//! Cross-platform shortcut creation.
//!
//! - Windows: COM IShellLink (.lnk files)
//! - Linux: freedesktop .desktop files
//! - macOS: symlinks in ~/Applications and desktop

use std::path::{Path, PathBuf};
use velocity_config::{CustomShortcut, ShortcutConfig};

// ===========================================================================
// Windows implementation (COM IShellLink)
// ===========================================================================
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use std::os::windows::ffi::OsStrExt;
    use tracing::{debug, info};
    use windows::core::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;

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

        for custom in &config.custom {
            create_custom_shortcut(custom, install_dir)?;
        }

        Ok(())
    }

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

    fn create_lnk(
        lnk_path: &Path,
        target: &Path,
        working_dir: &Path,
        description: Option<&str>,
        arguments: Option<&str>,
        icon_path: Option<&Path>,
    ) -> Result<()> {
        struct ComGuard;
        impl ComGuard {
            fn new() -> Self {
                unsafe {
                    let _ = CoInitialize(None).ok();
                }
                ComGuard
            }
        }
        impl Drop for ComGuard {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _com_guard = ComGuard::new();

        // SAFETY: COM smart pointers manage refcount. PCWSTR from Vec<u16> outlives calls.
        unsafe {
            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| CoreError::com("create IShellLink", format!("{}", e)))?;
            let target_w: Vec<u16> = target
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            shell_link
                .SetPath(PCWSTR(target_w.as_ptr()))
                .map_err(|e| CoreError::com("set path", format!("{}", e)))?;
            let dir_w: Vec<u16> = working_dir
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            shell_link
                .SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))
                .map_err(|e| CoreError::com("set workdir", format!("{}", e)))?;
            if let Some(desc) = description {
                let d: Vec<u16> = desc.encode_utf16().chain(std::iter::once(0)).collect();
                shell_link
                    .SetDescription(PCWSTR(d.as_ptr()))
                    .map_err(|e| CoreError::com("set desc", format!("{}", e)))?;
            }
            if let Some(args) = arguments {
                let a: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
                shell_link
                    .SetArguments(PCWSTR(a.as_ptr()))
                    .map_err(|e| CoreError::com("set args", format!("{}", e)))?;
            }
            if let Some(icon) = icon_path {
                let i: Vec<u16> = icon
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                shell_link
                    .SetIconLocation(PCWSTR(i.as_ptr()), 0)
                    .map_err(|e| CoreError::com("set icon", format!("{}", e)))?;
            }
            let persist: IPersistFile = shell_link
                .cast()
                .map_err(|e| CoreError::com("IPersistFile", format!("{}", e)))?;
            let lnk_w: Vec<u16> = lnk_path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            persist
                .Save(PCWSTR(lnk_w.as_ptr()), true)
                .map_err(|e| CoreError::com("save", format!("{}", e)))?;
        }
        debug!("Shortcut: {} -> {}", lnk_path.display(), target.display());
        Ok(())
    }

    /// Remove shortcuts.
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
                debug!("Removed: {}", lnk_path.display());
            }
        }
        if config.start_menu {
            if let Some(folder) = start_menu_folder {
                let programs = get_known_folder_path(KnownFolder::Programs)?;
                let menu_dir = programs.join(folder);
                let lnk_path = menu_dir.join(format!("{}.lnk", app_name));
                if lnk_path.exists() {
                    std::fs::remove_file(&lnk_path)?;
                    debug!("Removed: {}", lnk_path.display());
                }
                std::fs::remove_dir(&menu_dir).ok();
            }
        }
        Ok(())
    }

    enum KnownFolder {
        Desktop,
        Programs,
    }

    fn get_known_folder_path(folder: KnownFolder) -> Result<PathBuf> {
        use windows::Win32::System::Com::CoTaskMemFree;
        use windows::Win32::UI::Shell::*;
        let folder_id = match folder {
            KnownFolder::Desktop => &FOLDERID_Desktop,
            KnownFolder::Programs => &FOLDERID_Programs,
        };
        // SAFETY: SHGetKnownFolderPath allocates via COM; freed by CoTaskMemFree.
        unsafe {
            let path_ptr = SHGetKnownFolderPath(folder_id, KNOWN_FOLDER_FLAG(0), None)
                .map_err(|e| CoreError::com("SHGetKnownFolderPath", format!("{}", e)))?;
            let mut len = 0usize;
            let mut ptr = path_ptr.0;
            while *ptr != 0 {
                len += 1;
                ptr = ptr.add(1);
            }
            let slice = std::slice::from_raw_parts(path_ptr.0, len);
            let path_str = String::from_utf16(slice)
                .map_err(|e| CoreError::com("UTF-16", format!("{}", e)))?;
            let path = PathBuf::from(&path_str);
            CoTaskMemFree(Some(path_ptr.0 as *mut _));
            Ok(path)
        }
    }
}

// ===========================================================================
// Linux implementation (freedesktop .desktop files)
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use tracing::{debug, info};

    /// Create shortcuts based on the manifest configuration.
    pub fn create_shortcuts(
        config: &ShortcutConfig,
        app_name: &str,
        target_exe: &Path,
        install_dir: &Path,
        start_menu_folder: Option<&str>,
    ) -> Result<()> {
        if config.desktop {
            let desktop = crate::platform::desktop_dir();
            let desktop_path = desktop.join(format!(
                "{}.desktop",
                app_name.to_lowercase().replace(' ', "-")
            ));
            create_desktop_file(&desktop_path, app_name, target_exe, install_dir, None)?;
            info!("Created desktop shortcut: {}", desktop_path.display());
        }

        if config.start_menu {
            let apps_dir = crate::platform::start_menu_dir();
            let folder_name = start_menu_folder.unwrap_or(app_name);
            let menu_dir = apps_dir.join(folder_name.to_lowercase().replace(' ', "-"));
            std::fs::create_dir_all(&menu_dir)?;
            let desktop_path = menu_dir.join(format!(
                "{}.desktop",
                app_name.to_lowercase().replace(' ', "-")
            ));
            create_desktop_file(&desktop_path, app_name, target_exe, install_dir, None)?;
            info!(
                "Created application menu shortcut: {}",
                desktop_path.display()
            );
        }

        for custom in &config.custom {
            create_custom_shortcut(custom, install_dir)?;
        }

        Ok(())
    }

    fn create_custom_shortcut(custom: &CustomShortcut, install_dir: &Path) -> Result<()> {
        let target = install_dir.join(&custom.target);
        let location_dir = match custom.location.as_str() {
            "desktop" => crate::platform::desktop_dir(),
            "start_menu" => crate::platform::start_menu_dir(),
            _ => Path::new(&custom.location).to_path_buf(),
        };
        std::fs::create_dir_all(&location_dir)?;
        let desktop_path = location_dir.join(format!(
            "{}.desktop",
            custom.name.to_lowercase().replace(' ', "-")
        ));
        create_desktop_file(
            &desktop_path,
            &custom.name,
            &target,
            install_dir,
            custom.icon.as_deref().map(Path::new),
        )?;
        info!("Created custom shortcut: {}", desktop_path.display());
        Ok(())
    }

    /// Create a freedesktop .desktop file.
    fn create_desktop_file(
        path: &Path,
        name: &str,
        exe: &Path,
        workdir: &Path,
        icon: Option<&Path>,
    ) -> Result<()> {
        let icon_line = icon
            .map(|i| format!("Icon={}", i.display()))
            .unwrap_or_default();
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={}\n\
             Exec={} %U\n\
             Path={}\n\
             {}\n\
             Terminal=false\n\
             Categories=Utility;\n\
             Comment=Installed by Velocity\n",
            name,
            exe.display(),
            workdir.display(),
            icon_line,
        );
        std::fs::write(path, content)?;
        // Make executable (required by some desktop environments)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
        }
        debug!("Desktop file: {} -> {}", path.display(), exe.display());
        Ok(())
    }

    /// Remove shortcuts.
    pub fn remove_shortcuts(
        config: &ShortcutConfig,
        app_name: &str,
        start_menu_folder: Option<&str>,
    ) -> Result<()> {
        let slug = app_name.to_lowercase().replace(' ', "-");
        if config.desktop {
            let desktop = crate::platform::desktop_dir();
            let path = desktop.join(format!("{}.desktop", slug));
            if path.exists() {
                std::fs::remove_file(&path)?;
                debug!("Removed: {}", path.display());
            }
        }
        if config.start_menu {
            let apps_dir = crate::platform::start_menu_dir();
            let folder = start_menu_folder
                .unwrap_or(app_name)
                .to_lowercase()
                .replace(' ', "-");
            let menu_dir = apps_dir.join(folder);
            let path = menu_dir.join(format!("{}.desktop", slug));
            if path.exists() {
                std::fs::remove_file(&path)?;
                debug!("Removed: {}", path.display());
            }
            std::fs::remove_dir(&menu_dir).ok();
        }
        Ok(())
    }
}

// ===========================================================================
// macOS implementation (symlinks + aliases)
// ===========================================================================
#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use crate::error::{CoreError, Result};
    use tracing::{debug, info};

    /// Create shortcuts based on the manifest configuration.
    pub fn create_shortcuts(
        config: &ShortcutConfig,
        app_name: &str,
        target_exe: &Path,
        install_dir: &Path,
        start_menu_folder: Option<&str>,
    ) -> Result<()> {
        if config.desktop {
            let desktop = crate::platform::desktop_dir();
            let link_path = desktop.join(app_name);
            create_symlink(&link_path, target_exe)?;
            info!("Created desktop shortcut: {}", link_path.display());
        }

        if config.start_menu {
            let apps_dir = crate::platform::start_menu_dir();
            let folder_name = start_menu_folder.unwrap_or(app_name);
            let menu_dir = apps_dir.join(folder_name);
            std::fs::create_dir_all(&menu_dir)?;
            let link_path = menu_dir.join(app_name);
            create_symlink(&link_path, target_exe)?;
            info!("Created Applications shortcut: {}", link_path.display());
        }

        for custom in &config.custom {
            create_custom_shortcut(custom, install_dir)?;
        }

        Ok(())
    }

    fn create_custom_shortcut(custom: &CustomShortcut, install_dir: &Path) -> Result<()> {
        let target = install_dir.join(&custom.target);
        let location_dir = match custom.location.as_str() {
            "desktop" => crate::platform::desktop_dir(),
            "start_menu" => crate::platform::start_menu_dir(),
            _ => Path::new(&custom.location).to_path_buf(),
        };
        std::fs::create_dir_all(&location_dir)?;
        let link_path = location_dir.join(&custom.name);
        create_symlink(&link_path, &target)?;
        info!("Created custom shortcut: {}", link_path.display());
        Ok(())
    }

    fn create_symlink(link_path: &Path, target: &Path) -> Result<()> {
        if link_path.exists() {
            std::fs::remove_file(link_path)?;
        }
        std::os::unix::fs::symlink(target, link_path).map_err(|e| {
            CoreError::other(
                "symlink",
                format!(
                    "Failed to create symlink {} -> {}: {}",
                    link_path.display(),
                    target.display(),
                    e
                ),
            )
        })?;
        debug!("Symlink: {} -> {}", link_path.display(), target.display());
        Ok(())
    }

    /// Remove shortcuts.
    pub fn remove_shortcuts(
        config: &ShortcutConfig,
        app_name: &str,
        start_menu_folder: Option<&str>,
    ) -> Result<()> {
        if config.desktop {
            let desktop = crate::platform::desktop_dir();
            let path = desktop.join(app_name);
            if path.exists() {
                std::fs::remove_file(&path)?;
                debug!("Removed: {}", path.display());
            }
        }
        if config.start_menu {
            let apps_dir = crate::platform::start_menu_dir();
            let folder = start_menu_folder.unwrap_or(app_name);
            let menu_dir = apps_dir.join(folder);
            let path = menu_dir.join(app_name);
            if path.exists() {
                std::fs::remove_file(&path)?;
                debug!("Removed: {}", path.display());
            }
            std::fs::remove_dir(&menu_dir).ok();
        }
        Ok(())
    }
}

// ===========================================================================
// Cross-platform public API
// ===========================================================================

/// Create shortcuts based on the manifest configuration.
pub fn create_shortcuts(
    config: &ShortcutConfig,
    app_name: &str,
    target_exe: &Path,
    install_dir: &Path,
    start_menu_folder: Option<&str>,
) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::create_shortcuts(config, app_name, target_exe, install_dir, start_menu_folder)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::create_shortcuts(config, app_name, target_exe, install_dir, start_menu_folder)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::create_shortcuts(config, app_name, target_exe, install_dir, start_menu_folder)
    }
}

/// Remove shortcuts created during installation.
pub fn remove_shortcuts(
    config: &ShortcutConfig,
    app_name: &str,
    start_menu_folder: Option<&str>,
) -> crate::error::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::remove_shortcuts(config, app_name, start_menu_folder)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::remove_shortcuts(config, app_name, start_menu_folder)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::remove_shortcuts(config, app_name, start_menu_folder)
    }
}
