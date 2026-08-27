//! Installer detection and silent execution for cloud-fetch downloads.
//!
//! This module handles the "execute" action in the fetch pipeline:
//! 1. Auto-detect installer type from binary signatures (NSIS, InnoSetup, MSI, etc.)
//! 2. Generate appropriate silent install flags
//! 3. Execute the installer with timeout and exit code handling
//!
//! The execution path is Windows-only. On other platforms, `execute_silent_installer`
//! returns a clear error.

use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use tracing::{debug, info, warn};
use velocity_config::FetchAction;

// ─── Installer Type Detection ────────────────────────────────────────────

/// Detected installer framework type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerType {
    /// NSIS (Nullsoft Scriptable Install System) — uses `/S` for silent
    Nsis,
    /// Inno Setup — uses `/VERYSILENT` for silent
    InnoSetup,
    /// Windows Installer package — executed via `msiexec /i /qn`
    Msi,
    /// 7-Zip self-extracting archive (7z magic bytes)
    SevenZipSfx,
    /// 7-Zip installer (custom framework, uses `/S`)
    SevenZip,
    /// Unknown installer — user must provide args or we try common flags
    Unknown,
}

impl std::fmt::Display for InstallerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nsis => write!(f, "NSIS"),
            Self::InnoSetup => write!(f, "InnoSetup"),
            Self::Msi => write!(f, "MSI"),
            Self::SevenZipSfx => write!(f, "7z-SFX"),
            Self::SevenZip => write!(f, "7-Zip"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detect the installer type from a file's binary signature and extension.
///
/// Detection order:
/// 1. MSI: Check for OLE Compound header (D0 CF 11 E0)
/// 2. NSIS: Scan first 256KB for "Nullsoft" or "NSIS" strings
/// 3. InnoSetup: Scan first 256KB for "Inno Setup" or "Setup Software" strings
/// 4. 7z SFX: Check for 7z magic bytes (37 7A BC AF 27 1C)
/// 5. Fallback: Use file extension
pub fn detect_installer_type(path: &Path) -> InstallerType {
    // Try binary signature detection first
    if let Some(detected) = detect_from_binary(path) {
        debug!("Detected installer type from binary: {} ({})", detected, path.display());
        return detected;
    }

    // Fallback to file extension
    detect_from_extension(path)
}

/// Scan the first bytes of a file for known installer signatures.
///
/// Only reads the first 256KB to avoid loading large installers into memory.
/// 256KB is needed because some NSIS installers (e.g. Notepad++) have the
/// "Nullsoft" signature at ~185KB into the file.
fn detect_from_binary(path: &Path) -> Option<InstallerType> {
    // Read only the first 256KB — installer signatures are near the start
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = vec![0u8; 262144]; // 256KB
    let bytes_read = file.read(&mut header).ok()?;
    let data = &header[..bytes_read];

    if data.is_empty() {
        return None;
    }

    // MSI: OLE Compound Document header (D0 CF 11 E0 A1 B1 1A E1)
    if data.len() >= 8
        && data[0] == 0xD0
        && data[1] == 0xCF
        && data[2] == 0x11
        && data[3] == 0xE0
    {
        return Some(InstallerType::Msi);
    }

    // 7z SFX: 7z magic bytes (37 7A BC AF 27 1C)
    if data.len() >= 6
        && data[0] == 0x37
        && data[1] == 0x7A
        && data[2] == 0xBC
        && data[3] == 0xAF
        && data[4] == 0x27
        && data[5] == 0x1C
    {
        return Some(InstallerType::SevenZipSfx);
    }

    // NSIS: Look for "Nullsoft" or "NSIS" in the binary
    if contains_bytes(data, b"Nullsoft") || contains_bytes(data, b"NSIS") {
        return Some(InstallerType::Nsis);
    }

    // InnoSetup: Look for "Inno Setup" or "Setup Software"
    if contains_bytes(data, b"Inno Setup") || contains_bytes(data, b"Setup Software") {
        return Some(InstallerType::InnoSetup);
    }

    // 7-Zip: Look for "7-Zip" signature (custom installer framework)
    if contains_bytes(data, b"7-Zip") {
        return Some(InstallerType::SevenZip);
    }

    None
}

/// Check if a byte slice contains a given subsequence.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|window| window == needle)
}

/// Detect installer type from file extension.
fn detect_from_extension(path: &Path) -> InstallerType {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) {
        Some(ext) if ext == "msi" || ext == "msm" => InstallerType::Msi,
        _ => InstallerType::Unknown,
    }
}

/// Detect installer type from a config-specified file_type hint.
pub fn detect_from_file_type(file_type: Option<&str>, path: &Path) -> InstallerType {
    if let Some(ft) = file_type {
        match ft.to_lowercase().as_str() {
            "msi" | "msm" => return InstallerType::Msi,
            "nsis" => return InstallerType::Nsis,
            "innosetup" | "inno" => return InstallerType::InnoSetup,
            "7z" | "7zsfx" => return InstallerType::SevenZipSfx,
            "7zip" => return InstallerType::SevenZip,
            "exe" => {} // Fall through to binary detection
            _ => {}
        }
    }
    detect_installer_type(path)
}

// ─── Silent Argument Generation ──────────────────────────────────────────

