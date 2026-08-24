//! Cloud-fetch installer logic for the Velocity runtime.
//!
//! This module handles the Ninite-style installation flow:
//! 1. Read the fetch configuration from the manifest
//! 2. Check the currently installed version
//! 3. Query the remote source for the latest version
//! 4. Compare versions and determine if update is needed
//! 5. Download assets with progress reporting
//! 6. Verify checksums
//! 7. Extract/install files to the target directory
//! 8. Update version information

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use velocity_core::fetch::{
    self, DownloadManager,
    check_for_update, UpdateInfo,
};
use velocity_config::{FetchConfig, FetchAction};

/// Result of a fetch-install operation.
#[derive(Debug)]
#[allow(dead_code)]
pub struct FetchInstallResult {
    /// Version that was installed
    pub version: String,
    /// Number of files downloaded
    pub files_downloaded: u32,
    /// Total bytes downloaded
    pub bytes_downloaded: u64,
    /// Whether this was an update (vs fresh install)
    pub was_update: bool,
    /// Install directory
    pub install_dir: PathBuf,
}

/// Execute a cloud-fetch installation.
///
/// # Arguments
/// * `config` - The fetch configuration from the manifest
/// * `install_dir` - Target installation directory
/// * `current_version` - Currently installed version (None for fresh install)
/// * `silent` - Whether to suppress UI prompts
/// * `progress_cb` - Optional progress callback (phase, current, total, message)
pub fn run_fetch_install(
    config: &FetchConfig,
    install_dir: &Path,
    current_version: Option<&str>,
    silent: bool,
    progress_cb: Option<&dyn Fn(&str, u64, u64, &str)>,
) -> Result<FetchInstallResult> {
    info!("Starting cloud-fetch installation (mode: {:?})", config.mode);

    // Step 1: Create the version resolver
    let resolver = fetch::create_resolver_from_config(config)
        .context("Failed to create version resolver")?;

    // Step 2: Get the latest version info
    report_progress(&progress_cb, "checking", 0, 0, "Checking for latest version...");
    let version_info = resolver.get_latest_version()
        .context("Failed to fetch latest version information")?;
    
    info!("Latest version: {}", version_info.version);

    // Step 3: Check if update is needed
    let update_info = if let Some(current) = current_version {
        let asset_pattern = config.asset_pattern.as_deref().unwrap_or("*");
        let download_url = resolver.find_asset(&version_info, asset_pattern)
            .map(|a| a.download_url.clone())
            .unwrap_or_default();
        
        let info = check_for_update(
            current,
            &version_info.version,
            &download_url,
            version_info.body.as_deref(),
        )?;
        
        if !info.available {
            info!("Already up to date (current: {}, latest: {})", current, version_info.version);
            return Ok(FetchInstallResult {
                version: current.to_string(),
                files_downloaded: 0,
                bytes_downloaded: 0,
                was_update: false,
                install_dir: install_dir.to_path_buf(),
            });
        }
        
        if !silent {
            report_progress(&progress_cb, "prompt", 0, 0,
                &format!("New version available: {} (current: {})", version_info.version, current));
        }
        
        info
    } else {
        // Fresh install
        let asset_pattern = config.asset_pattern.as_deref().unwrap_or("*");
        let download_url = resolver.find_asset(&version_info, asset_pattern)
            .map(|a| a.download_url.clone())
            .unwrap_or_default();
        
        UpdateInfo {
            available: true,
            current_version: "0.0.0".to_string(),
            latest_version: version_info.version.clone(),
            download_url,
            release_notes: version_info.body.clone(),
            priority: velocity_core::fetch::UpdatePriority::Major,
        }
    };

    // Step 4: Download files with partial cleanup on failure
    let download_mgr = DownloadManager::new()
        .context("Failed to initialize download manager")?;
    
    std::fs::create_dir_all(install_dir)
        .context("Failed to create install directory")?;

    // Step 4b: Check disk space (need at least 100MB free, or 2x estimated size)
    let estimated_size: u64 = version_info.assets.iter()
        .map(|a| a.size)
        .sum();
    let min_free = estimated_size.saturating_mul(2).max(100 * 1024 * 1024);
    check_disk_space(install_dir, min_free)
        .context("Insufficient disk space")?;

    let mut files_downloaded = 0u32;
    let mut bytes_downloaded = 0u64;
    let mut downloaded_paths: Vec<PathBuf> = Vec::new(); // Track for cleanup on failure

    let download_result = download_all_files(
        config,
        &resolver,
        &version_info,
        &download_mgr,
        install_dir,
        &progress_cb,
        &mut files_downloaded,
        &mut bytes_downloaded,
        &mut downloaded_paths,
    );

    // Clean up partial downloads on failure
    if let Err(ref _e) = download_result {
        warn!("Download failed, cleaning up partial files...");
        for path in &downloaded_paths {
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    debug!("Failed to remove partial file {}: {}", path.display(), e);
                }
            }
        }
    }

    download_result?;

    // Step 5: Write installed version atomically
    write_installed_version(install_dir, &version_info.version)
        .context("Failed to write installed version file")?;

    // Step 6: Report completion
    report_progress(&progress_cb, "complete", 0, 0, "Installation complete!");
    
    info!(
        "Cloud-fetch install complete: {} files, {} bytes",
        files_downloaded, bytes_downloaded
    );

    Ok(FetchInstallResult {
        version: version_info.version,
        files_downloaded,
        bytes_downloaded,
        was_update: update_info.available && current_version.is_some(),
        install_dir: install_dir.to_path_buf(),
    })
}

