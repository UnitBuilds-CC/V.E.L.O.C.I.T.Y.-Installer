//! GitHub API client for fetching release information.

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::warn;
use super::{VersionInfo, ReleaseAsset, VersionResolver, match_glob, hardened_agent};

/// GitHub API client for fetching release information.
pub struct GitHubClient {
    owner: String,
    repo: String,
    api_url: String,
    auth_token: Option<String>,
    client: ureq::Agent,
}

impl GitHubClient {
    /// Create a new GitHub API client.
    ///
    /// # Arguments
    /// * `repo` - Repository in "owner/repo" format
    /// * `api_url` - Optional custom API URL for GitHub Enterprise
    /// * `auth_token` - Optional authentication token for private repositories
    pub fn new(repo: &str, api_url: Option<&str>, auth_token: Option<&str>) -> Result<Self> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid repository format: {}. Expected 'owner/repo'", repo);
        }

        let api_url = api_url
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "https://api.github.com".to_string());

        let client = hardened_agent();

        Ok(Self {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
            api_url,
            auth_token: auth_token.map(|s| s.to_string()),
            client,
        })
    }

    fn make_request(&self, endpoint: &str) -> Result<ureq::Response> {
        let url = format!("{}/repos/{}/{}{}", self.api_url, self.owner, self.repo, endpoint);
        
        let mut req = self.client.get(&url)
            .set("User-Agent", "Velocity-Installer/1.0")
            .set("Accept", "application/vnd.github.v3+json");
        
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("token {}", token));
        }
        
        let response = req.call()
            .with_context(|| format!("Failed to fetch from GitHub API: {}", url))?;

        // Check rate limit headers and warn if running low
        if let Some(remaining) = response.header("X-RateLimit-Remaining") {
            if let Ok(remaining) = remaining.parse::<u64>() {
                if remaining < 10 {
                    if let Some(reset) = response.header("X-RateLimit-Reset") {
                        warn!("GitHub API rate limit low: {} requests remaining. Resets at {}", remaining, reset);
                    } else {
                        warn!("GitHub API rate limit low: {} requests remaining", remaining);
                    }
                }
            }
        }

        Ok(response)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    content_type: Option<String>,
    download_count: u64,
}

impl From<GitHubRelease> for VersionInfo {
    fn from(release: GitHubRelease) -> Self {
        Self {
            version: release.tag_name,
            name: release.name,
            body: release.body,
            published_at: release.published_at,
            assets: release.assets.into_iter().map(|a| a.into()).collect(),
        }
    }
}

impl From<GitHubAsset> for ReleaseAsset {
    fn from(asset: GitHubAsset) -> Self {
        Self {
            name: asset.name,
            download_url: asset.browser_download_url,
            size: asset.size,
            content_type: asset.content_type,
            download_count: asset.download_count,
        }
    }
}

impl VersionResolver for GitHubClient {
    fn get_latest_version(&self) -> Result<VersionInfo> {
        let response = self.make_request("/releases/latest")?;
        let release: GitHubRelease = response
            .into_json()
            .context("Failed to parse GitHub release response")?;
        Ok(release.into())
    }

    fn get_version_by_tag(&self, tag: &str) -> Result<VersionInfo> {
        let response = self.make_request(&format!("/releases/tags/{}", tag))?;
        let release: GitHubRelease = response
            .into_json()
            .context("Failed to parse GitHub release response")?;
        Ok(release.into())
    }

