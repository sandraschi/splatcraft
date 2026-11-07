//! Gaussian splat file formats for splatcraft.
//!
//! * [`ply`]: binary little-endian 3DGS PLY, read and write.
//! * [`spz`]: Niantic SPZ v2/v3 (gzip), read; v3 write. v1 and v4 (zstd) are
//!   rejected with a clear error, never guessed at.
//! * [`stats`]: honest summary numbers for a splat set.
//! * [`synth`]: deterministic synthetic splats for tests and recipes.
//!
//! Everything here is hostile-input safe: sizes are validated against the real
//! byte length before allocation, and no path panics.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

pub mod error;
pub mod model;
pub mod ply;
pub mod spz;
pub mod stats;
pub mod synth;

pub use error::{FormatError, Result};
pub use model::{rest_coeffs, Coords, Splats, MAX_SPLATS};
