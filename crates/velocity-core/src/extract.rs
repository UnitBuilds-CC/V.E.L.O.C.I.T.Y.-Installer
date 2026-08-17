//! File extraction from compressed tar archives.
//!
//! Supports multiple compression formats:
//! - **zstd** (default) — fast decompression, good compression ratio
//! - **lzma2** — slower but smaller installers (Inno Setup compatible)
//!
//! The format is auto-detected from magic bytes in the compressed data.

use crate::error::{CoreError, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Supported compression formats for the payload archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionFormat {
    /// Zstandard — fast, good ratio (default)
    #[default]
    Zstd,
    /// LZMA2 — slower, better ratio (Inno Setup compatible)
    Lzma2,
}

impl CompressionFormat {
    /// Detect compression format from magic bytes.
    pub fn detect(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        // zstd magic: 0x28 0xB5 0x2F 0xFD
        if data[0] == 0x28 && data[1] == 0xB5 && data[2] == 0x2F && data[3] == 0xFD {
            return Some(CompressionFormat::Zstd);
        }
        // LZMA/XZ magic: 0xFD 0x37 0x7A 0x58 0x5A 0x00 (XZ format)
        if data.len() >= 6
            && data[0] == 0xFD
            && data[1] == 0x37
            && data[2] == 0x7A
            && data[3] == 0x58
            && data[4] == 0x5A
            && data[5] == 0x00
        {
            return Some(CompressionFormat::Lzma2);
        }
        // LZMA alone: starts with 0x5D (properties byte for lc=3,lp=0,pb=2)
        if data[0] == 0x5D && data.len() > 13 {
            return Some(CompressionFormat::Lzma2);
        }
        None
    }
}

/// Progress callback type: (files_extracted, total_files, current_file_name)
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send>;

/// Extract a compressed tar archive to a target directory.
///
/// Automatically detects the compression format (zstd or LZMA2) from
/// magic bytes. The archive should contain relative paths that map
/// directly to the installation directory.
///
/// Progress is reported as files are extracted.
pub fn extract_archive(
    compressed_data: &[u8],
    target_dir: &Path,
    progress: Option<&ProgressCallback>,
) -> Result<Vec<PathBuf>> {
    info!("Extracting to: {}", target_dir.display());

    // Create target directory if it doesn't exist
    std::fs::create_dir_all(target_dir)?;

    // Auto-detect compression format
    let format = CompressionFormat::detect(compressed_data).unwrap_or(CompressionFormat::Zstd);
    info!("Detected compression format: {:?}", format);

    // Decompress based on format
    let decompressed = match format {
        CompressionFormat::Zstd => zstd::decode_all(compressed_data)
            .map_err(|e| CoreError::compression("zstd decompression", format!("{}", e)))?,
        CompressionFormat::Lzma2 => {
            let mut output = Vec::new();
            // Try XZ format first (has magic bytes), fall back to raw LZMA
            let decompress_result = if compressed_data.len() >= 6
                && compressed_data[0] == 0xFD
                && compressed_data[1] == 0x37
                && compressed_data[2] == 0x7A
                && compressed_data[3] == 0x58
                && compressed_data[4] == 0x5A
                && compressed_data[5] == 0x00
            {
                lzma_rs::xz_decompress(&mut std::io::Cursor::new(compressed_data), &mut output)
            } else {
                lzma_rs::lzma_decompress(&mut std::io::Cursor::new(compressed_data), &mut output)
            };
            decompress_result
                .map_err(|e| CoreError::compression("lzma2 decompression", format!("{}", e)))?;
            output
        }
    };

    // Parse tar archive
    let mut archive = tar::Archive::new(decompressed.as_slice());
    let mut extracted_files = Vec::new();
    let mut file_count = 0usize;

    for entry in archive
        .entries()
        .map_err(|e| CoreError::compression("tar", format!("{}", e)))?
    {
        let mut entry = entry.map_err(|e| CoreError::compression("tar entry", format!("{}", e)))?;

        let path = entry
            .path()
            .map_err(|e| CoreError::compression("tar path", format!("{}", e)))?
            .into_owned();

        // Report progress — total is unknown during single-pass, use 0 to indicate dynamic
        if let Some(cb) = progress {
            let name = path.to_string_lossy().to_string();
            cb(file_count, 0, &name);
        }

        // Security: ensure we don't escape the target directory
        if !dest_path_within_target(&path, target_dir) {
            return Err(CoreError::permission_denied(
                "path traversal",
                format!(
                    "{} escapes target directory {}",
                    path.display(),
                    target_dir.display()
                ),
            ));
        }

        let dest_path = target_dir.join(&path);

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            debug!("Created directory: {}", dest_path.display());
        } else if entry.header().entry_type().is_symlink() {
            // Skip symlinks for security — they could point outside the target
            warn!("Skipping symlink in archive: {}", path.display());
            continue;
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

/// Check if a relative path stays within the target directory.
///
/// This is a pre-canonicalization check that validates path components
/// without requiring the path to exist on disk.
fn dest_path_within_target(relative: &Path, _target_dir: &Path) -> bool {
    // Reject absolute paths
    if relative.is_absolute() {
        return false;
    }

    // Walk components and check for parent dir traversal
    let mut depth = 0i32;
    for component in relative.components() {
        match component {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            std::path::Component::Normal(_) => {
                depth += 1;
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return false;
            }
            std::path::Component::CurDir => {
                // "." — no change in depth
            }
        }
    }

    true
}

/// Create a compressed tar archive from a list of files.
///
/// Each file is stored with its relative path as the archive entry name.
/// Uses zstd compression by default. For LZMA2, use `create_archive_with_format`.
pub fn create_archive(
    files: &[(PathBuf, String)], // (absolute_path, relative_name)
    compression_level: i32,
) -> Result<Vec<u8>> {
    create_archive_with_format(files, compression_level, CompressionFormat::Zstd)
}

/// Create a compressed tar archive with a specific compression format.
///
/// Supports zstd (fast, good ratio) and LZMA2 (slower, better ratio).
pub fn create_archive_with_format(
    files: &[(PathBuf, String)], // (absolute_path, relative_name)
    compression_level: i32,
    format: CompressionFormat,
) -> Result<Vec<u8>> {
    info!(
        "Creating {:?} archive with {} files, compression level {}",
        format,
        files.len(),
        compression_level
    );

    // Create tar in memory
    let mut tar_builder = tar::Builder::new(Vec::new());

    for (abs_path, rel_name) in files {
        let mut file = std::fs::File::open(abs_path)?;
        tar_builder
            .append_file(rel_name, &mut file)
            .map_err(|e| CoreError::compression("tar append", format!("{}", e)))?;
        debug!("Added to archive: {} -> {}", abs_path.display(), rel_name);
    }

    let tar_data = tar_builder
        .into_inner()
        .map_err(|e| CoreError::compression("tar finalize", format!("{}", e)))?;

    // Compress with selected format
    let tar_len = tar_data.len();
    let compressed = match format {
        CompressionFormat::Zstd => zstd::encode_all(tar_data.as_slice(), compression_level)
            .map_err(|e| CoreError::compression("zstd compression", format!("{}", e)))?,
        CompressionFormat::Lzma2 => {
            let mut output = Vec::new();
            lzma_rs::lzma_compress(&mut std::io::Cursor::new(tar_data), &mut output)
                .map_err(|e| CoreError::compression("lzma2 compression", format!("{}", e)))?;
            output
        }
    };

    let ratio = if tar_len > 0 {
        (1.0 - compressed.len() as f64 / tar_len as f64) * 100.0
    } else {
        0.0
    };
    info!(
        "Archive size: {} bytes (from {} bytes, {:.1}% compression)",
        compressed.len(),
        tar_len,
        ratio
    );
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

    #[test]
    fn test_lzma2_roundtrip() {
        let temp_dir = std::env::temp_dir().join("velocity_test_lzma2");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("lzma_test.txt"), "LZMA2 compressed data!").unwrap();

        let files = vec![(temp_dir.join("lzma_test.txt"), "lzma_test.txt".to_string())];

        // Create LZMA2 archive
        let archive = create_archive_with_format(&files, 6, CompressionFormat::Lzma2).unwrap();
        assert!(!archive.is_empty());

        // Verify format detection
        let detected = CompressionFormat::detect(&archive);
        assert_eq!(detected, Some(CompressionFormat::Lzma2));

        // Extract and verify
        let extract_dir = temp_dir.join("extracted_lzma2");
        let extracted = extract_archive(&archive, &extract_dir, None).unwrap();
        assert_eq!(extracted.len(), 1);

        let content = std::fs::read_to_string(extract_dir.join("lzma_test.txt")).unwrap();
        assert_eq!(content, "LZMA2 compressed data!");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_format_detection_zstd() {
        let temp_dir = std::env::temp_dir().join("velocity_test_detect");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("detect.txt"), "detect test").unwrap();
        let files = vec![(temp_dir.join("detect.txt"), "detect.txt".to_string())];

        let zstd_archive = create_archive(&files, 3).unwrap();
        assert_eq!(
            CompressionFormat::detect(&zstd_archive),
            Some(CompressionFormat::Zstd)
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_format_detection_unknown() {
        assert_eq!(CompressionFormat::detect(&[0, 1, 2, 3]), None);
        assert_eq!(CompressionFormat::detect(&[]), None);
    }
}
