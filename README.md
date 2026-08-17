# Velocity Installer

A free, open-source, universal Windows installer framework built in Rust.

Velocity produces standalone `.exe` installers from a simple TOML configuration, with a choice of modern or classic wizard UI. No commercial licensing required — fully free under MIT/Apache-2.0.

## Features

### Core Engine
- **High-performance Rust engine** — Built for maximum performance and minimal binary size
- **zstd compression** — Fast, efficient payload compression (up to 90%+ reduction)
- **Universal** — Handle any installation scenario: files, registry, shortcuts, services, env vars, file associations
- **Auto-generated config** — Minimal manual setup; the CLI detects your project structure from Cargo.toml/package.json
- **Standalone .exe output** — Each installer is a single self-contained executable
- **Rollback on failure** — All changes are tracked and automatically undone if installation fails

### Installation Experience
- **Multi-page wizard** — Welcome, License Agreement, Directory Selection, Component Selection, Progress with ETA, and Finish pages
- **Silent mode** — Full unattended installation with `/S`, `/D=path`, `--silent`, `--force` flags (Inno Setup compatible)
- **Component selection** — Users can choose which features to install (mandatory + optional components)
- **Progress tracking with ETA** — Real-time progress bar with estimated time remaining
- **Disk space validation** — Checks available space before starting installation
- **App-running detection** — Warns if the application is currently running before overwriting files
- **File overwrite handling** — Configurable behavior: always overwrite, skip, prompt, or newer-only
- **Install logging** — Human-readable timestamped log file written to the installation directory
- **UAC elevation** — Automatic admin elevation when required
- **Close app before install** — Optionally terminate running instances before installation

### Ninite-like Dependency Management
- **Remote dependencies** — Auto-download and silently install prerequisites (VC++ Redist, DirectX, .NET, etc.)
- **Condition-based installation** — Only install dependencies when needed (registry checks, file checks, arch detection, OS version, Add/Remove Programs lookup)
- **SHA256 verification** — Integrity checking of all downloaded files
- **Download resume** — Resumable downloads with HTTP Range requests
- **Bundled third-party apps** — Include installers like Notepad++, VLC, 7-Zip in your payload
- **Priority ordering** — Control the installation order of dependencies

### Localization (i18n)
- **Built-in English strings** — Complete default string table
- **Multi-language support** — Define additional languages in velocity.toml
- **Per-language overrides** — Override any UI string for any language
- **Variable substitution** — Dynamic values in localized strings (`{app_name}`, `{version}`)
- **Language auto-detection** — Uses system locale by default

### Security Hardening
- **AES-256-GCM encryption** — Authenticated encryption for installer payloads with password protection
- **PBKDF2-HMAC-SHA256 key derivation** — 600,000 iterations (OWASP 2023 recommendation) with cryptographically secure random salt (BCryptGenRandom)
- **CSPRNG salt and nonce** — All cryptographic randomness uses Windows BCryptGenRandom (kernel-level CSPRNG)
- **Secure temp directories** — Unique per-session directories with restricted access
- **Path traversal protection** — Prevents zip-slip attacks in archives
- **Install directory validation** — Rejects system directories (Windows, System32, ProgramData), drive roots, and paths with null bytes
- **Password length limit** — 1024-char max prevents PBKDF2 denial-of-service
- **Shell injection protection** — URL validation before passing to `cmd /C start`
- **Null byte rejection** — Blocks null byte injection in paths
- **Absolute path rejection** — Archive entries must use relative paths only
- **File backup before overwrite** — Automatic `.velocity_backup` files before replacing
- **File integrity verification** — SHA256 hash checking for downloaded files
- **Crash reporting** — Panic hook writes backtrace to `%TEMP%/velocity_crashes/` for diagnostics

