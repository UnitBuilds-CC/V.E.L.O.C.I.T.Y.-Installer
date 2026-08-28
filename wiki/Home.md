# Velocity Installer

Welcome to the Velocity Installer wiki — your comprehensive guide to building professional installers with Velocity.

## What is Velocity?

Velocity is a **free, open-source, universal installer framework** built in Rust. It produces standalone `.exe` installers from a simple TOML configuration, with modern or classic wizard UI. No commercial licensing required — fully free under MIT/Apache-2.0.

## Key Features at a Glance

| Category | Highlights |
|----------|------------|
| **Dual Output** | Standalone `.exe` installers + `.msi` packages for enterprise |
| **Cloud-Fetch** | Tiny bootstrapper installers that download at install time |
| **Compression** | zstd, LZMA2, ZIP, tar.gz, tar.xz, tar.bz2 support |
| **Delta Updates** | Binary diff (bsdiff) — 80-95% smaller update downloads |
| **Security** | AES-256-GCM encryption, PBKDF2 key derivation, CSPRNG, path traversal protection |
| **Cross-Platform** | Windows (full), Linux (core), macOS (core) |
| **Plugin System** | WASM-based sandboxed plugins with 9 lifecycle hooks |
| **Enterprise** | MSI packages, Group Policy, SCCM/Intune deployment |
| **Silent Mode** | Inno Setup compatible `/S` and `/D=path` flags |
| **Localization** | Built-in i18n with per-language string overrides |

## Wiki Pages

### Getting Started
- [[Getting-Started]] — Installation, quick start, building your first installer
- [[Configuration-Reference]] — Complete `velocity.toml` reference
- [[CLI-Reference]] — All CLI commands and options

### Features
- [[Cloud-Fetch-Installers]] — Tiny bootstrapper installers from Git releases
- [[Cross-Platform-Support]] — Platform coverage and Unix installer execution
- [[Delta-Updates]] — Binary delta patching for efficient updates
- [[Plugin-System]] — WASM-based plugin architecture
- [[MSI-Enterprise]] — MSI packages for enterprise deployment
- [[Code-Signing]] — Authenticode and platform-specific signing

### Technical
- [[Architecture]] — System design, crate structure, data flow
- [[Security]] — Encryption, audit reports, threat model
- [[Contributing]] — Development setup, coding standards, PR guidelines

## Quick Links

- [GitHub Repository](https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer)
- [Releases](https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer/releases)
- [Issues](https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer/issues)
- [Discussions](https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer/discussions)

## Comparison with Alternatives

| Feature | Velocity | Inno Setup | NSIS | WiX | Velopack |
|---------|----------|------------|------|-----|----------|
| Open Source | MIT/Apache | Inno Setup License | zlib | MS-RL | MIT |
| Config Format | TOML | Pascal Script | NSIS Script | XML | C# Code |
| Compression | zstd | LZMA | LZMA | MSI/CAB | LZMA |
| Delta Updates | Yes (bsdiff) | No | No | No | Yes |
| MSI Compliance | Yes | No | No | Yes | No |
| Cross-Platform | Yes | No | No | No | Yes |
| Dependency Management | Built-in | Manual | Manual | Manual | No |
| Localization | Built-in i18n | Language files | Language strings | Transform | No |
| Silent Mode | Inno-compatible | Yes | Yes | Yes | No |
| Rollback | Automatic | Yes | Yes | Yes (MSI) | No |
| Plugin System | WASM-ready | Pascal | NSIS Script | No | No |
| AES-256-GCM Encryption | Yes | No | No | No | No |
| Written In | Rust | Delphi | C++ | C# | C# |

## License

Licensed under either of:
- [MIT License](https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer/blob/main/LICENSE-MIT)
- [Apache License, Version 2.0](https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer/blob/main/LICENSE-APACHE)

at your option.
