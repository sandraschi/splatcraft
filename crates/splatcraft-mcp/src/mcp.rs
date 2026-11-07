//! Hand-rolled MCP (2024-11-05) stdio loop. One JSON-RPC message per line.

use std::io::{BufRead, Write};

use serde_json::{json, Value};
use splatcraft_core::{dispatch, error_json};

const PROTOCOL: &str = "2024-11-05";

/// tool name -> (core op, description, input schema)
pub fn tools_list() -> Value {
    json!([
        {
            "name": "splat_inspect",
            "description": "Summarise a Gaussian splat file (.ply or .spz): count, bounding box, SH degree, opacity and scale stats, non-finite count.",
            "inputSchema": {
                "type": "object",
                "properties": {"path": {"type": "string", "description": "Path to a .ply or .spz file"}},
                "required": ["path"]
            }
        },
        {
            "name": "splat_convert",
            "description": "Convert between .ply and .spz (SPZ v3 output; v2/v3 input). SPZ is lossy. Refuses to overwrite unless overwrite=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string"},
                    "overwrite": {"type": "boolean", "default": false}
                },
                "required": ["input", "output"]
            }
        },
        {
            "name": "splat_engine_status",
            "description": "Report which external engines (brush, ns-train, colmap, ffmpeg, splat-transform, bunx) are present on PATH. Honest up/down, no execution.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "splat_synth",
            "description": "Write synthetic random splats to a .ply or .spz file for tests and smoke runs. Not a real scene.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "count": {"type": "integer", "default": 1000},
                    "sh_degree": {"type": "integer", "minimum": 0, "maximum": 3, "default": 0},
                    "seed": {"type": "integer", "default": 1},
                    "overwrite": {"type": "boolean", "default": false}
                },
                "required": ["path"]
            }
        }
    ])
}

fn op_for(tool: &str) -> Option<&'static str> {
    match tool {
        "splat_inspect" => Some("inspect"),
        "splat_convert" => Some("convert"),
        "splat_engine_status" => Some("engine_status"),
        "splat_synth" => Some("synth"),
        _ => None,
    }
}

pub fn call_tool(name: &str, args: &Value) -> Value {
    let result = match op_for(name) {
        Some(op) => dispatch(op, args).unwrap_or_else(|e| error_json(&e)),
        None => {
            json!({"success": false, "error": format!("unknown tool '{name}'"), "error_type": "invalid"})
        }
    };
    let is_error = result.get("success") == Some(&Value::Bool(false));
    json!({
        "content": [{"type": "text", "text": serde_json::to_string(&result).unwrap_or_default()}],
        "isError": is_error
    })
}

/// Handle one parsed request. `None` means no response (notification).
pub fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let reply = |result: Value| {
        id.clone()
            .map(|i| json!({"jsonrpc": "2.0", "id": i, "result": result}))
    };
    let fail = |code: i64, text: &str| {
        id.clone()
            .map(|i| json!({"jsonrpc": "2.0", "id": i, "error": {"code": code, "message": text}}))
    };
    match method {
        "initialize" => reply(json!({
            "protocolVersion": PROTOCOL,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "splatcraft", "version": splatcraft_core::VERSION}
        })),
        "ping" => reply(json!({})),
        "tools/list" => reply(json!({"tools": tools_list()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            reply(call_tool(name, &args))
        }
        m if m.starts_with("notifications/") => None,
        "" => fail(-32600, "invalid request"),
        _ => fail(-32601, "method not found"),
    }
}

pub fn run_stdio() {
    eprintln!("splatcraft mcp {} ready on stdio", splatcraft_core::VERSION);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg),
            Err(_) => Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}),
            ),
        };
        if let Some(r) = resp {
            if writeln!(out, "{r}").is_err() || out.flush().is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn initialize_and_list() {
        let r = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize"})).unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL);
        let r = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "splat_inspect",
                "splat_convert",
                "splat_engine_status",
                "splat_synth"
            ]
        );
    }

    #[test]
    fn notification_gets_no_reply() {
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
    }

    #[test]
    fn unknown_method_and_tool() {
        let r = handle(&json!({"jsonrpc":"2.0","id":3,"method":"nope"})).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        let r =
            handle(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"zzz"}}))
                .unwrap();
        assert_eq!(r["result"]["isError"], true);
    }

    #[test]
    fn tool_call_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("t.spz").to_string_lossy().into_owned();
        let r = call_tool("splat_synth", &json!({"path": p, "count": 50}));
        assert_eq!(r["isError"], false);
        let r = call_tool("splat_inspect", &json!({"path": p}));
        assert_eq!(r["isError"], false);
        let text = r["content"][0]["text"].as_str().unwrap();
        let v: Value = serde_json::from_str(text).unwrap();
        assert_eq!(v["splats"]["count"], 50);
        let r = call_tool("splat_inspect", &json!({"path": "does/not/exist.ply"}));
        assert_eq!(r["isError"], true);
    }
}