### Windows Integration
- **Registry** — Full support for HKLM, HKCU, HKCR, HKU with REG_SZ, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ
- **Shortcuts** — Desktop, Start Menu, Quick Launch, and custom locations via IShellLink COM
- **Services** — Install, start, stop, and remove Windows services with dependency support
- **Environment variables** — System and user scope with WM_SETTINGCHANGE broadcast
- **File associations** — Register file types with icons and open commands
- **Add/Remove Programs** — Automatic registration with estimated size, install date, help/update URLs
- **Pre/post-install scripts** — Execute custom commands at any stage of installation

### Build Pipeline
- **Icon embedding** — Custom installer icon via rcedit or Resource Hacker
- **Version info** — PE version metadata (company, description, version)
- **Code signing** — Built-in `velocity sign` command wrapping signtool.exe
- **Path variables** — `{app}`, `{autopf}`, `{win}`, `{sys}`, `{tmp}`, `{home}`, and more
- **Deep validation** — `velocity check` validates all paths, registry roots, service configs, and more

### Architecture
- **Dual UI themes** — Modern (WebView2) or Classic (Win32) wizard, selectable per package
- **Plugin API** — WASM-ready plugin trait with lifecycle hooks
- **7-crate workspace** — Clean separation of concerns

### Self-Update & Scripting
- **HTTP self-update** — Checks a JSON endpoint for new versions at startup, notifies the user, and opens the download URL
- **Structured scripting engine** — Variable substitution (`{install_dir}`, `{app_name}`), condition evaluation (`file_exists`, `dir_exists`, `action_success`), 7 action types (shell, copy, delete, mkdir, registry, env var), and configurable error policies (Abort, Continue, Retry)
- **Component tree view** — Flattened hierarchy with indented display, disk space calculation, and dependency resolution

## Quick Start

```bash
# Install the CLI
cargo install velocity-cli

# Create a new installer project (auto-detects Cargo.toml/package.json)
velocity init my-app

# Auto-detect project settings
velocity detect

# Validate configuration
velocity check

# Build the installer
cd my-app
velocity build

# Sign the installer
velocity sign output/installer.exe --fingerprint YOUR_THUMBPRINT --timestamp http://timestamp.digicert.com
```

## Configuration

Create a `velocity.toml` in your project root:

```toml
[app]
name = "My Application"
version = "1.0.0"
publisher = "My Company"
icon = "assets/icon.ico"
license = "LICENSE.txt"

[install]
default_dir = "{autopf}/MyApp"
start_menu = "My Application"
require_admin = true
run_after_install = "myapp.exe"
close_app_before_install = "myapp.exe"

[files]
source = ["./build-output/**"]
exclude = ["*.pdb", "*.tmp"]

[shortcuts]
desktop = true
start_menu = true

# Components (user-selectable features)
[[components]]
id = "core"
name = "Core Application"
description = "Required application files"
selected_by_default = true
mandatory = true
source = ["./build/bin/**"]

[[components]]
id = "docs"
name = "Documentation"
description = "User manuals and API reference"
selected_by_default = true
source = ["./docs/**"]

[[components]]
id = "sdk"
name = "Developer SDK"
description = "Headers, libraries, and samples"
selected_by_default = false
source = ["./sdk/**"]

# Remote dependencies (auto-downloaded)
[[dependencies]]
name = "VC++ 2022 Redistributable"
url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
sha256 = "optional-sha256-hash"
install_args = "/install /quiet /norestart"
condition = "not_installed:Microsoft Visual C++ 2022"
required = true
priority = 10

# Bundled third-party apps
[[bundled_apps]]
name = "Notepad++"
installer = "third-party/npp-installer.exe"
install_args = "/S"
condition = "not_installed:Notepad++"
required = false
priority = 100

# Localization
[localization]
default_language = "en"

[[localization.languages]]
code = "de"
name = "Deutsch"
[localization.languages.strings]
btn_next = "Weiter (&N) >"
btn_back = "< Zurück (&B)"
btn_install = "Installieren (&I)"
btn_finish = "Fertigstellen (&F)"

# Registry
[[registry]]
key = "Software\\MyApp"
name = "InstallPath"
value = "{app}"
root = "HKLM"

# Environment variables
[[env_vars]]
name = "MY_APP_HOME"
value = "{app}"
scope = "system"

# Services
[[services]]
name = "MyAppService"
display_name = "My Application Service"
binary_path = "myapp-service.exe"
start_type = "auto"

# File associations
[[file_associations]]
extension = ".myext"
description = "My Application File"
handler = "myapp.exe"

# Scripts
[scripts]
pre_install = ["echo Installing..."]
post_install = ["echo Done!"]

# Structured script actions (optional, supports copy, delete, mkdir, registry, env_var)
[[scripts.post_install_actions]]
name = "Create config directory"
action = "mkdir"
path = "{install_dir}\\config"
on_error = "continue"

[[scripts.post_install_actions]]
name = "Copy default config"
action = "copy"
src = "{install_dir}\\defaults\\config.ini"
dest = "{install_dir}\\config\\config.ini"
condition = "file_missing:{install_dir}\\config\\config.ini"

[uninstall]
add_remove = true
help_url = "https://support.myapp.com"
update_url = "https://updates.myapp.com"

[ui]
theme = "classic"  # or "modern"
```

