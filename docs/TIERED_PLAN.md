# SplatCraft — Tiered Execution Plan

**Parent Standard:** [`RUST_ARTCRAFT_STANDARD.md`](../../mcp-central-docs/standards/RUST_ARTCRAFT_STANDARD.md)  
**Menu Bar Standard:** [`CRAFT_MENU_BAR_STANDARD.md`](../../mcp-central-docs/standards/rust-artcraft/CRAFT_MENU_BAR_STANDARD.md)  
**Architecture Model:** 4-Tier ArtCraft Incremental Delivery  
**Status:** Active  

---

## Architecture Flow

```
Tier 1: Core Engine ──> Tier 2: MCP / CLI ──> Tier 3: Desktop UI ──> Tier 4: Menu Bar, Help & Packaging
(splatcraft-core)      (splatcraft-mcp)      (splatcraft-app)      (Hardened Workstation)
```

---

## Tier Specifications

### Tier 1 — Core Splat Codecs & Ops (`splat-formats`, `splat-ops`, `splatcraft-core`)
- [x] L1 `Command::dispatch` universal 3DGS execution bus.
- [x] Binary PLY reader/writer (3DGS layout, SH degree 0-3).
- [x] SPZ v2/v3 compressed format support.
- [x] Geometric operations: crop bounding box, prune opacity, affine transforms, merge.
- [x] Headless GPU PNG rendering via `LiveScene` (`wgpu-3dgs-viewer`).
- [x] Training job supervisor (ffmpeg, COLMAP, Brush flag discovery).
- [x] `#![forbid(unsafe_code)]` and zero-panic clippy lints.

### Tier 2 — Headless Driver & FastMCP (`splatcraft-mcp`)
- [x] Unified CLI `splatcraft-cli` with `--json` output.
- [x] FastMCP stdio server communicating over JSON-RPC 2.0.
- [x] Marble client integration with fake-transport test harnesses and spend guards.
- [x] Headless `--smoke` verification without window initialization.

### Tier 3 — Desktop Workstation UI (`splatcraft-app`)
- [x] Immediate-mode `egui` 0.37 / `wgpu` workstation.
- [x] Interactive 3D Gaussian Splatting orbit viewport.
- [x] Splat density and spherical harmonics telemetry inspectors.

### Tier 4 — Craft Menu Bar Standard, Help Suite & Hardened Verification
- [ ] Universal 8-menu bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`) with `switch_on_hover`.
- [ ] Mandatory 5-dialog Help suite (`Command Palette...`, `Keyboard Shortcuts...`, `Project Files & Docs...`, `Licenses & Notices...`, `About SplatCraft`).
- [ ] Fleet UI/UX Pattern 2 adherence: viewport-centric navigation, top button row mapped to View toggles, panel visibility managed via `View` / `Window`.
- [ ] Tofu Guard verification (100% font glyph coverage, zero tofu placeholder boxes).
- [ ] Headless `--smoke` parity validation.
- [ ] Standalone release binary packaging and NSIS installer.
