//! `library` op: unified local catalog of DIY splats and World Labs Marble splats.
//!
//! Scans `<data>/worlds` (Marble), `<SPLATCRAFT_TOOLS>/scans` and finished
//! training job outputs (DIY), plus optional extra directories. Never hits the
//! network and never loads full splat payloads into memory.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::paths::{resolve, str_arg};
use crate::worlds::data_dir;

const MAX_DEPTH: usize = 4;
const MAX_LIMIT: usize = 500;
const DEFAULT_LIMIT: usize = 100;

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    path: String,
    format: &'static str,
    source: &'static str,
    size_bytes: u64,
    modified_secs: u64,
    has_collider: bool,
    has_thumbnail: bool,
}

fn opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>> {
    if args.get(key).is_none_or(Value::is_null) {
        Ok(None)
    } else {
        str_arg(args, key).map(Some)
    }
}

fn format_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "ply" => Some("ply"),
        "spz" => Some("spz"),
        _ => None,
    }
}

fn classify_local(path: &Path) -> &'static str {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with("_100k.spz") || name.ends_with("_500k.spz") || name.ends_with("_full.spz") {
        "marble"
    } else {
        "local"
    }
}

fn has_sibling_matching(dir: &Path, pred: impl Fn(&str) -> bool) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if pred(&name.to_ascii_lowercase()) {
                return true;
            }
        }
    }
    false
}

fn build_entry(path: &Path, source: &'static str) -> Option<Entry> {
    let format = format_of(path)?;
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let modified_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("splat")
        .to_string();
    let parent = path.parent();
    let has_collider = parent.is_some_and(|dir| has_sibling_matching(dir, |n| n.ends_with(".glb")));
    let has_thumbnail = parent.is_some_and(|dir| {
        has_sibling_matching(dir, |n| {
            n.contains("thumbnail")
                || n.ends_with("-front.png")
                || n.ends_with("_preview.png")
                || n.ends_with("_pano.jpg")
                || n.ends_with("_pano.png")
        })
    });
    Some(Entry {
        name,
        path: path.to_string_lossy().to_string(),
        format,
        source,
        size_bytes: meta.len(),
        modified_secs,
        has_collider,
        has_thumbnail,
    })
}

fn walk_dir(
    dir: &Path,
    source_override: Option<&'static str>,
    depth: usize,
    seen: &mut HashSet<PathBuf>,
    out: &mut Vec<Entry>,
) {
    if depth > MAX_DEPTH || !dir.is_dir() {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for item in rd.flatten() {
        let p = item.path();
        let Ok(ft) = item.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk_dir(&p, source_override, depth + 1, seen, out);
        } else if ft.is_file() && format_of(&p).is_some() {
            let key = std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                let src = source_override.unwrap_or_else(|| classify_local(&p));
                if let Some(e) = build_entry(&p, src) {
                    out.push(e);
                }
            }
        }
    }
}

fn collect_job_outputs(data: &Path, seen: &mut HashSet<PathBuf>, out: &mut Vec<Entry>) {
    let jobs_dir = data.join("jobs");
    let Ok(rd) = std::fs::read_dir(&jobs_dir) else {
        return;
    };
    for item in rd.flatten() {
        let job_json = item.path().join("job.json");
        let Ok(text) = std::fs::read_to_string(&job_json) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(out_path) = v.get("output").and_then(Value::as_str) else {
            continue;
        };
        let p = PathBuf::from(out_path);
        if p.is_file() && format_of(&p).is_some() {
            let key = std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                if let Some(e) = build_entry(&p, "diy") {
                    out.push(e);
                }
            }
        }
    }
}

