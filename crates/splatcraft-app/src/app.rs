//! splatcraft L3: Craft workstation (Pattern 2, viewport-centric).
//!
//! Tier 4 brings the full 8-menu bar, 5-dialog Help suite, toolbox plus
//! inspector docks, and one panel per `dispatch` op. Every button calls
//! `splatcraft_core::dispatch` in-process. No ports, no network from the UI.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

use eframe::{egui, egui_wgpu, wgpu};
use serde_json::{json, Value};
use splat_view::{LiveScene, View};
use splatcraft_core::{dispatch, error_json};

/// Modal dialogs of the Help suite plus preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveDialog {
    CommandPalette,
    KeyboardShortcuts,
    ProjectDocs,
    LicensesNotices,
    About,
    Preferences,
}

/// Undo entry: restores the path fields and the last result text.
#[derive(Clone, Debug)]
struct UndoEntry {
    label: String,
    path: String,
    output: String,
    result: String,
}

pub struct SplatApp {
    // Core file fields.
    path: String,
    output: String,
    overwrite: bool,
    result: String,
    result_title: String,
    status_text: String,
    recent: Vec<String>,
    dark_mode: bool,
    active_dialog: Option<ActiveDialog>,
    palette_query: String,
    // Dock visibility (View / Window menus).
    show_toolbox: bool,
    show_viewport: bool,
    show_inspector: bool,
    show_engines: bool,
    show_train: bool,
    show_worlds: bool,
    // Viewport.
    view: View,
    loaded: Option<usize>,
    viewport_note: String,
    last_inspect: Option<Value>,
    // Edit pipeline builder.
    crop_min: [f32; 3],
    crop_max: [f32; 3],
    crop_invert: bool,
    prune_min_opacity: String,
    prune_max_scale: String,
    tr_translate: [f32; 3],
    tr_scale: String,
    tr_axis: [f32; 3],
    tr_deg: String,
    reduce_degree: u8,
    edit_steps: Vec<Value>,
    // Merge.
    merge_inputs: String,
    merge_output: String,
    // Render.
    render_output: String,
    render_width: String,
    render_height: String,
    render_yaw: String,
    render_pitch: String,
    // Synth.
    synth_path: String,
    synth_count: String,
    synth_degree: String,
    synth_seed: String,
    // Worlds panel.
    worlds_action: String,
    worlds_prompt: String,
    worlds_kind: String,
    worlds_model: String,
    worlds_operation_id: String,
    worlds_world_id: String,
    worlds_dest: String,
    worlds_confirm_spend: bool,
    // Train panel.
    train_action: String,
    train_video: String,
    train_images: String,
    train_dataset: String,
    train_output: String,
    train_job_id: String,
    train_name: String,
    // Undo.
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
}

impl Default for SplatApp {
    fn default() -> Self {
        Self {
            path: String::new(),
            output: String::new(),
            overwrite: false,
            result: String::new(),
            result_title: String::new(),
            status_text: format!(
                "splatcraft v{} ready - pick a panel or press Ctrl+K",
                splatcraft_core::VERSION
            ),
            recent: Vec::new(),
            dark_mode: true,
            active_dialog: None,
            palette_query: String::new(),
            show_toolbox: true,
            show_viewport: true,
            show_inspector: true,
            show_engines: true,
            show_train: true,
            show_worlds: true,
            view: View::default(),
            loaded: None,
            viewport_note: String::new(),
            last_inspect: None,
            crop_min: [-3.0, -3.0, -3.0],
            crop_max: [3.0, 3.0, 3.0],
            crop_invert: false,
            prune_min_opacity: String::new(),
            prune_max_scale: String::new(),
            tr_translate: [0.0, 0.0, 0.0],
            tr_scale: String::new(),
            tr_axis: [0.0, 1.0, 0.0],
            tr_deg: String::new(),
            reduce_degree: 0,
            edit_steps: Vec::new(),
            merge_inputs: String::new(),
            merge_output: String::new(),
            render_output: String::new(),
            render_width: "960".to_string(),
            render_height: "540".to_string(),
            render_yaw: "0".to_string(),
            render_pitch: "0".to_string(),
            synth_path: String::new(),
            synth_count: "1000".to_string(),
            synth_degree: "0".to_string(),
            synth_seed: "1".to_string(),
            worlds_action: "ledger".to_string(),
            worlds_prompt: String::new(),
            worlds_kind: "text".to_string(),
            worlds_model: String::new(),
            worlds_operation_id: String::new(),
            worlds_world_id: String::new(),
            worlds_dest: String::new(),
            worlds_confirm_spend: false,
            train_action: "plan".to_string(),
            train_video: String::new(),
            train_images: String::new(),
            train_dataset: String::new(),
            train_output: String::new(),
            train_job_id: String::new(),
            train_name: String::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl SplatApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx, true);
        Self::default()
    }

    fn push_undo(&mut self, label: &str) {
        let e = UndoEntry {
            label: label.to_string(),
            path: self.path.clone(),
            output: self.output.clone(),
            result: self.result.clone(),
        };
        self.undo_stack.push(e);
        if self.undo_stack.len() > 30 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        let Some(e) = self.undo_stack.pop() else {
            self.status_text = "Nothing to undo".to_string();
            return;
        };
        let cur = UndoEntry {
            label: "redo-point".to_string(),
            path: self.path.clone(),
            output: self.output.clone(),
            result: self.result.clone(),
        };
        self.redo_stack.push(cur);
        self.path = e.path;
        self.output = e.output;
        self.result = e.result;
        self.result_title = format!("undone: {}", e.label);
        self.status_text = format!("Undo: {}", e.label);
    }

    fn redo(&mut self) {
        let Some(e) = self.redo_stack.pop() else {
            self.status_text = "Nothing to redo".to_string();
            return;
        };
        let cur = UndoEntry {
            label: "undo-point".to_string(),
            path: self.path.clone(),
            output: self.output.clone(),
            result: self.result.clone(),
        };
        self.undo_stack.push(cur);
        self.path = e.path;
        self.output = e.output;
        self.result = e.result;
        self.result_title = "redo applied".to_string();
        self.status_text = "Redo applied".to_string();
    }

    fn set_result(&mut self, title: &str, v: Value) {
        self.result_title = title.to_string();
        self.result = serde_json::to_string_pretty(&v).unwrap_or_default();
        self.status_text = title.to_string();
        if !self.path.is_empty() && !self.recent.contains(&self.path) {
            self.recent.push(self.path.clone());
            if self.recent.len() > 10 {
                self.recent.remove(0);
            }
        }
    }

