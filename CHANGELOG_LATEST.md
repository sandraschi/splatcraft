# splatcraft v0.1.0

First release. Pure-Rust Gaussian splat toolkit on the ArtCraft 3-tier pattern (core, MCP, app).
Windows x64 only for now.

## Real capture still (2026-11-14)

- Headless `render` drew Marcel Padilla's houseplant capture (CC-BY-4.0, 113,648 splats, SH degree 0) to a 1280x960 PNG. Front view yaw 18, pitch -8, covered_fraction 0.291. The still is `docs/screenshots/plant-front.png`.
- The ply and a second angle stay outside the repo at `D:\Dev\tools\splatcraft\scans\plant\` (`plant.ply`, `plant-quarter.png`). The source `.splat` is the antimatter15 32-byte layout, converted to a degree-0 3DGS PLY. Positions were left in the capture frame.
- This is a published capture rendered locally. No new COLMAP or Brush training run was started.
- With `SPLATCRAFT_TOOLS=D:\Dev\tools\splatcraft`, brush and colmap report up. ffmpeg and bunx are up. ns-train and splat-transform are down.

## Tier 5 mesh, recolor, recipes (2026-11-13)

- `mesh` writes a GLB box collider from the finite positions in a `.ply` or `.spz` file. Marble's downloaded collider is unchanged.
- `recolor` tints near-grey SH DC with `ghibli`, `cyberpunk`, or `ova`, clears higher SH bands on those splats, and writes a `*.texturecraft.json` bridge beside the output.
- `recipe info|convert|collider|recolor` runs the offline democraft demos into a directory.
- CLI: `mesh`, `recolor`, `recipe`. MCP: `splat_mesh`, `splat_recolor`, `splat_recipe`.
- Toolbox section and Commands menu entries call the same bus.

## Tier 4 desktop workstation (2026-11-13)

- Universal 8-menu bar per `CRAFT_MENU_BAR_STANDARD.md`: `File`, `Edit`, `View`,
  `Insert`, `Modify`, `Commands`, `Window`, `Help`. Every menu item calls
  `splatcraft_core::dispatch`; there is no separate UI code path.
- Mandatory 5-dialog Help suite: Command Palette (`Ctrl+K`, fuzzy command filter
  that runs on Enter), Keyboard Shortcuts (`F1`), Project Files and Docs,
  Licenses and Notices, About SplatCraft. Plus a Preferences dialog (`Ctrl+,`).
- Pattern 2 docks: left Toolbox (inspect, convert, synth, edit-pipeline builder
  for crop/prune/transform/reduce_sh with a visible step list, merge, render) and
  right Inspector (density + SH telemetry, engines probe, training jobs incl.
  plan/start/status/log/cancel/list, Marble Worlds ledger/credits/generate/
  status/fetch/list). Panel visibility is managed from `View` and `Window`.
- Quick-button row maps to the same commands; status bar shows the last action.
- `harsh_visuals` / `crisp_light_visuals` with a vector sun/moon theme toggle,
  `Ctrl+wheel` zoom, `Ctrl+O/S/E/K/Z/Y` shortcuts.
- `--smoke` now asserts menu-dispatch parity, the tofu guard, and an end-to-end
  synth -> convert -> inspect -> edit -> merge -> worlds-ledger run.
- New tests: `glyph_allowlist` (fails the build on any non-ASCII glyph outside
  the verified safe set) and `check_string_for_tofu`.

## What is in it

- PLY read/write (SH 0 to 3), SPZ v2/v3 read, SPZ v3 write. SPZ v4 is rejected with a clear error.
- `inspect`, `convert`, `edit` (crop, prune, transform, reduce_sh), `merge`, headless GPU `render`.
- `worlds`: World Labs Marble client with a dry-run-by-default spend guard, JSONL ledger and a
  cumulative credit cap.
- `train`: video, image folder or COLMAP dataset to splat through ffmpeg, COLMAP and Brush, run
  by a detached supervisor with a job ledger, cancel and timeouts.
- MCP stdio server, CLI and an egui desktop app with a live orbit viewport.
- API keys (World Labs) read from an env var or the Windows Credential Manager via the
  `craft-secrets` crate. `splatcraft-cli secret set worlds` stores a key with hidden input,
  `secret status` reports where it comes from. The key is never returned or logged.

## Known limits (read before relying on it)

- No live Marble call has been made; the client is tested against a fake transport only.
- A published real capture has been rendered to PNG (see above). No local video-to-splat
  training run has completed; the pipeline is tested with real processes and planned against
  real Brush 0.3.0 and COLMAP 4.2.1, but not run end to end.
- SH rotation is not implemented; rotating splats with SH degree > 0 is refused.
- Viewport mouse feel and surface colour have not been judged by eye.
- External engines (ffmpeg, COLMAP, Brush) are not bundled. Point `SPLATCRAFT_TOOLS` at them.
- The release is `splatcraft.exe`, `splatcraft-cli.exe`, the portable zip, and the
  `.mcpb` bundle. There is no NSIS setup.exe. The app does not need an installer.
