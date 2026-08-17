//! Integration tests for the WASM plugin system.
//!
//! These tests compile the sample plugin from WAT and load it through the
//! full plugin loader pipeline.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use velocity_plugin_api::*;

/// Mock host that records all calls for verification.
struct RecordingHost {
    logs: Mutex<Vec<(String, String)>>,
    variables: Mutex<HashMap<String, String>>,
}

impl RecordingHost {
    fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
            variables: Mutex::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    fn log_count(&self) -> usize {
        self.logs.lock().unwrap().len()
    }

    #[allow(dead_code)]
    fn has_log_containing(&self, text: &str) -> bool {
        self.logs
            .lock()
            .unwrap()
            .iter()
            .any(|(_, msg)| msg.contains(text))
    }
}

impl HostApi for RecordingHost {
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
    fn registry_write(&self, _key: &str, _value_name: &str, _value_data: &str) -> PluginResult<()> {
        Err(PluginError::not_implemented("registry_write"))
    }
    fn set_progress(&self, _percent: u32, _status_text: &str) {}
}

fn test_context() -> PluginContext {
    PluginContext {
        app_name: "IntegrationTest".into(),
        app_version: "2.0.0".into(),
        install_dir: "C:\\TestApp".into(),
        publisher: "Test Publisher".into(),
        arch: "x64".into(),
        parameters: HashMap::new(),
        quiet_mode: true,
        session_id: "integration-test-session".into(),
    }
}

/// Compile the sample plugin WAT and load it.
#[test]
fn test_load_sample_plugin() {
    // Compile the sample plugin from WAT
    let wat_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/sample-plugin/plugin.wat"
    ))
    .expect("Failed to read sample plugin WAT");

    let wasm = wat::parse_str(&wat_source).expect("Failed to compile WAT");

    // Write to temp file
    let temp_dir = std::env::temp_dir().join("velocity_plugin_integration_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let wasm_path = temp_dir.join("plugin.wasm");
    let json_path = temp_dir.join("plugin.json");
    std::fs::write(&wasm_path, &wasm).unwrap();

    // Copy the plugin.json from the example
    let json_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/sample-plugin/plugin.json"
    ))
    .unwrap();
    std::fs::write(&json_path, &json_source).unwrap();

    // Load the plugin
    let ctx = test_context();
    let host = Arc::new(RecordingHost::new());
    let plugin = load_wasm_plugin(&wasm_path, ctx, host.clone());

    assert!(
        plugin.is_ok(),
        "Failed to load sample plugin: {:?}",
        plugin.err()
    );
    let plugin = plugin.unwrap();

    // Verify manifest
    assert_eq!(plugin.manifest().name, "hello-world");
    assert_eq!(plugin.manifest().version, "1.0.0");
    assert_eq!(plugin.manifest().api_version, 1);
    assert!(plugin
        .manifest()
        .supported_events
        .contains(&"load".to_string()));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Test that a minimal WASM module without hooks loads and responds correctly.
#[test]
fn test_minimal_plugin_no_hooks() {
    // A minimal module with just memory — no hook exports
    let wasm = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap();

    let temp_dir = std::env::temp_dir().join("velocity_minimal_test");
    let _ = std::fs::create_dir_all(&temp_dir);
    let wasm_path = temp_dir.join("minimal.wasm");
    std::fs::write(&wasm_path, &wasm).unwrap();

    let ctx = test_context();
    let host = Arc::new(RecordingHost::new());
    let plugin = load_wasm_plugin(&wasm_path, ctx, host);

    assert!(plugin.is_ok());
    let plugin = plugin.unwrap();
    assert_eq!(plugin.manifest().name, "minimal"); // Derived from filename

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Test plugin event parsing from manifest.
#[test]
fn test_plugin_event_parsing() {
    let events = vec!["load", "pre_install", "post_install", "error", "unload"];
    for event_str in &events {
        let event = PluginEvent::from_str_name(event_str);
        assert!(event.is_some(), "Failed to parse event: {}", event_str);
        assert_eq!(event.unwrap().as_str(), *event_str);
    }
}

/// Test that invalid WASM is rejected gracefully.
#[test]
fn test_invalid_wasm_rejected() {
    let temp_dir = std::env::temp_dir().join("velocity_invalid_test");
    let _ = std::fs::create_dir_all(&temp_dir);
    let wasm_path = temp_dir.join("invalid.wasm");
    std::fs::write(&wasm_path, b"not a wasm file").unwrap();

    let ctx = test_context();
    let host = Arc::new(RecordingHost::new());
    let result = load_wasm_plugin(&wasm_path, ctx, host);

    assert!(result.is_err(), "Should reject invalid WASM");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
