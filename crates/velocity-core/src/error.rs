use thiserror::Error;

/// Errors that can occur during core engine operations.
///
/// Each variant carries a descriptive message providing context about
/// what operation was being performed when the error occurred.
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

    #[error("Download error: {0}")]
    Download(String),

    #[error("Dependency installation failed: {0}")]
    DependencyFailed(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    /// Create an I/O error with context string.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        CoreError::Io(std::io::Error::new(
            source.kind(),
            format!("{}: {}", context.into(), source),
        ))
    }

    /// Create a compression error.
    pub fn compression(context: impl Into<String>, details: impl Into<String>) -> Self {
        CoreError::Compression(format!("{}: {}", context.into(), details.into()))
    }

    /// Create a registry error.
    pub fn registry(context: impl Into<String>, details: impl Into<String>) -> Self {
        CoreError::Registry(format!("{}: {}", context.into(), details.into()))
    }

    /// Create a COM error.
    pub fn com(context: impl Into<String>, details: impl Into<String>) -> Self {
        CoreError::Com(format!("{}: {}", context.into(), details.into()))
    }

    /// Create a download error.
    pub fn download(context: impl Into<String>, details: impl Into<String>) -> Self {
        CoreError::Download(format!("{}: {}", context.into(), details.into()))
    }

    /// Create a permission denied error.
    pub fn permission_denied(context: impl Into<String>, details: impl Into<String>) -> Self {
        CoreError::PermissionDenied(format!("{}: {}", context.into(), details.into()))
    }

    /// Create a generic error.
    pub fn other(context: impl Into<String>, details: impl Into<String>) -> Self {
        CoreError::Other(format!("{}: {}", context.into(), details.into()))
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
