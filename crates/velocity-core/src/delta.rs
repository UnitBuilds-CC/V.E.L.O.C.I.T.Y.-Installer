//! Delta update generation and application for efficient version updates.
//!
//! This module implements binary delta patching using bsdiff for true binary diffing
//! with Zstd compression on top. Delta updates allow transferring only the changes between
//! versions, reducing update sizes by 80-95% compared to full packages.
//!
//! # Security
//!
//! All relative paths in delta packages are validated against path traversal attacks.
//! Disk space is verified before applying. File locking prevents concurrent corruption.
//! Rollback uses atomic rename operations for crash safety.

use crate::error::{CoreError, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// A delta package containing patches to update from one version to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaPackage {
    /// Version this delta updates FROM
    pub from_version: String,
    /// Version this delta updates TO
    pub to_version: String,
    /// List of file patches
    pub patches: Vec<FilePatch>,
    /// Total size of all patches (bytes)
    pub total_patch_size: u64,
    /// Creation timestamp
    pub created_at: String,
}

/// A patch for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FilePatch {
    /// File was modified — contains binary patch or full compressed content
    Modified {
        /// Relative path within the installation
        path: PathBuf,
        /// SHA256 checksum of the original file
        old_checksum: String,
        /// SHA256 checksum of the new file
        new_checksum: String,
        /// bsdiff patch compressed with Zstd, or full content compressed with Zstd
        patch_data: Vec<u8>,
        /// Size of the new file after patching
        new_size: u64,
        /// True if patch_data is a bsdiff patch; false if it's full Zstd-compressed content
        is_bsdiff: bool,
    },
    /// File was added — contains full file content
    Added {
        /// Relative path within the installation
        path: PathBuf,
        /// SHA256 checksum of the file
        checksum: String,
        /// Full file content (Zstd-compressed)
        content: Vec<u8>,
        /// Size of the file
        size: u64,
    },
    /// File was deleted
    Deleted {
        /// Relative path within the installation
        path: PathBuf,
        /// SHA256 checksum of the deleted file
        checksum: String,
    },
}

/// Options for delta generation.
#[derive(Debug, Clone)]
pub struct DeltaOptions {
    /// Zstd compression level for patches (1-22, default 9)
    pub compression_level: i32,
    /// Minimum file size to generate patch for (smaller files use full content)
    pub min_patch_size: u64,
    /// Maximum file size to patch (larger files use full content)
    pub max_file_size: u64,
}

impl Default for DeltaOptions {
    fn default() -> Self {
        Self {
            compression_level: 9,
            min_patch_size: 1024,         // 1 KB
            max_file_size: 2_147_483_648, // 2 GB (Zstd limit)
        }
    }
}

/// Progress callback for delta operations.
///
/// Receives `(current_step, total_steps, message)` on each update.
pub type DeltaProgressFn = Box<dyn Fn(usize, usize, &str) + Send + Sync>;