/// Get default silent install arguments for a detected installer type.
///
/// # Arguments
/// * `installer_type` - The detected installer framework
/// * `install_dir` - Optional target installation directory
///
/// # Returns
/// A vector of command-line arguments for silent installation.
pub fn get_silent_args(installer_type: InstallerType, install_dir: Option<&Path>) -> Vec<String> {
    match installer_type {
        InstallerType::Nsis => {
            let mut args = vec!["/S".to_string()];
            if let Some(dir) = install_dir {
                // NSIS /D must be the LAST argument and must not be quoted
                args.push(format!("/D={}", dir.display()));
            }
            args
        }
        InstallerType::InnoSetup => {
            let mut args = vec![
                "/VERYSILENT".to_string(),
                "/SUPPRESSMSGBOXES".to_string(),
                "/NORESTART".to_string(),
            ];
            if let Some(dir) = install_dir {
                args.push(format!("/DIR=\"{}\"", dir.display()));
            }
            args
        }
        InstallerType::Msi => {
            // MSI is handled specially via msiexec, but include common props
            let mut args = vec![
                "/qn".to_string(),
                "/norestart".to_string(),
            ];
            if let Some(dir) = install_dir {
                args.push(format!("INSTALLDIR=\"{}\"", dir.display()));
            }
            args
        }
        InstallerType::SevenZipSfx => {
            // 7z SFX typically supports -y for yes-to-all and -o for output dir
            let mut args = vec!["-y".to_string()];
            if let Some(dir) = install_dir {
                args.push(format!("-o\"{}\"", dir.display()));
            }
            args
        }
        InstallerType::SevenZip => {
            // 7-Zip installer uses /S for silent (similar to NSIS)
            let mut args = vec!["/S".to_string()];
            if let Some(dir) = install_dir {
                args.push(format!("/D={}", dir.display()));
            }
            args
        }
        InstallerType::Unknown => {
            // No default args for unknown installers
            Vec::new()
        }
    }
}

// ─── Installer Execution ─────────────────────────────────────────────────

/// Result of executing a silent installer.
#[derive(Debug)]
pub struct InstallerResult {
    /// Whether the installation was successful
    pub success: bool,
    /// Exit code from the installer process
    pub exit_code: i32,
    /// Detected installer type
    pub installer_type: InstallerType,
}

/// Execute a downloaded installer silently.
///
/// # Arguments
/// * `installer_path` - Path to the downloaded installer file
/// * `user_args` - Optional user-specified command-line arguments (overrides auto-detection)
/// * `file_type` - Optional file type hint from config
/// * `install_dir` - Optional target installation directory
/// * `timeout_secs` - Maximum time to wait for the installer (default 300s)
///
/// # Returns
/// `InstallerResult` with success status and exit code.
///
/// # Errors
/// Returns an error if the file doesn't exist, is empty, or if the process
/// fails to start. If the installer times out, the process is killed and
/// an error is returned.
#[cfg(target_os = "windows")]
pub fn execute_silent_installer(
    installer_path: &Path,
    user_args: Option<&str>,
    file_type: Option<&str>,
    install_dir: Option<&Path>,
    timeout_secs: u64,
) -> Result<InstallerResult> {
    // ── File validation ──────────────────────────────────────────────
    if !installer_path.exists() {
        anyhow::bail!(
            "Installer file not found: {}",
            installer_path.display()
        );
    }

    let file_size = std::fs::metadata(installer_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if file_size == 0 {
        anyhow::bail!(
            "Installer file is empty (0 bytes): {}",
            installer_path.display()
        );
    }

    let installer_type = detect_from_file_type(file_type, installer_path);
    info!(
        "Executing silent installer: {} (type: {}, size: {} bytes, timeout: {}s)",
        installer_path.display(),
        installer_type,
        file_size,
        timeout_secs
    );

    // Build command line
    let (program, arguments) = build_command_line(
        installer_path,
        user_args,
        file_type,
        install_dir,
        installer_type,
    )?;

    debug!("Running: {} {:?}", program, arguments);

    // ── Spawn with captured output ───────────────────────────────────
    let mut child = std::process::Command::new(&program)
        .args(&arguments)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context(format!(
            "Failed to start installer: {} {}",
            program,
            arguments.join(" ")
        ))?;

    // ── Wait with timeout enforcement ────────────────────────────────
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(500);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited — collect output for diagnostics
                let output = child.wait_with_output()
                    .unwrap_or_else(|_| {
                        // Fallback: construct a minimal Output
                        std::process::Output {
                            status: status,
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                        }
                    });

                let exit_code = status.code().unwrap_or(-1);
                let success = status.success() || is_acceptable_exit_code(exit_code);

                if success {
                    info!(
                        "Installer completed successfully (exit code: {}, type: {}, {:.1}s)",
                        exit_code, installer_type,
                        start.elapsed().as_secs_f64()
                    );
                    return Ok(InstallerResult {
                        success: true,
                        exit_code,
                        installer_type,
                    });
                } else {
                    // Include truncated output for diagnostics
                    let stdout_snippet = String::from_utf8_lossy(&output.stdout);
                    let stderr_snippet = String::from_utf8_lossy(&output.stderr);
                    let stdout_trunc = &stdout_snippet[..stdout_snippet.len().min(500)];
                    let stderr_trunc = &stderr_snippet[..stderr_snippet.len().min(500)];

                    warn!(
                        "Installer failed with exit code {} (type: {})\n  stdout: {}\n  stderr: {}",
                        exit_code, installer_type, stdout_trunc.trim(), stderr_trunc.trim()
                    );
                    return Ok(InstallerResult {
                        success: false,
                        exit_code,
                        installer_type,
                    });
                }
            }
            Ok(None) => {
                // Still running
                if start.elapsed() > timeout {
                    // ── Timeout: kill the process ────────────────────
                    warn!(
                        "Installer timed out after {}s, killing process: {}",
                        timeout_secs, installer_path.display()
                    );
                    let _ = child.kill();
                    let _ = child.wait(); // reap zombie
                    anyhow::bail!(
                        "Installer timed out after {} seconds: {}",
                        timeout_secs,
                        installer_path.display()
                    );
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                // try_wait error — try to kill and report
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!(
                    "Failed to wait for installer: {} ({})",
                    installer_path.display(), e
                );
            }
        }
    }
}

/// Stub for non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn execute_silent_installer(
    _installer_path: &Path,
    _user_args: Option<&str>,
    _file_type: Option<&str>,
    _install_dir: Option<&Path>,
    _timeout_secs: u64,
) -> Result<InstallerResult> {
    anyhow::bail!(
        "Silent installer execution is only supported on Windows. \
         The downloaded file is at: {}",
        _installer_path.display()
    )
}

