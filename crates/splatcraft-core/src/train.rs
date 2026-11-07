//! `train` op: video, images or a COLMAP dataset to a splat, as detached jobs.
//!
//! Actions: `plan` (dry: resolves tools and prints the exact commands),
//! `start`, `status`, `log`, `cancel`, `list`. `start` writes the job to
//! `<SPLATCRAFT_DATA>/jobs/<id>/` and launches a separate supervisor process
//! (`splatcraft-cli job-run`) that outlives this call.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use splat_train::locate::locate;
use splat_train::plan::{plan, Kind, Params, Plan, Source, Tools};
use splat_train::probe::SystemProbe;
use splat_train::{job, supervisor, TrainError};

use crate::error::{Error, Result};
use crate::paths::{resolve, str_arg};
use crate::worlds::data_dir;

impl From<TrainError> for Error {
    fn from(e: TrainError) -> Self {
        match e {
            TrainError::Invalid(m) => Error::Invalid(m),
            TrainError::Missing(m) => Error::Unsupported(m),
            TrainError::NotFound(m) => Error::NotFound(m),
            TrainError::Io(m) => Error::Io(m),
        }
    }
}

pub fn tools_dir() -> Option<PathBuf> {
    let raw = std::env::var("SPLATCRAFT_TOOLS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "tools".to_string());
    resolve(&raw).ok()
}

pub fn find_tools() -> Tools {
    let dir = tools_dir();
    let d = dir.as_deref();
    Tools {
        ffmpeg: locate("SPLATCRAFT_FFMPEG", &["ffmpeg"], d),
        ffprobe: locate("SPLATCRAFT_FFPROBE", &["ffprobe"], d),
        colmap: locate("SPLATCRAFT_COLMAP", &["colmap"], d),
        brush: locate(
            "SPLATCRAFT_BRUSH",
            &["brush", "brush_app", "brush-app", "brush-cli"],
            d,
        ),
    }
}

fn abs(p: PathBuf) -> Result<PathBuf> {
    std::path::absolute(&p)
        .map_err(|e| Error::Io(format!("cannot make '{}' absolute: {e}", p.display())))
}

fn opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>> {
    if args.get(key).is_none_or(Value::is_null) {
        Ok(None)
    } else {
        str_arg(args, key).map(Some)
    }
}

fn uint(args: &Value, key: &str, default: u32) -> Result<u32> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| Error::Invalid(format!("'{key}' must be a non-negative integer"))),
    }
}

fn clean(name: &str) -> String {
    let c: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    if c.is_empty() {
        "job".into()
    } else {
        c
    }
}

