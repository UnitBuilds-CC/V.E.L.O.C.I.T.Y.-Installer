//! Velocity Installer — Compiler
//!
//! Compiles a Velocity project (config + files) into a standalone .exe installer.

pub mod builder;
pub mod error;

pub use builder::*;
pub use error::*;
