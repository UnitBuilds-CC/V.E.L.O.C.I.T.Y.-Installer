# Plugin System

Velocity Installer supports **WASM-based plugins** that extend installer behavior with sandboxed custom actions. Plugins run in a secure Wasmtime runtime with no direct system access.

## Overview

### Key Features

- **WASM-based** — Plugins compile to WebAssembly for portability and safety
- **Sandboxed** — No direct system access; all operations go through Host API
- **9 lifecycle hooks** — Intercept every stage of installation
- **Host API** — Plugins can log, read/write files, execute commands, access registry, update progress
- **Auto-discovery** — Drop `.wasm` + `plugin.json` in the `plugins/` directory
- **Safe by default** — Wasmtime runtime prevents memory corruption and unauthorized access

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Velocity Installer                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              Plugin Manager                           │  │
│  │  ┌────────────────────────────────────────────────┐  │  │
│  │  │  Plugin 1 (plugin.wasm + plugin.json)          │  │  │
│  │  │  ┌──────────────────────────────────────────┐  │  │  │
│  │  │  │  Wasmtime Runtime (sandboxed)            │  │  │  │
│  │  │  │  - Memory isolation                      │  │  │  │
│  │  │  │  - CPU time limits                       │  │  │  │
│  │  │  │  - Host API access only                  │  │  │  │
│  │  │  └──────────────────────────────────────────┘  │  │  │
│  │  └────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────┐  │  │
│  │  │  Plugin 2 (plugin.wasm + plugin.json)          │  │  │
│  │  └────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              Host API                                 │  │
│  │  - Log messages                                      │  │
│  │  - Read/write files                                  │  │
│  │  - Execute commands                                  │  │
│  │  - Access registry (Windows)                         │  │
│  │  - Update progress                                   │  │
│  │  - Query install state                               │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Plugin Lifecycle

### 9 Hooks

Plugins can intercept these stages of the installation process:

| Hook | When Called | Use Case |
|------|-------------|----------|
| `on_load` | Plugin initialization | Setup, configuration loading |
| `on_pre_install` | Before installation starts | Validation, pre-flight checks |
| `on_file_extracted` | After each file extraction | File validation, custom processing |
| `on_post_install` | After installation completes | Cleanup, registration, notifications |
| `on_error` | On installation error | Error handling, rollback logic |
| `on_cancel` | On user cancellation | Cleanup, state restoration |
| `on_uninstall` | During uninstallation | Custom uninstall logic |
| `on_upgrade` | During version upgrade | Migration, data preservation |
| `on_rollback` | During rollback | Custom rollback actions |

### Execution Order

```
1. Installer starts
2. Plugin manager discovers plugins in plugins/ directory
3. For each plugin:
   a. Load plugin.wasm
   b. Call on_load()
4. Installation begins
5. Call on_pre_install() on all plugins
6. For each file extracted:
   a. Call on_file_extracted() on all plugins
7. Installation completes
8. Call on_post_install() on all plugins
9. On error: call on_error() on all plugins
10. On cancel: call on_cancel() on all plugins
```

---

## Creating a Plugin

### Step 1: Create plugin.json

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "My custom plugin",
  "author": "My Company",
  "hooks": ["on_pre_install", "on_post_install"]
}
```

### Step 2: Write Plugin Code (Rust)

```rust
// src/lib.rs
use velocity_plugin_api::{Plugin, PluginResult, HostApi};

struct MyPlugin;

impl Plugin for MyPlugin {
    fn on_pre_install(&self, api: &HostApi) -> PluginResult<()> {
        api.log("Running pre-install checks...")?;
        
        // Check if required file exists
        if !api.file_exists("C:\\required.txt")? {
            return Err("Required file not found".into());
        }
        
        Ok(())
    }
    
    fn on_post_install(&self, api: &HostApi) -> PluginResult<()> {
        api.log("Installation complete!")?;
        
        // Update progress
        api.set_progress(100, "Finalizing...")?;
        
        Ok(())
    }
}

// Export the plugin
velocity_plugin_api::export_plugin!(MyPlugin);
```

### Step 3: Compile to WASM

```bash
# Install wasm32-wasi target
rustup target add wasm32-wasi

# Build
cargo build --target wasm32-wasi --release

# Output: target/wasm32-wasi/release/my_plugin.wasm
```

### Step 4: Deploy Plugin

Copy the `.wasm` and `plugin.json` to your installer's `plugins/` directory:

```
my-installer/
├── velocity.toml
├── plugins/
│   └── my-plugin/
│       ├── plugin.wasm
│       └── plugin.json
└── files/
    └── (your app files)
```

---

## Host API

Plugins interact with the installer through the Host API:

### Logging

```rust
api.log("This is a log message")?;
api.log_error("This is an error")?;
api.log_warning("This is a warning")?;
```

### File Operations

```rust
// Check if file exists
if api.file_exists("C:\\path\\to\\file")? {
    // ...
}

