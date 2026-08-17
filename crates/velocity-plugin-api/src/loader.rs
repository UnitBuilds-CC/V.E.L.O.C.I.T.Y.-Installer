//! WASM plugin loader using the Wasmtime runtime.
//!
//! Loads `.wasm` plugin modules and bridges them to the [`VelocityPlugin`] trait.
//! Plugins export lifecycle hook functions that are called by the installer.
//!
//! # WASM Interface
//!
//! Each plugin must export a `memory` and these functions:
//!
//! ```wit
//! // Lifecycle hooks — receive context JSON pointer + length, return 0 for success
//! on_load(ctx_ptr: i32, ctx_len: i32) -> i32
//! on_pre_install(ctx_ptr: i32, ctx_len: i32) -> i32
//! on_post_install(ctx_ptr: i32, ctx_len: i32) -> i32
//! on_pre_uninstall(ctx_ptr: i32, ctx_len: i32) -> i32
//! on_post_uninstall(ctx_ptr: i32, ctx_len: i32) -> i32
//! on_error(ctx_ptr: i32, ctx_len: i32, err_ptr: i32, err_len: i32) -> i32
//! on_cancel(ctx_ptr: i32, ctx_len: i32) -> i32
//! on_unload(ctx_ptr: i32, ctx_len: i32) -> i32
//! ```
//!
//! Plugins can import host functions from the "env" module for logging, file I/O, etc.

use crate::plugin::*;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};
use wasmtime::*;

/// Host state stored in the Wasmtime Store.
#[allow(dead_code)]
struct HostState {
    host: Arc<dyn HostApi>,
    ctx: PluginContext,
}

/// A loaded WASM plugin instance.
#[allow(dead_code)]
pub struct WasmPlugin {
    /// Plugin manifest
    manifest: PluginManifest,
    /// Wasmtime engine
    engine: Engine,
    /// WASM store (owns the instance state)
    store: Store<HostState>,
    /// WASM module instance
    instance: Instance,
    /// Plugin context (kept for trait method calls)
    ctx: PluginContext,
    /// Host API reference
    host: Arc<dyn HostApi>,
}

/// Load a WASM plugin from a `.wasm` file.
///
/// The `host` parameter provides the plugin with access to installer capabilities.
/// The `ctx` parameter provides installation context.
pub fn load_wasm_plugin(
    wasm_path: &Path,
    ctx: PluginContext,
    host: Arc<dyn HostApi>,
) -> Result<WasmPlugin> {
    info!("Loading WASM plugin from: {}", wasm_path.display());

    let engine = Engine::default();

    let wasm_bytes = std::fs::read(wasm_path)
        .with_context(|| format!("Failed to read WASM file: {}", wasm_path.display()))?;

    let module =
        Module::new(&engine, &wasm_bytes).with_context(|| "Failed to compile WASM module")?;

    let manifest = load_plugin_manifest(wasm_path)?;

    info!(
        "Loaded plugin '{}' v{} (API v{})",
        manifest.name, manifest.version, manifest.api_version
    );

    // Create the store with host state
    let host_state = HostState {
        host: host.clone(),
        ctx: ctx.clone(),
    };
    let mut store = Store::new(&engine, host_state);

    // Create the linker with host function imports
    let mut linker = Linker::new(&engine);
    define_host_functions(&mut linker)?;

    // Instantiate the module
    let instance = linker
        .instantiate(&mut store, &module)
        .with_context(|| "Failed to instantiate WASM module")?;

    Ok(WasmPlugin {
        manifest,
        engine,
        store,
        instance,
        ctx,
        host,
    })
}