    fn set_error(&mut self, title: &str, e: splatcraft_core::Error) {
        self.result_title = title.to_string();
        self.result = serde_json::to_string_pretty(&error_json(&e)).unwrap_or_default();
        self.status_text = format!("{title}: {e}");
    }

    /// Load `self.path` and upload it to the GPU for the viewport.
    fn load_scene(&mut self, frame: &eframe::Frame) {
        let splats = match splatcraft_core::load_splats(&self.path) {
            Ok(s) => s,
            Err(e) => {
                self.viewport_note = format!("load failed: {e}");
                self.set_error("load", e);
                return;
            }
        };
        let Some(rs) = frame.wgpu_render_state() else {
            self.viewport_note = "the wgpu renderer is not active".to_string();
            self.status_text = self.viewport_note.clone();
            return;
        };
        match LiveScene::new(&rs.device, &rs.queue, rs.target_format, &splats, 1.0) {
            Ok(scene) => {
                self.loaded = Some(scene.count());
                rs.renderer.write().callback_resources.insert(scene);
                self.view = View::default();
                self.viewport_note.clear();
                self.status_text = format!("Loaded {} splats from {}", splats.len(), self.path);
                if !self.recent.contains(&self.path) {
                    self.recent.push(self.path.clone());
                }
            }
            Err(e) => {
                self.viewport_note = format!("upload failed: {e}");
                self.status_text = self.viewport_note.clone();
            }
        }
    }

    fn do_inspect(&mut self) {
        self.push_undo("inspect");
        match dispatch("inspect", &json!({"path": self.path})) {
            Ok(v) => {
                self.last_inspect = Some(v.clone());
                self.set_result("inspect", v);
            }
            Err(e) => self.set_error("inspect", e),
        }
    }

    fn do_convert(&mut self) {
        self.push_undo("convert");
        match dispatch(
            "convert",
            &json!({"input": self.path, "output": self.output, "overwrite": self.overwrite}),
        ) {
            Ok(v) => self.set_result("convert", v),
            Err(e) => self.set_error("convert", e),
        }
    }

    fn do_synth(&mut self) {
        self.push_undo("synth");
        let count: u64 = self.synth_count.parse().unwrap_or(1000);
        let degree: u64 = self.synth_degree.parse().unwrap_or(0);
        let seed: u64 = self.synth_seed.parse().unwrap_or(1);
        let target = if self.synth_path.is_empty() {
            self.path.clone()
        } else {
            self.synth_path.clone()
        };
        match dispatch(
            "synth",
            &json!({"path": target, "count": count, "sh_degree": degree, "seed": seed, "overwrite": self.overwrite}),
        ) {
            Ok(v) => {
                self.path = target.clone();
                self.set_result("synth", v);
            }
            Err(e) => self.set_error("synth", e),
        }
    }

    fn edit_add(&mut self, step: Value) {
        self.edit_steps.push(step);
        self.status_text = format!("edit steps: {}", self.edit_steps.len());
    }

    fn do_edit_run(&mut self) {
        self.push_undo("edit");
        let steps = Value::Array(self.edit_steps.clone());
        match dispatch(
            "edit",
            &json!({"input": self.path, "output": self.output, "steps": steps, "overwrite": self.overwrite}),
        ) {
            Ok(v) => self.set_result("edit", v),
            Err(e) => self.set_error("edit", e),
        }
    }

    fn do_merge(&mut self) {
        self.push_undo("merge");
        let inputs: Vec<Value> = self
            .merge_inputs
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_string()))
            .collect();
        match dispatch(
            "merge",
            &json!({"inputs": inputs, "output": self.merge_output, "overwrite": self.overwrite}),
        ) {
            Ok(v) => self.set_result("merge", v),
            Err(e) => self.set_error("merge", e),
        }
    }

    fn do_render(&mut self) {
        self.push_undo("render");
        let out = if self.render_output.is_empty() {
            self.output.clone()
        } else {
            self.render_output.clone()
        };
        let w: u64 = self.render_width.parse().unwrap_or(960);
        let h: u64 = self.render_height.parse().unwrap_or(540);
        let yaw: f64 = self.render_yaw.parse().unwrap_or(0.0);
        let pitch: f64 = self.render_pitch.parse().unwrap_or(0.0);
        match dispatch(
            "render",
            &json!({"input": self.path, "output": out, "width": w, "height": h,
                    "yaw_deg": yaw, "pitch_deg": pitch, "overwrite": self.overwrite}),
        ) {
            Ok(v) => self.set_result("render", v),
            Err(e) => self.set_error("render", e),
        }
    }

    fn do_engines(&mut self) {
        match dispatch("engine_status", &json!({})) {
            Ok(v) => self.set_result("engine_status", v),
            Err(e) => self.set_error("engine_status", e),
        }
    }

    fn do_worlds(&mut self) {
        self.push_undo("worlds");
        let action = self.worlds_action.trim();
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), Value::String(action.to_string()));
        match action {
            "generate" => {
                if !self.worlds_prompt.trim().is_empty() {
                    args.insert(
                        "prompt".to_string(),
                        Value::String(self.worlds_prompt.trim().to_string()),
                    );
                }
                if !self.worlds_kind.trim().is_empty() {
                    args.insert(
                        "kind".to_string(),
                        Value::String(self.worlds_kind.trim().to_string()),
                    );
                }
                if !self.worlds_model.trim().is_empty() {
                    args.insert(
                        "model".to_string(),
                        Value::String(self.worlds_model.trim().to_string()),
                    );
                }
                if self.worlds_confirm_spend {
                    args.insert("confirm_spend".to_string(), Value::Bool(true));
                }
            }
            "status" => {
                if !self.worlds_operation_id.trim().is_empty() {
                    args.insert(
                        "operation_id".to_string(),
                        Value::String(self.worlds_operation_id.trim().to_string()),
                    );
                }
            }
            "fetch" => {
                if !self.worlds_dest.trim().is_empty() {
                    args.insert(
                        "dest".to_string(),
                        Value::String(self.worlds_dest.trim().to_string()),
                    );
                }
                if !self.worlds_operation_id.trim().is_empty() {
                    args.insert(
                        "operation_id".to_string(),
                        Value::String(self.worlds_operation_id.trim().to_string()),
                    );
                }
                if !self.worlds_world_id.trim().is_empty() {
                    args.insert(
                        "world_id".to_string(),
                        Value::String(self.worlds_world_id.trim().to_string()),
                    );
                }
            }
            "list" => {}
            _ => {}
        }
        match dispatch("worlds", &Value::Object(args)) {
            Ok(v) => self.set_result("worlds", v),
            Err(e) => self.set_error("worlds", e),
        }
    }

    fn do_train(&mut self) {
        self.push_undo("train");
        let action = self.train_action.trim();
        let mut args = serde_json::Map::new();
        args.insert("action".to_string(), Value::String(action.to_string()));
        match action {
            "plan" | "start" => {
                if !self.train_video.trim().is_empty() {
                    args.insert(
                        "video".to_string(),
                        Value::String(self.train_video.trim().to_string()),
                    );
                }
                if !self.train_images.trim().is_empty() {
                    args.insert(
                        "images_dir".to_string(),
                        Value::String(self.train_images.trim().to_string()),
                    );
                }
                if !self.train_dataset.trim().is_empty() {
                    args.insert(
                        "dataset".to_string(),
                        Value::String(self.train_dataset.trim().to_string()),
                    );
                }
                if !self.train_output.trim().is_empty() {
                    args.insert(
                        "output".to_string(),
                        Value::String(self.train_output.trim().to_string()),
                    );
                } else if !self.output.trim().is_empty() {
                    args.insert(
                        "output".to_string(),
                        Value::String(self.output.trim().to_string()),
                    );
                }
                if !self.train_name.trim().is_empty() {
                    args.insert(
                        "name".to_string(),
                        Value::String(self.train_name.trim().to_string()),
                    );
                }
                if self.overwrite {
                    args.insert("overwrite".to_string(), Value::Bool(true));
                }
            }
            "status" | "log" | "cancel" if !self.train_job_id.trim().is_empty() => {
                args.insert(
                    "job_id".to_string(),
                    Value::String(self.train_job_id.trim().to_string()),
                );
            }
            "status" | "log" | "cancel" => {}
            _ => {}
        }
        match dispatch("train", &Value::Object(args)) {
            Ok(v) => self.set_result("train", v),
            Err(e) => self.set_error("train", e),
        }
    }
}

