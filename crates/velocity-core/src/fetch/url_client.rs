//! URL-based version resolver for direct HTTP downloads.
//!
//! Fetches version information from a plain-text version file
//! and constructs download URLs from a base URL pattern.

use anyhow::{Context, Result};
use tracing::debug;
use super::{VersionInfo, ReleaseAsset, VersionResolver, match_glob, substitute_placeholders, hardened_agent};

/// URL-based version resolver that fetches version info from a text file.
///
/// The version file should contain just the version string (e.g., "1.0.0" or "v1.0.0").
/// Download URLs are constructed from the base URL and asset pattern with
/// placeholder substitution for `{version}`, `{app}`, and `{arch}`.
pub struct UrlClient {
    /// Base URL for downloads (e.g., "https://releases.example.com/myapp")
    base_url: String,
    /// URL to the version text file
    version_url: String,
    /// Asset pattern with placeholders: {app}, {version}, {arch}
    asset_pattern: Option<String>,
    /// Optional checksum URL template (same placeholders as asset_pattern)
    checksum_url: Option<String>,
    /// HTTP client
    client: ureq::Agent,
}

impl UrlClient {
    /// Create a new URL-based version resolver.
    ///
    /// # Arguments
    /// * `base_url` - Base URL for file downloads
    /// * `version_url` - URL to the version text file
    /// * `asset_pattern` - Optional asset filename pattern with placeholders
    /// * `checksum_url` - Optional checksum file URL template
    pub fn new(
        base_url: &str,
        version_url: &str,
        asset_pattern: Option<&str>,
        checksum_url: Option<&str>,
    ) -> Self {
        let client = hardened_agent();
        
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            version_url: version_url.to_string(),
            asset_pattern: asset_pattern.map(|s| s.to_string()),
            checksum_url: checksum_url.map(|s| s.to_string()),
            client,
        }
    }

    /// Fetch the version string from the version URL.
    fn fetch_version_string(&self) -> Result<String> {
        let response = self.client.get(&self.version_url)
            .set("User-Agent", "Velocity-Installer/1.0")
            .call()
            .with_context(|| format!("Failed to fetch version from: {}", self.version_url))?;
        
        let body = response.into_string()
            .context("Failed to read version response")?;
        
        let version = body.trim().to_string();
        if version.is_empty() {
            anyhow::bail!("Version file at {} is empty", self.version_url);
        }
        
        debug!("Fetched version: {} from {}", version, self.version_url);
        Ok(version)
    }

    /// Fetch the checksum for a given asset from the checksum URL.
    ///
    /// The checksum URL may contain the same placeholders as the asset pattern.
    /// The response is expected to be either:
    /// - A bare SHA256 hash
    /// - SHA256SUMS-style format: `<hash>  <filename>`
    pub fn fetch_checksum(&self, version: &str, app_name: &str, filename: &str) -> Option<String> {
        let checksum_url_template = self.checksum_url.as_ref()?;
        
        let url = substitute_placeholders(checksum_url_template, version, app_name);
        
        debug!("Fetching checksum from: {}", url);
        
        let response = self.client.get(&url)
            .set("User-Agent", "Velocity-Installer/1.0")
            .call()
            .ok()?;
        
        let body = response.into_string().ok()?;
        
        // Try to find the checksum for the specific file in SHA256SUMS format
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            // SHA256SUMS format: "<hash>  <filename>" or "<hash> *<filename>"
            let parts: Vec<&str> = line.splitn(2, |c: char| c == ' ' || c == '\t').collect();
            if parts.len() == 2 {
                let hash = parts[0].trim();
                let listed_file = parts[1].trim_start_matches('*').trim();
                
                if hash.len() == 64 && listed_file == filename {
                    debug!("Found checksum for {}: {}", filename, hash);
                    return Some(hash.to_lowercase());
                }
            }
            
            // If the entire body is just a hash (single line, 64 hex chars)
            if body.lines().count() == 1 && body.trim().len() == 64 {
                debug!("Checksum file contains bare hash: {}", body.trim());
                return Some(body.trim().to_lowercase());
            }
        }
        
        debug!("No checksum found for {} in {}", filename, url);
        None
    }

    /// Build the asset filename by substituting placeholders.
    fn build_asset_name(&self, version: &str, app_name: &str) -> Option<String> {
        let pattern = self.asset_pattern.as_ref()?;
        Some(substitute_placeholders(pattern, version, app_name))
    }
}

