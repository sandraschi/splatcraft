//! Minimal host UI: inspect, convert, engine status. Every action is one
//! `dispatch` call, the same bus the CLI and MCP server use.

use eframe::{egui, egui_wgpu, wgpu};
use serde_json::{json, Value};
use splat_view::{LiveScene, View};
use splatcraft_core::{dispatch, error_json};

#[derive(Default)]
pub struct SplatApp {
    path: String,
    output: String,
    overwrite: bool,
    result: String,
    view: View,
    /// Splat count of the scene currently on the GPU, if any.
    loaded: Option<usize>,
    viewport_note: String,
}

/// Draws the uploaded [`LiveScene`] inside egui's own render pass. The scene
/// lives in egui-wgpu's callback resources, so it is shared with no one.
struct SplatCallback {
    view: View,
    size_px: [u32; 2],
}

impl egui_wgpu::CallbackTrait for SplatCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(scene) = resources.get_mut::<LiveScene>() {
            scene.set_view(queue, &self.view, self.size_px);
            scene.encode_compute(encoder);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(scene) = resources.get::<LiveScene>() {
            scene.draw(pass);
        }
    }
}

impl SplatApp {
    /// Load `self.path` and upload it to the GPU for the viewport.
    fn load_scene(&mut self, frame: &eframe::Frame) {
        let splats = match splatcraft_core::load_splats(&self.path) {
            Ok(s) => s,
            Err(e) => {
                self.viewport_note = format!("load failed: {e}");
                return;
            }
        };
        let Some(rs) = frame.wgpu_render_state() else {
            self.viewport_note = "the wgpu renderer is not active".into();
            return;
        };
        match LiveScene::new(&rs.device, &rs.queue, rs.target_format, &splats, 1.0) {
            Ok(scene) => {
                self.loaded = Some(scene.count());
                rs.renderer.write().callback_resources.insert(scene);
                self.view = View::default();
                self.viewport_note.clear();
            }
            Err(e) => self.viewport_note = format!("upload failed: {e}"),
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size().max(egui::vec2(64.0, 64.0));
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
        if self.loaded.is_none() {
            let note = if self.viewport_note.is_empty() {
                "enter an input path and press Load"
            } else {
                &self.viewport_note
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                note,
                egui::FontId::proportional(14.0),
                egui::Color32::GRAY,
            );
            return;
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            self.view.yaw_deg += d.x * 0.4;
            self.view.pitch_deg = (self.view.pitch_deg - d.y * 0.4).clamp(-89.0, 89.0);
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.view.distance_scale =
                    (self.view.distance_scale * (-scroll * 0.002).exp()).clamp(0.05, 20.0);
            }
        }
        if resp.double_clicked() {
            self.view = View::default();
        }
        let ppp = ui.pixels_per_point();
        let size_px = [
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        ];
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            SplatCallback {
                view: self.view,
                size_px,
            },
        ));
    }
}

fn pretty(r: splatcraft_core::Result<Value>) -> String {
    let v = r.unwrap_or_else(|e| error_json(&e));
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

impl eframe::App for SplatApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
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
                if ui.button("Load").clicked() {
                    self.load_scene(frame);
                }
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
            if !self.result.is_empty() {
                egui::CollapsingHeader::new("result")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                ui.monospace(&self.result);
                            });
                    });
            }
            if let Some(n) = self.loaded {
                ui.label(format!(
                    "{n} splats  |  drag: orbit, wheel: zoom, double-click: reset"
                ));
            }
            self.viewport(ui);
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
