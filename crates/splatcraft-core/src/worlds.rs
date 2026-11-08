//! `worlds` op: World Labs Marble generation with a spend guard.
//!
//! Actions: `ledger`, `credits`, `generate`, `status`, `fetch`, `list`.
//! `generate` is a dry run unless `confirm_spend` is true. Every confirmed
//! submit is appended to `marble-ledger.jsonl` and counts against a
//! cumulative cap (`SPLATCRAFT_MARBLE_CAP`, default 4500 credits).

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};
use splat_worlds::assets::{self, Which};
use splat_worlds::client::{HttpTransport, Marble, Transport};
use splat_worlds::ledger::{check_cap, Ledger, DEFAULT_CAP};
use splat_worlds::request::{estimate_credits, GenerateRequest, Kind, Source, DEFAULT_MODEL};
use splat_worlds::WorldsError;

use crate::error::{Error, Result};
use crate::paths::{resolve, str_arg};

const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;
const MAX_WAIT_SECS: u64 = 3600;

impl From<WorldsError> for Error {
    fn from(e: WorldsError) -> Self {
        match e {
            WorldsError::Auth(m) => Error::Auth(m),
            WorldsError::Network(m) => Error::Network(m),
            WorldsError::Api { .. } => Error::Api(e.to_string()),
            WorldsError::Invalid(m) => Error::Invalid(m),
            WorldsError::CapExceeded(m) => Error::CapExceeded(m),
            WorldsError::Io(m) => Error::Io(m),
        }
    }
}

pub(crate) fn data_dir() -> Result<PathBuf> {
    let raw = std::env::var("SPLATCRAFT_DATA")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "data".to_string());
    resolve(&raw)
}

fn cap() -> u64 {
    std::env::var("SPLATCRAFT_MARBLE_CAP")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(DEFAULT_CAP)
}

fn opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>> {
    if args.get(key).is_none_or(Value::is_null) {
        Ok(None)
    } else {
        str_arg(args, key).map(Some)
    }
}

