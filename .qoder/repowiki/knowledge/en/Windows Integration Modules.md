---
kind: windows_integration
name: Windows Integration Modules
category: platform_integration
scope:
    - 'crates/velocity-core/src/registry.rs'
    - 'crates/velocity-core/src/shortcuts.rs'
    - 'crates/velocity-core/src/services.rs'
    - 'crates/velocity-core/src/env_vars.rs'
    - 'crates/velocity-core/src/file_assoc.rs'
    - 'crates/velocity-core/src/uninstaller.rs'
source_files:
    - crates/velocity-core/src/registry.rs
    - crates/velocity-core/src/shortcuts.rs
    - crates/velocity-core/src/services.rs
    - crates/velocity-core/src/env_vars.rs
    - crates/velocity-core/src/file_assoc.rs
    - crates/velocity-core/src/uninstaller.rs
---

Velocity Installer provides deep Windows integration through six dedicated modules covering registry, shortcuts, services, environment variables, file associations, and uninstaller generation. All modules are Windows-only (guarded by `cfg(target_os = "windows")`) except where noted.

**Module Overview:**

| Module | Technology | Scope | Rollback |
|--------|-----------|-------|----------|
| `registry.rs` | `winreg` crate | HKLM, HKCU, HKCR, HKU | Full |
| `shortcuts.rs` | IShellLink COM | Desktop, Start Menu, custom | Full |
| `services.rs` | Windows Service API | Install, start, stop, remove | Full |
| `env_vars.rs` | Registry + WM_SETTINGCHANGE | System, User | Full |
| `file_assoc.rs` | Registry (HKCR) | Extension → handler mapping | Full |
| `uninstaller.rs` | Self-generated .exe | Add/Remove Programs entry | N/A |

**Registry Module:**
```rust
// Supported value types:
// - "string" / "REG_SZ" — String value
// - "dword" / "REG_DWORD" — 32-bit integer
// - "expand_string" / "REG_EXPAND_SZ" — Expandable string
// - "multi_string" / "REG_MULTI_SZ" — Multi-string (array)

// Root keys: HKLM, HKCU, HKCR, HKU
pub fn apply_registry_entries(entries: &[RegistryEntry]) -> Result<()>
```

**Shortcuts Module (IShellLink COM):**
```rust
// Creates .lnk files using the Windows Shell COM interface
// Properties: target path, arguments, working directory, icon, description
// Locations: Desktop, Start Menu, Quick Launch, custom paths
```

**Services Module:**
```rust
// Windows service management:
// - Install service with binary path and display name
// - Configure start type: auto, manual, disabled, delayed-auto
// - Set service dependencies
// - Start/stop/remove services
// - Rollback: restore original start type or remove newly installed service
```

**Environment Variables Module:**
```rust
// Two scopes:
// - "system" — HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment
// - "user"   — HKCU\Environment
// After modification, broadcasts WM_SETTINGCHANGE to notify all windows
// Supports append mode (add to existing value with separator)
```

**File Associations Module:**
```rust
// Registers file types in the registry:
// HKCR\.ext → ProgID
// HKCR\ProgID → description
// HKCR\ProgID\DefaultIcon → icon path
// HKCR\ProgID\shell\open\command → handler command
```

**Uninstaller Generation:**
```rust
// Creates a self-contained uninstaller that:
// 1. Shows confirmation dialog
// 2. Stops and removes services
// 3. Deletes registry entries (reverse order)
// 4. Removes shortcuts
// 5. Removes environment variables
// 6. Unregisters file associations
// 7. Deletes installed files
// 8. Removes Add/Remove Programs entry
// 9. Removes the uninstaller itself
// 10. Removes empty directories
```

**Add/Remove Programs Entry:**
```rust
// Written to: HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{AppName}
// Fields:
// - DisplayName, DisplayVersion, Publisher
// - InstallDate, InstallLocation
// - UninstallString, HelpLink, UpdateLink
// - EstimatedSize (calculated from installed files)
```

**Key files:**
- `crates/velocity-core/src/registry.rs` — Registry operations via winreg (234 lines)
- `crates/velocity-core/src/shortcuts.rs` — IShellLink COM shortcut creation
- `crates/velocity-core/src/services.rs` — Windows service management
- `crates/velocity-core/src/env_vars.rs` — Environment variable management
- `crates/velocity-core/src/file_assoc.rs` — File type association registration
- `crates/velocity-core/src/uninstaller.rs` — Self-contained uninstaller generation

**Rules for developers:**
1. All Windows-only modules must be guarded with `#[cfg(target_os = "windows")]`
2. All operations must be tracked in the rollback system
3. Registry operations must support all four root keys (HKLM, HKCU, HKCR, HKU)
4. Environment variable changes must broadcast WM_SETTINGCHANGE
5. Services must be stopped before removal
6. The uninstaller must undo operations in reverse order
7. Add/Remove Programs EstimatedSize should be calculated from actual installed files
