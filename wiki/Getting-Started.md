# Getting Started

This guide walks you through installing Velocity, creating your first installer, and building it.

## Prerequisites

### Required
- **Rust 1.75+** — [Install Rust](https://rustup.rs/) via rustup
- **Git** — For cloning the repository

### Optional (for full features)
- **Windows SDK** — For code signing (`signtool.exe`)
- **WebView2 Runtime** — For modern wizard UI (included in Windows 11)
- **Visual Studio Build Tools** — For compiling from source

## Installation

### From Source (Recommended)

```bash
git clone https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer.git
cd V.E.L.O.C.I.T.Y.-Installer
cargo build --release
```

The binary will be at `target/release/velocity.exe`.

### From Cargo

```bash
cargo install velocity-cli
```

### Verify Installation

```bash
velocity version
```

## Quick Start: Your First Installer

### Step 1: Initialize a Project

```bash
velocity init my-app
cd my-app
```

This creates a basic `velocity.toml` and project structure:

```
my-app/
├── velocity.toml      # Installer configuration
├── files/             # Your application files
│   └── (your files)
└── output/            # Build output directory
```

### Step 2: Auto-Detect Settings

If you have an existing project with `Cargo.toml` or `package.json`:

```bash
velocity detect
```

This automatically detects:
- App name and version
- Publisher
- Icon location
- Build output directory

### Step 3: Configure Your Installer

Edit `velocity.toml`:

```toml
[app]
name = "My Application"
version = "1.0.0"
publisher = "My Company"
icon = "assets/icon.ico"
license = "LICENSE.txt"

[install]
default_dir = "{autopf}/MyApp"
start_menu = true
require_admin = true

[files]
source = ["./build-output/**"]
exclude = ["*.pdb", "*.tmp"]

[shortcuts]
desktop = true
start_menu = true
```

### Step 4: Validate Configuration

```bash
velocity check
```

This performs deep validation:
- All file paths exist
- Registry roots are valid
- Service configurations are correct
- Path variables resolve properly

### Step 5: Build the Installer

```bash
velocity build
```

Output:
```
✓ Collected 42 files (15.3 MB)
✓ Compressed payload (5.2 MB, 66% reduction)
✓ Generated installer: output/MyApp_Setup.exe (5.8 MB)
```

### Step 6: Test Your Installer

```bash
# Interactive install
output/MyApp_Setup.exe

# Silent install
output/MyApp_Setup.exe /S

# Silent install to custom directory
output/MyApp_Setup.exe /S /D=C:\Custom\Path
```

## Build Modes

Velocity supports three installer types:

### Bundled (Default)
All files packaged inside the installer. Best for offline installation.

```bash
velocity build --mode bundled
```

### Cloud-Fetch
Tiny bootstrapper that downloads files at install time. Best for large apps with frequent updates.

```bash
velocity build --mode fetch
```

See [[Cloud-Fetch-Installers]] for details.

### Hybrid
Bundle critical files, fetch optional ones. Best of both worlds.

```bash
velocity build --mode hybrid
```

## Output Formats

### EXE Installer (Default)
Standalone `.exe` with custom UI.

```bash
velocity build
# Output: output/MyApp_Setup.exe
```

### MSI Package
For enterprise deployment via Group Policy, SCCM, or Intune.

```bash
velocity build --package-format msi
# Output: output/MyApp_Setup.msi
```

See [[MSI-Enterprise]] for details.

## Common Workflows

### Adding Dependencies

Dependencies are auto-downloaded and installed before your app:

```toml
[[dependencies]]
name = "VC++ 2022 Redistributable"
url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
sha256 = "optional-sha256-hash"
install_args = "/install /quiet /norestart"
condition = "not_installed:Microsoft Visual C++ 2022"
required = true
priority = 10
```

### Adding Components (Optional Features)

Let users choose what to install:

```toml
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
```

### Adding Registry Entries

```toml
[[registry]]
key = "Software\\MyApp"
name = "InstallPath"
value = "{app}"
root = "HKLM"

[[registry]]
key = "Software\\MyApp"
name = "Version"
value = "1.0.0"
root = "HKLM"
```

### Adding Environment Variables

```toml
[[env_vars]]
name = "MYAPP_HOME"
value = "{app}"
scope = "system"

[[env_vars]]
name = "PATH"
value = "{app}\\bin"
scope = "system"
append = true
```

### Adding Services

```toml
[[services]]
name = "MyAppService"
display_name = "My Application Service"
binary_path = "myapp-service.exe"
start_type = "auto"
```

### Localization

```toml
[localization]
default_language = "en"

[[localization.languages]]
code = "de"
name = "Deutsch"
[localization.languages.strings]
btn_next = "Weiter (&N) >"
btn_back = "< Zurück (&B)"
btn_install = "Installieren (&I)"
```

## Code Signing

Sign your installer to eliminate SmartScreen warnings:

```bash
# Sign with certificate file
velocity sign output/MyApp_Setup.exe \
    --cert my_cert.pfx \
    --timestamp http://timestamp.digicert.com

# Sign with fingerprint (hardware token)
velocity sign output/MyApp_Setup.exe \
    --fingerprint "AB:CD:EF:01:23:45:67:89:..."

# Verify signature
velocity sign --verify output/MyApp_Setup.exe
```

See [[Code-Signing]] for CI/CD integration.

## Delta Updates

Build with delta update support for efficient version upgrades:

```bash
velocity build --delta
```

This generates:
- `output/MyApp_Setup.exe` — Full installer
- `output/MyApp_Setup-delta.zip` — Delta package (80-95% smaller)

See [[Delta-Updates]] for details.

## Silent Installation

Velocity installers support multiple silent install modes:

```cmd
# Basic silent install
installer.exe /S

# Silent install to custom directory
installer.exe /S /D=C:\Custom\Path

# Force uninstall
installer.exe --uninstall --force

# Quiet mode (MSI-style)
installer.exe /quiet
```

## Path Variables

Use these variables in your configuration:

| Variable | Description | Example |
|----------|-------------|---------|
| `{app}` | Installation directory | `C:\Program Files\MyApp` |
| `{autopf}` | Program Files (arch-aware) | `C:\Program Files` or `C:\Program Files (x86)` |
| `{win}` | Windows directory | `C:\Windows` |
| `{sys}` | System32 directory | `C:\Windows\System32` |
| `{tmp}` | Temp directory | `C:\Users\...\AppData\Local\Temp` |
| `{home}` | User home directory | `C:\Users\username` |
| `{desktop}` | Desktop folder | `C:\Users\username\Desktop` |
| `{programs}` | Start Menu Programs | `C:\Users\username\AppData\Roaming\Microsoft\Windows\Start Menu\Programs` |

## Troubleshooting

### "velocity: command not found"

If installed via `cargo install`, ensure `~/.cargo/bin` is in your PATH:

```bash
# Windows PowerShell
$env:PATH += ";$env:USERPROFILE\.cargo\bin"

# Linux/macOS
export PATH="$HOME/.cargo/bin:$PATH"
```

### Build Fails with "signtool.exe not found"

Install the Windows SDK or add `signtool.exe` to your PATH. Velocity searches:
- `C:\Program Files (x86)\Windows Kits\10\bin\*\x64\`
- `C:\Program Files\Windows Kits\10\bin\*\x64\`

### SmartScreen Warning on Unsigned Installer

This is normal for unsigned installers. Sign your installer or instruct users to click "More info" → "Run anyway".

### Installer Won't Start

Check if another instance is already running. Velocity uses a named mutex to prevent concurrent installations.

## Next Steps

- [[Configuration-Reference]] — Complete `velocity.toml` reference
- [[CLI-Reference]] — All CLI commands and options
- [[Cloud-Fetch-Installers]] — Cloud-fetch bootstrapper installers
- [[MSI-Enterprise]] — Enterprise deployment with MSI packages
- [[Security]] — Encryption and security features