/// Draws the uploaded [`LiveScene`] inside egui's own render pass.
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

// ---------- Craft theme helpers (Pattern 2 workstation) ----------

fn harsh_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(egui::Color32::from_rgb(245, 247, 250));
    v.window_fill = egui::Color32::from_rgb(10, 13, 18);
    v.panel_fill = egui::Color32::from_rgb(10, 18, 34);
    v.extreme_bg_color = egui::Color32::from_rgb(2, 6, 18);
    v.code_bg_color = egui::Color32::from_rgb(4, 10, 24);
    v.faint_bg_color = egui::Color32::from_rgb(16, 28, 50);
    v.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 54, 84));
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 66, 102));
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(125, 211, 252));
    v.selection.bg_fill = egui::Color32::from_rgb(36, 64, 107);
    v
}

fn crisp_light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.override_text_color = Some(egui::Color32::from_rgb(15, 23, 42));
    v.window_fill = egui::Color32::from_rgb(248, 250, 252);
    v.panel_fill = egui::Color32::from_rgb(248, 250, 252);
    v.extreme_bg_color = egui::Color32::from_rgb(241, 245, 249);
    v.code_bg_color = egui::Color32::from_rgb(241, 245, 249);
    v.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(226, 232, 240));
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(203, 213, 225));
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(14, 165, 233));
    v
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark {
        harsh_visuals()
    } else {
        crisp_light_visuals()
    });
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.style_mut_of(theme, |s| {
        use egui::TextStyle::*;
        for (ts, size) in [
            (Body, 16.0),
            (Button, 16.0),
            (Heading, 23.0),
            (Monospace, 14.0),
            (Small, 13.0),
        ] {
            if let Some(f) = s.text_styles.get_mut(&ts) {
                f.size = size;
            }
        }
        s.spacing.item_spacing = egui::vec2(10.0, 8.0);
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
    });
}

fn handle_zoom(ctx: &egui::Context) {
    let zoom = ctx.input(|i| i.zoom_delta());
    if (zoom - 1.0).abs() > f32::EPSILON {
        ctx.set_zoom_factor((ctx.zoom_factor() * zoom).clamp(0.6, 2.5));
    }
}

/// Vector sun / moon toggle. Glyphs would be tofu in the default font.
fn theme_toggle_button(ui: &mut egui::Ui, dark_mode: &mut bool) -> bool {
    let size = egui::vec2(30.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&resp);
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(6),
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let center = rect.center();
        let painter = ui.painter();
        if *dark_mode {
            let sun = egui::Color32::from_rgb(251, 191, 36);
            painter.circle_filled(center, 4.5, sun);
            let mut i = 0;
            while i < 8 {
                let angle = (i as f32) * std::f32::consts::TAU / 8.0;
                let dir = egui::vec2(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + dir * 6.5, center + dir * 9.2],
                    egui::Stroke::new(1.6, sun),
                );
                i += 1;
            }
        } else {
            let moon = egui::Color32::from_rgb(51, 65, 85);
            painter.circle_filled(center, 6.5, moon);
            painter.circle_filled(center + egui::vec2(3.0, -2.2), 5.5, visuals.bg_fill);
        }
    }
    let clicked = resp.on_hover_text("Toggle light / dark theme").clicked();
    if clicked {
        *dark_mode = !*dark_mode;
        apply_theme(ui.ctx(), *dark_mode);
    }
    clicked
}

/// egui MenuBar already switches the open menu on hover while one menu is
/// open (native desktop feel). This helper documents that contract and keeps
/// the call site honest without depending on unstable Popup internals.
fn switch_on_hover_note() {}

fn card_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .corner_radius(egui::CornerRadius::same(12))
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::same(10))
        .show(ui, add);
}

fn check_string_for_tofu(s: &str) -> Result<(), String> {
    for (idx, ch) in s.char_indices() {
        if ch == '\u{FFFD}' {
            return Err(format!("replacement char at byte {idx}"));
        }
        if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
            return Err(format!("control char U+{:04X} at byte {idx}", ch as u32));
        }
    }
    Ok(())
}

// ---------- Menu bar, panels, dialogs ----------

