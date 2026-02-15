// COLD PATH: Configuration error types. Used only at startup.

use std::path::PathBuf;

/// Domain error type for configuration loading, parsing, validation, and CLI argument handling.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("validation failed: {reason}")]
    Validation { reason: String },

    #[error("CLI argument error: {reason}")]
    CliError { reason: String },
}