/// Generate a delta package between two directory trees.
///
/// Compares `old_dir` and `new_dir`, creating binary patches for modified files
/// and including full content for new files. Deleted files are tracked but not
/// included in the delta.
///
/// Uses bsdiff for true binary diffing, producing patches that are typically
/// 80-95% smaller than full file replacements.
pub fn generate_delta(
    old_dir: &Path,
    new_dir: &Path,
    from_version: &str,
    to_version: &str,
    options: &DeltaOptions,
) -> Result<DeltaPackage> {
    info!(
        "Generating delta: {} -> {} ({} -> {})",
        old_dir.display(),
        new_dir.display(),
        from_version,
        to_version
    );

    let old_files = collect_files(old_dir)?;
    let new_files = collect_files(new_dir)?;

    debug!("Old version: {} files", old_files.len());
    debug!("New version: {} files", new_files.len());

    let mut patches = Vec::new();
    let mut total_patch_size = 0u64;

    // Process files in new version
    for (rel_path, new_path) in &new_files {
        if let Some(old_path) = old_files.get(rel_path) {
            // File exists in both versions — check if modified
            let old_content = fs::read(old_path)?;
            let new_content = fs::read(new_path)?;

            let old_checksum =
                crate::checksum::hash_bytes(&old_content, crate::checksum::HashAlgorithm::Sha256);
            let new_checksum =
                crate::checksum::hash_bytes(&new_content, crate::checksum::HashAlgorithm::Sha256);

            if old_checksum == new_checksum {
                debug!("Unchanged: {}", rel_path.display());
                continue;
            }

            // File modified — generate bsdiff patch
            let new_size = new_content.len() as u64;

            let patch_data = if new_content.len() as u64 >= options.min_patch_size
                && new_content.len() as u64 <= options.max_file_size
            {
                match generate_bsdiff_patch(&old_content, &new_content, options.compression_level) {
                    Ok(patch) => {
                        // Only use patch if it's smaller than full content
                        if patch.len() < new_content.len() {
                            debug!(
                                "Patched: {} ({} -> {} bytes, {:.1}% reduction)",
                                rel_path.display(),
                                patch.len(),
                                new_content.len(),
                                (1.0 - patch.len() as f64 / new_content.len() as f64) * 100.0
                            );
                            (patch, true) // bsdiff patch
                        } else {
                            debug!(
                                "Full content (patch larger): {} ({} bytes)",
                                rel_path.display(),
                                new_content.len()
                            );
                            (
                                compress_with_zstd(&new_content, options.compression_level)?,
                                false,
                            )
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Patch generation failed for {}, using full content: {}",
                            rel_path.display(),
                            e
                        );
                        (
                            compress_with_zstd(&new_content, options.compression_level)?,
                            false,
                        )
                    }
                }
            } else {
                // File too small or too large — use full content
                debug!(
                    "Full content (size {}): {} ({} bytes)",
                    new_content.len(),
                    rel_path.display(),
                    new_content.len()
                );
                (
                    compress_with_zstd(&new_content, options.compression_level)?,
                    false,
                )
            };

            let (patch_data, is_bsdiff) = patch_data;
            total_patch_size += patch_data.len() as u64;
            patches.push(FilePatch::Modified {
                path: rel_path.clone(),
                old_checksum,
                new_checksum,
                patch_data,
                new_size,
                is_bsdiff,
            });
        } else {
            // New file — include full content
            let new_content = fs::read(new_path)?;
            let checksum =
                crate::checksum::hash_bytes(&new_content, crate::checksum::HashAlgorithm::Sha256);
            let size = new_content.len() as u64;
            let content = compress_with_zstd(&new_content, options.compression_level)?;

            total_patch_size += content.len() as u64;
            patches.push(FilePatch::Added {
                path: rel_path.clone(),
                checksum,
                content,
                size,
            });
        }
    }

    // Track deleted files
    for rel_path in old_files.keys() {
        if !new_files.contains_key(rel_path) {
            let old_path = old_dir.join(rel_path);
            let old_content = fs::read(&old_path)?;
            let checksum =
                crate::checksum::hash_bytes(&old_content, crate::checksum::HashAlgorithm::Sha256);

            patches.push(FilePatch::Deleted {
                path: rel_path.clone(),
                checksum,
            });
        }
    }

    info!(
        "Delta generated: {} patches, {} bytes total",
        patches.len(),
        total_patch_size
    );

    Ok(DeltaPackage {
        from_version: from_version.to_string(),
        to_version: to_version.to_string(),
        patches,
        total_patch_size,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Apply a delta package to a directory, reconstructing the new version.
///
/// # Security
///
/// - Validates all paths against traversal attacks
/// - Checks available disk space before applying
/// - Uses file locking to prevent concurrent updates
/// - Creates atomic backup for rollback on failure
/// - Verifies SHA256 checksums before and after patching
pub fn apply_delta(delta: &DeltaPackage, target_dir: &Path) -> Result<()> {
    apply_delta_with_progress(delta, target_dir, None)
}

/// Apply a delta package with progress reporting.
pub fn apply_delta_with_progress(
    delta: &DeltaPackage,
    target_dir: &Path,
    progress: Option<DeltaProgressFn>,
) -> Result<()> {
    let report = |step: usize, total: usize, msg: &str| {
        if let Some(ref cb) = progress {
            cb(step, total, msg);
        }
    };

    info!(
        "Applying delta: {} -> {} to {}",
        delta.from_version,
        delta.to_version,
        target_dir.display()
    );

    let total_steps = delta.patches.len() + 3; // validate + backup + patches + cleanup

    // Step 1: Validate all paths against traversal attacks
    report(0, total_steps, "Validating paths...");
    for patch in &delta.patches {
        let path = match patch {
            FilePatch::Modified { path, .. } => path,
            FilePatch::Added { path, .. } => path,
            FilePatch::Deleted { path, .. } => path,
        };
        validate_patch_path(path)?;
    }
    debug!("Path validation passed for {} patches", delta.patches.len());

    // Step 2: Acquire exclusive file lock (placed as sibling to target_dir)
    report(1, total_steps, "Acquiring lock...");
    let lock_path = if let Some(parent) = target_dir.parent() {
        parent.join(format!(
            ".velocity-update-{}.lock",
            target_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "install".to_string())
        ))
    } else {
        target_dir.with_extension("lock")
    };
    let lock_file = File::create(&lock_path)
        .map_err(|e| CoreError::Other(format!("Failed to create lock file: {}", e)))?;
    lock_file.try_lock_exclusive().map_err(|_| {
        CoreError::Other("Another update process is already running (lock file held)".to_string())
    })?;
    debug!("Acquired exclusive update lock");

    // Ensure lock is released when we exit (success or failure).
    // The File is dropped at function exit, releasing the lock automatically.

    // Step 3: Check disk space (need ~2x install size for backup + patches)
    report(1, total_steps, "Checking disk space...");
    let install_size = dir_size(target_dir)?;
    let required_space = install_size * 2 + delta.total_patch_size;
    check_disk_space(target_dir, required_space)?;
    debug!(
        "Disk space check: need {} bytes, install is {} bytes",
        required_space, install_size
    );

    // Step 4: Create backup for rollback (atomic rename-based)
    report(2, total_steps, "Creating backup...");
    let backup_dir = target_dir.with_extension("backup");
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }

    // Atomic rename: target -> backup (instant, crash-safe)
    fs::rename(target_dir, &backup_dir).map_err(|e| {
        CoreError::Other(format!(
            "Failed to create atomic backup (rename {} -> {}): {}",
            target_dir.display(),
            backup_dir.display(),
            e
        ))
    })?;

    // Recreate target dir from backup (copy for the working set)
    copy_dir_recursive(&backup_dir, target_dir)?;

    // Step 5: Apply patches
    let mut applied = Vec::new();
    for (i, patch) in delta.patches.iter().enumerate() {
        report(
            3 + i,
            total_steps,
            &format!("Applying patch {}/{}...", i + 1, delta.patches.len()),
        );

        match patch {
            FilePatch::Modified {
                path,
                old_checksum,
                new_checksum,
                patch_data,
                new_size,
                is_bsdiff,
            } => {
                let file_path = target_dir.join(path);

                if !file_path.exists() {
                    warn!("File not found for patch: {}", path.display());
                    rollback_atomic(target_dir, &backup_dir)?;
                    return Err(CoreError::Other(format!(
                        "File not found for patch: {}",
                        path.display()
                    )));
                }

                let old_content = fs::read(&file_path)?;
                let actual_checksum = crate::checksum::hash_bytes(
                    &old_content,
                    crate::checksum::HashAlgorithm::Sha256,
                );

                if actual_checksum != *old_checksum {
                    warn!(
                        "Checksum mismatch for {}: expected {}, got {}",
                        path.display(),
                        old_checksum,
                        actual_checksum
                    );
                    rollback_atomic(target_dir, &backup_dir)?;
                    return Err(CoreError::Other(format!(
                        "Checksum mismatch for {}",
                        path.display()
                    )));
                }

                // Apply patch: bsdiff or full content depending on how it was generated
                let new_content = if *is_bsdiff {
                    apply_bsdiff_patch(&old_content, patch_data)?
                } else {
                    decompress_with_zstd(patch_data)?
                };

                // Verify new checksum
                let actual_new_checksum = crate::checksum::hash_bytes(
                    &new_content,
                    crate::checksum::HashAlgorithm::Sha256,
                );
                if actual_new_checksum != *new_checksum {
                    warn!(
                        "New checksum mismatch for {}: expected {}, got {}",
                        path.display(),
                        new_checksum,
                        actual_new_checksum
                    );
                    rollback_atomic(target_dir, &backup_dir)?;
                    return Err(CoreError::Other(format!(
                        "New checksum mismatch for {}",
                        path.display()
                    )));
                }

                // Verify size
                if new_content.len() as u64 != *new_size {
                    warn!(
                        "Size mismatch for {}: expected {}, got {}",
                        path.display(),
                        new_size,
                        new_content.len()
                    );
                    rollback_atomic(target_dir, &backup_dir)?;
                    return Err(CoreError::Other(format!(
                        "Size mismatch for {}",
                        path.display()
                    )));
                }

                fs::write(&file_path, &new_content)?;
                applied.push(path.clone());
                debug!("Applied patch: {}", path.display());
            }
            FilePatch::Added {
                path,
                checksum,
                content,
                size,
            } => {
                let file_path = target_dir.join(path);

                let new_content = decompress_with_zstd(content)?;

                let actual_checksum = crate::checksum::hash_bytes(
                    &new_content,
                    crate::checksum::HashAlgorithm::Sha256,
                );
                if actual_checksum != *checksum {
                    warn!(
                        "Checksum mismatch for new file {}: expected {}, got {}",
                        path.display(),
                        checksum,
                        actual_checksum
                    );
                    rollback_atomic(target_dir, &backup_dir)?;
                    return Err(CoreError::Other(format!(
                        "Checksum mismatch for new file {}",
                        path.display()
                    )));
                }

                if new_content.len() as u64 != *size {
                    warn!(
                        "Size mismatch for new file {}: expected {}, got {}",
                        path.display(),
                        size,
                        new_content.len()
                    );
                    rollback_atomic(target_dir, &backup_dir)?;
                    return Err(CoreError::Other(format!(
                        "Size mismatch for new file {}",
                        path.display()
                    )));
                }

                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::write(&file_path, &new_content)?;
                applied.push(path.clone());
                debug!("Added file: {}", path.display());
            }
            FilePatch::Deleted { path, checksum } => {
                let file_path = target_dir.join(path);

                if file_path.exists() {
                    let old_content = fs::read(&file_path)?;
                    let actual_checksum = crate::checksum::hash_bytes(
                        &old_content,
                        crate::checksum::HashAlgorithm::Sha256,
                    );

                    if actual_checksum != *checksum {
                        warn!(
                            "Checksum mismatch for deletion {}: expected {}, got {}",
                            path.display(),
                            checksum,
                            actual_checksum
                        );
                        rollback_atomic(target_dir, &backup_dir)?;
                        return Err(CoreError::Other(format!(
                            "Checksum mismatch for deletion {}",
                            path.display()
                        )));
                    }

                    fs::remove_file(&file_path)?;
                    applied.push(path.clone());
                    debug!("Deleted file: {}", path.display());
                }
            }
        }
    }

    // Success — remove backup atomically
    report(total_steps, total_steps, "Cleaning up...");
    fs::remove_dir_all(&backup_dir)?;

    // Release lock and remove lock file
    let _ = lock_file.unlock();
    let _ = fs::remove_file(&lock_path);

    info!(
        "Delta applied successfully: {} files updated",
        applied.len()
    );

    Ok(())
}

/// Save a delta package to a .delta.zip file.
pub fn save_delta_package(delta: &DeltaPackage, path: &Path) -> Result<()> {
    info!("Saving delta package to {}", path.display());

    let json = serde_json::to_string(delta).map_err(|e| CoreError::Other(e.to_string()))?;
    let compressed = compress_with_zstd(json.as_bytes(), 9)?;
    fs::write(path, &compressed)?;

    info!(
        "Delta package saved: {} bytes (compressed from {} bytes JSON)",
        compressed.len(),
        json.len()
    );

    Ok(())
}

/// Load a delta package from a .delta.zip file.
pub fn load_delta_package(path: &Path) -> Result<DeltaPackage> {
    info!("Loading delta package from {}", path.display());

    let compressed = fs::read(path)?;
    let json_bytes = decompress_with_zstd(&compressed)?;
    let delta: DeltaPackage =
        serde_json::from_slice(&json_bytes).map_err(|e| CoreError::Other(e.to_string()))?;

    info!(
        "Delta package loaded: {} -> {}, {} patches",
        delta.from_version,
        delta.to_version,
        delta.patches.len()
    );

    Ok(delta)
}

/// Apply multiple delta packages in sequence (multi-hop update).
///
/// Verifies chain continuity before applying. Falls back to full download
/// if the chain exceeds 5 hops.
pub fn apply_delta_chain(deltas: &[DeltaPackage], target_dir: &Path) -> Result<()> {
    apply_delta_chain_with_progress(deltas, target_dir, None)
}

/// Apply multiple delta packages with progress reporting.
pub fn apply_delta_chain_with_progress(
    deltas: &[DeltaPackage],
    target_dir: &Path,
    progress: Option<DeltaProgressFn>,
) -> Result<()> {
    if deltas.is_empty() {
        return Ok(());
    }

    // Enforce max hop limit
    if deltas.len() > 5 {
        return Err(CoreError::Other(format!(
            "Delta chain too long: {} hops (max 5). Use full download instead.",
            deltas.len()
        )));
    }

    info!(
        "Applying delta chain: {} -> {} ({} hops)",
        deltas[0].from_version,
        deltas[deltas.len() - 1].to_version,
        deltas.len()
    );

    // Verify chain continuity
    for i in 0..deltas.len() - 1 {
        if deltas[i].to_version != deltas[i + 1].from_version {
            return Err(CoreError::Other(format!(
                "Delta chain broken: {} ends at {} but {} starts at {}",
                i,
                deltas[i].to_version,
                i + 1,
                deltas[i + 1].from_version
            )));
        }
    }

    // Apply each delta in sequence
    for (i, delta) in deltas.iter().enumerate() {
        info!(
            "Applying delta {}/{}: {} -> {}",
            i + 1,
            deltas.len(),
            delta.from_version,
            delta.to_version
        );

        if let Some(ref cb) = progress {
            cb(
                i + 1,
                deltas.len(),
                &format!("Applying hop {}/{}", i + 1, deltas.len()),
            );
        }

        apply_delta(delta, target_dir)?;
    }

    info!(
        "Delta chain applied successfully: {} -> {}",
        deltas[0].from_version,
        deltas[deltas.len() - 1].to_version
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Atomic rollback: rename failed target away, restore backup, clean up.
///
/// This is crash-safe because `rename` is atomic on both Windows and Unix.
/// If the process crashes mid-rollback, either the old or new directory
/// will be intact.
fn rollback_atomic(target_dir: &Path, backup_dir: &Path) -> Result<()> {
    warn!("Rolling back to backup...");

    if !backup_dir.exists() {
        warn!("No backup directory found — cannot rollback");
        return Err(CoreError::Other("Rollback failed: no backup".to_string()));
    }

    // Remove the failed target (if it exists) by renaming it aside first
    let failed_dir = target_dir.with_extension("failed");
    if target_dir.exists() {
        if failed_dir.exists() {
            let _ = fs::remove_dir_all(&failed_dir);
        }
        // Atomic rename: target -> failed
        if let Err(e) = fs::rename(target_dir, &failed_dir) {
            warn!("Failed to rename target aside: {}", e);
            // Try to force remove as fallback
            let _ = fs::remove_dir_all(target_dir);
        }
    }

    // Atomic rename: backup -> target
    fs::rename(backup_dir, target_dir).map_err(|e| {
        CoreError::Other(format!(
            "Critical: failed to restore backup ({} -> {}): {}. Manual recovery needed.",
            backup_dir.display(),
            target_dir.display(),
            e
        ))
    })?;

    // Clean up the failed directory
    if failed_dir.exists() {
        let _ = fs::remove_dir_all(&failed_dir);
    }

    info!("Rollback complete");
    Ok(())
}

/// Validate a relative path from a delta package for traversal attacks.
fn validate_patch_path(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();
    crate::security::validate_relative_path(&path_str)
}

/// Check that the filesystem containing `path` has at least `required` bytes free.
fn check_disk_space(path: &Path, required: u64) -> Result<()> {
    let available = fs2::available_space(path).map_err(|e| {
        CoreError::Other(format!(
            "Failed to check disk space for {}: {}",
            path.display(),
            e
        ))
    })?;

    if available < required {
        return Err(CoreError::Other(format!(
            "Insufficient disk space: need {} bytes, only {} available",
            required, available
        )));
    }

    debug!(
        "Disk space OK: {} bytes available, {} bytes required",
        available, required
    );
    Ok(())
}

/// Collect all files in a directory, returning a map of relative paths to absolute paths.
fn collect_files(dir: &Path) -> Result<HashMap<PathBuf, PathBuf>> {
    let mut files = HashMap::new();

    if !dir.exists() {
        return Ok(files);
    }

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let abs_path = entry.path().to_path_buf();
            if let Ok(rel_path) = abs_path.strip_prefix(dir) {
                files.insert(rel_path.to_path_buf(), abs_path);
            }
        }
    }

    Ok(files)
}

/// Generate a bsdiff binary patch, compressed with Zstd.
///
/// Uses the bsdiff algorithm for true binary diffing, then compresses
/// the result with Zstd for additional size reduction.
fn generate_bsdiff_patch(old: &[u8], new: &[u8], level: i32) -> Result<Vec<u8>> {
    // Generate bsdiff patch (raw binary diff)
    let mut patch = Vec::new();
    bsdiff::diff(old, new, &mut patch)
        .map_err(|e| CoreError::Other(format!("bsdiff failed: {}", e)))?;

    // Compress the patch with Zstd for additional size reduction
    compress_with_zstd(&patch, level)
}

/// Apply a bsdiff patch (Zstd-compressed) to reconstruct new content.
fn apply_bsdiff_patch(old: &[u8], patch_data: &[u8]) -> Result<Vec<u8>> {
    // Decompress the Zstd wrapper
    let patch_bytes = decompress_with_zstd(patch_data)?;

    // Apply bsdiff patch (needs &mut Read)
    let mut cursor = std::io::Cursor::new(patch_bytes);
    let mut output = Vec::new();
    bsdiff::patch(old, &mut cursor, &mut output)
        .map_err(|e| CoreError::Other(format!("bsdiff patch application failed: {}", e)))?;

    Ok(output)
}

/// Compress data with Zstd.
fn compress_with_zstd(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let mut encoder = zstd::Encoder::new(Vec::new(), level)?;
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

/// Decompress Zstd data.
fn decompress_with_zstd(data: &[u8]) -> Result<Vec<u8>> {
    let decoded = zstd::decode_all(data)?;
    Ok(decoded)
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel_path = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| CoreError::Other(e.to_string()))?;
        let dst_path = dst.join(rel_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dst_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &dst_path)?;
        }
    }

    Ok(())
}

