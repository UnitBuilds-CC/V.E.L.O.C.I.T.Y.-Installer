# Sample App — Velocity Installer Example

A realistic sample installer project that exercises all major Velocity features.
Use this as a starting template for your own application, or as a beta test to
verify the installer pipeline.

## Project Structure

```
examples/sample-app/
├── velocity.toml          # Installer configuration (all features enabled)
├── LICENSE.txt            # License agreement shown in the wizard
├── update-check.json      # Self-update version endpoint
├── assets/                # Installer icon (add your own icon.ico)
├── files/
│   ├── bin/               # Core application binaries (mandatory component)
│   ├── docs/              # Documentation (optional component, 2.4 MB)
│   ├── sdk/               # Developer SDK (optional component, 8.1 MB)
│   └── samples/           # Sample projects (optional component, 1.2 MB)
└── README.md              # This file
```

## Features Exercised

| Feature | velocity.toml Section |
|---|---|
| Component selection | `[[components]]` — core, docs, sdk, samples |
| Localization (4 languages) | `[localization]` — en, de, es, fr, ja |
| Registry entries | `[[registry]]` — HKLM + HKCU keys |
| Environment variables | `[[env_vars]]` — SAMPLE_APP_HOME, SAMPLE_APP_VERSION |
| File associations | `[[file_associations]]` — .sample extension |
| Structured scripts | `[[scripts.post_install_actions]]` — mkdir, copy |
| Self-update | `[uninstall] update_url` — JSON endpoint |
| Silent mode | `/S /D=path /P=password` |
| Desktop shortcut | `[install] create_desktop_shortcut = true` |

## Quick Start

```bash
# From the repository root
cd examples/sample-app

# Validate the configuration
velocity check

# Build the installer
velocity build

# Test: silent install to a custom directory
output\sample-app-installer.exe /S /D=C:\TestInstall

# Test: normal wizard install
output\sample-app-installer.exe

# Verify files were installed
dir C:\TestInstall

# Uninstall
C:\TestInstall\uninstall.exe --force
```

## Beta Testing Checklist

Use this checklist when testing the installer on real machines:

### Pre-Install
- [ ] Installer launches without errors
- [ ] Welcome page shows correct app name and version
- [ ] License agreement displays correctly
- [ ] Directory selection page works (Browse button, path validation)
- [ ] Component selection shows all 4 components with sizes
- [ ] Disk space label shows total required space
- [ ] Localization switches correctly for all 4 languages

### Installation
- [ ] Progress bar advances smoothly
- [ ] Files are extracted to the chosen directory
- [ ] Registry entries are created (check with regedit)
- [ ] Environment variables are set (check with `set` in cmd)
- [ ] Desktop shortcut is created
- [ ] Start Menu entries are created
- [ ] File association for `.sample` is registered

### Post-Install
- [ ] "Launch application" checkbox works
- [ ] Add/Remove Programs shows the app (check Settings > Apps)
- [ ] Update check runs at startup (check log file)

### Silent Mode
- [ ] `/S` installs without any UI
- [ ] `/D=path` installs to the custom directory
- [ ] `/S /D=C:\Windows` is rejected (system dir validation)
- [ ] Exit code is 0 on success

### Uninstall
- [ ] Uninstaller removes all installed files
- [ ] Registry entries are cleaned up
- [ ] Environment variables are removed
- [ ] Start Menu and Desktop shortcuts are removed
- [ ] Add/Remove Programs entry is removed

### Edge Cases
- [ ] Running installer twice shows "already running" message
- [ ] Installing to a path with Unicode characters works
- [ ] Cancel during installation triggers rollback
- [ ] Install with encrypted payload + wrong password shows error

## Encrypted Installer Test

```bash
# Build with encryption
velocity build --encrypt --password "test_password"

# Silent install with password
output\sample-app-installer.exe /S /P=test_password /D=C:\TestInstall

# Wrong password should fail
output\sample-app-installer.exe /S /P=wrong_password /D=C:\TestInstall
```
