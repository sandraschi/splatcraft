# SplatCraft PRD

**Repo:** `splatcraft` (Pure-Rust ArtCraft pattern, 9 crates)
**Version:** 0.1.0 (workspace.package)
**Transport:** MCP 2024-11-05 stdio (`splatcraft-cli mcp`); no control port claimed
**Crates:** `splat-formats` (PLY/SPZ codecs) / `splat-ops` (crop, prune,
transform, merge, reduce_sh) / `splat-view` (`LiveScene` via wgpu-3dgs-viewer
0.8) / `splat-worlds` (Marble client, fake-transport tests) / `splat-train`
(tool locator, planner, supervisor) / `craft-secrets` (env first, OS store
second) / `splatcraft-core` (L1 `dispatch`) / `splatcraft-mcp` (L2 CLI+MCP,
hidden `job-run`) / `splatcraft-app` (L3 egui viewport)

## 1. Problem

Gaussian-splat work is split across converters, viewers, the Marble web
credit flow, and trainer CLIs (ffmpeg, COLMAP, Brush) with incompatible
flags. A single local bus should inspect, convert, edit, merge, render,
generate, and train splats without moving secrets through the LLM context.

## 2. What works today (nothing else is claimed)

- Binary PLY read/write (3DGS layout, SH degree 0-3); SPZ v2/v3 read, v3
  write; SPZ v4 (zstd) refused with a clear error.
- `inspect`, `convert` (atomic write, no overwrite unless asked),
  `engine_status` (env, PATH, tools dir; nothing executed).
- `edit`: crop, prune, transform (translate, uniform scale, axis-angle
  rotation), `reduce_sh`, all in RDF. Rotating SH degree > 0 is refused;
  run `reduce_sh` to degree 0 first.
- `merge`: 2-64 files, mixed PLY/SPZ, SH reduced to the lowest degree.
- `render`: headless GPU PNG still (opaque black); `unsupported` with no
  GPU adapter.
- `worlds`: Marble ledger/credits/generate/status/fetch/list behind a
  `Transport` trait; scripted fake only, no live call yet. Spend guard:
  dry run unless `confirm_spend` is true, JSONL ledger after POST, cap
  `SPLATCRAFT_MARBLE_CAP` (default 4500), per-call `max_credits` can only
  lower it. See `SECRETS_STANDARD.md`: no MCP tool or bus op touches
  secrets; only `splatcraft-cli secret set|status|delete`.
- `train`: plan/start/status/log/cancel/list over ffmpeg + COLMAP + Brush
  with `--help` flag discovery, detached supervisor, heartbeat 5 s, stale
  45 s means `lost`. Layout `<data>/jobs/<id>/`, event log `jobs.jsonl`.
- L3 viewport: path box, Load, orbit drag, wheel zoom, double-click reset
  through the same `LiveScene` as headless `render`.

## 3. Non-goals

No SH rotation math before `reduce_sh`; no live Marble spend without owner
confirmation; no committed binaries or engines in the repo
(`SPLATCRAFT_TOOLS` lives outside, Goliath default `D:\Dev\tools\splatcraft`);
no second trainer before Brush flags are stable.

## 4. Rules (from AGENTS.md, binding here too)

LF line endings only; no em dashes in docs, code, or commits; realistic
day-scale timelines; single trunk `master` (short-lived `agent/<task>`
branches only, merged and deleted same day); install requests follow
`INSTALL.md` agent steps exactly.

## 5. Gates

`just fmt-check`, `just check`, `just test`, `just build-release`, then
`splatcraft --smoke`. `just ci` runs the first three. Render tests skip
with a stderr note when no GPU adapter exists.

## 6. UI/UX Architecture & Menu Bar Standard

In accordance with [`CRAFT_MENU_BAR_STANDARD.md`](../../mcp-central-docs/standards/rust-artcraft/CRAFT_MENU_BAR_STANDARD.md):
- **Pattern 2 (Viewport-Centric Craft Workstation):** Persistent 3D Gaussian Splatting viewport (`LiveScene` wgpu 3dgs renderer) with surrounding camera controls, orbit gizmos, and point-cloud inspectors. Top quick button row acts as accelerators into the `View` menu. To add a new splat tool or training panel:
  1. Register a visibility toggle in the `View` or `Window` menu.
  2. Add an entry in the sidebar Transform / Filter toolbox.
  3. Wire the action directly into the universal `Command::dispatch` bus.
- **Top Menu Bar Hierarchy:**
  - `File`: New Scene (`Ctrl+N`), Open Splat PLY/SPZ (`Ctrl+O`), Open Recent ▸, Save (`Ctrl+S`), Save As... (`Ctrl+Shift+S`), Export Still / Video (`Ctrl+E`), Close (`Ctrl+W`), Quit (`Ctrl+Q`).
  - `Edit`: Undo (`Ctrl+Z`), Redo (`Ctrl+Y`), Cut, Copy, Paste, Select All, Preferences (`Ctrl+,`).
  - `View`: 3D Splat Viewport, Orbit Camera HUD, Point Cloud Density Inspector, Training Jobs Monitor, Marble Worlds Ledger, Theme (`Dark`/`Light`).
  - `Insert`: Import Additional Splat (Merge), Add Bounding Crop Box, Load Camera Path, Add Training Footage.
  - `Modify`: Crop Selection, Prune Low-Opacity Splats, Transform (Translate/Rotate/Scale), Reduce SH Degree.
  - `Commands`: Run Headless Render, Plan Training Job (COLMAP/Brush), Convert PLY <-> SPZ, Check Marble Credits, Cancel Training Job.
  - `Window`: Reset Layout, 3D Viewport, Job Supervisor Drawer, Inspector Dock.
  - `Help` (Mandatory 5-Dialog Suite): `Command Palette...` (`Ctrl+K`), `Keyboard Shortcuts...` (`F1`), `Project Files & Docs...`, `Licenses & Notices...`, `About SplatCraft`.
- **Behavioral Mechanics:** Smooth `switch_on_hover` between top-level menus, zero-unsafe immediate-mode rendering, and headless `--smoke` parity.
