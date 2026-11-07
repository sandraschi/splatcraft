//! Error type for the Marble client.

use std::fmt;

#[derive(Debug)]
pub enum WorldsError {
    /// Missing or rejected API key.
    Auth(String),
    /// Transport failure (DNS, TLS, timeout).
    Network(String),
    /// The API answered with an error status.
    Api {
        status: u16,
        message: String,
    },
    Invalid(String),
    /// The spend cap would be exceeded.
    CapExceeded(String),
    Io(String),
}

pub type Result<T> = std::result::Result<T, WorldsError>;

impl fmt::Display for WorldsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldsError::Auth(m)
            | WorldsError::Network(m)
            | WorldsError::Invalid(m)
            | WorldsError::CapExceeded(m)
            | WorldsError::Io(m) => f.write_str(m),
            WorldsError::Api { status, message } => {
                write!(f, "World Labs API error {status}: {message}")
            }
        }
    }
}

impl std::error::Error for WorldsError {}

impl From<std::io::Error> for WorldsError {
    fn from(e: std::io::Error) -> Self {
        WorldsError::Io(e.to_string())
    }
}
