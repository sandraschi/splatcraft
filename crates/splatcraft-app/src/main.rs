//! splatcraft-app: L3 desktop host (eframe/egui) plus headless `--smoke`.
//!
//! The GUI calls `splatcraft_core::dispatch` in-process. No ports, no network.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

mod app;

fn open_arg(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--open" {
            return it.next().filter(|p| !p.is_empty()).cloned();
        }
        if let Some(rest) = a.strip_prefix("--open=") {
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("usage: splatcraft [--smoke] [--open PATH]");
        eprintln!("  --smoke      headless checks, exit 0 on success");
        eprintln!("  --open PATH  load a .ply or .spz into the viewport");
        return;
    }
    let open = open_arg(&args);
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
    let open_path = open.clone();
    let mut options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    // The splat viewer needs the adapter's real limits (large storage buffers),
    // not wgpu's conservative defaults.
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(c) = &mut options.wgpu_options.wgpu_setup {
        c.device_descriptor = std::sync::Arc::new(|adapter| eframe::wgpu::DeviceDescriptor {
            label: Some("splatcraft device"),
            required_limits: adapter.limits(),
            ..Default::default()
        });
    }
    let run = eframe::run_native(
        "splatcraft",
        options,
        Box::new(
            move |cc| -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>> {
                Ok(Box::new(app::SplatApp::new(cc, open_path.clone())))
            },
        ),
    );
    if let Err(e) = run {
        eprintln!("[splatcraft] failed to start: {e}");
        std::process::exit(1);
    }
}