impl VersionResolver for UrlClient {
    fn get_latest_version(&self) -> Result<VersionInfo> {
        let version = self.fetch_version_string()?;
        
        // Construct asset from base URL + resolved pattern
        let assets = if let Some(name) = self.build_asset_name(&version, "") {
            let url = format!("{}/{}", self.base_url, name);
            vec![ReleaseAsset {
                name,
                download_url: url,
                size: 0, // Unknown until HEAD request
                content_type: None,
                download_count: 0,
            }]
        } else {
            vec![]
        };

        Ok(VersionInfo {
            version,
            name: None,
            body: None,
            published_at: None,
            assets,
        })
    }

    fn get_version_by_tag(&self, tag: &str) -> Result<VersionInfo> {
        let assets = if let Some(name) = self.build_asset_name(tag, "") {
            let url = format!("{}/{}", self.base_url, name);
            vec![ReleaseAsset {
                name,
                download_url: url,
                size: 0,
                content_type: None,
                download_count: 0,
            }]
        } else {
            vec![]
        };

        Ok(VersionInfo {
            version: tag.to_string(),
            name: None,
            body: None,
            published_at: None,
            assets,
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
    fn test_url_client_new() {
        let client = UrlClient::new(
            "https://releases.example.com/myapp",
            "https://releases.example.com/myapp/version.txt",
            Some("{app}-{version}-win-x64.zip"),
            None,
        );
        assert_eq!(client.base_url, "https://releases.example.com/myapp");
        assert_eq!(client.version_url, "https://releases.example.com/myapp/version.txt");
    }

    #[test]
    fn test_url_client_trailing_slash() {
        let client = UrlClient::new(
            "https://releases.example.com/myapp/",
            "https://releases.example.com/myapp/version.txt",
            None,
            None,
        );
        assert_eq!(client.base_url, "https://releases.example.com/myapp");
    }

    #[test]
    fn test_url_client_placeholder_substitution() {
        let client = UrlClient::new(
            "https://releases.example.com/myapp",
            "https://releases.example.com/myapp/version.txt",
            Some("myapp-{version}-win-{arch}.zip"),
            None,
        );
        
        // Build asset name with version substitution
        let name = client.build_asset_name("v1.0.0", "myapp").unwrap();
        assert!(name.contains("1.0.0"));
        assert!(!name.contains("{version}"));
        assert!(!name.contains("{arch}"));
        assert!(name.ends_with(".zip"));
    }

    #[test]
    fn test_url_client_asset_name_substitution() {
        let client = UrlClient::new(
            "https://releases.example.com/myapp",
            "https://releases.example.com/myapp/version.txt",
            Some("myapp-{version}-win-{arch}.zip"),
            None,
        );
        
        let name = client.build_asset_name("v2.1.0", "myapp").unwrap();
        assert!(name.contains("2.1.0"));
        assert!(!name.contains("{version}"));
        assert!(!name.contains("{arch}"));
        assert!(name.ends_with(".zip"));
    }

    #[test]
    fn test_url_client_get_version_by_tag() {
        let client = UrlClient::new(
            "https://releases.example.com/myapp",
            "https://releases.example.com/myapp/version.txt",
            Some("myapp-{version}-win-x64.zip"),
            None,
        );
        
        let info = client.get_version_by_tag("v1.0.0").unwrap();
        assert_eq!(info.version, "v1.0.0");
        assert_eq!(info.assets.len(), 1);
        // The asset name should have placeholders substituted
        assert!(info.assets[0].name.contains("1.0.0"));
        assert!(!info.assets[0].name.contains("{version}"));
    }

    #[test]
    fn test_url_client_no_pattern() {
        let client = UrlClient::new(
            "https://releases.example.com/myapp",
            "https://releases.example.com/myapp/version.txt",
            None,
            None,
        );
        
        assert!(client.build_asset_name("1.0.0", "app").is_none());
    }
}
