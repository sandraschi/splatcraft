# splatcraft v0.1.0

First release. Pure-Rust Gaussian splat toolkit on the ArtCraft 3-tier pattern (core, MCP, app).
Windows x64 only for now.

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
- No installer yet. `splatcraft.exe` (app) and `splatcraft-cli.exe` (CLI and MCP server) are
  attached as plain assets; the `.mcpb` bundle wraps `splatcraft-cli.exe`.
