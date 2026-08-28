# Architecture

Velocity Installer is organized as a **7-crate Rust workspace** with clean separation of concerns. This page explains the system design, data flow, and how the pieces fit together.

## Crate Overview

```
velocity/
├── velocity-cli          # CLI entry point: init, build, detect, check, sign, dep
├── velocity-core         # Core engine: extraction, registry, shortcuts, services,
│                         #   env vars, rollback, logging, disk space, file associations,
│                         #   process detection, elevation, payload, downloader,
│                         #   dependency resolver, localization, security, encryption,
│                         #   delta updates, component tree, scripting, cloud-fetch
├── velocity-config       # Config parser, validator, auto-gen, path variables
├── velocity-ui           # Wizard UI: modern (WebView2) + classic (Win32)
├── velocity-compiler     # Compiles config + payload into standalone .exe + MSI builder
├── velocity-runtime      # Lightweight runtime embedded in each installer
├── velocity-plugin-api   # Plugin trait + SDK for WASM-based custom actions
└── velocity-msi          # MSI package generation and compliance
```

## Crate Responsibilities

### velocity-cli
The command-line interface that users interact with. Parses CLI arguments and dispatches to the appropriate crate.

**Key commands:**
- `velocity init` — Scaffold a new installer project
- `velocity build` — Build the installer
- `velocity detect` — Auto-detect project settings
- `velocity check` — Deep validation
- `velocity sign` — Code signing
- `velocity dep` — Dependency management

### velocity-core
The heart of the system. Contains all platform-specific logic and the bulk of the feature set.

**Major modules:**
| Module | Responsibility |
|--------|---------------|
| `extract` | Archive extraction (zstd, LZMA2, ZIP, tar, tar.gz, tar.xz, tar.bz2) |
| `registry` | Windows Registry operations (HKLM, HKCU, HKCR, HKU) |
| `shortcuts` | Desktop/Start Menu shortcuts via IShellLink COM |
| `services` | Windows service install/start/stop/remove |
| `env_vars` | Environment variable management (system + user scope) |
| `rollback` | Automatic rollback tracking and execution |
| `logging` | Install logging with timestamps |
| `disk_space` | Free disk space checking (Windows + Unix) |
| `file_associations` | File type registration |
| `elevation` | UAC elevation and admin detection |
| `payload` | Installer payload format reading |
| `downloader` | HTTP download with resume (WinHTTP + ureq backends) |
| `dep_installer` | Dependency condition resolution and installation |
| `localization` | i18n string resolution |
| `security` | Path validation, overwrite handling, integrity checks |
| `encryption` | AES-256-GCM payload encryption |
| `delta` | Binary delta patching (bsdiff) |
| `scripting` | Structured scripting engine with conditions |
| `fetch` | Cloud-fetch clients (GitHub, GitLab, Bitbucket, Gitea, URL) |
| `updater` | Self-update version checking |
| `component_tree` | Component selection tree view |
| `arch_detect` | Architecture detection (x86, x64, ARM, ARM64) |

### velocity-config
Parses and validates `velocity.toml` manifests. Handles path variable expansion and auto-generation.

**Key types:**
- `VelocityManifest` — Root config structure
- `FetchConfig` — Cloud-fetch configuration
- `InstallConfig` — Installation behavior settings
- `FileConfig` — File inclusion/exclusion rules

### velocity-ui
Provides two wizard UI implementations:

1. **Modern UI** — WebView2-based with dark/light themes, CSS animations, JS↔Rust bidirectional RPC
2. **Classic UI** — Native Win32 wizard with standard controls

Both UIs support:
- Multi-page wizard (Welcome, License, Directory, Components, Progress, Finish)
- Real-time progress tracking with ETA
- Silent mode (`/S` flag)
- Component selection

### velocity-compiler
Takes the manifest and payload files and produces a standalone `.exe` installer. Also handles MSI package generation.

**Build pipeline:**
1. Parse `velocity.toml`
2. Collect and validate source files
3. Compress payload (zstd/LZMA2)
4. Optionally encrypt payload (AES-256-GCM)
5. Generate delta packages (if `--delta`)
6. Embed payload into runtime stub
7. Set PE icon and version info
8. Output standalone `.exe`

### velocity-runtime
A lightweight runtime that gets embedded into each installer. Handles the actual installation process when the user runs the `.exe`.

**Responsibilities:**
- Parse command-line arguments (`/S`, `/D=path`, `--silent`)
- Extract payload to temp directory
- Execute installation (files, registry, shortcuts, services, etc.)
- Handle rollback on failure
- Display progress to UI
- Run post-install scripts
- Launch application (optional)

### velocity-plugin-api
Defines the plugin trait and provides a WASM loader for sandboxed plugins.

**Plugin lifecycle hooks:**
1. `on_load` — Plugin initialization
2. `on_pre_install` — Before installation starts
3. `on_file_extracted` — After each file extraction
4. `on_post_install` — After installation completes
5. `on_error` — On installation error
6. `on_cancel` — On user cancellation
7. `on_uninstall` — During uninstallation
8. `on_upgrade` — During version upgrade
9. `on_rollback` — During rollback

### velocity-msi
Generates Windows Installer (MSI) packages for enterprise deployment. Maps `velocity.toml` configuration to MSI database tables.

**Supported MSI tables:**
Property, Directory, Component, File, Media, Feature, Registry, Shortcut, Environment, ServiceInstall, ServiceControl, CustomAction, InstallExecuteSequence, Upgrade, LaunchCondition, Cabinet

## Data Flow

### Build Time

