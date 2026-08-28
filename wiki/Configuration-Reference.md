# Configuration Reference

Complete reference for `velocity.toml` — the configuration file that defines your installer.

## File Structure

```toml
[app]           # Application metadata
[install]       # Installation behavior
[files]         # File inclusion rules
[shortcuts]     # Desktop/Start Menu shortcuts
[ui]            # UI theme and behavior
[uninstall]     # Uninstaller settings
[scripts]       # Pre/post install scripts
[localization]  # i18n strings
[fetch]         # Cloud-fetch configuration
[[components]]  # Optional features
[[dependencies]] # Remote dependencies
[[bundled_apps]] # Bundled third-party apps
[[registry]]    # Registry entries
[[env_vars]]    # Environment variables
[[services]]    # Windows services
[[file_associations]] # File type associations
```

---

## `[app]` — Application Metadata

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Application name |
| `version` | string | Yes | Semantic version (e.g., `"1.0.0"`) |
| `publisher` | string | No | Company or publisher name |
| `icon` | string | No | Path to `.ico` file |
| `license` | string | No | Path to license file (displayed in wizard) |
| `url` | string | No | Application website URL |
| `id` | string | No | Unique identifier (used for UpgradeCode) |

### Example

```toml
[app]
name = "My Application"
version = "1.2.3"
publisher = "My Company"
icon = "assets/icon.ico"
license = "LICENSE.txt"
url = "https://myapp.com"
id = "com.mycompany.myapp"
```

---

## `[install]` — Installation Behavior

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `default_dir` | string | `"{autopf}/{app_name}"` | Default installation directory |
| `start_menu` | string/bool | `true` | Start Menu folder name or `false` |
| `require_admin` | bool | `false` | Require administrator privileges |
| `run_after_install` | string | — | Executable to launch after install |
| `close_app_before_install` | string | — | Process name to kill before install |
| `overwrite_behavior` | string | `"always"` | `always`, `skip`, `prompt`, `newer_only` |
| `create_uninstaller` | bool | `true` | Generate uninstaller |
| `compression` | string | `"zstd"` | `zstd` or `lzma2` |
| `compression_level` | int | `9` | Compression level (1-22 for zstd) |

### Example

```toml
[install]
default_dir = "{autopf}/MyApp"
start_menu = "My Application"
require_admin = true
run_after_install = "myapp.exe"
close_app_before_install = "myapp.exe"
overwrite_behavior = "newer_only"
compression = "zstd"
compression_level = 15
```

---

## `[files]` — File Inclusion

| Field | Type | Description |
|-------|------|-------------|
| `source` | array | Glob patterns for files to include |
| `exclude` | array | Glob patterns for files to exclude |
| `flatten` | bool | Flatten directory structure (default: `false`) |

### Example

```toml
[files]
source = ["./build-output/**"]
exclude = ["*.pdb", "*.tmp", "*.log"]
flatten = false
```

### Compression Settings

```toml
[files.compression]
format = "zstd"    # zstd, lzma2
level = 9          # Compression level
```

---

## `[shortcuts]` — Shortcuts

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `desktop` | bool | `false` | Create desktop shortcut |
| `start_menu` | bool | `true` | Create Start Menu shortcuts |
| `quick_launch` | bool | `false` | Create Quick Launch shortcut |

### Example

```toml
[shortcuts]
desktop = true
start_menu = true
quick_launch = false
```

---

## `[ui]` — User Interface

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `theme` | string | `"classic"` | `classic` (Win32) or `modern` (WebView2) |
| `language` | string | `"en"` | Default UI language |
| `show_welcome` | bool | `true` | Show welcome page |
| `show_license` | bool | `true` | Show license agreement page |
| `show_directory` | bool | `true` | Show directory selection page |
| `show_components` | bool | `true` | Show component selection page |

### Example

```toml
[ui]
theme = "modern"
language = "en"
show_welcome = true
show_license = true
show_directory = true
show_components = true
```

---

## `[uninstall]` — Uninstaller Settings

| Field | Type | Description |
|-------|------|-------------|
| `add_remove` | bool | Register in Add/Remove Programs |
| `help_url` | string | Support URL |
| `update_url` | string | Update URL |
| `comments` | string | ARP comments |

### Example

```toml
[uninstall]
add_remove = true
help_url = "https://support.myapp.com"
update_url = "https://updates.myapp.com"
```

---

## `[scripts]` — Pre/Post Install Scripts

| Field | Type | Description |
|-------|------|-------------|
| `pre_install` | array | Commands to run before installation |
| `post_install` | array | Commands to run after installation |

### Example