// Read file
let content = api.read_file("C:\\path\\to\\file")?;

// Write file
api.write_file("C:\\path\\to\\file", b"content")?;

// Delete file
api.delete_file("C:\\path\\to\\file")?;
```

### Command Execution

```rust
// Execute command
let output = api.execute_command("cmd", &["/C", "echo Hello"])?;
println!("Exit code: {}", output.exit_code);
println!("Stdout: {}", output.stdout);
```

### Registry (Windows)

```rust
// Read registry
let value = api.registry_get("HKLM", "Software\\MyApp", "InstallPath")?;

// Write registry
api.registry_set("HKLM", "Software\\MyApp", "Version", "1.0.0")?;

// Check if key exists
if api.registry_exists("HKLM", "Software\\MyApp")? {
    // ...
}
```

### Progress Updates

```rust
// Update progress bar
api.set_progress(50, "Halfway there...")?;

// Set status text
api.set_status("Processing files...")?;
```

### Install State

```rust
// Get install directory
let install_dir = api.get_install_dir()?;

// Get app name
let app_name = api.get_app_name()?;

// Get app version
let app_version = api.get_app_version()?;

// Check if running in silent mode
if api.is_silent()? {
    // Skip UI prompts
}
```

---

## Plugin Configuration

### Enabling Plugins

Plugins are automatically discovered from the `plugins/` directory. No configuration needed in `velocity.toml`.

### Plugin Order

Plugins are loaded in alphabetical order by directory name:

```
plugins/
├── 01-first-plugin/    # Loaded first
├── 02-second-plugin/   # Loaded second
└── 03-third-plugin/    # Loaded third
```

### Disabling Plugins

Remove the plugin directory or rename it to `*.disabled`:

```bash
mv plugins/my-plugin plugins/my-plugin.disabled
```

---

## Security Model

### Sandboxing

Plugins run in a **Wasmtime** sandbox with these restrictions:

| Restriction | Description |
|-------------|-------------|
| **Memory isolation** | Plugins cannot access host memory directly |
| **CPU time limits** | Plugins have execution time limits |
| **No direct I/O** | All file/network operations go through Host API |
| **No unsafe code** | WASM prevents unsafe memory operations |
| **Deterministic execution** | No access to system time, random, etc. (unless provided by Host API) |

### Host API Permissions

The Host API can be restricted per plugin:

```json
{
  "name": "my-plugin",
  "permissions": {
    "file_read": ["C:\\allowed\\path\\*"],
    "file_write": ["C:\\allowed\\path\\*"],
    "registry_read": ["HKLM\\Software\\MyApp\\*"],
    "registry_write": ["HKLM\\Software\\MyApp\\*"],
    "command_execute": ["cmd", "powershell"]
  }
}
```

### Audit Logging

All Host API calls are logged for security auditing:

```
[PLUGIN] my-plugin: log("Hello")
[PLUGIN] my-plugin: file_exists("C:\\path\\to\\file")
[PLUGIN] my-plugin: registry_get("HKLM", "Software\\MyApp", "Version")
```

---

## Example Plugins

### Example 1: License Validator

Validates a license file before installation:

```rust
use velocity_plugin_api::{Plugin, PluginResult, HostApi};

struct LicenseValidator;

impl Plugin for LicenseValidator {
    fn on_pre_install(&self, api: &HostApi) -> PluginResult<()> {
        api.log("Validating license...")?;
        
        let license_path = format!("{}\\license.key", api.get_install_dir()?);
        
        if !api.file_exists(&license_path)? {
            return Err("License file not found. Please install a valid license.".into());
        }
        
        let content = api.read_file(&license_path)?;
        if content.len() < 32 {
            return Err("Invalid license file".into());
        }
        
        api.log("License validated successfully")?;
        Ok(())
    }
}

velocity_plugin_api::export_plugin!(LicenseValidator);
```

### Example 2: Post-Install Notifier

Sends a notification after installation:

```rust
use velocity_plugin_api::{Plugin, PluginResult, HostApi};

struct PostInstallNotifier;

impl Plugin for PostInstallNotifier {
    fn on_post_install(&self, api: &HostApi) -> PluginResult<()> {
        let app_name = api.get_app_name()?;
        let install_dir = api.get_install_dir()?;
        
        api.log(&format!("{} installed to {}", app_name, install_dir))?;
        
        // Write installation log
        let log_content = format!(
            "App: {}\nDir: {}\nDate: {}",
            app_name,
            install_dir,
            chrono::Utc::now()
        );
        
        api.write_file(
            &format!("{}\\install_log.txt", install_dir),
            log_content.as_bytes()
        )?;
        
        Ok(())
    }
}

velocity_plugin_api::export_plugin!(PostInstallNotifier);
```

### Example 3: Custom Rollback

Performs custom cleanup during rollback:

```rust
use velocity_plugin_api::{Plugin, PluginResult, HostApi};

