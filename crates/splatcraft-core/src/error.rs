//! Core error type with a stable `error_type` string for front ends.

use std::fmt;

use serde_json::{json, Value};
use splat_formats::FormatError;

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    Invalid(String),
    /// Path escapes the configured root.
    Outside(String),
    Exists(String),
    Unsupported(String),
    Io(String),
    /// Missing or rejected API key.
    Auth(String),
    Network(String),
    /// Non-success reply from a remote API.
    Api(String),
    /// Cumulative spend cap would be exceeded.
    CapExceeded(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "not_found",
            Error::Invalid(_) => "invalid",
            Error::Outside(_) => "outside",
            Error::Exists(_) => "exists",
            Error::Unsupported(_) => "unsupported",
            Error::Io(_) => "io",
            Error::Auth(_) => "auth",
            Error::Network(_) => "network",
            Error::Api(_) => "api",
            Error::CapExceeded(_) => "cap_exceeded",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(m)
            | Error::Invalid(m)
            | Error::Outside(m)
            | Error::Exists(m)
            | Error::Unsupported(m)
            | Error::Io(m)
            | Error::Auth(m)
            | Error::Network(m)
            | Error::Api(m)
            | Error::CapExceeded(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(e.to_string())
        } else {
            Error::Io(e.to_string())
        }
    }
}

impl From<FormatError> for Error {
    fn from(e: FormatError) -> Self {
        match e {
            FormatError::Io(io) => io.into(),
            FormatError::Unsupported(m) => Error::Unsupported(m),
            FormatError::Invalid(m) | FormatError::Limit(m) => Error::Invalid(m),
        }
    }
}

pub fn error_json(e: &Error) -> Value {
    json!({"success": false, "error": e.to_string(), "error_type": e.error_type()})
}