```
┌─────────────────────────────────────────────────────────────┐
│                     velocity build                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  velocity.toml ──→ velocity-config ──→ VelocityManifest     │
│                                              │               │
│                                              ▼               │
│  files/** ──→ velocity-compiler ──→ Compress (zstd)         │
│                    │                    │                    │
│                    │                    ▼                    │
│                    │              Encrypt (optional)         │
│                    │                    │                    │
│                    ▼                    ▼                    │
│              Generate MSI         Embed in runtime           │
│                    │                    │                    │
│                    ▼                    ▼                    │
│              output/app.msi      output/app.exe             │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Install Time

```
┌─────────────────────────────────────────────────────────────┐
│                   app.exe (user runs)                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. velocity-runtime starts                                  │
│  2. Parse CLI args (/S, /D, --silent)                        │
│  3. Show UI (modern or classic wizard)                       │
│  4. Extract embedded payload to temp dir                     │
│  5. If encrypted: decrypt with password                      │
│  6. Decompress payload (zstd/LZMA2)                          │
│  7. Execute installation:                                    │
│     - Pre-install scripts                                    │
│     - Copy files                                             │
│     - Registry entries                                       │
│     - Shortcuts                                              │
│     - Services                                               │
│     - Environment variables                                  │
│     - Post-install scripts                                   │
│  8. Track all changes for rollback                           │
│  9. On failure: automatic rollback                           │
│ 10. Launch app (optional)                                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Cloud-Fetch Install Time

```
┌─────────────────────────────────────────────────────────────┐
│              app.exe (cloud-fetch bootstrapper)              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. velocity-runtime starts                                  │
│  2. Read bundled manifest                                    │
│  3. Check installed version (registry/file)                  │
│  4. Query Git platform API for latest release                │
│  5. Compare versions (semver)                                │
│  6. If newer: download assets with progress UI               │
│  7. Verify SHA256 checksums                                  │
│  8. Extract/install files                                    │
│  9. Update version info                                      │
│ 10. Launch app (optional)                                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Platform-Specific Code

Velocity uses Rust's conditional compilation to handle platform differences:

```rust
// Windows-specific code
#[cfg(target_os = "windows")]
fn execute_installer() { /* ShellExecuteW, registry, COM */ }

// Unix-specific code
#[cfg(not(target_os = "windows"))]
fn execute_installer() { /* sudo, dpkg, rpm, sh */ }
```

**Platform coverage:**

| Feature | Windows | Linux | macOS |
|---------|---------|-------|-------|
| Installer execution | Full (NSIS, InnoSetup, MSI) | dpkg, rpm, sh | pkg, dmg |
| Archive extraction | All formats | All formats | All formats |
| Download | WinHTTP | ureq | ureq |
| Elevation | ShellExecuteW + UAC | sudo | sudo |
| Registry | Full support | N/A | N/A |
| Services | Full SCM | systemd (planned) | launchd (planned) |
| UI | Modern + Classic | CLI only | CLI only |

## Dependency Graph

```
velocity-cli
    ├── velocity-core
    ├── velocity-config
    ├── velocity-compiler
    └── velocity-ui

velocity-compiler
    ├── velocity-core
    ├── velocity-config
    └── velocity-msi

velocity-runtime
    ├── velocity-core
    └── velocity-config

velocity-core
    ├── velocity-config
    └── (external crates)

velocity-plugin-api
    └── velocity-core
```

## External Dependencies

### Compression
- `zstd` — Primary compression (fast, high ratio)
- `lzma-rs` — LZMA2 support
- `bzip2` — tar.bz2 support
- `flate2` — gzip support
- `zip` — ZIP archive support
- `bsdiff` — Binary delta patching

### Cryptography
- `aes-gcm` — AES-256-GCM authenticated encryption
- `pbkdf2` — Password-based key derivation
- `sha2` — SHA-256 hashing
- `hmac` — HMAC construction
- `getrandom` — CSPRNG (cross-platform)

### Windows APIs
- `windows` — Win32 API bindings
- `winreg` — Registry access
- `webview2-com` — Modern UI (WebView2)

### Cross-Platform
- `ureq` — HTTP client (Unix backend)
- `semver` — Semantic version parsing
- `dirs` — Platform directory paths
- `wasmtime` — WASM plugin runtime

### Error Handling
- `thiserror` — Typed errors in core crate
- `anyhow` — Flexible errors in runtime/CLI

## Build Configuration

The workspace uses optimized release builds:

```toml
[profile.release]
opt-level = "z"    # Optimize for size
lto = true         # Link-time optimization
codegen-units = 1  # Single codegen unit for best optimization
strip = true       # Strip debug symbols
```

This produces small, fast binaries suitable for distribution.

## Testing Strategy

Velocity has **483 tests** across all crates:

| Crate | Tests | Coverage |
|-------|-------|----------|
| velocity-core | 297 | Extraction, registry, rollback, encryption, delta, cloud-fetch |
| velocity-config | 40 | Config parsing, validation, path variables |
| velocity-runtime | 44 | Runtime integration, input validation, fetch installer |
| velocity-msi | 50 | MSI table mapping, signing, validation |
| velocity-ui | 20 | Modern wizard, classic wizard |
| velocity-compiler | 9 | Build orchestration |
| velocity-plugin-api | 15 | Plugin loading, WASM execution |

Run the full test suite:
```bash
cargo test --workspace
```

## Further Reading

- [[Security]] — Encryption details and audit reports
- [[Contributing]] — How to contribute to Velocity
- [[Cloud-Fetch-Installers]] — Cloud-fetch architecture
- [[Delta-Updates]] — Delta update implementation
