//! Error types for snapconfig.

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SnapconfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] simd_json::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("INI parse error: {0}")]
    IniParse(String),

    #[error("Serialization error: {0}")]
    Serialize(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Unknown format: {0}")]
    UnknownFormat(String),

    #[error("Invalid cache: {0}")]
    InvalidCache(String),
}

impl From<SnapconfigError> for PyErr {
    fn from(err: SnapconfigError) -> PyErr {
        match err {
            SnapconfigError::Io(e) => PyIOError::new_err(e.to_string()),
            SnapconfigError::FileNotFound(path) => {
                PyIOError::new_err(format!("File not found: {}", path))
            }
            _ => PyValueError::new_err(err.to_string()),
        }
    }
}

/// Result type alias for snapconfig operations.
pub type Result<T> = std::result::Result<T, SnapconfigError>;
