//! Velocity Installer — Core Engine
//!
//! Handles file extraction, registry operations, shortcut creation,
//! service management, environment variables, uninstaller generation,
//! disk space validation, rollback, logging, file associations, and
//! process detection.

pub mod extract;
pub mod registry;
pub mod shortcuts;
pub mod services;
pub mod env_vars;
pub mod uninstaller;
pub mod elevation;
pub mod payload;
pub mod logging;
pub mod disk_space;
pub mod rollback;
pub mod file_assoc;
pub mod process_detect;
pub mod pe_icon;
pub mod error;

pub use error::*;
