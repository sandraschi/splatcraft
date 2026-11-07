//! Marble HTTP client behind a [`Transport`] trait, so everything above the
//! wire is testable without a network or an API key.

use std::io::Read;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::error::{Result, WorldsError};
use crate::request::GenerateRequest;

pub const BASE_URL: &str = "https://api.worldlabs.ai/marble/v1";
pub const KEY_ENV: &str = "WORLDLABS_API_KEY";

pub trait Transport {
    fn get_json(&self, path: &str) -> Result<Value>;
    fn post_json(&self, path: &str, body: &Value) -> Result<Value>;
    /// PUT raw bytes to a pre-signed URL (no API key is sent).
    fn put_bytes(&self, url: &str, headers: &[(String, String)], bytes: &[u8]) -> Result<()>;
    /// GET a pre-signed URL (no API key is sent), refusing bodies over `max`.
    fn get_bytes(&self, url: &str, max: u64) -> Result<Vec<u8>>;
}

/// Turn an HTTP error status into a typed error with an actionable message.
pub fn map_status(status: u16, body: &str) -> WorldsError {
    let detail = serde_json::from_str::<Value>(body).ok().and_then(|v| {
        v.pointer("/error/message")
            .or_else(|| v.get("detail"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let msg = detail.unwrap_or_else(|| body.chars().take(300).collect());
    match status {
        401 | 403 => WorldsError::Auth(format!(
            "World Labs rejected the API key ({status}). Check {KEY_ENV}. {msg}"
        )),
        402 => WorldsError::Api {
            status,
            message: format!(
                "payment required. API credits are separate from marble.worldlabs.ai web credits; \
                 check https://platform.worldlabs.ai/ . {msg}"
            ),
        },
        429 => WorldsError::Api {
            status,
            message: format!("rate limited, retry later. {msg}"),
        },
        _ => WorldsError::Api {
            status,
            message: msg,
        },
    }
}

pub struct HttpTransport {
    key: String,
    http: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn from_env() -> Result<HttpTransport> {
        let key = std::env::var(KEY_ENV).unwrap_or_default();
        if key.trim().is_empty() {
            return Err(WorldsError::Auth(format!("{KEY_ENV} is not set")));
        }
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        Ok(HttpTransport { key, http })
    }

    fn finish(resp: reqwest::blocking::Response) -> Result<Value> {
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(map_status(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| WorldsError::Api {
            status,
            message: format!("response was not JSON: {e}"),
        })
    }
}

fn require_https(url: &str) -> Result<()> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err(WorldsError::Invalid(
            "refusing a non-https download url".into(),
        ))
    }
}

impl Transport for HttpTransport {
    fn get_json(&self, path: &str) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{BASE_URL}{path}"))
            .header("WLT-Api-Key", &self.key)
            .send()
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        Self::finish(resp)
    }

    fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{BASE_URL}{path}"))
            .header("WLT-Api-Key", &self.key)
            .json(body)
            .send()
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        Self::finish(resp)
    }

    fn put_bytes(&self, url: &str, headers: &[(String, String)], bytes: &[u8]) -> Result<()> {
        require_https(url)?;
        let mut req = self.http.put(url).body(bytes.to_vec());
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req
            .send()
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(map_status(status, &resp.text().unwrap_or_default()))
        }
    }

    fn get_bytes(&self, url: &str, max: u64) -> Result<Vec<u8>> {
        require_https(url)?;
        let resp = self
            .http
            .get(url)
            .send()
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(map_status(status, ""));
        }
        let mut out = Vec::new();
        resp.take(max.saturating_add(1))
            .read_to_end(&mut out)
            .map_err(|e| WorldsError::Network(e.to_string()))?;
        if out.len() as u64 > max {
            return Err(WorldsError::Invalid(format!(
                "download exceeds {max} bytes"
            )));
        }
        Ok(out)
    }
}

/// A view of a Marble long-running operation.
#[derive(Debug, PartialEq)]
pub struct OpView {
    pub done: bool,
    pub error: Option<String>,
    pub progress: Option<String>,
}

