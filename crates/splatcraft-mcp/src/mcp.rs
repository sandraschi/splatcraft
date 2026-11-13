//! Hand-rolled MCP (2024-11-05) stdio loop. One JSON-RPC message per line.

use std::io::{BufRead, Write};

use serde_json::{json, Value};
use splatcraft_core::{dispatch, error_json};

const PROTOCOL: &str = "2024-11-05";

/// tool name -> (core op, description, input schema)
pub fn tools_list() -> Value {
    let mut list = json!([
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
            "name": "splat_edit",
            "description": "Edit a splat file with an ordered pipeline of steps, written to a new file. Edits run in RDF (x right, y down, z front). Steps: {op:'crop',min:[x,y,z],max:[x,y,z],invert?}, {op:'prune',min_opacity?(0..1),max_scale?,drop_non_finite?}, {op:'transform',translate?:[x,y,z],scale?,rotate?:{axis:[x,y,z],degrees}}, {op:'reduce_sh',degree:0..3}. Rotation of splats with SH degree > 0 is refused (SH rotation not built); reduce_sh to 0 first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string"},
                    "overwrite": {"type": "boolean", "default": false},
                    "steps": {"type": "array", "minItems": 1, "maxItems": 32, "items": {"type": "object", "properties": {"op": {"type": "string", "enum": ["crop", "prune", "transform", "reduce_sh"]}}, "required": ["op"]}}
                },
                "required": ["input", "output", "steps"]
            }
        },
        {
            "name": "splat_merge",
            "description": "Merge 2 to 64 splat files (.ply or .spz, mixed allowed) into one. If SH degrees differ, all are reduced to the lowest. Output is in the format of the output extension.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "inputs": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 64},
                    "output": {"type": "string"},
                    "overwrite": {"type": "boolean", "default": false}
                },
                "required": ["inputs", "output"]
            }
        },
        {
            "name": "splat_render",
            "description": "Render a still PNG of a splat file on the GPU (auto-framed orbit camera). Background is opaque black. Returns the output path and covered_fraction, not the image. Fails with 'unsupported' when no GPU adapter exists.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string", "description": "Must end in .png"},
                    "width": {"type": "integer", "minimum": 16, "maximum": 4096, "default": 960},
                    "height": {"type": "integer", "minimum": 16, "maximum": 4096, "default": 540},
                    "yaw_deg": {"type": "number", "default": 0},
                    "pitch_deg": {"type": "number", "default": 0},
                    "fov_deg": {"type": "number", "default": 60},
                    "distance_scale": {"type": "number", "default": 1},
                    "gaussian_scale": {"type": "number", "default": 1},
                    "overwrite": {"type": "boolean", "default": false}
                },
                "required": ["input", "output"]
            }
        },
        {
            "name": "splat_worlds",
            "description": "World Labs Marble world generation with a spend guard. Needs a World Labs API key (env WORLDLABS_API_KEY, or stored with `splatcraft-cli secret set worlds`) for everything except action=ledger and a dry-run generate. action: ledger (spend so far vs cap), credits (live balance, free), generate (DRY RUN unless confirm_spend=true; a marble-1.1 world costs about 1500 API credits; a cumulative cap, default 4500 via SPLATCRAFT_MARBLE_CAP, refuses further spend), status (operation_id, optional wait), fetch (download splats/mesh/pano by operation_id or world_id), list. API credits are separate from marble.worldlabs.ai web credits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["ledger", "credits", "generate", "status", "fetch", "list"]},
                    "kind": {"type": "string", "enum": ["text", "image", "video"], "default": "text"},
                    "prompt": {"type": "string"},
                    "url": {"type": "string", "description": "Public https URL of the image or video"},
                    "file": {"type": "string", "description": "Local image or video to upload (100 MB max)"},
                    "name": {"type": "string"},
                    "model": {"type": "string", "enum": ["marble-1.1", "marble-1.1-plus"], "default": "marble-1.1"},
                    "is_pano": {"type": "boolean"},
                    "disable_recaption": {"type": "boolean"},
                    "seed": {"type": "integer"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "confirm_spend": {"type": "boolean", "default": false},
                    "max_credits": {"type": "integer", "description": "Lowers the cap for this call; cannot raise it"},
                    "operation_id": {"type": "string"},
                    "world_id": {"type": "string"},
                    "wait": {"type": "boolean", "default": false},
                    "timeout_secs": {"type": "integer", "default": 600, "maximum": 3600},
                    "interval_secs": {"type": "integer", "default": 15},
                    "dest": {"type": "string", "description": "Directory for fetched assets"},
                    "assets": {"type": "array", "items": {"type": "string", "enum": ["100k", "500k", "full", "mesh", "pano", "thumbnail"]}, "default": ["500k", "mesh"]},
                    "overwrite": {"type": "boolean", "default": false},
                    "page_size": {"type": "integer", "default": 10},
                    "page_token": {"type": "string"}
                },
                "required": ["action"]
            }
        },
        {
            "name": "splat_train",
            "description": "Video, image folder or COLMAP dataset to a splat, as a detached job (ffmpeg frames, COLMAP poses, Brush training, then validated .ply or .spz output). action: plan (dry: resolves tools, checks their --help, shows the exact commands), start (writes a job and launches a supervisor that outlives this call), status (job_id; state running/succeeded/failed/cancelled/lost, per-stage status, log tail), log, cancel, list. Needs brush, and for video or images also colmap (and ffmpeg for video); see splat_engine_status. A real run takes minutes to hours on the GPU. Exactly one of video, images_dir, dataset.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["plan", "start", "status", "log", "cancel", "list"]},
                    "video": {"type": "string", "description": "Path to a video file (mp4, mov, ...)"},
                    "images_dir": {"type": "string", "description": "Folder of photos (at least 8 jpg or png)"},
                    "dataset": {"type": "string", "description": "Existing COLMAP or Nerfstudio dataset folder (skips ffmpeg and COLMAP)"},
                    "output": {"type": "string", "description": "Result path ending in .ply or .spz"},
                    "name": {"type": "string"},
                    "overwrite": {"type": "boolean", "default": false},
                    "target_frames": {"type": "integer", "minimum": 20, "maximum": 1500, "default": 150, "description": "Frames sampled from a video"},
                    "max_resolution": {"type": "integer", "minimum": 320, "maximum": 4096, "default": 1600},
                    "iters": {"type": "integer", "minimum": 100, "maximum": 200000, "default": 10000, "description": "Brush training steps; Brush's own default is 30000"},
                    "max_splats": {"type": "integer", "minimum": 10000, "maximum": 20000000, "default": 2000000},
                    "sh_degree": {"type": "integer", "minimum": 0, "maximum": 3, "default": 3},
                    "quality": {"type": "string", "enum": ["low", "medium", "high", "extreme"], "default": "medium", "description": "COLMAP reconstruction quality"},
                    "job_id": {"type": "string"},
                    "lines": {"type": "integer", "default": 15, "description": "Log lines to return"},
                    "limit": {"type": "integer", "default": 10}
                },
                "required": ["action"]
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
    ]);
    if let Some(tools) = list.as_array_mut() {
        tools.push(json!({
            "name": "splat_mesh",
            "description": "Write a GLB box collider from the finite positions in a .ply or .spz file. This is generated here. It is not Marble's downloaded collider.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string", "description": "Must end in .glb"},
                    "overwrite": {"type": "boolean", "default": false}
                },
                "required": ["input", "output"]
            }
        }));
        tools.push(json!({
            "name": "splat_recolor",
            "description": "Tint near-grey splats (SH DC channel spread at or below grey_delta, default 0.08) with a named recipe and write a texturecraft bridge JSON beside the output. action=list returns the recipes and needs no paths. Recipes: ghibli, cyberpunk, ova. Saturated splats are left unchanged. Higher SH bands on tinted splats are cleared.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list"]},
                    "input": {"type": "string"},
                    "output": {"type": "string"},
                    "recipe": {"type": "string", "enum": ["ghibli", "cyberpunk", "ova"]},
                    "grey_delta": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.08},
                    "overwrite": {"type": "boolean", "default": false}
                }
            }
        }));
        tools.push(json!({
            "name": "splat_recipe",
            "description": "Run an offline democraft recipe into a directory. name: info (inspect a synthetic scene), convert (ply to spz), collider (GLB box), recolor (ghibli tint of a grey scene plus texturecraft bridge).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "enum": ["info", "convert", "collider", "recolor"]},
                    "out": {"type": "string", "description": "Directory to write into"}
                },
                "required": ["name", "out"]
            }
        }));
    }
    list
}

fn op_for(tool: &str) -> Option<&'static str> {
    match tool {
        "splat_inspect" => Some("inspect"),
        "splat_convert" => Some("convert"),
        "splat_edit" => Some("edit"),
        "splat_merge" => Some("merge"),
        "splat_render" => Some("render"),
        "splat_worlds" => Some("worlds"),
        "splat_train" => Some("train"),
        "splat_engine_status" => Some("engine_status"),
        "splat_synth" => Some("synth"),
        "splat_mesh" => Some("mesh"),
        "splat_recolor" => Some("recolor"),
        "splat_recipe" => Some("recipe"),
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
                "splat_edit",
                "splat_merge",
                "splat_render",
                "splat_worlds",
                "splat_train",
                "splat_engine_status",
                "splat_synth",
                "splat_mesh",
                "splat_recolor",
                "splat_recipe"
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
        let out = d.path().join("e.ply").to_string_lossy().into_owned();
        let r = call_tool(
            "splat_edit",
            &json!({"input": p, "output": out, "steps": [{"op": "prune", "min_opacity": 0.5}]}),
        );
        assert_eq!(r["isError"], false);
    }
}
