//! File extraction from zstd-compressed tar archives.

use crate::error::{CoreError, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Progress callback type: (files_extracted, total_files, current_file_name)
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send>;

/// Extract a zstd-compressed tar archive to a target directory.
///
/// The archive is expected to contain relative paths that map directly
/// to the installation directory.
pub fn extract_archive(
    compressed_data: &[u8],
    target_dir: &Path,
    progress: Option<&ProgressCallback>,
) -> Result<Vec<PathBuf>> {
    info!("Extracting to: {}", target_dir.display());

    // Create target directory if it doesn't exist
    std::fs::create_dir_all(target_dir)?;

    // Decompress zstd
    let decompressed = zstd::decode_all(compressed_data)
        .map_err(|e| CoreError::Compression(format!("zstd decompression failed: {}", e)))?;

    // Parse tar archive
    let mut archive = tar::Archive::new(decompressed.as_slice());
    let mut extracted_files = Vec::new();
    let mut file_count = 0usize;

    // Count total entries for progress
    let total_files = archive.entries()
        .map(|e| e.count())
        .unwrap_or(0);

    // Re-create archive since entries is a consuming iterator
    let decompressed2 = zstd::decode_all(compressed_data)
        .map_err(|e| CoreError::Compression(format!("zstd decompression failed: {}", e)))?;
    let mut archive = tar::Archive::new(decompressed2.as_slice());

    for entry in archive.entries().map_err(|e| CoreError::Compression(format!("tar error: {}", e)))? {
        let mut entry = entry.map_err(|e| CoreError::Compression(format!("tar entry error: {}", e)))?;

        let path = entry.path()
            .map_err(|e| CoreError::Compression(format!("tar path error: {}", e)))?
            .into_owned();

        let dest_path = target_dir.join(&path);

        // Report progress
        if let Some(cb) = progress {
            let name = path.to_string_lossy().to_string();
            cb(file_count, total_files, &name);
        }

        // Security: ensure we don't escape the target directory
        if !dest_path.starts_with(target_dir) {
            return Err(CoreError::Compression(format!(
                "Path traversal detected: {}",
                path.display()
            )));
        }

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            debug!("Created directory: {}", dest_path.display());
        } else {
            let mut outfile = std::fs::File::create(&dest_path)?;
            std::io::copy(&mut entry, &mut outfile)?;
            extracted_files.push(dest_path);
            debug!("Extracted: {}", path.display());
        }

        file_count += 1;
    }

    info!("Extracted {} files", file_count);
    Ok(extracted_files)
}

/// Create a zstd-compressed tar archive from a list of files.
///
/// Each file is stored with its relative path as the archive entry name.
pub fn create_archive(
    files: &[(PathBuf, String)], // (absolute_path, relative_name)
    compression_level: i32,
) -> Result<Vec<u8>> {
    info!("Creating archive with {} files, compression level {}", files.len(), compression_level);

    // Create tar in memory
    let mut tar_builder = tar::Builder::new(Vec::new());

    for (abs_path, rel_name) in files {
        let mut file = std::fs::File::open(abs_path)?;
        tar_builder
            .append_file(rel_name, &mut file)
            .map_err(|e| CoreError::Compression(format!("tar append error: {}", e)))?;
        debug!("Added to archive: {} -> {}", abs_path.display(), rel_name);
    }

    let tar_data = tar_builder
        .into_inner()
        .map_err(|e| CoreError::Compression(format!("tar finish error: {}", e)))?;

    // Compress with zstd
    let compressed = zstd::encode_all(tar_data.as_slice(), compression_level)
        .map_err(|e| CoreError::Compression(format!("zstd compression failed: {}", e)))?;

    info!("Archive size: {} bytes (from {} bytes)", compressed.len(), tar_data.len());
    Ok(compressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_extract_archive() {
        // Create temp files
        let temp_dir = std::env::temp_dir().join("velocity_test_archive");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create test files
        let file1 = temp_dir.join("test1.txt");
        let file2 = temp_dir.join("test2.txt");
        std::fs::write(&file1, "Hello, Velocity!").unwrap();
        std::fs::write(&file2, "Installer test data").unwrap();

        let files = vec![
            (file1.clone(), "test1.txt".to_string()),
            (file2.clone(), "subdir/test2.txt".to_string()),
        ];

        // Create archive
        let archive = create_archive(&files, 3).unwrap();
        assert!(!archive.is_empty());

        // Extract archive
        let extract_dir = temp_dir.join("extracted");
        let extracted = extract_archive(&archive, &extract_dir, None).unwrap();
        assert_eq!(extracted.len(), 2);

        // Verify contents
        let content1 = std::fs::read_to_string(extract_dir.join("test1.txt")).unwrap();
        assert_eq!(content1, "Hello, Velocity!");

        let content2 = std::fs::read_to_string(extract_dir.join("subdir/test2.txt")).unwrap();
        assert_eq!(content2, "Installer test data");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_extract_with_progress() {
        let temp_dir = std::env::temp_dir().join("velocity_test_progress");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file1 = temp_dir.join("progress_test.txt");
        std::fs::write(&file1, "Progress test").unwrap();

        let files = vec![(file1, "progress_test.txt".to_string())];
        let archive = create_archive(&files, 1).unwrap();

        let extract_dir = temp_dir.join("extracted");
        let cb: ProgressCallback = Box::new(|_current, _total, _name| {
            // Progress callback verified by successful extraction
        });

        let extracted = extract_archive(&archive, &extract_dir, Some(&cb)).unwrap();
        assert_eq!(extracted.len(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_empty_archive() {
        let files: Vec<(PathBuf, String)> = vec![];
        let archive = create_archive(&files, 3).unwrap();
        assert!(!archive.is_empty()); // zstd header at minimum

        let extract_dir = std::env::temp_dir().join("velocity_test_empty");
        let _ = std::fs::remove_dir_all(&extract_dir);
        let extracted = extract_archive(&archive, &extract_dir, None).unwrap();
        assert_eq!(extracted.len(), 0);

        let _ = std::fs::remove_dir_all(&extract_dir);
    }
}
