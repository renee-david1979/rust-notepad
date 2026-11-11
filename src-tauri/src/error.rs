//! Application error types.
//!
//! `AppError` is the internal error vocabulary; `CommandError` is the
//! serializable shape returned across the Tauri IPC boundary.

use std::{fmt, path::PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    MissingFile(PathBuf),
    PermissionDenied(PathBuf),
    InvalidEncoding(PathBuf),
    Validation(String),
    Plugin(String),
    Timeout(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::MissingFile(path) => write!(f, "file not found: {}", path.display()),
            AppError::PermissionDenied(path) => {
                write!(f, "permission denied: {}", path.display())
            }
            AppError::InvalidEncoding(path) => {
                write!(f, "could not decode file: {}", path.display())
            }
            AppError::Validation(msg) => write!(f, "{msg}"),
            AppError::Plugin(msg) => write!(f, "plugin error: {msg}"),
            AppError::Timeout(msg) => write!(f, "plugin timed out: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

/// Error shape returned to the frontend (Tauri requires `Serialize`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub message: String,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<AppError> for CommandError {
    fn from(err: AppError) -> Self {
        CommandError {
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        CommandError {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Validation(err.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(err: serde_json::Error) -> Self {
        CommandError {
            message: err.to_string(),
        }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;

/// Map an I/O error against a path into the internal error vocabulary.
pub fn io_error(path: &std::path::Path, err: std::io::Error) -> AppError {
    match err.kind() {
        std::io::ErrorKind::NotFound => AppError::MissingFile(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => AppError::PermissionDenied(path.to_path_buf()),
        _ => AppError::Validation(err.to_string()),
    }
}
