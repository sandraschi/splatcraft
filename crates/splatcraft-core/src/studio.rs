//! Tier 5: local collision mesh, grey-splat recolor, and democraft recipes.
//!
//! `mesh` writes a GLB box from the splat's own positions. `recolor` tints
//! near-grey SH DC and writes a texturecraft bridge file beside the output.
//! `recipe` is the offline entry democraft already calls.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use splat_formats::{synth, Splats};
use splat_ops::{mesh, recolor};

use crate::error::{Error, Result};
use crate::ops::{self, format_of};
use crate::paths::{resolve, str_arg};

const DEFAULT_GREY_DELTA: f64 = 0.08;

pub fn mesh_op(args: &Value) -> Result<Value> {
    let input = resolve(str_arg(args, "input")?)?;
    let output = resolve(str_arg(args, "output")?)?;
    let overwrite = flag(args, "overwrite");
    require_ext(&output, "glb")?;
    refuse_same(&input, &output)?;
    if output.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            output.display()
        )));
    }
    let (splats, _fmt, _bytes) = ops::load(&input)?;
    let built = build_glb(&splats)?;
    let n = write_bytes(&output, &built.bytes)?;
    Ok(json!({
        "success": true,
        "input": input.to_string_lossy(),
        "output": output.to_string_lossy(),
        "finite": built.bounds.finite,
        "min": built.bounds.min,
        "max": built.bounds.max,
        "vertices": built.vertices,
        "triangles": built.triangles,
        "output_bytes": n,
        "note": "box collider from this splat's finite positions; not a Marble download",
    }))
}

pub fn recolor_op(args: &Value) -> Result<Value> {
    if args.get("action").and_then(Value::as_str) == Some("list") {
        return Ok(recipe_list());
    }
    let input = resolve(str_arg(args, "input")?)?;
    let output = resolve(str_arg(args, "output")?)?;
    let overwrite = flag(args, "overwrite");
    let recipe_id = str_arg(args, "recipe")?;
    let grey_delta = args
        .get("grey_delta")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_GREY_DELTA) as f32;
    format_of(&output)?;
    refuse_same(&input, &output)?;
    let bridge = bridge_path(&output)?;
    if output.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            output.display()
        )));
    }
    if bridge.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' exists (pass overwrite=true)",
            bridge.display()
        )));
    }
    let recipe = recolor::find(recipe_id)?;
    let (splats, fmt, _bytes) = ops::load(&input)?;
    let (tinted, stats) = recolor::apply(&splats, recipe, grey_delta)?;
    let out_bytes = ops::save(&output, &tinted, fmt)?;
    let bridge_json = json!({
        "schema": "splatcraft.texturecraft-bridge/1",
        "recipe": recipe.id,
        "tint_rgb": recipe.tint,
        "grey_delta": grey_delta,
        "tinted": stats.tinted,
        "skipped_color": stats.skipped_color,
        "skipped_bad": stats.skipped_bad,
        "splat": output.to_string_lossy(),
        "note": "Near-grey SH DC was multiplied by tint_rgb and higher SH bands on those splats were cleared. TextureCraft paints mesh textures; apply this tint when painting the collider.",
    });
    let bridge_bytes = write_bytes(
        &bridge,
        serde_json::to_string_pretty(&bridge_json)
            .map_err(|e| Error::Invalid(format!("bridge json: {e}")))?
            .as_bytes(),
    )?;
    Ok(json!({
        "success": true,
        "input": input.to_string_lossy(),
        "output": output.to_string_lossy(),
        "bridge": bridge.to_string_lossy(),
        "recipe": recipe.id,
        "tint_rgb": recipe.tint,
        "grey_delta": grey_delta,
        "tinted": stats.tinted,
        "skipped_color": stats.skipped_color,
        "skipped_bad": stats.skipped_bad,
        "output_bytes": out_bytes,
        "bridge_bytes": bridge_bytes,
    }))
}

pub fn recipe_op(args: &Value) -> Result<Value> {
    let name = str_arg(args, "name")?.to_ascii_lowercase();
    let out = resolve(str_arg(args, "out")?)?;
    if out.exists() && !out.is_dir() {
        return Err(Error::Invalid(format!(
            "'{}' is a file; --out must be a directory",
            out.display()
        )));
    }
    std::fs::create_dir_all(&out)?;
    match name.as_str() {
        "info" => recipe_info(&out),
        "convert" => recipe_convert(&out),
        "collider" => recipe_collider(&out),
        "recolor" => recipe_recolor(&out),
        other => Err(Error::Invalid(format!(
            "unknown recipe '{other}' (known: info, convert, collider, recolor)"
        ))),
    }
}

