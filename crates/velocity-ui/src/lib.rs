//! Velocity Installer — Wizard UI
//!
//! Provides both a modern (WebView2) and classic (Win32) installer wizard.
//! Currently Windows-only. Cross-platform wizard UI using wry+tao is planned.

#![cfg(target_os = "windows")]

pub mod classic;
pub mod error;
pub mod modern;
pub mod native_wizard;
pub mod progress_dialog;
pub mod wizard;

pub use error::*;
pub use wizard::*;
