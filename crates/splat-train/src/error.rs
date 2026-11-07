//! Error type for the training pipeline.

use std::fmt;

#[derive(Debug)]
pub enum TrainError {
    Invalid(String),
    /// A required external tool is missing or is a version we cannot drive.
    Missing(String),
    NotFound(String),
    Io(String),
}

pub type Result<T> = std::result::Result<T, TrainError>;

impl fmt::Display for TrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainError::Invalid(m)
            | TrainError::Missing(m)
            | TrainError::NotFound(m)
            | TrainError::Io(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for TrainError {}

impl From<std::io::Error> for TrainError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::NotFound {
            TrainError::NotFound(e.to_string())
        } else {
            TrainError::Io(e.to_string())
        }
    }
}