fn recipe_list() -> Value {
    let recipes: Vec<Value> = recolor::RECIPES
        .iter()
        .map(|r| json!({"id": r.id, "tint_rgb": r.tint}))
        .collect();
    json!({
        "success": true,
        "recipes": recipes,
        "grey_delta_default": DEFAULT_GREY_DELTA,
    })
}

fn recipe_info(out: &Path) -> Result<Value> {
    let ply = out.join("scene.ply");
    write_synth(&ply, 64, 1, 7)?;
    let inspected = ops::inspect(&json!({"path": ply.to_string_lossy()}))?;
    let report = out.join("info-report.md");
    write_bytes(
        &report,
        format!(
            "# SplatCraft inspect recipe\n\n- file: scene.ply\n- splats: {}\n- sh_degree: {}\n",
            inspected["splats"]["count"], inspected["splats"]["sh_degree"]
        )
        .as_bytes(),
    )?;
    Ok(json!({
        "success": true,
        "recipe": "info",
        "out": out.to_string_lossy(),
        "scene": ply.to_string_lossy(),
        "report": report.to_string_lossy(),
        "splats": inspected["splats"]["count"],
    }))
}

fn recipe_convert(out: &Path) -> Result<Value> {
    let ply = out.join("scene.ply");
    let spz = out.join("scene.spz");
    write_synth(&ply, 64, 0, 9)?;
    let converted = ops::convert(&json!({
        "input": ply.to_string_lossy(),
        "output": spz.to_string_lossy(),
        "overwrite": true,
    }))?;
    let report = out.join("convert-report.md");
    write_bytes(
        &report,
        format!(
            "# SplatCraft convert recipe\n\n- from: scene.ply ({} bytes)\n- to: scene.spz ({} bytes)\n",
            converted["input_bytes"], converted["output_bytes"]
        )
        .as_bytes(),
    )?;
    Ok(json!({
        "success": true,
        "recipe": "convert",
        "out": out.to_string_lossy(),
        "scene": ply.to_string_lossy(),
        "spz": spz.to_string_lossy(),
        "report": report.to_string_lossy(),
        "input_bytes": converted["input_bytes"],
        "output_bytes": converted["output_bytes"],
    }))
}

fn recipe_collider(out: &Path) -> Result<Value> {
    let ply = out.join("scene.ply");
    let glb = out.join("collider.glb");
    write_synth(&ply, 48, 0, 3)?;
    let built = mesh_op(&json!({
        "input": ply.to_string_lossy(),
        "output": glb.to_string_lossy(),
        "overwrite": true,
    }))?;
    let report = out.join("collider-report.md");
    write_bytes(
        &report,
        format!(
            "# SplatCraft collider recipe\n\n- scene: scene.ply\n- glb: collider.glb\n- finite: {}\n- triangles: {}\n",
            built["finite"], built["triangles"]
        )
        .as_bytes(),
    )?;
    Ok(json!({
        "success": true,
        "recipe": "collider",
        "out": out.to_string_lossy(),
        "scene": ply.to_string_lossy(),
        "glb": glb.to_string_lossy(),
        "report": report.to_string_lossy(),
        "finite": built["finite"],
        "triangles": built["triangles"],
        "output_bytes": built["output_bytes"],
    }))
}

fn recipe_recolor(out: &Path) -> Result<Value> {
    let ply = out.join("grey.ply");
    let tinted = out.join("recolored.ply");
    let scene = grey_scene(24);
    ops::save(&ply, &scene, crate::ops::Format::Ply)?;
    let done = recolor_op(&json!({
        "input": ply.to_string_lossy(),
        "output": tinted.to_string_lossy(),
        "recipe": "ghibli",
        "overwrite": true,
    }))?;
    let report = out.join("recolor-report.md");
    write_bytes(
        &report,
        format!(
            "# SplatCraft recolor recipe\n\n- recipe: ghibli\n- tinted: {}\n- bridge: {}\n",
            done["tinted"], done["bridge"]
        )
        .as_bytes(),
    )?;
    Ok(json!({
        "success": true,
        "recipe": "recolor",
        "out": out.to_string_lossy(),
        "scene": ply.to_string_lossy(),
        "output": tinted.to_string_lossy(),
        "bridge": done["bridge"],
        "report": report.to_string_lossy(),
        "tinted": done["tinted"],
        "skipped_color": done["skipped_color"],
    }))
}

