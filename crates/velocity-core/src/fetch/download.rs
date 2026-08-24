//! Download manager with caching, retry logic, and progress tracking.
//!
//! Wraps the existing `downloader` module to add:
//! - Download caching in `~/.velocity/cache/`
//! - Retry with exponential backoff (3 attempts)
//! - Progress reporting
//! - SHA256 verification integration

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::downloader;

/// Download manager with caching and retry support.
pub struct DownloadManager {
    /// Cache directory for downloaded files
    cache_dir: PathBuf,
    /// Maximum retry attempts
    max_retries: u32,
    /// Base delay between retries (doubles each attempt)
    retry_delay: Duration,
}

impl DownloadManager {
    /// Create a new download manager with default settings.
    ///
    /// Uses `~/.velocity/cache/` as the cache directory.
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .context("Could not determine home directory")?
            .join(".velocity")
            .join("cache");
        
        Ok(Self {
            cache_dir,
            max_retries: 3,
            retry_delay: Duration::from_secs(2),
        })
    }

    /// Create a download manager with a custom cache directory.
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            max_retries: 3,
            retry_delay: Duration::from_secs(2),
        }
    }

    /// Set the maximum number of retry attempts.
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Download a file with caching and retry support.
    ///
    /// # Arguments
    /// * `url` - URL to download from
    /// * `dest` - Destination directory for the file
    /// * `filename` - Optional filename override
    /// * `expected_sha256` - Optional SHA256 hash for verification
    /// * `progress` - Optional progress callback (bytes_downloaded, total_bytes, url)
    ///
    /// # Returns
    /// Path to the downloaded file.
    pub fn download(
        &self,
        url: &str,
        dest: &Path,
        filename: Option<&str>,
        expected_sha256: Option<&str>,
        progress: Option<&downloader::DownloadProgressCallback>,
    ) -> Result<PathBuf> {
        // Check cache first
        if let Some(cached) = self.cache_get(url, expected_sha256) {
            info!("Using cached file: {}", cached.display());
            
            // Copy from cache to destination
            let fname = filename.map(|s| s.to_string()).unwrap_or_else(|| {
                url.rsplit('/')
                    .next()
                    .unwrap_or("download")
                    .split('?')
                    .next()
                    .unwrap_or("download")
                    .to_string()
            });
            
            std::fs::create_dir_all(dest)?;
            let output_path = dest.join(&fname);
            std::fs::copy(&cached, &output_path)
                .with_context(|| format!("Failed to copy cached file to {}", output_path.display()))?;
            return Ok(output_path);
        }

        // Download with retry
        let result = self.download_with_retry(url, dest, filename, expected_sha256, progress)?;
        
        // Save to cache
        if let Err(e) = self.cache_put(url, &result) {
            warn!("Failed to cache downloaded file: {}", e);
        }

        // Auto-cleanup: run cache maintenance after successful download
        // Uses 30-day max age and 1GB max size as defaults
        if let Err(e) = self.cleanup(
            std::time::Duration::from_secs(30 * 86400),
            1024 * 1024 * 1024,
        ) {
            debug!("Cache cleanup failed (non-fatal): {}", e);
        }

        Ok(result)
    }

    /// Download a file into memory (for small payloads like version JSON).
    pub fn download_to_memory(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        let mut last_error: Option<anyhow::Error> = None;
        
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = self.retry_delay * 2u32.pow(attempt - 1);
                debug!("Retry attempt {} after {:?}", attempt, delay);
                std::thread::sleep(delay);
            }
            
            match downloader::download_to_memory(url) {
                Ok(data) => return Ok(data),
                Err(e) => {
                    warn!("Download attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e.into());
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Download failed after {} attempts", self.max_retries + 1)))
    }

    /// Download with retry logic.
    fn download_with_retry(
        &self,
        url: &str,
        dest: &Path,
        filename: Option<&str>,
        expected_sha256: Option<&str>,
        progress: Option<&downloader::DownloadProgressCallback>,
    ) -> anyhow::Result<PathBuf> {
        let mut last_error: Option<anyhow::Error> = None;
        
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = self.retry_delay * 2u32.pow(attempt - 1);
                debug!("Retry attempt {} after {:?}", attempt, delay);
                std::thread::sleep(delay);
            }
            
            // Use resumable download for better reliability
            match downloader::download_file_resumable(url, dest, filename, expected_sha256, progress) {
                Ok(path) => return Ok(path),
                Err(e) => {
                    warn!("Download attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e.into());
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Download failed after {} attempts", self.max_retries + 1)))
    }

    /// Get a file from the cache.
    ///
    /// Returns the cached file path if found and valid.
    fn cache_get(&self, url: &str, expected_sha256: Option<&str>) -> Option<PathBuf> {
        let cache_key = Self::url_to_cache_key(url);
        let cached_path = self.cache_dir.join(&cache_key);
        
        if !cached_path.exists() {
            return None;
        }
        
        // Verify SHA256 if provided
        if let Some(expected) = expected_sha256 {
            match downloader::compute_sha256_file(&cached_path) {
                Ok(actual) => {
                    if actual != expected.to_lowercase() {
                        warn!("Cached file checksum mismatch, re-downloading");
                        let _ = std::fs::remove_file(&cached_path);
                        return None;
                    }
                }
                Err(e) => {
                    warn!("Failed to verify cached file: {}", e);
                    return None;
                }
            }
        }
        
        Some(cached_path)
    }

    /// Save a downloaded file to the cache.
    fn cache_put(&self, url: &str, source: &Path) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir)
            .context("Failed to create cache directory")?;
        
        let cache_key = Self::url_to_cache_key(url);
        let cached_path = self.cache_dir.join(&cache_key);
        
        std::fs::copy(source, &cached_path)
            .with_context(|| format!("Failed to copy to cache: {}", cached_path.display()))?;
        
        debug!("Cached file: {}", cached_path.display());
        Ok(())
    }

    /// Convert a URL to a cache key (SHA256 hash of URL).
    fn url_to_cache_key(url: &str) -> String {
        let hash = downloader::compute_sha256(url.as_bytes());
        // Use first 16 chars of hash + original filename for readability
        let filename = url.rsplit('/')
            .next()
            .unwrap_or("download")
            .split('?')
            .next()
            .unwrap_or("download");
        format!("{}_{}", &hash[..16], filename)
    }

    /// Clean up old cached files.
    ///
    /// Removes files older than `max_age` and enforces `max_size` limit.
    pub fn cleanup(&self, max_age: Duration, max_size_bytes: u64) -> Result<CleanupStats> {
        if !self.cache_dir.exists() {
            return Ok(CleanupStats::default());
        }

        let mut stats = CleanupStats::default();
        let now = std::time::SystemTime::now();
        
        // Collect all cached files with their metadata
        let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        
        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                let metadata = entry.metadata()?;
                let modified = metadata.modified().unwrap_or(now);
                entries.push((path, metadata.len(), modified));
            }
        }
        
        // Remove files older than max_age
        entries.retain(|(path, size, modified)| {
            let age = now.duration_since(*modified).unwrap_or(Duration::ZERO);
            if age > max_age {
                stats.files_removed += 1;
                stats.bytes_freed += size;
                let _ = std::fs::remove_file(path);
                false
            } else {
                true
            }
        });
        
        // Sort by modification time (oldest first) for size-based cleanup
        entries.sort_by(|a, b| a.2.cmp(&b.2));
        
        // Enforce max size limit
        let total_size: u64 = entries.iter().map(|(_, size, _)| *size).sum();
        if total_size > max_size_bytes {
            let mut current_size = total_size;
            for (path, size, _) in &entries {
                if current_size <= max_size_bytes {
                    break;
                }
                if std::fs::remove_file(path).is_ok() {
                    current_size -= size;
                    stats.files_removed += 1;
                    stats.bytes_freed += size;
                }
            }
        }
        
        stats.remaining_files = entries.len() as u64;
        info!("Cache cleanup: removed {} files ({} bytes freed)", 
              stats.files_removed, stats.bytes_freed);
        
        Ok(stats)
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default DownloadManager")
    }
}

