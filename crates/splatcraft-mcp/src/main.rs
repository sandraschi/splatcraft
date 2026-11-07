//! splatcraft CLI and MCP stdio server. stdout carries only JSON; logs go to stderr.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

mod mcp;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use splatcraft_core::{dispatch, error_json};

#[derive(Parser)]
#[command(
    name = "splatcraft-cli",
    version,
    about = "Gaussian splat toolkit (PLY, SPZ)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the MCP server over stdio.
    Mcp,
    /// Version, ops and supported formats.
    Status,
    /// Probe external engines (brush, ns-train, colmap, ffmpeg, splat-transform).
    Engines,
    /// Summarise a .ply or .spz file.
    Inspect {
        #[arg(long)]
        path: String,
    },
    /// Convert between .ply and .spz.
    Convert {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output: String,
        #[arg(long)]
        overwrite: bool,
    },
    /// Write synthetic random splats (for tests and smoke runs).
    Synth {
        #[arg(long)]
        path: String,
        #[arg(long, default_value_t = 1000)]
        count: u64,
        #[arg(long, default_value_t = 0)]
        sh_degree: u64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long)]
        overwrite: bool,
    },
}

fn run(op: &str, args: Value) -> i32 {
    match dispatch(op, &args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&error_json(&e)).unwrap_or_default()
            );
            1
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.cmd {
        Cmd::Mcp => {
            mcp::run_stdio();
            0
        }
        Cmd::Status => run("status", json!({})),
        Cmd::Engines => run("engine_status", json!({})),
        Cmd::Inspect { path } => run("inspect", json!({"path": path})),
        Cmd::Convert {
            input,
            output,
            overwrite,
        } => run(
            "convert",
            json!({"input": input, "output": output, "overwrite": overwrite}),
        ),
        Cmd::Synth {
            path,
            count,
            sh_degree,
            seed,
            overwrite,
        } => run(
            "synth",
            json!({"path": path, "count": count, "sh_degree": sh_degree, "seed": seed, "overwrite": overwrite}),
        ),
    };
    std::process::exit(code);
}
