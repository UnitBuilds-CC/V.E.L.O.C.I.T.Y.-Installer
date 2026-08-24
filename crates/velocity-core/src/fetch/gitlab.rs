//! GitLab API client for fetching release information.

use anyhow::{Context, Result};
use serde::Deserialize;
use super::{VersionInfo, ReleaseAsset, VersionResolver, match_glob, hardened_agent};

/// GitLab API client for fetching release information.
pub struct GitLabClient {
    namespace: String,
    project: String,
    api_url: String,
    auth_token: Option<String>,
    client: ureq::Agent,
}

impl GitLabClient {
    /// Create a new GitLab API client.
    ///
    /// # Arguments
    /// * `repo` - Repository in "namespace/project" format
    /// * `api_url` - Optional custom API URL for self-hosted GitLab
    /// * `auth_token` - Optional authentication token for private repositories
    pub fn new(repo: &str, api_url: Option<&str>, auth_token: Option<&str>) -> Result<Self> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid repository format: {}. Expected 'namespace/project'", repo);
        }

        let api_url = api_url
            .map(|s| s.to_string())
            .unwrap_or_else(|| "https://gitlab.com/api/v4".to_string());

        let client = hardened_agent();

        Ok(Self {
            namespace: parts[0].to_string(),
            project: parts[1].to_string(),
            api_url,
            auth_token: auth_token.map(|s| s.to_string()),
            client,
        })
    }

    fn make_request(&self, endpoint: &str) -> Result<ureq::Response> {
        let project_id = format!("{}/{}", self.namespace, self.project);
        let url = format!("{}/projects/{}{}", self.api_url, urlencoding::encode(&project_id), endpoint);
        
        let mut req = self.client.get(&url);
        
        if let Some(ref token) = self.auth_token {
            req = req.set("PRIVATE-TOKEN", token);
        }
        
        req.call()
            .with_context(|| format!("Failed to fetch from GitLab API: {}", url))
    }
}

#[derive(Debug, Deserialize)]
struct GitLabRelease {
    tag_name: String,
    name: Option<String>,
    description: Option<String>,
    released_at: Option<String>,
    assets: GitLabAssets,
}

#[derive(Debug, Deserialize)]
struct GitLabAssets {
    links: Vec<GitLabLink>,
}

#[derive(Debug, Deserialize)]
struct GitLabLink {
    name: String,
    url: String,
}

impl From<GitLabRelease> for VersionInfo {
    fn from(release: GitLabRelease) -> Self {
        Self {
            version: release.tag_name,
            name: release.name,
            body: release.description,
            published_at: release.released_at,
            assets: release.assets.links.into_iter().map(|l| ReleaseAsset {
                name: l.name,
                download_url: l.url,
                size: 0, // GitLab doesn't provide size in release links
                content_type: None,
                download_count: 0,
            }).collect(),
        }
    }
}

impl VersionResolver for GitLabClient {
    fn get_latest_version(&self) -> Result<VersionInfo> {
        let response = self.make_request("/releases/permalink/latest")?;
        let release: GitLabRelease = response
            .into_json()
            .context("Failed to parse GitLab release response")?;
        Ok(release.into())
    }

    fn get_version_by_tag(&self, tag: &str) -> Result<VersionInfo> {
        let response = self.make_request(&format!("/releases/{}", tag))?;
        let release: GitLabRelease = response
            .into_json()
            .context("Failed to parse GitLab release response")?;
        Ok(release.into())
    }

    fn find_asset<'a>(&self, version_info: &'a VersionInfo, pattern: &str) -> Option<&'a ReleaseAsset> {
        version_info.assets.iter().find(|asset| match_glob(pattern, &asset.name))
    }
}

