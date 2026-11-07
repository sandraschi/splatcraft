//! Engine probe. Looks for external tools without executing them and
//! reports honestly: `up` only when the binary was actually found.
//!
//! Lookup order per tool: `SPLATCRAFT_<TOOL>` override, PATH, then the
//! tools directory (`SPLATCRAFT_TOOLS`, default `./tools`).

use serde_json::{json, Value};
use splat_train::locate::locate;

/// (engine name, override variable, binary names to look for, what it is used for)
const ENGINES: &[(&str, &str, &[&str], &str)] = &[
    (
        "brush",
        "SPLATCRAFT_BRUSH",
        &["brush", "brush_app", "brush-app", "brush-cli"],
        "default trainer (COLMAP or Nerfstudio dataset to splat)",
    ),
    (
        "nerfstudio",
        "SPLATCRAFT_NS_TRAIN",
        &["ns-train"],
        "second trainer (gsplat, splatfacto); not driven yet",
    ),
    (
        "colmap",
        "SPLATCRAFT_COLMAP",
        &["colmap"],
        "structure from motion for video or image input",
    ),
    (
        "ffmpeg",
        "SPLATCRAFT_FFMPEG",
        &["ffmpeg"],
        "video frame extraction",
    ),
    (
        "splat-transform",
        "SPLATCRAFT_SPLAT_TRANSFORM",
        &["splat-transform"],
        "optional oracle and exporter (PlayCanvas)",
    ),
    (
        "bunx",
        "SPLATCRAFT_BUNX",
        &["bunx"],
        "runs splat-transform without a global install",
    ),
];

pub fn engine_status() -> Value {
    let tools = crate::train::tools_dir();
    let engines: Vec<Value> = ENGINES
        .iter()
        .map(|(name, var, bins, purpose)| match locate(var, bins, tools.as_deref()) {
            Some(p) => json!({"engine": name, "status": "up", "path": p.to_string_lossy(), "purpose": purpose}),
            None => json!({"engine": name, "status": "down", "searched": bins, "override": var, "purpose": purpose}),
        })
        .collect();
    let up = engines.iter().filter(|e| e["status"] == "up").count();
    json!({
        "success": true,
        "engines": engines,
        "up": up,
        "total": ENGINES.len(),
        "tools_dir": tools.map(|t| t.to_string_lossy().into_owned()),
        "note": "Probe only looks for the files. Nothing is executed or version-checked here; \
                 `train action=plan` checks each tool's --help before a run."
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn status_shape() {
        let v = engine_status();
        assert_eq!(v["success"], true);
        assert_eq!(v["total"], ENGINES.len());
        assert!(v["engines"].is_array());
    }
}
