#![cfg(target_os = "windows")]
//! Dependency and bundled app installer orchestrator.
//!
//! Coordinates the full lifecycle:
//! 1. Evaluate conditions to determine which dependencies are needed
//! 2. Download remote dependencies (with progress)
//! 3. Verify SHA256 integrity
//! 4. Execute silent installers
//! 5. Handle bundled apps from the payload

use crate::dep_resolver;
use crate::downloader;
use crate::error::{CoreError, Result};
use crate::logging;
use crate::rollback::RollbackTracker;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};
use velocity_config::{BundledAppEntry, DependencyEntry};

/// Result of installing a single dependency.
#[derive(Debug)]
pub struct DepInstallResult {
    pub name: String,
    pub installed: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

/// Install all remote dependencies that pass their condition checks.
///
/// Returns a list of results for each dependency.
pub fn install_dependencies(
    dependencies: &[DependencyEntry],
    temp_dir: &Path,
    rollback: &mut RollbackTracker,
) -> Vec<DepInstallResult> {
    let mut results = Vec::new();

    // Sort by priority (lower first)
    let mut sorted: Vec<&DependencyEntry> = dependencies.iter().collect();
    sorted.sort_by_key(|d| d.priority);

    for dep in &sorted {
        info!(
            "Processing dependency: {} (priority: {})",
            dep.name, dep.priority
        );

        // Evaluate condition
        if !dep_resolver::evaluate_condition(&dep.condition) {
            logging::log(&format!(
                "Skipping {} (condition not met: {})",
                dep.name, dep.condition
            ));
            results.push(DepInstallResult {
                name: dep.name.clone(),
                installed: false,
                skipped: true,
                error: None,
            });
            continue;
        }

        // Download
        logging::log(&format!("Downloading {}...", dep.name));
        let download_result = download_dependency(dep, temp_dir);

        let file_path = match download_result {
            Ok(path) => path,
            Err(e) => {
                let err_msg = format!("Failed to download {}: {}", dep.name, e);
                error!("{}", err_msg);
                logging::log_error("DEP", &err_msg);

                if dep.required {
                    results.push(DepInstallResult {
                        name: dep.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                    // Stop processing if required dependency fails
                    break;
                } else {
                    results.push(DepInstallResult {
                        name: dep.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                    continue;
                }
            }
        };

        // Track for rollback (temp file cleanup)
        rollback.track_file(file_path.clone());

        // Install
        logging::log(&format!("Installing {}...", dep.name));
        let install_result =
            execute_installer(&file_path, &dep.install_args, &dep.file_type, temp_dir);

        match install_result {
            Ok(()) => {
                logging::log_success(&format!("Installed {}", dep.name));
                results.push(DepInstallResult {
                    name: dep.name.clone(),
                    installed: true,
                    skipped: false,
                    error: None,
                });
            }
            Err(e) => {
                let err_msg = format!("Failed to install {}: {}", dep.name, e);
                error!("{}", err_msg);
                logging::log_error("DEP", &err_msg);

                if dep.required {
                    results.push(DepInstallResult {
                        name: dep.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                    break;
                } else {
                    warn!("Optional dependency {} failed, continuing", dep.name);
                    results.push(DepInstallResult {
                        name: dep.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                }
            }
        }
    }

    results
}

/// Install bundled apps from the payload.
///
/// `payload_files` maps relative paths to their extracted absolute paths.
pub fn install_bundled_apps(
    bundled_apps: &[BundledAppEntry],
    payload_files: &HashMap<String, PathBuf>,
    temp_dir: &Path,
    _rollback: &mut RollbackTracker,
) -> Vec<DepInstallResult> {
    let mut results = Vec::new();

    // Sort by priority
    let mut sorted: Vec<&BundledAppEntry> = bundled_apps.iter().collect();
    sorted.sort_by_key(|b| b.priority);

    for app in &sorted {
        info!(
            "Processing bundled app: {} (priority: {})",
            app.name, app.priority
        );

        // Evaluate condition
        if !dep_resolver::evaluate_condition(&app.condition) {
            logging::log(&format!(
                "Skipping {} (condition not met: {})",
                app.name, app.condition
            ));
            results.push(DepInstallResult {
                name: app.name.clone(),
                installed: false,
                skipped: true,
                error: None,
            });
            continue;
        }

        // Find the installer in the extracted payload
        let installer_path = payload_files.get(&app.installer);
        let installer_path = match installer_path {
            Some(path) if path.exists() => path.clone(),
            _ => {
                let err_msg = format!(
                    "Bundled app installer not found in payload: {}",
                    app.installer
                );
                error!("{}", err_msg);
                logging::log_error("BUNDLED", &err_msg);

                if app.required {
                    results.push(DepInstallResult {
                        name: app.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                    break;
                } else {
                    results.push(DepInstallResult {
                        name: app.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                    continue;
                }
            }
        };

        // Determine file type from extension
        let file_type = installer_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("exe")
            .to_lowercase();

        // Working directory
        let work_dir = app
            .working_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| temp_dir.to_path_buf());

        // Install
        logging::log(&format!("Installing bundled app: {}...", app.name));
        let result = execute_installer_with_workdir(
            &installer_path,
            &app.install_args,
            &file_type,
            &work_dir,
        );

        match result {
            Ok(()) => {
                logging::log_success(&format!("Installed {}", app.name));
                results.push(DepInstallResult {
                    name: app.name.clone(),
                    installed: true,
                    skipped: false,
                    error: None,
                });
            }
            Err(e) => {
                let err_msg = format!("Failed to install {}: {}", app.name, e);
                error!("{}", err_msg);
                logging::log_error("BUNDLED", &err_msg);

                if app.required {
                    results.push(DepInstallResult {
                        name: app.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                    break;
                } else {
                    warn!("Optional bundled app {} failed, continuing", app.name);
                    results.push(DepInstallResult {
                        name: app.name.clone(),
                        installed: false,
                        skipped: false,
                        error: Some(err_msg),
                    });
                }
            }
        }
    }

    results
}

/// Download a dependency to the temp directory.
fn download_dependency(dep: &DependencyEntry, temp_dir: &Path) -> Result<PathBuf> {
    // Create a deps subdirectory
    let deps_dir = temp_dir.join("velocity_deps");
    std::fs::create_dir_all(&deps_dir)?;

    downloader::download_file(&dep.url, &deps_dir, None, dep.sha256.as_deref(), None)
}

/// Execute an installer with the given arguments.
fn execute_installer(
    installer_path: &Path,
    args: &str,
    file_type: &str,
    work_dir: &Path,
) -> Result<()> {
    execute_installer_with_workdir(installer_path, args, file_type, work_dir)
}

/// Execute an installer with a specific working directory.
fn execute_installer_with_workdir(
    installer_path: &Path,
    args: &str,
    file_type: &str,
    work_dir: &Path,
) -> Result<()> {
    let (program, arguments) = build_command_line(installer_path, args, file_type)?;

    debug!("Executing: {} {:?}", program, arguments);

    let output = std::process::Command::new(&program)
        .args(&arguments)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(())
            } else {
                let code = out.status.code().unwrap_or(-1);
                // Some installers return non-zero for "already installed" which is OK
                if is_acceptable_exit_code(code) {
                    debug!("Installer exited with code {} (acceptable)", code);
                    Ok(())
                } else {
                    Err(CoreError::other(
                        "installer execution",
                        format!("Installer exited with code {}", code),
                    ))
                }
            }
        }
        Err(e) => Err(CoreError::other(
            "installer execution",
            format!("Failed to execute installer: {}", e),
        )),
    }
}

/// Build the command line for executing an installer based on file type.
fn build_command_line(
    installer_path: &Path,
    args: &str,
    file_type: &str,
) -> Result<(String, Vec<String>)> {
    let path_str = installer_path.to_string_lossy().to_string();

    match file_type.to_lowercase().as_str() {
        "msi" | "msm" => {
            // Use msiexec for MSI packages
            let mut arguments = vec![
                "/i".to_string(),
                path_str,
                "/qn".to_string(),
                "/norestart".to_string(),
            ];
            if !args.is_empty() {
                arguments.push(args.to_string());
            }
            Ok(("msiexec.exe".to_string(), arguments))
        }
        "exe" | "" => {
            // Execute directly with provided args
            let mut arguments = Vec::new();
            if !args.is_empty() {
                // Split args by spaces, respecting quotes
                arguments.extend(parse_args(args));
            }
            Ok((path_str, arguments))
        }
        _other => {
            // Try to execute directly
            let mut arguments = Vec::new();
            if !args.is_empty() {
                arguments.extend(parse_args(args));
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
fn is_acceptable_exit_code(code: i32) -> bool {
    matches!(
        code as u32,
        0    // Success
        | 3010 // MSI: success, reboot required
        | 1641 // MSI: success, reboot initiated
        | 1605 // MSI: product not installed (uninstall context)
        | 1638 // MSI: product is already installed
        | 1 // Generic "already installed" for some NSIS installers
    )
}

/// Check if all required dependencies were installed successfully.
pub fn all_required_installed(results: &[DepInstallResult], deps: &[DependencyEntry]) -> bool {
    for result in results {
        if result.error.is_some() {
            // Check if this dep was required
            if let Some(dep) = deps.iter().find(|d| d.name == result.name) {
                if dep.required {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_build_command_line_msi() {
        let (prog, args) =
            build_command_line(Path::new("C:\\temp\\setup.msi"), "ADDLOCAL=ALL", "msi").unwrap();
        assert_eq!(prog, "msiexec.exe");
        assert!(args.contains(&"/i".to_string()));
        assert!(args.contains(&"/qn".to_string()));
        assert!(args.contains(&"/norestart".to_string()));
        assert!(args.contains(&"ADDLOCAL=ALL".to_string()));
    }

    #[test]
    fn test_build_command_line_exe() {
        let (prog, args) =
            build_command_line(Path::new("C:\\temp\\setup.exe"), "/S /v/qn", "exe").unwrap();
        assert_eq!(prog, "C:\\temp\\setup.exe");
        assert_eq!(args, vec!["/S", "/v/qn"]);
    }

    #[test]
    fn test_acceptable_exit_codes() {
        assert!(is_acceptable_exit_code(0));
        assert!(is_acceptable_exit_code(3010));
        assert!(is_acceptable_exit_code(1641));
        assert!(is_acceptable_exit_code(1638)); // Already installed
        assert!(!is_acceptable_exit_code(1602)); // User cancelled
        assert!(!is_acceptable_exit_code(1603)); // MSI fatal error
        assert!(!is_acceptable_exit_code(9999)); // Random failure
    }

    #[test]
    fn test_all_required_installed() {
        let deps = vec![DependencyEntry {
            name: "VC++".to_string(),
            url: "https://example.com/vc.exe".to_string(),
            sha256: None,
            install_args: "/S".to_string(),
            condition: "always".to_string(),
            priority: 100,
            required: true,
            file_type: "exe".to_string(),
        }];
        let results = vec![DepInstallResult {
            name: "VC++".to_string(),
            installed: true,
            skipped: false,
            error: None,
        }];
        assert!(all_required_installed(&results, &deps));
    }
}
