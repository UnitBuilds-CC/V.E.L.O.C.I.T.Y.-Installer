# Architecture Overview

<cite>
**Referenced Files in This Document**
- [Cargo.toml](file://Cargo.toml)
- [crates/velocity-core/src/lib.rs](file://crates/velocity-core/src/lib.rs)
- [crates/velocity-config/src/lib.rs](file://crates/velocity-config/src/lib.rs)
- [crates/velocity-compiler/src/lib.rs](file://crates/velocity-compiler/src/lib.rs)
- [crates/velocity-runtime/src/main.rs](file://crates/velocity-runtime/src/main.rs)
- [crates/velocity-ui/src/lib.rs](file://crates/velocity-ui/src/lib.rs)
- [crates/velocity-plugin-api/src/lib.rs](file://crates/velocity-plugin-api/src/lib.rs)
</cite>

## Table of Contents
1. [System Architecture](#system-architecture)
2. [Crate Responsibilities](#crate-responsibilities)
3. [Build Pipeline](#build-pipeline)
4. [Installation Execution Flow](#installation-execution-flow)
5. [UI Architecture](#ui-architecture)
6. [Security Architecture](#security-architecture)
7. [Plugin System Architecture](#plugin-system-architecture)
8. [Dependency Management Pipeline](#dependency-management-pipeline)
9. [Rollback and Error Recovery](#rollback-and-error-recovery)
10. [Data Flow](#data-flow)

## System Architecture

Velocity Installer is a 7-crate Rust workspace that produces standalone `.exe` installers from TOML configuration. The system is organized into three layers: build-time, runtime, and extensibility.

```mermaid
graph TB
    subgraph "Build Time"
        A[velocity-cli<br/>CLI Tool] --> B[velocity-config<br/>TOML Parser]
        A --> C[velocity-compiler<br/>Build Pipeline]
        B --> C
        C --> D[Standalone .exe<br/>config + payload + runtime]
    end

    subgraph "Runtime"
        D --> E[velocity-runtime<br/>Embedded Runtime]
        E --> F[velocity-core<br/>Core Engine]
        E --> G[velocity-ui<br/>Wizard UI]
        F --> H[Windows System]
    end

    subgraph "Extensibility"
        I[velocity-plugin-api<br/>WASM Plugins] --> F
        J[.wasm + plugin.json] --> I
    end

    subgraph "Core Engine Modules"
        F --> K[extract, registry, shortcuts]
        F --> L[services, env_vars, file_assoc]
        F --> M[rollback, security, encryption]
        F --> N[downloader, dep_resolver, dep_installer]
        F --> O[localization, scripting, updater]
    end
```

**Key Design Principles:**
- **Configuration-driven** — All installer behavior defined in `velocity.toml`
- **Standalone output** — Each installer is a single self-contained `.exe`
- **Automatic rollback** — All changes tracked and undone on failure
- **Security-first** — AES-256-GCM encryption, CSPRNG, path traversal protection
- **Dual UI** — Modern WebView2 or Classic Win32, selectable per package
- **Extensible** — WASM plugin system with sandboxed execution

## Crate Responsibilities

### velocity-cli (CLI Layer)
The user-facing command-line interface built with `clap`.

```mermaid
graph LR
    A[velocity init] --> B[Scaffold project]
    C[velocity build] --> D[Compile installer]
    E[velocity detect] --> F[Auto-detect settings]
    G[velocity check] --> H[Validate config]
    I[velocity sign] --> J[Code signing]
    K[velocity dep *] --> L[Dependency management]
```

**Dependencies:** velocity-config, velocity-core, velocity-compiler, clap

### velocity-core (Engine Layer)
The heart of the installer with 37 modules covering all installation operations.

```mermaid
graph TB
    subgraph "File Operations"
        A[extract<br/>zstd/LZMA decompression]
        B[payload<br/>Archive format]
        C[pe_icon<br/>PE icon editing]
    end

    subgraph "Windows Integration"
        D[registry<br/>HKLM/HKCU/HKCR/HKU]
        E[shortcuts<br/>IShellLink COM]
        F[services<br/>Service management]
        G[env_vars<br/>System/User env vars]
        H[file_assoc<br/>File type registration]
    end

    subgraph "Safety"
        I[rollback<br/>Automatic change tracking]
        J[security<br/>Path validation, CSPRNG]
        K[encryption<br/>AES-256-GCM]
        L[checksum<br/>SHA256 verification]
    end

    subgraph "Network"
        M[downloader<br/>HTTP with resume]
        N[dep_resolver<br/>Condition evaluation]
        O[dep_installer<br/>Silent installation]
        P[updater<br/>Self-update check]
    end

    subgraph "UX"
        Q[localization<br/>i18n string tables]
        R[scripting<br/>Structured actions]
        S[component_tree<br/>Selection hierarchy]
        T[logging<br/>Timestamped log files]
    end

    subgraph "Platform"
        U[platform<br/>Windows/Linux/macOS]
        V[arch_detect<br/>x86/x64 detection]
        W[elevation<br/>UAC handling]
        X[process_detect<br/>Running app detection]
    end
```

### velocity-config (Configuration Layer)
Handles TOML parsing, path variable resolution, and auto-generation.

```mermaid
graph LR
    A[velocity.toml] --> B[parser.rs<br/>TOML deserialization]
    B --> C[manifest.rs<br/>Struct definitions]
    C --> D[variables.rs<br/>Path variable resolution]
    E[auto_gen.rs<br/>Auto-detect from project] --> B
```

**Path Variables:**
| Variable | Resolves To |
|----------|-------------|
| `{app}` | Installation directory |
| `{autopf}` | Program Files (arch-aware) |
| `{win}` | Windows directory |
| `{sys}` | System32 directory |
| `{tmp}` | Temp directory |
| `{home}` | User home directory |
| `{desktop}` | Desktop folder |
| `{programs}` | Start Menu Programs |

### velocity-compiler (Build Layer)
Compiles configuration and payload into a standalone `.exe`.

```mermaid
graph LR
    A[velocity.toml] --> C[Compiler]
    B[Application Files] --> D[zstd Compression]
    D --> E[Payload Archive]
    C --> F[Embed config + payload + runtime]
    E --> F
    F --> G[Standalone installer.exe]
```

### velocity-runtime (Execution Layer)
Lightweight binary embedded in each installer `.exe`. Executes when the user runs the installer.

```mermaid
graph LR
    A[installer.exe launched] --> B[velocity-runtime]
    B --> C[Parse embedded config]
    C --> D[Show wizard UI]
    D --> E[Execute installation]
    E --> F[velocity-core operations]
```

### velocity-ui (Presentation Layer)
Dual-theme wizard with progress tracking and ETA.

```mermaid
graph TB
    subgraph "Classic (Win32)"
        A[classic.rs<br/>Native Win32 dialogs]
    end

    subgraph "Modern (WebView2)"
        B[modern.rs<br/>HTML/CSS/JS frontend]
        C[wizard_html.rs<br/>Embedded HTML templates]
    end

    subgraph "Cross-Platform"
        D[wry_wizard.rs<br/>wry + tao backend]
    end

    subgraph "Shared"
        E[wizard.rs<br/>Wizard flow logic]
        F[progress_dialog.rs<br/>Progress + ETA]
    end

    A --> E
    B --> E
    D --> E
    E --> F
```

## Build Pipeline

### Compilation Flow

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Config
    participant Compiler
    participant Core

    User->>CLI: velocity build
    CLI->>Config: Parse velocity.toml
    Config-->>CLI: Manifest + resolved paths
    CLI->>CLI: Validate configuration
    CLI->>Compiler: Build installer
    Compiler->>Core: Collect source files
    Core-->>Compiler: File list
    Compiler->>Compiler: Compress with zstd
    Compiler->>Compiler: Create payload archive
    Compiler->>Compiler: Embed config + payload + runtime
    Compiler-->>CLI: output/installer.exe
    CLI-->>User: Build complete
```

### Release Profile
```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true            # Link-time optimization
codegen-units = 1     # Single codegen unit
strip = true          # Strip debug symbols
```

## Installation Execution Flow

### Runtime Execution

```mermaid
sequenceDiagram
    participant User
    participant Runtime
    participant UI
    participant Core
    participant Plugins
    participant Windows

    User->>Runtime: Launch installer.exe
    Runtime->>Runtime: Extract embedded config + payload
    Runtime->>UI: Show wizard
    UI->>User: Welcome, License, Directory, Components

    User->>UI: Confirm installation
    UI->>Core: Begin installation

    Core->>Core: Check disk space
    Core->>Core: Check admin elevation
    Core->>Plugins: on_pre_install
    Core->>Core: Execute pre-install scripts
    Core->>Core: Install dependencies
    Core->>Core: Extract files (track for rollback)
    Core->>Plugins: on_file_extracted
    Core->>Core: Create registry entries
    Core->>Core: Create shortcuts
    Core->>Core: Set environment variables
    Core->>Core: Register services
    Core->>Core: Register file associations
    Core->>Core: Execute post-install scripts
    Core->>Plugins: on_post_install
    Core->>Core: Register Add/Remove Programs
    Core->>Core: Write install log

    Core-->>UI: Installation complete
    UI->>User: Finish page (run app, etc.)
```

### Rollback on Failure

```mermaid
sequenceDiagram
    participant Core
    participant Rollback
    participant Windows

    Core->>Core: Error during installation
    Core->>Rollback: Trigger rollback
    Rollback->>Windows: Remove extracted files (reverse order)
    Rollback->>Windows: Delete registry entries
    Rollback->>Windows: Remove shortcuts
    Rollback->>Windows: Remove env variables
    Rollback->>Windows: Stop/remove services
    Rollback->>Windows: Unregister file associations
    Rollback-->>Core: Rollback complete
    Core-->>Core: Report failure to user
```

## UI Architecture

### Classic Wizard (Win32)
```mermaid
graph TB
    A[Welcome Page] --> B[License Agreement]
    B --> C[Directory Selection]
    C --> D[Component Selection]
    D --> E[Progress + ETA]
    E --> F[Finish Page]
```

- Uses Win32 API via `windows` crate
- Native look and feel
- Windows-only

### Modern Wizard (WebView2)
```mermaid
graph TB
    A[HTML Template<br/>wizard_html.rs] --> B[WebView2<br/>modern.rs]
    B --> C[JS↔Rust RPC]
    C --> D[Dark/Light Theme]
    C --> E[CSS Animations]
    C --> F[Progress + ETA]
```

- Uses `webview2-com` crate
- Contemporary design with CSS animations
- Bidirectional JS↔Rust communication
- Windows-only (WebView2 required)

### Cross-Platform Wizard
```mermaid
graph TB
    A[wry_wizard.rs] --> B{Platform?}
    B -->|Windows| C[WebView2]
    B -->|Linux| D[webkit2gtk]
    B -->|macOS| E[WebKit]
    A --> F[tao window management]
```

- Uses `wry` + `tao` crates
- Linux requires GTK + webkit2gtk
- macOS uses native WebKit

## Security Architecture

### Encryption Pipeline

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Encryption
    participant Core

    User->>CLI: velocity build --encrypt
    CLI->>User: Ask for password
    User->>CLI: Enter password
    CLI->>Encryption: Derive key (PBKDF2)
    Note over Encryption: 600,000 iterations<br/>HMAC-SHA256<br/>CSPRNG salt (16 bytes)
    Encryption->>Encryption: Generate nonce (CSPRNG)
    Encryption->>Core: Encrypt payload (AES-256-GCM)
    Core-->>CLI: Encrypted payload
    CLI->>CLI: Embed in installer.exe
```

### Security Layers

```mermaid
graph TB
    subgraph "Cryptography"
        A[AES-256-GCM<br/>Authenticated encryption]
        B[PBKDF2-HMAC-SHA256<br/>600k iterations key derivation]
        C[BCryptGenRandom<br/>Windows CSPRNG]
    end

    subgraph "Path Safety"
        D[Path traversal protection<br/>Zip-slip prevention]
        E[Install dir validation<br/>Reject system directories]
        F[Null byte rejection<br/>Block injection attacks]
        G[Absolute path rejection<br/>Relative-only archives]
    end

    subgraph "Runtime Safety"
        H[Shell injection protection<br/>URL validation]
        I[Password length limit<br/>1024-char max]
        J[Secure temp dirs<br/>Per-session isolation]
        K[File backup<br/>.velocity_backup before overwrite]
    end
```

### Rejected Directories
The installer rejects these as installation targets:
- `C:\Windows`
- `C:\Windows\System32`
- `C:\ProgramData`
- Drive roots (`C:\`)
- Paths containing null bytes

## Plugin System Architecture

### WASM Plugin Pipeline

```mermaid
graph TB
    subgraph "Plugin Development"
        A[Rust/WAT source] --> B[Compile to WASM]
        B --> C[plugin.wasm]
        D[plugin.json] --> E[Manifest]
    end

    subgraph "Plugin Loading"
        C --> F[Wasmtime Loader]
        E --> F
        F --> G[Validate manifest]
        G --> H[Instantiate WASM module]
        H --> I[Link Host API]
    end

    subgraph "Execution"
        I --> J[Sandboxed WASM Runtime]
        J --> K[Host API calls]
        K --> L[velocity-core operations]
    end
```

### Lifecycle Hooks
```mermaid
graph LR
    A[on_load] --> B[on_pre_install]
    B --> C[on_file_extracted]
    C --> D[on_post_install]
    D --> E[on_shutdown]

    B -.->|on error| F[on_error]
    B -.->|on cancel| G[on_cancel]
    D -.->|uninstall| H[on_pre_uninstall]
    H --> I[on_post_uninstall]
```

### Host API
Plugins can call these host functions:
- `log(message)` — Write to installer log
- `read_file(path)` — Read file contents
- `write_file(path, data)` — Write file
- `exec_command(cmd, args)` — Execute system command
- `registry_read(key)` — Read registry value
- `registry_write(key, value)` — Write registry value
- `set_progress(percent)` — Update progress bar
- `get_variable(name)` — Read path variable

## Dependency Management Pipeline

### Condition Resolution

```mermaid
graph TB
    A[Dependency defined in velocity.toml] --> B{Evaluate condition}
    B -->|always| C[Install]
    B -->|never| D[Skip]
    B -->|registry_missing:KEY| E{Key exists?}
    B -->|not_installed:NAME| F{In Add/Remove Programs?}
    B -->|file_exists:PATH| G{File exists?}
    B -->|arch:x64| H{CPU is x64?}
    B -->|os_version:>=10.0| I{OS version check}

    E -->|No| C
    E -->|Yes| D
    F -->|No| C
    F -->|Yes| D
    G -->|No| C
    G -->|Yes| D
```

### Download and Install Flow

```mermaid
sequenceDiagram
    participant Core
    participant Downloader
    participant Condition
    participant Installer
    participant Windows

    Core->>Condition: Evaluate condition
    Condition-->>Core: Install needed

    Core->>Downloader: Download file
    Downloader->>Downloader: HTTP GET with Range support
    Downloader->>Downloader: Verify SHA256
    Downloader-->>Core: Downloaded file

    Core->>Installer: Execute installer
    Installer->>Windows: Run with install_args (silent)
    Windows-->>Installer: Exit code
    Installer-->>Core: Success/Failure
```

## Rollback and Error Recovery

### Transaction Tracking

```mermaid
graph TB
    subgraph "Tracked Operations"
        A[File extracted] --> B[Registry written]
        B --> C[Shortcut created]
        C --> D[Env var set]
        D --> E[Service installed]
        E --> F[File association registered]
    end

    subgraph "Rollback Stack"
        G[Operation 6: Remove file assoc]
        H[Operation 5: Remove service]
        I[Operation 4: Remove env var]
        J[Operation 3: Delete shortcut]
        K[Operation 2: Delete registry]
        L[Operation 1: Delete file]
    end

    F --> M[Error occurs!]
    M --> G
    G --> H
    H --> I
    I --> J
    J --> K
    K --> L
```

### Rollback Guarantees
- All file extractions are tracked and reversible
- Registry entries are deleted in reverse order
- Shortcuts are removed
- Environment variables are unset
- Services are stopped and removed
- File associations are unregistered
- Stress tested with 1000+ operations

## Data Flow

### Installer Build Data Flow

```mermaid
graph LR
    A[velocity.toml] --> B[Config Parser]
    C[Source Files] --> D[zstd Compressor]
    B --> E[Compiler]
    D --> F[Payload Archive]
    F --> E
    G[velocity-runtime] --> E
    E --> H[Standalone installer.exe]
```

### Installation Data Flow

```mermaid
graph LR
    A[installer.exe] --> B[Self-extract]
    B --> C[Embedded Config]
    B --> D[Compressed Payload]
    C --> E[Runtime]
    D --> F[zstd Decompress]
    F --> G[Extract Files]
    E --> H[Execute Installation]
    G --> H
    H --> I[Registry + Shortcuts + Services]
```

### Self-Update Data Flow

```mermaid
sequenceDiagram
    participant Runtime
    participant Updater
    participant Server

    Runtime->>Updater: Check for updates
    Updater->>Server: GET update-check.json
    Server-->>Updater: { version, url, sha256 }
    Updater->>Updater: Compare versions
    alt New version available
        Updater-->>Runtime: Update available
        Runtime-->>Runtime: Notify user
        Runtime->>Runtime: Open download URL
    else Up to date
        Updater-->>Runtime: No update needed
    end
```

**Section sources**
- [Cargo.toml](file://Cargo.toml)
- [crates/velocity-core/src/lib.rs](file://crates/velocity-core/src/lib.rs)
- [crates/velocity-config/src/lib.rs](file://crates/velocity-config/src/lib.rs)
- [crates/velocity-compiler/src/lib.rs](file://crates/velocity-compiler/src/lib.rs)
- [crates/velocity-runtime/src/main.rs](file://crates/velocity-runtime/src/main.rs)
- [crates/velocity-ui/src/lib.rs](file://crates/velocity-ui/src/lib.rs)
- [crates/velocity-plugin-api/src/lib.rs](file://crates/velocity-plugin-api/src/lib.rs)
