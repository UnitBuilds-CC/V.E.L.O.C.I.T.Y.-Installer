//! Archive extraction support for cloud-fetch downloads.
//!
//! Supports:
//! - ZIP archives (`.zip`) — using the `zip` crate
//! - TAR archives (`.tar`) — using the `tar` crate
//! - TAR + gzip (`.tar.gz`, `.tgz`) — using `tar` + `flate2`
//!
//! The main entry point is `extract_archive()` which auto-detects the format
//! from the file extension and extracts to the target directory.

use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use tracing::{debug, info, warn};

/// Supported archive formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
}

/// Detect archive format from file extension.
pub fn detect_archive_format(path: &Path) -> Option<ArchiveFormat> {
    let name = path.file_name()?.to_str()?.to_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveFormat::TarGz)
    } else if name.ends_with(".tar") {
        Some(ArchiveFormat::Tar)
    } else if name.ends_with(".zip") {
        Some(ArchiveFormat::Zip)
    } else {
        None
    }
}

/// Check if a path contains traversal components (`..`).
///
/// This is a simple but effective check: reject any entry name that
/// contains `..` as a path component. This prevents zip-slip attacks
/// without needing to canonicalize non-existent paths.
fn is_path_traversal(entry_name: &str) -> bool {
    entry_name.split(['/', '\\']).any(|component| component == "..")
}

/// Extract an archive to the target directory.
///
/// Auto-detects format from extension. Returns the number of files extracted.
pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<u64> {
    let format = detect_archive_format(archive_path)
        .with_context(|| format!("Unsupported archive format: {}", archive_path.display()))?;

    info!("Extracting {} ({:?}) to {}", archive_path.display(), format, dest_dir.display());
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("Failed to create destination: {}", dest_dir.display()))?;

    match format {
        ArchiveFormat::Zip => extract_zip(archive_path, dest_dir),
        ArchiveFormat::Tar => extract_tar_from_reader(
            std::fs::File::open(archive_path)
                .with_context(|| format!("Failed to open TAR: {}", archive_path.display()))?,
            dest_dir,
        ),
        ArchiveFormat::TarGz => {
            let file = std::fs::File::open(archive_path)
                .with_context(|| format!("Failed to open TAR.GZ: {}", archive_path.display()))?;
            let decoder = flate2::read::GzDecoder::new(file);
            extract_tar_from_reader(decoder, dest_dir)
        }
    }
}

/// Extract a ZIP archive.
///
/// Security: Validates entry names to prevent path traversal (zip-slip).
/// Creates directories as needed. Preserves Unix permissions.
fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<u64> {
    let file = std::fs::File::open(archive_path)
        .with_context(|| format!("Failed to open ZIP: {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("Failed to read ZIP: {}", archive_path.display()))?;

    let mut count: u64 = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .with_context(|| format!("Failed to read ZIP entry {}", i))?;

        let entry_name = entry.name().to_string();

        // Security: prevent path traversal (zip-slip attack)
        if is_path_traversal(&entry_name) {
            warn!("Skipping ZIP entry with path traversal: {}", entry_name);
            continue;
        }

        let entry_path = dest_dir.join(&entry_name);

        if entry.is_dir() {
            std::fs::create_dir_all(&entry_path)
                .with_context(|| format!("Failed to create dir: {}", entry_path.display()))?;
            debug!("ZIP dir: {}", entry_name);
        } else {
            if let Some(parent) = entry_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut outfile = std::fs::File::create(&entry_path)
                .with_context(|| format!("Failed to create: {}", entry_path.display()))?;
            std::io::copy(&mut entry, &mut outfile)
                .with_context(|| format!("Failed to extract: {}", entry_name))?;

            // Preserve Unix permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    std::fs::set_permissions(&entry_path, std::fs::Permissions::from_mode(mode)).ok();
                }
            }

            debug!("ZIP file: {} ({} bytes)", entry_name, entry.size());
            count += 1;
        }
    }

    info!("Extracted {} files from ZIP", count);
    Ok(count)
}

