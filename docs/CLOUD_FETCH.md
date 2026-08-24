# Cloud-Fetch Installer Guide

Velocity supports **cloud-fetch installers** — tiny bootstrapper installers (similar to Ninite) that download files at install time from Git release tags or HTTP URLs, enabling auto-updates via the installer.

## Overview

### Dual-Mode System

```
velocity build --mode bundled  → Traditional installer (all files bundled)
velocity build --mode fetch    → Cloud-fetch installer (tiny bootstrapper)
velocity build --mode hybrid   → Bundle critical files + fetch optional
```

### Cloud-Fetch Flow

```
1. Installer starts (tiny, ~1-2MB)
2. Read manifest (bundled in installer)
3. Check installed version (registry/file)
4. Query remote for latest version (Git API or HTTP)
5. Compare versions (semver)
6. If newer: download assets with progress UI
7. Verify SHA256 checksums
8. Extract/install files
9. Update version info
10. Launch app (optional)
```

---

## Configuration

### Git Release Mode (GitHub/GitLab/Bitbucket/Gitea)

```toml
[app]
name = "MyApp"
version = "1.0.0"

[fetch]
mode = "git-release"
platform = "github"  # or "gitlab", "bitbucket", "gitea"
repo = "user/myapp"
asset_pattern = "{app}-{version}-win-{arch}.zip"
# Optional: override API URL for self-hosted instances
api_url = "https://github.example.com/api/v3"

[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" },
  { pattern = "README.md", dest = "." }
]

# Optional: version check interval (for auto-update)
[fetch.update]
check_interval = "24h"  # or "1d", "1w", "never"
auto_download = false
auto_install = false
show_notification = true
```

### Generic HTTP Mode

```toml
[fetch]
mode = "url"
base_url = "https://releases.example.com/myapp"
version_url = "https://releases.example.com/myapp/version.txt"
asset_pattern = "{app}-{version}-win-{arch}.zip"
checksum_url = "https://releases.example.com/myapp/{version}/SHA256SUMS"

[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" }
]
```

### Hybrid Mode (Bundle Critical + Fetch Optional)

```toml
[fetch]
mode = "hybrid"
platform = "github"
repo = "user/myapp"

# Bundle these files in installer
[fetch.bundle]
source = ["bin/critical.exe", "bin/runtime.dll"]

# Fetch these files at install time
[fetch.files]
download = [
  { pattern = "optional-feature.pack", dest = "features/" },
  { pattern = "language-*.pack", dest = "lang/" }
]
```

---

## Git Platforms

### GitHub
- **API**: `GET /repos/{owner}/{repo}/releases/latest`
- **Auth**: Optional token for private repos (`auth_token` field)
- **Enterprise**: Set `api_url` to your GitHub Enterprise API endpoint

### GitLab
- **API**: `GET /projects/{id}/releases/permalink/latest`
- **Auth**: Uses `PRIVATE-TOKEN` header
- **Self-hosted**: Set `api_url` to your GitLab instance API URL

### Bitbucket
- **API**: Uses tags as version indicators (no releases API)
- **Downloads**: Fetches from `/downloads` endpoint
- **Auth**: Bearer token support

### Gitea
- **API**: Compatible with GitHub API (`/repos/{owner}/{repo}/releases/latest`)
- **Required**: `api_url` must be set (self-hosted instances)
- **Auth**: Token support for private repos

---

## Auto-Update

Configure auto-update behavior in the `[fetch.update]` section:

```toml
[fetch.update]
check_interval = "24h"     # How often to check: "1h", "24h", "1d", "1w", "never"
auto_download = false       # Download updates without prompting
auto_install = false        # Install updates without prompting
show_notification = true    # Show notification when update available
```

### Update Priority

Updates are classified by priority:
- **Major**: Breaking changes (1.x.x → 2.0.0)
- **Minor**: New features (1.0.x → 1.1.0)
- **Patch**: Bug fixes (1.0.0 → 1.0.1)

---

## Build Commands

```bash
# Traditional bundled installer
velocity build --mode bundled

# Cloud-fetch installer (tiny)
velocity build --mode fetch

# Hybrid (bundle critical + fetch optional)
velocity build --mode hybrid
```

---

## Caching

Downloaded files are cached in `~/.velocity/cache/` to avoid re-downloads:
- Cache key: SHA256 hash of the download URL
- Automatic cleanup of files older than 30 days
- Default cache size limit: 1GB
- Checksum verification on cached files

---

## Error Handling

The cloud-fetch installer handles these error scenarios:

| Error | Behavior |
|-------|----------|
| Network failure | Retry 3 times with exponential backoff |
| Checksum mismatch | Abort installation, delete corrupt file |
| API rate limit | Use cached version info, support auth tokens |
| No releases found | Show error message, abort install |
| Partial download | Resume from where it left off (HTTP Range) |

---

## Architecture

### Components

- **`velocity-config`** — `[fetch]` section parsing (`FetchConfig`, `FetchMode`, `GitPlatform`)
- **`velocity-core::fetch`** — Git platform API clients and download manager
  - `GitHubClient`, `GitLabClient`, `BitbucketClient`, `GiteaClient` — Platform-specific API clients
  - `UrlClient` — Direct HTTP version resolver
  - `DownloadManager` — Download with caching and retry
  - `VersionResolver` trait — Unified interface for all platforms
- **`velocity-runtime::fetch_installer`** — Runtime integration for cloud-fetch installs

### Dependencies

- `ureq` — HTTP client (synchronous, with TLS and JSON support)
- `semver` — Semantic version parsing and comparison
- `sha2` — SHA256 checksum verification
