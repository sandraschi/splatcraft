//! Minimal host UI: inspect, convert, engine status. Every action is one
//! `dispatch` call, the same bus the CLI and MCP server use.

use eframe::egui;
use serde_json::{json, Value};
use splatcraft_core::{dispatch, error_json};

#[derive(Default)]
pub struct SplatApp {
    path: String,
    output: String,
    overwrite: bool,
    result: String,
}

fn pretty(r: splatcraft_core::Result<Value>) -> String {
    let v = r.unwrap_or_else(|e| error_json(&e));
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

impl eframe::App for SplatApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("splatcraft");
            ui.horizontal(|ui| {
                ui.label("input");
                ui.text_edit_singleline(&mut self.path);
            });
            ui.horizontal(|ui| {
                ui.label("output");
                ui.text_edit_singleline(&mut self.output);
                ui.checkbox(&mut self.overwrite, "overwrite");
            });
            ui.horizontal(|ui| {
                if ui.button("Inspect").clicked() {
                    self.result = pretty(dispatch("inspect", &json!({"path": self.path})));
                }
                if ui.button("Convert").clicked() {
                    self.result = pretty(dispatch(
                        "convert",
                        &json!({"input": self.path, "output": self.output, "overwrite": self.overwrite}),
                    ));
                }
                if ui.button("Engines").clicked() {
                    self.result = pretty(dispatch("engine_status", &json!({})));
                }
                if ui.button("Status").clicked() {
                    self.result = pretty(dispatch("status", &json!({})));
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.monospace(&self.result);
            });
        });
    }
}

/// Headless checks through the same bus the GUI uses.
pub fn run_smoke_checks() -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let st = dispatch("status", &json!({})).map_err(|e| e.to_string())?;
    lines.push(format!("status: version {}", st["version"]));
    let eng = dispatch("engine_status", &json!({})).map_err(|e| e.to_string())?;
    lines.push(format!("engines: {} of {} up", eng["up"], eng["total"]));
    let dir = std::env::temp_dir().join(format!("splatcraft-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ply = dir.join("s.ply").to_string_lossy().into_owned();
    let spz = dir.join("s.spz").to_string_lossy().into_owned();
    let run = (|| -> Result<(), String> {
        dispatch("synth", &json!({"path": ply, "count": 300, "sh_degree": 1}))
            .map_err(|e| e.to_string())?;
        dispatch("convert", &json!({"input": ply, "output": spz})).map_err(|e| e.to_string())?;
        let i = dispatch("inspect", &json!({"path": spz})).map_err(|e| e.to_string())?;
        if i["splats"]["count"] != 300 {
            return Err(format!("expected 300 splats, got {}", i["splats"]["count"]));
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    run?;
    lines.push("synth -> convert -> inspect: 300 splats ok".into());
    Ok(lines)
}
