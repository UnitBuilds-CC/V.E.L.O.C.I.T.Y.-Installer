//! Velocity Installer — Wizard UI
//!
//! Provides both a modern (WebView2) and classic (Win32) installer wizard on Windows,
//! and a terminal-based wizard on Linux/macOS.

pub mod error;

#[cfg(target_os = "windows")]
pub mod classic;
#[cfg(target_os = "windows")]
pub mod modern;
#[cfg(target_os = "windows")]
pub mod native_wizard;
#[cfg(target_os = "windows")]
pub mod progress_dialog;

#[cfg(not(target_os = "windows"))]
pub mod cross_platform;

pub mod wizard;

pub use error::*;
pub use wizard::*;