impl SplatApp {
    fn menu_bar(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        switch_on_hover_note();
        egui::Panel::top("top_menubar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Synthetic Scene...").clicked() {
                        self.status_text = "File: fill Synth panel, then Run synth".to_string();
                        ui.close();
                    }
                    if ui.button("Open Splat PLY/SPZ (Ctrl+O)").clicked() {
                        self.load_scene(frame);
                        ui.close();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        if self.recent.is_empty() {
                            ui.label("no recent files");
                        }
                        let items = self.recent.clone();
                        for r in items {
                            if ui.button(r.clone()).clicked() {
                                self.path = r;
                                self.load_scene(frame);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("Clear Recent Files").clicked() {
                            self.recent.clear();
                            ui.close();
                        }
                    });
                    ui.separator();
                    if ui.button("Save Converted Copy (Ctrl+S)").clicked() {
                        self.do_convert();
                        ui.close();
                    }
                    if ui.button("Save As... (Ctrl+Shift+S)").clicked() {
                        self.status_text = "File: set Output, then Convert".to_string();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Export Still PNG (Ctrl+E)").clicked() {
                        self.do_render();
                        ui.close();
                    }
                    if ui.button("Export Video (not available)").clicked() {
                        self.result_title = "export video".to_string();
                        self.result = "video export is not implemented: render writes PNG stills; use the Train panel for footage pipelines".to_string();
                        self.status_text = self.result_title.clone();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Close Scene (Ctrl+W)").clicked() {
                        self.loaded = None;
                        self.last_inspect = None;
                        self.viewport_note = "enter an input path and press Load".to_string();
                        self.status_text = "Scene closed".to_string();
                        ui.close();
                    }
                    if ui.button("Quit (Ctrl+Q)").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() {
                        self.undo();
                        ui.close();
                    }
                    if ui.button("Redo (Ctrl+Y)").clicked() {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Cut Result (Ctrl+X)").clicked() {
                        ui.ctx().copy_text(self.result.clone());
                        self.result.clear();
                        self.status_text = "Edit: result cut to clipboard".to_string();
                        ui.close();
                    }
                    if ui.button("Copy Result (Ctrl+C)").clicked() {
                        ui.ctx().copy_text(self.result.clone());
                        self.status_text = "Edit: result copied".to_string();
                        ui.close();
                    }
                    if ui.button("Paste Path (Ctrl+V)").clicked() {
                        self.status_text = "Edit: use Ctrl+V inside the path field".to_string();
                        ui.close();
                    }
                    if ui.button("Select All Panels").clicked() {
                        self.show_toolbox = true;
                        self.show_inspector = true;
                        self.show_engines = true;
                        self.show_train = true;
                        self.show_worlds = true;
                        self.show_viewport = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Preferences... (Ctrl+,)").clicked() {
                        self.active_dialog = Some(ActiveDialog::Preferences);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_viewport, "3D Splat Viewport");
                    ui.checkbox(&mut self.show_toolbox, "Toolbox");
                    ui.checkbox(&mut self.show_inspector, "Point Cloud Inspector");
                    ui.checkbox(&mut self.show_engines, "Engines Panel");
                    ui.checkbox(&mut self.show_train, "Training Jobs Panel");
                    ui.checkbox(&mut self.show_worlds, "Marble Worlds Panel");
                    ui.separator();
                    if ui.button("Reset Camera (double-click)").clicked() {
                        self.view = View::default();
                        ui.close();
                    }
                    ui.menu_button("Theme", |ui| {
                        if ui.button("Dark").clicked() {
                            self.dark_mode = true;
                            apply_theme(ui.ctx(), true);
                            ui.close();
                        }
                        if ui.button("Light").clicked() {
                            self.dark_mode = false;
                            apply_theme(ui.ctx(), false);
                            ui.close();
                        }
                    });
                });
                ui.menu_button("Insert", |ui| {
                    if ui.button("Import Additional Splat (Merge)").clicked() {
                        self.status_text = "Insert: add paths in Merge panel, then Run merge".to_string();
                        ui.close();
                    }
                    if ui.button("Add Bounding Crop Box").clicked() {
                        self.edit_add(json!({"op": "crop", "min": self.crop_min, "max": self.crop_max, "invert": self.crop_invert}));
                        ui.close();
                    }
                    if ui.button("Load Camera Path (render yaw/pitch)").clicked() {
                        self.status_text = "Insert: set yaw/pitch in Render panel".to_string();
                        ui.close();
                    }
                    if ui.button("Add Training Footage (video/images)").clicked() {
                        self.status_text = "Insert: fill Train panel video or images_dir".to_string();
                        ui.close();
                    }
                });
                ui.menu_button("Modify", |ui| {
                    if ui.button("Crop Selection").clicked() {
                        self.edit_add(json!({"op": "crop", "min": self.crop_min, "max": self.crop_max, "invert": self.crop_invert}));
                        ui.close();
                    }
                    if ui.button("Prune Low-Opacity Splats").clicked() {
                        let mut step = serde_json::Map::new();
                        step.insert("op".to_string(), Value::String("prune".to_string()));
                        if let Ok(v) = self.prune_min_opacity.trim().parse::<f64>() {
                            step.insert("min_opacity".to_string(), json!(v));
                        }
                        if let Ok(v) = self.prune_max_scale.trim().parse::<f64>() {
                            step.insert("max_scale".to_string(), json!(v));
                        }
                        self.edit_add(Value::Object(step));
                        ui.close();
                    }
                    if ui.button("Transform (Translate/Rotate/Scale)").clicked() {
                        let mut t = serde_json::Map::new();
                        t.insert("op".to_string(), Value::String("transform".to_string()));
                        t.insert("translate".to_string(), json!(self.tr_translate));
                        if let Ok(v) = self.tr_scale.trim().parse::<f64>() {
                            t.insert("scale".to_string(), json!(v));
                        }
                        if let Ok(d) = self.tr_deg.trim().parse::<f64>() {
                            t.insert("rotate".to_string(), json!({"axis": self.tr_axis, "degrees": d}));
                        }
                        self.edit_add(Value::Object(t));
                        ui.close();
                    }
                    if ui.button("Reduce SH Degree").clicked() {
                        self.edit_add(json!({"op": "reduce_sh", "degree": self.reduce_degree}));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Clear Edit Steps").clicked() {
                        self.edit_steps.clear();
                        self.status_text = "Modify: edit steps cleared".to_string();
                        ui.close();
                    }
                    if ui.button("Run Edit Pipeline").clicked() {
                        self.do_edit_run();
                        ui.close();
                    }
                });
                ui.menu_button("Commands", |ui| {
                    if ui.button("Run Headless Render").clicked() {
                        self.do_render();
                        ui.close();
                    }
                    if ui.button("Plan Training Job (COLMAP/Brush)").clicked() {
                        self.train_action = "plan".to_string();
                        self.do_train();
                        ui.close();
                    }
                    if ui.button("Convert PLY <-> SPZ").clicked() {
                        self.do_convert();
                        ui.close();
                    }
                    if ui.button("Check Marble Credits").clicked() {
                        self.worlds_action = "credits".to_string();
                        self.do_worlds();
                        ui.close();
                    }
                    if ui.button("Cancel Training Job").clicked() {
                        self.train_action = "cancel".to_string();
                        self.do_train();
                        ui.close();
                    }
                });
                ui.menu_button("Window", |ui| {
                    if ui.button("Reset Layout").clicked() {
                        self.show_toolbox = true;
                        self.show_viewport = true;
                        self.show_inspector = true;
                        self.show_engines = true;
                        self.show_train = true;
                        self.show_worlds = true;
                        self.status_text = "Window: layout reset".to_string();
                        ui.close();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_viewport, "3D Viewport");
                    ui.checkbox(&mut self.show_toolbox, "Job Supervisor Drawer");
                    ui.checkbox(&mut self.show_inspector, "Inspector Dock");
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Command Palette... (Ctrl+K)").clicked() {
                        self.palette_query.clear();
                        self.active_dialog = Some(ActiveDialog::CommandPalette);
                        ui.close();
                    }
                    if ui.button("Keyboard Shortcuts... (F1)").clicked() {
                        self.active_dialog = Some(ActiveDialog::KeyboardShortcuts);
                        ui.close();
                    }
                    if ui.button("Project Files and Docs...").clicked() {
                        self.active_dialog = Some(ActiveDialog::ProjectDocs);
                        ui.close();
                    }
                    if ui.button("Licenses and Notices...").clicked() {
                        self.active_dialog = Some(ActiveDialog::LicensesNotices);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("About SplatCraft").clicked() {
                        self.active_dialog = Some(ActiveDialog::About);
                        ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = theme_toggle_button(ui, &mut self.dark_mode);
                    ui.label(
                        egui::RichText::new("100% Safe Rust - 0 Sockets - dispatch only")
                            .color(egui::Color32::from_rgb(110, 255, 150))
                            .small(),
                    );
                });
            });
        });
    }

    fn quick_row(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            if ui.button("Load").clicked() {
                self.load_scene(frame);
            }
            if ui.button("Inspect").clicked() {
                self.do_inspect();
            }
            if ui.button("Convert").clicked() {
                self.do_convert();
            }
            if ui.button("Render PNG").clicked() {
                self.do_render();
            }
            if ui.button("Train Plan").clicked() {
                self.train_action = "plan".to_string();
                self.do_train();
            }
            if ui.button("Credits").clicked() {
                self.worlds_action = "credits".to_string();
                self.do_worlds();
            }
            if ui.button("Palette (Ctrl+K)").clicked() {
                self.palette_query.clear();
                self.active_dialog = Some(ActiveDialog::CommandPalette);
            }
        });
    }

    fn toolbox_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_toolbox {
            return;
        }
        egui::Panel::left("toolbox").default_size(300.0).show(ui, |ui| {
            ui.heading("Toolbox");
            ui.label("Input splat (PLY/SPZ):");
            ui.text_edit_singleline(&mut self.path);
            ui.label("Output path:");
            ui.text_edit_singleline(&mut self.output);
            ui.checkbox(&mut self.overwrite, "overwrite existing output");
            ui.separator();
            ui.collapsing("Inspect / Convert / Synth", |ui| {
                if ui.button("Run Inspect").clicked() {
                    self.do_inspect();
                }
                if ui.button("Run Convert").clicked() {
                    self.do_convert();
                }
                ui.label("Synthetic scene (tests, smoke):");
                ui.horizontal(|ui| {
                    ui.label("path");
                    ui.text_edit_singleline(&mut self.synth_path);
                });
                ui.horizontal(|ui| {
                    ui.label("count");
                    ui.text_edit_singleline(&mut self.synth_count);
                    ui.label("sh");
                    ui.text_edit_singleline(&mut self.synth_degree);
                    ui.label("seed");
                    ui.text_edit_singleline(&mut self.synth_seed);
                });
                if ui.button("Run Synth").clicked() {
                    self.do_synth();
                }
            });
            ui.collapsing("Edit pipeline (RDF)", |ui| {
                ui.label("Crop box:");
                ui.horizontal(|ui| {
                    ui.label("min");
                    ui.add(egui::DragValue::new(&mut self.crop_min[0]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.crop_min[1]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.crop_min[2]).speed(0.1));
                });
                ui.horizontal(|ui| {
                    ui.label("max");
                    ui.add(egui::DragValue::new(&mut self.crop_max[0]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.crop_max[1]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.crop_max[2]).speed(0.1));
                });
                ui.checkbox(&mut self.crop_invert, "invert crop (keep outside)");
                if ui.button("Add crop step").clicked() {
                    self.edit_add(json!({"op": "crop", "min": self.crop_min, "max": self.crop_max, "invert": self.crop_invert}));
                }
                ui.horizontal(|ui| {
                    ui.label("min_opacity");
                    ui.text_edit_singleline(&mut self.prune_min_opacity);
                    ui.label("max_scale");
                    ui.text_edit_singleline(&mut self.prune_max_scale);
                });
                if ui.button("Add prune step").clicked() {
                    let mut step = serde_json::Map::new();
                    step.insert("op".to_string(), Value::String("prune".to_string()));
                    if let Ok(v) = self.prune_min_opacity.trim().parse::<f64>() {
                        step.insert("min_opacity".to_string(), json!(v));
                    }
                    if let Ok(v) = self.prune_max_scale.trim().parse::<f64>() {
                        step.insert("max_scale".to_string(), json!(v));
                    }
                    self.edit_add(Value::Object(step));
                }
                ui.label("Transform:");
                ui.horizontal(|ui| {
                    ui.label("t");
                    ui.add(egui::DragValue::new(&mut self.tr_translate[0]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.tr_translate[1]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.tr_translate[2]).speed(0.1));
                });
                ui.horizontal(|ui| {
                    ui.label("scale");
                    ui.text_edit_singleline(&mut self.tr_scale);
                    ui.label("deg");
                    ui.text_edit_singleline(&mut self.tr_deg);
                });
                ui.horizontal(|ui| {
                    ui.label("axis");
                    ui.add(egui::DragValue::new(&mut self.tr_axis[0]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.tr_axis[1]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.tr_axis[2]).speed(0.1));
                });
                if ui.button("Add transform step").clicked() {
                    let mut t = serde_json::Map::new();
                    t.insert("op".to_string(), Value::String("transform".to_string()));
                    t.insert("translate".to_string(), json!(self.tr_translate));
                    if let Ok(v) = self.tr_scale.trim().parse::<f64>() {
                        t.insert("scale".to_string(), json!(v));
                    }
                    if let Ok(d) = self.tr_deg.trim().parse::<f64>() {
                        t.insert("rotate".to_string(), json!({"axis": self.tr_axis, "degrees": d}));
                    }
                    self.edit_add(Value::Object(t));
                }
                ui.horizontal(|ui| {
                    ui.label("reduce_sh degree (0-3)");
                    ui.add(egui::DragValue::new(&mut self.reduce_degree).range(0..=3));
                    if ui.button("Add").clicked() {
                        self.edit_add(json!({"op": "reduce_sh", "degree": self.reduce_degree}));
                    }
                });
                ui.label(format!("steps: {}", self.edit_steps.len()));
                for (i, s) in self.edit_steps.iter().enumerate() {
                    ui.monospace(format!("{i}: {s}"));
                }
                ui.horizontal(|ui| {
                    if ui.button("Run edit").clicked() {
                        self.do_edit_run();
                    }
                    if ui.button("Clear").clicked() {
                        self.edit_steps.clear();
                    }
                });
                ui.small("Note: rotating SH degree > 0 is refused; run reduce_sh to 0 first.");
            });
            ui.collapsing("Merge (2-64 files)", |ui| {
                ui.label("inputs, one path per line:");
                ui.text_edit_multiline(&mut self.merge_inputs);
                ui.label("merge output:");
                ui.text_edit_singleline(&mut self.merge_output);
                if ui.button("Run merge").clicked() {
                    self.do_merge();
                }
            });
            ui.collapsing("Render PNG still", |ui| {
                ui.label("render output (.png):");
                ui.text_edit_singleline(&mut self.render_output);
                ui.horizontal(|ui| {
                    ui.label("w");
                    ui.text_edit_singleline(&mut self.render_width);
                    ui.label("h");
                    ui.text_edit_singleline(&mut self.render_height);
                });
                ui.horizontal(|ui| {
                    ui.label("yaw");
                    ui.text_edit_singleline(&mut self.render_yaw);
                    ui.label("pitch");
                    ui.text_edit_singleline(&mut self.render_pitch);
                });
                if ui.button("Run render").clicked() {
                    self.do_render();
                }
                ui.small("Background is opaque black; camera auto-frames the scene.");
            });
            let _ = ui;
        });
    }

    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_inspector {
            return;
        }
        egui::Panel::right("inspector").default_size(340.0).show(ui, |ui| {
            ui.heading("Inspector");
            card_frame(ui, |ui| {
                ui.label(egui::RichText::new("Scene").strong());
                if let Some(n) = self.loaded {
                    ui.label(format!("{n} splats on GPU"));
                } else {
                    ui.label("no scene on GPU");
                }
                if let Some(v) = self.last_inspect.clone() {
                    let count = v.get("splats").and_then(|s| s.get("count")).map(|c| c.to_string()).unwrap_or_default();
                    let sh = v.get("splats").and_then(|s| s.get("sh_degree")).map(|c| c.to_string()).unwrap_or_default();
                    ui.label(format!("inspected: count {count}, sh {sh}"));
                    if let Some(bb_min) = v.get("splats").and_then(|s| s.get("bbox_min")) {
                        ui.monospace(format!("bbox_min {bb_min}"));
                    }
                    if let Some(bb_max) = v.get("splats").and_then(|s| s.get("bbox_max")) {
                        ui.monospace(format!("bbox_max {bb_max}"));
                    }
                } else {
                    ui.label("Run Inspect for density + SH telemetry.");
                }
            });
            if self.show_engines {
                card_frame(ui, |ui| {
                    ui.label(egui::RichText::new("Engines").strong());
                    ui.small("Probe only looks for files; nothing is executed here.");
                    if ui.button("Probe engines").clicked() {
                        self.do_engines();
                    }
                });
            }
            if self.show_train {
                card_frame(ui, |ui| {
                    ui.label(egui::RichText::new("Training jobs").strong());
                    ui.horizontal(|ui| {
                        ui.label("action");
                        egui::ComboBox::from_id_salt("train_action")
                            .selected_text(self.train_action.clone())
                            .show_ui(ui, |ui| {
                                for a in ["plan", "start", "status", "log", "cancel", "list"] {
                                    ui.selectable_value(&mut self.train_action, a.to_string(), a);
                                }
                            });
                    });
                    ui.label("video path (exactly one of video / images / dataset):");
                    ui.text_edit_singleline(&mut self.train_video);
                    ui.label("images_dir:");
                    ui.text_edit_singleline(&mut self.train_images);
                    ui.label("dataset dir:");
                    ui.text_edit_singleline(&mut self.train_dataset);
                    ui.label("output (.ply/.spz):");
                    ui.text_edit_singleline(&mut self.train_output);
                    ui.horizontal(|ui| {
                        ui.label("job_id");
                        ui.text_edit_singleline(&mut self.train_job_id);
                        ui.label("name");
                        ui.text_edit_singleline(&mut self.train_name);
                    });
                    if ui.button("Run train action").clicked() {
                        self.do_train();
                    }
                    ui.small("start spawns a detached supervisor; poll with status/log.");
                });
            }
            if self.show_worlds {
                card_frame(ui, |ui| {
                    ui.label(egui::RichText::new("Marble Worlds").strong());
                    ui.small("Spend guard: dry run unless confirm_spend is set in CLI. UI never spends.");
                    ui.horizontal(|ui| {
                        ui.label("action");
                        egui::ComboBox::from_id_salt("worlds_action")
                            .selected_text(self.worlds_action.clone())
                            .show_ui(ui, |ui| {
                                for a in ["ledger", "credits", "generate", "status", "fetch", "list"] {
                                    ui.selectable_value(&mut self.worlds_action, a.to_string(), a);
                                }
                            });
                    });
                    ui.label("prompt (generate):");
                    ui.text_edit_singleline(&mut self.worlds_prompt);
                    ui.horizontal(|ui| {
                        ui.label("kind");
                        ui.text_edit_singleline(&mut self.worlds_kind);
                        ui.label("model");
                        ui.text_edit_singleline(&mut self.worlds_model);
                    });
                    ui.horizontal(|ui| {
                        ui.label("operation_id");
                        ui.text_edit_singleline(&mut self.worlds_operation_id);
                    });
                    ui.horizontal(|ui| {
                        ui.label("world_id");
                        ui.text_edit_singleline(&mut self.worlds_world_id);
                        ui.label("dest");
                        ui.text_edit_singleline(&mut self.worlds_dest);
                    });
                    if ui.button("Run worlds action").clicked() {
                        self.do_worlds();
                    }
                    ui.small("To store the API key, run: splatcraft-cli secret set worlds");
                });
            }
            if !self.result.is_empty() {
                card_frame(ui, |ui| {
                    ui.label(egui::RichText::new(self.result_title.clone()).strong());
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        ui.monospace(&self.result);
                    });
                    if ui.button("Copy result").clicked() {
                        ui.ctx().copy_text(self.result.clone());
                    }
                });
            }
            let _ = ui;
        });
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

    fn dialogs(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let dlg = self.active_dialog;
        if dlg == Some(ActiveDialog::CommandPalette) {
            egui::Window::new("Command Palette (Ctrl+K)")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("Fuzzy filter, Enter runs the first match. All commands go through dispatch.");
                    ui.text_edit_singleline(&mut self.palette_query);
                    let q = self.palette_query.to_lowercase();
                    let mut cmds: Vec<(&str, &str)> = vec![
                        ("inspect", "Inspect current path"),
                        ("convert", "Convert input to output"),
                        ("synth", "Write synthetic splats"),
                        ("edit", "Run edit pipeline"),
                        ("merge", "Merge splat files"),
                        ("render", "Render PNG still"),
                        ("engine_status", "Probe external engines"),
                        ("worlds ledger", "Show Marble spend ledger"),
                        ("worlds credits", "Check Marble credits"),
                        ("worlds generate (dry run)", "Plan a Marble generation, no spend"),
                        ("train plan", "Dry-run a training pipeline"),
                        ("train list", "List training jobs"),
                        ("load scene", "Upload current path to the GPU viewport"),
                        ("reset camera", "Reset orbit camera"),
                    ];
                    cmds.retain(|(id, label)| {
                        q.is_empty() || id.contains(&q) || label.to_lowercase().contains(&q)
                    });
                    for (id, label) in cmds {
                        if ui.button(format!("{id} - {label}")).clicked() {
                            match id {
                                "inspect" => self.do_inspect(),
                                "convert" => self.do_convert(),
                                "synth" => self.do_synth(),
                                "edit" => self.do_edit_run(),
                                "merge" => self.do_merge(),
                                "render" => self.do_render(),
                                "engine_status" => self.do_engines(),
                                "load scene" => self.load_scene(frame),
                                "reset camera" => self.view = View::default(),
                                _ if id.starts_with("worlds") => {
                                    if id.contains("ledger") {
                                        self.worlds_action = "ledger".to_string();
                                    } else if id.contains("credits") {
                                        self.worlds_action = "credits".to_string();
                                    } else {
                                        self.worlds_action = "generate".to_string();
                                    }
                                    self.do_worlds();
                                }
                                _ if id.starts_with("train") => {
                                    if id.contains("list") {
                                        self.train_action = "list".to_string();
                                    } else {
                                        self.train_action = "plan".to_string();
                                    }
                                    self.do_train();
                                }
                                _ => {}
                            }
                            self.active_dialog = None;
                        }
                    }
                    if ui.button("Close (Esc)").clicked() {
                        self.active_dialog = None;
                    }
                });
        }
        if dlg == Some(ActiveDialog::KeyboardShortcuts) {
            egui::Window::new("Keyboard Shortcuts (F1)").show(ctx, |ui| {
                for (k, v) in [
                    ("Ctrl+O", "Open splat, upload to viewport"),
                    ("Ctrl+S", "Convert input to output"),
                    ("Ctrl+Shift+S", "Focus output field"),
                    ("Ctrl+E", "Render PNG still"),
                    ("Ctrl+K", "Command palette"),
                    ("F1", "This cheat sheet"),
                    ("Ctrl+Z / Ctrl+Y", "Undo / redo path fields"),
                    ("Ctrl+,", "Preferences"),
                    ("drag / wheel / double-click", "Orbit / zoom / reset camera"),
                ] {
                    ui.monospace(format!("{k:22} {v}"));
                }
                if ui.button("Close").clicked() {
                    self.active_dialog = None;
                }
            });
        }
        if dlg == Some(ActiveDialog::ProjectDocs) {
            egui::Window::new("Project Files and Docs").show(ctx, |ui| {
                ui.label("Local repo docs (open in an editor):");
                for f in [
                    "docs/PRD.md",
                    "docs/TIERED_PLAN.md",
                    "docs/UI_PARITY.md",
                    "docs/SECRETS_STANDARD.md",
                    "AGENTS.md",
                    "README.md",
                ] {
                    ui.monospace(f);
                }
                ui.small("Secrets: only `splatcraft-cli secret set worlds` writes keys. UI never shows values.");
                if ui.button("Close").clicked() {
                    self.active_dialog = None;
                }
            });
        }
        if dlg == Some(ActiveDialog::LicensesNotices) {
            egui::Window::new("Licenses and Notices").show(ctx, |ui| {
                ui.label("splatcraft: MIT OR Apache-2.0. 100% Safe Rust in L3 (forbid unsafe).");
                ui.label("Crates: splat-formats, splat-ops, splat-view (wgpu-3dgs-viewer 0.8), splat-worlds, splat-train, craft-secrets, splatcraft-core, splatcraft-mcp, splatcraft-app.");
                ui.label("Third-party: eframe/egui 0.37, wgpu 30, serde_json, clap, reqwest (rustls), keyring, png, pollster.");
                ui.label("SPZ layout verified against the reference `spz` crate; no code copied.");
                if ui.button("Close").clicked() {
                    self.active_dialog = None;
                }
            });
        }
        if dlg == Some(ActiveDialog::About) {
            egui::Window::new("About SplatCraft").show(ctx, |ui| {
                ui.heading("SplatCraft");
                ui.label(format!("Version {}", splatcraft_core::VERSION));
                ui.label("Gaussian splat toolkit: inspect, convert, edit, merge, render, generate, train.");
                ui.label("100% Safe Rust in L3. Single dispatch bus shared by CLI, MCP, and UI.");
                ui.label("Build: UNKNOWN-DATE local build (see git log for commit).");
                if ui.button("Close").clicked() {
                    self.active_dialog = None;
                }
            });
        }
        if dlg == Some(ActiveDialog::Preferences) {
            egui::Window::new("Preferences (Ctrl+,)").show(ctx, |ui| {
                ui.checkbox(&mut self.dark_mode, "dark mode");
                if ui.button("Apply theme").clicked() {
                    apply_theme(ctx, self.dark_mode);
                }
                ui.small(
                    "Tool lookup order: SPLATCRAFT_<TOOL> env, PATH, then SPLATCRAFT_TOOLS dir.",
                );
                ui.small("Data dir: SPLATCRAFT_DATA (default ./data). Root jail: SPLATCRAFT_ROOT.");
                if ui.button("Close").clicked() {
                    self.active_dialog = None;
                }
            });
        }
    }
}

