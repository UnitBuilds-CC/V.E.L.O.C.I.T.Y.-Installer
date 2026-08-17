# MSI Compliance

Velocity Installer can generate **Windows Installer (MSI)** packages for enterprise deployment via Group Policy, SCCM, or `msiexec`.

## Overview

MSI packages are an alternative output format to the standard `.exe` installer. They provide:

- **Group Policy deployment** — Deploy via Active Directory GPO
- **SCCM/Intune distribution** — Push to managed endpoints
- **Silent installation** — `msiexec /qn /i package.msi`
- **ARP integration** — Standard Add/Remove Programs entry
- **Major upgrade support** — Clean version transitions via UpgradeCode

## Building MSI Packages

### CLI Usage

```bash
# Build MSI package
velocity build --package-format msi

# Specify output path
velocity build --package-format msi --output releases/MyApp.msi

# With compression settings
velocity build --package-format msi --compression 15
```

### Programmatic Usage

```rust
use velocity_compiler::{build_msi, MsiOptions};
use velocity_config::parse_manifest;

let manifest = parse_manifest("velocity.toml")?;

let options = MsiOptions {
    output_path: "output/MyApp.msi".into(),
    project_dir: std::env::current_dir()?,
    architecture: "x64".to_string(),
    language: 1033, // English US
    per_machine: true,
    upgrade_code: None, // Auto-generated
};

let result = build_msi(&manifest, &options)?;
println!("MSI: {} ({} bytes)", result.msi_path.display(), result.msi_size);
println!("ProductCode: {}", result.product_code);
println!("UpgradeCode: {}", result.upgrade_code);
```

## Config → MSI Table Mapping

Velocity maps `velocity.toml` configuration directly to MSI database tables:

| Velocity Config | MSI Table(s) | Description |
|----------------|--------------|-------------|
| `[app]` | Property | ProductName, Manufacturer, ProductVersion |
| `[files]` | File, Component, Directory, Media | Application files |
| `[registry]` | Registry | Registry entries |
| `[shortcuts]` | Shortcut, Icon | Desktop/Start Menu shortcuts |
| `[scripts]` | CustomAction, InstallExecuteSequence | Pre/post install commands |
| `[env_vars]` | Environment | Environment variables |
| `[services]` | ServiceInstall, ServiceControl | Windows services |
| `[components]` | Feature, FeatureComponents | Optional features |
| `[file_associations]` | Class, ProgId, Extension | File type associations |

### Property Table

Standard MSI properties generated from `[app]`:

| Property | Source | Example |
|----------|--------|---------|
| ProductCode | Auto-generated GUID | `{550e8400-e29b-41d4-a716-446655440000}` |
| UpgradeCode | From `app.id` or auto-generated | `{550e8400-e29b-41d4-a716-446655440001}` |
| ProductName | `app.name` | `My Application` |
| Manufacturer | `app.publisher` | `My Company` |
| ProductVersion | `app.version` | `1.2.3` |
| ProductLanguage | Default 1033 (English US) | `1033` |
| ALLUSERS | From `install.require_admin` | `1` (per-machine) |
| ARPPRODUCTICON | From `app.icon` | `AppIcon.ico` |
| ARPURLINFOABOUT | From `app.url` | `https://example.com` |

### Directory Structure

The MSI directory tree is generated automatically:

```
TARGETDIR
├── ProgramFiles64Folder (or ProgramFilesFolder)
│   └── INSTALLDIR (app name directory)
├── ProgramMenuFolder
│   └── ApplicationProgramsFolder (if start_menu = true)
└── DesktopFolder (if desktop = true)
```

## Enterprise Deployment

### Silent Installation

```powershell
# Basic silent install
msiexec /i MyApp.msi /qn

# Silent install with logging
msiexec /i MyApp.msi /qn /l*v install.log

# Install to custom directory
msiexec /i MyApp.msi /qn INSTALLDIR="C:\Custom\Path"

# Wait for completion in scripts
msiexec /i MyApp.msi /qn /wait
```

### Group Policy Deployment

1. Open **Group Policy Management**
2. Create or edit a GPO
3. Navigate to **Computer Configuration → Policies → Software Settings → Software installation**
4. Right-click → **New → Package**
5. Select the MSI file (must be on a network share)
6. Choose **Assigned** or **Published**
7. The MSI installs automatically on next policy refresh

### SCCM Deployment

