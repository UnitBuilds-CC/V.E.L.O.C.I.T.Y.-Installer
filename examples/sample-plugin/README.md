# Sample WASM Plugin — Hello World

A minimal Velocity Installer WASM plugin that demonstrates the plugin interface.

## What It Does

Logs a message at each lifecycle hook via the host's `host_log` function:
- `on_load` — "Hello from WASM plugin! on_load called"
- `on_pre_install` — "Hello from WASM plugin! on_pre_install called"
- `on_post_install` — "Hello from WASM plugin! on_post_install called"
- `on_error` — "Hello from WASM plugin! on_error called"
- `on_unload` — "Hello from WASM plugin! on_unload called"

## Project Structure

```
examples/sample-plugin/
├── plugin.json    # Plugin manifest (name, version, events, parameters)
├── plugin.wat     # WebAssembly Text source
└── README.md      # This file
```

## Building

### Option 1: Using `wat2wasm` (from WABT)

```bash
wat2wasm plugin.wat -o plugin.wasm
```

### Option 2: Using Rust

```rust
let wasm = wat::parse_file("plugin.wat").unwrap();
std::fs::write("plugin.wasm", &wasm).unwrap();
```

### Option 3: Using the Velocity CLI

```bash
velocity build-plugin examples/sample-plugin/
```

## Installing

Place the compiled `plugin.wasm` and `plugin.json` in the installer's `plugins/` directory:

```
C:\Program Files\MyApp\plugins\
├── plugin.wasm
└── plugin.json
```

The installer will automatically discover and load plugins at startup.

## Plugin Interface

### Exports (WASM → Host)

| Function | Parameters | Returns | Description |
|---|---|---|---|
| `on_load` | `ctx_ptr: i32, ctx_len: i32` | `i32` (0=ok) | Called when plugin loads |
| `on_pre_install` | `ctx_ptr: i32, ctx_len: i32` | `i32` | Before installation |
| `on_post_install` | `ctx_ptr: i32, ctx_len: i32` | `i32` | After installation |
| `on_error` | `ctx_ptr, ctx_len, err_ptr, err_len` | `i32` | On error |
| `on_unload` | `ctx_ptr: i32, ctx_len: i32` | `i32` | Before unload |
| `memory` | — | — | Exported linear memory |

### Imports (Host → WASM)

| Function | Module | Description |
|---|---|---|
| `host_log(level_ptr, level_len, msg_ptr, msg_len)` | `env` | Log a message |
| `host_file_exists(path_ptr, path_len)` | `env` | Check file existence |
| `host_set_progress(percent, text_ptr, text_len)` | `env` | Update progress bar |
| `host_show_message(title_ptr, title_len, msg_ptr, msg_len)` | `env` | Show UI message |

## Context JSON

The `ctx_ptr`/`ctx_len` parameters point to a JSON object in WASM memory:

```json
{
    "app_name": "MyApp",
    "app_version": "1.0.0",
    "install_dir": "C:\\Program Files\\MyApp",
    "publisher": "Acme Corp",
    "arch": "x64",
    "parameters": { "greeting": "Hello!" },
    "quiet_mode": false,
    "session_id": "abc-123"
}
```