## Dependency Conditions

The condition system determines when a dependency needs installation:

| Condition | Description |
|-----------|-------------|
| `always` | Always install |
| `never` | Never install |
| `registry_missing:HKLM\Software\Foo` | Install if registry key doesn't exist |
| `registry_exists:HKLM\Software\Foo` | Install if registry key exists |
| `file_missing:C:\path\to\file` | Install if file doesn't exist |
| `file_exists:C:\path\to\file` | Install if file exists |
| `not_installed:Product Name` | Install if not in Add/Remove Programs |
| `installed:Product Name` | Install if in Add/Remove Programs |
| `arch:x64` | Install only on x64 |
| `arch:x86` | Install only on x86 |
| `os_version:>=10.0` | Install on Windows 10+ |

## Silent Installation

```cmd
# Basic silent install
installer.exe /S

# Silent install to custom directory
installer.exe /S /D=C:\Custom\Path

# Force uninstall without confirmation
installer.exe --uninstall --force

# Quiet mode (MSI-style)
installer.exe /quiet
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `velocity init [name]` | Scaffold a new installer project |
| `velocity build` | Build the installer .exe |
| `velocity detect` | Auto-detect project settings |
| `velocity check` | Deep validation of velocity.toml |
| `velocity info <path>` | Show installer package info |
| `velocity sign <path>` | Code-sign the installer |
| `velocity dep list` | List configured dependencies |
| `velocity dep add` | Add a new dependency |
| `velocity dep resolve` | Check which dependencies need installation |
| `velocity dep remove` | Remove a dependency |
| `velocity version` | Show version info |

## Path Variables

| Variable | Description |
|----------|-------------|
| `{app}` | Installation directory |
| `{autopf}` | `C:\Program Files` or `C:\Program Files (x86)` based on arch |
| `{win}` | Windows directory (e.g., `C:\Windows`) |
| `{sys}` | System32 directory |
| `{tmp}` | Temporary directory |
| `{home}` | User's home directory |
| `{desktop}` | Desktop folder |
| `{programs}` | Start Menu Programs folder |
| `{sendto}` | SendTo folder |
| `{startup}` | Startup folder |

## Architecture

```
velocity/
├── crates/
│   ├── velocity-cli/          # CLI: init, build, detect, check, info, sign, dep
│   ├── velocity-core/         # Engine: extract, registry, shortcuts, services, env vars,
│   │                          #        rollback, logging, disk space, file associations,
│   │                          #        process detection, PE icon, elevation, payload,
│   │                          #        downloader, dep resolver, dep installer,
│   │                          #        localization, security, encryption (AES-256-GCM),
│   │                          #        updater, component tree, scripting engine
│   ├── velocity-config/       # Config parser, validator, auto-gen, path variables
│   ├── velocity-ui/           # Wizard UI with progress tracking + ETA
│   ├── velocity-compiler/     # Compiles config+payload into standalone .exe
│   ├── velocity-runtime/      # Lightweight runtime embedded in each installer
│   └── velocity-plugin-api/   # Plugin trait + SDK for custom actions
├── themes/
│   ├── modern/                # WebView2-based modern UI (Phase 2)
│   └── classic/               # Native Win32 wizard UI
└── templates/
    └── default/               # Scaffold template for `velocity init`
