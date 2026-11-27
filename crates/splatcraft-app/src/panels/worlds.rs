//! Marble Worlds panel: text/image/multi-image/video generation, list, get, fetch, delete.

use eframe::egui;
use serde_json::{json, Map, Value};

#[derive(Clone, Debug)]
pub struct CachedWorld {
    pub id: String,
    pub name: String,
}

pub struct WorldsPanelState {
    pub action: String,
    pub prompt: String,
    pub kind: String,
    pub model: String,
    pub url: String,
    pub file: String,
    pub multi_images_raw: String,
    pub operation_id: String,
    pub world_id: String,
    pub dest: String,
    pub confirm_spend: bool,
    pub confirm_delete: bool,
    pub cached_worlds: Vec<CachedWorld>,
}

impl Default for WorldsPanelState {
    fn default() -> Self {
        Self {
            action: "ledger".to_string(),
            prompt: String::new(),
            kind: "text".to_string(),
            model: "marble-1.1".to_string(),
            url: String::new(),
            file: String::new(),
            multi_images_raw: String::new(),
            operation_id: String::new(),
            world_id: String::new(),
            dest: String::new(),
            confirm_spend: false,
            confirm_delete: false,
            cached_worlds: Vec::new(),
        }
    }
}

pub enum WorldsUiEvent {
    None,
    RunAction,
    QuickGet(String),
    QuickFetch(String),
}

impl WorldsPanelState {
    /// Build the `dispatch("worlds", ...)` JSON args for the current form state.
    pub fn build_args(&self) -> Result<Value, String> {
        let action = self.action.trim();
        let mut args = Map::new();
        args.insert("action".to_string(), Value::String(action.to_string()));
        match action {
            "generate" => {
                if !self.prompt.trim().is_empty() {
                    args.insert(
                        "prompt".to_string(),
                        Value::String(self.prompt.trim().to_string()),
                    );
                }
                if !self.kind.trim().is_empty() {
                    args.insert(
                        "kind".to_string(),
                        Value::String(self.kind.trim().to_string()),
                    );
                }
                if !self.model.trim().is_empty() {
                    args.insert(
                        "model".to_string(),
                        Value::String(self.model.trim().to_string()),
                    );
                }
                if !self.url.trim().is_empty() {
                    args.insert(
                        "url".to_string(),
                        Value::String(self.url.trim().to_string()),
                    );
                }
                if !self.file.trim().is_empty() {
                    args.insert(
                        "file".to_string(),
                        Value::String(self.file.trim().to_string()),
                    );
                }
                if self.kind.trim() == "multi-image" && !self.multi_images_raw.trim().is_empty() {
                    let parsed: Value = serde_json::from_str(self.multi_images_raw.trim())
                        .map_err(|e| format!("multi_images must be valid JSON array: {e}"))?;
                    args.insert("multi_images".to_string(), parsed);
                }
                if self.confirm_spend {
                    args.insert("confirm_spend".to_string(), Value::Bool(true));
                }
            }
            "status" => {
                if !self.operation_id.trim().is_empty() {
                    args.insert(
                        "operation_id".to_string(),
                        Value::String(self.operation_id.trim().to_string()),
                    );
                }
            }
            "get" => {
                if !self.world_id.trim().is_empty() {
                    args.insert(
                        "world_id".to_string(),
                        Value::String(self.world_id.trim().to_string()),
                    );
                }
            }
            "delete" => {
                if !self.world_id.trim().is_empty() {
                    args.insert(
                        "world_id".to_string(),
                        Value::String(self.world_id.trim().to_string()),
                    );
                }
                if self.confirm_delete {
                    args.insert("confirm_delete".to_string(), Value::Bool(true));
                }
            }
            "fetch" => {
                if !self.dest.trim().is_empty() {
                    args.insert(
                        "dest".to_string(),
                        Value::String(self.dest.trim().to_string()),
                    );
                }
                if !self.operation_id.trim().is_empty() {
                    args.insert(
                        "operation_id".to_string(),
                        Value::String(self.operation_id.trim().to_string()),
                    );
                }
                if !self.world_id.trim().is_empty() {
                    args.insert(
                        "world_id".to_string(),
                        Value::String(self.world_id.trim().to_string()),
                    );
                }
            }
            "list" => {}
            _ => {}
        }
        Ok(Value::Object(args))
    }

