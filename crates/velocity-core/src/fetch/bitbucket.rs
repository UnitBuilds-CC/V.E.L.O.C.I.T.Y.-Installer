//! Bitbucket API client for fetching release information.

use anyhow::{Context, Result};
use serde::Deserialize;
use super::{VersionInfo, ReleaseAsset, VersionResolver, match_glob, hardened_agent};

/// Bitbucket API client for fetching release information.
pub struct BitbucketClient {
    workspace: String,
    repo: String,
    api_url: String,
    auth_token: Option<String>,
    client: ureq::Agent,
}

impl BitbucketClient {
    /// Create a new Bitbucket API client.
    ///
    /// # Arguments
    /// * `repo` - Repository in "workspace/repo" format
    /// * `api_url` - Optional custom API URL for Bitbucket Server
    /// * `auth_token` - Optional authentication token for private repositories
    pub fn new(repo: &str, api_url: Option<&str>, auth_token: Option<&str>) -> Result<Self> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid repository format: {}. Expected 'workspace/repo'", repo);
        }

        let api_url = api_url
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "https://api.bitbucket.org/2.0".to_string());

        let client = hardened_agent();

        Ok(Self {
            workspace: parts[0].to_string(),
            repo: parts[1].to_string(),
            api_url,
            auth_token: auth_token.map(|s| s.to_string()),
            client,
        })
    }

    fn make_request(&self, endpoint: &str) -> Result<ureq::Response> {
        let url = format!("{}/repositories/{}/{}{}", self.api_url, self.workspace, self.repo, endpoint);
        
        let mut req = self.client.get(&url);
        
        if let Some(ref token) = self.auth_token {
            req = req.set("Authorization", &format!("Bearer {}", token));
        }
        
        req.call()
            .with_context(|| format!("Failed to fetch from Bitbucket API: {}", url))
    }
}

#[derive(Debug, Deserialize)]
struct BitbucketDownloads {
    values: Vec<BitbucketDownload>,
}

#[derive(Debug, Deserialize)]
struct BitbucketDownload {
    name: String,
    links: BitbucketLinks,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct BitbucketLinks {
    #[serde(rename = "self")]
    self_link: BitbucketLink,
}

#[derive(Debug, Deserialize)]
struct BitbucketLink {
    href: String,
}

#[derive(Debug, Deserialize)]
struct BitbucketRef {
    name: String,
    target: BitbucketTarget,
}

#[derive(Debug, Deserialize)]
struct BitbucketTarget {
    #[allow(dead_code)]
    hash: String,
    date: Option<String>,
    message: Option<String>,
}

impl VersionResolver for BitbucketClient {
    fn get_latest_version(&self) -> Result<VersionInfo> {
        // Bitbucket doesn't have a "releases" API like GitHub
        // We use tags as version indicators
        let response = self.make_request("/refs/tags?sort=-target.date&pagelen=1")?;
        let refs: serde_json::Value = response
            .into_json()
            .context("Failed to parse Bitbucket tags response")?;
        
        let values = refs["values"].as_array()
            .context("No tags found in repository")?;
        
        if values.is_empty() {
            anyhow::bail!("No tags found in repository");
        }

        let latest_tag = &values[0];
        let tag_name = latest_tag["name"].as_str()
            .context("Invalid tag format")?;
        
        // Get downloads for this repository
        let downloads_response = self.make_request("/downloads")?;
        let downloads: BitbucketDownloads = downloads_response
            .into_json()
            .context("Failed to parse Bitbucket downloads response")?;

        Ok(VersionInfo {
            version: tag_name.to_string(),
            name: Some(tag_name.to_string()),
            body: None,
            published_at: latest_tag["target"]["date"].as_str().map(|s: &str| s.to_string()),
            assets: downloads.values.into_iter().map(|d| ReleaseAsset {
                name: d.name,
                download_url: d.links.self_link.href,
                size: d.size,
                content_type: None,
                download_count: 0,
            }).collect(),
        })
    }

