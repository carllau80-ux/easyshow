use std::path::PathBuf;

/// Errors produced by easyshow-core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported image format: {0}")]
    UnsupportedFormat(String),
    #[error("failed to decode image: {0}")]
    Decode(String),
    #[cfg(feature = "heic")]
    #[error("HEIC support unavailable ({0})")]
    HeifUnavailable(String),
}

pub type Result<T> = std::result::Result<T, Error>;
