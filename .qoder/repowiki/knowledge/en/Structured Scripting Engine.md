---
kind: scripting_engine
name: Structured Scripting Engine
category: automation
scope:
    - 'crates/velocity-core/src/scripting.rs'
    - 'crates/velocity-config/src/manifest.rs'
source_files:
    - crates/velocity-core/src/scripting.rs
    - crates/velocity-config/src/manifest.rs
---

The Velocity Installer includes a structured scripting engine for custom actions during installation and uninstallation. It goes beyond simple shell commands with typed actions, variable substitution, condition evaluation, and configurable error policies.

**Architecture:**
- **7 action types** — shell, copy, delete, mkdir, delete_dir, registry, env_var
- **Variable substitution** — `{install_dir}`, `{app_name}`, `{version}` resolved at runtime
- **Condition evaluation** — Skip actions based on file/dir/env/action state
- **Error policies** — Abort, Continue, or Retry(N) per action
- **Sequence execution** — Actions run in order with early termination on Abort

**Action Types:**
```rust
pub enum ActionType {
    ShellCommand(String),              // cmd /C on Windows, sh -c on Unix
    CopyFile { src: String, dest: String },
    DeleteFile(String),
    CreateDir(String),
    DeleteDir(String),                 // recursive
    WriteRegistry { key: String, name: String, value: String },
    SetEnvVar { name: String, value: String, scope: String },
}
```

**TOML Configuration:**
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
on_error = "continue"

[[scripts.post_install_actions]]
name = "Set API endpoint"
action = "registry"
key = "Software\\MyApp\\Settings"
value_name = "ApiUrl"
value = "https://api.example.com"
on_error = "abort"
```

**Condition Expressions:**
| Condition | Description |
|-----------|-------------|
| `""` (empty) | Always true |
| `"always"` | Always true |
| `"never"` | Always false |
| `"file_exists:<path>"` | True if file exists |
| `"file_missing:<path>"` | True if file doesn't exist |
| `"dir_exists:<path>"` | True if directory exists |
| `"reg_exists:<HKLM\\...>"` | True if registry key exists |
| `"action_success:<name>"` | True if named action succeeded |
| `"env_set:<NAME>"` | True if environment variable is set |

**Error Policies:**
```rust
pub enum ErrorPolicy {
    Abort,          // Stop the script sequence immediately
    Continue,       // Log error and proceed to next action
    Retry(u32),     // Retry up to N times, then abort
}
```

**Config-to-Action Mapping:**
| TOML `action` value | ActionType | Required fields |
|---------------------|-----------|-----------------|
| `"shell"` / `"cmd"` | ShellCommand | `path` |
| `"copy"` | CopyFile | `src`, `dest` |
| `"delete"` | DeleteFile | `path` |
| `"delete_dir"` / `"rmdir"` | DeleteDir | `path` |
| `"mkdir"` | CreateDir | `path` |
| `"registry"` | WriteRegistry | `key`, `value_name`, `value` |
| `"env_var"` / `"env"` | SetEnvVar | `env_name`, `value`, `scope` |

**Variable Context:**
```rust
pub fn build_variable_context(
    install_dir: &str, app_name: &str, version: &str,
) -> HashMap<String, String> {
    // Provides: install_dir, app_name, version,
    //           system_root, system32, program_files, temp
}
```

**Platform Behavior:**
- Windows: Shell commands run via `cmd /C <command>`
- Unix: Shell commands run via `sh -c <command>`
- Registry actions: Windows-only (no-op with error on Unix)
- Env var actions: Cross-platform via velocity-core env_vars module

**Key files:**
- `crates/velocity-core/src/scripting.rs` — ScriptEngine, ActionType, condition evaluation (690 lines, 13 tests)
- `crates/velocity-config/src/manifest.rs` — ScriptActionConfig struct definition

**Rules for developers:**
1. All action paths must go through variable substitution before execution
2. Unknown action types are skipped with a warning (not an error)
3. Unknown condition types default to `true` with a warning
4. The `Retry` policy re-executes the full action (including condition check)
5. Shell commands inherit the install directory as working directory
6. Registry actions delegate to the registry module (tracked for rollback)