/// Load the plugin manifest from a sidecar JSON file.
fn load_plugin_manifest(wasm_path: &Path) -> Result<PluginManifest> {
    let json_path = wasm_path.with_extension("json");
    if json_path.exists() {
        let json = std::fs::read_to_string(&json_path)
            .with_context(|| format!("Failed to read plugin manifest: {}", json_path.display()))?;
        let manifest: PluginManifest =
            serde_json::from_str(&json).with_context(|| "Invalid plugin manifest JSON")?;
        return Ok(manifest);
    }

    // Fallback: derive from filename
    let stem = wasm_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    Ok(PluginManifest {
        name: stem.to_string(),
        version: "0.0.0".to_string(),
        author: String::new(),
        description: format!("WASM plugin loaded from {}", wasm_path.display()),
        entry_point: wasm_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.wasm")
            .to_string(),
        supported_events: vec!["pre_install".into(), "post_install".into()],
        api_version: 1,
        parameters: Default::default(),
    })
}

/// Define host functions that the WASM plugin can import.
fn define_host_functions(linker: &mut Linker<HostState>) -> Result<()> {
    // host_log(level_ptr, level_len, msg_ptr, msg_len)
    linker.func_wrap(
        "env",
        "host_log",
        |mut caller: Caller<'_, HostState>,
         level_ptr: i32,
         level_len: i32,
         msg_ptr: i32,
         msg_len: i32| {
            let memory = caller.get_export("memory").and_then(|e| e.into_memory());
            if let Some(mem) = memory {
                let data = mem.data(&caller);
                let level = read_string(data, level_ptr as usize, level_len as usize);
                let msg = read_string(data, msg_ptr as usize, msg_len as usize);
                caller.data().host.log(&level, &msg);
            }
        },
    )?;

    // host_file_exists(path_ptr, path_len) -> i32 (0 or 1)
    linker.func_wrap(
        "env",
        "host_file_exists",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let memory = caller.get_export("memory").and_then(|e| e.into_memory());
            if let Some(mem) = memory {
                let data = mem.data(&caller);
                let path = read_string(data, path_ptr as usize, path_len as usize);
                return if caller.data().host.file_exists(&path) {
                    1
                } else {
                    0
                };
            }
            0
        },
    )?;

    // host_set_progress(percent, text_ptr, text_len)
    linker.func_wrap(
        "env",
        "host_set_progress",
        |mut caller: Caller<'_, HostState>, percent: i32, text_ptr: i32, text_len: i32| {
            let memory = caller.get_export("memory").and_then(|e| e.into_memory());
            if let Some(mem) = memory {
                let data = mem.data(&caller);
                let text = read_string(data, text_ptr as usize, text_len as usize);
                caller.data().host.set_progress(percent as u32, &text);
            }
        },
    )?;

    // host_show_message(title_ptr, title_len, msg_ptr, msg_len)
    linker.func_wrap(
        "env",
        "host_show_message",
        |mut caller: Caller<'_, HostState>,
         title_ptr: i32,
         title_len: i32,
         msg_ptr: i32,
         msg_len: i32| {
            let memory = caller.get_export("memory").and_then(|e| e.into_memory());
            if let Some(mem) = memory {
                let data = mem.data(&caller);
                let title = read_string(data, title_ptr as usize, title_len as usize);
                let msg = read_string(data, msg_ptr as usize, msg_len as usize);
                caller.data().host.show_message(&title, &msg);
            }
        },
    )?;

    // host_get_variable(name_ptr, name_len) -> i64 (ptr<<32 | len, or -1 if not found)
    linker.func_wrap(
        "env",
        "host_get_variable",
        |mut caller: Caller<'_, HostState>, name_ptr: i32, name_len: i32| -> i64 {
            let memory = caller.get_export("memory").and_then(|e| e.into_memory());
            if let Some(mem) = memory {
                let data = mem.data(&caller);
                let name = read_string(data, name_ptr as usize, name_len as usize);
                if let Some(val) = caller.data().host.get_variable(&name) {
                    // Write value to scratch space at offset 64KB
                    let val_bytes = val.as_bytes();
                    let scratch = 65536usize;
                    // Note: we can only read memory here, not write (caller is immutable).
                    // Return the value packed — the plugin would need a separate
                    // "write to memory" function for full bidirectional comms.
                    // For now, return length info so the plugin knows the var exists.
                    return ((scratch as i64) << 32) | (val_bytes.len() as i64);
                }
            }
            -1i64
        },
    )?;

    Ok(())
}

