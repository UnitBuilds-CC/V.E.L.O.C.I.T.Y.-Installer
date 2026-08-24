//! Gitea API client for fetching release information.
//!
//! Gitea's API is largely compatible with the GitHub API, making this
//! implementation similar to the GitHub client with adjusted defaults.

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::warn;
use super::{VersionInfo, ReleaseAsset, VersionResolver, match_glob, hardened_agent};

/// Gitea API client for fetching release information.
pub struct GiteaClient {
    owner: String,
    repo: String,
    api_url: String,
    auth_token: Option<String>,
    client: ureq::Agent,
}

impl GiteaClient {
    /// Create a new Gitea API client.
    ///
    /// # Arguments
    /// * `repo` - Repository in "owner/repo" format
    /// * `api_url` - Custom API URL for the Gitea instance (required for self-hosted)
    /// * `auth_token` - Optional authentication token for private repositories
    pub fn new(repo: &str, api_url: Option<&str>, auth_token: Option<&str>) -> Result<Self> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid repository format: {}. Expected 'owner/repo'", repo);
        }

        // Gitea requires a custom API URL since it's self-hosted
        let api_url = api_url
            .map(|s| s.trim_end_matches('/').to_string())
            .ok_or_else(|| anyhow::anyhow!(
                "Gitea requires an api_url for self-hosted instances (e.g., 'https://gitea.example.com/api/v1')"
            ))?;

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
            .set("User-Agent", "Velocity-Installer/1.0");
        
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("token {}", token));
        }
        
        let response = req.call()
            .with_context(|| format!("Failed to fetch from Gitea API: {}", url))?;

        // Check rate limit headers (Gitea uses same headers as GitHub)
        if let Some(remaining) = response.header("X-RateLimit-Remaining") {
            if let Ok(remaining) = remaining.parse::<u64>() {
                if remaining < 10 {
                    warn!("Gitea API rate limit low: {} requests remaining", remaining);
                }
            }
        }

        Ok(response)
    }
}

#[derive(Debug, Deserialize)]
struct GiteaRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GiteaAsset>,
}

#[derive(Debug, Deserialize)]
struct GiteaAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    content_type: Option<String>,
    download_count: u64,
}

impl From<GiteaRelease> for VersionInfo {
    fn from(release: GiteaRelease) -> Self {
        Self {
            version: release.tag_name,
            name: release.name,
            body: release.body,
            published_at: release.published_at,
            assets: release.assets.into_iter().map(|a| a.into()).collect(),
        }
    }
}

impl From<GiteaAsset> for ReleaseAsset {
    fn from(asset: GiteaAsset) -> Self {
        Self {
            name: asset.name,
            download_url: asset.browser_download_url,
            size: asset.size,
            content_type: asset.content_type,
            download_count: asset.download_count,
        }
    }
}

impl VersionResolver for GiteaClient {
    fn get_latest_version(&self) -> Result<VersionInfo> {
        let response = self.make_request("/releases/latest")?;
        let release: GiteaRelease = response
            .into_json()
            .context("Failed to parse Gitea release response")?;
        Ok(release.into())
    }

    fn get_version_by_tag(&self, tag: &str) -> Result<VersionInfo> {
        let response = self.make_request(&format!("/releases/tags/{}", tag))?;
        let release: GiteaRelease = response
            .into_json()
            .context("Failed to parse Gitea release response")?;
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
    fn test_gitea_client_new() {
        let client = GiteaClient::new("owner/repo", Some("https://gitea.example.com/api/v1"), None);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.owner, "owner");
        assert_eq!(client.repo, "repo");
        assert_eq!(client.api_url, "https://gitea.example.com/api/v1");
    }

    #[test]
    fn test_gitea_client_requires_api_url() {
        let client = GiteaClient::new("owner/repo", None, None);
        assert!(client.is_err());
    }

    #[test]
    fn test_gitea_client_invalid_repo() {
        let client = GiteaClient::new("invalid-format", Some("https://gitea.example.com/api/v1"), None);
        assert!(client.is_err());
    }

    #[test]
    fn test_gitea_client_trailing_slash() {
        let client = GiteaClient::new("owner/repo", Some("https://gitea.example.com/api/v1/"), None).unwrap();
        assert_eq!(client.api_url, "https://gitea.example.com/api/v1");
    }

    // ── JSON parsing tests (Gitea uses same format as GitHub) ─────────

    #[test]
    fn test_parse_gitea_release_json() {
        let json = r#"{
            "tag_name": "v3.0.0",
            "name": "Gitea Release 3.0",
            "body": "Self-hosted release notes",
            "published_at": "2026-03-01T08:00:00Z",
            "assets": [
                {
                    "name": "myapp-v3.0.0-win-x64.zip",
                    "browser_download_url": "https://gitea.example.com/owner/repo/releases/download/v3.0.0/myapp-v3.0.0-win-x64.zip",
                    "size": 9876543,
                    "content_type": "application/zip",
                    "download_count": 100
                }
            ]
        }"#;

        let release: GiteaRelease = serde_json::from_str(json).unwrap();
        let version_info: VersionInfo = release.into();

        assert_eq!(version_info.version, "v3.0.0");
        assert_eq!(version_info.name.as_deref(), Some("Gitea Release 3.0"));
        assert_eq!(version_info.assets.len(), 1);
        assert_eq!(version_info.assets[0].size, 9876543);
        assert_eq!(version_info.assets[0].download_count, 100);
    }

    #[test]
    fn test_find_asset_gitea() {
        let client = GiteaClient::new("owner/repo", Some("https://gitea.example.com/api/v1"), None).unwrap();
        let version_info = VersionInfo {
            version: "v3.0.0".to_string(),
            name: None,
            body: None,
            published_at: None,
            assets: vec![
                ReleaseAsset {
                    name: "myapp-v3.0.0-win-x64.zip".to_string(),
                    download_url: "https://example.com/win.zip".to_string(),
                    size: 100,
                    content_type: None,
                    download_count: 0,
                },
                ReleaseAsset {
                    name: "SHA256SUMS".to_string(),
                    download_url: "https://example.com/SHA256SUMS".to_string(),
                    size: 200,
                    content_type: None,
                    download_count: 0,
                },
            ],
        };

        assert!(client.find_asset(&version_info, "*.zip").is_some());
        assert!(client.find_asset(&version_info, "SHA256*").is_some());
        assert!(client.find_asset(&version_info, "*.exe").is_none());
    }
}
