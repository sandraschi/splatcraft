//! `trellis`: one photo of an object to a textured GLB through Microsoft TRELLIS.2.
//!
//! The output is a mesh, not a Gaussian splat. `plan` resolves the runner and
//! prints the command. `start` writes a job the same supervisor runs. Status,
//! log, cancel and list are the training job store.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use splat_train::job;
use splat_train::locate::locate;
use splat_train::plan::{Kind, Stage};

use crate::error::{Error, Result};
use crate::paths::resolve;
use crate::train::{abs, clean, opt_str, spawn_supervisor, supervisor_exe, tools_dir, uint};
use crate::worlds::data_dir;

const SCRIPT: &str = include_str!("trellis2_image.py");
const SCRIPT_NAME: &str = "trellis2_image.py";
const TIMEOUT_TRELLIS: u64 = 2 * 3600;

enum Runner {
    /// A program that already accepts `--image --output --pipeline --seed`.
    Program(PathBuf),
    /// The bundled script, executed by this interpreter.
    Python(PathBuf),
}

fn pipeline_type(resolution: u32) -> Result<&'static str> {
    match resolution {
        512 => Ok("512"),
        1024 => Ok("1024_cascade"),
        1536 => Ok("1536_cascade"),
        _ => Err(Error::Invalid(
            "resolution must be 512, 1024, or 1536".into(),
        )),
    }
}

fn image_ok(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp")
    )
}

/// Resolve the runner. A set but missing `SPLATCRAFT_TRELLIS2` does not fall
/// back to Python: the caller asked for that file.
fn find_runner() -> Result<Runner> {
    if let Some(raw) = std::env::var_os("SPLATCRAFT_TRELLIS2").filter(|v| !v.is_empty()) {
        let p = PathBuf::from(&raw);
        if p.is_file() {
            return Ok(Runner::Program(p));
        }
        return Err(Error::Unsupported(format!(
            "SPLATCRAFT_TRELLIS2 is set to '{}' and that file is missing",
            p.display()
        )));
    }
    locate(
        "SPLATCRAFT_PYTHON",
        &["py", "python", "python3"],
        tools_dir().as_deref(),
    )
    .map(Runner::Python)
    .ok_or_else(|| {
        Error::Unsupported(
            "no Python interpreter for TRELLIS.2. Set SPLATCRAFT_PYTHON, or set \
             SPLATCRAFT_TRELLIS2 to a runner that accepts --image --output --pipeline --seed. \
             The official model is Linux, an NVIDIA GPU with 24GB, CUDA 12.4, and \
             microsoft/TRELLIS.2-4B."
                .into(),
        )
    })
}

fn runner_json(runner: &Runner, script: &Path) -> (String, Vec<String>) {
    match runner {
        Runner::Program(p) => (p.to_string_lossy().into_owned(), Vec::new()),
        Runner::Python(py) => (
            py.to_string_lossy().into_owned(),
            vec![script.to_string_lossy().into_owned()],
        ),
    }
}

pub(crate) fn trellis_engine() -> Value {
    match find_runner() {
        Ok(runner) => {
            let (path, _) = runner_json(&runner, Path::new(SCRIPT_NAME));
            json!({
                "engine": "trellis2",
                "status": "up",
                "path": path,
                "purpose": "one photo to a TRELLIS.2 textured GLB. The probe finds the runner only; the model package is checked when the job runs.",
                "output": "glb",
                "weights": "microsoft/TRELLIS.2-4B",
            })
        }
        Err(e) => json!({
            "engine": "trellis2",
            "status": "down",
            "override": "SPLATCRAFT_TRELLIS2",
            "purpose": "one photo to a TRELLIS.2 textured GLB",
            "reason": e.to_string(),
        }),
    }
}

pub(crate) fn trellis(args: &Value) -> Result<Value> {
    let action = crate::paths::str_arg(args, "action")?;
    match action {
        "status" | "log" | "cancel" | "list" => crate::train::train(args),
        "plan" | "start" => plan_or_start(action, args),
        other => Err(Error::Invalid(format!(
            "unknown trellis action '{other}' (known: plan, start, status, log, cancel, list)"
        ))),
    }
}