fn params_from(args: &Value, data: &Path) -> Result<(Params, String, &'static str)> {
    let video = opt_str(args, "video")?;
    let images = opt_str(args, "images_dir")?;
    let dataset = opt_str(args, "dataset")?;
    let given = [video, images, dataset]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if given != 1 {
        return Err(Error::Invalid(
            "give exactly one of video, images_dir or dataset".into(),
        ));
    }
    let (source, kind, stem) = if let Some(v) = video {
        let p = abs(resolve(v)?)?;
        if !p.is_file() {
            return Err(Error::NotFound(format!("video '{v}' is not a file")));
        }
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video")
            .to_string();
        (Source::Video(p), "video_to_splat", stem)
    } else if let Some(i) = images {
        let p = abs(resolve(i)?)?;
        if !p.is_dir() {
            return Err(Error::NotFound(format!(
                "images_dir '{i}' is not a directory"
            )));
        }
        let stem = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("images")
            .to_string();
        (Source::Images(p), "images_to_splat", stem)
    } else {
        let d = dataset.unwrap_or_default();
        let p = abs(resolve(d)?)?;
        if !p.is_dir() {
            return Err(Error::NotFound(format!("dataset '{d}' is not a directory")));
        }
        let stem = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dataset")
            .to_string();
        (Source::Dataset(p), "dataset_to_splat", stem)
    };
    let output = abs(resolve(str_arg(args, "output")?)?)?;
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext != "ply" && ext != "spz" {
        return Err(Error::Invalid("output must end in .ply or .spz".into()));
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
    let name = opt_str(args, "name")?.map_or(stem, str::to_string);
    let work_dir = abs(data
        .join("work")
        .join(format!("{}-{}", clean(&name), job::now())))?;
    let params = Params {
        source,
        work_dir,
        output,
        overwrite,
        target_frames: uint(args, "target_frames", 150)?,
        max_resolution: uint(args, "max_resolution", 1600)?,
        iters: uint(args, "iters", 10_000)?,
        max_splats: uint(args, "max_splats", 2_000_000)?,
        sh_degree: uint(args, "sh_degree", 3)?,
        quality: opt_str(args, "quality")?.unwrap_or("medium").to_string(),
    };
    Ok((params, name, kind))
}

fn describe(p: &Plan) -> Vec<Value> {
    p.stages
        .iter()
        .map(|s| match &s.kind {
            Kind::Exec { program, args } => json!({
                "stage": s.name, "program": program.to_string_lossy(), "args": args,
                "timeout_secs": s.timeout_secs
            }),
            other => json!({"stage": s.name, "step": format!("{other:?}"), "timeout_secs": s.timeout_secs}),
        })
        .collect()
}

fn supervisor_exe() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("SPLATCRAFT_CLI").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let me = std::env::current_exe()?;
    let name = if cfg!(windows) {
        "splatcraft-cli.exe"
    } else {
        "splatcraft-cli"
    };
    if me.file_name().and_then(|n| n.to_str()) == Some(name) {
        return Ok(me);
    }
    let sibling = me.with_file_name(name);
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err(Error::Unsupported(
        "cannot find splatcraft-cli next to this program to supervise the job; \
         set SPLATCRAFT_CLI to its path"
            .into(),
    ))
}

#[cfg(windows)]
fn detach(cmd: &mut Command, breakaway: bool) {
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
    let mut flags = 0x0000_0008 | 0x0000_0200 | 0x0800_0000;
    if breakaway {
        flags |= 0x0100_0000; // CREATE_BREAKAWAY_FROM_JOB
    }
    cmd.creation_flags(flags);
}

#[cfg(not(windows))]
fn detach(_cmd: &mut Command, _breakaway: bool) {}

fn spawn_supervisor(exe: &Path, dir: &Path) -> Result<()> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(job::log_path(dir))?;
    let build = |breakaway: bool| -> Result<Command> {
        let mut cmd = Command::new(exe);
        cmd.arg("job-run")
            .arg("--dir")
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log.try_clone()?));
        detach(&mut cmd, breakaway);
        Ok(cmd)
    };
    // Prefer leaving the caller's job object so the run survives the host;
    // some hosts forbid that, so fall back to a plain detached spawn.
    let first = build(true)?.spawn();
    match first {
        Ok(_) => Ok(()),
        Err(_) => build(false)?
            .spawn()
            .map(|_| ())
            .map_err(|e| Error::Io(format!("cannot start supervisor {}: {e}", exe.display()))),
    }
}

fn job_arg(args: &Value, data: &Path) -> Result<PathBuf> {
    let id = str_arg(args, "job_id")?;
    let dir = job::job_dir(data, id)?;
    if !dir.join("job.json").is_file() {
        return Err(Error::NotFound(format!("no job '{id}'")));
    }
    Ok(dir)
}

