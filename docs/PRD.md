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
  GPU adapter. A published capture (Marcel Padilla houseplant, CC-BY-4.0,
  113,648 splats, SH degree 0) has been drawn to
  `docs/screenshots/plant-front.png`. A local training run has not.
- `worlds`: Marble ledger/credits/generate/status/fetch/list behind a
  `Transport` trait; scripted fake only, no live call yet. Spend guard:
  dry run unless `confirm_spend` is true, JSONL ledger after POST, cap
  `SPLATCRAFT_MARBLE_CAP` (default 4500), per-call `max_credits` can only
  lower it. See `SECRETS_STANDARD.md`: no MCP tool or bus op touches
  secrets; only `splatcraft-cli secret set|status|delete`.
- `mesh`: GLB box collider from the file's own finite positions. Marble's
  downloaded collider stays a fetch asset.
- `recolor`: tints near-grey SH DC with `ghibli`, `cyberpunk`, or `ova`,
  clears higher SH bands on those splats, and writes a texturecraft bridge
  JSON beside the output. Saturated splats are left unchanged.
- `recipe`: offline democraft runs `info`, `convert`, `collider`, `recolor`.
- `train`: plan/start/status/log/cancel/list over ffmpeg + COLMAP + Brush
  with `--help` flag discovery, detached supervisor, heartbeat 5 s, stale
  45 s means `lost`. Layout `<data>/jobs/<id>/`, event log `jobs.jsonl`.
- L3 workstation (Tier 4): viewport-centric Craft Pattern 2 shell with the
  universal 8-menu bar (`File` `Edit` `View` `Insert` `Modify` `Commands`
  `Window` `Help`), the 5-dialog Help suite (Command Palette `Ctrl+K`,
  Shortcuts `F1`, Project Docs, Licenses, About) plus Preferences, a left
  Toolbox and right Inspector dock, and a quick-button row. Modify, Commands,
  and the toolbox call `dispatch(op, args)`. File New, Save As, Export Video,
  and some Insert items only set status text. Panels: inspect, convert,
  synth, an edit-pipeline builder (crop / prune / transform / reduce_sh with
  step list), merge, render, collider, recolor, engines probe, training jobs,
  and the Worlds ledger. GPU viewport draws through the same `LiveScene` as headless
  `render`; orbit drag, wheel zoom, double-click reset.
- L3 honesty: "Export Video" is refused with a message (render writes PNG
  stills only); the Worlds panel never passes `confirm_spend`; SH rotation
  refusal is surfaced as guidance, not hidden.
- L3 shell: `#![forbid(unsafe_code)]`, `harsh_visuals` / `crisp_light_visuals`
  with a vector sun/moon toggle (no tofu glyphs), `Ctrl+wheel` zoom, and a
  `glyph_allowlist` + `check_string_for_tofu` test pair.
- `--smoke` covers menu-dispatch parity, the tofu guard, and an end-to-end
  synth -> convert -> inspect -> edit -> merge -> worlds-ledger -> mesh ->
  recolor-recipe run; `render` passes either as a PNG or as a clean
  `unsupported` skip with no GPU.

## 3. Non-goals

No SH rotation math before `reduce_sh`; no live Marble spend without owner
confirmation; no committed binaries or engines in the repo
(`SPLATCRAFT_TOOLS` lives outside, Goliath default `D:\Dev\tools\splatcraft`);
no second trainer before Brush flags are stable; no NSIS installer. The
release is the two executables and the portable zip.

## 4. Rules (from AGENTS.md, binding here too)

LF line endings only; no em dashes in docs, code, or commits; a plan row is
about an hour of agent work; single trunk `master` (short-lived `agent/<task>`
branches only, merged and deleted same day); install requests follow
`INSTALL.md` agent steps exactly.

## 5. Gates

`just fmt-check`, `just check`, `just test`, `just build-release`, then
`splatcraft --smoke`. `just ci` runs the first three. Render tests skip
with a stderr note when no GPU adapter exists.

## 6. UI/UX Architecture and Menu Bar

The shell follows the Craft 8-menu bar. The bus is `dispatch(op, args)`. There is no `Command` enum. To add a tool: a View or Window toggle when it is a panel, a toolbox or inspector control, then a real op.

- `File`: New Synthetic Scene (status text), Open Splat PLY/SPZ (`Ctrl+O`, loads the viewport), Open Recent, Save Converted Copy (`Ctrl+S`, runs `convert`), Save As (status text), Export Still PNG (`Ctrl+E`, runs `render`), Export Video (status text: PNG stills only), Close Scene (`Ctrl+W`), Quit (`Ctrl+Q`).
- `Edit`: Undo (`Ctrl+Z`) and Redo (`Ctrl+Y`) restore path fields and the result text. Cut and Copy move the result text. Paste Path is status text. Select All Panels shows the docks. Preferences (`Ctrl+,`).
- `View`: toggles for the viewport, toolbox, inspector, engines, training jobs, and Marble worlds. Reset Camera. Theme Dark or Light.
- `Insert`: Import Additional Splat, Load Camera Path, and Add Training Footage set status text. Add Bounding Crop Box appends a crop step.
- `Modify`: Crop, Prune, Transform, and Reduce SH append steps. Clear Edit Steps empties the list. Run Edit Pipeline calls `edit`.
- `Commands`: Run Headless Render, Plan Training Job, Convert PLY to SPZ, Build Collision Mesh (`mesh`), Recolor Grey Splats (`recolor`), Check Marble Credits, Cancel Training Job.
- `Window`: Reset Layout, plus toggles labelled 3D Viewport, Job Supervisor Drawer (the toolbox), and Inspector Dock.
- `Help`: Command Palette (`Ctrl+K`), Keyboard Shortcuts (`F1`), Project Files and Docs, Licenses and Notices, About SplatCraft.

Hover-switching between open menus is not implemented (`switch_on_hover_note` is empty). `--smoke` checks that `mesh`, `recolor`, and `recipe` are on the bus.
