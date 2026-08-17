//! Velocity Installer — Core Engine
//!
//! Handles file extraction, registry operations, shortcut creation,
//! service management, environment variables, and uninstaller generation.

pub mod extract;
pub mod registry;
pub mod shortcuts;
pub mod services;
pub mod env_vars;
pub mod uninstaller;
pub mod elevation;
pub mod payload;
pub mod error;

pub use error::*;
