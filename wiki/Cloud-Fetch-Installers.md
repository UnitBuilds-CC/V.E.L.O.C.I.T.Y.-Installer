# Cloud-Fetch Installers

Velocity supports **cloud-fetch installers** — tiny bootstrapper installers (similar to Ninite) that download files at install time from Git release tags or HTTP URLs. This enables automatic updates and keeps installer sizes small.

## Overview

### Dual-Mode System

```bash
velocity build --mode bundled  # Traditional installer (all files bundled)
velocity build --mode fetch    # Cloud-fetch installer (tiny bootstrapper)
velocity build --mode hybrid   # Bundle critical files + fetch optional
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

### Benefits

- **Tiny installer size** — 1-2MB bootstrapper vs 50-500MB bundled installer
- **Always up-to-date** — Downloads latest version at install time
- **Reduced bandwidth** — Only downloads what's needed
- **Auto-update support** — Built-in version checking and update notifications
- **Multiple platforms** — GitHub, GitLab, Bitbucket, Gitea, or generic HTTP

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

# Optional: authentication token for private repos
auth_token = "ghp_xxxxxxxxxxxx"

[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" },
  { pattern = "README.md", dest = "." }
]

# Optional: auto-update configuration
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

**API Endpoint:** `GET /repos/{owner}/{repo}/releases/latest`

**Configuration:**
```toml
[fetch]
platform = "github"
repo = "user/myapp"
# Optional: GitHub Enterprise
api_url = "https://github.example.com/api/v3"
# Optional: private repo token
auth_token = "ghp_xxxxxxxxxxxx"
```

**Features:**
- Automatic release detection
- Asset pattern matching
- Rate limit handling (with cached fallback)
- Private repository support with auth tokens

### GitLab

**API Endpoint:** `GET /projects/{id}/releases/permalink/latest`

**Configuration:**
```toml
[fetch]
platform = "gitlab"
repo = "user/myapp"  # or project ID
api_url = "https://gitlab.example.com/api/v4"
auth_token = "glpat-xxxxxxxxxxxx"  # PRIVATE-TOKEN header
```

**Features:**
- Self-hosted GitLab support
- RFC 3986 URL encoding
- Private token authentication

### Bitbucket

**API Endpoint:** Uses tags as version indicators (no releases API)

**Configuration:**
```toml
[fetch]
platform = "bitbucket"
repo = "user/myapp"
auth_token = "bearer-token"  # Optional
```

**Features:**
- Tag-based version detection
- Downloads endpoint
- Bearer token authentication

### Gitea

**API Endpoint:** Compatible with GitHub API (`/repos/{owner}/{repo}/releases/latest`)

**Configuration:**
```toml
[fetch]
platform = "gitea"
repo = "user/myapp"
api_url = "https://gitea.example.com/api/v1"  # Required for self-hosted
auth_token = "token"  # Optional
```

**Features:**
- GitHub-compatible API
- Self-hosted instance support
- Rate limit header parsing

---

## Asset Patterns

The `asset_pattern` field uses placeholders to match release assets:

| Placeholder | Description | Example |
|-------------|-------------|---------|
| `{app}` | Application name | `MyApp` |
| `{version}` | Version string | `1.0.0` |
| `{arch}` | Architecture | `x64`, `x86`, `arm64` |
| `{os}` | Operating system | `win`, `linux`, `macos` |

### Examples

```toml
# Windows x64
asset_pattern = "{app}-{version}-win-{arch}.zip"
# Matches: MyApp-1.0.0-win-x64.zip

# Cross-platform
asset_pattern = "{app}-{version}-{os}-{arch}.tar.gz"
# Matches: MyApp-1.0.0-win-x64.tar.gz

