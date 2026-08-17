# Velocity Installer

A free, open-source, universal Windows installer framework built in Rust.

Velocity produces standalone `.exe` installers from a simple TOML configuration, with a choice of modern or classic wizard UI. No commercial licensing required — fully free under MIT/Apache-2.0.

## Features

### Core
- **Zero-allocation engine** — Built in Rust for maximum performance and minimal binary size
- **zstd compression** — Fast, efficient payload compression (up to 90%+ reduction)
- **Universal** — Handle any installation scenario: files, registry, shortcuts, services, env vars, file associations
- **Auto-generated config** — Minimal manual setup; the CLI detects your project structure from Cargo.toml/package.json
- **Standalone .exe output** — Each installer is a single self-contained executable

### Installation
- **Multi-page wizard** — Welcome, License Agreement, Directory Selection, Progress, and Finish pages
- **Silent mode** — Full unattended installation with `/S`, `/D=path`, `--silent`, `--force` flags (Inno Setup compatible)
- **Rollback on failure** — All changes are tracked and automatically undone if installation fails
- **Disk space validation** — Checks available space before starting installation
- **App-running detection** — Warns if the application is currently running before overwriting files
- **Install logging** — Detailed log file written to the installation directory
- **UAC elevation** — Automatic admin elevation when required

### Windows Integration
- **Registry** — Full support for HKLM, HKCU, HKCR, HKU with REG_SZ, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ
- **Shortcuts** — Desktop, Start Menu, Quick Launch, and custom locations via IShellLink COM
- **Services** — Install, start, stop, and remove Windows services
- **Environment variables** — System and user scope with WM_SETTINGCHANGE broadcast
- **File associations** — Register file types with icons and open commands
- **Add/Remove Programs** — Automatic registration with uninstaller

### Build Pipeline
- **Icon embedding** — Custom installer icon via rcedit or Resource Hacker
- **Version info** — PE version metadata (company, description, version)
- **Code signing** — Built-in `velocity sign` command wrapping signtool.exe
- **Path variables** — `{app}`, `{autopf}`, `{win}`, `{sys}`, `{tmp}`, `{home}`, and more

### Architecture
- **Dual UI themes** — Modern (WebView2) or Classic (Win32) wizard, selectable per package
- **Plugin API** — WASM-ready plugin trait with lifecycle hooks (Phase 3)
- **7-crate workspace** — Clean separation of concerns

## Quick Start

```bash
# Install the CLI
cargo install velocity-cli

# Create a new installer project (auto-detects Cargo.toml/package.json)
velocity init my-app

# Auto-detect project settings
velocity detect

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

[install]
default_dir = "{autopf}/MyApp"
start_menu = "My Application"
require_admin = true
run_after_install = "myapp.exe"

[files]
source = ["./build-output/**"]
exclude = ["*.pdb", "*.tmp"]

[shortcuts]
desktop = true
start_menu = true

[[registry]]
key = "Software\\MyApp"
name = "InstallPath"
value = "{app}"
root = "HKLM"

[[env_vars]]
name = "MY_APP_HOME"
value = "{app}"
scope = "system"

[[services]]
name = "MyAppService"
display_name = "My Application Service"
binary_path = "myapp-service.exe"
start_type = "auto"

[[file_associations]]
extension = ".myext"
description = "My Application File"
handler = "myapp.exe"

[uninstall]
add_remove = true

[ui]
theme = "classic"  # or "modern"
```

## Silent Installation

```cmd
# Basic silent install
installer.exe /S

# Silent install to custom directory
installer.exe /S /D=C:\Custom\Path

# Force uninstall without confirmation
installer.exe --uninstall --force
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `velocity init [name]` | Scaffold a new installer project |
| `velocity build` | Build the installer .exe |
| `velocity detect` | Auto-detect project settings |
| `velocity check` | Validate velocity.toml |
| `velocity info <path>` | Show installer package info |
| `velocity sign <path>` | Code-sign the installer |
| `velocity version` | Show version info |

## Architecture

```
velocity/
├── crates/
│   ├── velocity-cli/          # CLI tool: init, build, detect, check, info, sign
│   ├── velocity-core/         # Engine: extract, registry, shortcuts, services, env vars,
│   │                          #        rollback, logging, disk space, file associations,
│   │                          #        process detection, PE icon, elevation, payload
│   ├── velocity-config/       # Config parser, validator, auto-generator, path variables
│   ├── velocity-ui/           # Installer wizard UI (modern + classic)
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

Currently 26 tests across all crates covering config parsing, variable resolution, archive operations, payload format, rollback tracking, disk space, file associations, and process detection.

## Roadmap

- [x] **Phase 1: Foundation** — Core engine, classic UI, TOML config, compiler, runtime
- [ ] **Phase 2: Modern UI** — WebView2-based wizard with dark/light themes, animations
- [ ] **Phase 3: Advanced** — Auto-update, WASM plugins, delta compression
- [ ] **Phase 4: Ecosystem** — GUI config editor, template marketplace, CI/CD integration

## License

Licensed under either of:
- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.
