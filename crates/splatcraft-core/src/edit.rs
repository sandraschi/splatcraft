//! Edit ops: a step pipeline (`edit`) and `merge`. Edits run in RDF (the PLY
//! convention: x right, y down, z front) regardless of the input format.

use std::path::Path;

use serde_json::{json, Value};
use splat_formats::{Coords, Splats};
use splat_ops::{crop_aabb, merge as merge_sets, prune, quat_axis_angle, reduce_sh, transform};
use splat_ops::{Prune, Transform};

use crate::error::{Error, Result};
use crate::ops::{format_of, load, save};
use crate::paths::{resolve, str_arg};

const MAX_STEPS: usize = 32;
const MAX_MERGE_INPUTS: usize = 64;

fn num(step: &Value, key: &str) -> Result<Option<f32>> {
    match step.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite())
            .map(|x| Some(x as f32))
            .ok_or_else(|| Error::Invalid(format!("'{key}' must be a finite number"))),
    }
}

fn vec3(step: &Value, key: &str) -> Result<Option<[f32; 3]>> {
    match step.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let a = v
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or_else(|| Error::Invalid(format!("'{key}' must be an array of 3 numbers")))?;
            let mut out = [0.0f32; 3];
            for (i, x) in a.iter().enumerate() {
                out[i] =
                    x.as_f64().filter(|f| f.is_finite()).ok_or_else(|| {
                        Error::Invalid(format!("'{key}' must hold finite numbers"))
                    })? as f32;
            }
            Ok(Some(out))
        }
    }
}

fn check_output(output: &Path, overwrite: bool) -> Result<()> {
    format_of(output)?;
    if output.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            output.display()
        )));
    }
    Ok(())
}

fn apply(s: &Splats, step: &Value) -> Result<Splats> {
    let op = step
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Invalid("each step needs a string 'op'".into()))?;
    match op {
        "crop" => {
            let min =
                vec3(step, "min")?.ok_or_else(|| Error::Invalid("crop needs 'min'".into()))?;
            let max =
                vec3(step, "max")?.ok_or_else(|| Error::Invalid("crop needs 'max'".into()))?;
            let invert = step.get("invert").and_then(Value::as_bool).unwrap_or(false);
            Ok(crop_aabb(s, min, max, invert)?)
        }
        "prune" => {
            let p = Prune {
                min_opacity: num(step, "min_opacity")?,
                max_scale: num(step, "max_scale")?,
                drop_non_finite: step
                    .get("drop_non_finite")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            };
            Ok(prune(s, &p)?)
        }
        "transform" => {
            let rotate = match step.get("rotate") {
                None | Some(Value::Null) => None,
                Some(r) => {
                    let axis = vec3(r, "axis")?
                        .ok_or_else(|| Error::Invalid("rotate needs 'axis'".into()))?;
                    let deg = num(r, "degrees")?
                        .ok_or_else(|| Error::Invalid("rotate needs 'degrees'".into()))?;
                    Some(quat_axis_angle(axis, deg)?)
                }
            };
            let t = Transform {
                translate: vec3(step, "translate")?.unwrap_or([0.0; 3]),
                scale: num(step, "scale")?.unwrap_or(1.0),
                rotate,
            };
            Ok(transform(s, &t)?)
        }
        "reduce_sh" => {
            let d = step
                .get("degree")
                .and_then(Value::as_u64)
                .filter(|d| *d <= 3)
                .ok_or_else(|| Error::Invalid("reduce_sh needs 'degree' 0..=3".into()))?;
            Ok(reduce_sh(s, d as u8)?)
        }
        other => Err(Error::Invalid(format!(
            "unknown step op '{other}' (crop, prune, transform, reduce_sh)"
        ))),
    }
}

pub fn edit(args: &Value) -> Result<Value> {
    let input = resolve(str_arg(args, "input")?)?;
    let output = resolve(str_arg(args, "output")?)?;
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let steps = args
        .get("steps")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty() && a.len() <= MAX_STEPS)
        .ok_or_else(|| Error::Invalid(format!("'steps' must hold 1 to {MAX_STEPS} steps")))?;
    if input == output {
        return Err(Error::Invalid("input and output are the same path".into()));
    }
    check_output(&output, overwrite)?;
    let (loaded, _, _) = load(&input)?;
    let started = loaded.len();
    let mut cur = loaded.to_coords(Coords::Rdf);
    let mut report = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        let before = cur.len();
        cur = apply(&cur, step).map_err(|e| match e {
            Error::Invalid(m) => Error::Invalid(format!("step {i}: {m}")),
            Error::Unsupported(m) => Error::Unsupported(format!("step {i}: {m}")),
            other => other,
        })?;
        report.push(json!({
            "step": i,
            "op": step.get("op"),
            "before": before,
            "after": cur.len(),
        }));
    }
    let bytes = save(&output, &cur, format_of(&output)?)?;
    Ok(json!({
        "success": true,
        "input": input.to_string_lossy(),
        "output": output.to_string_lossy(),
        "frame": "rdf",
        "splats_in": started,
        "splats_out": cur.len(),
        "sh_degree": cur.sh_degree,
        "output_bytes": bytes,
        "steps": report,
    }))
}