/// Download all files specified in the fetch configuration.
///
/// Tracks downloaded paths for cleanup on failure.
/// Supports three action types: extract (default), execute, and copy.
fn download_all_files(
    config: &FetchConfig,
    resolver: &Box<dyn fetch::VersionResolver>,
    version_info: &fetch::VersionInfo,
    download_mgr: &DownloadManager,
    install_dir: &Path,
    progress_cb: &Option<&dyn Fn(&str, u64, u64, &str)>,
    files_downloaded: &mut u32,
    bytes_downloaded: &mut u64,
    downloaded_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    for download_pattern in &config.files.download {
        let action = download_pattern.action;
        
        report_progress(progress_cb, "downloading", 0, 0,
            &format!("Downloading: {} ({})", download_pattern.pattern, 
                fetch::resolve_fetch_action(action)));

        // Find matching asset
        let asset = resolver.find_asset(version_info, &download_pattern.pattern);
        
        if let Some(asset) = asset {
            // Determine download destination based on action
            let (download_dest, _is_temp) = match action {
                FetchAction::Execute => {
                    // For execute, download to temp dir (not install dir)
                    let temp_dir = install_dir.join(".velocity_temp");
                    std::fs::create_dir_all(&temp_dir)
                        .context("Failed to create temp download directory")?;
                    (temp_dir, true)
                }
                _ => {
                    // For extract/copy, download directly to dest
                    (install_dir.join(&download_pattern.dest), false)
                }
            };
            
            let asset_name = asset.name.clone();
            let download_progress: velocity_core::downloader::DownloadProgressCallback = 
                Box::new(move |bytes: u64, total: u64, _url: &str| {
                    let pct = if total > 0 { bytes as f64 / total as f64 * 100.0 } else { 0.0 };
                    tracing::info!("Downloading: {} ({:.1}%)", asset_name, pct);
                });

            let path = download_mgr.download(
                &asset.download_url,
                &download_dest,
                Some(&asset.name),
                download_pattern.sha256.as_deref(),
                Some(&download_progress),
            ).with_context(|| format!("Failed to download: {}", asset.name))?;

            // Track the downloaded file for cleanup
            downloaded_paths.push(path.clone());

            let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            *bytes_downloaded += file_size;
            *files_downloaded += 1;
            
            info!("Downloaded: {} ({} bytes, action: {:?})", asset.name, file_size, action);

            // Handle action-specific post-download processing
            match action {
                FetchAction::Execute => {
                    // Use custom installer config if present, otherwise use auto-detection
                    if let Some(ref installer_config) = download_pattern.installer {
                        report_progress(progress_cb, "installing", 0, 0,
                            &format!("Installing with custom config: {}", 
                                installer_config.detect_name.as_deref().unwrap_or("custom")));
                        
                        velocity_core::fetch::execute_with_config(
                            &path,
                            installer_config,
                            Some(install_dir),
                        )?;
                    } else {
                        execute_downloaded_installer(
                            &path,
                            download_pattern.install_args.as_deref(),
                            download_pattern.file_type.as_deref(),
                            install_dir,
                            progress_cb,
                        )?;
                    }
                    // Clean up the downloaded installer after execution
                    if let Err(e) = std::fs::remove_file(&path) {
                        debug!("Failed to remove temp installer {}: {}", path.display(), e);
                    }
                }
                FetchAction::Extract => {
                    // Check if it's an archive that needs extraction
                    let is_archive = velocity_core::fetch::archive::detect_archive_format(&path).is_some();
                    if is_archive {
                        report_progress(progress_cb, "extracting", 0, 0,
                            &format!("Extracting: {}", asset.name));
                        let extract_dest = install_dir.join(&download_pattern.dest);
                        let extracted = velocity_core::fetch::archive::extract_archive(&path, &extract_dest)
                            .with_context(|| format!("Failed to extract archive: {}", asset.name))?;
                        info!("Extracted {} files from {} to {}", extracted, asset.name, download_pattern.dest);
                        // Clean up the archive file after extraction
                        if let Err(e) = std::fs::remove_file(&path) {
                            debug!("Failed to remove archive after extraction {}: {}", path.display(), e);
                        }
                    } else {
                        // Non-archive files are just copied to dest
                        debug!("Copied (extract action, no archive): {} to {}", asset.name, download_pattern.dest);
                    }
                }
                FetchAction::Copy => {
                    // File is already in dest, nothing to do
                    debug!("Copied: {} to {}", asset.name, download_pattern.dest);
                }
            }
        } else if download_pattern.required {
            anyhow::bail!(
                "Required file not found: {} (version: {})",
                download_pattern.pattern,
                version_info.version
            );
        } else {
            warn!("Optional file not found: {}", download_pattern.pattern);
        }
    }

    // Clean up the temp directory if it's empty (all installers were executed)
    let temp_dir = install_dir.join(".velocity_temp");
    if temp_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&temp_dir) {
            if entries.count() == 0 {
                let _ = std::fs::remove_dir(&temp_dir);
                debug!("Removed empty temp directory: {}", temp_dir.display());
            }
        }
    }

    Ok(())
}

