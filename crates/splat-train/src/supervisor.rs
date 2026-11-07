//! Runs a queued job's stages in order, in its own process.
//!
//! Exec stages are spawned from argument vectors with stdout and stderr
//! appended to the job log. The supervisor polls for exit, the cancel
//! marker and the stage timeout, and refreshes the heartbeat.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use splat_formats::{ply, spz};

use crate::error::{Result, TrainError};
use crate::job;
use crate::plan::{Kind, Stage};

const BEAT_EVERY: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(300);
const IMAGE_EXTS: [&str; 3] = ["jpg", "jpeg", "png"];
const MIN_IMAGES: usize = 8;

enum End {
    Failed(String),
    Cancelled,
}

impl From<TrainError> for End {
    fn from(e: TrainError) -> Self {
        End::Failed(e.to_string())
    }
}

impl From<std::io::Error> for End {
    fn from(e: std::io::Error) -> Self {
        End::Failed(e.to_string())
    }
}

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

struct Sup {
    dir: PathBuf,
    job: Value,
    last_beat: Instant,
}

impl Sup {
    fn save(&mut self) -> Result<()> {
        self.job["heartbeat_unix"] = json!(job::now());
        self.last_beat = Instant::now();
        job::save(&self.dir, &self.job)
    }

    fn log(&self, msg: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(job::log_path(&self.dir))
        {
            let _ = writeln!(f, "[splatcraft {}] {msg}", job::now());
        }
    }

    fn set_stage(&mut self, i: usize, key: &str, value: Value) {
        if let Some(st) = self.job["stages"].get_mut(i) {
            st[key] = value;
        }
    }

    fn work_dir(&self) -> PathBuf {
        PathBuf::from(self.job["work_dir"].as_str().unwrap_or("."))
    }

    fn exec(
        &mut self,
        program: &Path,
        args: &[String],
        timeout: u64,
    ) -> std::result::Result<Option<Value>, End> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(job::log_path(&self.dir))?;
        let err = log.try_clone()?;
        let work = self.work_dir();
        std::fs::create_dir_all(&work)?;
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(&work)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err));
        no_window(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| End::Failed(format!("cannot start {}: {e}", program.display())))?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return if status.success() {
                    Ok(None)
                } else {
                    let code = status.code();
                    Err(End::Failed(match code {
                        Some(c) => format!("exited with code {c}"),
                        None => "terminated by a signal".to_string(),
                    }))
                };
            }
            if job::cancel_requested(&self.dir) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(End::Cancelled);
            }
            if start.elapsed() > Duration::from_secs(timeout) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(End::Failed(format!("timed out after {timeout}s")));
            }
            if self.last_beat.elapsed() >= BEAT_EVERY {
                self.save()?;
            }
            std::thread::sleep(POLL);
        }
    }
}

fn copy_images(from: &Path, to: &Path, max: usize) -> std::result::Result<Option<Value>, End> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(from)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    files.sort();
    files.truncate(max);
    if files.len() < MIN_IMAGES {
        return Err(End::Failed(format!(
            "found {} images in {}, need at least {MIN_IMAGES}",
            files.len(),
            from.display()
        )));
    }
    std::fs::create_dir_all(to)?;
    for f in &files {
        if let Some(name) = f.file_name() {
            std::fs::copy(f, to.join(name))?;
        }
    }
    Ok(Some(json!({"copied": files.len()})))
}