    fn find_asset<'a>(&self, version_info: &'a VersionInfo, pattern: &str) -> Option<&'a ReleaseAsset> {
        version_info.assets.iter().find(|asset| match_glob(pattern, &asset.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_client_new() {
        let client = GitHubClient::new("owner/repo", None, None);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.owner, "owner");
        assert_eq!(client.repo, "repo");
        assert_eq!(client.api_url, "https://api.github.com");
        assert!(client.auth_token.is_none());
    }

    #[test]
    fn test_github_client_with_custom_api_url() {
        let client = GitHubClient::new("owner/repo", Some("https://github.example.com/api/v3"), None).unwrap();
        assert_eq!(client.api_url, "https://github.example.com/api/v3");
    }

    #[test]
    fn test_github_client_with_auth_token() {
        let client = GitHubClient::new("owner/repo", None, Some("ghp_test123")).unwrap();
        assert_eq!(client.auth_token.as_deref(), Some("ghp_test123"));
    }

    #[test]
    fn test_github_client_trailing_slash_api_url() {
        let client = GitHubClient::new("owner/repo", Some("https://github.example.com/api/v3/"), None).unwrap();
        assert_eq!(client.api_url, "https://github.example.com/api/v3");
    }

    #[test]
    fn test_github_client_invalid_repo() {
        let client = GitHubClient::new("invalid-format", None, None);
        assert!(client.is_err());
    }

    #[test]
    fn test_github_client_invalid_repo_too_many_parts() {
        let client = GitHubClient::new("a/b/c", None, None);
        assert!(client.is_err());
    }

    // ── JSON parsing tests (no network required) ──────────────────────

    #[test]
    fn test_parse_github_release_json() {
        let json = r#"{
            "tag_name": "v1.2.3",
            "name": "Release 1.2.3",
            "body": "Bug fixes and improvements",
            "published_at": "2026-01-15T10:00:00Z",
            "assets": [
                {
                    "name": "myapp-v1.2.3-win-x64.zip",
                    "browser_download_url": "https://github.com/owner/repo/releases/download/v1.2.3/myapp-v1.2.3-win-x64.zip",
                    "size": 12345678,
                    "content_type": "application/zip",
                    "download_count": 42
                },
                {
                    "name": "myapp-v1.2.3-win-x86.zip",
                    "browser_download_url": "https://github.com/owner/repo/releases/download/v1.2.3/myapp-v1.2.3-win-x86.zip",
                    "size": 11223344,
                    "content_type": "application/zip",
                    "download_count": 17
                }
            ]
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        let version_info: VersionInfo = release.into();

        assert_eq!(version_info.version, "v1.2.3");
        assert_eq!(version_info.name.as_deref(), Some("Release 1.2.3"));
        assert_eq!(version_info.body.as_deref(), Some("Bug fixes and improvements"));
        assert_eq!(version_info.published_at.as_deref(), Some("2026-01-15T10:00:00Z"));
        assert_eq!(version_info.assets.len(), 2);
        assert_eq!(version_info.assets[0].name, "myapp-v1.2.3-win-x64.zip");
        assert_eq!(version_info.assets[0].size, 12345678);
        assert_eq!(version_info.assets[0].download_count, 42);
        assert_eq!(version_info.assets[1].name, "myapp-v1.2.3-win-x86.zip");
    }

    #[test]
    fn test_parse_github_release_minimal_json() {
        // GitHub API always returns tag_name and assets, but name/body/published_at may be null
        let json = r#"{
            "tag_name": "v0.1.0",
            "name": null,
            "body": null,
            "published_at": null,
            "assets": []
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        let version_info: VersionInfo = release.into();

        assert_eq!(version_info.version, "v0.1.0");
        assert!(version_info.name.is_none());
        assert!(version_info.body.is_none());
        assert!(version_info.published_at.is_none());
        assert_eq!(version_info.assets.len(), 0);
    }

    #[test]
    fn test_find_asset_glob_pattern() {
        let client = GitHubClient::new("owner/repo", None, None).unwrap();
        let version_info = VersionInfo {
            version: "v1.0.0".to_string(),
            name: None,
            body: None,
            published_at: None,
            assets: vec![
                ReleaseAsset {
                    name: "myapp-v1.0.0-win-x64.zip".to_string(),
                    download_url: "https://example.com/x64.zip".to_string(),
                    size: 100,
                    content_type: Some("application/zip".to_string()),
                    download_count: 10,
                },
                ReleaseAsset {
                    name: "myapp-v1.0.0-win-x86.zip".to_string(),
                    download_url: "https://example.com/x86.zip".to_string(),
                    size: 90,
                    content_type: Some("application/zip".to_string()),
                    download_count: 5,
                },
                ReleaseAsset {
                    name: "myapp-v1.0.0-linux-x64.tar.gz".to_string(),
                    download_url: "https://example.com/linux.tar.gz".to_string(),
                    size: 80,
                    content_type: Some("application/gzip".to_string()),
                    download_count: 3,
                },
            ],
        };

        // Match Windows x64
        let asset = client.find_asset(&version_info, "*-win-x64.zip");
        assert!(asset.is_some());
        assert_eq!(asset.unwrap().name, "myapp-v1.0.0-win-x64.zip");

        // Match any .zip
        let asset = client.find_asset(&version_info, "*.zip");
        assert!(asset.is_some());
        assert_eq!(asset.unwrap().name, "myapp-v1.0.0-win-x64.zip"); // First match

        // Match Linux
        let asset = client.find_asset(&version_info, "*-linux-*");
        assert!(asset.is_some());
        assert_eq!(asset.unwrap().name, "myapp-v1.0.0-linux-x64.tar.gz");

        // No match
        let asset = client.find_asset(&version_info, "*-mac-*");
        assert!(asset.is_none());
    }
}