/// Statistics from a cache cleanup operation.
#[derive(Debug, Default)]
pub struct CleanupStats {
    /// Number of files removed
    pub files_removed: u64,
    /// Total bytes freed
    pub bytes_freed: u64,
    /// Number of files remaining
    pub remaining_files: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_url_to_cache_key() {
        let key1 = DownloadManager::url_to_cache_key("https://example.com/file.exe");
        let key2 = DownloadManager::url_to_cache_key("https://example.com/other.exe");
        assert_ne!(key1, key2);
        assert!(key1.contains("file.exe"));
        assert!(key2.contains("other.exe"));
    }

    #[test]
    fn test_url_to_cache_key_consistent() {
        let key1 = DownloadManager::url_to_cache_key("https://example.com/file.exe");
        let key2 = DownloadManager::url_to_cache_key("https://example.com/file.exe");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_url_to_cache_key_strips_query_params() {
        let key1 = DownloadManager::url_to_cache_key("https://example.com/file.exe?token=abc");
        let key2 = DownloadManager::url_to_cache_key("https://example.com/file.exe?token=xyz");
        // Both should have the same filename part
        assert!(key1.contains("file.exe"));
        assert!(key2.contains("file.exe"));
        // But different hashes (different full URLs)
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_download_manager_new() {
        let dm = DownloadManager::new();
        assert!(dm.is_ok());
    }

    #[test]
    fn test_download_manager_with_cache_dir() {
        let dm = DownloadManager::with_cache_dir(PathBuf::from("/tmp/test-cache"));
        assert_eq!(dm.cache_dir, PathBuf::from("/tmp/test-cache"));
        assert_eq!(dm.max_retries, 3);
    }

    #[test]
    fn test_download_manager_with_max_retries() {
        let dm = DownloadManager::with_cache_dir(PathBuf::from("/tmp/test"))
            .with_max_retries(5);
        assert_eq!(dm.max_retries, 5);
    }

    // ── Cache operation tests ─────────────────────────────────────────

    #[test]
    fn test_cache_put_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let dm = DownloadManager::with_cache_dir(dir.path().to_path_buf());

        // Create a fake downloaded file
        let source_file = dir.path().join("source.txt");
        std::fs::write(&source_file, "test content").unwrap();

        // Put into cache
        dm.cache_put("https://example.com/test.txt", &source_file).unwrap();

        // Get from cache (without SHA256 check)
        let cached = dm.cache_get("https://example.com/test.txt", None);
        assert!(cached.is_some());
        assert!(cached.unwrap().exists());
    }

    #[test]
    fn test_cache_get_miss() {
        let dir = tempfile::tempdir().unwrap();
        let dm = DownloadManager::with_cache_dir(dir.path().to_path_buf());

        let cached = dm.cache_get("https://example.com/nonexistent.txt", None);
        assert!(cached.is_none());
    }

    #[test]
    fn test_cache_cleanup_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dm = DownloadManager::with_cache_dir(dir.path().to_path_buf());

        let stats = dm.cleanup(
            Duration::from_secs(86400),
            1024 * 1024,
        ).unwrap();

        assert_eq!(stats.files_removed, 0);
        assert_eq!(stats.bytes_freed, 0);
    }

