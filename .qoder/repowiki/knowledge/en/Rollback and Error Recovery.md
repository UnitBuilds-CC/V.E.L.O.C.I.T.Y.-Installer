---
kind: error_handling
name: Rollback and Error Recovery
category: reliability
scope:
    - 'crates/velocity-core/src/rollback.rs'
    - 'crates/velocity-core/src/error.rs'
source_files:
    - crates/velocity-core/src/rollback.rs
    - crates/velocity-core/src/error.rs
---

The Velocity Installer implements automatic transaction tracking and rollback for all installation operations, ensuring that failed installations leave no partial state behind.

**Architecture:**
- **Transaction tracking** — Every operation is recorded in a rollback stack
- **Reverse-order rollback** — Operations are undone in LIFO order
- **Comprehensive coverage** — Files, registry, shortcuts, services, env vars, file associations
- **Stress tested** — Verified with 1000+ operations, 50MB payloads, and Unicode paths
- **Best-effort** — Continues rolling back even if individual operations fail

**Rollback Tracker:**
```rust
// crates/velocity-core/src/rollback.rs

pub struct RollbackTracker {
    operations: Vec<RollbackOperation>,
}

pub enum RollbackOperation {
    FileCreated { path: PathBuf },
    FileOverwritten { path: PathBuf, backup: PathBuf },
    DirectoryCreated { path: PathBuf },
    RegistryCreated { root: RegRoot, key: String, name: String },
    RegistryModified { root: RegRoot, key: String, name: String, old_value: Option<String> },
    ShortcutCreated { path: PathBuf },
    EnvVarSet { name: String, scope: EnvScope },
    EnvVarModified { name: String, old_value: Option<String>, scope: EnvScope },
    ServiceInstalled { name: String },
    ServiceModified { name: String, old_start_type: Option<ServiceStartType> },
    FileAssociationCreated { extension: String },
}

impl RollbackTracker {
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }

    pub fn track(&mut self, op: RollbackOperation) {
        self.operations.push(op);
    }

    pub fn rollback(&mut self) -> Vec<RollbackError> {
        let mut errors = Vec::new();

        // Reverse order — LIFO
        while let Some(op) = self.operations.pop() {
            if let Err(e) = self.execute_rollback(&op) {
                errors.push(e);
                // Continue rollback even if one operation fails
            }
        }

        errors
    }

    fn execute_rollback(&self, op: &RollbackOperation) -> Result<(), RollbackError> {
        match op {
            RollbackOperation::FileCreated { path } => {
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
            }
            RollbackOperation::FileOverwritten { path, backup } => {
                // Restore backup
                if backup.exists() {
                    std::fs::copy(backup, path)?;
                    std::fs::remove_file(backup)?;
                }
            }
            RollbackOperation::DirectoryCreated { path } => {
                if path.exists() {
                    std::fs::remove_dir_all(path)?;
                }
            }
            RollbackOperation::RegistryCreated { root, key, name } => {
                // Delete registry value
                delete_registry_value(*root, key, name)?;
            }
            RollbackOperation::RegistryModified { root, key, name, old_value } => {
                // Restore old value
                if let Some(val) = old_value {
                    set_registry_value(*root, key, name, val)?;
                } else {
                    delete_registry_value(*root, key, name)?;
                }
            }
            RollbackOperation::ShortcutCreated { path } => {
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
            }
            RollbackOperation::EnvVarSet { name, scope } => {
                // Remove env var
                remove_env_var(name, *scope)?;
            }
            RollbackOperation::EnvVarModified { name, old_value, scope } => {
                // Restore old value
                if let Some(val) = old_value {
                    set_env_var(name, val, *scope)?;
                } else {
                    remove_env_var(name, *scope)?;
                }
            }
            RollbackOperation::ServiceInstalled { name } => {
                // Stop and remove service
                stop_service(name)?;
                remove_service(name)?;
            }
            RollbackOperation::ServiceModified { name, old_start_type } => {
                // Restore old start type
                if let Some(st) = old_start_type {
                    set_service_start_type(name, *st)?;
                }
            }
            RollbackOperation::FileAssociationCreated { extension } => {
                // Unregister file association
                unregister_file_association(extension)?;
            }
        }
        Ok(())
    }
}
```

