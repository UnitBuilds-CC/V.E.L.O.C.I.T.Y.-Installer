//! Installation logging to file.
//!
//! Writes a detailed log of all installation operations to a file
//! in the installation directory (or temp directory during install).

use crate::error::{CoreError, Result};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Global logger instance.
static LOGGER: Mutex<Option<InstallLogger>> = Mutex::new(None);

/// Initialize the global logger for this installation session.
pub fn init_logger(log_dir: &Path, app_name: &str) -> Result<PathBuf> {
    let log_path = log_dir.join(format!("{}_install.log", sanitize_filename(app_name)));
    let logger = InstallLogger::new(&log_path)?;
    let mut guard = LOGGER
        .lock()
        .map_err(|e| CoreError::other("logger lock", format!("{}", e)))?;
    *guard = Some(logger);
    Ok(log_path)
}

/// Initialize a temporary logger (before install dir is known).
pub fn init_temp_logger(app_name: &str) -> Result<PathBuf> {
    let log_path =
        std::env::temp_dir().join(format!("{}_install.log", sanitize_filename(app_name)));
    let logger = InstallLogger::new(&log_path)?;
    let mut guard = LOGGER
        .lock()
        .map_err(|e| CoreError::other("logger lock", format!("{}", e)))?;
    *guard = Some(logger);
    Ok(log_path)
}

/// Move the log file to the final install directory.
pub fn move_log_to_install_dir(install_dir: &Path, app_name: &str) -> Result<PathBuf> {
    let temp_log =
        std::env::temp_dir().join(format!("{}_install.log", sanitize_filename(app_name)));
    let final_log = install_dir.join(format!("{}_install.log", sanitize_filename(app_name)));

    if temp_log.exists() {
        std::fs::copy(&temp_log, &final_log)?;
        let _ = std::fs::remove_file(&temp_log);
    }

    // Update the logger to point to the new location
    let mut guard = LOGGER
        .lock()
        .map_err(|e| CoreError::other("logger lock", format!("{}", e)))?;
    if let Some(ref mut logger) = *guard {
        logger.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&final_log)?;
        logger.log_path = final_log.clone();
    }

    Ok(final_log)
}

/// Log a message globally.
pub fn log(message: &str) {
    if let Ok(mut guard) = LOGGER.lock() {
        if let Some(ref mut logger) = *guard {
            let _ = logger.write_entry(message);
        }
    }
}

/// Log an operation with timestamp and category.
pub fn log_op(category: &str, message: &str) {
    log(&format!("[{}] {}", category, message));
}

/// Log a file extraction.
pub fn log_extract(file_name: &str) {
    log_op("EXTRACT", file_name);
}

/// Log a registry operation.
pub fn log_registry(key: &str, value: &str) {
    log_op("REGISTRY", &format!("{} = {}", key, value));
}

/// Log a shortcut creation.
pub fn log_shortcut(name: &str, target: &str) {
    log_op("SHORTCUT", &format!("{} -> {}", name, target));
}

/// Log an environment variable change.
pub fn log_env_var(name: &str, value: &str, scope: &str) {
    log_op("ENV", &format!("[{}] {} = {}", scope, name, value));
}

/// Log a service operation.
pub fn log_service(name: &str, action: &str) {
    log_op("SERVICE", &format!("{}: {}", name, action));
}

/// Log an error.
pub fn log_error(context: &str, error: &str) {
    log_op("ERROR", &format!("{}: {}", context, error));
}

/// Log a warning.
pub fn log_warning(message: &str) {
    log_op("WARN", message);
}

/// Log success.
pub fn log_success(message: &str) {
    log_op("OK", message);
}

/// Record an install outcome metric for telemetry.
///
/// Writes a single structured line to the install log that can be parsed
/// by log aggregation tools to compute success/failure/cancel rates.
///
/// Outcome values: "success", "failed", "cancelled", "rolled_back"
pub fn log_install_outcome(app_name: &str, version: &str, outcome: &str, duration_secs: u64) {
    log_op(
        "TELEMETRY",
        &format!(
            "app={} version={} outcome={} duration={}s",
            app_name, version, outcome, duration_secs
        ),
    );
}

/// Get the current log file path.
pub fn log_path() -> Option<PathBuf> {
    LOGGER
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|l| l.log_path.clone()))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// The install logger.
struct InstallLogger {
    file: File,
    log_path: PathBuf,
}

impl InstallLogger {
    fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        writeln!(file, "=== Velocity Installer Log ===")?;
        writeln!(file, "Timestamp: {}", timestamp())?;
        writeln!(file, "================================")?;

        Ok(Self {
            file,
            log_path: path.to_path_buf(),
        })
    }

    fn write_entry(&mut self, message: &str) -> std::io::Result<()> {
        writeln!(self.file, "[{}] {}", timestamp(), message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_format() {
        let ts = timestamp();
        // Should be in YYYY-MM-DD HH:MM:SS format
        assert_eq!(ts.len(), 19, "Timestamp should be 19 chars: got '{}'", ts);
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], " ");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My App!"), "My_App_");
        assert_eq!(sanitize_filename("test-123"), "test-123");
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    }
}
