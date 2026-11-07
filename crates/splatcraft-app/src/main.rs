//! splatcraft-app: L3 desktop host (eframe/egui) plus headless `--smoke`.
//!
//! The GUI calls `splatcraft_core::dispatch` in-process. No ports, no network.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

mod app;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("usage: splatcraft [--smoke]");
        eprintln!("  --smoke   headless checks, exit 0 on success");
        return;
    }
    if args.iter().any(|a| a == "--smoke") {
        match app::run_smoke_checks() {
            Ok(lines) => {
                for l in lines {
                    println!("{l}");
                }
                println!("smoke: PASS");
            }
            Err(e) => {
                eprintln!("[splatcraft] smoke FAILED: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let run = eframe::run_native(
        "splatcraft",
        eframe::NativeOptions::default(),
        Box::new(
            |_cc| -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>> {
                Ok(Box::new(app::SplatApp::default()))
            },
        ),
    );
    if let Err(e) = run {
        eprintln!("[splatcraft] failed to start: {e}");
        std::process::exit(1);
    }
}