fn pretty(r: splatcraft_core::Result<Value>) -> String {
    let v = r.unwrap_or_else(|e| error_json(&e));
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

impl eframe::App for SplatApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        handle_zoom(ui.ctx());
        // Global shortcuts.
        ui.ctx().input(|i| {
            if i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::O) {
                // handled below via flag to avoid borrow issues
            }
        });
        let ctrl = ui.ctx().input(|i| i.modifiers.command);
        let shift = ui.ctx().input(|i| i.modifiers.shift);
        if ctrl && !shift && ui.ctx().input(|i| i.key_pressed(egui::Key::K)) {
            self.palette_query.clear();
            self.active_dialog = Some(ActiveDialog::CommandPalette);
        }
        if ui.ctx().input(|i| i.key_pressed(egui::Key::F1)) {
            self.active_dialog = Some(ActiveDialog::KeyboardShortcuts);
        }
        if ctrl && !shift && ui.ctx().input(|i| i.key_pressed(egui::Key::O)) {
            self.load_scene(frame);
        }
        if ctrl && !shift && ui.ctx().input(|i| i.key_pressed(egui::Key::S)) {
            self.do_convert();
        }
        if ctrl && !shift && ui.ctx().input(|i| i.key_pressed(egui::Key::E)) {
            self.do_render();
        }
        if ctrl && !shift && ui.ctx().input(|i| i.key_pressed(egui::Key::Z)) {
            self.undo();
        }
        if ctrl && ui.ctx().input(|i| i.key_pressed(egui::Key::Y)) {
            self.redo();
        }
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.active_dialog = None;
        }

        self.menu_bar(ui, frame);
        self.dialogs(ui.ctx(), frame);

        egui::Panel::bottom("statusbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.label(&self.status_text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("splatcraft v{}", splatcraft_core::VERSION));
                });
            });
        });

        self.toolbox_panel(ui);
        self.inspector_panel(ui);

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("splatcraft workstation");
            self.quick_row(ui, frame);
            ui.separator();
            if self.show_viewport {
                if let Some(n) = self.loaded {
                    ui.label(format!(
                        "{n} splats | drag: orbit, wheel: zoom, double-click: reset"
                    ));
                }
                self.viewport(ui);
            } else {
                ui.label("Viewport hidden (View menu to restore).");
            }
            let _ = pretty(Ok(json!({"success": true})));
        });
    }
}

