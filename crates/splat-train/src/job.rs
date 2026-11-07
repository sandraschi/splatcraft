//! Job store: one directory per job under `<data>/jobs/<id>/` holding
//! `job.json` (authoritative state), `log.txt`, and an optional `cancel`
//! marker, plus an append-only event ledger `<data>/jobs.jsonl`.
//!
//! The supervisor is the only writer of `job.json` while a job runs. It
//! refreshes `heartbeat_unix` every few seconds, so a job whose heartbeat is
//! stale is reported as `lost` instead of `running` forever.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::{Result, TrainError};
use crate::plan::Stage;

pub const STALE_SECS: u64 = 45;
const NOT_STARTED_SECS: u64 = 60;

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn check_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(TrainError::Invalid(
            "job id must be 1-64 chars of [A-Za-z0-9_-]".into(),
        ))
    }
}

pub fn job_dir(data: &Path, id: &str) -> Result<PathBuf> {
    check_id(id)?;
    Ok(data.join("jobs").join(id))
}

pub fn log_path(dir: &Path) -> PathBuf {
    dir.join("log.txt")
}

pub fn ledger_append(data: &Path, entry: &Value) -> Result<()> {
    std::fs::create_dir_all(data)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data.join("jobs.jsonl"))?;
    let line =
        serde_json::to_string(entry).map_err(|e| TrainError::Io(format!("ledger encode: {e}")))?;
    writeln!(f, "{line}")?;
    Ok(())
}

pub fn save(dir: &Path, job: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(job)
        .map_err(|e| TrainError::Io(format!("job encode: {e}")))?;
    let path = dir.join("job.json");
    let tmp = dir.join("job.json.splatcraft-tmp");
    std::fs::write(&tmp, text)?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

pub fn load(dir: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(dir.join("job.json"))?;
    serde_json::from_str(&text).map_err(|e| TrainError::Invalid(format!("corrupt job.json: {e}")))
}

/// Create a queued job on disk. Returns its id and directory.
pub fn create(
    data: &Path,
    name: &str,
    kind: &str,
    stages: &[Stage],
    work_dir: &Path,
    output: &Path,
    notes: &[String],
) -> Result<(String, PathBuf)> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let id = format!("j{}-{:04x}", now(), nanos & 0xffff);
    let dir = job_dir(data, &id)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(log_path(&dir), b"")?;
    let stage_json: Vec<Value> = stages
        .iter()
        .map(|s| {
            let mut v = s.to_json();
            v["status"] = json!("pending");
            v
        })
        .collect();
    let job = json!({
        "id": id,
        "name": name,
        "kind": kind,
        "created_unix": now(),
        "state": "queued",
        "heartbeat_unix": now(),
        "finished_unix": null,
        "error": null,
        "work_dir": work_dir.to_string_lossy(),
        "output": output.to_string_lossy(),
        "notes": notes,
        "stages": stage_json,
        "result": null,
    });
    save(&dir, &job)?;
    ledger_append(
        data,
        &json!({"ts": now(), "event": "created", "job": id, "name": name, "kind": kind}),
    )?;
    Ok((id, dir))
}

/// The state to report, accounting for dead or never-started supervisors.
pub fn effective_state(job: &Value, at: u64) -> String {
    let state = job
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let beat = job
        .get("heartbeat_unix")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let created = job.get("created_unix").and_then(Value::as_u64).unwrap_or(0);
    match state {
        "running" if at.saturating_sub(beat) > STALE_SECS => "lost".into(),
        "queued" if at.saturating_sub(created) > NOT_STARTED_SECS => "not_started".into(),
        other => other.to_string(),
    }
}

pub fn is_terminal(state: &str) -> bool {
    matches!(state, "succeeded" | "failed" | "cancelled")
}

/// Last `n` lines of a log, each capped in length.
pub fn tail(path: &Path, n: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map_or(0, |m| m.len());
    let start = len.saturating_sub(64 * 1024);
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(start)).is_err() || f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    // Progress bars rewrite one line with carriage returns; show the latest.
    let lines: Vec<String> = text
        .lines()
        .map(|l| {
            let last = l.rsplit('\r').next().unwrap_or(l);
            last.chars().take(300).collect::<String>()
        })
        .filter(|l| !l.trim().is_empty())
        .collect();
    let skip = lines.len().saturating_sub(n);
    lines.into_iter().skip(skip).collect()
}