// ─── User-Configurable Installer Execution ────────────────────────────────

use velocity_config::InstallerConfig;

/// Execute an installer with full user configuration.
///
/// This function supports all features of `InstallerConfig`:
/// - Custom arguments with `{dir}` and `{file}` placeholder substitution
/// - Custom success exit codes
/// - Pre/post install commands
/// - Environment variables
/// - Custom timeout
/// - Custom working directory
///
/// This allows silent installation of ANY installer, including complex
/// setups like Adobe Acrobat, Visual Studio, or custom enterprise packages.
#[cfg(target_os = "windows")]
pub fn execute_with_config(
    installer_path: &Path,
    config: &InstallerConfig,
    install_dir: Option<&Path>,
) -> Result<InstallerResult> {
    // ── File validation ──────────────────────────────────────────────
    if !installer_path.exists() {
        anyhow::bail!("Installer file not found: {}", installer_path.display());
    }
    let file_size = std::fs::metadata(installer_path).map(|m| m.len()).unwrap_or(0);
    if file_size == 0 {
        anyhow::bail!("Installer file is empty (0 bytes): {}", installer_path.display());
    }

    info!(
        "Executing installer with custom config: {} ({} bytes)",
        installer_path.display(), file_size
    );

    // ── Custom signature detection (for logging) ────────────────────
    if !config.detect_signatures.is_empty() {
        match detect_custom_signatures(installer_path, &config.detect_signatures) {
            Some(sig) => info!(
                "Custom signature matched: '{}' (type: {})",
                sig,
                config.detect_name.as_deref().unwrap_or("custom")
            ),
            None => warn!(
                "None of the custom detect_signatures were found in {}",
                installer_path.display()
            ),
        }
    }

    // ── Kill processes before install ─────────────────────────────────
    for proc_name in &config.kill_processes {
        kill_process_by_name(proc_name);
    }

    // ── Pre-install commands ─────────────────────────────────────────
    for cmd in &config.pre_install {
        info!("Running pre-install command: {}", cmd);
        let status = run_shell_command(cmd)
            .with_context(|| format!("Failed to run pre-install command: {}", cmd))?;

        if !status.success() {
            warn!(
                "Pre-install command exited with code {}: {}",
                status.code().unwrap_or(-1), cmd
            );
            // Continue anyway — some pre-install commands may fail harmlessly
        }
    }

    // ── Build arguments with placeholder substitution ────────────────
    let args_str = if let Some(ref custom_args) = config.args {
        // Substitute placeholders: {dir} and {file}
        let substituted = custom_args
            .replace("{dir}", install_dir.map(|p| p.to_string_lossy().to_string()).as_deref().unwrap_or(""))
            .replace("{file}", &installer_path.to_string_lossy().to_string());
        Some(substituted)
    } else {
        None
    };

    // ── Determine timeout ────────────────────────────────────────────
    let timeout = config.timeout_secs.unwrap_or(300);

    // ── Determine working directory ──────────────────────────────────
    let working_dir = if let Some(ref wd) = config.working_dir {
        std::path::PathBuf::from(wd)
    } else {
        installer_path.parent().unwrap_or(installer_path).to_path_buf()
    };

    // ── Build and execute command ────────────────────────────────────
    let program = installer_path.to_string_lossy().to_string();
    let arguments = if let Some(ref args) = args_str {
        parse_args(args)
    } else {
        vec![]
    };

    debug!(
        "Custom installer command: {} {}",
        program,
        arguments.join(" ")
    );

    let mut cmd = std::process::Command::new(&program);
    cmd.args(&arguments)
        .current_dir(&working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // ── Set environment variables ────────────────────────────────────
    for (key, value) in &config.env {
        cmd.env(key, value);
        debug!("Set env: {}={}", key, value);
    }

    // ── Check if elevation is requested ──────────────────────────────
    let needs_elevation = config.elevate.unwrap_or(false);

    if needs_elevation {
        info!("Installer requires elevation (admin privileges)");

        // Use ShellExecuteW with "runas" verb for elevation
        let pid = spawn_elevated(&program, &arguments, &working_dir, &config.env)?;

        if pid == 0 {
            // Process may have already completed
            info!("Elevated installer appears to have completed quickly");
            for post_cmd in &config.post_install {
                run_post_install_cmd(post_cmd);
            }
            verify_installed_files(install_dir, &config.verify_files)?;
            if config.add_to_path {
                if let Some(dir) = install_dir {
                    add_directory_to_path(dir)?;
                }
            }
            return Ok(InstallerResult {
                installer_type: InstallerType::Unknown,
                exit_code: 0,
                success: true,
            });
        }

        // Poll for process completion by checking if PID still exists
        let timeout_dur = std::time::Duration::from_secs(timeout);
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_secs(2);

        loop {
            if !is_process_running(pid) {
                info!("Elevated installer (PID {}) has completed", pid);

                // Try to retrieve exit code via wmic
                let exit_code = get_process_exit_code(pid).unwrap_or(0);

                // Check success codes
                let is_success = if let Some(ref codes) = config.success_codes {
                    codes.contains(&exit_code)
                } else {
                    is_acceptable_exit_code(exit_code)
                };

                // Run post-install commands regardless of success
                for post_cmd in &config.post_install {
                    run_post_install_cmd(post_cmd);
                }

                if !is_success {
                    anyhow::bail!(
                        "Elevated installer failed with exit code {} (PID: {})",
                        exit_code, pid
                    );
                }

                // Post-install verification
                verify_installed_files(install_dir, &config.verify_files)?;
                if config.add_to_path {
                    if let Some(dir) = install_dir {
                        add_directory_to_path(dir)?;
                    }
                }

                return Ok(InstallerResult {
                    installer_type: InstallerType::Unknown,
                    exit_code,
                    success: true,
                });
            }

            if start.elapsed() > timeout_dur {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output();
                anyhow::bail!(
                    "Elevated installer timed out after {} seconds (PID: {})",
                    timeout, pid
                );
            }
            std::thread::sleep(poll_interval);
        }
    }

    // ── Normal (non-elevated) spawn and wait with timeout ────────────
    let mut child = cmd.spawn().with_context(|| {
        format!(
            "Failed to start installer: {} {}",
            program,
            arguments.join(" ")
        )
    })?;

    let timeout_dur = std::time::Duration::from_secs(timeout);
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(500);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                let stdout = collect_output(child.stdout.take());
                let stderr = collect_output(child.stderr.take());

                // ── Check success codes ──────────────────────────────
                let success_codes = config.success_codes.as_ref();
                let is_success = if let Some(codes) = success_codes {
                    codes.contains(&exit_code)
                } else {
                    is_acceptable_exit_code(exit_code)
                };

                if !is_success {
                    let stdout_preview = stdout.chars().take(500).collect::<String>();
                    let stderr_preview = stderr.chars().take(500).collect::<String>();
                    anyhow::bail!(
                        "Installer failed with exit code {}.\nstdout: {}\nstderr: {}",
                        exit_code, stdout_preview, stderr_preview
                    );
                }

                info!("Installer completed with exit code {}", exit_code);

                // ── Post-install commands ────────────────────────────
                for post_cmd in &config.post_install {
                    run_post_install_cmd(post_cmd);
                }

                // ── Post-install verification ────────────────────────
                verify_installed_files(install_dir, &config.verify_files)?;

                // ── PATH management ──────────────────────────────────
                if config.add_to_path {
                    if let Some(dir) = install_dir {
                        add_directory_to_path(dir)?;
                    }
                }

                return Ok(InstallerResult {
                    installer_type: InstallerType::Unknown, // Custom config - type determined by user
                    exit_code,
                    success: true,
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout_dur {
                    let _ = child.kill();
                    let _ = child.wait();
                    anyhow::bail!(
                        "Installer timed out after {} seconds: {}",
                        timeout,
                        installer_path.display()
                    );
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!(
                    "Failed to wait for installer: {} ({})",
                    installer_path.display(), e
                );
            }
        }
    }
}

/// Stub for non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn execute_with_config(
    _installer_path: &Path,
    _config: &InstallerConfig,
    _install_dir: Option<&Path>,
) -> Result<InstallerResult> {
    anyhow::bail!(
        "Silent installer execution is only supported on Windows. \
         The downloaded file is at: {}",
        _installer_path.display()
    )
}