struct CustomRollback;

impl Plugin for CustomRollback {
    fn on_rollback(&self, api: &HostApi) -> PluginResult<()> {
        api.log("Performing custom rollback...")?;
        
        let install_dir = api.get_install_dir()?;
        
        // Remove custom files
        api.delete_file(&format!("{}\\custom.dat", install_dir))?;
        
        // Remove custom registry keys
        api.registry_delete("HKLM", "Software\\MyApp\\Custom")?;
        
        api.log("Custom rollback complete")?;
        Ok(())
    }
}

velocity_plugin_api::export_plugin!(CustomRollback);
```

---

## Plugin API Reference

### Plugin Trait

```rust
pub trait Plugin {
    fn on_load(&self, api: &HostApi) -> PluginResult<()> { Ok(()) }
    fn on_pre_install(&self, api: &HostApi) -> PluginResult<()> { Ok(()) }
    fn on_file_extracted(&self, api: &HostApi, path: &Path) -> PluginResult<()> { Ok(()) }
    fn on_post_install(&self, api: &HostApi) -> PluginResult<()> { Ok(()) }
    fn on_error(&self, api: &HostApi, error: &str) -> PluginResult<()> { Ok(()) }
    fn on_cancel(&self, api: &HostApi) -> PluginResult<()> { Ok(()) }
    fn on_uninstall(&self, api: &HostApi) -> PluginResult<()> { Ok(()) }
    fn on_upgrade(&self, api: &HostApi, old_version: &str, new_version: &str) -> PluginResult<()> { Ok(()) }
    fn on_rollback(&self, api: &HostApi) -> PluginResult<()> { Ok(()) }
}
```

### HostApi Methods

```rust
pub struct HostApi {
    // Logging
    pub fn log(&self, message: &str) -> PluginResult<()>;
    pub fn log_error(&self, message: &str) -> PluginResult<()>;
    pub fn log_warning(&self, message: &str) -> PluginResult<()>;
    
    // File operations
    pub fn file_exists(&self, path: &str) -> PluginResult<bool>;
    pub fn read_file(&self, path: &str) -> PluginResult<Vec<u8>>;
    pub fn write_file(&self, path: &str, content: &[u8]) -> PluginResult<()>;
    pub fn delete_file(&self, path: &str) -> PluginResult<()>;
    
    // Command execution
    pub fn execute_command(&self, program: &str, args: &[&str]) -> PluginResult<CommandOutput>;
    
    // Registry (Windows)
    pub fn registry_get(&self, root: &str, key: &str, name: &str) -> PluginResult<String>;
    pub fn registry_set(&self, root: &str, key: &str, name: &str, value: &str) -> PluginResult<()>;
    pub fn registry_exists(&self, root: &str, key: &str) -> PluginResult<bool>;
    pub fn registry_delete(&self, root: &str, key: &str) -> PluginResult<()>;
    
    // Progress
    pub fn set_progress(&self, percent: u8, message: &str) -> PluginResult<()>;
    pub fn set_status(&self, message: &str) -> PluginResult<()>;
    
    // Install state
    pub fn get_install_dir(&self) -> PluginResult<String>;
    pub fn get_app_name(&self) -> PluginResult<String>;
    pub fn get_app_version(&self) -> PluginResult<String>;
    pub fn is_silent(&self) -> PluginResult<bool>;
}
```

---

## Testing Plugins

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use velocity_plugin_api::MockHostApi;
    
    #[test]
    fn test_license_validator() {
        let api = MockHostApi::new();
        api.set_file_exists("C:\\license.key", true);
        api.set_file_content("C:\\license.key", b"valid-license-content");
        
        let plugin = LicenseValidator;
        assert!(plugin.on_pre_install(&api).is_ok());
    }
}
```

### Integration Tests

Deploy the plugin to a test installer and run through the installation process:

```bash
# Build plugin
cargo build --target wasm32-wasi --release

# Copy to test installer
cp target/wasm32-wasi/release/my_plugin.wasm test-installer/plugins/my-plugin/

# Build test installer
cd test-installer
velocity build

# Run installer
output/test_installer.exe
```

---

## Troubleshooting

### Plugin Not Loading

- Check that `plugin.json` is valid JSON
- Verify the `.wasm` file exists in the same directory
- Check the plugin name matches in `plugin.json`

### Plugin Crashes

- Check the install log for error messages
- Verify all Host API calls are within permissions
- Test the plugin locally with `wasmtime run plugin.wasm`

### Performance Issues

- Plugins run synchronously — slow plugins block installation
- Minimize Host API calls (batch operations when possible)
- Avoid large file reads/writes in plugins

---

## Further Reading

- [[Architecture]] — System design and crate structure
- [[Security]] — Plugin security model
- [[Contributing]] — Contributing plugins to Velocity
