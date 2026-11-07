//! `render`: headless GPU still of a splat file, written as PNG.

use serde_json::{json, Value};
use splat_view::{RenderOptions, ViewError};

use crate::error::{Error, Result};
use crate::ops::load;
use crate::paths::{resolve, str_arg};

fn opt_f32(args: &Value, key: &str, default: f32) -> Result<f32> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite())
            .map(|x| x as f32)
            .ok_or_else(|| Error::Invalid(format!("'{key}' must be a finite number"))),
    }
}

fn opt_u32(args: &Value, key: &str, default: u32) -> Result<u32> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .ok_or_else(|| Error::Invalid(format!("'{key}' must be a positive integer"))),
    }
}

fn map_view(e: ViewError) -> Error {
    match e {
        ViewError::NoGpu(m) => Error::Unsupported(format!("no GPU available for rendering: {m}")),
        ViewError::Input(m) => Error::Invalid(m),
        ViewError::Gpu(m) | ViewError::Encode(m) => Error::Io(m),
    }
}

pub fn render(args: &Value) -> Result<Value> {
    let input = resolve(str_arg(args, "input")?)?;
    let output = resolve(str_arg(args, "output")?)?;
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let is_png = output
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !is_png {
        return Err(Error::Unsupported(
            "render output must be a .png file".into(),
        ));
    }
    if output.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            output.display()
        )));
    }
    let d = RenderOptions::default();
    let opts = RenderOptions {
        width: opt_u32(args, "width", d.width)?,
        height: opt_u32(args, "height", d.height)?,
        yaw_deg: opt_f32(args, "yaw_deg", d.yaw_deg)?,
        pitch_deg: opt_f32(args, "pitch_deg", d.pitch_deg)?,
        fov_deg: opt_f32(args, "fov_deg", d.fov_deg)?,
        distance_scale: opt_f32(args, "distance_scale", d.distance_scale)?,
        gaussian_scale: opt_f32(args, "gaussian_scale", d.gaussian_scale)?,
    };
    let (splats, _, _) = load(&input)?;
    let image = splat_view::render(&splats, &opts).map_err(map_view)?;
    let png = image.to_png().map_err(map_view)?;
    if let Some(dir) = output.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    let mut tmp = output.as_os_str().to_owned();
    tmp.push(".splatcraft-tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, &png)?;
    if let Err(e) = std::fs::rename(&tmp, &output) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(json!({
        "success": true,
        "input": input.to_string_lossy(),
        "output": output.to_string_lossy(),
        "width": image.width,
        "height": image.height,
        "splats": splats.len(),
        "covered_fraction": image.covered_fraction(),
        "output_bytes": png.len(),
        "note": "background is opaque black; camera auto-frames the scene",
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::dispatch;

    #[test]
    fn render_validates_before_touching_the_gpu() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.ply").to_string_lossy().into_owned();
        dispatch("synth", &json!({"path": a, "count": 50})).unwrap();
        let bad = d.path().join("o.jpg").to_string_lossy().into_owned();
        let e = dispatch("render", &json!({"input": a, "output": bad})).unwrap_err();
        assert_eq!(e.error_type(), "unsupported");
        let png = d.path().join("o.png").to_string_lossy().into_owned();
        let e = dispatch("render", &json!({"input": a, "output": png, "width": 3})).unwrap_err();
        assert!(matches!(e.error_type(), "invalid" | "unsupported"));
        let e = dispatch(
            "render",
            &json!({"input": a, "output": png, "width": "wide"}),
        )
        .unwrap_err();
        assert_eq!(e.error_type(), "invalid");
    }
}