    fn get_version_by_tag(&self, tag: &str) -> Result<VersionInfo> {
        let response = self.make_request(&format!("/refs/tags/{}", tag))?;
        let tag_ref: BitbucketRef = response
            .into_json()
            .context("Failed to parse Bitbucket tag response")?;
        
        // Get downloads
        let downloads_response = self.make_request("/downloads")?;
        let downloads: BitbucketDownloads = downloads_response
            .into_json()
            .context("Failed to parse Bitbucket downloads response")?;

        Ok(VersionInfo {
            version: tag_ref.name.clone(),
            name: Some(tag_ref.name),
            body: tag_ref.target.message,
            published_at: tag_ref.target.date,
            assets: downloads.values.into_iter().map(|d| ReleaseAsset {
                name: d.name,
                download_url: d.links.self_link.href,
                size: d.size,
                content_type: None,
                download_count: 0,
            }).collect(),
        })
    }

    fn find_asset<'a>(&self, version_info: &'a VersionInfo, pattern: &str) -> Option<&'a ReleaseAsset> {
        version_info.assets.iter().find(|asset| match_glob(pattern, &asset.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitbucket_client_new() {
        let client = BitbucketClient::new("workspace/repo", None, None).unwrap();
        assert_eq!(client.workspace, "workspace");
        assert_eq!(client.repo, "repo");
        assert_eq!(client.api_url, "https://api.bitbucket.org/2.0");
    }

    #[test]
    fn test_bitbucket_client_custom_api() {
        let client = BitbucketClient::new("ws/rp", Some("https://bitbucket.example.com/rest/api/1.0"), None).unwrap();
        assert_eq!(client.api_url, "https://bitbucket.example.com/rest/api/1.0");
    }

    #[test]
    fn test_bitbucket_client_with_token() {
        let client = BitbucketClient::new("ws/rp", None, Some("bb_token")).unwrap();
        assert_eq!(client.auth_token.as_deref(), Some("bb_token"));
    }

    #[test]
    fn test_bitbucket_client_invalid_repo() {
        assert!(BitbucketClient::new("invalid", None, None).is_err());
        assert!(BitbucketClient::new("a/b/c", None, None).is_err());
    }

    // ── JSON parsing tests ────────────────────────────────────────────

    #[test]
    fn test_parse_bitbucket_downloads_json() {
        let json = r#"{
            "values": [
                {
                    "name": "myapp-v1.0.0-win-x64.zip",
                    "links": {
                        "self": {
                            "href": "https://api.bitbucket.org/2.0/repositories/ws/repo/downloads/myapp-v1.0.0-win-x64.zip"
                        }
                    },
                    "size": 5000000
                }
            ]
        }"#;

        let downloads: BitbucketDownloads = serde_json::from_str(json).unwrap();
        assert_eq!(downloads.values.len(), 1);
        assert_eq!(downloads.values[0].name, "myapp-v1.0.0-win-x64.zip");
        assert_eq!(downloads.values[0].size, 5000000);
        assert_eq!(downloads.values[0].links.self_link.href,
            "https://api.bitbucket.org/2.0/repositories/ws/repo/downloads/myapp-v1.0.0-win-x64.zip");
    }

    #[test]
    fn test_parse_bitbucket_ref_json() {
        let json = r#"{
            "name": "v1.0.0",
            "target": {
                "hash": "abc123def456",
                "date": "2026-01-15T10:00:00+00:00",
                "message": "Release v1.0.0"
            }
        }"#;

        let tag_ref: BitbucketRef = serde_json::from_str(json).unwrap();
        assert_eq!(tag_ref.name, "v1.0.0");
        assert_eq!(tag_ref.target.hash, "abc123def456");
        assert_eq!(tag_ref.target.message.as_deref(), Some("Release v1.0.0"));
    }

    #[test]
    fn test_find_asset_bitbucket() {
        let client = BitbucketClient::new("ws/repo", None, None).unwrap();
        let version_info = VersionInfo {
            version: "v1.0.0".to_string(),
            name: None,
            body: None,
            published_at: None,
            assets: vec![
                ReleaseAsset {
                    name: "myapp-v1.0.0-win-x64.zip".to_string(),
                    download_url: "https://example.com/dl.zip".to_string(),
                    size: 100,
                    content_type: None,
                    download_count: 0,
                },
            ],
        };

        assert!(client.find_asset(&version_info, "*.zip").is_some());
        assert!(client.find_asset(&version_info, "*.exe").is_none());
    }
}
