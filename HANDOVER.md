# HANDOVER - splatcraft

Written for the next agent. Plain markdown, no assumed context.

**Last touched:** 2026-11-13
**Trunk:** `master`, code commit `29b7fb7` (Tier 5 mesh, recolor, recipe).

## What was being worked on

Bringing the L3 desktop app up to Craft app standards. The app was a runt: five
buttons (`Load / Inspect / Convert / Engines / Status`) in a single
`CentralPanel`, while the PRD promised a full Pattern 2 workstation and the
Tiered Plan had all six Tier 4 boxes unchecked.

## Last concrete action

Rewrote `crates/splatcraft-app/src/app.rs` and `main.rs`:

- Universal 8-menu bar (`File Edit View Insert Modify Commands Window Help`),
  each item calling `splatcraft_core::dispatch`.
- 5-dialog Help suite (Command Palette `Ctrl+K`, Shortcuts `F1`, Project Docs,
  Licenses, About) plus Preferences.
- Left Toolbox: inspect, convert, synth, edit-pipeline builder (crop / prune /
  transform / reduce_sh, visible step list), merge, render.
- Right Inspector: scene density + SH telemetry, engines probe, training jobs
  (plan/start/status/log/cancel/list), Marble Worlds (ledger/credits/generate/
  status/fetch/list).
- `harsh_visuals` / `crisp_light_visuals`, vector sun/moon toggle, `Ctrl+wheel`
  zoom, `Ctrl+O/S/E/K/Z/Y`.
- `--smoke` extended: menu-dispatch parity, tofu guard, and an end-to-end
  synth -> convert -> inspect -> edit -> merge -> worlds-ledger run.
- New tests: `glyph_allowlist`, `check_string_for_tofu`.

Gates run and green on 2026-11-13: `cargo fmt --all --check`, `cargo clippy
--workspace --all-targets --all-features -- -D warnings`, `cargo test
--workspace` (all suites), `cargo build --release --workspace`,
`target\release\splatcraft.exe --smoke` -> `smoke: PASS` (render produced a real
PNG on this machine's GPU).

Tier 5 is commit `29b7fb7`: `mesh`, `recolor`, `recipe`,
toolbox and Commands entries, and a fourth democraft demo `recolor-grey`.
Checked with `cargo fmt --all --check`, clippy `-D warnings` on the touched
crates, and `cargo test` for splat-ops, splatcraft-core, splatcraft-mcp, and
the app. A full workspace clippy and release smoke were not re-run after Tier 5.

## Next step

Tier 5 is in the tree: `mesh` (GLB box), `recolor` (ghibli / cyberpunk / ova plus
`*.texturecraft.json`), and `recipe info|convert|collider|recolor`. Democraft's
splat demos call those recipe verbs and write into the demo directory. NSIS stays
out. PRD section 6 still describes a celcraft document menu. Eye-check the
viewport, and run one real training clip, when those are the job.

## Deliberately not done

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
- README.md carries a pre-existing uncommitted badge block (CRLF warning). It
  was left untouched on purpose.
