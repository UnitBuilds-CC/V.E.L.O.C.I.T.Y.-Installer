# Velocity Installer .qoder Directory Structure

This directory contains AI-optimized documentation and specifications for the V.E.L.O.C.I.T.Y. Installer project, following the pattern established by the Dwarven Stronghold project.

## Structure

```
.qoder/
├── repowiki/                    # Comprehensive documentation wiki
│   ├── en/                      # English documentation
│   │   ├── content/             # Main documentation pages
│   │   │   ├── Getting Started.md
│   │   │   ├── Development Guide.md
│   │   │   └── Architecture Overview.md
│   │   └── meta/                # Metadata and indexes
│   │       └── repowiki-metadata.json
│   └── knowledge/               # Knowledge cards (patterns, conventions)
│       └── en/
│           ├── TOML Configuration System.md
│           ├── WASM Plugin Architecture.md
│           ├── AES-256-GCM Encryption and Security.md
│           ├── Rollback and Error Recovery.md
│           ├── Structured Scripting Engine.md
│           ├── Condition Evaluation and Dependency Pipeline.md
│           ├── UI Wizard Architecture.md
│           ├── Windows Integration Modules.md
│           └── _index.yaml      # Knowledge card index
└── specs/                       # Feature specifications
    └── Velocity_Installer_Feature_Spec.md
```

## Documentation Pages

### Getting Started.md
Introduction to Velocity Installer, project structure, installation, building, and testing.

**Contents:**
- Project overview and key features
- Directory structure and 7-crate workspace
- Installation and setup instructions
- Building from source
- CLI commands quick reference
- Sample installer project walkthrough
- Testing guide (213+ tests)

### Development Guide.md
Comprehensive guide for developers contributing to Velocity Installer.

**Contents:**
- Development environment setup (Rust toolchain)
- Workspace architecture and crate organization
- Building and testing procedures
- Code style and conventions (Rust)
- Adding new core modules
- Adding new CLI commands
- Configuration system development
- UI development (Classic Win32 + Modern WebView2)
- Plugin development (WASM)
- Cross-platform considerations (Windows primary, Linux/macOS build support)
- CI/CD with GitHub Actions

### Architecture Overview.md
Deep dive into Velocity Installer's system architecture and design decisions.

**Contents:**
- System architecture diagram (7-crate dependency graph)
- Crate responsibilities and boundaries
- Build pipeline (config → compile → runtime)
- Installation execution flow
- UI architecture (Classic vs Modern wizard)
- Security architecture (AES-256-GCM, PBKDF2, CSPRNG)
- Plugin system architecture (WASM + Wasmtime)
- Dependency management pipeline
- Rollback and error recovery system
- Data flow diagrams

## Knowledge Cards

### TOML Configuration System.md
Documents the velocity.toml configuration format and parsing system.

**Key topics:**
- Configuration schema and sections
- Path variable resolution ({app}, {autopf}, {win}, etc.)
- Auto-generation from project structure
- Validation and deep checking
- Manifest structure

### WASM Plugin Architecture.md
Documents the WASM-based plugin system.

**Key topics:**
- Plugin trait with 9 lifecycle hooks
- Host API capabilities
- Wasmtime loader and sandboxing
- plugin.json manifest format
- Safety and isolation guarantees

### AES-256-GCM Encryption and Security.md
Documents the encryption and security hardening system.

**Key topics:**
- AES-256-GCM authenticated encryption
- PBKDF2-HMAC-SHA256 key derivation (600k iterations)
- BCryptGenRandom CSPRNG
- Path traversal protection
- Install directory validation
- Shell injection protection

### Rollback and Error Recovery.md
Documents the automatic rollback system.

**Key topics:**
- Transaction tracking for all changes
- Rollback execution on failure
- File, registry, shortcut, service rollback
- Stress testing (1000+ operations)
- Crash recovery

### Structured Scripting Engine.md
Documents the structured scripting engine for custom install/uninstall actions.

**Key topics:**
- 7 action types (shell, copy, delete, mkdir, delete_dir, registry, env_var)
- Variable substitution ({install_dir}, {app_name}, {version})
- Condition expressions (file_exists, dir_exists, reg_exists, etc.)
- Error policies (Abort, Continue, Retry)
- Config-to-action mapping from TOML

### Condition Evaluation and Dependency Pipeline.md
Documents the condition system and download/dependency pipeline.

**Key topics:**
- 20+ condition types (cross-platform and Windows-only)
- Inno Setup compatibility aliases
- Dual HTTP backends (WinHTTP on Windows, ureq on Unix)
- Resumable downloads with Range requests
- SHA256 checksum verification
- Dependency detection and installation

### UI Wizard Architecture.md
Documents the installation wizard UI system with four backends.

**Key topics:**
- Theme selection (classic, modern, webview)
- Classic Win32 wizard (native dialogs)
- Modern wizard (embedded HTML with CSS animations)
- WebView2 wizard (full Edge Chromium rendering)
- Cross-platform wry+tao wizard (Linux/macOS)
- Atomic progress tracking with ETA calculation
- Component selection with disk space calculation

### Windows Integration Modules.md
Documents six Windows-specific integration modules.

**Key topics:**
- Registry operations (winreg, HKLM/HKCU/HKCR/HKU, 4 value types)
- Shortcut creation (IShellLink COM)
- Windows service management (install, start, stop, remove)
- Environment variable management (system/user scope, WM_SETTINGCHANGE)
- File association registration (HKCR)
- Self-contained uninstaller generation (Add/Remove Programs entry)

## Specifications

### Velocity_Installer_Feature_Spec.md
Comprehensive feature specification covering all installer capabilities.

**Key sections:**
- Core engine features
- UI wizard specification
- Dependency management
- Localization system
- Security hardening requirements
- Build pipeline specification
- Self-update mechanism
- Comparison with competitors (Inno Setup, NSIS, WiX)

## Metadata

### repowiki-metadata.json
YAML-formatted metadata about the project:
- Project information (name, description, repository)
- Crate inventory with descriptions
- Dependency summary
- Build configuration
- Test coverage summary
- CI/CD pipeline details
- Feature roadmap status

## Usage

This documentation is designed to be:
1. **AI-readable** — Structured for AI assistants to understand the codebase
2. **Developer-friendly** — Clear guides for human developers
3. **Comprehensive** — Covers architecture, patterns, and conventions
4. **Maintainable** — Easy to update as the project evolves

### For AI Assistants
When working on Velocity Installer, reference these docs to understand:
- Where to find specific functionality
- What patterns and conventions to follow
- How different crates interact
- What security requirements must be met

### For Developers
Use these docs to:
- Get started with Velocity Installer development
- Understand the 7-crate architecture before making changes
- Follow established patterns and conventions
- Learn about the security and encryption systems
- Plan and implement new features

## Contributing

When adding new features or patterns:
1. Update relevant documentation pages in `repowiki/en/content/`
2. Add knowledge cards for new patterns in `repowiki/knowledge/en/`
3. Create specs for major features in `specs/`
4. Update metadata in `repowiki/en/meta/repowiki-metadata.json`

## License

This documentation is part of the V.E.L.O.C.I.T.Y. Installer project and follows the same license (MIT OR Apache-2.0).
