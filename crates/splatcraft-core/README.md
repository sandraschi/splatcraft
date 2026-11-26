# `splatcraft-core` (L1 Unified Dispatch Bus)

Single L1 command dispatch bus shared by `splatcraft-mcp` (CLI + MCP stdio) and `splatcraft-app` (egui desktop): routes `inspect`, `convert`, `edit`, `merge`, `render`, `worlds`, `train`, `engine_status`, `mesh`, `recolor`, `recipe`, `trellis`, and `synth` with `SPLATCRAFT_ROOT` path confinement.