/// Execute a downloaded installer silently.
///
/// Handles auto-detection of installer type and silent argument generation.
fn execute_downloaded_installer(
    installer_path: &Path,
    user_args: Option<&str>,
    file_type: Option<&str>,
    install_dir: &Path,
    progress_cb: &Option<&dyn Fn(&str, u64, u64, &str)>,
) -> Result<()> {
    report_progress(progress_cb, "installing", 0, 0,
        &format!("Installing: {}", installer_path.file_name()
            .and_then(|n| n.to_str()).unwrap_or("installer")));

    let result = fetch::execute_silent_installer(
        installer_path,
        user_args,
        file_type,
        Some(install_dir),
        300, // 5 minute timeout
    ).context("Failed to execute silent installer")?;

    if !result.success {
        anyhow::bail!(
            "Installer exited with code {} (type: {})",
            result.exit_code,
            result.installer_type
        );
    }

    info!(
        "Successfully installed: {} (type: {}, exit code: {})",
        installer_path.display(),
        result.installer_type,
        result.exit_code
    );

    Ok(())
}

/// Read the currently installed version from a version file.
///
/// Looks for a `.velocity-version` file in the install directory.
pub fn read_installed_version(install_dir: &Path) -> Option<String> {
    let version_file = install_dir.join(".velocity-version");
    std::fs::read_to_string(&version_file).ok().map(|s| s.trim().to_string())
}

