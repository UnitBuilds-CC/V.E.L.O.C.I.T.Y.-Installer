//! Self-update mechanism — check for newer versions of the installed application.
//!
//! Queries an HTTP endpoint for version information and compares it against
//! the currently installed version. Supports semantic version comparison.

use crate::error::{CoreError, Result};
use tracing::{debug, info, warn};

/// Version check result.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Latest available version string
    pub latest_version: String,
    /// Download URL for the full update
    pub download_url: String,
    /// Release notes / chang
    pub release_notes: Option<String>,
    /// Whether an update is available
    pub update_available: bool,
    /// Delta update information (if available)
    pub delta: Option<DeltaInfo>,
}

/// Delta update information.
#[derive(Debug, Clone)]
pub struct DeltaInfo {
    /// Download URL for the delta package
    pub url: String,
    /// Size of the delta package in bytes
    pub size: u64,
    /// Size of the full package in bytes
    pub full_size: u64,
    /// Number of intermediate versions (for multi-hop updates)
    pub hops: u32,
}

/// Check for updates by querying a version endpoint.
///
/// The endpoint should return a JSON response with:
/// ```json
/// {
///   "version": "1.2.3",
///   "download_url": "https://example.com/installer.exe",
///   "release_notes": "Bug fixes and improvements"
/// }
/// ```
///
/// `current_version` is the currently installed version (semver).
/// `update_url` is the URL to query for version info.
pub fn check_for_update(current_version: &str, update_url: &str) -> Result<UpdateInfo> {
    if update_url.is_empty() {
        return Ok(UpdateInfo {
            latest_version: current_version.to_string(),
            download_url: String::new(),
            release_notes: None,
            update_available: false,
            delta: None,
        });
    }

    info!("Checking for updates at: {}", update_url);
    info!("Current version: {}", current_version);

    // Fetch version info from the update URL
    let response = match fetch_update_info(update_url) {
        Ok(info) => info,
        Err(e) => {
            warn!("Failed to check for updates: {}", e);
            return Ok(UpdateInfo {
                latest_version: current_version.to_string(),
                download_url: String::new(),
                release_notes: None,
                update_available: false,
                delta: None,
            });
        }
    };

    // Compare versions
    let update_available = is_newer_version(&response.latest_version, current_version);

    info!(
        "Update check complete: latest={}, current={}, update_available={}",
        response.latest_version, current_version, update_available
    );

    Ok(UpdateInfo {
        update_available,
        ..response
    })
}

/// Compare two semver version strings.
/// Returns true if `latest` is newer than `current`.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);

    debug!(
        "Version comparison: {:?} vs {:?}",
        latest_parts, current_parts
    );

    // Compare major.minor.patch
    for i in 0..3 {
        let l = latest_parts.get(i).copied().unwrap_or(0);
        let c = current_parts.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false // Equal versions
}

/// Parse a version string into numeric parts.
/// Handles formats like "1.2.3", "v1.2.3", "1.2.3-beta".
fn parse_version(version: &str) -> Vec<u64> {
    let v = version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V');
    // Strip pre-release suffix for comparison
    let v = v.split('-').next().unwrap_or(v);
    v.split('.').filter_map(|s| s.parse::<u64>().ok()).collect()
}

