//! Installation logging to file.
//!
//! Writes a detailed log of all installation operations to a file
//! in the installation directory (or temp directory during install).

use crate::error::{CoreError, Result};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Global logger instance.
static LOGGER: Mutex<Option<InstallLogger>> = Mutex::new(None);

/// Initialize the global logger for this installation session.
pub fn init_logger(log_dir: &Path, app_name: &str) -> Result<PathBuf> {
    let log_path = log_dir.join(format!("{}_install.log", sanitize_filename(app_name)));
    let logger = InstallLogger::new(&log_path)?;
    let mut guard = LOGGER.lock().map_err(|e| CoreError::other("logger lock", format!("{}", e)))?;
    *guard = Some(logger);
    Ok(log_path)
}

/// Initialize a temporary logger (before install dir is known).
pub fn init_temp_logger(app_name: &str) -> Result<PathBuf> {
    let log_path = std::env::temp_dir().join(format!("{}_install.log", sanitize_filename(app_name)));
    let logger = InstallLogger::new(&log_path)?;
    let mut guard = LOGGER.lock().map_err(|e| CoreError::other("logger lock", format!("{}", e)))?;
    *guard = Some(logger);
    Ok(log_path)
}

/// Move the log file to the final install directory.
pub fn move_log_to_install_dir(install_dir: &Path, app_name: &str) -> Result<PathBuf> {
    let temp_log = std::env::temp_dir().join(format!("{}_install.log", sanitize_filename(app_name)));
    let final_log = install_dir.join(format!("{}_install.log", sanitize_filename(app_name)));
    
    if temp_log.exists() {
        std::fs::copy(&temp_log, &final_log)?;
        let _ = std::fs::remove_file(&temp_log);
    }
    
    // Update the logger to point to the new location
    let mut guard = LOGGER.lock().map_err(|e| CoreError::other("logger lock", format!("{}", e)))?;
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

/// Get the current log file path.
pub fn log_path() -> Option<PathBuf> {
    LOGGER.lock().ok()
        .and_then(|guard| guard.as_ref().map(|l| l.log_path.clone()))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn timestamp() -> String {
    let now = SystemTime::now();
    let duration = now.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    
    // Convert Unix timestamp to human-readable date/time
    // Simple algorithm: compute year/month/day from days since epoch
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    
    // Compute year, month, day from days since 1970-01-01
    let (year, month, day) = days_to_ymd(days as i64);
    
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, minutes, seconds)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days_since_epoch: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    // Uses floor division throughout (not truncation towards zero).
    let days = days_since_epoch + 719468;
    let era = floor_div(days, 146097);
    let doe = (days - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Floor division (rounds towards negative infinity, not towards zero).
fn floor_div(a: i64, b: i64) -> i64 {
    let d = a / b;
    let r = a % b;
    if (r > 0 && b < 0) || (r < 0 && b > 0) { d - 1 } else { d }
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
        
        Ok(Self { file, log_path: path.to_path_buf() })
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
    fn test_days_to_ymd_epoch() {
        // Day 0 = 1970-01-01
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_known_date() {
        // 2000-01-01 = day 10957
        let (y, m, d) = days_to_ymd(10957);
        assert_eq!((y, m, d), (2000, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_2024() {
        // 2024-08-10 = day 19945
        let (y, m, d) = days_to_ymd(19945);
        assert_eq!((y, m, d), (2024, 8, 10));
    }

    #[test]
    fn test_floor_div_positive() {
        assert_eq!(floor_div(10, 3), 3);
        assert_eq!(floor_div(9, 3), 3);
    }

    #[test]
    fn test_floor_div_negative() {
        assert_eq!(floor_div(-1, 146097), -1);
        assert_eq!(floor_div(-146097, 146097), -1);
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My App!"), "My_App_");
        assert_eq!(sanitize_filename("test-123"), "test-123");
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    }
}
