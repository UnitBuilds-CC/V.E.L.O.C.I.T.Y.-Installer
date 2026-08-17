use thiserror::Error;

#[derive(Error, Debug)]
pub enum UiError {
    #[error("Window creation failed: {0}")]
    WindowCreation(String),

    #[error("User cancelled installation")]
    Cancelled,

    #[error("Win32 error: {0}")]
    Win32(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, UiError>;
