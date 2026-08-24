//! Cloud-fetch module for Ninite-style installers.
//!
//! This module provides:
//! - Git platform API clients (GitHub, GitLab, Bitbucket, Gitea)
//! - URL-based version resolver for direct HTTP downloads
//! - Download manager with caching and retry logic
//! - Version comparison and update checking

mod github;
mod gitlab;
mod bitbucket;
mod gitea;
mod url_client;
pub mod download;
pub mod update;
pub mod installer;
pub mod archive;

pub use github::GitHubClient;
pub use gitlab::GitLabClient;
pub use bitbucket::BitbucketClient;
pub use gitea::GiteaClient;
pub use url_client::UrlClient;
pub use download::DownloadManager;
pub use update::{UpdateInfo, UpdatePriority, check_for_update, is_update_available, parse_check_interval};
pub use installer::{
    InstallerType, InstallerResult, detect_installer_type, get_silent_args,
    execute_silent_installer, execute_with_config, resolve_fetch_action,
};

use anyhow::Result;
use std::time::Duration;
use velocity_config::{GitPlatform, FetchConfig, FetchMode};

// ─── Hardened HTTP Agent ─────────────────────────────────────────────────
//
// All platform clients share this builder to ensure consistent security:
// - Connection timeout: 10s (prevent hanging on slow networks)
// - Read timeout: 30s (prevent hanging on large responses)
// - Write timeout: 10s (prevent hanging on uploads)
// - Max 10 redirects (prevent redirect loops)

/// Build a hardened ureq agent with timeouts and redirect limits.
///
/// All platform clients MUST use this instead of `ureq::AgentBuilder::new()`
/// to ensure consistent security posture.
pub(crate) fn hardened_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(10))
        .redirects(10)
        .build()
}

/// Information about a release version.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// Version tag (e.g., "v1.0.0" or "1.0.0")
    pub version: String,
    /// Release name/title
    pub name: Option<String>,
    /// Release notes/description
    pub body: Option<String>,
    /// Published date (ISO 8601 format)
    pub published_at: Option<String>,
    /// List of release assets
    pub assets: Vec<ReleaseAsset>,
}

/// A release asset (file) available for download.
#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    /// Asset filename
    pub name: String,
    /// Download URL
    pub download_url: String,
    /// File size in bytes
    pub size: u64,
    /// Content type (MIME type)
    pub content_type: Option<String>,
    /// Download count
    pub download_count: u64,
}

/// Trait for Git platform API clients.
///
/// Each Git hosting platform (GitHub, GitLab, Bitbucket, Gitea) implements
/// this trait to provide a unified interface for fetching release information.
pub trait VersionResolver {
    /// Get the latest release version from the repository.
    ///
    /// # Returns
    /// `VersionInfo` containing version tag, release notes, and available assets.
    ///
    /// # Errors
    /// Returns an error if the API request fails or no releases are found.
    fn get_latest_version(&self) -> Result<VersionInfo>;

    /// Get a specific release version by tag name.
    ///
    /// # Arguments
    /// * `tag` - The release tag to fetch (e.g., "v1.0.0")
    ///
    /// # Returns
    /// `VersionInfo` for the specified tag.
    ///
    /// # Errors
    /// Returns an error if the tag doesn't exist or API request fails.
    fn get_version_by_tag(&self, tag: &str) -> Result<VersionInfo>;

    /// Find a release asset matching the given pattern.
    ///
    /// # Arguments
    /// * `version_info` - The release to search
    /// * `pattern` - Glob pattern to match asset filenames (e.g., "*.exe", "app-*.zip")
    ///
    /// # Returns
    /// `Some(ReleaseAsset)` if a match is found, `None` otherwise.
    fn find_asset<'a>(&self, version_info: &'a VersionInfo, pattern: &str) -> Option<&'a ReleaseAsset>;
}