```toml
[scripts]
pre_install = [
    "taskkill /f /im myapp.exe",
    "net stop MyService"
]
post_install = [
    "net start MyService",
    "\"[INSTALLDIR]myapp.exe\" --register"
]
```

### Structured Actions

For more control, use structured actions instead of shell commands:

```toml
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

[[scripts.post_install_actions]]
name = "Set registry"
action = "registry"
key = "HKLM\\Software\\MyApp"
value_name = "Installed"
value_data = "1"
value_type = "REG_DWORD"
```

**Action types:** `shell`, `copy`, `delete`, `mkdir`, `registry`, `env_var`

**Error policies:** `abort`, `continue`, `retry`

---

## `[localization]` — Internationalization

| Field | Type | Description |
|-------|------|-------------|
| `default_language` | string | Default language code |
| `languages` | array | Additional language definitions |

### Example

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
btn_finish = "Fertigstellen (&F)"
btn_cancel = "Abbrechen"
title_welcome = "Willkommen"
```

---

## `[[components]]` — Optional Features

Define user-selectable installation components.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | string | — | Unique component identifier |
| `name` | string | — | Display name |
| `description` | string | — | Description shown in UI |
| `selected_by_default` | bool | `true` | Pre-selected in UI |
| `mandatory` | bool | `false` | Cannot be deselected |
| `source` | array | — | File patterns for this component |
| `depends_on` | array | — | Component IDs this depends on |

### Example

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
depends_on = ["core"]
```

---

## `[[dependencies]]` — Remote Dependencies

Auto-download and install prerequisites.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | — | Display name |
| `url` | string | — | Download URL |
| `sha256` | string | — | Expected SHA256 hash |
| `install_args` | string | — | Silent install arguments |
| `condition` | string | `"always"` | When to install |
| `required` | bool | `true` | Fail install if dependency fails |
| `priority` | int | `100` | Installation order (lower = first) |

### Conditions

| Condition | Description |
|-----------|-------------|
| `always` | Always install |
| `never` | Never install |
| `registry_missing:HKLM\Software\Foo` | Install if key missing |
| `registry_exists:HKLM\Software\Foo` | Install if key exists |
| `file_missing:C:\path\to\file` | Install if file missing |
| `file_exists:C:\path\to\file` | Install if file exists |
| `not_installed:Product Name` | Install if not in ARP |
| `installed:Product Name` | Install if in ARP |
| `arch:x64` | Install only on x64 |
| `arch:x86` | Install only on x86 |
| `os_version:>=10.0` | Install on Windows 10+ |

### Example

```toml
[[dependencies]]
name = "VC++ 2022 Redistributable"
url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
sha256 = "abc123..."
install_args = "/install /quiet /norestart"
condition = "not_installed:Microsoft Visual C++ 2022"
required = true
priority = 10

[[dependencies]]
name = ".NET Desktop Runtime 8"
url = "https://download.visualstudio.microsoft.com/download/pr/.../windowsdesktop-runtime-8.0.x-win-x64.exe"
install_args = "/install /quiet /norestart"
condition = "not_installed:.NET Desktop Runtime 8"
required = true
priority = 20
```

---

## `[[bundled_apps]]` — Bundled Third-Party Apps

Include installers in your payload.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name |
| `installer` | string | Path to bundled installer |
| `install_args` | string | Silent install arguments |
| `condition` | string | When to install |
| `required` | bool | Fail if this app fails |
| `priority` | int | Installation order |

### Example

```toml
[[bundled_apps]]
name = "Notepad++"
installer = "third-party/npp-installer.exe"
install_args = "/S"
condition = "not_installed:Notepad++"
required = false
priority = 100
```

---

## `[[registry]]` — Registry Entries