/// Fetch update info from the HTTP endpoint.
fn fetch_update_info(url: &str) -> Result<UpdateInfo> {
    // Use the existing downloader to fetch the version info
    let response = crate::downloader::download_to_memory(url)?;
    let body = String::from_utf8_lossy(&response);

    // Parse JSON response
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| CoreError::Other(format!("Failed to parse update info: {}", e)))?;

    let latest_version = json["version"].as_str().unwrap_or("0.0.0").to_string();
    let download_url = json["download_url"].as_str().unwrap_or("").to_string();
    let release_notes = json["release_notes"].as_str().map(|s| s.to_string());

    // Parse delta information if available
    let delta = if let Some(delta_json) = json.get("delta") {
        let delta_url = delta_json["url"].as_str().unwrap_or("").to_string();
        let delta_size = delta_json["size"].as_u64().unwrap_or(0);
        let full_size = delta_json["full_size"].as_u64().unwrap_or(0);
        let hops = delta_json["hops"].as_u64().unwrap_or(0) as u32;

        if !delta_url.is_empty() && delta_size > 0 {
            Some(DeltaInfo {
                url: delta_url,
                size: delta_size,
                full_size,
                hops,
            })
        } else {
            None
        }
    } else {
        None
    };

    Ok(UpdateInfo {
        latest_version,
        download_url,
        release_notes,
        update_available: false, // Will be set by caller
        delta,
    })
}

/// Decide whether to use delta or full update based on size heuristic.
///
/// Returns true if delta should be used (smaller total size), false for full update.
pub fn should_use_delta(delta_info: &Option<DeltaInfo>, full_size: u64) -> bool {
    match delta_info {
        Some(delta) => {
            // Heuristic: use delta if it's less than 70% of full size
            // and not too many hops (max 5)
            let delta_threshold = (full_size as f64 * 0.7) as u64;
            let use_delta = delta.size < delta_threshold && delta.hops <= 5;
            
            if use_delta {
                info!(
                    "Using delta update: {} bytes (vs {} bytes full, {}% reduction)",
                    delta.size,
                    full_size,
                    ((1.0 - delta.size as f64 / full_size as f64) * 100.0) as u32
                );
            } else {
                info!(
                    "Using full update: delta {} bytes >= 70% of full {} bytes, or too many hops ({})",
                    delta.size, full_size, delta.hops
                );
            }
            
            use_delta
        }
        None => false,
    }
}

/// Download and apply a delta update.
///
/// Downloads the delta package, applies it to the current installation,
/// and verifies the result.
pub fn apply_delta_update(
    install_dir: &std::path::Path,
    delta_info: &DeltaInfo,
) -> Result<()> {
    use crate::delta::{apply_delta, load_delta_package};

    info!("Downloading delta update from: {}", delta_info.url);

    // Download delta package
    let delta_data = crate::downloader::download_to_memory(&delta_info.url)?;
    
    info!("Downloaded {} bytes", delta_data.len());

    // Save to temporary file
    let temp_delta = install_dir.join(".delta-temp.zip");
    std::fs::write(&temp_delta, &delta_data)?;

    // Load delta package
    let delta = load_delta_package(&temp_delta)?;

    // Apply delta
    info!("Applying delta: {} -> {}", delta.from_version, delta.to_version);
    apply_delta(&delta, install_dir)?;

    // Cleanup temporary file
    std::fs::remove_file(&temp_delta)?;

    info!("Delta update applied successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.2.3"), vec![1, 2, 3]);
        assert_eq!(parse_version("v1.2.3"), vec![1, 2, 3]);
        assert_eq!(parse_version("V2.0.1"), vec![2, 0, 1]);
        assert_eq!(parse_version("1.2.3-beta"), vec![1, 2, 3]);
        assert_eq!(parse_version("10.20.30"), vec![10, 20, 30]);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("1.1.0", "1.0.9"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.1"));
        assert!(!is_newer_version("0.9.9", "1.0.0"));
    }

    #[test]
    fn test_is_newer_with_v_prefix() {
        assert!(is_newer_version("v1.0.1", "1.0.0"));
        assert!(is_newer_version("1.0.1", "v1.0.0"));
        assert!(!is_newer_version("v1.0.0", "v1.0.0"));
    }

    #[test]
    fn test_is_newer_with_prerelease() {
        assert!(!is_newer_version("1.0.0-beta", "1.0.0"));
        assert!(is_newer_version("1.0.1", "1.0.0-beta"));
    }

    #[test]
    fn test_check_no_url() {
        let result = check_for_update("1.0.0", "").unwrap();
        assert!(!result.update_available);
    }
}
