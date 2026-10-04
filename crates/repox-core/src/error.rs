use std::path::PathBuf;
use thiserror::Error;

/// Core error types for the `repox` engine.
#[derive(Error, Debug)]
pub enum RepoxError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to traverse filesystem: {0}")]
    Ignore(#[from] ignore::Error),

    #[error("File size {size} bytes exceeds limit of {limit} bytes: {path}")]
    FileSizeExceeded {
        path: PathBuf,
        size: u64,
        limit: u64,
    },

    #[error("File is binary or contains invalid UTF-8: {0}")]
    BinaryFile(PathBuf),

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Formatting error: {0}")]
    Format(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, RepoxError>;
