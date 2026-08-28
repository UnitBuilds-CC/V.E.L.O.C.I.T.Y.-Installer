# CLI Reference

Complete reference for all Velocity CLI commands and options.

## Global Options

| Option | Description |
|--------|-------------|
| `--help`, `-h` | Show help information |
| `--version`, `-V` | Show version |
| `--verbose`, `-v` | Enable verbose output |
| `--quiet`, `-q` | Suppress non-essential output |

---

## `velocity init`

Scaffold a new installer project.

### Usage

```bash
velocity init [OPTIONS] [NAME]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `NAME` | Project name (optional, defaults to current directory) |

### Options

| Option | Description |
|--------|-------------|
| `--template <TEMPLATE>` | Use a specific template |
| `--force` | Overwrite existing files |

### Examples

```bash
# Create new project
velocity init my-app

# Create in current directory
velocity init

# Force overwrite
velocity init my-app --force
```

---

## `velocity build`

Build the installer.

### Usage

```bash
velocity build [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--mode <MODE>` | Build mode: `bundled`, `fetch`, `hybrid` (default: `bundled`) |
| `--package-format <FORMAT>` | Output format: `exe`, `msi` (default: `exe`) |
| `--output <PATH>` | Output directory (default: `./output`) |
| `--compression <LEVEL>` | Compression level 1-22 (default: 9) |
| `--delta` | Generate delta update package |
| `--password <PASSWORD>` | Encrypt payload with password |
| `--icon <PATH>` | Custom installer icon |
| `--silent` | Build without UI prompts |

### Examples

```bash
# Basic build
velocity build

# Cloud-fetch installer
velocity build --mode fetch

# MSI package
velocity build --package-format msi

# With delta updates
velocity build --delta

# Encrypted payload
velocity build --password "my-secret-password"

# Custom output directory
velocity build --output releases/

# High compression
velocity build --compression 15
```

---

## `velocity detect`

Auto-detect project settings from `Cargo.toml`, `package.json`, etc.

### Usage

```bash
velocity detect [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--write` | Write detected settings to `velocity.toml` |
| `--format <FORMAT>` | Output format: `toml`, `json` (default: `toml`) |

### Examples

```bash
# Show detected settings
velocity detect

# Write to velocity.toml
velocity detect --write

# Output as JSON
velocity detect --format json
```

---

## `velocity check`

Deep validation of `velocity.toml`.

### Usage

```bash
velocity check [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--strict` | Treat warnings as errors |
| `--fix` | Auto-fix common issues |

### Examples

```bash
# Validate configuration
velocity check

# Strict mode
velocity check --strict

# Auto-fix issues
velocity check --fix
```

---

## `velocity info`

Show installer package info.

### Usage

```bash
velocity info <PATH>
```

### Arguments

| Argument | Description |
|----------|-------------|
| `PATH` | Path to installer `.exe` or `.msi` |

### Examples

```bash
# Show EXE info
velocity info output/MyApp_Setup.exe

# Show MSI info
velocity info output/MyApp_Setup.msi
```

---

## `velocity sign`

Code-sign the installer.

### Usage

```bash
velocity sign <PATH> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `PATH` | Path to installer to sign |

### Options

| Option | Description |
|--------|-------------|
| `--cert <PATH>` | Certificate file (`.pfx` or `.p12`) |
| `--password <PASSWORD>` | Certificate password |
| `--fingerprint <HASH>` | Certificate fingerprint |
| `--subject <NAME>` | Certificate subject name |
| `--timestamp <URL>` | Timestamp server URL |
| `--description <TEXT>` | Signature description |
| `--verify` | Verify existing signature |

### Examples

```bash
# Sign with certificate file
velocity sign output/MyApp_Setup.exe --cert my_cert.pfx

# Sign with fingerprint (hardware token)
velocity sign output/MyApp_Setup.exe --fingerprint "AB:CD:EF:01:23:45:67:89:..."

# Sign with timestamp
velocity sign output/MyApp_Setup.exe --cert cert.pfx --timestamp http://timestamp.digicert.com

# Verify signature
velocity sign --verify output/MyApp_Setup.exe
```

---

## `velocity dep`

Dependency management commands.

### `velocity dep list`

List configured dependencies.

```bash
velocity dep list
```

### `velocity dep add`

Add a new dependency.

```bash
velocity dep add [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--name <NAME>` | Dependency name |
| `--url <URL>` | Download URL |
| `--sha256 <HASH>` | Expected SHA256 hash |
| `--args <ARGS>` | Install arguments |
| `--condition <COND>` | Install condition |
| `--required` | Mark as required |
| `--priority <NUM>` | Installation priority |

**Example:**

```bash
velocity dep add \
  --name "VC++ 2022 Redistributable" \
  --url "https://aka.ms/vs/17/release/vc_redist.x64.exe" \
  --args "/install /quiet /norestart" \
  --condition "not_installed:Microsoft Visual C++ 2022" \
  --required
```

### `velocity dep resolve`

Check which dependencies need installation.

```bash
velocity dep resolve
```

### `velocity dep remove`

Remove a dependency.

```bash
velocity dep remove <NAME>
```

---

## `velocity version`

Show version info.

### Usage

```bash
velocity version
```

---

## Exit Codes

| Code | Description |
|------|-------------|
| `0` | Success |
| `1` | General error |
| `2` | Invalid arguments |
| `3` | Configuration error |
| `4` | Build error |
| `5` | Signing error |

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `VELOCITY_SIGN_CERT_PASSWORD` | Certificate password for signing |
| `VELOCITY_CACHE_DIR` | Override cache directory |
| `VELOCITY_LOG_LEVEL` | Log level: `error`, `warn`, `info`, `debug`, `trace` |

---

## Examples

### Complete Build Workflow

```bash
# 1. Initialize project
velocity init my-app
cd my-app

# 2. Auto-detect settings
velocity detect --write

# 3. Validate configuration
velocity check

# 4. Build installer
velocity build

# 5. Sign installer
velocity sign output/MyApp_Setup.exe --cert cert.pfx --timestamp http://timestamp.digicert.com

# 6. Verify signature
velocity sign --verify output/MyApp_Setup.exe

# 7. Show info
velocity info output/MyApp_Setup.exe
```

### Cloud-Fetch Build

```bash
# Build tiny bootstrapper
velocity build --mode fetch

# Output: output/MyApp_Setup.exe (1.5 MB)
```

### MSI Build

```bash
# Build MSI package
velocity build --package-format msi

# Output: output/MyApp_Setup.msi
```

### Delta Update Build

```bash
# Build with delta
velocity build --delta

# Output:
# - output/MyApp_Setup.exe (full installer)
# - output/MyApp_Setup-delta.zip (delta package)
```

---

## Further Reading

- [[Getting-Started]] — Installation and quick start
- [[Configuration-Reference]] — Complete `velocity.toml` reference
- [[Cloud-Fetch-Installers]] — Cloud-fetch bootstrapper installers