pub fn train(args: &Value) -> Result<Value> {
    let action = str_arg(args, "action")?;
    let data = data_dir()?;
    match action {
        "plan" | "start" => {
            let (params, name, kind) = params_from(args, &data)?;
            let tools = find_tools();
            let pl = plan(&params, &tools, &SystemProbe)?;
            if action == "plan" {
                return Ok(json!({
                    "success": true,
                    "dry_run": true,
                    "note": "nothing was started; use action=start to run these stages",
                    "work_dir": params.work_dir.to_string_lossy(),
                    "output": params.output.to_string_lossy(),
                    "notes": pl.notes,
                    "stages": describe(&pl),
                }));
            }
            let exe = supervisor_exe()?;
            let (id, dir) = job::create(
                &data,
                &name,
                kind,
                &pl.stages,
                &params.work_dir,
                &params.output,
                &pl.notes,
            )?;
            spawn_supervisor(&exe, &dir)?;
            Ok(json!({
                "success": true,
                "job_id": id,
                "state": "queued",
                "work_dir": params.work_dir.to_string_lossy(),
                "output": params.output.to_string_lossy(),
                "notes": pl.notes,
                "next": "poll with action=status; the job keeps running if this process exits",
            }))
        }
        "status" => {
            let dir = job_arg(args, &data)?;
            let lines = uint(args, "lines", 15)?.min(200) as usize;
            let mut v = job::status(&dir, lines)?;
            v["success"] = json!(true);
            Ok(v)
        }
        "log" => {
            let dir = job_arg(args, &data)?;
            let lines = uint(args, "lines", 50)?.clamp(1, 500) as usize;
            Ok(json!({"success": true, "log_tail": job::tail(&job::log_path(&dir), lines)}))
        }
        "cancel" => {
            let dir = job_arg(args, &data)?;
            let mut v = job::request_cancel(&dir)?;
            v["success"] = json!(true);
            Ok(v)
        }
        "list" => {
            let limit = uint(args, "limit", 10)?.clamp(1, 100) as usize;
            Ok(json!({"success": true, "jobs": job::list(&data, limit)?}))
        }
        other => Err(Error::Invalid(format!(
            "unknown train action '{other}' (known: plan, start, status, log, cancel, list)"
        ))),
    }
}

/// Entry for the hidden `job-run` command: run one queued job to the end.
pub fn run_supervisor(dir: &str) -> Result<()> {
    let p = resolve(dir)?;
    let in_jobs = p
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|n| n == "jobs");
    if !in_jobs || !p.join("job.json").is_file() {
        return Err(Error::Invalid("not a splatcraft job directory".into()));
    }
    Ok(supervisor::run(&p)?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn setup() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
        let g = crate::ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("SPLATCRAFT_DATA", d.path().join("data"));
        std::env::remove_var("SPLATCRAFT_ROOT");
        // Make every tool "missing" no matter what is installed on this machine.
        for v in [
            "SPLATCRAFT_BRUSH",
            "SPLATCRAFT_COLMAP",
            "SPLATCRAFT_FFMPEG",
            "SPLATCRAFT_FFPROBE",
        ] {
            std::env::set_var(v, d.path().join("absent"));
        }
        (g, d)
    }

    #[test]
    fn argument_validation() {
        let (_g, d) = setup();
        let vid = d.path().join("v.mp4");
        std::fs::write(&vid, b"x").unwrap();
        let out = d.path().join("o.ply");
        let v = vid.to_string_lossy().to_string();
        let o = out.to_string_lossy().to_string();
        let cases = [
            json!({"action": "plan"}),
            json!({"action": "plan", "output": o}),
            json!({"action": "plan", "video": v, "images_dir": v, "output": o}),
            json!({"action": "plan", "video": "nope.mp4", "output": o}),
            json!({"action": "plan", "video": v, "output": "x.txt"}),
            json!({"action": "nope"}),
            json!({"action": "status"}),
            json!({"action": "status", "job_id": "../x"}),
            json!({"action": "status", "job_id": "jmissing"}),
        ];
        for c in cases {
            assert!(train(&c).is_err(), "{c}");
        }
    }

    #[test]
    fn existing_output_is_refused_and_missing_brush_is_unsupported() {
        let (_g, d) = setup();
        let vid = d.path().join("v.mp4");
        std::fs::write(&vid, b"x").unwrap();
        let out = d.path().join("o.ply");
        std::fs::write(&out, b"x").unwrap();
        let mut a = json!({"action": "plan", "video": vid.to_string_lossy(), "output": out.to_string_lossy()});
        assert_eq!(train(&a).unwrap_err().error_type(), "exists");
        a["overwrite"] = json!(true);
        assert_eq!(train(&a).unwrap_err().error_type(), "unsupported");
    }

    #[test]
    fn list_and_log_on_an_empty_store() {
        let (_g, _d) = setup();
        let r = train(&json!({"action": "list"})).unwrap();
        assert_eq!(r["jobs"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn supervisor_entry_rejects_foreign_directories() {
        let (_g, d) = setup();
        assert!(run_supervisor(&d.path().to_string_lossy()).is_err());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean("a b/c"), "a_b_c");
        assert_eq!(clean(""), "job");
    }
}
