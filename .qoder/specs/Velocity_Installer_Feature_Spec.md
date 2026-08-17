# Velocity Installer Feature Specification

**Date:** 2026-08-17
**Status:** Active — Phases 1-3 Complete, Phase 4 Pending
**Version:** 0.1.0

---

## Executive Summary

Velocity Installer is a free, open-source, universal Windows installer framework built in Rust. It produces standalone `.exe` installers from TOML configuration with modern or classic wizard UI. This document specifies all implemented and planned features.

---

## 1. Core Engine

### 1.1 File Extraction
- **Compression:** zstd (primary), LZMA (fallback)
- **Archive format:** Custom tar-based payload with zstd compression
- **Path resolution:** All paths resolved through `{variable}` system
- **Overwrite modes:** always, skip, prompt, newer-only
- **Progress tracking:** Per-file progress with overall ETA

### 1.2 Windows Registry
- **Root keys:** HKLM, HKCU, HKCR, HKU
- **Value types:** REG_SZ, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ
- **Variable substitution:** Registry values support `{app}`, `{version}`, etc.
- **Rollback:** All registry changes tracked and reversible

### 1.3 Shortcuts
- **Locations:** Desktop, Start Menu, Quick Launch, custom paths
- **Implementation:** IShellLink COM interface
- **Properties:** Icon, working directory, arguments, hotkey

### 1.4 Services
- **Operations:** Install, start, stop, remove
- **Start types:** auto, manual, disabled, delayed-auto
- **Dependencies:** Service dependency support
- **Rollback:** Service state restored on failure

### 1.5 Environment Variables
- **Scope:** System and User
- **Broadcast:** WM_SETTINGCHANGE sent after modification
- **Rollback:** Old values preserved and restored

### 1.6 File Associations
- **Registration:** Extension → handler mapping
- **Icon support:** Custom icons per association
- **Open command:** Configurable handler command

### 1.7 Add/Remove Programs
- **Automatic registration:** Creates uninstall entry
- **Metadata:** Display name, version, publisher, estimated size
- **URLs:** Help URL, update URL
- **Uninstall command:** Self-contained uninstaller

---

## 2. UI Wizard

### 2.1 Classic Wizard (Win32)
- **Pages:** Welcome, License, Directory, Components, Progress, Finish
- **Native:** Uses Win32 API for authentic Windows look
- **Progress:** Real-time progress bar with ETA calculation
- **Platform:** Windows only

### 2.2 Modern Wizard (WebView2)
- **Technology:** WebView2 (Edge Chromium)
- **Themes:** Dark and Light
- **Animations:** CSS transitions and animations
- **RPC:** Bidirectional JS↔Rust communication
- **Responsive:** Adapts to window size
- **Platform:** Windows only (WebView2 required)

### 2.3 Cross-Platform Wizard
- **Technology:** wry + tao
- **Backend:** webkit2gtk (Linux), WebKit (macOS)
- **Fallback:** For non-Windows builds

### 2.4 Silent Mode
- **Flags:** `/S`, `/D=path`, `--silent`, `--force`
- **Compatibility:** Inno Setup compatible syntax
- **Logging:** Full log output to file

---

## 3. Configuration System

### 3.1 TOML Format
- **File:** `velocity.toml` in project root
- **Sections:** app, install, files, components, dependencies, registry, env_vars, services, file_associations, scripts, localization, uninstall, ui
- **Validation:** Deep validation via `velocity check`

### 3.2 Path Variables
- **Built-in:** `{app}`, `{autopf}`, `{win}`, `{sys}`, `{tmp}`, `{home}`, `{desktop}`, `{programs}`, `{sendto}`, `{startup}`
- **Dynamic:** `{install_dir}`, `{app_name}`, `{version}`
- **Resolution:** Runtime resolution with context

### 3.3 Auto-Generation
- **Detection:** Scans Cargo.toml, package.json for project metadata
- **Scaffolding:** `velocity init` creates project structure
- **Smart defaults:** Sensible defaults based on project type

---

## 4. Dependency Management

### 4.1 Remote Dependencies
- **Download:** HTTP with Range request support (resume)
- **Verification:** SHA256 integrity checking
- **Installation:** Silent install with configurable arguments
- **Ordering:** Priority-based installation order

### 4.2 Condition System
| Condition | Description |
|-----------|-------------|
| `always` | Always install |
| `never` | Never install |
| `registry_missing:KEY` | Install if registry key missing |
| `registry_exists:KEY` | Install if registry key exists |
| `file_missing:PATH` | Install if file missing |
| `file_exists:PATH` | Install if file exists |
| `not_installed:NAME` | Not in Add/Remove Programs |
| `installed:NAME` | In Add/Remove Programs |
| `arch:x64` / `arch:x86` | Architecture check |
| `os_version:>=10.0` | OS version check |

### 4.3 Bundled Apps
- **Third-party:** Include Notepad++, VLC, 7-Zip installers
- **Silent install:** Configurable silent arguments
- **Conditional:** Install only when needed

---

## 5. Security

### 5.1 Encryption
- **Algorithm:** AES-256-GCM (authenticated encryption)
- **Key derivation:** PBKDF2-HMAC-SHA256, 600,000 iterations
- **Random:** BCryptGenRandom (Windows CSPRNG)
- **Salt:** 16-byte CSPRNG salt per encryption
- **Nonce:** 12-byte CSPRNG nonce per encryption
- **Password limit:** 1024 characters max