pub fn status(dir: &Path, tail_lines: usize) -> Result<Value> {
    let job = load(dir)?;
    let state = effective_state(&job, now());
    let stages: Vec<Value> = job
        .get("stages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|s| {
                    json!({
                        "name": s.get("name"),
                        "status": s.get("status"),
                        "exit_code": s.get("exit_code"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "id": job.get("id"),
        "name": job.get("name"),
        "kind": job.get("kind"),
        "state": state,
        "recorded_state": job.get("state"),
        "error": job.get("error"),
        "created_unix": job.get("created_unix"),
        "finished_unix": job.get("finished_unix"),
        "output": job.get("output"),
        "work_dir": job.get("work_dir"),
        "notes": job.get("notes"),
        "stages": stages,
        "result": job.get("result"),
        "log_tail": tail(&log_path(dir), tail_lines),
    }))
}

/// Newest-first summaries of up to `limit` jobs.
pub fn list(data: &Path, limit: usize) -> Result<Vec<Value>> {
    let root = data.join("jobs");
    let mut ids: Vec<String> = match std::fs::read_dir(&root) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    ids.sort();
    ids.reverse();
    let at = now();
    Ok(ids
        .into_iter()
        .take(limit)
        .filter_map(|id| {
            let job = load(&root.join(&id)).ok()?;
            Some(json!({
                "id": id,
                "name": job.get("name"),
                "state": effective_state(&job, at),
                "created_unix": job.get("created_unix"),
                "output": job.get("output"),
            }))
        })
        .collect())
}

pub fn cancel_requested(dir: &Path) -> bool {
    dir.join("cancel").exists()
}

/// Ask a job to stop. A live supervisor notices the marker; a dead or
/// never-started one is marked cancelled here, since nobody else will.
pub fn request_cancel(dir: &Path) -> Result<Value> {
    let mut job = load(dir)?;
    let state = effective_state(&job, now());
    if is_terminal(&state) {
        return Err(TrainError::Invalid(format!("job already {state}")));
    }
    std::fs::write(dir.join("cancel"), now().to_string())?;
    if matches!(state.as_str(), "lost" | "not_started") {
        job["state"] = json!("cancelled");
        job["finished_unix"] = json!(now());
        job["error"] = json!("cancelled (supervisor was not running)");
        save(dir, &job)?;
    }
    status(dir, 5)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::plan::Kind;

    fn stage(name: &str) -> Stage {
        Stage {
            name: name.into(),
            kind: Kind::Mkdir(PathBuf::from("/x")),
            timeout_secs: 5,
        }
    }

    fn make(data: &Path) -> (String, PathBuf) {
        create(
            data,
            "t",
            "video_to_splat",
            &[stage("a"), stage("b")],
            Path::new("/w"),
            Path::new("/o"),
            &[],
        )
        .unwrap()
    }

    #[test]
    fn ids_are_checked() {
        assert!(check_id("j1-abcd").is_ok());
        for bad in ["", "../x", "a/b", "a b", &"x".repeat(65)] {
            assert!(check_id(bad).is_err(), "{bad}");
        }
        assert!(job_dir(Path::new("/d"), "..").is_err());
    }

    #[test]
    fn create_load_status_and_ledger() {
        let d = tempfile::tempdir().unwrap();
        let (id, dir) = make(d.path());
        let st = status(&dir, 5).unwrap();
        assert_eq!(st["id"], id.as_str());
        assert_eq!(st["state"], "queued");
        assert_eq!(st["stages"].as_array().unwrap().len(), 2);
        let ledger = std::fs::read_to_string(d.path().join("jobs.jsonl")).unwrap();
        assert!(ledger.contains("\"event\":\"created\""));
        assert_eq!(list(d.path(), 10).unwrap().len(), 1);
    }

    #[test]
    fn stale_heartbeat_means_lost_and_unstarted_means_not_started() {
        let at = 10_000;
        let running =
            json!({"state": "running", "heartbeat_unix": at - STALE_SECS - 1, "created_unix": 0});
        assert_eq!(effective_state(&running, at), "lost");
        let fresh = json!({"state": "running", "heartbeat_unix": at - 5, "created_unix": 0});
        assert_eq!(effective_state(&fresh, at), "running");
        let queued = json!({"state": "queued", "heartbeat_unix": 0, "created_unix": at - 61});
        assert_eq!(effective_state(&queued, at), "not_started");
        let done = json!({"state": "succeeded", "heartbeat_unix": 0, "created_unix": 0});
        assert_eq!(effective_state(&done, at), "succeeded");
    }

    #[test]
    fn cancel_marks_dead_jobs_and_rejects_finished_ones() {
        let d = tempfile::tempdir().unwrap();
        let (_, dir) = make(d.path());
        let mut job = load(&dir).unwrap();
        job["state"] = json!("running");
        job["heartbeat_unix"] = json!(1);
        save(&dir, &job).unwrap();
        let st = request_cancel(&dir).unwrap();
        assert_eq!(st["state"], "cancelled");
        assert!(cancel_requested(&dir));
        assert!(request_cancel(&dir).is_err());
    }

    #[test]
    fn tail_keeps_last_lines_and_latest_carriage_return_segment() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("log.txt");
        std::fs::write(&p, "one\ntwo\nstep 1\rstep 2\rstep 3\n\nlast\n").unwrap();
        assert_eq!(tail(&p, 3), ["two", "step 3", "last"]);
        assert!(tail(&d.path().join("none"), 3).is_empty());
    }
}
