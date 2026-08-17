# Velocity Installer

A free, open-source, universal Windows installer framework built in Rust.

Velocity produces standalone `.exe` installers from a simple TOML configuration, with a choice of modern or classic wizard UI. No commercial licensing required — fully free under MIT/Apache-2.0.

## Features

- **Zero-allocation core** — Built in Rust for maximum performance and minimal binary size
- **Dual UI themes** — Modern (WebView2) or Classic (Win32) wizard, selectable per package
- **Auto-generated config** — Minimal manual setup; the CLI detects your project structure
- **Universal** — Handle any installation scenario: files, registry, shortcuts, services, env vars
- **Built-in auto-update** — Delta updates with zstd compression
- **WASM plugins** — Sandboxed custom actions via WebAssembly
- **Open source** — MIT or Apache-2.0, no commercial restrictions

## Quick Start

```bash
# Install the CLI
cargo install velocity-cli

# Create a new installer project
velocity init my-app

# Build the installer
cd my-app
velocity build
```

## Configuration

Create a `velocity.toml` in your project root:

```toml
[app]
name = "My Application"
version = "1.0.0"
publisher = "My Company"

[install]
default_dir = "{autopf}/MyApp"

[files]
source = "./build-output/**"

[shortcuts]
desktop = true
start_menu = true

[ui]
theme = "modern"
```

## Architecture

```
velocity/
├── crates/
│   ├── velocity-cli/          # CLI tool: scaffold, build, sign, test
│   ├── velocity-core/         # Engine: extract, registry, shortcuts, services
│   ├── velocity-config/       # Config parser + auto-generator
│   ├── velocity-ui/           # Installer wizard UI (modern + classic)
│   ├── velocity-compiler/     # Compiles config+payload into standalone .exe
│   ├── velocity-runtime/      # Lightweight runtime embedded in each installer
│   └── velocity-plugin-api/   # Plugin trait + SDK for custom actions
├── themes/
│   ├── modern/                # WebView2-based modern UI
│   └── classic/               # Native Win32 wizard UI
└── templates/
    └── default/               # Scaffold template for `velocity init`
```

## Building from Source

```bash
git clone https://github.com/UnitBuilds-CC/V.E.L.O.C.I.T.Y.-Installer.git
cd V.E.L.O.C.I.T.Y.-Installer
cargo build --release
```

## License

Licensed under either of:
- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.
