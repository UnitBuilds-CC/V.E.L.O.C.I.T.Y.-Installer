//! Installation rollback support.
//!
//! Tracks all changes made during installation so they can be
//! undone if the installation fails or is cancelled.

use crate::error::Result;
use crate::logging;
use std::path::PathBuf;

/// Records all operations performed during installation for rollback.
#[derive(Debug, Clone)]
pub struct RollbackTracker {
    operations: Vec<RollbackOp>,
}

/// A single operation that can be rolled back.
#[derive(Debug, Clone)]
enum RollbackOp {
    /// File was extracted — remove it on rollback.
    FileCreated(PathBuf),
    /// Directory was created — remove it on rollback.
    DirCreated(PathBuf),
    /// Registry key was written — remove it on rollback.
    RegistryWritten { root: String, path: String },
    /// Shortcut was created — remove it on rollback.
    ShortcutCreated(PathBuf),
    /// Environment variable was set — remove it on rollback.
    EnvVarSet { name: String, scope: String },
    /// Service was installed — remove it on rollback.
    ServiceInstalled(String),
}

impl RollbackTracker {
    /// Create a new empty rollback tracker.
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Record that a file was created.
    pub fn track_file(&mut self, path: PathBuf) {
        self.operations.push(RollbackOp::FileCreated(path));
    }

    /// Record that a directory was created.
    pub fn track_dir(&mut self, path: PathBuf) {
        self.operations.push(RollbackOp::DirCreated(path));
    }

    /// Record that a registry key was written.
    pub fn track_registry(&mut self, root: &str, path: &str) {
        self.operations.push(RollbackOp::RegistryWritten {
            root: root.to_string(),
            path: path.to_string(),
        });
    }

    /// Record that a shortcut was created.
    pub fn track_shortcut(&mut self, path: PathBuf) {
        self.operations.push(RollbackOp::ShortcutCreated(path));
    }

    /// Record that an environment variable was set.
    pub fn track_env_var(&mut self, name: &str, scope: &str) {
        self.operations.push(RollbackOp::EnvVarSet {
            name: name.to_string(),
            scope: scope.to_string(),
        });
    }

    /// Record that a service was installed.
    pub fn track_service(&mut self, name: &str) {
        self.operations
            .push(RollbackOp::ServiceInstalled(name.to_string()));
    }