fn flag(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Entry point used by the dispatch bus (real network).
pub fn worlds(args: &Value) -> Result<Value> {
    run(args, || {
        Ok(HttpTransport::with_key(crate::secrets::worlds_key()?)?)
    })
}

/// Testable core: `make` builds the transport only when a call needs one.
pub fn run<T: Transport>(args: &Value, make: impl FnOnce() -> Result<T>) -> Result<Value> {
    let action = str_arg(args, "action")?;
    match action {
        "ledger" => ledger_view(),
        "generate" => generate(args, make),
        "credits" => Ok(json!({"success": true, "credits": Marble { t: make()? }.credits()?})),
        "status" => status(args, make),
        "fetch" => fetch(args, make),
        "list" => list(args, make),
        other => Err(Error::Invalid(format!(
            "unknown worlds action '{other}' (known: ledger, credits, generate, status, fetch, list)"
        ))),
    }
}

fn ledger_view() -> Result<Value> {
    let l = Ledger::in_dir(&data_dir()?);
    let (entries, bad) = l.entries()?;
    let spent = l.spent()?;
    let cap = cap();
    Ok(json!({
        "success": true,
        "ledger": l.path().to_string_lossy(),
        "spent_credits": spent,
        "cap_credits": cap,
        "headroom_credits": cap.saturating_sub(spent),
        "entries": entries.len(),
        "unreadable_lines": bad,
        "recent": entries.iter().rev().take(5).collect::<Vec<_>>(),
    }))
}

fn op_id(resp: &Value) -> Option<String> {
    ["operation_id", "id", "name"]
        .iter()
        .find_map(|k| resp.get(*k).and_then(Value::as_str))
        .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
}

fn build_request(args: &Value) -> Result<(GenerateRequest, Option<PathBuf>)> {
    let kind = match opt_str(args, "kind")? {
        Some(k) => Kind::parse(k)
            .ok_or_else(|| Error::Invalid("kind must be text, image or video".into()))?,
        None => Kind::Text,
    };
    let url = opt_str(args, "url")?;
    let file = opt_str(args, "file")?;
    if url.is_some() && file.is_some() {
        return Err(Error::Invalid("give either url or file, not both".into()));
    }
    let (source, file) = match (url, file) {
        (Some(u), None) => (Source::Uri(u.to_string()), None),
        (None, Some(f)) => {
            let p = resolve(f)?;
            let meta = std::fs::metadata(&p)?;
            if meta.len() > MAX_UPLOAD_BYTES {
                return Err(Error::Invalid(
                    "file exceeds the 100 MB upload limit".into(),
                ));
            }
            // Placeholder id, replaced after the upload on a confirmed run.
            (Source::MediaAsset("pending-upload".into()), Some(p))
        }
        _ => (Source::None, None),
    };
    let req = GenerateRequest {
        kind,
        prompt: opt_str(args, "prompt")?.map(str::to_string),
        source,
        name: opt_str(args, "name")?
            .unwrap_or("splatcraft world")
            .to_string(),
        model: opt_str(args, "model")?.unwrap_or(DEFAULT_MODEL).to_string(),
        is_pano: flag(args, "is_pano"),
        disable_recaption: flag(args, "disable_recaption"),
        seed: args.get("seed").and_then(Value::as_u64),
        tags: args
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    };
    req.validate()?;
    Ok((req, file))
}

fn generate<T: Transport>(args: &Value, make: impl FnOnce() -> Result<T>) -> Result<Value> {
    let (mut req, file) = build_request(args)?;
    let cost = estimate_credits(&req.model)?;
    let ledger = Ledger::in_dir(&data_dir()?);
    let spent = ledger.spent()?;
    // A per-call limit may only lower the cap, never raise it.
    let cap = args
        .get("max_credits")
        .and_then(Value::as_u64)
        .map_or(cap(), |m| m.min(cap()));
    check_cap(spent, cost, cap)?;
    if !flag(args, "confirm_spend") {
        return Ok(json!({
            "success": true,
            "dry_run": true,
            "note": "nothing was sent and no credits were spent; pass confirm_spend=true to submit",
            "est_credits": cost,
            "model": req.model,
            "spent_credits": spent,
            "cap_credits": cap,
            "headroom_after": cap - spent - cost,
            "uploads_file": file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "body": req.body()?,
        }));
    }
    let m = Marble { t: make()? };
    if let Some(p) = file {
        let bytes = std::fs::read(&p)?;
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload")
            .to_string();
        let id = m.upload(&name, req.kind.as_str(), &ext, &bytes)?;
        req.source = Source::MediaAsset(id);
    }
    let resp = m.generate(&req)?;
    // Written only after the POST succeeded, so a failed submit costs nothing.
    ledger.append(&json!({
        "event": "submit",
        "est_credits": cost,
        "model": req.model,
        "kind": req.kind.as_str(),
        "name": req.name,
        "operation_id": op_id(&resp),
    }))?;
    Ok(json!({
        "success": true,
        "dry_run": false,
        "est_credits": cost,
        "spent_credits": spent + cost,
        "cap_credits": cap,
        "operation_id": op_id(&resp),
        "response": resp,
        "next": "poll with action=status (wait=true), then action=fetch",
    }))
}

fn status<T: Transport>(args: &Value, make: impl FnOnce() -> Result<T>) -> Result<Value> {
    let id = str_arg(args, "operation_id")?;
    let m = Marble { t: make()? };
    let op = if flag(args, "wait") {
        let secs = args
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(600)
            .min(MAX_WAIT_SECS);
        let every = args
            .get("interval_secs")
            .and_then(Value::as_u64)
            .unwrap_or(15)
            .clamp(1, 120);
        m.wait(id, Duration::from_secs(secs), Duration::from_secs(every))?
    } else {
        m.operation(id)?
    };
    let v = splat_worlds::client::view_operation(&op);
    let mut out = json!({
        "success": true,
        "operation_id": id,
        "done": v.done,
        "error": v.error,
        "progress": v.progress,
    });
    if v.done && v.error.is_none() {
        if let Some(world) = op.get("response") {
            let a = assets::extract(world);
            out["world_id"] = world.get("id").cloned().unwrap_or(Value::Null);
            out["caption"] = json!(a.caption);
            out["assets"] = json!({
                "100k": a.splat_100k.is_some(), "500k": a.splat_500k.is_some(),
                "full": a.splat_full.is_some(), "mesh": a.mesh.is_some(),
                "pano": a.pano.is_some(), "thumbnail": a.thumbnail.is_some(),
            });
        }
    }
    Ok(out)
}

fn fetch<T: Transport>(args: &Value, make: impl FnOnce() -> Result<T>) -> Result<Value> {
    let dest = resolve(str_arg(args, "dest")?)?;
    let which: Vec<Which> = match args.get("assets").and_then(Value::as_array) {
        Some(a) => a
            .iter()
            .map(|x| {
                x.as_str().and_then(Which::parse).ok_or_else(|| {
                    Error::Invalid("assets entries: 100k, 500k, full, mesh, pano, thumbnail".into())
                })
            })
            .collect::<Result<_>>()?,
        None => vec![Which::Splat500k, Which::Mesh],
    };
    let m = Marble { t: make()? };
    let (world, tag) = match (opt_str(args, "operation_id")?, opt_str(args, "world_id")?) {
        (Some(op_id), None) => {
            let op = m.operation(op_id)?;
            if !splat_worlds::client::view_operation(&op).done {
                return Err(Error::Invalid("operation is not finished yet".into()));
            }
            let w = op
                .get("response")
                .cloned()
                .ok_or_else(|| Error::Invalid("finished operation has no world".into()))?;
            (w, op_id.to_string())
        }
        (None, Some(w)) => (m.world(w)?, w.to_string()),
        _ => {
            return Err(Error::Invalid(
                "give exactly one of operation_id or world_id".into(),
            ))
        }
    };
    let urls = assets::extract(&world);
    let results = assets::fetch(&m.t, &urls, &which, &dest, &tag, flag(args, "overwrite"))?;
    Ok(json!({"success": true, "dest": dest.to_string_lossy(), "assets": results}))
}

fn list<T: Transport>(args: &Value, make: impl FnOnce() -> Result<T>) -> Result<Value> {
    let size = args
        .get("page_size")
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .clamp(1, 100);
    let mut body = json!({"page_size": size, "sort_by": "created_at", "status": "SUCCEEDED"});
    if let Some(t) = opt_str(args, "page_token")? {
        body["page_token"] = json!(t);
    }
    let m = Marble { t: make()? };
    Ok(json!({"success": true, "worlds": m.t.post_json("/worlds:list", &body)?}))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use splat_worlds::client::fake::Fake;
    // These tests mutate process environment; they share one lock with the
    // other core tests that do so.

    fn with_data<R>(cap: Option<&str>, f: impl FnOnce(&std::path::Path) -> R) -> R {
        let _g = crate::ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("SPLATCRAFT_DATA", d.path());
        std::env::remove_var("SPLATCRAFT_ROOT");
        match cap {
            Some(c) => std::env::set_var("SPLATCRAFT_MARBLE_CAP", c),
            None => std::env::remove_var("SPLATCRAFT_MARBLE_CAP"),
        }
        f(d.path())
    }

    fn no_net() -> Result<Fake> {
        Err(Error::Invalid("network must not be touched".into()))
    }

    #[test]
    fn dry_run_never_touches_network_or_ledger() {
        with_data(None, |d| {
            let r = run(&json!({"action": "generate", "prompt": "a cave"}), no_net).unwrap();
            assert_eq!(r["dry_run"], true);
            assert_eq!(r["est_credits"], 1500);
            assert_eq!(r["headroom_after"], 3000);
            assert!(!d.join("marble-ledger.jsonl").exists());
        });
    }

    #[test]
    fn confirmed_submit_is_ledgered_and_cap_enforced() {
        with_data(Some("3000"), |_| {
            let mk = || {
                let f = Fake::default();
                *f.post_reply.borrow_mut() = Some(json!({"operation_id": "op9"}));
                Ok(f)
            };
            let a = json!({"action": "generate", "prompt": "a cave", "confirm_spend": true});
            let r = run(&a, mk).unwrap();
            assert_eq!(r["operation_id"], "op9");
            assert_eq!(r["spent_credits"], 1500);
            run(&a, mk).unwrap();
            // Third would reach 4500 > cap 3000.
            let e = run(&a, mk).unwrap_err();
            assert_eq!(e.error_type(), "cap_exceeded");
            let l = run(&json!({"action": "ledger"}), no_net).unwrap();
            assert_eq!(l["spent_credits"], 3000);
            assert_eq!(l["entries"], 2);
        });
    }

    #[test]
    fn max_credits_can_only_lower_the_cap() {
        with_data(Some("3000"), |_| {
            let low = json!({"action": "generate", "prompt": "x", "max_credits": 1000});
            assert_eq!(run(&low, no_net).unwrap_err().error_type(), "cap_exceeded");
            // Raising above the env cap has no effect.
            let high = json!({"action": "generate", "prompt": "x", "max_credits": 99999});
            assert_eq!(run(&high, no_net).unwrap()["cap_credits"], 3000);
        });
    }

    #[test]
    fn failed_submit_costs_nothing() {
        with_data(None, |_| {
            let mk = || Ok(Fake::default()); // post has no script, so it errors
            let a = json!({"action": "generate", "prompt": "x", "confirm_spend": true});
            assert!(run(&a, mk).is_err());
            let l = run(&json!({"action": "ledger"}), no_net).unwrap();
            assert_eq!(l["spent_credits"], 0);
        });
    }

    #[test]
    fn status_reports_assets_when_done() {
        with_data(None, |_| {
            let mk = || {
                let f = Fake::default();
                f.queue_get(
                    "/operations/op1",
                    json!({"done": true, "response": {"id": "w1", "assets": {
                        "splats": {"spz_urls": {"500k": "https://cdn.test/a.spz"}}}}}),
                );
                Ok(f)
            };
            let r = run(&json!({"action": "status", "operation_id": "op1"}), mk).unwrap();
            assert_eq!(r["done"], true);
            assert_eq!(r["world_id"], "w1");
            assert_eq!(r["assets"]["500k"], true);
            assert_eq!(r["assets"]["mesh"], false);
        });
    }

    #[test]
    fn bad_input_is_rejected_before_the_network() {
        with_data(None, |_| {
            for a in [
                json!({}),
                json!({"action": "nope"}),
                json!({"action": "generate"}),
                json!({"action": "generate", "prompt": "x", "model": "m"}),
                json!({"action": "generate", "kind": "image", "url": "http://x.test/a.jpg"}),
                json!({"action": "generate", "kind": "image", "url": "https://a.test/a.jpg", "file": "b.jpg"}),
                json!({"action": "fetch", "dest": "d"}),
            ] {
                assert!(run(&a, no_net).is_err(), "{a}");
            }
        });
    }
}