pub fn view_operation(op: &Value) -> OpView {
    let done = op.get("done").and_then(Value::as_bool).unwrap_or(false);
    let error = op.get("error").filter(|e| !e.is_null()).map(|e| {
        e.get("message")
            .and_then(Value::as_str)
            .map_or_else(|| e.to_string(), str::to_string)
    });
    let progress = op
        .pointer("/metadata/progress")
        .filter(|p| !p.is_null())
        .map(Value::to_string);
    OpView {
        done,
        error,
        progress,
    }
}

fn check_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(WorldsError::Invalid(
            "id must be 1-128 chars of [A-Za-z0-9_-]".into(),
        ))
    }
}

pub struct Marble<T: Transport> {
    pub t: T,
}

impl<T: Transport> Marble<T> {
    pub fn credits(&self) -> Result<Value> {
        self.t.get_json("/credits")
    }

    pub fn generate(&self, req: &GenerateRequest) -> Result<Value> {
        self.t.post_json("/worlds:generate", &req.body()?)
    }

    pub fn operation(&self, id: &str) -> Result<Value> {
        check_id(id)?;
        self.t.get_json(&format!("/operations/{id}"))
    }

    pub fn world(&self, id: &str) -> Result<Value> {
        check_id(id)?;
        self.t.get_json(&format!("/worlds/{id}"))
    }

    /// Poll until the operation is done, fails, or `timeout` passes. A timeout
    /// returns the last operation state (not an error); the caller reports it.
    pub fn wait(&self, id: &str, timeout: Duration, interval: Duration) -> Result<Value> {
        let start = Instant::now();
        loop {
            let op = self.operation(id)?;
            let v = view_operation(&op);
            if v.done || v.error.is_some() || start.elapsed() >= timeout {
                return Ok(op);
            }
            std::thread::sleep(interval);
        }
    }