```powershell
# Create application in SCCM
# Source content: \\server\share\MyApp.msi
# Install program: msiexec /i MyApp.msi /qn
# Detection method: ProductCode GUID
```

### Intune Deployment

1. Upload MSI to **Apps → Windows apps + Add**
2. Select **Windows app (MSI)**
3. Configure assignment groups
4. MSI installs silently on enrolled devices

## Upgrade Support

MSI packages support **major upgrades** through the UpgradeCode mechanism:

### How It Works

1. Each MSI has a unique **ProductCode** (changes per version)
2. The **UpgradeCode** stays constant across versions
3. Windows Installer detects existing installations with the same UpgradeCode
4. Old version is removed, new version is installed

### Configuration

```toml
[app]
id = "com.mycompany.myapp"  # Used to generate stable UpgradeCode
version = "2.0.0"
```

The `app.id` field generates a deterministic UpgradeCode via UUID v5, ensuring the same UpgradeCode across all versions of your application.

### Upgrade Behavior

| Scenario | Behavior |
|----------|----------|
| Fresh install | Installs normally |
| Same version | No change (already installed) |
| Newer version available | Removes old, installs new |
| Older version | Blocked by Windows Installer |

## Custom Actions

Velocity generates MSI Custom Actions from your script configuration:

### Pre-install Commands

```toml
[scripts]
pre_install = [
    "taskkill /f /im myapp.exe",
    "net stop MyService"
]
```

These run before `InstallInitialize` (sequence 155+).

### Post-install Commands

```toml
[scripts]
post_install = [
    "net start MyService",
    "\"[INSTALLDIR]myapp.exe\" --register"
]
```

These run after `InstallFinalize` (sequence 401+).

### Launch After Install

```toml
[install]
run_after_install = "myapp.exe"
```

Creates a CustomAction that launches the application after installation.

## Services

Windows services defined in `velocity.toml` map to MSI ServiceInstall/ServiceControl tables:

```toml
[[services]]
name = "MyService"
display_name = "My Application Service"
description = "Background service for MyApp"
binary_path = "myapp-service.exe"
start_type = "auto"
start_on_install = true
remove_on_uninstall = true
```

### MSI Service Behavior

| Action | Behavior |
|--------|----------|
| Install | Service registered and started |
| Upgrade | Service stopped, re-registered, started |
| Uninstall | Service stopped and removed |

## Environment Variables

```toml
[[env_vars]]
name = "MYAPP_HOME"
value = "[INSTALLDIR]"
scope = "system"
delete_on_uninstall = true

[[env_vars]]
name = "PATH"
value = "[INSTALLDIR]bin"
scope = "system"
append = true
```

## Limitations

- **Windows-only output** — MSI packages only work on Windows
- **No custom UI** — MSI uses standard Windows Installer UI (not WebView2)
- **Cabinet-less approach** — Files embedded as streams (larger MSI size vs cabinet compression)
- **No patch/MST support** — Full MSI required for each version (transforms planned for future)
- **No digital signature** — MSI signing must be done separately with `signtool`

## Signing MSI Packages

After building, sign the MSI for enterprise trust:

```powershell
signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 ^
    /n "My Company" output\installer.msi
```

## Comparison: EXE vs MSI

| Feature | EXE Installer | MSI Package |
|---------|--------------|-------------|
| Custom UI | WebView2 modern UI | Standard Windows Installer UI |
| Silent install | `--silent` flag | `msiexec /qn` |
| Group Policy | Not supported | Fully supported |
| SCCM | Via command line | Native support |
| Delta updates | Supported | Not applicable |
| Self-extracting | Yes | No (requires msiexec) |
| File compression | Zstd/LZMA2 | MSI cabinet (or streams) |
| Encryption | AES-256-GCM | Not supported |
| Rollback | Built-in | MSI rollback |

## Troubleshooting

### MSI Won't Install

```powershell
# Enable verbose logging
msiexec /i MyApp.msi /l*v install.log

# Check Windows Installer service
sc query msiserver

# Re-register Windows Installer
msiexec /unregister
msiexec /regserver
```

### Upgrade Not Working

- Ensure `app.id` is set in `velocity.toml` for stable UpgradeCode
- Verify the old version was installed via MSI (not EXE)
- Check that ProductVersion follows semver

### Group Policy Not Applying

- MSI must be on a network share (not local path)
- Computer account needs read access to the share
- Check GPO scope and security filtering
- Run `gpupdate /force` and check Event Viewer