/// Scan a file for custom signature strings.
/// Returns the first matching signature string, or None.
fn detect_custom_signatures(path: &Path, signatures: &[String]) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = vec![0u8; 262144]; // 256KB scan window
    let bytes_read = file.read(&mut header).ok()?;
    let data = &header[..bytes_read];

    for sig in signatures {
        if contains_bytes(data, sig.as_bytes()) {
            return Some(sig.clone());
        }
    }
    None
}

/// Launch an elevated process on Windows using ShellExecuteW with "runas" verb.
/// Returns the process ID so we can track completion.
#[cfg(target_os = "windows")]
fn spawn_elevated(
    program: &str,
    arguments: &[String],
    working_dir: &Path,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<u32> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    // Set environment variables (inherited by child processes)
    for (key, value) in env_vars {
        std::env::set_var(key, value);
    }

    // Build wide strings for ShellExecuteW
    let program_wide: Vec<u16> = OsStr::new(program)
        .encode_wide().chain(Some(0)).collect();
    let args_wide: Vec<u16> = OsStr::new(&arguments.join(" "))
        .encode_wide().chain(Some(0)).collect();
    let verb_wide: Vec<u16> = OsStr::new("runas")
        .encode_wide().chain(Some(0)).collect();
    let dir_wide: Vec<u16> = OsStr::new(&working_dir.to_string_lossy().to_string())
        .encode_wide().chain(Some(0)).collect();

    let result = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None, // no parent window
            windows::core::PCWSTR(verb_wide.as_ptr()),
            windows::core::PCWSTR(program_wide.as_ptr()),
            windows::core::PCWSTR(args_wide.as_ptr()),
            windows::core::PCWSTR(dir_wide.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns > 32 on success (result is HINSTANCE)
    let result_code = result.0 as usize;
    if result_code <= 32 {
        anyhow::bail!(
            "Failed to launch elevated installer (ShellExecuteW code {}). \
             The installer may require admin privileges. Run from an elevated terminal.",
            result_code as isize
        );
    }

    // Wait briefly for the process to start, then find its PID
    std::thread::sleep(std::time::Duration::from_millis(500));
    let exe_name = Path::new(program)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    for pid in find_process_by_name(&exe_name) {
        return Ok(pid);
    }

    // Process may have already completed (fast installer)
    Ok(0)
}

/// Find processes by name, return their PIDs.
#[cfg(target_os = "windows")]
fn find_process_by_name(name: &str) -> Vec<u32> {
    use std::process::Command;
    let output = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", name), "/FO", "CSV", "/NH"])
        .output()
        .ok();

    let mut pids = Vec::new();
    if let Some(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            // CSV format: "name","pid","session","session#","mem"
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }
    pids
}

/// Check if a process with the given PID is still running.
#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> bool {
    use std::process::Command;
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .ok();

    if let Some(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        // If tasklist output contains the PID, the process is running
        text.contains(&pid.to_string())
    } else {
        false
    }
}

/// Try to retrieve the exit code of a completed process by PID.
///
/// Uses `wmic process` to query the ExitCode. Returns None if the
/// process has already been reaped or the query fails.
#[cfg(target_os = "windows")]
fn get_process_exit_code(pid: u32) -> Option<i32> {
    let output = std::process::Command::new("wmic")
        .args([
            "process",
            "where",
            &format!("ProcessId={}", pid),
            "get",
            "ExitCode",
            "/Value",
        ])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    // Parse "ExitCode=<code>" from wmic output
    for line in text.lines() {
        let line = line.trim();
        if let Some(code_str) = line.strip_prefix("ExitCode=") {
            return code_str.trim().parse::<i32>().ok();
        }
    }
    None
}

