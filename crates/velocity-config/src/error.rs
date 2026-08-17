use thiserror::Error;

/// Errors that can occur during configuration parsing and processing.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read configuration file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse TOML configuration: {0}")]
    TomlParseError(#[from] toml::de::Error),

    #[error("Failed to serialize configuration: {0}")]
    TomlSerializeError(#[from] toml::ser::Error),

    #[error("Invalid path variable '{variable}' at position {position}")]
    InvalidVariable { variable: String, position: usize },

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid configuration: {0}")]
    Validation(String),

    #[error("Glob pattern error: {0}")]
    GlobError(#[from] glob::PatternError),

    #[error("File not found: {path}")]
    FileNotFound { path: String },
}

pub type Result<T> = std::result::Result<T, ConfigError>;