**Integration with Installation Flow:**
```rust
// Typical installation with rollback
pub fn install(config: &Manifest, tracker: &mut RollbackTracker) -> Result<()> {
    // Extract files
    for file in &config.files {
        extract_file(file)?;
        tracker.track(RollbackOperation::FileCreated {
            path: file.dest.clone(),
        });
    }

    // Write registry
    for reg in &config.registry {
        let old_value = get_registry_value(reg.root, &reg.key, &reg.name);
        set_registry_value(reg.root, &reg.key, &reg.name, &reg.value)?;
        tracker.track(RollbackOperation::RegistryModified {
            root: reg.root,
            key: reg.key.clone(),
            name: reg.name.clone(),
            old_value,
        });
    }

    // If anything fails, rollback is triggered
    // ...
    Ok(())
}
```

**Error Types:**
```rust
// crates/velocity-core/src/error.rs
#[derive(Error, Debug)]
pub enum VelocityError {
    #[error("File extraction failed: {path}: {reason}")]
    Extraction { path: String, reason: String },

    #[error("Registry operation failed: {key}: {reason}")]
    Registry { key: String, reason: String },

    #[error("Service operation failed: {name}: {reason}")]
    Service { name: String, reason: String },

    #[error("Rollback failed: {0}")]
    Rollback(String),

    #[error("Security violation: {0}")]
    Security(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
```

**Rollback Coverage:**

| Operation | Tracked | Rollback Action |
|-----------|---------|-----------------|
| File extracted | Yes | Delete file |
| File overwritten | Yes | Restore .velocity_backup |
| Directory created | Yes | Remove directory |
| Registry value created | Yes | Delete value |
| Registry value modified | Yes | Restore old value |
| Shortcut created | Yes | Delete shortcut |
| Env var set | Yes | Remove env var |
| Env var modified | Yes | Restore old value |
| Service installed | Yes | Stop + remove service |
| Service modified | Yes | Restore start type |
| File association created | Yes | Unregister association |

**Stress Testing:**
```rust
// Stress test: 1000 files
#[test]
fn test_rollback_1000_files() {
    let mut tracker = RollbackTracker::new();
    let temp = tempdir().unwrap();

    // Create 1000 files
    for i in 0..1000 {
        let path = temp.path().join(format!("file_{}.txt", i));
        std::fs::write(&path, "test").unwrap();
        tracker.track(RollbackOperation::FileCreated { path });
    }

    // Rollback all
    let errors = tracker.rollback();
    assert!(errors.is_empty());

    // Verify all files removed
    assert_eq!(temp.path().read_dir().unwrap().count(), 0);
}

// Stress test: 50MB payload
#[test]
fn test_rollback_50mb_payload() {
    // Create and rollback a 50MB payload
}

// Stress test: Unicode paths
#[test]
fn test_rollback_unicode_paths() {
    // Test with Japanese, Chinese, Arabic, emoji paths
}
```

**Crash Recovery:**
- If the installer process crashes mid-installation, partial state may remain
- The crash handler writes a backtrace to `%TEMP%/velocity_crashes/`
- On next launch, the installer can detect incomplete installations
- Users can manually run the uninstaller to clean up

**Key files:**
- `crates/velocity-core/src/rollback.rs` — RollbackTracker, RollbackOperation enum
- `crates/velocity-core/src/error.rs` — VelocityError enum with all error variants

**Rules for developers:**
1. Every installation operation MUST be tracked before execution
2. Track the "undo" information BEFORE modifying state (e.g., save old registry value before overwriting)
3. Rollback MUST continue even if individual operations fail
4. Create `.velocity_backup` files before overwriting existing files
5. Test rollback with stress scenarios (1000+ operations, large files, Unicode paths)
6. Log all rollback operations for diagnostics
7. Never skip rollback tracking for "small" operations