/// Run a post-install command, logging any failures.
#[cfg(target_os = "windows")]
fn run_post_install_cmd(cmd: &str) {
    info!("Running post-install command: {}", cmd);
    let status = run_shell_command(cmd);

    match status {
        Ok(s) if !s.success() => {
            warn!(
                "Post-install command exited with code {}: {}",
                s.code().unwrap_or(-1), cmd
            );
        }
        Err(e) => {
            warn!("Failed to run post-install command: {}", e);
        }
        _ => {}
    }
}

/// Collect output from a child process pipe, truncated to a reasonable size.
#[cfg(target_os = "windows")]
fn collect_output<R: std::io::Read>(pipe: Option<R>) -> String {
    if let Some(mut p) = pipe {
        let mut buf = String::new();
        let _ = p.read_to_string(&mut buf);
        // Truncate to 10KB max
        if buf.len() > 10_000 {
            buf.truncate(10_000);
            buf.push_str("\n... (truncated)");
        }
        buf
    } else {
        String::new()
    }
}

/// Terminate a process by name using `taskkill /IM /F`.
///
/// Logs whether the process was found and terminated.
/// If the process isn't running, logs at debug level and continues.
/// After killing, waits 1 second for file handles to release.
#[cfg(target_os = "windows")]
fn kill_process_by_name(name: &str) {
    // Ensure .exe extension
    let proc_name = if name.to_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{}.exe", name)
    };

    info!("Attempting to terminate process: {}", proc_name);
    let output = std::process::Command::new("taskkill")
        .args(["/IM", &proc_name, "/F"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if out.status.success() {
                info!("Terminated process {}: {}", proc_name, stdout.trim());
                // Wait briefly for file handles to release
                std::thread::sleep(std::time::Duration::from_secs(1));
            } else {
                // taskkill returns non-zero if process not found — that's OK
                debug!("Process {} not running or could not be terminated: {}", proc_name, stdout.trim());
            }
        }
        Err(e) => {
            warn!("Failed to run taskkill for {}: {}", proc_name, e);
        }
    }
}

/// Verify that expected files exist after installation.
///
/// Checks each path in `verify_files` relative to `install_dir`.
/// Returns an error listing all missing files if any are not found.
#[cfg(target_os = "windows")]
fn verify_installed_files(install_dir: Option<&Path>, verify_files: &[String]) -> Result<()> {
    if verify_files.is_empty() {
        return Ok(());
    }
    let base = install_dir.unwrap_or_else(|| std::path::Path::new("."));
    let mut missing = Vec::new();
    for relative_path in verify_files {
        let full_path = base.join(relative_path);
        if !full_path.exists() {
            missing.push(relative_path.clone());
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "Post-install verification failed — missing files: {}",
            missing.join(", ")
        );
    }
    info!("Post-install verification passed ({} files checked)", verify_files.len());
    Ok(())
}

/// Add a directory to the user's PATH environment variable (Windows).
///
/// Reads the current user PATH from the registry, appends the directory
/// if not already present, and writes it back. Also broadcasts a
/// WM_SETTINGCHANGE so other apps pick up the change.
#[cfg(target_os = "windows")]
fn add_directory_to_path(dir: &Path) -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let dir_str = dir.to_string_lossy().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .context("Failed to open Environment registry key")?;

    let current_path: String = env_key.get_value("Path").unwrap_or_default();
    let dirs: Vec<&str> = current_path.split(';').collect();

    // Check if already in PATH (case-insensitive on Windows)
    let dir_lower = dir_str.to_lowercase();
    if dirs.iter().any(|d| d.trim_end_matches('\\').to_lowercase() == dir_lower) {
        debug!("Directory already in PATH: {}", dir_str);
        return Ok(());
    }

    // Append to PATH
    let new_path = if current_path.is_empty() {
        dir_str.clone()
    } else if current_path.ends_with(';') {
        format!("{}{}", current_path, dir_str)
    } else {
        format!("{};{}", current_path, dir_str)
    };

    env_key.set_value("Path", &new_path)
        .context("Failed to write PATH to registry")?;

    // Broadcast WM_SETTINGCHANGE so other apps pick up the change
    broadcast_setting_change();

    info!("Added to user PATH: {}", dir_str);
    Ok(())
}

/// Broadcast WM_SETTINGCHANGE to notify other applications of environment changes.
#[cfg(target_os = "windows")]
fn broadcast_setting_change() {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let env_str: Vec<u16> = "Environment\0".encode_utf16().collect();
        let result = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(env_str.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
        if result.0 == 0 {
            debug!("WM_SETTINGCHANGE broadcast timed out (non-critical)");
        }
    }
}

/// Run a command via the system shell.
///
/// Commands starting with "powershell" or "pwsh" are re-routed through
/// `powershell -EncodedCommand <base64>` to avoid nested-quote issues
/// when cmd.exe strips outer quotes.
///
/// All other commands are run via `cmd /S /C`.
#[cfg(target_os = "windows")]
fn run_shell_command(cmd: &str) -> std::io::Result<std::process::ExitStatus> {
    let trimmed = cmd.trim();
    if trimmed.starts_with("powershell") || trimmed.starts_with("pwsh") {
        // Extract the script after "powershell -Command " (or "pwsh -Command ")
        // Then encode it as UTF-16LE base64 for -EncodedCommand
        let script = extract_ps_script(trimmed);
        let encoded = encode_utf16le_base64(&script);
        let exe = if trimmed.starts_with("pwsh") { "pwsh.exe" } else { "powershell.exe" };
        std::process::Command::new(exe)
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    } else {
        // Use cmd /S /C for regular commands
        std::process::Command::new("cmd")
            .args(["/S", "/C", cmd])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    }
}