```

## Building from Source

```bash
git clone https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer.git
cd V.E.L.O.C.I.T.Y.-Installer
cargo build --release
```

## Testing

```bash
cargo test
```

187 tests across all crates covering:
- Config parsing and validation (14 tests)
- Archive creation and extraction (3 tests)
- Payload format (1 test)
- Rollback tracking (3 tests)
- Disk space operations (2 tests)
- Logging and date formatting (7 tests)
- Downloader URL parsing and SHA256 (5 tests)
- Dependency condition resolution (9 tests)
- Dependency installer command building (5 tests)
- Security: path traversal, overwrite handling (8 tests)
- Localization string resolution (10 tests)
- Progress tracking and ETA (5 tests)
- File association parsing (1 test)
- Process detection (2 tests)
- Uninstaller generation (2 tests)
- Compiler integration (3 tests)
- AES-256-GCM encryption + CSPRNG (12 tests)
- Self-update version parsing (5 tests)
- Component tree view (7 tests)
- Scripting engine (13 tests)
- End-to-end integration (2 tests)
- Stress testing: 1000 files, 50MB, Unicode (3 tests)
- Rollback correctness including stress (8 tests)
- Fuzz-like parser robustness (12 tests)
- Runtime input validation (9 tests)

## Comparison

| Feature | Velocity | Inno Setup | NSIS | WiX |
|---------|----------|------------|------|-----|
| Open Source | MIT/Apache | Inno Setup License | zlib | MS-RL |
| Config Format | TOML | Pascal Script | NSIS Script | XML |
| Compression | zstd | LZMA | LZMA | MSI/CAB |
| Dependency Management | Built-in | Manual | Manual | Manual |
| Localization | Built-in i18n | Language files | Language strings | Transform |
| Silent Mode | Inno-compatible | Yes | Yes | Yes |
| Rollback | Automatic | Yes | Yes | Yes (MSI) |
| Component Selection | Yes | Yes | Yes | Yes |
| Plugin System | WASM-ready | Pascal | NSIS Script | No |
| Written In | Rust | Delphi | C++ | C# |

## Roadmap

- [x] **Phase 1: Foundation** — Core engine, classic UI, TOML config, compiler, runtime
- [x] **Phase 1.5: Robustness** — Dependency management, localization, security hardening, component selection, progress tracking
- [x] **Phase 6: Hardening** — AES-256-GCM encryption, self-update mechanism, component tree view, structured scripting engine, end-to-end integration tests
- [x] **Phase 7: Quality** — Clippy cleanup, E2E integration tests, structured scripting, README updates
- [x] **Phase 8: Production Hardening** — Stress testing, rollback testing, PBKDF2 key derivation, unsafe safety audit, GitHub Actions CI/CD, crash reporting, code signing docs, fuzz-like parser robustness
- [x] **Phase 9: Final Fixes** — CSPRNG for encryption (BCryptGenRandom), runtime input validation (install dir, password limits, shell injection protection)
- [ ] **Phase 2: Modern UI** — WebView2-based wizard with dark/light themes, animations
- [ ] **Phase 3: Advanced** — WASM plugins, delta compression, full auto-update with download-and-swap
- [ ] **Phase 4: Ecosystem** — GUI config editor, template marketplace, CI/CD integration

## License

Licensed under either of:
- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.
