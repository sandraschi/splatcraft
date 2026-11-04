# splatcraft

**3D Gaussian Splat & Radiance Field manipulation CLI and MCP server.**

`splatcraft` is part of the fleet's Craft-pattern application stable (`artcraft`, `gamecraft`, `travelcraft`, `solidcraft`). It unifies cloud radiance field generation (World Labs Marble API) and zero-cost local RTX 4090 training (Nerfstudio / Splatfacto / Postshot) into a headless CLI and stdio MCP server.

---

## Features & Verbs

* **`splatcraft info --file <scene.ply|spz>`**: Fast header inspection (<10ms). Emits splat count, bounding box, Spherical Harmonics degree, and byte size.
* **`splatcraft convert --in <in.ply> --to spz --out <out.spz>`**: Headless format conversion & compression (achieving standard 5x-10x compression).
* **`splatcraft collider --file <scene.ply> --out <collider.glb>`**: Extracts oriented bounding boxes and binary GLTF 2.0 (`.glb`) mesh colliders for direct import into `gamecraft` and Godot.
* **`splatcraft create --prompt "..." [--provider marble] [--dry-run]`**: Cloud radiance field generation via World Labs Marble API.
* **`splatcraft train --input <vid.mp4|images_dir> [--engine nerfstudio|postshot] [--dry-run]`**: Local 3DGS training runner on RTX 4090.
* **`splatcraft recipe all [--out ./outbox]`**: Executes Democraft headless benchmark recipes emitting `run.json` and markdown reports.
* **`splatcraft mcp [--allow-path <DIR>]`**: Stdio FastMCP server for AI agents.

---

## Quickstart

```bash
# Sync environment
uv sync

# Inspect a splat file
uv run splatcraft info --file sample.ply --json

# Run MCP server
uv run splatcraft mcp
```
