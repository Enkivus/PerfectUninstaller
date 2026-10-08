use std::path::{Path, PathBuf};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("operation not supported on this platform: {0}")]
    Unsupported(&'static str),

    #[error("refusing to touch unsafe path: {0}")]
    UnsafePath(PathBuf),

    #[error("corrupt or unreadable metadata at {path}: {message}")]
    Metadata { path: PathBuf, message: String },

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io {
            path: PathBuf::from("<unknown>"),
            source,
        }
    }
}
