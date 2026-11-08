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
| `splat-view` | lib | `LiveScene` (GPU upload, camera, compute, draw) via wgpu-3dgs-viewer 0.8 (wgpu 30, same as egui-wgpu 0.37); headless `render` and the egui viewport both use it |
| `splat-worlds` | lib | Marble API client behind a `Transport` trait, JSONL spend ledger, credit cap (reqwest blocking, rustls) |
| `craft-secrets` | lib | Env var first, then OS credential store (keyring). No splatcraft dependencies; shared pattern for craft apps, see docs/SECRETS_STANDARD.md |
| `splat-train` | lib | Tool locator, `Probe` trait (reads `--help`), pure stage planner, job store, detached supervisor |
| `splatcraft-core` | L1 | `dispatch(op, &Value) -> Result<Value>`; paths, engines probe, ops |
| `splatcraft-mcp` | L2 | `splatcraft-cli` binary: clap CLI plus MCP 2024-11-05 stdio; hidden `job-run` supervisor entry |
| `splatcraft-app` | L3 | egui desktop host, calls `dispatch` in-process, `--smoke` |

CLI, MCP and app share `dispatch`. Never put logic in a front end.
Ops: status, engine_status, inspect, convert, synth, edit, merge, render, worlds, train.

## Rules

- Workspace lints deny `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `todo`.
  Tests may `#![allow]` unwrap/expect.
- stdout of the MCP server is 100% JSON-RPC. Logs go to stderr.
- Errors are `{success:false,error,error_type}`. No silent fallbacks.
- String args capped at 4096 chars. If `SPLATCRAFT_ROOT` is set, paths must stay inside it.
- Hostile input: validate sizes against real byte length before allocating.
- No stubs presented as features. Unsupported formats return `unsupported` (SPZ v4 does today).
- No mocks that hide missing behaviour. `synth` data is labelled synthetic.
- Secrets: never read, print, log, store or ask for an API key value. No MCP tool or `dispatch` op may expose secrets; only `splatcraft-cli secret set|status|delete`, typed by the owner, may write them. If a key is needed, tell the owner to run `splatcraft-cli secret set worlds`.
- No em dashes in docs, code or commit messages.
- Realistic AI-assisted timelines (days, not weeks) in all docs.

## Gates (all must be green before commit)

`just fmt-check`, `just check` (clippy `-D warnings`), `just test`, `just build-release`,
then `splatcraft --smoke`. `just ci` runs the first three.

## Git and branch hygiene

Single trunk: `master`. This repo must never grow a pile of agent branches (that happened
in gamecraft and must not recur).

- Work directly on `master`. Do not create branches, worktrees or stashes for routine work.
  One commit per crate or concern, small, gates green.
- If parallel agents are genuinely needed: one short-lived branch per agent, named
  `agent/<task>`, created from current `master`, rebased onto `master`, merged (fast-forward
  or squash) and deleted locally and on the remote the same day. Never leave one overnight
  without telling the owner. Agents never share a branch.
- No WIP commits left behind. Either finish and commit with green gates, or discard
  (`git restore`), or tell the owner exactly what is unfinished. No `wip/`, `tmp/`, `backup/`
  or `old/` branches, no tags as bookmarks, no `git stash` as storage.
- Start of every session: `git status --short`, `git branch -a`, `git stash list`,
  `git worktree list`, and `git log origin/master..HEAD --oneline`. If anything other than
  clean `master` shows up, report it before starting new work.
- End of every session: working tree clean, only `master` exists (local and remote), no
  stashes. Report the count of commits not yet pushed.
- Do not push without being asked. Never force-push `master`.
- Housekeeping: `git fetch --prune`; delete merged branches with `git branch -d` (no `-D`
  without the owner's say-so) and `git push origin --delete <name>`.

## Install requests

If asked to install splatcraft (for example "install splatcraft from github.com/sandraschi/splatcraft"), follow the "Agent install steps" in INSTALL.md exactly. Keep INSTALL.md in step with the release script and README whenever install paths, env vars or artifacts change.

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
- Marble spend is real money. `worlds generate` must stay a dry run unless `confirm_spend`
  is true, and the ledger entry is written only after a successful POST. Never run a
  confirmed generate without the owner's explicit say-so. Never print or return the API
  key (`WORLDLABS_API_KEY`). Tests use the fake transport (`splat-worlds` feature `testing`),
  never the network. Tests that touch `SPLATCRAFT_DATA` or the cap env serialize on a mutex
  (`crate::ENV_LOCK` in core).
- The viewport needs the adapter's real limits (`main.rs` overrides eframe's default device
  descriptor); with wgpu defaults, large scenes fail on the 128 MiB storage buffer limit.
  `LiveScene::encode_compute` must be recorded before the render pass that calls `draw`.
- SPZ is lossy by design (8-bit colour, 24-bit fixed positions, quantised SH).
- Training: tool flags are discovered from `--help` (`Probe`), never assumed. Brush v0.3.x
  uses `--total-steps`, newer builds `--total-train-iters`. The supervisor runs no shell.
  Tool lookup: env override (`SPLATCRAFT_<TOOL>`, wrong override means not found), PATH, then
  `SPLATCRAFT_TOOLS`. Job state lives in `<data>/jobs/<id>/` (`job.json`, `log.txt`, `cancel`)
  plus `<data>/jobs.jsonl`; heartbeat 5 s, stale after 45 s means `lost`. Tests that spawn
  tools point the tool env vars at nonexistent files.
- Downloaded engines live outside the repo (Goliath: `D:\Dev\tools\splatcraft`, set
  `SPLATCRAFT_TOOLS`). Brush ships as `brush_app.exe`. Never commit binaries.
- Generated or animated clips make poor COLMAP input; use steady orbit footage of a real
  object. `check_sparse` fails with a hint when COLMAP finds no model.
