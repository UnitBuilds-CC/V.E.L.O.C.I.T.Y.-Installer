//! Checksum verification — SHA256/MD5 hash computation and validation.
//!
//! Provides file integrity verification by computing cryptographic hashes
//! and comparing them against expected values stored in the manifest.

use crate::error::{CoreError, Result};
use sha2::{Digest, Sha256, Sha512};
use std::io::Read;
use std::path::Path;
use tracing::{debug, info, warn};

/// Supported hash algorithms for checksum verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha512,
}

impl HashAlgorithm {
    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sha512" | "sha-512" => HashAlgorithm::Sha512,
            _ => HashAlgorithm::Sha256,
        }
    }
}

/// Compute the hash of a file.
pub fn hash_file(path: &Path, algo: HashAlgorithm) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| {
        CoreError::Other(format!("Failed to open file for hashing {}: {}", path.display(), e))
    })?;

    let mut buffer = [0u8; 8192];

    match algo {
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            loop {
                let n = file.read(&mut buffer).map_err(|e| {
                    CoreError::Other(format!("Read error hashing {}: {}", path.display(), e))
                })?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        }
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            loop {
                let n = file.read(&mut buffer).map_err(|e| {
                    CoreError::Other(format!("Read error hashing {}: {}", path.display(), e))
                })?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        }
    }
}

/// Compute SHA256 hash of a byte slice.
pub fn hash_bytes(data: &[u8], algo: HashAlgorithm) -> String {
    match algo {
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            format!("{:x}", hasher.finalize())
        }
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            hasher.update(data);
            format!("{:x}", hasher.finalize())
        }
    }
}

/// Verify a file's hash against an expected value.
///
/// Returns `Ok(true)` if the hash matches, `Ok(false)` if it doesn't,
/// or `Err` if the file can't be read.
pub fn verify_file(path: &Path, expected_hash: &str, algo: HashAlgorithm) -> Result<bool> {
    let actual = hash_file(path, algo)?;
    let matches = actual.eq_ignore_ascii_case(expected_hash);
    if !matches {
        warn!(
            "Checksum mismatch for {}: expected {}, got {}",
            path.display(),
            expected_hash,
            actual
        );
    } else {
        debug!("Checksum OK for {}: {}", path.display(), actual);
    }
    Ok(matches)
}

/// Verify checksums for all files in a directory against a manifest of expected hashes.
///
/// `checksums` maps relative file paths to their expected hex-encoded hashes.
/// Returns a list of files that failed verification (empty if all pass).
pub fn verify_directory(
    install_dir: &Path,
    checksums: &std::collections::HashMap<String, String>,
    algo: HashAlgorithm,
) -> Result<Vec<String>> {
    let mut failures = Vec::new();

    info!(
        "Verifying checksums for {} files in {}",
        checksums.len(),
        install_dir.display()
    );

    for (rel_path, expected) in checksums {
        let full_path = install_dir.join(rel_path);
        if !full_path.exists() {
            warn!("Missing file during verification: {}", rel_path);
            failures.push(format!("{}: file missing", rel_path));
            continue;
        }
        match verify_file(&full_path, expected, algo) {
            Ok(true) => {}
            Ok(false) => {
                failures.push(format!("{}: hash mismatch", rel_path));
            }
            Err(e) => {
                failures.push(format!("{}: {}", rel_path, e));
            }
        }
    }

    if failures.is_empty() {
        info!("All {} files passed checksum verification", checksums.len());
    } else {
        warn!(
            "{} of {} files failed checksum verification",
            failures.len(),
            checksums.len()
        );
    }

    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_hash_bytes_sha256() {
        let hash = hash_bytes(b"hello world", HashAlgorithm::Sha256);
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_hash_bytes_sha512() {
        let hash = hash_bytes(b"hello world", HashAlgorithm::Sha512);
        assert!(hash.len() == 128); // SHA512 produces 128 hex chars
    }

    #[test]
    fn test_hash_file_and_verify() {
        let dir = std::env::temp_dir().join("velocity_checksum_test");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("test_hash.txt");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"test data for hashing").unwrap();
        drop(f);

        let hash = hash_file(&file_path, HashAlgorithm::Sha256).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 = 64 hex chars

        // Verify correct hash
        assert!(verify_file(&file_path, &hash, HashAlgorithm::Sha256).unwrap());

        // Verify wrong hash
        assert!(!verify_file(&file_path, "0000000000000000000000000000000000000000000000000000000000000000", HashAlgorithm::Sha256).unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_directory() {
        let dir = std::env::temp_dir().join("velocity_checksum_dir_test");
        let _ = std::fs::create_dir_all(&dir);

        let file1 = dir.join("a.txt");
        let file2 = dir.join("b.txt");
        std::fs::write(&file1, b"content a").unwrap();
        std::fs::write(&file2, b"content b").unwrap();

        let hash1 = hash_file(&file1, HashAlgorithm::Sha256).unwrap();
        let hash2 = hash_file(&file2, HashAlgorithm::Sha256).unwrap();

        let mut checksums = std::collections::HashMap::new();
        checksums.insert("a.txt".to_string(), hash1);
        checksums.insert("b.txt".to_string(), hash2);

        let failures = verify_directory(&dir, &checksums, HashAlgorithm::Sha256).unwrap();
        assert!(failures.is_empty());

        // Add a wrong hash
        checksums.insert("a.txt".to_string(), "bad_hash".to_string());
        let failures = verify_directory(&dir, &checksums, HashAlgorithm::Sha256).unwrap();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("hash mismatch"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_hash_algorithm_from_str() {
        assert_eq!(HashAlgorithm::from_str("sha256"), HashAlgorithm::Sha256);
        assert_eq!(HashAlgorithm::from_str("SHA256"), HashAlgorithm::Sha256);
        assert_eq!(HashAlgorithm::from_str("sha512"), HashAlgorithm::Sha512);
        assert_eq!(HashAlgorithm::from_str("SHA-512"), HashAlgorithm::Sha512);
        assert_eq!(HashAlgorithm::from_str("unknown"), HashAlgorithm::Sha256);
    }

    #[test]
    fn test_missing_file_verification() {
        let dir = std::env::temp_dir().join("velocity_checksum_missing");
        let _ = std::fs::create_dir_all(&dir);

        let mut checksums = std::collections::HashMap::new();
        checksums.insert("nonexistent.txt".to_string(), "abc123".to_string());

        let failures = verify_directory(&dir, &checksums, HashAlgorithm::Sha256).unwrap();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("file missing"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