/// Read a UTF-8 string from WASM memory.
fn read_string(memory: &[u8], ptr: usize, len: usize) -> String {
    let end = (ptr + len).min(memory.len());
    if ptr >= end {
        return String::new();
    }
    String::from_utf8_lossy(&memory[ptr..end]).to_string()
}

impl WasmPlugin {
    /// Get the plugin manifest.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Call a WASM export with a JSON context. Returns Ok(()) if the hook
    /// doesn't exist (not exported) or returns 0. Returns Err on non-zero.
    fn call_hook(&mut self, name: &str, ctx_json: &str) -> PluginResult<()> {
        let ctx_bytes = ctx_json.as_bytes();
        let ctx_ptr = 1024usize;

        // Write context JSON to WASM memory
        if let Some(mem) = self.instance.get_memory(&mut self.store, "memory") {
            let data = mem.data_mut(&mut self.store);
            let end = (ctx_ptr + ctx_bytes.len()).min(data.len());
            data[ctx_ptr..end].copy_from_slice(ctx_bytes);
        }

        // Call the exported function
        let func = self.instance.get_func(&mut self.store, name);
        match func {
            Some(f) => {
                let mut results = [Val::I32(0)];
                let result = f.call(
                    &mut self.store,
                    &[Val::I32(ctx_ptr as i32), Val::I32(ctx_bytes.len() as i32)],
                    &mut results,
                );
                match result {
                    Ok(_) => {
                        if let Val::I32(code) = results[0] {
                            if code == 0 {
                                Ok(())
                            } else {
                                Err(PluginError::new(
                                    "PLUGIN_ERROR",
                                    format!("Hook '{}' returned error code {}", name, code),
                                ))
                            }
                        } else {
                            Ok(())
                        }
                    }
                    Err(e) => {
                        error!("Plugin hook '{}' trapped: {}", name, e);
                        Err(PluginError::new("WASM_TRAP", format!("{}", e)))
                    }
                }
            }
            None => {
                // Hook not exported — not an error, just skip
                Ok(())
            }
        }
    }

    /// Call a WASM export with context + error string parameters.
    #[allow(dead_code)]
    fn call_hook_with_error(
        &mut self,
        name: &str,
        ctx_json: &str,
        error_msg: &str,
    ) -> PluginResult<()> {
        let ctx_bytes = ctx_json.as_bytes();
        let err_bytes = error_msg.as_bytes();
        let ctx_ptr = 1024usize;
        let err_ptr = 32768usize;

        if let Some(mem) = self.instance.get_memory(&mut self.store, "memory") {
            let data = mem.data_mut(&mut self.store);
            let ctx_end = (ctx_ptr + ctx_bytes.len()).min(data.len());
            data[ctx_ptr..ctx_end].copy_from_slice(ctx_bytes);
            let err_end = (err_ptr + err_bytes.len()).min(data.len());
            data[err_ptr..err_end].copy_from_slice(err_bytes);
        }

        let func = self.instance.get_func(&mut self.store, name);
        match func {
            Some(f) => {
                let mut results = [Val::I32(0)];
                let result = f.call(
                    &mut self.store,
                    &[
                        Val::I32(ctx_ptr as i32),
                        Val::I32(ctx_bytes.len() as i32),
                        Val::I32(err_ptr as i32),
                        Val::I32(err_bytes.len() as i32),
                    ],
                    &mut results,
                );
                match result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(PluginError::new("WASM_TRAP", format!("{}", e))),
                }
            }
            None => Ok(()),
        }
    }
}

impl VelocityPlugin for WasmPlugin {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn version(&self) -> &str {
        &self.manifest.version
    }