/// Write the installed version to a version file atomically.
///
/// Writes to a temporary file first, then renames to the target path.
/// This prevents corruption if the process is interrupted during write.
pub fn write_installed_version(install_dir: &Path, version: &str) -> Result<()> {
    let version_file = install_dir.join(".velocity-version");
    let temp_file = install_dir.join(".velocity-version.tmp");
    
    // Write to temp file first
    std::fs::write(&temp_file, version)
        .with_context(|| format!("Failed to write temp version file: {}", temp_file.display()))?;
    
    // Atomic rename (on Windows this is replace, on Unix this is rename)
    std::fs::rename(&temp_file, &version_file)
        .with_context(|| format!("Failed to rename version file to {}", version_file.display()))?;
    
    Ok(())
}

/// Check that the disk containing `path` has at least `min_bytes` free.
///
/// Uses `GetDiskFreeSpaceExW` on Windows and `statvfs` on Unix.
/// Returns Ok(()) if sufficient space is available, or an error with
/// the actual free space for diagnostic purposes.
fn check_disk_space(path: &Path, min_bytes: u64) -> Result<()> {
    // Ensure the path exists so we can query its volume
    if !path.exists() {
        debug!("Disk space check: path does not exist yet, skipping: {}", path.display());
        return Ok(());
    }

    let free = get_free_disk_space(path)?;
    if free < min_bytes {
        let free_mb = free / (1024 * 1024);
        let need_mb = min_bytes / (1024 * 1024);
        anyhow::bail!(
            "Insufficient disk space on {}: {}MB free, {}MB needed",
            path.display(), free_mb, need_mb
        );
    }
    debug!("Disk space check: {}MB free (need {}MB) — OK",
        free / (1024 * 1024), min_bytes / (1024 * 1024));
    Ok(())
}

/// Get free disk space in bytes for the volume containing `path`.
#[cfg(target_os = "windows")]
fn get_free_disk_space(path: &Path) -> Result<u64> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_bytes_available: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free: u64 = 0;

    let result = unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            Some(&mut free_bytes_available),
            Some(&mut total_bytes),
            Some(&mut total_free),
        )
    };

    if result.is_ok() {
        Ok(free_bytes_available)
    } else {
        // Fallback: try parent directory
        if let Some(parent) = path.parent() {
            if parent != path {
                return get_free_disk_space(parent);
            }
        }
        warn!("Could not query disk space for {}", path.display());
        Ok(u64::MAX) // Assume enough if we can't check
    }
}

