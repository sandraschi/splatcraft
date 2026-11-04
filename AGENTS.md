# AGENTS.md — splatcraft

**Purpose**: Guidance for coding agents working in the `splatcraft` repository.
**Scope**: 3D Gaussian Splats, radiance fields, and spatial world manipulation CLI + FastMCP server.

## Architectural Role

`splatcraft` is the Craft-pattern successor fusing:
- `worldlabs-mcp` (World Labs Marble API, Spark 2.0 streaming, Godot scene portals)
- `splatmaker-mcp` (local Nerfstudio/Postshot training on RTX 4090)

It adheres to the Craft standard defined in `mcp-central-docs/projects/craft-fleet-rebuild/PLAN.md` and `splatcraft-DOSSIER.md`:
1. **Single Model**: `SplatWorld` (3DGS point cloud, bounding box, camera trajectory, collision mesh).
2. **Headless Verbs**: `info`, `convert`, `collider`, `generate`, `train`, `render`.
3. **Files In / Out**: Reads/writes `.ply`, `.spz`, `.rad`, `.glb` colliders, and `run.json` artifacts in `outbox/`.
4. **Thin MCP Bridge**: FastMCP 3.x stdio transport for agent invocation.

## Conventions

- Python $\ge$ 3.11 with `uv`, `ruff`, and `justfile`.
- CLI entrypoint: `splatcraft` / `splatcraft-cli`.
- MCP stdio: `splatcraft mcp --allow-path <DIR>`.
- Never execute long training loops in unit tests or CI; mock subprocess probes or use offline fixture PLY files.
