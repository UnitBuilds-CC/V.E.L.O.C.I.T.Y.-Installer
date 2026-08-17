//! Velocity Installer — Configuration Parser and Auto-Generator
//!
//! This crate handles parsing `velocity.toml` configuration files,
//! resolving path variables, and auto-generating configuration from
//! existing project structures.

mod manifest;
mod parser;
mod variables;
mod auto_gen;
mod error;

pub use manifest::*;
pub use parser::*;
pub use variables::*;
pub use auto_gen::*;
pub use error::*;