    #[test]
    fn test_cache_cleanup_nonexistent_dir() {
        let dm = DownloadManager::with_cache_dir(PathBuf::from("/nonexistent/cache/dir"));

        let stats = dm.cleanup(
            Duration::from_secs(86400),
            1024 * 1024,
        ).unwrap();

        assert_eq!(stats.files_removed, 0);
    }

    #[test]
    fn test_cache_cleanup_removes_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let dm = DownloadManager::with_cache_dir(dir.path().to_path_buf());

        // Create a cached file
        let cache_file = dir.path().join("test_cached_file.txt");
        {
            let mut f = std::fs::File::create(&cache_file).unwrap();
            f.write_all(b"old data").unwrap();
        }

        // Test size-based cleanup: set max_size to 0 to force removal
        let stats = dm.cleanup(
            Duration::from_secs(365 * 86400), // Very long max_age
            0, // max_size = 0 bytes → everything should be cleaned
        ).unwrap();

        assert!(stats.files_removed >= 1);
    }

    #[test]
    fn test_cache_cleanup_respects_max_size() {
        let dir = tempfile::tempdir().unwrap();
        let dm = DownloadManager::with_cache_dir(dir.path().to_path_buf());

        // Create a small cached file
        let cache_file = dir.path().join("small_file.txt");
        std::fs::write(&cache_file, "small").unwrap();

        // Max size is 1GB — file should not be removed
        let stats = dm.cleanup(
            Duration::from_secs(365 * 86400),
            1024 * 1024 * 1024,
        ).unwrap();

        assert_eq!(stats.files_removed, 0);
        assert!(cache_file.exists());
    }
}