/// Extract the PowerShell script portion from a command string.
///
/// Handles: `powershell -Command "script"` → `script`
/// Falls back to everything after the first space if no `-Command` flag.
#[cfg(target_os = "windows")]
fn extract_ps_script(cmd: &str) -> String {
    // Try to find "-Command" flag and extract what follows
    let lower = cmd.to_lowercase();
    if let Some(idx) = lower.find("-command") {
        let after = &cmd[idx + "-command".len()..];
        let trimmed = after.trim();
        // Strip surrounding double quotes if present
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
        return trimmed.to_string();
    }
    // Fallback: everything after the first space (the exe name)
    if let Some(idx) = cmd.find(' ') {
        cmd[idx..].trim().to_string()
    } else {
        String::new()
    }
}

/// Encode a string as UTF-16LE then base64 (for PowerShell -EncodedCommand).
#[cfg(target_os = "windows")]
fn encode_utf16le_base64(s: &str) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let utf16: Vec<u8> = s
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();

    let mut out = String::with_capacity((utf16.len() + 2) / 3 * 4);
    for chunk in utf16.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Build the command line for executing an installer.
///
/// If `user_args` is provided, those are used directly.
/// Otherwise, auto-detected silent args are generated based on installer type.
fn build_command_line(
    installer_path: &Path,
    user_args: Option<&str>,
    _file_type: Option<&str>,
    install_dir: Option<&Path>,
    installer_type: InstallerType,
) -> Result<(String, Vec<String>)> {
    let path_str = installer_path.to_string_lossy().to_string();

    match installer_type {
        InstallerType::Msi => {
            // MSI files are executed via msiexec
            let mut arguments = vec![
                "/i".to_string(),
                path_str,
                "/qn".to_string(),
                "/norestart".to_string(),
            ];
            if let Some(args) = user_args {
                if !args.is_empty() {
                    arguments.extend(parse_args(args));
                }
            } else if let Some(dir) = install_dir {
                arguments.push(format!("INSTALLDIR=\"{}\"", dir.display()));
            }
            Ok(("msiexec.exe".to_string(), arguments))
        }
        _ => {
            // EXE-based installers: execute directly
            let mut arguments = Vec::new();
            if let Some(args) = user_args {
                if !args.is_empty() {
                    arguments.extend(parse_args(args));
                }
            } else {
                // Auto-detect silent args
                arguments.extend(get_silent_args(installer_type, install_dir));
            }
            Ok((path_str, arguments))
        }
    }
}

