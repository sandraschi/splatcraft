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
    /// Edit with a JSON step list, e.g. '[{"op":"crop","min":[-1,-1,-1],"max":[1,1,1]}]'.
    Edit {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output: String,
        /// JSON array of steps (see the splat_edit tool description).
        #[arg(long)]
        steps: String,
        #[arg(long)]
        overwrite: bool,
    },
    /// Render a PNG still on the GPU.
    Render {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output: String,
        #[arg(long, default_value_t = 960)]
        width: u32,
        #[arg(long, default_value_t = 540)]
        height: u32,
        #[arg(long, default_value_t = 0.0)]
        yaw_deg: f64,
        #[arg(long, default_value_t = 0.0)]
        pitch_deg: f64,
        #[arg(long)]
        overwrite: bool,
    },
    /// Merge several splat files into one.
    Merge {
        #[arg(long, num_args = 2.., required = true)]
        inputs: Vec<String>,
        #[arg(long)]
        output: String,
        #[arg(long)]
        overwrite: bool,
    },
    /// Marble world generation. Takes the splat_worlds arguments as JSON
    /// (inline or @file.json), e.g. '{"action":"ledger"}'. Generation is a
    /// dry run unless it contains "confirm_spend":true.
    Worlds {
        #[arg(long)]
        args: String,
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

/// `--steps` is inline JSON, or `@path` to read JSON from a file (avoids shell
/// quoting problems, notably PowerShell stripping inner double quotes).
fn read_steps(arg: &str) -> Result<Value, String> {
    let text = match arg.strip_prefix('@') {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?
        }
        None => arg.to_string(),
    };
    serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())
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
        Cmd::Edit {
            input,
            output,
            steps,
            overwrite,
        } => match read_steps(&steps) {
            Ok(steps) => run(
                "edit",
                json!({"input": input, "output": output, "steps": steps, "overwrite": overwrite}),
            ),
            Err(e) => {
                println!(
                    "{}",
                    json!({"success": false, "error": format!("--steps is not valid JSON ({e}); use @file.json to avoid shell quoting"), "error_type": "invalid"})
                );
                1
            }
        },
        Cmd::Render {
            input,
            output,
            width,
            height,
            yaw_deg,
            pitch_deg,
            overwrite,
        } => run(
            "render",
            json!({"input": input, "output": output, "width": width, "height": height,
                   "yaw_deg": yaw_deg, "pitch_deg": pitch_deg, "overwrite": overwrite}),
        ),
        Cmd::Merge {
            inputs,
            output,
            overwrite,
        } => run(
            "merge",
            json!({"inputs": inputs, "output": output, "overwrite": overwrite}),
        ),
        Cmd::Worlds { args } => match read_steps(&args) {
            Ok(v) => run("worlds", v),
            Err(e) => {
                println!(
                    "{}",
                    json!({"success": false, "error": format!("--args is not valid JSON ({e}); use @file.json to avoid shell quoting"), "error_type": "invalid"})
                );
                1
            }
        },
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
