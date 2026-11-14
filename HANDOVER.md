# HANDOVER - splatcraft

Written for the next agent. Plain markdown, no assumed context.

**Last touched:** 2026-11-14
**Trunk:** `master`, docs commit `1e3a85a` (Tier 5 recorded, menu bar matches the app). Code commit `29b7fb7` (mesh, recolor, recipe). The houseplant still is a docs addition on top of that, not yet committed.

## What was being worked on

Tier 5 is in the tree and pushed. The follow-up was a visible splat: the 24-splat recipe fixtures were rejected as too small. A published real capture was converted and rendered on the GPU.

## Last concrete action

2026-11-14, release CLI (the sandbox build, not `D:\Dev\repos\splatcraft\target\release`):

- Downloaded Marcel Padilla's houseplant capture, CC-BY-4.0, `source_method: capture`, 113,648 splats, SH degree 0. File `plant.splat` is 3,636,736 bytes (antimatter15 layout, 32 bytes per gaussian) from `github.com/marcelpadilla/splats`.
- Converted it to a degree-0 3DGS PLY: log scales, SH DC from RGB, opacity logit, quaternion `rot_0..3`. Positions were left in the capture frame. Negating Y put the pot on the ceiling; the renderer already shows this frame upright.
- `inspect`: 113,648 splats, 0 non-finite, mean opacity 0.245, bbox about 0.52 x 1.00 x 0.61.
- `render` 1280x960, opaque black, auto-framed. Front yaw 18 pitch -8, covered_fraction 0.291, 499,032 bytes. Quarter yaw 55 pitch -6, covered_fraction 0.278.
- Front still copied to `docs/screenshots/plant-front.png`. Ply and quarter still stay at `D:\Dev\tools\splatcraft\scans\plant\`.
- Engines with `SPLATCRAFT_TOOLS=D:\Dev\tools\splatcraft`: brush `D:\Dev\tools\splatcraft\brush\brush_app.exe` up, colmap `D:\Dev\tools\splatcraft\colmap\bin\colmap.exe` up, ffmpeg and bunx up, ns-train and splat-transform down.
- CLI used: `C:\Users\sandr\AppData\Local\Temp\cursor-sandbox-cache\0ec55dc8eff576534c15bba334c805fe\cargo-target\release\splatcraft-cli.exe`.

Release build and `--smoke` on that sandbox target passed earlier the same night (13 ops, real PNG). `D:\Dev\repos\splatcraft\target\release\splatcraft.exe` is older (2026-11-13) and still reports 10 ops. Do not smoke that copy.

## Next step

Eye-check the viewport against `docs/screenshots/plant-front.png`: load `D:\Dev\tools\splatcraft\scans\plant\plant.ply`, reset camera, compare orbit and colour. A local training run still needs an orbit photo set. The YCB clamp zip that was sized (`19_large_clamp.zip`) is 2.4 GB; do not start it without a size decision. Brush on a 4070 Ti class GPU is on the order of tens of minutes at 30k steps.

## Deliberately not done

- **No new scan trained here.** The stills are a published capture rendered locally. COLMAP and Brush were probed, not launched.
- **No fal.ai integration.** Rejected on purpose. Marble returns Gaussian
  splats; fal.ai's 3D endpoints are mesh generators (TRELLIS/Hunyuan3D class).
  The zero-marginal-cost path already exists locally in `splat-train`
  (ffmpeg + COLMAP + Brush on the RTX 4090) and in the sibling
  `splatmaker-mcp`. A third paid backend with no splat-native output is not
  worth the key management or the spend surface.
- **No video export.** `crates/splatcraft-core/src/render.rs` writes PNG stills
  only. The File menu item says so explicitly instead of pretending.
- **No live Marble call.** `splat-worlds` is still tested against the fake
  transport only. `worlds generate` stays a dry run unless `confirm_spend` is
  true, and the UI never sets it.
- **No SH rotation.** Rotating splats with SH degree > 0 is still refused; run
  `reduce_sh` to 0 first. Surfaced as a hint in the Toolbox, not hidden.
- **Democraft edits are uncommitted** on `agent/opencode-stripe-reconcile`
  (`src/democraft_mcp/crafts.py`, `recipes.py`: recipe `--out` writes into the
  demo dir, plus a `recolor-grey` demo). Do not commit them onto that branch.
  `main` is a different history.

## Traps

- The GPU viewport needs the adapter's real limits. `main.rs` overrides
  eframe's device descriptor; do not remove it or large scenes fail on the
  128 MiB storage buffer limit.
- `LiveScene::encode_compute` must be recorded before the render pass that
  calls `draw` (see `SplatCallback::prepare` / `paint`).
- egui 0.37: panels take `&mut Ui` (`Panel::left(...).show(ui, ..)`), not
  `&Context`; `Context::style_mut` does not exist (`style_mut_of(theme, ..)`).
- Workspace clippy denies `unwrap_used`, `expect_used`, `panic`. Tests may
  `#![allow]` them locally - the app test module does.
- Every file is LF. `.gitattributes` enforces it; check `git ls-files --eol`
  shows `w/lf` after any bulk write.
- `SPLATCRAFT_TOOLS` must be set. The default `./tools` does not see
  `D:\Dev\tools\splatcraft`, so brush and colmap report down while the exes
  are on disk.
- PowerShell eats a bare negative number: `--pitch-deg -8` fails as an
  unexpected argument. Pass `--pitch-deg=-8`.
- Native formats are `.ply` and `.spz` only. An antimatter15 `.splat` has to
  be converted first. Do not negate Y for this houseplant; the renderer
  already frames it upright.
- The app package name is `splatcraft`, not `splatcraft-app`.
