//! Video and images to splat, as detached jobs.
//!
//! A job is a list of stages (ffmpeg, COLMAP, Brush, plus a few in-process
//! steps) planned up front, persisted as `job.json`, and run by a separate
//! supervisor process so it survives the caller. Programs are spawned from
//! argument vectors, never through a shell. Planning checks each tool's
//! `--help` for the flags it needs and refuses to guess when a version does
//! not match.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

pub mod error;
pub mod job;
pub mod locate;
pub mod plan;
pub mod probe;
pub mod supervisor;

pub use error::{Result, TrainError};
