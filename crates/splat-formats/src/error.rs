//! Error type for splat format handling.

use std::fmt;

#[derive(Debug)]
pub enum FormatError {
    /// Underlying I/O failure.
    Io(std::io::Error),
    /// The bytes are not a valid file of the claimed format.
    Invalid(String),
    /// A valid file that this build does not handle (yet).
    Unsupported(String),
    /// A size limit was exceeded (hostile or enormous input).
    Limit(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Io(e) => write!(f, "io error: {e}"),
            FormatError::Invalid(m) => write!(f, "invalid splat data: {m}"),
            FormatError::Unsupported(m) => write!(f, "unsupported: {m}"),
            FormatError::Limit(m) => write!(f, "limit exceeded: {m}"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<std::io::Error> for FormatError {
    fn from(e: std::io::Error) -> Self {
        FormatError::Io(e)
    }
}
