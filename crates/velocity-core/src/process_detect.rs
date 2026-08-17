//! Process detection — check if an application is currently running.
//!
//! Used before installation to warn the user, and after installation
//! to know when it's safe to replace files.
//!
//! Cross-platform: uses `tasklist` on Windows, `pgrep`/`ps` on Unix.

use crate::error::{CoreError, Result};
use std::path::Path;

/// Check if a process with the given name is currently running.
///
/// `process_name` should be just the filename (e.g., "myapp.exe" on Windows,
/// "myapp" on Unix). Uses exact name matching to avoid false positives.
#[cfg(target_os = "windows")]
pub fn is_process_running(process_name: &str) -> Result<bool> {
    let filter = format!("IMAGENAME eq {}", process_name);
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &filter, "/NH", "/FO", "CSV"])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run tasklist: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let name_lower = process_name.to_lowercase();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(first_field) = line.split(',').next() {
            let name = first_field.trim_matches('"').to_lowercase();
            if name == name_lower {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Check if a process with the given name is currently running (Unix).
///
/// Tries `pgrep -x` first (exact match), falls back to parsing `ps` output.
#[cfg(not(target_os = "windows"))]
pub fn is_process_running(process_name: &str) -> Result<bool> {
    // Try pgrep first (available on most Linux distros and macOS)
    if let Ok(output) = std::process::Command::new("pgrep")
        .args(["-x", process_name])
        .output()
    {
        if output.status.success() {
            // pgrep found at least one match
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Ok(stdout.lines().any(|l| !l.trim().is_empty()));
        }
        // pgrep returned non-zero = no match found (exit code 1)
        if output.status.code() == Some(1) {
            return Ok(false);
        }
        // Other error — fall through to ps
    }

    // Fallback: parse ps output
    let output = std::process::Command::new("ps")
        .args(["-eo", "comm"])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run ps: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let name_lower = process_name.to_lowercase();
    for line in stdout.lines().skip(1) {
        // comm column may contain full path on some systems
        let proc_name = line
            .trim()
            .rsplit('/')
            .next()
            .unwrap_or(line.trim())
            .to_lowercase();
        if proc_name == name_lower {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Check if the application's main executable is running.
///
/// Uses the manifest's main_exe field to determine the process name.
pub fn is_app_running(main_exe: &str) -> Result<bool> {
    let exe_name = Path::new(main_exe)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(main_exe);

    is_process_running(exe_name)
}

/// Wait for a process to exit, with a timeout.
///
/// Returns true if the process exited within the timeout.
pub fn wait_for_process_exit(process_name: &str, timeout_secs: u64) -> Result<bool> {
    use std::time::{Duration, Instant};

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        if !is_process_running(process_name)? {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Ok(false)
}

/// Get a list of all running process names.
#[cfg(target_os = "windows")]
pub fn list_running_processes() -> Result<Vec<String>> {
    let output = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run tasklist: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let processes: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            line.split(',')
                .next()
                .map(|s| s.trim_matches('"').to_string())
        })
        .collect();

    Ok(processes)
}

/// Get a list of all running process names (Unix).
#[cfg(not(target_os = "windows"))]
pub fn list_running_processes() -> Result<Vec<String>> {
    let output = std::process::Command::new("ps")
        .args(["-eo", "comm"])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run ps: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let processes: Vec<String> = stdout
        .lines()
        .skip(1)
        .map(|line| {
            // Extract just the filename from potentially full paths
            line.trim()
                .rsplit('/')
                .next()
                .unwrap_or(line.trim())
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();

    Ok(processes)
}

/// Kill a process by name (Unix).
///
/// Uses `pkill` to send SIGTERM to matching processes.
#[cfg(not(target_os = "windows"))]
pub fn kill_process_by_name(process_name: &str) -> Result<()> {
    let output = std::process::Command::new("pkill")
        .args(["-x", process_name])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run pkill: {}", e)))?;

    if output.status.success() || output.status.code() == Some(1) {
        // success = killed something, code 1 = no match (also fine)
        Ok(())
    } else {
        Err(CoreError::Other(format!(
            "pkill failed with exit code: {:?}",
            output.status.code()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_process_running_nonexistent() {
        let result = is_process_running("velocity_nonexistent_test_process_12345");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_list_running_processes() {
        let processes = list_running_processes();
        assert!(processes.is_ok());
        assert!(!processes.unwrap().is_empty());
    }

    #[test]
    fn test_is_app_running_nonexistent() {
        let result = is_app_running("/usr/bin/velocity_nonexistent_12345");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
