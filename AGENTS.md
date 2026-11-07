# splatcraft agent rules

Pure-Rust ArtCraft-pattern app for Gaussian splats. Three tiers, one bus.

## Line endings: LF ONLY

Every file in this repo uses LF line endings. Never write CRLF.
`.gitattributes` enforces `eol=lf`, but the Windows host has `core.autocrlf=true`,
so tools that write files must emit LF. After bulk writes, check:
`git ls-files --eol` must show `w/lf` for every text file.

## Layout

| Crate | Tier | Role |
|---|---|---|
| `splat-formats` | lib | PLY and SPZ codecs, stats, synthetic data. No I/O policy. |
| `splat-ops` | lib | Pure edit ops: crop, prune, transform, merge, reduce_sh |
| `splat-view` | lib | Headless wgpu render via wgpu-3dgs-viewer 0.8 (wgpu 30, same as egui-wgpu 0.37) |
| `splatcraft-core` | L1 | `dispatch(op, &Value) -> Result<Value>`; paths, engines probe, ops |
| `splatcraft-mcp` | L2 | `splatcraft-cli` binary: clap CLI plus MCP 2024-11-05 stdio |
| `splatcraft-app` | L3 | egui desktop host, calls `dispatch` in-process, `--smoke` |

CLI, MCP and app share `dispatch`. Never put logic in a front end.

## Rules

- Workspace lints deny `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `todo`.
  Tests may `#![allow]` unwrap/expect.
- stdout of the MCP server is 100% JSON-RPC. Logs go to stderr.
- Errors are `{success:false,error,error_type}`. No silent fallbacks.
- String args capped at 4096 chars. If `SPLATCRAFT_ROOT` is set, paths must stay inside it.
- Hostile input: validate sizes against real byte length before allocating.
- No stubs presented as features. Unsupported formats return `unsupported` (SPZ v4 does today).
- No mocks that hide missing behaviour. `synth` data is labelled synthetic.
- No em dashes in docs, code or commit messages.
- Realistic AI-assisted timelines (days, not weeks) in all docs.

## Gates (all must be green before commit)

`just fmt-check`, `just check` (clippy `-D warnings`), `just test`, `just build-release`,
then `splatcraft --smoke`. `just ci` runs the first three.

## Git

Work on the existing branch. Do not create branches for routine work. One commit per
crate or concern, small. Do not push without being asked.

## Lineage

Replaces the earlier Python splatcraft (removed, nothing reused). Absorbs over time the
functions of `worldlabs-mcp` (Marble world generation) and `splatmaker-mcp` (video and
images to splat); those repos stay untouched. SPZ layout was verified against the
Apache-2.0/MIT `spz` crate source as reference only; no code copied.

## Known facts worth remembering

- PLY is RDF (right, down, front); SPZ is RUB. `Splats::convert_coords` flips y and z.
  The reader sets `coords`, the writers convert as their format requires.
- PlayCanvas `splat-transform` 3.10 reads our SPZ but does not apply the RUB to RDF flip
  when writing PLY (y and z come out negated versus ours). We follow the reference spz
  converter. Do not "fix" this to match splat-transform without a new decision.
- `edit` and `merge` work in RDF. SH rotation is not implemented; transforms that rotate
  splats with SH degree > 0 return `unsupported` instead of faking it.
- PowerShell strips inner double quotes from native args; pass JSON via `@file.json`.
- Render tests need a GPU; they skip with a stderr note when no adapter exists.
- SPZ is lossy by design (8-bit colour, 24-bit fixed positions, quantised SH).
