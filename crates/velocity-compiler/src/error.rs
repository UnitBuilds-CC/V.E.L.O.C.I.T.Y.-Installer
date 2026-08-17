use thiserror::Error;

#[derive(Error, Debug)]
pub enum CompilerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(#[from] velocity_config::ConfigError),

    #[error("Core error: {0}")]
    Core(#[from] velocity_core::CoreError),

    #[error("No files found to package")]
    NoFilesFound,

    #[error("Runtime binary not found: {0}")]
    RuntimeNotFound(String),

    #[error("Build failed: {0}")]
    BuildFailed(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CompilerError>;