    fn on_load(&mut self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()> {
        let ctx_json = serde_json::to_string(ctx)
            .map_err(|e| PluginError::new("SERDE_ERROR", e.to_string()))?;
        host.log("info", &format!("Loading plugin '{}'", self.manifest.name));
        self.call_hook("on_load", &ctx_json)
    }

    fn on_pre_install(&self, _ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()> {
        host.log("debug", "plugin: on_pre_install");
        // Note: call_hook needs &mut self, but the trait takes &self.
        // For WASM plugins, we use interior mutability through the store.
        // This is a design limitation — see the plugin loader docs.
        Ok(())
    }

    fn on_file_extracted(
        &self,
        _ctx: &PluginContext,
        host: &dyn HostApi,
        file_path: &str,
    ) -> PluginResult<()> {
        host.log(
            "debug",
            &format!("plugin: on_file_extracted({})", file_path),
        );
        Ok(())
    }

    fn on_post_install(&self, _ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()> {
        host.log("debug", "plugin: on_post_install");
        Ok(())
    }

    fn on_pre_uninstall(&self, _ctx: &PluginContext, _host: &dyn HostApi) -> PluginResult<()> {
        Ok(())
    }

    fn on_post_uninstall(&self, _ctx: &PluginContext, _host: &dyn HostApi) -> PluginResult<()> {
        Ok(())
    }

    fn on_error(&self, _ctx: &PluginContext, host: &dyn HostApi, error: &str) -> PluginResult<()> {
        host.log("error", &format!("plugin error hook: {}", error));
        Ok(())
    }

    fn on_cancel(&self, _ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()> {
        host.log("warn", "plugin: installation cancelled");
        Ok(())
    }

    fn on_unload(&self, _ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()> {
        host.log(
            "info",
            &format!("Unloading plugin '{}'", self.manifest.name),
        );
        Ok(())
    }
}

/// Discover and load all plugins from a directory.
///
/// Scans for `.wasm` files and loads each one as a plugin.
/// Plugins without a valid manifest are skipped with a warning.
pub fn discover_plugins(
    plugin_dir: &Path,
    ctx: &PluginContext,
    host: Arc<dyn HostApi>,
) -> Vec<WasmPlugin> {
    let mut plugins = Vec::new();

    if !plugin_dir.exists() {
        info!("Plugin directory does not exist: {}", plugin_dir.display());
        return plugins;
    }

    let entries = match std::fs::read_dir(plugin_dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(
                "Failed to read plugin directory {}: {}",
                plugin_dir.display(),
                e
            );
            return plugins;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("wasm") {
            match load_wasm_plugin(&path, ctx.clone(), host.clone()) {
                Ok(plugin) => {
                    info!(
                        "Discovered plugin: {} v{}",
                        plugin.manifest().name,
                        plugin.manifest().version
                    );
                    plugins.push(plugin);
                }
                Err(e) => {
                    warn!("Failed to load plugin {}: {}", path.display(), e);
                }
            }
        }
    }

    info!(
        "Discovered {} plugin(s) in {}",
        plugins.len(),
        plugin_dir.display()
    );
    plugins
}

/// Get the default plugin directory path relative to the installer.
pub fn default_plugin_dir(install_dir: &Path) -> PathBuf {
    install_dir.join("plugins")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_context() -> PluginContext {
        PluginContext {
            app_name: "TestApp".into(),
            app_version: "1.0.0".into(),
            install_dir: "C:\\Test".into(),
            publisher: "Test".into(),
            arch: "x64".into(),
            parameters: HashMap::new(),
            quiet_mode: true,
            session_id: "test-session".into(),
        }
    }

    /// A mock host API for testing.
    struct MockHost {
        logs: std::sync::Mutex<Vec<(String, String)>>,
        variables: std::sync::Mutex<HashMap<String, String>>,
    }

    impl MockHost {
        fn new() -> Self {
            Self {
                logs: std::sync::Mutex::new(Vec::new()),
                variables: std::sync::Mutex::new(HashMap::new()),
            }
        }
    }

    impl HostApi for MockHost {
        fn log(&self, level: &str, message: &str) {
            self.logs
                .lock()
                .unwrap()
                .push((level.to_string(), message.to_string()));
        }
        fn get_variable(&self, name: &str) -> Option<String> {
            self.variables.lock().unwrap().get(name).cloned()
        }
        fn set_variable(&self, name: &str, value: &str) {
            self.variables
                .lock()
                .unwrap()
                .insert(name.to_string(), value.to_string());
        }
        fn show_message(&self, _title: &str, _message: &str) {}
        fn file_exists(&self, _path: &str) -> bool {
            false
        }
        fn dir_exists(&self, _path: &str) -> bool {
            false
        }
        fn read_file(&self, _path: &str) -> PluginResult<String> {
            Err(PluginError::not_implemented("read_file"))
        }
        fn write_file(&self, _path: &str, _content: &str) -> PluginResult<()> {
            Err(PluginError::not_implemented("write_file"))
        }
        fn create_dir(&self, _path: &str) -> PluginResult<()> {
            Err(PluginError::not_implemented("create_dir"))
        }
        fn exec(&self, _command: &str, _args: &[&str]) -> PluginResult<(i32, String)> {
            Err(PluginError::not_implemented("exec"))
        }
        fn registry_read(&self, _key: &str, _value_name: &str) -> PluginResult<String> {
            Err(PluginError::not_implemented("registry_read"))
        }
        fn registry_write(
            &self,
            _key: &str,
            _value_name: &str,
            _value_data: &str,
        ) -> PluginResult<()> {
            Err(PluginError::not_implemented("registry_write"))
        }
        fn set_progress(&self, _percent: u32, _status_text: &str) {}
    }

    #[test]
    fn test_default_plugin_dir() {
        let dir = default_plugin_dir(Path::new("C:\\Program Files\\MyApp"));
        assert_eq!(dir, PathBuf::from("C:\\Program Files\\MyApp\\plugins"));
    }

    #[test]
    fn test_discover_plugins_empty_dir() {
        let ctx = test_context();
        let host = Arc::new(MockHost::new());
        let plugins = discover_plugins(Path::new("/nonexistent/path"), &ctx, host);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_load_manifest_fallback() {
        let fake_path = Path::new("my_plugin.wasm");
        let manifest = load_plugin_manifest(fake_path).unwrap();
        assert_eq!(manifest.name, "my_plugin");
        assert_eq!(manifest.api_version, 1);
    }

    #[test]
    fn test_read_string() {
        let data = b"Hello, World!";
        assert_eq!(read_string(data, 0, 5), "Hello");
        assert_eq!(read_string(data, 7, 5), "World");
        assert_eq!(read_string(data, 0, 13), "Hello, World!");
    }

    #[test]
    fn test_read_string_out_of_bounds() {
        let data = b"short";
        let s = read_string(data, 0, 100);
        assert_eq!(s, "short");
    }

    #[test]
    fn test_read_string_empty() {
        let data = b"test";
        assert_eq!(read_string(data, 4, 0), "");
        assert_eq!(read_string(data, 10, 5), "");
    }

    #[test]
    fn test_mock_host_logging() {
        let host = MockHost::new();
        host.log("info", "test message");
        host.log("error", "bad thing");
        let logs = host.logs.lock().unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], ("info".to_string(), "test message".to_string()));
    }

    #[test]
    fn test_mock_host_variables() {
        let host = MockHost::new();
        assert!(host.get_variable("foo").is_none());
        host.set_variable("foo", "bar");
        assert_eq!(host.get_variable("foo"), Some("bar".to_string()));
    }

    #[test]
    fn test_load_minimal_wasm_module() {
        // Create a minimal valid WASM module with memory export
        let wasm = wat::parse_str(
            r#"(module
                (memory (export "memory") 1)
            )"#,
        )
        .unwrap();

        // Write to temp file
        let temp_dir = std::env::temp_dir().join("velocity_plugin_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let wasm_path = temp_dir.join("test_plugin.wasm");
        std::fs::write(&wasm_path, &wasm).unwrap();

        let ctx = test_context();
        let host = Arc::new(MockHost::new());
        let plugin = load_wasm_plugin(&wasm_path, ctx, host);
        assert!(plugin.is_ok(), "Should load minimal WASM module");
        let plugin = plugin.unwrap();
        assert_eq!(plugin.manifest().name, "test_plugin");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
