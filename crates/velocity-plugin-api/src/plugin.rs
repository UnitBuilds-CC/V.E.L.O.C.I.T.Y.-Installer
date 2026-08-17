//! Plugin trait and types for Velocity Installer custom actions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The trait that all Velocity plugins must implement.
///
/// Plugins are loaded as WASM modules and can perform custom actions
/// during installation, uninstallation, or in response to events.
pub trait VelocityPlugin {
    /// Plugin name.
    fn name(&self) -> &str;

    /// Plugin version.
    fn version(&self) -> &str;

    /// Called when the plugin is loaded.
    fn on_load(&mut self, context: &PluginContext) -> PluginResult<()>;

    /// Called before installation begins.
    fn on_pre_install(&self, context: &PluginContext) -> PluginResult<()>;

    /// Called after installation completes.
    fn on_post_install(&self, context: &PluginContext) -> PluginResult<()>;

    /// Called before uninstallation begins.
    fn on_pre_uninstall(&self, context: &PluginContext) -> PluginResult<()>;

    /// Called after uninstallation completes.
    fn on_post_uninstall(&self, context: &PluginContext) -> PluginResult<()>;
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

/// Plugin manifest describing a plugin's capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub entry_point: String,
    pub supported_events: Vec<String>,
}
