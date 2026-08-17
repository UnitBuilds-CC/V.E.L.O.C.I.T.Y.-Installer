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
    /// Download URL for the update
    pub download_url: String,
    /// Release notes / chang
    pub release_notes: Option<String>,
    /// Whether an update is available
    pub update_available: bool,
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

    debug!("Version comparison: {:?} vs {:?}", latest_parts, current_parts);

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
    let v = version.trim().trim_start_matches('v').trim_start_matches('V');
    // Strip pre-release suffix for comparison
    let v = v.split('-').next().unwrap_or(v);
    v.split('.')
        .filter_map(|s| s.parse::<u64>().ok())
        .collect()
}

/// Fetch update info from the HTTP endpoint.
fn fetch_update_info(url: &str) -> Result<UpdateInfo> {
    // Use the existing downloader to fetch the version info
    let response = crate::downloader::download_to_memory(url)?;
    let body = String::from_utf8_lossy(&response);

    // Parse JSON response
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        CoreError::Other(format!("Failed to parse update info: {}", e))
    })?;

    let latest_version = json["version"]
        .as_str()
        .unwrap_or("0.0.0")
        .to_string();
    let download_url = json["download_url"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let release_notes = json["release_notes"]
        .as_str()
        .map(|s| s.to_string());

    Ok(UpdateInfo {
        latest_version,
        download_url,
        release_notes,
        update_available: false, // Will be set by caller
    })
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