fn newest_ply(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut stack = vec![(dir.to_path_buf(), 0u8)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() && depth < 2 {
                stack.push((p, depth + 1));
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("ply"))
            {
                let m = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().is_none_or(|(t, _)| m > *t) {
                    best = Some((m, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::result::Result<(), End> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".splatcraft-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

fn finalize(
    from_dir: &Path,
    output: &Path,
    overwrite: bool,
) -> std::result::Result<Option<Value>, End> {
    let src = newest_ply(from_dir)
        .ok_or_else(|| End::Failed(format!("Brush wrote no .ply under {}", from_dir.display())))?;
    if output.exists() && !overwrite {
        return Err(End::Failed(format!(
            "{} already exists (pass overwrite=true to replace it)",
            output.display()
        )));
    }
    let bytes = std::fs::read(&src)?;
    let parsed = ply::read(&bytes);
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut note = Value::Null;
    let (data, count, sh) = match (ext.as_str(), parsed) {
        (_, Ok(s)) if s.is_empty() => {
            return Err(End::Failed("Brush output contains no splats".into()))
        }
        ("ply", Ok(s)) => (bytes, Some(s.len()), Some(s.sh_degree)),
        ("ply", Err(e)) => {
            note = json!(format!("copied unvalidated, our PLY reader said: {e}"));
            (bytes, None, None)
        }
        ("spz", Ok(s)) => {
            let out = spz::write(&s).map_err(|e| End::Failed(e.to_string()))?;
            (out, Some(s.len()), Some(s.sh_degree))
        }
        ("spz", Err(e)) => {
            return Err(End::Failed(format!(
                "cannot convert Brush output to SPZ, our PLY reader said: {e}"
            )))
        }
        _ => return Err(End::Failed("output must end in .ply or .spz".into())),
    };
    atomic_write(output, &data)?;
    Ok(Some(json!({
        "source_ply": src.to_string_lossy(),
        "output": output.to_string_lossy(),
        "bytes": data.len(),
        "splats": count,
        "sh_degree": sh,
        "note": note,
    })))
}

fn run_stage(sup: &mut Sup, st: &Stage) -> std::result::Result<Option<Value>, End> {
    match &st.kind {
        Kind::Mkdir(p) => {
            std::fs::create_dir_all(p)?;
            Ok(None)
        }
        Kind::Exec { program, args } => sup.exec(program, args, st.timeout_secs),
        Kind::CopyImages { from, to, max } => copy_images(from, to, *max),
        Kind::Require { any_of, hint } => {
            if any_of.iter().any(|p| p.exists()) {
                Ok(None)
            } else {
                Err(End::Failed(hint.clone()))
            }
        }
        Kind::Finalize {
            from_dir,
            output,
            overwrite,
        } => finalize(from_dir, output, *overwrite),
    }
}

fn finish(sup: &mut Sup, state: &str, error: Option<String>) -> Result<()> {
    sup.job["state"] = json!(state);
    sup.job["finished_unix"] = json!(job::now());
    sup.job["error"] = json!(error);
    sup.save()?;
    if let Some(data) = sup.dir.parent().and_then(Path::parent) {
        let _ = job::ledger_append(
            data,
            &json!({"ts": job::now(), "event": "finished", "job": sup.job["id"], "state": state, "error": sup.job["error"]}),
        );
    }
    sup.log(&format!("job {state}"));
    Ok(())
}

/// Run the job in `dir` to completion. Returns `Ok` even when the job itself
/// failed; the outcome is recorded in `job.json`.
pub fn run(dir: &Path) -> Result<()> {
    let job = job::load(dir)?;
    let state = job["state"].as_str().unwrap_or("").to_string();
    if state != "queued" {
        return Err(TrainError::Invalid(format!(
            "job is not queued (state: {state})"
        )));
    }
    let stages: Vec<Stage> = job["stages"]
        .as_array()
        .ok_or_else(|| TrainError::Invalid("job has no stages".into()))?
        .iter()
        .map(Stage::from_json)
        .collect::<Result<_>>()?;
    let mut sup = Sup {
        dir: dir.to_path_buf(),
        job,
        last_beat: Instant::now(),
    };
    sup.job["state"] = json!("running");
    sup.save()?;
    for (i, st) in stages.iter().enumerate() {
        if job::cancel_requested(dir) {
            sup.log("cancel requested before stage start");
            return finish(&mut sup, "cancelled", Some("cancelled by request".into()));
        }
        sup.log(&format!("stage {} ({}) starting", i + 1, st.name));
        sup.set_stage(i, "status", json!("running"));
        sup.set_stage(i, "started_unix", json!(job::now()));
        sup.save()?;
        match run_stage(&mut sup, st) {
            Ok(result) => {
                sup.set_stage(i, "status", json!("ok"));
                sup.set_stage(i, "finished_unix", json!(job::now()));
                if let Some(r) = result {
                    if st.name == "finalize" {
                        sup.job["result"] = r;
                    } else {
                        sup.set_stage(i, "detail", r);
                    }
                }
                sup.save()?;
            }
            Err(End::Cancelled) => {
                sup.set_stage(i, "status", json!("cancelled"));
                sup.log("cancelled while running");
                return finish(&mut sup, "cancelled", Some("cancelled by request".into()));
            }
            Err(End::Failed(msg)) => {
                sup.set_stage(i, "status", json!("failed"));
                sup.set_stage(i, "finished_unix", json!(job::now()));
                let full = format!("stage '{}' failed: {msg}", st.name);
                sup.log(&full);
                return finish(&mut sup, "failed", Some(full));
            }
        }
    }
    finish(&mut sup, "succeeded", None)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use splat_formats::synth::synthetic;

    fn me() -> PathBuf {
        std::env::current_exe().unwrap()
    }

    fn exec(args: &[&str], timeout: u64) -> Stage {
        Stage {
            name: "exec".into(),
            kind: Kind::Exec {
                program: me(),
                args: args.iter().map(|a| (*a).to_string()).collect(),
            },
            timeout_secs: timeout,
        }
    }

    fn new_job(data: &Path, stages: &[Stage], out: &Path) -> PathBuf {
        let work = data.join("work");
        let (_, dir) = job::create(data, "t", "test", stages, &work, out, &[]).unwrap();
        dir
    }

    fn state(dir: &Path) -> Value {
        job::load(dir).unwrap()
    }

    #[test]
    fn successful_job_validates_and_converts_the_output() {
        let d = tempfile::tempdir().unwrap();
        let out_dir = d.path().join("work").join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(
            out_dir.join("splat.ply"),
            ply::write(&synthetic(200, 1, 3)).unwrap(),
        )
        .unwrap();
        let output = d.path().join("final.spz");
        let stages = vec![
            Stage {
                name: "mk".into(),
                kind: Kind::Mkdir(d.path().join("work").join("images")),
                timeout_secs: 5,
            },
            exec(&["--list"], 60),
            Stage {
                name: "check".into(),
                kind: Kind::Require {
                    any_of: vec![out_dir.join("splat.ply")],
                    hint: "x".into(),
                },
                timeout_secs: 5,
            },
            Stage {
                name: "finalize".into(),
                kind: Kind::Finalize {
                    from_dir: out_dir,
                    output: output.clone(),
                    overwrite: false,
                },
                timeout_secs: 60,
            },
        ];
        let dir = new_job(d.path(), &stages, &output);
        run(&dir).unwrap();
        let j = state(&dir);
        assert_eq!(j["state"], "succeeded", "{j}");
        assert_eq!(j["result"]["splats"], 200);
        assert!(spz::read(&std::fs::read(&output).unwrap()).unwrap().len() == 200);
        let ledger = std::fs::read_to_string(d.path().join("jobs.jsonl")).unwrap();
        assert!(ledger.contains("\"event\":\"finished\""));
        // A finished job cannot be run again.
        assert!(run(&dir).is_err());
    }

    #[test]
    fn failing_stage_stops_the_job_and_records_why() {
        let d = tempfile::tempdir().unwrap();
        let stages = vec![exec(&["--no-such-flag-zzz"], 60), exec(&["--list"], 60)];
        let dir = new_job(d.path(), &stages, &d.path().join("o.ply"));
        run(&dir).unwrap();
        let j = state(&dir);
        assert_eq!(j["state"], "failed");
        assert!(j["error"].as_str().unwrap().contains("exited with code"));
        assert_eq!(j["stages"][0]["status"], "failed");
        assert_eq!(j["stages"][1]["status"], "pending");
    }

    #[test]
    fn missing_sparse_model_gives_the_hint() {
        let d = tempfile::tempdir().unwrap();
        let stages = vec![Stage {
            name: "check_sparse".into(),
            kind: Kind::Require {
                any_of: vec![d.path().join("nope")],
                hint: "no overlap between frames".into(),
            },
            timeout_secs: 5,
        }];
        let dir = new_job(d.path(), &stages, &d.path().join("o.ply"));
        run(&dir).unwrap();
        assert!(state(&dir)["error"]
            .as_str()
            .unwrap()
            .contains("no overlap"));
    }

    #[test]
    fn timeout_kills_the_child() {
        let d = tempfile::tempdir().unwrap();
        let (prog, args): (&str, Vec<&str>) = if cfg!(windows) {
            (
                "powershell",
                vec!["-NoProfile", "-Command", "Start-Sleep 30"],
            )
        } else {
            ("sleep", vec!["30"])
        };
        let stages = vec![Stage {
            name: "slow".into(),
            kind: Kind::Exec {
                program: PathBuf::from(prog),
                args: args.iter().map(|a| (*a).to_string()).collect(),
            },
            timeout_secs: 1,
        }];
        let dir = new_job(d.path(), &stages, &d.path().join("o.ply"));
        let t = Instant::now();
        run(&dir).unwrap();
        assert!(t.elapsed() < Duration::from_secs(20));
        assert!(state(&dir)["error"].as_str().unwrap().contains("timed out"));
    }

    #[test]
    fn cancel_marker_stops_before_the_next_stage() {
        let d = tempfile::tempdir().unwrap();
        let dir = new_job(d.path(), &[exec(&["--list"], 60)], &d.path().join("o.ply"));
        std::fs::write(dir.join("cancel"), "1").unwrap();
        run(&dir).unwrap();
        let j = state(&dir);
        assert_eq!(j["state"], "cancelled");
        assert_eq!(j["stages"][0]["status"], "pending");
    }

    #[test]
    fn copy_images_needs_enough_images_and_finalize_respects_overwrite() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("in");
        std::fs::create_dir_all(&from).unwrap();
        for i in 0..3 {
            std::fs::write(from.join(format!("a{i}.jpg")), b"x").unwrap();
        }
        assert!(copy_images(&from, &d.path().join("to"), 100).is_err());
        for i in 3..10 {
            std::fs::write(from.join(format!("a{i}.JPG")), b"x").unwrap();
        }
        std::fs::write(from.join("notes.txt"), b"x").unwrap();
        let r = copy_images(&from, &d.path().join("to"), 100)
            .ok()
            .flatten()
            .unwrap();
        assert_eq!(r["copied"], 10);
        let ply_dir = d.path().join("p");
        std::fs::create_dir_all(&ply_dir).unwrap();
        std::fs::write(
            ply_dir.join("a.ply"),
            ply::write(&synthetic(20, 0, 1)).unwrap(),
        )
        .unwrap();
        let out = d.path().join("o.ply");
        assert!(finalize(&ply_dir, &out, false).is_ok());
        assert!(finalize(&ply_dir, &out, false).is_err());
        assert!(finalize(&ply_dir, &out, true).is_ok());
        assert!(finalize(&ply_dir, &d.path().join("o.txt"), true).is_err());
        assert!(finalize(&d.path().join("empty"), &d.path().join("z.ply"), true).is_err());
    }
}