pub fn library(args: &Value) -> Result<Value> {
    let action = opt_str(args, "action")?.unwrap_or("list");
    if action != "list" {
        return Err(Error::Invalid(format!(
            "unknown library action '{action}' (known: list)"
        )));
    }
    let source_filter = opt_str(args, "source")?.unwrap_or("all");
    if !matches!(source_filter, "all" | "diy" | "marble" | "local") {
        return Err(Error::Invalid(
            "source must be all, diy, marble or local".into(),
        ));
    }
    let query = opt_str(args, "query")?.map(str::to_ascii_lowercase);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_LIMIT, |n| (n as usize).clamp(1, MAX_LIMIT));

    let data = data_dir()?;
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut roots: Vec<Value> = Vec::new();

    // 1. Marble worlds fetched into <data>/worlds.
    let worlds_dir = data.join("worlds");
    roots.push(json!({"source": "marble", "path": worlds_dir.to_string_lossy(), "exists": worlds_dir.is_dir()}));
    walk_dir(&worlds_dir, Some("marble"), 0, &mut seen, &mut entries);

    // 2. DIY trained job outputs recorded under <data>/jobs/*/job.json.
    collect_job_outputs(&data, &mut seen, &mut entries);

    // 3. DIY scans under <SPLATCRAFT_TOOLS>/scans (if set).
    if let Ok(tools) = std::env::var("SPLATCRAFT_TOOLS") {
        if !tools.trim().is_empty() {
            let scans = PathBuf::from(tools.trim()).join("scans");
            roots.push(
                json!({"source": "diy", "path": scans.to_string_lossy(), "exists": scans.is_dir()}),
            );
            walk_dir(&scans, Some("diy"), 0, &mut seen, &mut entries);
        }
    }

    // 4. Optional extra directories from caller.
    if let Some(arr) = args.get("dirs").and_then(Value::as_array) {
        for raw in arr {
            let Some(s) = raw.as_str() else {
                return Err(Error::Invalid("dirs entries must be strings".into()));
            };
            let dir = resolve(s)?;
            roots.push(
                json!({"source": "local", "path": dir.to_string_lossy(), "exists": dir.is_dir()}),
            );
            walk_dir(&dir, None, 0, &mut seen, &mut entries);
        }
    }

    // Sort newest first, then by name for determinism.
    entries.sort_by(|a, b| {
        b.modified_secs
            .cmp(&a.modified_secs)
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut counts_diy = 0usize;
    let mut counts_marble = 0usize;
    let mut counts_local = 0usize;
    for e in &entries {
        match e.source {
            "diy" => counts_diy += 1,
            "marble" => counts_marble += 1,
            _ => counts_local += 1,
        }
    }

    let filtered: Vec<Value> = entries
        .into_iter()
        .filter(|e| source_filter == "all" || e.source == source_filter)
        .filter(|e| {
            query.as_deref().is_none_or(|q| {
                e.name.to_ascii_lowercase().contains(q) || e.path.to_ascii_lowercase().contains(q)
            })
        })
        .take(limit)
        .map(|e| {
            json!({
                "name": e.name,
                "path": e.path,
                "format": e.format,
                "source": e.source,
                "size_bytes": e.size_bytes,
                "modified_secs": e.modified_secs,
                "has_collider": e.has_collider,
                "has_thumbnail": e.has_thumbnail,
            })
        })
        .collect();

    Ok(json!({
        "success": true,
        "count": filtered.len(),
        "counts": {
            "total": counts_diy + counts_marble + counts_local,
            "diy": counts_diy,
            "marble": counts_marble,
            "local": counts_local,
        },
        "roots": roots,
        "items": filtered,
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn lists_diy_and_marble_splats_with_filters() {
        let _g = crate::ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let tools = d.path().join("tools");
        let worlds_dir = data.join("worlds").join("w-1");
        let scans_dir = tools.join("scans").join("plant");
        std::fs::create_dir_all(&worlds_dir).unwrap();
        std::fs::create_dir_all(&scans_dir).unwrap();
        std::fs::write(worlds_dir.join("w-1_500k.spz"), b"spz").unwrap();
        std::fs::write(worlds_dir.join("w-1_mesh.glb"), b"glb").unwrap();
        std::fs::write(scans_dir.join("plant.ply"), b"ply").unwrap();

        std::env::set_var("SPLATCRAFT_DATA", &data);
        std::env::set_var("SPLATCRAFT_TOOLS", &tools);
        std::env::remove_var("SPLATCRAFT_ROOT");

        let all = library(&json!({})).unwrap();
        assert_eq!(all["counts"]["diy"], 1);
        assert_eq!(all["counts"]["marble"], 1);
        assert_eq!(all["count"], 2);

        let marble_only = library(&json!({"source": "marble"})).unwrap();
        assert_eq!(marble_only["count"], 1);
        assert_eq!(marble_only["items"][0]["has_collider"], true);
        assert_eq!(marble_only["items"][0]["format"], "spz");

        let diy_only = library(&json!({"source": "diy", "query": "plant"})).unwrap();
        assert_eq!(diy_only["count"], 1);
        assert_eq!(diy_only["items"][0]["name"], "plant");
        assert_eq!(diy_only["items"][0]["format"], "ply");

        assert!(library(&json!({"source": "bogus"})).is_err());
        assert!(library(&json!({"action": "delete"})).is_err());
    }
}
