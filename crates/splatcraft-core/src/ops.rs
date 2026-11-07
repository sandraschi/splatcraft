//! Core operations: status, inspect, convert, synth.

use std::path::Path;

use serde_json::{json, Value};
use splat_formats::{ply, spz, stats, synth, Splats};

use crate::error::{Error, Result};
use crate::paths::{resolve, str_arg};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Ply,
    Spz,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::Ply => "ply",
            Format::Spz => "spz",
        }
    }
}

pub(crate) fn format_of(path: &Path) -> Result<Format> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("ply") => Ok(Format::Ply),
        Some("spz") => Ok(Format::Spz),
        Some(other) => Err(Error::Unsupported(format!(
            "file extension '.{other}' (supported: .ply, .spz)"
        ))),
        None => Err(Error::Unsupported(
            "no file extension (supported: .ply, .spz)".into(),
        )),
    }
}

pub(crate) fn load(path: &Path) -> Result<(Splats, Format, u64)> {
    let fmt = format_of(path)?;
    let bytes = std::fs::read(path)?;
    let size = bytes.len() as u64;
    let s = match fmt {
        Format::Ply => ply::read(&bytes)?,
        Format::Spz => spz::read(&bytes)?,
    };
    Ok((s, fmt, size))
}

pub(crate) fn save(path: &Path, s: &Splats, fmt: Format) -> Result<u64> {
    let bytes = match fmt {
        Format::Ply => ply::write(s)?,
        Format::Spz => spz::write(s)?,
    };
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    // Write beside the target, then rename, so a crash never leaves a half file.
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".splatcraft-tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, &bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(bytes.len() as u64)
}

pub fn status() -> Value {
    json!({
        "success": true,
        "name": "splatcraft",
        "version": crate::VERSION,
        "ops": crate::OPS,
        "formats": {"read": ["ply", "spz (v2, v3)"], "write": ["ply", "spz (v3)"]},
        "root": std::env::var("SPLATCRAFT_ROOT").ok(),
    })
}

fn stats_json(s: &Splats) -> Value {
    let st = stats::compute(s);
    json!({
        "count": st.count,
        "sh_degree": st.sh_degree,
        "antialiased": st.antialiased,
        "coords": s.coords.as_str(),
        "bbox_min": st.bbox_min,
        "bbox_max": st.bbox_max,
        "mean_opacity": st.mean_opacity,
        "scale_min": st.scale_min,
        "scale_max": st.scale_max,
        "non_finite": st.non_finite,
    })
}

pub fn inspect(args: &Value) -> Result<Value> {
    let path = resolve(str_arg(args, "path")?)?;
    let (s, fmt, size) = load(&path)?;
    Ok(json!({
        "success": true,
        "path": path.to_string_lossy(),
        "format": fmt.name(),
        "file_bytes": size,
        "splats": stats_json(&s),
    }))
}

pub fn convert(args: &Value) -> Result<Value> {
    let input = resolve(str_arg(args, "input")?)?;
    let output = resolve(str_arg(args, "output")?)?;
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if input == output {
        return Err(Error::Invalid("input and output are the same path".into()));
    }
    let out_fmt = format_of(&output)?;
    if output.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            output.display()
        )));
    }
    let (s, in_fmt, in_bytes) = load(&input)?;
    let out_bytes = save(&output, &s, out_fmt)?;
    let mut notes = Vec::new();
    if out_fmt == Format::Spz {
        notes.push("SPZ is lossy (8-bit colour, quantised scale, 24-bit positions)");
    }
    Ok(json!({
        "success": true,
        "input": input.to_string_lossy(),
        "output": output.to_string_lossy(),
        "from": in_fmt.name(),
        "to": out_fmt.name(),
        "splats": s.len(),
        "input_bytes": in_bytes,
        "output_bytes": out_bytes,
        "notes": notes,
    }))
}

