//! Process detection — check if an application is currently running.
//!
//! Used before installation to warn the user, and after installation
//! to know when it's safe to replace files.

use crate::error::{CoreError, Result};
use std::path::Path;

/// Check if a process with the given name is currently running.
///
/// `process_name` should be just the filename (e.g., "myapp.exe").
/// Uses exact name matching to avoid false positives from partial matches.
pub fn is_process_running(process_name: &str) -> Result<bool> {
    let filter = format!("IMAGENAME eq {}", process_name);
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &filter, "/NH", "/FO", "CSV"])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run tasklist: {}", e)))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // With CSV output and exact IMAGENAME filter, check if any line contains
    // the exact process name as a quoted CSV field
    let name_lower = process_name.to_lowercase();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // CSV format: "name.exe","pid","session","session#","mem"
        if let Some(first_field) = line.split(',').next() {
            let name = first_field.trim_matches('"').to_lowercase();
            if name == name_lower {
                return Ok(true);
            }
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
pub fn list_running_processes() -> Result<Vec<String>> {
    let output = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| CoreError::Other(format!("Failed to run tasklist: {}", e)))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let processes: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // CSV format: "name.exe","pid","session","session#","mem"
            line.split(',')
                .next()
                .map(|s| s.trim_matches('"').to_string())
        })
        .collect();
    
    Ok(processes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_process_running_nonexistent() {
        // A process that definitely doesn't exist
        let result = is_process_running("velocity_nonexistent_test_process_12345.exe");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_list_running_processes() {
        let processes = list_running_processes();
        assert!(processes.is_ok());
        // Should have at least some processes running
        assert!(!processes.unwrap().is_empty());
    }
}
