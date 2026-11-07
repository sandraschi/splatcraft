//! Engine probe. Looks for external tools on PATH without executing them and
//! reports honestly: `up` only when the binary was actually found.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// (engine name, binary names to look for, what it is used for)
const ENGINES: &[(&str, &[&str], &str)] = &[
    (
        "brush",
        &["brush", "brush_app"],
        "default trainer (COLMAP or Nerfstudio dataset to splat)",
    ),
    (
        "nerfstudio",
        &["ns-train"],
        "second trainer (gsplat, splatfacto)",
    ),
    (
        "colmap",
        &["colmap"],
        "structure from motion for video or image input",
    ),
    ("ffmpeg", &["ffmpeg"], "video frame extraction"),
    (
        "splat-transform",
        &["splat-transform"],
        "optional oracle and exporter (PlayCanvas)",
    ),
    (
        "bunx",
        &["bunx"],
        "runs splat-transform without a global install",
    ),
];

fn exts() -> Vec<String> {
    if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![String::new()]
    }
}

/// Find `name` in the directories of `path_var`.
pub fn find_in(path_var: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    let exts = exts();
    for dir in std::env::split_paths(path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if is_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_file(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

pub fn engine_status() -> Value {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let engines: Vec<Value> = ENGINES
        .iter()
        .map(|(name, bins, purpose)| {
            let found = bins.iter().find_map(|b| find_in(&path_var, b));
            match found {
                Some(p) => json!({"engine": name, "status": "up", "path": p.to_string_lossy(), "purpose": purpose}),
                None => json!({"engine": name, "status": "down", "searched": bins, "purpose": purpose}),
            }
        })
        .collect();
    let up = engines.iter().filter(|e| e["status"] == "up").count();
    json!({
        "success": true,
        "engines": engines,
        "up": up,
        "total": ENGINES.len(),
        "note": "Probe only checks PATH. Nothing is executed or version-checked yet."
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn empty_path_finds_nothing() {
        assert!(find_in(std::ffi::OsStr::new(""), "ffmpeg").is_none());
    }

    #[test]
    fn finds_a_file_in_a_dir() {
        let d = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) {
            "faketool.exe"
        } else {
            "faketool"
        };
        std::fs::write(d.path().join(name), b"x").unwrap();
        let p = std::env::join_paths([d.path()]).unwrap();
        assert!(find_in(&p, "faketool").is_some());
        assert!(find_in(&p, "othertool").is_none());
    }

    #[test]
    fn status_shape() {
        let v = engine_status();
        assert_eq!(v["success"], true);
        assert_eq!(v["total"], ENGINES.len());
        assert!(v["engines"].is_array());
    }
}