# Simple pattern
asset_pattern = "release-{version}.zip"
# Matches: release-1.0.0.zip
```

---

## File Selection

The `[fetch.files]` section defines which assets to download and where to extract them:

```toml
[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" },
  { pattern = "*.pak", dest = "resources/" },
  { pattern = "README.md", dest = "." }
]
```

**Fields:**
- `pattern` — Glob pattern to match asset filenames
- `dest` — Destination directory relative to install path

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
- **Major** — Breaking changes (1.x.x → 2.0.0)
- **Minor** — New features (1.0.x → 1.1.0)
- **Patch** — Bug fixes (1.0.0 → 1.0.1)

### Version Checking

The update checker:
1. Reads the installed version from registry or file
2. Queries the Git platform API for the latest release
3. Compares versions using semver
4. If newer version available, triggers update flow

---

## Caching

Downloaded files are cached in `~/.velocity/cache/` to avoid re-downloads:

| Feature | Description |
|---------|-------------|
| **Cache key** | SHA256 hash of the download URL |
| **Auto cleanup** | Files older than 30 days are removed |
| **Size limit** | Default 1GB max cache size |
| **Checksum verification** | Cached files are verified before use |

### Cache Location

- **Windows:** `%USERPROFILE%\.velocity\cache\`
- **Linux:** `~/.velocity/cache/`
- **macOS:** `~/.velocity/cache/`

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
| Invalid asset pattern | Show error with expected vs actual filenames |

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

### Download Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│                    Download Pipeline                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. VersionResolver queries Git platform API                │
│     └─→ Returns: version, asset URLs, checksums             │
│                                                              │
│  2. DownloadManager receives asset list                     │
│     ├─→ Check cache (SHA256 of URL)                         │
│     ├─→ If cached & valid: use cached file                  │
│     └─→ If not cached: download with retry                  │
│                                                              │
│  3. Download with progress                                  │
│     ├─→ HTTP Range requests for resume                      │
│     ├─→ Progress callback for UI                            │
│     └─→ SHA256 verification after download                  │
│                                                              │
│  4. Extract to install directory                            │
│     ├─→ Detect archive format (zip, tar, tar.gz, etc.)      │
│     ├─→ Extract with path traversal protection              │
│     └─→ Verify installed files                              │
│                                                              │
│  5. Update version info                                     │
│     └─→ Write version to registry or file                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Security Features

- **HTTP timeouts** — 10s connect, 30s read, 10s write (hardened agent)
- **URL validation** — All URLs validated before use
- **Redirect safety** — Max 10 redirects, validated redirect targets
- **Rate limit handling** — Parses `X-RateLimit-Remaining` headers
- **Checksum verification** — SHA256 verification of all downloads
- **Path traversal protection** — Prevents zip-slip attacks
- **Partial download cleanup** — Removes incomplete files on failure
- **Atomic file writes** — Write to .tmp, then rename

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

## Example: GitHub Releases

### Project Setup

```toml
# velocity.toml
[app]
name = "MyApp"
version = "1.0.0"
publisher = "My Company"

[fetch]
mode = "git-release"
platform = "github"
repo = "mycompany/myapp"
asset_pattern = "{app}-{version}-win-{arch}.zip"

[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" }
]

[fetch.update]
check_interval = "24h"
show_notification = true
```

### GitHub Release

Create a release on GitHub with these assets:
- `MyApp-1.0.0-win-x64.zip`
- `MyApp-1.0.0-win-x64.zip.sha256` (optional checksum file)

### Build

```bash
velocity build --mode fetch
# Output: output/MyApp_Setup.exe (1.5 MB)
```

### User Experience

1. User downloads `MyApp_Setup.exe` (1.5 MB)
2. Runs the installer
3. Installer checks GitHub for latest release
4. Downloads `MyApp-1.0.0-win-x64.zip` (50 MB)
5. Extracts and installs
6. Future runs check for updates every 24h

---

## Example: Self-Hosted HTTP

### Project Setup

```toml
# velocity.toml
[app]
name = "MyApp"
version = "1.0.0"

[fetch]
mode = "url"
base_url = "https://releases.mycompany.com/myapp"
version_url = "https://releases.mycompany.com/myapp/version.txt"
asset_pattern = "{app}-{version}-win-{arch}.zip"
checksum_url = "https://releases.mycompany.com/myapp/{version}/SHA256SUMS"

[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" }
]
```

### Server Setup

Host these files on your server:
```
/releases/myapp/
├── version.txt                    # Contains: 1.0.0
├── 1.0.0/
│   ├── MyApp-1.0.0-win-x64.zip
│   └── SHA256SUMS                 # Contains: <sha256>  MyApp-1.0.0-win-x64.zip
└── 1.0.1/
    ├── MyApp-1.0.1-win-x64.zip
    └── SHA256SUMS
```

### Build

```bash
velocity build --mode fetch
# Output: output/MyApp_Setup.exe
```

---

## Troubleshooting

### "No releases found"

- Verify the repository name is correct (`owner/repo`)
- Check that at least one release exists on the platform
- For private repos, ensure `auth_token` is set

### "Asset pattern mismatch"

- Check that your release assets match the `asset_pattern`
- Use `{app}`, `{version}`, `{arch}` placeholders correctly
- Example: if pattern is `{app}-{version}-win-{arch}.zip`, asset should be `MyApp-1.0.0-win-x64.zip`

### "Checksum mismatch"

- Verify the checksum file is correct
- For GitHub, ensure the `.sha256` file contains the hash followed by the filename
- Re-download the file manually and verify with `sha256sum`

### "Rate limit exceeded"

- Add an `auth_token` to increase rate limits
- The installer will fall back to cached version info if available
- Wait for rate limit reset (usually 1 hour)

---

## Further Reading

- [[Configuration-Reference]] — Complete `velocity.toml` reference
- [[Security]] — Encryption and security features
- [[Delta-Updates]] — Binary delta patching for efficient updates