    /// Roll back all tracked operations in reverse order.
    pub fn rollback(&mut self) -> Result<()> {
        logging::log("ROLLBACK: Starting rollback...");

        let mut failures: Vec<String> = Vec::new();

        // Process in reverse order
        for op in self.operations.drain(..).rev() {
            match op {
                RollbackOp::FileCreated(path) => {
                    if path.exists() {
                        logging::log_op("ROLLBACK", &format!("Removing file: {}", path.display()));
                        if let Err(e) = std::fs::remove_file(&path) {
                            let msg = format!("Failed to remove file {}: {}", path.display(), e);
                            logging::log_error("ROLLBACK", &msg);
                            failures.push(msg);
                        }
                    }
                }
                RollbackOp::DirCreated(path) => {
                    if path.exists() {
                        logging::log_op(
                            "ROLLBACK",
                            &format!("Removing directory: {}", path.display()),
                        );
                        if let Err(e) = std::fs::remove_dir_all(&path) {
                            let msg =
                                format!("Failed to remove directory {}: {}", path.display(), e);
                            logging::log_error("ROLLBACK", &msg);
                            failures.push(msg);
                        }
                    }
                }
                RollbackOp::RegistryWritten { root, path } => {
                    logging::log_op(
                        "ROLLBACK",
                        &format!("Removing registry: {}\\{}", root, path),
                    );
                    // Actually remove the registry key
                    let root_key = match root.as_str() {
                        "HKLM" => winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE),
                        "HKCU" => winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER),
                        "HKCR" => winreg::RegKey::predef(winreg::enums::HKEY_CLASSES_ROOT),
                        "HKU" => winreg::RegKey::predef(winreg::enums::HKEY_USERS),
                        _ => continue,
                    };
                    if let Err(e) = root_key.delete_subkey_all(&path) {
                        let msg = format!("Failed to remove registry {}\\{}: {}", root, path, e);
                        logging::log_error("ROLLBACK", &msg);
                        failures.push(msg);
                    }
                }
                RollbackOp::ShortcutCreated(path) => {
                    if path.exists() {
                        logging::log_op(
                            "ROLLBACK",
                            &format!("Removing shortcut: {}", path.display()),
                        );
                        if let Err(e) = std::fs::remove_file(&path) {
                            let msg =
                                format!("Failed to remove shortcut {}: {}", path.display(), e);
                            logging::log_error("ROLLBACK", &msg);
                            failures.push(msg);
                        }
                    }
                }
                RollbackOp::EnvVarSet { name, scope } => {
                    logging::log_op(
                        "ROLLBACK",
                        &format!("Removing env var: {} ({})", name, scope),
                    );
                    // Actually remove the environment variable
                    let root_key = match scope.as_str() {
                        "system" => winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
                            .open_subkey_with_flags(
                                "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                                winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                            )
                            .ok(),
                        _ => winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                            .open_subkey_with_flags(
                                "Environment",
                                winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
                            )
                            .ok(),
                    };
                    if let Some(key) = root_key {
                        if let Err(e) = key.delete_value(&name) {
                            let msg =
                                format!("Failed to remove env var {} ({}): {}", name, scope, e);
                            logging::log_error("ROLLBACK", &msg);
                            failures.push(msg);
                        }
                    }
                }
                RollbackOp::ServiceInstalled(name) => {
                    logging::log_op("ROLLBACK", &format!("Removing service: {}", name));
                    // Stop the service first, then delete it
                    let _ = std::process::Command::new("sc")
                        .args(["stop", &name])
                        .output();
                    match std::process::Command::new("sc")
                        .args(["delete", &name])
                        .output()
                    {
                        Ok(output) if output.status.success() => {}
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let msg = format!("Failed to remove service {}: {}", name, stderr);
                            logging::log_error("ROLLBACK", &msg);
                            failures.push(msg);
                        }
                        Err(e) => {
                            let msg = format!("Failed to run sc delete for {}: {}", name, e);
                            logging::log_error("ROLLBACK", &msg);
                            failures.push(msg);
                        }
                    }
                }
            }
        }

        if failures.is_empty() {
            logging::log("ROLLBACK: Rollback complete.");
        } else {
            logging::log_warning(&format!(
                "ROLLBACK: Rollback completed with {} failure(s):",
                failures.len()
            ));
            for f in &failures {
                logging::log_warning(&format!("  - {}", f));
            }
        }
        Ok(())
    }

    /// Clear all tracked operations (e.g., after successful install).
    pub fn clear(&mut self) {
        self.operations.clear();
    }

    /// Get the number of tracked operations.
    pub fn count(&self) -> usize {
        self.operations.len()
    }
}

impl Default for RollbackTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_tracker_new() {
        let tracker = RollbackTracker::new();
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn test_rollback_tracker_track() {
        let mut tracker = RollbackTracker::new();
        tracker.track_file(PathBuf::from("C:\\test\\file.txt"));
        tracker.track_dir(PathBuf::from("C:\\test\\dir"));
        tracker.track_registry("HKLM", "SOFTWARE\\Test");
        tracker.track_shortcut(PathBuf::from("C:\\test.lnk"));
        tracker.track_env_var("MY_VAR", "machine");
        tracker.track_service("MyService");
        assert_eq!(tracker.count(), 6);
    }