fn plan_or_start(action: &str, args: &Value) -> Result<Value> {
    let image_arg = crate::paths::str_arg(args, "image")?;
    let image = abs(resolve(image_arg)?)?;
    if !image.is_file() {
        return Err(Error::NotFound(format!(
            "image '{image_arg}' is not a file"
        )));
    }
    if !image_ok(&image) {
        return Err(Error::Invalid(
            "image must be a .png, .jpg, .jpeg, or .webp file".into(),
        ));
    }
    let resolution = uint(args, "resolution", 512)?;
    let pipeline = pipeline_type(resolution)?;
    let seed = uint(args, "seed", 42)?;
    let stem = image
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("object");
    let output = match opt_str(args, "output")? {
        Some(p) => abs(resolve(p)?)?,
        None => image.with_file_name(format!("{stem}.trellis2.glb")),
    };
    if output
        .extension()
        .and_then(|e| e.to_str())
        .is_none_or(|e| !e.eq_ignore_ascii_case("glb"))
    {
        return Err(Error::Invalid("output must end in .glb".into()));
    }
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if output.exists() && !overwrite {
        return Err(Error::Exists(format!(
            "'{}' already exists (pass overwrite=true)",
            output.display()
        )));
    }
    let runner = find_runner()?;
    let data = data_dir()?;
    let name = opt_str(args, "name")?.map_or_else(|| stem.to_string(), str::to_string);
    let work_dir = abs(data
        .join("work")
        .join(format!("trellis-{}-{}", clean(&name), job::now())))?;
    let script = work_dir.join(SCRIPT_NAME);
    let (program, mut cmd) = runner_json(&runner, &script);
    cmd.extend([
        "--image".into(),
        image.to_string_lossy().into_owned(),
        "--output".into(),
        output.to_string_lossy().into_owned(),
        "--pipeline".into(),
        pipeline.into(),
        "--seed".into(),
        seed.to_string(),
    ]);
    let mut notes = vec![
        "TRELLIS.2 writes a textured GLB. It is not a Gaussian splat, and the viewport opens .ply and .spz.".to_string(),
        format!("pipeline {pipeline} from resolution {resolution}, seed {seed}, weights microsoft/TRELLIS.2-4B"),
        "Official install: Linux, NVIDIA GPU with 24GB, CUDA 12.4. A set SPLATCRAFT_TRELLIS2 replaces the bundled script.".to_string(),
    ];
    if matches!(runner, Runner::Python(_)) {
        notes.push(
            "start writes trellis2_image.py into the work dir and runs it with the interpreter above."
                .into(),
        );
    }
    let stages = vec![
        Stage {
            name: "workdir".into(),
            kind: Kind::Mkdir(work_dir.clone()),
            timeout_secs: 60,
        },
        Stage {
            name: "trellis2".into(),
            kind: Kind::Exec {
                program: PathBuf::from(&program),
                args: cmd.clone(),
            },
            timeout_secs: TIMEOUT_TRELLIS,
        },
        Stage {
            name: "require_glb".into(),
            kind: Kind::Require {
                any_of: vec![output.clone()],
                hint: "TRELLIS.2 finished without writing the .glb".into(),
            },
            timeout_secs: 60,
        },
    ];
    let stages_json: Vec<Value> = stages
        .iter()
        .map(|s| match &s.kind {
            Kind::Exec { program, args } => json!({
                "stage": s.name,
                "program": program.to_string_lossy(),
                "args": args,
                "timeout_secs": s.timeout_secs,
            }),
            Kind::Mkdir(p) => json!({
                "stage": s.name,
                "step": "mkdir",
                "path": p.to_string_lossy(),
                "timeout_secs": s.timeout_secs,
            }),
            Kind::Require { any_of, hint } => json!({
                "stage": s.name,
                "step": "require",
                "any_of": any_of.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                "hint": hint,
                "timeout_secs": s.timeout_secs,
            }),
            _ => json!({"stage": s.name, "timeout_secs": s.timeout_secs}),
        })
        .collect();
    if action == "plan" {
        return Ok(json!({
            "success": true,
            "dry_run": true,
            "output_kind": "glb",
            "note": "nothing was started; use action=start to run TRELLIS.2",
            "image": image.to_string_lossy(),
            "output": output.to_string_lossy(),
            "pipeline": pipeline,
            "work_dir": work_dir.to_string_lossy(),
            "notes": notes,
            "stages": stages_json,
        }));
    }
    if let Runner::Python(_) = &runner {
        std::fs::create_dir_all(&work_dir)?;
        let tmp = work_dir.join(format!("{SCRIPT_NAME}.splatcraft-tmp"));
        std::fs::write(&tmp, SCRIPT)?;
        std::fs::rename(&tmp, &script)?;
    }
    let exe = supervisor_exe()?;
    let (id, dir) = job::create(
        &data,
        &name,
        "photo_to_trellis2",
        &stages,
        &work_dir,
        &output,
        &notes,
    )?;
    spawn_supervisor(&exe, &dir)?;
    Ok(json!({
        "success": true,
        "job_id": id,
        "state": "queued",
        "output_kind": "glb",
        "work_dir": work_dir.to_string_lossy(),
        "output": output.to_string_lossy(),
        "notes": notes,
        "next": "poll with action=status; the job keeps running if this process exits",
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    struct Clear(&'static str);
    impl Drop for Clear {
        fn drop(&mut self) {
            std::env::remove_var(self.0);
        }
    }

    #[test]
    fn plan_names_the_photo_and_refuses_a_bad_request() {
        let _g = crate::ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("SPLATCRAFT_DATA", d.path().join("data"));
        let _data = Clear("SPLATCRAFT_DATA");
        std::env::remove_var("SPLATCRAFT_ROOT");
        let runner = d.path().join("trellis2-runner");
        std::fs::write(&runner, b"runner").unwrap();
        std::env::set_var("SPLATCRAFT_TRELLIS2", &runner);
        let _runner_env = Clear("SPLATCRAFT_TRELLIS2");
        let image = d.path().join("cup.png");
        std::fs::write(&image, b"png").unwrap();
        let output = d.path().join("cup.glb");
        let planned = trellis(&json!({
            "action": "plan",
            "image": image.to_string_lossy(),
            "output": output.to_string_lossy(),
            "resolution": 1024,
        }))
        .unwrap();
        assert_eq!(planned["dry_run"], true);
        assert_eq!(planned["output_kind"], "glb");
        assert_eq!(planned["pipeline"], "1024_cascade");
        let text = planned.to_string();
        assert!(text.contains("cup.png"));
        assert!(text.contains("--pipeline"));
        assert_eq!(
            trellis(&json!({
                "action": "plan",
                "image": image.to_string_lossy(),
                "output": d.path().join("cup.ply").to_string_lossy(),
            }))
            .unwrap_err()
            .error_type(),
            "invalid"
        );
        assert_eq!(
            trellis(&json!({
                "action": "plan",
                "image": d.path().join("missing.png").to_string_lossy(),
                "output": output.to_string_lossy(),
            }))
            .unwrap_err()
            .error_type(),
            "not_found"
        );
        std::fs::write(&output, b"glb").unwrap();
        assert_eq!(
            trellis(&json!({
                "action": "plan",
                "image": image.to_string_lossy(),
                "output": output.to_string_lossy(),
            }))
            .unwrap_err()
            .error_type(),
            "exists"
        );
        std::env::set_var("SPLATCRAFT_TRELLIS2", d.path().join("absent-runner"));
        assert_eq!(
            trellis(&json!({
                "action": "plan",
                "image": image.to_string_lossy(),
                "output": d.path().join("fresh.glb").to_string_lossy(),
                "overwrite": true,
            }))
            .unwrap_err()
            .error_type(),
            "unsupported"
        );
    }
}