### 5.2 Path Safety
- **Traversal protection:** Blocks `../` sequences in archives
- **Absolute path rejection:** Archive entries must be relative
- **Null byte rejection:** Blocks null byte injection
- **Directory validation:** Rejects system directories (Windows, System32, ProgramData)
- **Drive root rejection:** Cannot install to drive roots

### 5.3 Runtime Safety
- **Shell injection:** URL validation before shell commands
- **Secure temp:** Per-session temp directories
- **File backup:** `.velocity_backup` before overwrite
- **Crash reporting:** Backtrace to `%TEMP%/velocity_crashes/`

---

## 6. Localization (i18n)

### 6.1 Built-in English
- Complete default string table for all UI elements

### 6.2 Multi-Language
- Define additional languages in `velocity.toml`
- Per-language string overrides
- Variable substitution in strings (`{app_name}`, `{version}`)

### 6.3 Language Detection
- Uses system locale by default
- User can override in wizard

---

## 7. Plugin System

### 7.1 WASM Runtime
- **Engine:** Wasmtime (sandboxed execution)
- **Safety:** No direct system access
- **Format:** `.wasm` compiled modules

### 7.2 Lifecycle Hooks
1. `on_load` — Plugin initialization
2. `on_pre_install` — Before installation
3. `on_file_extracted` — After each file
4. `on_post_install` — After installation
5. `on_error` — On error
6. `on_cancel` — On user cancel
7. `on_pre_uninstall` — Before uninstall
8. `on_post_uninstall` — After uninstall
9. `on_shutdown` — On shutdown

### 7.3 Host API
- Logging, file I/O, command execution, registry access, progress updates
- Permission-based: declared in plugin.json manifest

---

## 8. Build Pipeline

### 8.1 Compilation
- **Input:** velocity.toml + source files
- **Compression:** zstd compression of all source files
- **Output:** Standalone `.exe` with embedded config + payload + runtime
- **Optimization:** opt-level "z", LTO, codegen-units 1, stripped

### 8.2 Icon Embedding
- **Tool:** rcedit or Resource Hacker
- **Format:** `.ico` files
- **PE metadata:** Company, description, version

### 8.3 Code Signing
- **Tool:** signtool.exe (Windows), codesign (macOS), GPG (Linux)
- **Timestamp:** RFC 3161 timestamping
- **CI integration:** Automatic on tagged releases

---

## 9. Self-Update

### 9.1 Update Check
- **Endpoint:** HTTP JSON endpoint
- **Response:** `{ version, url, sha256 }`
- **Comparison:** Semantic version comparison
- **Notification:** User notified of available update
- **Action:** Opens download URL in browser

---

## 10. Scripting Engine

### 10.1 Shell Scripts
- **Pre-install:** Commands run before installation
- **Post-install:** Commands run after installation
- **Variable substitution:** `{install_dir}`, `{app_name}` in commands

### 10.2 Structured Actions
| Action | Description |
|--------|-------------|
| `shell` | Execute shell command |
| `copy` | Copy file |
| `delete` | Delete file |
| `mkdir` | Create directory |
| `registry` | Registry operation |
| `env_var` | Set environment variable |

### 10.3 Conditions
- `file_exists:PATH` — Check file existence
- `file_missing:PATH` — Check file absence
- `dir_exists:PATH` — Check directory existence
- `action_success` — Check previous action success

### 10.4 Error Policies
- `abort` — Stop on error
- `continue` — Skip and continue
- `retry` — Retry the operation

---

## 11. Component Selection

### 11.1 Component Tree
- **Hierarchy:** Parent-child component groups
- **Display:** Flattened with indentation
- **Selection:** Checkboxes with dependency resolution
- **Disk space:** Per-component size calculation

### 11.2 Component Properties
- `id` — Unique identifier
- `name` — Display name
- `description` — User-facing description
- `selected_by_default` — Initial selection state
- `mandatory` — Cannot be deselected
- `source` — File patterns for this component

---

## 12. Comparison with Competitors

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
| Plugin System | WASM | Pascal | NSIS Script | No |
| Written In | Rust | Delphi | C++ | C# |
| Encryption | AES-256-GCM | None built-in | None built-in | MSI transform |

---

## 13. Roadmap Status

### Completed
- [x] Phase 1: Foundation — Core engine, classic UI, TOML config, compiler, runtime
- [x] Phase 1.5: Robustness — Dependency management, localization, security hardening, component selection, progress tracking
- [x] Phase 6: Hardening — AES-256-GCM encryption, self-update mechanism, component tree view, structured scripting engine, E2E tests
- [x] Phase 7: Quality — Clippy cleanup, E2E integration tests, structured scripting, README updates
- [x] Phase 8: Production Hardening — Stress testing, rollback testing, PBKDF2 key derivation, unsafe safety audit, GitHub Actions CI/CD, crash reporting, code signing docs, fuzz-like parser robustness
- [x] Phase 9: Final Fixes — CSPRNG for encryption (BCryptGenRandom), runtime input validation (install dir, password limits, shell injection protection)
- [x] Phase 10: Beta Test + Ops — Sample installer project, code signing automation, crypto audit, crash telemetry
- [x] Phase 2: Modern UI — WebView2 wizard with dark/light themes, CSS animations, JS↔Rust RPC, `--modern` CLI flag
- [x] Phase 3: WASM Plugins — Plugin trait with 9 lifecycle hooks, Host API, Wasmtime loader, sample plugin, integration tests

### Pending
- [ ] Phase 4: Ecosystem — GUI config editor, template marketplace, delta compression, full auto-update with download-and-swap
