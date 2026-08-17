//! Velocity Installer — Plugin API
//!
//! Defines the plugin trait, host API, and WASM loader for custom installer actions.
//!
//! # Architecture
//!
//! - [`VelocityPlugin`] trait — lifecycle hooks every plugin implements
//! - [`HostApi`] trait — installer functions callable from plugins
//! - [`loader`] module — discovers and loads `.wasm` plugin files via Wasmtime

pub mod loader;
pub mod plugin;

pub use loader::{default_plugin_dir, discover_plugins, load_wasm_plugin, WasmPlugin};
pub use plugin::*;