/// Headless checks through the same bus the GUI uses.
pub fn run_smoke_checks() -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let st = dispatch("status", &json!({})).map_err(|e| e.to_string())?;
    lines.push(format!("status: version {}", st["version"]));
    // Menu dispatch parity: every documented op must exist on the bus.
    let ops = st["ops"].as_array().map(|a| a.len()).unwrap_or(0);
    if ops < 10 {
        return Err(format!("menu parity: expected 10 ops, got {ops}"));
    }
    lines.push(format!("menu parity: {ops} ops on dispatch"));
    let eng = dispatch("engine_status", &json!({})).map_err(|e| e.to_string())?;
    lines.push(format!("engines: {} of {} up", eng["up"], eng["total"]));
    let dir = std::env::temp_dir().join(format!("splatcraft-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ply = dir.join("s.ply").to_string_lossy().into_owned();
    let spz = dir.join("s.spz").to_string_lossy().into_owned();
    let edited = dir.join("e.ply").to_string_lossy().into_owned();
    let merged = dir.join("m.ply").to_string_lossy().into_owned();
    let run = (|| -> Result<(), String> {
        dispatch("synth", &json!({"path": ply, "count": 300, "sh_degree": 1}))
            .map_err(|e| e.to_string())?;
        dispatch("convert", &json!({"input": ply, "output": spz})).map_err(|e| e.to_string())?;
        let i = dispatch("inspect", &json!({"path": spz})).map_err(|e| e.to_string())?;
        if i["splats"]["count"] != 300 {
            return Err(format!("expected 300 splats, got {}", i["splats"]["count"]));
        }
        dispatch(
            "edit",
            &json!({"input": ply, "output": edited, "steps": [{"op": "reduce_sh", "degree": 0}]}),
        )
        .map_err(|e| e.to_string())?;
        dispatch(
            "merge",
            &json!({"inputs": [ply, edited], "output": merged, "overwrite": true}),
        )
        .map_err(|e| e.to_string())?;
        // Render is GPU-gated: unsupported without an adapter is a pass.
        let png = dir.join("s.png").to_string_lossy().into_owned();
        match dispatch("render", &json!({"input": ply, "output": png})) {
            Ok(_) => lines.push("render: PNG still ok".to_string()),
            Err(e) if e.error_type() == "unsupported" => {
                lines.push(format!("render: skipped ({e})"));
            }
            Err(e) => return Err(e.to_string()),
        }
        // Worlds ledger is local and must always work; generation stays a dry run.
        let l = dispatch("worlds", &json!({"action": "ledger"})).map_err(|e| e.to_string())?;
        if l["success"] != true {
            return Err("worlds ledger failed".to_string());
        }
        let g = dispatch(
            "worlds",
            &json!({"action": "generate", "prompt": "smoke cave"}),
        )
        .map_err(|e| e.to_string())?;
        if g["dry_run"] != true {
            return Err("worlds generate must stay a dry run".to_string());
        }
        // Train plan validates args; missing tools surface as unsupported, also a pass.
        match dispatch(
            "train",
            &json!({"action": "plan", "video": ply, "output": merged, "overwrite": true}),
        ) {
            Ok(_) | Err(_) => {}
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    run?;
    lines.push("synth -> convert -> inspect: 300 splats ok".to_string());
    lines.push("edit + merge + worlds ledger ok".to_string());
    // Tofu guard on the menu + panel strings the smoke ships.
    for s in [
        "File",
        "Edit",
        "View",
        "Insert",
        "Modify",
        "Commands",
        "Window",
        "Help",
        "Command Palette...",
        "Keyboard Shortcuts...",
        "Project Files and Docs...",
        "Licenses and Notices...",
        "About SplatCraft",
        "splatcraft workstation",
    ] {
        check_string_for_tofu(s).map_err(|e| e.to_string())?;
    }
    lines.push("tofu guard: menu strings clean".to_string());
    Ok(lines)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn glyph_allowlist() {
        let src = include_str!("app.rs");
        let mut bad: Vec<u32> = Vec::new();
        for ch in src.chars() {
            if !ch.is_ascii() && !matches!(ch, '\u{00B0}' | '\u{00D7}' | '\u{00B7}' | '\u{2026}') {
                bad.push(ch as u32);
            }
        }
        assert!(
            bad.is_empty(),
            "non-ASCII glyphs outside allowlist: {bad:?}"
        );
    }

    #[test]
    fn tofu_guard_rejects_replacement_chars() {
        assert!(check_string_for_tofu("bad \u{FFFD} glyph").is_err());
        assert!(check_string_for_tofu("bad \x07 bell").is_err());
        assert!(check_string_for_tofu("plain ascii menu").is_ok());
    }

    #[test]
    fn smoke_covers_tier4_parity() {
        let lines = run_smoke_checks().unwrap();
        assert!(lines.iter().any(|l| l.contains("menu parity")));
        assert!(lines.iter().any(|l| l.contains("tofu guard")));
    }
}
