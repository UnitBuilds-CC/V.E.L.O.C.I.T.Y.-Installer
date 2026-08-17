//! Plugin trait and types for Velocity Installer custom actions.
//!
//! # Plugin Lifecycle
//!
//! Plugins are WASM modules that hook into installer lifecycle events:
//!
//! ```text
//! on_load → on_pre_install → [on_file_extracted...] → on_post_install → on_unload
//!                 ↓ (on error)
//!             on_error → on_unload
//! ```
//!
//! # Host API
//!
//! Plugins can call back into the installer via the [`HostApi`] trait, which
//! provides logging, file operations, registry access, and UI notifications.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The trait that all Velocity plugins must implement.
///
/// Plugins are loaded as WASM modules and can perform custom actions
/// during installation, uninstallation, or in response to events.
///
/// All lifecycle methods receive a [`PluginContext`] for reading installer state
/// and a mutable [`HostApi`] reference for calling back into the installer.
pub trait VelocityPlugin: Send + Sync {
    /// Plugin name (must be unique).
    fn name(&self) -> &str;

    /// Plugin version (semver).
    fn version(&self) -> &str;

    /// Called when the plugin is loaded. Use for initialization.
    fn on_load(&mut self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;

    /// Called before installation begins. Return Err to abort.
    fn on_pre_install(&self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;

    /// Called after each file is extracted.
    fn on_file_extracted(
        &self,
        ctx: &PluginContext,
        host: &dyn HostApi,
        file_path: &str,
    ) -> PluginResult<()>;

    /// Called after installation completes successfully.
    fn on_post_install(&self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;

    /// Called before uninstallation begins.
    fn on_pre_uninstall(&self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;

    /// Called after uninstallation completes.
    fn on_post_uninstall(&self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;

    /// Called when an error occurs during installation.
    fn on_error(&self, ctx: &PluginContext, host: &dyn HostApi, error: &str) -> PluginResult<()>;

    /// Called when the user cancels installation.
    fn on_cancel(&self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;

    /// Called when the plugin is unloaded. Use for cleanup.
    fn on_unload(&self, ctx: &PluginContext, host: &dyn HostApi) -> PluginResult<()>;
}

/// Host API — functions the plugin can call back into the installer.
///
/// This trait is implemented by the installer runtime and provides plugins
/// with controlled access to the installer's capabilities.
pub trait HostApi: Send + Sync {
    /// Log a message at the given level ("info", "warn", "error", "debug").
    fn log(&self, level: &str, message: &str);

    /// Get the value of an installer variable.
    fn get_variable(&self, name: &str) -> Option<String>;

    /// Set an installer variable.
    fn set_variable(&self, name: &str, value: &str);

    /// Show a message to the user (only effective in non-silent mode).
    fn show_message(&self, title: &str, message: &str);

    /// Check if a file exists on disk.
    fn file_exists(&self, path: &str) -> bool;

    /// Check if a directory exists on disk.
    fn dir_exists(&self, path: &str) -> bool;

    /// Read a text file's contents.
    fn read_file(&self, path: &str) -> PluginResult<String>;

    /// Write content to a text file.
    fn write_file(&self, path: &str, content: &str) -> PluginResult<()>;

    /// Create a directory (and parents).
    fn create_dir(&self, path: &str) -> PluginResult<()>;

    /// Execute a shell command and return (exit_code, stdout).
    fn exec(&self, command: &str, args: &[&str]) -> PluginResult<(i32, String)>;

    /// Read a Windows registry value.
    fn registry_read(&self, key: &str, value_name: &str) -> PluginResult<String>;

    /// Write a Windows registry value.
    fn registry_write(&self, key: &str, value_name: &str, value_data: &str) -> PluginResult<()>;

    /// Update the progress bar (0-100).
    fn set_progress(&self, percent: u32, status_text: &str);
}

/// Context provided to plugins during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginContext {
    /// Application name
    pub app_name: String,
    /// Application version
    pub app_version: String,
    /// Installation directory
    pub install_dir: String,
    /// Publisher name
    pub publisher: String,
    /// Architecture (x64, x86, arm64)
    pub arch: String,
    /// Custom parameters passed to the plugin
    pub parameters: HashMap<String, String>,
    /// Whether the installer is running in quiet mode
    pub quiet_mode: bool,
    /// Unique session ID for this installation
    pub session_id: String,
}

/// Result type for plugin operations.
pub type PluginResult<T> = std::result::Result<T, PluginError>;

/// Errors that can occur in plugin operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for PluginError {}

impl PluginError {
    /// Create a new plugin error.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Create a "not implemented" error.
    pub fn not_implemented(hook: &str) -> Self {
        Self {
            code: "NOT_IMPLEMENTED".into(),
            message: format!("Plugin hook '{}' not implemented", hook),
        }
    }
}

/// Plugin manifest describing a plugin's capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name
    pub name: String,
    /// Plugin version (semver)
    pub version: String,
    /// Author name
    #[serde(default)]
    pub author: String,
    /// Short description
    #[serde(default)]
    pub description: String,
    /// WASM entry point file name
    pub entry_point: String,
    /// Which lifecycle events this plugin handles
    #[serde(default)]
    pub supported_events: Vec<String>,
    /// Plugin API version (must be compatible with host)
    #[serde(default = "default_api_version")]
    pub api_version: u32,
    /// Parameters the plugin accepts (with defaults)
    #[serde(default)]
    pub parameters: HashMap<String, String>,
}

fn default_api_version() -> u32 {
    1
}

/// Lifecycle event types that plugins can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginEvent {
    Load,
    PreInstall,
    FileExtracted,
    PostInstall,
    PreUninstall,
    PostUninstall,
    Error,
    Cancel,
    Unload,
}

impl PluginEvent {
    /// Parse an event from a string name.
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s {
            "load" => Some(Self::Load),
            "pre_install" => Some(Self::PreInstall),
            "file_extracted" => Some(Self::FileExtracted),
            "post_install" => Some(Self::PostInstall),
            "pre_uninstall" => Some(Self::PreUninstall),
            "post_uninstall" => Some(Self::PostUninstall),
            "error" => Some(Self::Error),
            "cancel" => Some(Self::Cancel),
            "unload" => Some(Self::Unload),
            _ => None,
        }
    }

    /// Get the string name of this event.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::PreInstall => "pre_install",
            Self::FileExtracted => "file_extracted",
            Self::PostInstall => "post_install",
            Self::PreUninstall => "pre_uninstall",
            Self::PostUninstall => "post_uninstall",
            Self::Error => "error",
            Self::Cancel => "cancel",
            Self::Unload => "unload",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::new("E001", "Something went wrong");
        assert_eq!(err.to_string(), "[E001] Something went wrong");
    }

    #[test]
    fn test_plugin_error_not_implemented() {
        let err = PluginError::not_implemented("on_file_extracted");
        assert_eq!(err.code, "NOT_IMPLEMENTED");
        assert!(err.message.contains("on_file_extracted"));
    }

    #[test]
    fn test_plugin_context_serialization() {
        let ctx = PluginContext {
            app_name: "Test".into(),
            app_version: "1.0.0".into(),
            install_dir: "C:\\Test".into(),
            publisher: "Acme".into(),
            arch: "x64".into(),
            parameters: HashMap::new(),
            quiet_mode: false,
            session_id: "abc-123".into(),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: PluginContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.app_name, "Test");
        assert_eq!(back.session_id, "abc-123");
    }

    #[test]
    fn test_plugin_manifest_deserialization() {
        let json = r#"{
            "name": "my-plugin",
            "version": "0.1.0",
            "entry_point": "plugin.wasm",
            "supported_events": ["pre_install", "post_install"]
        }"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, "my-plugin");
        assert_eq!(m.api_version, 1);
        assert_eq!(m.supported_events.len(), 2);
    }

    #[test]
    fn test_plugin_event_roundtrip() {
        let events = [
            PluginEvent::Load,
            PluginEvent::PreInstall,
            PluginEvent::FileExtracted,
            PluginEvent::PostInstall,
            PluginEvent::Error,
            PluginEvent::Cancel,
            PluginEvent::Unload,
        ];
        for event in &events {
            let s = event.as_str();
            let back = PluginEvent::from_str_name(s).unwrap();
            assert_eq!(&back, event);
        }
    }

    #[test]
    fn test_plugin_event_from_str_unknown() {
        assert!(PluginEvent::from_str_name("unknown_event").is_none());
    }
}