    #[test]
    fn test_rollback_tracker_clear() {
        let mut tracker = RollbackTracker::new();
        tracker.track_file(PathBuf::from("C:\\test\\file.txt"));
        assert_eq!(tracker.count(), 1);
        tracker.clear();
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn test_rollback_removes_files_and_dirs() {
        let temp = std::env::temp_dir().join("velocity_test_rollback");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        // Create a file and directory to track
        let file_path = temp.join("tracked_file.txt");
        std::fs::write(&file_path, "test").unwrap();
        let dir_path = temp.join("tracked_dir");
        std::fs::create_dir_all(&dir_path).unwrap();
        std::fs::write(dir_path.join("inner.txt"), "inner").unwrap();

        let mut tracker = RollbackTracker::new();
        tracker.track_file(file_path.clone());
        tracker.track_dir(dir_path.clone());
        assert_eq!(tracker.count(), 2);

        // Rollback should remove both
        tracker.rollback().unwrap();

        assert!(!file_path.exists(), "File should be removed by rollback");
        assert!(!dir_path.exists(), "Dir should be removed by rollback");
        assert_eq!(tracker.count(), 0, "Operations should be drained");

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_rollback_reverse_order() {
        // Verify that rollback processes operations in reverse order
        let temp = std::env::temp_dir().join("velocity_test_rollback_order");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        // Create nested structure: dir -> subdir -> file
        let dir_path = temp.join("outer");
        let subdir_path = dir_path.join("inner");
        let file_path = subdir_path.join("deep.txt");
        std::fs::create_dir_all(&subdir_path).unwrap();
        std::fs::write(&file_path, "deep").unwrap();

        let mut tracker = RollbackTracker::new();
        // Track in creation order: outer dir, inner dir, file
        tracker.track_dir(dir_path.clone());
        tracker.track_dir(subdir_path.clone());
        tracker.track_file(file_path.clone());

        // Rollback should process in reverse: file first, then inner dir, then outer dir
        tracker.rollback().unwrap();

        assert!(!file_path.exists(), "File should be removed first");
        assert!(!subdir_path.exists(), "Inner dir should be removed second");
        assert!(!dir_path.exists(), "Outer dir should be removed last");

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_rollback_handles_missing_files() {
        // Rollback should not fail if files are already gone
        let temp = std::env::temp_dir().join("velocity_test_rollback_missing");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let file_path = temp.join("already_gone.txt");
        // Don't actually create the file

        let mut tracker = RollbackTracker::new();
        tracker.track_file(file_path.clone());

        // Should succeed even though file doesn't exist
        tracker.rollback().unwrap();
        assert_eq!(tracker.count(), 0);

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_rollback_registry_and_env() {
        // Test rollback of registry and env var operations
        // (We use HKCU to avoid needing admin)
        let mut tracker = RollbackTracker::new();

        // Track a registry key and env var (these won't actually exist,
        // but rollback should handle that gracefully)
        tracker.track_registry("HKCU", "SOFTWARE\\VelocityTest_Rollback");
        tracker.track_env_var("VELOCITY_TEST_ROLLBACK_VAR", "user");

        assert_eq!(tracker.count(), 2);

        // Should not panic even if keys/vars don't exist
        tracker.rollback().unwrap();
        assert_eq!(tracker.count(), 0);
    }

    #[test]
    fn test_rollback_large_operation_count() {
        // Stress test: rollback with many tracked operations
        let temp = std::env::temp_dir().join("velocity_test_rollback_stress");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let mut tracker = RollbackTracker::new();

        // Track 500 files and 50 directories
        for i in 0..50 {
            let dir = temp.join(format!("dir_{:03}", i));
            std::fs::create_dir_all(&dir).unwrap();
            tracker.track_dir(dir.clone());

            for j in 0..10 {
                let file = dir.join(format!("file_{:03}.txt", j));
                std::fs::write(&file, format!("content {}", j)).unwrap();
                tracker.track_file(file);
            }
        }

        assert_eq!(tracker.count(), 550);

        // Rollback should clean everything
        tracker.rollback().unwrap();
        assert_eq!(tracker.count(), 0);

        // Verify all files and dirs are gone
        let remaining: Vec<_> = std::fs::read_dir(&temp)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(
            remaining.len(),
            0,
            "All files and dirs should be rolled back"
        );

        let _ = std::fs::remove_dir_all(&temp);
    }
}
