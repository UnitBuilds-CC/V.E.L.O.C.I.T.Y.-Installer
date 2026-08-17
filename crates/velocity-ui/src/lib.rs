//! Velocity Installer — Wizard UI
//!
//! Provides both a modern (WebView2) and classic (Win32) installer wizard.

pub mod classic;
pub mod wizard;
pub mod error;

pub use error::*;
pub use wizard::*;
