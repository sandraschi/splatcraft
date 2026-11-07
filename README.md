# splatcraft

Pure-Rust toolkit for Gaussian splats, built on the ArtCraft 3-tier pattern.

**Status: Day 2.** What works today, and nothing else is claimed:

- Read and write binary PLY (3DGS layout, SH degree 0 to 3).
- Read SPZ v2 and v3, write SPZ v3. SPZ v4 (zstd) is rejected with a clear error.
- `inspect`: count, bounding box, SH degree, opacity and scale stats, non-finite count.
- `convert`: PLY <-> SPZ, atomic write, no overwrite unless asked.
- `engine_status`: honest up/down for brush, ns-train, colmap, ffmpeg, splat-transform, bunx
  (PATH probe only, nothing is executed).
- `edit`: ordered step pipeline: `crop` (box, optional invert), `prune` (min opacity, max scale,
  non-finite), `transform` (translate, uniform scale, axis-angle rotation), `reduce_sh`.
  Edits run in RDF. Rotating splats that carry SH degree > 0 is refused, because SH rotation
  is not built; run `reduce_sh` to degree 0 first.
- `merge`: 2 to 64 files, mixed PLY/SPZ, frames unified, SH reduced to the lowest degree.
- `render`: headless GPU PNG still (wgpu-3dgs-viewer, auto-framed orbit camera, opaque black
  background). Reports `unsupported` when no GPU adapter exists.
- MCP stdio server and CLI over one dispatch bus; egui host with `--smoke`.

Not built yet: live egui viewport, training, world generation (Marble), collision mesh,
texturecraft recolor of monochrome splats, SH rotation. See the plan below.

## Use

```
just ci                                   # fmt, clippy, tests
cargo run -p splatcraft-mcp -- inspect --path scene.ply
cargo run -p splatcraft-mcp -- convert --input scene.ply --output scene.spz
cargo run -p splatcraft-mcp -- engines
cargo run -p splatcraft-mcp -- edit --input a.ply --output b.spz --steps "@steps.json"
cargo run -p splatcraft-mcp -- merge --inputs a.ply b.spz --output m.ply
cargo run -p splatcraft-mcp -- render --input a.ply --output a.png --yaw-deg 30
cargo run -p splatcraft-mcp -- mcp        # MCP stdio server
cargo run -p splatcraft --bin splatcraft  # desktop host
```

`--steps` takes inline JSON or `@file.json` (use the file form in PowerShell, which strips
inner double quotes).

MCP tools: `splat_inspect`, `splat_convert`, `splat_edit`, `splat_merge`, `splat_render`,
`splat_engine_status`, `splat_synth`.
Set `SPLATCRAFT_ROOT` to confine all paths to a directory.

## Plan (AI-assisted days)

| Day | Scope |
|---|---|
| 1 | Workspace, PLY and SPZ codecs, inspect, convert, engine probe, gates (done) |
| 2 | Edit ops (crop, transform, merge, prune), headless render (done); live egui viewport next |
| 3 | Marble client (absorbs worldlabs-mcp), spend ledger |
| 4 | Brush trainer adapter, ffmpeg and COLMAP prep, job ledger, first video-to-splat run |
| 5 | Collision mesh, texturecraft bridge spike |
| 6 to 7 | Recolor of grey splats, democraft recipes, installer |

## License

MIT OR Apache-2.0.