    /// Upload local bytes; returns the media asset id for use as a source.
    pub fn upload(&self, file_name: &str, kind: &str, ext: &str, bytes: &[u8]) -> Result<String> {
        let prep = self.t.post_json(
            "/media-assets:prepare_upload",
            &json!({"file_name": file_name, "kind": kind, "extension": ext}),
        )?;
        let id = prep
            .pointer("/media_asset/id")
            .and_then(Value::as_str)
            .ok_or_else(|| WorldsError::Api {
                status: 200,
                message: "prepare_upload response had no media_asset.id".into(),
            })?
            .to_string();
        let info = prep.get("upload_info").ok_or_else(|| WorldsError::Api {
            status: 200,
            message: "prepare_upload response had no upload_info".into(),
        })?;
        let url = info
            .get("upload_url")
            .and_then(Value::as_str)
            .ok_or_else(|| WorldsError::Api {
                status: 200,
                message: "upload_info had no upload_url".into(),
            })?;
        let headers: Vec<(String, String)> = info
            .get("required_headers")
            .or_else(|| info.get("headers"))
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        self.t.put_bytes(url, &headers, bytes)?;
        Ok(id)
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod fake {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Scripted transport: canned GET/POST replies, recorded calls.
    #[derive(Default)]
    pub struct Fake {
        pub gets: RefCell<HashMap<String, Vec<Value>>>,
        pub post_reply: RefCell<Option<Value>>,
        pub bytes: RefCell<HashMap<String, Vec<u8>>>,
        pub calls: RefCell<Vec<String>>,
    }

    impl Fake {
        pub fn queue_get(&self, path: &str, v: Value) {
            self.gets
                .borrow_mut()
                .entry(path.into())
                .or_default()
                .push(v);
        }
    }

    impl Transport for Fake {
        fn get_json(&self, path: &str) -> Result<Value> {
            self.calls.borrow_mut().push(format!("GET {path}"));
            let mut g = self.gets.borrow_mut();
            let q = g.get_mut(path).ok_or_else(|| WorldsError::Api {
                status: 404,
                message: format!("no script for {path}"),
            })?;
            if q.len() > 1 {
                Ok(q.remove(0))
            } else {
                q.first()
                    .cloned()
                    .ok_or(WorldsError::Network("empty".into()))
            }
        }
        fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
            self.calls.borrow_mut().push(format!("POST {path} {body}"));
            self.post_reply
                .borrow()
                .clone()
                .ok_or(WorldsError::Network("no post script".into()))
        }
        fn put_bytes(&self, url: &str, _h: &[(String, String)], b: &[u8]) -> Result<()> {
            self.calls
                .borrow_mut()
                .push(format!("PUT {url} {}", b.len()));
            Ok(())
        }
        fn get_bytes(&self, url: &str, max: u64) -> Result<Vec<u8>> {
            self.calls.borrow_mut().push(format!("GETB {url}"));
            let b = self
                .bytes
                .borrow()
                .get(url)
                .cloned()
                .ok_or(WorldsError::Api {
                    status: 404,
                    message: "no bytes".into(),
                })?;
            if b.len() as u64 > max {
                return Err(WorldsError::Invalid("too big".into()));
            }
            Ok(b)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::fake::Fake;
    use super::*;

    #[test]
    fn status_mapping() {
        assert!(matches!(map_status(401, "{}"), WorldsError::Auth(_)));
        assert!(matches!(map_status(403, ""), WorldsError::Auth(_)));
        let e = map_status(402, r#"{"error":{"message":"Insufficient credits."}}"#);
        assert!(e.to_string().contains("Insufficient credits."));
        assert!(e.to_string().contains("platform.worldlabs.ai"));
        assert!(map_status(429, "").to_string().contains("rate limited"));
        assert!(map_status(500, r#"{"detail":"boom"}"#)
            .to_string()
            .contains("boom"));
    }

    #[test]
    fn operation_views() {
        assert_eq!(
            view_operation(&json!({"done": false, "metadata": {"progress": {"pct": 40}}})),
            OpView {
                done: false,
                error: None,
                progress: Some(r#"{"pct":40}"#.into())
            }
        );
        assert!(view_operation(&json!({"done": true})).done);
        assert_eq!(
            view_operation(&json!({"done": true, "error": {"message": "filtered"}})).error,
            Some("filtered".into())
        );
    }

    #[test]
    fn ids_are_validated_before_use() {
        let m = Marble { t: Fake::default() };
        assert!(m.operation("../etc").is_err());
        assert!(m.operation("").is_err());
        assert!(m.world("a/b").is_err());
        assert!(m.t.calls.borrow().is_empty());
    }

    #[test]
    fn wait_polls_until_done() {
        let f = Fake::default();
        f.queue_get("/operations/op1", json!({"done": false}));
        f.queue_get("/operations/op1", json!({"done": false}));
        f.queue_get(
            "/operations/op1",
            json!({"done": true, "response": {"id": "w1"}}),
        );
        let m = Marble { t: f };
        let op = m
            .wait("op1", Duration::from_secs(5), Duration::ZERO)
            .unwrap();
        assert_eq!(op["response"]["id"], "w1");
        assert_eq!(m.t.calls.borrow().len(), 3);
    }

    #[test]
    fn wait_times_out_with_last_state() {
        let f = Fake::default();
        f.queue_get("/operations/op2", json!({"done": false}));
        let m = Marble { t: f };
        let op = m.wait("op2", Duration::ZERO, Duration::ZERO).unwrap();
        assert_eq!(op["done"], false);
    }

    #[test]
    fn upload_flow_returns_media_asset_id() {
        let f = Fake::default();
        *f.post_reply.borrow_mut() = Some(json!({
            "media_asset": {"id": "ma-1"},
            "upload_info": {"upload_url": "https://gcs.test/u", "required_headers": {"x-goog": "1"}}
        }));
        let m = Marble { t: f };
        let id = m.upload("a.png", "image", "png", b"12345").unwrap();
        assert_eq!(id, "ma-1");
        let calls = m.t.calls.borrow();
        assert!(calls[0].starts_with("POST /media-assets:prepare_upload"));
        assert_eq!(calls[1], "PUT https://gcs.test/u 5");
    }

    #[test]
    fn missing_key_is_an_auth_error() {
        // Only meaningful when the variable is unset in the test environment.
        if std::env::var(KEY_ENV).unwrap_or_default().trim().is_empty() {
            assert!(matches!(
                HttpTransport::from_env(),
                Err(WorldsError::Auth(_))
            ));
        }
    }
}
