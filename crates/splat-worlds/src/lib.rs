//! World Labs Marble client for splatcraft.
//!
//! API shape taken from the working Python bridge in `worldlabs-mcp`:
//! base `https://api.worldlabs.ai/marble/v1`, header `WLT-Api-Key`,
//! `POST /worlds:generate`, `GET /operations/{id}`, `GET /credits`,
//! `POST /media-assets:prepare_upload`.
//!
//! Spending is guarded three ways: generation is a dry run unless the caller
//! passes `confirm_spend`, every submission is appended to a JSONL ledger, and
//! a cumulative credit cap (default 4500, three worlds) refuses further spend.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

pub mod assets;
pub mod client;
pub mod error;
pub mod ledger;
pub mod request;

pub use error::{Result, WorldsError};
