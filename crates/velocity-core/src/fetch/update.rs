//! Version comparison and update checking logic.
//!
//! Compares semantic versions to determine if updates are available,
//! calculates update priority, and provides update information.

use anyhow::{Context, Result};
use semver::Version;
use tracing::debug;

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Whether an update is available
    pub available: bool,
    /// Currently installed version
    pub current_version: String,
    /// Latest available version
    pub latest_version: String,
    /// Download URL for the update
    pub download_url: String,
    /// Release notes for the update
    pub release_notes: Option<String>,
    /// Update priority based on version difference
    pub priority: UpdatePriority,
}

/// Priority level for an available update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePriority {
    /// No update available
    None,
    /// Patch update (bug fixes)
    Patch,
    /// Minor update (new features)
    Minor,
    /// Major update (breaking changes)
    Major,
}

impl std::fmt::Display for UpdatePriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdatePriority::None => write!(f, "none"),
            UpdatePriority::Patch => write!(f, "patch"),
            UpdatePriority::Minor => write!(f, "minor"),
            UpdatePriority::Major => write!(f, "major"),
        }
    }
}

/// Check if an update is available by comparing versions.
///
/// # Arguments
/// * `current` - Currently installed version string (e.g., "1.0.0" or "v1.0.0")
/// * `remote` - Remote/latest version string (e.g., "1.0.1" or "v1.0.1")
///
/// # Returns
/// `true` if the remote version is newer than the current version.
pub fn is_update_available(current: &str, remote: &str) -> Result<bool> {
    let current_ver = parse_version(current)?;
    let remote_ver = parse_version(remote)?;
    Ok(remote_ver > current_ver)
}

/// Check for an update and return detailed information.
///
/// # Arguments
/// * `current_version` - Currently installed version
/// * `latest_version` - Latest available version
/// * `download_url` - URL to download the update
/// * `release_notes` - Optional release notes
///
/// # Returns
/// `UpdateInfo` with details about the available update.
pub fn check_for_update(
    current_version: &str,
    latest_version: &str,
    download_url: &str,
    release_notes: Option<&str>,
) -> Result<UpdateInfo> {
    let current_ver = parse_version(current_version)?;
    let latest_ver = parse_version(latest_version)?;
    
    let available = latest_ver > current_ver;
    let priority = if !available {
        UpdatePriority::None
    } else if latest_ver.major > current_ver.major {
        UpdatePriority::Major
    } else if latest_ver.minor > current_ver.minor {
        UpdatePriority::Minor
    } else {
        UpdatePriority::Patch
    };
    
    debug!(
        "Update check: current={}, latest={}, available={}, priority={}",
        current_version, latest_version, available, priority
    );
    
    Ok(UpdateInfo {
        available,
        current_version: current_version.to_string(),
        latest_version: latest_version.to_string(),
        download_url: download_url.to_string(),
        release_notes: release_notes.map(|s| s.to_string()),
        priority,
    })
}

/// Parse a version string, stripping optional "v" prefix.
///
/// Accepts both "1.0.0" and "v1.0.0" formats.
fn parse_version(version: &str) -> Result<Version> {
    let cleaned = version.strip_prefix('v').unwrap_or(version);
    Version::parse(cleaned)
        .with_context(|| format!("Failed to parse version: {}", version))
}

/// Parse a check interval string into a Duration.
///
/// Supported formats: "1h", "24h", "1d", "7d", "1w", "never"
pub fn parse_check_interval(interval: &str) -> Result<Option<std::time::Duration>> {
    let interval = interval.trim().to_lowercase();
    
    match interval.as_str() {
        "never" | "0" => Ok(None),
        s if s.ends_with('h') => {
            let hours: u64 = s[..s.len()-1].parse()
                .with_context(|| format!("Invalid hours interval: {}", interval))?;
            Ok(Some(std::time::Duration::from_secs(hours * 3600)))
        }
        s if s.ends_with('d') => {
            let days: u64 = s[..s.len()-1].parse()
                .with_context(|| format!("Invalid days interval: {}", interval))?;
            Ok(Some(std::time::Duration::from_secs(days * 86400)))
        }
        s if s.ends_with('w') => {
            let weeks: u64 = s[..s.len()-1].parse()
                .with_context(|| format!("Invalid weeks interval: {}", interval))?;
            Ok(Some(std::time::Duration::from_secs(weeks * 604800)))
        }
        _ => anyhow::bail!("Invalid interval format: {}. Use 'Nh', 'Nd', 'Nw', or 'never'", interval),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_plain() {
        let v = parse_version("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_parse_version_with_v_prefix() {
        let v = parse_version("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_is_update_available() {
        assert!(is_update_available("1.0.0", "1.0.1").unwrap());
        assert!(is_update_available("1.0.0", "1.1.0").unwrap());
        assert!(is_update_available("1.0.0", "2.0.0").unwrap());
        assert!(!is_update_available("1.0.1", "1.0.0").unwrap());
        assert!(!is_update_available("1.0.0", "1.0.0").unwrap());
    }

    #[test]
    fn test_is_update_available_with_v_prefix() {
        assert!(is_update_available("v1.0.0", "v1.0.1").unwrap());
        assert!(is_update_available("v1.0.0", "1.0.1").unwrap());
    }

    #[test]
    fn test_check_for_update_major() {
        let info = check_for_update("1.0.0", "2.0.0", "https://example.com/dl", Some("Major release")).unwrap();
        assert!(info.available);
        assert_eq!(info.priority, UpdatePriority::Major);
        assert_eq!(info.current_version, "1.0.0");
        assert_eq!(info.latest_version, "2.0.0");
    }

    #[test]
    fn test_check_for_update_minor() {
        let info = check_for_update("1.0.0", "1.1.0", "https://example.com/dl", None).unwrap();
        assert!(info.available);
        assert_eq!(info.priority, UpdatePriority::Minor);
    }

    #[test]
    fn test_check_for_update_patch() {
        let info = check_for_update("1.0.0", "1.0.1", "https://example.com/dl", None).unwrap();
        assert!(info.available);
        assert_eq!(info.priority, UpdatePriority::Patch);
    }

    #[test]
    fn test_check_for_update_not_available() {
        let info = check_for_update("1.0.1", "1.0.0", "https://example.com/dl", None).unwrap();
        assert!(!info.available);
        assert_eq!(info.priority, UpdatePriority::None);
    }

    #[test]
    fn test_parse_check_interval_hours() {
        let d = parse_check_interval("24h").unwrap().unwrap();
        assert_eq!(d.as_secs(), 86400);
    }

    #[test]
    fn test_parse_check_interval_days() {
        let d = parse_check_interval("7d").unwrap().unwrap();
        assert_eq!(d.as_secs(), 604800);
    }

    #[test]
    fn test_parse_check_interval_weeks() {
        let d = parse_check_interval("1w").unwrap().unwrap();
        assert_eq!(d.as_secs(), 604800);
    }

    #[test]
    fn test_parse_check_interval_never() {
        assert!(parse_check_interval("never").unwrap().is_none());
    }

    #[test]
    fn test_parse_check_interval_invalid() {
        assert!(parse_check_interval("invalid").is_err());
    }
}
