//! splatcraft L1. One entry point, [`dispatch`], used by the CLI, the MCP
//! server and the desktop app. Errors surface as `{success:false,error,error_type}`
//! via [`error_json`].
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

pub mod edit;
pub mod engines;
pub mod error;
pub mod ops;
pub mod paths;
pub mod render;

pub use error::{error_json, Error, Result};

use serde_json::Value;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// All operations the bus understands.
pub const OPS: &[&str] = &[
    "status",
    "engine_status",
    "inspect",
    "convert",
    "synth",
    "edit",
    "merge",
    "render",
];

/// Run `op` with JSON `args`. The only function front ends need.
pub fn dispatch(op: &str, args: &Value) -> Result<Value> {
    match op {
        "status" => Ok(ops::status()),
        "engine_status" => Ok(engines::engine_status()),
        "inspect" => ops::inspect(args),
        "convert" => ops::convert(args),
        "synth" => ops::synth(args),
        "edit" => edit::edit(args),
        "merge" => edit::merge(args),
        "render" => render::render(args),
        other => Err(Error::Invalid(format!(
            "unknown op '{other}' (known: {})",
            OPS.join(", ")
        ))),
    }
}
