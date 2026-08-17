//! Velocity Installer — Core Engine
//!
//! Handles file extraction, registry operations, shortcut creation,
//! service management, environment variables, uninstaller generation,
//! disk space validation, rollback, logging, file associations,
//! process detection, HTTP downloading, and dependency management.

pub mod arch_detect;
pub mod checksum;
pub mod component_tree;
pub mod condition;
pub mod dep_installer;
pub mod dep_resolver;
pub mod disk_space;
pub mod downloader;
pub mod elevation;
pub mod encryption;
pub mod env_vars;
pub mod error;
pub mod extract;
pub mod file_assoc;
pub mod installer_mutex;
pub mod localization;
pub mod logging;
pub mod payload;
pub mod pe_icon;
pub mod platform;
pub mod process_detect;
pub mod reboot;
pub mod registry;
pub mod rollback;
pub mod scripting;
pub mod security;
pub mod services;
pub mod shortcuts;
pub mod uninstaller;
pub mod updater;

pub use error::*;