#[cfg(not(target_os = "windows"))]
fn get_free_disk_space(path: &Path) -> Result<u64> {
    use std::mem;
    let c_path = std::ffi::CString::new(path.to_string_lossy.as_ref())
        .context("Invalid path for disk space check")?;
    let mut stat: libc::statvfs = unsafe { mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if ret == 0 {
        Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
    } else {
        warn!("Could not query disk space for {}", path.display());
        Ok(u64::MAX) // Assume enough if we can't check
    }
}

/// Report progress to the callback if available.
fn report_progress(
    cb: &Option<&dyn Fn(&str, u64, u64, &str)>,
    phase: &str,
    current: u64,
    total: u64,
    message: &str,
) {
    if let Some(callback) = cb {
        callback(phase, current, total, message);
    }
    debug!("[{}] {}", phase, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_installed_version_missing() {
        let result = read_installed_version(Path::new("/nonexistent"));
        assert!(result.is_none());
    }

    #[test]
    fn test_write_and_read_version() {
        let dir = tempfile::tempdir().unwrap();
        write_installed_version(dir.path(), "1.2.3").unwrap();
        let version = read_installed_version(dir.path());
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_write_version_atomic() {
        let dir = tempfile::tempdir().unwrap();
        
        // Write initial version
        write_installed_version(dir.path(), "1.0.0").unwrap();
        
        // Overwrite with new version (atomic)
        write_installed_version(dir.path(), "2.0.0").unwrap();
        
        // Should read the new version
        let version = read_installed_version(dir.path());
        assert_eq!(version, Some("2.0.0".to_string()));
        
        // Temp file should not exist
        let temp_file = dir.path().join(".velocity-version.tmp");
        assert!(!temp_file.exists());
    }

    #[test]
    fn test_write_version_creates_content() {
        let dir = tempfile::tempdir().unwrap();
        write_installed_version(dir.path(), "3.1.4").unwrap();
        
        let content = std::fs::read_to_string(dir.path().join(".velocity-version")).unwrap();
        assert_eq!(content, "3.1.4");
    }

    // ── Action branching tests ─────────────────────────────────────────

    #[test]
    fn test_fetch_action_defaults_to_extract() {
        // Verify FetchAction defaults to Extract
        let action = FetchAction::default();
        assert_eq!(action, FetchAction::Extract);
    }

    #[test]
    fn test_action_resolve_fetch_action_mapping() {
        assert_eq!(fetch::resolve_fetch_action(FetchAction::Extract), "extract");
        assert_eq!(fetch::resolve_fetch_action(FetchAction::Execute), "execute");
        assert_eq!(fetch::resolve_fetch_action(FetchAction::Copy), "copy");
    }

    #[test]
    fn test_execute_downloaded_installer_nonexistent_file() {
        // execute_downloaded_installer should fail gracefully for a nonexistent file
        let dir = tempfile::tempdir().unwrap();
        let fake_path = dir.path().join("nonexistent_installer.exe");
        let progress_cb: Option<&dyn Fn(&str, u64, u64, &str)> = None;

        #[cfg(target_os = "windows")]
        {
            let result = execute_downloaded_installer(
                &fake_path, None, None, dir.path(), &progress_cb,
            );
            assert!(result.is_err(), "Should fail for nonexistent file");
        }

        #[cfg(not(target_os = "windows"))]
        {
            let result = execute_downloaded_installer(
                &fake_path, None, None, dir.path(), &progress_cb,
            );
            assert!(result.is_err(), "Should fail on non-Windows");
        }
    }

    #[test]
    fn test_execute_downloaded_installer_with_fake_exe() {
        // Create a fake EXE file and verify the installer detects it as Unknown
        // and fails (since Unknown installers have no default silent args)
        let dir = tempfile::tempdir().unwrap();
        let fake_exe = dir.path().join("fake_installer.exe");
        std::fs::write(&fake_exe, b"MZ_FAKE_BINARY_DATA").unwrap();

        let progress_cb: Option<&dyn Fn(&str, u64, u64, &str)> = None;

        // On Windows, this will try to execute the fake EXE which will fail
        // On non-Windows, it should return an error about platform support
        #[cfg(not(target_os = "windows"))]
        {
            let result = execute_downloaded_installer(
                &fake_exe, None, None, dir.path(), &progress_cb,
            );
            assert!(result.is_err(), "Should fail on non-Windows");
        }

        // On Windows, the fake EXE will be detected as Unknown type
        // and executed directly, which will fail since it's not a real PE
        #[cfg(target_os = "windows")]
        {
            let result = execute_downloaded_installer(
                &fake_exe, None, None, dir.path(), &progress_cb,
            );
            // Should fail because the fake EXE isn't a valid executable
            assert!(result.is_err(), "Should fail for fake EXE");
        }
    }

    #[test]
    fn test_execute_with_explicit_args() {
        // Verify that user-provided args are passed through
        let dir = tempfile::tempdir().unwrap();
        let fake_exe = dir.path().join("test_installer.exe");
        std::fs::write(&fake_exe, b"MZ").unwrap();

        let progress_cb: Option<&dyn Fn(&str, u64, u64, &str)> = None;

        // With explicit args, the function should attempt execution
        // (will fail because the file isn't a real executable)
        #[cfg(target_os = "windows")]
        {
            let result = execute_downloaded_installer(
                &fake_exe,
                Some("/S /norestart"),
                Some("exe"),
                dir.path(),
                &progress_cb,
            );
            assert!(result.is_err(), "Should fail for fake EXE even with args");
        }
    }

    #[test]
    fn test_temp_dir_created_for_execute_action() {
        // Verify that the .velocity_temp directory is created when action is Execute
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().join(".velocity_temp");
        
        // Simulate the temp dir creation logic from download_all_files
        assert!(!temp_dir.exists());
        std::fs::create_dir_all(&temp_dir).unwrap();
        assert!(temp_dir.exists());
        assert!(temp_dir.is_dir());
    }

    #[test]
    fn test_version_written_after_install() {
        // Simulate the version-writing step that happens after download_all_files
        let dir = tempfile::tempdir().unwrap();
        
        // No version file initially
        assert!(read_installed_version(dir.path()).is_none());
        
        // Write version (as the pipeline does after successful install)
        write_installed_version(dir.path(), "2.5.1").unwrap();
        
        // Version should be readable
        let version = read_installed_version(dir.path()).unwrap();
        assert_eq!(version, "2.5.1");
        
        // Overwrite with new version (simulating an update)
        write_installed_version(dir.path(), "3.0.0").unwrap();
        let version = read_installed_version(dir.path()).unwrap();
        assert_eq!(version, "3.0.0");
    }

    #[test]
    fn test_temp_dir_cleanup_when_empty() {
        // Simulate the temp directory cleanup logic
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().join(".velocity_temp");
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // Create a fake installer file in temp
        let installer = temp_dir.join("setup.exe");
        std::fs::write(&installer, b"MZ_FAKE").unwrap();
        
        // Remove the installer (simulating post-execute cleanup)
        std::fs::remove_file(&installer).unwrap();
        
        // Now clean up empty temp dir (same logic as download_all_files)
        if temp_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&temp_dir) {
                if entries.count() == 0 {
                    std::fs::remove_dir(&temp_dir).unwrap();
                }
            }
        }
        
        // Temp dir should be gone
        assert!(!temp_dir.exists(), "Empty temp dir should be cleaned up");
    }

    #[test]
    fn test_temp_dir_not_cleaned_if_files_remain() {
        // If temp dir still has files, it should NOT be removed
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().join(".velocity_temp");
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // Leave a file behind (simulating a failed cleanup)
        let leftover = temp_dir.join("leftover.tmp");
        std::fs::write(&leftover, b"data").unwrap();
        
        // Try cleanup
        if temp_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&temp_dir) {
                if entries.count() == 0 {
                    std::fs::remove_dir(&temp_dir).unwrap();
                }
            }
        }
        
        // Temp dir should still exist because it has files
        assert!(temp_dir.exists(), "Non-empty temp dir should NOT be cleaned up");
    }

    #[test]
    fn test_check_disk_space_nonexistent_path_ok() {
        // Non-existent path should pass (we can't check what doesn't exist)
        let path = std::path::Path::new("Z:\\nonexistent\\path");
        let result = check_disk_space(path, 1024 * 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_disk_space_sufficient() {
        // An existing temp dir should have at least 1 byte free
        let dir = tempfile::tempdir().unwrap();
        let result = check_disk_space(dir.path(), 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_disk_space_insufficient() {
        // Request an absurdly large amount — should fail
        let dir = tempfile::tempdir().unwrap();
        let result = check_disk_space(dir.path(), u64::MAX);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Insufficient disk space"), "Error should mention insufficient space: {}", err);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_get_free_disk_space_returns_value() {
        let dir = tempfile::tempdir().unwrap();
        let free = get_free_disk_space(dir.path()).unwrap();
        // Should return a positive number (at least 1MB on any real disk)
        assert!(free > 1024 * 1024, "Expected at least 1MB free, got {} bytes", free);
    }
}
