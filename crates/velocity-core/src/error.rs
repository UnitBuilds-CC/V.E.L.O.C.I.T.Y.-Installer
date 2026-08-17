use thiserror::Error;

/// Errors that can occur during core engine operations.
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("COM error: {0}")]
    Com(String),

    #[error("File not found in payload: {0}")]
    FileNotFound(String),

    #[error("Invalid payload format: {0}")]
    InvalidPayload(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Elevation required")]
    ElevationRequired,

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