// URL encoding for GitLab project IDs — encodes all characters that are not
// unreserved in RFC 3986 (alphanumeric, hyphen, period, underscore, tilde).
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitlab_client_new() {
        let client = GitLabClient::new("namespace/project", None, None).unwrap();
        assert_eq!(client.namespace, "namespace");
        assert_eq!(client.project, "project");
        assert_eq!(client.api_url, "https://gitlab.com/api/v4");
    }

    #[test]
    fn test_gitlab_client_custom_api_url() {
        let client = GitLabClient::new("ns/proj", Some("https://gitlab.example.com/api/v4"), None).unwrap();
        assert_eq!(client.api_url, "https://gitlab.example.com/api/v4");
    }

    #[test]
    fn test_gitlab_client_with_token() {
        let client = GitLabClient::new("ns/proj", None, Some("glpat-test123")).unwrap();
        assert_eq!(client.auth_token.as_deref(), Some("glpat-test123"));
    }

    #[test]
    fn test_gitlab_client_invalid_repo() {
        assert!(GitLabClient::new("invalid-format", None, None).is_err());
        assert!(GitLabClient::new("a/b/c", None, None).is_err());
    }

    #[test]
    fn test_urlencoding_basic() {
        assert_eq!(urlencoding::encode("hello"), "hello");
        assert_eq!(urlencoding::encode("hello/world"), "hello%2Fworld");
    }

    #[test]
    fn test_urlencoding_special_chars() {
        assert_eq!(urlencoding::encode("my project"), "my%20project");
        assert_eq!(urlencoding::encode("a+b"), "a%2Bb");
        assert_eq!(urlencoding::encode("test#tag"), "test%23tag");
    }

    #[test]
    fn test_urlencoding_preserves_unreserved() {
        // RFC 3986 unreserved: A-Z a-z 0-9 - . _ ~
        assert_eq!(urlencoding::encode("a-b.c_d~e"), "a-b.c_d~e");
        assert_eq!(urlencoding::encode("ABC123"), "ABC123");
    }

    // ── JSON parsing tests ────────────────────────────────────────────

    #[test]
    fn test_parse_gitlab_release_json() {
        let json = r#"{
            "tag_name": "v2.0.0",
            "name": "Version 2.0",
            "description": "Major release with new features",
            "released_at": "2026-06-01T12:00:00Z",
            "assets": {
                "links": [
                    {
                        "name": "myapp-v2.0.0-win-x64.zip",
                        "url": "https://gitlab.com/ns/proj/-/releases/v2.0.0/downloads/myapp-v2.0.0-win-x64.zip"
                    },
                    {
                        "name": "myapp-v2.0.0-linux-x64.tar.gz",
                        "url": "https://gitlab.com/ns/proj/-/releases/v2.0.0/downloads/myapp-v2.0.0-linux-x64.tar.gz"
                    }
                ]
            }
        }"#;

        let release: GitLabRelease = serde_json::from_str(json).unwrap();
        let version_info: VersionInfo = release.into();

        assert_eq!(version_info.version, "v2.0.0");
        assert_eq!(version_info.name.as_deref(), Some("Version 2.0"));
        assert_eq!(version_info.body.as_deref(), Some("Major release with new features"));
        assert_eq!(version_info.assets.len(), 2);
        assert_eq!(version_info.assets[0].name, "myapp-v2.0.0-win-x64.zip");
        // GitLab doesn't provide size in release links
        assert_eq!(version_info.assets[0].size, 0);
    }

    #[test]
    fn test_find_asset_gitlab() {
        let client = GitLabClient::new("ns/proj", None, None).unwrap();
        let version_info = VersionInfo {
            version: "v2.0.0".to_string(),
            name: None,
            body: None,
            published_at: None,
            assets: vec![
                ReleaseAsset {
                    name: "myapp-v2.0.0-win-x64.zip".to_string(),
                    download_url: "https://example.com/win.zip".to_string(),
                    size: 0,
                    content_type: None,
                    download_count: 0,
                },
            ],
        };

        assert!(client.find_asset(&version_info, "*.zip").is_some());
        assert!(client.find_asset(&version_info, "*-win-*").is_some());
        assert!(client.find_asset(&version_info, "*.tar.gz").is_none());
    }
}