pub fn synth(args: &Value) -> Result<Value> {
    let path = resolve(str_arg(args, "path")?)?;
    let count = args.get("count").and_then(Value::as_u64).unwrap_or(1000);
    if count > 5_000_000 {
        return Err(Error::Invalid(
            "count above 5,000,000 for synthetic data".into(),
        ));
    }
    let degree = args.get("sh_degree").and_then(Value::as_u64).unwrap_or(0);
    if degree > 3 {
        return Err(Error::Invalid("sh_degree must be 0..=3".into()));
    }
    let seed = args.get("seed").and_then(Value::as_u64).unwrap_or(1);
    let fmt = format_of(&path)?;
    if path.exists()
        && !args
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            path.display()
        )));
    }
    let s = synth::synthetic(count as usize, degree as u8, seed);
    let bytes = save(&path, &s, fmt)?;
    Ok(json!({
        "success": true,
        "path": path.to_string_lossy(),
        "format": fmt.name(),
        "splats": s.len(),
        "file_bytes": bytes,
        "note": "synthetic random splats, not a real scene",
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::dispatch;

    fn p(d: &tempfile::TempDir, n: &str) -> String {
        d.path().join(n).to_string_lossy().into_owned()
    }

    #[test]
    fn synth_inspect_convert_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let a = p(&d, "a.ply");
        let b = p(&d, "b.spz");
        let c = p(&d, "c.ply");
        dispatch("synth", &json!({"path": a, "count": 400, "sh_degree": 2})).unwrap();
        let i = dispatch("inspect", &json!({"path": a})).unwrap();
        assert_eq!(i["splats"]["count"], 400);
        assert_eq!(i["splats"]["sh_degree"], 2);
        let v = dispatch("convert", &json!({"input": a, "output": b})).unwrap();
        assert_eq!(v["to"], "spz");
        assert!(v["output_bytes"].as_u64().unwrap() < v["input_bytes"].as_u64().unwrap());
        dispatch("convert", &json!({"input": b, "output": c})).unwrap();
        let i2 = dispatch("inspect", &json!({"path": c})).unwrap();
        assert_eq!(i2["splats"]["count"], 400);
        assert_eq!(i2["splats"]["sh_degree"], 2);
        assert_eq!(i2["splats"]["coords"], "rdf");
    }

    #[test]
    fn convert_refuses_overwrite_and_same_path() {
        let d = tempfile::tempdir().unwrap();
        let a = p(&d, "a.ply");
        let b = p(&d, "b.spz");
        dispatch("synth", &json!({"path": a, "count": 10})).unwrap();
        dispatch("convert", &json!({"input": a, "output": b})).unwrap();
        let e = dispatch("convert", &json!({"input": a, "output": b})).unwrap_err();
        assert_eq!(e.error_type(), "exists");
        dispatch(
            "convert",
            &json!({"input": a, "output": b, "overwrite": true}),
        )
        .unwrap();
        let e = dispatch("convert", &json!({"input": a, "output": a})).unwrap_err();
        assert_eq!(e.error_type(), "invalid");
    }

    #[test]
    fn errors_are_typed() {
        let d = tempfile::tempdir().unwrap();
        let missing = p(&d, "nope.ply");
        let e = dispatch("inspect", &json!({"path": missing})).unwrap_err();
        assert_eq!(e.error_type(), "not_found");
        let bad = p(&d, "x.obj");
        let e = dispatch("inspect", &json!({"path": bad})).unwrap_err();
        assert_eq!(e.error_type(), "unsupported");
        let e = dispatch("bogus", &json!({})).unwrap_err();
        assert_eq!(e.error_type(), "invalid");
        let corrupt = p(&d, "bad.spz");
        std::fs::write(&corrupt, b"junk").unwrap();
        assert!(dispatch("inspect", &json!({"path": corrupt})).is_err());
    }

    #[test]
    fn status_lists_ops() {
        let v = dispatch("status", &json!({})).unwrap();
        assert_eq!(v["success"], true);
        assert!(v["ops"].as_array().unwrap().len() >= 5);
    }
}