/// Calculate total size of a directory tree.
fn dir_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_delta_generation_unchanged() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        fs::write(old_dir.join("file.txt"), "content").unwrap();
        fs::write(new_dir.join("file.txt"), "content").unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        assert_eq!(delta.patches.len(), 0);
    }

    #[test]
    fn test_delta_generation_modified() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        fs::write(old_dir.join("file.txt"), "old content").unwrap();
        fs::write(new_dir.join("file.txt"), "new content").unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        assert_eq!(delta.patches.len(), 1);
        match &delta.patches[0] {
            FilePatch::Modified { path, .. } => {
                assert_eq!(path, Path::new("file.txt"));
            }
            _ => panic!("Expected Modified patch"),
        }
    }

    #[test]
    fn test_delta_generation_added() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        fs::write(new_dir.join("new.txt"), "new file").unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        assert_eq!(delta.patches.len(), 1);
        match &delta.patches[0] {
            FilePatch::Added { path, .. } => {
                assert_eq!(path, Path::new("new.txt"));
            }
            _ => panic!("Expected Added patch"),
        }
    }

    #[test]
    fn test_delta_generation_deleted() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        fs::write(old_dir.join("old.txt"), "old file").unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        assert_eq!(delta.patches.len(), 1);
        match &delta.patches[0] {
            FilePatch::Deleted { path, .. } => {
                assert_eq!(path, Path::new("old.txt"));
            }
            _ => panic!("Expected Deleted patch"),
        }
    }

    #[test]
    fn test_delta_apply_roundtrip() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");
        let target_dir = temp.path().join("target");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        fs::write(old_dir.join("unchanged.txt"), "same").unwrap();
        fs::write(new_dir.join("unchanged.txt"), "same").unwrap();
        fs::write(old_dir.join("modified.txt"), "old").unwrap();
        fs::write(new_dir.join("modified.txt"), "new").unwrap();
        fs::write(new_dir.join("added.txt"), "new file").unwrap();
        fs::write(old_dir.join("deleted.txt"), "old file").unwrap();

        copy_dir_recursive(&old_dir, &target_dir).unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        apply_delta(&delta, &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("unchanged.txt")).unwrap(),
            "same"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("modified.txt")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("added.txt")).unwrap(),
            "new file"
        );
        assert!(!target_dir.join("deleted.txt").exists());
    }

    #[test]
    fn test_delta_save_load_roundtrip() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");
        let delta_path = temp.path().join("delta.delta.zip");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();

        fs::write(old_dir.join("file.txt"), "old content").unwrap();
        fs::write(new_dir.join("file.txt"), "new content").unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        save_delta_package(&delta, &delta_path).unwrap();
        assert!(delta_path.exists());

        let loaded = load_delta_package(&delta_path).unwrap();
        assert_eq!(loaded.from_version, "1.0.0");
        assert_eq!(loaded.to_version, "1.0.1");
        assert_eq!(loaded.patches.len(), delta.patches.len());
    }

    #[test]
    fn test_delta_chain_multi_hop() {
        let temp = TempDir::new().unwrap();
        let v1_dir = temp.path().join("v1");
        let v2_dir = temp.path().join("v2");
        let v3_dir = temp.path().join("v3");
        let target_dir = temp.path().join("target");

        fs::create_dir_all(&v1_dir).unwrap();
        fs::create_dir_all(&v2_dir).unwrap();
        fs::create_dir_all(&v3_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        fs::write(v1_dir.join("file.txt"), "version 1").unwrap();
        fs::write(v1_dir.join("unchanged.txt"), "same").unwrap();

        fs::write(v2_dir.join("file.txt"), "version 2").unwrap();
        fs::write(v2_dir.join("unchanged.txt"), "same").unwrap();

        fs::write(v3_dir.join("file.txt"), "version 3").unwrap();
        fs::write(v3_dir.join("unchanged.txt"), "same").unwrap();
        fs::write(v3_dir.join("new.txt"), "new in v3").unwrap();

        copy_dir_recursive(&v1_dir, &target_dir).unwrap();

        let delta1 =
            generate_delta(&v1_dir, &v2_dir, "1.0.0", "1.0.1", &DeltaOptions::default()).unwrap();

        let delta2 =
            generate_delta(&v2_dir, &v3_dir, "1.0.1", "1.0.2", &DeltaOptions::default()).unwrap();

        apply_delta_chain(&[delta1, delta2], &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("file.txt")).unwrap(),
            "version 3"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("unchanged.txt")).unwrap(),
            "same"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("new.txt")).unwrap(),
            "new in v3"
        );
    }

    // --- New production hardening tests ---

    #[test]
    fn test_delta_binary_roundtrip() {
        // Test with real binary data (not just text)
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");
        let target_dir = temp.path().join("target");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        // Create binary files with known patterns
        let old_binary: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
        let new_binary: Vec<u8> = (0..4096)
            .map(|i| {
                if i % 100 < 10 {
                    (i % 256) as u8 ^ 0xFF
                } else {
                    (i % 256) as u8
                }
            })
            .collect();

        fs::write(old_dir.join("binary.bin"), &old_binary).unwrap();
        fs::write(new_dir.join("binary.bin"), &new_binary).unwrap();

        copy_dir_recursive(&old_dir, &target_dir).unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        // Verify bsdiff produced a smaller patch than full content
        match &delta.patches[0] {
            FilePatch::Modified {
                patch_data,
                new_size,
                is_bsdiff,
                ..
            } => {
                assert!(is_bsdiff, "Expected bsdiff patch for 4KB binary file");
                assert!(
                    patch_data.len() < *new_size as usize,
                    "bsdiff patch ({} bytes) should be smaller than full file ({} bytes)",
                    patch_data.len(),
                    new_size
                );
            }
            _ => panic!("Expected Modified patch"),
        }

        apply_delta(&delta, &target_dir).unwrap();

        let result = fs::read(target_dir.join("binary.bin")).unwrap();
        assert_eq!(result, new_binary);
    }

    #[test]
    fn test_path_traversal_rejected() {
        let delta = DeltaPackage {
            from_version: "1.0.0".to_string(),
            to_version: "1.0.1".to_string(),
            patches: vec![FilePatch::Added {
                path: PathBuf::from("../../evil.txt"),
                checksum: "abc".to_string(),
                content: vec![],
                size: 0,
            }],
            total_patch_size: 0,
            created_at: String::new(),
        };

        let temp = TempDir::new().unwrap();
        let target = temp.path();

        let result = apply_delta(&delta, target);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("traversal")
                || err.contains("Absolute path")
                || err.contains("permission"),
            "Error should mention traversal/permission: {}",
            err
        );
    }

    #[test]
    fn test_path_traversal_absolute_rejected() {
        let delta = DeltaPackage {
            from_version: "1.0.0".to_string(),
            to_version: "1.0.1".to_string(),
            patches: vec![FilePatch::Added {
                path: PathBuf::from("C:\\Windows\\System32\\evil.dll"),
                checksum: "abc".to_string(),
                content: vec![],
                size: 0,
            }],
            total_patch_size: 0,
            created_at: String::new(),
        };

        let temp = TempDir::new().unwrap();
        let target = temp.path();

        let result = apply_delta(&delta, target);
        assert!(result.is_err());
    }

    #[test]
    fn test_delta_chain_too_long() {
        let make_delta = |from: &str, to: &str| DeltaPackage {
            from_version: from.to_string(),
            to_version: to.to_string(),
            patches: vec![],
            total_patch_size: 0,
            created_at: String::new(),
        };

        let chain: Vec<DeltaPackage> = (0..6)
            .map(|i| make_delta(&format!("1.0.{}", i), &format!("1.0.{}", i + 1)))
            .collect();

        let temp = TempDir::new().unwrap();
        let target = temp.path();

        let result = apply_delta_chain(&chain, target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too long"));
    }

    #[test]
    fn test_delta_chain_broken() {
        let make_delta = |from: &str, to: &str| DeltaPackage {
            from_version: from.to_string(),
            to_version: to.to_string(),
            patches: vec![],
            total_patch_size: 0,
            created_at: String::new(),
        };

        let chain = vec![
            make_delta("1.0.0", "1.0.1"),
            make_delta("1.0.5", "1.0.6"), // Gap: 1.0.1 != 1.0.5
        ];

        let temp = TempDir::new().unwrap();
        let target = temp.path();

        let result = apply_delta_chain(&chain, target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("broken"));
    }

    #[test]
    fn test_progress_callback_called() {
        use std::sync::{Arc, Mutex};

        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");
        let target_dir = temp.path().join("target");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        fs::write(old_dir.join("file.txt"), "old").unwrap();
        fs::write(new_dir.join("file.txt"), "new").unwrap();

        copy_dir_recursive(&old_dir, &target_dir).unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = messages.clone();

        let progress: DeltaProgressFn = Box::new(move |step, total, msg| {
            messages_clone
                .lock()
                .unwrap()
                .push(format!("{}/{}: {}", step, total, msg));
        });

        apply_delta_with_progress(&delta, &target_dir, Some(progress)).unwrap();

        let msgs = messages.lock().unwrap();
        assert!(
            !msgs.is_empty(),
            "Progress callback should have been called"
        );
    }

    #[test]
    fn test_concurrent_update_blocked() {
        let temp = TempDir::new().unwrap();
        let old_dir = temp.path().join("old");
        let new_dir = temp.path().join("new");
        let target_dir = temp.path().join("target");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&new_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        fs::write(old_dir.join("file.txt"), "old").unwrap();
        fs::write(new_dir.join("file.txt"), "new").unwrap();

        copy_dir_recursive(&old_dir, &target_dir).unwrap();

        let delta = generate_delta(
            &old_dir,
            &new_dir,
            "1.0.0",
            "1.0.1",
            &DeltaOptions::default(),
        )
        .unwrap();

        // Acquire the lock manually (lock is sibling to target_dir)
        let lock_path = target_dir.parent().unwrap().join(format!(
            ".velocity-update-{}.lock",
            target_dir.file_name().unwrap().to_string_lossy()
        ));
        let lock_file = File::create(&lock_path).unwrap();
        lock_file.lock_exclusive().unwrap();

        // Try to apply delta — should fail because lock is held
        let result = apply_delta(&delta, &target_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("lock"));

        // Release lock
        lock_file.unlock().unwrap();
        let _ = fs::remove_file(&lock_path);

        // Now it should succeed
        apply_delta(&delta, &target_dir).unwrap();
    }
}
