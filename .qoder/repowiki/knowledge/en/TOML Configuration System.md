---
kind: configuration_system
name: TOML Configuration System
category: configuration
scope:
    - 'crates/velocity-config/**'
    - 'examples/**'
source_files:
    - crates/velocity-config/src/manifest.rs
    - crates/velocity-config/src/parser.rs
    - crates/velocity-config/src/variables.rs
    - crates/velocity-config/src/auto_gen.rs
---

The Velocity Installer uses a TOML-based configuration system (`velocity.toml`) to define all aspects of an installer package.

**Architecture:**
- **Declarative format** — All installer behavior defined in TOML, no scripting required
- **Path variable resolution** — Dynamic paths like `{app}`, `{autopf}`, `{win}` resolved at runtime
- **Auto-generation** — CLI can detect project structure from Cargo.toml/package.json
- **Deep validation** — `velocity check` validates all paths, registry roots, service configs
- **Modular sections** — Each feature area is a separate TOML section

**Configuration Schema:**
```toml
[app]                    # Application metadata
name = "My App"
version = "1.0.0"
publisher = "Company"
icon = "assets/icon.ico"
license = "LICENSE.txt"

[install]                # Installation behavior
default_dir = "{autopf}/MyApp"
start_menu = "My App"
require_admin = true
run_after_install = "app.exe"
close_app_before_install = "app.exe"
overwrite_mode = "always"     # always, skip, prompt, newer

[files]                  # Source files
source = ["./build/**"]
exclude = ["*.pdb", "*.tmp"]

[[components]]           # User-selectable features
id = "core"
name = "Core Application"
description = "Required binaries"
selected_by_default = true
mandatory = true
source = ["./bin/**"]

[[dependencies]]         # Remote dependencies
name = "VC++ 2022 Redistributable"
url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
sha256 = "optional-hash"
install_args = "/install /quiet /norestart"
condition = "not_installed:Microsoft Visual C++ 2022"
required = true
priority = 10

[[registry]]             # Windows Registry
key = "Software\\MyApp"
name = "InstallPath"
value = "{app}"
root = "HKLM"

[[env_vars]]             # Environment Variables
name = "MY_APP_HOME"
value = "{app}"
scope = "system"

[[file_associations]]    # File Type Associations
extension = ".myext"
description = "My App File"
handler = "myapp.exe"

[scripts]                # Pre/Post Scripts
pre_install = ["echo Installing..."]
post_install = ["echo Done!"]

[[scripts.post_install_actions]]  # Structured Actions
name = "Create config dir"
action = "mkdir"
path = "{install_dir}\\config"
on_error = "continue"

[localization]           # i18n
default_language = "en"

[[localization.languages]]
code = "de"
name = "Deutsch"
[localization.languages.strings]
btn_next = "Weiter (&N) >"

[uninstall]              # Uninstaller
add_remove = true
help_url = "https://support.example.com"

[ui]                     # UI Theme
theme = "classic"        # or "modern"
```

**Path Variables:**
| Variable | Resolves To |
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
| `{install_dir}` | Same as `{app}` |
| `{app_name}` | Application name from config |
| `{version}` | Application version from config |

**Variable Resolution:**
```rust
// crates/velocity-config/src/variables.rs
pub fn resolve_variables(input: &str, context: &PathContext) -> String {
    // Replaces all {variable} patterns with resolved values
    // Handles nested resolution and unknown variables
}
```

**Auto-Generation:**
```rust
// crates/velocity-config/src/auto_gen.rs
pub fn auto_detect_config(project_dir: &Path) -> Result<Manifest> {
    // Detects Cargo.toml → extracts name, version, publisher
    // Detects package.json → extracts name, version
    // Scans for common directories (bin, docs, assets)
    // Generates velocity.toml with detected values
}
```

**Validation (`velocity check`):**
- All source file paths exist
- Registry roots are valid (HKLM, HKCU, HKCR, HKU)
- Service configurations are complete
- Path variables resolve correctly
- Dependencies have valid URLs
- Conditions are well-formed
- Icon file exists
- License file exists

**Key files:**
- `crates/velocity-config/src/manifest.rs` — Struct definitions (serde)
- `crates/velocity-config/src/parser.rs` — TOML parsing and validation
- `crates/velocity-config/src/variables.rs` — Path variable resolution
- `crates/velocity-config/src/auto_gen.rs` — Auto-detection from project

**Rules for developers:**
1. All new config fields must have serde defaults
2. Path variables must be resolved before use
3. Validation must catch all common misconfigurations
4. Auto-generation should handle Cargo.toml and package.json
5. Keep the TOML schema backward-compatible
