//! Unified Library panel: browse and load both DIY splats and World Labs Marble splats.

use eframe::egui;
use serde_json::{json, Value};
use splatcraft_core::dispatch;

#[derive(Clone, Debug)]
pub struct LibraryItem {
    pub name: String,
    pub path: String,
    pub format: String,
    pub source: String,
    pub size_bytes: u64,
    pub has_collider: bool,
}

#[derive(Default)]
pub struct LibraryState {
    pub source_filter: String,
    pub query: String,
    pub items: Vec<LibraryItem>,
    pub count_diy: usize,
    pub count_marble: usize,
    pub count_local: usize,
    pub loaded_once: bool,
    pub last_error: Option<String>,
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            source_filter: "all".to_string(),
            ..Self::default()
        }
    }

    pub fn refresh(&mut self) {
        self.loaded_once = true;
        let mut args = json!({
            "action": "list",
            "source": if self.source_filter.is_empty() { "all" } else { &self.source_filter },
        });
        if !self.query.trim().is_empty() {
            args["query"] = Value::String(self.query.trim().to_string());
        }
        match dispatch("library", &args) {
            Ok(v) => {
                self.last_error = None;
                self.count_diy = v["counts"]["diy"].as_u64().unwrap_or(0) as usize;
                self.count_marble = v["counts"]["marble"].as_u64().unwrap_or(0) as usize;
                self.count_local = v["counts"]["local"].as_u64().unwrap_or(0) as usize;
                self.items = v["items"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| {
                                Some(LibraryItem {
                                    name: item["name"].as_str()?.to_string(),
                                    path: item["path"].as_str()?.to_string(),
                                    format: item["format"].as_str().unwrap_or("ply").to_string(),
                                    source: item["source"].as_str().unwrap_or("local").to_string(),
                                    size_bytes: item["size_bytes"].as_u64().unwrap_or(0),
                                    has_collider: item["has_collider"].as_bool().unwrap_or(false),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
            }
        }
    }
}

fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

/// Renders the Unified Library card. Returns `Some(path)` when the user clicks Load.
pub fn show(ui: &mut egui::Ui, state: &mut LibraryState) -> Option<String> {
    if !state.loaded_once {
        state.refresh();
    }
    let mut load_path = None;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Unified Splat Library").strong());
        if ui.small_button("Refresh").clicked() {
            state.refresh();
        }
    });
    ui.small(format!(
        "DIY: {} | Marble: {} | Local: {}",
        state.count_diy, state.count_marble, state.count_local
    ));
    ui.horizontal(|ui| {
        let mut changed = false;
        for (val, label) in [
            ("all", "All"),
            ("diy", "DIY"),
            ("marble", "Marble"),
            ("local", "Local"),
        ] {
            if ui
                .selectable_label(state.source_filter == val, label)
                .clicked()
            {
                state.source_filter = val.to_string();
                changed = true;
            }
        }
        if changed {
            state.refresh();
        }
    });
    ui.horizontal(|ui| {
        ui.label("filter");
        let resp = ui.text_edit_singleline(&mut state.query);
        if resp.changed() {
            state.refresh();
        }
    });
    if let Some(err) = &state.last_error {
        ui.small(format!("error: {err}"));
    }
    if state.items.is_empty() {
        ui.small("No splats found yet. Train a DIY scan or fetch a Marble world.");
    } else {
        egui::ScrollArea::vertical()
            .id_salt("library_scroll")
            .max_height(180.0)
            .show(ui, |ui| {
                for item in &state.items {
                    ui.horizontal(|ui| {
                        let tag = match item.source.as_str() {
                            "diy" => "[DIY]",
                            "marble" => "[MARBLE]",
                            _ => "[LOCAL]",
                        };
                        let col = if item.has_collider { " +glb" } else { "" };
                        ui.small(format!(
                            "{tag} {} ({}, {}{col})",
                            item.name,
                            item.format.to_ascii_uppercase(),
                            human_bytes(item.size_bytes),
                        ));
                        if ui.small_button("Load").clicked() {
                            load_path = Some(item.path.clone());
                        }
                    });
                }
            });
    }
    load_path
}