/// Create a version resolver for the specified Git platform.
///
/// # Arguments
/// * `platform` - The Git hosting platform
/// * `repo` - Repository in "owner/repo" format
/// * `api_url` - Optional custom API URL for self-hosted instances
/// * `auth_token` - Optional authentication token for private repositories
///
/// # Returns
/// A boxed `VersionResolver` implementation for the specified platform.
pub fn create_resolver(
    platform: GitPlatform,
    repo: &str,
    api_url: Option<&str>,
    auth_token: Option<&str>,
) -> Result<Box<dyn VersionResolver>> {
    match platform {
        GitPlatform::GitHub => Ok(Box::new(GitHubClient::new(repo, api_url, auth_token)?)),
        GitPlatform::GitLab => Ok(Box::new(GitLabClient::new(repo, api_url, auth_token)?)),
        GitPlatform::Bitbucket => Ok(Box::new(BitbucketClient::new(repo, api_url, auth_token)?)),
        GitPlatform::Gitea => Ok(Box::new(GiteaClient::new(repo, api_url, auth_token)?)),
    }
}

/// Create a version resolver from a FetchConfig.
///
/// Automatically selects the appropriate resolver based on the fetch mode:
/// - `GitRelease` mode: creates a platform-specific Git API client
/// - `Url` mode: creates a URL-based resolver
/// - `Hybrid` mode: creates a platform-specific Git API client (like GitRelease)
pub fn create_resolver_from_config(config: &FetchConfig) -> Result<Box<dyn VersionResolver>> {
    match config.mode {
        FetchMode::GitRelease | FetchMode::Hybrid => {
            let platform = config.platform
                .ok_or_else(|| anyhow::anyhow!("fetch.platform is required for git-release mode"))?;
            let repo = config.repo.as_deref()
                .ok_or_else(|| anyhow::anyhow!("fetch.repo is required for git-release mode"))?;
            create_resolver(
                platform,
                repo,
                config.api_url.as_deref(),
                config.auth_token.as_deref(),
            )
        }
        FetchMode::Url => {
            let base_url = config.base_url.as_deref()
                .ok_or_else(|| anyhow::anyhow!("fetch.base_url is required for url mode"))?;
            let version_url = config.version_url.as_deref()
                .ok_or_else(|| anyhow::anyhow!("fetch.version_url is required for url mode"))?;
            Ok(Box::new(UrlClient::new(
                base_url,
                version_url,
                config.asset_pattern.as_deref(),
                config.checksum_url.as_deref(),
            )))
        }
    }
}

/// Match a filename against a glob pattern.
///
/// Supports glob patterns:
/// - `*` matches any sequence of characters (except path separators)
/// - `?` matches exactly one character
/// - All other characters match themselves
///
/// # Arguments
/// * `pattern` - Glob pattern (e.g., "*.exe", "app-?.zip")
/// * `filename` - Filename to match
///
/// # Returns
/// `true` if the filename matches the pattern, `false` otherwise.
pub fn match_glob(pattern: &str, filename: &str) -> bool {
    glob_match(pattern.as_bytes(), filename.as_bytes())
}

/// Recursive byte-level glob matcher supporting `*` and `?`.
fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
    let mut pi = 0; // pattern index
    let mut ti = 0; // text index
    let mut star_pi = None; // pattern index after last '*'
    let mut star_ti = None; // text index when last '*' was hit

    while ti < text.len() {
        if pi < pattern.len() && pattern[pi] == b'?' {
            // '?' matches exactly one character
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            // '*' matches zero or more characters — record position for backtracking
            star_pi = Some(pi);
            star_ti = Some(ti);
            pi += 1;
        } else if pi < pattern.len() && pattern[pi] == text[ti] {
            // Literal match
            pi += 1;
            ti += 1;
        } else if let (Some(spi), Some(sti)) = (star_pi, star_ti) {
            // Mismatch — backtrack: let '*' consume one more character
            pi = spi + 1;
            let new_ti = sti + 1;
            star_ti = Some(new_ti);
            ti = new_ti;
        } else {
            return false;
        }
    }

    // Consume any trailing '*' in pattern
    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
}