| Field | Type | Description |
|-------|------|-------------|
| `root` | string | Registry root (`HKLM`, `HKCU`, `HKCR`, `HKU`) |
| `key` | string | Registry key path |
| `name` | string | Value name |
| `value` | string | Value data |
| `type` | string | Value type (`REG_SZ`, `REG_DWORD`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`) |

### Example

```toml
[[registry]]
root = "HKLM"
key = "Software\\MyApp"
name = "InstallPath"
value = "{app}"
type = "REG_SZ"

[[registry]]
root = "HKLM"
key = "Software\\MyApp"
name = "Version"
value = "1.0.0"
type = "REG_SZ"

[[registry]]
root = "HKCU"
key = "Software\\MyApp\\Settings"
name = "AutoUpdate"
value = "1"
type = "REG_DWORD"
```

---

## `[[env_vars]]` — Environment Variables

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | — | Variable name |
| `value` | string | — | Variable value |
| `scope` | string | `"system"` | `system` or `user` |
| `append` | bool | `false` | Append to existing value |
| `delete_on_uninstall` | bool | `true` | Remove on uninstall |

### Example

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

---

## `[[services]]` — Windows Services

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | — | Service name |
| `display_name` | string | — | Display name |
| `description` | string | — | Service description |
| `binary_path` | string | — | Path to service executable |
| `start_type` | string | `"manual"` | `auto`, `manual`, `disabled` |
| `start_on_install` | bool | `true` | Start service after install |
| `remove_on_uninstall` | bool | `true` | Remove service on uninstall |

### Example

```toml
[[services]]
name = "MyAppService"
display_name = "My Application Service"
description = "Background service for MyApp"
binary_path = "myapp-service.exe"
start_type = "auto"
start_on_install = true
remove_on_uninstall = true
```

---

## `[[file_associations]]` — File Type Associations

| Field | Type | Description |
|-------|------|-------------|
| `extension` | string | File extension (e.g., `".myext"`) |
| `description` | string | File type description |
| `handler` | string | Executable to open files |
| `icon` | string | Icon for file type |

### Example

```toml
[[file_associations]]
extension = ".myext"
description = "My Application File"
handler = "myapp.exe"
icon = "myapp-file.ico"
```

---

## `[fetch]` — Cloud-Fetch Configuration

See [[Cloud-Fetch-Installers]] for full details.

### Git Release Mode

```toml
[fetch]
mode = "git-release"
platform = "github"
repo = "user/myapp"
asset_pattern = "{app}-{version}-win-{arch}.zip"
api_url = "https://github.example.com/api/v3"  # Optional: self-hosted

[fetch.files]
download = [
  { pattern = "*.exe", dest = "bin/" },
  { pattern = "*.dll", dest = "bin/" }
]

[fetch.update]
check_interval = "24h"
auto_download = false
auto_install = false
show_notification = true
```

### URL Mode

```toml
[fetch]
mode = "url"
base_url = "https://releases.example.com/myapp"
version_url = "https://releases.example.com/myapp/version.txt"
asset_pattern = "{app}-{version}-win-{arch}.zip"
checksum_url = "https://releases.example.com/myapp/{version}/SHA256SUMS"
```

---

## Path Variables

Use these in any string field:

| Variable | Description |
|----------|-------------|
| `{app}` | Installation directory |
| `{autopf}` | Program Files (arch-aware) |
| `{win}` | Windows directory |
| `{sys}` | System32 directory |
| `{tmp}` | Temp directory |
| `{home}` | User home directory |
| `{desktop}` | Desktop folder |
| `{programs}` | Start Menu Programs |
| `{sendto}` | SendTo folder |
| `{startup}` | Startup folder |

---

## Complete Example

```toml
[app]
name = "My Application"
version = "1.0.0"
publisher = "My Company"
icon = "assets/icon.ico"
license = "LICENSE.txt"
url = "https://myapp.com"
id = "com.mycompany.myapp"

[install]
default_dir = "{autopf}/MyApp"
start_menu = "My Application"
require_admin = true
run_after_install = "myapp.exe"
close_app_before_install = "myapp.exe"
overwrite_behavior = "newer_only"
compression = "zstd"
compression_level = 15

[files]
source = ["./build-output/**"]
exclude = ["*.pdb", "*.tmp"]

[shortcuts]
desktop = true
start_menu = true

[ui]
theme = "modern"

[uninstall]
add_remove = true
help_url = "https://support.myapp.com"
update_url = "https://updates.myapp.com"

[scripts]
pre_install = ["taskkill /f /im myapp.exe"]
post_install = ["echo Installation complete!"]

[[components]]
id = "core"
name = "Core Application"
description = "Required files"
mandatory = true
source = ["./build/bin/**"]

[[components]]
id = "docs"
name = "Documentation"
description = "User manuals"
selected_by_default = true
source = ["./docs/**"]

[[dependencies]]
name = "VC++ 2022 Redistributable"
url = "https://aka.ms/vs/17/release/vc_redist.x64.exe"
install_args = "/install /quiet /norestart"
condition = "not_installed:Microsoft Visual C++ 2022"
required = true
priority = 10

[[registry]]
root = "HKLM"
key = "Software\\MyApp"
name = "InstallPath"
value = "{app}"

[[env_vars]]
name = "MYAPP_HOME"
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

[localization]
default_language = "en"

[[localization.languages]]
code = "de"
name = "Deutsch"
[localization.languages.strings]
btn_next = "Weiter (&N) >"
btn_install = "Installieren (&I)"
```
