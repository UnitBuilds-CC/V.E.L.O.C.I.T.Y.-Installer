//! Velocity Installer — Core Engine
//!
//! Handles file extraction, registry operations, shortcut creation,
//! service management, environment variables, uninstaller generation,
//! disk space validation, rollback, logging, file associations,
//! process detection, HTTP downloading, and dependency management.

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
pub mod downloader;
pub mod dep_resolver;
pub mod dep_installer;
pub mod localization;
pub mod security;
pub mod arch_detect;
pub mod condition;
pub mod reboot;
pub mod error;

pub use error::*;