/// Extract TAR entries from any reader.
fn extract_tar_from_reader<R: Read>(reader: R, dest_dir: &Path) -> Result<u64> {
    let mut archive = tar::Archive::new(reader);
    let mut count: u64 = 0;

    for entry in archive.entries().context("Failed to read TAR entries")? {
        let mut entry = entry.context("Failed to read TAR entry")?;
        let entry_path_buf = entry.path().context("Invalid TAR entry path")?.into_owned();
        let entry_name = entry_path_buf.to_string_lossy().to_string();

        // Security: prevent path traversal
        if is_path_traversal(&entry_name) {
            warn!("Skipping TAR entry with path traversal: {}", entry_name);
            continue;
        }

        let full_path = dest_dir.join(&entry_path_buf);
        entry.unpack(&full_path)
            .with_context(|| format!("Failed to extract: {}", entry_name))?;
        debug!("TAR: {}", entry_name);
        count += 1;
    }

    info!("Extracted {} entries from TAR", count);
    Ok(count)
}

/// Check if a file is a ZIP archive by magic bytes (PK\x03\x04).
pub fn is_zip_file(path: &Path) -> bool {
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut magic = [0u8; 4];
        if f.read_exact(&mut magic).is_ok() {
            return magic == [0x50, 0x4B, 0x03, 0x04];
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_detect_archive_format() {
        assert_eq!(detect_archive_format(Path::new("app.zip")), Some(ArchiveFormat::Zip));
        assert_eq!(detect_archive_format(Path::new("app.tar")), Some(ArchiveFormat::Tar));
        assert_eq!(detect_archive_format(Path::new("app.tar.gz")), Some(ArchiveFormat::TarGz));
        assert_eq!(detect_archive_format(Path::new("app.tgz")), Some(ArchiveFormat::TarGz));
        assert_eq!(detect_archive_format(Path::new("app.exe")), None);
        assert_eq!(detect_archive_format(Path::new("app.msi")), None);
        assert_eq!(detect_archive_format(Path::new("APP.ZIP")), Some(ArchiveFormat::Zip));
    }

    #[test]
    fn test_is_path_traversal() {
        assert!(is_path_traversal("../../../etc/passwd"));
        assert!(is_path_traversal("foo/../../bar"));
        assert!(is_path_traversal("..\\..\\windows\\system32"));
        assert!(!is_path_traversal("hello.txt"));
        assert!(!is_path_traversal("subdir/nested.txt"));
        assert!(!is_path_traversal("foo/bar/baz.exe"));
    }

    #[test]
    fn test_is_zip_file_negative() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(b"Not a ZIP file").unwrap();
        assert!(!is_zip_file(tmp.path()));
    }

    #[test]
    fn test_extract_zip_basic() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let zip_path = tmp_dir.path().join("test.zip");
        let extract_dir = tmp_dir.path().join("output");

        // Create a ZIP with files and subdirectory
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zw.start_file("hello.txt", opts).unwrap();
        zw.write_all(b"Hello, World!").unwrap();
        zw.start_file("subdir/nested.txt", opts).unwrap();
        zw.write_all(b"Nested file").unwrap();
        zw.finish().unwrap();

        let count = extract_archive(&zip_path, &extract_dir).unwrap();
        assert_eq!(count, 2);
        assert!(extract_dir.join("hello.txt").exists());
        assert_eq!(
            std::fs::read_to_string(extract_dir.join("hello.txt")).unwrap(),
            "Hello, World!"
        );
        assert!(extract_dir.join("subdir/nested.txt").exists());
        assert_eq!(
            std::fs::read_to_string(extract_dir.join("subdir/nested.txt")).unwrap(),
            "Nested file"
        );
    }

    #[test]
    fn test_extract_zip_path_traversal_blocked() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let zip_path = tmp_dir.path().join("evil.zip");
        let extract_dir = tmp_dir.path().join("output");
        std::fs::create_dir_all(&extract_dir).unwrap();

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zw.start_file("../../../etc/passwd", opts).unwrap();
        zw.write_all(b"evil").unwrap();
        zw.start_file("safe.txt", opts).unwrap();
        zw.write_all(b"safe").unwrap();
        zw.finish().unwrap();

        let count = extract_archive(&zip_path, &extract_dir).unwrap();
        assert_eq!(count, 1); // Only safe.txt
        assert!(extract_dir.join("safe.txt").exists());
    }

    #[test]
    fn test_extract_unsupported_format() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let fake = tmp_dir.path().join("app.exe");
        std::fs::write(&fake, b"not an archive").unwrap();
        let result = extract_archive(&fake, tmp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported"));
    }
}
