# splatcraft

**3D Gaussian Splat & Radiance Field manipulation CLI and MCP server.**

`splatcraft` is part of the fleet's Craft-pattern application stable (`artcraft`, `gamecraft`, `travelcraft`, `solidcraft`). It unifies cloud radiance field generation (World Labs Marble API) and zero-cost local RTX 4090 training (Nerfstudio / Splatfacto / Postshot) into a headless CLI and stdio MCP server.

---

## Features & Verbs

* **`splatcraft info --file <scene.ply|spz>`**: Fast header inspection (<10ms). Emits splat count, bounding box, Spherical Harmonics degree, and byte size.
* **`splatcraft convert --in <in.ply> --to spz --out <out.spz>`**: Headless format conversion & compression.
* **`splatcraft collider --in <scene.ply> --out <collider.glb>`**: Extracts oriented bounding boxes and collision meshes for import into `gamecraft` / Godot.
* **`splatcraft create --prompt "..." [--provider marble]`**: Cloud radiance field generation.
* **`splatcraft train --video <vid.mp4> [--engine nerfstudio]`**: Local RTX 4090 3DGS training.
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
