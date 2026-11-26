# `splatcraft-app` (L3 Native egui Desktop Host)

Native `eframe`/`egui` desktop host calling `splatcraft-core` in-process, featuring a live `egui-wgpu` Gaussian splat viewport (orbit drag, wheel zoom, double-click reset) sharing `splat-view`'s `LiveScene` pipeline.

```powershell
cargo run -p splatcraft-app -- --smoke
```