/// Parse command-line arguments, respecting quoted strings.
fn parse_args(args: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in args.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    result.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Check if an exit code is acceptable (e.g., "already installed").
///
/// Negative exit codes (from signals or crashes) are never acceptable.
fn is_acceptable_exit_code(code: i32) -> bool {
    // Negative codes indicate signals/crashes — never acceptable
    if code < 0 {
        return false;
    }
    matches!(
        code as u32,
        0      // Success
        | 3010 // MSI: success, reboot required
        | 1641 // MSI: success, reboot initiated
        | 1605 // MSI: product not installed (uninstall context)
        | 1638 // MSI: product is already installed
        | 1    // Generic "already installed" for some NSIS installers
    )
}

/// Resolve the action to perform for a downloaded file.
///
/// This is a helper for the fetch pipeline to decide what to do after download.
pub fn resolve_fetch_action(action: FetchAction) -> &'static str {
    match action {
        FetchAction::Extract => "extract",
        FetchAction::Execute => "execute",
        FetchAction::Copy => "copy",
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detect_from_extension_msi() {
        let path = Path::new("setup.msi");
        assert_eq!(detect_from_extension(path), InstallerType::Msi);
    }

    #[test]
    fn test_detect_from_extension_exe_unknown() {
        let path = Path::new("setup.exe");
        assert_eq!(detect_from_extension(path), InstallerType::Unknown);
    }

    #[test]
    fn test_detect_from_file_type_hint() {
        let path = Path::new("setup.exe");
        assert_eq!(
            detect_from_file_type(Some("msi"), path),
            InstallerType::Msi
        );
        assert_eq!(
            detect_from_file_type(Some("nsis"), path),
            InstallerType::Nsis
        );
        assert_eq!(
            detect_from_file_type(Some("innosetup"), path),
            InstallerType::InnoSetup
        );
        assert_eq!(
            detect_from_file_type(Some("inno"), path),
            InstallerType::InnoSetup
        );
    }

    #[test]
    fn test_detect_from_binary_msi() {
        // Create a temp file with OLE header
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]).unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::Msi);
    }

    #[test]
    fn test_detect_from_binary_7z_sfx() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]).unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::SevenZipSfx);
    }

    #[test]
    fn test_detect_from_binary_nsis() {
        let mut tmp = NamedTempFile::new().unwrap();
        // Write some padding then "Nullsoft" signature
        tmp.write_all(&[0u8; 100]).unwrap();
        tmp.write_all(b"Nullsoft Install System").unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::Nsis);
    }

    #[test]
    fn test_detect_from_binary_innosetup() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&[0u8; 100]).unwrap();
        tmp.write_all(b"Inno Setup Setup Data").unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::InnoSetup);
    }

    #[test]
    fn test_detect_from_binary_unknown() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"Just some random binary data").unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::Unknown);
    }

    #[test]
    fn test_detect_from_binary_7zip_installer() {
        // 7-Zip installer contains "7-Zip" string in the binary
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&[0u8; 100]).unwrap();
        tmp.write_all(b"7-Zip is free software").unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::SevenZip);
    }

    #[test]
    fn test_get_silent_args_nsis() {
        let args = get_silent_args(InstallerType::Nsis, None);
        assert_eq!(args, vec!["/S"]);

        let args = get_silent_args(InstallerType::Nsis, Some(Path::new("C:\\MyApp")));
        assert_eq!(args, vec!["/S", "/D=C:\\MyApp"]);
    }

    #[test]
    fn test_get_silent_args_innosetup() {
        let args = get_silent_args(InstallerType::InnoSetup, None);
        assert!(args.contains(&"/VERYSILENT".to_string()));
        assert!(args.contains(&"/SUPPRESSMSGBOXES".to_string()));
        assert!(args.contains(&"/NORESTART".to_string()));

        let args = get_silent_args(InstallerType::InnoSetup, Some(Path::new("C:\\MyApp")));
        assert!(args.iter().any(|a| a.contains("/DIR=")));
    }

    #[test]
    fn test_get_silent_args_msi() {
        let args = get_silent_args(InstallerType::Msi, None);
        assert!(args.contains(&"/qn".to_string()));
        assert!(args.contains(&"/norestart".to_string()));
    }

    #[test]
    fn test_get_silent_args_unknown() {
        let args = get_silent_args(InstallerType::Unknown, None);
        assert!(args.is_empty());
    }

    #[test]
    fn test_get_silent_args_7zip() {
        let args = get_silent_args(InstallerType::SevenZip, None);
        assert_eq!(args, vec!["/S"]);

        let args = get_silent_args(InstallerType::SevenZip, Some(Path::new("C:\\7zTest")));
        assert_eq!(args, vec!["/S", "/D=C:\\7zTest"]);
    }

    #[test]
    fn test_parse_args_simple() {
        let args = parse_args("/S /v/qn");
        assert_eq!(args, vec!["/S", "/v/qn"]);
    }

    #[test]
    fn test_parse_args_quoted() {
        let args = parse_args(r#"/S /D="C:\Program Files\App""#);
        assert_eq!(args, vec!["/S", r#"/D="C:\Program Files\App""#]);
    }

    #[test]
    fn test_parse_args_empty() {
        let args = parse_args("");
        assert!(args.is_empty());
    }

    #[test]
    fn test_acceptable_exit_codes() {
        assert!(is_acceptable_exit_code(0));
        assert!(is_acceptable_exit_code(3010));
        assert!(is_acceptable_exit_code(1641));
        assert!(is_acceptable_exit_code(1638));
        assert!(!is_acceptable_exit_code(1602)); // User cancelled
        assert!(!is_acceptable_exit_code(1603)); // Fatal error
        assert!(!is_acceptable_exit_code(9999));
    }

    #[test]
    fn test_build_command_line_msi() {
        let path = Path::new("C:\\temp\\setup.msi");
        let (prog, args) = build_command_line(
            path, None, None, None, InstallerType::Msi
        ).unwrap();
        assert_eq!(prog, "msiexec.exe");
        assert!(args.contains(&"/i".to_string()));
        assert!(args.contains(&"/qn".to_string()));
        assert!(args.contains(&"/norestart".to_string()));
    }

    #[test]
    fn test_build_command_line_exe_auto_detect() {
        let path = Path::new("C:\\temp\\setup.exe");
        let (prog, args) = build_command_line(
            path, None, None, None, InstallerType::Nsis
        ).unwrap();
        assert_eq!(prog, "C:\\temp\\setup.exe");
        assert_eq!(args, vec!["/S"]);
    }

    #[test]
    fn test_build_command_line_exe_user_args() {
        let path = Path::new("C:\\temp\\setup.exe");
        let (prog, args) = build_command_line(
            path, Some("/quiet /norestart"), None, None, InstallerType::Nsis
        ).unwrap();
        assert_eq!(prog, "C:\\temp\\setup.exe");
        assert_eq!(args, vec!["/quiet", "/norestart"]);
    }

    #[test]
    fn test_installer_type_display() {
        assert_eq!(format!("{}", InstallerType::Nsis), "NSIS");
        assert_eq!(format!("{}", InstallerType::InnoSetup), "InnoSetup");
        assert_eq!(format!("{}", InstallerType::Msi), "MSI");
        assert_eq!(format!("{}", InstallerType::SevenZipSfx), "7z-SFX");
        assert_eq!(format!("{}", InstallerType::SevenZip), "7-Zip");
        assert_eq!(format!("{}", InstallerType::Unknown), "Unknown");
    }

    #[test]
    fn test_contains_bytes() {
        assert!(contains_bytes(b"hello world", b"world"));
        assert!(contains_bytes(b"Nullsoft Install System", b"Nullsoft"));
        assert!(!contains_bytes(b"hello", b"world"));
        assert!(contains_bytes(b"", b""));
    }

    #[test]
    fn test_resolve_fetch_action() {
        assert_eq!(resolve_fetch_action(FetchAction::Extract), "extract");
        assert_eq!(resolve_fetch_action(FetchAction::Execute), "execute");
        assert_eq!(resolve_fetch_action(FetchAction::Copy), "copy");
    }

    // ── Hardened tests ─────────────────────────────────────────────────

    #[test]
    fn test_detect_from_binary_empty_file() {
        let tmp = NamedTempFile::new().unwrap();
        // Empty file should return None (no signature found)
        assert_eq!(detect_from_binary(tmp.path()), None);
    }

    #[test]
    fn test_detect_from_binary_nonexistent() {
        let path = Path::new("/nonexistent/file.exe");
        assert_eq!(detect_from_binary(path), None);
    }

    #[test]
    fn test_detect_from_binary_large_file_only_reads_header() {
        // Create a file with NSIS signature at the start, followed by lots of data
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"Nullsoft Signature At Start").unwrap();
        // Write 100KB of padding to prove we don't read the whole file
        tmp.write_all(&vec![0u8; 102400]).unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::Nsis);
    }

    #[test]
    fn test_detect_from_binary_signature_beyond_256k() {
        // Put NSIS signature BEYOND the 256KB read window — should NOT be detected
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&vec![0u8; 270000]).unwrap(); // 270KB of zeros
        tmp.write_all(b"Nullsoft").unwrap();
        tmp.flush().unwrap();
        // Should NOT detect NSIS since signature is beyond 256KB window
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::Unknown);
    }

    #[test]
    fn test_detect_from_binary_nsis_deep_signature_190k() {
        // Real NSIS installers (e.g. Notepad++) have "Nullsoft" at ~185-190KB.
        // With 256KB scan window this should be detected.
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&vec![0u8; 190_000]).unwrap(); // 190KB of padding
        tmp.write_all(b"Nullsoft Install System").unwrap();
        tmp.flush().unwrap();
        assert_eq!(detect_installer_type(tmp.path()), InstallerType::Nsis);
    }

    #[test]
    fn test_negative_exit_codes_not_acceptable() {
        assert!(!is_acceptable_exit_code(-1));
        assert!(!is_acceptable_exit_code(-100));
        assert!(!is_acceptable_exit_code(i32::MIN));
    }

    #[test]
    fn test_acceptable_exit_code_edge_cases() {
        // 1602 is user cancelled — NOT acceptable
        assert!(!is_acceptable_exit_code(1602));
        // 1603 is fatal error — NOT acceptable
        assert!(!is_acceptable_exit_code(1603));
        // 2 is generic error — NOT acceptable
        assert!(!is_acceptable_exit_code(2));
        // Large positive codes — NOT acceptable
        assert!(!is_acceptable_exit_code(99999));
    }

    #[test]
    fn test_contains_bytes_edge_cases() {
        // Empty needle always matches
        assert!(contains_bytes(b"anything", b""));
        assert!(contains_bytes(b"", b""));
        // Needle larger than haystack
        assert!(!contains_bytes(b"hi", b"hello"));
        // Exact match
        assert!(contains_bytes(b"NSIS", b"NSIS"));
        // Case sensitivity
        assert!(!contains_bytes(b"nsis", b"NSIS"));
    }

    #[test]
    fn test_parse_args_whitespace_handling() {
        // Multiple spaces between args
        let args = parse_args("/S   /norestart");
        assert_eq!(args, vec!["/S", "/norestart"]);
        // Leading/trailing whitespace
        let args = parse_args("  /S /norestart  ");
        assert_eq!(args, vec!["/S", "/norestart"]);
        // Tabs
        let args = parse_args("/S\t/norestart");
        assert_eq!(args, vec!["/S", "/norestart"]);
    }

    #[test]
    fn test_build_command_line_msi_with_user_args() {
        let path = Path::new("C:\\temp\\setup.msi");
        let (prog, args) = build_command_line(
            path, Some("INSTALLDIR=\"C:\\My App\" /qn"), None, None, InstallerType::Msi
        ).unwrap();
        assert_eq!(prog, "msiexec.exe");
        // Should have base msiexec args + user args
        assert!(args.contains(&"/i".to_string()));
        assert!(args.iter().any(|a| a.contains("INSTALLDIR")));
    }

    #[test]
    fn test_build_command_line_msi_with_install_dir() {
        let path = Path::new("C:\\temp\\setup.msi");
        let (prog, args) = build_command_line(
            path, None, None, Some(Path::new("C:\\MyApp")), InstallerType::Msi
        ).unwrap();
        assert_eq!(prog, "msiexec.exe");
        assert!(args.iter().any(|a| a.contains("INSTALLDIR")));
    }

    #[test]
    fn test_get_silent_args_7zsfx_with_dir() {
        let args = get_silent_args(InstallerType::SevenZipSfx, Some(Path::new("C:\\Extract")));
        assert_eq!(args[0], "-y");
        assert!(args.iter().any(|a| a.contains("-o")));
    }

    #[test]
    fn test_detect_from_file_type_case_insensitive() {
        let path = Path::new("setup.exe");
        assert_eq!(detect_from_file_type(Some("NSIS"), path), InstallerType::Nsis);
        assert_eq!(detect_from_file_type(Some("Nsis"), path), InstallerType::Nsis);
        assert_eq!(detect_from_file_type(Some("INNOSETUP"), path), InstallerType::InnoSetup);
        assert_eq!(detect_from_file_type(Some("MSI"), path), InstallerType::Msi);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_execute_nonexistent_file_returns_error() {
        let path = Path::new("C:\\nonexistent\\installer.exe");
        let result = execute_silent_installer(path, None, None, None, 30);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"), "Error should mention file not found: {}", err_msg);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_execute_empty_file_returns_error() {
        let tmp = NamedTempFile::new().unwrap();
        // File exists but is empty (0 bytes)
        let result = execute_silent_installer(tmp.path(), None, None, None, 30);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty"), "Error should mention empty file: {}", err_msg);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_verify_installed_files_all_present() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Create expected files
        std::fs::write(tmp.path().join("app.exe"), b"binary").unwrap();
        std::fs::create_dir_all(tmp.path().join("lib")).unwrap();
        std::fs::write(tmp.path().join("lib/core.dll"), b"dll").unwrap();

        let files = vec!["app.exe".to_string(), "lib/core.dll".to_string()];
        let result = verify_installed_files(Some(tmp.path()), &files);
        assert!(result.is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_verify_installed_files_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("app.exe"), b"binary").unwrap();

        let files = vec!["app.exe".to_string(), "missing.dll".to_string()];
        let result = verify_installed_files(Some(tmp.path()), &files);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing.dll"), "Error should list missing file: {}", err);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_verify_installed_files_empty_list() {
        let result = verify_installed_files(None, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_installer_config_deserialize_verify_files() {
        let toml = r#"
            args = "/S"
            verify_files = ["bin/app.exe", "lib/core.dll"]
            add_to_path = true
        "#;
        let config: velocity_config::InstallerConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.verify_files, vec!["bin/app.exe", "lib/core.dll"]);
        assert!(config.add_to_path);
    }

    #[test]
    fn test_installer_config_defaults_no_verify() {
        let toml = r#"args = "/S""#;
        let config: velocity_config::InstallerConfig = toml::from_str(toml).unwrap();
        assert!(config.verify_files.is_empty());
        assert!(!config.add_to_path);
        assert!(config.kill_processes.is_empty());
    }

    #[test]
    fn test_installer_config_kill_processes() {
        let toml = r#"
            args = "/S"
            kill_processes = ["notepad++", "code.exe", "MyApp"]
        "#;
        let config: velocity_config::InstallerConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.kill_processes, vec!["notepad++", "code.exe", "MyApp"]);
    }
}