fn grey_scene(n: usize) -> Splats {
    let mut s = Splats::empty(0);
    for i in 0..n {
        let t = i as f32;
        s.positions.push([(t - 4.0) * 0.2, 0.0, (t * 0.15) - 1.0]);
        s.scales.push([-4.0, -4.0, -4.0]);
        s.rotations.push([1.0, 0.0, 0.0, 0.0]);
        s.opacities.push(2.0);
        s.sh_dc.push([0.0, 0.0, 0.0]);
    }
    s
}

fn write_synth(path: &Path, count: usize, degree: u8, seed: u64) -> Result<()> {
    let s = synth::synthetic(count, degree, seed);
    ops::save(path, &s, crate::ops::Format::Ply)?;
    Ok(())
}

fn build_glb(splats: &Splats) -> Result<mesh::BoxGlb> {
    let b = mesh::bounds(&splats.positions)?;
    Ok(mesh::box_glb(&b)?)
}

fn bridge_path(output: &Path) -> Result<PathBuf> {
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Invalid("output file name is not valid UTF-8".into()))?;
    let name = format!("{stem}.texturecraft.json");
    Ok(match output.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        _ => PathBuf::from(name),
    })
}

fn require_ext(path: &Path, ext: &str) -> Result<()> {
    let got = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if got.as_deref() != Some(ext) {
        return Err(Error::Unsupported(format!("output must be a .{ext} file")));
    }
    Ok(())
}

fn refuse_same(a: &Path, b: &Path) -> Result<()> {
    if a == b {
        Err(Error::Invalid("input and output are the same path".into()))
    } else {
        Ok(())
    }
}

fn flag(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<u64> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".splatcraft-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::dispatch;

    #[test]
    fn mesh_recolor_and_recipes_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let ply = d.path().join("g.ply");
        ops::save(&ply, &grey_scene(16), crate::ops::Format::Ply).unwrap();
        let ply_s = ply.to_string_lossy().into_owned();
        let glb = d.path().join("c.glb").to_string_lossy().into_owned();
        let mesh = dispatch("mesh", &json!({"input": ply_s, "output": glb})).unwrap();
        assert_eq!(mesh["triangles"], 12);
        assert_eq!(mesh["finite"], 16);
        let bytes = std::fs::read(d.path().join("c.glb")).unwrap();
        assert_eq!(&bytes[0..4], b"glTF");

        let out = d.path().join("t.ply").to_string_lossy().into_owned();
        let rec = dispatch(
            "recolor",
            &json!({"input": ply_s, "output": out, "recipe": "cyberpunk"}),
        )
        .unwrap();
        assert_eq!(rec["tinted"], 16);
        let bridge = std::fs::read_to_string(d.path().join("t.texturecraft.json")).unwrap();
        assert!(bridge.contains("splatcraft.texturecraft-bridge/1"));
        assert!(bridge.contains("cyberpunk"));

        let listed = dispatch("recolor", &json!({"action": "list"})).unwrap();
        assert_eq!(listed["recipes"].as_array().unwrap().len(), 3);

        for name in ["info", "convert", "collider", "recolor"] {
            let dir = d.path().join(name);
            let v = dispatch(
                "recipe",
                &json!({"name": name, "out": dir.to_string_lossy()}),
            )
            .unwrap();
            assert_eq!(v["recipe"], name);
            assert_eq!(v["success"], true);
        }
        let glb = std::fs::read(d.path().join("collider").join("collider.glb")).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
        let tinted = dispatch(
            "inspect",
            &json!({"path": d.path().join("recolor").join("recolored.ply").to_string_lossy()}),
        )
        .unwrap();
        assert_eq!(tinted["splats"]["count"], 24);
    }

    #[test]
    fn unknown_recipe_name_is_invalid() {
        let d = tempfile::tempdir().unwrap();
        let e = dispatch(
            "recipe",
            &json!({"name": "nope", "out": d.path().to_string_lossy()}),
        )
        .unwrap_err();
        assert_eq!(e.error_type(), "invalid");
    }
}