pub fn merge(args: &Value) -> Result<Value> {
    let output = resolve(str_arg(args, "output")?)?;
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let inputs = args
        .get("inputs")
        .and_then(Value::as_array)
        .filter(|a| a.len() >= 2 && a.len() <= MAX_MERGE_INPUTS)
        .ok_or_else(|| {
            Error::Invalid(format!("'inputs' must list 2 to {MAX_MERGE_INPUTS} paths"))
        })?;
    check_output(&output, overwrite)?;
    let mut parts = Vec::new();
    for v in inputs {
        let raw = v
            .as_str()
            .ok_or_else(|| Error::Invalid("'inputs' must be strings".into()))?;
        let wrapped = json!({ "p": raw });
        let path = resolve(str_arg(&wrapped, "p")?)?;
        if path == output {
            return Err(Error::Invalid("output is also an input".into()));
        }
        parts.push(load(&path)?.0.to_coords(Coords::Rdf));
    }
    let degrees: Vec<u8> = parts.iter().map(|p| p.sh_degree).collect();
    let counts: Vec<usize> = parts.iter().map(Splats::len).collect();
    let merged = merge_sets(&parts)?;
    let bytes = save(&output, &merged, format_of(&output)?)?;
    Ok(json!({
        "success": true,
        "output": output.to_string_lossy(),
        "frame": "rdf",
        "input_counts": counts,
        "input_sh_degrees": degrees,
        "splats_out": merged.len(),
        "sh_degree": merged.sh_degree,
        "output_bytes": bytes,
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
    fn pipeline_reports_each_step() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (p(&d, "a.ply"), p(&d, "b.spz"));
        dispatch("synth", &json!({"path": a, "count": 2000, "sh_degree": 2})).unwrap();
        let r = dispatch(
            "edit",
            &json!({"input": a, "output": b, "steps": [
                {"op": "crop", "min": [-3, -3, -3], "max": [3, 3, 3]},
                {"op": "prune", "min_opacity": 0.3},
                {"op": "reduce_sh", "degree": 0},
                {"op": "transform", "translate": [1, 0, 0], "scale": 0.5,
                 "rotate": {"axis": [0, 1, 0], "degrees": 45}}
            ]}),
        )
        .unwrap();
        let steps = r["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 4);
        assert!(steps[0]["after"].as_u64().unwrap() < 2000);
        assert_eq!(steps[3]["before"], steps[3]["after"]);
        let i = dispatch("inspect", &json!({"path": b})).unwrap();
        assert_eq!(i["splats"]["count"], r["splats_out"]);
        assert_eq!(i["splats"]["sh_degree"], 0);
    }

    #[test]
    fn rotation_with_sh_fails_with_step_context() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (p(&d, "a.ply"), p(&d, "b.ply"));
        dispatch("synth", &json!({"path": a, "count": 10, "sh_degree": 1})).unwrap();
        let e = dispatch(
            "edit",
            &json!({"input": a, "output": b, "steps": [
                {"op": "transform", "rotate": {"axis": [0, 0, 1], "degrees": 10}}]}),
        )
        .unwrap_err();
        assert_eq!(e.error_type(), "unsupported");
        assert!(e.to_string().contains("step 0"));
        assert!(!std::path::Path::new(&b).exists());
    }

    #[test]
    fn bad_steps_are_rejected() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (p(&d, "a.ply"), p(&d, "b.ply"));
        dispatch("synth", &json!({"path": a, "count": 10})).unwrap();
        for steps in [
            json!([]),
            json!([{"op": "nope"}]),
            json!([{"op": "crop", "min": [0, 0], "max": [1, 1, 1]}]),
            json!([{"op": "reduce_sh", "degree": 9}]),
            json!([{"op": "transform", "scale": -1}]),
        ] {
            let e = dispatch("edit", &json!({"input": a, "output": b, "steps": steps}));
            assert!(e.is_err());
        }
    }

    #[test]
    fn merge_mixed_formats_and_degrees() {
        let d = tempfile::tempdir().unwrap();
        let (a, b, m) = (p(&d, "a.ply"), p(&d, "b.spz"), p(&d, "m.ply"));
        dispatch("synth", &json!({"path": a, "count": 100, "sh_degree": 3})).unwrap();
        dispatch(
            "synth",
            &json!({"path": b, "count": 50, "sh_degree": 1, "seed": 9}),
        )
        .unwrap();
        let r = dispatch("merge", &json!({"inputs": [a, b], "output": m})).unwrap();
        assert_eq!(r["splats_out"], 150);
        assert_eq!(r["sh_degree"], 1);
        assert_eq!(r["input_counts"], json!([100, 50]));
        let e = dispatch("merge", &json!({"inputs": [a], "output": m})).unwrap_err();
        assert_eq!(e.error_type(), "invalid");
        let e = dispatch(
            "merge",
            &json!({"inputs": [a, b], "output": a, "overwrite": true}),
        );
        assert!(e.is_err());
    }
}