/// Substitute placeholders in an asset pattern string.
///
/// Replaces `{version}` with the actual version, `{arch}` with the detected
/// architecture, and `{app}` with the application name.
///
/// # Arguments
/// * `pattern` - Pattern with placeholders (e.g., "{app}-{version}-win-{arch}.zip")
/// * `version` - Version string to substitute (e.g., "1.0.0")
/// * `app_name` - Application name
///
/// # Returns
/// The pattern with all placeholders replaced.
pub fn substitute_placeholders(pattern: &str, version: &str, app_name: &str) -> String {
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    };

    // Strip leading 'v' from version for substitution
    let clean_version = version.strip_prefix('v').unwrap_or(version);

    pattern
        .replace("{version}", clean_version)
        .replace("{app}", app_name)
        .replace("{arch}", arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── match_glob tests ──────────────────────────────────────────────

    #[test]
    fn test_match_glob_exact() {
        assert!(match_glob("file.exe", "file.exe"));
        assert!(!match_glob("file.exe", "file.txt"));
    }

    #[test]
    fn test_match_glob_wildcard() {
        assert!(match_glob("*.exe", "app.exe"));
        assert!(match_glob("*.exe", "installer.exe"));
        assert!(!match_glob("*.exe", "app.txt"));
        assert!(match_glob("app-*.zip", "app-v1.0.0.zip"));
        assert!(match_glob("app-*-win-*.zip", "app-v1.0.0-win-x64.zip"));
    }

    #[test]
    fn test_match_glob_complex() {
        assert!(match_glob("*-win-x64.zip", "myapp-v1.0.0-win-x64.zip"));
        assert!(!match_glob("*-win-x64.zip", "myapp-v1.0.0-linux-x64.zip"));
    }

    #[test]
    fn test_match_glob_question_mark() {
        assert!(match_glob("app-?.zip", "app-1.zip"));
        assert!(match_glob("app-?.zip", "app-a.zip"));
        assert!(!match_glob("app-?.zip", "app-12.zip"));
        assert!(!match_glob("app-?.zip", "app-.zip"));
    }

    #[test]
    fn test_match_glob_mixed_wildcards() {
        assert!(match_glob("app-?-win-*.zip", "app-1-win-x64.zip"));
        assert!(match_glob("*-?.?.?-*.zip", "myapp-1.0.0-win.zip"));
        assert!(!match_glob("?-*.exe", "ab-test.exe"));
    }

    #[test]
    fn test_match_glob_edge_cases() {
        // Empty pattern matches empty string
        assert!(match_glob("", ""));
        assert!(!match_glob("", "file.exe"));
        // Star matches empty
        assert!(match_glob("*", ""));
        assert!(match_glob("*", "anything"));
        // Multiple stars
        assert!(match_glob("**", "anything"));
        assert!(match_glob("*.*", "file.exe"));
        assert!(!match_glob("*.*", "noextension"));
    }

    // ── substitute_placeholders tests ─────────────────────────────────

    #[test]
    fn test_substitute_version() {
        let result = substitute_placeholders("app-{version}.zip", "1.2.3", "myapp");
        assert_eq!(result, "app-1.2.3.zip");
    }

    #[test]
    fn test_substitute_strips_v_prefix() {
        let result = substitute_placeholders("app-{version}.zip", "v1.2.3", "myapp");
        assert_eq!(result, "app-1.2.3.zip");
    }

    #[test]
    fn test_substitute_app_name() {
        let result = substitute_placeholders("{app}-{version}.zip", "1.0.0", "MyApp");
        assert_eq!(result, "MyApp-1.0.0.zip");
    }

    #[test]
    fn test_substitute_all_placeholders() {
        let result = substitute_placeholders(
            "{app}-{version}-win-{arch}.zip",
            "v2.0.0",
            "CloudFetchApp",
        );
        assert!(result.starts_with("CloudFetchApp-2.0.0-win-"));
        assert!(result.ends_with(".zip"));
        // Should not contain any placeholders
        assert!(!result.contains('{'));
        assert!(!result.contains('}'));
    }

    #[test]
    fn test_substitute_no_placeholders() {
        let result = substitute_placeholders("fixed-name.exe", "1.0.0", "app");
        assert_eq!(result, "fixed-name.exe");
    }
}