    /// Update cached world rows when a `list` call succeeds.
    pub fn ingest_result(&mut self, action: &str, value: &Value) {
        if action == "list" {
            let list = value
                .pointer("/worlds/worlds")
                .or_else(|| value.get("worlds"))
                .and_then(Value::as_array);
            if let Some(arr) = list {
                self.cached_worlds = arr
                    .iter()
                    .filter_map(|w| {
                        let id = w
                            .get("id")
                            .or_else(|| w.get("world_id"))
                            .and_then(Value::as_str)?
                            .to_string();
                        let name = w
                            .get("display_name")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&id)
                            .to_string();
                        Some(CachedWorld { id, name })
                    })
                    .collect();
            }
        }
        // Always disarm destructive/spend toggles after any dispatch.
        self.confirm_spend = false;
        self.confirm_delete = false;
    }

    pub fn fetch_args_for(world_id: &str, dest: &str) -> Value {
        let mut v = json!({"action": "fetch", "world_id": world_id});
        if !dest.trim().is_empty() {
            v["dest"] = Value::String(dest.trim().to_string());
        }
        v
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut WorldsPanelState) -> WorldsUiEvent {
    let mut event = WorldsUiEvent::None;
    ui.label(egui::RichText::new("Marble Worlds").strong());
    ui.small("Dry run by default. Spend and delete require explicit confirmation.");
    ui.horizontal(|ui| {
        ui.label("action");
        egui::ComboBox::from_id_salt("worlds_action")
            .selected_text(state.action.clone())
            .show_ui(ui, |ui| {
                for a in [
                    "ledger", "credits", "list", "get", "generate", "status", "fetch", "delete",
                ] {
                    ui.selectable_value(&mut state.action, a.to_string(), a);
                }
            });
    });

    match state.action.as_str() {
        "generate" => {
            ui.horizontal(|ui| {
                ui.label("kind");
                egui::ComboBox::from_id_salt("worlds_kind")
                    .selected_text(state.kind.clone())
                    .show_ui(ui, |ui| {
                        for k in ["text", "image", "multi-image", "video"] {
                            ui.selectable_value(&mut state.kind, k.to_string(), k);
                        }
                    });
                ui.label("model");
                egui::ComboBox::from_id_salt("worlds_model")
                    .selected_text(state.model.clone())
                    .show_ui(ui, |ui| {
                        for m in ["marble-1.1", "marble-1.1-plus"] {
                            ui.selectable_value(&mut state.model, m.to_string(), m);
                        }
                    });
            });
            ui.label("prompt:");
            ui.text_edit_singleline(&mut state.prompt);
            if matches!(state.kind.as_str(), "image" | "video") {
                ui.label("https url (or local file below):");
                ui.text_edit_singleline(&mut state.url);
                ui.label("local file:");
                ui.text_edit_singleline(&mut state.file);
            } else if state.kind == "multi-image" {
                ui.label("multi_images JSON ([{\"azimuth\":0,\"url\":\"https://...\"}, ...]):");
                ui.text_edit_multiline(&mut state.multi_images_raw);
            }
            ui.checkbox(
                &mut state.confirm_spend,
                "Confirm real API credit spend (~1500 cr)",
            );
        }
        "status" => {
            ui.horizontal(|ui| {
                ui.label("operation_id");
                ui.text_edit_singleline(&mut state.operation_id);
            });
        }
        "get" => {
            ui.horizontal(|ui| {
                ui.label("world_id");
                ui.text_edit_singleline(&mut state.world_id);
            });
        }
        "delete" => {
            ui.horizontal(|ui| {
                ui.label("world_id");
                ui.text_edit_singleline(&mut state.world_id);
            });
            ui.checkbox(&mut state.confirm_delete, "Confirm permanent cloud delete");
        }
        "fetch" => {
            ui.horizontal(|ui| {
                ui.label("world_id");
                ui.text_edit_singleline(&mut state.world_id);
            });
            ui.horizontal(|ui| {
                ui.label("operation_id");
                ui.text_edit_singleline(&mut state.operation_id);
            });
            ui.horizontal(|ui| {
                ui.label("dest (blank = <data>/worlds)");
                ui.text_edit_singleline(&mut state.dest);
            });
        }
        _ => {}
    }

    if ui.button("Run worlds action").clicked() {
        event = WorldsUiEvent::RunAction;
    }

    if !state.cached_worlds.is_empty() {
        ui.separator();
        ui.small(format!("Listed worlds ({}):", state.cached_worlds.len()));
        egui::ScrollArea::vertical()
            .id_salt("worlds_cached_scroll")
            .max_height(120.0)
            .show(ui, |ui| {
                for w in &state.cached_worlds {
                    ui.horizontal(|ui| {
                        ui.small(&w.name);
                        if ui.small_button("Get").clicked() {
                            event = WorldsUiEvent::QuickGet(w.id.clone());
                        }
                        if ui.small_button("Fetch").clicked() {
                            event = WorldsUiEvent::QuickFetch(w.id.clone());
                        }
                    });
                }
            });
    }

    ui.small("API key: run `splatcraft-cli secret set worlds` in a terminal.");
    event
}
