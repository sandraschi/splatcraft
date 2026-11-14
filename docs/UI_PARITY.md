# SplatCraft UI Parity

**Stack:** `egui` viewport in L3 `splatcraft-app` drawing through the same
`LiveScene` as headless `render` (egui-wgpu paint callback, wgpu-3dgs-viewer
0.8). No WebView2, no HTTP sidecar.

## 1. Oracle loop

The headless `render` PNG is the reference; the live viewport must match it
for the same scene and camera.

1. `render` a fixture splat headless (auto-framed orbit camera, opaque
   black background).
2. Load the same path in the app viewport, reset camera (double-click),
   screenshot the window surface.
3. Compare: orbit direction, zoom speed, and color on the surface are the
   known unjudged items (per README); fix `LiveScene` wiring, not the PNG.
4. Lock with `egui_kittest` goldens once mouse feel and color are judged
   by eye.

## 2. L3 requirements

- `--smoke` headless CI mode (exits 0, no window).
- Golden PNGs for: loaded splat orbit view, empty state (no path), engine
  panel (`engine_status` up/down rows), train job row
  (planning/running/lost/not_started).
- Adapter limits come from the real adapter (`main.rs` device descriptor
  override); the 128 MiB storage buffer default must not silently clip
  large scenes. `encode_compute` is recorded before the render pass that
  calls `draw`.
- Viewport needs the adapter's real limits; tests that need a GPU skip
  with a stderr note when no adapter exists.
- L3 never shells out to locate engines; lookup order is env override
  (`SPLATCRAFT_<TOOL>`), PATH, then `SPLATCRAFT_TOOLS`.

## 3. Current status

Viewport compiles, app starts on the wgpu backend, shared scene path
passes the GPU render test. Headless `render` of a real 113,648-splat
capture is `docs/screenshots/plant-front.png` (1280x960, yaw 18, pitch -8,
opaque black, covered_fraction 0.291). Mouse feel and window-surface
color are not yet eye-judged against that still. This file is the bar
that judgment must clear.

## 4. Screenshots

`docs/screenshots/`:

- `plant-front.png` - headless `render` of the published houseplant
  capture. Source ply is `D:\Dev\tools\splatcraft\scans\plant\plant.ply`.
- `viewport-orbit.png` - loaded splat, reset camera. Not taken yet.
- `engines-panel.png` - `engine_status` rows for brush, ns-train, colmap,
  ffmpeg, splat-transform, bunx. Not taken yet.
