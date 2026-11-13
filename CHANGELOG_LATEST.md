# splatcraft v0.1.0

First release. Pure-Rust Gaussian splat toolkit on the ArtCraft 3-tier pattern (core, MCP, app).
Windows x64 only for now.

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
- No real video-to-splat run has completed yet; the pipeline is tested with real processes and
  planned against real Brush 0.3.0 and COLMAP 4.2.1, but not run end to end.
- SH rotation is not implemented; rotating splats with SH degree > 0 is refused.
- Viewport mouse feel and surface colour have not been judged by eye.
- External engines (ffmpeg, COLMAP, Brush) are not bundled. Point `SPLATCRAFT_TOOLS` at them.
- No installer is produced yet. `scripts/release.ps1` in this repo was cut down
  and explicitly skips NSIS, even though `craft.config.psd1` sets `Installer = $true`
  and `installer/craft-installer.nsi` is present. `splatcraft.exe` (app) and
  `splatcraft-cli.exe` (CLI and MCP server) are attached as plain assets; the
  `.mcpb` bundle wraps `splatcraft-cli.exe`.
  `splatcraft-0.1.0-windows-x64-portable.zip` holds the same two exes and exists only so the
  craft launcher can install the app.
