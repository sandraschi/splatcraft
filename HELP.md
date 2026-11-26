# SplatCraft Help & Operator Guide

Pure-Rust Gaussian splat toolkit (`PLY`, `SPZ v2/v3`), edit pipeline, headless & live `wgpu` 3DGS renderer, World Labs Marble client with hard spend cap, detached video-to-splat training supervisor (`ffmpeg` -> `COLMAP` -> `Brush`), and TRELLIS.2 sidecar planner.

## 1. Get Started

1. **Check External Tools**: Run `splatcraft-cli engines` (or `cargo run -p splatcraft-mcp -- engines`) to check presence of `brush`, `colmap`, `ffmpeg`, `ns-train`, `splat-transform`, and `bunx` (set `SPLATCRAFT_TOOLS` if tools live outside `PATH`, e.g. `D:\Dev\tools\splatcraft`).
2. **Inspect & Convert a Splat**:
   ```powershell
   splatcraft-cli inspect --path scene.ply
   splatcraft-cli convert --input scene.ply --output scene.spz
   ```
3. **Edit & Render**:
   ```powershell
   splatcraft-cli edit --input a.ply --output b.spz --steps "@steps.json"
   splatcraft-cli render --input a.ply --output a.png --yaw-deg 30
   ```
4. **Open the Live 3D Viewport**: Run `splatcraft` (`cargo run -p splatcraft --bin splatcraft`), enter a `.ply`/`.spz` path, click **Load**, drag to orbit, scroll to zoom, double-click to reset.
5. **Plan a Training Run (Dry Run First)**:
   ```powershell
   splatcraft-cli train --args "@plan.json"   # {"action":"plan","video":"clip.mp4","output":"out.ply"}
   ```

## 2. Safety & Spend Guards

- **World Labs Marble Spend Cap**: `worlds` `generate` is a dry run unless `confirm_spend: true`. Confirmed submits are logged to `marble-ledger.jsonl` and capped by `SPLATCRAFT_MARBLE_CAP` (default `4500` credits).
- **Secret Isolation (`craft-secrets`)**: API keys come from `WORLDLABS_API_KEY` or Windows Credential Manager (`splatcraft-cli secret set worlds`). No MCP tool can read or write secrets.
- **SH Rotation Guard**: Rotating splats with SH degree > 0 is explicitly refused until you run `reduce_sh` to degree 0 first.
- **Path Confinement**: Set `SPLATCRAFT_ROOT` to restrict all file reads/writes beneath one directory.
- **No Overwrite by Default**: `convert` writes atomically and refuses to overwrite unless explicitly requested.

## 3. Crate Documentation

- Core & Codecs: [splatcraft-core](crates/splatcraft-core/README.md) · [splat-formats](crates/splat-formats/README.md) · [splat-ops](crates/splat-ops/README.md) · [craft-secrets](crates/craft-secrets/README.md)
- Rendering, Training & Worlds: [splat-view](crates/splat-view/README.md) · [splat-train](crates/splat-train/README.md) · [splat-worlds](crates/splat-worlds/README.md)
- CLI, MCP & Desktop: [splatcraft-mcp](crates/splatcraft-mcp/README.md) · [splatcraft-app](crates/splatcraft-app/README.md)
